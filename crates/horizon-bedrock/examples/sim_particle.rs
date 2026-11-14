//! Runs a pack effect over flat ground (solid below y = 64) and prints what it spawns.
//! cargo run -p horizon-bedrock --example sim_particle -- <pack dir> <effect id>
//! Env: `WATER=1` water instead of ground, `VS=-40` vertical speed, `SIZES=1` first sprite position/size/uv.
use horizon_bedrock::molang::{Host, Value};
use horizon_bedrock::particle::{Effect, Emitter, Spawn, World};
use std::{collections::HashMap, sync::Arc};

struct Q;
impl Host for Q {
    fn query(&mut self, name: &str, _: &[Value]) -> Value {
        Value::Num(match name {
            "is_pack_setting_enabled" => 1.0,
            // `VS=-20`: the mob's fall speed, for effects that read it (a splash).
            "vertical_speed" => std::env::var("VS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            _ => 0.0,
        })
    }
}
struct Flat;
impl World for Flat {
    fn solid(&self, p: [f32; 3]) -> bool {
        std::env::var("WATER").is_err() && p[1] < 64.0
    }
    fn block_id(&self, p: [f32; 3]) -> Option<String> {
        Some(
            if p[1] >= 64.0 {
                "minecraft:air"
            } else if std::env::var("WATER").is_ok() {
                "minecraft:water"
            } else {
                "minecraft:grass_block"
            }
            .to_owned(),
        )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut fx = HashMap::new();
    for f in walk(std::path::Path::new(&args[1]).join("particles")) {
        let Ok(t) = std::fs::read_to_string(&f) else {
            continue;
        };
        let Ok(j) = serde_json::from_str::<serde_json::Value>(&t) else {
            continue;
        };
        if let Some(e) = Effect::parse(&j) {
            fx.insert(e.id.to_ascii_lowercase(), Arc::new(e));
        }
    }
    let mut live: Vec<(String, Emitter)> = Vec::new();
    let mut e = Emitter::new(fx[&args[2]].clone(), 7);
    e.set_transform([0.5, 64.0, 0.5], 0.0);
    e.request_manual(1);
    live.push((args[2].clone(), e));
    for step in 0..60 {
        let mut out = Vec::new();
        for (_, e) in &mut live {
            e.update(0.05, &mut Q, &Flat, &mut out);
        }
        for (id, e) in &mut live {
            let sp = e.sprites(&mut Q);
            let n = sp.len();
            if let (Some(s), true) = (sp.first(), std::env::var("SIZES").is_ok()) {
                println!("  first sprite pos {:?} size {:?} uv {:?}", s.pos, s.size, s.uv);
            }
            if n > 0 || e.alive() > 0 || step < 3 {
                println!(
                    "t={:.2} {id}: {n} sprites alive {} finished {}",
                    step as f32 * 0.05,
                    e.alive(),
                    e.finished()
                );
            }
        }
        for s in out {
            if let Spawn::Effect { effect, at, seed } = s {
                println!(
                    "t={:.2} spawn {effect} at {at:?} seed {seed:?}",
                    step as f32 * 0.05
                );
                if let Some(f) = fx.get(&effect.to_ascii_lowercase()) {
                    let mut n = Emitter::new(f.clone(), step);
                    n.set_transform(at, 0.0);
                    n.set_init(None, seed);
                    live.push((effect, n));
                }
            }
        }
        live.retain(|(_, e)| !e.finished());
    }
}

fn walk(d: std::path::PathBuf) -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(walk(p));
        } else {
            v.push(p);
        }
    }
    v
}
