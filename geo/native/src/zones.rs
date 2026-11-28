// Zone textures for the GL fill shader: zone id per cell (+dilation bit), smooth field, cut lines per zone.
use crate::sim::Sim;
use crate::topo::MW;
use crate::world::{GH, GW};
use std::collections::{HashMap, HashSet};

pub struct ZoneTex {
    pub zid: Vec<u16>,
    pub ff: Vec<u8>,
    pub dirty: Option<[usize; 4]>,
    pub full: bool,
    pub cut: HashMap<u16, (u32, Vec<f32>)>,
    pub cutq: Vec<u32>,
    pub cut_ver: u64,
    pub anx_segs: Vec<Vec<f32>>, // per LOD: minor-border segments near zones (s,l interleaved: x0 y0 x1 y1 l r)
    pub anx_t: f64,
    pub anx_ver: u64,
}

impl Default for ZoneTex {
    fn default() -> Self {
        ZoneTex { zid: vec![0; GW * GH], ff: vec![0; GW * GH], dirty: None, full: true, cut: HashMap::new(), cutq: Vec::new(), cut_ver: 1, anx_segs: vec![], anx_t: -1e9, anx_ver: 0 }
    }
}

impl Sim {
    // field: 1 inside zone set, 0 outside (inside region), outside cells replicate nearest inside value, 3x3 blur
    pub fn zone_field(&self, r: u32, in_z: &dyn Fn(usize) -> bool) -> (i64, i64, usize, usize, Vec<f32>) {
        let [x0, y0, x1, y1] = self.cell_box(r as usize);
        let (bx, by) = (x0 as i64 - 2, y0 as i64 - 2);
        let (w, h) = (x1 - x0 + 5, y1 - y0 + 5);
        let mut ind = vec![-1f32; w * h];
        let r1 = (r + 1) as u16;
        for j in 0..h {
            let cj = by + j as i64;
            if cj < 0 || cj >= GH as i64 {
                continue;
            }
            for i in 0..w {
                let ci = bx + i as i64;
                if ci < 0 || ci >= GW as i64 {
                    continue;
                }
                let k = cj as usize * GW + ci as usize;
                if self.w.reg[k] == r1 {
                    ind[j * w + i] = if in_z(k) { 1.0 } else { 0.0 };
                }
            }
        }
        for _ in 0..3 {
            let src = ind.clone();
            for j in 0..h {
                for i in 0..w {
                    let q = j * w + i;
                    if src[q] >= 0.0 {
                        continue;
                    }
                    let mut v = -1f32;
                    if i > 0 && src[q - 1] > v {
                        v = src[q - 1];
                    }
                    if i < w - 1 && src[q + 1] > v {
                        v = src[q + 1];
                    }
                    if j > 0 && src[q - w] > v {
                        v = src[q - w];
                    }
                    if j < h - 1 && src[q + w] > v {
                        v = src[q + w];
                    }
                    if v >= 0.0 {
                        ind[q] = v;
                    }
                }
            }
        }
        for v in ind.iter_mut() {
            if *v < 0.0 {
                *v = 1.0;
            }
        }
        let mut f = vec![0f32; w * h];
        for j in 0..h {
            for i in 0..w {
                let (mut s, mut ws) = (0f32, 0f32);
                for dj in -1i64..=1 {
                    let jj = j as i64 + dj;
                    if jj < 0 || jj >= h as i64 {
                        continue;
                    }
                    for di in -1i64..=1 {
                        let ii = i as i64 + di;
                        if ii < 0 || ii >= w as i64 {
                            continue;
                        }
                        let wt = (if di != 0 { 1.0 } else { 2.0 }) * (if dj != 0 { 1.0 } else { 2.0 });
                        s += ind[jj as usize * w + ii as usize] * wt;
                        ws += wt;
                    }
                }
                f[j * w + i] = s / ws;
            }
        }
        (bx, by, w, h, f)
    }
    // cut line of a zone (marching squares on the field), as segments in map coords
    pub fn mask_lines(&self, r: u32, in_z: &dyn Fn(usize) -> bool) -> Vec<f32> {
        let (bx, by, w, h, f) = self.zone_field(r, in_z);
        let sx = MW / GW as f32;
        let x = |i: f32| (bx as f32 + i + 0.5) * sx;
        let y = |j: f32| (by as f32 + j + 0.5) * sx;
        let mut out = Vec::new();
        for j in 0..h.saturating_sub(1) {
            for i in 0..w.saturating_sub(1) {
                let cs = [(i, j, f[j * w + i]), (i + 1, j, f[j * w + i + 1]), (i + 1, j + 1, f[(j + 1) * w + i + 1]), (i, j + 1, f[(j + 1) * w + i])];
                let ins: Vec<bool> = cs.iter().map(|c| c.2 >= 0.5).collect();
                if ins.iter().all(|&b| b) || ins.iter().all(|&b| !b) {
                    continue;
                }
                let mut cr: Vec<[f32; 2]> = Vec::new();
                for k in 0..4 {
                    let (p, q) = (cs[k], cs[(k + 1) & 3]);
                    if ins[k] != ins[(k + 1) & 3] {
                        let t = (0.5 - p.2) / (q.2 - p.2);
                        cr.push([x(p.0 as f32 + (q.0 as f32 - p.0 as f32) * t), y(p.1 as f32 + (q.1 as f32 - p.1 as f32) * t)]);
                    }
                }
                let mut k = 0;
                while k + 1 < cr.len() {
                    out.extend_from_slice(&[cr[k][0], cr[k][1], cr[k + 1][0], cr[k + 1][1]]);
                    k += 2;
                }
            }
        }
        out
    }
    fn zone_region(&mut self, zt: &mut ZoneTex, r: u32) -> [usize; 4] {
        let r1 = (r + 1) as u16;
        let [x0, y0, x1, y1] = self.cell_box(r as usize);
        for j in y0..=y1 {
            for i in x0..=x1 {
                let k = j * GW + i;
                if self.w.reg[k] == r1 {
                    zt.zid[k] = self.ctl[k];
                    zt.ff[k] = 0;
                }
            }
        }
        let has_any = self.g.zones.iter().any(|z| z.r == r && z.c > 0);
        if !has_any {
            zt.cutq.push(r);
            return [x0, y0, x1, y1];
        }
        let ctl = &self.ctl;
        let (bx, by, w, h, f) = self.zone_field(r, &|k| ctl[k] > 0);
        let has_z: HashSet<u32> = self.g.zones.iter().filter(|z| z.c > 0).map(|z| z.r).collect();
        for j in 0..h {
            let cj = by + j as i64;
            if cj < 0 || cj >= GH as i64 {
                continue;
            }
            for i in 0..w {
                let ci = bx + i as i64;
                if ci < 0 || ci >= GW as i64 {
                    continue;
                }
                let k = cj as usize * GW + ci as usize;
                let rg = self.w.reg[k];
                if rg == r1 || rg == 0 || !has_z.contains(&(rg as u32 - 1)) {
                    zt.ff[k] = (f[j * w + i] * 255.0).round() as u8;
                }
            }
        }
        for _ in 0..3 {
            let mut add = Vec::new();
            for j in 1..h.saturating_sub(1) {
                let cj = by + j as i64;
                if cj < 1 || cj >= GH as i64 - 1 {
                    continue;
                }
                for i in 1..w.saturating_sub(1) {
                    let ci = bx + i as i64;
                    if ci < 1 || ci >= GW as i64 - 1 {
                        continue;
                    }
                    let k = cj as usize * GW + ci as usize;
                    if zt.zid[k] != 0 && zt.zid[k] & 32768 == 0 {
                        continue;
                    }
                    let rg = self.w.reg[k];
                    if rg != 0 && rg != r1 && has_z.contains(&(rg as u32 - 1)) {
                        continue;
                    }
                    for o in [-1i64, 1, -(GW as i64), GW as i64] {
                        let z = zt.zid[(k as i64 + o) as usize] & 32767;
                        if z != 0 && self.g.zones[z as usize - 1].r == r {
                            add.push((k, z));
                            break;
                        }
                    }
                }
            }
            for (k, z) in add {
                zt.zid[k] = z | 32768;
            }
        }
        zt.cutq.push(r);
        [bx.max(0) as usize, by.max(0) as usize, ((bx + w as i64 - 1).min(GW as i64 - 1)) as usize, ((by + h as i64 - 1).min(GH as i64 - 1)) as usize]
    }
    // returns true if textures changed; budget in ms (0 = all)
    pub fn zone_flush(&mut self, zt: &mut ZoneTex, full: bool, budget_ms: f64, clock: &dyn Fn() -> f64) {
        let regs: Vec<u32> = if full {
            zt.zid.iter_mut().for_each(|v| *v = 0);
            zt.ff.iter_mut().for_each(|v| *v = 0);
            zt.cut.clear();
            self.chr.clear();
            let mut s: Vec<u32> = self.g.zones.iter().filter(|z| z.c > 0).map(|z| z.r).collect();
            s.sort();
            s.dedup();
            zt.full = true;
            s
        } else {
            let mut v: Vec<u32> = self.chr.drain().collect();
            v.sort();
            v
        };
        let t0 = clock();
        let mut b = zt.dirty.unwrap_or([GW, GH, 0, 0]);
        for r in regs {
            if !full && budget_ms > 0.0 && clock() - t0 > budget_ms {
                self.chr.insert(r);
                continue;
            }
            let bb = self.zone_region(zt, r);
            b = [b[0].min(bb[0]), b[1].min(bb[1]), b[2].max(bb[2]), b[3].max(bb[3])];
        }
        if b[2] >= b[0] {
            zt.dirty = Some(b);
        }
        if full {
            zt.cutq.clear();
        }
    }
    pub fn cut_flush(&mut self, zt: &mut ZoneTex, budget_ms: f64, clock: &dyn Fn() -> f64) {
        let t0 = clock();
        let mut l = Vec::new();
        zt.cutq.sort();
        zt.cutq.dedup();
        while let Some(r) = zt.cutq.pop() {
            l.push(r);
            if budget_ms > 0.0 && clock() - t0 > budget_ms {
                break;
            }
        }
        for r in l {
            zt.cut.retain(|_, v| v.0 != r);
            for (i, z) in self.g.zones.iter().enumerate() {
                if z.r == r && z.c > 0 {
                    let id = (i + 1) as u16;
                    let ctl = &self.ctl;
                    let segs = self.mask_lines(r, &|k| ctl[k] == id);
                    zt.cut.insert(id, (r, segs));
                }
            }
        }
        zt.cut_ver += 1;
    }
    // minor (intra-nation) borders near zones, always drawn (otherwise only at high zoom)
    pub fn anx_lines(&self, zt: &mut ZoneTex) {
        zt.anx_segs.clear();
        for lod in &self.w.lods {
            let s = &lod.line2.s;
            let l = &lod.line2.l;
            let mut out = Vec::new();
            for i in 0..s.len() / 4 {
                let cx = ((s[4 * i] + s[4 * i + 2]) / 2.0 * GW as f32 / MW) as i64;
                let cy = ((s[4 * i + 1] + s[4 * i + 3]) / 2.0 * GH as f32 / crate::topo::MH) as i64;
                let mut hit = false;
                'o: for dy in [-3i64, 0, 3] {
                    for dx in [-3i64, 0, 3] {
                        let (x, y) = (cx + dx, cy + dy);
                        if x >= 0 && y >= 0 && x < GW as i64 && y < GH as i64 && zt.zid[y as usize * GW + x as usize] != 0 {
                            hit = true;
                            break 'o;
                        }
                    }
                }
                if hit {
                    out.extend_from_slice(&[s[4 * i], s[4 * i + 1], s[4 * i + 2], s[4 * i + 3], l[2 * i], l[2 * i + 1]]);
                }
            }
            zt.anx_segs.push(out);
        }
        zt.anx_ver += 1;
    }
}
