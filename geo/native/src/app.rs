// Application: camera, input, selection, UI JSON, simulation stepping and frame rendering.
use glow::HasContext;
use crate::batch::{rgba, Atlas, Batch, Col};
use crate::colors::nat_color;
use crate::labels::Labels;
use crate::render::{Renderer, View, TW};
use crate::sim::{fmt, fmt_n, Sim, SPEEDS};
use crate::topo::{MH, MW};
use crate::world::{point_in_rings, World, GH, GW};
use crate::zones::ZoneTex;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub x: f32,
    pub y: f32,
    pub k: f32,
}

#[derive(Clone, Copy)]
pub struct SelZ {
    pub r: u32,
    pub z: u16,
    pub g: u32,
}

pub struct App {
    pub w: Arc<World>,
    pub s: Sim,
    pub zt: ZoneTex,
    pub r: Option<Renderer>,
    pub at: Atlas,
    pub lbl: Labels,
    pub batch: Batch,
    pub cam: Cam,
    pub sw: f32,
    pub sh: f32,
    pub dpr: f32,
    pub started: bool,
    pub paused: bool,
    pub sel: i64,
    pub sel_z: Option<SelZ>,
    pub sel_w: Option<(bool, usize)>,
    pub sel_sea: i64,
    pub draw: Option<String>,
    pub drawp: Vec<[f32; 2]>,
    pub draw_dn: [f32; 2],
    pub own_seen: u64,
    pub z_flush_t: f64,
    pub cut_t: f64,
    pub pk_t: f64,
    pub lbl_t: f64,
    pub debug: bool,
    pub anim: Option<(Cam, Cam, f64, f64)>,
    pub last_ms: f64,
    pub ui_ver: u64,
    pub atlas_gen: u64,
    pub sel_cache: Option<(u64, u64, Vec<[f32; 4]>)>,
    pub pc_rect: [f32; 4],
    pub pc_hi_t: f64,
    pub prof: Vec<(String, f64)>,
    pub pf_acc: std::collections::HashMap<&'static str, f64>,
    pub pf_frames: u32,
    pub pf_ticks: u32,
    pub pf_t0: f64,
    pub clock: fn() -> f64,
    pub anx_seen: u64,
}

fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

impl App {
    pub fn new(w: Arc<World>, fonts: Vec<Vec<u8>>) -> App {
        let mut s = Sim::new(w.clone());
        s.seed((now_ms() as u64).wrapping_mul(6364136223846793005));
        App {
            w,
            s,
            zt: ZoneTex::default(),
            r: None,
            at: Atlas::new(fonts),
            lbl: Labels::new(),
            batch: Batch::new(),
            cam: Cam { x: 0.0, y: 0.0, k: 1.0 },
            sw: 1000.0,
            sh: 500.0,
            dpr: 2.0,
            started: false,
            paused: false,
            sel: -1,
            sel_z: None,
            sel_w: None,
            sel_sea: -1,
            draw: None,
            drawp: vec![],
            draw_dn: [0.0, 0.0],
            own_seen: 0,
            z_flush_t: 0.0,
            cut_t: 0.0,
            pk_t: 0.0,
            lbl_t: 0.0,
            debug: false,
            anim: None,
            last_ms: 0.0,
            ui_ver: 1,
            atlas_gen: u64::MAX,
            sel_cache: None,
            pc_rect: [0.0, 0.0, 0.0, 0.0],
            pc_hi_t: 0.0,
            prof: vec![],
            pf_acc: Default::default(),
            pf_frames: 0,
            pf_ticks: 0,
            pf_t0: 0.0,
            clock: now_ms,
            anx_seen: u64::MAX,
        }
    }
    // ---------------- camera ----------------
    pub fn s0(&self) -> f32 {
        self.sh / MH
    }
    pub fn kk(&self) -> f32 {
        self.s0() * self.cam.k
    }
    pub fn constrain(&self, mut c: Cam) -> Cam {
        c.k = c.k.clamp(1.0, 400.0);
        let ww = MW * self.s0() * c.k;
        let hh = self.sh * c.k;
        let mut x = c.x % ww;
        if x > 0.0 {
            x -= ww;
        }
        c.x = x;
        c.y = c.y.max(self.sh - hh).min(0.0);
        c
    }
    pub fn resize(&mut self, w: f32, h: f32, dpr: f32) {
        self.sw = w.max(1.0);
        self.sh = h.max(1.0);
        self.dpr = dpr.max(0.5);
        self.cam = self.constrain(self.cam);
    }
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.anim = None;
        if let Some(pc) = self.s.pc.as_mut() {
            pc.cam[0] += dx;
            pc.cam[1] += dy;
            return;
        }
        self.cam.x += dx;
        self.cam.y += dy;
        self.cam = self.constrain(self.cam);
    }
    pub fn zoom(&mut self, f: f32, fx: f32, fy: f32) {
        self.anim = None;
        if let Some(pc) = self.s.pc.as_mut() {
            let nk = (pc.cam[2] * f).clamp(0.2, 2000.0);
            let f2 = nk / pc.cam[2];
            pc.cam[0] = fx - (fx - pc.cam[0]) * f2;
            pc.cam[1] = fy - (fy - pc.cam[1]) * f2;
            pc.cam[2] = nk;
            return;
        }
        let nk = (self.cam.k * f).clamp(1.0, 400.0);
        let f2 = nk / self.cam.k;
        self.cam.x = fx - (fx - self.cam.x) * f2;
        self.cam.y = fy - (fy - self.cam.y) * f2;
        self.cam.k = nk;
        self.cam = self.constrain(self.cam);
    }
    pub fn to_map(&self, x: f32, y: f32) -> [f32; 2] {
        let k = self.kk();
        let mx = ((x - self.cam.x) / k).rem_euclid(MW);
        [mx, (y - self.cam.y) / k]
    }
    pub fn zoom_to_box(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let s = self.s0();
        let (w, h) = (x1 - x0, y1 - y0);
        let k = (0.6 / (w * s / self.sw).max(h * s / self.sh)).clamp(1.0, 60.0);
        let (cx, cy) = ((x0 + x1) / 2.0 * s, (y0 + y1) / 2.0 * s);
        let to = self.constrain(Cam { x: self.sw / 2.0 - cx * k, y: self.sh / 2.0 - cy * k, k });
        // keep x continuous for the animation
        let mut from = self.cam;
        let ww = MW * s * to.k;
        let _ = ww;
        from.x = self.cam.x;
        self.anim = Some((from, to, self.last_ms, 700.0));
    }
    pub fn zoom_to_nat(&mut self, n: u16) {
        let rs: Vec<u32> = if self.started { self.s.terr(n).clone() } else { (0..self.w.nreg() as u32).filter(|&r| self.w.regions[r as usize].nat == n).collect() };
        if rs.is_empty() {
            return;
        }
        let mut big = rs[0];
        for &r in &rs {
            if self.w.area(r as usize) > self.w.area(big as usize) {
                big = r;
            }
        }
        let c = self.w.lp(big as usize);
        let (mut x0, mut y0, mut x1, mut y1) = (1e9f32, 1e9f32, -1e9f32, -1e9f32);
        for &r in &rs {
            if (self.w.lp(r as usize)[0] - c[0]).abs() > 300.0 {
                continue;
            }
            let b = self.w.bb(r as usize);
            x0 = x0.min(b[0]);
            y0 = y0.min(b[1]);
            x1 = x1.max(b[2]);
            y1 = y1.max(b[3]);
        }
        self.zoom_to_box(x0, y0, x1, y1);
    }
    pub fn zoom_to_reg(&mut self, r: usize) {
        let b = self.w.bb(r);
        self.zoom_to_box(b[0], b[1], b[2], b[3]);
    }
    fn lod_for(&self, k: f32) -> usize {
        if k > 9.0 {
            2
        } else if k > 2.2 {
            1
        } else {
            0
        }
    }
    // region rings (mid LOD) for hit testing
    fn region_rings(&self, r: usize, li: usize) -> Vec<Vec<[f32; 2]>> {
        let l = &self.w.lods[li];
        let mut out = Vec::new();
        for poly in &l.shapes[r] {
            for ring in poly {
                let mut pts = Vec::new();
                for (n, &ai) in ring.iter().enumerate() {
                    let rev = ai < 0;
                    let a = crate::topo::arc_index(ai);
                    let f = &l.arcs[a];
                    let m = f.len();
                    for k in 0..m {
                        if k == 0 && n > 0 {
                            continue;
                        }
                        pts.push(f[if rev { m - 1 - k } else { k }]);
                    }
                }
                out.push(pts);
            }
        }
        out
    }
    pub fn hit(&self, x: f32, y: f32) -> i64 {
        let [mx, my] = self.to_map(x, y);
        let li = self.lod_for(self.cam.k).max(1);
        let m = self.w.meta();
        let mut b = -1i64;
        let mut ba = f32::MAX;
        for r in 0..self.w.nreg() {
            let bb = m.bb[r];
            if mx < bb[0] || mx > bb[2] || my < bb[1] || my > bb[3] {
                continue;
            }
            if m.area[r] < ba && point_in_rings(&self.region_rings(r, li), mx, my) {
                ba = m.area[r];
                b = r as i64;
            }
        }
        b
    }
    fn hit_water(&self, x: f32, y: f32) -> Option<(bool, usize)> {
        let [mx, my] = self.to_map(x, y);
        let mr = crate::render::w_rank(self.cam.k);
        let k = self.kk();
        let mut best = None;
        let mut ba = f32::MAX;
        for (i, l) in self.w.lakes.iter().enumerate() {
            if l.rank > mr {
                continue;
            }
            let b = l.bb;
            if mx < b[0] || mx > b[2] || my < b[1] || my > b[3] {
                continue;
            }
            if point_in_rings(&l.rings, mx, my) {
                let a = (b[2] - b[0]) * (b[3] - b[1]);
                if a < ba {
                    ba = a;
                    best = Some((true, i));
                }
            }
        }
        if best.is_some() {
            return best;
        }
        let tol = 10.0 * self.dpr / k;
        let mut bd = tol * tol;
        for (i, v) in self.w.rivers.iter().enumerate() {
            if v.rank > mr {
                continue;
            }
            let b = v.bb;
            if mx < b[0] - tol || mx > b[2] + tol || my < b[1] - tol || my > b[3] + tol {
                continue;
            }
            for l in &v.lines {
                for j in 1..l.len() {
                    let (ax, ay, bx, by) = (l[j - 1][0], l[j - 1][1], l[j][0], l[j][1]);
                    let (dx, dy) = (bx - ax, by - ay);
                    let l2 = dx * dx + dy * dy;
                    let u = if l2 > 0.0 { (((mx - ax) * dx + (my - ay) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                    let (ex, ey) = (ax + u * dx - mx, ay + u * dy - my);
                    let d = ex * ex + ey * ey;
                    if d < bd {
                        bd = d;
                        best = Some((false, i));
                    }
                }
            }
        }
        best
    }
    fn hit_sea(&self, x: f32, y: f32) -> i64 {
        let [mx, my] = self.to_map(x, y);
        let mut b = -1i64;
        let mut ba = f32::MAX;
        for (i, s) in self.w.seas.iter().enumerate() {
            if mx < s.bb[0] || mx > s.bb[2] || my < s.bb[1] || my > s.bb[3] {
                continue;
            }
            if s.area < ba && point_in_rings(&s.rings, mx, my) {
                ba = s.area;
                b = i as i64;
            }
        }
        if b < 0 {
            let mut bd = f32::MAX;
            for (i, s) in self.w.seas.iter().enumerate() {
                if !s.ocean {
                    continue;
                }
                let d = (s.c[0] - mx).powi(2) + (s.c[1] - my).powi(2);
                if d < bd {
                    bd = d;
                    b = i as i64;
                }
            }
        }
        b
    }
    fn hit_div(&self, cx: f32, cy: f32) -> Option<u32> {
        if !self.started || self.cam.k < 1.8 {
            return None;
        }
        let k = self.kk();
        let ww = MW * k;
        let mut b = None;
        let mut bd = 14.0 * self.dpr;
        for d in &self.s.g.divs {
            let mut c = -1;
            while (c as f32) <= (self.sw / ww).ceil() + 1.0 {
                let x = d.x * k + self.cam.x + c as f32 * ww;
                let y = d.y * k + self.cam.y;
                let dd = (cx - x).hypot(cy - y);
                if dd < bd {
                    bd = dd;
                    b = Some(d.id);
                }
                c += 1;
            }
        }
        b
    }

    pub fn close_panel(&mut self) {
        self.sel_w = None;
        self.sel_z = None;
        self.sel = -1;
        self.sel_sea = -1;
        self.ui_ver += 1;
    }

    // tap on the map: returns JSON action for the UI layer
    pub fn tap(&mut self, x: f32, y: f32) -> String {
        if self.s.pc.is_some() {
            return "{}".into();
        }
        self.ui_ver += 1;
        let r = self.hit(x, y);
        if self.started {
            if let Some(id) = self.hit_div(x, y) {
                self.s.sel_div = if self.s.sel_div == Some(id) { None } else { Some(id) };
                self.s.sels = if self.s.sel_div.is_some() { vec![id] } else { vec![] };
                if self.s.sel_div.is_some() {
                    if let Some(i) = self.s.div_index(id) {
                        let d = &self.s.g.divs[i];
                        let msg = if !d.path.is_empty() {
                            let pr = (100.0 * d.p_done / d.p_tot.max(1.0)).round();
                            format!("{} дивизия · {} · {}% · прибытие через {}", d.n, fmt_n(d.str_ as f64), pr, fmt_eta(self.s.div_eta(d)))
                        } else {
                            format!("{} дивизия · {} · нажмите куда идти", d.n, fmt_n(d.str_ as f64))
                        };
                        self.s.toast(msg);
                    }
                }
                self.close_panel();
                return "{}".into();
            }
            if self.s.sels.len() > 1 && r >= 0 {
                let [mx, my] = self.to_map(x, y);
                let ds = self.s.sel_divs();
                for di in ds {
                    let mut xx = mx;
                    let dx = self.s.g.divs[di].x;
                    while xx - dx > MW / 2.0 {
                        xx -= MW;
                    }
                    while xx - dx < -MW / 2.0 {
                        xx += MW;
                    }
                    let (a, b) = (self.s.rand() as f32, self.s.rand() as f32);
                    self.s.order_div(di, xx + (a - 0.5) * 0.8, my + (b - 0.5) * 0.5, r as u32, false);
                }
                return "{}".into();
            }
            if let Some(id) = self.s.sel_div {
                if r >= 0 {
                    if let Some(di) = self.s.div_index(id) {
                        let [mx, my] = self.to_map(x, y);
                        let mut xx = mx;
                        let dx = self.s.g.divs[di].x;
                        while xx - dx > MW / 2.0 {
                            xx -= MW;
                        }
                        while xx - dx < -MW / 2.0 {
                            xx += MW;
                        }
                        self.s.order_div(di, xx, my, r as u32, false);
                        return "{}".into();
                    }
                }
            }
            if self.s.sel_div.is_some() || !self.s.sels.is_empty() {
                self.s.sel_div = None;
                self.s.sels.clear();
            }
            if let Some(wt) = self.hit_water(x, y) {
                self.sel_w = Some(wt);
                self.sel_sea = -1;
                self.sel = -1;
                self.sel_z = None;
                return "{}".into();
            }
        }
        self.sel_w = None;
        if r < 0 {
            let si = self.hit_sea(x, y);
            if si >= 0 && self.started {
                self.sel_sea = si;
                self.sel = -1;
                self.sel_z = None;
                return "{}".into();
            }
            self.close_panel();
            return "{}".into();
        }
        self.sel_sea = -1;
        self.sel_z = None;
        let mut r = r;
        if self.started {
            let [mx, my] = self.to_map(x, y);
            let ci = self.s.cell_at(mx, my);
            let rg = self.w.reg[ci];
            let rr = if rg != 0 { rg as i64 - 1 } else { r };
            if self.s.region_zoned(rr as u32) || self.s.g.rg.contains_key(&(rr as u32)) {
                r = rr;
                let z = self.s.ctl[ci];
                let zz = if z != 0 && self.s.g.zones[z as usize - 1].r == r as u32 { z } else { 0 };
                self.sel_z = Some(SelZ { r: r as u32, z: zz, g: 0 });
            }
        }
        self.sel = r;
        if !self.started {
            let n = self.w.regions[r as usize].nat;
            return json!({"pick": n}).to_string();
        }
        self.resolve_group();
        "{}".into()
    }
    fn resolve_group(&mut self) {
        let i = self.sel;
        if i < 0 {
            return;
        }
        let i = i as u32;
        let mut g = 0;
        if let Some(sz) = self.sel_z {
            if sz.r == i && sz.z != 0 {
                g = self.s.g.zones[sz.z as usize - 1].g;
            }
        }
        if g == 0 {
            if let Some(&gg) = self.s.g.rg.get(&i) {
                if self.s.g.owner[i as usize] == self.s.g.player {
                    g = gg;
                }
            }
        }
        if g != 0 {
            let z = self.sel_z.filter(|s| s.r == i).map(|s| s.z).unwrap_or(0);
            self.sel_z = Some(SelZ { r: i, z, g });
            return;
        }
        if let Some(sz) = self.sel_z {
            if sz.r != i {
                self.sel_z = Some(SelZ { r: i, z: 0, g: 0 });
            }
            if !self.s.region_zoned(i) {
                self.sel_z = None;
            }
        }
    }

    // ---------------- UI JSON ----------------
    fn flag(&self, n: u16) -> String {
        self.w.nations[n as usize].f.clone()
    }
    fn btn(label: &str, sub: &str, cls: &str, en: bool, cmd: &str, a: i64, b: i64, icon: &str) -> Value {
        json!({"label":label,"sub":sub,"cls":cls,"en":en,"cmd":cmd,"a":a,"b":b,"icon":icon})
    }
    fn own_buttons(&self, i: u32, z: u16, divcmd: &str) -> Vec<Value> {
        let s = &self.s;
        let m = &s.g.nat[s.g.player as usize];
        vec![
            App::btn("Мобилизация", &format!("${}", fmt(s.rec_cost())), "p", m.money >= s.rec_cost(), "act", 0, 0, "users"),
            App::btn("В регион", &format!("${}", fmt(s.inv_cost(i as usize))), "", m.money >= s.inv_cost(i as usize), "inv", i as i64, 0, "factory"),
            App::btn("Пехотная дивизия", &format!("${} · 10 тыс", fmt(s.div_cost())), "", m.money >= s.div_cost() && m.army >= 12000.0, divcmd, i as i64, z as i64, "shield"),
            App::btn("Наука", &format!("${}", fmt(s.tech_cost())), "", m.money >= s.tech_cost(), "tech", 0, 0, "flask-conical"),
            App::btn("Тест: +10 дивизий", "", "", true, "test", i as i64, z as i64, "plus"),
        ]
    }
    pub fn panel_json(&self) -> Value {
        if let Some((lake, i)) = self.sel_w {
            let (n, t, img) = if lake {
                let l = &self.w.lakes[i];
                (l.name.clone(), l.kind.clone(), "lake")
            } else {
                let v = &self.w.rivers[i];
                (v.name.clone(), v.kind.clone(), if v.canal { "strait" } else { "riverv" })
            };
            return json!({"type":"sea","title":n,"tag":t,"img":img});
        }
        if self.sel_sea >= 0 {
            let s = &self.w.seas[self.sel_sea as usize];
            let img = match s.kind.as_str() {
                "Океан" => "ocean",
                "Море" => "sea",
                "Залив" => "bay",
                "Пролив" => "strait",
                "Лагуна" => "lagoon",
                "Фьорд" => "fjord",
                "Риф" => "reef",
                "Эстуарий" => "river",
                _ => "generic",
            };
            return json!({"type":"sea","title":s.name,"tag":s.kind,"img":img});
        }
        if self.sel < 0 || !self.started {
            return Value::Null;
        }
        let i = self.sel as u32;
        let s = &self.s;
        let me = s.g.player;
        let iu = i as usize;
        if let Some(sz) = self.sel_z {
            if sz.g != 0 {
                // group panel
                let (mut p, mut gd, mut n, mut o) = (0f64, 0f64, String::new(), me);
                for z in &s.g.zones {
                    if z.g == sz.g && z.c > 0 && self.w.rc[z.r as usize] > 0 {
                        let sh = (z.c as f64 / self.w.rc[z.r as usize] as f64).min(1.0);
                        p += s.g.pop[z.r as usize] * sh;
                        gd += s.g.gdp[z.r as usize] * sh;
                        n = z.n.clone();
                        o = z.o;
                    }
                }
                for (&r, &g) in &s.g.rg {
                    if g == sz.g && s.g.owner[r as usize] == me {
                        p += s.g.pop[r as usize];
                        gd += s.g.gdp[r as usize];
                        if n.is_empty() {
                            n = s.rn[r as usize].clone();
                        }
                    }
                }
                let mut bt = if o == me { self.own_buttons(i, sz.z, "divz") } else { vec![] };
                bt.push(App::btn("Регион", "", "", true, "zoomreg", i as i64, 0, "zoom-in"));
                bt.push(App::btn("Страна", "", "", true, "zoomnat", o as i64, 0, "map"));
                return json!({"type":"region","title":s.nm(o),"flag":self.flag(o),"tag":n,
                  "rows":[["Население региона",fmt_n(p)],["ВВП региона",format!("${}",fmt(gd))],["ВВП державы",format!("${}",fmt(s.income(o)))],["Население державы",fmt_n(s.pop_of(o))]],
                  "buttons":bt});
            }
            if s.region_zoned(i) {
                let z = sz.z;
                let zz = if z != 0 { Some(&s.g.zones[z as usize - 1]) } else { None };
                let own = s.g.owner[iu];
                let ctl = zz.map(|z| z.o).unwrap_or(own);
                let rc = self.w.rc[iu];
                let cells = match zz {
                    Some(z) => z.c,
                    None => rc.saturating_sub(s.zone_sum(i)),
                };
                let sh = if rc > 0 { cells as f64 / rc as f64 } else { 0.0 };
                let tag = match zz {
                    Some(z) if z.k == 'o' => format!("Оккупационная зона: {} в «{}»", s.nm(z.o), s.rn[iu]),
                    Some(z) => format!("{} · {}", z.n, s.nm(z.o)),
                    None => format!("Под контролем {}: {}", s.nm(own), s.rn[iu]),
                };
                let mut bt = if ctl == me { self.own_buttons(i, z, "divz") } else { vec![] };
                let foe = if own != me { Some(own) } else if ctl != me { Some(ctl) } else { None };
                if let Some(f) = foe {
                    if s.g.war.contains(&f) {
                        bt.push(App::btn("Мирная конференция", "", "", true, "peace", f as i64, 0, "bird"));
                    }
                }
                bt.push(App::btn("Регион", "", "", true, "zoomreg", i as i64, 0, "zoom-in"));
                bt.push(App::btn("Страна", "", "", true, "zoomnat", own as i64, 0, "map"));
                return json!({"type":"region","title":s.nm(ctl),"flag":self.flag(ctl),"tag":tag,
                  "rows":[["Доля региона",format!("{:.1}%",sh*100.0)],["Население зоны",fmt_n(s.g.pop[iu]*sh)],["ВВП зоны",format!("${}",fmt(s.g.gdp[iu]*sh))],["Регион",s.rn[iu].clone()],["Законный владелец",s.nm(own)]],
                  "buttons":bt});
            }
        }
        let o = s.g.owner[iu];
        let n = &s.g.nat[o as usize];
        let orig = self.w.regions[iu].nat;
        let mut rows = vec![
            json!(["ВВП региона", format!("${}", fmt(s.g.gdp[iu]))]),
            json!(["Население региона", fmt_n(s.g.pop[iu])]),
            json!(["ВВП державы", format!("${}", fmt(s.income(o)))]),
            json!(["Население державы", fmt_n(s.pop_of(o))]),
            json!(["Армия", fmt_n(n.army)]),
            json!(["Технологии", format!("ур. {:.1}", n.tech)]),
            json!(["Регионов", s.terr(o).len().to_string()]),
        ];
        if o != me {
            rows.push(json!(["Отношения", s.rel_str(o)]));
        }
        let mut bt = Vec::new();
        if o == me {
            bt = self.own_buttons(i, 0, "div");
        } else {
            let war = s.g.war.contains(&o);
            let ally = s.g.allies.contains(&o);
            let m = &s.g.nat[me as usize];
            if war {
                bt.push(App::btn("Мирная конференция", "", "", true, "peace", o as i64, 0, "bird"));
            } else {
                bt.push(App::btn("Объявить войну", "", "r", !ally, "war", o as i64, 0, "swords"));
                bt.push(App::btn("Союз", "", "g", !ally, "ally", o as i64, 0, "handshake"));
            }
            bt.push(App::btn("Торговля", "", "", !war, "trade", o as i64, 0, "package"));
            bt.push(App::btn("Санкции", "", "", true, "sanc", o as i64, 0, "ban"));
            bt.push(App::btn("Переворот", &format!("${}", fmt(s.spy_cost())), "", m.money >= s.spy_cost(), "spy", o as i64, 0, "venetian-mask"));
        }
        bt.push(App::btn("Регион", "", "", true, "zoomreg", i as i64, 0, "zoom-in"));
        bt.push(App::btn("Страна", "", "", true, "zoomnat", o as i64, 0, "map"));
        let mut v = json!({"type":"region","title":s.nm(o),"flag":self.flag(o),"tag":s.rn[iu],"rows":rows,"buttons":bt});
        if o != orig {
            v["hist"] = json!({"flag": self.flag(orig), "name": s.nm(orig)});
        }
        v
    }
    pub fn hud_json(&self) -> Value {
        if !self.started {
            return Value::Null;
        }
        let s = &self.s;
        let p = s.g.player;
        let n = &s.g.nat[p as usize];
        json!({"name":s.nm(p),"flag":self.flag(p),"money":format!("${}",fmt(n.money)),"inc":format!(" +{}/мес",fmt(s.daily_inc(p)*30.0)),
          "army":fmt_n(n.army),"pop":fmt_n(s.pop_of(p)),"date":s.date_str(),"clock":self.clock_str(),"speed":s.speed})
    }
    pub fn clock_str(&self) -> String {
        let h = (self.s.g.hour % 24) as f64 + self.s.h_acc.min(0.999);
        let m = (h * 60.0) as i64;
        let (hh, mm) = (m / 60, m % 60);
        format!("{:02}:{:02}", hh, if self.s.speed >= 3 { 0 } else { mm - mm % 10 })
    }
    pub fn ftb_json(&self) -> Value {
        if !self.started || self.s.g.divs.is_empty() {
            return Value::Null;
        }
        let n = if !self.s.sels.is_empty() { self.s.sels.len() } else if self.s.sel_div.is_some() { 1 } else { 0 };
        let ready = self.s.g.fronts.iter().any(|f| f.plan.is_some() && !f.exec);
        json!({"n":n,"sel":!self.s.sels.is_empty(),"draw":self.draw.clone().unwrap_or_default(),"ready":ready,"debug":self.debug})
    }
    pub fn ui_json(&mut self) -> String {
        let toasts = std::mem::take(&mut self.s.toasts);
        let over = self.s.game_over.take();
        json!({"hud":self.hud_json(),"panel":self.panel_json(),"ftb":self.ftb_json(),"toasts":toasts,"over":over,
          "pc": if self.s.pc.is_some() { serde_json::from_str::<Value>(&self.s.pc_state_json()).unwrap_or(Value::Null) } else { Value::Null },
          "started":self.started})
        .to_string()
    }
    pub fn nations_json(&self) -> String {
        let mut order: Vec<usize> = (0..self.w.nations.len()).filter(|&i| self.w.nations[i].pop > 0.0 && self.w.regions.iter().any(|r| r.nat as usize == i)).collect();
        order.sort_by(|&a, &b| self.w.nations[b].gdp.partial_cmp(&self.w.nations[a].gdp).unwrap());
        let v: Vec<Value> = order
            .iter()
            .map(|&i| {
                let n = &self.w.nations[i];
                let c = nat_color(i, &n.iso, &n.e);
                json!({"i":i,"n":n.n,"e":n.e,"f":n.f,"gdp":format!("${}",fmt(n.gdp)),"pop":fmt_n(n.pop),"col":format!("#{:02x}{:02x}{:02x}",c[0],c[1],c[2])})
            })
            .collect();
        json!(v).to_string()
    }
    pub fn nation_info(&self, i: usize) -> String {
        let n = &self.w.nations[i];
        json!({"i":i,"n":n.n,"f":n.f,"pop":fmt_n(n.pop),"gdp":format!("${}",fmt(n.gdp))}).to_string()
    }

    // ---------------- commands ----------------
    pub fn cmd(&mut self, c: &str, a: i64, b: i64, sarg: &str) -> String {
        self.ui_ver += 1;
        match c {
            "new" => {
                self.s.new_game(a as u16);
                self.after_start();
                self.zoom_to_nat(a as u16);
            }
            "load" => {
                if self.s.load_json(sarg) {
                    self.after_start();
                    let p = self.s.g.player;
                    self.zoom_to_nat(p);
                    return "ok".into();
                }
                return "fail".into();
            }
            "save" => return self.s.save_json(),
            "restart" => {
                self.started = false;
                self.s.g = Default::default();
                self.s.pc = None;
                self.draw = None;
                self.drawp.clear();
                self.s.sels.clear();
                self.s.sel_div = None;
                self.close_panel();
                self.zt = ZoneTex::default();
                self.s.ctl.iter_mut().for_each(|c| *c = 0);
                self.s.ctl_ver += 1;
                self.own_seen = 0;
                self.s.lbl_dirty = true;
                if let Some(r) = self.r.as_mut() {
                    r.zon = false;
                    r.z_alloc = false;
                }
            }
            "speed" => self.s.speed = a.clamp(0, 4) as usize,
            "pause" => self.paused = a != 0,
            "act" => self.s.act("rec", 0),
            "inv" | "tech" | "war" | "ally" | "trade" | "sanc" | "spy" => {
                self.s.act(c, a);
                if c == "war" {
                    self.own_seen = 0;
                }
            }
            "div" => {
                self.s.new_div(a as u32, None, None, false);
                self.close_panel();
            }
            "divz" => {
                self.s.new_div_z(a as u32, b as u16);
                self.close_panel();
            }
            "test" => self.s.test_divs(a as u32, b as u16),
            "peace" => {
                self.close_panel();
                self.s.open_peace(a as u16);
            }
            "zoomreg" => self.zoom_to_reg(a as usize),
            "zoomnat" => self.zoom_to_nat(a as u16),
            "mynat" => {
                let p = self.s.g.player;
                self.zoom_to_nat(p);
            }
            "close" => self.close_panel(),
            "selall" => {
                if self.s.sels.len() == self.s.g.divs.len() && !self.s.sels.is_empty() {
                    self.s.sels.clear();
                    self.s.sel_div = None;
                } else {
                    self.s.sels = self.s.g.divs.iter().map(|d| d.id).collect();
                }
            }
            "draw" => {
                let m = sarg.to_string();
                self.draw = if self.draw.as_deref() == Some(&m) { None } else { Some(m) };
                self.drawp.clear();
                if let Some(d) = &self.draw {
                    self.s.toast(match d.as_str() {
                        "spear" => "Проведите стрелку прорыва",
                        "front" => "Нажмите на страну или проведите линию фронта",
                        _ => "Проведите линию плана наступления",
                    });
                }
            }
            "exec" => self.s.exec_plans(),
            "debug" => self.debug = !self.debug,
            "pctool" => {
                if let Some(pc) = self.s.pc.as_mut() {
                    pc.tool = sarg.to_string();
                }
            }
            "pcbr" => {
                if let Some(pc) = self.s.pc.as_mut() {
                    pc.br = a as f32;
                }
            }
            "pcall" => self.s.pc_all(a != 0),
            "pcitem" => self.s.pc_item(if a == 0 { "new" } else { "add" }, sarg, b),
            "pcdel" => self.s.pc_del(a as usize),
            "pcsign" => {
                self.s.pc_sign();
                self.own_seen = 0;
            }
            "pcclose" => self.s.pc_close(),
            _ => {}
        }
        "{}".into()
    }
    pub fn g_plan(&mut self, fi: usize, p: Vec<[f32; 2]>) {
        self.s.g.fronts[fi].plan = Some(p);
        self.s.g.fronts[fi].exec = false;
    }
    fn after_start(&mut self) {
        self.started = true;
        self.zt = ZoneTex::default();
        self.s.zone_full = true;
        self.own_seen = 0;
        self.s.lbl_dirty = true;
        self.close_panel();
        self.draw = None;
        self.drawp.clear();
        self.s.sels.clear();
        self.s.sel_div = None;
    }

    // drawing mode (front/plan/spear) touch stream; returns true if consumed
    pub fn draw_touch(&mut self, phase: i32, x: f32, y: f32) -> bool {
        if self.draw.is_none() {
            return false;
        }
        let q = self.to_map(x, y);
        match phase {
            0 => {
                self.draw_dn = [x, y];
                self.drawp = vec![q];
            }
            1 => {
                if let Some(l) = self.drawp.last().copied() {
                    let mut xx = q[0];
                    while xx - l[0] > MW / 2.0 {
                        xx -= MW;
                    }
                    while xx - l[0] < -MW / 2.0 {
                        xx += MW;
                    }
                    if (xx - l[0]).hypot(q[1] - l[1]) * self.kk() > 8.0 * self.dpr {
                        self.drawp.push([xx, q[1]]);
                    }
                }
            }
            _ => {
                let p = std::mem::take(&mut self.drawp);
                let m = self.draw.take().unwrap();
                self.ui_ver += 1;
                if m == "front" && (x - self.draw_dn[0]).hypot(y - self.draw_dn[1]) < 12.0 * self.dpr {
                    self.s.front_on_nation(q[0], q[1]);
                    return true;
                }
                if p.len() < 2 {
                    return true;
                }
                match m.as_str() {
                    "front" => self.s.make_front(p, None),
                    "spear" => self.s.make_spear(p),
                    _ => self.s.make_plan(p),
                }
            }
        }
        true
    }
    // peace conference touch (brush); returns true if consumed
    pub fn pc_touch(&mut self, phase: i32, x: f32, y: f32) -> bool {
        let pc = match self.s.pc.as_ref() {
            Some(p) => p,
            None => return false,
        };
        if pc.tool == "pan" {
            return false;
        }
        let k = pc.cam[2];
        let mx = (x - pc.cam[0]) / k;
        let my = (y - pc.cam[1]) / k;
        self.s.pc_brush(mx * GW as f32 / MW, my * GH as f32 / MH);
        if phase == 2 {
            self.ui_ver += 1;
        }
        true
    }
    pub fn pc_set_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.pc_rect = [x, y, w, h];
        if let Some(pc) = self.s.pc.as_mut() {
            if !pc.cam_init && w > 10.0 {
                let b = pc.bbox;
                let pad = 0.15 * (b[2] - b[0]).max((b[3] - b[1]) * 2.0);
                let k = (w / (b[2] - b[0] + 2.0 * pad)).min(h / (b[3] - b[1] + 2.0 * pad));
                pc.cam = [x + w / 2.0 - k * (b[0] + b[2]) / 2.0, y + h / 2.0 - k * (b[1] + b[3]) / 2.0, k];
                pc.cam_init = true;
            }
        }
    }

    // ---------------- simulation ----------------
    fn pf(&mut self, k: &'static str, dt: f64) {
        *self.pf_acc.entry(k).or_insert(0.0) += dt;
    }
    pub fn hour_tick(&mut self) -> bool {
        let clock = self.clock;
        for d in self.s.g.divs.iter_mut() {
            d.ox = d.x;
            d.oy = d.y;
        }
        let t0 = clock();
        let mv = !self.s.g.divs.is_empty() && self.s.move_divs(1.0);
        self.s.g.hour += 1;
        self.pf_ticks += 1;
        let t1 = clock();
        self.pf("движение", t1 - t0);
        if mv {
            self.s.hmv = true;
        }
        if !self.s.g.fronts.is_empty() {
            let mut fi = 0;
            while fi < self.s.g.fronts.len() {
                if self.s.g.fronts[fi].exec {
                    if self.s.g.fronts[fi].plan.is_some() {
                        self.s.plan_tick(fi);
                    } else {
                        self.s.g.fronts[fi].exec = false;
                    }
                }
                fi += 1;
            }
            let t2 = clock();
            self.pf("план", t2 - t1);
            self.s.cover_tick();
            self.pf("бреши", clock() - t2);
        }
        if self.s.g.hour % 24 == 0 {
            let t3 = clock();
            self.s.day_tick();
            self.pf("экономика", clock() - t3);
        }
        mv
    }
    pub fn step(&mut self, now: f64) {
        let clock = self.clock;
        let dt = if self.last_ms > 0.0 { ((now - self.last_ms) / 1000.0).min(1.0).max(0.0) } else { 0.0 };
        self.last_ms = now;
        self.s.now_ms = now;
        // camera animation
        if let Some((a, b, t0, dur)) = self.anim {
            let t = ((now - t0) / dur).clamp(0.0, 1.0) as f32;
            let e = if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 };
            // interpolate in log-zoom around screen centre
            let (cx, cy) = (self.sw / 2.0, self.sh / 2.0);
            let ma = [(cx - a.x) / a.k, (cy - a.y) / a.k];
            let mb = [(cx - b.x) / b.k, (cy - b.y) / b.k];
            let s0 = self.s0() * MW;
            let mut dmx = mb[0] - ma[0];
            let wa = s0;
            let _ = wa;
            let period = MW * self.s0();
            while dmx > period / 2.0 {
                dmx -= period;
            }
            while dmx < -period / 2.0 {
                dmx += period;
            }
            let k = (a.k.ln() + (b.k.ln() - a.k.ln()) * e).exp();
            let m = [ma[0] + dmx * e, ma[1] + (mb[1] - ma[1]) * e];
            self.cam = self.constrain(Cam { x: cx - m[0] * k, y: cy - m[1] * k, k });
            if t >= 1.0 {
                self.anim = None;
            }
        }
        if !self.started {
            return;
        }
        if self.s.pc.is_none() && !self.paused && self.s.speed > 0 {
            self.s.h_acc = (self.s.h_acc + dt * SPEEDS[self.s.speed] * 24.0).min(480.0);
            let tb0 = clock();
            let mut n = 0;
            while self.s.h_acc >= 1.0 && n < 480 && (n < 1 || clock() - tb0 < 10.0) {
                self.s.h_acc -= 1.0;
                n += 1;
                self.hour_tick();
            }
            if self.s.hmv && now - self.s.sm_t > 250.0 {
                self.s.sm_t = now;
                self.s.hmv = false;
                let b = std::mem::take(&mut self.s.dbl);
                self.s.osq.extend(b);
            }
            if !self.s.osq.is_empty() {
                let t0 = clock();
                let me = self.s.g.player;
                while !self.s.osq.is_empty() && clock() - t0 < 3.0 {
                    let b = self.s.osq.remove(0);
                    self.s.occ_smooth_box(me, b);
                }
                self.pf("сглаживание", clock() - t0);
            }
            self.s.divf = self.s.h_acc.min(1.0) as f32;
        }
        if !self.s.anq.is_empty() && now - self.pk_t > 16.0 {
            self.pk_t = now;
            self.s.step_pockets();
        }
        if self.s.zone_full {
            self.s.zone_full = false;
            let mut zt = std::mem::take(&mut self.zt);
            self.s.zone_flush(&mut zt, true, 0.0, &clock);
            self.s.anx_lines(&mut zt);
            zt.anx_t = now;
            self.zt = zt;
            self.z_flush_t = now;
        } else if !self.s.chr.is_empty() && now - self.z_flush_t > 90.0 {
            self.z_flush_t = now;
            let t0 = clock();
            let mut zt = std::mem::take(&mut self.zt);
            self.s.zone_flush(&mut zt, false, 4.0, &clock);
            if now - zt.anx_t > 6000.0 {
                zt.anx_t = now;
                self.s.anx_lines(&mut zt);
            }
            self.zt = zt;
            self.pf("карта", clock() - t0);
        }
        if !self.zt.cutq.is_empty() && now - self.cut_t > 600.0 {
            self.cut_t = now;
            let t0 = clock();
            let mut zt = std::mem::take(&mut self.zt);
            self.s.cut_flush(&mut zt, 4.0, &clock);
            self.zt = zt;
            self.pf("линии", clock() - t0);
        }
        if self.s.dirty_ui {
            self.s.dirty_ui = false;
            self.ui_ver += 1;
        }
    }

    // ---------------- rendering ----------------
    pub fn upload(&mut self) {
        let r = match self.r.as_mut() {
            Some(r) => r,
            None => return,
        };
        let pcx = self.s.pc.as_ref().map(|p| p.x);
        let key = self.s.own_ver.wrapping_add(if pcx.is_some() { 7919 } else { 0 });
        if self.own_seen != key {
            self.own_seen = key;
            let n = self.w.nreg();
            let mut col = vec![0u8; TW * r.th * 4];
            let mut own = vec![0u32; TW * r.th * 4];
            let started = self.started;
            let me = self.s.g.player;
            for i in 0..n {
                let o = if started { self.s.g.owner[i] } else { self.w.regions[i].nat };
                let nd = &self.w.nations[o as usize];
                let c = nat_color(o as usize, &nd.iso, &nd.e);
                let a = match pcx {
                    Some(x) => {
                        if o == me || o == x {
                            255
                        } else {
                            0
                        }
                    }
                    None => 255,
                };
                col[4 * i..4 * i + 4].copy_from_slice(&[c[0], c[1], c[2], a]);
                own[4 * i] = o as u32;
                own[4 * i + 1] = if started && self.s.g.war.contains(&o) { 1 } else { 0 };
                own[4 * i + 2] = if started { self.s.g.rg.get(&(i as u32)).copied().unwrap_or(0) } else { 0 };
            }
            unsafe { r.upload_regions(&col, &own) };
        }
        if self.zt.dirty.is_some() || self.zt.full {
            let zh = (self.s.g.zones.len() + 1 + TW - 1) / TW;
            let zh = zh.max(1);
            let mut inf = vec![0u32; TW * zh * 4];
            for (i, z) in self.s.g.zones.iter().enumerate() {
                let nd = &self.w.nations[z.o as usize];
                let c = nat_color(z.o as usize, &nd.iso, &nd.e);
                let q = 4 * (i + 1);
                inf[q] = z.r;
                inf[q + 1] = (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32;
                inf[q + 2] = z.o as u32;
                inf[q + 3] = z.g | if z.k == 'o' { 0x80000000 } else { 0 };
            }
            unsafe { r.upload_zones(&self.zt.zid, &self.zt.ff, self.zt.dirty, self.zt.full, &inf, zh) };
            r.zon = self.s.g.zones.iter().any(|z| z.c > 0);
            self.zt.dirty = None;
            self.zt.full = false;
        }
        if self.anx_seen != self.zt.anx_ver {
            unsafe { r.set_anx(&self.zt.anx_segs) };
            self.anx_seen = self.zt.anx_ver;
        }
    }

    pub fn frame(&mut self, now: f64) {
        let clock = self.clock;
        self.pf_frames += 1;
        if now - self.pf_t0 >= 1000.0 {
            let el = (now - self.pf_t0) / 1000.0;
            let mut out = vec![("fps".to_string(), self.pf_frames as f64 / el), ("tps".to_string(), self.pf_ticks as f64 / el)];
            let mut keys: Vec<_> = self.pf_acc.iter().map(|(k, v)| (k.to_string(), *v / self.pf_frames.max(1) as f64)).collect();
            keys.sort_by(|a, b| a.0.cmp(&b.0));
            out.extend(keys);
            self.prof = out;
            self.pf_acc.clear();
            self.pf_frames = 0;
            self.pf_ticks = 0;
            self.pf_t0 = now;
        }
        self.step(now);
        if self.r.is_none() {
            return;
        }
        if self.s.lbl_dirty && now - self.lbl_t > 300.0 {
            self.lbl_t = now;
            self.s.lbl_dirty = false;
            let owners: Vec<u16> = if self.started { self.s.g.owner.clone() } else { self.w.regions.iter().map(|r| r.nat).collect() };
            self.lbl.compute(&self.s, &self.at, &owners);
        }
        self.upload();
        let t0 = clock();
        if self.s.pc.is_some() {
            self.render_pc();
        } else {
            self.render_main();
        }
        self.pf("рендер", clock() - t0);
    }

    fn view(&self, ox: f32, oy: f32, kd: f32) -> View {
        View { res: [self.sw, self.sh], kd, off: [ox, oy], vx0: -ox / kd, vx1: (self.sw - ox) / kd, vy0: -oy / kd, vy1: (self.sh - oy) / kd }
    }
    fn render_main(&mut self) {
        let k = self.cam.k;
        let kd = self.kk();
        let ww = MW * kd;
        let li = self.lod_for(k);
        let conq = self.started && (0..self.w.nreg()).any(|r| self.s.g.owner[r] != self.w.regions[r].nat);
        {
            let r = self.r.as_ref().unwrap();
            let gl = &r.gl;
            unsafe {
                gl.viewport(0, 0, self.sw as i32, self.sh as i32);
                gl.clear_color(0.05, 0.2, 0.35, 1.0);
                gl.clear_stencil(0);
                gl.clear(glow::COLOR_BUFFER_BIT | glow::STENCIL_BUFFER_BIT);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            }
            let mut c = 0;
            while self.cam.x + c as f32 * ww < self.sw {
                let ox = self.cam.x + c as f32 * ww;
                c += 1;
                if ox + ww < 0.0 {
                    continue;
                }
                let v = self.view(ox, self.cam.y, kd);
                unsafe {
                    r.gl.clear(glow::STENCIL_BUFFER_BIT);
                    r.draw_map(&v, k, self.dpr, li, conq, false, 0.88);
                }
            }
        }
        self.batch.clear();
        let mut c = 0;
        while self.cam.x + c as f32 * ww < self.sw {
            let ox = self.cam.x + c as f32 * ww;
            c += 1;
            if ox + ww < 0.0 {
                continue;
            }
            self.overlay(ox, kd);
        }
        self.overlay_screen();
        self.flush_batch();
    }
    fn flush_batch(&mut self) {
        let r = self.r.as_ref().unwrap();
        if self.at.full || self.atlas_gen != self.at.gen {
            unsafe { r.upload_atlas(crate::batch::AW, crate::batch::AH, &self.at.px, true, [0, 0, 0, 0]) };
            self.at.full = false;
            self.at.dirty = None;
            self.atlas_gen = self.at.gen;
        } else if let Some(d) = self.at.dirty.take() {
            unsafe { r.upload_atlas(crate::batch::AW, crate::batch::AH, &self.at.px, false, d) };
        }
        unsafe { r.draw_overlay([self.sw, self.sh], &self.batch.v) };
    }

    fn m2s(&self, p: [f32; 2], ox: f32, kd: f32) -> [f32; 2] {
        [p[0] * kd + ox, p[1] * kd + self.cam.y]
    }
    fn outline_region(&mut self, r: usize, ox: f32, kd: f32, w: f32, c: Col) {
        let li = self.lod_for(self.cam.k);
        let shapes = self.w.lods[li].shapes[r].clone();
        for poly in &shapes {
            for ring in poly {
                for &ai in ring {
                    let a = crate::topo::arc_index(ai);
                    let f = &self.w.lods[li].arcs[a];
                    let pts: Vec<[f32; 2]> = f.iter().map(|p| [p[0] * kd + ox, p[1] * kd + self.cam.y]).collect();
                    for j in 1..pts.len() {
                        if crate::world::World::edge_like(f[j - 1], f[j]) {
                            continue;
                        }
                        self.batch.line(pts[j - 1], pts[j], w, c);
                    }
                }
            }
        }
    }
    fn overlay(&mut self, ox: f32, kd: f32) {
        let dpr = self.dpr;
        let k = self.cam.k;
        let (vx0, vx1, vy0, vy1) = (-ox / kd, (self.sw - ox) / kd, -self.cam.y / kd, (self.sh - self.cam.y) / kd);
        // water selection
        if let Some((lake, i)) = self.sel_w {
            if lake {
                let name = self.w.lakes[i].name.clone();
                for l in self.w.lakes.iter().filter(|l| l.name == name) {
                    for ring in &l.rings {
                        let p: Vec<[f32; 2]> = ring.iter().map(|q| [q[0] * kd + ox, q[1] * kd + self.cam.y]).collect();
                        self.batch.dashed(&p, 2.5 * dpr, rgba(127, 208, 255, 1.0), 6.0 * dpr, 4.0 * dpr);
                    }
                }
            } else {
                let name = self.w.rivers[i].name.clone();
                let lines: Vec<Vec<[f32; 2]>> = self.w.rivers.iter().filter(|v| v.name == name).flat_map(|v| v.lines.clone()).collect();
                for l in lines {
                    let p: Vec<[f32; 2]> = l.iter().map(|q| [q[0] * kd + ox, q[1] * kd + self.cam.y]).collect();
                    self.batch.polyline(&p, 4.0 * dpr, rgba(127, 208, 255, 0.9));
                }
            }
        }
        if self.sel_sea >= 0 {
            let oi = self.w.seas[self.sel_sea as usize].outline;
            let segs = self.w.sea_ol[oi].clone();
            for s in segs {
                let a = [s[0] * kd + ox, s[1] * kd + self.cam.y];
                let b = [s[2] * kd + ox, s[3] * kd + self.cam.y];
                self.batch.line(a, b, 2.0 * dpr, rgba(127, 208, 255, 1.0));
            }
        }
        if self.sel >= 0 && self.sel_z.is_none() {
            self.outline_region(self.sel as usize, ox, kd, 2.5 * dpr, rgba(255, 216, 74, 1.0));
        }
        if self.started {
            // annexed/occupied cut lines
            let cuts: Vec<Vec<f32>> = self.zt.cut.values().map(|v| v.1.clone()).collect();
            for segs in cuts {
                for s in segs.chunks(4) {
                    if (s[0] < vx0 && s[2] < vx0) || (s[0] > vx1 && s[2] > vx1) || (s[1] < vy0 && s[3] < vy0) || (s[1] > vy1 && s[3] > vy1) {
                        continue;
                    }
                    self.batch.line(self.m2s([s[0], s[1]], ox, kd), self.m2s([s[2], s[3]], ox, kd), 1.5 * dpr, rgba(0, 0, 0, 0.9));
                }
            }
            if let Some(sz) = self.sel_z {
                self.draw_sel(sz, ox, kd);
            }
        }
        self.draw_cities(ox, kd);
        if k >= 1.6 {
            let canals: Vec<Vec<[f32; 2]>> = self.w.rivers.iter().filter(|v| v.canal).map(|v| v.lines[0].clone()).collect();
            for l in canals {
                let p: Vec<[f32; 2]> = l.iter().map(|q| self.m2s(*q, ox, kd)).collect();
                self.batch.polyline(&p, 3.2 * dpr, rgba(0, 0, 0, 0.6));
                self.batch.polyline(&p, 1.8 * dpr, rgba(61, 143, 214, 1.0));
            }
        }
        if self.started {
            self.draw_fronts(ox, kd);
            if self.debug {
                self.draw_plan_dbg(ox, kd);
                self.draw_cover_dbg(ox, kd);
            }
            self.draw_divs(ox, kd);
        }
        self.draw_labels(ox, kd);
    }
    fn overlay_screen(&mut self) {
        if !self.debug || !self.started {
            return;
        }
        let dpr = self.dpr;
        let mut y = 150.0 * dpr;
        let prof = self.prof.clone();
        self.batch.rect(6.0 * dpr, y - 12.0 * dpr, 230.0 * dpr, 14.0 * dpr * (prof.len() as f32 + 1.0), rgba(0, 0, 0, 0.55));
        for (k, v) in prof {
            let s = if k == "fps" || k == "tps" { format!("{}: {:.0}", k, v) } else { format!("{}: {:.2} мс/кадр", k, v) };
            self.batch.text(&mut self.at, 0, &s, 10.0 * dpr, y - 4.0 * dpr, 11.0 * dpr, rgba(150, 255, 150, 1.0), None, 0);
            y += 14.0 * dpr;
        }
        let h = self.s.g.hour;
        let s = format!("тик {} ({} д {} ч)", h, h / 24, h % 24);
        self.batch.text(&mut self.at, 0, &s, 10.0 * dpr, 76.0 * dpr, 11.0 * dpr, rgba(255, 255, 255, 1.0), Some(rgba(0, 0, 0, 0.6)), 0);
    }
    fn draw_sel(&mut self, sz: SelZ, ox: f32, kd: f32) {
        let key = (self.s.ctl_ver, (sz.r as u64) << 32 | (sz.z as u64) << 16 | sz.g as u64);
        let segs = match &self.sel_cache {
            Some((v, kk, s)) if *v == key.0 && *kk == key.1 => s.clone(),
            _ => {
                let mut out: Vec<[f32; 4]> = Vec::new();
                let mut add = |s: &Sim, r: u32, test: &dyn Fn(usize) -> bool| {
                    let reg = &s.w.reg;
                    let r1 = (r + 1) as u16;
                    let l = s.mask_lines(r, &|k| reg[k] == r1 && test(k));
                    for c in l.chunks(4) {
                        out.push([c[0], c[1], c[2], c[3]]);
                    }
                };
                let s = &self.s;
                if sz.g != 0 {
                    for (id, z) in s.g.zones.iter().enumerate() {
                        if z.g == sz.g && z.c > 0 {
                            let idd = (id + 1) as u16;
                            add(s, z.r, &|k| s.ctl[k] == idd);
                        }
                    }
                    for (&r, &g) in &s.g.rg {
                        if g == sz.g && s.g.owner[r as usize] == s.g.player {
                            add(s, r, &|_| true);
                        }
                    }
                } else if sz.z != 0 {
                    add(s, sz.r, &|k| s.ctl[k] == sz.z);
                } else {
                    add(s, sz.r, &|k| s.ctl[k] == 0);
                }
                self.sel_cache = Some((key.0, key.1, out.clone()));
                out
            }
        };
        let dpr = self.dpr;
        for s in segs {
            self.batch.line(self.m2s([s[0], s[1]], ox, kd), self.m2s([s[2], s[3]], ox, kd), 4.0 * dpr, rgba(255, 216, 74, 1.0));
        }
    }
    fn draw_cities(&mut self, ox: f32, kd: f32) {
        let k = self.cam.k;
        if k < 2.5 {
            return;
        }
        let dpr = self.dpr;
        let all = k >= 5.0;
        let (vx0, vx1, vy0, vy1) = (-ox / kd - 5.0, (self.sw - ox) / kd + 5.0, -self.cam.y / kd - 5.0, (self.sh - self.cam.y) / kd + 5.0);
        if self.started {
            self.s.cap_update();
        }
        let mut taken: Vec<[f32; 4]> = Vec::new();
        let n = self.w.cities.len();
        let w = self.w.clone();
        for i in 0..n {
            let c = &w.cities[i];
            let (cp, hidden) = if self.started && i < self.s.caps.len() {
                let st = &self.s.caps[i];
                (st.cap, !st.cap && c.cap && st.ctl == -2)
            } else {
                (c.cap, false)
            };
            if hidden || (!cp && !all) {
                continue;
            }
            if c.x < vx0 || c.x > vx1 || c.y < vy0 || c.y > vy1 {
                continue;
            }
            let x = c.x * kd + ox;
            let y = c.y * kd + self.cam.y;
            let tw = (c.name.chars().count() as f32 * 5.6 + 10.0) * dpr;
            let bx = [x - 6.0 * dpr, y - 7.0 * dpr, x + tw, y + 7.0 * dpr];
            if taken.iter().any(|b| bx[0] < b[2] && bx[2] > b[0] && bx[1] < b[3] && bx[3] > b[1]) {
                continue;
            }
            taken.push(bx);
            if cp {
                self.batch.star([x, y], 6.5 * dpr, rgba(0, 0, 0, 0.6));
                self.batch.star([x, y], 5.5 * dpr, rgba(242, 193, 78, 1.0));
            } else {
                self.batch.circle([x, y], 3.0 * dpr, rgba(0, 0, 0, 0.6), 8);
                self.batch.circle([x, y], 2.2 * dpr, rgba(255, 255, 255, 0.85), 8);
            }
            self.batch.text(&mut self.at, 0, &c.name, x + 6.0 * dpr, y, 10.0 * dpr, rgba(255, 255, 255, 0.78), Some(rgba(0, 0, 0, 0.35)), 0);
        }
    }
    fn draw_labels(&mut self, ox: f32, kd: f32) {
        let k = self.cam.k;
        let fade = if k < 5.0 { 1.0 } else if k > 8.0 { 0.0 } else { 1.0 - (k - 5.0) / 3.0 };
        if fade <= 0.0 {
            return;
        }
        let (vx0, vx1, vy0, vy1) = (-ox / kd, (self.sw - ox) / kd, -self.cam.y / kd, (self.sh - self.cam.y) / kd);
        let list = std::mem::take(&mut self.lbl.list);
        for l in &list {
            let px = l.fs * kd;
            if px < 8.0 * self.dpr {
                continue;
            }
            let (c0, c1) = (l.c[0], l.c[l.c.len() - 1]);
            if c0.1.max(c1.1) + l.fs < vx0 || c0.1.min(c1.1) - l.fs > vx1 || c0.2.max(c1.2) + l.fs < vy0 || c0.2.min(c1.2) - l.fs > vy1 {
                continue;
            }
            let size = (px.min(160.0)).round().max(8.0) as u16;
            let sc = px / size as f32;
            let ow = (px * 0.06).max(1.2);
            for &(ch, x, y, a, _w) in &l.c {
                if let Some(g) = self.at.glyph(1, ch, size) {
                    let (sx, sy) = (x * kd + ox, y * kd + self.cam.y);
                    // centre glyph horizontally at (sx,sy), baseline offset by 0.35*px
                    let hx = g.adv * sc / 2.0;
                    let (cs, sn) = (a.cos(), a.sin());
                    let pen = [sx - hx * cs - 0.35 * px * sn, sy - hx * sn + 0.35 * px * cs];
                    for (dx, dy) in [(-ow, 0.0), (ow, 0.0), (0.0, -ow), (0.0, ow)] {
                        self.batch.glyph(&g, pen[0] + dx, pen[1] + dy, sc, a, [0.0, 0.0, 0.0, 0.5 * fade]);
                    }
                    self.batch.glyph(&g, pen[0], pen[1], sc, a, [1.0, 1.0, 1.0, 0.92 * fade]);
                }
            }
        }
        self.lbl.list = list;
    }
    fn draw_fronts(&mut self, ox: f32, kd: f32) {
        let dpr = self.dpr;
        let fronts = self.s.g.fronts.clone();
        for f in &fronts {
            if let (Some(sg), Some(_)) = (self.s.fseg.get(&f.id).cloned(), f.t) {
                for pass in 0..2 {
                    let (w, c) = if pass == 0 { (7.0, rgba(0, 0, 0, 0.6)) } else { (3.5, rgba(232, 67, 58, 1.0)) };
                    for s in sg.chunks(4) {
                        self.batch.line(self.m2s([s[0], s[1]], ox, kd), self.m2s([s[2], s[3]], ox, kd), w * dpr, c);
                    }
                }
            } else {
                let p: Vec<[f32; 2]> = f.line.iter().map(|q| self.m2s(*q, ox, kd)).collect();
                self.batch.polyline(&p, 7.0 * dpr, rgba(0, 0, 0, 0.6));
                self.batch.polyline(&p, 3.5 * dpr, rgba(232, 67, 58, 1.0));
            }
            if let Some(pl) = &f.plan {
                let p: Vec<[f32; 2]> = pl.iter().map(|q| self.m2s(*q, ox, kd)).collect();
                let c = if f.exec { rgba(255, 216, 74, 1.0) } else { rgba(156, 255, 122, 1.0) };
                self.batch.polyline(&p, 7.0 * dpr, rgba(0, 0, 0, 0.6));
                self.batch.dashed(&p, 3.5 * dpr, c, 10.0 * dpr, 6.0 * dpr);
                if p.len() >= 2 {
                    self.batch.arrow(p[p.len() - 2], p[p.len() - 1], 12.0 * dpr, c);
                }
            }
        }
        let now = self.last_ms;
        self.s.spears.retain(|s| now - s.1 < 4000.0);
        let spears = self.s.spears.clone();
        for (sp, _) in spears {
            let p: Vec<[f32; 2]> = sp.iter().map(|q| self.m2s(*q, ox, kd)).collect();
            self.batch.polyline(&p, 7.0 * dpr, rgba(0, 0, 0, 0.6));
            self.batch.polyline(&p, 3.5 * dpr, rgba(255, 157, 46, 1.0));
            if p.len() >= 2 {
                self.batch.arrow(p[p.len() - 2], p[p.len() - 1], 12.0 * dpr, rgba(255, 157, 46, 1.0));
            }
        }
        if self.drawp.len() > 1 {
            let p: Vec<[f32; 2]> = self.drawp.iter().map(|q| self.m2s(*q, ox, kd)).collect();
            let c = match self.draw.as_deref() {
                Some("front") => rgba(232, 67, 58, 1.0),
                Some("spear") => rgba(255, 157, 46, 1.0),
                _ => rgba(156, 255, 122, 1.0),
            };
            self.batch.polyline(&p, 6.0 * dpr, rgba(0, 0, 0, 0.6));
            self.batch.polyline(&p, 3.0 * dpr, c);
        }
    }
    fn draw_divs(&mut self, ox: f32, kd: f32) {
        let k = self.cam.k;
        if self.s.g.divs.is_empty() || k < 1.8 {
            return;
        }
        let dpr = self.dpr;
        let me = self.s.g.player;
        let nd = &self.w.nations[me as usize];
        let col = nat_color(me as usize, &nd.iso, &nd.e);
        let colf = rgba(col[0], col[1], col[2], 1.0);
        let many = self.s.g.divs.len() > 60;
        let (vx0, vx1, vy0, vy1) = (-ox / kd - 2.0, (self.sw - ox) / kd + 2.0, -self.cam.y / kd - 2.0, (self.sh - self.cam.y) / kd + 2.0);
        let divf = self.s.divf;
        let ipos = |d: &crate::sim::Div| -> [f32; 2] {
            if !d.path.is_empty() && d.ox != 0.0 && (d.x - d.ox).abs() < MW / 2.0 {
                [d.ox + (d.x - d.ox) * divf, d.oy + (d.y - d.oy) * divf]
            } else {
                [d.x, d.y]
            }
        };
        let sel = self.s.sel_div;
        let sels = self.s.sels.clone();
        let divs = &self.s.g.divs;
        let mut paths: Vec<(Vec<[f32; 2]>, bool)> = Vec::new();
        for d in divs {
            if d.path.is_empty() {
                continue;
            }
            let is_s = sel == Some(d.id);
            if many && !is_s && k < 4.0 {
                continue;
            }
            let (mut a, mut b, mut c, mut e) = (d.x, d.y, d.x, d.y);
            for q in &d.path {
                a = a.min(q[0]);
                c = c.max(q[0]);
                b = b.min(q[1]);
                e = e.max(q[1]);
            }
            if c < vx0 || a > vx1 || e < vy0 || b > vy1 {
                continue;
            }
            let ip = ipos(d);
            let mut p = vec![[ip[0] * kd + ox, ip[1] * kd + self.cam.y]];
            let st = ((d.path.len() as f32 / (8f32).max((c - a + e - b) * kd / (4.0 * dpr))).floor() as usize).max(1);
            let mut i = st - 1;
            while i < d.path.len() {
                p.push([d.path[i][0] * kd + ox, d.path[i][1] * kd + self.cam.y]);
                i += st;
            }
            let l = d.path[d.path.len() - 1];
            p.push([l[0] * kd + ox, l[1] * kd + self.cam.y]);
            paths.push((p, is_s));
        }
        for (p, is_s) in &paths {
            self.batch.polyline(p, 4.0 * dpr, rgba(0, 0, 0, 0.55));
            let c = if *is_s { rgba(255, 225, 74, 1.0) } else { rgba(140, 255, 140, 0.95) };
            self.batch.polyline(p, 2.0 * dpr, c);
            if p.len() >= 2 {
                self.batch.arrow(p[p.len() - 2], p[p.len() - 1], 9.0 * dpr, c);
            }
        }
        let cs = if k < 4.0 { 16.0 * dpr } else { 0.0 };
        let mut cells: std::collections::HashMap<(i32, i32), (f32, f32, u32)> = std::collections::HashMap::new();
        let mut sprites: Vec<([f32; 2], bool, f32)> = Vec::new();
        for d in divs {
            let ip = ipos(d);
            let (x, y) = (ip[0] * kd + ox, ip[1] * kd + self.cam.y);
            if x < -20.0 * dpr || x > self.sw + 20.0 * dpr || y < -20.0 * dpr || y > self.sh + 20.0 * dpr {
                continue;
            }
            let is_s = sel == Some(d.id) || sels.contains(&d.id);
            if cs > 0.0 && !is_s {
                let key = ((x / cs) as i32, (y / cs) as i32);
                if let Some(c) = cells.get_mut(&key) {
                    c.2 += 1;
                    continue;
                }
                cells.insert(key, (x, y, 1));
            }
            sprites.push(([x, y], is_s, d.str_));
        }
        for (p, is_s, st) in sprites {
            let (w, h) = (14.0 * dpr, 9.0 * dpr);
            let (x0, y0) = (p[0] - w / 2.0, p[1] - h / 2.0);
            self.batch.rect(x0 - 1.5 * dpr, y0 - 1.5 * dpr, w + 3.0 * dpr, h + 3.0 * dpr, rgba(0, 0, 0, 0.6));
            self.batch.rect(x0, y0, w, h, colf);
            let oc = if is_s { rgba(255, 225, 74, 1.0) } else { rgba(255, 255, 255, 1.0) };
            self.batch.rect_line(x0, y0, w, h, if is_s { 1.6 * dpr } else { 1.0 * dpr }, oc);
            self.batch.line([x0 + 2.0 * dpr, y0 + 1.5 * dpr], [x0 + w - 2.0 * dpr, y0 + h - 1.5 * dpr], dpr, rgba(0, 0, 0, 0.85));
            self.batch.line([x0 + w - 2.0 * dpr, y0 + 1.5 * dpr], [x0 + 2.0 * dpr, y0 + h - 1.5 * dpr], dpr, rgba(0, 0, 0, 0.85));
            if st < 10000.0 {
                self.batch.rect(p[0] - 7.0 * dpr, p[1] + 6.0 * dpr, 14.0 * dpr, 2.0 * dpr, rgba(0, 0, 0, 0.66));
                self.batch.rect(p[0] - 7.0 * dpr, p[1] + 6.0 * dpr, 14.0 * dpr * st / 10000.0, 2.0 * dpr, rgba(125, 255, 125, 1.0));
            }
        }
        for (_, (x, y, n)) in cells {
            if n > 1 {
                let t = n.to_string();
                let w = (t.len() as f32 * 6.0 + 5.0) * dpr;
                self.batch.rect(x + 6.0 * dpr, y - 11.0 * dpr, w, 10.0 * dpr, rgba(0, 0, 0, 0.8));
                self.batch.text(&mut self.at, 0, &t, x + 8.5 * dpr, y - 6.0 * dpr, 9.0 * dpr, rgba(255, 255, 255, 1.0), None, 0);
            }
        }
    }
    fn draw_plan_dbg(&mut self, ox: f32, kd: f32) {
        let me = self.s.g.player;
        let (cw, ch) = (MW / GW as f32 * kd, MH / GH as f32 * kd);
        let fronts = self.s.g.fronts.clone();
        for f in &fronts {
            if !f.exec {
                continue;
            }
            let a = match self.s.parea.get(&f.id) {
                Some(a) => a,
                None => continue,
            };
            let n = a.nd.max(1);
            let mut rects = Vec::new();
            for (i, &k) in a.cells.iter().enumerate() {
                if self.s.ctrl_of(k as usize) == me {
                    continue;
                }
                let l = ((a.t[i] * n as f32).floor() as usize).min(n - 1);
                let x = (k as usize % GW) as f32 * cw + ox;
                let y = (k as usize / GW) as f32 * ch + self.cam.y;
                if x < -cw || x > self.sw || y < -ch || y > self.sh {
                    continue;
                }
                rects.push((x, y, l));
            }
            let left = a.left;
            for (x, y, l) in rects {
                let h = ((l * 67) % 360) as f32;
                let c = hsl_col(h, 0.9, 0.55, 0.55);
                self.batch.rect(x, y, cw + 0.5, ch + 0.5, c);
            }
            for di in self.s.plan_divs(f) {
                let d = &self.s.g.divs[di];
                if let Some(tk) = d.tk {
                    let a = [d.x * kd + ox, d.y * kd + self.cam.y];
                    let b = [((tk as usize % GW) as f32 + 0.5) * cw + ox, ((tk as usize / GW) as f32 + 0.5) * ch + self.cam.y];
                    self.batch.line(a, b, 1.5 * self.dpr, rgba(255, 255, 255, 1.0));
                }
            }
            let s = format!("осталось клеток: {}", left);
            let dpr = self.dpr;
            self.batch.text(&mut self.at, 0, &s, 10.0 * dpr, 90.0 * dpr, 11.0 * dpr, rgba(255, 255, 255, 1.0), Some(rgba(0, 0, 0, 0.6)), 0);
        }
    }
    fn draw_cover_dbg(&mut self, ox: f32, kd: f32) {
        let dpr = self.dpr;
        let fronts = self.s.g.fronts.clone();
        for f in &fronts {
            let c = match self.s.cover.get(&f.id) {
                Some(c) => c.clone(),
                None => continue,
            };
            let mut gaps = 0;
            for s in &c {
                let p = [s.p[0] * kd + ox, s.p[1] * kd + self.cam.y];
                let col = match s.state {
                    0 => rgba(47, 220, 90, 1.0),
                    1 => rgba(255, 216, 74, 1.0),
                    3 => rgba(255, 138, 138, 1.0),
                    _ => rgba(255, 42, 42, 1.0),
                };
                if s.state >= 2 {
                    gaps += 1;
                }
                let r = if s.ech || s.res { 3.5 } else { 5.0 } * dpr;
                self.batch.circle(p, r + dpr, rgba(0, 0, 0, 1.0), 10);
                self.batch.circle(p, r, if s.res { rgba(88, 166, 255, 1.0) } else { col }, 10);
                if self.cam.k > 4.0 {
                    let t = format!("{}", s.pr);
                    self.batch.text(&mut self.at, 0, &t, p[0] + 6.0 * dpr, p[1] - 4.0 * dpr, 10.0 * dpr, rgba(255, 255, 255, 1.0), None, 0);
                }
            }
            let t = format!("фронт #{}: брешей {}", f.id, gaps);
            self.batch.text(&mut self.at, 0, &t, 10.0 * dpr, (104.0 + f.id as f32 * 13.0) * dpr, 11.0 * dpr, rgba(255, 255, 255, 1.0), Some(rgba(0, 0, 0, 0.6)), 0);
        }
    }

    fn render_pc(&mut self) {
        let now = self.last_ms;
        // selection overlay texture
        let ver = self.s.pc.as_ref().map(|p| p.ver).unwrap_or(0);
        let need = self.s.pc.as_ref().map(|p| p.hi_ver != p.ver).unwrap_or(false);
        if need && now - self.pc_hi_t > 80.0 {
            self.pc_hi_t = now;
            if let Some((w, h, d)) = self.s.pc_hi() {
                unsafe { self.r.as_mut().unwrap().upload_pc(w, h, &d) };
            }
            if let Some(pc) = self.s.pc.as_mut() {
                pc.hi_ver = ver;
            }
        }
        let (cam, rect_cells) = {
            let pc = self.s.pc.as_ref().unwrap();
            (pc.cam, [pc.x0 as f32 * MW / GW as f32, pc.y0 as f32 * MH / GH as f32, (pc.x1 - pc.x0 + 1) as f32 * MW / GW as f32, (pc.y1 - pc.y0 + 1) as f32 * MH / GH as f32])
        };
        let r = self.r.as_ref().unwrap();
        let gl = &r.gl;
        let [rx, ry, rw, rh] = self.pc_rect;
        unsafe {
            gl.viewport(0, 0, self.sw as i32, self.sh as i32);
            gl.clear_color(0.043, 0.07, 0.125, 1.0);
            gl.clear_stencil(0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::STENCIL_BUFFER_BIT);
            gl.enable(glow::SCISSOR_TEST);
            gl.scissor(rx as i32, (self.sh - ry - rh) as i32, rw as i32, rh as i32);
            gl.clear_color(0.05, 0.1, 0.17, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            let kd = cam[2];
            let v = self.view(cam[0], cam[1], kd);
            let k = kd / self.s0();
            r.draw_map(&v, k.max(12.0), self.dpr, 1, true, true, 1.0);
            r.draw_pc_overlay(&v, rect_cells);
            gl.disable(glow::SCISSOR_TEST);
        }
    }
}

pub fn fmt_eta(h: f32) -> String {
    let h = h.round() as i64;
    if h < 24 {
        format!("{} ч", h)
    } else {
        format!("{} д {} ч", h / 24, h % 24)
    }
}

fn hsl_col(h: f32, s: f32, l: f32, a: f32) -> Col {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [r + m, g + m, b + m, a]
}

impl World {
    pub fn edge_like(a: [f32; 2], b: [f32; 2]) -> bool {
        let e = 0.6;
        (a[0] < e && b[0] < e) || (a[0] > MW - e && b[0] > MW - e) || (a[1] > MH - e && b[1] > MH - e) || (a[1] < e && b[1] < e)
    }
}
