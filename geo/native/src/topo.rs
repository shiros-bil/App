// TopoJSON decoding (quantized, delta-encoded arcs) into map coordinates (MW x MH equirectangular).
use serde::Deserialize;
use serde_json::Value;

pub const MW: f32 = 2000.0;
pub const MH: f32 = 1000.0;

#[derive(Deserialize)]
struct Transform {
    scale: [f64; 2],
    translate: [f64; 2],
}

#[derive(Deserialize)]
struct RawTopo {
    transform: Option<Transform>,
    arcs: Vec<Vec<[f64; 2]>>,
    objects: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug)]
pub enum Shape {
    // polygons: list of polygons, each = list of rings, each ring = list of arc refs (negative = reversed, ~a)
    Polys(Vec<Vec<Vec<i32>>>),
    // lines: list of line strings, each = list of arc refs
    Lines(Vec<Vec<i32>>),
    None,
}

#[derive(Clone, Debug)]
pub struct Geom {
    pub shape: Shape,
    pub props: Value,
}

pub struct Topo {
    pub arcs: Vec<Vec<[f32; 2]>>,
    pub objects: std::collections::HashMap<String, Vec<Geom>>,
}

fn parse_ring(v: &Value) -> Vec<i32> {
    v.as_array().map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0) as i32).collect()).unwrap_or_default()
}

pub fn parse(bytes: &[u8]) -> Topo {
    let raw: RawTopo = serde_json::from_slice(bytes).expect("topojson");
    let (sx, sy, tx, ty, q) = match &raw.transform {
        Some(t) => (t.scale[0], t.scale[1], t.translate[0], t.translate[1], true),
        None => (1.0, 1.0, 0.0, 0.0, false),
    };
    let arcs: Vec<Vec<[f32; 2]>> = raw
        .arcs
        .iter()
        .map(|a| {
            let mut x = 0.0f64;
            let mut y = 0.0f64;
            a.iter()
                .map(|p| {
                    if q {
                        x += p[0];
                        y += p[1];
                    } else {
                        x = p[0];
                        y = p[1];
                    }
                    let lon = x * sx + tx;
                    let lat = y * sy + ty;
                    [((lon + 180.0) * MW as f64 / 360.0) as f32, ((90.0 - lat) * MH as f64 / 180.0) as f32]
                })
                .collect()
        })
        .collect();
    let mut objects = std::collections::HashMap::new();
    for (name, obj) in raw.objects.iter() {
        let mut out = Vec::new();
        if let Some(gs) = obj.get("geometries").and_then(|g| g.as_array()) {
            for g in gs {
                let t = g.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let arcs = g.get("arcs");
                let shape = match (t, arcs) {
                    ("Polygon", Some(a)) => Shape::Polys(vec![a.as_array().unwrap().iter().map(parse_ring).collect()]),
                    ("MultiPolygon", Some(a)) => Shape::Polys(
                        a.as_array().unwrap().iter().map(|p| p.as_array().unwrap().iter().map(parse_ring).collect()).collect(),
                    ),
                    ("LineString", Some(a)) => Shape::Lines(vec![parse_ring(a)]),
                    ("MultiLineString", Some(a)) => Shape::Lines(a.as_array().unwrap().iter().map(parse_ring).collect()),
                    _ => Shape::None,
                };
                out.push(Geom { shape, props: g.get("properties").cloned().unwrap_or(Value::Null) });
            }
        }
        objects.insert(name.clone(), out);
    }
    Topo { arcs, objects }
}

impl Topo {
    // expand a ring (arc refs) into a point list; shared endpoints are de-duplicated like topojson-client
    pub fn ring(&self, ring: &[i32], out: &mut Vec<[f32; 2]>) {
        out.clear();
        for (n, &ai) in ring.iter().enumerate() {
            let rev = ai < 0;
            let a = if rev { !ai } else { ai } as usize;
            let f = &self.arcs[a];
            let m = f.len();
            for k in 0..m {
                if k == 0 && n > 0 {
                    continue;
                }
                let j = if rev { m - 1 - k } else { k };
                out.push(f[j]);
            }
        }
    }
    pub fn line(&self, line: &[i32], out: &mut Vec<[f32; 2]>) {
        self.ring(line, out)
    }
}

pub fn arc_index(ai: i32) -> usize {
    (if ai < 0 { !ai } else { ai }) as usize
}
