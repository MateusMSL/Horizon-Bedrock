//! What a Bedrock pack sets off around mobs: its sounds and particle effects. The mob frames
//! ([`horizon_client::mobs::bedrock`]) queue them; here they are played and drawn.

use super::Game;
use horizon_client::mesher::Vertex;
use horizon_client::mobs::bedrock::Emitted;

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
            return;
        }
        horizon_client::mobs::bedrock::set_camera_rotation(self.yaw, self.pitch);
        let mut pending = bedrock.take_emitted();
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
            let (sprites, more) = bedrock.step_particles();
            drawn = sprites;
            if more.is_empty() {
                break;
            }
            pending = more;
        }
        let world = &self.world;
        for (texture, sprites, lit) in drawn {
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
                out,
            );
        }
    }
}
