//! Live switches for debugging a Bedrock pack in the game, read from a file that is reloaded
//! while the game runs: edit, save, and the next frame draws with it. Nothing to compile.
//!
//! The file is `bedrock-debug.json` in the game folder (`HORIZON_BEDROCK_DEBUG_FILE` names
//! another). With `HORIZON_BEDROCK_DEBUG` set and no file, a template is written there.
//! The `HORIZON_BEDROCK_DEBUG`, `_DUMP`, `_DUMP_VARS` and `_HIDE` variables still work and add
//! to the file's lists.
//!
//! ```json
//! {
//!   "debug": true,
//!   "hide": ["crv"],
//!   "dump": ["spider", "enderman"],
//!   "dump_vars": ["habcyx", "acloko"],
//!   "dump_bones": ["rightarm", "rightitem"],
//!   "held_anchor": "auto",
//!   "trace_rider": false,
//!   "materials": { "oreville_auulis": { "additive": false }, "*": {} },
//!   "queries": { "is_powered": 1 }
//! }
//! ```
//!
//! * `hide`: layers whose texture ends with one of these names are not drawn.
//! * `dump` / `dump_vars`: kinds (part of the id) logged once a second, and the `v.` variables
//!   logged with them. `dump_bones`: bones (part of the name) whose pose the dump lists.
//! * `first_person_off`, `fp_offset`, `fp_rotate`, `fp_scale`: the pack's attachable in the first-person hand (shift in blocks, turn in degrees x,y,z, scale).
//! * `hide_bones` / `hide_geometry`: bones (part of the name, any model) and models (part of the
//!   geometry id) not drawn, to take layers off a body: `["hat", "jacket", "sleeve", "pants"]`
//!   leaves the skin's base layer; `["zwjvro"]` drops a pack's face-expression model.
//! * `push_bones`: `{"name": pixels}`, bones (part of the name) moved forward by that many
//!   pixels (negative: back), each by its own amount, to see which layer of a face shows.
//!   The log lists every model's bones once (`bedrock-debug: bones ...`).
//! * `rotate_bones` / `move_bones`: `{"name": [x, y, z]}`, bones (part of the name) turned by
//!   degrees / moved by pixels on top of the animation (the pack's own `rotation` / `position`
//!   axes), to find the orientation a pose needs: `{"rotate_bones": {"arm": [0, 90, 0]}}`.
//! * `worn`: moves what a mob wears, by slot (`head`, `chest`, `legs`, `feet`, `body`, `cape`,
//!   `elytra`), from where the body's bone carries it: `"offset"` in pixels (x right, y down, z
//!   towards the back), `"rotate"` in degrees (x, y, z) about that bone's pivot, `"scale"`.
//!   `{"cape": {"offset": [0, 1, 0.5]}, "head": {"rotate": [0, 0, 5]}}`.
//! * `held_anchor`: `"auto"`, `"origin"`, `"pivot"` or `"frame"`, how held items are put on the hand.
//! * `trace_rider`: while riding, writes `rider-trace.log` (the logs folder): `rider-trace-frame` (every
//!   frame), `rider-trace-tick` and `rider-trace-pose` (the mount's and the rider's bones, as
//!   `dump_bones` names them, else root/waist/body/head) lines: the mount's and the rider's old/new/drawn heights and yaws, to find a tremor.
//! * `materials`: flags forced on a material by its name (`*` for every one): `true` sets it,
//!   `false` clears it, absent leaves what the pack says.
//! * `queries`: `query.x` answers this number for every mob, whatever the game says.
//! * `items`: item ids whose inventory drawing goes to the log (`latest.log`, lines starting
//!   `bedrock-debug: item`): definition file, model file and each sprite file with its size and
//!   hash. Change the list to log again.

use horizon_bedrock::material::MaterialFlags;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant, SystemTime},
};

/// What of a held item goes on the hand (`Settings::held_anchor_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldAnchor {
    Frame,
    Origin,
    Pivot,
}

/// Flags forced on a material; absent ones keep what the pack's material says.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Patch {
    pub alpha_test: Option<bool>,
    pub blend: Option<bool>,
    pub additive: Option<bool>,
    pub emissive: Option<bool>,
    pub emissive_alpha: Option<bool>,
    pub double_sided: Option<bool>,
    pub uv_anim: Option<bool>,
    pub energy_swirl: Option<bool>,
    pub no_color: Option<bool>,
    pub glint: Option<bool>,
    pub invert_culling: Option<bool>,
    pub no_depth_write: Option<bool>,
    pub no_depth_test: Option<bool>,
    pub multitexture: Option<bool>,
    /// `0` clears the stencil write.
    pub stencil_write: Option<u8>,
    /// `0` clears the stencil test.
    pub stencil_not_equal: Option<u8>,
}

impl Patch {
    pub fn apply(&self, f: &mut MaterialFlags) {
        let set = |dst: &mut bool, v: Option<bool>| {
            if let Some(v) = v {
                *dst = v;
            }
        };
        set(&mut f.alpha_test, self.alpha_test);
        set(&mut f.blend, self.blend);
        set(&mut f.additive, self.additive);
        set(&mut f.emissive, self.emissive);
        set(&mut f.emissive_alpha, self.emissive_alpha);
        set(&mut f.double_sided, self.double_sided);
        set(&mut f.uv_anim, self.uv_anim);
        set(&mut f.energy_swirl, self.energy_swirl);
        set(&mut f.no_color, self.no_color);
        set(&mut f.glint, self.glint);
        set(&mut f.invert_culling, self.invert_culling);
        set(&mut f.no_depth_write, self.no_depth_write);
        set(&mut f.no_depth_test, self.no_depth_test);
        set(&mut f.multitexture, self.multitexture);
        if let Some(v) = self.stencil_write {
            f.stencil_write = v;
        }
        if let Some(v) = self.stencil_not_equal {
            f.stencil_not_equal = v;
        }
    }
}

/// A shift, a turn and a scale applied to what a bone carries.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Worn {
    /// Pixels: x right, y down, z towards the back.
    pub offset: Vec<f32>,
    /// Degrees about x, y, z, around the bone's pivot.
    pub rotate: Vec<f32>,
    pub scale: Option<f32>,
}

impl Worn {
    fn matrix(&self) -> glam::Mat4 {
        let at = |v: &[f32], i: usize| v.get(i).copied().unwrap_or(0.0);
        glam::Mat4::from_translation(
            glam::Vec3::new(at(&self.offset, 0), at(&self.offset, 1), at(&self.offset, 2)) / 16.0,
        ) * glam::Mat4::from_euler(
            glam::EulerRot::ZYX,
            at(&self.rotate, 2).to_radians(),
            at(&self.rotate, 1).to_radians(),
            at(&self.rotate, 0).to_radians(),
        ) * glam::Mat4::from_scale(glam::Vec3::splat(self.scale.unwrap_or(1.0)))
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Free text: ignored.
    #[serde(rename = "_help")]
    pub help: Option<serde_json::Value>,
    pub debug: bool,
    pub hide: Vec<String>,
    /// Bones (part of the name, any case) and models (part of the geometry id) not drawn.
    pub hide_bones: Vec<String>,
    pub hide_geometry: Vec<String>,
    /// Bones (part of the name, any case) moved forward (towards the front the model faces) by
    /// this many pixels, negative to pull back: to tell which of the layers stacked on a face
    /// shows through, push each by a different amount.
    pub push_bones: std::collections::BTreeMap<String, f32>,
    /// Bones (part of the name, any case) turned by these degrees (x, y, z, as the pack's
    /// `rotation`), added to the animation's: to find the orientation a pose needs.
    pub rotate_bones: std::collections::BTreeMap<String, [f32; 3]>,
    /// Bones (part of the name, any case) moved by these pixels (x, y, z, as the pack's
    /// `position`), added to the animation's.
    pub move_bones: std::collections::BTreeMap<String, [f32; 3]>,
    /// `rotate_bones` / `move_bones` for the bones of what is held or worn (attachables), in
    /// third and first person.
    pub held_rotate_bones: std::collections::BTreeMap<String, [f32; 3]>,
    pub held_move_bones: std::collections::BTreeMap<String, [f32; 3]>,
    /// The same, only for the first-person hand (a torch's flame seen from the eye).
    pub first_person_rotate_bones: std::collections::BTreeMap<String, [f32; 3]>,
    pub first_person_move_bones: std::collections::BTreeMap<String, [f32; 3]>,
    /// `true` stops turning a first-person item's glows (bones turned relative to the entity) to
    /// the camera: they keep the pack's own turn.
    pub first_person_glow_fixed: bool,
    pub dump: Vec<String>,
    pub dump_vars: Vec<String>,
    /// Bones (part of the name) whose pose the dump lists, moved or not, instead of the first
    /// thirty moved ones.
    pub dump_bones: Vec<String>,
    /// Where what is held sits on the hand: `"auto"` (default) puts the frame origin on it for a
    /// bound bone with no parent (the vanilla-made models, whose animations add the 24 sixteenths
    /// themselves) and the bone's pivot for one under a parent bone; `"pivot"` and `"frame"`
    /// force one of the two.
    pub held_anchor: String,
    /// The same for the first-person hand: by default (or `"third"`) `held_anchor`'s choice, as
    /// in third person; `"holder"` keeps the attachable where the holder's bone carries a point
    /// of the holder's own model (the bone follows the hand's change from rest); `"frame"`,
    /// `"origin"` or `"pivot"` force one.
    pub fp_held_anchor: String,
    /// Signs multiplied into the animated turns (x, y, z) while the first-person view model
    /// draws: to test which turn convention matches Bedrock's for the large first-person poses.
    pub fp_rot_signs: Vec<f32>,
    /// `true` leaves the first-person hand on the game's own item models (the pack's attachable
    /// draws it by default).
    pub first_person_off: bool,
    /// Where a pack's attachable sits on the first-person hand, from the game's hand pose: the
    /// shift in blocks, the turn in degrees (x, y, z) and the scale (1 when empty).
    pub fp_offset: Vec<f32>,
    pub fp_rotate: Vec<f32>,
    pub fp_scale: Option<f32>,
    /// Writes `rider-trace.log` (logs folder, every frame and tick) while the player rides: heights, yaws and
    /// the seat of the mount and of the rider's model.
    pub trace_rider: bool,
    /// Writes a line per frame (`fp-trace`: time, walk cycle, arm and item bone positions) while the first-person view model draws.
    pub trace_fp: bool,
    /// What a mob wears, moved from where its bone carries it, by slot.
    pub worn: HashMap<String, Worn>,
    /// Item ids (`"torch"`, `"minecraft:glass"`) whose inventory drawing is written to the log
    /// when this list changes: the item definition, the model, and every sprite file read with
    /// its size and hash.
    pub items: Vec<String>,
    pub materials: HashMap<String, Patch>,
    /// Flags forced on one bone's cubes (part of the bone name, any case; `*` = all): the same
    /// fields as `materials`, but for a bone, whatever material the pack gives it. To tell
    /// whether a shell, a body or an eye draws wrongly because of its own material.
    pub bone_materials: HashMap<String, Patch>,
    /// Only these bones (part of the name, any case) draw; the rest of the model hides.
    pub solo_bones: Vec<String>,
    /// Colour and alpha multiplied into a texture's layer: `{"cmj": [1, 1, 1, 0.5]}` (r, g, b, a;
    /// the key is the end of the texture path). Shows how much alpha the draw needs to look as
    /// the game's.
    pub layer_tint: HashMap<String, [f32; 4]>,
    /// Colour and alpha multiplied into one bone's cubes (part of the bone name, any case):
    /// `{"2dh923": [1, 1, 1, 0.5]}`. Like `layer_tint`, but for the bone alone.
    pub bone_tint: HashMap<String, [f32; 4]>,
    /// `true` draws a texture's layer from its opaque copy (`_solid`, alpha as glow only),
    /// `false` from the converted texture with its alpha, whatever the material says.
    pub solid: HashMap<String, bool>,
    /// With `dump` naming the mob: logs every layer drawn (geometry, texture, material, flags
    /// per bone group, bones, tint) every 5 s.
    pub dump_draw: bool,
    /// Ticks between the dump's lines (20 when 0 or unset): 1 follows a tremor frame by frame.
    pub dump_every: f32,
    /// Layers of the mob's pack model (the `L0`, `L1`... of the dump's `layers:`) not drawn.
    pub hide_layers: Vec<usize>,
    /// Only these layers (by number) draw; the rest of the pack model hides.
    pub solo_layers: Vec<usize>,
    /// Alpha multiplied into a layer by number: `{"2": 0.3}`.
    pub layer_alpha: HashMap<usize, f32>,
    /// Layers of what a mob holds or wears not drawn: `"main_hand"` (the slot's every layer) or
    /// `"main_hand:1"` (the second layer of that slot's attachable).
    pub hide_attached: Vec<String>,
    /// Alpha multiplied into an attachable's layer: `{"main_hand": 0.5, "main_hand:1": 0.2}`.
    pub attached_alpha: HashMap<String, f32>,
    /// Which bone of the holder an attachable's bone follows, in place of the pack's own
    /// binding: `{"item": "bodyq"}` (every slot) or `{"main_hand:item": "bodyq"}`.
    pub bind_bones: HashMap<String, String>,
    pub queries: HashMap<String, f32>,
}

fn list(var: &str) -> Vec<String> {
    std::env::var(var)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

impl Settings {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut s: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        // A blank name is a part of every name: `[""]` would hide, solo or dump everything.
        for list in [
            &mut s.hide,
            &mut s.hide_bones,
            &mut s.hide_geometry,
            &mut s.solo_bones,
            &mut s.hide_attached,
            &mut s.dump,
            &mut s.dump_vars,
            &mut s.dump_bones,
        ] {
            list.retain(|name| !name.trim().is_empty());
        }
        s.materials = std::mem::take(&mut s.materials)
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v))
            .collect();
        s.worn = std::mem::take(&mut s.worn)
            .into_iter()
            .map(|(k, v)| {
                let k = k.to_ascii_lowercase();
                (if k == "torso" { "chest".to_owned() } else { k }, v)
            })
            .collect();
        s.queries = std::mem::take(&mut s.queries)
            .into_iter()
            .map(|(k, v)| (k.trim_start_matches("query.").to_ascii_lowercase(), v))
            .collect();
        Ok(s)
    }

    /// The file's settings with the environment variables' lists added.
    fn with_env(mut self) -> Self {
        self.debug |= std::env::var_os("HORIZON_BEDROCK_DEBUG").is_some();
        self.hide.extend(list("HORIZON_BEDROCK_HIDE"));
        self.dump
            .extend(list("HORIZON_BEDROCK_DUMP").into_iter().map(|k| k.to_ascii_lowercase()));
        self.dump_vars.extend(list("HORIZON_BEDROCK_DUMP_VARS"));
        self.dump_bones.extend(list("HORIZON_BEDROCK_DUMP_BONES"));
        for k in &mut self.dump {
            k.make_ascii_lowercase();
        }
        self
    }

    /// What of a held item's bound bone goes on the hand: the frame's origin (`Frame`: 24
    /// pixels up, a model drawn in the player's own frame beside the hand, as the vanilla shield
    /// is from 17 pixels up), the model's origin (`Origin`: a model drawn around its own origin,
    /// as a pack's bell, lantern or sword is, the hand at 0), or the bone's pivot (`Pivot`).
    /// `has_parent`: the bone sits under another bone of its model; `pivot_height`: its pivot,
    /// pixels up from the model's floor; `floor`: its lowest cube at rest, the same way.
    pub fn held_anchor_of(&self, has_parent: bool, pivot_height: f32, floor: Option<f32>) -> HeldAnchor {
        match self.held_anchor.to_ascii_lowercase().as_str() {
            "frame" => HeldAnchor::Frame,
            "pivot" => HeldAnchor::Pivot,
            "origin" => HeldAnchor::Origin,
            _ => {
                // A top-level bound bone: Bedrock puts the model's 24-pixel point on the hand
                // whatever its pivot (the test pack's paper, pivot 0, hangs its origin 1.5
                // blocks off the hand in Bedrock, first and third person).
                let _ = pivot_height;
                let in_frame = floor.is_some_and(|y| y >= 12.0) || !has_parent;
                if in_frame { HeldAnchor::Frame } else { HeldAnchor::Origin }
            }
        }
    }

    /// The first-person item's own matrix on the hand: the shift and turn from the file, then
    /// `orient` (the item stood upright, `mobs::render::stand_upright`), at the file's scale.
    pub fn first_person_matrix(&self) -> glam::Mat4 {
        // The first-person view model, moved by hand (camera space, blocks and degrees):
        // nothing unless the file says so.
        let at = |v: &[f32], i: usize| v.get(i).copied().unwrap_or(0.0);
        glam::Mat4::from_translation(glam::Vec3::new(
            at(&self.fp_offset, 0),
            at(&self.fp_offset, 1),
            at(&self.fp_offset, 2),
        )) * glam::Mat4::from_euler(
            glam::EulerRot::YXZ,
            at(&self.fp_rotate, 1).to_radians(),
            at(&self.fp_rotate, 0).to_radians(),
            at(&self.fp_rotate, 2).to_radians(),
        ) * glam::Mat4::from_scale(glam::Vec3::splat(self.fp_scale.unwrap_or(1.0)))
    }

    /// The shift of what is worn in `slot` (`head`, `chest`/`torso`, `legs`, `feet`, `body`,
    /// `cape`, `elytra`): identity when the file says nothing.
    pub fn worn_matrix(&self, slot: &str) -> glam::Mat4 {
        let slot = if slot == "torso" { "chest" } else { slot };
        self.worn
            .get(slot)
            .map_or(glam::Mat4::IDENTITY, Worn::matrix)
    }

    /// Whether the pack model's layer number `i` does not draw (`hide_layers`, `solo_layers`).
    pub fn hides_layer(&self, i: usize) -> bool {
        self.hide_layers.contains(&i) || (!self.solo_layers.is_empty() && !self.solo_layers.contains(&i))
    }

    /// The alpha factor of the pack model's layer number `i`.
    pub fn layer_alpha_for(&self, i: usize) -> f32 {
        self.layer_alpha.get(&i).copied().unwrap_or(1.0)
    }

    fn attached_key(slot: &str, i: usize) -> (String, String) {
        (slot.to_ascii_lowercase(), format!("{}:{i}", slot.to_ascii_lowercase()))
    }

    /// Whether layer `i` of the attachable in `slot` does not draw.
    pub fn hides_attached(&self, slot: &str, i: usize) -> bool {
        let (whole, one) = Self::attached_key(slot, i);
        self.hide_attached
            .iter()
            .any(|h| h.eq_ignore_ascii_case(&whole) || h.eq_ignore_ascii_case(&one))
    }

    /// The alpha factor of layer `i` of the attachable in `slot`.
    pub fn attached_alpha_for(&self, slot: &str, i: usize) -> f32 {
        let (whole, one) = Self::attached_key(slot, i);
        let find = |k: &str| {
            self.attached_alpha
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(k))
                .map(|(_, v)| *v)
        };
        find(&one).or_else(|| find(&whole)).unwrap_or(1.0)
    }

    /// The holder bone `bone` of the attachable in `slot` follows, when the file says one.
    pub fn bind_for(&self, slot: &str, bone: &str) -> Option<&str> {
        if self.bind_bones.is_empty() {
            return None;
        }
        let slot = slot.to_ascii_lowercase();
        let find = |k: &str| {
            self.bind_bones
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.as_str())
        };
        find(&format!("{slot}:{bone}")).or_else(|| find(bone))
    }

    pub fn hides_bone(&self, bone: &str) -> bool {
        !self.hide_bones.is_empty() && {
            let bone = bone.to_ascii_lowercase();
            self.hide_bones
                .iter()
                .any(|h| bone.contains(&h.to_ascii_lowercase()))
        }
    }

    /// The pixels `bone` moves forward: every `push_bones` entry its name contains, added.
    pub fn push_for(&self, bone: &str) -> f32 {
        if self.push_bones.is_empty() {
            return 0.0;
        }
        let bone = bone.to_ascii_lowercase();
        self.push_bones
            .iter()
            .filter(|(k, _)| bone.contains(&k.to_ascii_lowercase()))
            .map(|(_, px)| *px)
            .sum()
    }

    /// The degrees `bone` is turned: every `rotate_bones` entry its name contains, added.
    pub fn rotate_for(&self, bone: &str) -> [f32; 3] {
        Self::sum3(&self.rotate_bones, bone)
    }

    /// The pixels `bone` is moved: every `move_bones` entry its name contains, added.
    pub fn move_for(&self, bone: &str) -> [f32; 3] {
        Self::sum3(&self.move_bones, bone)
    }

    /// The degrees and pixels a bone of a held or worn piece is turned and moved
    /// (`held_*_bones`, plus `first_person_*_bones` in first person).
    pub fn held_adjust_for(&self, bone: &str, first_person: bool) -> ([f32; 3], [f32; 3]) {
        let mut turn = Self::sum3(&self.held_rotate_bones, bone);
        let mut shift = Self::sum3(&self.held_move_bones, bone);
        if first_person {
            let (t, m) = (
                Self::sum3(&self.first_person_rotate_bones, bone),
                Self::sum3(&self.first_person_move_bones, bone),
            );
            for i in 0..3 {
                turn[i] += t[i];
                shift[i] += m[i];
            }
        }
        (turn, shift)
    }

    fn sum3(map: &std::collections::BTreeMap<String, [f32; 3]>, bone: &str) -> [f32; 3] {
        let mut out = [0.0; 3];
        if map.is_empty() {
            return out;
        }
        let bone = bone.to_ascii_lowercase();
        for (k, v) in map {
            if bone.contains(&k.to_ascii_lowercase()) {
                for i in 0..3 {
                    out[i] += v[i];
                }
            }
        }
        out
    }

    /// Whether `bone` is out of the `solo_bones` list (which, when set, names the only ones drawn).
    pub fn solo_hides(&self, bone: &str) -> bool {
        !self.solo_bones.is_empty() && {
            let bone = bone.to_ascii_lowercase();
            !self
                .solo_bones
                .iter()
                .any(|s| bone.contains(&s.to_ascii_lowercase()))
        }
    }

    /// The `layer_tint` entry whose key ends the texture path, if any.
    pub fn tint_for(&self, texture: &str) -> Option<[f32; 4]> {
        self.layer_tint
            .iter()
            .find(|(k, _)| texture.ends_with(k.as_str()))
            .map(|(_, v)| *v)
    }

    /// The `solid` entry whose key ends the texture path, if any.
    pub fn solid_for(&self, texture: &str) -> Option<bool> {
        self.solid
            .iter()
            .find(|(k, _)| texture.ends_with(k.as_str()))
            .map(|(_, v)| *v)
    }

    /// The product of the `bone_tint` entries whose key the bone's name contains.
    pub fn bone_tint_for(&self, bone: &str) -> [f32; 4] {
        let bone = bone.to_ascii_lowercase();
        let mut out = [1.0; 4];
        for (k, v) in &self.bone_tint {
            if k == "*" || bone.contains(&k.to_ascii_lowercase()) {
                for (o, f) in out.iter_mut().zip(v) {
                    *o *= f;
                }
            }
        }
        out
    }

    /// `flags` with the `bone_materials` patches whose key the bone's name contains.
    pub fn bone_flags(&self, mut f: MaterialFlags, bone: &str) -> MaterialFlags {
        if self.bone_materials.is_empty() {
            return f;
        }
        let bone = bone.to_ascii_lowercase();
        for (k, p) in &self.bone_materials {
            if k == "*" || bone.contains(&k.to_ascii_lowercase()) {
                p.apply(&mut f);
            }
        }
        f
    }

    pub fn hides_geometry(&self, geometry: &str) -> bool {
        !self.hide_geometry.is_empty() && {
            let geometry = geometry.to_ascii_lowercase();
            self.hide_geometry
                .iter()
                .any(|h| geometry.contains(&h.to_ascii_lowercase()))
        }
    }

    pub fn hides(&self, texture: &str) -> bool {
        self.hide.iter().any(|h| texture.ends_with(h.as_str()))
    }

    pub fn dumps(&self, kind: &str) -> bool {
        let kind = kind.to_ascii_lowercase();
        self.dump
            .iter()
            .any(|k| k == "all" || kind.contains(k.as_str()))
    }

    pub fn flags(&self, mut f: MaterialFlags, material: &str) -> MaterialFlags {
        if self.materials.is_empty() {
            return f;
        }
        if let Some(p) = self.materials.get("*") {
            p.apply(&mut f);
        }
        if let Some(p) = self.materials.get(&material.to_ascii_lowercase()) {
            p.apply(&mut f);
        }
        f
    }
}

const TEMPLATE: &str = r#"{
  "_help": "Salvar recarrega no jogo (1 s). hide: sufixos de textura que nao desenham. hide_bones: ossos (parte do nome) que nao desenham (ex.: hat, jacket, sleeve, pants); hide_geometry: modelos (parte do id) que nao desenham. push_bones: {\"osso\": pixels} move o osso (parte do nome) para frente (negativo: para tras), um valor por osso. rotate_bones: {\"osso\": [x,y,z]} gira o osso (graus) alem da animacao; move_bones: {\"osso\": [x,y,z]} move o osso (pixels), eixos do pack; o log lista os ossos de cada modelo (bedrock-debug: bones). dump: ids (parte) logados 1x/s; dump_vars: variaveis v.* logadas junto; dump_bones: ossos (parte do nome) cuja pose entra no dump. held_rotate_bones / held_move_bones: como rotate_bones/move_bones, para ossos do que o mob segura/veste; first_person_rotate_bones / first_person_move_bones: o mesmo so na mao em primeira pessoa (ex.: {\"7d2e6\": [0,90,0]} gira a chama da tocha); first_person_glow_fixed: true para de virar os brilhos do item em primeira pessoa para a camera. held_anchor: 'auto' (modelo desenhado perto da mao do jogador, cubos a partir de 12 px: frame; senao origem do modelo na mao), 'origin', 'pivot' ou 'frame'. materials: flags forcadas por nome do material ('*' = todos); true liga, false desliga. bone_materials: as mesmas flags, por osso (parte do nome, '*' = todos), ex.: {\"2dh923\": {\"blend\": true, \"double_sided\": false}}. solo_bones: so esses ossos desenham. layer_tint: {\"cmj\": [1,1,1,0.5]} multiplica cor e alfa da camada cuja textura termina assim. bone_tint: o mesmo, so para o osso (parte do nome), ex.: {\"2dh923\": [1,1,1,0.5]}. solid: {\"cmj\": true} usa a copia opaca (false: a textura com alfa). dump_every: ticks entre linhas do dump (20; 1 segue um tremor quadro a quadro). dump_draw: com dump no mob, loga cada camada desenhada (material, flags por grupo de ossos, tint) a cada 5 s. queries: query.x fixa em um numero para todo mob. worn: move o que o mob veste por slot (head, chest, legs, feet, body, cape, elytra): offset em pixels (x direita, y baixo, z costas), rotate em graus (x,y,z) em torno do pivo do osso, scale. Ex.: {\"cape\": {\"offset\": [0,1,0.5]}}; o que o mob segura (main_hand, off_hand) aceita o mesmo. hide_layers / solo_layers: camadas do modelo do pack por numero (o L0, L1... de layers: no dump); layer_alpha: {\"2\": 0.3} multiplica o alfa da camada. hide_attached: [\"main_hand\", \"main_hand:1\"] camadas do que o mob segura/veste (slot ou slot:numero); attached_alpha: {\"main_hand:1\": 0.3}; bind_bones: {\"item\": \"bodyq\"} (ou {\"main_hand:item\": \"bodyq\"}) prende o osso do anexo em outro osso do mob, no lugar da ligacao do pack.",
  "debug": true,
  "hide": [],
  "hide_bones": [],
  "hide_geometry": [],
  "push_bones": {},
  "rotate_bones": {},
  "move_bones": {},
  "dump": [],
  "dump_vars": [],
  "dump_bones": [],
  "held_anchor": "auto",
  "trace_rider": false,
  "worn": {},
  "materials": {},
  "bone_materials": {},
  "solo_bones": [],
  "layer_tint": {},
  "bone_tint": {},
  "solid": {},
  "dump_draw": false,
  "dump_every": 20,
  "hide_layers": [],
  "solo_layers": [],
  "layer_alpha": {},
  "hide_attached": [],
  "attached_alpha": {},
  "bind_bones": {},
  "queries": {}
}
"#;

struct State {
    path: PathBuf,
    checked: Option<Instant>,
    stamp: Option<(SystemTime, u64)>,
    cur: Arc<Settings>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn file_path() -> PathBuf {
    match std::env::var_os("HORIZON_BEDROCK_DEBUG_FILE").filter(|p| !p.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => horizon_client_render::logging::game_home().join("bedrock-debug.json"),
    }
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// The settings now in force; the file is looked at once a second.
pub fn settings() -> Arc<Settings> {
    let mut guard = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    let first = guard.is_none();
    let st = guard.get_or_insert_with(|| State {
        path: file_path(),
        checked: None,
        stamp: None,
        cur: Arc::new(Settings::default().with_env()),
    });
    if st.checked.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
        return st.cur.clone();
    }
    st.checked = Some(Instant::now());
    if first && st.cur.debug {
        log::info!("bedrock: engine build {}", super::bedrock::ENGINE_BUILD);
        if !st.path.exists() && std::fs::write(&st.path, TEMPLATE).is_ok() {
            log::info!("bedrock-debug: template written to {}", st.path.display());
        }
    }
    let now = stamp(&st.path);
    if now == st.stamp && !first {
        return st.cur.clone();
    }
    let had_file = st.stamp.is_some();
    st.stamp = now;
    match now {
        None => {
            if had_file {
                log::info!("bedrock-debug: {} gone, back to the environment", st.path.display());
                st.cur = Arc::new(Settings::default().with_env());
            }
        }
        Some(_) => match std::fs::read_to_string(&st.path)
            .map_err(|e| e.to_string())
            .and_then(|t| Settings::parse(&t))
        {
            Ok(s) => {
                let s = s.with_env();
                log::info!(
                    "bedrock-debug: loaded {}: debug={} hide={:?} dump={:?} dump_vars={:?} held_anchor={:?} materials={:?} queries={:?}",
                    st.path.display(),
                    s.debug,
                    s.hide,
                    s.dump,
                    s.dump_vars,
                    s.held_anchor,
                    s.materials.keys().collect::<Vec<_>>(),
                    s.queries,
                );
                st.cur = Arc::new(s);
            }
            Err(e) => log::warn!(
                "bedrock-debug: {} not applied, keeping the last one: {e}",
                st.path.display()
            ),
        },
    }
    st.cur.clone()
}

/// One line of the `trace_rider` trace, in `rider-trace.log` beside the game's saves (emptied at
/// the first line of a run), stamped with the milliseconds since the first one. Not the game's
/// log: a line every frame would flood it.
pub fn trace(line: &str) {
    use std::io::Write;
    static FILE: Mutex<Option<(std::fs::File, Instant)>> = Mutex::new(None);
    let mut guard = FILE.lock().unwrap_or_else(PoisonError::into_inner);
    if guard.is_none() {
        let logs = horizon_client_render::logging::game_home().join("logs");
        let path = if logs.is_dir() { logs } else { horizon_client_render::logging::game_home() }
            .join("rider-trace.log");
        match std::fs::File::create(&path) {
            Ok(f) => {
                log::info!("bedrock-debug: trace_rider writes {}", path.display());
                *guard = Some((f, Instant::now()));
            }
            Err(_) => return,
        }
    }
    if let Some((f, start)) = guard.as_mut() {
        let _ = writeln!(f, "t={} {line}", start.elapsed().as_millis());
    }
}

static TRACED_MOUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);

/// The mob the player rides now (for `trace_rider`'s bone lines).
pub fn set_traced_mount(id: Option<u32>) {
    TRACED_MOUNT.store(id.unwrap_or(u32::MAX), std::sync::atomic::Ordering::Relaxed);
}

pub fn is_traced_mount(id: u32) -> bool {
    TRACED_MOUNT.load(std::sync::atomic::Ordering::Relaxed) == id
}

pub fn debug() -> bool {
    settings().debug
}

/// A material's flags with the file's patches on them.
pub fn flags(materials: &horizon_bedrock::material::Materials, name: &str) -> MaterialFlags {
    settings().flags(materials.flags(name), name)
}

/// A bone's flags: its material's, with the file's patches by material and by bone.
pub fn bone_flags(
    materials: &horizon_bedrock::material::Materials,
    material: &str,
    bone: &str,
) -> MaterialFlags {
    let s = settings();
    s.bone_flags(s.flags(materials.flags(material), material), bone)
}

/// What a query is fixed to, if the file says.
pub fn query(name: &str) -> Option<f32> {
    let s = settings();
    if s.queries.is_empty() {
        return None;
    }
    s.queries.get(name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_material_patch_sets_and_clears_flags() {
        let s = Settings::parse(
            r#"{"materials":{"Oreville_X":{"additive":false,"blend":true,"stencil_write":0},"*":{"emissive":true}}}"#,
        )
        .unwrap();
        let base = MaterialFlags {
            additive: true,
            stencil_write: 2,
            ..Default::default()
        };
        let f = s.flags(base, "oreville_x");
        assert!(!f.additive && f.blend && f.emissive && f.stencil_write == 0);
        let g = s.flags(base, "other");
        assert!(g.additive && g.emissive && g.stencil_write == 2);
    }

    #[test]
    fn a_misspelt_key_is_an_error_not_a_silent_no_op() {
        assert!(Settings::parse(r#"{"materials":{"a":{"additve":true}}}"#).is_err());
        assert!(Settings::parse(r#"{"hid":[]}"#).is_err());
    }

    #[test]
    fn hide_dump_and_queries_read_as_written() {
        let s = Settings::parse(
            r#"{"hide":["crv"],"dump":["Spider"],"queries":{"query.is_powered":1}}"#,
        )
        .unwrap()
        .with_env();
        assert!(s.hides("entity/bedrock/x/crv") && !s.hides("entity/bedrock/x/bkj"));
        assert!(s.dumps("minecraft:cave_spider"));
        assert_eq!(s.queries.get("is_powered"), Some(&1.0));
    }

    #[test]
    fn bone_tools_patch_solo_tint_and_solid() {
        use horizon_bedrock::material::MaterialFlags;
        let s = Settings::parse(
            r#"{"bone_materials":{"2dh9":{"blend":true,"double_sided":false}},
                "solo_bones":["armor","2dh9"],
                "layer_tint":{"cmj":[1,1,1,0.5]},
                "bone_tint":{"2DH9":[1,1,1,0.5],"2dh923":[1,0.5,1,1]},
                "solid":{"cmj":false},
                "dump_draw":true,"dump_every":2}"#,
        )
        .unwrap();
        let f = s.bone_flags(MaterialFlags::default(), "2DH923d6#cube1");
        assert!(f.blend && !f.double_sided);
        assert!(!s.bone_flags(MaterialFlags::default(), "body").blend);
        assert!(!s.solo_hides("armor") && !s.solo_hides("2dh923") && s.solo_hides("body"));
        assert_eq!(s.tint_for("textures/oreville/ans/cmj"), Some([1.0, 1.0, 1.0, 0.5]));
        assert_eq!(s.tint_for("textures/oreville/ans/cmk"), None);
        assert_eq!(s.bone_tint_for("2dh923#cube0"), [1.0, 0.5, 1.0, 0.5]);
        assert_eq!(s.bone_tint_for("body"), [1.0; 4]);
        assert_eq!(s.solid_for("textures/oreville/ans/cmj"), Some(false));
        assert!(s.dump_draw && !Settings::default().solo_hides("body"));
        assert!((s.dump_every - 2.0).abs() < 1e-6);
    }

    #[test]
    fn blank_names_hide_nothing() {
        let s = Settings::parse(
            r#"{"hide_bones": [""], "hide_attached": [" "], "solo_bones": [""], "dump": [""]}"#,
        )
        .unwrap();
        assert!(!s.hides_bone("gml6j"));
        assert!(!s.hides_attached("main_hand", 0));
        assert!(!s.dumps("minecraft:sulfur_cube"));
    }

    #[test]
    fn layers_attached_and_binds_by_number() {
        let s = Settings::parse(
            r#"{"hide_layers":[1],"layer_alpha":{"2":0.25},
                "hide_attached":["main_hand:1","Head"],
                "attached_alpha":{"main_hand":0.5,"main_hand:0":0.1},
                "bind_bones":{"item":"bodyq","off_hand:item":"root"}}"#,
        )
        .unwrap();
        assert!(s.hides_layer(1) && !s.hides_layer(0));
        assert!(Settings::parse(r#"{"solo_layers":[3]}"#).unwrap().hides_layer(0));
        assert_eq!(s.layer_alpha_for(2), 0.25);
        assert_eq!(s.layer_alpha_for(0), 1.0);
        assert!(s.hides_attached("main_hand", 1) && !s.hides_attached("main_hand", 0));
        assert!(s.hides_attached("head", 4));
        assert_eq!(s.attached_alpha_for("main_hand", 0), 0.1);
        assert_eq!(s.attached_alpha_for("main_hand", 2), 0.5);
        assert_eq!(s.bind_for("main_hand", "item"), Some("bodyq"));
        assert_eq!(s.bind_for("off_hand", "item"), Some("root"));
        assert_eq!(s.bind_for("main_hand", "x"), None);
    }

    #[test]
    fn worn_shifts_by_slot() {
        let s = Settings::parse(
            r#"{"worn":{"Torso":{"offset":[0,16,0]},"cape":{"rotate":[0,0,90],"scale":2}}}"#,
        )
        .unwrap();
        let moved = s.worn_matrix("chest").transform_point3(glam::Vec3::ZERO);
        assert!((moved - glam::Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6);
        assert_eq!(s.worn_matrix("torso"), s.worn_matrix("chest"));
        let turned = s.worn_matrix("cape").transform_point3(glam::Vec3::X);
        assert!((turned - glam::Vec3::new(0.0, 2.0, 0.0)).length() < 1e-5);
        assert_eq!(s.worn_matrix("head"), glam::Mat4::IDENTITY);
    }

    #[test]
    fn bones_and_models_hide_by_part_of_the_name() {
        let s = Settings::parse(r#"{"hide_bones":["Hat","sleeve"],"hide_geometry":["zwjvro"]}"#).unwrap();
        assert!(s.hides_bone("rightSleeve") && s.hides_bone("hat") && !s.hides_bone("head"));
        assert!(s.hides_geometry("geometry.oreville_ans.zwjvro") && !s.hides_geometry("geometry.humanoid.custom"));
        assert!(!Settings::default().hides_bone("hat"));
        let s = Settings::parse(r#"{"push_bones":{"Eye":1.5,"eye_l":0.5}}"#).unwrap();
        assert_eq!(s.push_for("eye_l"), 2.0);
        assert_eq!(s.push_for("Eye_R"), 1.5);
        assert_eq!(s.push_for("head"), 0.0);
    }

    #[test]
    fn the_template_parses() {
        assert!(Settings::parse(TEMPLATE).is_ok());
    }
}
