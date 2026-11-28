// Game state and core rules: economy, diplomacy, conquest, occupation cells/zones (port of the JS game).
use crate::topo::{MH, MW};
use crate::world::{World, GH, GW};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Nat {
    pub money: f64,
    pub army: f64,
    pub stab: f64,
    pub tech: f64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Zone {
    pub r: u32,
    pub o: u16,
    pub k: char, // 'o' occupation, 'a' annexed
    #[serde(default)]
    pub c: u32,
    #[serde(default)]
    pub n: String,
    #[serde(default)]
    pub m: i64,
    #[serde(default)]
    pub g: u32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Div {
    pub id: u32,
    pub n: String,
    pub x: f32,
    pub y: f32,
    pub r: u32,
    pub str_: f32,
    pub path: Vec<[f32; 3]>,
    #[serde(default)]
    pub hold: bool,
    #[serde(default)]
    pub pf: u32,
    #[serde(default)]
    pub tk: Option<u32>,
    #[serde(default)]
    pub ln: Option<usize>,
    #[serde(default)]
    pub cv: Option<[f32; 2]>,
    #[serde(default)]
    pub cv_t: i64,
    #[serde(default)]
    pub role: u8,
    #[serde(default)]
    pub k: f32,
    #[serde(default)]
    pub dist: f32,
    #[serde(default)]
    pub p_tot: f32,
    #[serde(default)]
    pub p_done: f32,
    #[serde(default)]
    pub wait: f32,
    #[serde(default)]
    pub kmh: f32,
    #[serde(skip)]
    pub ox: f32,
    #[serde(skip)]
    pub oy: f32,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Front {
    pub id: u32,
    pub divs: Vec<u32>,
    pub line: Vec<[f32; 2]>,
    pub plan: Option<Vec<[f32; 2]>>,
    pub exec: bool,
    pub t: Option<u16>,
    #[serde(default)]
    pub line0: Option<Vec<[f32; 2]>>,
    // divisions executing the plan (the rest keep holding the front)
    #[serde(default)]
    pub atk: Vec<u32>,
    #[serde(skip)]
    pub v: u64,
    #[serde(skip)]
    pub cv_t: i64,
    #[serde(skip)]
    pub cv_v: u64,
    #[serde(skip)]
    pub cv_s: i64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Game {
    pub player: u16,
    pub day: i64,
    #[serde(default)]
    pub hour: i64,
    pub owner: Vec<u16>,
    pub gdp: Vec<f64>,
    pub pop: Vec<f64>,
    pub nat: Vec<Nat>,
    pub allies: Vec<u16>,
    pub war: Vec<u16>,
    pub rel: HashMap<u16, f64>,
    pub log: Vec<String>,
    pub ai_war: Vec<[u16; 2]>,
    #[serde(default)]
    pub zones: Vec<Zone>,
    #[serde(default)]
    pub rg: HashMap<u32, u32>,
    #[serde(default)]
    pub rn: HashMap<u32, String>,
    #[serde(default)]
    pub gid: u32,
    #[serde(default)]
    pub divs: Vec<Div>,
    #[serde(default)]
    pub div_n: u32,
    #[serde(default)]
    pub fronts: Vec<Front>,
    #[serde(default)]
    pub fr_n: u32,
    #[serde(default)]
    pub ctl: Vec<u32>,
}

pub struct Pocket {
    pub me: u16,
    pub c: Vec<u32>,
    pub i: usize,
    pub t0: f64,
}

pub const MONTHS: [&str; 12] = ["янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"];
pub const SPEEDS: [f64; 5] = [0.0, 1.0, 3.0, 8.0, 20.0];

pub fn fmt(n: f64) -> String {
    let n = n.round();
    let a = n.abs();
    if a >= 1e6 {
        format!("{:.2}трлн", n / 1e6)
    } else if a >= 1e3 {
        format!("{:.1}млрд", n / 1e3)
    } else {
        format!("{}млн", n as i64)
    }
}
pub fn fmt_n(n: f64) -> String {
    if n >= 1e6 {
        format!("{:.2} млн", n / 1e6)
    } else if n >= 1e3 {
        format!("{:.0} тыс", n / 1e3)
    } else {
        format!("{}", n.round() as i64)
    }
}

pub struct Sim {
    pub w: Arc<World>,
    pub g: Game,
    pub rn: Vec<String>,
    pub ter: Vec<Vec<u32>>,
    pub inc: Vec<f64>,
    pub ppn: Vec<f64>,
    pub zi: Vec<f64>,
    pub zp: Vec<f64>,
    pub ctl: Vec<u16>,
    pub zk: HashMap<(u32, u16), u16>,
    pub ctl_ver: u64,
    pub own_ver: u64,
    pub econ_ver: u64,
    pub chr: HashSet<u32>,
    pub dbl: Vec<[f32; 4]>,
    pub osq: Vec<[f32; 4]>,
    pub anq: Vec<Pocket>,
    pub toasts: Vec<String>,
    pub rng: u64,
    pub speed: usize,
    pub h_acc: f64,
    pub divf: f32,
    pub caplog: Option<Vec<u32>>,
    pub route_own: bool,
    pub game_over: Option<bool>,
    pub dirty_ui: bool,
    pub lbl_dirty: bool,
    pub rcn: u64,
    pub now_ms: f64,
    pub hmv: bool,
    pub sm_t: f64,
    pub caps: Vec<CityState>,
    pub cap_key: (u64, u64),
    pub astar: crate::divs::AStar,
    pub sel_div: Option<u32>,
    pub sels: Vec<u32>,
    pub fseg: HashMap<u32, Vec<f32>>,
    pub parea: HashMap<u32, crate::front::Area>,
    pub cover: HashMap<u32, Vec<crate::front::CoverSlot>>,
    pub spears: Vec<(Vec<[f32; 2]>, f64)>,
    pub pc: Option<crate::peace::Peace>,
    pub zone_full: bool,
}

#[derive(Clone, Default)]
pub struct CityState {
    pub k: i64,
    pub on: i32,
    pub ctl: i32,
    pub cap: bool,
}

impl Sim {
    pub fn new(w: Arc<World>) -> Sim {
        let nn = w.nations.len();
        let n = w.nreg();
        Sim {
            rn: w.regions.iter().map(|r| r.name.clone()).collect(),
            w,
            g: Game::default(),
            ter: vec![Vec::new(); nn],
            inc: vec![0.0; nn],
            ppn: vec![0.0; nn],
            zi: vec![0.0; nn],
            zp: vec![0.0; nn],
            ctl: vec![0u16; GW * GH],
            zk: HashMap::new(),
            ctl_ver: 1,
            own_ver: 1,
            econ_ver: 0,
            chr: HashSet::new(),
            dbl: Vec::new(),
            osq: Vec::new(),
            anq: Vec::new(),
            toasts: Vec::new(),
            rng: 0x9E3779B97F4A7C15 ^ (n as u64),
            speed: 0,
            h_acc: 0.0,
            divf: 0.0,
            caplog: None,
            route_own: false,
            game_over: None,
            dirty_ui: true,
            lbl_dirty: true,
            rcn: 0,
            now_ms: 0.0,
            hmv: false,
            sm_t: 0.0,
            caps: Vec::new(),
            cap_key: (u64::MAX, 0),
            astar: Default::default(),
            sel_div: None,
            sels: Vec::new(),
            fseg: HashMap::new(),
            parea: HashMap::new(),
            cover: HashMap::new(),
            spears: Vec::new(),
            pc: None,
            zone_full: false,
        }
    }
    pub fn seed(&mut self, s: u64) {
        self.rng = s | 1;
    }
    pub fn rand(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn toast(&mut self, s: impl Into<String>) {
        self.toasts.push(s.into());
    }
    pub fn nm(&self, n: u16) -> &str {
        &self.w.nations[n as usize].n
    }
    pub fn date_str(&self) -> String {
        // days since 2026-01-01
        let mut d = self.g.day;
        let mut y = 2026i64;
        loop {
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            let yl = if leap { 366 } else { 365 };
            if d < yl {
                break;
            }
            d -= yl;
            y += 1;
        }
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let ml = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut m = 0;
        while d >= ml[m] {
            d -= ml[m];
            m += 1;
        }
        format!("{} {} {}", d + 1, MONTHS[m], y)
    }
    pub fn add_log(&mut self, t: String) {
        let s = format!("{} {}", self.date_str(), t);
        self.g.log.insert(0, s);
        self.g.log.truncate(50);
    }

    // ---------------- new game / start ----------------
    pub fn new_game(&mut self, p: u16) {
        let w = self.w.clone();
        let n = w.nreg();
        let mut g = Game { player: p, owner: w.regions.iter().map(|r| r.nat).collect(), gdp: vec![0.0; n], ..Default::default() };
        let mut ws: HashMap<u16, f64> = HashMap::new();
        for r in 0..n {
            let x = (w.area(r) as f64 + 0.01).sqrt();
            g.gdp[r] = x;
            *ws.entry(w.regions[r].nat).or_insert(0.0) += x;
        }
        for r in 0..n {
            let k = w.regions[r].nat;
            g.gdp[r] = g.gdp[r] / ws[&k] * w.nations[k as usize].gdp.max(30.0);
        }
        g.pop = w.regions.iter().map(|r| r.pop).collect();
        for nd in &w.nations {
            let gdp = nd.gdp.max(30.0);
            let army = nd.army.unwrap_or_else(|| (500f64).max(nd.pop.max(0.0).sqrt() * 25.0 + gdp * 0.004));
            g.nat.push(Nat { money: gdp * 0.05, army, stab: 60.0 + self.rand() * 30.0, tech: 1.0 });
        }
        for (i, nd) in w.nations.iter().enumerate() {
            let par = match nd.par {
                Some(p) => p as u16,
                None => continue,
            };
            let peace = nd.peace.as_ref().map(|v| v.as_bool().unwrap_or(false) || v.as_f64().unwrap_or(0.0) != 0.0).unwrap_or(false);
            if peace {
                continue;
            }
            if i as u16 == p {
                g.war.push(par);
            } else if par == p {
                g.war.push(i as u16);
            } else {
                g.ai_war.push([i as u16, par]);
            }
        }
        let hou = w.nations.iter().position(|x| x.iso == "hou");
        let stc = w.nations.iter().position(|x| x.iso == "stc");
        if let (Some(h), Some(s)) = (hou, stc) {
            if p as usize == h {
                g.war.push(s as u16);
            }
        }
        self.g = g;
        let nmp = self.nm(p).to_string();
        self.add_log(format!("Вы возглавили: {}.", nmp));
        if !self.g.war.is_empty() {
            let l: Vec<String> = self.g.war.iter().map(|&x| self.nm(x).to_string()).collect();
            self.add_log(format!("Гражданская война! Противники: {}.", l.join(", ")));
        }
        self.start_game();
    }

    pub fn start_game(&mut self) {
        self.own_ver += 1;
        self.rn = self.w.regions.iter().map(|r| r.name.clone()).collect();
        self.occ_init_game();
        self.recalc();
        if self.g.hour == 0 {
            self.g.hour = self.g.day * 24;
        }
        self.speed = 0;
        self.game_over = None;
        self.dirty_ui = true;
    }

    pub fn recalc(&mut self) {
        self.rcn += 1;
        let nn = self.w.nations.len();
        for t in self.ter.iter_mut() {
            t.clear();
        }
        self.inc = vec![0.0; nn];
        self.ppn = vec![0.0; nn];
        for r in 0..self.w.nreg() {
            let o = self.g.owner[r] as usize;
            self.ter[o].push(r as u32);
            self.inc[o] += self.g.gdp[r];
            self.ppn[o] += self.g.pop[r];
        }
        self.zone_econ();
        self.lbl_dirty = true;
        self.own_ver += 1;
    }
    pub fn terr(&self, o: u16) -> &Vec<u32> {
        &self.ter[o as usize]
    }
    pub fn income(&self, o: u16) -> f64 {
        self.inc[o as usize] + self.zi[o as usize]
    }
    pub fn pop_of(&self, o: u16) -> f64 {
        self.ppn[o as usize] + self.zp[o as usize]
    }
    pub fn power(&self, o: u16) -> f64 {
        let n = &self.g.nat[o as usize];
        n.army * n.tech
    }
    pub fn nations(&self) -> Vec<u16> {
        (0..self.ter.len()).filter(|&o| !self.ter[o].is_empty()).map(|o| o as u16).collect()
    }
    pub fn daily_inc(&self, o: u16) -> f64 {
        let n = &self.g.nat[o as usize];
        self.income(o) * 0.0009 * (n.stab / 100.0 + 0.3) - n.army * 0.000015
    }
    pub fn at_war(&self, a: u16, b: u16) -> bool {
        let p = self.g.player;
        if a == p {
            return self.g.war.contains(&b);
        }
        if b == p {
            return self.g.war.contains(&a);
        }
        self.g.ai_war.iter().any(|w| (w[0] == a && w[1] == b) || (w[0] == b && w[1] == a))
    }
    pub fn can_enter(&self, r: u32) -> bool {
        let o = self.g.owner[r as usize];
        let me = self.g.player;
        o == me || self.g.allies.contains(&o) || self.at_war(me, o)
    }

    pub fn rec_cost(&self) -> f64 {
        self.income(self.g.player) * 0.03 + 20.0
    }
    pub fn inv_cost(&self, i: usize) -> f64 {
        self.g.gdp[i] * 0.08 + 5.0
    }
    pub fn tech_cost(&self) -> f64 {
        self.income(self.g.player) * 0.06 * self.g.nat[self.g.player as usize].tech + 30.0
    }
    pub fn spy_cost(&self) -> f64 {
        self.income(self.g.player) * 0.05 + 40.0
    }
    pub fn div_cost(&self) -> f64 {
        self.income(self.g.player) * 0.004 + 8.0
    }

    pub fn conquer(&mut self, a: u16, t: u32) {
        self.occ_clear_region(t);
        let o = self.g.owner[t as usize];
        self.g.owner[t as usize] = a;
        self.recalc();
        if a == self.g.player || o == self.g.player {
            let s = format!("{} занимает: {} ({}).", self.nm(a), self.rn[t as usize], self.nm(o));
            self.add_log(s);
        }
        if self.terr(o).is_empty() {
            let m = self.g.nat[o as usize].money * 0.3;
            self.g.nat[a as usize].money += m;
            self.g.war.retain(|&w| w != o);
            self.g.allies.retain(|&w| w != o);
            self.g.ai_war.retain(|w| w[0] != o && w[1] != o);
            if o == self.g.player {
                self.game_over = Some(false);
            }
        }
        self.dirty_ui = true;
    }

    // ---------------- daily economy ----------------
    pub fn day_tick(&mut self) {
        self.g.day += 1;
        if self.econ_ver != self.ctl_ver {
            self.zone_econ();
        }
        let me = self.g.player;
        let ns = self.nations();
        for &o in &ns {
            let di = self.daily_inc(o);
            let n = &mut self.g.nat[o as usize];
            n.money += di;
            n.stab += (65.0 - n.stab) * 0.004;
            if n.money < 0.0 {
                n.stab -= 0.15;
                n.army *= 0.998;
            }
            if o != me {
                n.army *= 1.0004;
            }
        }
        if self.g.day % 30 == 0 {
            for r in 0..self.w.nreg() {
                let gr = self.w.nations[self.w.regions[r].nat as usize].g;
                self.g.pop[r] *= (1.0 + gr).powf(30.0 / 365.0);
            }
            self.recalc();
        }
        for _ in 0..6 {
            if ns.is_empty() {
                break;
            }
            let i = (self.rand() * ns.len() as f64) as usize;
            self.ai_think(ns[i.min(ns.len() - 1)]);
        }
        if self.rand() < 0.004 {
            self.random_event();
        }
        if ns.len() == 1 {
            self.game_over = Some(true);
        }
        self.dirty_ui = true;
    }
    fn ai_think(&mut self, o: u16) {
        if o == self.g.player || self.ter[o as usize].is_empty() {
            return;
        }
        let inc = self.income(o);
        let n = &mut self.g.nat[o as usize];
        if n.money > inc * 0.06 {
            n.army *= 1.06;
            n.money -= inc * 0.04;
        }
    }
    fn random_event(&mut self) {
        let me = self.g.player;
        let k = (self.rand() * 5.0) as usize;
        let inc = self.income(me);
        let m = &mut self.g.nat[me as usize];
        let s = match k {
            0 => {
                m.money += inc * 0.02;
                "Найдено месторождение нефти!"
            }
            1 => {
                m.stab -= 7.0;
                "Массовые протесты в столице."
            }
            2 => {
                let t = self.ter[me as usize].clone();
                for r in t {
                    self.g.gdp[r as usize] *= 0.97;
                }
                self.recalc();
                "Мировой кризис ударил по экономике."
            }
            3 => {
                m.tech += 0.1;
                "Учёные совершили открытие."
            }
            _ => {
                m.army *= 0.96;
                "Стихийное бедствие."
            }
        };
        self.add_log(s.to_string());
    }

    // ---------------- player actions ----------------
    pub fn act(&mut self, a: &str, x: i64) {
        let me = self.g.player;
        match a {
            "rec" => {
                let c = self.rec_cost();
                let m = &mut self.g.nat[me as usize];
                m.money -= c;
                let add = (2000f64).max(m.army * 0.12);
                m.army += add;
                m.stab -= 1.5;
                self.add_log(format!("Мобилизовано {} солдат.", fmt_n(add)));
            }
            "inv" => {
                let c = self.inv_cost(x as usize);
                self.g.nat[me as usize].money -= c;
                self.g.gdp[x as usize] *= 1.08;
                self.recalc();
                let s = format!("Инвестиции: {}, ВВП +8%.", self.rn[x as usize]);
                self.add_log(s);
            }
            "tech" => {
                let c = self.tech_cost();
                let m = &mut self.g.nat[me as usize];
                m.money -= c;
                m.tech += 0.2;
                let s = format!("Технологии ур. {:.1}.", m.tech);
                self.add_log(s);
            }
            "war" => {
                let x = x as u16;
                self.g.war.push(x);
                self.g.rel.insert(x, -100.0);
                self.g.nat[me as usize].stab -= 4.0;
                let s = format!("Вы объявили войну: {}!", self.nm(x));
                self.add_log(s);
                self.own_ver += 1;
            }
            "ally" => {
                let x = x as u16;
                let r = self.g.rel.get(&x).copied().unwrap_or(0.0) + self.rand() * 60.0 - 10.0 + if self.power(me) > self.power(x) { 20.0 } else { 0.0 };
                if r > 45.0 {
                    self.g.allies.push(x);
                    self.g.rel.insert(x, 80.0);
                    let s = format!("{} — ваш союзник!", self.nm(x));
                    self.add_log(s.clone());
                    self.toast(s);
                } else {
                    *self.g.rel.entry(x).or_insert(0.0) += 5.0;
                    let s = format!("{} отклоняет союз.", self.nm(x));
                    self.add_log(s.clone());
                    self.toast(s);
                }
            }
            "trade" => {
                let x = x as u16;
                let gg = self.income(x).min(self.income(me)) * 0.03;
                self.g.nat[me as usize].money += gg;
                self.g.nat[x as usize].money += gg * 0.5;
                *self.g.rel.entry(x).or_insert(0.0) += 10.0;
                let s = format!("Сделка с {}: +${}.", self.nm(x), fmt(gg));
                self.add_log(s.clone());
                self.toast(s);
            }
            "sanc" => {
                let x = x as u16;
                let n = &mut self.g.nat[x as usize];
                n.money *= 0.85;
                n.stab -= 5.0;
                *self.g.rel.entry(x).or_insert(0.0) -= 30.0;
                self.g.allies.retain(|&y| y != x);
                let inc = self.income(me);
                self.g.nat[me as usize].money -= inc * 0.01;
                let s = format!("Санкции против {}.", self.nm(x));
                self.add_log(s.clone());
                self.toast(s);
            }
            "spy" => {
                let x = x as u16;
                let c = self.spy_cost();
                self.g.nat[me as usize].money -= c;
                let st = self.g.nat[x as usize].stab;
                if self.rand() < (100.0 - st) / 130.0 {
                    let n = &mut self.g.nat[x as usize];
                    n.stab -= 25.0;
                    n.army *= 0.7;
                    let s = format!("Переворот в {} удался!", self.nm(x));
                    self.add_log(s.clone());
                    self.toast(s);
                } else {
                    self.g.rel.insert(x, -80.0);
                    let s = format!("Агенты раскрыты: {}!", self.nm(x));
                    self.add_log(s.clone());
                    self.toast(s);
                }
            }
            _ => {}
        }
        self.dirty_ui = true;
    }

    pub fn rel_str(&self, o: u16) -> &'static str {
        let r = self.g.rel.get(&o).copied().unwrap_or(0.0);
        if self.g.allies.contains(&o) {
            "Союзник"
        } else if self.g.war.contains(&o) {
            "Война"
        } else if r > 30.0 {
            "Дружба"
        } else if r < -30.0 {
            "Враждебность"
        } else {
            "Нейтралитет"
        }
    }

    // ================= OCCUPATION =================
    #[inline]
    pub fn reg(&self, i: usize) -> u16 {
        self.w.reg[i]
    }
    #[inline]
    pub fn ctrl_of(&self, i: usize) -> u16 {
        let z = self.ctl[i];
        if z != 0 {
            self.g.zones[z as usize - 1].o
        } else {
            self.g.owner[self.w.reg[i] as usize - 1]
        }
    }
    pub fn set_cell(&mut self, i: usize, z: u16) {
        let o = self.ctl[i];
        if o == z {
            return;
        }
        if o != 0 {
            self.g.zones[o as usize - 1].c -= 1;
        }
        if z != 0 {
            self.g.zones[z as usize - 1].c += 1;
        }
        self.ctl[i] = z;
        self.ctl_ver += 1;
        let rg = self.w.reg[i];
        if rg != 0 {
            self.chr.insert(rg as u32 - 1);
        }
    }
    pub fn zone_for(&mut self, r: u32, o: u16) -> u16 {
        if let Some(&id) = self.zk.get(&(r, o)) {
            return id;
        }
        self.g.zones.push(Zone { r, o, k: 'o', c: 0, n: String::new(), m: 0, g: 0 });
        let id = self.g.zones.len() as u16;
        self.zk.insert((r, o), id);
        id
    }
    pub fn cap_cell(&mut self, me: u16, i: usize) -> bool {
        if let Some(l) = self.caplog.as_mut() {
            l.push(i as u32);
        }
        let rg = self.w.reg[i];
        if rg == 0 {
            return false;
        }
        let r = rg as u32 - 1;
        let own = self.g.owner[r as usize];
        let c = self.ctrl_of(i);
        if c == me {
            return false;
        }
        if own == me {
            self.set_cell(i, 0);
            return true;
        }
        if !self.at_war(me, c) {
            return false;
        }
        let z = self.zone_for(r, me);
        self.set_cell(i, z);
        true
    }
    pub fn occ_paint(&mut self, me: u16, x: f32, y: f32, ry: f32) {
        let x = ((x % MW) + MW) % MW;
        let cx = x * GW as f32 / MW;
        let cy = y * GH as f32 / MH;
        let mut found = false;
        for b in self.dbl.iter_mut() {
            if cx > b[0] - 24.0 && cx < b[2] + 24.0 && cy > b[1] - 24.0 && cy < b[3] + 24.0 && b[2] - b[0] < 120.0 && b[3] - b[1] < 120.0 {
                b[0] = b[0].min(cx);
                b[1] = b[1].min(cy);
                b[2] = b[2].max(cx);
                b[3] = b[3].max(cy);
                found = true;
                break;
            }
        }
        if !found {
            self.dbl.push([cx, cy, cx, cy]);
        }
        let rx = ry / ((90.0 - y / MH * 180.0) * std::f32::consts::PI / 180.0).cos().max(0.2);
        let j0 = ((cy - ry).floor() as i32).max(0);
        let j1 = ((cy + ry).ceil() as i32).min(GH as i32 - 1);
        for j in j0..=j1 {
            let dy = (j as f32 + 0.5 - cy) / ry;
            if dy * dy > 1.0 {
                continue;
            }
            let hw = rx * (1.0 - dy * dy).sqrt();
            for ii in (cx - hw).floor() as i32..=(cx + hw).ceil() as i32 {
                let i = j as usize * GW + (ii.rem_euclid(GW as i32)) as usize;
                let rg = self.w.reg[i];
                if rg == 0 {
                    continue;
                }
                let r = rg as u32 - 1;
                let own = self.g.owner[r as usize];
                let c = self.ctrl_of(i);
                if c == me {
                    continue;
                }
                if own == me {
                    self.set_cell(i, 0);
                    continue;
                }
                if !self.at_war(me, c) {
                    continue;
                }
                let z = self.zone_for(r, me);
                self.set_cell(i, z);
            }
        }
    }
    pub fn occ_trail(&mut self, me: u16, ax: f32, ay: f32, bx: f32, by: f32, di: Option<usize>) {
        let l = ((bx - ax).hypot(by - ay)) * GW as f32 / MW;
        let n = ((l / 0.6).ceil() as i32).max(1);
        let (s0, sd) = match di {
            Some(i) => (self.g.divs[i].dist, self.g.divs[i].id as f32 * 7.13),
            None => (0.0, 0.0),
        };
        for k in 1..=n {
            let t = (s0 + l * k as f32 / n as f32) / 5.0;
            let ry = 1.25 + 0.55 * (t * 0.83 + sd).sin() + 0.35 * (t * 2.1 + sd * 1.7).sin() + 0.2 * (t * 4.7 + sd * 0.3).sin();
            let f = k as f32 / n as f32;
            self.occ_paint(me, ax + (bx - ax) * f, ay + (by - ay) * f, ry.max(0.9));
        }
        if let Some(i) = di {
            self.g.divs[i].dist = s0 + l;
        }
    }
    pub fn cell_box(&self, r: usize) -> [usize; 4] {
        let b = self.w.bb(r);
        let x0 = ((b[0] * GW as f32 / MW).floor() as i32 - 1).max(0) as usize;
        let x1 = ((b[2] * GW as f32 / MW).ceil() as i32 + 1).min(GW as i32 - 1) as usize;
        let y0 = ((b[1] * GH as f32 / MH).floor() as i32 - 1).max(0) as usize;
        let y1 = ((b[3] * GH as f32 / MH).ceil() as i32 + 1).min(GH as i32 - 1) as usize;
        [x0, y0, x1, y1]
    }
    pub fn occ_clear_region(&mut self, r: u32) {
        let [x0, y0, x1, y1] = self.cell_box(r as usize);
        for j in y0..=y1 {
            for i in j * GW + x0..=j * GW + x1 {
                if self.ctl[i] != 0 && self.w.reg[i] as u32 == r + 1 {
                    self.set_cell(i, 0);
                }
            }
        }
    }
    pub fn occ_smooth(&mut self, me: u16) {
        let l = std::mem::take(&mut self.dbl);
        for b in l {
            self.occ_smooth_box(me, b);
        }
    }
    fn cand(&self, me: u16, i: usize) -> bool {
        let rg = self.w.reg[i];
        if rg == 0 {
            return false;
        }
        let c = self.ctrl_of(i);
        c != me && (self.g.owner[rg as usize - 1] == me || self.at_war(me, c))
    }
    fn mine(&self, me: u16, i: usize) -> bool {
        self.w.reg[i] != 0 && self.ctrl_of(i) == me
    }
    pub fn occ_smooth_box(&mut self, me: u16, db: [f32; 4]) {
        let x0 = ((db[0].floor() as i32) - 14).max(1) as usize;
        let y0 = ((db[1].floor() as i32) - 14).max(1) as usize;
        let x1 = ((db[2].ceil() as i32) + 14).min(GW as i32 - 2) as usize;
        let y1 = ((db[3].ceil() as i32) + 14).min(GH as i32 - 2) as usize;
        if x1 < x0 || y1 < y0 {
            return;
        }
        let g = GW as isize;
        let offs8 = [-g - 1, -g, -g + 1, -1, 1, g - 1, g, g + 1];
        for _pass in 0..2 {
            let mut fl = Vec::new();
            for j in y0..=y1 {
                for i in x0..=x1 {
                    let k = j * GW + i;
                    if !self.cand(me, k) {
                        continue;
                    }
                    let (mut m, mut z) = (0, 0);
                    for &o in &offs8 {
                        let q = (k as isize + o) as usize;
                        if self.mine(me, q) {
                            m += 1;
                            if self.ctl[q] != 0 {
                                z += 1;
                            }
                        }
                    }
                    if m >= 5 && z > 0 {
                        fl.push(k);
                    }
                }
            }
            if fl.is_empty() {
                break;
            }
            for k in fl {
                let r = self.w.reg[k] as u32 - 1;
                if self.g.owner[r as usize] == me {
                    self.set_cell(k, 0);
                } else {
                    let z = self.zone_for(r, me);
                    self.set_cell(k, z);
                }
            }
        }
        let w = x1 - x0 + 1;
        let h = y1 - y0 + 1;
        let mut vis = vec![0u8; w * h];
        let mut q: Vec<usize> = Vec::new();
        for j in y0..=y1 {
            for i in x0..=x1 {
                let li = (j - y0) * w + (i - x0);
                let k = j * GW + i;
                if vis[li] != 0 || !self.cand(me, k) {
                    continue;
                }
                let (mut ok, mut zn, mut on) = (true, 0, 0);
                q.clear();
                q.push(k);
                vis[li] = 1;
                let mut h2 = 0;
                while h2 < q.len() {
                    let c = q[h2];
                    h2 += 1;
                    let (ci, cj) = (c % GW, c / GW);
                    if ci <= x0 || ci >= x1 || cj <= y0 || cj >= y1 {
                        ok = false;
                    }
                    if q.len() > 1500 {
                        ok = false;
                        break;
                    }
                    for o in [-g, g, -1, 1] {
                        let n2 = (c as isize + o) as usize;
                        let (ni, nj) = (n2 % GW, n2 / GW);
                        if ni < x0 || ni > x1 || nj < y0 || nj > y1 {
                            continue;
                        }
                        let nl = (nj - y0) * w + (ni - x0);
                        if vis[nl] != 0 {
                            continue;
                        }
                        if self.cand(me, n2) {
                            vis[nl] = 1;
                            q.push(n2);
                        } else if self.w.reg[n2] != 0 && !self.mine(me, n2) {
                            ok = false;
                        } else if self.w.reg[n2] != 0 {
                            if self.ctl[n2] != 0 {
                                zn += 1;
                            } else {
                                on += 1;
                            }
                        }
                    }
                }
                if ok && zn >= 3 && zn >= on {
                    let cells: Vec<u32> = q.iter().map(|&c| c as u32).collect();
                    self.queue_pocket(me, cells);
                }
            }
        }
    }
    pub fn queue_pocket(&mut self, me: u16, cells: Vec<u32>) {
        let mut set: HashMap<u32, i32> = HashMap::with_capacity(cells.len());
        for &c in &cells {
            set.insert(c, -1);
        }
        let mut order = Vec::with_capacity(cells.len());
        let mut fr: Vec<u32> = Vec::new();
        let g = GW as i64;
        for &c in &cells {
            for o in [-1i64, 1, -g, g] {
                let n = (c as i64 + o) as u32;
                if !set.contains_key(&n) && self.mine(me, n as usize) {
                    set.insert(c, 0);
                    fr.push(c);
                    break;
                }
            }
        }
        let mut h = 0;
        while h < fr.len() {
            let c = fr[h];
            h += 1;
            let d = set[&c];
            order.push(c);
            for o in [-1i64, 1, -g, g] {
                let n = (c as i64 + o) as u32;
                if set.get(&n) == Some(&-1) {
                    set.insert(n, d + 1);
                    fr.push(n);
                }
            }
        }
        for &c in &cells {
            if set[&c] == -1 {
                order.push(c);
            }
        }
        self.anq.push(Pocket { me, c: order, i: 0, t0: self.now_ms });
    }
    pub fn step_pockets(&mut self) {
        let now = self.now_ms;
        let mut anq = std::mem::take(&mut self.anq);
        for a in anq.iter_mut() {
            let n = a.c.len();
            let e = n.min(((n as f64) * ((now - a.t0) / 1200.0).min(1.0)).ceil() as usize);
            while a.i < e {
                let k = a.c[a.i] as usize;
                a.i += 1;
                let rg = self.w.reg[k];
                if rg == 0 {
                    continue;
                }
                let r = rg as u32 - 1;
                let c = self.ctrl_of(k);
                if c == a.me {
                    continue;
                }
                if self.g.owner[r as usize] == a.me {
                    self.set_cell(k, 0);
                } else if self.at_war(a.me, c) {
                    let z = self.zone_for(r, a.me);
                    self.set_cell(k, z);
                }
            }
        }
        anq.retain(|a| a.i < a.c.len());
        anq.extend(std::mem::take(&mut self.anq));
        self.anq = anq;
    }
    pub fn zone_econ(&mut self) {
        let nn = self.w.nations.len();
        self.zi = vec![0.0; nn];
        self.zp = vec![0.0; nn];
        for z in &self.g.zones {
            let rc = self.w.rc[z.r as usize];
            if z.c == 0 || rc == 0 {
                continue;
            }
            let own = self.g.owner[z.r as usize];
            if own == z.o {
                continue;
            }
            let sh = (z.c as f64 / rc as f64).min(1.0);
            let gg = self.g.gdp[z.r as usize] * sh;
            let p = self.g.pop[z.r as usize] * sh;
            self.zi[own as usize] -= gg;
            self.zi[z.o as usize] += gg;
            self.zp[own as usize] -= p;
            self.zp[z.o as usize] += p;
        }
        self.econ_ver = self.ctl_ver;
    }
    pub fn region_zoned(&self, r: u32) -> bool {
        self.g.zones.iter().any(|z| z.r == r && z.c > 0)
    }
    pub fn zone_sum(&self, r: u32) -> u32 {
        self.g.zones.iter().filter(|z| z.r == r).map(|z| z.c).sum()
    }
    pub fn occ_init_game(&mut self) {
        self.ctl.iter_mut().for_each(|c| *c = 0);
        self.zk.clear();
        for z in self.g.zones.iter_mut() {
            z.c = 0;
        }
        for (i, z) in self.g.zones.iter().enumerate() {
            if z.k == 'o' {
                self.zk.insert((z.r, z.o), i as u16 + 1);
            }
        }
        let a = std::mem::take(&mut self.g.ctl);
        let mut k = 0;
        while k + 2 < a.len() {
            let (s, n, v) = (a[k] as usize, a[k + 1] as usize, a[k + 2] as u16);
            k += 3;
            if v == 0 || v as usize > self.g.zones.len() {
                continue;
            }
            for i in s..(s + n).min(self.ctl.len()) {
                self.ctl[i] = v;
                self.g.zones[v as usize - 1].c += 1;
            }
        }
        for (k, v) in self.g.rn.clone() {
            if (k as usize) < self.rn.len() {
                self.rn[k as usize] = v;
            }
        }
        self.ctl_ver += 1;
        self.zone_econ();
        self.chr.clear();
    }
    pub fn occ_rle(&self) -> Vec<u32> {
        let mut o = Vec::new();
        let mut i = 0;
        let n = self.ctl.len();
        while i < n {
            let v = self.ctl[i];
            if v == 0 {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < n && self.ctl[j] == v {
                j += 1;
            }
            o.extend_from_slice(&[i as u32, (j - i) as u32, v as u32]);
            i = j;
        }
        o
    }
    pub fn region_at(&self, x: f32, y: f32) -> i64 {
        let x = ((x % MW) + MW) % MW;
        let j = ((y * GH as f32 / MH) as i32).clamp(0, GH as i32 - 1) as usize;
        let i = ((x * GW as f32 / MW) as i32).min(GW as i32 - 1).max(0) as usize;
        let v = self.w.reg[j * GW + i];
        if v != 0 {
            v as i64 - 1
        } else {
            -1
        }
    }
    pub fn cell_at(&self, x: f32, y: f32) -> usize {
        let x = ((x % MW) + MW) % MW;
        let j = ((y * GH as f32 / MH) as i32).clamp(0, GH as i32 - 1) as usize;
        let i = ((x * GW as f32 / MW) as i32).clamp(0, GW as i32 - 1) as usize;
        j * GW + i
    }

    pub fn save_json(&mut self) -> String {
        self.g.ctl = self.occ_rle();
        let s = serde_json::to_string(&self.g).unwrap_or_default();
        self.g.ctl.clear();
        s
    }
    pub fn load_json(&mut self, s: &str) -> bool {
        match serde_json::from_str::<Game>(s) {
            Ok(g) => {
                if g.owner.len() != self.w.nreg() {
                    return false;
                }
                self.g = g;
                self.start_game();
                true
            }
            Err(_) => false,
        }
    }

    // dynamic capitals (star follows the nation's remaining cities)
    pub fn cap_update(&mut self) {
        let key = (self.ctl_ver, self.rcn);
        if key == self.cap_key {
            return;
        }
        self.cap_key = key;
        let n = self.w.cities.len();
        if self.caps.len() != n {
            self.caps = vec![CityState { k: -1, on: i32::MIN, ctl: -1, cap: false }; n];
        }
        for (i, c) in self.w.cities.iter().enumerate() {
            let k = self.cell_at(c.x, c.y);
            let st = &mut self.caps[i];
            st.k = if self.w.reg[k] != 0 { k as i64 } else { -1 };
            if st.on == i32::MIN {
                st.on = if st.k >= 0 { self.w.regions[self.w.reg[k] as usize - 1].nat as i32 } else { -1 };
            }
            st.cap = false;
        }
        for i in 0..n {
            let k = self.caps[i].k;
            self.caps[i].ctl = if k >= 0 { self.ctrl_of(k as usize) as i32 } else { -1 };
        }
        let mut done = HashSet::new();
        for (i, c) in self.w.cities.iter().enumerate() {
            let s = &self.caps[i];
            if c.cap && s.ctl == s.on && s.on >= 0 && !done.contains(&s.on) {
                done.insert(s.on);
                self.caps[i].cap = true;
            }
        }
        for i in 0..n {
            let s = self.caps[i].clone();
            if s.ctl >= 0 && !done.contains(&s.ctl) && s.on == s.ctl && !self.ter[s.ctl as usize].is_empty() {
                done.insert(s.ctl);
                self.caps[i].cap = true;
            }
        }
    }
}
