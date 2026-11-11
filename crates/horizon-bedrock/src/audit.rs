//! What a pack says that the engine does not read.
//!
//! Each file kind has the keys the engine reads and the keys that have nothing to show
//! (`spawn_egg`, `min_engine_version`...). Any other key a pack carries is counted here, by
//! where it sits (`particles.components.minecraft:emitter_lifetime_events`), so a gap in the
//! engine shows up from the packs themselves.

use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;

/// Where an ignored key sits, with how many times a pack carried it.
pub type Report = BTreeMap<String, u32>;

fn check(report: &mut Report, context: &str, o: &Map<String, Json>, read: &[&str], inert: &[&str]) {
    for key in o.keys() {
        if read.contains(&key.as_str()) || inert.contains(&key.as_str()) {
            continue;
        }
        *report.entry(format!("{context}.{key}")).or_default() += 1;
    }
}

fn objects<'a>(v: Option<&'a Json>) -> impl Iterator<Item = &'a Map<String, Json>> {
    v.and_then(Json::as_object)
        .into_iter()
        .flat_map(|o| o.values().filter_map(Json::as_object))
}

fn list<'a>(v: Option<&'a Json>) -> impl Iterator<Item = &'a Map<String, Json>> {
    v.and_then(Json::as_array)
        .into_iter()
        .flat_map(|a| a.iter().filter_map(Json::as_object))
}

const PARTICLE_COMPONENTS: &[&str] = &[
    "minecraft:emitter_initialization",
    "minecraft:emitter_lifetime_events",
    "minecraft:emitter_lifetime_expression",
    "minecraft:emitter_lifetime_looping",
    "minecraft:emitter_lifetime_once",
    "minecraft:emitter_local_space",
    "minecraft:emitter_rate_instant",
    "minecraft:emitter_rate_manual",
    "minecraft:emitter_rate_steady",
    "minecraft:emitter_shape_point",
    "minecraft:emitter_shape_custom",
    "minecraft:emitter_shape_sphere",
    "minecraft:emitter_shape_box",
    "minecraft:emitter_shape_disc",
    "minecraft:emitter_shape_entity_aabb",
    "minecraft:particle_appearance_billboard",
    "minecraft:particle_appearance_lighting",
    "minecraft:particle_appearance_tinting",
    "minecraft:particle_expire_if_in_blocks",
    "minecraft:particle_expire_if_not_in_blocks",
    "minecraft:particle_initial_speed",
    "minecraft:particle_initial_spin",
    "minecraft:particle_initialization",
    "minecraft:particle_kill_plane",
    "minecraft:particle_lifetime_events",
    "minecraft:particle_lifetime_expression",
    "minecraft:particle_motion_collision",
    "minecraft:particle_motion_dynamic",
    "minecraft:particle_motion_parametric",
];

/// Adds to `report` the keys of `root`, a file of `kind` (its folder in the pack), that the
/// engine ignores.
pub fn scan(kind: &str, root: &Json, report: &mut Report) {
    let Some(o) = root.as_object() else { return };
    match kind {
        "entity" | "attachables" => entity(o, report),
        "render_controllers" => render_controllers(o, report),
        "animations" | "animation_controllers" => animations(o, report),
        "models" => models(o, report),
        "particles" => particles(o, report),
        _ => {}
    }
}

fn entity(root: &Map<String, Json>, r: &mut Report) {
    check(
        r,
        "entity",
        root,
        &[
            "format_version",
            "minecraft:client_entity",
            "minecraft:attachable",
        ],
        &[],
    );
    let Some(body) = root
        .get("minecraft:client_entity")
        .or_else(|| root.get("minecraft:attachable"))
        .and_then(Json::as_object)
    else {
        return;
    };
    check(r, "entity", body, &["description"], &[]);
    let Some(desc) = body.get("description").and_then(Json::as_object) else {
        return;
    };
    check(
        r,
        "entity.description",
        desc,
        &[
            "identifier",
            "materials",
            "textures",
            "geometry",
            "animations",
            "scripts",
            "particle_effects",
            "sound_effects",
            "render_controllers",
            "enable_attachables",
            "item",
            "animation_controllers",
            "hide_armor",
            "held_item_scale",
            "held_item_ignores_lighting",
        ],
        &["min_engine_version", "spawn_egg", "queryable_geometry"],
    );
    if let Some(s) = desc.get("scripts").and_then(Json::as_object) {
        check(
            r,
            "entity.scripts",
            s,
            &[
                "initialize",
                "pre_animation",
                "parent_setup",
                "animate",
                "scale",
                "scalex",
                "scaley",
                "scalez",
                "hide_held_items",
            ],
            // Which variables other entities may read: all of them can be here. The offscreen
            // flags only keep animating what is out of view, which is never drawn here.
            &[
                "variables",
                "should_update_effects_offscreen",
                "should_update_bones_and_effects_offscreen",
            ],
        );
    }
}

fn render_controllers(root: &Map<String, Json>, r: &mut Report) {
    check(
        r,
        "render_controllers",
        root,
        &["format_version", "render_controllers"],
        &[],
    );
    for c in objects(root.get("render_controllers")) {
        check(
            r,
            "render_controllers.controller",
            c,
            &[
                "arrays",
                "color",
                "filter_lighting",
                "geometry",
                "ignore_lighting",
                "is_hurt_color",
                "light_color_multiplier",
                "materials",
                "on_fire_color",
                "overlay_color",
                "part_visibility",
                "rebuild_animation_matrices",
                "textures",
                "uv_anim",
            ],
            &[],
        );
        if let Some(u) = c.get("uv_anim").and_then(Json::as_object) {
            check(
                r,
                "render_controllers.uv_anim",
                u,
                &["offset", "scale"],
                &[],
            );
        }
    }
}

fn animations(root: &Map<String, Json>, r: &mut Report) {
    check(
        r,
        "animations",
        root,
        &["format_version", "animations", "animation_controllers"],
        &[],
    );
    for a in objects(root.get("animations")) {
        check(
            r,
            "animations.animation",
            a,
            &[
                "loop",
                "animation_length",
                "override_previous_animation",
                "anim_time_update",
                "blend_weight",
                "bones",
                "particle_effects",
                "sound_effects",
                "timeline",
                "start_delay",
                "loop_delay",
            ],
            &[],
        );
        for b in objects(a.get("bones")) {
            check(
                r,
                "animations.bone",
                b,
                &["position", "rotation", "scale", "relative_to"],
                &[],
            );
        }
    }
    for c in objects(root.get("animation_controllers")) {
        check(
            r,
            "animation_controllers.controller",
            c,
            &["initial_state", "states"],
            &[],
        );
        for s in objects(c.get("states")) {
            check(
                r,
                "animation_controllers.state",
                s,
                &[
                    "animations",
                    "transitions",
                    "blend_transition",
                    "blend_via_shortest_path",
                    "on_entry",
                    "on_exit",
                    "particle_effects",
                    "sound_effects",
                ],
                &[],
            );
        }
    }
}

fn models(root: &Map<String, Json>, r: &mut Report) {
    for g in list(root.get("minecraft:geometry")) {
        check(r, "models.geometry", g, &["description", "bones"], &[]);
        if let Some(d) = g.get("description").and_then(Json::as_object) {
            check(
                r,
                "models.description",
                d,
                &["identifier", "texture_width", "texture_height"],
                // Culling bounds: the game culls with its own.
                &[
                    "visible_bounds_width",
                    "visible_bounds_height",
                    "visible_bounds_offset",
                ],
            );
        }
        for b in list(g.get("bones")) {
            check(
                r,
                "models.bone",
                b,
                &[
                    "name",
                    "parent",
                    "pivot",
                    "rotation",
                    "bind_pose_rotation",
                    "mirror",
                    "inflate",
                    "neverRender",
                    "binding",
                    "cubes",
                    "locators",
                    "texture_meshes",
                ],
                // `reset` clears what a vanilla entity's own animation did to the bone: a pack's
                // models start from nothing.
                &["reset"],
            );
            for c in list(b.get("cubes")) {
                check(
                    r,
                    "models.cube",
                    c,
                    &[
                        "origin", "size", "uv", "inflate", "mirror", "pivot", "rotation",
                    ],
                    &[],
                );
            }
            for m in list(b.get("texture_meshes")) {
                check(
                    r,
                    "models.texture_mesh",
                    m,
                    &[
                        "texture",
                        "position",
                        "rotation",
                        "local_pivot",
                        "scale",
                        "use_pixel_depth",
                    ],
                    &[],
                );
            }
        }
    }
}

fn particles(root: &Map<String, Json>, r: &mut Report) {
    check(
        r,
        "particles",
        root,
        &["format_version", "particle_effect"],
        &[],
    );
    let Some(e) = root.get("particle_effect").and_then(Json::as_object) else {
        return;
    };
    check(
        r,
        "particles.effect",
        e,
        &["description", "components", "curves", "events"],
        &[],
    );
    if let Some(d) = e.get("description").and_then(Json::as_object) {
        check(
            r,
            "particles.description",
            d,
            &["identifier", "basic_render_parameters"],
            &[],
        );
    }
    if let Some(c) = e.get("components").and_then(Json::as_object) {
        check(r, "particles.components", c, PARTICLE_COMPONENTS, &[]);
        if let Some(b) = c
            .get("minecraft:particle_appearance_billboard")
            .and_then(Json::as_object)
        {
            check(
                r,
                "particles.billboard",
                b,
                &["size", "facing_camera_mode", "uv", "direction"],
                &[],
            );
        }
        if let Some(m) = c
            .get("minecraft:particle_motion_collision")
            .and_then(Json::as_object)
        {
            check(
                r,
                "particles.collision",
                m,
                &[
                    "enabled",
                    "collision_drag",
                    "coefficient_of_restitution",
                    "collision_radius",
                    "expire_on_contact",
                    "events",
                ],
                &[],
            );
        }
    }
}

/// The lines of a report, the most frequent first.
pub fn lines(report: &Report) -> Vec<String> {
    let mut v: Vec<(&String, &u32)> = report.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    v.into_iter().map(|(k, n)| format!("{n:5}  {k}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonc;

    #[test]
    fn counts_keys_the_engine_does_not_read() {
        let mut r = Report::new();
        scan(
            "particles",
            &jsonc::parse(
                r#"{"format_version":"1.10.0","particle_effect":{"description":{"identifier":"a:b"},
                  "components":{"minecraft:emitter_made_up":{},"minecraft:emitter_rate_instant":{}}}}"#,
            )
            .unwrap(),
            &mut r,
        );
        scan(
            "entity",
            &jsonc::parse(
                r#"{"minecraft:client_entity":{"description":{"identifier":"a:b","spawn_egg":{},"made_up_key":true,
                  "scripts":{"animate":[],"scalex":"1","made_up_script":"1"}}}}"#,
            )
            .unwrap(),
            &mut r,
        );
        let keys: Vec<&str> = r.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "entity.description.made_up_key",
                "entity.scripts.made_up_script",
                "particles.components.minecraft:emitter_made_up",
            ]
        );
    }
}
