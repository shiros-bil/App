// OpenGL ES 3 renderer (glow): background, region fills with zone override, borders, water, overlay batch.
use crate::world::{FillMesh, LineMesh, World, GH, GW, NC};
use glow::HasContext;

pub const TW: usize = 4096;

pub struct Prog {
    pub p: glow::Program,
    pub u: std::collections::HashMap<String, glow::UniformLocation>,
}
impl Prog {
    pub fn u(&self, n: &str) -> Option<&glow::UniformLocation> {
        self.u.get(n)
    }
}

pub struct GFill {
    pub vao: glow::VertexArray,
    pub fr: Vec<[u32; 2]>,
    pub fbb: Vec<[f32; 4]>,
}
pub struct GLine {
    pub vao: glow::VertexArray,
    pub sb: glow::Buffer,
    pub lb: glow::Buffer,
    pub rg: Vec<[u32; 2]>,
    pub bb: Vec<[f32; 4]>,
    pub an: i32,
}
pub struct GLod {
    pub fill: GFill,
    pub line: GLine,
    pub line2: GLine,
}

pub struct Renderer {
    pub gl: glow::Context,
    pub fill: Prog,
    pub line: Prog,
    pub img: Prog,
    pub wfill: Prog,
    pub wline: Prog,
    pub ov: Prog,
    pub quad: glow::Buffer,
    pub seg: glow::Buffer,
    pub lods: Vec<GLod>,
    pub lakes: Vec<GFill>,
    pub rivers: Vec<GLine>,
    pub lake_ol: Vec<GLine>,
    pub water_rg: Vec<i32>,
    pub col_t: glow::Texture,
    pub own_t: glow::Texture,
    pub zid_t: glow::Texture,
    pub zin_t: glow::Texture,
    pub zf_t: glow::Texture,
    pub th: usize,
    pub bg_t: Option<glow::Texture>,
    pub tiles: Vec<Option<glow::Texture>>,
    pub atlas_t: glow::Texture,
    pub ov_vao: glow::VertexArray,
    pub ov_vbo: glow::Buffer,
    pub zon: bool,
    pub z_alloc: bool,
    pub anx: Vec<Option<GLine>>,
    pub pc_t: Option<glow::Texture>,
    pub pc_dim: [usize; 2],
}

const HDR: &str = "#version 300 es\nprecision highp float;precision highp int;\nuniform vec2 uRes;uniform float uK;uniform vec2 uOff;\nvec4 toClip(vec2 s){vec2 c=s/uRes*2.0-1.0;return vec4(c.x,-c.y,0.,1.);}\nivec2 rid(int i){return ivec2(i%4096,i/4096);}\n";

fn zlk() -> String {
    let (sx, sy) = (GW as f32 / 2000.0, GH as f32 / 1000.0);
    format!(
        "uniform highp usampler2D uZid;uniform highp usampler2D uZin;uniform sampler2D uZf;uniform int uZon;\n\
uvec4 zoneNear(vec2 m,int reg,int st,bool real){{ivec2 ci=ivec2(clamp(m*vec2({sx:.4},{sy:.4}),vec2(0.),vec2({gw}.,{gh}.)));\n\
  for(int k=0;k<9;k++){{ivec2 o=ivec2(k%3-1,k/3-1);uint zr=texelFetch(uZid,clamp(ci+o*st,ivec2(0),ivec2({gw},{gh})),0).r,z=zr&32767u;if(real&&zr>=32768u)continue;\n\
    if(z>0u){{uvec4 f=texelFetch(uZin,ivec2(int(z)%4096,int(z)/4096),0);if(int(f.r)==reg)return f;}}}}return uvec4(99999u);}}\n",
        sx = sx,
        sy = sy,
        gw = GW - 1,
        gh = GH - 1
    )
}

unsafe fn shader(gl: &glow::Context, t: u32, src: &str) -> Result<glow::Shader, String> {
    let s = gl.create_shader(t)?;
    gl.shader_source(s, src);
    gl.compile_shader(s);
    if !gl.get_shader_compile_status(s) {
        return Err(format!("shader: {}\n{}", gl.get_shader_info_log(s), src));
    }
    Ok(s)
}
unsafe fn prog(gl: &glow::Context, vs: &str, fs: &str) -> Result<Prog, String> {
    let p = gl.create_program()?;
    let a = shader(gl, glow::VERTEX_SHADER, vs)?;
    let b = shader(gl, glow::FRAGMENT_SHADER, fs)?;
    gl.attach_shader(p, a);
    gl.attach_shader(p, b);
    gl.link_program(p);
    if !gl.get_program_link_status(p) {
        return Err(format!("link: {}", gl.get_program_info_log(p)));
    }
    let mut u = std::collections::HashMap::new();
    let n = gl.get_active_uniforms(p);
    for i in 0..n {
        if let Some(au) = gl.get_active_uniform(p, i) {
            let name = au.name.trim_end_matches("[0]").to_string();
            if let Some(l) = gl.get_uniform_location(p, &name) {
                u.insert(name, l);
            }
        }
    }
    Ok(Prog { p, u })
}

unsafe fn tex(gl: &glow::Context, filter: i32) -> glow::Texture {
    let t = gl.create_texture().unwrap();
    gl.bind_texture(glow::TEXTURE_2D, Some(t));
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    t
}

fn f32s(v: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4) }
}
fn u32s(v: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4) }
}
pub fn u16s(v: &[u16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) }
}

unsafe fn mk_fill(gl: &glow::Context, d: &FillMesh) -> GFill {
    let vao = gl.create_vertex_array().unwrap();
    gl.bind_vertex_array(Some(vao));
    let b = gl.create_buffer().unwrap();
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(b));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, f32s(&d.pos), glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);
    let b = gl.create_buffer().unwrap();
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(b));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, f32s(&d.rid), glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 1, glow::FLOAT, false, 0, 0);
    let b = gl.create_buffer().unwrap();
    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(b));
    gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, u32s(&d.sidx), glow::STATIC_DRAW);
    gl.bind_vertex_array(None);
    GFill { vao, fr: d.fr.clone(), fbb: d.fbb.clone() }
}
unsafe fn mk_line(gl: &glow::Context, seg: glow::Buffer, d: &LineMesh) -> GLine {
    let vao = gl.create_vertex_array().unwrap();
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(seg));
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);
    let sb = gl.create_buffer().unwrap();
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(sb));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, f32s(&d.s), glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, 0, 0);
    gl.vertex_attrib_divisor(1, 1);
    let lb = gl.create_buffer().unwrap();
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(lb));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, f32s(&d.l), glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(2);
    gl.vertex_attrib_pointer_f32(2, d.an as i32, glow::FLOAT, false, 0, 0);
    gl.vertex_attrib_divisor(2, 1);
    gl.bind_vertex_array(None);
    GLine { vao, sb, lb, rg: d.rg.clone(), bb: d.bb.clone(), an: d.an as i32 }
}

pub struct View {
    pub res: [f32; 2],
    pub kd: f32,
    pub off: [f32; 2],
    pub vx0: f32,
    pub vx1: f32,
    pub vy0: f32,
    pub vy1: f32,
}

impl Renderer {
    pub unsafe fn new(gl: glow::Context, w: &World) -> Result<Renderer, String> {
        let (sx, sy) = (GW as f32 / 2000.0, GH as f32 / 1000.0);
        let fill = prog(
            &gl,
            &format!("{HDR}layout(location=0) in vec2 aP;layout(location=1) in float aR;\nuniform sampler2D uCol;out vec4 vC;out vec2 vM;flat out int vR;\nvoid main(){{gl_Position=toClip(aP*uK+uOff);vR=int(aR+.5);vC=texelFetch(uCol,rid(vR),0);vM=aP;}}"),
            &format!(
                "#version 300 es\nprecision highp float;precision highp int;in vec4 vC;in vec2 vM;flat in int vR;uniform float uA;uniform float uK;uniform int uZon;uniform vec4 uGray;\n\
uniform highp usampler2D uZid;uniform highp usampler2D uZin;uniform sampler2D uZf;out vec4 o;\n\
void main(){{vec3 c=vC.rgb;\n\
  if(uZon==1){{ivec2 ci=ivec2(clamp(vM*vec2({sx:.4},{sy:.4}),vec2(0.),vec2({gw}.,{gh}.)));uvec4 inf=uvec4(0u);bool ok=false;\n\
    for(int k=0;k<9;k++){{ivec2 o=ivec2(k%3-1,k/3-1);if(k==0)o=ivec2(0);else if(k==4)o=ivec2(-1);\n\
      uint z=texelFetch(uZid,clamp(ci+o,ivec2(0),ivec2({gw},{gh})),0).r&32767u;if(z>0u){{uvec4 f=texelFetch(uZin,ivec2(int(z)%4096,int(z)/4096),0);if(int(f.r)==vR){{inf=f;ok=true;break;}}}}}}\n\
    if(ok&&texture(uZf,vM/vec2(2000.,1000.)).r>.5){{uint g=inf.g;vec3 zc=vec3(float((g>>16)&255u),float((g>>8)&255u),float(g&255u))/255.;\n\
      if(inf.a>=2147483648u&&mod((vM.x+vM.y)*uK,18.)<7.)c=mix(c,zc,.4);else c=zc;}}}}\n\
  if(uGray.a>0.&&vC.a<.5)c=uGray.rgb;\n\
  o=vec4(c,uA);}}",
                gw = GW - 1,
                gh = GH - 1
            ),
        )?;
        let z = zlk();
        let line = prog(
            &gl,
            &format!(
                "{HDR}{z}layout(location=0) in vec2 aT;layout(location=1) in vec4 aS;layout(location=2) in vec2 aLR;\n\
uniform highp usampler2D uOwn;uniform float uWC,uWR,uAR;out vec4 vC;out vec2 vM;flat out ivec2 vLR;flat out uvec4 vO;flat out int vMode;\n\
void main(){{int l=int(aLR.x+.5),r=int(aLR.y+.5);uvec4 ol=texelFetch(uOwn,rid(l),0);\n\
  bool coast=r>=65535;uvec4 orr=coast?uvec4(99999u):texelFetch(uOwn,rid(r),0);\n\
  vLR=ivec2(l,r);vO=uvec4(ol.r,orr.r,ol.b,coast?0u:orr.b);vMode=0;\n\
  bool same=!coast&&ol.r==orr.r,grp=!coast&&ol.b!=0u&&ol.b==orr.b;\n\
  bool near=false;if(uZon==1&&!coast){{vec2 m=(aS.xy+aS.zw)*.5;ivec2 ci=ivec2(clamp(m*vec2({sx:.4},{sy:.4}),vec2(0.),vec2({gw}.,{gh}.)));for(int k=0;k<9;k++){{ivec2 o=ivec2(k%3-1,k/3-1)*3;if(texelFetch(uZid,clamp(ci+o,ivec2(0),ivec2({gw},{gh})),0).r>0u){{near=true;break;}}}}}}\n\
  float hw;vec4 col;bool war=!coast&&(ol.g!=orr.g);\n\
  if(near){{vMode=1;hw=max(uWC,uWR);col=war?vec4(1.,.27,.27,.85):vec4(0.,0.,0.,.9);}}\n\
  else if(grp){{hw=0.;col=vec4(0.);}}\n\
  else if(same){{hw=uWR;col=vec4(0.,0.,0.,uAR);}}else{{hw=uWC;col=war?vec4(1.,.27,.27,.85):vec4(0.,0.,0.,.9);}}\n\
  vec2 a=aS.xy*uK+uOff,b=aS.zw*uK+uOff;vec2 d=b-a;float L=length(d);d=L>0.?d/L:vec2(1.,0.);vec2 n=vec2(-d.y,d.x);\n\
  vec2 p=mix(a,b,aT.x)+d*(aT.x*2.-1.)*hw*.5+n*aT.y*hw;vM=mix(aS.xy,aS.zw,aT.x);\n\
  if(hw<=0.)p=vec2(-9999.);\n\
  gl_Position=toClip(p);vC=col;}}",
                gw = GW - 1,
                gh = GH - 1
            ),
            &format!(
                "#version 300 es\nprecision highp float;precision highp int;\nin vec4 vC;in vec2 vM;flat in ivec2 vLR;flat in uvec4 vO;flat in int vMode;uniform float uWR,uAR;out vec4 o;\n{z}\n\
void main(){{if(vMode==0){{o=vC;return;}}\n\
  uint oL=vO.x,oR=vO.y,gL=vO.z,gR=vO.w;\n\
  uvec4 f=zoneNear(vM,vLR.x,1,true);if(f.r!=99999u){{oL=f.b;gL=f.a&2147483647u;}}\n\
  f=zoneNear(vM,vLR.y,1,true);if(f.r!=99999u){{oR=f.b;gR=f.a&2147483647u;}}\n\
  if(gL!=0u&&gL==gR)discard;\n\
  if(oL==oR){{if(uWR<=0.)discard;o=vec4(0.,0.,0.,uAR);return;}}\n\
  o=vC;}}"
            ),
        )?;
        let img = prog(
            &gl,
            &format!("{HDR}layout(location=0) in vec2 aP;uniform vec4 uRect;out vec2 vU;\nvoid main(){{vU=aP;gl_Position=toClip((uRect.xy+aP*uRect.zw)*uK+uOff);}}"),
            "#version 300 es\nprecision mediump float;in vec2 vU;uniform sampler2D uT;uniform float uA;out vec4 o;void main(){vec4 c=texture(uT,vU);o=vec4(c.rgb,c.a*uA);}",
        )?;
        let wfill = prog(
            &gl,
            "#version 300 es\nprecision highp float;uniform vec2 uRes;uniform float uK;uniform vec2 uOff;uniform float uMax;\nlayout(location=0) in vec2 aP;layout(location=1) in float aR;\nvoid main(){vec2 s=aP*uK+uOff;vec2 c=s/uRes*2.0-1.0;gl_Position=aR>uMax?vec4(2.,2.,2.,1.):vec4(c.x,-c.y,0.,1.);}",
            "#version 300 es\nprecision mediump float;uniform vec4 uC;out vec4 o;void main(){o=uC;}",
        )?;
        let wline = prog(
            &gl,
            "#version 300 es\nprecision highp float;uniform vec2 uRes;uniform float uK;uniform vec2 uOff;uniform float uMax,uW;\nlayout(location=0) in vec2 aT;layout(location=1) in vec4 aS;layout(location=2) in float aR;\nvoid main(){float hw=uW*max(.35,1.2-aR*.09);vec2 a=aS.xy*uK+uOff,b=aS.zw*uK+uOff;vec2 d=b-a;float L=length(d);d=L>0.?d/L:vec2(1.,0.);vec2 n=vec2(-d.y,d.x);\n  vec2 p=mix(a,b,aT.x)+d*(aT.x*2.-1.)*hw+n*aT.y*hw;vec2 c=p/uRes*2.0-1.0;gl_Position=aR>uMax?vec4(2.,2.,2.,1.):vec4(c.x,-c.y,0.,1.);}",
            "#version 300 es\nprecision mediump float;uniform vec4 uC;out vec4 o;void main(){o=uC;}",
        )?;
        let ov = prog(
            &gl,
            "#version 300 es\nprecision highp float;uniform vec2 uRes;layout(location=0) in vec2 aP;layout(location=1) in vec2 aU;layout(location=2) in vec4 aC;out vec2 vU;out vec4 vC;\nvoid main(){vec2 c=aP/uRes*2.0-1.0;gl_Position=vec4(c.x,-c.y,0.,1.);vU=aU;vC=aC;}",
            "#version 300 es\nprecision mediump float;in vec2 vU;in vec4 vC;uniform sampler2D uT;out vec4 o;void main(){vec4 t=texture(uT,vU);o=vec4(vC.rgb*t.rgb,vC.a*t.a);}",
        )?;
        let quad = gl.create_buffer()?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, f32s(&[0., 0., 1., 0., 0., 1., 1., 1.]), glow::STATIC_DRAW);
        let seg = gl.create_buffer()?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(seg));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, f32s(&[0., -1., 1., -1., 0., 1., 1., 1.]), glow::STATIC_DRAW);
        let mut lods = Vec::new();
        for l in &w.lods {
            lods.push(GLod { fill: mk_fill(&gl, &l.fill), line: mk_line(&gl, seg, &l.line), line2: mk_line(&gl, seg, &l.line2) });
        }
        let lakes = w.water.fill.iter().map(|d| mk_fill(&gl, d)).collect();
        let rivers = w.water.riv.iter().map(|d| mk_line(&gl, seg, d)).collect();
        let lake_ol = w.water.ol.iter().map(|d| mk_line(&gl, seg, d)).collect();
        let r = w.nreg();
        let th = (r + TW - 1) / TW;
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        let col_t = tex(&gl, glow::NEAREST as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, TW as i32, th as i32, 0, glow::RGBA, glow::UNSIGNED_BYTE, None);
        let own_t = tex(&gl, glow::NEAREST as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA32UI as i32, TW as i32, th as i32, 0, glow::RGBA_INTEGER, glow::UNSIGNED_INT, None);
        let zid_t = tex(&gl, glow::NEAREST as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::R16UI as i32, 1, 1, 0, glow::RED_INTEGER, glow::UNSIGNED_SHORT, Some(&[0, 0]));
        let zin_t = tex(&gl, glow::NEAREST as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA32UI as i32, 1, 1, 0, glow::RGBA_INTEGER, glow::UNSIGNED_INT, Some(&[0; 16]));
        let zf_t = tex(&gl, glow::LINEAR as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::R8 as i32, 1, 1, 0, glow::RED, glow::UNSIGNED_BYTE, Some(&[0]));
        let atlas_t = tex(&gl, glow::LINEAR as i32);
        let ov_vao = gl.create_vertex_array()?;
        let ov_vbo = gl.create_buffer()?;
        gl.bind_vertex_array(Some(ov_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(ov_vbo));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 32, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 32, 8);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, 32, 16);
        gl.bind_vertex_array(None);
        Ok(Renderer {
            gl,
            fill,
            line,
            img,
            wfill,
            wline,
            ov,
            quad,
            seg,
            lods,
            lakes,
            rivers,
            lake_ol,
            water_rg: w.water.rg.clone(),
            col_t,
            own_t,
            zid_t,
            zin_t,
            zf_t,
            th,
            bg_t: None,
            tiles: vec![None; 8],
            atlas_t,
            ov_vao,
            ov_vbo,
            zon: false,
            z_alloc: false,
            anx: vec![],
            pc_t: None,
            pc_dim: [0, 0],
        })
    }

    pub unsafe fn image(&mut self, slot: i32, w: i32, h: i32, rgba: &[u8]) {
        let gl = &self.gl;
        let t = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(t));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, w, h, 0, glow::RGBA, glow::UNSIGNED_BYTE, Some(rgba));
        gl.generate_mipmap(glow::TEXTURE_2D);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR_MIPMAP_LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        if slot < 0 {
            self.bg_t = Some(t);
        } else if (slot as usize) < self.tiles.len() {
            self.tiles[slot as usize] = Some(t);
        }
    }
    pub unsafe fn upload_regions(&self, col: &[u8], own: &[u32]) {
        let gl = &self.gl;
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.col_t));
        gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, TW as i32, self.th as i32, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(col));
        gl.bind_texture(glow::TEXTURE_2D, Some(self.own_t));
        gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, TW as i32, self.th as i32, glow::RGBA_INTEGER, glow::UNSIGNED_INT, glow::PixelUnpackData::Slice(u32s(own)));
    }
    pub unsafe fn upload_zones(&mut self, zid: &[u16], ff: &[u8], dirty: Option<[usize; 4]>, full: bool, inf: &[u32], zh: usize) {
        let gl = &self.gl;
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        if full || !self.z_alloc {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.zid_t));
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::R16UI as i32, GW as i32, GH as i32, 0, glow::RED_INTEGER, glow::UNSIGNED_SHORT, Some(u16s(zid)));
            gl.bind_texture(glow::TEXTURE_2D, Some(self.zf_t));
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::R8 as i32, GW as i32, GH as i32, 0, glow::RED, glow::UNSIGNED_BYTE, Some(ff));
            self.z_alloc = true;
        } else if let Some([x0, y0, x1, y1]) = dirty {
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            let mut za = vec![0u16; w * h];
            let mut fa = vec![0u8; w * h];
            for j in 0..h {
                let s = (y0 + j) * GW + x0;
                za[j * w..j * w + w].copy_from_slice(&zid[s..s + w]);
                fa[j * w..j * w + w].copy_from_slice(&ff[s..s + w]);
            }
            gl.bind_texture(glow::TEXTURE_2D, Some(self.zid_t));
            gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, x0 as i32, y0 as i32, w as i32, h as i32, glow::RED_INTEGER, glow::UNSIGNED_SHORT, glow::PixelUnpackData::Slice(u16s(&za)));
            gl.bind_texture(glow::TEXTURE_2D, Some(self.zf_t));
            gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, x0 as i32, y0 as i32, w as i32, h as i32, glow::RED, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(&fa));
        }
        gl.bind_texture(glow::TEXTURE_2D, Some(self.zin_t));
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA32UI as i32, TW as i32, zh as i32, 0, glow::RGBA_INTEGER, glow::UNSIGNED_INT, Some(u32s(inf)));
    }
    pub unsafe fn set_anx(&mut self, segs: &[Vec<f32>]) {
        let gl = &self.gl;
        self.anx.clear();
        for s in segs {
            if s.is_empty() {
                self.anx.push(None);
                continue;
            }
            let n = s.len() / 6;
            let mut ss = Vec::with_capacity(n * 4);
            let mut ll = Vec::with_capacity(n * 2);
            for i in 0..n {
                ss.extend_from_slice(&s[6 * i..6 * i + 4]);
                ll.extend_from_slice(&s[6 * i + 4..6 * i + 6]);
            }
            let mut rg = vec![[0u32; 2]; NC];
            rg[0] = [0, n as u32];
            let mut bb = vec![[0f32; 4]; NC];
            bb[0] = [-1e9, -1e9, 1e9, 1e9];
            let lm = LineMesh { s: ss, l: ll, an: 2, rg, bb };
            self.anx.push(Some(mk_line(gl, self.seg, &lm)));
        }
    }
    pub unsafe fn upload_atlas(&self, w: usize, h: usize, rgba: &[u8], full: bool, rect: [usize; 4]) {
        let gl = &self.gl;
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_t));
        if full {
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, w as i32, h as i32, 0, glow::RGBA, glow::UNSIGNED_BYTE, Some(rgba));
        } else {
            let [x0, y0, x1, y1] = rect;
            let (rw, rh) = (x1 - x0, y1 - y0);
            let mut sub = vec![0u8; rw * rh * 4];
            for j in 0..rh {
                let s = ((y0 + j) * w + x0) * 4;
                sub[j * rw * 4..(j + 1) * rw * 4].copy_from_slice(&rgba[s..s + rw * 4]);
            }
            gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, x0 as i32, y0 as i32, rw as i32, rh as i32, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(&sub));
        }
    }
    pub unsafe fn upload_pc(&mut self, w: usize, h: usize, rgba: &[u8]) {
        let gl = &self.gl;
        if self.pc_t.is_none() {
            self.pc_t = Some(tex(gl, glow::LINEAR as i32));
        }
        gl.bind_texture(glow::TEXTURE_2D, self.pc_t);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, w as i32, h as i32, 0, glow::RGBA, glow::UNSIGNED_BYTE, Some(rgba));
        self.pc_dim = [w, h];
    }

    unsafe fn draw_fill(&self, f: &GFill, v: &View) {
        let gl = &self.gl;
        gl.bind_vertex_array(Some(f.vao));
        let (mut st, mut cn) = (-1i64, 0i64);
        let flush = |st: &mut i64, cn: &mut i64| {
            if *cn > 0 {
                gl.draw_elements(glow::TRIANGLES, *cn as i32, glow::UNSIGNED_INT, (*st * 4) as i32);
            }
            *cn = 0;
            *st = -1;
        };
        for k in 0..NC {
            let n = f.fr[k][1] as i64;
            if n == 0 {
                continue;
            }
            let b = f.fbb[k];
            if b[2] < v.vx0 || b[0] > v.vx1 || b[3] < v.vy0 || b[1] > v.vy1 {
                flush(&mut st, &mut cn);
                continue;
            }
            let s = f.fr[k][0] as i64;
            if st < 0 {
                st = s;
            }
            if s == st + cn {
                cn += n;
            } else {
                flush(&mut st, &mut cn);
                st = s;
                cn = n;
            }
        }
        flush(&mut st, &mut cn);
    }
    unsafe fn draw_line(&self, l: &GLine, v: &View, pad: f32) {
        let gl = &self.gl;
        gl.bind_vertex_array(Some(l.vao));
        let (mut st, mut cn) = (-1i64, 0i64);
        let an = l.an;
        let flush = |st: &mut i64, cn: &mut i64| {
            if *cn > 0 {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(l.sb));
                gl.vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, 0, (*st * 16) as i32);
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(l.lb));
                gl.vertex_attrib_pointer_f32(2, an, glow::FLOAT, false, 0, (*st * 4 * an as i64) as i32);
                gl.draw_arrays_instanced(glow::TRIANGLE_STRIP, 0, 4, *cn as i32);
            }
            *cn = 0;
            *st = -1;
        };
        for k in 0..NC {
            let n = l.rg[k][1] as i64;
            if n == 0 {
                continue;
            }
            let b = l.bb[k];
            if b[2] + pad < v.vx0 || b[0] - pad > v.vx1 || b[3] + pad < v.vy0 || b[1] - pad > v.vy1 {
                flush(&mut st, &mut cn);
                continue;
            }
            let s = l.rg[k][0] as i64;
            if st < 0 {
                st = s;
            }
            if s == st + cn {
                cn += n;
            } else {
                flush(&mut st, &mut cn);
                st = s;
                cn = n;
            }
        }
        flush(&mut st, &mut cn);
    }
    unsafe fn set_view(&self, p: &Prog, v: &View) {
        let gl = &self.gl;
        gl.use_program(Some(p.p));
        gl.uniform_2_f32(p.u("uRes"), v.res[0], v.res[1]);
        gl.uniform_1_f32(p.u("uK"), v.kd);
        gl.uniform_2_f32(p.u("uOff"), v.off[0], v.off[1]);
    }

    // full map pass for one world copy
    pub unsafe fn draw_map(&self, v: &View, k: f32, dpr: f32, li: usize, conq: bool, gray_mode: bool, alpha: f32) {
        let gl = &self.gl;
        let mw = 2000.0;
        let mh = 1000.0;
        // background
        let p = &self.img;
        self.set_view(p, v);
        gl.uniform_1_i32(p.u("uT"), 0);
        gl.uniform_1_f32(p.u("uA"), 1.0);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_vertex_array(None);
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.quad));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);
        if !gray_mode {
            if let Some(t) = self.bg_t {
                gl.bind_texture(glow::TEXTURE_2D, Some(t));
                gl.uniform_4_f32(p.u("uRect"), 0.0, 0.0, mw, mh);
                gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            }
            let use_tiles = v.kd * mw > 4096.0 * 1.1;
            if use_tiles {
                for ty in 0..2 {
                    for tx in 0..4 {
                        let (x0, y0) = (tx as f32 * mw / 4.0, ty as f32 * mh / 2.0);
                        if x0 + mw / 4.0 < v.vx0 || x0 > v.vx1 || y0 + mh / 2.0 < v.vy0 || y0 > v.vy1 {
                            continue;
                        }
                        if let Some(t) = self.tiles[ty * 4 + tx] {
                            gl.bind_texture(glow::TEXTURE_2D, Some(t));
                            gl.uniform_4_f32(p.u("uRect"), x0, y0, mw / 4.0, mh / 2.0);
                            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
                        }
                    }
                }
            }
        }
        // fills
        let p = &self.fill;
        self.set_view(p, v);
        gl.uniform_1_f32(p.u("uA"), alpha);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.col_t));
        gl.uniform_1_i32(p.u("uCol"), 0);
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.zid_t));
        gl.uniform_1_i32(p.u("uZid"), 1);
        gl.active_texture(glow::TEXTURE2);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.zin_t));
        gl.uniform_1_i32(p.u("uZin"), 2);
        gl.active_texture(glow::TEXTURE3);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.zf_t));
        gl.uniform_1_i32(p.u("uZf"), 3);
        gl.uniform_1_i32(p.u("uZon"), self.zon as i32);
        if gray_mode {
            gl.uniform_4_f32(p.u("uGray"), 0.2, 0.23, 0.286, 1.0);
        } else {
            gl.uniform_4_f32(p.u("uGray"), 0.0, 0.0, 0.0, 0.0);
        }
        gl.active_texture(glow::TEXTURE0);
        let l = &self.lods[li];
        if li > 0 {
            gl.enable(glow::STENCIL_TEST);
            gl.stencil_func(glow::ALWAYS, 1, 255);
            gl.stencil_op(glow::KEEP, glow::KEEP, glow::REPLACE);
            self.draw_fill(&l.fill, v);
            gl.stencil_func(glow::EQUAL, 0, 255);
            gl.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
            self.draw_fill(&self.lods[li - 1].fill, v);
            gl.disable(glow::STENCIL_TEST);
        } else {
            self.draw_fill(&l.fill, v);
        }
        // borders
        let p = &self.line;
        self.set_view(p, v);
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.own_t));
        gl.uniform_1_i32(p.u("uOwn"), 1);
        gl.uniform_1_f32(p.u("uWC"), (1.0 + k * 0.06).min(2.4) * dpr * 0.5);
        gl.uniform_1_f32(p.u("uWR"), if k >= 12.0 { 0.6 * dpr } else { 0.0 });
        gl.uniform_1_f32(p.u("uAR"), ((k - 12.0) * 0.03 + 0.12).min(0.35));
        gl.active_texture(glow::TEXTURE4);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.zid_t));
        gl.uniform_1_i32(p.u("uZid"), 4);
        gl.active_texture(glow::TEXTURE2);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.zin_t));
        gl.uniform_1_i32(p.u("uZin"), 2);
        gl.active_texture(glow::TEXTURE3);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.zf_t));
        gl.uniform_1_i32(p.u("uZf"), 3);
        gl.uniform_1_i32(p.u("uZon"), self.zon as i32);
        gl.active_texture(glow::TEXTURE0);
        let pad = 4.0 / (v.kd / dpr);
        self.draw_line(&l.line, v, pad);
        if k >= 12.0 || conq {
            self.draw_line(&l.line2, v, pad);
        } else if self.zon {
            if let Some(Some(a)) = self.anx.get(li) {
                self.draw_line(a, v, pad);
            }
        }
        if !gray_mode {
            // water
            let mr = w_rank(k) as f32;
            let p = &self.wline;
            self.set_view(p, v);
            gl.uniform_1_f32(p.u("uMax"), mr);
            gl.uniform_1_f32(p.u("uW"), (0.7 + k * 0.05).min(2.2) * dpr * 0.5);
            gl.uniform_4_f32(p.u("uC"), 0.16, 0.42, 0.7, 1.0);
            for (g, _) in self.water_rg.iter().enumerate() {
                if g == 0 || (self.water_rg[g - 1] as f32) < mr {
                    self.draw_line(&self.rivers[g], v, pad);
                }
            }
            let p = &self.wfill;
            self.set_view(p, v);
            gl.uniform_1_f32(p.u("uMax"), mr);
            gl.uniform_4_f32(p.u("uC"), 0.11, 0.33, 0.58, 1.0);
            for (g, _) in self.water_rg.iter().enumerate() {
                if g == 0 || (self.water_rg[g - 1] as f32) < mr {
                    self.draw_fill(&self.lakes[g], v);
                }
            }
            let p = &self.wline;
            self.set_view(p, v);
            gl.uniform_1_f32(p.u("uMax"), mr);
            gl.uniform_1_f32(p.u("uW"), 0.5 * dpr);
            gl.uniform_4_f32(p.u("uC"), 0.0, 0.0, 0.0, 0.55);
            for (g, _) in self.water_rg.iter().enumerate() {
                if g == 0 || (self.water_rg[g - 1] as f32) < mr {
                    self.draw_line(&self.lake_ol[g], v, pad);
                }
            }
        }
        gl.bind_vertex_array(None);
    }
    pub unsafe fn draw_pc_overlay(&self, v: &View, rect: [f32; 4]) {
        if let Some(t) = self.pc_t {
            let gl = &self.gl;
            let p = &self.img;
            self.set_view(p, v);
            gl.uniform_1_i32(p.u("uT"), 0);
            gl.uniform_1_f32(p.u("uA"), 1.0);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(t));
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.quad));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);
            gl.uniform_4_f32(p.u("uRect"), rect[0], rect[1], rect[2], rect[3]);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }
    pub unsafe fn draw_overlay(&self, res: [f32; 2], verts: &[f32]) {
        if verts.is_empty() {
            return;
        }
        let gl = &self.gl;
        gl.use_program(Some(self.ov.p));
        gl.uniform_2_f32(self.ov.u("uRes"), res[0], res[1]);
        gl.uniform_1_i32(self.ov.u("uT"), 0);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_t));
        gl.bind_vertex_array(Some(self.ov_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.ov_vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, f32s(verts), glow::STREAM_DRAW);
        gl.draw_arrays(glow::TRIANGLES, 0, (verts.len() / 8) as i32);
        gl.bind_vertex_array(None);
    }
}

pub fn w_rank(k: f32) -> i32 {
    if k < 1.6 {
        1
    } else if k < 2.5 {
        3
    } else if k < 4.0 {
        5
    } else if k < 7.0 {
        7
    } else if k < 12.0 {
        8
    } else {
        10
    }
}
