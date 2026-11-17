/// The distinct material flags among a layer's bones, in order: one group per look. Kept by
/// what they follow from (the layer's materials, the model, the debug file, the stencil pass):
/// each look asks every bone's material, and ordering for the stencil pass asks it again per
/// look, which for a model of many bones drawn every frame (the first-person player) was a
/// good part of drawing it. The model is held, so its address is not reused while kept.
fn material_groups(
    layer: &horizon_bedrock::entity::Layer,
    built: &std::sync::Arc<super::bedrock::Built>,
    materials: &horizon_bedrock::material::Materials,
) -> std::rc::Rc<MatGroups> {
    struct Kept {
        material: String,
        bone_materials: Vec<(String, String)>,
        built: std::sync::Arc<super::bedrock::Built>,
        materials: usize,
        settings: std::sync::Arc<super::bedrock_debug::Settings>,
        stencil: bool,
        groups: std::rc::Rc<MatGroups>,
    }
    thread_local! {
        static KEPT: std::cell::RefCell<Vec<Kept>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    let settings = super::bedrock_debug::settings();
    let stencil = super::bedrock_stencil::on();
    let materials_at = std::ptr::from_ref(materials) as usize;
    let found = KEPT.with(|kept| {
        kept.borrow()
            .iter()
            .find(|k| {
                std::sync::Arc::ptr_eq(&k.built, built)
                    && std::sync::Arc::ptr_eq(&k.settings, &settings)
                    && k.materials == materials_at
                    && k.stencil == stencil
                    && k.material == layer.material
                    && k.bone_materials == layer.bone_materials
            })
            .map(|k| k.groups.clone())
    });
    if let Some(groups) = found {
        return groups;
    }
    let list = material_groups_of(layer, &built.names, materials);
    let bone = if list.len() < 2 {
        vec![0; built.names.len()]
    } else {
        built
            .names
            .iter()
            .map(|n| {
                let g = bone_group(layer, materials, n);
                list.iter().position(|x| *x == g).unwrap_or(0)
            })
            .collect()
    };
    let groups = std::rc::Rc::new(MatGroups { list, bone });
    KEPT.with(|kept| {
        let mut kept = kept.borrow_mut();
        // A few hundred models and layers in sight at most; past that, models let go.
        if kept.len() >= 512 {
            kept.clear();
        }
        kept.push(Kept {
            material: layer.material.clone(),
            bone_materials: layer.bone_materials.clone(),
            built: built.clone(),
            materials: materials_at,
            settings,
            stencil,
            groups: groups.clone(),
        });
    });
    groups
}

/// [`material_groups`], worked out.
fn material_groups_of(
    layer: &horizon_bedrock::entity::Layer,
    names: &[String],
    materials: &horizon_bedrock::material::Materials,
) -> Vec<MatGroup> {
    let debug = super::bedrock_debug::settings();
    let mut groups = vec![(
        super::bedrock_debug::flags(materials, &layer.material),
        [1.0; 4],
    )];
    if layer.bone_materials.is_empty()
        && debug.bone_materials.is_empty()
        && debug.bone_tint.is_empty()
    {
        return groups;
    }
    groups.clear();
    for g in names.iter().map(|n| bone_group(layer, materials, n)) {
        if !groups.contains(&g) {
            groups.push(g);
        }
    }
    if groups.is_empty() {
        groups.push((
            super::bedrock_debug::flags(materials, &layer.material),
            [1.0; 4],
        ));
    }
    // The translucent looks go last, as the game sorts them after the opaque ones: a shell
    // drawn before the core inside it would cover the core's depth and hide it.
    if super::bedrock_stencil::on() {
        // With the stencil pass (`bedrock_stencil`), in the render controller's material order,
        // as Bedrock draws them (`"*"` first, then each named rule): Actions & Stuff's clock is
        // its case (writes 0), its window (writes 3) and its face (where 3 is). A mask moved
        // before the case had its 3 written over by the case's 0, and the face never showed.
        let rank = |g: &MatGroup| {
            names
                .iter()
                .filter(|n| bone_group(layer, materials, n) == *g)
                .map(|n| {
                    let m = layer.material_for(n);
                    layer
                        .bone_materials
                        .iter()
                        .position(|(_, x)| x == m)
                        .unwrap_or(0)
                })
                .min()
                .unwrap_or(0)
        };
        groups.sort_by_cached_key(|g| (g.0.blend, rank(g)));
        return groups;
    }
    // A stencil mask (writes a value, no colour) goes first, whatever its bone's place: the
    // pieces that test for its value draw only once it is written. The test pack's clock (face
    // bone before its window) and Actions & Stuff's clock showed only the frame. (The CPU
    // stand-in only: the stencil pass keeps the pack's order.)
    groups.sort_by_key(|g| (!(g.0.no_color && g.0.stencil_write != 0), g.0.blend));
    groups
}

/// What a draw pass shares: the flags of the material and the debug file's tint for the bone.
type MatGroup = (horizon_bedrock::material::MaterialFlags, [f32; 4]);

/// A layer's looks ([`material_groups`]) and the one each of its model's bones draws in.
struct MatGroups {
    list: Vec<MatGroup>,
    /// By bone, as the model names them (`Built::names`): its look's index in `list`.
    bone: Vec<usize>,
}

/// The pass a bone's cubes draw in.
fn bone_group(
    layer: &horizon_bedrock::entity::Layer,
    materials: &horizon_bedrock::material::Materials,
    bone: &str,
) -> MatGroup {
    (
        super::bedrock_debug::bone_flags(materials, layer.material_for(bone), bone),
        super::bedrock_debug::settings().bone_tint_for(bone),
    )
}

/// `filter_lighting`: every face lit as one facing up, so the shading does not follow the
/// direction the face points.
fn flatten_lighting(quads: &mut [Vertex]) {
    for v in quads {
        v.normal = [0, 127, 0, 0];
    }
}

/// `uv_anim`: moves and scales the coordinates inside the texture's rectangle of the atlas,
/// wrapping at its edges.
fn animate_uv(quads: &mut [Vertex], rect: [f32; 4], anim: Option<([f32; 2], [f32; 2])>) {
    let Some((offset, scale)) = anim else {
        return;
    };
    let size = [rect[2] - rect[0], rect[3] - rect[1]];
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return;
    }
    for v in quads {
        for i in 0..2 {
            let local = (v.uv[i] - rect[i]) / size[i];
            let moved = local * scale[i] + offset[i];
            v.uv[i] = rect[i] + moved.rem_euclid(1.0) * size[i];
        }
    }
}

