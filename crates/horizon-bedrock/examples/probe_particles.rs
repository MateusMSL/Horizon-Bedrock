//! Alive particles of effects over time: `probe_particles <particle json>... `
use horizon_bedrock::jsonc;
use horizon_bedrock::molang::NoHost;
use horizon_bedrock::particle::{Effect, Emitter, NoWorld};
use std::sync::Arc;

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).unwrap();
        let fx = Effect::parse(&jsonc::parse(&text).unwrap()).unwrap();
        let mut e = Emitter::new(Arc::new(fx), 7);
        let mut spawns = Vec::new();
        let (mut peak, mut total) = (0usize, 0usize);
        let dt = 1.0 / 60.0;
        let mut samples = Vec::new();
        for i in 0..600 {
            spawns.clear();
            e.update(dt, &mut NoHost, &NoWorld, &mut spawns);
            total += spawns.len();
            peak = peak.max(e.alive());
            if i % 120 == 0 {
                samples.push(e.alive());
            }
        }
        println!("{path}: spawned {total} in 10s, peak alive {peak}, alive every 2s {samples:?}");
    }
}
