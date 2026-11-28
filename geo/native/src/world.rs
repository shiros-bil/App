// Static world data: regions (3 LODs), meshes for GL, neighbours, raster grid, water, seas, cities.
use crate::topo::{self, arc_index, Shape, Topo, MH, MW};
use serde::Deserialize;
use serde_json::Value;

pub const GW: usize = 4096;
pub const GH: usize = 2048;
pub const GX: usize = 32;
pub const GY: usize = 16;
pub const NC: usize = GX * GY;

fn cell_of(x: f32, y: f32) -> usize {
    let cw = MW / GX as f32;
    let ch = MH / GY as f32;
    let cy = ((y / ch) as i32).clamp(0, GY as i32 - 1) as usize;
    let cx = ((x / cw) as i32).clamp(0, GX as i32 - 1) as usize;
    cy * GX + cx
}

#[derive(Default)]
pub struct FillMesh {
    pub pos: Vec<f32>,
    pub rid: Vec<f32>,
    pub sidx: Vec<u32>,
    pub fr: Vec<[u32; 2]>,
    pub fbb: Vec<[f32; 4]>,
}

#[derive(Default)]
pub struct LineMesh {
    pub s: Vec<f32>,
    pub l: Vec<f32>,
    pub an: usize,
    pub rg: Vec<[u32; 2]>,
    pub bb: Vec<[f32; 4]>,
}

pub fn build_fill(pos: Vec<f32>, rid: Vec<f32>, idx: &[u32]) -> FillMesh {
    let n = idx.len();
    let mut cnt = vec![0u32; NC];
    let mut cell = vec![0u16; n / 3];
    let mut fbb = vec![[1e9f32, 1e9, -1e9, -1e9]; NC];
    for q in 0..n / 3 {
        let (a, b, c) = (idx[3 * q] as usize, idx[3 * q + 1] as usize, idx[3 * q + 2] as usize);
        let (xa, ya, xb, yb, xc, yc) = (pos[2 * a], pos[2 * a + 1], pos[2 * b], pos[2 * b + 1], pos[2 * c], pos[2 * c + 1]);
        let k = cell_of((xa + xb + xc) / 3.0, (ya + yb + yc) / 3.0);
        cell[q] = k as u16;
        cnt[k] += 3;
        let bb = &mut fbb[k];
        bb[0] = bb[0].min(xa.min(xb).min(xc));
        bb[1] = bb[1].min(ya.min(yb).min(yc));
        bb[2] = bb[2].max(xa.max(xb).max(xc));
        bb[3] = bb[3].max(ya.max(yb).max(yc));
    }
    let mut fr = vec![[0u32; 2]; NC];
    let mut o = 0u32;
    for k in 0..NC {
        fr[k] = [o, cnt[k]];
        o += cnt[k];
    }
    let mut w: Vec<u32> = fr.iter().map(|f| f[0]).collect();
    let mut sidx = vec![0u32; n];
    for q in 0..n / 3 {
        let k = cell[q] as usize;
        for j in 0..3 {
            sidx[w[k] as usize] = idx[3 * q + j];
            w[k] += 1;
        }
    }
    FillMesh { pos, rid, sidx, fr, fbb }
}

pub fn build_lines(seg: &[f32], attr: &[f32], an: usize) -> LineMesh {
    let ns = seg.len() / 4;
    let mut cell = vec![0u16; ns];
    let mut cnt = vec![0u32; NC];
    for i in 0..ns {
        let k = cell_of((seg[4 * i] + seg[4 * i + 2]) / 2.0, (seg[4 * i + 1] + seg[4 * i + 3]) / 2.0);
        cell[i] = k as u16;
        cnt[k] += 1;
    }
    let mut rg = vec![[0u32; 2]; NC];
    let mut bb = vec![[1e9f32, 1e9, -1e9, -1e9]; NC];
    let mut o = 0;
    for k in 0..NC {
        rg[k] = [o, cnt[k]];
        o += cnt[k];
    }
    let mut w: Vec<u32> = rg.iter().map(|f| f[0]).collect();
    let mut s = vec![0f32; seg.len()];
    let mut l = vec![0f32; ns * an];
    for i in 0..ns {
        let k = cell[i] as usize;
        let q = w[k] as usize;
        w[k] += 1;
        s[4 * q..4 * q + 4].copy_from_slice(&seg[4 * i..4 * i + 4]);
        l[an * q..an * q + an].copy_from_slice(&attr[an * i..an * i + an]);
        let b = &mut bb[k];
        b[0] = b[0].min(seg[4 * i].min(seg[4 * i + 2]));
        b[1] = b[1].min(seg[4 * i + 1].min(seg[4 * i + 3]));
        b[2] = b[2].max(seg[4 * i].max(seg[4 * i + 2]));
        b[3] = b[3].max(seg[4 * i + 1].max(seg[4 * i + 3]));
    }
    LineMesh { s, l, an, rg, bb }
}

fn triangulate(flat: &[f64], holes: &[usize]) -> Vec<usize> {
    earcutr::earcut(flat, holes, 2).unwrap_or_default()
}

pub struct Lod {
    pub arcs: Vec<Vec<[f32; 2]>>,
    pub abb: Vec<[f32; 4]>,
    pub arl: Vec<i32>,
    pub arr: Vec<i32>,
    pub shapes: Vec<Vec<Vec<Vec<i32>>>>,
    pub fill: FillMesh,
    pub line: LineMesh,
    pub line2: LineMesh,
    pub bb: Vec<[f32; 4]>,
    pub area: Vec<f32>,
    pub lp: Vec<[f32; 2]>,
    pub lw: Vec<f32>,
}

pub struct RegionInfo {
    pub name: String,
    pub nat: u16,
    pub pop: f64,
}

fn build_lod(t: &Topo, nat_of: &[u16]) -> Lod {
    let geoms = &t.objects["r"];
    let r_n = geoms.len();
    let na = t.arcs.len();
    let mut arl = vec![-1i32; na];
    let mut arr = vec![-1i32; na];
    let mut shapes = Vec::with_capacity(r_n);
    for (r, g) in geoms.iter().enumerate() {
        let polys = match &g.shape {
            Shape::Polys(p) => p.clone(),
            _ => vec![],
        };
        for poly in &polys {
            for ring in poly {
                for &ai in ring {
                    let a = arc_index(ai);
                    if arl[a] < 0 {
                        arl[a] = r as i32;
                    } else if arl[a] != r as i32 {
                        arr[a] = r as i32;
                    }
                }
            }
        }
        shapes.push(polys);
    }
    let abb: Vec<[f32; 4]> = t
        .arcs
        .iter()
        .map(|f| {
            let mut b = [1e9f32, 1e9, -1e9, -1e9];
            for p in f {
                b[0] = b[0].min(p[0]);
                b[1] = b[1].min(p[1]);
                b[2] = b[2].max(p[0]);
                b[3] = b[3].max(p[1]);
            }
            b
        })
        .collect();
    // fills + metadata
    let mut pos: Vec<f32> = Vec::new();
    let mut rid: Vec<f32> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut bb = vec![[0f32; 4]; r_n];
    let mut area = vec![0f32; r_n];
    let mut lp = vec![[0f32; 2]; r_n];
    let mut lw = vec![0f32; r_n];
    let mut pts: Vec<[f32; 2]> = Vec::new();
    for r in 0..r_n {
        let (mut x0, mut y0, mut x1, mut y1) = (1e9f32, 1e9f32, -1e9f32, -1e9f32);
        let (mut tot, mut big_a, mut bx, mut by, mut bw) = (0f64, -1f64, 0f32, 0f32, 0f32);
        for poly in &shapes[r] {
            let mut flat: Vec<f64> = Vec::new();
            let mut holes: Vec<usize> = Vec::new();
            for (ri, ring) in poly.iter().enumerate() {
                t.ring(ring, &mut pts);
                // area/centroid/bbox (same as JS buildLOD)
                let (mut a, mut cx, mut cy) = (0f64, 0f64, 0f64);
                let (mut rx0, mut rx1, mut ry0, mut ry1) = (1e9f32, -1e9f32, 1e9f32, -1e9f32);
                let m = pts.len();
                for k in 0..m {
                    let p = pts[k];
                    if k > 0 {
                        let q = pts[k - 1];
                        let cr = q[0] as f64 * p[1] as f64 - p[0] as f64 * q[1] as f64;
                        a += cr;
                        cx += (q[0] + p[0]) as f64 * cr;
                        cy += (q[1] + p[1]) as f64 * cr;
                    }
                    rx0 = rx0.min(p[0]);
                    rx1 = rx1.max(p[0]);
                    ry0 = ry0.min(p[1]);
                    ry1 = ry1.max(p[1]);
                }
                if m > 0 {
                    let (p, f) = (pts[m - 1], pts[0]);
                    let cr = p[0] as f64 * f[1] as f64 - f[0] as f64 * p[1] as f64;
                    a += cr;
                    cx += (p[0] + f[0]) as f64 * cr;
                    cy += (p[1] + f[1]) as f64 * cr;
                }
                x0 = x0.min(rx0);
                y0 = y0.min(ry0);
                x1 = x1.max(rx1);
                y1 = y1.max(ry1);
                if ri == 0 {
                    let aa = (a / 2.0).abs();
                    tot += aa;
                    if aa > big_a && a != 0.0 {
                        big_a = aa;
                        bx = (cx / (3.0 * a)) as f32;
                        by = (cy / (3.0 * a)) as f32;
                        bw = (rx1 - rx0).min((ry1 - ry0) * 2.2);
                    } else if big_a < 0.0 {
                        bx = (rx0 + rx1) / 2.0;
                        by = (ry0 + ry1) / 2.0;
                    }
                }
                // triangulation input
                if ri > 0 {
                    holes.push(flat.len() / 2);
                }
                let st = flat.len();
                for p in &pts {
                    flat.push(p[0] as f64);
                    flat.push(p[1] as f64);
                }
                let l = flat.len();
                if l - st >= 4 && flat[l - 2] == flat[st] && flat[l - 1] == flat[st + 1] {
                    flat.truncate(l - 2);
                }
            }
            let base = (pos.len() / 2) as u32;
            let tri = triangulate(&flat, &holes);
            for k in 0..flat.len() / 2 {
                pos.push(flat[2 * k] as f32);
                pos.push(flat[2 * k + 1] as f32);
                rid.push(r as f32);
            }
            for v in tri {
                idx.push(base + v as u32);
            }
        }
        bb[r] = [x0, y0, x1, y1];
        area[r] = tot as f32;
        lp[r] = [bx, by];
        lw[r] = bw;
    }
    let fill = build_fill(pos, rid, &idx);
    // lines: major (between nations / coast) and minor (inside one nation)
    let (mut s1, mut l1, mut s2, mut l2) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (a, f) in t.arcs.iter().enumerate() {
        if arl[a] < 0 {
            continue;
        }
        let l = arl[a];
        let r = if arr[a] < 0 { 65535 } else { arr[a] };
        let minor = r != 65535 && nat_of[l as usize] == nat_of[r as usize];
        let (s, lr) = if minor { (&mut s2, &mut l2) } else { (&mut s1, &mut l1) };
        for k in 0..f.len().saturating_sub(1) {
            let (x0, y0, x1, y1) = (f[k][0], f[k][1], f[k + 1][0], f[k + 1][1]);
            if (x0 < 0.05 && x1 < 0.05) || (x0 > MW - 0.05 && x1 > MW - 0.05) || (y0 > MH - 0.05 && y1 > MH - 0.05) {
                continue;
            }
            s.extend_from_slice(&[x0, y0, x1, y1]);
            lr.extend_from_slice(&[l as f32, r as f32]);
        }
    }
    let line = build_lines(&s1, &l1, 2);
    let line2 = build_lines(&s2, &l2, 2);
    Lod { arcs: t.arcs.clone(), abb, arl, arr, shapes, fill, line, line2, bb, area, lp, lw }
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct NationDef {
    pub n: String,
    #[serde(default)]
    pub e: String,
    #[serde(default)]
    pub iso: String,
    #[serde(default)]
    pub f: String,
    #[serde(default)]
    pub pop: f64,
    #[serde(default)]
    pub gdp: f64,
    #[serde(default)]
    pub c: String,
    #[serde(default)]
    pub g: f64,
    pub par: Option<i64>,
    pub army: Option<f64>,
    pub peace: Option<Value>,
}

pub struct Lake {
    pub name: String,
    pub kind: String,
    pub rank: i32,
    pub rings: Vec<Vec<[f32; 2]>>,
    pub bb: [f32; 4],
}
pub struct River {
    pub name: String,
    pub kind: String,
    pub rank: i32,
    pub canal: bool,
    pub lines: Vec<Vec<[f32; 2]>>,
    pub bb: [f32; 4],
}
pub struct Sea {
    pub name: String,
    pub kind: String,
    pub ocean: bool,
    pub rings: Vec<Vec<[f32; 2]>>,
    pub area: f32,
    pub bb: [f32; 4],
    pub c: [f32; 2],
    pub outline: usize, // index into sea_ol (shared per name)
}
pub struct City {
    pub x: f32,
    pub y: f32,
    pub name: String,
    pub cap: bool,
}

pub struct WaterMesh {
    pub rg: Vec<i32>,
    pub fill: Vec<FillMesh>,
    pub riv: Vec<LineMesh>,
    pub ol: Vec<LineMesh>,
}

pub struct World {
    pub nations: Vec<NationDef>,
    pub regions: Vec<RegionInfo>,
    pub lods: Vec<Lod>,
    pub nb: Vec<Vec<u32>>,
    pub reg: Vec<u16>,
    pub rc: Vec<u32>,
    pub rkm: Vec<f32>,
    pub bp: std::collections::HashMap<u64, [f32; 3]>,
    pub lakes: Vec<Lake>,
    pub rivers: Vec<River>,
    pub water: WaterMesh,
    pub seas: Vec<Sea>,
    pub sea_ol: Vec<Vec<[f32; 4]>>,
    pub cities: Vec<City>,
}

pub const KMU: f32 = 20.04;

impl World {
    pub fn meta(&self) -> &Lod {
        &self.lods[1]
    }
    pub fn bb(&self, r: usize) -> [f32; 4] {
        self.lods[1].bb[r]
    }
    pub fn lp(&self, r: usize) -> [f32; 2] {
        self.lods[1].lp[r]
    }
    pub fn area(&self, r: usize) -> f32 {
        self.lods[1].area[r]
    }
    pub fn nreg(&self) -> usize {
        self.regions.len()
    }
}

fn rasterize(t: &Topo) -> Vec<u16> {
    let mut grid = vec![0u16; GW * GH];
    let geoms = &t.objects["r"];
    let (gw, gh) = (GW as f64, GH as f64);
    let mut rows: std::collections::HashMap<i32, Vec<f64>> = std::collections::HashMap::new();
    for (r, g) in geoms.iter().enumerate() {
        let polys = match &g.shape {
            Shape::Polys(p) => p,
            _ => continue,
        };
        for poly in polys {
            rows.clear();
            for ring in poly {
                let (mut px, mut py) = (f64::NAN, f64::NAN);
                for &ai in ring {
                    let rev = ai < 0;
                    let a = arc_index(ai);
                    let f = &t.arcs[a];
                    let m = f.len();
                    for k in 0..m {
                        let j = if rev { m - 1 - k } else { k };
                        let x = f[j][0] as f64 / MW as f64 * gw;
                        let y = f[j][1] as f64 / MH as f64 * gh;
                        if !px.is_nan() && py != y {
                            let (y0, y1) = (py.min(y), py.max(y));
                            let j0 = (y0 - 0.5).ceil() as i32;
                            let j1 = (y1 - 0.5).ceil() as i32;
                            for row in j0.max(0)..j1.min(GH as i32) {
                                let yc = row as f64 + 0.5;
                                let cx = px + (yc - py) * (x - px) / (y - py);
                                rows.entry(row).or_default().push(cx);
                            }
                        }
                        px = x;
                        py = y;
                    }
                }
            }
            for (row, l) in rows.iter_mut() {
                l.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let o = *row as usize * GW;
                let mut i = 0;
                while i + 1 < l.len() {
                    let a = ((l[i] - 0.5).ceil().max(0.0)) as usize;
                    let b = ((l[i + 1] - 0.5).ceil().min(gw)) as usize;
                    for c in a..b {
                        grid[o + c] = (r + 1) as u16;
                    }
                    i += 2;
                }
            }
        }
    }
    grid
}

fn neighbours(t: &Topo, n: usize) -> Vec<Vec<u32>> {
    let mut users: Vec<Vec<u32>> = vec![Vec::new(); t.arcs.len()];
    for (r, g) in t.objects["r"].iter().enumerate() {
        if let Shape::Polys(polys) = &g.shape {
            for poly in polys {
                for ring in poly {
                    for &ai in ring {
                        let a = arc_index(ai);
                        let u = &mut users[a];
                        if !u.contains(&(r as u32)) {
                            u.push(r as u32);
                        }
                    }
                }
            }
        }
    }
    let mut nb: Vec<Vec<u32>> = vec![Vec::new(); n];
    for u in &users {
        for i in 0..u.len() {
            for j in 0..u.len() {
                if i != j && !nb[u[i] as usize].contains(&u[j]) {
                    nb[u[i] as usize].push(u[j]);
                }
            }
        }
    }
    for l in nb.iter_mut() {
        l.sort();
    }
    nb
}

fn ring_area_c(r: &[[f32; 2]]) -> (f64, f64, f64) {
    let (mut a, mut cx, mut cy) = (0f64, 0f64, 0f64);
    let n = r.len();
    for i in 0..n {
        let p = r[i];
        let q = r[(i + 1) % n];
        let cr = p[0] as f64 * q[1] as f64 - q[0] as f64 * p[1] as f64;
        a += cr;
        cx += (p[0] + q[0]) as f64 * cr;
        cy += (p[1] + q[1]) as f64 * cr;
    }
    (a / 2.0, cx, cy)
}

fn edge_seg(a: [f32; 2], b: [f32; 2]) -> bool {
    let e = 0.6;
    (a[0] < e && b[0] < e) || (a[0] > MW - e && b[0] > MW - e) || (a[1] > MH - e && b[1] > MH - e) || (a[1] < e && b[1] < e)
}

pub struct Assets<'a> {
    pub nations: &'a [u8],
    pub world_lo: &'a [u8],
    pub world_mid: &'a [u8],
    pub world_hi: &'a [u8],
    pub water: &'a [u8],
    pub marine: &'a [u8],
    pub cities: &'a [u8],
}

pub fn load(a: &Assets, progress: &dyn Fn(&str)) -> World {
    progress("Загрузка стран…");
    let nations: Vec<NationDef> = serde_json::from_slice(a.nations).expect("nations");
    progress("Загрузка карты (1/3)…");
    let lo = topo::parse(a.world_lo);
    let regions: Vec<RegionInfo> = lo.objects["r"]
        .iter()
        .map(|g| RegionInfo {
            name: g.props.get("n").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            nat: g.props.get("k").and_then(|v| v.as_i64()).unwrap_or(0) as u16,
            pop: g.props.get("p").and_then(|v| v.as_f64()).unwrap_or(0.0),
        })
        .collect();
    let nat_of: Vec<u16> = regions.iter().map(|r| r.nat).collect();
    let nb = neighbours(&lo, regions.len());
    let lod0 = build_lod(&lo, &nat_of);
    drop(lo);
    progress("Загрузка карты (2/3)…");
    let mid = topo::parse(a.world_mid);
    let lod1 = build_lod(&mid, &nat_of);
    let reg = rasterize(&mid);
    drop(mid);
    progress("Загрузка карты (3/3)…");
    let hi = topo::parse(a.world_hi);
    let lod2 = build_lod(&hi, &nat_of);
    drop(hi);
    let n = regions.len();
    let mut rc = vec![0u32; n];
    for &v in &reg {
        if v > 0 {
            rc[v as usize - 1] += 1;
        }
    }
    let mut rkm = vec![0f32; n];
    for r in 0..n {
        let c = ((90.0 - lod1.lp[r][1] / MH * 180.0) * std::f32::consts::PI / 180.0).cos();
        rkm[r] = lod1.area[r] * KMU * KMU * c.max(0.05);
    }
    // border points between neighbouring regions (from the coarse LOD), used by region routes
    let mut bp = std::collections::HashMap::new();
    for a in 0..lod0.arcs.len() {
        let (l, r) = (lod0.arl[a], lod0.arr[a]);
        if l < 0 || r < 0 {
            continue;
        }
        let f = &lod0.arcs[a];
        let k = if l < r { l as u64 * 100000 + r as u64 } else { r as u64 * 100000 + l as u64 };
        let b = lod0.abb[a];
        let len = (b[2] - b[0]).abs() + (b[3] - b[1]).abs();
        if let Some(o) = bp.get(&k) {
            let o: &[f32; 3] = o;
            if o[2] >= len {
                continue;
            }
        }
        let j = f.len() >> 1;
        bp.insert(k, [f[j][0], f[j][1], len]);
    }
    progress("Загрузка озёр и рек…");
    let w = topo::parse(a.water);
    let mut lakes = Vec::new();
    let mut rivers = Vec::new();
    let mut pts = Vec::new();
    let rgs = vec![1, 3, 5, 7, 8, 99];
    let grp = |r: i32| rgs.iter().position(|&t| r <= t).unwrap_or(rgs.len() - 1);
    let mut lpos: Vec<f32> = Vec::new();
    let mut lrk: Vec<f32> = Vec::new();
    let mut lidx: Vec<Vec<u32>> = vec![Vec::new(); rgs.len()];
    let mut oseg: Vec<Vec<f32>> = vec![Vec::new(); rgs.len()];
    let mut osr: Vec<Vec<f32>> = vec![Vec::new(); rgs.len()];
    let mut rseg: Vec<Vec<f32>> = vec![Vec::new(); rgs.len()];
    let mut rsr: Vec<Vec<f32>> = vec![Vec::new(); rgs.len()];
    for g in &w.objects["l"] {
        let rank = g.props.get("r").and_then(|v| v.as_i64()).unwrap_or(10) as i32;
        let gi = grp(rank);
        let mut rings_all = Vec::new();
        if let Shape::Polys(polys) = &g.shape {
            for poly in polys {
                let mut flat: Vec<f64> = Vec::new();
                let mut holes = Vec::new();
                for (ri, ring) in poly.iter().enumerate() {
                    w.ring(ring, &mut pts);
                    if ri > 0 {
                        holes.push(flat.len() / 2);
                    }
                    for k in 0..pts.len().saturating_sub(1) {
                        flat.push(pts[k][0] as f64);
                        flat.push(pts[k][1] as f64);
                    }
                    for k in 1..pts.len() {
                        oseg[gi].extend_from_slice(&[pts[k - 1][0], pts[k - 1][1], pts[k][0], pts[k][1]]);
                        osr[gi].push(rank as f32);
                    }
                    rings_all.push(pts.clone());
                }
                let base = (lpos.len() / 2) as u32;
                for k in 0..flat.len() / 2 {
                    lpos.push(flat[2 * k] as f32);
                    lpos.push(flat[2 * k + 1] as f32);
                    lrk.push(rank as f32);
                }
                for v in triangulate(&flat, &holes) {
                    lidx[gi].push(base + v as u32);
                }
            }
        }
        let mut bb = [1e9f32, 1e9, -1e9, -1e9];
        for r in &rings_all {
            for p in r {
                bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
            }
        }
        lakes.push(Lake {
            name: g.props.get("n").and_then(|v| v.as_str()).unwrap_or("").into(),
            kind: g.props.get("t").and_then(|v| v.as_str()).unwrap_or("Озеро").into(),
            rank,
            rings: rings_all,
            bb,
        });
    }
    for g in &w.objects["v"] {
        let rank = g.props.get("r").and_then(|v| v.as_i64()).unwrap_or(10) as i32;
        let gi = grp(rank);
        let mut lines = Vec::new();
        if let Shape::Lines(ls) = &g.shape {
            for l in ls {
                w.line(l, &mut pts);
                for k in 1..pts.len() {
                    rseg[gi].extend_from_slice(&[pts[k - 1][0], pts[k - 1][1], pts[k][0], pts[k][1]]);
                    rsr[gi].push(rank as f32);
                }
                lines.push(pts.clone());
            }
        }
        let mut bb = [1e9f32, 1e9, -1e9, -1e9];
        for l in &lines {
            for p in l {
                bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
            }
        }
        rivers.push(River {
            name: g.props.get("n").and_then(|v| v.as_str()).unwrap_or("").into(),
            kind: g.props.get("t").and_then(|v| v.as_str()).unwrap_or("Река").into(),
            rank,
            canal: false,
            lines,
            bb,
        });
    }
    let pr = |lon: f32, lat: f32| [(lon + 180.0) / 360.0 * MW, (90.0 - lat) / 180.0 * MH];
    let canals: [(&str, &[[f32; 2]]); 2] = [
        ("Суэцкий канал", &[[32.30, 31.27], [32.31, 31.05], [32.30, 30.80], [32.28, 30.60], [32.33, 30.45], [32.42, 30.33], [32.53, 30.15], [32.56, 29.95]]),
        ("Панамский канал", &[[-79.92, 9.38], [-79.92, 9.27], [-79.85, 9.20], [-79.75, 9.15], [-79.68, 9.10], [-79.62, 9.02], [-79.58, 8.96], [-79.55, 8.90]]),
    ];
    for (n, c) in canals.iter() {
        let l: Vec<[f32; 2]> = c.iter().map(|q| pr(q[0], q[1])).collect();
        let mut bb = [1e9f32, 1e9, -1e9, -1e9];
        for p in &l {
            bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
        }
        rivers.push(River { name: n.to_string(), kind: "Канал".into(), rank: 0, canal: true, lines: vec![l], bb });
    }
    let water = WaterMesh {
        rg: rgs.clone(),
        fill: (0..rgs.len()).map(|g| build_fill(lpos.clone(), lrk.clone(), &lidx[g])).collect(),
        riv: (0..rgs.len()).map(|g| build_lines(&rseg[g], &rsr[g], 1)).collect(),
        ol: (0..rgs.len()).map(|g| build_lines(&oseg[g], &osr[g], 1)).collect(),
    };
    drop(w);
    progress("Загрузка морей…");
    let m = topo::parse(a.marine);
    let mut seas = Vec::new();
    let mut segs_by: Vec<Vec<([f32; 2], [f32; 2])>> = Vec::new();
    for g in &m.objects["m"] {
        let mut rings = Vec::new();
        let mut segs = Vec::new();
        if let Shape::Polys(polys) = &g.shape {
            for poly in polys {
                for ring in poly {
                    m.ring(ring, &mut pts);
                    for k in 1..pts.len() {
                        if !edge_seg(pts[k - 1], pts[k]) {
                            segs.push((pts[k - 1], pts[k]));
                        }
                    }
                    rings.push(pts.clone());
                }
            }
        }
        let (mut ta, mut tcx, mut tcy) = (0f64, 0f64, 0f64);
        let mut bb = [1e9f32, 1e9, -1e9, -1e9];
        for r in &rings {
            let (a, cx, cy) = ring_area_c(r);
            ta += a;
            tcx += cx;
            tcy += cy;
            for p in r {
                bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
            }
        }
        let c = if ta.abs() > 1e-9 { [(tcx / (6.0 * ta)) as f32, (tcy / (6.0 * ta)) as f32] } else { [(bb[0] + bb[2]) / 2.0, (bb[1] + bb[3]) / 2.0] };
        seas.push(Sea {
            name: g.props.get("n").and_then(|v| v.as_str()).unwrap_or("").into(),
            kind: g.props.get("t").and_then(|v| v.as_str()).unwrap_or("").into(),
            ocean: g.props.get("o").and_then(|v| v.as_i64()).unwrap_or(0) != 0,
            rings,
            area: ta.abs() as f32,
            bb,
            c,
            outline: 0,
        });
        segs_by.push(segs);
    }
    // outline per sea name: segments that appear exactly once among all parts with that name
    let mut sea_ol = Vec::new();
    let mut by_name: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, s) in seas.iter().enumerate() {
        by_name.entry(s.name.clone()).or_default().push(i);
    }
    let key = |a: [f32; 2], b: [f32; 2]| {
        let k1 = format!("{:.1},{:.1}", a[0], a[1]);
        let k2 = format!("{:.1},{:.1}", b[0], b[1]);
        if k1 < k2 { format!("{}|{}", k1, k2) } else { format!("{}|{}", k2, k1) }
    };
    for (_, idxs) in by_name.iter() {
        let mut cnt: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for &i in idxs {
            for (a, b) in &segs_by[i] {
                *cnt.entry(key(*a, *b)).or_insert(0) += 1;
            }
        }
        let mut ol = Vec::new();
        for &i in idxs {
            for (a, b) in &segs_by[i] {
                if cnt[&key(*a, *b)] == 1 {
                    ol.push([a[0], a[1], b[0], b[1]]);
                }
            }
        }
        let oi = sea_ol.len();
        sea_ol.push(ol);
        for &i in idxs {
            seas[i].outline = oi;
        }
    }
    progress("Загрузка городов…");
    let raw: Vec<(f32, f32, String, i32)> = serde_json::from_slice(a.cities).unwrap_or_default();
    let mut cities: Vec<City> = raw
        .into_iter()
        .map(|(lon, lat, n, c)| City { x: (lon + 180.0) / 360.0 * MW, y: (90.0 - lat) / 180.0 * MH, name: n.replace('\u{301}', ""), cap: c != 0 })
        .collect();
    cities.sort_by_key(|c| if c.cap { 0 } else { 1 });
    World { nations, regions, lods: vec![lod0, lod1, lod2], nb, reg, rc, rkm, bp, lakes, rivers, water, seas, sea_ol, cities }
}

pub fn point_in_rings(rings: &[Vec<[f32; 2]>], x: f32, y: f32) -> bool {
    let mut inside = false;
    for r in rings {
        let n = r.len();
        let mut j = n.wrapping_sub(1);
        for i in 0..n {
            let (a, b) = (r[i], r[j]);
            if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}
