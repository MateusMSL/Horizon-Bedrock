//! A Bedrock resource pack as a Java resource pack folder.
//!
//! [`to_java`] writes what the Java client already knows how to load: the textures the pack's
//! entities, attachables and particles use as PNG under `assets/minecraft/textures/entity/bedrock/`
//! (so the entity atlas picks them up), the sounds as `assets/<namespace>/sounds/` with a
//! `sounds.json`, and the pack's own files, unchanged, under `assets/horizon_bedrock/` for the
//! renderer's Bedrock layer to read ([`crate::entity`], [`crate::animation`], and the rest).
//!
//! Nothing here touches game rules: the output is textures, sounds and the visual description.

use crate::pack::Pack;
use crate::{Error, Result, entity::ClientEntity, particle::Effect};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Where the pack's own files go, relative to the output folder.
pub const DATA_DIR: &str = "assets/horizon_bedrock";
/// Where its textures go, relative to `assets/minecraft/textures/`.
pub const TEXTURE_DIR: &str = "entity/bedrock";
/// The sound namespace of definitions that name none.
pub const DEFAULT_NAMESPACE: &str = "bedrock";

/// What a conversion wrote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub textures: usize,
    pub sounds: usize,
    pub sound_events: usize,
    pub data_files: usize,
    /// Textures the pack names but does not hold (the game's own).
    pub missing_textures: usize,
    /// Sound files in a format Java cannot play.
    pub skipped_sounds: usize,
    /// Block, item, sky and colour-map textures the pack holds for the game's own, and the
    /// splash lines it adds.
    pub world_textures: usize,
}

/// The data folders copied unchanged.
const DATA_FOLDERS: [&str; 12] = [
    "entity/",
    "attachables/",
    "animations/",
    "animation_controllers/",
    "render_controllers/",
    "models/",
    "particles/",
    "materials/",
    "fogs/",
    "biomes/",
    "biomes_client/",
    "block_culling/",
];

/// The path of a Bedrock texture (`textures/entity/x`) in the Java pack, as an atlas name
/// (`entity/bedrock/entity/x`).
pub fn atlas_name(texture: &str) -> String {
    let rest = texture.strip_prefix("textures/").unwrap_or(texture);
    format!("{TEXTURE_DIR}/{}", rest.to_ascii_lowercase())
}

/// Bedrock's name for a dye colour, Java's: `silver` is `light_gray`.
fn dye(color: &str) -> &str {
    if color == "silver" {
        "light_gray"
    } else {
        color
    }
}

/// The Java sprite a Bedrock texture the pack does not ship stands for: packs point at vanilla
/// textures (`textures/entity/skeleton/skeleton`, `textures/blocks/stone`, `textures/items/
/// iron_sword`) that the base game provides under names of its own. The answer is a sprite name
/// of the game's atlas (`entity/skeleton/skeleton`, `block/stone`, `item/iron_sword`).
pub fn java_texture(texture: &str) -> Option<String> {
    let rest = texture.strip_prefix("textures/")?.to_ascii_lowercase();
    let rest = rest
        .strip_suffix(".png")
        .or_else(|| rest.strip_suffix(".tga"))
        .unwrap_or(&rest)
        .to_owned();
    if let Some(name) = rest.strip_prefix("blocks/") {
        // Folders a pack keeps blocks in; the game's sprites are all in one.
        let name = match name.split_once('/') {
            Some(("candles" | "deepslate" | "huge_fungus", n)) if !n.contains('/') => n,
            Some(_) => return None,
            None => name,
        };
        if is_material_map(name) {
            return None;
        }
        return Some(java_block(name));
    }
    if let Some(name) = rest.strip_prefix("items/") {
        let name = match name.split_once('/') {
            Some(("candles", n)) if !n.contains('/') => n.to_owned(),
            Some(("spawn_eggs", n)) if !n.contains('/') => {
                format!("egg_{}", n.strip_prefix("spawn_egg_")?)
            }
            Some(("harness", n)) if !n.contains('/') => {
                format!("{}_harness", dye(n.strip_prefix("harness_")?))
            }
            Some(("spear", n)) if !n.contains('/') => n.to_owned(),
            Some(("nautilus_armor", n)) if !n.contains('/') => n.to_owned(),
            Some(_) => return None,
            None => name.to_owned(),
        };
        return Some(format!("item/{}", java_item(&name)));
    }
    if let Some(armor) = rest.strip_prefix("models/armor/") {
        let (material, layer) = armor.rsplit_once('_').unwrap_or((armor, "1"));
        let folder = if layer == "2" {
            "humanoid_leggings"
        } else {
            "humanoid"
        };
        return Some(match material {
            "elytra" => "entity/equipment/wings/elytra".into(),
            "chain" => format!("entity/equipment/{folder}/chainmail"),
            m => format!("entity/equipment/{folder}/{}", m.trim_end_matches('_')),
        });
    }
    if !rest.starts_with("entity/") {
        return None;
    }
    // Where the game names what Bedrock names otherwise.
    Some(match rest.as_str() {
        "entity/copper_golem/coper_golem_weathered" => {
            "entity/copper_golem/copper_golem_weathered".into()
        }
        "entity/iron_golem/cracked_low" => "entity/iron_golem/iron_golem_crackiness_low".into(),
        "entity/iron_golem/cracked_medium" => {
            "entity/iron_golem/iron_golem_crackiness_medium".into()
        }
        "entity/iron_golem/cracked_high" => "entity/iron_golem/iron_golem_crackiness_high".into(),
        "entity/endercrystal/endercrystal_beam" => "entity/end_crystal/end_crystal_beam".into(),
        "entity/fishhook" => "entity/fishing_hook".into(),
        "entity/polarbear" => "entity/bear/polarbear".into(),
        "entity/shield" => "entity/shield/shield_base_nopattern".into(),
        "entity/skulls/creeper" => "entity/creeper/creeper".into(),
        "entity/skulls/skeleton" => "entity/skeleton/skeleton".into(),
        "entity/skulls/wither_skeleton" => "entity/skeleton/wither_skeleton".into(),
        "entity/skulls/zombie" => "entity/zombie/zombie".into(),
        r if r.starts_with("entity/villager2/levels/level_") => format!(
            "entity/villager/profession_level/{}",
            &r["entity/villager2/levels/level_".len()..]
        ),
        r if r.starts_with("entity/bed/") => {
            format!("entity/bed/{}", dye(&r["entity/bed/".len()..]))
        }
        _ => rest,
    })
}

/// A PBR map beside a texture (`_mer`, `_mers`, `_normal`, `_heightmap`): the game has no sprite
/// for it.
fn is_material_map(name: &str) -> bool {
    // The sandstones' sides are `..._normal`: a texture, not a map.
    !matches!(name, "sandstone_normal" | "red_sandstone_normal")
        && ["_mer", "_mers", "_normal", "_heightmap"]
            .iter()
            .any(|s| name.ends_with(s))
}

/// Bedrock's wood names, Java's: `big_oak` and `roofed_oak` are `dark_oak`, `wood` is `oak`.
fn wood(name: &str) -> &str {
    match name {
        "big_oak" | "roofed_oak" | "darkoak" | "wood" => {
            if name == "wood" {
                "oak"
            } else {
                "dark_oak"
            }
        }
        other => other,
    }
}

/// The five coral kinds Bedrock names by colour, Java by the animal.
fn coral(color: &str) -> Option<&'static str> {
    Some(match color {
        "blue" => "tube",
        "pink" => "brain",
        "purple" => "bubble",
        "red" => "fire",
        "yellow" => "horn",
        _ => return None,
    })
}

/// A Bedrock block texture's sprite in Java.
fn java_block(name: &str) -> String {
    let named = |n: &str| format!("block/{n}");
    if let Some(c) = name.strip_prefix("wool_colored_") {
        return named(&format!("{}_wool", dye(c)));
    }
    if let Some(c) = name.strip_prefix("concrete_powder_") {
        return named(&format!("{}_concrete_powder", dye(c)));
    }
    if let Some(c) = name.strip_prefix("concrete_") {
        return named(&format!("{}_concrete", dye(c)));
    }
    if let Some(c) = name.strip_prefix("hardened_clay_stained_") {
        return named(&format!("{}_terracotta", dye(c)));
    }
    if let Some(c) = name.strip_prefix("glazed_terracotta_") {
        return named(&format!("{}_glazed_terracotta", dye(c)));
    }
    if let Some(c) = name.strip_prefix("glass_pane_top_") {
        return named(&format!("{}_stained_glass_pane_top", dye(c)));
    }
    if let Some(c) = name.strip_prefix("glass_")
        && !matches!(c, "pane" | "pane_top")
    {
        return named(&format!("{}_stained_glass", dye(c)));
    }
    if name.ends_with("_pottery_pattern") {
        return format!("entity/decorated_pot/{name}");
    }
    if let Some(w) = name.strip_prefix("planks_") {
        return named(&format!("{}_planks", wood(w)));
    }
    if let Some(rest) = name.strip_prefix("log_") {
        let (w, top) = rest.strip_suffix("_top").map_or((rest, false), |w| (w, true));
        if !w.chars().last().is_some_and(|c| c.is_ascii_digit()) && w != "top_normal_weak" {
            return named(&format!("{}_log{}", wood(w), if top { "_top" } else { "" }));
        }
    }
    if let Some(c) = name.strip_prefix("shulker_top_")
        && c != "undyed"
    {
        return named(&format!("{}_shulker_box", dye(c)));
    }
    if let Some(w) = name.strip_prefix("sapling_") {
        return named(&format!("{}_sapling", wood(w)));
    }
    if let Some(rest) = name.strip_prefix("door_") {
        for (suffix, java) in [("_lower", "bottom"), ("_upper", "top")] {
            if let Some(w) = rest.strip_suffix(suffix) {
                return named(&format!("{}_door_{java}", wood(w)));
            }
        }
    }
    if let Some(rest) = name.strip_prefix("coral_fan_") {
        let (color, dead) = rest.strip_suffix("_dead").map_or((rest, ""), |c| (c, "dead_"));
        if let Some(kind) = coral(color) {
            return named(&format!("{dead}{kind}_coral_fan"));
        }
    }
    if let Some(rest) = name.strip_prefix("coral_plant_") {
        let (color, dead) = rest.strip_suffix("_dead").map_or((rest, ""), |c| (c, "dead_"));
        if let Some(kind) = coral(color) {
            return named(&format!("{dead}{kind}_coral"));
        }
    }
    if let Some(rest) = name.strip_prefix("coral_") {
        let (color, dead) = rest.strip_suffix("_dead").map_or((rest, ""), |c| (c, "dead_"));
        if let Some(kind) = coral(color) {
            return named(&format!("{dead}{kind}_coral_block"));
        }
    }
    // Fungus stems: Bedrock calls them logs, and some of its files spell "stripped" otherwise.
    {
        let (stripped, base) = ["stripped_", "strd_", "stred_", "strped_"]
            .iter()
            .find_map(|p| name.strip_prefix(p))
            .map_or(("", name), |rest| ("stripped_", rest));
        for (suffix, java) in [
            ("_stem_side", ""),
            ("_log_side", ""),
            ("_stem_top", "_top"),
            ("_log_top", "_top"),
        ] {
            if let Some(fungus) = base.strip_suffix(suffix)
                && matches!(fungus, "crimson" | "warped")
            {
                return named(&format!("{stripped}{fungus}_stem{java}"));
            }
        }
    }
    if let Some(w) = name.strip_suffix("_log_side") {
        return named(&format!("{w}_log"));
    }
    if let Some(n) = name.strip_suffix("_nylium_top") {
        return named(&format!("{n}_nylium"));
    }
    for (suffix, java) in [("_door_lower", "_door_bottom"), ("_door_upper", "_door_top")] {
        if let Some(w) = name.strip_suffix(suffix) {
            return named(&format!("{w}{java}"));
        }
    }
    if let Some(n) = name.strip_prefix("double_plant_") {
        let n = n
            .replace("paeonia", "peony")
            .replace("syringa", "lilac")
            .replace("rose_", "rose_bush_");
        let n = match n.split_once('_') {
            Some(("grass", rest)) => format!("tall_grass_{rest}"),
            Some(("fern", rest)) => format!("large_fern_{rest}"),
            _ => n,
        };
        if !n.ends_with("_carried") {
            return named(&n);
        }
    }
    if let Some(w) = name.strip_prefix("leaves_")
        && let Some(java) = match w {
            "oak" => Some("oak"),
            "spruce" => Some("spruce"),
            "birch" => Some("birch"),
            "jungle" => Some("jungle"),
            "acacia" => Some("acacia"),
            "big_oak" => Some("dark_oak"),
            _ => None,
        }
    {
        return named(&format!("{java}_leaves"));
    }
    if name == "reeds" {
        return named("sugar_cane");
    }
    if let Some(n) = name.strip_prefix("stonecutter2_") {
        return named(&format!("stonecutter_{n}"));
    }
    for crop in ["wheat", "carrots", "potatoes", "beetroots", "nether_wart", "cocoa"] {
        if let Some(n) = name
            .strip_prefix(crop)
            .and_then(|r| r.strip_prefix("_stage_"))
            && n.chars().all(|c| c.is_ascii_digit())
        {
            return named(&format!("{crop}_stage{n}"));
        }
    }
    if let Some(flower) = name.strip_prefix("flower_") {
        let java = match flower {
            "houstonia" => "azure_bluet",
            "tulip_red" => "red_tulip",
            "tulip_orange" => "orange_tulip",
            "tulip_pink" => "pink_tulip",
            "tulip_white" => "white_tulip",
            "paeonia" => "peony",
            "rose" => "poppy",
            other => other,
        };
        if !java.ends_with(|c: char| c.is_ascii_digit()) {
            return named(java);
        }
    }
    named(match name {
        "hardened_clay" => "terracotta",
        "cobblestone_mossy" => "mossy_cobblestone",
        "stone_andesite" => "andesite",
        "stone_diorite" => "diorite",
        "stone_granite" => "granite",
        "stone_andesite_smooth" => "polished_andesite",
        "stone_diorite_smooth" => "polished_diorite",
        "stone_granite_smooth" => "polished_granite",
        "nether_brick" => "nether_bricks",
        "red_nether_brick" => "red_nether_bricks",
        "brick" => "bricks",
        "prismarine_dark" => "dark_prismarine",
        "prismarine_rough" => "prismarine",
        "pumpkin_face_off" => "carved_pumpkin",
        "pumpkin_face_on" => "jack_o_lantern",
        "pale_moss" => "pale_moss_block",
        "deepslate/cobbled_deepslate" => "cobbled_deepslate",
        "stonebrick" => "stone_bricks",
        "stonebrick_carved" => "chiseled_stone_bricks",
        "stonebrick_cracked" => "cracked_stone_bricks",
        "stonebrick_mossy" => "mossy_stone_bricks",
        "grass_top" => "grass_block_top",
        "grass_side" => "grass_block_side",
        "grass_side_snowed" => "grass_block_snow",
        "grass_path_side" => "dirt_path_side",
        "grass_path_top" => "dirt_path_top",
        "dirt_podzol_side" => "podzol_side",
        "dirt_podzol_top" => "podzol_top",
        "tallgrass" => "short_grass",
        "deadbush" => "dead_bush",
        "waterlily" => "lily_pad",
        "web" => "cobweb",
        "sponge_wet" => "wet_sponge",
        "slime" => "slime_block",
        "ice_packed" => "packed_ice",
        "quartz_ore" => "nether_quartz_ore",
        "furnace_front_off" => "furnace_front",
        "blast_furnace_front_off" => "blast_furnace_front",
        "smoker_front_off" => "smoker_front",
        "farmland_dry" => "farmland",
        "farmland_wet" => "farmland_moist",
        "rail_normal" => "rail",
        "rail_normal_turned" => "rail_corner",
        "rail_golden" => "powered_rail",
        "rail_golden_powered" => "powered_rail_on",
        "rail_detector" => "detector_rail",
        "rail_detector_powered" => "detector_rail_on",
        "rail_activator" => "activator_rail",
        "rail_activator_powered" => "activator_rail_on",
        "pumpkin_stem_connected" => "attached_pumpkin_stem",
        "pumpkin_stem_disconnected" => "pumpkin_stem",
        "melon_stem_connected" => "attached_melon_stem",
        "melon_stem_disconnected" => "melon_stem",
        "mushroom_block_skin_brown" => "brown_mushroom_block",
        "mushroom_block_skin_red" => "red_mushroom_block",
        "mushroom_block_skin_stem" => "mushroom_stem",
        "mushroom_brown" => "brown_mushroom",
        "mushroom_red" => "red_mushroom",
        "quartz_block_chiseled" => "chiseled_quartz_block",
        "quartz_block_chiseled_top" => "chiseled_quartz_block_top",
        "quartz_block_lines" => "quartz_pillar",
        "quartz_block_lines_top" => "quartz_pillar_top",
        "sandstone_normal" => "sandstone",
        "red_sandstone_normal" => "red_sandstone",
        "shulker_top_undyed" => "shulker_box",
        "sandstone_carved" => "chiseled_sandstone",
        "sandstone_smooth" => "cut_sandstone",
        "red_sandstone_carved" => "chiseled_red_sandstone",
        "red_sandstone_smooth" => "cut_red_sandstone",
        "stone_slab_top" => "smooth_stone",
        "stone_slab_side" => "smooth_stone_slab_side",
        "end_bricks" => "end_stone_bricks",
        "noteblock" => "note_block",
        "redstone_lamp_off" => "redstone_lamp",
        "observer_back_lit" => "observer_back_on",
        "comparator_off" => "comparator",
        "torch_on" => "torch",
        "redstone_torch_on" => "redstone_torch",
        "mob_spawner" => "spawner",
        "honey_bottom" => "honey_block_bottom",
        "honey_side" => "honey_block_side",
        "honey_top" => "honey_block_top",
        "honeycomb" => "honeycomb_block",
        "beehive_top" => "beehive_end",
        "trapdoor" => "oak_trapdoor",
        "trip_wire" => "tripwire",
        "trip_wire_source" => "tripwire_hook",
        "magma_0" => "magma",
        "compost" => "composter_compost",
        "compost_ready" => "composter_ready",
        "anvil_base" => "anvil",
        "bamboo_stem" => "bamboo_stalk",
        "cave_vines_head" => "cave_vines",
        "cave_vines_head_berries" => "cave_vines_lit",
        "cave_vines_body" => "cave_vines_plant",
        "cave_vines_body_berries" => "cave_vines_plant_lit",
        "twisting_vines_base" => "twisting_vines_plant",
        "twisting_vines_bottom" => "twisting_vines",
        "weeping_vines_base" => "weeping_vines_plant",
        "weeping_vines_bottom" => "weeping_vines",
        "dirt_with_roots" => "rooted_dirt",
        "firefly_bush_firefly" => "firefly_bush_emissive",
        "fletcher_table_side1" => "fletching_table_side",
        "fletcher_table_top" => "fletching_table_top",
        "repeater_off" => "repeater",
        "endframe_top" => "end_portal_frame_top",
        "endframe_side" => "end_portal_frame_side",
        "endframe_eye" => "end_portal_frame_eye",
        "dispenser_front_horizontal" => "dispenser_front",
        "dropper_front_horizontal" => "dropper_front",
        "turtle_egg_not_cracked" => "turtle_egg",
        "command_block_front_mipmap" => "command_block_front",
        "command_block_back_mipmap" => "command_block_back",
        "command_block_side_mipmap" => "command_block_side",
        "command_block_conditional_mipmap" => "command_block_conditional",
        other => other,
    })
}

/// A Bedrock item texture's name in Java.
fn java_item(name: &str) -> String {
    for (bedrock, java) in [("wood_", "wooden_"), ("gold_", "golden_")] {
        if let Some(rest) = name.strip_prefix(bedrock)
            && matches!(
                rest,
                "sword"
                    | "axe"
                    | "pickaxe"
                    | "shovel"
                    | "hoe"
                    | "helmet"
                    | "chestplate"
                    | "leggings"
                    | "boots"
                    | "horse_armor"
            )
        {
            return format!("{java}{rest}");
        }
    }
    for (bedrock, java) in [("wood_", "wooden_"), ("gold_", "golden_")] {
        if let Some(rest) = name.strip_prefix(bedrock)
            && (rest == "spear" || rest == "nautilus_armor")
        {
            return format!("{java}{rest}");
        }
    }
    if let Some(color) = name.strip_prefix("bed_") {
        return format!("{}_bed", dye(color));
    }
    if let Some(disc) = name.strip_prefix("record_") {
        return format!("music_disc_{disc}");
    }
    if let Some(mob) = name.strip_prefix("egg_") {
        let mob = match mob {
            "elderguardian" => "elder_guardian",
            "mushroomcow" => "mooshroom",
            "pigzombie" => "zombified_piglin",
            "polarbear" => "polar_bear",
            "skeletonhorse" => "skeleton_horse",
            "zombiehorse" => "zombie_horse",
            "zombievillager" => "zombie_villager",
            "wanderingtrader" => "wandering_trader",
            "lava_slime" => "magma_cube",
            "clownfish" | "tropicalfish" => "tropical_fish",
            "fish" => "cod",
            "irongolem" => "iron_golem",
            "snowgolem" => "snow_golem",
            other => other,
        };
        return format!("{mob}_spawn_egg");
    }
    if let Some(color) = name.strip_prefix("dye_powder_") {
        let color = color.strip_suffix("_new").unwrap_or(color);
        let old = !name.ends_with("_new");
        return match color {
            "black" if old => "ink_sac".into(),
            "blue" if old => "lapis_lazuli".into(),
            "brown" if old => "cocoa_beans".into(),
            "white" if old => "bone_meal".into(),
            "glow" => "glow_ink_sac".into(),
            c => format!("{}_dye", dye(c)),
        };
    }
    if let Some(kind) = name.strip_prefix("bucket_") {
        return match kind {
            "empty" => "bucket".into(),
            "tropical" => "tropical_fish_bucket".into(),
            k => format!("{k}_bucket"),
        };
    }
    for kind in ["boat", "door", "sign"] {
        if let Some(wood_name) = name
            .strip_prefix(kind)
            .and_then(|r| r.strip_prefix('_'))
        {
            return format!("{}_{kind}", wood(wood_name));
        }
    }
    if let Some(crop) = name.strip_prefix("seeds_") {
        return format!("{crop}_seeds");
    }
    if let Some(rest) = name.strip_prefix("bundle_") {
        let (color, open) = match rest.find("_open") {
            Some(at) => (&rest[..at], &rest[at..]),
            None => (rest, ""),
        };
        if !color.is_empty() {
            return format!("{}_bundle{open}", dye(color));
        }
    }
    match name {
        "crossbow_standby" => "crossbow",
        "boat" => "oak_boat",
        "sign" => "oak_sign",
        "redstone_dust" => "redstone",
        "netherbrick" => "nether_brick",
        "fireball" => "fire_charge",
        "reeds" => "sugar_cane",
        "bow_standby" => "bow",
        "fishing_rod_uncast" => "fishing_rod",
        "dragons_breath" => "dragon_breath",
        "fireworks" => "firework_rocket",
        "heartofthesea_closed" => "heart_of_the_sea",
        "nautilus" => "nautilus_shell",
        "villagebell" => "bell",
        "chorus_fruit_popped" => "popped_chorus_fruit",
        "melon" => "melon_slice",
        "melon_speckled" => "glistering_melon_slice",
        "apple_golden" => "golden_apple",
        "carrot_golden" => "golden_carrot",
        "potato_baked" => "baked_potato",
        "potato_poisonous" => "poisonous_potato",
        "beef_cooked" => "cooked_beef",
        "beef_raw" => "beef",
        "porkchop_cooked" => "cooked_porkchop",
        "porkchop_raw" => "porkchop",
        "mutton_cooked" => "cooked_mutton",
        "mutton_raw" => "mutton",
        "chicken_cooked" => "cooked_chicken",
        "chicken_raw" => "chicken",
        "rabbit_cooked" => "cooked_rabbit",
        "rabbit_raw" => "rabbit",
        "fish_raw" => "cod",
        "fish_cooked" => "cooked_cod",
        "fish_salmon_raw" => "salmon",
        "fish_salmon_cooked" => "cooked_salmon",
        "fish_clownfish_raw" => "tropical_fish",
        "fish_pufferfish_raw" => "pufferfish",
        "book_enchanted" => "enchanted_book",
        "book_normal" => "book",
        "book_writable" => "writable_book",
        "book_written" => "written_book",
        "minecart_normal" => "minecart",
        "minecart_chest" => "chest_minecart",
        "minecart_command_block" => "command_block_minecart",
        "minecart_furnace" => "furnace_minecart",
        "minecart_hopper" => "hopper_minecart",
        "minecart_tnt" => "tnt_minecart",
        "map_empty" => "map",
        "map_filled" => "filled_map",
        "potion_bottle_empty" => "glass_bottle",
        "potion_bottle_drinkable" => "potion",
        "potion_bottle_splash" => "splash_potion",
        "potion_bottle_lingering" => "lingering_potion",
        "chain" => "iron_chain",
        "slimeball" => "slime_ball",
        "spider_eye_fermented" => "fermented_spider_eye",
        "totem" => "totem_of_undying",
        "turtle_shell_piece" => "turtle_scute",
        "broken_elytra" => "elytra_broken",
        "tipped_arrow" => "tipped_arrow_head",
        other => other,
    }
    .into()
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-./".contains(&b))
        && !s.contains("..")
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

/// Textures a pack's entities, attachables and particles name.
pub fn referenced_textures(pack: &Pack) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        if !p.ends_with(".json") {
            continue;
        }
        let in_entity = p.starts_with("entity/") || p.starts_with("attachables/");
        let in_particles = p.starts_with("particles/");
        if !in_entity && !in_particles {
            continue;
        }
        let Ok(v) = pack.read_json(&p) else { continue };
        if in_entity {
            if let Some(def) = ClientEntity::parse(&v) {
                out.extend(def.textures.into_values());
            }
        } else if let Some(fx) = Effect::parse(&v)
            && !fx.texture.is_empty()
        {
            out.insert(fx.texture);
        }
    }
    out
}

/// Finds a texture's file (`.png`, `.tga`) and returns it as PNG bytes, no bigger than the
/// game's atlas takes.
fn texture_png(pack: &Pack, texture: &str, lift: bool, tall: bool) -> Option<Vec<u8>> {
    let png = if let Ok(b) = pack.read(&format!("{texture}.png")) {
        b
    } else {
        let tga = pack.read(&format!("{texture}.tga")).ok()?;
        encode_png(&decode_tga(&tga)?)?
    };
    let png = fit_to_atlas(&png, tall).unwrap_or(png);
    // A texture's faint texels (an eye painted at alpha 3) are lifted.
    Some(if lift {
        lift_faint_alpha(&png).unwrap_or(png)
    } else {
        png
    })
}

/// The widest and tallest texture the game's atlas packs (`HudAtlas::pack`): a bigger one is
/// left out of it, and a model wearing it is not drawn.
const ATLAS_MAX_WIDTH: u32 = 1024;
const ATLAS_MAX_HEIGHT: u32 = 512;
/// An entity's or attachable's texture (`entity/bedrock/...`) may be 1024 tall: a model's UVs
/// reach single texels of it (an eye's face, a pupil), and averaging pairs of texels blends
/// the neighbours of an odd offset into them.
const ENTITY_MAX_HEIGHT: u32 = 1024;

/// The side factor (a power of two) that brings a `width` x `height` texture within the atlas
/// (`tall`: an entity's texture, which the atlas takes up to `ENTITY_MAX_HEIGHT`).
fn atlas_shrink(width: u32, height: u32, tall: bool) -> u32 {
    let max_height = if tall { ENTITY_MAX_HEIGHT } else { ATLAS_MAX_HEIGHT };
    let mut factor = 1;
    while width.div_ceil(factor) > ATLAS_MAX_WIDTH || height.div_ceil(factor) > max_height {
        factor *= 2;
    }
    factor
}

/// A texture past the atlas's size, averaged down by a whole factor (a model's UVs are
/// fractions of its texture, so it still wears the same picture). `None` when it fits.
fn fit_to_atlas(png_bytes: &[u8], tall: bool) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let (w, h) = (reader.info().width, reader.info().height);
    let factor = atlas_shrink(w, h, tall);
    if factor == 1 {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let src = &buf[..info.buffer_size()];
    let rgba = |i: usize| -> [u32; 4] {
        match info.color_type {
            png::ColorType::Rgba => {
                [src[i * 4].into(), src[i * 4 + 1].into(), src[i * 4 + 2].into(), src[i * 4 + 3].into()]
            }
            png::ColorType::Rgb => [src[i * 3].into(), src[i * 3 + 1].into(), src[i * 3 + 2].into(), 255],
            png::ColorType::GrayscaleAlpha => {
                [src[i * 2].into(), src[i * 2].into(), src[i * 2].into(), src[i * 2 + 1].into()]
            }
            _ => [src[i].into(), src[i].into(), src[i].into(), 255],
        }
    };
    let (nw, nh) = (w.div_ceil(factor), h.div_ceil(factor));
    let mut out = Vec::with_capacity((nw * nh * 4) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let (mut sum, mut alpha, mut count) = ([0u64; 3], 0u64, 0u64);
            for sy in (y * factor)..((y + 1) * factor).min(h) {
                for sx in (x * factor)..((x + 1) * factor).min(w) {
                    let p = rgba((sy * w + sx) as usize);
                    let a = u64::from(p[3]);
                    for c in 0..3 {
                        sum[c] += u64::from(p[c]) * a;
                    }
                    alpha += a;
                    count += 1;
                }
            }
            if alpha == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                for c in sum {
                    out.push((c / alpha) as u8);
                }
                out.push((alpha / count) as u8);
            }
        }
    }
    encode_png(&Image {
        width: nw,
        height: nh,
        rgba: out,
    })
}

/// What an emissive material lights in a texture: the texels that are neither clear nor opaque,
/// the more the lower their alpha. Drawn at full brightness over the lit model. `None` when the
/// texture has none.
fn glow_png(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let pixels = &mut buf[..info.buffer_size()];
    let mut any = false;
    for px in pixels.chunks_exact_mut(4) {
        if (1..255).contains(&px[3]) {
            px[3] = 255 - px[3];
            any = true;
        } else {
            px.copy_from_slice(&[0, 0, 0, 0]);
        }
    }
    any.then(|| {
        encode_png(&Image {
            width: info.width,
            height: info.height,
            rgba: pixels.to_vec(),
        })
    })
    .flatten()
}

/// The texture as an emissive material draws it: its alpha is how much a texel glows, not how
/// see-through it is, so every texel that is not clear is opaque. `None` when the texture has no
/// texel between clear and opaque (it is drawn as it is).
fn solid_png(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let pixels = &mut buf[..info.buffer_size()];
    let mut any = false;
    for px in pixels.chunks_exact_mut(4) {
        if (1..255).contains(&px[3]) {
            px[3] = 255;
            any = true;
        }
    }
    any.then(|| {
        encode_png(&Image {
            width: info.width,
            height: info.height,
            rgba: pixels.to_vec(),
        })
    })
    .flatten()
}

/// The highest alpha a texel can have and still count as "faint".
const FAINT_ALPHA: u8 = 8;

/// A cut-out texture whose texels are drawn at an alpha of 1 to 8 (a villager's eye white and
/// pupil are 2 in a pack that wants them shown) has them made solid: Bedrock draws what the
/// pack painted there, while the game's alpha test throws them away. Textures with any real
/// translucency (a glow, a ghost) are left alone, since their faint texels are faint on purpose.
/// `None` when nothing changes.
fn lift_faint_alpha(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let pixels = &mut buf[..info.buffer_size()];
    let (mut faint, mut solid, mut translucent) = (0usize, 0usize, false);
    let mut faint_white = true;
    for px in pixels.chunks_exact(4) {
        match px[3] {
            1..=FAINT_ALPHA => {
                faint += 1;
                faint_white &= px[..3].iter().all(|&c| c >= 240);
            }
            255 => solid += 1,
            a if a > FAINT_ALPHA => translucent = true,
            _ => {}
        }
    }
    // A few white faint texels in a body of solid ones (an allay's two eye strips beside its
    // translucent wing) are eyes, translucency elsewhere or not.
    if translucent && faint_white && faint <= 32 && faint * 8 <= solid {
        translucent = false;
    }
    // Faint texels that outnumber the solid ones are a filler (a profession's clothes sit on
    // white at alpha 1): the pack means them clear, not an eye to show.
    if faint == 0 || translucent || (solid > 0 && faint > solid) {
        return None;
    }
    for px in pixels.chunks_exact_mut(4) {
        if (1..=FAINT_ALPHA).contains(&px[3]) {
            px[3] = 255;
        }
    }
    encode_png(&Image {
        width: info.width,
        height: info.height,
        rgba: pixels.to_vec(),
    })
}

/// A decoded image: RGBA, top row first.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Targa: uncompressed or run-length, 8 (grey or indexed), 24 or 32 bit.
pub fn decode_tga(b: &[u8]) -> Option<Image> {
    if b.len() < 18 {
        return None;
    }
    let id_len = b[0] as usize;
    let cmap_type = b[1];
    let kind = b[2];
    let cmap_first = u16::from_le_bytes([b[3], b[4]]) as usize;
    let cmap_len = u16::from_le_bytes([b[5], b[6]]) as usize;
    let cmap_bits = b[7] as usize;
    let width = u16::from_le_bytes([b[12], b[13]]) as usize;
    let height = u16::from_le_bytes([b[14], b[15]]) as usize;
    let bits = b[16] as usize;
    let desc = b[17];
    if width == 0 || height == 0 || width * height > 1 << 26 {
        return None;
    }
    let rle = matches!(kind, 9..=11);
    let base = kind & 7;
    if !matches!(base, 1..=3) || bits % 8 != 0 || bits == 0 {
        return None;
    }
    let bpp = bits / 8;
    let mut at = 18 + id_len;
    let mut palette: Vec<[u8; 4]> = Vec::new();
    if cmap_type == 1 {
        let entry = cmap_bits.div_ceil(8);
        let mut cm = vec![[0, 0, 0, 255]; cmap_first + cmap_len];
        for i in 0..cmap_len {
            let e = b.get(at..at + entry)?;
            cm[cmap_first + i] = pixel(e, entry);
            at += entry;
        }
        palette = cm;
    }
    let px = |raw: &[u8]| -> [u8; 4] {
        match base {
            1 => palette
                .get(raw[0] as usize)
                .copied()
                .unwrap_or([0, 0, 0, 255]),
            3 => {
                if bpp == 1 {
                    [raw[0], raw[0], raw[0], 255]
                } else {
                    [raw[0], raw[0], raw[0], raw[1]]
                }
            }
            _ => pixel(raw, bpp),
        }
    };
    let total = width * height;
    let mut pixels: Vec<[u8; 4]> = Vec::with_capacity(total);
    if rle {
        while pixels.len() < total {
            let h = *b.get(at)?;
            at += 1;
            let n = (h & 0x7f) as usize + 1;
            if h & 0x80 != 0 {
                let p = px(b.get(at..at + bpp)?);
                at += bpp;
                pixels.extend(std::iter::repeat_n(p, n));
            } else {
                for _ in 0..n {
                    pixels.push(px(b.get(at..at + bpp)?));
                    at += bpp;
                }
            }
        }
        pixels.truncate(total);
    } else {
        for _ in 0..total {
            pixels.push(px(b.get(at..at + bpp)?));
            at += bpp;
        }
    }
    let top_first = desc & 0x20 != 0;
    let right_first = desc & 0x10 != 0;
    let mut rgba = vec![0u8; total * 4];
    for y in 0..height {
        let src_y = if top_first { y } else { height - 1 - y };
        for x in 0..width {
            let src_x = if right_first { width - 1 - x } else { x };
            rgba[(y * width + x) * 4..][..4].copy_from_slice(&pixels[src_y * width + src_x]);
        }
    }
    Some(Image {
        width: width as u32,
        height: height as u32,
        rgba,
    })
}

/// One pixel of 8 (grey), 16 (5-5-5-1), 24 or 32 bits, from BGR(A).
fn pixel(raw: &[u8], bpp: usize) -> [u8; 4] {
    match bpp {
        1 => [raw[0], raw[0], raw[0], 255],
        2 => {
            let v = u16::from_le_bytes([raw[0], raw[1]]);
            let c = |s: u16| (((v >> s) & 31) as u32 * 255 / 31) as u8;
            [c(10), c(5), c(0), if v & 0x8000 != 0 { 255 } else { 0 }]
        }
        3 => [raw[2], raw[1], raw[0], 255],
        _ => [raw[2], raw[1], raw[0], raw[3]],
    }
}

fn encode_png(img: &Image) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&img.rgba).ok()?;
    }
    Some(out)
}

fn average(v: Option<&Value>, d: f64) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(d),
        Some(Value::Array(a)) if a.len() >= 2 => {
            (a[0].as_f64().unwrap_or(d) + a[1].as_f64().unwrap_or(d)) / 2.0
        }
        _ => d,
    }
}

/// `sound_definitions` as Java sounds: files copied, one `sounds.json` per namespace.
fn convert_sounds(pack: &Pack, out: &Path, report: &mut Report) -> Result<()> {
    let Ok(root) = pack.read_json("sounds/sound_definitions.json") else {
        return Ok(());
    };
    let Some(defs) = root.get("sound_definitions").and_then(Value::as_object) else {
        return Ok(());
    };
    let mut by_ns: BTreeMap<String, serde_json::Map<String, Value>> = BTreeMap::new();
    let mut copied: BTreeSet<String> = BTreeSet::new();
    for (key, def) in defs {
        let key = key.to_ascii_lowercase();
        let (ns, event) = key
            .split_once(':')
            .unwrap_or((DEFAULT_NAMESPACE, key.as_str()));
        if !valid_name(ns) || !valid_name(event) {
            continue;
        }
        let Some(list) = def.get("sounds").and_then(Value::as_array) else {
            continue;
        };
        let mut sounds = Vec::new();
        for s in list {
            let (name, volume, pitch, weight, stream) = match s {
                Value::String(n) => (n.as_str(), 1.0, 1.0, 1, false),
                Value::Object(o) => (
                    o.get("name").and_then(Value::as_str).unwrap_or(""),
                    average(o.get("volume"), 1.0),
                    average(o.get("pitch"), 1.0),
                    o.get("weight").and_then(Value::as_u64).unwrap_or(1),
                    o.get("stream").and_then(Value::as_bool).unwrap_or(false),
                ),
                _ => continue,
            };
            let rel = name
                .strip_prefix("sounds/")
                .unwrap_or(name)
                .to_ascii_lowercase();
            if !valid_name(&rel) {
                continue;
            }
            let java_path = out.join(format!("assets/{ns}/sounds/{rel}.ogg"));
            if copied.contains(&java_path.to_string_lossy().into_owned()) || java_path.exists() {
                // already written
            } else if let Ok(bytes) = pack.read(&format!("{name}.ogg")) {
                write(&java_path, &bytes)?;
                copied.insert(java_path.to_string_lossy().into_owned());
                report.sounds += 1;
            } else {
                if pack.contains(&format!("{name}.wav")) || pack.contains(&format!("{name}.fsb")) {
                    report.skipped_sounds += 1;
                }
                continue;
            }
            let mut entry =
                json!({"name": format!("{ns}:{rel}"), "volume": volume, "pitch": pitch});
            if weight > 1 {
                entry["weight"] = json!(weight);
            }
            if stream {
                entry["stream"] = json!(true);
            }
            sounds.push(entry);
        }
        if sounds.is_empty() {
            continue;
        }
        let mut event_json = json!({"sounds": sounds});
        if let Some(c) = def.get("category").and_then(Value::as_str) {
            event_json["category"] = json!(c);
        }
        by_ns
            .entry(ns.to_owned())
            .or_default()
            .insert(event.to_owned(), event_json);
        report.sound_events += 1;
    }
    for (ns, events) in by_ns {
        write(
            &out.join(format!("assets/{ns}/sounds.json")),
            serde_json::to_string(&Value::Object(events))
                .map_err(|e| Error::Json(e.to_string()))?
                .as_bytes(),
        )?;
    }
    Ok(())
}

/// Writes the Java form of `pack` into `out` (created; existing files are replaced).
/// `format` is the Java resource pack format `pack.mcmeta` declares.
pub fn to_java(pack: &Pack, out: &Path, format: (i64, i64)) -> Result<Report> {
    let mut report = Report::default();
    fs::create_dir_all(out)?;
    let name = pack.manifest().name.clone();
    let meta = json!({"pack": {
        "description": format!("Bedrock: {name}"),
        "min_format": [format.0, format.1],
        "max_format": [format.0, format.1],
    }});
    write(&out.join("pack.mcmeta"), meta.to_string().as_bytes())?;
    if let Ok(icon) = pack.read("pack_icon.png") {
        write(&out.join("pack.png"), &icon)?;
    }
    // The pack's own files.
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        let wanted = DATA_FOLDERS
            .iter()
            .any(|d| p.starts_with(d) && (p.ends_with(".json") || p.ends_with(".material")))
            || p == "sounds.json"
            || p == "blocks.json"
            || p == "biomes_client.json"
            || p == "sounds/sound_definitions.json";
        if !wanted {
            continue;
        }
        write(
            &out.join(format!("{DATA_DIR}/{}/{p}", namespace(pack))),
            &pack.read(&p)?,
        )?;
        report.data_files += 1;
    }
    // Textures.
    for t in referenced_textures(pack) {
        match texture_png(pack, &t, true, true) {
            Some(bytes) => {
                let name = atlas_name(&t);
                if !valid_name(&name) {
                    continue;
                }
                write(
                    &out.join(format!("assets/minecraft/textures/{name}.png")),
                    &bytes,
                )?;
                report.textures += 1;
                // The texels an emissive material lights (alpha below opaque), apart.
                if let Some(glow) = texture_png(pack, &t, false, true).and_then(|b| glow_png(&b)) {
                    write(
                        &out.join(format!("assets/minecraft/textures/{name}_glow.png")),
                        &glow,
                    )?;
                }
                // The same texture with those texels opaque: what an emissive material shows
                // under the glow.
                if let Some(solid) = texture_png(pack, &t, false, true).and_then(|b| solid_png(&b)) {
                    write(
                        &out.join(format!("assets/minecraft/textures/{name}_solid.png")),
                        &solid,
                    )?;
                }
            }
            None => report.missing_textures += 1,
        }
    }
    convert_world(pack, out, &mut report)?;
    convert_sounds(pack, out, &mut report)?;
    convert_settings(pack, out)?;
    Ok(report)
}

/// The sixteen dye colours, Java's names.
const DYES: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray",
    "cyan", "purple", "blue", "brown", "green", "red", "black",
];

/// A Bedrock effect's potion icons and the potions of the game they stand for.
const POTION_ICONS: [(&str, &[&str]); 20] = [
    ("movespeed", &["swiftness", "long_swiftness", "strong_swiftness"]),
    ("moveslowdown", &["slowness", "long_slowness", "strong_slowness"]),
    ("damageboost", &["strength", "long_strength", "strong_strength"]),
    ("jump", &["leaping", "long_leaping", "strong_leaping"]),
    ("heal", &["healing", "strong_healing"]),
    ("harm", &["harming", "strong_harming"]),
    ("poison", &["poison", "long_poison", "strong_poison"]),
    ("regeneration", &["regeneration", "long_regeneration", "strong_regeneration"]),
    ("fireresistance", &["fire_resistance", "long_fire_resistance"]),
    ("nightvision", &["night_vision", "long_night_vision"]),
    ("invisibility", &["invisibility", "long_invisibility"]),
    ("waterbreathing", &["water_breathing", "long_water_breathing"]),
    ("weakness", &["weakness", "long_weakness"]),
    ("slowfall", &["slow_falling", "long_slow_falling"]),
    ("turtlemaster", &["turtle_master", "long_turtle_master", "strong_turtle_master"]),
    ("luck", &["luck"]),
    ("infested", &["infested"]),
    ("oozing", &["oozing"]),
    ("weaving", &["weaving"]),
    ("windcharged", &["wind_charged"]),
];

/// An RGBA picture from a PNG.
fn decode_rgba(png_bytes: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let src = &buf[..info.buffer_size()];
    let n = (info.width * info.height) as usize;
    let mut rgba = Vec::with_capacity(n * 4);
    for i in 0..n {
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(&src[i * 4..i * 4 + 4]),
            png::ColorType::Rgb => rgba.extend_from_slice(&[src[i * 3], src[i * 3 + 1], src[i * 3 + 2], 255]),
            png::ColorType::GrayscaleAlpha => {
                rgba.extend_from_slice(&[src[i * 2], src[i * 2], src[i * 2], src[i * 2 + 1]]);
            }
            _ => rgba.extend_from_slice(&[src[i], src[i], src[i], 255]),
        }
    }
    Some(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}

/// The zombie's 64x32 skin as the game's 64x64 one: the left arm and leg, which the old layout
/// mirrored from the right, drawn where the new one keeps them.
fn zombie_skin(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let src = decode_rgba(png_bytes)?;
    if (src.width, src.height) != (64, 32) {
        return None;
    }
    let mut rgba = src.rgba.clone();
    rgba.resize(64 * 64 * 4, 0);
    // The hat layer: the old skin keeps stray texels there that a head would draw as a stripe.
    for y in 0..16usize {
        for x in 32..64usize {
            let at = (y * 64 + x) * 4;
            rgba[at..at + 4].copy_from_slice(&[0, 0, 0, 0]);
        }
    }
    // (source x, source y, destination x, destination y), 16x16 each.
    for (sx, sy, dx, dy) in [(0, 16, 16, 48), (40, 16, 32, 48)] {
        for y in 0..16usize {
            for x in 0..16usize {
                let from = ((sy + y) * 64 + sx + (15 - x)) * 4;
                let to = ((dy + y) * 64 + dx + x) * 4;
                let px: [u8; 4] = rgba[from..from + 4].try_into().ok()?;
                rgba[to..to + 4].copy_from_slice(&px);
            }
        }
    }
    encode_png(&Image {
        width: 64,
        height: 64,
        rgba,
    })
}

/// Terrain sprite of a flat block's carried icon, with the item it is the icon of.
fn carried_icons() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for same in [
        "torch", "soul_torch", "redstone_torch", "copper_torch", "end_rod", "lever", "ladder",
        "rail", "powered_rail", "activator_rail", "detector_rail", "iron_bars", "copper_bars",
        "exposed_copper_bars", "weathered_copper_bars", "oxidized_copper_bars", "lightning_rod",
        "allium", "azure_bluet", "blue_orchid", "cactus_flower", "cornflower", "dandelion",
        "golden_dandelion", "lilac", "peony", "rose_bush", "sunflower", "lily_of_the_valley",
        "oxeye_daisy", "poppy", "wither_rose", "torchflower", "pitcher_plant", "spore_blossom",
        "hanging_roots", "glow_lichen", "frogspawn", "sculk_vein", "vine", "twisting_vines",
        "weeping_vines", "crimson_fungus", "warped_fungus", "crimson_roots", "warped_roots",
        "seagrass", "short_dry_grass", "tall_dry_grass", "pale_hanging_moss", "sulfur_spike",
        "cherry_sapling", "pale_oak_sapling", "amethyst_cluster",
    ] {
        v.push((format!("{same}_carried"), same.into()));
    }
    for (bedrock, java) in [
        ("tulip_red", "red_tulip"),
        ("tulip_orange", "orange_tulip"),
        ("tulip_pink", "pink_tulip"),
        ("tulip_white", "white_tulip"),
        ("mushroom_brown", "brown_mushroom"),
        ("mushroom_red", "red_mushroom"),
        ("deadbush", "dead_bush"),
        ("dripstone", "pointed_dripstone"),
        ("eyeblossom_dormant", "closed_eyeblossom"),
        ("open_eyeblossom", "open_eyeblossom"),
        ("web", "cobweb"),
        ("amethyst_bud_large", "large_amethyst_bud"),
        ("amethyst_bud_medium", "medium_amethyst_bud"),
        ("amethyst_bud_small", "small_amethyst_bud"),
    ] {
        v.push((format!("{bedrock}_carried"), java.into()));
    }
    for (bedrock, java) in [
        ("oak", "oak"),
        ("spruce", "spruce"),
        ("birch", "birch"),
        ("jungle", "jungle"),
        ("acacia", "acacia"),
        ("roofed_oak", "dark_oak"),
    ] {
        v.push((format!("{bedrock}_carried_sapling"), format!("{java}_sapling")));
    }
    for (color, coral) in [
        ("blue", "tube"),
        ("pink", "brain"),
        ("purple", "bubble"),
        ("red", "fire"),
        ("yellow", "horn"),
    ] {
        for (kind, java_kind) in [("plant", ""), ("fan", "_fan")] {
            v.push((
                format!("coral_{kind}_{color}_carried"),
                format!("{coral}_coral{java_kind}"),
            ));
            v.push((
                format!("coral_{kind}_{color}_dead_carried"),
                format!("dead_{coral}_coral{java_kind}"),
            ));
        }
    }
    v
}

/// What the game draws from a model of its own and not from a flat sprite: beds, shulker boxes,
/// shields, heads and potions. A pack's picture reaches them by the entity texture of the same
/// layout, or by an item definition that points the item at the pack's icon.
fn convert_special_models(pack: &Pack, out: &Path, report: &mut Report) -> Result<()> {
    let by_lower: BTreeMap<String, String> = pack
        .paths()
        .map(|p| (p.to_ascii_lowercase(), p.to_owned()))
        .collect();
    let read = |path: &str| -> Option<Vec<u8>> {
        by_lower
            .get(path)
            .and_then(|real| pack.read(real).ok())
    };
    let textures = out.join("assets/minecraft/textures");
    // Entity textures laid out as the game's own.
    let mut entity: Vec<(String, Vec<String>)> = vec![
        (
            "textures/entity/shield.png".into(),
            vec![
                "entity/shield/shield_base_nopattern".into(),
                "entity/shield_base_nopattern".into(),
            ],
        ),
        (
            "textures/entity/shulker/shulker_undyed.png".into(),
            vec!["entity/shulker/shulker".into()],
        ),
        (
            "textures/entity/skulls/skeleton.png".into(),
            vec!["entity/skeleton/skeleton".into()],
        ),
        (
            "textures/entity/skulls/wither_skeleton.png".into(),
            vec!["entity/skeleton/wither_skeleton".into()],
        ),
        (
            "textures/entity/skulls/creeper.png".into(),
            vec!["entity/creeper/creeper".into()],
        ),
        (
            "textures/entity/piglin/piglin.png".into(),
            vec!["entity/piglin/piglin".into()],
        ),
        (
            "textures/entity/steve.png".into(),
            vec!["entity/player/wide/steve".into()],
        ),
        (
            "textures/entity/alex.png".into(),
            vec!["entity/player/slim/alex".into()],
        ),
    ];
    for color in DYES {
        let bedrock = if color == "light_gray" { "silver" } else { color };
        entity.push((
            format!("textures/entity/shulker/shulker_{bedrock}.png"),
            vec![format!("entity/shulker/shulker_{color}")],
        ));
    }
    for (source, targets) in entity {
        let Some(bytes) = read(&source) else {
            continue;
        };
        for target in targets {
            write(&textures.join(format!("{target}.png")), &bytes)?;
            report.world_textures += 1;
        }
    }
    // The dragon's head: its texture may be a Targa.
    if let Some(png) = texture_png(pack, "textures/entity/dragon/dragon", false, false) {
        write(&textures.join("entity/enderdragon/dragon.png"), &png)?;
        report.world_textures += 1;
    }
    if let Some(skin) = read("textures/entity/skulls/zombie.png").and_then(|b| zombie_skin(&b)) {
        write(&textures.join("entity/zombie/zombie.png"), &skin)?;
        report.world_textures += 1;
    }
    // Item definitions: the flat icon of a pack for an item the game draws from a model.
    let items = out.join("assets/minecraft/items");
    let models = out.join("assets/minecraft/models/item");
    let flat = |sprite: &str, model: &str| -> Result<()> {
        write(
            &models.join(format!("{model}.json")),
            json!({"parent": "minecraft:item/generated", "textures": {"layer0": format!("minecraft:item/{sprite}")}})
                .to_string()
                .as_bytes(),
        )
    };
    for color in DYES {
        let bedrock = if color == "light_gray" { "silver" } else { color };
        if !by_lower.contains_key(&format!("textures/items/bed_{bedrock}.png")) {
            continue;
        }
        let model = format!("{color}_bed_flat");
        flat(&format!("{color}_bed"), &model)?;
        write(
            &items.join(format!("{color}_bed.json")),
            json!({"model": {"type": "minecraft:model", "model": format!("minecraft:item/{model}")}})
                .to_string()
                .as_bytes(),
        )?;
        report.world_textures += 1;
    }
    for (item, prefix) in [
        ("potion", "potion_bottle_"),
        ("splash_potion", "potion_bottle_splash_"),
        ("lingering_potion", "potion_bottle_lingering_"),
    ] {
        let mut cases = Vec::new();
        for (effect, potions) in POTION_ICONS {
            let sprite = format!("{prefix}{effect}");
            if !by_lower.contains_key(&format!("textures/items/{sprite}.png")) {
                continue;
            }
            flat(&sprite, &sprite)?;
            cases.push(json!({
                "when": potions.iter().map(|p| format!("minecraft:{p}")).collect::<Vec<_>>(),
                "model": {"type": "minecraft:model", "model": format!("minecraft:item/{sprite}")},
            }));
        }
        if cases.is_empty() {
            continue;
        }
        write(
            &items.join(format!("{item}.json")),
            json!({"model": {
                "type": "minecraft:select",
                "property": "minecraft:component",
                "component": "minecraft:potion_contents",
                "cases": cases,
                "fallback": {
                    "type": "minecraft:model",
                    "model": format!("minecraft:item/{item}"),
                    "tints": [{"type": "minecraft:potion", "default": -13083194}],
                },
            }})
            .to_string()
            .as_bytes(),
        )?;
        report.world_textures += 1;
    }
    Ok(())
}

/// What a pack retextures of the game itself: blocks and items by the names Java gives them, the
/// sky, the colour maps, and the splash texts.
fn convert_world(pack: &Pack, out: &Path, report: &mut Report) -> Result<()> {
    for p in pack.paths().map(str::to_owned).collect::<Vec<_>>() {
        // A texture is a `.png`, or a `.tga` (leaves, ferns, tall grass, kelp, ...) when the pack
        // has no `.png` of that name.
        let (stem, tga) = if let Some(stem) = p.strip_suffix(".png") {
            (stem, false)
        } else if let Some(stem) = p.strip_suffix(".tga") {
            if pack.read(&format!("{stem}.png")).is_ok() {
                continue;
            }
            (stem, true)
        } else {
            continue;
        };
        let name = stem.to_ascii_lowercase();
        let target = if let Some(n) = name
            .strip_prefix("textures/blocks/")
            .or_else(|| name.strip_prefix("textures/items/"))
        {
            // `java_texture` knows the folders under these (candles, spawn eggs, ...); the
            // others are the pack's own.
            if is_material_map(n) {
                continue;
            }
            let java = java_texture(&name).filter(|j| {
                j.starts_with("block/") || j.starts_with("item/") || j.starts_with("entity/")
            });
            match java {
                Some(j) => j,
                None => continue,
            }
        } else if matches!(
            name.as_str(),
            "textures/environment/sun"
                | "textures/environment/moon_phases"
                | "textures/environment/clouds"
                | "textures/environment/rain"
                | "textures/environment/snow"
                | "textures/colormap/grass"
                | "textures/colormap/foliage"
        ) {
            name.trim_start_matches("textures/").to_owned()
        } else {
            continue;
        };
        if !valid_name(&target) {
            continue;
        }
        let bytes = if tga {
            match texture_png(pack, stem, false, false) {
                Some(bytes) => bytes,
                None => continue,
            }
        } else {
            pack.read(&p)?
        };
        write(
            &out.join(format!("assets/minecraft/textures/{target}.png")),
            &bytes,
        )?;
        report.world_textures += 1;
    }
    convert_special_models(pack, out, report)?;
    // `terrain_texture.json` and `item_texture.json` name the file a block or item uses, which a
    // pack may keep under any name: the file they name is the one the game's sprite gets.
    for (file, folder) in [
        ("textures/terrain_texture.json", "block"),
        ("textures/item_texture.json", "item"),
    ] {
        for (key, path) in texture_index(pack, file) {
            let Some(java) = java_texture(&format!(
                "textures/{}s/{}",
                folder,
                key.to_ascii_lowercase()
            )) else {
                continue;
            };
            if !valid_name(&java) || key == "missing" {
                continue;
            }
            let Some(bytes) = pack
                .read(&format!("{path}.png"))
                .ok()
                .or_else(|| texture_png(pack, &path, false, false))
            else {
                continue;
            };
            write(&out.join(format!("assets/minecraft/textures/{java}.png")), &bytes)?;
            report.world_textures += 1;
        }
    }
    convert_grass_side(pack, out, report)?;
    convert_carried_icons(out, report)?;
    // `flipbook_textures.json`: a texture that is a strip of frames. Java's `.png.mcmeta`.
    for book in flipbooks(pack) {
        let Some(java) = java_texture(&book.texture.to_ascii_lowercase()) else {
            continue;
        };
        let target = out.join(format!("assets/minecraft/textures/{java}.png"));
        if !valid_name(&java) || pack.read(&format!("{}.png", book.texture)).is_err() {
            continue;
        }
        write(&target, &pack.read(&format!("{}.png", book.texture))?)?;
        let mut animation = serde_json::Map::new();
        if let Some(ticks) = book.ticks {
            animation.insert("frametime".into(), json!(ticks.round().max(1.0) as i64));
        }
        if !book.frames.is_empty() {
            animation.insert("frames".into(), json!(book.frames));
        }
        animation.insert("interpolate".into(), json!(book.blend));
        write(
            &out.join(format!("assets/minecraft/textures/{java}.png.mcmeta")),
            json!({"animation": animation}).to_string().as_bytes(),
        )?;
        report.world_textures += 1;
    }
    // `splashes.json`: `{"splashes": ["..."]}`, Java's `texts/splashes.txt` one to a line.
    if let Ok(v) = pack.read_json("splashes.json")
        && let Some(list) = v.get("splashes").and_then(Value::as_array)
    {
        let lines: Vec<&str> = list
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.contains('\n'))
            .collect();
        if !lines.is_empty() {
            write(
                &out.join("assets/minecraft/texts/splashes.txt"),
                lines.join("\n").as_bytes(),
            )?;
            report.world_textures += lines.len();
        }
    }
    Ok(())
}

/// Inventory icons of flat blocks: the pack's `<name>_carried` terrain sprite. Run once every
/// block sprite is written (the `terrain_texture.json` ones come last).
fn convert_carried_icons(out: &Path, report: &mut Report) -> Result<()> {
    let items = out.join("assets/minecraft/items");
    let models = out.join("assets/minecraft/models/item");
    let blocks = out.join("assets/minecraft/textures/block");
    for (sprite, item) in carried_icons() {
        if !blocks.join(format!("{sprite}.png")).is_file() {
            continue;
        }
        let model = format!("{item}_carried");
        write(
            &models.join(format!("{model}.json")),
            json!({"parent": "minecraft:item/generated", "textures": {"layer0": format!("minecraft:block/{sprite}")}})
                .to_string()
                .as_bytes(),
        )?;
        write(
            &items.join(format!("{item}.json")),
            json!({"model": {"type": "minecraft:model", "model": format!("minecraft:item/{model}")}})
                .to_string()
                .as_bytes(),
        )?;
        report.world_textures += 1;
    }
    Ok(())
}

/// The grass block's side. Bedrock's `grass_side` keeps the grass fringe in the pixels whose alpha
/// is set (the tinted part) and the dirt in the others; Java draws `grass_block_side` (dirt) and
/// tints `grass_block_side_overlay` (the fringe) over it.
fn convert_grass_side(pack: &Pack, out: &Path, report: &mut Report) -> Result<()> {
    let Some(side) = texture_png(pack, "textures/blocks/grass_side", false, false)
        .and_then(|b| decode_rgba(&b))
    else {
        return Ok(());
    };
    // Not a mask: every pixel opaque, the picture already as it is drawn.
    if side.rgba.chunks_exact(4).all(|p| p[3] >= 128) {
        return Ok(());
    }
    let dirt = texture_png(pack, "textures/blocks/dirt", false, false)
        .and_then(|b| decode_rgba(&b))
        .filter(|d| d.width == side.width && d.height == side.height);
    let mut base = side.rgba.clone();
    let mut overlay = vec![0u8; side.rgba.len()];
    for (i, p) in side.rgba.chunks_exact(4).enumerate() {
        let at = i * 4;
        if p[3] >= 128 {
            overlay[at..at + 4].copy_from_slice(&[p[0], p[1], p[2], 255]);
            if let Some(dirt) = &dirt {
                base[at..at + 4].copy_from_slice(&dirt.rgba[at..at + 4]);
            }
        } else {
            base[at + 3] = 255;
        }
    }
    let (width, height) = (side.width, side.height);
    for (name, rgba) in [("grass_block_side", base), ("grass_block_side_overlay", overlay)] {
        let Some(png) = encode_png(&Image { width, height, rgba }) else {
            continue;
        };
        write(
            &out.join(format!("assets/minecraft/textures/block/{name}.png")),
            &png,
        )?;
        report.world_textures += 1;
    }
    Ok(())
}

/// A texture that is a strip of frames (`textures/flipbook_textures.json`).
struct Flipbook {
    /// `textures/blocks/water_still`.
    texture: String,
    ticks: Option<f64>,
    frames: Vec<i64>,
    blend: bool,
}

fn flipbooks(pack: &Pack) -> Vec<Flipbook> {
    let Ok(v) = pack.read_json("textures/flipbook_textures.json") else {
        return Vec::new();
    };
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(Flipbook {
                texture: e.get("flipbook_texture")?.as_str()?.to_owned(),
                ticks: e.get("ticks_per_frame").and_then(Value::as_f64),
                frames: e
                    .get("frames")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_i64).collect())
                    .unwrap_or_default(),
                blend: e.get("blend_frames").and_then(Value::as_bool).unwrap_or(true),
            })
        })
        .collect()
}

/// What `terrain_texture.json` / `item_texture.json` map: a name to the one texture file it
/// uses. An entry listing different files is by data value, which Java has no way to say, and
/// is left to the file names.
fn texture_index(pack: &Pack, file: &str) -> Vec<(String, String)> {
    let Ok(v) = pack.read_json(file) else {
        return Vec::new();
    };
    let Some(data) = v.get("texture_data").and_then(Value::as_object) else {
        return Vec::new();
    };
    data.iter()
        .filter_map(|(key, entry)| {
            let paths: Vec<&str> = match entry.get("textures")? {
                Value::String(s) => vec![s.as_str()],
                Value::Array(a) => a
                    .iter()
                    .filter_map(|t| t.as_str().or_else(|| t.get("path")?.as_str()))
                    .collect(),
                _ => return None,
            };
            let first = *paths.first()?;
            paths
                .iter()
                .all(|p| *p == first)
                .then(|| (key.clone(), first.to_owned()))
        })
        .collect()
}

/// The folder under `assets/horizon_bedrock/` that holds one pack's data: its UUID, so that
/// packs stacked in the game (the overlay merges by path, last wins) never replace each other's
/// `materials/entity.material` or `sounds.json`.
pub fn namespace(pack: &Pack) -> String {
    let id: String = pack
        .manifest()
        .uuid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if id.is_empty() { "pack".to_owned() } else { id }
}

/// The pack's settings, labels looked up in its language file (`en_US`, else the first one),
/// in the pack's own folder.
fn convert_settings(pack: &Pack, out: &Path) -> Result<()> {
    let defs = crate::settings::of_pack(pack);
    if defs.is_empty() {
        return Ok(());
    }
    write(
        &out.join(format!(
            "{DATA_DIR}/{}/{}",
            namespace(pack),
            crate::settings::FILE
        )),
        crate::settings::to_json(&defs).to_string().as_bytes(),
    )
}

/// Bumped whenever the conversion's output changes, so packs converted by an older engine
/// convert again instead of coming out of the cache.
pub const CONVERTER_REVISION: u64 = 20;

/// A folder name for a converted pack: stable for the same input, different for others.
pub fn cache_name(source: &Path, modified_secs: u64, size: u64) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in source
        .to_string_lossy()
        .bytes()
        .chain(modified_secs.to_le_bytes())
        .chain(size.to_le_bytes())
        .chain(CONVERTER_REVISION.to_le_bytes())
    {
        h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
    }
    PathBuf::from(format!("{clean}-{h:016x}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn flat_blocks_take_the_packs_carried_icon() {
        let icons = super::carried_icons();
        let has = |sprite: &str, item: &str| icons.iter().any(|(s, i)| s == sprite && i == item);
        assert!(has("torch_carried", "torch"));
        assert!(has("end_rod_carried", "end_rod"));
        assert!(has("oak_carried_sapling", "oak_sapling"));
        assert!(has("roofed_oak_carried_sapling", "dark_oak_sapling"));
        assert!(has("coral_fan_blue_dead_carried", "dead_tube_coral_fan"));
        assert!(has("coral_plant_red_carried", "fire_coral"));
        assert!(has("tulip_red_carried", "red_tulip"));
    }

    #[test]
    fn the_old_zombie_skin_gains_the_new_layouts_left_limbs() {
        let mut rgba = vec![0u8; 64 * 32 * 4];
        // The right leg's leftmost texel of its first row, marked.
        let at = (16 * 64) * 4;
        rgba[at..at + 4].copy_from_slice(&[9, 8, 7, 255]);
        let old = encode_png(&Image {
            width: 64,
            height: 32,
            rgba,
        })
        .unwrap();
        let new = decode_rgba(&zombie_skin(&old).unwrap()).unwrap();
        assert_eq!((new.width, new.height), (64, 64));
        // Mirrored: it lands on the left leg's rightmost texel.
        let to = (48 * 64 + 16 + 15) * 4;
        assert_eq!(&new.rgba[to..to + 4], &[9, 8, 7, 255]);
        // The hat layer comes out clear.
        let mut marked = vec![0u8; 64 * 32 * 4];
        marked[(3 * 64 + 40) * 4..(3 * 64 + 40) * 4 + 4].copy_from_slice(&[1, 2, 3, 255]);
        let png = encode_png(&Image {
            width: 64,
            height: 32,
            rgba: marked,
        })
        .unwrap();
        let skin = decode_rgba(&zombie_skin(&png).unwrap()).unwrap();
        assert_eq!(skin.rgba[(3 * 64 + 40) * 4 + 3], 0);
    }

    #[test]
    fn vanilla_textures_map_to_the_games_sprites() {
        let j = |t: &str| java_texture(t).unwrap();
        assert_eq!(j("textures/blocks/stone"), "block/stone");
        assert_eq!(j("textures/entity/polarbear"), "entity/bear/polarbear");
        assert_eq!(
            j("textures/blocks/wool_colored_silver"),
            "block/light_gray_wool"
        );
        assert_eq!(
            j("textures/blocks/hardened_clay_stained_red"),
            "block/red_terracotta"
        );
        assert_eq!(j("textures/blocks/stone_andesite"), "block/andesite");
        assert_eq!(j("textures/blocks/leaves_oak"), "block/oak_leaves");
        assert_eq!(j("textures/blocks/leaves_big_oak"), "block/dark_oak_leaves");
        assert_eq!(j("textures/blocks/reeds"), "block/sugar_cane");
        assert_eq!(j("textures/blocks/double_plant_grass_top"), "block/tall_grass_top");
        assert_eq!(j("textures/blocks/double_plant_fern_bottom"), "block/large_fern_bottom");
        assert_eq!(j("textures/blocks/double_plant_syringa_top"), "block/lilac_top");
        assert_eq!(j("textures/blocks/candles/red_candle_lit"), "block/red_candle_lit");
        assert_eq!(j("textures/blocks/deepslate/deepslate_bricks"), "block/deepslate_bricks");
        assert_eq!(
            j("textures/blocks/huge_fungus/strd_crimson_stem_side"),
            "block/stripped_crimson_stem"
        );
        assert_eq!(j("textures/blocks/huge_fungus/warped_stem_top"), "block/warped_stem_top");
        assert_eq!(j("textures/blocks/shulker_top_silver"), "block/light_gray_shulker_box");
        assert_eq!(j("textures/blocks/sandstone_normal"), "block/sandstone");
        assert!(java_texture("textures/blocks/slab/oak_slab").is_none());
        assert!(java_texture("textures/blocks/deepslate/deepslate_mer").is_none());
        assert_eq!(j("textures/items/candles/blue_candle"), "item/blue_candle");
        assert_eq!(
            j("textures/items/spawn_eggs/spawn_egg_tropicalfish"),
            "item/tropical_fish_spawn_egg"
        );
        assert_eq!(
            j("textures/items/spawn_eggs/spawn_egg_zombified_piglin"),
            "item/zombified_piglin_spawn_egg"
        );
        assert_eq!(j("textures/items/harness/harness_silver"), "item/light_gray_harness");
        assert_eq!(j("textures/items/spear/wood_spear"), "item/wooden_spear");
        assert_eq!(
            j("textures/items/nautilus_armor/gold_nautilus_armor"),
            "item/golden_nautilus_armor"
        );
        assert_eq!(j("textures/items/bed_silver"), "item/light_gray_bed");
        assert_eq!(j("textures/items/villagebell"), "item/bell");
        assert_eq!(
            j("textures/blocks/angler_pottery_pattern"),
            "entity/decorated_pot/angler_pottery_pattern"
        );
        assert_eq!(j("textures/items/wood_sword"), "item/wooden_sword");
        assert_eq!(j("textures/items/gold_pickaxe"), "item/golden_pickaxe");
        assert_eq!(j("textures/items/diamond_axe"), "item/diamond_axe");
        // Names Bedrock gives its items and blocks and the game does not.
        for (bedrock, java) in [
            ("textures/items/record_13", "item/music_disc_13"),
            ("textures/items/egg_mushroomcow", "item/mooshroom_spawn_egg"),
            ("textures/items/dye_powder_blue", "item/lapis_lazuli"),
            ("textures/items/dye_powder_blue_new", "item/blue_dye"),
            ("textures/items/dye_powder_silver", "item/light_gray_dye"),
            ("textures/items/beef_cooked", "item/cooked_beef"),
            ("textures/items/fish_salmon_raw", "item/salmon"),
            ("textures/items/bucket_water", "item/water_bucket"),
            ("textures/items/bucket_empty", "item/bucket"),
            ("textures/items/boat_darkoak", "item/dark_oak_boat"),
            ("textures/items/door_wood", "item/oak_door"),
            ("textures/items/sign_spruce", "item/spruce_sign"),
            ("textures/items/seeds_wheat", "item/wheat_seeds"),
            ("textures/items/bundle_light_blue_open_front", "item/light_blue_bundle_open_front"),
            ("textures/items/gold_horse_armor", "item/golden_horse_armor"),
            ("textures/items/redstone_dust", "item/redstone"),
            ("textures/blocks/planks_big_oak", "block/dark_oak_planks"),
            ("textures/blocks/log_oak_top", "block/oak_log_top"),
            ("textures/blocks/glass_silver", "block/light_gray_stained_glass"),
            ("textures/blocks/glazed_terracotta_red", "block/red_glazed_terracotta"),
            ("textures/blocks/door_iron_lower", "block/iron_door_bottom"),
            ("textures/blocks/wheat_stage_7", "block/wheat_stage7"),
            ("textures/blocks/coral_fan_blue_dead", "block/dead_tube_coral_fan"),
            ("textures/blocks/flower_houstonia", "block/azure_bluet"),
            ("textures/blocks/grass_top", "block/grass_block_top"),
            ("textures/blocks/stonebrick_cracked", "block/cracked_stone_bricks"),
            ("textures/blocks/cherry_log_side", "block/cherry_log"),
            ("textures/blocks/rail_golden_powered", "block/powered_rail_on"),
        ] {
            assert_eq!(j(bedrock), java, "{bedrock}");
        }
        assert_eq!(j("textures/entity/bed/silver"), "entity/bed/light_gray");
        assert_eq!(
            j("textures/entity/skulls/zombie.png"),
            "entity/zombie/zombie"
        );
        assert_eq!(
            j("textures/entity/villager2/levels/level_iron"),
            "entity/villager/profession_level/iron"
        );
        assert_eq!(
            j("textures/models/armor/chain_1"),
            "entity/equipment/humanoid/chainmail"
        );
        assert_eq!(
            j("textures/models/armor/iron_2"),
            "entity/equipment/humanoid_leggings/iron"
        );
        assert_eq!(
            j("textures/models/armor/elytra"),
            "entity/equipment/wings/elytra"
        );
        assert_eq!(java_texture("ui/x"), None);
    }

    #[test]
    fn a_texture_past_the_atlas_is_averaged_down_by_a_power_of_two() {
        assert_eq!(atlas_shrink(1024, 512, false), 1);
        assert_eq!(atlas_shrink(1024, 1024, false), 2);
        assert_eq!(atlas_shrink(1024, 1024, true), 1);
        assert_eq!(atlas_shrink(2048, 1024, true), 2);
        assert_eq!(atlas_shrink(3072, 128, false), 4);
        assert_eq!(atlas_shrink(64, 880, false), 2);
        let mut rgba = Vec::new();
        for y in 0..1024u32 {
            for x in 0..1024u32 {
                rgba.extend_from_slice(if (x + y) % 2 == 0 { &[200, 0, 0, 255] } else { &[0, 0, 0, 0] });
            }
        }
        let png = encode_png(&Image { width: 1024, height: 1024, rgba }).unwrap();
        let small = fit_to_atlas(&png, false).unwrap();
        let px = decode_png_for_test(&small);
        assert_eq!(px.len(), 512 * 512);
        // Half the texels are opaque red: the colour stays red, the alpha halves.
        assert_eq!(px[0], [200, 0, 0, 127]);
        assert!(fit_to_atlas(&small, false).is_none());
    }

    #[test]
    fn the_solid_texture_makes_every_texel_that_is_not_clear_opaque() {
        let eyes = encode_png(&Image {
            width: 2,
            height: 2,
            rgba: vec![
                1, 2, 3, 0, //
                4, 5, 6, 157, //
                7, 8, 9, 255, //
                10, 11, 12, 2,
            ],
        })
        .unwrap();
        let out = solid_png(&eyes).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(out))
            .read_info()
            .unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut buf).unwrap();
        let alpha: Vec<u8> = buf.chunks_exact(4).map(|p| p[3]).collect();
        assert_eq!(alpha, [0, 255, 255, 255]);
        assert!(
            solid_png(
                &encode_png(&Image {
                    width: 1,
                    height: 1,
                    rgba: vec![0, 0, 0, 255]
                })
                .unwrap()
            )
            .is_none()
        );
    }

    #[test]
    fn the_glow_holds_only_the_texels_less_than_opaque() {
        let png = |px: [[u8; 4]; 4]| {
            encode_png(&Image {
                width: 2,
                height: 2,
                rgba: px.concat(),
            })
            .unwrap()
        };
        let solid = png([[9, 9, 9, 255]; 4]);
        assert!(glow_png(&solid).is_none());
        let eyes = png([[1, 2, 3, 255], [4, 5, 6, 2], [7, 8, 9, 0], [1, 1, 1, 128]]);
        let out = glow_png(&eyes).unwrap();
        let img = decode_png_for_test(&out);
        assert_eq!(img[0], [0, 0, 0, 0]);
        assert_eq!(img[1], [4, 5, 6, 253]);
        assert_eq!(img[2], [0, 0, 0, 0]);
        assert_eq!(img[3], [1, 1, 1, 127]);
    }

    fn decode_png_for_test(b: &[u8]) -> Vec<[u8; 4]> {
        let mut r = png::Decoder::new(std::io::Cursor::new(b))
            .read_info()
            .unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let i = r.next_frame(&mut buf).unwrap();
        buf[..i.buffer_size()]
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect()
    }

    #[test]
    fn faint_texels_of_a_cutout_texture_turn_solid() {
        let png = |px: [[u8; 4]; 4]| {
            encode_png(&Image {
                width: 2,
                height: 2,
                rgba: px.concat(),
            })
            .unwrap()
        };
        let decode = |b: &[u8]| {
            let mut d = png::Decoder::new(std::io::Cursor::new(b));
            d.set_transformations(png::Transformations::EXPAND);
            let mut r = d.read_info().unwrap();
            let mut buf = vec![0; r.output_buffer_size().unwrap()];
            r.next_frame(&mut buf).unwrap();
            buf
        };
        // An eye white at alpha 2 beside solid and empty texels: it turns solid.
        let eye = png([
            [255, 255, 255, 2],
            [9, 9, 9, 255],
            [0, 0, 0, 0],
            [1, 2, 3, 255],
        ]);
        let lifted = lift_faint_alpha(&eye).unwrap();
        assert_eq!(decode(&lifted)[..4], [255, 255, 255, 255]);
        assert_eq!(decode(&lifted)[8..12], [0, 0, 0, 0]);
        // A few white faint texels among many solid ones are eyes even beside a translucent texel.
        let big = {
            let mut px = vec![[0u8, 0, 0, 255]; 40];
            px[0] = [255, 255, 255, 3];
            px[1] = [9, 9, 9, 160];
            encode_png(&Image {
                width: 8,
                height: 5,
                rgba: px.concat(),
            })
            .unwrap()
        };
        assert!(lift_faint_alpha(&big).is_some());
        // Real translucency anywhere keeps the faint texel faint.
        let glow = png([
            [255, 255, 255, 2],
            [9, 9, 9, 128],
            [0, 0, 0, 0],
            [1, 2, 3, 255],
        ]);
        assert!(lift_faint_alpha(&glow).is_none());
        // A filler of faint texels, more than the solid ones: left clear.
        let filler = png([
            [255, 255, 255, 1],
            [255, 255, 255, 1],
            [9, 9, 9, 255],
            [0, 0, 0, 0],
        ]);
        assert!(lift_faint_alpha(&filler).is_none());
        // Nothing faint, nothing to do.
        let plain = png([
            [255, 255, 255, 0],
            [9, 9, 9, 255],
            [0, 0, 0, 0],
            [1, 2, 3, 255],
        ]);
        assert!(lift_faint_alpha(&plain).is_none());
    }

    use super::*;
    use crate::pack::Pack;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "horizon-bedrock-conv-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(root: &Path, rel: &str, bytes: &[u8]) {
        write(&root.join(rel), bytes).unwrap();
    }

    fn tga(w: u16, h: u16, top_first: bool, rle: bool) -> Vec<u8> {
        let mut b = vec![0u8; 18];
        b[2] = if rle { 10 } else { 2 };
        b[12..14].copy_from_slice(&w.to_le_bytes());
        b[14..16].copy_from_slice(&h.to_le_bytes());
        b[16] = 32;
        b[17] = if top_first { 0x28 } else { 0x08 };
        // Two rows: red over green (BGRA).
        let row = |c: [u8; 4]| {
            std::iter::repeat_n(c, w as usize)
                .flatten()
                .collect::<Vec<u8>>()
        };
        let (first, second) = (row([0, 0, 255, 255]), row([0, 255, 0, 255]));
        if rle {
            for r in [&first, &second] {
                b.push(0x80 | (w as u8 - 1));
                b.extend_from_slice(&r[..4]);
            }
        } else {
            b.extend(first);
            b.extend(second);
        }
        b
    }

    #[test]
    fn tga_variants_decode_to_the_same_picture() {
        for (top, rle) in [(true, false), (true, true)] {
            let img = decode_tga(&tga(2, 2, top, rle)).unwrap();
            assert_eq!((img.width, img.height), (2, 2));
            assert_eq!(&img.rgba[0..4], &[255, 0, 0, 255], "top={top} rle={rle}");
            assert_eq!(&img.rgba[8..12], &[0, 255, 0, 255]);
        }
        // Bottom-left origin: the first stored row is the bottom one.
        let img = decode_tga(&tga(2, 2, false, false)).unwrap();
        assert_eq!(&img.rgba[0..4], &[0, 255, 0, 255]);
        assert!(decode_tga(&[0; 5]).is_none());
        let png = encode_png(&img).unwrap();
        assert_eq!(&png[1..4], b"PNG");
    }

    #[test]
    fn a_tga_with_a_faint_texel_has_it_lifted_like_a_png() {
        let dir = tmp("tga-faint");
        fixture(&dir);
        // Two pixels (BGRA): an eye at alpha 3 and a solid one.
        let mut b = vec![0u8; 18];
        b[2] = 2;
        b[12] = 2;
        b[14] = 1;
        b[16] = 32;
        b[17] = 0x28;
        b.extend([0, 0, 255, 3, 9, 9, 9, 255]);
        put(&dir, "textures/entity/eye.tga", &b);
        let pack = Pack::open(&dir).unwrap();
        let alpha = |png: &[u8]| {
            let mut d = png::Decoder::new(std::io::Cursor::new(png));
            d.set_transformations(png::Transformations::EXPAND);
            let mut r = d.read_info().unwrap();
            let mut buf = vec![0; r.output_buffer_size().unwrap()];
            r.next_frame(&mut buf).unwrap();
            (buf[3], buf[7])
        };
        let lifted = texture_png(&pack, "textures/entity/eye", true, false).unwrap();
        assert_eq!(alpha(&lifted), (255, 255));
        let raw = texture_png(&pack, "textures/entity/eye", false, false).unwrap();
        assert_eq!(alpha(&raw), (3, 255));
    }

    fn fixture(dir: &Path) {
        put(
            dir,
            "manifest.json",
            br#"{"format_version":2,"header":{"name":"Demo","uuid":"00000000-0000-0000-0000-000000000001","version":[1,0,0]},
                "modules":[{"type":"resources","uuid":"00000000-0000-0000-0000-000000000002","version":[1,0,0]}]}"#,
        );
        put(dir, "pack_icon.png", b"\x89PNG icon");
        put(
            dir,
            "entity/zombie.entity.json",
            br#"{"minecraft:client_entity":{"description":{"identifier":"minecraft:zombie","textures":{"default":"textures/entity/Z","tga":"textures/entity/t","gone":"textures/entity/missing"},
                "geometry":{"default":"geometry.z"},"materials":{"default":"zombie"},"render_controllers":["controller.render.z"]}}}"#,
        );
        put(dir, "textures/entity/Z.png", b"\x89PNG zombie");
        put(dir, "textures/entity/t.tga", &tga(1, 1, true, false)[..]);
        put(dir, "textures/unused/x.png", b"no");
        put(
            dir,
            "particles/p.particle.json",
            br#"{"particle_effect":{"description":{"identifier":"d:p","basic_render_parameters":{"material":"particles_alpha","texture":"textures/particle/dust"}},"components":{}}}"#,
        );
        put(dir, "textures/particle/dust.png", b"\x89PNG dust");
        put(
            dir,
            "sounds/sound_definitions.json",
            br#"{"sound_definitions":{"Demo:Hit.Big":{"category":"neutral","sounds":["sounds/demo/a",{"name":"sounds/demo/b","volume":[0.5,1],"pitch":1.2,"weight":2},{"name":"sounds/demo/w"}]},
                "mob.zombie.step":{"sounds":["sounds/demo/a"]}}}"#,
        );
        put(dir, "sounds/demo/a.ogg", b"OggS a");
        put(dir, "sounds/demo/b.ogg", b"OggS b");
        put(dir, "sounds/demo/w.wav", b"RIFF");
        put(dir, "animations/a.animation.json", b"{}");
        put(dir, "models/entity/m.geo.json", b"{}");
        put(dir, "sounds.json", b"{}");
        put(dir, "texts/en_US.lang", b"a=b");
    }

    #[test]
    fn writes_a_java_pack() {
        let src = tmp("src");
        fixture(&src);
        let out = tmp("out");
        let pack = Pack::open(&src).unwrap();
        let r = to_java(&pack, &out, (64, 0)).unwrap();
        let ns = namespace(&pack);
        let meta: Value =
            serde_json::from_slice(&fs::read(out.join("pack.mcmeta")).unwrap()).unwrap();
        assert_eq!(meta["pack"]["min_format"], json!([64, 0]));
        assert!(out.join("pack.png").exists());
        // Textures: lower-cased atlas names, TGA converted, the unused one left out.
        assert!(
            out.join("assets/minecraft/textures/entity/bedrock/entity/z.png")
                .exists()
        );
        assert!(
            fs::read(out.join("assets/minecraft/textures/entity/bedrock/entity/t.png"))
                .unwrap()
                .starts_with(b"\x89PNG")
        );
        assert!(
            out.join("assets/minecraft/textures/entity/bedrock/particle/dust.png")
                .exists()
        );
        assert!(
            !out.join("assets/minecraft/textures/entity/bedrock/unused/x.png")
                .exists()
        );
        assert_eq!((r.textures, r.missing_textures), (3, 1));
        // Data copied as is.
        assert!(
            out.join(format!("assets/horizon_bedrock/{ns}/entity/zombie.entity.json"))
                .exists()
        );
        assert!(
            out.join(format!("assets/horizon_bedrock/{ns}/animations/a.animation.json"))
                .exists()
        );
        assert!(
            out.join(format!("assets/horizon_bedrock/{ns}/models/entity/m.geo.json"))
                .exists()
        );
        assert!(
            out.join(format!("assets/horizon_bedrock/{ns}/sounds/sound_definitions.json"))
                .exists()
        );
        assert!(!out.join(format!("assets/horizon_bedrock/{ns}/texts/en_US.lang")).exists());
        // Sounds: per namespace, Java names, averaged ranges, wav skipped.
        let sj: Value =
            serde_json::from_slice(&fs::read(out.join("assets/demo/sounds.json")).unwrap())
                .unwrap();
        let hit = &sj["hit.big"];
        assert_eq!(hit["category"], "neutral");
        assert_eq!(hit["sounds"].as_array().unwrap().len(), 2);
        assert_eq!(
            hit["sounds"][1],
            json!({"name":"demo:demo/b","volume":0.75,"pitch":1.2,"weight":2})
        );
        assert!(out.join("assets/demo/sounds/demo/a.ogg").exists());
        assert!(out.join("assets/bedrock/sounds.json").exists());
        assert_eq!(r.skipped_sounds, 1);
        assert_eq!(r.sound_events, 2);
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn cache_names_follow_the_input() {
        let a = cache_name(Path::new("/p/My Pack.mcpack"), 5, 10);
        assert!(a.to_string_lossy().starts_with("my_pack-"));
        assert_eq!(a, cache_name(Path::new("/p/My Pack.mcpack"), 5, 10));
        assert_ne!(a, cache_name(Path::new("/p/My Pack.mcpack"), 6, 10));
    }
}
