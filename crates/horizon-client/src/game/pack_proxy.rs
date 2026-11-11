//! Entities the game draws by itself (armor stands, minecarts, projectiles, the dragon) drawn
//! with a Bedrock pack's model instead, when a pack has one: a mob stands in for the entity, so
//! the pack's animations, render controllers and attachables run on it as on any mob.

use super::Game;
use horizon_client::{
    mesher::Vertex,
    mobs::{Mob, MobType, Species},
};
use horizon_core::geometry::Vec3;
use std::{cell::RefCell, collections::HashMap, sync::Arc};

thread_local! {
    /// The mob type that stands in for each entity type, by its Java id.
    static KINDS: RefCell<HashMap<String, Option<Arc<MobType>>>> = RefCell::new(HashMap::new());
    /// Each entity's stand-in, kept between frames (the pack's state lives on its id), with the
    /// game time it was last drawn.
    static PROXIES: RefCell<HashMap<u32, (Mob, i64)>> = RefCell::new(HashMap::new());
}

/// Where and how a stand-in is drawn.
pub(super) struct Proxy<'a> {
    /// The entity's Java id (`minecraft:minecart`).
    pub entity: &'a str,
    pub id: u32,
    /// Its position this frame (already blended by the partial tick).
    pub at: glam::DVec3,
    /// Its body's yaw this frame, in the mob's convention (0 faces south).
    pub yaw: f32,
    /// What the pack reads as the head's yaw and pitch (`query.head_y_rotation`, and
    /// `target_y_rotation` from the body's): a projectile's own rotation, a mob's look.
    pub head_yaw: f32,
    pub pitch: f32,
    /// The model's frame when the game's own (`base_transform`'s) does not fit: the pose the
    /// pack's model hangs from, its ground 1.501 below the origin after a `(-1, -1, 1)` scale.
    pub base: Option<glam::Mat4>,
    pub packed: u32,
}

/// The id a stand-in goes by: apart from every mob's and player's (a pack's state lives on the
/// id, and a dragon or a projectile not yet tracked to guests is 0, the local player's id), and
/// from the stand-ins of other entity types with the same id.
fn proxy_id(entity: &str, id: u32) -> u32 {
    let kind = entity.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    0x4000_0000 | ((id ^ (kind << 22)) & 0x3FFF_FFFF)
}

impl Game<'_> {
    /// Whether a pack has a model for the entity `entity` (a Java id).
    pub(super) fn pack_draws(&self, entity: &str) -> bool {
        let bedrock = self
            .res
            .mob_data()
            .bedrock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        !bedrock.is_empty() && bedrock.has_entity(entity)
    }

    /// A pack's projectile: its model turned by the projectile's own yaw and pitch, which the pack
    /// reads as the head's (`target_y_rotation`, `target_x_rotation`); a thrown trident turned as
    /// the game turns its model (`ThrownTridentRenderer`), which a pack's trident follows.
    /// `Some(on_fire)` when the pack drew it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pack_projectile(
        &self,
        a: &horizon_client::projectiles::Projectile,
        at: [f64; 3],
        c: glam::Vec3,
        camera: glam::DVec3,
        alpha: f64,
        packed: u32,
        out: &mut Vec<Vertex>,
    ) -> Option<bool> {
        use horizon_client::projectiles::ProjectileKind;
        let alpha32 = alpha as f32;
        let yaw = a.previous_yaw + alpha32 * (a.yaw - a.previous_yaw);
        let pitch = a.previous_pitch + alpha32 * (a.pitch - a.previous_pitch);
        let trident = matches!(a.kind, ProjectileKind::Trident);
        let proxy = Proxy {
            entity: super::native_projectiles::projectile_type(a),
            id: a.id,
            at: glam::DVec3::from_array(at),
            yaw: 0.0,
            head_yaw: yaw,
            pitch,
            base: trident.then(|| {
                glam::Mat4::from_translation(c)
                    * glam::Mat4::from_rotation_y((a.yaw - 90.0).to_radians())
                    * glam::Mat4::from_rotation_z((a.pitch + 90.0).to_radians())
            }),
            packed,
        };
        self.pack_proxy(&proxy, camera, alpha, |_| {}, out)
            .then(|| a.arrow.as_ref().is_some_and(|s| s.on_fire()))
    }

    /// The pack's cart from the game's model frame: its ground (the wheels' bottom, two pixels
    /// under the floor's middle) where the game's floor is, 4 pixels under the model's origin.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pack_cart(
        &self,
        entity: &str,
        id: u32,
        at: glam::DVec3,
        (yaw, pitch): (f32, f32),
        m: glam::Mat4,
        packed: u32,
        setup: impl FnOnce(&mut Mob),
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) -> bool {
        let proxy = Proxy {
            entity,
            id,
            at,
            yaw: 90.0 - yaw,
            head_yaw: 90.0 - yaw,
            pitch,
            base: Some(m * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.25 - 1.501, 0.0))),
            packed,
        };
        self.pack_proxy(&proxy, camera, alpha, setup, out)
    }

    /// Draws `p` with the pack's model, `setup` filling in the stand-in's state (equipment,
    /// flags) first: false, with nothing drawn, when no pack has a model for it.
    pub(super) fn pack_proxy(
        &self,
        p: &Proxy,
        camera: glam::DVec3,
        alpha: f64,
        setup: impl FnOnce(&mut Mob),
        out: &mut Vec<Vertex>,
    ) -> bool {
        if !self.pack_draws(p.entity) {
            return false;
        }
        let Some(kind) = KINDS.with(|kinds| {
            kinds
                .borrow_mut()
                .entry(p.entity.to_owned())
                .or_insert_with(|| {
                    let types = self.res.mob_types();
                    let base = types
                        .get("minecraft:mannequin")
                        .or_else(|| types.get("minecraft:zombie"))
                        .or_else(|| types.values().next())?;
                    let mut kind = (**base).clone();
                    kind.id = p.entity.to_owned();
                    // No species' own drawing (the mannequin's is only its name and skin).
                    kind.species = Species::Mannequin;
                    kind.eye_height = kind.height * 0.85;
                    Some(Arc::new(kind))
                })
                .clone()
        }) else {
            return false;
        };
        let now = self.world.game_time;
        let atlas = &self.gui.atlas;
        let lookup = |name: &str| atlas.sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
        let data = self.res.mob_data();
        PROXIES.with(|proxies| {
            let mut proxies = proxies.borrow_mut();
            if proxies.len() > 256 {
                proxies.retain(|_, (_, seen)| now - *seen < 200);
            }
            let at = Vec3::new(p.at.x, p.at.y, p.at.z);
            let id = proxy_id(p.entity, p.id);
            let (m, seen) = proxies
                .entry(id)
                .or_insert_with(|| (Mob::new(id, kind.clone(), at, p.yaw), now));
            if !Arc::ptr_eq(&m.kind, &kind) {
                *m = Mob::new(id, kind.clone(), at, p.yaw);
            }
            *seen = now;
            m.position = at;
            m.previous = at;
            m.yaw = p.yaw;
            m.previous_yaw = p.yaw;
            m.body_yaw = p.yaw;
            m.previous_body_yaw = p.yaw;
            m.head_yaw = p.head_yaw;
            m.previous_head_yaw = p.head_yaw;
            m.head_pitch = p.pitch;
            m.previous_head_pitch = p.pitch;
            m.age = now.max(0) as u32;
            m.invisible = false;
            setup(m);
            match p.base {
                Some(base) => horizon_client::mobs::render::render_pack_at(
                    m,
                    alpha as f32,
                    base,
                    &lookup,
                    p.packed,
                    data,
                    out,
                ),
                None => horizon_client::mobs::render::render_pack_only(
                    m,
                    alpha as f32,
                    camera,
                    &lookup,
                    p.packed,
                    data,
                    out,
                ),
            }
        })
    }
}
