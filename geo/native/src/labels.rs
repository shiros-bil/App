// HOI4-style country labels: laid out once in map coordinates along the main landmass, curved, de-overlapped.
use crate::batch::Atlas;
use crate::sim::Sim;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct Label {
    pub fs: f32,
    pub c: Vec<(char, f32, f32, f32, f32)>, // ch, x, y, angle, width
}

pub fn layout_nation(s: &Sim, at: &Atlas, n: u16, rs: &[u32]) -> Option<Label> {
    let w = &s.w;
    let m = w.meta();
    let in_n: HashSet<u32> = rs.iter().copied().collect();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut best: Vec<u32> = Vec::new();
    let mut b_a = -1f32;
    for &r0 in rs {
        if seen.contains(&r0) {
            continue;
        }
        let mut comp = Vec::new();
        let mut st = vec![r0];
        seen.insert(r0);
        let mut a = 0f32;
        while let Some(r) = st.pop() {
            comp.push(r);
            a += m.area[r as usize];
            for &q in &w.nb[r as usize] {
                if in_n.contains(&q) && !seen.contains(&q) {
                    seen.insert(q);
                    st.push(q);
                }
            }
        }
        if a > b_a {
            b_a = a;
            best = comp;
        }
    }
    let mut p: Vec<(f32, f32, f32)> = Vec::new();
    for &r in &best {
        let r = r as usize;
        let (x, y) = (m.lp[r][0], m.lp[r][1]);
        let wt = m.area[r];
        let h = m.lw[r] / 2.0;
        let bb = m.bb[r];
        p.push((x, y, wt));
        p.push((bb[0].max(x - h), y, wt * 0.3));
        p.push((bb[2].min(x + h), y, wt * 0.3));
        p.push((x, bb[1].max(y - h / 2.2), wt * 0.3));
        p.push((x, bb[3].min(y + h / 2.2), wt * 0.3));
    }
    let (mut sw, mut mx, mut my) = (0f64, 0f64, 0f64);
    for &(x, y, wt) in &p {
        sw += wt as f64;
        mx += x as f64 * wt as f64;
        my += y as f64 * wt as f64;
    }
    if sw <= 0.0 {
        return None;
    }
    mx /= sw;
    my /= sw;
    let (mut sxx, mut syy, mut sxy) = (0f64, 0f64, 0f64);
    for &(x, y, wt) in &p {
        let (dx, dy) = (x as f64 - mx, y as f64 - my);
        sxx += wt as f64 * dx * dx;
        syy += wt as f64 * dy * dy;
        sxy += wt as f64 * dx * dy;
    }
    let mut ang = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let pi = std::f64::consts::PI;
    if ang > pi / 2.0 {
        ang -= pi;
    }
    if ang < -pi / 2.0 {
        ang += pi;
    }
    ang = ang.clamp(-0.85, 0.85);
    let ev = ((sxx - syy).powi(2) / 4.0 + sxy * sxy).max(0.0).sqrt();
    let (l1, l2) = ((sxx + syy) / 2.0 + ev, (sxx + syy) / 2.0 - ev);
    if l1 < l2 * 1.6 {
        ang *= ((l1 / l2.max(1e-9) - 1.0) / 0.6).max(0.0);
    }
    let (ux, uy) = (ang.cos(), ang.sin());
    let (mut t0, mut t1, mut o0, mut o1) = (1e9f64, -1e9f64, 1e9f64, -1e9f64);
    let mut sm = [0f64; 5];
    let mut yv = [0f64; 3];
    for &(x, y, wt) in &p {
        let (dx, dy) = (x as f64 - mx, y as f64 - my);
        let t = dx * ux + dy * uy;
        let o = -dx * uy + dy * ux;
        t0 = t0.min(t);
        t1 = t1.max(t);
        o0 = o0.min(o);
        o1 = o1.max(o);
        let mut tp = 1.0;
        for k in 0..5 {
            sm[k] += wt as f64 * tp;
            tp *= t;
        }
        yv[0] += wt as f64 * o;
        yv[1] += wt as f64 * o * t;
        yv[2] += wt as f64 * o * t * t;
    }
    let len = t1 - t0;
    let thick = o1 - o0;
    if len <= 0.0 {
        return None;
    }
    let mm = [[sm[0], sm[1], sm[2]], [sm[1], sm[2], sm[3]], [sm[2], sm[3], sm[4]]];
    let det = |m: &[[f64; 3]; 3]| m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let d = det(&mm);
    let (mut ca, mut cb, mut cc) = (0f64, 0f64, 0f64);
    if d.abs() > 1e-12 {
        let rep = |c: usize| {
            let mut r = mm;
            for i in 0..3 {
                r[i][c] = yv[i];
            }
            r
        };
        ca = det(&rep(0)) / d;
        cb = det(&rep(1)) / d;
        cc = det(&rep(2)) / d;
    }
    cc *= 0.7;
    let half = len / 2.0;
    let l2b = thick * 0.35;
    if (cc * half * half).abs() > l2b {
        cc = cc.signum() * l2b / (half * half);
    }
    let tc = (t0 + t1) / 2.0;
    let txt: Vec<char> = w.nations[n as usize].n.chars().collect();
    let nch = txt.len().max(1) as f64;
    let mut fs = (thick * 0.36).min(len * 0.7 / (nch * 0.62));
    let mut ws: Vec<f64> = txt.iter().map(|&c| at.advance(1, c, 100.0) as f64 * fs / 100.0).collect();
    let mut wn: f64 = ws.iter().sum();
    if wn > len * 0.62 {
        let f = len * 0.62 / wn;
        fs *= f;
        for v in ws.iter_mut() {
            *v *= f;
        }
        wn *= f;
    }
    let target = (len * 0.66).min(wn + fs * 0.55 * (nch - 1.0));
    let sp = if txt.len() > 1 { ((target - wn) / (nch - 1.0)).max(0.0) } else { 0.0 };
    let mut sacc = -(wn + sp * (nch - 1.0)) / 2.0;
    let mut c = Vec::new();
    for (i, &ch) in txt.iter().enumerate() {
        let sc = sacc + ws[i] / 2.0;
        sacc += ws[i] + sp;
        let tt = tc + sc;
        let o = ca + cb * tt + cc * tt * tt;
        let dd = cb + 2.0 * cc * tt;
        let a = ((uy.atan2(ux)) + dd.atan()).clamp(-0.95, 0.95);
        c.push((ch, (mx + tt * ux - o * uy) as f32, (my + tt * uy + o * ux) as f32, a as f32, ws[i] as f32));
    }
    Some(Label { fs: fs as f32, c })
}

pub struct Labels {
    pub list: Vec<Label>,
    cache: HashMap<u16, (u64, Option<Label>)>,
}

impl Labels {
    pub fn new() -> Labels {
        Labels { list: vec![], cache: HashMap::new() }
    }
    pub fn compute(&mut self, s: &Sim, at: &Atlas, owners: &[u16]) {
        let mut by: HashMap<u16, Vec<u32>> = HashMap::new();
        let m = s.w.meta();
        for r in 0..owners.len() {
            if !(m.area[r] > 0.0) {
                continue;
            }
            by.entry(owners[r]).or_default().push(r as u32);
        }
        let mut out = Vec::new();
        let mut keys: Vec<&u16> = by.keys().collect();
        keys.sort();
        for &n in keys {
            let rs = &by[&n];
            let mut sig: u64 = rs.len() as u64;
            for &r in rs {
                sig = (sig.wrapping_mul(31).wrapping_add(r as u64)) % 1_000_000_007;
            }
            if let Some((cs, l)) = self.cache.get(&n) {
                if *cs == sig {
                    if let Some(l) = l {
                        out.push(l.clone());
                    }
                    continue;
                }
            }
            let l = layout_nation(s, at, n, rs);
            self.cache.insert(n, (sig, l.clone()));
            if let Some(l) = l {
                out.push(l);
            }
        }
        out.sort_by(|a, b| b.fs.partial_cmp(&a.fs).unwrap());
        let gs = 20f32;
        let mut grid: HashMap<(i32, i32), Vec<[f32; 4]>> = HashMap::new();
        self.list.clear();
        for l in out {
            let bx: Vec<[f32; 4]> = l.c.iter().map(|&(_, x, y, _, w)| [x - w / 2.0, y - l.fs * 0.42, x + w / 2.0, y + l.fs * 0.42]).collect();
            let mut hit = false;
            'o: for c in &bx {
                for i in (c[0] / gs).floor() as i32..=(c[2] / gs).floor() as i32 {
                    for j in (c[1] / gs).floor() as i32..=(c[3] / gs).floor() as i32 {
                        if let Some(g) = grid.get(&(i, j)) {
                            if g.iter().any(|b| c[0] < b[2] && c[2] > b[0] && c[1] < b[3] && c[3] > b[1]) {
                                hit = true;
                                break 'o;
                            }
                        }
                    }
                }
            }
            if hit {
                continue;
            }
            for c in &bx {
                for i in (c[0] / gs).floor() as i32..=(c[2] / gs).floor() as i32 {
                    for j in (c[1] / gs).floor() as i32..=(c[3] / gs).floor() as i32 {
                        grid.entry((i, j)).or_default().push(*c);
                    }
                }
            }
            self.list.push(l);
        }
    }
}
