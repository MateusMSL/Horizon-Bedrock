/// `color` (ABGR) times a layer's RGBA.
fn tint(color: u32, rgba: [f32; 4]) -> u32 {
    let ch =
        |shift: u32, f: f32| ((((color >> shift) & 255) as f32) * f.clamp(0.0, 1.0)).round() as u32;
    ch(24, rgba[3]) << 24 | ch(16, rgba[2]) << 16 | ch(8, rgba[1]) << 8 | ch(0, rgba[0])
}
/// The mob drawn the way a Bedrock pack describes it ([`super::bedrock`]): each render
/// controller's model and texture, posed by the pack's animations. False when no pack dresses
/// the mob or none of its textures is in the atlas, and the vanilla model is drawn instead.
/// How much a baby is scaled when a Bedrock pack dresses it: the pack's models are adult-sized
/// and Bedrock's own engine shrinks a baby to half.
const BEDROCK_BABY_SCALE: f32 = 0.5;

/// Blocks a Bedrock layer is lifted off the one under it, per layer (a thirtieth of a pixel).
const BEDROCK_LAYER_BIAS: f32 = 0.002;
/// Blocks what the stencil kept to the outside of every piece worn (the rim and the enchantment's
/// outline of an armour) is set back along the line of sight: behind its own piece, the piece it
/// overlaps and the body (a few pixels), so it shows only past the whole silhouette, where nothing
/// is in front of it. A point moved along its own line of sight stays on the same pixel.
const BEDROCK_REAR_BIAS: f32 = 0.15;
/// Blocks the part of the rim a shine band lights is brought forward of the rest of the rim, to
/// cover it over the band.
const BEDROCK_BAND_LIFT: f32 = 0.01;
/// Blocks a material pass is lifted off the one before it inside one layer: under the hair a
/// pack leaves between two planes of a face (a hundredth of a pixel), so an eye keeps behind the
/// lid a pack placed in front of it.
const BEDROCK_PASS_BIAS: f32 = 0.0002;

/// Drops the faces turned away from the camera (the vertices are camera-relative), as a material
/// that keeps its culling does: the game draws every model double-sided, so a translucent shell
/// would show its far walls over its near ones, twice as opaque as the pack made it.
fn cull_back_faces(out: &mut Vec<Vertex>, start: usize) {
    let mut kept = start;
    for t in (start..out.len().saturating_sub(3)).step_by(4) {
        let facing: f32 = out[t..t + 4]
            .iter()
            .map(|v| {
                glam::Vec3::new(
                    f32::from(v.normal[0]),
                    f32::from(v.normal[1]),
                    f32::from(v.normal[2]),
                )
                .dot(glam::Vec3::from_array(v.pos))
            })
            .sum();
        if facing <= 0.0 {
            out.copy_within(t..t + 4, kept);
            kept += 4;
        }
    }
    out.truncate(kept);
}

/// Lifts a layer `push` blocks toward the camera (the vertices are camera-relative), so it wins
/// the depth test over the layer it shares faces with. Moving every vertex the same way keeps
/// the cubes' edges joined; pushing each face along its own normal opened a crack at every edge
/// through which the layer below showed (the yellow, blue and white dots along armour, mob and
/// tool edges).
fn lift_toward_camera(vertices: &mut [Vertex], push: f32) {
    for v in vertices {
        let p = glam::Vec3::from_array(v.pos);
        let length = p.length();
        if length > 1e-4 {
            v.pos = (p - p / length * push).to_array();
        }
    }
}

/// What a layer that writes the stencil without colour (an armour's shine band) covers on the
/// screen, for the layers that test for its value: for each face of its shape, the four planes
/// through the camera (the origin: the vertices are camera-relative) and the face's edges, their
/// normals pointing in. A point is on a pixel the face covers when it is inside all four. The two
/// sides of a flat shape cover the same pixels and count once.
fn screen_shapes(quads: &[Vertex]) -> Vec<ScreenShape> {
    let mut shapes: Vec<ScreenShape> = Vec::new();
    let mut seen: Vec<glam::Vec3> = Vec::new();
    for q in quads.chunks_exact(4) {
        let p: [glam::Vec3; 4] = std::array::from_fn(|i| glam::Vec3::from_array(q[i].pos));
        // Edge-on faces (the sides of a flat shape) cover no pixel.
        if (p[2] - p[0]).cross(p[3] - p[1]).length() < 1e-5 {
            continue;
        }
        let centre = (p[0] + p[1] + p[2] + p[3]) / 4.0;
        let direction = centre.normalize_or_zero();
        if direction == glam::Vec3::ZERO || seen.iter().any(|d| d.dot(direction) > 0.999_999) {
            continue;
        }
        let mut planes: [glam::Vec3; 4] = std::array::from_fn(|i| p[i].cross(p[(i + 1) % 4]));
        if planes.iter().any(|n| n.dot(centre).abs() < 1e-7) {
            continue;
        }
        for n in &mut planes {
            if n.dot(centre) < 0.0 {
                *n = -*n;
            }
        }
        seen.push(direction);
        // The face's own plane, positive on the camera's side: the mask writes depth too, so
        // what tests for its value shows only in front of it.
        let mut n = (p[1] - p[0]).cross(p[2] - p[0]);
        if n.length_squared() < 1e-12 {
            n = (p[2] - p[0]).cross(p[3] - p[0]);
        }
        let n = n.normalize_or_zero();
        let mut face = n.extend(-n.dot(p[0]));
        if face.w < 0.0 {
            face = -face;
        }
        shapes.push((planes, face));
    }
    shapes
}

/// Cuts the quads from `start` on to the pixels the shapes cover (`screen_shapes`): what a
/// stencil test for the shape's value kept. Each quad's part inside a shape is laid out as
/// triangles (a quad of three distinct corners and a repeated one), its attributes interpolated.
/// A mask face's screen region (`screen_shapes`): the four planes through the camera and its
/// edges, and the face's own plane (xyz normal, w offset; positive on the camera's side).
type ScreenShape = ([glam::Vec3; 4], glam::Vec4);

/// How far behind a mask's face a piece still shows (blocks): a piece on the face itself (an
/// armour's shine, drawn on the band's own surface) is not cut.
const MASK_DEPTH_SLACK: f32 = 0.002;

fn clip_to_screen_shapes(vertices: &mut Vec<Vertex>, start: usize, shapes: &[ScreenShape]) {
    fn mix(a: &Vertex, b: &Vertex, t: f32) -> Vertex {
        let mut v = *a;
        v.pos = (glam::Vec3::from_array(a.pos).lerp(glam::Vec3::from_array(b.pos), t)).to_array();
        for i in 0..2 {
            v.uv[i] = a.uv[i] + (b.uv[i] - a.uv[i]) * t;
            v.glint_uv[i] = a.glint_uv[i] + (b.glint_uv[i] - a.glint_uv[i]) * t;
        }
        v
    }
    let tail = vertices.split_off(start);
    for shape in shapes {
        for quad in tail.chunks_exact(4) {
            let mut poly: Vec<Vertex> = quad.to_vec();
            // The four edge planes through the camera, then the face's own plane: what is
            // behind the mask fails its depth (Bedrock's masks write depth, colour off).
            let planes = shape.0.map(|n| n.extend(0.0)).into_iter().chain(std::iter::once(
                shape.1 + glam::Vec4::new(0.0, 0.0, 0.0, MASK_DEPTH_SLACK),
            ));
            for n in planes {
                let mut next: Vec<Vertex> = Vec::with_capacity(poly.len() + 1);
                for i in 0..poly.len() {
                    let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
                    let da = n.truncate().dot(glam::Vec3::from_array(a.pos)) + n.w;
                    let db = n.truncate().dot(glam::Vec3::from_array(b.pos)) + n.w;
                    if da >= 0.0 {
                        next.push(*a);
                    }
                    if (da >= 0.0) != (db >= 0.0) {
                        next.push(mix(a, b, da / (da - db)));
                    }
                }
                poly = next;
                if poly.len() < 3 {
                    break;
                }
            }
            if poly.len() < 3 {
                continue;
            }
            for i in 1..poly.len() - 1 {
                vertices.extend_from_slice(&[poly[0], poly[i], poly[i + 1], poly[i + 1]]);
            }
        }
    }
}

/// `HORIZON_BEDROCK_DEBUG=1`: the first draw of each Bedrock-dressed entity is logged, with what
/// each layer drew and where the model's lowest point is (`feet` is 1.501 when it stands on the
/// mob's feet, the same as a vanilla model).
fn bedrock_debug() -> bool {
    super::bedrock_debug::debug()
}

/// `HORIZON_BEDROCK_HIDE=crv,bkj`: layers whose texture ends with one of these names are not
/// drawn, to tell which layer of a model makes what is seen.
fn hidden_layer(texture: &str) -> bool {
    super::bedrock_debug::settings().hides(texture)
}

pub(super) fn log_bedrock_once(key: String, line: impl FnOnce() -> String) {
    static SEEN: std::sync::Mutex<Option<std::collections::HashSet<String>>> =
        std::sync::Mutex::new(None);
    let mut seen = SEEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if seen.get_or_insert_with(Default::default).insert(key) {
        log::info!("bedrock: {}", line());
    }
}

/// The scale a pack gives the mob along each axis: `scale` (halved for a baby) times `scalex/y/z`.
fn bedrock_scale(frame: &super::bedrock::MobFrame, m: &Mob, alpha: f32) -> glam::Vec3 {
    let s = frame.scale * if m.is_baby() { BEDROCK_BABY_SCALE } else { 1.0 };
    let mut axes = glam::Vec3::from_array(frame.scale_axes) * s;
    // `SlimeRenderer.scale`: the body squashes on landing and stretches in the air. A pack's
    // own hop (when it plays one) rides on top of this.
    if matches!(m.species(), Species::Slime | Species::MagmaCube) {
        let size = m.extra.size.max(1) as f32;
        let h = 1.0 / (m.anim.lerp(0, alpha) / (size * 0.5 + 1.0) + 1.0);
        axes *= glam::Vec3::new(h, 1.0 / h, h);
    }
    axes
}

/// What the pack's body of an avatar did in its last frame: the root transform and the body
/// bone's posed and rest matrices, for what the game still draws by itself on that body (the
/// cape and the wings), so these ride the pack's pose and not the game's own.
#[derive(Clone, Copy)]
struct WornBody {
    base: glam::Mat4,
    bones: [(&'static str, Option<(glam::Mat4, glam::Mat4)>); 1],
    /// The pack dresses the chest slot with an attachable of its own.
    torso_dressed: bool,
}

type WornFrames = std::collections::HashMap<u32, (std::time::Instant, WornBody)>;
fn worn_frames() -> std::sync::MutexGuard<'static, WornFrames> {
    static MAP: std::sync::OnceLock<std::sync::Mutex<WornFrames>> = std::sync::OnceLock::new();
    MAP.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn worn_record(m: &Mob, body: Option<WornBody>) {
    let mut frames = worn_frames();
    match body {
        Some(b) => {
            frames.insert(m.id, (std::time::Instant::now(), b));
        }
        None => {
            frames.remove(&m.id);
        }
    }
    // Mobs not drawn for a while are forgotten.
    if frames.len() > 64 {
        frames.retain(|_, (at, _)| at.elapsed() < std::time::Duration::from_secs(5));
    }
}
fn worn_body(m: &Mob) -> Option<WornBody> {
    let frames = worn_frames();
    let (at, body) = frames.get(&m.id)?;
    (at.elapsed() < std::time::Duration::from_millis(250)).then_some(*body)
}
/// The transform for what the game draws on an avatar's body (cape, wings): the pack's body
/// pose when the pack dressed the avatar this frame, `base` (the game's own) otherwise.
/// `slot` names the debug adjustment (`worn` in `bedrock-debug.json`).
pub fn worn_base(m: &Mob, game_base: glam::Mat4, slot: &str) -> glam::Mat4 {
    let Some(body) = worn_body(m) else {
        return game_base;
    };
    let Some((now, rest)) = body.bones[0].1 else {
        return game_base;
    };
    let moved = body.base * now * super::bedrock_debug::settings().worn_matrix(slot) * rest.inverse();
    if super::bedrock_debug::settings().dumps(&m.kind.id) {
        log_worn(m, slot, game_base, body, now, rest, moved);
    }
    moved
}
/// Whether a log line keyed `key` is due: the first time, then once a second.
fn once_a_second(key: String) -> bool {
    static LAST: std::sync::Mutex<Option<std::collections::HashMap<String, std::time::Instant>>> =
        std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let at = last
        .get_or_insert_with(Default::default)
        .entry(key)
        .or_insert_with(|| std::time::Instant::now() - std::time::Duration::from_secs(5));
    if at.elapsed() < std::time::Duration::from_secs(1) {
        return false;
    }
    *at = std::time::Instant::now();
    true
}
/// `bedrock-worn`: once a second per slot, where the game's own base, the pack's body bone and
/// the transform that carries the cape or wings are (positions in pixels from the model's
/// origin, turns in degrees x, y, z).
fn log_worn(
    m: &Mob,
    slot: &str,
    game_base: glam::Mat4,
    body: WornBody,
    now: glam::Mat4,
    rest: glam::Mat4,
    moved: glam::Mat4,
) {
    if !once_a_second(format!("{}/{slot}", m.id)) {
        return;
    }
    let pos = |t: glam::Mat4| {
        let p = t.w_axis.truncate() * 16.0;
        format!("[{:.1}, {:.1}, {:.1}]", p.x, p.y, p.z)
    };
    let turn = |t: glam::Mat4| {
        let (_, rotation, _) = t.to_scale_rotation_translation();
        let (x, y, z) = rotation.to_euler(glam::EulerRot::XYZ);
        format!("[{:.0}, {:.0}, {:.0}]", x.to_degrees(), y.to_degrees(), z.to_degrees())
    };
    log::info!(
        "bedrock-worn {}#{} {slot}: gliding={} crouching={} torso_dressed={} game base at {} turn {}; pack base at {} turn {}; body now at {} turn {}, rest at {}; carried at {} turn {}",
        m.kind.id,
        m.id,
        m.extra.fall_fly_ticks > 0,
        m.extra.crouching,
        body.torso_dressed,
        pos(game_base),
        turn(game_base),
        pos(body.base),
        turn(body.base),
        pos(now),
        turn(now),
        pos(rest),
        pos(moved),
        turn(moved),
    );
}
/// Whether the pack's body poses what the game draws on it (so the game's own sneak and
/// lean must not be added on top).
pub fn worn_follows_pack(m: &Mob) -> bool {
    worn_body(m).is_some_and(|b| b.bones[0].1.is_some())
}
/// Whether the pack dresses the chest slot itself (an elytra attachable): the game's own
/// wings are not drawn over it.
pub fn pack_dresses_torso(m: &Mob) -> bool {
    worn_body(m).is_some_and(|b| b.torso_dressed)
}

/// Where the mob is from the camera for Bedrock's camera queries (`rotation_to_camera`,
/// `distance_from_camera`), from a model base (`base_transform`, whose origin is 1.501 blocks
/// above the feet): the entity's position, its feet, or a player's eyes (Bedrock places a player
/// there, and packs add the eyes' height back). From the model's origin, a pack aiming a glow
/// at the camera (a spider's eyes) saw the camera a block and a half too low.
fn feet_from_camera(m: &Mob, base: glam::Mat4) -> [f32; 3] {
    let feet = base.transform_point3(glam::Vec3::new(0.0, 1.501, 0.0));
    let eyes = if avatar(m) {
        if m.extra.crouching || m.extra.sneaking { 1.12 } else { 1.62 }
    } else {
        0.0
    };
    (feet + glam::Vec3::new(0.0, eyes, 0.0)).to_array()
}

fn render_bedrock(
    m: &Mob,
    alpha: f32,
    base: glam::Mat4,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) -> bool {
    let bedrock = data
        .bedrock
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if bedrock.is_empty() {
        return false;
    }
    let base = pack_model_base(m, alpha, base);
    let Some(frame) = bedrock.frame(m, alpha, feet_from_camera(m, base)) else {
        return false;
    };
    // A pack whose own animation lays the body down on death replaces the game's tilt: the
    // flip `base_transform` turned about the entity's feet is turned back, in the space it was
    // made in (the frame that follows it in the chain, which holds the model's 1.501 drop).
    let mut lying = None;
    // The dragon is never laid down (`EnderDragonRenderer` has no death flip).
    let base = if m.death_time > 0
        && !super::creaking::tearing_down(m)
        && &*m.kind.id != "minecraft:ender_dragon"
        && frame.pose.bones.values().any(|b| {
            b.rotation
                .iter()
                .chain(&b.rotation_entity)
                .any(|r| r.abs() >= 45.0)
        }) {
        let f = ((m.death_time as f32 + alpha - 1.0) / 20.0 * 1.6)
            .sqrt()
            .min(1.0);
        // Where the mob's feet are, to lay the body down on them below.
        lying = Some((f, base.transform_point3(glam::Vec3::new(0.0, 1.501, 0.0))));
        let after = species_rotations(m, alpha)
            * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0))
            * species_scale(m, alpha)
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.501, 0.0));
        base * after.inverse()
            * glam::Mat4::from_rotation_z(-f * flip_degrees(m).to_radians())
            * after
    } else {
        base
    };
    let scale = bedrock_scale(&frame, m, alpha);
    // Scaled about the feet: a model's ground is 1.501 blocks below its origin. The
    // first-person view model is scaled about the eye instead (the camera, `eye_height` above
    // the feet), so its eye stays at the camera: Bedrock's vanilla player (`scale` 0.9375)
    // holds the item about 0.1 block higher than one scaled about the feet, the hand at the
    // bottom of the screen as in the game.
    let pivot = match FP_EYE.with(std::cell::Cell::get) {
        Some(eye) if m.extra.first_person => 1.501 - eye,
        _ => 1.501,
    };
    let base = if (scale - glam::Vec3::ONE).abs().max_element() > 1e-4 {
        let feet = glam::Mat4::from_translation(glam::Vec3::new(0.0, pivot, 0.0));
        base * feet * glam::Mat4::from_scale(scale) * feet.inverse()
    } else {
        base
    };
    let (color, overlay) = (body_color(m, alpha), body_overlay(m, alpha));
    let mut poses = Poses::pooled();
    let mut drew = false;
    let first_vertex = out.len();
    let mut layers_drawn = 0usize;
    // Stencil references (as bits) the layers drawn so far wrote.
    let mut stenciled = 0u32;
    // The holder's bones, for what the mob holds or wears.
    let mut holder: std::collections::HashMap<String, (glam::Mat4, glam::Mat4)> =
        std::collections::HashMap::new();
    for (layer_no, layer) in frame.layers.iter().enumerate() {
        // The debug file's `hide_layers` / `solo_layers`, by the number the dump lists.
        if super::bedrock_debug::settings().hides_layer(layer_no) {
            continue;
        }
        // A layer the controller turned off (armour not worn, say) or with no bone shown.
        if layer.layer.color[3] <= 0.0
            || !layer
                .built
                .names
                .iter()
                .any(|n| layer.layer.bone_visible(n))
        {
            // Its bones still carry what is held: Bedrock's vanilla first-person player hides
            // every part (`*: false`) while it holds an item, and the attachable still hangs
            // from its hand bone.
            if holder.is_empty() && !frame.attached.is_empty() {
                poses.reset(&layer.built.part);
                super::bedrock::apply_pose(&frame.pose, &layer.built.names, &mut poses);
                if m.extra.first_person {
                    let arms = ["rightarm".to_owned(), "leftarm".to_owned()];
                    for att in &frame.attached {
                        super::bedrock::apply_pose(&att.pose, &arms, &mut poses);
                    }
                }
                bone_matrices(&layer.built.part, &poses, &mut holder);
            }
            continue;
        }
        if hidden_layer(&layer.texture)
            || super::bedrock_debug::settings().hides_geometry(&layer.layer.geometry)
        {
            continue;
        }
        let found = atlas(&layer.texture);
        let Some(uv) = found.or_else(|| layer.fallback.as_deref().and_then(atlas)) else {
            if bedrock_debug() {
                log_bedrock_once(format!("{}/{}", m.kind.id, layer.texture), || {
                    format!(
                        "{}: texture {} is in neither the pack nor the game; layer skipped",
                        m.kind.id, layer.texture
                    )
                });
            }
            continue;
        };
        // Bones the pack gives another material (a glowing eye, a scrolling mantle) draw in
        // their own pass, with that material's flags.
        let groups = material_groups(
            &layer.layer,
            &layer.built.names,
            &bedrock.bundle().materials,
        );
        let layers_before = layers_drawn;
        let mut drew_here = false;
        for (gi, group) in groups.iter().enumerate() {
            poses.reset(&layer.built.part);
            super::bedrock::apply_pose(&frame.pose, &layer.built.names, &mut poses);
            // First person: the attachable's own animations pose the holder's arm (Bedrock
            // draws the attachable in the holder's skeleton), so the arm, its sleeve and the
            // item bone under it move together, about the arm's pivot.
            if m.extra.first_person {
                // Both arms, whatever the slot: a main-hand item's animations pose the other
                // arm too (flint and steel's left hand bringing the steel).
                let arms = ["rightarm".to_owned(), "leftarm".to_owned()];
                for att in &frame.attached {
                    super::bedrock::apply_pose(&att.pose, &arms, &mut poses);
                }
            }
            for name in &layer.built.names {
                let debug = super::bedrock_debug::settings();
                // A hidden bone hides its own cubes only: its children follow their own rules.
                if !layer.layer.bone_visible(name)
                    || debug.hides_bone(name)
                    || debug.solo_hides(name)
                {
                    poses.skip_draw(name);
                }
                // The debug file's `push_bones`: a bone forward of the ones under it (-z).
                let push = debug.push_for(name);
                if push != 0.0 {
                    poses.get(name)[2] -= push;
                }
                // A glide's pitch is the game's (`-90 - pitch`, upright looking straight up): the
                // pack's own `root` turn by the look pitch would count it twice, lying the body
                // back down (or upside down) on a climb.
                if m.extra.fall_flying
                    && !m.extra.auto_spin_attack
                    && name == "root"
                    && super::bedrock::pack_glides(m.id)
                {
                    poses.get(name)[3] = 0.0;
                }
                // `rotate_bones` / `move_bones`: the orientation of a pose, by hand.
                let (turn, shift) = (debug.rotate_for(name), debug.move_for(name));
                if turn != [0.0; 3] || shift != [0.0; 3] {
                    let r = horizon_bedrock::geometry::rotation_to_java(turn);
                    let p = poses.get(name);
                    p[0] += shift[0];
                    p[1] -= shift[1];
                    p[2] += shift[2];
                    p[3] += r[0];
                    p[4] += r[1];
                    p[5] += r[2];
                }
            }
            if gi == 0 && (!frame.attached.is_empty() || avatar(m)) {
                let mut here = std::collections::HashMap::new();
                bone_matrices(&layer.built.part, &poses, &mut here);
                for (name, m) in here {
                    holder.entry(name).or_insert(m);
                }
            }
            if groups.len() > 1 {
                for name in &layer.built.names {
                    if bone_group(&layer.layer, &bedrock.bundle().materials, name) != *group {
                        // Its own cubes only: a child with another material still draws, in
                        // its own pass (a slime's outer shell hangs under a bone of another look).
                        poses.skip_draw(name);
                    }
                }
            }
            let (mut flags, bone_tint) = *group;
            // With the stencil pass on (`bedrock_stencil`), the stand-ins below see no stencil:
            // the piece draws whole and the pass applies its material's test and write.
            let stencil_flags = flags;
            flags = super::bedrock_stencil::emulated(flags);
            // The debug file's `layer_alpha` below 1 makes the layer a blended one, so the
            // alpha shows on an opaque material too.
            let fade = super::bedrock_debug::settings().layer_alpha_for(layer_no) < 1.0;
            if fade {
                flags.blend = true;
            }
            // The second texture of a render controller is read only by a multitexture
            // material (a mask laid over the first); any other material draws the first alone.
            if layer.layer.secondary && !flags.multitexture {
                continue;
            }
            // A mask that writes no colour draws nothing to see (what it stencils is not
            // tracked past the layers that test it, which show only past a body).
            if flags.no_color {
                continue;
            }
            // An additive layer tinted black adds nothing: a pack that fades a glow out by
            // overlaying black on an additive material means it not to show.
            if flags.additive && layer.layer.tint()[..3].iter().all(|c| *c < 0.02) {
                continue;
            }
            // An emissive material lights only the texels the texture leaves less than opaque
            // (its `_glow` sprite): the rest of the model keeps the world's light.
            // (A layer that adds light needs no second draw: it already is all glow.)
            let glow = if flags.emissive
                && !flags.additive
                && !layer.layer.ignore_lighting
                && found.is_some()
            {
                atlas(&format!("{}_glow", layer.texture))
            } else {
                None
            };
            // Its alpha is how much a texel glows, not how see-through the model is: the model
            // is drawn from the texture with those texels opaque.
            // (A material that also blends or adds, an eye's soft glow, keeps its alpha: there it
            // is how see-through the glow is.)
            let debug = super::bedrock_debug::settings();
            let solid = debug
                .solid_for(&layer.texture)
                .unwrap_or_else(|| ignores_alpha(&flags) && !fade);
            let uv = if solid && found.is_some() {
                atlas(&format!("{}_solid", layer.texture)).unwrap_or(uv)
            } else {
                uv
            };
            let mut layer_tint = layer.layer.tint();
            if let Some(t) = debug.tint_for(&layer.texture) {
                for (c, f) in layer_tint.iter_mut().zip(t) {
                    *c *= f;
                }
            }
            for (c, f) in layer_tint.iter_mut().zip(bone_tint) {
                *c *= f;
            }
            layer_tint[3] *= debug.layer_alpha_for(layer_no);
            if debug.dump_draw && debug.dumps(&m.kind.id) {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() / 5);
                let drawn: Vec<&str> = layer
                    .built
                    .names
                    .iter()
                    .filter(|n| {
                        layer.layer.bone_visible(n)
                            && !debug.hides_bone(n)
                            && !debug.solo_hides(n)
                            && (groups.len() < 2
                                || bone_group(&layer.layer, &bedrock.bundle().materials, n)
                                    == *group)
                    })
                    .map(String::as_str)
                    .collect();
                log_bedrock_once(
                    format!("draw/{}/{}/{gi}/{secs}", m.kind.id, layer.texture),
                    || {
                        format!(
                            "bedrock-debug: draw {} geometry {} texture {} uv {:?} material {} group {gi}/{} solid {solid} tint {:?} light {} flags {{blend {}, alpha_test {}, double_sided {}, additive {}, emissive {}, no_depth_write {}, uv_anim {}, stencil_write {}, stencil_not_equal {}}} bones {:?}",
                            m.kind.id,
                            layer.layer.geometry,
                            layer.texture,
                            uv,
                            layer.layer.material,
                            groups.len(),
                            layer_tint,
                            if layer.layer.ignore_lighting { "full" } else { "world" },
                            flags.blend,
                            flags.alpha_test,
                            flags.double_sided,
                            flags.additive,
                            flags.emissive,
                            flags.no_depth_write,
                            flags.uv_anim,
                            flags.stencil_write,
                            flags.stencil_not_equal,
                            drawn,
                        )
                    },
                );
            }
            let drawn_light = if layer.layer.ignore_lighting {
                FULL_BRIGHT
            } else {
                light
            };
            let start = out.len();
            if flags.energy_swirl {
                // The game's energy swirl: additive, the texture scrolling over the model.
                let [u0, v0, u1, v1] = uv;
                let (du, dv) = layer.layer.uv_anim.map_or((0.0, 0.0), |(o, _)| {
                    (o[0].rem_euclid(1.0), o[1].rem_euclid(1.0))
                });
                draw(
                    &layer.built.part,
                    base,
                    &poses,
                    [du, dv, 1.0 + du, 1.0 + dv],
                    crate::breaking::stage_light([u1 - u0, v1 - v0]),
                    crate::breaking::stage_light([u0, v0]),
                    crate::gfx::ENERGY_SWIRL_OVERLAY,
                    out,
                );
                for vertex in &mut out[start..] {
                    vertex.normal = [0; 4];
                }
                drew |= out.len() > start;
                continue;
            }
            // A glinting material shows the enchantment's shimmer over its texture.
            let overlay = if flags.glint { ARMOR_GLINT } else { overlay };
            draw(
                &layer.built.part,
                base,
                &poses,
                uv,
                tint(color, layer_tint),
                drawn_light,
                overlay,
                out,
            );
            if flags.invert_culling {
                // The faces the game culls are the front ones: turn every quad around.
                for quad in out[start..].chunks_exact_mut(4) {
                    quad.swap(1, 3);
                    for v in quad {
                        v.normal = v.normal.map(i8::saturating_neg);
                    }
                }
            }
            // A single-sided material draws only the faces that look at the camera: a layer with
            // an inside-out cube (the sulfur cube's inverted core, under its real one) would
            // otherwise show that cube's near walls over the model, tied in depth with the real
            // faces.
            if !flags.double_sided
                && (flags.blend || (!flags.additive && bedrock.has_inside_out(&layer.layer.geometry)))
            {
                cull_back_faces(out, start);
            }
            if flags.uv_anim {
                animate_uv(&mut out[start..], uv, layer.layer.uv_anim);
            }
            if flags.stencil_not_equal != 0 && stenciled & (1 << flags.stencil_not_equal) != 0 {
                // Inverted hull: only the faces turned away from the camera stay, so the hull
                // hides behind the body and shows past its silhouette, as the stencil leaves it.
                let mut kept = start;
                for t in (start..out.len().saturating_sub(3)).step_by(4) {
                    let facing: f32 = out[t..t + 4]
                        .iter()
                        .map(|v| {
                            let n = glam::Vec3::new(
                                f32::from(v.normal[0]),
                                f32::from(v.normal[1]),
                                f32::from(v.normal[2]),
                            );
                            n.dot(glam::Vec3::from_array(v.pos))
                        })
                        .sum();
                    // The edges of a flat cube (a wing) have no texture area: they only
                    // smear one texel across the rim.
                    let uv = |i: usize| out[t + i].uv;
                    let area = (0..4)
                        .map(|i| {
                            let (a, b) = (uv(i), uv((i + 1) % 4));
                            a[0] * b[1] - b[0] * a[1]
                        })
                        .sum::<f32>()
                        .abs();
                    if facing >= 0.0 && area > 1e-9 {
                        out.copy_within(t..t + 4, kept);
                        kept += 4;
                    }
                }
                out.truncate(kept);
            }
            if flags.stencil_write != 0 && out.len() > start {
                stenciled |= 1 << flags.stencil_write;
            }
            // Layers share faces with the ones below (eyes over a head, a coat over a body): each
            // is pushed out along its faces by a hair more than the last so the two never tie in
            // the depth test, which shows as flicker.
            if layers_before > 0 || gi > 0 {
                let push = BEDROCK_LAYER_BIAS * layers_before as f32 + BEDROCK_PASS_BIAS * gi as f32;
                lift_toward_camera(&mut out[start..], push);
            }
            drew_here |= out.len() > start;
            if layer.layer.flat_lighting {
                flatten_lighting(&mut out[start..]);
            }
            tag_bedrock_material(
                &mut out[start..],
                &flags,
                layer.layer.secondary,
                layer.layer.color[3] < 1.0,
            );
            super::bedrock_stencil::hand_over(&stencil_flags, out, start);
            if bedrock_debug() && gi == 0 {
                let id = m.kind.id.clone();
                let carried = m.extra.carried.is_some();
                log_bedrock_once(
                    format!("{id}/bones/{}/{carried}", layer.layer.geometry),
                    || {
                        let shown: Vec<&str> = layer
                            .built
                            .names
                            .iter()
                            .filter(|n| layer.layer.bone_visible(n))
                            .map(String::as_str)
                            .collect();
                        format!(
                            "bedrock-debug: bones {id} {} (carried {carried}): {} | shown: {}",
                            layer.layer.geometry,
                            layer.built.names.join(", "),
                            shown.join(", ")
                        )
                    },
                );
            }
            if bedrock_debug()
                && let Some(v) = out[start..].first()
            {
                let (id, bits) = (m.kind.id.clone(), v.layer);
                log_bedrock_once(format!("{id}/tag/{}/{}", layer.layer.geometry, layer.texture), || {
                    format!(
                        "{id}: layer {} {} draws as oit={} additive={} no_depth_write={} no_depth_test={} blend={} emissive={} stencil_write={} stencil_not_equal={} color_alpha={}",
                        layer.layer.geometry,
                        layer.texture,
                        bits & crate::gfx::OIT_ENTITY != 0,
                        bits & crate::gfx::ADDITIVE != 0,
                        bits & crate::gfx::NO_DEPTH_WRITE != 0,
                        bits & crate::gfx::NO_DEPTH_TEST != 0,
                        flags.blend,
                        flags.emissive,
                        flags.stencil_write,
                        flags.stencil_not_equal,
                        layer.layer.color[3],
                    )
                });
            }
            if let Some(glow) = glow {
                let from = out.len();
                draw(
                    &layer.built.part,
                    base,
                    &poses,
                    glow,
                    tint(color, layer.layer.tint()),
                    FULL_BRIGHT,
                    overlay,
                    out,
                );
                // Over its own faces by a hair, under the planes a pack puts that close in front.
                let push = BEDROCK_LAYER_BIAS * layers_before as f32
                    + BEDROCK_PASS_BIAS * (gi + 1) as f32;
                lift_toward_camera(&mut out[from..], push);
                mark_oit(&mut out[from..]);
            }
            if bedrock_debug() {
                let inverse = base.inverse();
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for v in &out[start..] {
                    let p = inverse.transform_point3(glam::Vec3::from_array(v.pos));
                    lo = lo.min(p.y);
                    hi = hi.max(p.y);
                }
                let vertices = out.len() - start;
                let (id, baby) = (m.kind.id.clone(), m.is_baby());
                log_bedrock_once(format!("{id}/{}/{baby}", layer.layer.geometry), || {
                    format!(
                        "{id} baby={baby} scale={scale}: geometry {} texture {} ({}, rect {uv:?}) vertices {vertices} \
                     top {lo:.3} feet {hi:.3}",
                        layer.layer.geometry,
                        layer.texture,
                        if found.is_some() {
                            "pack"
                        } else {
                            "game's own"
                        },
                    )
                });
            }
            // Only what put vertices on screen counts: a pack whose layers all draw nothing leaves
            // the mob to the vanilla model, not invisible.
            drew |= out.len() > start;
        }
        layers_drawn += usize::from(drew_here);
    }
    if m.extra.first_person {
        // Bedrock's first person: the holder's model posed by its first-person animations
        // (its body turned with the look, its arm before the camera), drawn as its first-person
        // render controllers show it (the arms). What the attachable's own animations do to the
        // holder's arm and item bones (a sword's first-person swing) moves the item bone from
        // where the holder put it, as Bedrock, which draws the attachable in the holder's
        // skeleton.
        drew = holder.contains_key("rightitem") || holder.contains_key("leftitem");
        if super::bedrock_debug::settings().trace_fp {
            let at = |b: &str| {
                holder
                    .get(b)
                    .map_or([0.0; 3], |(now, _)| now.w_axis.truncate().to_array())
            };
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() % 1_000_000);
            log::info!(
                "fp-trace t={ms} limb={:.3} speed={:.3} arm={:.4?} item={:.4?}",
                m.limb_position,
                m.limb_speed,
                at("rightarm"),
                at("rightitem")
            );
        }
    }
    if avatar(m) {
        worn_record(m, drew.then(|| WornBody {
            base,
            bones: ["body"].map(|b| (b, holder.get(b).copied())),
            torso_dressed: frame
                .attached
                .iter()
                .any(|a| a.slot == "torso" && !a.layers.is_empty()),
        }));
    }
    // Nothing of the body drawn: the mob is the game's, unless the pack still hangs what it
    // holds from its (hidden) bones.
    if !drew && (holder.is_empty() || frame.attached.is_empty()) {
        return false;
    }
    // What the stencil masks that draw no colour (the shine bands) cover on the screen, by the
    // value they write: the layers drawn after them that test for it are cut to it.
    let mut bands: Vec<(u8, ScreenShape)> = Vec::new();
    // What the mob holds and wears follows the holder's bones.
    if super::bedrock_debug::settings().dumps(&m.kind.id) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        log_bedrock_once(format!("attloop/{}/{}/{secs}", m.kind.id, m.id), || {
            let mut keys: Vec<&String> = holder.keys().collect();
            keys.sort();
            format!(
                "bedrock-attloop {}#{}: {} attached {:?}; holder bones {}: {:?}",
                m.kind.id,
                m.id,
                frame.attached.len(),
                frame.attached.iter().map(|a| (a.slot, a.layers.len(), a.binds.clone())).collect::<Vec<_>>(),
                holder.len(),
                keys.iter().take(12).collect::<Vec<_>>()
            )
        });
    }
    for att in &frame.attached {
        let att_start = out.len();
        // A bone the pack scaled to nothing hides what is bound to it (the sulfur cube's pack
        // hides the held block so for the blocks its own core draws).
        if att
            .binds
            .values()
            .any(|to| holder.get(to).is_some_and(|(now, _)| now.determinant().abs() < 1e-12))
        {
            continue;
        }
        let bound_only = !att.binds.is_empty();
        let in_hand = matches!(att.slot, "main_hand" | "off_hand");
        let hand_scale = if in_hand { frame.held.scale } else { 1.0 };
        let base_of = |bone: &str| -> Option<glam::Mat4> {
            // A bone the pack binds follows that bone of the holder; with no bindings at all, a
            // bone follows the holder's bone of its own name (worn armour on the arm it covers).
            let target = match att.binds.get(bone) {
                Some(to) => to.as_str(),
                None if !bound_only => bone,
                None => return None,
            };
            // A worn model with no bone of the holder's name (the elytra's `leftwing` and
            // `rightwing` hang from nothing) follows the bone its slot means (the body), from
            // its top-level bones: else it stays where the game's own frame puts it and does
            // not turn with the body.
            let slot_follow = (!bound_only
                && !in_hand
                && !holder.contains_key(target)
                && matches!(att.slot, "torso" | "head" | "body")
                && att
                    .layers
                    .iter()
                    .any(|l| l.built.part.children.iter().any(|c| c.name == bone)))
            .then(|| super::bedrock::slot_bone(att.slot));
            let (now, rest) = holder.get(slot_follow.unwrap_or(target))?;
            let held = glam::Mat4::from_scale(glam::Vec3::splat(hand_scale));
            // What is held is modelled around its own origin, which the game puts on the
            // hand's bone (`rightitem`, a locator at the grip): the bone's whole pose, pivot
            // and all. Worn pieces are modelled where they sit on the holder (a helmet over
            // the head), so they only follow the bone's change from rest.
            Some(if in_hand && att.binds.contains_key(bone) {
                // The bone's pivot lands on the hand; a bone left at the origin keeps the
                // feet-line anchor.
                let has_parent = !att
                    .layers
                    .iter()
                    .any(|l| l.built.part.children.iter().any(|c| c.name == bone));
                let rest_at = att
                    .layers
                    .iter()
                    .find_map(|l| rest_pivot(&l.built.part, bone))
                    .unwrap_or(glam::Vec3::new(0.0, MODEL_FEET, 0.0));
                let height = (MODEL_FEET - rest_at.y) * 16.0;
                let floor = att
                    .layers
                    .iter()
                    .filter_map(|l| rest_floor(&l.built.part, bone))
                    .reduce(f32::min);
                // First person (`fp_held_anchor`): Bedrock binds the bone to the hand the way
                // a child bone hangs from its parent, so the model keeps its place against the
                // holder's own model and follows the hand's change from rest (`rest` inverted),
                // as worn pieces do. The third-person guess from the pivot height
                // (`held_anchor_of`) shifts the model by the hand's height off its rest, which
                // the first-person arm, turned over before the camera, throws off the screen.
                let fp_anchor = super::bedrock_debug::settings().fp_held_anchor.to_ascii_lowercase();
                if m.extra.first_person && fp_anchor == "holder" {
                    return Some(
                        base * *now
                            * super::bedrock_debug::settings().worn_matrix(att.slot)
                            * held
                            * rest.inverse(),
                    );
                }
                let anchor_kind = match (m.extra.first_person, fp_anchor.as_str()) {
                    (true, "frame") => super::bedrock_debug::HeldAnchor::Frame,
                    (true, "origin") => super::bedrock_debug::HeldAnchor::Origin,
                    (true, "pivot") => super::bedrock_debug::HeldAnchor::Pivot,
                    _ => super::bedrock_debug::settings().held_anchor_of(has_parent, height, floor),
                };
                let pivot = match anchor_kind {
                    super::bedrock_debug::HeldAnchor::Frame => glam::Vec3::ZERO,
                    super::bedrock_debug::HeldAnchor::Origin => glam::Vec3::new(0.0, MODEL_FEET, 0.0),
                    super::bedrock_debug::HeldAnchor::Pivot => rest_at,
                };
                base * held_anchor_at(*now, hand_scale, pivot)
                    * super::bedrock_debug::settings().worn_matrix(att.slot)
            } else {
                // A worn bone modelled around the model's origin (an armour piece whose pivot is
                // about `[0, 0, 0]`, a pixel off at most: the helmet's `[0, -0.2, 0]`) is meant to sit on the pivot of the holder bone it covers: the
                // game puts the bone's pivot there, so its cubes, modelled around the joint,
                // land on the arm, leg or head (arms and legs apart from each other, the head
                // on the neck). One modelled in place (its pivot already the holder's) stays.
                let anchor = if slot_follow.is_none() {
                    att.layers
                        .iter()
                        .find_map(|l| rest_pivot(&l.built.part, bone))
                        .filter(|p| {
                            p.x.abs() < 1.0 / 16.0 && p.z.abs() < 1.0 / 16.0 && (p.y - MODEL_FEET).abs() <= 1.0 / 16.0
                        })
                        .map_or(glam::Mat4::IDENTITY, |p| {
                            glam::Mat4::from_translation(rest.w_axis.truncate() - p)
                        })
                } else {
                    glam::Mat4::IDENTITY
                };
                base * *now
                    * super::bedrock_debug::settings().worn_matrix(att.slot)
                    * held
                    * rest.inverse()
                    * anchor
            })
        };
        if super::bedrock_debug::settings().dumps(&m.kind.id) && in_hand {
            // `bedrock-held`: where the holder's bones sit against what the attachable draws.
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() / 500);
            log_bedrock_once(format!("held/{}/{}/{secs}", m.kind.id, att.slot), || {
                let show = |name: &str| {
                    holder.get(name).map_or("none".to_owned(), |(now, rest)| {
                        let (s, r, t) = now.to_scale_rotation_translation();
                        let (_, rr, rt) = rest.to_scale_rotation_translation();
                        let e = r.to_euler(glam::EulerRot::YXZ);
                        let er = rr.to_euler(glam::EulerRot::YXZ);
                        format!(
                            "now t=[{:.3},{:.3},{:.3}] s=[{:.2},{:.2},{:.2}] yxz=[{:.1},{:.1},{:.1}] rest t=[{:.3},{:.3},{:.3}] yxz=[{:.1},{:.1},{:.1}]",
                            t.x, t.y, t.z, s.x, s.y, s.z, e.0.to_degrees(), e.1.to_degrees(), e.2.to_degrees(),
                            rt.x, rt.y, rt.z, er.0.to_degrees(), er.1.to_degrees(), er.2.to_degrees()
                        )
                    })
                };
                let (_, br, bt) = base.to_scale_rotation_translation();
                let be = br.to_euler(glam::EulerRot::YXZ);
                let anchor = base_of("item").or_else(|| att.binds.keys().next().and_then(|k| base_of(k)));
                format!(
                    "bedrock-held {}#{} {} binds={:?} hand_scale={hand_scale} base t=[{:.2},{:.2},{:.2}] yaw={:.1} | rightitem {} | 3dg4c {} | bodyq {} | bound matrix {:?}",
                    m.kind.id, m.id, att.slot, att.binds, bt.x, bt.y, bt.z, be.0.to_degrees(),
                    show("rightitem"), show("3dg4c"), show("bodyq"),
                    anchor.map(|a| { let (s, r, t) = a.to_scale_rotation_translation(); let e = r.to_euler(glam::EulerRot::YXZ); (t.to_array(), s.to_array(), [e.0.to_degrees(), e.1.to_degrees(), e.2.to_degrees()]) })
                )
            });
        }
        if m.extra.first_person && in_hand && super::bedrock_debug::settings().trace_fp {
            // `fp-attach`: the attachable's bone poses as the first-person frame left them, and
            // where its bound bones land, once a second.
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            log_bedrock_once(format!("fp-attach/{}/{secs}", att.slot), || {
                let mut bones: Vec<_> = att.pose.bones.iter().collect();
                bones.sort_by(|a, b| a.0.cmp(b.0));
                let poses: Vec<String> = bones
                    .iter()
                    .filter(|(_, b)| {
                        b.rotation != [0.0; 3]
                            || b.rotation_entity != [0.0; 3]
                            || b.entity_frame
                            || b.position != [0.0; 3]
                            || b.scale != [1.0; 3]
                    })
                    .map(|(n, b)| {
                        format!(
                            "{n}: rot={:.1?} rot_entity={:.1?} entity_frame={} pos={:.2?} scale={:.2?}",
                            b.rotation, b.rotation_entity, b.entity_frame, b.position, b.scale
                        )
                    })
                    .collect();
                let bound: Vec<String> = att
                    .binds
                    .keys()
                    .filter_map(|k| {
                        base_of(k).map(|a| {
                            let (s, r, t) = a.to_scale_rotation_translation();
                            let e = r.to_euler(glam::EulerRot::YXZ);
                            format!(
                                "{k}: t={:.2?} s={:.2?} yxz=[{:.1},{:.1},{:.1}]",
                                t.to_array(),
                                s.to_array(),
                                e.0.to_degrees(),
                                e.1.to_degrees(),
                                e.2.to_degrees()
                            )
                        })
                    })
                    .collect();
                format!(
                    "fp-attach {} hand_scale={hand_scale} binds={:?} | poses: {} | bound: {}",
                    att.slot,
                    att.binds,
                    poses.join(" ; "),
                    bound.join(" ; ")
                )
            });
        }
        let mut layers_on = 0usize;
        // The stencil values the layers drawn so far wrote, and whether one carried the glint:
        // what a later layer's stencil test (and a glint layer of its own) answers to.
        let mut written: Vec<u8> = Vec::new();
        let mut glinted = false;
        for (layer_no, layer) in att.layers.iter().enumerate() {
            // The debug file's `hide_item_parts`: this item's layers, bones or materials.
            let hidden_parts = super::bedrock_debug::settings().item_parts(&att.item);
            if hidden_parts.iter().any(|p| *p == format!("l{layer_no}")) {
                continue;
            }
            if super::bedrock_debug::settings().hides_attached(att.slot, layer_no) {
                continue;
            }
            let attached_alpha = super::bedrock_debug::settings().attached_alpha_for(att.slot, layer_no);
            let layer_tint_of = |t: [f32; 4]| [t[0], t[1], t[2], t[3] * attached_alpha];
            if layer.layer.color[3] <= 0.0 {
                if bedrock_debug() {
                    log_bedrock_once(
                        format!("{}/{}/{}/alpha", m.kind.id, att.slot, layer.layer.geometry),
                        || {
                            format!(
                                "{}: {} layer {} ({}) skipped: color alpha {}",
                                m.kind.id,
                                att.slot,
                                layer.layer.geometry,
                                layer.texture,
                                layer.layer.color[3]
                            )
                        },
                    );
                }
                continue;
            }
            // The wearer's cape texture carries the elytra's (`WingsLayer.getPlayerElytraTexture`):
            // a pack's plain elytra layer draws it in place of the vanilla elytra texture.
            let cape_wings = (att.slot == "torso"
                && layer.layer.texture.ends_with("textures/models/armor/elytra"))
            .then(|| m.cape.as_deref().and_then(atlas))
            .flatten();
            let Some(uv) = cape_wings
                .or_else(|| atlas(&layer.texture))
                .or_else(|| layer.fallback.as_deref().and_then(atlas))
            else {
                if bedrock_debug() {
                    log_bedrock_once(
                        format!("{}/{}/{}/atlas", m.kind.id, att.slot, layer.layer.geometry),
                        || {
                            format!(
                                "{}: {} layer {} skipped: texture {} (fallback {:?}) is in neither the pack nor the game",
                                m.kind.id,
                                att.slot,
                                layer.layer.geometry,
                                layer.texture,
                                layer.fallback
                            )
                        },
                    );
                }
                continue;
            };
            // Bones the pack gives another material (a torch's flame and halo, a harness's
            // glow) draw in their own pass, with that material's flags: the layer's own
            // material drew a halo as an opaque disk.
            let groups = material_groups(
                &layer.layer,
                &layer.built.names,
                &bedrock.bundle().materials,
            );
            for group in &groups {
                poses.reset(&layer.built.part);
                super::bedrock::apply_pose(&att.pose, &layer.built.names, &mut poses);
                for name in &layer.built.names {
                    // A hidden bone hides its own cubes only: its children follow their own rules
                    // (a pack hides every bone, then shows the ones it names under hidden parents).
                    let part_hidden = !hidden_parts.is_empty() && {
                        let lower = name.to_ascii_lowercase();
                        let material = layer.layer.material_for(name).to_ascii_lowercase();
                        let key = layer
                            .layer
                            .bone_materials
                            .iter()
                            .rev()
                            .find(|(_, m)| m.eq_ignore_ascii_case(&material))
                            .map(|(b, _)| b.to_ascii_lowercase());
                        hidden_parts.iter().any(|p| {
                            lower.contains(p.as_str())
                                || material == *p
                                || key.as_deref() == Some(p.as_str())
                        })
                    };
                    if !layer.layer.bone_visible(name)
                        || super::bedrock_debug::settings().hides_bone(name)
                        || part_hidden
                    {
                        poses.skip_draw(name);
                    }
                    // The debug file's turn and shift for a held piece's bones.
                    let (turn, shift) = super::bedrock_debug::settings()
                        .held_adjust_for(name, m.extra.first_person);
                    if turn != [0.0; 3] || shift != [0.0; 3] {
                        let r = horizon_bedrock::geometry::rotation_to_java(turn);
                        let p = poses.get(name);
                        p[0] += shift[0];
                        p[1] -= shift[1];
                        p[2] += shift[2];
                        p[3] += r[0];
                        p[4] += r[1];
                        p[5] += r[2];
                    }
                }
                if groups.len() > 1 {
                    for name in &layer.built.names {
                        if bone_group(&layer.layer, &bedrock.bundle().materials, name) != *group {
                            // Its own cubes only: a child with another material draws in its pass.
                            poses.skip_draw(name);
                        }
                    }
                }
                // As on the mob's own layers: the stencil pass, when on, draws the stencil.
                let stencil_flags = group.0;
                let mut flags = super::bedrock_stencil::emulated(group.0);
                if attached_alpha < 1.0 {
                    flags.blend = true;
                }
                // A mask that writes no colour (an armour's shine sweep is stencilled through one)
                // draws nothing to see; what shows only inside its shape would show everywhere
                // without it.
                // What draws only where nothing else did (`stencil_equal` at its widest) is drawn
                // behind the piece: depth keeps it to the piece's gaps, where the stencil kept it
                // out of the piece's pixels.
                // The same for a `NotEqual` test that a layer drawn before it fails over its own
                // pixels (the enchantment's outline, kept off the piece that wrote its reference).
                let mask = if flags.stencil_mask == 0 { u8::MAX } else { flags.stencil_mask };
                let kept_off = flags.stencil_not_equal != 0
                    && written
                        .iter()
                        .any(|w| w & mask == flags.stencil_not_equal & mask);
                // A mask that writes a value without colour (the shine band) draws nothing, and what it
                // covers is where a later layer testing for the value shows.
                if flags.no_color && flags.stencil_write != 0 {
                    let from = out.len();
                    draw_attached(
                        &layer.built.part,
                        glam::Mat4::IDENTITY,
                        base,
                        bound_only,
                        &base_of,
                        &poses,
                        uv,
                        tint(color, layer_tint_of(layer.layer.tint())),
                        light,
                        overlay,
                        out,
                    );
                    bands.extend(
                        screen_shapes(&out[from..])
                            .into_iter()
                            .map(|shape| (flags.stencil_write, shape)),
                    );
                    out.truncate(from);
                    continue;
                }
                let banded = flags.stencil_equal != 0
                    && flags.stencil_equal != u8::MAX
                    && bands.iter().any(|(value, _)| *value == flags.stencil_equal);
                let behind = flags.stencil_equal == u8::MAX || kept_off || banded;
                if flags.no_color
                    || (flags.stencil_equal != 0 && flags.stencil_equal != u8::MAX && !banded)
                {
                    continue;
                }
                // A screen preview (the inventory's doll) has no depth to keep it behind the piece:
                // it would cover the piece whole.
                if behind && super::cem_anim::is_in_gui() {
                    continue;
                }
                // A glint layer that only adds the shimmer over a piece (`depthFunc Equal`, the
                // piece's own shape) is the piece's own glint: the piece draws it itself, masked by
                // its texture, so this one would only paint its holes.
                if flags.glint && flags.additive && glinted {
                    continue;
                }
                // Only a multitexture material reads a controller's second texture.
                if layer.layer.secondary && !flags.multitexture {
                    if bedrock_debug() {
                        log_bedrock_once(
                            format!("{}/{}/{}/second", m.kind.id, att.slot, layer.layer.geometry),
                            || {
                                format!(
                                    "{}: {} layer {} ({}) skipped: second texture of a material without multitexture ({})",
                                    m.kind.id,
                                    att.slot,
                                    layer.layer.geometry,
                                    layer.texture,
                                    layer.layer.material
                                )
                            },
                        );
                    }
                    continue;
                }
                let uv = if ignores_alpha(&flags) {
                    atlas(&format!("{}_solid", layer.texture)).unwrap_or(uv)
                } else {
                    uv
                };
                // An emissive material whose alpha is the glow (`entity_emissive`) lights only the
                // texels the texture leaves less than opaque: a texture with none (a pack's flat
                // colour for a rim) keeps the world's light. One that adds, or that glows by being
                // an eye, is all glow.
                // A piece whose glowing bones the pack gives a material of their own (a redstone
                // torch's lit head, a torch's flame) lights them whole.
                let glows = flags.emissive
                    && (!flags.emissive_alpha
                        || flags.additive
                        || groups.len() > 1
                        || atlas(&format!("{}_glow", layer.texture)).is_some());
                // A layer the stencil keeps to the outside of the piece (its rim, its shine) is the
                // piece's own edge: it darkens with the light the piece does, whatever the
                // controller says about lighting.
                let drawn_light = if (layer.layer.ignore_lighting && !behind)
                    || glows
                    || (in_hand && frame.held.full_bright)
                {
                    FULL_BRIGHT
                } else {
                    light
                };
                if super::bedrock_debug::settings().dumps(&m.kind.id)
                    && once_a_second(format!("{}/{}/{}/bones", m.id, att.slot, layer.layer.geometry))
                {
                    // Each bone of the piece: the holder's bone it follows (or none: it stays where
                    // its own pose puts it) and how far that bone has moved from rest, in pixels.
                    fn walk(
                        part: &Part,
                        base_of: &dyn Fn(&str) -> Option<glam::Mat4>,
                        base: glam::Mat4,
                        out: &mut Vec<String>,
                    ) {
                        out.push(match base_of(&part.name) {
                            Some(t) => {
                                let d = (base.inverse() * t).w_axis.truncate() * 16.0;
                                format!("{}->moved[{:.1},{:.1},{:.1}]", part.name, d.x, d.y, d.z)
                            }
                            None => format!("{}->unbound", part.name),
                        });
                        for c in &part.children {
                            walk(c, base_of, base, out);
                        }
                    }
                    let mut bones = Vec::new();
                    walk(&layer.built.part, &base_of, base, &mut bones);
                    log::info!(
                        "bedrock-worn {}#{} {} {}: {} | holder bones {}",
                        m.kind.id,
                        m.id,
                        att.slot,
                        layer.layer.geometry,
                        bones.join(" "),
                        {
                            let mut names: Vec<_> = holder
                                .iter()
                                .map(|(n, (now, rest))| {
                                    let p = now.w_axis.truncate() * 16.0;
                                    let r = rest.w_axis.truncate() * 16.0;
                                    format!("{n}@[{:.0},{:.0},{:.0}]rest[{:.0},{:.0},{:.0}]", p.x, p.y, p.z, r.x, r.y, r.z)
                                })
                                .collect();
                            names.sort();
                            names.join(" ")
                        }
                    );
                }
                let start = out.len();
                // Leather takes the dye of the piece worn (undyed: its own brown).
                let dyed = match (flags.armor_dye, att.slot) {
                    (true, "head" | "torso" | "legs" | "feet") => {
                        let n = match att.slot {
                            "head" => 0,
                            "torso" => 1,
                            "legs" => 2,
                            _ => 3,
                        };
                        let c = |ch| super::bedrock::armor_color(m, n, ch);
                        tint(color, [c(0), c(1), c(2), 1.0])
                    }
                    _ => color,
                };
                // A glinting material lays the enchantment's shimmer over its texture.
                let drawn_overlay = if flags.glint && !flags.additive {
                    ARMOR_GLINT
                } else {
                    overlay
                };
                draw_attached(
                    &layer.built.part,
                    glam::Mat4::IDENTITY,
                    base,
                    bound_only,
                    &base_of,
                    &poses,
                    uv,
                    tint(dyed, layer.layer.tint()),
                    drawn_light,
                    drawn_overlay,
                    out,
                );
                if out.len() > start {
                    glinted |= drawn_overlay == ARMOR_GLINT;
                    if flags.stencil_write != 0 {
                        written.push(flags.stencil_write);
                    }
                }
                // As on the mob's own model: a single-sided material shows an inside-out cube's
                // far walls only (a bell's rim, a hull 0.3 px around it), not its near ones over
                // the piece.
                if !flags.double_sided
                    && (flags.blend
                        || (!flags.additive && bedrock.has_inside_out(&layer.layer.geometry)))
                {
                    cull_back_faces(out, start);
                }
                if layer.built.single_texel {
                    let centre = [(uv[0] + uv[2]) / 2.0, (uv[1] + uv[3]) / 2.0];
                    for v in &mut out[start..] {
                        v.uv = centre;
                    }
                }
                if banded {
                    // Only inside the band and in front of it (cut before the shift, at the
                    // depth it is drawn at); then in front of the rim it lights, behind the piece.
                    let shapes: Vec<ScreenShape> = bands
                        .iter()
                        .filter(|(value, _)| *value == flags.stencil_equal)
                        .map(|(_, shape)| *shape)
                        .collect();
                    clip_to_screen_shapes(out, start, &shapes);
                    lift_toward_camera(&mut out[start..], -(BEDROCK_REAR_BIAS - BEDROCK_BAND_LIFT));
                } else if behind {
                    lift_toward_camera(&mut out[start..], -BEDROCK_REAR_BIAS);
                } else if layers_on > 0 {
                    let push = BEDROCK_LAYER_BIAS * layers_on as f32;
                    lift_toward_camera(&mut out[start..], push);
                }
                layers_on += usize::from(out.len() > start);
                if flags.uv_anim {
                    animate_uv(&mut out[start..], uv, layer.layer.uv_anim);
                }
                // A glowing layer is not shaded by the direction its faces point: an emissive
                // texel shows its own colour whichever way the item is turned.
                if layer.layer.flat_lighting || glows {
                    flatten_lighting(&mut out[start..]);
                }
                if bedrock_debug() {
                    let vertices = out.len() - start;
                    let (id, item) = (m.kind.id.clone(), att.slot);
                    // Where the item is, in blocks from the mob's feet (the world's axes).
                    let feet = base.w_axis.truncate();
                    let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
                    for v in &out[start..] {
                        let p = glam::Vec3::from_array(v.pos) - feet;
                        lo = lo.min(p);
                        hi = hi.max(p);
                    }
                    let hand = att
                        .binds
                        .values()
                        .find_map(|b| holder.get(b.as_str()))
                        .map(|(now, _)| (base * *now).w_axis.truncate() - feet);
                    log_bedrock_once(format!("{id}/{item}/{}/{drawn_light}", layer.layer.geometry), || {
                        format!(
                            "{id}: {item} draws {} with {vertices} vertices, from the feet: x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}, light {drawn_light:#x} (world {light:#x}) behind {behind} banded {banded} ignore_lighting {} emissive {} material {}, hand bone at {hand:.2?} (bound to {:?}; holder lacks {:?})",
                            layer.layer.geometry,
                            lo.x, hi.x, lo.y, hi.y, lo.z, hi.z,
                            layer.layer.ignore_lighting,
                            flags.emissive,
                            layer.layer.material,
                            att.binds,
                            att.binds
                                .values()
                                .filter(|b| !holder.contains_key(*b))
                                .collect::<Vec<_>>()
                        )
                    });
                }
                // What draws behind the piece (the rim) is opaque where it shows: the plain pass keeps
                // it to the piece's gaps by depth, where a blended one would not.
                let mut tagged = flags;
                if behind && !flags.additive {
                    tagged.blend = false;
                }
                tag_bedrock_material(
                    &mut out[start..],
                    &tagged,
                    layer.layer.secondary,
                    !behind && layer.layer.color[3] < 1.0,
                );
                super::bedrock_stencil::hand_over(&stencil_flags, out, start);
                // Leather's faint texels (alpha 1) are the stitching and highlights the dye does not
                // reach: drawn again over the dyed piece in the texture's own colours.
                if flags.armor_dye
                    && out.len() > start
                    && let Some(faint) = atlas(&format!("{}_glow", layer.texture))
                {
                    let from = out.len();
                    draw_attached(
                        &layer.built.part,
                        glam::Mat4::IDENTITY,
                        base,
                        bound_only,
                        &base_of,
                        &poses,
                        faint,
                        tint(color, layer_tint_of(layer.layer.tint())),
                        drawn_light,
                        overlay,
                        out,
                    );
                    if out.len() > from {
                        lift_toward_camera(
                            &mut out[from..],
                            BEDROCK_LAYER_BIAS * (layers_on as f32 + 1.0),
                        );
                        tag_bedrock_material(
                            &mut out[from..],
                            &flags,
                            layer.layer.secondary,
                            layer.layer.color[3] < 1.0,
                        );
                    }
                }
                if bedrock_debug() && in_hand && out.len() > start {
                    // Where the held item sits, in the model's own frame (pixels; the model's
                    // origin is 24 above the feet, y grows downward): the box of its vertices and
                    // the bones of the holder it follows.
                    let inverse = base.inverse();
                    let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
                    for v in &out[start..] {
                        let p = inverse.transform_point3(glam::Vec3::from_array(v.pos)) * 16.0;
                        lo = lo.min(p);
                        hi = hi.max(p);
                    }
                    let anchors: Vec<String> = att
                        .binds
                        .values()
                        .filter_map(|b| {
                            holder.get(b).map(|(now, _)| {
                                let p = now.transform_point3(glam::Vec3::ZERO);
                                format!("{b} at [{:.1}, {:.1}, {:.1}]", p.x, p.y, p.z)
                            })
                        })
                        .collect();
                    let (id, item) = (m.kind.id.clone(), att.slot);
                    log_bedrock_once(
                        format!("{id}/{item}/{}/at", layer.layer.geometry),
                        || {
                            format!(
                                "{id}: {item} {} box x {:.1}..{:.1} y {:.1}..{:.1} z {:.1}..{:.1} (px, feet at y 24); holder {}; scale {}",
                                layer.layer.geometry,
                                lo.x, hi.x, lo.y, hi.y, lo.z, hi.z,
                                anchors.join(", "),
                                hand_scale
                            )
                        },
                    );
                }
            }
        }
        if m.extra.first_person && in_hand && super::bedrock_debug::settings().trace_fp {
            // `fp-view`: the bound bone's frame and what the attachable drew, in the camera's
            // space (x right, y up, -z ahead), to compare with Bedrock's screen.
            if let Some(view) = FP_VIEW.with(|v| v.get()) {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
                for v in &out[att_start..] {
                    let p = view * glam::Vec3::from_array(v.pos);
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
                let frames: Vec<String> = att
                    .binds
                    .keys()
                    .filter_map(|k| {
                        base_of(k).map(|a| {
                            let a = glam::Mat4::from_quat(view) * a;
                            let f = |v: glam::Vec4| [v.x, v.y, v.z].map(|c| (c * 100.0).round() / 100.0);
                            format!(
                                "{k}: t={:?} x={:?} y={:?} z={:?}",
                                f(a.w_axis),
                                f(a.x_axis),
                                f(a.y_axis),
                                f(a.z_axis)
                            )
                        })
                    })
                    .collect();
                log_bedrock_once(format!("fp-view/{}/{secs}", att.slot), || {
                    format!(
                        "fp-view {} item vertices {} x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2} | {}",
                        att.slot,
                        out.len() - att_start,
                        lo.x, hi.x, lo.y, hi.y, lo.z, hi.z,
                        frames.join(" ; ")
                    )
                });
            }
        }
    }
    // A body the pack laid down rests on the ground and, as the fall ends, on the spot where the
    // mob stood: the pack's own offsets assume its geometry and queries, which the game answers
    // only in part.
    if let (Some((f, feet)), true) = (lying, out.len() > first_vertex) {
        let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
        for v in &out[first_vertex..] {
            let p = glam::Vec3::from_array(v.pos);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let centre = (lo + hi) / 2.0;
        let shift = glam::Vec3::new((feet.x - centre.x) * f, feet.y - lo.y, (feet.z - centre.z) * f);
        for v in &mut out[first_vertex..] {
            for (p, d) in v.pos.iter_mut().zip(shift.to_array()) {
                *p += d;
            }
        }
    }
    drew
}
thread_local! {
    /// While the first-person view model draws: the turn from the game's camera-relative space
    /// into the camera's own (for the `fp-view` trace).
    static FP_VIEW: std::cell::Cell<Option<glam::Quat>> = const { std::cell::Cell::new(None) };
}
thread_local! {
    /// The player's standing eye height, last seen standing: what the view model stands below
    /// the camera by while sneaking.
    static FP_STAND_EYE: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}
thread_local! {
    /// While the first-person view model draws: the eye height its feet stand below the camera.
    static FP_EYE: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}
/// Whether the first-person view model is being drawn now.
pub fn in_first_person_view() -> bool {
    FP_VIEW.with(|v| v.get().is_some())
}
thread_local! {
    /// How far each hand is raised in the first-person view, 0 lowered to 1 up (Java's
    /// `mainHandHeight` / `offHandHeight`, interpolated): Bedrock's `v.player_arm_height` and
    /// `c.player_offhand_arm_height`.
    static FP_ARM_HEIGHTS: std::cell::Cell<[f32; 2]> = const { std::cell::Cell::new([1.0, 1.0]) };
}
/// Sets the hands' heights the next first-person view models read (`FP_ARM_HEIGHTS`).
pub fn set_first_person_arm_heights(heights: [f32; 2]) {
    FP_ARM_HEIGHTS.with(|h| h.set(heights));
}
/// The hands' heights in the first-person view (main, off), 1 when raised.
pub fn first_person_arm_heights() -> [f32; 2] {
    FP_ARM_HEIGHTS.with(std::cell::Cell::get)
}
/// Ids of the local player's two first-person copies (one per hand), apart from every mob's.
const FIRST_PERSON_IDS: [u32; 2] = [u32::MAX - 2, u32::MAX - 3];

/// The first-person view model of a pack's attachable: what the local player `player` holds in a
/// hand (`item`), drawn as the pack dresses it for `context.is_first_person`, on the game's hand
/// pose `hand` (camera-relative world space, items upright and facing away from the camera).
/// `false` when no attachable of the pack dresses the item, which leaves it to the game's own
/// item model.
#[allow(clippy::too_many_arguments)]
pub fn first_person_item(
    player: &Mob,
    item: &str,
    off_hand: bool,
    alpha: f32,
    hand: glam::Mat4,
    look: [f32; 3],
    atlas: Atlas,
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) -> bool {
    let settings = super::bedrock_debug::settings();
    if settings.first_person_off {
        return false;
    }
    let mut copy = player.clone();
    copy.id = FIRST_PERSON_IDS[usize::from(off_hand)];
    copy.extra.first_person = true;
    copy.extra.main_hand = None;
    copy.extra.off_hand = None;
    copy.extra.head = None;
    copy.extra.chest = None;
    copy.extra.legs = None;
    copy.extra.feet = None;
    copy.extra.body = None;
    let stack: std::sync::Arc<str> = item.into();
    if off_hand {
        copy.extra.off_hand = Some(stack);
    } else {
        copy.extra.main_hand = Some(stack);
    }
    // Bedrock draws the first-person view model as the player itself, standing where it
    // stands with the camera at its eye (camera-relative space: the feet `eye_height` below
    // the origin), its pack's first-person animations putting the arm before the camera.
    // The game's hand pose (`hand`) is not used. The view model faces the other way from
    // the third-person body (+z ahead): the vanilla first-person poses put the right arm at
    // +x and +z (`empty_hand`: 10.24, -4.08, 11.52) and turn the head half round
    // (`base_pose`).
    // It is fixed to the view: it follows the camera this frame (`look`: its yaw, pitch and
    // eye height), turned with the look's pitch about the eye, so it stays where it is on the
    // screen when the player looks up or down (a pack's first-person poses, Actions & Stuff's
    // among them, turn nothing with the pitch). The pitch the animations read
    // (`q.target_x_rotation`) is then level, so a pose that turns the body with it
    // (`base_pose`) does not turn it twice.
    let [yaw, pitch, eye_height] = look;
    // The view model stands below the camera by the standing eye height, crouched or not: the
    // camera drops when sneaking, the arms in front of it stay where they were (Bedrock). With
    // the crouched eye the model rose 0.35 block toward the camera and the arms came up.
    let eye_height = FP_STAND_EYE.with(|stand| {
        if player.extra.crouching || player.extra.sneaking {
            stand.get().unwrap_or(eye_height)
        } else {
            stand.set(Some(eye_height));
            eye_height
        }
    });
    copy.yaw = yaw;
    copy.previous_yaw = yaw;
    copy.body_yaw = yaw;
    copy.previous_body_yaw = yaw;
    copy.head_yaw = yaw;
    copy.previous_head_yaw = yaw;
    copy.head_pitch = 0.0;
    copy.previous_head_pitch = 0.0;
    // `camera_rotation`: rotationYXZ(π - yaw, -pitch, 0); the view model turns from the
    // level look to this one.
    let level = glam::Quat::from_rotation_y(std::f32::consts::PI - yaw.to_radians());
    let tilt = level * glam::Quat::from_rotation_x(-pitch.to_radians()) * level.inverse();
    let start = out.len();
    FP_EYE.with(|v| v.set(Some(eye_height)));
    FP_VIEW.with(|v| {
        v.set(Some(
            glam::Quat::from_euler(
                glam::EulerRot::YXZ,
                std::f32::consts::PI - yaw.to_radians(),
                -pitch.to_radians(),
                0.0,
            )
            .inverse(),
        ))
    });
    render_bedrock(
        &copy,
        alpha,
        settings.first_person_matrix()
            * glam::Mat4::from_quat(tilt)
            * base_transform_at(
                &copy,
                alpha,
                glam::Mat4::from_translation(glam::Vec3::new(0.0, -eye_height, 0.0)),
            )
            * glam::Mat4::from_rotation_y(std::f32::consts::PI),
        atlas,
        light,
        data,
        out,
    );
    FP_VIEW.with(|v| v.set(None));
    FP_EYE.with(|v| v.set(None));
    let drawn = out.len() - start;
    if bedrock_debug() {
        let inverse = hand.inverse();
        let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
        for v in &out[start..] {
            let p = inverse.transform_point3(glam::Vec3::from_array(v.pos));
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let size = ((hi - lo).max_element() * 10.0) as i32;
        // The same vertices in the camera's own space (x right, y up, z ahead as the camera
        // sees them): `camera_rotation` is rotationYXZ(π - yaw, -pitch, 0), and the screen
        // position of a point follows from these (x/z, y/z against the field of view).
        let to_view = glam::Quat::from_euler(
            glam::EulerRot::YXZ,
            std::f32::consts::PI - yaw.to_radians(),
            -pitch.to_radians(),
            0.0,
        )
        .inverse();
        let (mut vlo, mut vhi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
        for v in &out[start..] {
            let p = to_view * glam::Vec3::from_array(v.pos);
            vlo = vlo.min(p);
            vhi = vhi.max(p);
        }
        log_bedrock_once(format!("first-person/{item}/{off_hand}/{size}"), || {
            if drawn == 0 {
                format!("first person {item}: no attachable of the pack draws it")
            } else {
                format!(
                    "first person {item} ({}): {drawn} vertices, from the hand: x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}; \
                     in the camera's space (yaw {yaw:.0} pitch {pitch:.0}): x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}",
                    if off_hand { "off hand" } else { "main hand" },
                    lo.x, hi.x, lo.y, hi.y, lo.z, hi.z,
                    vlo.x, vhi.x, vlo.y, vhi.y, vlo.z, vhi.z
                )
            }
        });
    }
    drawn > 0
}
/// Only a Bedrock pack's model of `m` (a stand-in for an entity the game draws by itself, such
/// as an armor stand): false when no pack draws it, and then nothing was drawn.
pub fn render_pack_only(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) -> bool {
    render_pack_at(m, alpha, base_transform(m, alpha, camera), atlas, light, data, out)
}
/// `render_pack_only` from the model's frame `base` (as `base_transform` makes it: the ground
/// 1.501 below the origin, after a `(-1, -1, 1)` scale).
pub fn render_pack_at(
    m: &Mob,
    alpha: f32,
    base: glam::Mat4,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) -> bool {
    let start = out.len();
    let drew = render_bedrock(m, alpha, base, atlas, light, data, out);
    if !drew {
        out.truncate(start);
    }
    drew
}
