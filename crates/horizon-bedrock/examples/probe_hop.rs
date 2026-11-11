//! What a pack draws for one entity: its layers (geometry, texture, material), whether the
//! geometry and texture exist, how many bones show, the scale and the first bones' pose.
//!
//! `cargo run --release -p horizon-bedrock --example inspect_entity -- <pack> <entity id> [name tag]`
//!
//! `<entity id>` is a client entity identifier (`minecraft:spider`); `all` lists every id.

use horizon_bedrock::animation::Library;
use horizon_bedrock::entity::{Assets, ClientEntity, Instance, RenderController};
use horizon_bedrock::molang::{self, Host};
use horizon_bedrock::pack::Pack;
use std::collections::HashMap;
use std::path::Path;

// A hopping entity: ground 12 ticks, rising 6, falling 6. Prints the root's scale.
// `cargo run --release -p horizon-bedrock --example probe_hop -- <pack> <entity id> [variant]`
use molang::Value;

struct Hop {
    tick: u32,
    variant: f32,
}

impl Hop {
    fn phase(&self) -> u32 {
        self.tick % 24
    }
}

impl Host for Hop {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        let p = self.phase();
        match name {
            "is_alive" => Value::bool(true),
            "death_ticks" => Value::Num(0.0),
            "life_time" => Value::Num(self.tick as f32 / 20.0),
            "delta_time" => Value::Num(0.05),
            "frame_alpha" => Value::Num(0.0),
            "is_on_ground" => Value::bool(p < 12),
            "vertical_speed" => Value::Num(if p < 12 { 0.0 } else if p < 18 { 6.0 } else { -6.0 }),
            "variant" => Value::Num(self.variant),
            "distance_from_camera" => Value::Num(5.0),
            "model_scale" => Value::Num(1.0),
            "health" => Value::Num(20.0),
            "max_health" => Value::Num(20.0),
            "time_stamp" => Value::Num(self.tick as f32),
            "rotation_to_camera" | "camera_rotation" | "body_y_rotation" | "target_y_rotation"
            | "target_x_rotation" | "ground_speed" | "modified_move_speed" | "is_local_player"
            | "is_name_any" | "is_baby" | "is_in_ui" | "is_attached" | "is_on_fire"
            | "is_in_water" | "is_in_lava" | "head_is_in_water" => {
                let _ = args;
                Value::Num(0.0)
            }
            _ => Value::Null,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pack_path, id, ..] = args.as_slice() else {
        eprintln!("usage: probe_hop <pack> <entity id>");
        return;
    };
    let pack = Pack::open(Path::new(pack_path)).expect("pack");
    let mut lib = Library::default();
    let mut rcs = HashMap::new();
    let mut defs = Vec::new();
    let mut geos = HashMap::new();
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
        if p.starts_with("entity/") {
            if let Some(d) = ClientEntity::parse(&v) {
                defs.push(d);
            }
        }
    }
    let assets = Assets {
        animations: &lib,
        render_controllers: &rcs,
        geometries: Some(&geos),
    };
    let def = defs.iter().find(|d| d.id == *id).expect("entity");
    let mut inst = Instance::new(1);
    let variant: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(4.0);
    let mut host = Hop { tick: 0, variant };
    for t in 0..std::env::var("TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(48u32) {
        host.tick = t;
        let f = inst.update(def, &assets, &mut host, 0.05);
        if t % std::env::var("EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(8u32) == 0 {
            println!("t={t:2} {}", inst.describe(def, &assets).chars().take(200).collect::<String>());
            let want = std::env::var("BONES").unwrap_or_default();
            for (n, b) in &f.pose.bones {
                if want.split(',').any(|w| !w.is_empty() && n.contains(w)) {
                    println!("   {n}: pos {:?} rot {:?} scale {:?}", b.position, b.rotation, b.scale);
                }
            }
        }
    }
    let names = std::env::var("VARS").unwrap_or_else(|_| "dxvpbr,habcyx,acloko,fuppgm,rdmkec,jgzabo,qugnsz".into());
    for n in names.split(',').filter(|n| !n.is_empty()) {
        println!("  v.{n} = {:?}", inst.vars.get(&molang::intern(n)));
    }
}
