//! Whether a bone the pack turns toward the camera (a billboard: a glow disc, a name plate) ends
//! up facing it, checked against a bone that tracks the camera by the game's own convention: for
//! each camera bearing the entity looks at the camera (`target_y_rotation`), and the front of the
//! `--head` bone and of the `--bone` bone are compared. They face the same way when the
//! billboard's maths and the engine's rotation convention agree.
//!
//! `cargo run --release -p horizon-bedrock --example probe_facing -- <pack dir> <entity id> <geometry substring> <billboard bone> <head bone>`
//!
//! `PITCH=n` (camera pitch, default 10: positive is a camera below the entity; `up:` is how far
//! above the horizon each front points), `TICKS=n` (default 60), `YAWS=-135,-90,...`,
//! `Q=name=value,...`. The table prints, per bearing, each front's heading (degrees about the
//! vertical, in the model's frame) and their difference; `v.hcmvys`/`v.rdmkec` follow.

use horizon_bedrock::animation::{Library, Pose};
use horizon_bedrock::entity::{Assets, ClientEntity, Instance, RenderController};
use horizon_bedrock::geometry::Part;
use horizon_bedrock::material::Materials;
use horizon_bedrock::molang::{self, Host, Value};
use horizon_bedrock::pack::Pack;
use std::path::Path;

struct World {
    tick: u32,
    yaw: f32,
    pitch: f32,
}

impl Host for World {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        if let Ok(q) = std::env::var("Q") {
            for kv in q.split(',') {
                if let Some((k, v)) = kv.split_once('=')
                    && k == name
                {
                    return Value::Num(v.parse().unwrap_or(0.0));
                }
            }
        }
        let axis = args.first().map_or(0.0, |a| a.num());
        match name {
            "is_alive" | "is_on_ground" => Value::bool(true),
            "life_time" => Value::Num(self.tick as f32 / 20.0),
            "delta_time" => Value::Num(0.05),
            "distance_from_camera" => Value::Num(3.0),
            "model_scale" => Value::Num(1.0),
            "health" | "max_health" => Value::Num(20.0),
            "time_stamp" => Value::Num(self.tick as f32),
            "graphics_mode_is_any" => Value::Num(1.0),
            "rotation_to_camera" => Value::Num(if axis < 0.5 { self.pitch } else { self.yaw }),
            "target_y_rotation" => Value::Num(self.yaw),
            "target_x_rotation" => Value::Num(self.pitch),
            "has_target" => Value::Num(1.0),
            "camera_rotation" | "body_y_rotation" | "ground_speed" | "modified_move_speed"
            | "vertical_speed" | "is_local_player" | "is_name_any" | "is_in_ui" | "is_attached"
            | "is_on_fire" | "is_in_water" | "is_in_lava" | "head_is_in_water" => Value::Num(0.0),
            _ => Value::Null,
        }
    }
}

type M = [[f32; 4]; 4];

fn ident() -> M {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

fn mul(a: &M, b: &M) -> M {
    let mut o = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                o[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    o
}

fn translate(x: f32, y: f32, z: f32) -> M {
    let mut m = ident();
    m[0][3] = x;
    m[1][3] = y;
    m[2][3] = z;
    m
}

fn scale(x: f32, y: f32, z: f32) -> M {
    let mut m = ident();
    m[0][0] = x;
    m[1][1] = y;
    m[2][2] = z;
    m
}

fn rot_x(a: f32) -> M {
    let (s, c) = a.sin_cos();
    let mut m = ident();
    m[1][1] = c;
    m[1][2] = -s;
    m[2][1] = s;
    m[2][2] = c;
    m
}

fn rot_y(a: f32) -> M {
    let (s, c) = a.sin_cos();
    let mut m = ident();
    m[0][0] = c;
    m[0][2] = s;
    m[2][0] = -s;
    m[2][2] = c;
    m
}

fn rot_z(a: f32) -> M {
    let (s, c) = a.sin_cos();
    let mut m = ident();
    m[0][0] = c;
    m[0][1] = -s;
    m[1][0] = s;
    m[1][1] = c;
    m
}

/// The matrix of every bone, as the game's model draws it (translate, Rz Ry Rx, scale).
fn walk(part: &Part, parent: &M, pose: &Pose, out: &mut Vec<(String, M)>) {
    let mut p = part.pose;
    if let Some(b) = pose.get(&part.name) {
        let r = horizon_bedrock::geometry::rotation_to_java([
            b.rotation[0] + b.rotation_entity[0],
            b.rotation[1] + b.rotation_entity[1],
            b.rotation[2] + b.rotation_entity[2],
        ]);
        p[0] += b.position[0];
        p[1] -= b.position[1];
        p[2] += b.position[2];
        p[3] += r[0];
        p[4] += r[1];
        p[5] += r[2];
        p[6] *= b.scale[0];
        p[7] *= b.scale[1];
        p[8] *= b.scale[2];
    }
    let mut m = mul(parent, &translate(p[0] / 16.0, p[1] / 16.0, p[2] / 16.0));
    m = mul(&m, &mul(&rot_z(p[5]), &mul(&rot_y(p[4]), &rot_x(p[3]))));
    m = mul(&m, &scale(p[6], p[7], p[8]));
    out.push((part.name.clone(), m));
    for c in &part.children {
        walk(c, &m, pose, out);
    }
}

fn heading(m: &M, v: [f32; 3]) -> f32 {
    let x = m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2];
    let z = m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2];
    x.atan2(z).to_degrees()
}

/// How far a front points above the horizon (down is negative; the model's y is down).
fn elevation(m: &M, v: [f32; 3]) -> f32 {
    let x = m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2];
    let y = m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2];
    let z = m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2];
    (-y).atan2(x.hypot(z)).to_degrees()
}

fn wrap(a: f32) -> f32 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pack_path, id, geo, bill, head, ..] = args.as_slice() else {
        eprintln!("usage: probe_facing <pack dir> <entity id> <geometry substring> <billboard bone> <head bone>");
        return;
    };
    let pack = Pack::open(Path::new(pack_path)).expect("pack");
    let mut builtin = horizon_bedrock::bundle::Bundle::default();
    builtin.add_builtins();
    let mut lib: Library = builtin.library;
    let mut rcs = builtin.render_controllers;
    let mut geos = builtin.geometries;
    let mut defs = Vec::new();
    let _ = Materials::default();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if !p.ends_with(".json") {
            continue;
        }
        let Ok(v) = pack.read_json(&p) else { continue };
        if p.starts_with("animations/") || p.starts_with("animation_controllers/") {
            lib.add_file(&v);
        }
        if p.starts_with("render_controllers/") {
            RenderController::add_file(&v, &mut rcs);
        }
        if p.starts_with("models/") {
            for g in horizon_bedrock::geometry::parse_file(&v).unwrap_or_default() {
                geos.insert(g.id.to_ascii_lowercase(), g);
            }
        }
        if p.starts_with("entity/")
            && let Some(d) = ClientEntity::parse(&v)
        {
            defs.push(d);
        }
    }
    let assets = Assets {
        animations: &lib,
        render_controllers: &rcs,
        geometries: Some(&geos),
    };
    let def = defs.iter().find(|d| d.id == *id).expect("entity");
    let env = |k: &str| std::env::var(k).ok();
    let pitch: f32 = env("PITCH").and_then(|v| v.parse().ok()).unwrap_or(10.0);
    let ticks: u32 = env("TICKS").and_then(|v| v.parse().ok()).unwrap_or(60);
    let yaws: Vec<f32> = env("YAWS")
        .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![-135.0, -90.0, -45.0, -20.0, 0.0, 20.0, 45.0, 90.0, 135.0, 180.0]);
    println!("bearing | head front | billboard front | difference   (hcmvys, rdmkec)");
    for yaw in yaws {
        let mut host = World {
            tick: 0,
            yaw,
            pitch,
        };
        let mut inst = Instance::new(1);
        let mut frame = None;
        for t in 0..ticks {
            host.tick = t;
            frame = Some(inst.update(def, &assets, &mut host, 0.05));
        }
        let frame = frame.expect("frame");
        let Some(layer) = frame
            .layers
            .iter()
            .find(|l| l.geometry.to_ascii_lowercase().contains(geo.as_str()))
        else {
            println!("no layer for {geo}");
            return;
        };
        let g = geos
            .get(&layer.geometry.to_ascii_lowercase())
            .expect("geometry");
        let mut mats = Vec::new();
        for root in g.to_tree() {
            walk(&root, &ident(), &frame.pose, &mut mats);
        }
        // The front of a cube is its north face: the model's -z, as the game's cubes carry it.
        let front = [0.0, 0.0, -1.0];
        let of = |name: &str| mats.iter().find(|(n, _)| n == name).map(|(_, m)| heading(m, front));
        let var = |n: &str| inst.vars.get(&molang::intern(n)).map(|v| v.num());
        let up = |name: &str| mats.iter().find(|(n, _)| n == name).map(|(_, m)| elevation(m, front));
        match (of(head), of(bill)) {
            (Some(h), Some(b)) => println!(
                "{yaw:7.1} | {h:10.1} | {b:15.1} | {:10.1}   (hcmvys {:?}, rdmkec {:?}) up: head {:.1} billboard {:.1}",
                wrap(b - h),
                var("hcmvys"),
                var("rdmkec"),
                up(head).unwrap_or(0.0),
                up(bill).unwrap_or(0.0),
            ),
            _ => {
                println!("bones not found: {head:?} {bill:?}");
                return;
            }
        }
    }
}
