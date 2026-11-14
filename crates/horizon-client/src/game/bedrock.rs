//! What a Bedrock pack sets off around mobs: its sounds and particle effects. The mob frames
//! ([`horizon_client::mobs::bedrock`]) queue them; here they are played and drawn.

use super::Game;
use horizon_client::mesher::Vertex;
use horizon_client::blocks::{BlockFacts, Fluid};
use horizon_client::mobs::bedrock::{Emitted, Ground};
use horizon_client::world::World;

/// The world's blocks for the pack's particles: a block stops one where its collision shape
/// stands (a rain drop's splash lands on a slab's top), and water is `minecraft:water`.
struct Blocks<'a, 'w> {
    world: &'a World,
    facts: &'a BlockFacts,
    collision: &'a horizon_world::block_collision::BlockCollisionRegistry<'w>,
}

impl Ground for Blocks<'_, '_> {
    fn solid(&self, p: [f64; 3]) -> bool {
        let b = p.map(|v| v.floor());
        let state = self.world.block(b[0] as i32, b[1] as i32, b[2] as i32);
        match self.collision.cached_shapes().cached_shape(state) {
            Ok(Some(shape)) if !shape.is_empty() => {
                p[1] - b[1] < shape.max_at(horizon_core::geometry::Axis::Y, p[0] - b[0], p[2] - b[2])
            }
            _ => false,
        }
    }

    fn block_id(&self, p: [f64; 3]) -> Option<String> {
        let b = p.map(|v| v.floor() as i32);
        let f = self.facts.get(self.world.block(b[0], b[1], b[2]));
        Some(match f.fluid {
            Fluid::Water => "minecraft:water".to_owned(),
            Fluid::Lava => "minecraft:lava".to_owned(),
            _ => f.name.to_string(),
        })
    }
}

/// Rounds of effects started by effects (a pack's event chain) in one frame.
const CHAIN_LIMIT: usize = 4;

impl Game<'_> {
    /// The pack's attachable for the item in the first-person hand, drawn into `out` as the pack
    /// dresses the view model. False when no attachable of the pack draws it.
    pub(super) fn first_person_pack_item(
        &self,
        item: &str,
        off_hand: bool,
        alpha: f32,
        rest: glam::Mat4,
        packed: u32,
        out: &mut Vec<Vertex>,
    ) -> bool {
        let Some(player) = self.player_model.as_ref() else {
            return false;
        };
        let sprites = &self.gui.atlas.sprites;
        let lookup = |name: &str| sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
        horizon_client::mobs::render::set_first_person_arm_heights(self.heights.interpolated(alpha));
        horizon_client::mobs::render::first_person_item(
            player,
            item,
            off_hand,
            alpha,
            rest,
            [
                self.view.y_rot,
                self.view.x_rot,
                self.view.eye_height_old
                    + (self.view.eye_height - self.view.eye_height_old) * alpha,
            ],
            &lookup,
            packed,
            self.res.mob_data(),
            out,
        )
    }

    /// Plays the queued sounds, starts the queued effects, runs the effects and draws their
    /// sprites into `out` with the particles. Free when no Bedrock pack is on.
    pub(super) fn bedrock_effects(&mut self, camera: glam::DVec3, out: &mut Vec<Vertex>) {
        let bedrock = self
            .res
            .mob_data()
            .bedrock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if bedrock.is_empty() {
            // No pack: the game draws all its own particles again.
            horizon_client_render::bedrock_particles::clear();
            return;
        }
        bedrock.take_game_particles();
        horizon_client::mobs::bedrock::set_camera_rotation(self.yaw, self.pitch);
        // Spawn compasses point at the world spawn in the overworld and spin elsewhere.
        horizon_client::mobs::bedrock::set_compass_target(
            self.spawn
                .filter(|_| self.dim.id() == "minecraft:overworld")
                .map(|s| [s.x, s.z]),
        );
        let mut pending = bedrock.take_emitted();
        // The game's own particles the pack draws (rain splashes, water splashes).
        let requests = horizon_client_render::bedrock_particles::take();
        if !requests.is_empty() && horizon_client::mobs::bedrock_debug::debug() {
            log::info!("bedrock: {} game particles to the pack (first {})", requests.len(), requests[0].effect);
        }
        for r in requests {
            bedrock.spawn_particle(
                &r.effect,
                r.at,
                0.0,
                false,
                None,
                [0.0, 0.0],
                None,
                Default::default(),
            );
        }
        let mut drawn = Vec::new();
        for _ in 0..CHAIN_LIMIT {
            for e in pending.drain(..) {
                match e {
                    Emitted::Sound { event, at } => self.audio.play(&event, Some(at), 1.0, 1.0),
                    Emitted::Particle {
                        effect,
                        at,
                        yaw,
                        baby,
                        name,
                        size,
                        follow,
                        init,
                    } => bedrock.spawn_particle(&effect, at, yaw, baby, name, size, follow, init),
                }
            }
            let ground = Blocks {
                world: &self.world,
                facts: self.res.facts(),
                collision: self.collision,
            };
            let (sprites, more) = bedrock.step_particles_on(Some(&ground));
            drawn = sprites;
            if more.is_empty() {
                break;
            }
            pending = more;
        }
        let world = &self.world;
        let mut additive = Vec::new();
        for horizon_client::mobs::bedrock::Drawn {
            texture,
            sprites,
            lit,
            additive: adds,
        } in drawn
        {
            let Some(rect) = self.gui.atlas.sprites.get(&*texture) else {
                if horizon_client::mobs::bedrock_debug::debug() {
                    log::info!("bedrock-debug: particle texture {texture} is not in the atlas");
                }
                continue;
            };
            horizon_client::mobs::bedrock::sprite_quads(
                &sprites,
                [rect.u0, rect.v0, rect.u1, rect.v1],
                lit,
                camera,
                self.yaw,
                self.pitch,
                &|b| world.light(b[0], b[1], b[2]),
                if adds { &mut additive } else { &mut *out },
            );
        }
        // `particles_add`: drawn with the game's additive particles (`take_additive`).
        for v in &mut additive {
            v.layer = (v.layer & !0xffff) | horizon_client_render::gfx::ADDITIVE_PARTICLE_OVERLAY;
        }
        horizon_client_render::bedrock_particles::stash_additive(additive);
    }
}
