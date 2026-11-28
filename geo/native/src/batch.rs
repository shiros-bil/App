// Screen-space overlay batch (triangles with atlas UVs) + glyph atlas with fontdue.
use std::collections::HashMap;

pub const AW: usize = 2048;
pub const AH: usize = 2048;

#[derive(Clone, Copy)]
pub struct Glyph {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub w: f32,
    pub h: f32,
    pub xo: f32,
    pub yo: f32,
    pub adv: f32,
}

pub struct Atlas {
    pub px: Vec<u8>,
    pub fonts: Vec<fontdue::Font>,
    pub map: HashMap<(u8, char, u16), Glyph>,
    pub sx: usize,
    pub sy: usize,
    pub sh: usize,
    pub dirty: Option<[usize; 4]>,
    pub full: bool,
    pub gen: u64,
}

impl Atlas {
    pub fn new(fonts: Vec<Vec<u8>>) -> Atlas {
        let mut px = vec![0u8; AW * AH * 4];
        for j in 0..4 {
            for i in 0..4 {
                let p = (j * AW + i) * 4;
                px[p..p + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let fonts = fonts.into_iter().filter_map(|b| fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).ok()).collect();
        Atlas { px, fonts, map: HashMap::new(), sx: 6, sy: 0, sh: 6, dirty: None, full: true, gen: 0 }
    }
    pub fn white(&self) -> (f32, f32) {
        (1.5 / AW as f32, 1.5 / AH as f32)
    }
    fn clear(&mut self) {
        self.map.clear();
        for v in self.px.iter_mut() {
            *v = 0;
        }
        for j in 0..4 {
            for i in 0..4 {
                let p = (j * AW + i) * 4;
                self.px[p..p + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        self.sx = 6;
        self.sy = 0;
        self.sh = 6;
        self.full = true;
        self.gen += 1;
    }
    pub fn glyph(&mut self, font: u8, c: char, size: u16) -> Option<Glyph> {
        if let Some(g) = self.map.get(&(font, c, size)) {
            return Some(*g);
        }
        let f = self.fonts.get(font as usize).or(self.fonts.first())?;
        let (m, bmp) = f.rasterize(c, size as f32);
        let (w, h) = (m.width, m.height);
        if self.sx + w + 2 > AW {
            self.sx = 0;
            self.sy += self.sh + 2;
            self.sh = 0;
        }
        if self.sy + h + 2 > AH {
            self.clear();
            return self.glyph(font, c, size);
        }
        let (x0, y0) = (self.sx + 1, self.sy + 1);
        for j in 0..h {
            for i in 0..w {
                let p = ((y0 + j) * AW + x0 + i) * 4;
                self.px[p] = 255;
                self.px[p + 1] = 255;
                self.px[p + 2] = 255;
                self.px[p + 3] = bmp[j * w + i];
            }
        }
        let d = self.dirty.get_or_insert([x0, y0, x0 + w, y0 + h]);
        d[0] = d[0].min(x0);
        d[1] = d[1].min(y0);
        d[2] = d[2].max(x0 + w);
        d[3] = d[3].max(y0 + h);
        self.sx += w + 2;
        self.sh = self.sh.max(h);
        let g = Glyph {
            u0: x0 as f32 / AW as f32,
            v0: y0 as f32 / AH as f32,
            u1: (x0 + w) as f32 / AW as f32,
            v1: (y0 + h) as f32 / AH as f32,
            w: w as f32,
            h: h as f32,
            xo: m.xmin as f32,
            yo: -(m.ymin as f32) - h as f32,
            adv: m.advance_width,
        };
        self.map.insert((font, c, size), g);
        Some(g)
    }
    pub fn advance(&self, font: u8, c: char, size: f32) -> f32 {
        match self.fonts.get(font as usize).or(self.fonts.first()) {
            Some(f) => f.metrics(c, size).advance_width,
            None => size * 0.6,
        }
    }
}

pub type Col = [f32; 4];
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Col {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

pub struct Batch {
    pub v: Vec<f32>,
    pub wu: f32,
    pub wv: f32,
}

impl Batch {
    pub fn new() -> Batch {
        Batch { v: Vec::with_capacity(1 << 16), wu: 1.5 / AW as f32, wv: 1.5 / AH as f32 }
    }
    pub fn clear(&mut self) {
        self.v.clear();
    }
    #[inline]
    fn vert(&mut self, x: f32, y: f32, u: f32, v: f32, c: Col) {
        self.v.extend_from_slice(&[x, y, u, v, c[0], c[1], c[2], c[3]]);
    }
    pub fn tri(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], col: Col) {
        let (u, v) = (self.wu, self.wv);
        self.vert(a[0], a[1], u, v, col);
        self.vert(b[0], b[1], u, v, col);
        self.vert(c[0], c[1], u, v, col);
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Col) {
        self.tri([x, y], [x + w, y], [x, y + h], c);
        self.tri([x + w, y], [x + w, y + h], [x, y + h], c);
    }
    pub fn rect_line(&mut self, x: f32, y: f32, w: f32, h: f32, lw: f32, c: Col) {
        self.rect(x, y, w, lw, c);
        self.rect(x, y + h - lw, w, lw, c);
        self.rect(x, y, lw, h, c);
        self.rect(x + w - lw, y, lw, h, c);
    }
    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], w: f32, c: Col) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy);
        if l < 1e-4 {
            return;
        }
        let (nx, ny) = (-dy / l * w * 0.5, dx / l * w * 0.5);
        let (ex, ey) = (dx / l * w * 0.25, dy / l * w * 0.25);
        let p0 = [a[0] + nx - ex, a[1] + ny - ey];
        let p1 = [b[0] + nx + ex, b[1] + ny + ey];
        let p2 = [b[0] - nx + ex, b[1] - ny + ey];
        let p3 = [a[0] - nx - ex, a[1] - ny - ey];
        self.tri(p0, p1, p2, c);
        self.tri(p0, p2, p3, c);
    }
    pub fn polyline(&mut self, p: &[[f32; 2]], w: f32, c: Col) {
        for i in 1..p.len() {
            self.line(p[i - 1], p[i], w, c);
        }
        if w > 3.0 {
            for q in p.iter().skip(1).take(p.len().saturating_sub(2)) {
                self.circle(*q, w * 0.5, c, 6);
            }
        }
    }
    pub fn dashed(&mut self, p: &[[f32; 2]], w: f32, c: Col, on: f32, off: f32) {
        let mut acc = 0f32;
        let per = on + off;
        for i in 1..p.len() {
            let (a, b) = (p[i - 1], p[i]);
            let l = (b[0] - a[0]).hypot(b[1] - a[1]);
            if l < 1e-4 {
                continue;
            }
            let mut s = 0f32;
            while s < l {
                let ph = (acc + s) % per;
                let seg = if ph < on { (on - ph).min(l - s) } else { (per - ph).min(l - s) };
                if ph < on {
                    let t0 = s / l;
                    let t1 = (s + seg) / l;
                    self.line([a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0], [a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1], w, c);
                }
                s += seg.max(0.01);
            }
            acc += l;
        }
    }
    pub fn circle(&mut self, c0: [f32; 2], r: f32, c: Col, n: usize) {
        let n = n.max(6);
        for i in 0..n {
            let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
            self.tri(c0, [c0[0] + r * a0.cos(), c0[1] + r * a0.sin()], [c0[0] + r * a1.cos(), c0[1] + r * a1.sin()], c);
        }
    }
    pub fn ring(&mut self, c0: [f32; 2], r: f32, w: f32, c: Col) {
        let n = 16;
        for i in 0..n {
            let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
            self.line([c0[0] + r * a0.cos(), c0[1] + r * a0.sin()], [c0[0] + r * a1.cos(), c0[1] + r * a1.sin()], w, c);
        }
    }
    pub fn arrow(&mut self, a: [f32; 2], b: [f32; 2], size: f32, c: Col) {
        let ang = (b[1] - a[1]).atan2(b[0] - a[0]);
        let tip = [b[0] + size * 0.5 * ang.cos(), b[1] + size * 0.5 * ang.sin()];
        let l = [b[0] - size * (ang - 0.5).cos(), b[1] - size * (ang - 0.5).sin()];
        let r = [b[0] - size * (ang + 0.5).cos(), b[1] - size * (ang + 0.5).sin()];
        self.tri(tip, l, r, c);
    }
    pub fn star(&mut self, c0: [f32; 2], r: f32, c: Col) {
        let mut pts = Vec::new();
        for i in 0..10 {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rr = if i % 2 == 0 { r } else { r * 0.45 };
            pts.push([c0[0] + rr * a.cos(), c0[1] + rr * a.sin()]);
        }
        for i in 0..10 {
            self.tri(c0, pts[i], pts[(i + 1) % 10], c);
        }
    }
    pub fn glyph(&mut self, g: &Glyph, x: f32, y: f32, scale: f32, rot: f32, c: Col) {
        // x,y = pen position at baseline; rotation around (x,y)
        let (x0, y0) = (g.xo * scale, g.yo * scale);
        let (w, h) = (g.w * scale, g.h * scale);
        let (cs, sn) = (rot.cos(), rot.sin());
        let tr = |px: f32, py: f32| [x + px * cs - py * sn, y + px * sn + py * cs];
        let p0 = tr(x0, y0);
        let p1 = tr(x0 + w, y0);
        let p2 = tr(x0 + w, y0 + h);
        let p3 = tr(x0, y0 + h);
        self.vert(p0[0], p0[1], g.u0, g.v0, c);
        self.vert(p1[0], p1[1], g.u1, g.v0, c);
        self.vert(p2[0], p2[1], g.u1, g.v1, c);
        self.vert(p0[0], p0[1], g.u0, g.v0, c);
        self.vert(p2[0], p2[1], g.u1, g.v1, c);
        self.vert(p3[0], p3[1], g.u0, g.v1, c);
    }
    // text with optional dark outline; align: 0 left, 1 center; y = vertical middle
    pub fn text(&mut self, at: &mut Atlas, font: u8, s: &str, x: f32, y: f32, px: f32, c: Col, outline: Option<Col>, align: u8) -> f32 {
        let size = px.round().clamp(6.0, 200.0) as u16;
        let sc = px / size as f32;
        let mut wsum = 0f32;
        for ch in s.chars() {
            wsum += at.advance(font, ch, size as f32) * sc;
        }
        let mut pen = if align == 1 { x - wsum / 2.0 } else { x };
        let base = y + px * 0.35;
        let glyphs: Vec<(Glyph, f32)> = s
            .chars()
            .filter_map(|ch| {
                let g = at.glyph(font, ch, size)?;
                let p = pen;
                pen += g.adv * sc;
                Some((g, p))
            })
            .collect();
        if let Some(oc) = outline {
            let o = (px * 0.09).max(1.0);
            for (g, p) in &glyphs {
                for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (-o * 0.7, -o * 0.7), (o * 0.7, o * 0.7), (-o * 0.7, o * 0.7), (o * 0.7, -o * 0.7)] {
                    self.glyph(g, p + dx, base + dy, sc, 0.0, oc);
                }
            }
        }
        for (g, p) in &glyphs {
            self.glyph(g, *p, base, sc, 0.0, c);
        }
        wsum
    }
}
