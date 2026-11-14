//! The game's own particles that a Bedrock pack draws itself.
//!
//! A pack replaces a vanilla Bedrock particle by shipping an effect with its identifier
//! (`minecraft:rain_splash_particle`, the splash of a rain drop). Bedrock's own code then asks
//! that effect for one particle wherever it would have made its own (`emitter_rate_manual`).
//! The Java particle the same moment makes (`rain`) is taken out of the game's particle engine
//! here and handed to the Bedrock layer, which starts the pack's effect in its place.
//!
//! Kept apart from the particle engine: `particles.rs` only asks [`redirect`] before it creates
//! a particle.

use horizon_client_core::particle_type::ParticleType;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError, RwLock};

/// The Bedrock identifier Bedrock's own code uses where Java makes `ty`, when there is one.
pub fn bedrock_id(ty: ParticleType) -> Option<&'static [&'static str]> {
    use ParticleType as P;
    Some(match ty {
        P::Rain => &["minecraft:rain_splash_particle"],
        P::Splash => &[
            "minecraft:water_splash_particle",
            "minecraft:water_splash_particle_manual",
        ],
        P::Fishing => &["minecraft:water_wake_particle"],
        P::Crit => &["minecraft:basic_crit_particle"],
        P::Heart => &["minecraft:heart_particle"],
        P::HappyVillager => &["minecraft:villager_happy"],
        P::Glow => &["minecraft:glow_particle"],
        P::TotemOfUndying => &["minecraft:totem_manual", "minecraft:totem_particle"],
        P::Portal => &["minecraft:mob_portal"],
        P::DragonBreath => &["minecraft:dragon_breath_trail"],
        P::Enchant => &["minecraft:enchanting_table_particle"],
        P::CampfireCosySmoke => &["minecraft:campfire_smoke_particle"],
        P::CampfireSignalSmoke => &["minecraft:campfire_tall_smoke_particle"],
        P::SmallFlame => &["minecraft:candle_flame_particle"],
        P::Firefly => &["minecraft:firefly_particle"],
        P::NoxiousGas => &["minecraft:noxious_gas_particle"],
        P::SulfurBubbles => &["minecraft:sulfur_bubble_particle"],
        P::SulfurCubeGoo => &["minecraft:sulfur_cube_goo"],
        P::GeyserBase => &["minecraft:geyser_base"],
        P::ReversePortal => &["minecraft:portal_reverse_particle"],
        P::RedPoplarLeaves => &["minecraft:red_poplar_leaves_particle"],
        P::OrangePoplarLeaves => &["minecraft:orange_poplar_leaves_particle"],
        P::YellowPoplarLeaves => &["minecraft:yellow_poplar_leaves_particle"],
        P::PauseMobGrowth => &["minecraft:pause_mob_growth"],
        P::ResetMobGrowth => &["minecraft:reset_mob_growth"],
        _ => return None,
    })
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Which pack the table was made for ([`install_for`]); 0 for none.
static INSTALLED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static TABLE: RwLock<Option<HashMap<ParticleType, String>>> = RwLock::new(None);
static QUEUE: Mutex<Vec<Request>> = Mutex::new(Vec::new());
/// More than a frame's worth of rain and splashes is not kept.
const QUEUE_LIMIT: usize = 4096;

/// One particle the game asked for, for the pack's effect.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub effect: String,
    pub at: [f64; 3],
    pub velocity: [f64; 3],
}

/// The game particles the loaded pack draws: each Java type whose Bedrock identifier `has` (the
/// pack ships an effect by that name that makes one particle per request: a stream would start
/// for every particle the game makes). Empty when no pack replaces any.
pub fn install(has: impl Fn(&str) -> bool) {
    let table: HashMap<ParticleType, String> = ParticleType::ALL
        .iter()
        .filter_map(|&ty| {
            let id = bedrock_id(ty)?.iter().find(|id| has(id))?;
            Some((ty, (*id).to_owned()))
        })
        .collect();
    ACTIVE.store(!table.is_empty(), Ordering::Relaxed);
    *TABLE.write().unwrap_or_else(PoisonError::into_inner) = Some(table);
}

/// [`install`] once per pack: `key` names the loaded pack (its address); the table is made again
/// only when another pack is in use. Returns the Bedrock identifiers now taken, when it made one.
pub fn install_for(key: usize, has: impl Fn(&str) -> bool) -> Option<Vec<String>> {
    if INSTALLED.swap(key, Ordering::Relaxed) == key {
        return None;
    }
    install(has);
    let table = TABLE.read().unwrap_or_else(PoisonError::into_inner);
    let mut ids: Vec<String> = table.iter().flat_map(|t| t.values().cloned()).collect();
    ids.sort();
    Some(ids)
}

/// Whether the pack draws `ty`: then the request is queued for it and the game makes none.
pub fn redirect(ty: ParticleType, at: [f64; 3], velocity: [f64; 3]) -> bool {
    if !ACTIVE.load(Ordering::Relaxed) {
        return false;
    }
    let table = TABLE.read().unwrap_or_else(PoisonError::into_inner);
    let Some(effect) = table.as_ref().and_then(|t| t.get(&ty)) else {
        return false;
    };
    let mut queue = QUEUE.lock().unwrap_or_else(PoisonError::into_inner);
    if queue.len() < QUEUE_LIMIT {
        queue.push(Request {
            effect: effect.clone(),
            at,
            velocity,
        });
    }
    true
}

static ADDITIVE: Mutex<Vec<crate::mesher::Vertex>> = Mutex::new(Vec::new());

/// A pack's additive particle quads (`particles_add`) for this frame, drawn with the game's own
/// additive particles.
pub fn stash_additive(quads: Vec<crate::mesher::Vertex>) {
    *ADDITIVE.lock().unwrap_or_else(PoisonError::into_inner) = quads;
}

/// The quads [`stash_additive`] left, once.
pub fn take_additive() -> Vec<crate::mesher::Vertex> {
    std::mem::take(&mut *ADDITIVE.lock().unwrap_or_else(PoisonError::into_inner))
}

/// No pack draws any game particle (the pack was taken off).
pub fn clear() {
    INSTALLED.store(0, Ordering::Relaxed);
    if ACTIVE.swap(false, Ordering::Relaxed) {
        QUEUE.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }
}

/// The requests since the last call.
pub fn take() -> Vec<Request> {
    if !ACTIVE.load(Ordering::Relaxed) {
        return Vec::new();
    }
    std::mem::take(&mut *QUEUE.lock().unwrap_or_else(PoisonError::into_inner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_effect_takes_the_place_of_the_game_particle() {
        install(|id| {
            id == "minecraft:rain_splash_particle" || id == "minecraft:water_splash_particle_manual"
        });
        assert!(redirect(ParticleType::Rain, [1.0, 2.0, 3.0], [0.0; 3]));
        assert!(redirect(ParticleType::Splash, [0.0; 3], [0.0; 3]));
        assert!(!redirect(ParticleType::Flame, [0.0; 3], [0.0; 3]));
        let got = take();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].effect, "minecraft:rain_splash_particle");
        assert_eq!(got[1].effect, "minecraft:water_splash_particle_manual");
        install(|_| false);
        assert!(!redirect(ParticleType::Rain, [0.0; 3], [0.0; 3]));
    }
}
