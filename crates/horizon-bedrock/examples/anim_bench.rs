//! Times the Bedrock engine's posing (`Instance::update`: scripts, animations, controllers,
//! render controllers) for each entity and attachable of a converted pack, with no game around
//! it: a measure that stays the same from run to run, for work on the engine's speed.
//!
//! `cargo run --release -p horizon-bedrock --example anim_bench -- <converted pack folder> [frames]`
//!
//! The converted folder is the one the game keeps in `.horizon-cache/bedrock/`. The host
//! answers a few movement queries as a walking mob would and nothing else, so the controllers
//! take the paths a mob walking on the ground takes.
//!
//! With `ANIM_CHECK=1` it also prints a hash of what each entity's next updates give (its sorted
//! bone poses, layers, emits, scale): the same before and after a change that only makes
//! posing faster.

use horizon_bedrock::bundle::Bundle;
use horizon_bedrock::entity::{Assets, Instance};
use horizon_bedrock::molang::{Host, Value};
use std::time::Instant;

struct Walking {
    time: f32,
}

impl Host for Walking {
    fn query(&mut self, name: &str, _args: &[Value]) -> Value {
        match name {
            "modified_move_speed" | "ground_speed" => Value::Num(0.6),
            "modified_distance_moved" | "walk_distance" => Value::Num(self.time * 4.0),
            "is_on_ground" | "is_alive" => Value::Num(1.0),
            "life_time" | "anim_time" => Value::Num(self.time),
            "health" | "max_health" => Value::Num(20.0),
            _ => Value::Null,
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(root) = args.next() else {
        eprintln!("usage: anim_bench <converted pack folder> [frames]");
        std::process::exit(2);
    };
    let frames: u32 = args.next().and_then(|n| n.parse().ok()).unwrap_or(300);
    let loaded = Instant::now();
    let bundle = Bundle::load(std::path::Path::new(&root));
    eprintln!(
        "loaded {} entities, {} attachables in {:.2} s",
        bundle.entities.len(),
        bundle.attachables.len(),
        loaded.elapsed().as_secs_f32()
    );
    let assets = Assets {
        animations: &bundle.library,
        render_controllers: &bundle.render_controllers,
        geometries: Some(&bundle.geometries),
    };
    let mut ids: Vec<&String> = bundle.entities.keys().collect();
    ids.sort();
    let dt = 1.0 / 60.0;
    let mut rows: Vec<(f64, &str)> = Vec::new();
    let started = Instant::now();
    for id in ids {
        let def = &bundle.entities[id];
        let mut instance = Instance::new(1);
        let mut host = Walking { time: 0.0 };
        // Settle first (a controller's first frames run its entry scripts), then time.
        for _ in 0..30 {
            host.time += dt;
            std::hint::black_box(instance.update(def, &assets, &mut host, dt));
        }
        let t = Instant::now();
        for _ in 0..frames {
            host.time += dt;
            std::hint::black_box(instance.update(def, &assets, &mut host, dt));
        }
        rows.push((t.elapsed().as_secs_f64() * 1e6 / f64::from(frames), id));
        if std::env::var_os("ANIM_CHECK").is_some() {
            let mut sum = String::new();
            for _ in 0..40 {
                host.time += dt;
                let f = instance.update(def, &assets, &mut host, dt);
                let mut bones: Vec<_> = f.pose.bones.iter().collect();
                bones.sort_by(|a, b| a.0.cmp(b.0));
                sum += &format!("{:?}{:?}{:?}{:?}", bones, f.layers, f.emits, f.scale);
            }
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            sum.hash(&mut h);
            eprintln!("check {id} {:016x}", h.finish());
        }
    }
    let total: f64 = rows.iter().map(|(us, _)| us).sum();
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!(
        "{} entities, {frames} frames each: {:.1} us a frame for one of each, mean {:.2} us ({:.1} s)",
        rows.len(),
        total,
        total / rows.len().max(1) as f64,
        started.elapsed().as_secs_f32()
    );
    // `ANIM_ROWS=n`: how many of the slowest to list (15).
    let shown = std::env::var("ANIM_ROWS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(15);
    for (us, id) in rows.iter().take(shown) {
        println!("  {us:8.2} us  {id}");
    }
}
