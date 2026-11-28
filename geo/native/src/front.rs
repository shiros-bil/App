// Fronts (by nation border), offensive plans (target area + lanes), front coverage (no gaps), spearheads.
use crate::divs::km_dist;
use crate::sim::{Front, Sim};
use crate::topo::{MH, MW};
use crate::world::{GH, GW};
use std::collections::{HashMap, HashSet};

pub struct Cfg {
    pub plan_full_ticks: i64,
    pub sector_len: f32,
    pub cover_h: i64,
    pub cover_safety_h: i64,
    pub reassign_h: i64,
    pub keep_dist: f32,
    pub reserve_pct: f32,
    pub max_sec_per_div: usize,
    pub arrive_dist: f32,
    pub echelon_back: f32,
    pub reserve_back: f32,
    pub deep_break: bool,
    pub spear_max_km: f32,
    pub plan_gap: f32,
}
pub const CFG: Cfg = Cfg {
    plan_full_ticks: 24,
    sector_len: 1.6,
    cover_h: 6,
    cover_safety_h: 48,
    reassign_h: 48,
    keep_dist: 1.2,
    reserve_pct: 0.15,
    max_sec_per_div: 2,
    arrive_dist: 0.35,
    echelon_back: 0.9,
    reserve_back: 1.6,
    deep_break: false,
    spear_max_km: 160.0,
    plan_gap: 0.2,
};

pub struct Area {
    pub cells: Vec<u32>,
    pub t: Vec<f32>,
    pub set: HashSet<u32>,
    pub ix: HashMap<u32, u32>,
    pub nd: usize,
    pub tot: Vec<i32>,
    pub tot_n: usize,
    pub lv: Option<Vec<u32>>,
    pub lv_h: i64,
    pub left: usize,
    pub fr: Vec<u8>,
    pub in_l: Vec<u32>,
    pub gen: u32,
}

#[derive(Clone)]
pub struct CoverSlot {
    pub p: [f32; 2],
    pub pr: f32,
    pub res: bool,
    pub ech: bool,
    pub d: u32,
    pub state: u8, // 0 ok 1 move 2 gap 3 open
}

const MSQ: [&[usize]; 16] = [&[], &[3, 0], &[0, 1], &[3, 1], &[1, 2], &[3, 0, 1, 2], &[0, 2], &[3, 2], &[2, 3], &[0, 2], &[0, 1, 2, 3], &[1, 2], &[1, 3], &[0, 1], &[3, 0], &[]];

pub fn pl_len(p: &[[f32; 2]], gap: f32) -> Vec<f32> {
    let mut c = vec![0f32];
    for i in 1..p.len() {
        let d = (p[i][0] - p[i - 1][0]).hypot(p[i][1] - p[i - 1][1]);
        c.push(c[i - 1] + if gap > 0.0 && d > gap { 0.0 } else { d });
    }
    c
}
pub fn pl_at(p: &[[f32; 2]], c: &[f32], t: f32) -> [f32; 2] {
    if p.len() < 2 {
        return p[0];
    }
    let l = c[c.len() - 1] * t;
    let mut i = 1;
    while i < c.len() - 1 && c[i] < l {
        i += 1;
    }
    let s = if c[i] - c[i - 1] != 0.0 { c[i] - c[i - 1] } else { 1.0 };
    let f = (l - c[i - 1]) / s;
    [p[i - 1][0] + (p[i][0] - p[i - 1][0]) * f, p[i - 1][1] + (p[i][1] - p[i - 1][1]) * f]
}
pub fn pl_proj(p: &[[f32; 2]], c: &[f32], x: f32, y: f32) -> f32 {
    let (mut bt, mut bd) = (0f32, f32::MAX);
    let tot = if c[c.len() - 1] != 0.0 { c[c.len() - 1] } else { 1.0 };
    for i in 1..p.len() {
        let (ax, ay) = (p[i - 1][0], p[i - 1][1]);
        let (dx, dy) = (p[i][0] - ax, p[i][1] - ay);
        let l2 = if dx * dx + dy * dy != 0.0 { dx * dx + dy * dy } else { 1.0 };
        let u = (((x - ax) * dx + (y - ay) * dy) / l2).clamp(0.0, 1.0);
        let d = (ax + u * dx - x).powi(2) + (ay + u * dy - y).powi(2);
        if d < bd {
            bd = d;
            bt = (c[i - 1] + u * (c[i] - c[i - 1])) / tot;
        }
    }
    bt
}
fn lane_of(t: f32, n: usize) -> usize {
    ((t * n as f32).floor() as i64).clamp(0, n as i64 - 1) as usize
}

impl Sim {
    pub fn plan_divs(&self, f: &Front) -> Vec<usize> {
        let mut m: HashMap<u32, usize> = HashMap::with_capacity(self.g.divs.len());
        for (i, d) in self.g.divs.iter().enumerate() {
            m.insert(d.id, i);
        }
        f.divs.iter().filter_map(|id| m.get(id).copied()).collect()
    }
    pub fn sel_divs(&self) -> Vec<usize> {
        let mut s: Vec<usize> = self.sels.iter().filter_map(|&id| self.div_index(id)).collect();
        if s.is_empty() {
            if let Some(id) = self.sel_div {
                if let Some(i) = self.div_index(id) {
                    s.push(i);
                }
            }
        }
        s
    }
    pub fn snap_mine(&self, x: f32, y: f32, rr: i32) -> Option<[f32; 3]> {
        let me = self.g.player;
        let ci = (((x % MW) + MW) % MW * GW as f32 / MW).floor() as i32;
        let cj = (y * GH as f32 / MH).floor() as i32;
        let mut best: Option<[f32; 3]> = None;
        let mut bd = i32::MAX;
        for r in 0..=rr {
            if best.is_some() {
                break;
            }
            for b in -r..=r {
                for a in -r..=r {
                    if a.abs().max(b.abs()) != r {
                        continue;
                    }
                    let j = cj + b;
                    if j < 0 || j >= GH as i32 {
                        continue;
                    }
                    let i = (ci + a).rem_euclid(GW as i32) as usize;
                    let k = j as usize * GW + i;
                    if self.w.reg[k] != 0 && self.ctrl_of(k) == me {
                        let d = a * a + b * b;
                        if d < bd {
                            bd = d;
                            best = Some([x + a as f32 * MW / GW as f32, (j as f32 + 0.5) * MH / GH as f32, (self.w.reg[k] - 1) as f32]);
                        }
                    }
                }
            }
        }
        best
    }
    fn is_frontier(&self, k: usize) -> bool {
        let me = self.g.player;
        let i = k % GW;
        (i > 0 && self.w.reg[k - 1] != 0 && self.ctrl_of(k - 1) == me)
            || (i < GW - 1 && self.w.reg[k + 1] != 0 && self.ctrl_of(k + 1) == me)
            || (k >= GW && self.w.reg[k - GW] != 0 && self.ctrl_of(k - GW) == me)
            || (k + GW < GW * GH && self.w.reg[k + GW] != 0 && self.ctrl_of(k + GW) == me)
    }

    pub fn send_along(&mut self, divs: &[usize], p: &[[f32; 2]]) -> usize {
        let gap = if p.len() > 40 { 3.0 } else { 0.0 };
        let c = pl_len(p, gap);
        let n = divs.len();
        if n == 0 || p.is_empty() {
            return 0;
        }
        let len = c[c.len() - 1];
        let slots = ((len / 0.7).floor() as usize + 1).min(n).max(1);
        let mut ord: Vec<(usize, f32)> = divs.iter().map(|&d| (d, pl_proj(p, &c, self.g.divs[d].x, self.g.divs[d].y))).collect();
        ord.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let mut ok = 0;
        for (i, &(di, _)) in ord.iter().enumerate() {
            let sl = i * slots / n;
            let ech = i as i64 - ((sl * n) as f32 / slots as f32).ceil() as i64;
            let pt = pl_at(p, &c, (sl as f32 + 0.5) / slots as f32);
            let (dx, dy) = (self.g.divs[di].x, self.g.divs[di].y);
            let (mut x, mut y) = (pt[0], pt[1]);
            while x - dx > MW / 2.0 {
                x -= MW;
            }
            while x - dx < -MW / 2.0 {
                x += MW;
            }
            if ech > 0 {
                let (vx, vy) = (dx - x, dy - y);
                let l = vx.hypot(vy).max(1e-6);
                x += vx / l * 0.8 * ech as f32;
                y += vy / l * 0.8 * ech as f32;
            }
            let sp = match self.snap_mine(x, y, 8) {
                Some(s) => s,
                None => continue,
            };
            self.g.divs[di].tk = None;
            if (sp[0] - dx).hypot(sp[1] - dy) < 0.05 {
                self.g.divs[di].path.clear();
                self.g.divs[di].hold = true;
                ok += 1;
                continue;
            }
            if self.order_div(di, sp[0], sp[1], sp[2] as u32, true) {
                ok += 1;
            }
            self.g.divs[di].hold = true;
            self.g.divs[di].pf = 0;
        }
        ok
    }

    // exact border between me and nation t: ordered points + marching-squares segments
    pub fn front_geom(&self, t: u16) -> Option<(Vec<[f32; 2]>, Vec<f32>)> {
        let me = self.g.player;
        let (mut bx0, mut by0, mut bx1, mut by1) = (GW, GH, 0usize, 0usize);
        let mut ext = |b: [usize; 4]| {
            bx0 = bx0.min(b[0]);
            by0 = by0.min(b[1]);
            bx1 = bx1.max(b[2]);
            by1 = by1.max(b[3]);
        };
        for &r in self.terr(t) {
            ext(self.cell_box(r as usize));
        }
        for z in &self.g.zones {
            if z.o == t && z.c > 0 {
                ext(self.cell_box(z.r as usize));
            }
        }
        if bx1 < bx0 {
            return None;
        }
        let (gx0, gy0, gx1, gy1) = (bx0.saturating_sub(3).max(1), by0.saturating_sub(3).max(1), (bx1 + 3).min(GW - 2), (by1 + 3).min(GH - 2));
        let (mut x0, mut y0, mut x1, mut y1) = (GW, GH, 0usize, 0usize);
        let mut bins: HashMap<usize, [f32; 2]> = HashMap::new();
        let mut order: Vec<usize> = Vec::new();
        for j in gy0..=gy1 {
            for i in gx0..=gx1 {
                let k = j * GW + i;
                if self.w.reg[k] == 0 || self.ctrl_of(k) != me {
                    continue;
                }
                let hit = (self.w.reg[k - 1] != 0 && self.ctrl_of(k - 1) == t)
                    || (self.w.reg[k + 1] != 0 && self.ctrl_of(k + 1) == t)
                    || (self.w.reg[k - GW] != 0 && self.ctrl_of(k - GW) == t)
                    || (self.w.reg[k + GW] != 0 && self.ctrl_of(k + GW) == t);
                if hit {
                    x0 = x0.min(i);
                    x1 = x1.max(i);
                    y0 = y0.min(j);
                    y1 = y1.max(j);
                    let b = (j / 2) * 4096 + i / 2;
                    if !bins.contains_key(&b) {
                        bins.insert(b, [(i as f32 + 0.5) * MW / GW as f32, (j as f32 + 0.5) * MH / GH as f32]);
                        order.push(b);
                    }
                }
            }
        }
        if order.is_empty() {
            return None;
        }
        let pt: Vec<[f32; 2]> = order.iter().map(|b| bins[b]).collect();
        let (mut cx, mut cy) = (0f32, 0f32);
        for q in &pt {
            cx += q[0];
            cy += q[1];
        }
        cx /= pt.len() as f32;
        cy /= pt.len() as f32;
        let mut si = 0;
        let mut sd = -1f32;
        for (i, q) in pt.iter().enumerate() {
            let d = (q[0] - cx).powi(2) + (q[1] - cy).powi(2);
            if d > sd {
                sd = d;
                si = i;
            }
        }
        let mut used = vec![false; pt.len()];
        let mut p = vec![pt[si]];
        used[si] = true;
        for _ in 1..pt.len() {
            let a = *p.last().unwrap();
            let (mut bi, mut bd) = (0, f32::MAX);
            for i in 0..pt.len() {
                if !used[i] {
                    let d = (pt[i][0] - a[0]).powi(2) + (pt[i][1] - a[1]).powi(2);
                    if d < bd {
                        bd = d;
                        bi = i;
                    }
                }
            }
            used[bi] = true;
            p.push(pt[bi]);
        }
        // contour
        let (x0, y0, x1, y1) = (x0.saturating_sub(3).max(1), y0.saturating_sub(3).max(1), (x1 + 3).min(GW - 2), (y1 + 3).min(GH - 2));
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut m = vec![0f32; w * h];
        let mut tn = vec![0u8; w * h];
        for j in 0..h {
            for i in 0..w {
                let k = (y0 + j) * GW + x0 + i;
                if self.w.reg[k] != 0 {
                    let c = self.ctrl_of(k);
                    if c == me {
                        m[j * w + i] = 1.0;
                    } else if c == t {
                        tn[j * w + i] = 1;
                    }
                }
            }
        }
        let mut f = vec![0f32; w * h];
        for j in 0..h {
            for i in 0..w {
                let (mut sm, mut n) = (0f32, 0f32);
                for b in -1i32..=1 {
                    for a in -1i32..=1 {
                        let (ii, jj) = (i as i32 + a, j as i32 + b);
                        if ii < 0 || jj < 0 || ii >= w as i32 || jj >= h as i32 {
                            continue;
                        }
                        sm += m[jj as usize * w + ii as usize];
                        n += 1.0;
                    }
                }
                f[j * w + i] = sm / n;
            }
        }
        let near_t = |i: usize, j: usize| {
            for b in -2i32..=2 {
                for a in -2i32..=2 {
                    let (ii, jj) = (i as i32 + a, j as i32 + b);
                    if ii >= 0 && jj >= 0 && ii < w as i32 && jj < h as i32 && tn[jj as usize * w + ii as usize] != 0 {
                        return true;
                    }
                }
            }
            false
        };
        let mut seg = Vec::new();
        let (sx, sy) = (MW / GW as f32, MH / GH as f32);
        for j in 0..h - 1 {
            for i in 0..w - 1 {
                let (fa, fb, fc, fd) = (f[j * w + i], f[j * w + i + 1], f[(j + 1) * w + i + 1], f[(j + 1) * w + i]);
                let id = (fa > 0.5) as usize | ((fb > 0.5) as usize) << 1 | ((fc > 0.5) as usize) << 2 | ((fd > 0.5) as usize) << 3;
                if id == 0 || id == 15 || !near_t(i, j) {
                    continue;
                }
                let e = |q: usize| {
                    let (ax, ay, bx, by, va, vb) = match q {
                        0 => (i, j, i + 1, j, fa, fb),
                        1 => (i + 1, j, i + 1, j + 1, fb, fc),
                        2 => (i, j + 1, i + 1, j + 1, fd, fc),
                        _ => (i, j, i, j + 1, fa, fd),
                    };
                    let dv = vb - va;
                    let u = (0.5 - va) / if dv != 0.0 { dv } else { 1e-6 };
                    [(x0 as f32 + ax as f32 + (bx as f32 - ax as f32) * u + 0.5) * sx, (y0 as f32 + ay as f32 + (by as f32 - ay as f32) * u + 0.5) * sy]
                };
                let l = MSQ[id];
                let mut q = 0;
                while q < l.len() {
                    let (a, b) = (e(l[q]), e(l[q + 1]));
                    seg.extend_from_slice(&[a[0], a[1], b[0], b[1]]);
                    q += 2;
                }
            }
        }
        if p.len() == 1 {
            p.push(p[0]);
        }
        Some((p, seg))
    }
    pub fn front_upd(&mut self, fi: usize) -> Option<Vec<[f32; 2]>> {
        let t = self.g.fronts[fi].t?;
        let g = self.front_geom(t);
        self.g.fronts[fi].v = self.ctl_ver;
        let id = self.g.fronts[fi].id;
        match g {
            None => {
                self.fseg.remove(&id);
                None
            }
            Some((pts, seg)) => {
                self.fseg.insert(id, seg);
                if !self.g.fronts[fi].exec {
                    self.g.fronts[fi].line = pts.clone();
                }
                Some(pts)
            }
        }
    }

    pub fn make_front(&mut self, p: Vec<[f32; 2]>, tn: Option<u16>) {
        let ds = self.sel_divs();
        if ds.is_empty() {
            self.toast("Сначала выберите дивизии");
            return;
        }
        let ids: Vec<u32> = ds.iter().map(|&i| self.g.divs[i].id).collect();
        for f in self.g.fronts.iter_mut() {
            f.divs.retain(|id| !ids.contains(id));
        }
        self.g.fronts.retain(|f| !f.divs.is_empty());
        self.g.fr_n += 1;
        let f = Front { id: self.g.fr_n, divs: ids.clone(), line: p.clone(), plan: None, exec: false, t: tn, ..Default::default() };
        self.g.fronts.push(f);
        let fi = self.g.fronts.len() - 1;
        if tn.is_some() {
            for &i in &ds {
                self.g.divs[i].cv = None;
            }
            self.cover_front(fi, true);
            self.g.fronts[fi].cv_t = self.g.hour;
            self.g.fronts[fi].cv_v = self.ctl_ver;
        } else {
            self.send_along(&ds, &p);
        }
        if self.speed == 0 {
            self.speed = 1;
        }
        let s = match tn {
            Some(t) => format!("Фронт против {}: {} див.", self.nm(t), ids.len()),
            None => format!("Фронт: {} див.", ids.len()),
        };
        self.toast(s);
        self.dirty_ui = true;
    }
    pub fn front_on_nation(&mut self, mx: f32, my: f32) {
        let me = self.g.player;
        let k0 = self.cell_at(mx, my);
        if self.w.reg[k0] == 0 {
            self.toast("Нажмите на страну");
            return;
        }
        let t = self.ctrl_of(k0);
        if t == me {
            self.toast("Это ваша территория");
            return;
        }
        let g = match self.front_geom(t) {
            Some(g) => g,
            None => {
                let s = format!("Нет общей границы с {}", self.nm(t));
                self.toast(s);
                return;
            }
        };
        if self.sels.is_empty() && self.sel_div.is_none() {
            self.sels = self.g.divs.iter().map(|d| d.id).collect();
        }
        self.make_front(g.0, Some(t));
        if let Some(f) = self.g.fronts.last() {
            if f.t == Some(t) {
                let id = f.id;
                self.fseg.insert(id, g.1);
                let fi = self.g.fronts.len() - 1;
                self.g.fronts[fi].v = self.ctl_ver;
            }
        }
    }
    fn cur_front(&self) -> Option<usize> {
        for di in self.sel_divs() {
            let id = self.g.divs[di].id;
            if let Some(fi) = self.g.fronts.iter().position(|f| f.divs.contains(&id)) {
                return Some(fi);
            }
        }
        if self.g.fronts.is_empty() {
            None
        } else {
            Some(self.g.fronts.len() - 1)
        }
    }
    pub fn make_plan(&mut self, p: Vec<[f32; 2]>) {
        match self.cur_front() {
            None => self.toast("Сначала нарисуйте линию фронта"),
            Some(fi) => {
                self.g.fronts[fi].plan = Some(p);
                self.g.fronts[fi].exec = false;
                self.toast("План наступления готов · нажмите «Наступать»");
            }
        }
        self.dirty_ui = true;
    }
    fn order_path(&mut self, di: usize, p: &[[f32; 2]]) -> bool {
        let (mut x, mut y) = (self.g.divs[di].x, self.g.divs[di].y);
        let mut out: Vec<[f32; 3]> = Vec::new();
        let step = (p.len() / 12).max(1);
        let mut i = step - 1;
        while i < p.len() {
            let q = p[i.min(p.len() - 1)];
            let mut tx = q[0];
            while tx - x > MW / 2.0 {
                tx -= MW;
            }
            while tx - x < -MW / 2.0 {
                tx += MW;
            }
            let r = self.region_at(tx, q[1]);
            if r < 0 || !self.can_enter(r as u32) {
                break;
            }
            match self.grid_route(x, y, tx, q[1]) {
                None => break,
                Some(g) => {
                    for pp in g {
                        out.push([pp[0], pp[1], -1.0]);
                    }
                }
            }
            x = tx;
            y = q[1];
            i += step;
        }
        if out.is_empty() {
            return false;
        }
        let r = self.region_at(x, y);
        if let Some(l) = out.last_mut() {
            l[2] = r as f32;
        }
        let d = &mut self.g.divs[di];
        d.path = out;
        d.hold = false;
        d.pf = 0;
        d.p_tot = Sim::path_km(d);
        d.p_done = 0.0;
        true
    }
    pub fn make_spear(&mut self, mut p: Vec<[f32; 2]>) {
        let ds = self.sel_divs();
        if ds.is_empty() {
            self.toast("Сначала выберите дивизии");
            return;
        }
        if !CFG.deep_break {
            let mut l = 0.0;
            for i in 1..p.len() {
                l += km_dist(p[i - 1][0], p[i - 1][1], p[i][0], p[i][1]);
                if l > CFG.spear_max_km {
                    p.truncate(i + 1);
                    self.toast(format!("Прорыв ограничен {} км (глубокий прорыв выключен)", CFG.spear_max_km));
                    break;
                }
            }
        }
        let mut ok = 0;
        for &di in &ds {
            let jit: Vec<[f32; 2]> = p.iter().map(|q| [q[0] + (self.rand() as f32 - 0.5) * 0.6, q[1] + (self.rand() as f32 - 0.5) * 0.4]).collect();
            if self.order_path(di, &jit) {
                ok += 1;
            }
        }
        self.spears.push((p, self.now_ms));
        if ok > 0 {
            self.toast(format!("Прорыв: {} див.", ok));
            if self.speed == 0 {
                self.speed = 1;
            }
        } else {
            self.toast("Нельзя пройти по этому маршруту");
        }
    }

    // ---------------- plan execution ----------------
    pub fn build_area(&mut self, fi: usize) {
        let me = self.g.player;
        let f = &self.g.fronts[fi];
        let p = f.plan.clone().unwrap_or_default();
        if p.len() < 2 {
            return;
        }
        let ds = self.plan_divs(f);
        let (sx, sy) = (GW as f32 / MW, GH as f32 / MH);
        let side = |x: f32, y: f32| -> i32 {
            let (mut bd, mut bs, mut end) = (f32::MAX, 0i32, false);
            for i in 1..p.len() {
                let (ax, ay) = (p[i - 1][0], p[i - 1][1]);
                let (dx, dy) = (p[i][0] - ax, p[i][1] - ay);
                let l2 = if dx * dx + dy * dy != 0.0 { dx * dx + dy * dy } else { 1e-9 };
                let mut u = ((x - ax) * dx + (y - ay) * dy) / l2;
                let e = (i == 1 && u < 0.0) || (i == p.len() - 1 && u > 1.0);
                u = u.clamp(0.0, 1.0);
                let d = (ax + u * dx - x).powi(2) + (ay + u * dy - y).powi(2);
                if d < bd {
                    bd = d;
                    end = e;
                    let cr = dx * (y - ay) - dy * (x - ax);
                    bs = if cr > 0.0 { 1 } else if cr < 0.0 { -1 } else { 0 };
                }
            }
            if end {
                0
            } else {
                bs
            }
        };
        let mut s: i32 = 0;
        for &di in &ds {
            s += side(self.g.divs[di].x, self.g.divs[di].y);
        }
        if s == 0 {
            if let Some(l0) = &f.line0 {
                for q in l0 {
                    s += side(q[0], q[1]);
                }
            }
        }
        let s = if s > 0 { 1 } else if s < 0 { -1 } else { 1 };
        let mut bar: HashSet<usize> = HashSet::new();
        for i in 1..p.len() {
            let (ax, ay, bx, by) = (p[i - 1][0] * sx, p[i - 1][1] * sy, p[i][0] * sx, p[i][1] * sy);
            let n = ((bx - ax).hypot(by - ay) * 2.0).ceil() as i32 + 1;
            for q in 0..=n {
                let x = (ax + (bx - ax) * q as f32 / n as f32).floor() as i32;
                let y = (ay + (by - ay) * q as f32 / n as f32).floor() as i32;
                if y < 1 || y >= GH as i32 - 1 {
                    continue;
                }
                let k = y as usize * GW + x.rem_euclid(GW as i32) as usize;
                bar.insert(k);
                bar.insert(k + 1);
                bar.insert(k + GW);
            }
        }
        let en = |k: usize| -> bool {
            if self.w.reg[k] == 0 {
                return false;
            }
            let c = self.ctrl_of(k);
            c != me && (self.g.owner[self.w.reg[k] as usize - 1] == me || self.at_war(me, c))
        };
        for (a, b) in [(p[1], p[0]), (p[p.len() - 2], p[p.len() - 1])] {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy).max(1e-6);
            for q in 1..=80 {
                let x = b[0] + dx / l * q as f32 * 0.25;
                let y = b[1] + dy / l * q as f32 * 0.25;
                let j = (y * sy).floor() as i32;
                if j < 1 || j >= GH as i32 - 1 {
                    break;
                }
                let k = j as usize * GW + ((x * sx).floor() as i32).rem_euclid(GW as i32) as usize;
                if !en(k) {
                    break;
                }
                bar.insert(k);
                bar.insert(k + 1);
                bar.insert(k + GW);
            }
        }
        let ok = |k: usize| !bar.contains(&k) && en(k);
        let mut sd: HashMap<usize, i32> = HashMap::new();
        let mut side_k = |k: usize| -> i32 {
            *sd.entry(k).or_insert_with(|| side(((k % GW) as f32 + 0.5) / sx, ((k / GW) as f32 + 0.5) / sy))
        };
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut pts: Vec<[f32; 2]> = p.clone();
        if let Some(l0) = &f.line0 {
            pts.extend_from_slice(l0);
        }
        for &di in &ds {
            pts.push([self.g.divs[di].x, self.g.divs[di].y]);
        }
        for q in &pts {
            x0 = x0.min(q[0]);
            x1 = x1.max(q[0]);
            y0 = y0.min(q[1]);
            y1 = y1.max(q[1]);
        }
        let gx0 = (x0 * sx).floor() as i32 - 40;
        let gx1 = (x1 * sx).ceil() as i32 + 40;
        let gy0 = ((y0 * sy).floor() as i32 - 40).max(1);
        let gy1 = ((y1 * sy).ceil() as i32 + 40).min(GH as i32 - 2);
        let mut seeds = Vec::new();
        for j in gy0..=gy1 {
            for ii in gx0..=gx1 {
                let k = j as usize * GW + ii.rem_euclid(GW as i32) as usize;
                if !ok(k) {
                    continue;
                }
                let nb = (self.w.reg[k - 1] != 0 && self.ctrl_of(k - 1) == me)
                    || (self.w.reg[k + 1] != 0 && self.ctrl_of(k + 1) == me)
                    || (self.w.reg[k - GW] != 0 && self.ctrl_of(k - GW) == me)
                    || (self.w.reg[k + GW] != 0 && self.ctrl_of(k + GW) == me);
                if nb {
                    seeds.push(k);
                }
            }
        }
        let flood = |strict: bool, side_k: &mut dyn FnMut(usize) -> i32| -> (Vec<u32>, HashSet<u32>) {
            let mut a: HashSet<u32> = HashSet::new();
            let mut order = Vec::new();
            let mut st = Vec::new();
            for &k in &seeds {
                let sv = side_k(k);
                if if strict { sv != s } else { sv == -s } {
                    continue;
                }
                if a.insert(k as u32) {
                    order.push(k as u32);
                    st.push(k);
                }
            }
            while let Some(k) = st.pop() {
                if a.len() >= 400000 {
                    break;
                }
                for o in [-1i64, 1, -(GW as i64), GW as i64] {
                    let n = (k as i64 + o) as usize;
                    if n < GW || n >= GW * (GH - 1) || a.contains(&(n as u32)) || !ok(n) {
                        continue;
                    }
                    if strict && side_k(n) != s {
                        continue;
                    }
                    a.insert(n as u32);
                    order.push(n as u32);
                    st.push(n);
                }
            }
            (order, a)
        };
        let (mut all, mut set) = flood(false, &mut side_k);
        let far = all.iter().filter(|&&k| side_k(k as usize) == -s).count();
        if far as f32 > all.len() as f32 * 0.05 || all.len() >= 400000 {
            let r = flood(true, &mut side_k);
            all = r.0;
            set = r.1;
        }
        all.sort();
        let c = pl_len(&p, 0.0);
        let mut cache: HashMap<usize, f32> = HashMap::new();
        let t: Vec<f32> = all
            .iter()
            .map(|&k| {
                let k = k as usize;
                let bk = ((k / GW) / 3) * 2048 + (k % GW) / 3;
                *cache.entry(bk).or_insert_with(|| pl_proj(&p, &c, ((k % GW) as f32 + 0.5) / sx, ((k / GW) as f32 + 0.5) / sy))
            })
            .collect();
        let ix: HashMap<u32, u32> = all.iter().enumerate().map(|(i, &k)| (k, i as u32)).collect();
        let id = self.g.fronts[fi].id;
        self.parea.insert(id, Area { cells: all, t, set, ix, nd: 0, tot: vec![], tot_n: 0, lv: None, lv_h: 0, left: 0, fr: vec![], in_l: vec![], gen: 0 });
    }

    pub fn plan_bubble(&mut self, di: usize) {
        let pf = self.g.divs[di].pf;
        let me = self.g.player;
        let (cx, cy, r) = {
            let d = &mut self.g.divs[di];
            let cx = ((d.x % MW) + MW) % MW * GW as f32 / MW;
            let cy = d.y * GH as f32 / MH;
            d.dist += 0.7;
            let t = d.dist / 5.0;
            let sd = d.id as f32 * 7.13;
            (cx, cy, 1.6 + 0.6 * (t * 0.83 + sd).sin() + 0.4 * (t * 2.1 + sd * 1.7).sin())
        };
        if !self.parea.contains_key(&pf) {
            return;
        }
        let mut any = false;
        for _pass in 0..2 {
            for j in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
                for ii in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                    let (dx, dy) = (ii as f32 + 0.5 - cx, j as f32 + 0.5 - cy);
                    if dx * dx + dy * dy > r * r || j < 1 || j >= GH as i32 - 1 {
                        continue;
                    }
                    let i = j as usize * GW + ii.rem_euclid(GW as i32) as usize;
                    if !self.parea[&pf].set.contains(&(i as u32)) || self.ctrl_of(i) == me {
                        continue;
                    }
                    let nb = (self.w.reg[i - 1] != 0 && self.ctrl_of(i - 1) == me)
                        || (self.w.reg[i + 1] != 0 && self.ctrl_of(i + 1) == me)
                        || (self.w.reg[i - GW] != 0 && self.ctrl_of(i - GW) == me)
                        || (self.w.reg[i + GW] != 0 && self.ctrl_of(i + GW) == me);
                    if !nb {
                        continue;
                    }
                    if self.cap_cell(me, i) {
                        any = true;
                    }
                }
            }
        }
        if any {
            self.dbl.push([cx - r, cy - r, cx + r, cy + r]);
        }
    }

    fn clean_holes(&mut self, id: u32) {
        let me = self.g.player;
        let (mut x0, mut y0, mut x1, mut y1) = (GW, GH, 0usize, 0usize);
        if let Some(a) = self.parea.get(&id) {
            for &k in &a.cells {
                let (x, y) = (k as usize % GW, k as usize / GW);
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        } else {
            return;
        }
        if x1 < x0 {
            return;
        }
        let (x0, y0, x1, y1) = (x0.saturating_sub(6).max(1), y0.saturating_sub(6).max(1), (x1 + 6).min(GW - 2), (y1 + 6).min(GH - 2));
        let w = x1 - x0 + 1;
        let mut seen = vec![0u8; w * (y1 - y0 + 1)];
        let l = |k: usize| (k / GW - y0) * w + k % GW - x0;
        let mut out = Vec::new();
        for j in y0..=y1 {
            for i in x0..=x1 {
                let k0 = j * GW + i;
                if seen[l(k0)] != 0 || self.w.reg[k0] == 0 {
                    continue;
                }
                if self.ctrl_of(k0) == me {
                    seen[l(k0)] = 1;
                    continue;
                }
                let mut comp = vec![k0];
                let mut st = vec![k0];
                seen[l(k0)] = 1;
                let mut open = false;
                while let Some(k) = st.pop() {
                    for o in [-1i64, 1, -(GW as i64), GW as i64] {
                        let n = (k as i64 + o) as usize;
                        let (x, y) = (n % GW, n / GW);
                        if x < x0 || x > x1 || y < y0 || y > y1 {
                            open = true;
                            continue;
                        }
                        if self.w.reg[n] == 0 || self.ctrl_of(n) == me || seen[l(n)] != 0 {
                            continue;
                        }
                        seen[l(n)] = 1;
                        st.push(n);
                        comp.push(n);
                    }
                }
                if !open && comp.len() <= 120 {
                    for k in comp {
                        let c = self.ctrl_of(k);
                        if self.g.owner[self.w.reg[k] as usize - 1] == me || self.at_war(me, c) {
                            out.push(k as u32);
                        }
                    }
                }
            }
        }
        if !out.is_empty() {
            self.queue_pocket(me, out);
        }
    }

    pub fn plan_tick(&mut self, fi: usize) {
        let me = self.g.player;
        let ds = self.plan_divs(&self.g.fronts[fi]);
        let id = self.g.fronts[fi].id;
        if ds.is_empty() {
            self.g.fronts[fi].exec = false;
            self.parea.remove(&id);
            return;
        }
        if !self.parea.contains_key(&id) {
            self.build_area(fi);
        }
        let mut a = match self.parea.remove(&id) {
            Some(a) => a,
            None => {
                self.g.fronts[fi].exec = false;
                return;
            }
        };
        let n = ds.len();
        let plan = self.g.fronts[fi].plan.clone().unwrap_or_default();
        if a.nd != n {
            let pc = pl_len(&plan, 0.0);
            let mut o: Vec<(usize, f32)> = ds.iter().map(|&d| (d, pl_proj(&plan, &pc, self.g.divs[d].x, self.g.divs[d].y))).collect();
            o.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
            for (i, (d, _)) in o.iter().enumerate() {
                self.g.divs[*d].ln = Some(i);
            }
            a.nd = n;
        }
        if a.tot_n != n {
            a.tot = vec![0; n];
            for i in 0..a.cells.len() {
                a.tot[lane_of(a.t[i], n)] += 1;
            }
            a.tot_n = n;
            a.lv = None;
        }
        let mut rem = vec![0i32; n];
        let mut left: Vec<u32> = Vec::new();
        let use_src = a.lv.is_some() && self.g.hour - a.lv_h < CFG.plan_full_ticks;
        if use_src {
            for &i in a.lv.as_ref().unwrap() {
                if self.ctrl_of(a.cells[i as usize] as usize) != me {
                    left.push(i);
                    rem[lane_of(a.t[i as usize], n)] += 1;
                }
            }
        } else {
            for i in 0..a.cells.len() {
                if self.ctrl_of(a.cells[i] as usize) != me {
                    left.push(i as u32);
                    rem[lane_of(a.t[i], n)] += 1;
                }
            }
            a.lv_h = self.g.hour;
        }
        a.left = left.len();
        a.lv = Some(left.clone());
        if left.is_empty() {
            self.parea.insert(id, a);
            for &d in &ds {
                if self.g.divs[d].tk.is_some() {
                    self.g.divs[d].path.clear();
                    self.g.divs[d].tk = None;
                }
            }
            self.finish_plan(fi, &ds);
            return;
        }
        let tot = a.tot.clone();
        let prog = |l: usize, rem: &Vec<i32>| if tot[l] != 0 { 1.0 - rem[l] as f32 / tot[l] as f32 } else { 1.0 };
        // frontier flags / lane lists
        if a.fr.len() != a.cells.len() {
            a.fr = vec![0; a.cells.len()];
        }
        if a.in_l.len() != a.cells.len() {
            a.in_l = vec![0; a.cells.len()];
        }
        let mut built = false;
        let mut ll: Vec<Vec<u32>> = Vec::new();
        let mut fa: Vec<u32> = Vec::new();
        for &di in &ds {
            let own = self.g.divs[di].ln.unwrap_or(0).min(n - 1);
            let tk = self.g.divs[di].tk;
            if !self.g.divs[di].path.is_empty() {
                if let Some(t) = tk {
                    if self.ctrl_of(t as usize) != me {
                        continue;
                    }
                }
            }
            if self.g.divs[di].path.is_empty() {
                if let Some(t) = tk {
                    if self.ctrl_of(t as usize) != me {
                        self.g.divs[di].pf = id;
                        self.caplog = Some(Vec::new());
                        self.parea.insert(id, a);
                        self.plan_bubble(di);
                        a = self.parea.remove(&id).unwrap();
                        let lg = self.caplog.take().unwrap_or_default();
                        if built {
                            for c in lg {
                                for o in [0i64, -1, 1, -(GW as i64), GW as i64] {
                                    let k = (c as i64 + o) as u32;
                                    if let Some(&j) = a.ix.get(&k) {
                                        if a.in_l[j as usize] != a.gen {
                                            continue;
                                        }
                                        let f = self.is_frontier(k as usize) as u8;
                                        if f == 1 && a.fr[j as usize] == 0 {
                                            ll[lane_of(a.t[j as usize], n)].push(j);
                                            fa.push(j);
                                        }
                                        a.fr[j as usize] = f;
                                    }
                                }
                            }
                        }
                        self.g.divs[di].tk = None;
                    }
                }
            }
            let mut lane = own;
            if rem[own] == 0 {
                let mut bd = usize::MAX;
                for l in 0..n {
                    if rem[l] != 0 && (l as i64 - own as i64).unsigned_abs() as usize <= bd {
                        if ((l as i64 - own as i64).unsigned_abs() as usize) < bd {
                            bd = (l as i64 - own as i64).unsigned_abs() as usize;
                            lane = l;
                        }
                    }
                }
            } else {
                let (mut lo, mut lp) = (None, 2.0f32);
                for l in [own as i64 - 1, own as i64 + 1] {
                    if l >= 0 && (l as usize) < n && rem[l as usize] != 0 {
                        let pv = prog(l as usize, &rem);
                        if pv < lp {
                            lp = pv;
                            lo = Some(l as usize);
                        }
                    }
                }
                if let Some(lo) = lo {
                    if prog(own, &rem) > lp + CFG.plan_gap {
                        lane = lo;
                    }
                }
            }
            if !built {
                built = true;
                ll = vec![Vec::new(); n];
                fa.clear();
                a.gen = a.gen.wrapping_add(1);
                for &i in &left {
                    let f = self.is_frontier(a.cells[i as usize] as usize) as u8;
                    a.fr[i as usize] = f;
                    a.in_l[i as usize] = a.gen;
                    if f == 1 {
                        ll[lane_of(a.t[i as usize], n)].push(i);
                        fa.push(i);
                    }
                }
            }
            let (dx, dy) = (self.g.divs[di].x * GW as f32 / MW, self.g.divs[di].y * GH as f32 / MH);
            let (mut bk, mut bd, mut bi) = (-1i64, f32::MAX, u32::MAX);
            for pass in 0..2 {
                let lst = if pass == 0 { &ll[lane] } else { &fa };
                for &ii in lst.iter() {
                    if a.fr[ii as usize] == 0 {
                        continue;
                    }
                    let k = a.cells[ii as usize] as usize;
                    let (x, y) = ((k % GW) as f32, (k / GW) as f32);
                    let mut ex = (x - dx).abs();
                    if ex > GW as f32 / 2.0 {
                        ex = GW as f32 - ex;
                    }
                    let dd = ex * ex + (y - dy) * (y - dy);
                    if dd < bd || (dd == bd && ii < bi) {
                        bd = dd;
                        bk = k as i64;
                        bi = ii;
                    }
                }
                if bk >= 0 {
                    break;
                }
            }
            if bk < 0 {
                self.g.divs[di].tk = None;
                continue;
            }
            let bk = bk as usize;
            let tx = ((bk % GW) as f32 + 0.5) * MW / GW as f32;
            let ty = ((bk / GW) as f32 + 0.5) * MH / GH as f32;
            self.g.divs[di].tk = Some(bk as u32);
            let rr = self.w.reg[bk] as u32 - 1;
            self.route_own = true;
            let mut okr = self.order_div(di, tx, ty, rr, true);
            self.route_own = false;
            if !okr {
                okr = self.order_div(di, tx, ty, rr, true);
            }
            if !okr {
                self.g.divs[di].tk = None;
            } else {
                self.g.divs[di].pf = id;
                self.g.divs[di].hold = true;
            }
        }
        self.parea.insert(id, a);
    }
    fn finish_plan(&mut self, fi: usize, ds: &[usize]) {
        let id = self.g.fronts[fi].id;
        self.clean_holes(id);
        let pl = self.g.fronts[fi].plan.take().unwrap_or_default();
        self.g.fronts[fi].line0 = None;
        self.g.fronts[fi].exec = false;
        self.parea.remove(&id);
        for &d in ds {
            let dv = &mut self.g.divs[d];
            dv.tk = None;
            dv.ln = None;
            dv.pf = 0;
        }
        if self.g.fronts[fi].t.is_some() {
            match self.front_upd(fi) {
                Some(pts) => {
                    self.g.fronts[fi].line = pts;
                    for &d in ds {
                        self.g.divs[d].cv = None;
                    }
                    self.cover_front(fi, true);
                }
                None => {
                    self.g.fronts[fi].line = pl.clone();
                    self.send_along(ds, &pl);
                }
            }
        } else {
            self.g.fronts[fi].line = pl.clone();
            self.send_along(ds, &pl);
        }
        self.toast("Наступление завершено: область захвачена");
        self.dirty_ui = true;
    }
    pub fn exec_plans(&mut self) {
        let mut n = 0;
        for fi in 0..self.g.fronts.len() {
            if self.g.fronts[fi].plan.is_none() || self.g.fronts[fi].exec {
                continue;
            }
            let ds = self.plan_divs(&self.g.fronts[fi]);
            if ds.is_empty() {
                continue;
            }
            let line = self.g.fronts[fi].line.clone();
            self.g.fronts[fi].line0 = Some(line);
            self.g.fronts[fi].exec = true;
            let id = self.g.fronts[fi].id;
            self.parea.remove(&id);
            self.build_area(fi);
            if self.parea.get(&id).map(|a| a.cells.is_empty()).unwrap_or(true) {
                self.toast("В области плана нет вражеской территории");
                self.g.fronts[fi].exec = false;
                self.parea.remove(&id);
                continue;
            }
            for &d in &ds {
                self.g.divs[d].path.clear();
                self.g.divs[d].tk = None;
            }
            self.plan_tick(fi);
            n += 1;
        }
        self.toast(if n > 0 { format!("Наступление: {} {}", n, if n == 1 { "фронт" } else { "фронта" }) } else { "Нет готовых планов".into() });
        if n > 0 && self.speed == 0 {
            self.speed = 1;
        }
        self.dirty_ui = true;
    }

    // ---------------- coverage ----------------
    fn enemy_dir(&self, x: f32, y: f32, t: u16) -> [f32; 2] {
        let ci = (((x % MW) + MW) % MW * GW as f32 / MW).floor() as i32;
        let cj = (y * GH as f32 / MH).floor() as i32;
        let (mut sx, mut sy, mut n) = (0f32, 0f32, 0);
        for b in -4..=4 {
            for a in -4..=4 {
                let j = cj + b;
                if j < 0 || j >= GH as i32 {
                    continue;
                }
                let k = j as usize * GW + (ci + a).rem_euclid(GW as i32) as usize;
                if self.w.reg[k] != 0 && self.ctrl_of(k) == t {
                    sx += a as f32;
                    sy += b as f32;
                    n += 1;
                }
            }
        }
        if n == 0 {
            return [0.0, 0.0];
        }
        let l = sx.hypot(sy).max(1e-6);
        [sx / l, sy / l]
    }
    fn behind(&self, x: f32, y: f32, t: u16, dist: f32) -> Option<[f32; 3]> {
        let e = self.enemy_dir(x, y, t);
        self.snap_mine(x - e[0] * dist, y - e[1] * dist, 8).or_else(|| self.snap_mine(x, y, 8))
    }
    fn cover_order(&mut self, di: usize, p: [f32; 3]) {
        let d = &self.g.divs[di];
        if let Some(cv) = d.cv {
            if (cv[0] - p[0]).hypot(cv[1] - p[1]) < 0.05 && (!d.path.is_empty() || (d.x - p[0]).hypot(d.y - p[1]) < CFG.arrive_dist) {
                return;
            }
        }
        let h = self.g.hour;
        let d = &mut self.g.divs[di];
        d.cv = Some([p[0], p[1]]);
        d.cv_t = h;
        if (d.x - p[0]).hypot(d.y - p[1]) < 0.05 {
            d.path.clear();
            d.hold = true;
            d.pf = 0;
            return;
        }
        self.route_own = true;
        let mut ok = self.order_div(di, p[0], p[1], p[2] as u32, true);
        self.route_own = false;
        if !ok {
            ok = self.order_div(di, p[0], p[1], p[2] as u32, true);
        }
        let _ = ok;
        self.g.divs[di].hold = true;
        self.g.divs[di].pf = 0;
    }
    fn front_sectors(&mut self, p: &[[f32; 2]], t: u16) -> Vec<(Vec<[f32; 2]>, f32)> {
        let me = self.g.player;
        let mut s: Vec<Vec<[f32; 2]>> = Vec::new();
        let mut cur = vec![p[0]];
        let mut l = 0.0;
        for i in 1..p.len() {
            let d = (p[i][0] - p[i - 1][0]).hypot(p[i][1] - p[i - 1][1]);
            if d > 3.0 || l + d > CFG.sector_len {
                s.push(std::mem::take(&mut cur));
                l = 0.0;
            } else {
                l += d;
            }
            cur.push(p[i]);
        }
        if !cur.is_empty() {
            s.push(cur);
        }
        self.cap_update();
        let caps: Vec<(f32, f32, bool)> = self.w.cities.iter().zip(self.caps.iter()).filter(|(_, st)| st.ctl == me as i32).map(|(c, st)| (c.x, c.y, st.cap)).collect();
        let army = self.g.nat.get(t as usize).map(|n| n.army).unwrap_or(0.0);
        s.into_iter()
            .map(|pts| {
                let c = pts[pts.len() / 2];
                let r = self.region_at(c[0], c[1]);
                let mut rear = 0.0;
                for &(x, y, cap) in &caps {
                    let dd = (x - c[0]).hypot(y - c[1]);
                    if dd < 12.0 {
                        rear += if cap { 3.0 } else { 0.6 } * (-dd / 6.0).exp();
                    }
                }
                let enemy = (1.0 + army / 1e4).log10().min(3.0) as f32;
                let terr = if r >= 0 { self.reg_spd(r as u32) } else { 1.0 };
                let pr = ((1.0 + enemy + rear + terr) * 100.0).round() / 100.0;
                let seg = if pts.len() > 1 { pts } else { vec![c, c] };
                (seg, pr)
            })
            .collect()
    }
    pub fn cover_front(&mut self, fi: usize, force: bool) {
        let f = &self.g.fronts[fi];
        let t = match f.t {
            Some(t) => t,
            None => return,
        };
        if f.exec {
            return;
        }
        let ds = self.plan_divs(f);
        if ds.is_empty() {
            return;
        }
        let now = self.g.hour;
        let id = f.id;
        let (pts, seg) = match self.front_geom(t) {
            Some(g) => g,
            None => {
                self.cover.remove(&id);
                self.g.fronts[fi].v = self.ctl_ver;
                return;
            }
        };
        self.g.fronts[fi].v = self.ctl_ver;
        self.fseg.insert(id, seg);
        self.g.fronts[fi].line = pts.clone();
        let secs = self.front_sectors(&pts, t);
        let n = ds.len();
        let n_res = ((n as f32 * CFG.reserve_pct).floor() as usize).min(n.saturating_sub(secs.len()));
        let act = n - n_res;
        struct Slot {
            sec: usize,
            pr: f32,
            seg: Vec<[f32; 2]>,
            p: Option<[f32; 3]>,
            ech: bool,
            res: bool,
        }
        let mut slots: Vec<Slot> = secs.into_iter().map(|(seg, pr)| Slot { sec: 1, pr, seg, p: None, ech: false, res: false }).collect();
        while slots.len() > act {
            let (mut bi, mut bv) = (usize::MAX, f32::MAX);
            for i in 0..slots.len() - 1 {
                if slots[i].sec + slots[i + 1].sec > CFG.max_sec_per_div {
                    continue;
                }
                let a = *slots[i].seg.last().unwrap();
                let b = slots[i + 1].seg[0];
                if (a[0] - b[0]).hypot(a[1] - b[1]) > 3.0 {
                    continue;
                }
                let v = slots[i].pr + slots[i + 1].pr;
                if v < bv {
                    bv = v;
                    bi = i;
                }
            }
            if bi == usize::MAX {
                break;
            }
            let b = slots.remove(bi + 1);
            let a = &mut slots[bi];
            a.sec += b.sec;
            a.pr = a.pr.max(b.pr);
            a.seg.extend(b.seg);
        }
        for s in slots.iter_mut() {
            let c = pl_len(&s.seg, 0.0);
            let m = pl_at(&s.seg, &c, 0.5);
            s.p = self.snap_mine(m[0], m[1], 8).or_else(|| self.snap_mine(s.seg[0][0], s.seg[0][1], 8));
        }
        slots.retain(|s| s.p.is_some());
        slots.sort_by(|a, b| b.pr.partial_cmp(&a.pr).unwrap());
        let open: Vec<Slot> = if slots.len() > act { slots.split_off(act) } else { Vec::new() };
        let mut extra: Vec<Slot> = Vec::new();
        let base = slots.len();
        if base > 0 {
            for i in 0..act.saturating_sub(base) {
                let s = &slots[i % base];
                let p = s.p.unwrap();
                let pr = s.pr * 0.5;
                let bp = self.behind(p[0], p[1], t, CFG.echelon_back * (1 + i / base) as f32);
                extra.push(Slot { sec: 0, pr, seg: vec![], p: bp, ech: true, res: false });
            }
            let top = slots[0].p.unwrap();
            for i in 0..n_res {
                let bp = self.behind(top[0], top[1], t, CFG.reserve_back + 0.3 * i as f32);
                extra.push(Slot { sec: 0, pr: 0.0, seg: vec![], p: bp, ech: false, res: true });
            }
        }
        slots.extend(extra);
        slots.retain(|s| s.p.is_some());
        let mut free: Vec<usize> = ds.clone();
        let mut taken: Vec<Option<usize>> = vec![None; slots.len()];
        for (si, s) in slots.iter().enumerate() {
            let p = s.p.unwrap();
            let (mut bd, mut bdv) = (CFG.keep_dist, None);
            for (fi2, &d) in free.iter().enumerate() {
                if let Some(cv) = self.g.divs[d].cv {
                    let dd = (cv[0] - p[0]).hypot(cv[1] - p[1]);
                    if dd < bd {
                        bd = dd;
                        bdv = Some(fi2);
                    }
                }
            }
            if let Some(i) = bdv {
                taken[si] = Some(free.remove(i));
            }
        }
        for (si, s) in slots.iter().enumerate() {
            if taken[si].is_some() {
                continue;
            }
            if free.is_empty() {
                break;
            }
            let p = s.p.unwrap();
            let (mut bd, mut bi) = (f32::MAX, 0);
            for (i, &d) in free.iter().enumerate() {
                let dd = (self.g.divs[d].x - p[0]).hypot(self.g.divs[d].y - p[1]);
                if dd < bd {
                    bd = dd;
                    bi = i;
                }
            }
            taken[si] = Some(free.remove(bi));
        }
        for (si, s) in slots.iter().enumerate() {
            let d = match taken[si] {
                Some(d) => d,
                None => continue,
            };
            let p = s.p.unwrap();
            let moved = self.g.divs[d].cv.map(|cv| (cv[0] - p[0]).hypot(cv[1] - p[1])).unwrap_or(0.0);
            if moved > 0.05 && !force && now - self.g.divs[d].cv_t < CFG.reassign_h && !s.ech && !s.res && moved < CFG.keep_dist {
                continue;
            }
            self.cover_order(d, p);
            self.g.divs[d].role = if s.res { 2 } else if s.ech { 1 } else { 0 };
        }
        let mut st: Vec<CoverSlot> = slots
            .iter()
            .enumerate()
            .map(|(si, s)| {
                let p = s.p.unwrap();
                let (did, state) = match taken[si] {
                    None => (0, 2),
                    Some(d) => {
                        let dv = &self.g.divs[d];
                        (dv.id, if (dv.x - p[0]).hypot(dv.y - p[1]) < CFG.arrive_dist { 0 } else { 1 })
                    }
                };
                CoverSlot { p: [p[0], p[1]], pr: s.pr, res: s.res, ech: s.ech, d: did, state }
            })
            .collect();
        for s in &open {
            let p = s.p.unwrap();
            st.push(CoverSlot { p: [p[0], p[1]], pr: s.pr, res: false, ech: false, d: 0, state: 3 });
        }
        self.cover.insert(id, st);
    }
    pub fn cover_tick(&mut self) {
        let now = self.g.hour;
        for fi in 0..self.g.fronts.len() {
            if self.g.fronts[fi].t.is_none() || self.g.fronts[fi].exec {
                continue;
            }
            let ds = self.plan_divs(&self.g.fronts[fi]);
            let mut sig: i64 = ds.len() as i64;
            for &d in &ds {
                sig = sig.wrapping_mul(31).wrapping_add(self.g.divs[d].id as i64);
            }
            let f = &self.g.fronts[fi];
            if f.cv_v != self.ctl_ver || f.cv_s != sig || now - f.cv_t > CFG.cover_safety_h {
                if now - f.cv_t < CFG.cover_h && f.cv_s == sig {
                    continue;
                }
                let cv = self.ctl_ver;
                let f = &mut self.g.fronts[fi];
                f.cv_t = now;
                f.cv_v = cv;
                f.cv_s = sig;
                self.cover_front(fi, false);
            } else {
                let id = f.id;
                let pos: HashMap<u32, (f32, f32)> = ds.iter().map(|&d| (self.g.divs[d].id, (self.g.divs[d].x, self.g.divs[d].y))).collect();
                if let Some(c) = self.cover.get_mut(&id) {
                    for s in c.iter_mut() {
                        if s.d == 0 {
                            continue;
                        }
                        if let Some(&(x, y)) = pos.get(&s.d) {
                            s.state = if (x - s.p[0]).hypot(y - s.p[1]) < CFG.arrive_dist { 0 } else { 1 };
                        }
                    }
                }
            }
        }
    }
}
