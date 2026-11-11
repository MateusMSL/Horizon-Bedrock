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

// A dying entity, step by step: the largest bone offset each tick.
// `cargo run --release -p horizon-bedrock --example probe_death -- <pack> <entity id>`
use molang::Value;

struct Dying {
    tick: f32,
    alive_for: f32,
    y: f32,
}

impl Host for Dying {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        let alive = self.tick < self.alive_for;
        let death = (self.tick - self.alive_for).max(0.0);
        match name {
            "is_alive" => Value::bool(alive),
            "death_ticks" => Value::Num(death),
            "life_time" => Value::Num(self.tick / 20.0),
            "delta_time" => Value::Num(0.05),
            "frame_alpha" => Value::Num(0.0),
            "is_on_ground" => Value::bool(true),
            "position" => Value::Num(match args.first().map_or(0.0, |v| v.num()) as i32 {
                1 => self.y,
                0 => 100.5,
                _ => -20.5,
            }),
            // Ground below the feet only.
            "relative_block_has_any_tag" => Value::bool(args.get(1).map_or(0.0, |v| v.num()) < 0.0),
            "distance_from_camera" => Value::Num(5.0),
            "camera_rotation" | "rotation_to_camera" => Value::Num(
                std::env::var("CAM")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0),
            ),
            "is_local_player" => Value::Num(0.0),
            "graphics_mode_is_any" => Value::Num(1.0),
            "vertical_speed" => Value::Num(
                std::env::var("VS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0),
            ),
            "ground_speed" => Value::Num(
                std::env::var("GS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0),
            ),
            "modified_move_speed" => Value::Num(0.0),
            "health" => Value::Num(if alive { 20.0 } else { 0.0 }),
            "max_health" => Value::Num(20.0),
            "time_stamp" => Value::Num(self.tick),
            "model_scale" => Value::Num(1.0),
            "target_y_rotation" if std::env::var_os("AIM").is_some() => Value::Num(25.0),
            "target_x_rotation" if std::env::var_os("AIM").is_some() => Value::Num(-10.0),
            "facing_target_to_range_attack" | "has_target" | "is_item_name_any"
                if std::env::var_os("AIM").is_some() =>
            {
                Value::Num(1.0)
            }
            "body_y_rotation" | "target_y_rotation" | "target_x_rotation" => Value::Num(0.0),
            "is_name_any" | "is_baby" | "is_in_ui" | "is_attached" | "is_on_fire"
            | "is_in_water" | "is_in_lava" | "head_is_in_water" => Value::Num(0.0),
            _ => Value::Null,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pack_path, id, ..] = args.as_slice() else {
        return;
    };
    let ticks: u32 = std::env::var("TICKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
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
    let mut host = Dying {
        tick: 0.0,
        alive_for: 1e9,
        y: 70.0,
    };
    let mut last = String::new();
    for t in 0..ticks {
        host.tick = t as f32;
        let f = inst.update(def, &assets, &mut host, 0.05);
        let desc: Vec<String> = f
            .layers
            .iter()
            .map(|l| format!("{}|{}|{:?}|{:?}", l.geometry, l.texture, l.color, l.overlay))
            .collect();
        let d = format!("{desc:?}");
        if d != last {
            println!("t={t:3} {d}");
            last = d;
        }
    }
    for n in std::env::var("VARS")
        .unwrap_or_default()
        .split(',')
        .filter(|n| !n.is_empty())
    {
        println!("  v.{n} = {:?}", inst.vars.get(&molang::intern(n)));
    }
    for n in std::env::var("BONES")
        .unwrap_or_default()
        .split(',')
        .filter(|n| !n.is_empty())
    {
        let f = inst.update(def, &assets, &mut host, 0.05);
        println!(
            "  bone {n}: {:?}",
            f.pose.get(n).map(|b| (b.position, b.rotation, b.scale))
        );
    }
}
