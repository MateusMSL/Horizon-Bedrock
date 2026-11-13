/// A bedrock layer's quads in the draws its material asks for: blended (`mark_oit`), without
/// a depth write (`DisableDepthWrite`, and the textures a multitexture material lays over its
/// first), without a depth test (`DisableDepthTest`).
fn tag_bedrock_material(
    quads: &mut [Vertex],
    flags: &horizon_bedrock::material::MaterialFlags,
    secondary: bool,
    translucent: bool,
) {
    let laid = flags.multitexture && secondary;
    let mut bits = 0;
    if flags.no_depth_write || laid {
        bits |= crate::gfx::NO_DEPTH_WRITE;
    }
    if flags.no_depth_test {
        bits |= crate::gfx::NO_DEPTH_TEST;
    }
    // Light that adds (`blendSrc SourceAlpha`, `blendDst One`): its own pipeline, which adds the
    // colour times the alpha to what is behind instead of laying the texture over it.
    if flags.additive {
        bits |= crate::gfx::ADDITIVE;
    }
    if flags.blend || laid || translucent || bits != 0 {
        mark_oit(quads);
    }
    for vertex in quads {
        vertex.layer |= bits;
    }
}
/// Whether a material draws a texture's alpha as nothing: without a blend, an add or an alpha test
/// it paints every texel it samples, so a texel at alpha 157 or 2 (a charged creeper's skin and
/// its white face) shows opaque, and an emissive-alpha one reads its alpha as glow, not as cover.
/// A change-colour one (`USE_COLOR_MASK`) reads it as where the tint goes: a texel at alpha 2 is
/// opaque and barely tinted.
fn ignores_alpha(flags: &horizon_bedrock::material::MaterialFlags) -> bool {
    !flags.blend
        && !flags.additive
        && (flags.emissive_alpha
            || !flags.alpha_test
            || (flags.color_mask && !flags.armor_dye && !flags.multitexture))
}
