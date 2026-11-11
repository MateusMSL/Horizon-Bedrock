# Bedrock resource packs: spec × engine

`✔` read and drawn · `≈` read, partly drawn · `–` read, nothing to show · `✘` ignored (counted by the audit log).

Audit log: `HORIZON_BEDROCK_DEBUG=1` prints, once per pack, each key the engine does not read
(`n  where.key`), most frequent first. `cargo run --release -p horizon-bedrock --example audit_pack -- <pack>`
prints the same offline.

## Client entity / attachable (`entity/`, `attachables/`)

| Key | State |
| --- | --- |
| `identifier`, `materials`, `textures`, `geometry`, `render_controllers` | ✔ |
| `animations`, `animation_controllers` (description, merged) | ✔ |
| `scripts.initialize`, `pre_animation`, `animate`, `parent_setup`, `scale` | ✔ |
| `scripts.variables`, `should_update_*_offscreen`, `min_engine_version`, `spawn_egg`, `queryable_geometry` | – |
| `particle_effects`, `sound_effects` | ✔ |
| `enable_attachables`, `item` (attachable → item map) | ✔ |
| `scripts.scalex/y/z`, `held_item_scale`, `held_item_ignores_lighting`, `hide_held_items` | ✔ |
| `hide_armor` | – the game's own armour is not drawn on a mob a pack dresses (the pack's attachables dress it) |

## Render controllers

| Key | State |
| --- | --- |
| `geometry`, `textures`, `materials`, `arrays`, `part_visibility` | ✔ |
| `color`, `light_color_multiplier`, `ignore_lighting` | ✔ |
| `overlay_color` | ≈ the texture is multiplied toward the overlay by its alpha (exact for black overlays) |
| `on_fire_color` | ≈ same, while burning; the alpha is the mix, never transparency |
| `is_hurt_color` | – the game's own red flash covers it |
| `uv_anim` | ✔ per bone, by the bone's material (`USE_UV_ANIM`); energy swirl scrolls |
| `filter_lighting` | ≈ flat lighting (every face as one facing up) |
| `rebuild_animation_matrices` | – pose rebuilt every frame |

## Materials

| Feature | State |
| --- | --- |
| `.material` inheritance (`parent:variant`), `+defines`, `-defines`, `+states`, `-states` | ✔ |
| blend, alpha test, additive, double sided, emissive | ✔ |
| `USE_UV_ANIM`, energy swirl | ✔ |
| per-bone materials (`materials` map with bone globs) | ✔ one pass per distinct material |
| compiled shaders (`.material.bin`, `.shader`) | ✘ out of scope |

## Models (`models/`)

| Key | State |
| --- | --- |
| geometry 1.8 / 1.12 / 1.16+ layouts, `bones`, `cubes`, box and per-face UV, `mirror`, `inflate`, `pivot`, `rotation`, `bind_pose_rotation`, `parent`, `neverRender`, `binding` | ✔ |
| `locators` (particles, attachments) | ✔ |
| `texture_meshes` | ✔ texture size from the PNG; `use_pixel_depth` = solid cut to opaque pixels, else flat plane |
| `bone.reset` | – only meaningful on vanilla entities |
| bone `scale` in poses | ✔ |
| `visible_bounds_*` | – |

## Animations / controllers

| Feature | State |
| --- | --- |
| `bones` position / rotation / scale, keyframes, `lerp_mode`, `catmull_rom`, `bezier`, easings, `relative_to` | ✔ |
| `relative_to: { rotation: "entity" }` | ✔ the bone ends up turned in the entity's space, whatever the bones above do |
| `loop`, `hold_on_last_frame`, `animation_length`, `anim_time_update`, `blend_weight`, `override_previous_animation` | ✔ |
| `start_delay`, `loop_delay` | ✔ |
| `particle_effects`, `sound_effects`, `timeline` | ✔ |
| controllers: `states`, `transitions`, `blend_transition`, `blend_via_shortest_path`, `on_entry`, `on_exit` | ✔ |

## Molang

| Area | State |
| --- | --- |
| operators, ternary, `?:`, `??`, loops (`loop`, `for_each`), `return`, strings, structs, arrays, `math.*`, `variable`, `temp`, `context`, `array`, `geometry`, `texture`, `material` | ✔ |
| `query.*` | ✔ ~250 answered (mob state, held item, world around the mob: `block_neighbor_has_any_tag`, `relative_block_has_any_tag`, `biome_has_any_tag`, `time_of_day`, `moon_phase`, `bone_origin`, `bone_rotation`); the rest answer 0 and are listed by the log |

## Particles

| Feature | State |
| --- | --- |
| emitter rate (instant / steady / manual), lifetime (once / looping / expression / events), shapes (point / sphere / box / disc / custom / entity AABB) | ✔ |
| `emitter_initialization`, `emitter_local_space`, `emitter_lifetime_events` (creation, expiration, timeline) | ✔ |
| particle initialization, lifetime, speed, spin, motion (dynamic / parametric / collision with events) | ✔ |
| appearance (billboard facing modes, flipbook, direction), tinting (fixed / gradient), `kill_plane`, `expire_if_in_blocks`, `expire_if_not_in_blocks` | ✔ |
| `curves` (linear / catmull_rom / bezier / bezier_chain) | ✔ |
| variables | ✔ per particle; emitter variables shared |
| `events`: effect, sound, expression, sequence, randomize | ✔ |
| `emitter_shape_entity_aabb` | ✔ mob's box |
| `particle_appearance_lighting` | ✔ |

## What the game gives every pack

| Item | State |
| --- | --- |
| Molang queries listed in Microsoft's documentation | ✔ every one answers (`q.any`, `q.all`, `q.approx_eq` are builtins; the rest from the mob; logic the game keeps reads 0). A test fails when a documented query has no answer |
| `variable.*` of vanilla resources (`attack_time`, `is_paperdoll`, `is_holding_left/right`, `player_x_rotation`, ...) | ✔ set before the pack's scripts run |
| Vanilla animations a pack names without defining (`baby_transform`, `humanoid.big_head`, `riding.*`, `elytra.*`, `bee.no_stinger`, flops) | ✔ built in; a pack's own definition wins |
| Vanilla render controllers (`controller.render.default`, `armor`, `item_default`) | ✔ built in |
| Vanilla geometries (`humanoid.custom`, `customSlim`, `cape`, `elytra`, `shield`, `evocation_fang`, `bed`, `item_sprite`, `bow_standby`/`bow_pulling_*`, `crossbow_*`, `polarbear`, `cat`, `chicken`) | ✔ built in (`evocation_fang` with its `bite` animation; the bed's UV is unchecked; the rest from Mojang/bedrock-samples) |
| Vanilla textures a pack names without shipping (`textures/blocks`, `items`, `models/armor`, bed, skulls, villager levels) | ✔ mapped to the game's sprites |
| Materials: `GLINT`, `InvertCulling`, `DisableDepthWrite/Test`, `MASKED_MULTITEXTURE` (flags read) | ✔ glint, culling inversion, depth write off, depth test off (own pipelines) and multitexture (the second texture laid over the first, blended) drawn |
| Players and mannequins | ✔ the pack's player entity on the game's skin and slim model (`HORIZON_BEDROCK_PLAYERS=off` to turn off); the pack draws the armour |
| Blocks and items the pack retextures (`textures/blocks/<name>`, `items/<name>`), sky, colour maps, `splashes.json` | ✔ written as Java resources |

## World files (`fogs/`, `biomes/`, `blocks.json`, `block_culling/`)

| File | State |
| --- | --- |
| `fogs/*.json` (`minecraft:fog_settings`: `distance` by medium, `volumetric.density`) | – read (`Bundle::fogs`) |
| `biomes/*.client_biome.json`, `biomes_client/`, `biomes_client.json` (water, sky, grass, foliage colour, fog, ambient loop, music, deferred identifiers) | – read (`Bundle::client_biomes`) |
| `blocks.json` (`textures`, `carried_textures`, `sound`, `isotropic`, `brightness_gamma`) | – read (`Bundle::block_defs`); the blocks themselves come from a behavior pack |
| `block_culling/*.json` | – read (`Bundle::culling_rules`) |
| `lighting/`, `atmospherics/`, `color_grading/`, `point_lights/`, `shadows/`, `water/`, `pbr/`, `*.texture_set.json` | ✘ deferred lighting / RTX, out of scope |

## Out of scope

Persona and premium skins, RTX / PBR, compiled shaders (Iris port), `behavior_pack` scripting (later: `.hmod` importer).

## Known gaps

- The game's own armour is not drawn on a mob a pack dresses unless the pack ships attachables for it.
- First-person hands (`context.is_first_person` stays 0) and JSON UI (`ui/`): not read.
- `fogs/`, client biomes (`biomes/*.client_biome.json`, `biomes_client/`, `biomes_client.json`), `blocks.json` and `block_culling/`: read into the bundle (`world_data`), not drawn.
- Particle effects that outlive their frame read a snapshot of the mob (baby, name, age, size) for `query.*`; other queries read 0.
- `texture_meshes` of attachables with `use_pixel_depth` are cut to opaque pixels, not extruded per face.

