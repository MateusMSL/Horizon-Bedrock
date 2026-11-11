/// `base` with what the game's own model needs taken back, for a pack's model (which starts at
/// the feet at its own size). The body and what the pack's model holds share it: else the
/// held item sits apart from the body.
fn pack_model_base(m: &Mob, alpha: f32, base: glam::Mat4) -> glam::Mat4 {
    match m.species() {
        // `PhantomRenderer.scale` moves the game's model down into its box (it is modelled
        // 1.3125 blocks above the feet).
        Species::Phantom => {
            base * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.3125, -0.1875))
        }
        // `SulfurCubeRenderer.scale` sizes the game's model and moves it to its box (0.98 above
        // the feet, half size): left in, it sank a pack's cube 0.92 blocks into the ground.
        Species::SulfurCube => {
            let drop = glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.501, 0.0));
            base * drop.inverse() * super::sulfur_cube::scale(m, alpha).inverse() * drop
        }
        _ => base,
    }
}
/// Where a Bedrock pack's model holds an item: its `rightitem` / `leftitem` bone as the
/// animation posed it (the arm bone when the model has none), placed as `ItemInHandLayer`
/// places an item on a hand. `None` when no pack dresses the mob.
fn bedrock_hand(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    data: &MobData,
    left: bool,
) -> Option<glam::Mat4> {
    let bedrock = data
        .bedrock
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if bedrock.is_empty() || !bedrock.has_entity(&m.kind.id) {
        return None;
    }
    let frame = bedrock.frame(m, alpha, feet_from_camera(m, base_transform(m, alpha, camera)))?;
    let scale = bedrock_scale(&frame, m, alpha);
    let mut base = pack_model_base(m, alpha, base_transform(m, alpha, camera));
    if (scale - glam::Vec3::ONE).abs().max_element() > 1e-4 {
        let feet = glam::Mat4::from_translation(glam::Vec3::new(0.0, 1.501, 0.0));
        base = base * feet * glam::Mat4::from_scale(scale) * feet.inverse();
    }
    let (item, arm) = if left {
        ("leftitem", "leftarm")
    } else {
        ("rightitem", "rightarm")
    };
    let mut poses = Poses::pooled();
    for layer in &frame.layers {
        if layer.layer.color[3] <= 0.0 {
            continue;
        }
        poses.reset(&layer.built.part);
        super::bedrock::apply_pose(&frame.pose, &layer.built.names, &mut poses);
        let mut here = std::collections::HashMap::new();
        bone_matrices(&layer.built.part, &poses, &mut here);
        let turn =
            glam::Mat4::from_rotation_x((-90.0f32).to_radians()) * glam::Mat4::from_rotation_y(PI);
        if let Some((now, _)) = here.get(item) {
            return Some(
                base * *now * turn * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.125, 0.0)),
            );
        }
        if let Some((now, _)) = here.get(arm) {
            let (ox, oy, oz) = (1.0, 2.0, -10.0);
            return Some(
                base * *now
                    * turn
                    * glam::Mat4::from_translation(glam::Vec3::new(
                        if left { -ox } else { ox } / 16.0,
                        oy / 16.0,
                        oz / 16.0,
                    )),
            );
        }
    }
    None
}
/// Where a pack that dresses `m` has its bone `bone` this frame, from the entity's `base`
/// (`base_transform`): `None` when no pack dresses the mob, `Some(None)` when the pack's model
/// has no such bone or scaled it to nothing (a pack hides what hangs from it so).
pub fn bedrock_bone_frame(
    m: &Mob,
    alpha: f32,
    base: glam::Mat4,
    data: &MobData,
    bone: &str,
) -> Option<Option<glam::Mat4>> {
    let bedrock = data
        .bedrock
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if bedrock.is_empty() || !bedrock.has_entity(&m.kind.id) {
        return None;
    }
    let frame = bedrock.frame(m, alpha, feet_from_camera(m, base))?;
    let scale = bedrock_scale(&frame, m, alpha);
    let mut base = pack_model_base(m, alpha, base);
    if (scale - glam::Vec3::ONE).abs().max_element() > 1e-4 {
        let feet = glam::Mat4::from_translation(glam::Vec3::new(0.0, 1.501, 0.0));
        base = base * feet * glam::Mat4::from_scale(scale) * feet.inverse();
    }
    let mut poses = Poses::pooled();
    for layer in &frame.layers {
        if layer.layer.color[3] <= 0.0 {
            continue;
        }
        poses.reset(&layer.built.part);
        super::bedrock::apply_pose(&frame.pose, &layer.built.names, &mut poses);
        let mut here = std::collections::HashMap::new();
        bone_matrices(&layer.built.part, &poses, &mut here);
        if let Some((now, _)) = here.get(bone) {
            return Some((now.determinant().abs() > 1e-12).then_some(base * *now));
        }
    }
    Some(None)
}
/// Whether a pack's attachable draws the item in a hand of the mob, in place of the game's own
/// model of it.
pub fn held_item_replaced(m: &Mob, data: &MobData, off_hand: bool) -> bool {
    data.bedrock
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .held(m)
        .is_some_and(|h| if off_hand { h.off_replaced } else { h.main_replaced })
}
