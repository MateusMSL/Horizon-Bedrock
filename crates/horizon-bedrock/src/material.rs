//! `materials/*.material`: which vanilla material a pack material derives from, and what it
//! changes. Only the effects that matter to the game's renderer are kept.

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// How a surface is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaterialFlags {
    /// Pixels below an alpha threshold are discarded.
    pub alpha_test: bool,
    /// Alpha blends with what is behind.
    pub blend: bool,
    /// Colour adds to what is behind.
    pub additive: bool,
    /// Drawn at full brightness.
    pub emissive: bool,
    /// The texture's alpha is how much a texel glows (`entity_emissive*`, `EMISSIVE`), not how
    /// see-through it is: the surface itself is opaque.
    pub emissive_alpha: bool,
    pub double_sided: bool,
    /// Scrolls the texture as the render controller's `uv_anim` says.
    pub uv_anim: bool,
    /// The game's energy swirl (a charged creeper's or a powered wither's armour): additive, the
    /// texture scrolling over the model.
    pub energy_swirl: bool,
    /// The reference this material writes to the stencil buffer (`StencilWrite` with the
    /// `Replace` pass), or 0. A pack draws a body this way, then an outline that only shows where
    /// the body is not.
    pub stencil_write: u8,
    /// The reference this material draws only where the stencil differs from (`NotEqual`), or 0.
    pub stencil_not_equal: u8,
    /// The reference this material draws only where the stencil equals (`Equal`, a mask's
    /// shape), or 0.
    pub stencil_equal: u8,
    /// The bits of the stencil the test reads (`stencilReadMask`); 0 when the material names
    /// none, which reads them all.
    pub stencil_mask: u8,
    /// `DisableColorWrite`: a mask that fills the depth or stencil buffer and draws no colour.
    pub no_color: bool,
    /// The enchantment glint scrolls over the surface (`GLINT`, `*_glint`, `armor_enchanted`).
    pub glint: bool,
    /// `InvertCulling`: the faces that cull are the ones facing the camera.
    pub invert_culling: bool,
    /// `DisableDepthWrite`: the surface hides nothing behind it.
    pub no_depth_write: bool,
    /// `DisableDepthTest`: the surface draws over what is nearer.
    pub no_depth_test: bool,
    /// `MASKED_MULTITEXTURE` or a `*_multitexture*` material: a second texture is the mask
    /// of the first.
    pub multitexture: bool,
    /// `armor_leather`: the worn piece's dye (`query.armor_color_slot`) multiplies the texture.
    pub armor_dye: bool,
    /// `USE_COLOR_MASK` (`entity_alphatest_change_color`, `*_change_color`): the texture's alpha
    /// is where the entity's change colour tints, not how see-through it is. Every texel that
    /// is not clear is opaque (a ghost sheep's body at alpha 2).
    pub color_mask: bool,
    /// The material's stencil state exactly as the pack wrote it (`EnableStencilTest`), for a
    /// renderer with a stencil buffer; the fields above are what the game draws without one.
    pub stencil: Stencil,
}

/// A material's stencil test and write (`EnableStencilTest`, `StencilWrite`, `frontFace`,
/// `stencilRef`, `stencilReadMask`, `stencilWriteMask`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stencil {
    /// `EnableStencilTest`: the material tests (and may write) the stencil.
    pub enabled: bool,
    /// `frontFace.stencilFunc`: when a pixel passes (`Always` when the pack names none).
    pub func: StencilFunc,
    /// `StencilWrite` with `frontFace.stencilPassOp` `Replace`: a pixel that passes takes the
    /// reference.
    pub replace: bool,
    /// `stencilRef`.
    pub reference: u8,
    /// `stencilReadMask` (all bits when the pack names none).
    pub read_mask: u8,
    /// `stencilWriteMask` (all bits when the pack names none).
    pub write_mask: u8,
}

impl Default for Stencil {
    fn default() -> Self {
        Self {
            enabled: false,
            func: StencilFunc::Always,
            replace: false,
            reference: 0,
            read_mask: u8::MAX,
            write_mask: u8::MAX,
        }
    }
}

impl Stencil {
    /// Whether the stencil changes what is drawn or what later draws see.
    pub fn matters(&self) -> bool {
        self.enabled && (self.replace || self.func != StencilFunc::Always)
    }
}

/// A stencil comparison (`stencilFunc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StencilFunc {
    Always,
    Never,
    Less,
    LessEqual,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
}

impl StencilFunc {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "Always" => Self::Always,
            "Never" => Self::Never,
            "Less" => Self::Less,
            "LessEqual" => Self::LessEqual,
            "Equal" => Self::Equal,
            "NotEqual" => Self::NotEqual,
            "Greater" => Self::Greater,
            "GreaterEqual" => Self::GreaterEqual,
            _ => return None,
        })
    }
}

#[derive(Debug, Default)]
struct Def {
    parent: Option<String>,
    add_defines: Vec<String>,
    remove_defines: Vec<String>,
    add_states: Vec<String>,
    remove_states: Vec<String>,
    blend_src: Option<String>,
    blend_dst: Option<String>,
    stencil_ref: Option<u8>,
    stencil_read_mask: Option<u8>,
    stencil_func: Option<String>,
    stencil_pass: Option<String>,
    stencil_write_mask: Option<u8>,
}

/// The materials of a pack, by lower-case name.
#[derive(Debug, Default)]
pub struct Materials {
    defs: HashMap<String, Def>,
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

impl Materials {
    /// Adds the materials of one `.material` file (`{"materials": {"name:parent": {...}}}`).
    pub fn add_file(&mut self, root: &Value) {
        let Some(o) = root.get("materials").and_then(Value::as_object) else {
            return;
        };
        for (key, body) in o {
            if key == "version" {
                continue;
            }
            let (name, parent) = match key.split_once(':') {
                Some((n, p)) => (n, Some(p.to_ascii_lowercase())),
                None => (key.as_str(), None),
            };
            let def = Def {
                parent,
                add_defines: strings(body.get("+defines").or_else(|| body.get("defines"))),
                remove_defines: strings(body.get("-defines")),
                add_states: strings(body.get("+states").or_else(|| body.get("states"))),
                remove_states: strings(body.get("-states")),
                blend_src: body
                    .get("blendSrc")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                blend_dst: body
                    .get("blendDst")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                stencil_ref: body
                    .get("stencilRef")
                    .and_then(Value::as_u64)
                    .and_then(|r| u8::try_from(r).ok()),
                stencil_read_mask: body
                    .get("stencilReadMask")
                    .and_then(Value::as_u64)
                    .and_then(|r| u8::try_from(r).ok()),
                stencil_func: body
                    .pointer("/frontFace/stencilFunc")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                stencil_pass: body
                    .pointer("/frontFace/stencilPassOp")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                stencil_write_mask: body
                    .get("stencilWriteMask")
                    .and_then(Value::as_u64)
                    .and_then(|r| u8::try_from(r).ok()),
            };
            self.defs.insert(name.to_ascii_lowercase(), def);
        }
    }

    /// Flags of the material `name` (a pack material or a vanilla one).
    pub fn flags(&self, name: &str) -> MaterialFlags {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut cur = name.to_ascii_lowercase();
        while seen.insert(cur.clone()) {
            match self.defs.get(&cur) {
                Some(d) => {
                    chain.push(d);
                    match &d.parent {
                        Some(p) => cur = p.clone(),
                        None => break,
                    }
                }
                None => break,
            }
        }
        // `cur` is the first name without a definition: a vanilla material.
        let mut f = vanilla(&cur);
        let mut defines: HashSet<&str> = HashSet::new();
        let mut states: HashSet<&str> = HashSet::new();
        if f.alpha_test {
            defines.insert("ALPHA_TEST");
        }
        let (mut src, mut dst) = (None, None);
        for d in chain.iter().rev() {
            defines.extend(d.add_defines.iter().map(String::as_str));
            for r in &d.remove_defines {
                defines.remove(r.as_str());
            }
            states.extend(d.add_states.iter().map(String::as_str));
            for r in &d.remove_states {
                states.remove(r.as_str());
            }
            src = d.blend_src.as_deref().or(src);
            dst = d.blend_dst.as_deref().or(dst);
        }
        f.alpha_test = defines.contains("ALPHA_TEST");
        f.uv_anim |= defines.contains("USE_UV_ANIM");
        f.blend |= states.contains("Blending");
        f.emissive |= defines.contains("EMISSIVE") || defines.contains("USE_EMISSIVE");
        f.emissive_alpha |= defines.contains("EMISSIVE") || defines.contains("USE_EMISSIVE");
        if states.contains("DisableCulling") {
            f.double_sided = true;
        } else if chain
            .iter()
            .any(|d| d.remove_states.iter().any(|s| s == "DisableCulling"))
        {
            f.double_sided = false;
        }
        f.no_color = states.contains("DisableColorWrite");
        f.glint |= defines.contains("GLINT");
        if chain
            .iter()
            .any(|d| d.remove_defines.iter().any(|r| r == "GLINT"))
            && !defines.contains("GLINT")
        {
            f.glint = false;
        }
        f.invert_culling = states.contains("InvertCulling");
        f.no_depth_write = states.contains("DisableDepthWrite");
        f.no_depth_test = states.contains("DisableDepthTest");
        f.multitexture |= defines.contains("MASKED_MULTITEXTURE");
        f.color_mask |= defines.contains("USE_COLOR_MASK");
        if chain
            .iter()
            .any(|d| d.remove_defines.iter().any(|r| r == "USE_COLOR_MASK"))
            && !defines.contains("USE_COLOR_MASK")
        {
            f.color_mask = false;
        }
        if states.contains("EnableStencilTest") {
            let (mut func, mut pass, mut reference, mut read_mask) = (None, None, None, None);
            for d in chain.iter().rev() {
                func = d.stencil_func.as_deref().or(func);
                pass = d.stencil_pass.as_deref().or(pass);
                reference = d.stencil_ref.or(reference);
                read_mask = d.stencil_read_mask.or(read_mask);
            }
            f.stencil_mask = read_mask.unwrap_or(0);
            let write_mask = chain.iter().rev().fold(None, |m, d| d.stencil_write_mask.or(m));
            f.stencil = Stencil {
                enabled: true,
                func: func.and_then(StencilFunc::parse).unwrap_or(StencilFunc::Always),
                replace: states.contains("StencilWrite") && pass == Some("Replace"),
                reference: reference.unwrap_or(0),
                read_mask: read_mask.unwrap_or(u8::MAX),
                write_mask: write_mask.unwrap_or(u8::MAX),
            };
            // Equal to 0 under a read mask that names bits other layers write: it draws only
            // where those layers did not (outside a body), which nothing here tracks per
            // pixel; drawn everywhere it would cover what it is kept away from.
            if func == Some("Equal")
                && !states.contains("StencilWrite")
                && reference.unwrap_or(0) == 0
                && read_mask.is_some_and(|m| m != 0)
            {
                f.stencil_equal = u8::MAX;
            }
            // References past 31 cannot be tracked as bits; none of the game's are.
            if let Some(r) = reference.filter(|r| (1..32).contains(r)) {
                if states.contains("StencilWrite") && pass == Some("Replace") {
                    f.stencil_write = r;
                } else if func == Some("NotEqual") {
                    f.stencil_not_equal = r;
                } else if func == Some("Equal") {
                    f.stencil_equal = r;
                }
            }
        }
        if (src, dst) == (Some("One"), Some("One"))
            || (src, dst) == (Some("SourceAlpha"), Some("One"))
        {
            f.additive = true;
            f.blend = true;
        }
        f
    }
}

/// Flags of the materials the game ships, by name.
fn vanilla(name: &str) -> MaterialFlags {
    // The magma cube's lava glows through the texels its texture leaves nearly transparent:
    // its material is an emissive-alpha one.
    if name == "magma_cube" {
        return vanilla("entity_emissive_alpha");
    }
    let has = |s: &str| name.contains(s);
    let alpha_blend = has("alphablend")
        || has("alpha_blend")
        || has("translucent")
        || has("beam")
        || has("ghost")
        || has("spectral");
    MaterialFlags {
        alpha_test: has("alphatest")
            || has("emissive_alpha")
            || has("masked")
            || has("cutout")
            // Armour textures are cut out of the surface they cover.
            || has("armor_leather")
            || has("armor_base"),
        // An emissive-alpha material is opaque: its alpha is how much a texel glows (a texel
        // at alpha 2 is all glow, not nearly clear), only a clear one is not drawn.
        blend: alpha_blend || has("additive"),
        additive: has("additive"),
        emissive: has("emissive") || has("eyes") || has("spider_eyes"),
        emissive_alpha: has("emissive"),
        energy_swirl: has("charged_creeper") || has("wither_armor") || has("energy_swirl"),
        uv_anim: has("charged_creeper")
            || has("creeper_armor")
            || has("wither_armor")
            || has("wind")
            || has("entity_static")
            || has("energy_swirl"),
        double_sided: !(has("cull") || has("one_sided"))
            && (has("alpha") || has("nocull") || has("double")),
        stencil_write: 0,
        stencil_not_equal: 0,
        stencil_equal: 0,
        stencil_mask: 0,
        no_color: false,
        glint: has("glint") || has("enchanted"),
        invert_culling: false,
        no_depth_write: false,
        no_depth_test: false,
        // The villager's materials lay the profession over the biome texture.
        multitexture: has("multitexture") || has("villager_v2"),
        armor_dye: has("armor_leather"),
        color_mask: has("change_color") || has("color_mask"),
        stencil: Stencil::default(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_change_color_material_reads_alpha_as_a_mask() {
        let mut m = Materials::default();
        m.add_file(&serde_json::json!({"materials": {
            "ghost:entity_alphatest_change_color": {},
            "tinted:entity": {"+defines": ["USE_COLOR_MASK"]},
            "plain:entity_alphatest": {}
        }}));
        assert!(m.flags("ghost").color_mask && m.flags("ghost").alpha_test);
        assert!(m.flags("tinted").color_mask);
        assert!(!m.flags("plain").color_mask);
    }

    #[test]
    fn stencil_state_is_kept_as_written() {
        let mut m = Materials::default();
        m.add_file(&serde_json::json!({"materials": {
            "window:entity": {"+states": ["EnableStencilTest", "StencilWrite", "DisableColorWrite"],
                "frontFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"}, "stencilRef": 3},
            "face:entity_alphatest": {"+states": ["EnableStencilTest"],
                "frontFace": {"stencilFunc": "Equal"}, "stencilRef": 3, "stencilReadMask": 239},
            "plain:entity": {}
        }}));
        let w = m.flags("window").stencil;
        assert!(w.enabled && w.replace && w.func == super::StencilFunc::Always && w.reference == 3);
        let f = m.flags("face").stencil;
        assert!(f.enabled && !f.replace && f.func == super::StencilFunc::Equal);
        assert_eq!((f.reference, f.read_mask, f.write_mask), (3, 239, 255));
        assert!(w.matters() && f.matters() && !m.flags("plain").stencil.matters());
    }

    #[test]
    fn stencil_materials_say_what_they_write_and_what_they_avoid() {
        let mut m = Materials::default();
        m.add_file(&serde_json::json!({"materials": {
            "body:entity": {"+states": ["EnableStencilTest", "StencilWrite"],
                "frontFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"}, "stencilRef": 2},
            "rim:entity_emissive": {"+states": ["EnableStencilTest"],
                "frontFace": {"stencilFunc": "NotEqual"}, "stencilRef": 2},
            "rim2:rim": {"+states": ["Blending"]},
        }}));
        assert_eq!(m.flags("body").stencil_write, 2);
        assert_eq!(m.flags("body").stencil_not_equal, 0);
        assert_eq!(m.flags("rim").stencil_not_equal, 2);
        assert_eq!(m.flags("rim").stencil_write, 0);
        assert_eq!(m.flags("rim2").stencil_not_equal, 2);
        assert_eq!(m.flags("entity").stencil_write, 0);
    }

    #[test]
    fn the_magma_cube_material_glows_like_emissive_alpha() {
        let m = Materials::default();
        let f = m.flags("magma_cube");
        assert!(f.emissive && f.emissive_alpha && !f.blend);
        assert_eq!(f, m.flags("entity_emissive_alpha"));
    }

    #[test]
    fn the_leather_armour_material_cuts_out_its_texture() {
        let mut m = Materials::default();
        m.add_file(&serde_json::json!({"materials": {
            "mine:armor_leather": {"+states": ["EnableStencilTest", "StencilWrite"], "stencilRef": 2},
        }}));
        assert!(m.flags("mine").alpha_test);
        assert!(m.flags("armor_leather").alpha_test);
        assert!(m.flags("mine").armor_dye && !m.flags("entity").armor_dye);
    }

    #[test]
    fn a_test_for_clear_bits_under_a_read_mask_is_not_drawn_over_the_body() {
        let mut m = Materials::default();
        m.add_file(&serde_json::json!({"materials": {
            "away:entity": {"+states": ["EnableStencilTest"], "stencilReadMask": 18,
                "frontFace": {"stencilFunc": "Equal", "stencilPassOp": "Keep"}, "stencilRef": 0},
        }}));
        assert_eq!(m.flags("away").stencil_equal, u8::MAX);
    }

    #[test]
    fn a_not_equal_test_keeps_the_mask_it_reads_under() {
        let mut m = Materials::default();
        m.add_file(&serde_json::json!({"materials": {
            "edge:entity_emissive": {"+states": ["EnableStencilTest"], "stencilReadMask": 7,
                "frontFace": {"stencilFunc": "NotEqual"}, "stencilRef": 11},
        }}));
        let f = m.flags("edge");
        assert_eq!((f.stencil_not_equal, f.stencil_mask), (11, 7));
    }

    #[test]
    fn a_stencil_mask_and_what_shows_inside_it_are_told_apart() {
        let mut m = Materials::default();
        m.add_file(&serde_json::json!({"materials": {
            "mask:entity_alphatest": {"+states": ["EnableStencilTest", "StencilWrite", "DisableColorWrite"],
                "frontFace": {"stencilFunc": "Equal", "stencilPassOp": "Replace"}, "stencilRef": 16},
            "inside:entity_alphatest": {"+states": ["EnableStencilTest"],
                "frontFace": {"stencilFunc": "Equal", "stencilPassOp": "Keep"}, "stencilRef": 16},
            "outside:entity_alphatest": {"+states": ["EnableStencilTest"],
                "frontFace": {"stencilFunc": "Equal", "stencilPassOp": "Keep"}, "stencilRef": 0},
        }}));
        let mask = m.flags("mask");
        assert!(mask.no_color && mask.stencil_write == 16 && mask.stencil_equal == 0);
        assert_eq!(m.flags("inside").stencil_equal, 16);
        assert_eq!(m.flags("outside").stencil_equal, 0);
    }

    use super::*;
    use crate::jsonc;

    fn mats(t: &str) -> Materials {
        let mut m = Materials::default();
        m.add_file(&jsonc::parse(t).unwrap());
        m
    }

    #[test]
    fn inherits_through_pack_materials_to_vanilla() {
        let m = mats(
            r#"{"materials":{"version":"1.0.0",
              "aaa:entity_alphablend":{"-defines":["FANCY"]},
              "bbb:aaa":{"+states":["DisableCulling"]},
              "ccc:entity_alphatest":{"-states":["DisableCulling"]},
              "ddd:entity":{"+defines":["ALPHA_TEST"]}}}"#,
        );
        let b = m.flags("BBB");
        assert!(b.blend && b.double_sided && !b.alpha_test);
        let c = m.flags("ccc");
        assert!(c.alpha_test && !c.double_sided);
        assert!(m.flags("ddd").alpha_test);
        assert!(m.flags("entity_emissive_alpha").emissive);
        assert!(m.flags("entity_emissive_alpha").emissive_alpha);
        assert!(!m.flags("spider_eyes").emissive_alpha);
        assert_eq!(m.flags("zombie"), MaterialFlags::default());
    }

    #[test]
    fn states_and_defines_that_change_how_a_surface_draws() {
        let m = mats(
            r#"{"materials":{"version":"1.0.0",
              "a:entity_alphatest":{"+defines":["GLINT"],"+states":["InvertCulling","DisableDepthWrite"]},
              "b:entity":{"+defines":["MASKED_MULTITEXTURE"],"+states":["DisableDepthTest"]},
              "c:a":{"-defines":["GLINT"]}}}"#,
        );
        let a = m.flags("a");
        assert!(a.glint && a.invert_culling && a.no_depth_write && !a.no_depth_test);
        let b = m.flags("b");
        assert!(b.multitexture && b.no_depth_test);
        assert!(!m.flags("c").glint);
        assert!(m.flags("entity_alphatest_glint").glint);
        assert!(m.flags("armor_enchanted").glint);
        assert!(m.flags("entity_multitexture_masked").multitexture);
    }

    #[test]
    fn cycles_end() {
        let m = mats(r#"{"materials":{"a:b":{},"b:a":{}}}"#);
        let _ = m.flags("a");
    }

    #[test]
    fn additive_by_blend_factors() {
        let m = mats(r#"{"materials":{"x:particles_blend":{"blendSrc":"One","blendDst":"One"}}}"#);
        assert!(m.flags("x").additive);
    }
}
