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
                let (f0, f1): (f32, f32) = if p.len() > 3 { (p[2].parse().unwrap(), p[3].parse().unwrap()) } else { (0.0, 1.0) };
                let (x0, x1) = (x0 + (x1 - x0) * f0, x0 + (x1 - x0) * f1);
                let mut x = x0 - 3.0;
                while x <= x1 + 3.0 {
                    pl.push([x, cy]);
                    x += 2.0;
                }
                let _ = cx;
                app.s.make_plan(pl);
                app.s.exec_plans();
            }
            "bench" => {
                // same scenario as geo/perf/sim_bench.js (old JS engine)
                let nn: usize = p[1].parse().unwrap();
                let me = app.w.nations.iter().position(|n| n.iso == "ru").unwrap();
                app.cmd("new", me as i64, 0, "");
                let tg: Vec<u16> = ["ua", "kz", "mn", "by", "ge"].iter().map(|i| app.w.nations.iter().position(|n| &n.iso == i).unwrap() as u16).collect();
                for &t in &tg { app.cmd("war", t as i64, 0, ""); }
                let per = (nn + 49) / 50;
                for &t in &tg {
                    let before: std::collections::HashSet<u32> = app.s.g.divs.iter().map(|d| d.id).collect();
                    let q = app.s.terr(t)[0] as usize;
                    let qp = app.w.lp(q);
                    let mut near = app.s.terr(me as u16).clone();
                    near.sort_by(|a, b| { let (pa, pb) = (app.w.lp(*a as usize), app.w.lp(*b as usize)); (pa[0]-qp[0]).hypot(pa[1]-qp[1]).partial_cmp(&(pb[0]-qp[0]).hypot(pb[1]-qp[1])).unwrap() });
                    for i in 0..per { app.s.test_divs(near[i % 5], 0); }
                    app.s.sels = app.s.g.divs.iter().filter(|d| !before.contains(&d.id)).map(|d| d.id).collect();
                    app.s.sel_div = None;
                    app.s.front_on_nation(qp[0], qp[1]);
                }
                let clk = || std::time::Instant::now();
                let mut tm = std::collections::BTreeMap::<&str, (f64, u32)>::new();
                let run = |app: &mut App, n: usize, tm: &mut std::collections::BTreeMap<&str, (f64, u32)>| {
                    let me = app.s.g.player;
                    for i in 0..n {
                        let t = clk(); app.hour_tick(); let e = tm.entry("hourTick").or_default(); e.0 += t.elapsed().as_secs_f64()*1e3; e.1 += 1;
                        if i % 3 == 0 { let t = clk(); app.s.occ_smooth(me); let e = tm.entry("occSmooth").or_default(); e.0 += t.elapsed().as_secs_f64()*1e3; e.1 += 1; }
                        if i % 4 == 0 { let t = clk(); let mut zt = std::mem::take(&mut app.zt); app.s.zone_flush(&mut zt, false, 0.0, &|| 0.0); app.zt = zt; let e = tm.entry("zoneFlush").or_default(); e.0 += t.elapsed().as_secs_f64()*1e3; e.1 += 1; }
                        if i % 12 == 0 { let t = clk(); let mut zt = std::mem::take(&mut app.zt); app.s.cut_flush(&mut zt, 0.0, &|| 0.0); app.zt = zt; let e = tm.entry("cutFlush").or_default(); e.0 += t.elapsed().as_secs_f64()*1e3; e.1 += 1; }
                    }
                };
                run(&mut app, 120, &mut tm);
                for fi in 0..app.s.g.fronts.len() {
                    let t = match app.s.g.fronts[fi].t { Some(t) => t, None => continue };
                    let rs = app.s.terr(t).clone(); if rs.is_empty() { continue; }
                    let (mut cx, mut cy) = (0f32, 0f32);
                    for &r in &rs { let l = app.w.lp(r as usize); cx += l[0]; cy += l[1]; }
                    cx /= rs.len() as f32; cy /= rs.len() as f32;
                    let mut pl = vec![]; let mut a = -1.0f32; while a <= 1.001 { pl.push([cx + a * 8.0, cy + (a * 3.0).sin()]); a += 0.1; }
                    app.g_plan(fi, pl);
                }
                tm.clear();
                let t0 = clk(); app.s.exec_plans(); let ex = t0.elapsed().as_secs_f64()*1e3;
                let t1 = clk(); run(&mut app, 300, &mut tm); let tot = t1.elapsed().as_secs_f64()*1e3 + ex;
                let mut out = format!("{{\"divs\":{},\"totalMs\":{:.0},\"msPerTick\":{:.2},\"execMs\":{:.1}", app.s.g.divs.len(), tot, tot/300.0, ex);
                for (k, (ms, c)) in &tm { out += &format!(",\"{}\":{{\"ms\":{:.0},\"calls\":{},\"perCall\":{:.3}}}", k, ms, c, ms / *c as f64); }
                out += "}";
                println!("{}", out);
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
            "uidraw" => {
                // UI path: toolbar mode + touch drag in screen coords + toasts
                app.cmd("draw", 0, 0, p[1]);
                let v: Vec<f32> = p[2..6].iter().map(|x| x.parse().unwrap()).collect();
                app.draw_touch(0, v[0], v[1]);
                for i in 1..=20 { let t = i as f32 / 20.0; app.draw_touch(1, v[0] + (v[2] - v[0]) * t, v[1] + (v[3] - v[1]) * t); }
                app.draw_touch(2, v[2], v[3]);
                eprintln!("after draw: {}", app.ui_json());
            }
            "uitap" => {
                app.cmd("draw", 0, 0, p[1]);
                let (x, y): (f32, f32) = (p[2].parse().unwrap(), p[3].parse().unwrap());
                app.draw_touch(0, x, y);
                app.draw_touch(2, x, y);
                eprintln!("after tap: {}", app.ui_json());
            }
            "exec" => { app.cmd("exec", 0, 0, ""); eprintln!("after exec: {}", app.ui_json()); }
            "atk" => {
                for f in &app.s.g.fronts { eprintln!("front divs {} atk {} exec {}", f.divs.len(), f.atk.len(), f.exec); }
                let c: usize = app.s.cover.values().map(|v| v.iter().filter(|s| s.d != 0).count()).sum();
                let g: usize = app.s.cover.values().map(|v| v.iter().filter(|s| s.d == 0).count()).sum();
                eprintln!("cover slots filled {} empty {}", c, g);
            }
            "scr" => {
                let i = app.w.nations.iter().position(|n| n.iso == p[1]).unwrap();
                eprintln!("scr {}", app.cmd("scr", i as i64, p[2].parse().unwrap(), ""));
            }
            "own" => {
                let i = app.w.nations.iter().position(|n| n.iso == p[1]).unwrap() as u16;
                for r in 0..app.w.nreg() { if app.w.regions[r].nat == i { let zs: Vec<_> = app.s.g.zones.iter().filter(|z| z.r as usize == r).map(|z| (z.o, z.k, z.c)).collect(); eprintln!("r{} owner {} zones {:?}", r, app.s.g.owner[r], zs); } }
            }
            _ => eprintln!("? {}", cmd),
        }
    }
}
