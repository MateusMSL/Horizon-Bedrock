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
            "rotation_to_camera" | "camera_rotation"
                if args.first().map_or(0.0, |v| v.num()) == 0.0 =>
            {
                Value::Num(0.0)
            }
            "camera_rotation" | "rotation_to_camera" => Value::Num(
                std::env::var("CAM")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0),
            ),
            "is_local_player" => Value::Num(0.0),
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
        eprintln!("usage: probe_death <pack> <entity id>");
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
    let mut host = Dying {
        tick: 0.0,
        alive_for: 40.0,
        y: 70.0,
    };
    for t in 0..120 {
        host.tick = t as f32;
        let f = inst.update(def, &assets, &mut host, 0.05);
        let mut best: Option<(&String, f32, [f32; 3])> = None;
        for (n, b) in &f.pose.bones {
            let m = b.position.iter().map(|v| v.abs()).fold(0.0, f32::max);
            if best.is_none_or(|x| m > x.1) {
                best = Some((n, m, b.position));
            }
            if n == "root" && t % 10 == 0 {
                println!(
                    "   root {:?} rot {:?} rot_e {:?}",
                    b.position, b.rotation, b.rotation_entity
                );
            }
        }
        if t % 5 == 0 || (38..46).contains(&t) {
            println!("t={t:3} {:?}", best);
        }
        if t == 119 {
            let g = geos
                .get(&def.geometry.get("default").unwrap().to_ascii_lowercase())
                .expect("geo");
            let (o, _, _) = horizon_bedrock::entity::bone_origins(g, &f.pose);
            let mut names: Vec<_> = o.keys().collect();
            names.sort();
            for n in names {
                println!("  origin {n}: {:?}", o[n]);
            }
            for n in [
                "oxsltg", "oxsltg.x", "oxsltg.y", "oxsltg.z", "jbzhtg", "mtzbhe", "oqdvqj",
                "gphemh", "owqcut", "wbrmqy", "ipkwqh", "igbugh", "gfqqvj", "aduuro", "tnglpc",
            ] {
                println!("  v.{n} = {:?}", inst.vars.get(&molang::intern(n)));
            }
            for (n, b) in &f.pose.bones {
                if b.position.iter().any(|v| v.abs() > 0.5)
                    || b.rotation.iter().any(|v| v.abs() > 1.0)
                {
                    println!(
                        "  {n}: pos {:?} rot {:?} rot_e {:?}",
                        b.position, b.rotation, b.rotation_entity
                    );
                }
            }
        }
    }
}
