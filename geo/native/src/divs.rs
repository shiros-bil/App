// Divisions: free movement, grid A* with allocation-free buffers, hourly movement, capture trail.
use crate::sim::{Div, Sim};
use crate::topo::{MH, MW};
use crate::world::{GH, GW, KMU};

pub const CGS: usize = 4;
pub const CGW: usize = GW / CGS;
pub const CGH: usize = GH / CGS;
pub const BASE_KMH: f32 = 4.0;

pub fn km_dist(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let c = ((90.0 - (ay + by) / 2.0 / MH * 180.0) * std::f32::consts::PI / 180.0).cos();
    ((bx - ax) * c).hypot(by - ay) * KMU
}

#[derive(Default)]
pub struct AStar {
    g: Vec<f32>,
    pr: Vec<i32>,
    pi: Vec<i32>,
    st: Vec<u32>,
    cl: Vec<u32>,
    gen: u32,
    hf: Vec<f64>,
    hk: Vec<i32>,
    ce_c: Vec<u8>,
    ce_st: Vec<u32>,
    ce_gen: u32,
}

impl Sim {
    pub fn reg_spd(&self, r: u32) -> f32 {
        let a = self.w.rkm[r as usize].max(1.0) / 1000.0;
        (1.25 - 0.12 * a.log10()).clamp(0.55, 1.25)
    }
    pub fn div_spd(&self, d: &Div) -> f32 {
        let base = if d.kmh > 0.0 { d.kmh } else { BASE_KMH };
        let plan = if d.k > 0.0 { d.k } else { 1.0 };
        base * self.reg_spd(d.r) * plan
    }
    pub fn path_km(d: &Div) -> f32 {
        let (mut x, mut y, mut l) = (d.x, d.y, 0.0);
        for q in &d.path {
            l += km_dist(x, y, q[0], q[1]);
            x = q[0];
            y = q[1];
        }
        l
    }
    pub fn div_eta(&self, d: &Div) -> f32 {
        let tot = if d.p_tot > 0.0 { d.p_tot } else { Sim::path_km(d) };
        (tot - d.p_done).max(0.0) / self.div_spd(d).max(0.01)
    }

    fn ce_ok(&self, a: &mut AStar, r: u32) -> bool {
        let r = r as usize;
        if a.ce_st[r] != a.ce_gen {
            a.ce_st[r] = a.ce_gen;
            a.ce_c[r] = self.can_enter(r as u32) as u8;
        }
        a.ce_c[r] == 1
    }
    fn pass_k(&self, a: &mut AStar, k: usize) -> bool {
        let r = self.w.reg[k];
        if r == 0 {
            return false;
        }
        if self.route_own && self.ctrl_of(k) != self.g.player {
            return false;
        }
        self.ce_ok(a, r as u32 - 1)
    }
    fn los_ok(&self, a: &mut AStar, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
        let (fa, fb, fc, fe) = (x0 * GW as f32 / MW, y0 * GH as f32 / MH, x1 * GW as f32 / MW, y1 * GH as f32 / MH);
        let n = ((fc - fa).hypot(fe - fb) * 2.0).ceil() as i32 + 1;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let x = ((fa + (fc - fa) * t).floor() as i32).rem_euclid(GW as i32) as usize;
            let y = ((fb + (fe - fb) * t).floor() as i32).clamp(0, GH as i32 - 1) as usize;
            if !self.pass_k(a, y * GW + x) {
                return false;
            }
        }
        true
    }
    pub fn los_free(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
        let mut a = std::mem::take(&mut self.astar);
        self.prep_ce(&mut a);
        let ok = self.los_ok(&mut a, x0, y0, x1, y1);
        self.astar = a;
        ok
    }
    fn prep_ce(&self, a: &mut AStar) {
        let nr = self.w.nreg();
        if a.ce_c.len() != nr {
            a.ce_c = vec![0; nr];
            a.ce_st = vec![0; nr];
        }
        a.ce_gen = a.ce_gen.wrapping_add(1);
        if a.ce_gen == 0 {
            a.ce_st.iter_mut().for_each(|v| *v = 0);
            a.ce_gen = 1;
        }
    }
    pub fn grid_route(&mut self, sx: f32, sy: f32, tx: f32, ty: f32) -> Option<Vec<[f32; 2]>> {
        let mut a = std::mem::take(&mut self.astar);
        let r = self.grid_route_in(&mut a, sx, sy, tx, ty);
        self.astar = a;
        r
    }
    fn grid_route_in(&self, a: &mut AStar, sx: f32, sy: f32, tx: f32, ty: f32) -> Option<Vec<[f32; 2]>> {
        self.prep_ce(a);
        if self.los_ok(a, sx, sy, tx, ty) {
            return Some(vec![[tx, ty]]);
        }
        let n = CGW * CGH;
        if a.g.len() != n {
            a.g = vec![0.0; n];
            a.pr = vec![0; n];
            a.pi = vec![0; n];
            a.st = vec![0; n];
            a.cl = vec![0; n];
        }
        a.gen = a.gen.wrapping_add(1);
        if a.gen == 0 {
            a.st.iter_mut().for_each(|v| *v = 0);
            a.cl.iter_mut().for_each(|v| *v = 0);
            a.gen = 1;
        }
        let gen = a.gen;
        let off = [2usize, 2, 1, 1, 3, 3, 1, 3, 3, 1];
        let cgw = CGW as i32;
        let si = (sx * CGW as f32 / MW).floor() as i32;
        let sj = (sy * CGH as f32 / MH).floor() as i32;
        let mut ti = (tx * CGW as f32 / MW).floor() as i32;
        let tj = (ty * CGH as f32 / MH).floor() as i32;
        while ti - si > cgw / 2 {
            ti -= cgw;
        }
        while ti - si < -cgw / 2 {
            ti += cgw;
        }
        let key = |i: i32, j: i32| (j * cgw + i.rem_euclid(cgw)) as usize;
        let mut hn = 0usize;
        a.hf.clear();
        a.hk.clear();
        macro_rules! push {
            ($f:expr, $k:expr) => {{
                if hn == a.hf.len() {
                    a.hf.push(0.0);
                    a.hk.push(0);
                }
                a.hf[hn] = $f;
                a.hk[hn] = $k as i32;
                let mut i = hn;
                hn += 1;
                while i > 0 {
                    let q = (i - 1) >> 1;
                    if a.hf[q] <= a.hf[i] {
                        break;
                    }
                    a.hf.swap(q, i);
                    a.hk.swap(q, i);
                    i = q;
                }
            }};
        }
        if sj < 0 || sj >= CGH as i32 || tj < 0 || tj >= CGH as i32 {
            return None;
        }
        let s0 = key(si, sj);
        let t0 = key(ti, tj);
        a.st[s0] = gen;
        a.g[s0] = 0.0;
        a.pr[s0] = -1;
        a.pi[s0] = si;
        push!(((ti - si) as f64).hypot((tj - sj) as f64), s0);
        let mut it = 0;
        let mut found = false;
        while hn > 0 && it < 400000 {
            it += 1;
            // pop
            let k = a.hk[0] as usize;
            hn -= 1;
            if hn > 0 {
                a.hf[0] = a.hf[hn];
                a.hk[0] = a.hk[hn];
                let mut i = 0;
                loop {
                    let l = 2 * i + 1;
                    let r = l + 1;
                    let mut m = i;
                    if l < hn && a.hf[l] < a.hf[m] {
                        m = l;
                    }
                    if r < hn && a.hf[r] < a.hf[m] {
                        m = r;
                    }
                    if m == i {
                        break;
                    }
                    a.hf.swap(m, i);
                    a.hk.swap(m, i);
                    i = m;
                }
            }
            if a.cl[k] == gen {
                continue;
            }
            a.cl[k] = gen;
            if k == t0 {
                found = true;
                break;
            }
            let ui = a.pi[k];
            let uj = (k / CGW) as i32;
            let g = a.g[k];
            for dj in -1..=1 {
                for di in -1..=1 {
                    if di == 0 && dj == 0 {
                        continue;
                    }
                    let vi = ui + di;
                    let vj = uj + dj;
                    if vj < 0 || vj >= CGH as i32 {
                        continue;
                    }
                    let v = key(vi, vj);
                    if a.cl[v] == gen {
                        continue;
                    }
                    if v != t0 {
                        let x = vi.rem_euclid(cgw) as usize;
                        let mut p = false;
                        let mut q = 0;
                        while q < 10 {
                            if self.pass_k(a, (vj as usize * CGS + off[q + 1]) * GW + x * CGS + off[q]) {
                                p = true;
                                break;
                            }
                            q += 2;
                        }
                        if !p {
                            continue;
                        }
                    }
                    let ng = g + if di != 0 && dj != 0 { 1.4142 } else { 1.0 };
                    let gv = if a.st[v] == gen { a.g[v] } else { 1e30 };
                    if ng < gv {
                        a.st[v] = gen;
                        a.g[v] = ng;
                        a.pr[v] = k as i32;
                        a.pi[v] = vi;
                        push!(ng as f64 + ((ti - vi) as f64).hypot((tj - vj) as f64), v);
                    }
                }
            }
        }
        if !found {
            return None;
        }
        let mut cells = Vec::new();
        let mut k = t0 as i32;
        while k >= 0 && k as usize != s0 {
            let ku = k as usize;
            cells.push([(a.pi[ku] as f32 + 0.5) * MW / CGW as f32, ((ku / CGW) as f32 + 0.5) * MH / CGH as f32]);
            k = a.pr[ku];
        }
        cells.reverse();
        if let Some(l) = cells.last_mut() {
            *l = [tx, ty];
        } else {
            cells.push([tx, ty]);
        }
        let mut out = Vec::new();
        let (mut cx, mut cy) = (sx, sy);
        let mut i = 0;
        while i < cells.len() {
            let mut j = cells.len() - 1;
            while j > i && !self.los_ok(a, cx, cy, cells[j][0], cells[j][1]) {
                j -= 1;
            }
            out.push(cells[j]);
            cx = cells[j][0];
            cy = cells[j][1];
            i = j + 1;
        }
        Some(out)
    }

    fn find_route(&self, a: u32, b: u32) -> Option<Vec<u32>> {
        if a == b {
            return Some(vec![a]);
        }
        let n = self.w.nreg();
        let mut dist = vec![f64::MAX; n];
        let mut prev = vec![-1i64; n];
        dist[a as usize] = 0.0;
        let mut h = std::collections::BinaryHeap::new();
        #[derive(PartialEq)]
        struct E(f64, u32);
        impl Eq for E {}
        impl PartialOrd for E {
            fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(o))
            }
        }
        impl Ord for E {
            fn cmp(&self, o: &Self) -> std::cmp::Ordering {
                o.0.partial_cmp(&self.0).unwrap_or(std::cmp::Ordering::Equal)
            }
        }
        h.push(E(0.0, a));
        while let Some(E(d, u)) = h.pop() {
            if u == b {
                break;
            }
            if d > dist[u as usize] {
                continue;
            }
            for &v in &self.w.nb[u as usize] {
                if !self.can_enter(v) {
                    continue;
                }
                let (pu, pv) = (self.w.lp(u as usize), self.w.lp(v as usize));
                let w = km_dist(pu[0], pu[1], pv[0], pv[1]) as f64 * (1.0 / self.reg_spd(u) as f64 + 1.0 / self.reg_spd(v) as f64) / 2.0;
                let nd = d + w;
                if nd < dist[v as usize] {
                    dist[v as usize] = nd;
                    prev[v as usize] = u as i64;
                    h.push(E(nd, v));
                }
            }
        }
        if prev[b as usize] < 0 {
            return None;
        }
        let mut out = vec![b];
        let mut u = b;
        while u != a {
            u = prev[u as usize] as u32;
            out.push(u);
        }
        out.reverse();
        Some(out)
    }

    // order a division to the exact point; returns false (with toast unless quiet) if impossible
    // divisions marching to the same place don't walk in single file: each keeps its own lane, shifted
    // sideways from the common route (fading in after the start and out before the target), where the
    // shifted point is still land it may enter
    fn spread_path(&self, di: usize, p: &mut Vec<[f32; 2]>) {
        if p.len() < 3 {
            return;
        }
        let mut cum = vec![0f32; p.len()];
        for i in 1..p.len() {
            cum[i] = cum[i - 1] + (p[i][0] - p[i - 1][0]).hypot(p[i][1] - p[i - 1][1]);
        }
        let tot = cum[p.len() - 1];
        if tot < 3.0 {
            return;
        }
        let id = self.g.divs[di].id;
        let h = ((id.wrapping_mul(2654435761) >> 8) & 0xffff) as f32 / 65535.0;
        let lane = (h - 0.5) * 2.0 * 1.1;
        let ramp = 2.5f32.min(tot / 3.0);
        let src = p.clone();
        for i in 1..p.len() - 1 {
            let f = (cum[i].min(tot - cum[i]) / ramp).min(1.0);
            if f <= 0.0 {
                continue;
            }
            let (a, b) = (src[i - 1], src[i + 1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy);
            if l < 1e-6 {
                continue;
            }
            let q = [src[i][0] - dy / l * lane * f, src[i][1] + dx / l * lane * f];
            let r = self.region_at(q[0], q[1]);
            if r >= 0 && self.can_enter(r as u32) {
                p[i] = q;
            }
        }
    }
    pub fn order_div(&mut self, di: usize, tx: f32, ty: f32, tr: u32, quiet: bool) -> bool {
        let had = self.g.divs[di].path.last().copied();
        let ok = self.order_div0(di, tx, ty, tr, quiet);
        if ok {
            let d = &mut self.g.divs[di];
            let same = had.map(|h| (h[0] - tx).hypot(h[1] - ty) < 0.3).unwrap_or(false);
            if same {
                let done = d.p_done;
                d.p_tot = Sim::path_km(d) + done;
            } else {
                d.p_tot = Sim::path_km(d);
                d.p_done = 0.0;
            }
        }
        ok
    }
    fn order_div0(&mut self, di: usize, mut tx: f32, ty: f32, tr: u32, quiet: bool) -> bool {
        {
            let d = &mut self.g.divs[di];
            d.hold = false;
            d.pf = 0;
        }
        let (dx, dy) = (self.g.divs[di].x, self.g.divs[di].y);
        while tx - dx > MW / 2.0 {
            tx -= MW;
        }
        while tx - dx < -MW / 2.0 {
            tx += MW;
        }
        if !self.can_enter(tr) {
            if !quiet {
                let s = format!("Нельзя: нет войны с {}", self.nm(self.g.owner[tr as usize]));
                self.toast(s);
            }
            return false;
        }
        let mut gp = self.grid_route(dx, dy, tx, ty);
        if let Some(g) = &gp {
            if g.is_empty() {
                gp = Some(vec![[tx, ty]]);
            }
        }
        if gp.is_none() && self.route_own {
            return false;
        }
        if let Some(gp) = gp {
            let mut p: Vec<[f32; 2]> = vec![[dx, dy]];
            p.extend(gp);
            if p.len() > 2 {
                let saved = self.route_own;
                self.route_own = false;
                for _ in 0..2 {
                    let mut q = vec![p[0]];
                    for i in 1..p.len() - 1 {
                        let (a, b, c) = (p[i - 1], p[i], p[i + 1]);
                        let p1 = [b[0] * 0.75 + a[0] * 0.25, b[1] * 0.75 + a[1] * 0.25];
                        let p2 = [b[0] * 0.75 + c[0] * 0.25, b[1] * 0.75 + c[1] * 0.25];
                        if self.los_free(p1[0], p1[1], p2[0], p2[1]) {
                            q.push(p1);
                            q.push(p2);
                        } else {
                            q.push(b);
                        }
                    }
                    q.push(*p.last().unwrap());
                    p = q;
                }
                self.route_own = saved;
            }
            self.spread_path(di, &mut p);
            let d = &mut self.g.divs[di];
            d.path = p[1..].iter().map(|q| [q[0], q[1], -1.0]).collect();
            if let Some(l) = d.path.last_mut() {
                l[2] = tr as f32;
            }
            return true;
        }
        // fallback: region graph route through shared border points
        let dr = self.g.divs[di].r;
        let rt = match self.find_route(dr, tr) {
            Some(r) => r,
            None => {
                if !quiet {
                    self.toast("Нет сухопутного пути");
                }
                return false;
            }
        };
        let mut p: Vec<[f32; 3]> = vec![[dx, dy, dr as f32]];
        for i in 1..rt.len() {
            let (a, b) = (rt[i - 1], rt[i]);
            let k = if a < b { a as u64 * 100000 + b as u64 } else { b as u64 * 100000 + a as u64 };
            match self.w.bp.get(&k) {
                Some(q) => p.push([q[0], q[1], b as f32]),
                None => {
                    let l = self.w.lp(b as usize);
                    p.push([l[0], l[1], b as f32])
                }
            }
        }
        p.push([tx, ty, tr as f32]);
        for i in 1..p.len() {
            while p[i][0] - p[i - 1][0] > MW / 2.0 {
                p[i][0] -= MW;
            }
            while p[i][0] - p[i - 1][0] < -MW / 2.0 {
                p[i][0] += MW;
            }
        }
        for _ in 0..3 {
            let mut q = vec![p[0]];
            for i in 0..p.len() - 1 {
                let (a, b) = (p[i], p[i + 1]);
                q.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25, b[2]]);
                q.push([a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75, b[2]]);
            }
            q.push(*p.last().unwrap());
            p = q;
        }
        self.g.divs[di].path = p[1..].to_vec();
        true
    }

    pub fn new_div(&mut self, r: u32, px: Option<f32>, py: Option<f32>, free: bool) -> usize {
        if !free {
            let c = self.div_cost();
            let n = &mut self.g.nat[self.g.player as usize];
            n.army -= 10000.0;
            n.money -= c;
        }
        self.g.div_n += 1;
        let lp = self.w.lp(r as usize);
        let d = Div { id: self.g.div_n, n: format!("{}-я пехотная", self.g.div_n), x: px.unwrap_or(lp[0]), y: py.unwrap_or(lp[1]), r, str_: 10000.0, ..Default::default() };
        self.g.divs.push(d);
        let i = self.g.divs.len() - 1;
        if !free {
            let id = self.g.divs[i].id;
            self.sel_div = Some(id);
            self.sels = vec![id];
            let s = format!("{} дивизия сформирована · нажмите куда идти", self.g.divs[i].n);
            self.toast(s);
        }
        self.dirty_ui = true;
        i
    }
    pub fn zone_pt(&self, r: u32, z: u16) -> [f32; 2] {
        let [x0, y0, x1, y1] = self.cell_box(r as usize);
        let (mut sx, mut sy, mut n) = (0f64, 0f64, 0u64);
        let mut best: Option<(usize, usize)> = None;
        for j in y0..=y1 {
            for i in x0..=x1 {
                let k = j * GW + i;
                if self.w.reg[k] as u32 == r + 1 && (if z != 0 { self.ctl[k] == z } else { self.ctrl_of(k) == self.g.player }) {
                    sx += i as f64;
                    sy += j as f64;
                    n += 1;
                    if best.is_none() {
                        best = Some((i, j));
                    }
                }
            }
        }
        if n == 0 {
            return self.w.lp(r as usize);
        }
        let (mut cx, mut cy) = (sx / n as f64, sy / n as f64);
        let k = cy as usize * GW + cx as usize;
        if !(self.w.reg[k] as u32 == r + 1 && (z == 0 || self.ctl[k] == z)) {
            let b = best.unwrap();
            cx = b.0 as f64;
            cy = b.1 as f64;
        }
        [((cx + 0.5) * MW as f64 / GW as f64) as f32, ((cy + 0.5) * MH as f64 / GH as f64) as f32]
    }
    pub fn new_div_z(&mut self, r: u32, z: u16) {
        let p = self.zone_pt(r, z);
        self.new_div(r, Some(p[0]), Some(p[1]), false);
    }
    pub fn test_divs(&mut self, r: u32, z: u16) {
        let p = self.zone_pt(r, z);
        for _ in 0..10 {
            let (a, b) = (self.rand(), self.rand());
            self.new_div(r, Some(p[0] + (a as f32 - 0.5) * 1.5), Some(p[1] + (b as f32 - 0.5)), true);
        }
        self.toast("+10 дивизий (тест)");
    }

    pub fn move_divs(&mut self, hrs: f32) -> bool {
        let mut mv = false;
        let me = self.g.player;
        for di in 0..self.g.divs.len() {
            let mut left = hrs;
            if self.g.divs[di].wait > 0.0 {
                let w = self.g.divs[di].wait.min(left);
                self.g.divs[di].wait -= w;
                left -= w;
            }
            while left > 0.0 && !self.g.divs[di].path.is_empty() {
                let q = self.g.divs[di].path[0];
                let dr = self.g.divs[di].r;
                if q[2] >= 0.0 && q[2] as u32 != dr && !self.can_enter(q[2] as u32) {
                    self.g.divs[di].path.clear();
                    let s = format!("{}: граница закрыта", self.g.divs[di].n);
                    self.toast(s);
                    break;
                }
                let spd = self.div_spd(&self.g.divs[di]);
                let (sx, sy) = (self.g.divs[di].x, self.g.divs[di].y);
                let kmf = km_dist(sx, sy, q[0], q[1]);
                let km = kmf.min(15.0);
                let t = km / spd;
                mv = true;
                {
                    let d = &mut self.g.divs[di];
                    if t <= left {
                        left -= t;
                        if km >= kmf {
                            d.x = q[0];
                            d.y = q[1];
                            d.path.remove(0);
                        } else {
                            let f = km / kmf;
                            d.x += (q[0] - d.x) * f;
                            d.y += (q[1] - d.y) * f;
                        }
                    } else {
                        let f = left / t * km / kmf.max(1e-6);
                        d.x += (q[0] - d.x) * f;
                        d.y += (q[1] - d.y) * f;
                        left = 0.0;
                    }
                }
                let (nx, ny) = (self.g.divs[di].x, self.g.divs[di].y);
                let mut nr = self.region_at(nx, ny);
                if nr < 0 && q[2] >= 0.0 && self.g.divs[di].path.is_empty() {
                    nr = q[2] as i64;
                }
                if nr >= 0 && nr as u32 != self.g.divs[di].r {
                    let nr = nr as u32;
                    if !self.can_enter(nr) {
                        let last = self.g.divs[di].path.last().copied();
                        if let Some(l) = last {
                            if l[2] >= 0.0 && !self.can_enter(l[2] as u32) {
                                self.g.divs[di].path.clear();
                                let s = format!("{}: граница закрыта", self.g.divs[di].n);
                                self.toast(s);
                            }
                        }
                    } else {
                        self.g.divs[di].r = nr;
                        let o = self.g.owner[nr as usize];
                        if o != me && self.at_war(me, o) && self.w.rc[nr as usize] < 4 {
                            self.conquer(me, nr);
                        }
                    }
                }
                self.g.divs[di].p_done += km_dist(sx, sy, nx, ny);
                if self.g.divs[di].pf != 0 {
                    self.plan_bubble(di);
                } else if !self.g.divs[di].hold {
                    self.occ_trail(me, sx, sy, nx, ny, Some(di));
                }
            }
            let d = &mut self.g.divs[di];
            if d.path.is_empty() && d.k != 0.0 {
                d.k = 0.0;
            }
            if d.x < 0.0 || d.x >= MW {
                let sh = if d.x < 0.0 { MW } else { -MW };
                d.x += sh;
                for q in d.path.iter_mut() {
                    q[0] += sh;
                }
            }
        }
        mv
    }

    pub fn div_index(&self, id: u32) -> Option<usize> {
        self.g.divs.iter().position(|d| d.id == id)
    }
}
