// Desktop harness: loads assets, renders frames offscreen with ANGLE (from Chromium) and writes PNGs.
// usage: shot <assets_dir> <out_prefix> [script]
use geopolit::app::App;
use geopolit::world;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).cloned().unwrap_or("../src/main/assets".into());
    let out = args.get(2).cloned().unwrap_or("/tmp/shot".into());
    let script = args.get(3).cloned().unwrap_or_default();
    let rd = |n: &str| std::fs::read(format!("{}/{}", dir, n)).expect(n);
    let t0 = std::time::Instant::now();
    let (n, l, m, h, w, mr, c) = (rd("nations.json"), rd("world_lo.json"), rd("world_mid.json"), rd("world.json"), rd("water.json"), rd("marine.json"), rd("cities.json"));
    let a = world::Assets { nations: &n, world_lo: &l, world_mid: &m, world_hi: &h, water: &w, marine: &mr, cities: &c };
    let wd = Arc::new(world::load(&a, &|s| eprintln!("{}", s)));
    eprintln!("load {:?}", t0.elapsed());
    let fonts = vec![std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").unwrap(), std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf").unwrap()];
    let mut app = App::new(wd, fonts);
    let (sw, sh) = (1600i32, 800i32);
    app.resize(sw as f32, sh as f32, 2.0);

    // EGL via ANGLE
    let lib = unsafe { libloading::Library::new("libEGL.so.1").expect("libEGL") };
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_4>::load_required_from(lib).expect("egl") };
    let disp = unsafe { egl.get_display(khronos_egl::DEFAULT_DISPLAY) }.expect("display");
    egl.initialize(disp).expect("init");
    let attrs = [
        khronos_egl::RED_SIZE, 8, khronos_egl::GREEN_SIZE, 8, khronos_egl::BLUE_SIZE, 8, khronos_egl::ALPHA_SIZE, 8, khronos_egl::STENCIL_SIZE, 8,
        khronos_egl::SURFACE_TYPE, khronos_egl::PBUFFER_BIT, khronos_egl::RENDERABLE_TYPE, 0x40, khronos_egl::NONE,
    ];
    let cfg = egl.choose_first_config(disp, &attrs).expect("cfg").expect("no cfg");
    let surf = egl.create_pbuffer_surface(disp, cfg, &[khronos_egl::WIDTH, sw, khronos_egl::HEIGHT, sh, khronos_egl::NONE]).expect("pbuf");
    egl.bind_api(khronos_egl::OPENGL_ES_API).unwrap();
    let ctx = egl.create_context(disp, cfg, None, &[0x3098, 3, khronos_egl::NONE]).expect("ctx");
    egl.make_current(disp, Some(surf), Some(surf), Some(ctx)).unwrap();
    let gl = unsafe { glow::Context::from_loader_function(|s| egl.get_proc_address(s).map(|p| p as *const _).unwrap_or(std::ptr::null())) };
    let r = unsafe { geopolit::render::Renderer::new(gl, &app.w) }.unwrap_or_else(|e| panic!("{}", e));
    app.r = Some(r);
    // background image
    if let Ok(f) = std::fs::File::open(format!("{}/bg.png", "/tmp")) {
        let dec = png::Decoder::new(f);
        if let Ok(mut rd) = dec.read_info() {
            let mut buf = vec![0; rd.output_buffer_size()];
            if let Ok(info) = rd.next_frame(&mut buf) {
                unsafe { app.r.as_mut().unwrap().image(-1, info.width as i32, info.height as i32, &buf) };
            }
        }
    }
    let mut now = 1000.0;
    let mut shot_n = 0;
    let mut shoot = |app: &mut App, now: &mut f64| {
        for _ in 0..3 {
            *now += 16.0;
            app.frame(*now);
        }
        use glow::HasContext;
        let gl = &app.r.as_ref().unwrap().gl;
        let mut px = vec![0u8; (sw * sh * 4) as usize];
        unsafe { gl.read_pixels(0, 0, sw, sh, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut px)) };
        let mut flip = vec![0u8; px.len()];
        for y in 0..sh as usize {
            let s = (sh as usize - 1 - y) * sw as usize * 4;
            flip[y * sw as usize * 4..(y + 1) * sw as usize * 4].copy_from_slice(&px[s..s + sw as usize * 4]);
        }
        let path = format!("{}{}.png", out, shot_n);
        shot_n += 1;
        let f = std::fs::File::create(&path).unwrap();
        let mut e = png::Encoder::new(f, sw as u32, sh as u32);
        e.set_color(png::ColorType::Rgba);
        e.write_header().unwrap().write_image_data(&flip).unwrap();
        eprintln!("wrote {}", path);
    };
    for cmd in script.split(';').filter(|s| !s.trim().is_empty()) {
        let p: Vec<&str> = cmd.trim().split_whitespace().collect();
        match p[0] {
            "shot" => shoot(&mut app, &mut now),
            "new" => {
                let iso = p[1];
                let i = app.w.nations.iter().position(|n| n.iso == iso).unwrap();
                app.cmd("new", i as i64, 0, "");
                now += 2000.0;
                app.frame(now);
            }
            "zoomnat" => {
                let i = app.w.nations.iter().position(|n| n.iso == p[1]).unwrap();
                app.zoom_to_nat(i as u16);
                eprintln!("anim {:?}", app.anim.map(|a| (a.0, a.1)));
                now += 2000.0;
                app.frame(now);
                now += 800.0;
                app.frame(now);
                eprintln!("cam {:?}", app.cam);
            }
            "war" => {
                let i = app.w.nations.iter().position(|n| n.iso == p[1]).unwrap();
                app.cmd("war", i as i64, 0, "");
            }
            "tap" => {
                let r = app.tap(p[1].parse().unwrap(), p[2].parse().unwrap());
                eprintln!("tap -> {} | {}", r, app.ui_json());
            }
            "testdivs" => {
                let me = app.s.g.player;
                let rs = app.s.terr(me).clone();
                let nn: usize = p[1].parse().unwrap();
                for i in 0..nn {
                    app.s.test_divs(rs[i % rs.len()], 0);
                }
            }
            "front" => {
                let i = app.w.nations.iter().position(|n| n.iso == p[1]).unwrap();
                let r = app.s.terr(i as u16)[0];
                let lp = app.w.lp(r as usize);
                app.s.sels.clear();
                app.s.front_on_nation(lp[0], lp[1]);
            }
            "ticks" => {
                let nn: usize = p[1].parse().unwrap();
                let t = std::time::Instant::now();
                for _ in 0..nn {
                    app.hour_tick();
                    now += 1.0;
                    app.s.now_ms = now;
                    app.s.step_pockets();
                    let me = app.s.g.player;
                    app.s.occ_smooth(me);
                }
                eprintln!("{} ticks {:?}", nn, t.elapsed());
            }
            "plan" => {
                // plan across the enemy through its centroid (horizontal)
                let i = app.w.nations.iter().position(|n| n.iso == p[1]).unwrap() as u16;
                let rs = app.s.terr(i).clone();
                let (mut cx, mut cy, mut x0, mut x1) = (0f32, 0f32, 1e9f32, -1e9f32);
                for &r in &rs {
                    let lp = app.w.lp(r as usize);
                    cx += lp[0];
                    cy += lp[1];
                    x0 = x0.min(lp[0]);
                    x1 = x1.max(lp[0]);
                }
                cx /= rs.len() as f32;
                cy /= rs.len() as f32;
                let mut pl = vec![];
                let mut x = x0 - 3.0;
                while x <= x1 + 3.0 {
                    pl.push([x, cy]);
                    x += 2.0;
                }
                let _ = cx;
                app.s.make_plan(pl);
                app.s.exec_plans();
            }
            "speed" => { app.cmd("speed", p[1].parse().unwrap(), 0, ""); }
            "frames" => {
                let nn: usize = p[1].parse().unwrap();
                for _ in 0..nn {
                    now += 50.0;
                    app.frame(now);
                }
            }
            "zoom" => {
                app.zoom(p[1].parse().unwrap(), sw as f32 / 2.0, sh as f32 / 2.0);
            }
            "peace" => {
                let i = app.w.nations.iter().position(|n| n.iso == p[1]).unwrap();
                app.cmd("peace", i as i64, 0, "");
                app.pc_set_rect(0.0, 0.0, sw as f32 * 0.7, sh as f32);
                eprintln!("{}", app.s.pc_state_json());
            }
            "pcall" => { app.cmd("pcall", 1, 0, ""); }
            "pcsign" => { app.cmd("pcsign", 0, 0, ""); }
            "debug" => { app.cmd("debug", 0, 0, ""); }
            "ui" => eprintln!("{}", app.ui_json()),
            _ => eprintln!("? {}", cmd),
        }
    }
}
