//! The files of a resource pack that describe the world around the models: `fogs/`, the client
//! biomes (`biomes/*.client_biome.json`, `biomes_client/`, `biomes_client.json`), `blocks.json`
//! and `block_culling/`.
//!
//! They are read into plain data the client asks for by identifier. Nothing here draws.

use serde_json::Value;
use std::collections::BTreeMap;

/// A colour as `0xRRGGBB`.
pub type Rgb = u32;

/// `#RRGGBB` (or `#AARRGGBB`, the alpha dropped), or `[r, g, b]` in 0..1 or 0..255.
pub fn color(v: &Value) -> Option<Rgb> {
    match v {
        Value::String(s) => {
            let hex = s.trim().trim_start_matches('#');
            let hex = match hex.len() {
                6 => hex,
                8 => &hex[2..],
                _ => return None,
            };
            u32::from_str_radix(hex, 16).ok()
        }
        Value::Array(a) if a.len() >= 3 => {
            let c: Vec<f64> = a.iter().take(3).filter_map(Value::as_f64).collect();
            if c.len() != 3 {
                return None;
            }
            let scale = if c.iter().all(|x| *x <= 1.0) { 255.0 } else { 1.0 };
            let byte = |x: f64| (x * scale).round().clamp(0.0, 255.0) as u32;
            Some(byte(c[0]) << 16 | byte(c[1]) << 8 | byte(c[2]))
        }
        _ => None,
    }
}

/// How far a fog reaches (`render_distance_type`): a fixed distance in blocks, or a fraction of
/// the render distance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FogReach {
    Fixed,
    #[default]
    Render,
}

/// One medium's fog (`distance.air`, `.water`, `.lava`, `.lava_resistance`, `.weather`,
/// `.powder_snow`).
#[derive(Clone, Debug, PartialEq)]
pub struct FogDistance {
    pub start: f32,
    pub end: f32,
    pub color: Rgb,
    pub reach: FogReach,
}

/// `minecraft:fog_settings`.
#[derive(Clone, Debug, Default)]
pub struct Fog {
    pub id: String,
    /// By medium (`air`, `water`, ...).
    pub distance: BTreeMap<String, FogDistance>,
    /// `volumetric.density.<medium>`: (max density, max density height, zero density height).
    pub density: BTreeMap<String, (f32, f32, f32)>,
}

impl Fog {
    pub fn parse(v: &Value) -> Option<Fog> {
        let root = v.get("minecraft:fog_settings")?;
        let id = root.pointer("/description/identifier")?.as_str()?.to_owned();
        let mut fog = Fog { id, ..Fog::default() };
        if let Some(distance) = root.get("distance").and_then(Value::as_object) {
            for (medium, d) in distance {
                let number = |key: &str| d.get(key).and_then(Value::as_f64).map(|n| n as f32);
                let (Some(start), Some(end)) = (number("fog_start"), number("fog_end")) else {
                    continue;
                };
                fog.distance.insert(
                    medium.clone(),
                    FogDistance {
                        start,
                        end,
                        color: d.get("fog_color").and_then(color).unwrap_or(0xFFFFFF),
                        reach: match d.get("render_distance_type").and_then(Value::as_str) {
                            Some("fixed") => FogReach::Fixed,
                            _ => FogReach::Render,
                        },
                    },
                );
            }
        }
        if let Some(density) = root.pointer("/volumetric/density").and_then(Value::as_object) {
            for (medium, d) in density {
                let number = |key: &str| d.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                fog.density.insert(
                    medium.clone(),
                    (
                        number("max_density"),
                        number("max_density_height"),
                        number("zero_density_height"),
                    ),
                );
            }
        }
        Some(fog)
    }
}

/// What a pack says of one biome's look: `minecraft:client_biome` components, or an entry of the
/// older `biomes_client.json`.
#[derive(Clone, Debug, Default)]
pub struct ClientBiome {
    pub id: String,
    pub water_color: Option<Rgb>,
    pub water_transparency: Option<f32>,
    pub water_fog_color: Option<Rgb>,
    pub sky_color: Option<Rgb>,
    pub grass_color: Option<Rgb>,
    pub foliage_color: Option<Rgb>,
    /// The identifier of the fog in `fogs/` (`minecraft:fog_appearance`).
    pub fog: Option<String>,
    pub ambient_loop: Option<String>,
    pub music: Option<(String, f32)>,
    /// `minecraft:atmosphere_identifier`, `color_grading_identifier`, `lighting_identifier` and
    /// `water_identifier`: the deferred-lighting settings the biome picks (not drawn).
    pub deferred: BTreeMap<String, String>,
}

impl ClientBiome {
    /// A `biomes/<name>.client_biome.json` file.
    pub fn parse(v: &Value) -> Option<ClientBiome> {
        let root = v.get("minecraft:client_biome")?;
        let id = root.pointer("/description/identifier")?.as_str()?.to_owned();
        let components = root.get("components")?;
        let get = |path: &str| components.pointer(path);
        let mut b = ClientBiome { id, ..ClientBiome::default() };
        b.water_color = get("/minecraft:water_appearance/surface_color").and_then(color);
        b.water_transparency = get("/minecraft:water_appearance/surface_transparency")
            .and_then(Value::as_f64)
            .map(|n| n as f32);
        b.sky_color = get("/minecraft:sky_color/sky_color").and_then(color);
        b.grass_color = get("/minecraft:grass_appearance/color").and_then(color);
        b.foliage_color = get("/minecraft:foliage_appearance/color").and_then(color);
        b.fog = get("/minecraft:fog_appearance/fog_identifier")
            .and_then(Value::as_str)
            .map(str::to_owned);
        b.ambient_loop = get("/minecraft:ambient_sounds/ambient_loop")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(music) = get("/minecraft:music/music").and_then(Value::as_str) {
            let volume = get("/minecraft:music/volume").and_then(Value::as_f64).unwrap_or(1.0);
            b.music = Some((music.to_owned(), volume as f32));
        }
        for (component, key) in [
            ("minecraft:atmosphere_identifier", "atmosphere_identifier"),
            ("minecraft:color_grading_identifier", "color_grading_identifier"),
            ("minecraft:lighting_identifier", "lighting_identifier"),
            ("minecraft:water_identifier", "water_identifier"),
        ] {
            if let Some(id) = get(&format!("/{component}/{key}")).and_then(Value::as_str) {
                b.deferred.insert(key.to_owned(), id.to_owned());
            }
        }
        Some(b)
    }

    /// The biomes of a `biomes_client.json`: `{"biomes": {"<name>": {"water_surface_color": ...}}}`.
    pub fn parse_legacy(v: &Value) -> Vec<ClientBiome> {
        let Some(biomes) = v.get("biomes").and_then(Value::as_object) else {
            return Vec::new();
        };
        biomes
            .iter()
            .filter_map(|(name, b)| {
                let b = b.as_object()?;
                let mut out = ClientBiome { id: name.clone(), ..ClientBiome::default() };
                out.water_color = b.get("water_surface_color").and_then(color);
                out.water_transparency = b
                    .get("water_surface_transparency")
                    .and_then(Value::as_f64)
                    .map(|n| n as f32);
                out.water_fog_color = b.get("water_fog_color").and_then(color);
                out.sky_color = b.get("sky_color").and_then(color);
                out.grass_color = b.get("grass_color").and_then(color);
                out.foliage_color = b.get("foliage_color").and_then(color);
                Some(out)
            })
            .collect()
    }
}

/// Where a block's textures come from (`blocks.json` `textures`).
#[derive(Clone, Debug, PartialEq)]
pub enum BlockTextures {
    /// One `terrain_texture.json` name for every face.
    All(String),
    /// By face: `up`, `down`, `north`, `south`, `east`, `west`, and the groups `side` and `*`.
    Faces(BTreeMap<String, String>),
}

/// An entry of `blocks.json`.
#[derive(Clone, Debug, Default)]
pub struct BlockDef {
    pub textures: Option<BlockTextures>,
    pub carried_textures: Option<String>,
    pub sound: Option<String>,
    pub isotropic: bool,
    pub brightness_gamma: Option<f32>,
    pub ambient_occlusion_exponent: Option<f32>,
}

impl BlockDef {
    /// Every block of a `blocks.json`, by identifier.
    pub fn parse_file(v: &Value) -> BTreeMap<String, BlockDef> {
        let mut out = BTreeMap::new();
        let Some(root) = v.as_object() else {
            return out;
        };
        for (id, def) in root {
            let Some(def) = def.as_object() else {
                continue;
            };
            let mut b = BlockDef::default();
            b.textures = match def.get("textures") {
                Some(Value::String(s)) => Some(BlockTextures::All(s.clone())),
                Some(Value::Object(faces)) => Some(BlockTextures::Faces(
                    faces
                        .iter()
                        .filter_map(|(k, t)| Some((k.clone(), t.as_str()?.to_owned())))
                        .collect(),
                )),
                _ => None,
            };
            b.carried_textures = match def.get("carried_textures") {
                Some(Value::String(s)) => Some(s.clone()),
                Some(Value::Object(faces)) => {
                    faces.values().find_map(|t| t.as_str()).map(str::to_owned)
                }
                _ => None,
            };
            b.sound = def.get("sound").and_then(Value::as_str).map(str::to_owned);
            b.isotropic = match def.get("isotropic") {
                Some(Value::Bool(on)) => *on,
                Some(Value::Object(faces)) => faces.values().any(|f| f.as_bool() == Some(true)),
                _ => false,
            };
            b.brightness_gamma = def.get("brightness_gamma").and_then(Value::as_f64).map(|n| n as f32);
            b.ambient_occlusion_exponent = def
                .get("ambient_occlusion_exponent")
                .and_then(Value::as_f64)
                .map(|n| n as f32);
            out.insert(id.clone(), b);
        }
        out
    }
}

/// One rule of `minecraft:block_culling_rules`: the face of a cube in a geometry that a
/// neighbouring block on `direction` hides.
#[derive(Clone, Debug, PartialEq)]
pub struct CullRule {
    pub bone: String,
    pub cube: usize,
    pub face: String,
    pub direction: String,
}

/// `minecraft:block_culling_rules`.
#[derive(Clone, Debug, Default)]
pub struct CullRules {
    pub id: String,
    pub rules: Vec<CullRule>,
}

impl CullRules {
    pub fn parse(v: &Value) -> Option<CullRules> {
        let root = v.get("minecraft:block_culling_rules")?;
        let id = root.pointer("/description/identifier")?.as_str()?.to_owned();
        let rules = root
            .get("rules")?
            .as_array()?
            .iter()
            .filter_map(|r| {
                let part = r.get("geometry_part")?;
                Some(CullRule {
                    bone: part.get("bone")?.as_str()?.to_owned(),
                    cube: part.get("cube").and_then(Value::as_u64).unwrap_or(0) as usize,
                    face: part.get("face")?.as_str()?.to_owned(),
                    direction: r.get("direction")?.as_str()?.to_owned(),
                })
            })
            .collect();
        Some(CullRules { id, rules })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn colours_read_as_hex_or_as_a_list() {
        assert_eq!(color(&json!("#3a1f5c")), Some(0x3a1f5c));
        assert_eq!(color(&json!("#ff3a1f5c")), Some(0x3a1f5c));
        assert_eq!(color(&json!([1.0, 0.0, 0.5])), Some(0xff0080));
        assert_eq!(color(&json!([255, 128, 0])), Some(0xff8000));
        assert_eq!(color(&json!("nope")), None);
    }

    #[test]
    fn a_fog_has_a_reach_and_a_colour_for_each_medium() {
        let fog = Fog::parse(&json!({
            "format_version": "1.21.40",
            "minecraft:fog_settings": {
                "description": {"identifier": "custom:caverns"},
                "distance": {
                    "air": {"fog_start": 0.2, "fog_end": 1.0, "fog_color": "#3a1f5c",
                            "render_distance_type": "render"},
                    "water": {"fog_start": 0.0, "fog_end": 8.0, "fog_color": [0.1, 0.2, 0.3],
                              "render_distance_type": "fixed"}
                },
                "volumetric": {"density": {"air": {"max_density": 0.15,
                    "max_density_height": 32, "zero_density_height": 70}}}
            }
        }))
        .unwrap();
        assert_eq!(fog.id, "custom:caverns");
        assert_eq!(fog.distance["air"].color, 0x3a1f5c);
        assert_eq!(fog.distance["air"].reach, FogReach::Render);
        assert_eq!(fog.distance["water"].reach, FogReach::Fixed);
        assert_eq!(fog.distance["water"].end, 8.0);
        assert_eq!(fog.density["air"], (0.15, 32.0, 70.0));
        assert!(Fog::parse(&json!({"minecraft:fog_settings": {}})).is_none());
    }

    #[test]
    fn a_client_biome_reads_its_water_fog_and_sounds() {
        let b = ClientBiome::parse(&json!({
            "minecraft:client_biome": {
                "description": {"identifier": "ice_plains"},
                "components": {
                    "minecraft:fog_appearance": {"fog_identifier": "minecraft:fog_ice_plains"},
                    "minecraft:water_appearance": {"surface_color": "#14559b",
                                                   "surface_transparency": 0.8},
                    "minecraft:ambient_sounds": {"ambient_loop": "ambient.cave"},
                    "minecraft:music": {"music": "music.game.creative", "volume": 0.5},
                    "minecraft:lighting_identifier": {"lighting_identifier": "minecraft:default_lighting"}
                }
            }
        }))
        .unwrap();
        assert_eq!(b.id, "ice_plains");
        assert_eq!(b.water_color, Some(0x14559b));
        assert_eq!(b.water_transparency, Some(0.8));
        assert_eq!(b.fog.as_deref(), Some("minecraft:fog_ice_plains"));
        assert_eq!(b.music, Some(("music.game.creative".into(), 0.5)));
        assert_eq!(b.deferred["lighting_identifier"], "minecraft:default_lighting");
        let legacy = ClientBiome::parse_legacy(&json!({
            "biomes": {"swamp": {"water_surface_color": "#617B64", "water_surface_transparency": 0.65}}
        }));
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].water_color, Some(0x617B64));
    }

    #[test]
    fn blocks_json_names_textures_sounds_and_faces() {
        let blocks = BlockDef::parse_file(&json!({
            "format_version": "1.21.40",
            "starter:simple_cube": {
                "textures": {"up": "top", "side": "side"},
                "carried_textures": "icon", "isotropic": true, "sound": "stone"
            },
            "starter:crate": {"sound": "wood", "textures": "crate"}
        }));
        assert_eq!(blocks.len(), 2);
        let cube = &blocks["starter:simple_cube"];
        assert!(cube.isotropic);
        assert_eq!(cube.sound.as_deref(), Some("stone"));
        assert_eq!(cube.carried_textures.as_deref(), Some("icon"));
        let BlockTextures::Faces(faces) = cube.textures.as_ref().unwrap() else {
            panic!("faces")
        };
        assert_eq!(faces["side"], "side");
        assert_eq!(
            blocks["starter:crate"].textures,
            Some(BlockTextures::All("crate".into()))
        );
    }

    #[test]
    fn culling_rules_name_a_face_of_a_cube_and_the_side_that_hides_it() {
        let rules = CullRules::parse(&json!({
            "minecraft:block_culling_rules": {
                "description": {"identifier": "test:sushi_cull"},
                "rules": [{"geometry_part": {"bone": "bb_main", "cube": 0, "face": "north"},
                           "direction": "north"}]
            }
        }))
        .unwrap();
        assert_eq!(rules.id, "test:sushi_cull");
        assert_eq!(
            rules.rules,
            vec![CullRule {
                bone: "bb_main".into(),
                cube: 0,
                face: "north".into(),
                direction: "north".into()
            }]
        );
    }
}
