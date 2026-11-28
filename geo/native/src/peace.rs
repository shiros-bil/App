// Peace conference: paint occupied cells into treaty items (new region / join region), then sign.
use crate::sim::{fmt, fmt_n, Sim, Zone};
use crate::world::{GH, GW};
use std::collections::{BTreeMap, HashMap};

pub const PCOL: [[u8; 3]; 7] = [[80, 200, 255], [255, 120, 200], [140, 255, 120], [255, 170, 60], [180, 140, 255], [255, 90, 90], [90, 255, 220]];

#[derive(Clone)]
pub struct PcItem {
    pub k: String,
    pub n: String,
    pub t: i64,
    pub dead: bool,
}

pub struct Peace {
    pub x: u16,
    pub av: Vec<u32>,
    pub ps: Vec<u8>,
    pub avs: Vec<bool>,
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
    pub bbox: [f32; 4],
    pub items: Vec<PcItem>,
    pub tool: String,
    pub br: f32,
    pub ver: u64,
    pub hi_ver: u64,
    pub cam: [f32; 3], // x, y, k (screen = map*k + xy)
    pub cam_init: bool,
}

const ADJ: &[(&str, &str)] = &[
    ("ru", "Русск"), ("cn", "Китайск"), ("us", "Американск"), ("de", "Немецк"), ("ua", "Украинск"), ("fr", "Французск"), ("gb", "Британск"), ("tr", "Турецк"), ("jp", "Японск"),
    ("in", "Индийск"), ("pl", "Польск"), ("by", "Белорусск"), ("kz", "Казахск"), ("it", "Итальянск"), ("es", "Испанск"), ("ir", "Иранск"), ("br", "Бразильск"), ("ca", "Канадск"),
    ("mx", "Мексиканск"), ("kr", "Корейск"), ("kp", "Северокорейск"), ("sa", "Саудовск"), ("eg", "Египетск"), ("il", "Израильск"), ("ge", "Грузинск"), ("am", "Армянск"),
    ("az", "Азербайджанск"), ("ro", "Румынск"), ("hu", "Венгерск"), ("fi", "Финск"), ("se", "Шведск"), ("no", "Норвежск"), ("pk", "Пакистанск"), ("af", "Афганск"), ("iq", "Иракск"),
    ("sy", "Сирийск"), ("mn", "Монгольск"), ("vn", "Вьетнамск"), ("th", "Тайск"), ("id", "Индонезийск"), ("au", "Австралийск"), ("ar", "Аргентинск"), ("at", "Австрийск"),
    ("ch", "Швейцарск"), ("nl", "Голландск"), ("be", "Бельгийск"), ("gr", "Греческ"), ("rs", "Сербск"), ("md", "Молдавск"), ("lt", "Литовск"), ("lv", "Латвийск"), ("ee", "Эстонск"),
    ("cz", "Чешск"), ("sk", "Словацк"), ("bg", "Болгарск"), ("uz", "Узбекск"), ("tm", "Туркменск"), ("kg", "Киргизск"), ("tj", "Таджикск"), ("dnr", "Донецк"), ("lnr", "Луганск"),
];

impl Sim {
    pub fn auto_name(&self, o: u16, r: u32) -> String {
        let n = if self.rn[r as usize].is_empty() { "регион".to_string() } else { self.rn[r as usize].clone() };
        let iso = &self.w.nations[o as usize].iso;
        let st = match ADJ.iter().find(|a| a.0 == iso) {
            Some(a) => a.1,
            None => return format!("{} ({})", n, self.nm(o)),
        };
        let w = n.split(' ').next().unwrap_or("").to_lowercase();
        let ends = |s: &[&str]| s.iter().any(|e| w.ends_with(e));
        let g = if ends(&["ая", "яя", "а", "я", "ь"]) && !ends(&["ий", "ый", "ой"]) {
            "ая"
        } else if ends(&["ое", "ее", "о", "е"]) {
            "ое"
        } else {
            "ий"
        };
        format!("{}{} {}", st, g, n)
    }

    pub fn open_peace(&mut self, x: u16) {
        self.speed = 0;
        let me = self.g.player;
        let mut av = Vec::new();
        let (mut x0, mut y0, mut x1, mut y1) = (GW, GH, 0, 0);
        for i in 0..self.ctl.len() {
            let z = self.ctl[i];
            if z == 0 {
                continue;
            }
            let zz = &self.g.zones[z as usize - 1];
            if zz.k != 'o' || zz.o != me || self.g.owner[zz.r as usize] != x {
                continue;
            }
            av.push(i as u32);
            let (cx, cy) = (i % GW, i / GW);
            x0 = x0.min(cx);
            x1 = x1.max(cx);
            y0 = y0.min(cy);
            y1 = y1.max(cy);
        }
        let bbox = if !av.is_empty() {
            [x0 as f32 * crate::topo::MW / GW as f32, y0 as f32 * crate::topo::MH / GH as f32, (x1 + 1) as f32 * crate::topo::MW / GW as f32, (y1 + 1) as f32 * crate::topo::MH / GH as f32]
        } else {
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for &r in self.terr(x) {
                let bb = self.w.bb(r as usize);
                b = [b[0].min(bb[0]), b[1].min(bb[1]), b[2].max(bb[2]), b[3].max(bb[3])];
            }
            x0 = 0;
            y0 = 0;
            x1 = 0;
            y1 = 0;
            b
        };
        let mut avs = vec![false; GW * GH];
        for &i in &av {
            avs[i as usize] = true;
        }
        self.pc = Some(Peace { x, av, ps: vec![0; GW * GH], avs, x0, y0, x1, y1, bbox, items: vec![], tool: "brush".into(), br: 4.0, ver: 1, hi_ver: 0, cam: [0.0, 0.0, 1.0], cam_init: false });
        self.dirty_ui = true;
    }
    fn pc_set(pc: &mut Peace, i: usize, v: u8) {
        if pc.ps[i] != v {
            pc.ps[i] = v;
            pc.ver += 1;
        }
    }
    pub fn pc_brush(&mut self, cx: f32, cy: f32) {
        let ctl = &self.ctl;
        let pc = match self.pc.as_mut() {
            Some(p) => p,
            None => return,
        };
        if pc.tool == "zone" {
            let i = (cy as usize).min(GH - 1) * GW + (cx as usize).min(GW - 1);
            if !pc.avs[i] {
                return;
            }
            let z = ctl[i];
            let av = pc.av.clone();
            for j in av {
                if ctl[j as usize] == z && pc.ps[j as usize] < 2 {
                    Sim::pc_set(pc, j as usize, 1);
                }
            }
            return;
        }
        let ry = pc.br;
        let rx = ry / ((90.0 - cy / GH as f32 * 180.0) * std::f32::consts::PI / 180.0).cos().max(0.2);
        let erase = pc.tool == "erase";
        for j in (cy - ry).floor() as i64..=(cy + ry).ceil() as i64 {
            if j < 0 || j >= GH as i64 {
                continue;
            }
            let dy = (j as f32 + 0.5 - cy) / ry;
            if dy * dy > 1.0 {
                continue;
            }
            let hw = rx * (1.0 - dy * dy).sqrt();
            for ii in (cx - hw).floor() as i64..=(cx + hw).ceil() as i64 {
                if ii < 0 || ii >= GW as i64 {
                    continue;
                }
                let i = j as usize * GW + ii as usize;
                if !pc.avs[i] {
                    continue;
                }
                if erase {
                    Sim::pc_set(pc, i, 0);
                } else if pc.ps[i] == 0 {
                    Sim::pc_set(pc, i, 1);
                }
            }
        }
    }
    pub fn pc_all(&mut self, v: bool) {
        if let Some(pc) = self.pc.as_mut() {
            let av = pc.av.clone();
            for i in av {
                let i = i as usize;
                if v && pc.ps[i] == 0 {
                    Sim::pc_set(pc, i, 1);
                } else if !v && pc.ps[i] == 1 {
                    Sim::pc_set(pc, i, 0);
                }
            }
        }
    }
    fn pc_stats(&self, f: &dyn Fn(u8) -> bool) -> (usize, f64, f64, BTreeMap<u32, u32>) {
        let pc = self.pc.as_ref().unwrap();
        let mut n = 0;
        let mut per: BTreeMap<u32, u32> = BTreeMap::new();
        for &i in &pc.av {
            if !f(pc.ps[i as usize]) {
                continue;
            }
            n += 1;
            *per.entry(self.w.reg[i as usize] as u32 - 1).or_insert(0) += 1;
        }
        let (mut p, mut g) = (0.0, 0.0);
        for (&r, &c) in &per {
            let rc = self.w.rc[r as usize].max(1) as f64;
            p += self.g.pop[r as usize] * c as f64 / rc;
            g += self.g.gdp[r as usize] * c as f64 / rc;
        }
        (n, p, g, per)
    }
    pub fn pc_state_json(&self) -> String {
        let pc = match self.pc.as_ref() {
            Some(p) => p,
            None => return "null".into(),
        };
        let me = self.g.player;
        let sel = self.pc_stats(&|v| v == 1);
        let all_sel = self.pc_stats(&|v| v >= 1);
        let taken = self.pc_stats(&|_| true);
        let info = if !pc.av.is_empty() {
            format!("Занято вами: {} чел.\nВыделено: {} чел. · ${}\nМожно забрать только оккупированные земли. Неразмеченное вернётся {}.", fmt_n(taken.1), fmt_n(sel.1), fmt(sel.2), self.nm(pc.x))
        } else {
            format!("У вас нет оккупированных земель {}. Можно подписать белый мир.", self.nm(pc.x))
        };
        let mut cand: Vec<u32> = Vec::new();
        for &r in all_sel.3.keys() {
            for &n in &self.w.nb[r as usize] {
                if self.g.owner[n as usize] == me && !cand.contains(&n) {
                    cand.push(n);
                }
            }
        }
        let mut list: Vec<u32> = if cand.is_empty() { self.terr(me).clone() } else { cand };
        list.retain(|&r| !self.rn[r as usize].is_empty());
        list.sort_by(|a, b| self.rn[*a as usize].cmp(&self.rn[*b as usize]));
        let items: Vec<serde_json::Value> = pc
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| !it.dead)
            .map(|(k, it)| {
                let v = (k + 2) as u8;
                let st = self.pc_stats(&|x| x == v);
                let c = PCOL[k % PCOL.len()];
                serde_json::json!({"k":k,"label":format!("{} «{}»", if it.k=="new" {"Новый регион"} else {"К региону"}, it.n),"pop":fmt_n(st.1),"color":format!("#{:02x}{:02x}{:02x}",c[0],c[1],c[2])})
            })
            .collect();
        serde_json::json!({
            "x": pc.x, "me": me, "meName": self.nm(me), "xName": self.nm(pc.x),
            "meFlag": self.w.nations[me as usize].f, "xFlag": self.w.nations[pc.x as usize].f,
            "info": info, "tool": pc.tool, "br": pc.br,
            "regs": list.iter().map(|&r| serde_json::json!([r, self.rn[r as usize]])).collect::<Vec<_>>(),
            "items": items,
        })
        .to_string()
    }
    pub fn pc_item(&mut self, kind: &str, name: &str, reg: i64) {
        if self.pc_stats(&|v| v == 1).0 == 0 {
            self.toast("Сначала выделите территорию кистью");
            return;
        }
        let n = if kind == "new" {
            let n = name.trim().to_string();
            if n.is_empty() {
                self.toast("Введите название региона");
                return;
            }
            n
        } else {
            if reg < 0 {
                return;
            }
            self.rn[reg as usize].clone()
        };
        let pc = self.pc.as_mut().unwrap();
        pc.items.push(PcItem { k: kind.into(), n, t: if kind == "add" { reg } else { -1 }, dead: false });
        let v = (pc.items.len() + 1) as u8;
        let av = pc.av.clone();
        for i in av {
            if pc.ps[i as usize] == 1 {
                Sim::pc_set(pc, i as usize, v);
            }
        }
    }
    pub fn pc_del(&mut self, k: usize) {
        if let Some(pc) = self.pc.as_mut() {
            if k < pc.items.len() {
                pc.items[k].dead = true;
                let v = (k + 2) as u8;
                let av = pc.av.clone();
                for i in av {
                    if pc.ps[i as usize] == v {
                        Sim::pc_set(pc, i as usize, 0);
                    }
                }
            }
        }
    }
    fn fill_holes(&mut self, r: u32, zid: u16) {
        let [x0, y0, x1, y1] = self.cell_box(r as usize);
        let w = x1 - x0 + 1;
        let h = y1 - y0 + 1;
        let mut vis = vec![0u8; w * h];
        let r1 = (r + 1) as u16;
        let mut q: Vec<usize> = Vec::new();
        for j in y0..=y1 {
            for i in x0..=x1 {
                let li = (j - y0) * w + i - x0;
                let k = j * GW + i;
                if vis[li] != 0 || self.w.reg[k] != r1 || self.ctl[k] == zid {
                    continue;
                }
                q.clear();
                q.push(k);
                vis[li] = 1;
                let mut ok = true;
                let mut a = 0;
                while a < q.len() {
                    let c = q[a];
                    a += 1;
                    if q.len() > 80 {
                        ok = false;
                        break;
                    }
                    for o in [-1i64, 1, -(GW as i64), GW as i64] {
                        let n = (c as i64 + o) as usize;
                        let (ni, nj) = (n % GW, n / GW);
                        if ni < x0 || ni > x1 || nj < y0 || nj > y1 {
                            ok = false;
                            continue;
                        }
                        let nl = (nj - y0) * w + ni - x0;
                        if self.w.reg[n] == r1 && self.ctl[n] != zid && vis[nl] == 0 {
                            vis[nl] = 1;
                            q.push(n);
                        }
                    }
                }
                if ok {
                    for &c in q.clone().iter() {
                        self.set_cell(c, zid);
                    }
                }
            }
        }
    }
    pub fn pc_sign(&mut self) {
        let mut pc = match self.pc.take() {
            Some(p) => p,
            None => return,
        };
        let me = self.g.player;
        let x = pc.x;
        let (mut got, mut _whole) = (0, 0);
        let mut sel1: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for &i in &pc.av {
            if pc.ps[i as usize] == 1 {
                sel1.entry(self.w.reg[i as usize] as u32 - 1).or_default().push(i);
            }
        }
        for (r, cells) in sel1 {
            let n = self.auto_name(me, r);
            pc.items.push(PcItem { k: "auto".into(), n, t: -1, dead: false });
            let v = (pc.items.len() + 1) as u8;
            for i in cells {
                pc.ps[i as usize] = v;
            }
        }
        let items = pc.items.clone();
        for (k, it) in items.iter().enumerate() {
            if it.dead {
                continue;
            }
            self.g.gid += 1;
            let mut gid = self.g.gid;
            if it.k == "add" && it.t >= 0 && self.g.owner[it.t as usize] == me {
                if let Some(&g) = self.g.rg.get(&(it.t as u32)) {
                    gid = g;
                } else {
                    self.g.rg.insert(it.t as u32, gid);
                }
            }
            let v = (k + 2) as u8;
            let mut per: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
            for &i in &pc.av {
                if pc.ps[i as usize] == v {
                    per.entry(self.w.reg[i as usize] as u32 - 1).or_default().push(i);
                }
            }
            for (r, cells) in per {
                got += 1;
                if cells.len() as u32 >= self.w.rc[r as usize] {
                    self.g.owner[r as usize] = me;
                    self.occ_clear_region(r);
                    _whole += 1;
                    if it.k != "add" {
                        self.g.rg.insert(r, gid);
                    }
                    if it.k == "new" {
                        self.g.rn.insert(r, it.n.clone());
                        self.rn[r as usize] = it.n.clone();
                    }
                    continue;
                }
                self.g.zones.push(Zone { r, o: me, k: 'a', c: 0, n: it.n.clone(), m: it.t, g: gid });
                let zid = self.g.zones.len() as u16;
                for i in cells {
                    self.set_cell(i as usize, zid);
                }
                self.fill_holes(r, zid);
            }
        }
        for &i in &pc.av {
            let z = self.ctl[i as usize];
            if z != 0 && self.g.zones[z as usize - 1].k == 'o' && self.g.zones[z as usize - 1].o == me {
                self.set_cell(i as usize, 0);
            }
        }
        self.g.war.retain(|&w| w != x);
        self.g.rel.insert(x, -20.0);
        let terr_me: Vec<u32> = self.terr(me).clone();
        for di in 0..self.g.divs.len() {
            let r = self.g.divs[di].r;
            if self.can_enter(r) {
                continue;
            }
            let (dx, dy) = (self.g.divs[di].x, self.g.divs[di].y);
            let mut b: i64 = -1;
            let mut bd = f32::MAX;
            for &rr in &terr_me {
                let lp = self.w.lp(rr as usize);
                let dd = (lp[0] - dx).powi(2) + (lp[1] - dy).powi(2);
                if dd < bd {
                    bd = dd;
                    b = rr as i64;
                }
            }
            if b >= 0 {
                let lp = self.w.lp(b as usize);
                let d = &mut self.g.divs[di];
                d.x = lp[0];
                d.y = lp[1];
                d.r = b as u32;
                d.path.clear();
            }
        }
        let word = if got % 10 == 1 && got % 100 != 11 {
            "территория"
        } else if (2..=4).contains(&(got % 10)) && !(10..20).contains(&(got % 100)) {
            "территории"
        } else {
            "территорий"
        };
        let s = if got > 0 { format!("Мирный договор с {}: присоединено {} {}.", self.nm(x), got, word) } else { format!("Мирный договор с {} (белый мир).", self.nm(x)) };
        self.add_log(s.clone());
        self.toast(s);
        self.recalc();
        self.zone_full = true;
        self.dirty_ui = true;
    }
    pub fn pc_close(&mut self) {
        self.pc = None;
        self.dirty_ui = true;
    }
    // hi-res selection overlay (4x, smooth threshold), RGBA, covering cells x0..x1, y0..y1
    pub fn pc_hi(&self) -> Option<(usize, usize, Vec<u8>)> {
        let pc = self.pc.as_ref()?;
        if pc.av.is_empty() {
            return None;
        }
        let (w, h) = (pc.x1 - pc.x0 + 1, pc.y1 - pc.y0 + 1);
        let o = 4usize;
        let v = |i: i64, j: i64| -> u8 {
            if i < 0 || j < 0 || i >= w as i64 || j >= h as i64 {
                0
            } else {
                pc.ps[(pc.y0 + j as usize) * GW + pc.x0 + i as usize]
            }
        };
        let w2 = w + 2;
        let mut f = vec![0f32; w2 * (h + 2)];
        for j in -1..=h as i64 {
            for i in -1..=w as i64 {
                let mut a = 0f32;
                for dj in -1..=1 {
                    for di in -1..=1 {
                        if v(i + di, j + dj) != 0 {
                            a += (if di != 0 { 1.0 } else { 2.0 }) * (if dj != 0 { 1.0 } else { 2.0 });
                        }
                    }
                }
                f[(j + 1) as usize * w2 + (i + 1) as usize] = a / 16.0;
            }
        }
        let (pw, ph) = (w * o, h * o);
        if pw * ph > 16_000_000 {
            return None;
        }
        let mut d = vec![0u8; pw * ph * 4];
        for py in 0..ph {
            let vy = (py as f32 + 0.5) / o as f32 - 0.5;
            let j0 = vy.floor();
            let fy = vy - j0;
            for px in 0..pw {
                let vx = (px as f32 + 0.5) / o as f32 - 0.5;
                let i0 = vx.floor();
                let fx = vx - i0;
                let q = ((j0 as i64 + 1) as usize) * w2 + (i0 as i64 + 1) as usize;
                let fv = f[q] * (1.0 - fx) * (1.0 - fy) + f[q + 1] * fx * (1.0 - fy) + f[q + w2] * (1.0 - fx) * fy + f[q + w2 + 1] * fx * fy;
                if fv < 0.5 {
                    continue;
                }
                let mut c = v(vx.round() as i64, vy.round() as i64);
                if c == 0 {
                    let (a, b) = (i0 as i64, j0 as i64);
                    c = [v(a, b), v(a + 1, b), v(a, b + 1), v(a + 1, b + 1)].into_iter().find(|&x| x != 0).unwrap_or(0);
                }
                if c == 0 {
                    continue;
                }
                let p = (py * pw + px) * 4;
                let (col, al) = if c == 1 { ([255u8, 225, 74], 210u8) } else { (PCOL[(c as usize - 2) % PCOL.len()], 235u8) };
                d[p] = col[0];
                d[p + 1] = col[1];
                d[p + 2] = col[2];
                d[p + 3] = al;
            }
        }
        Some((pw, ph, d))
    }
}

pub fn _unused(_: HashMap<u8, u8>) {}
