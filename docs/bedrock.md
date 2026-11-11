# Bedrock resource packs

Horizon reads Bedrock **resource packs** (`.mcpack`, `.mcaddon`, a folder or zip with a `manifest.json`) from
`resourcepacks/` and gives Java mobs the pack's look and sound. It reads the visual half only: the game rules
stay Java's. Nothing in a pack runs game logic, spawns, damages or moves anything.

| From the pack | Used for |
|---|---|
| `entity/*.json`, `render_controllers/`, `models/` (geometry) | the model, texture and material of a mob, chosen by the pack's Molang (a name tag, a baby, the biome...) |
| `animations/`, `animation_controllers/` | the mob's pose each frame, on top of the Java movement |
| `particles/` | particle effects an animation sets off, simulated on the CPU and drawn with the game's particles |
| `sounds/sound_definitions.json`, `.ogg` files | Java sound events `<namespace>:<name>` (a definition with no namespace is `bedrock:<name>`), played by the animations |
| `textures/` (the ones entities and particles name) | the entity atlas, as `entity/bedrock/...` |

Not read: block and item textures, `attachables/` (held items) beyond parsing, `blocks.json`, behavior
packs and scripts, Vibrant Visuals / PBR (`texture_set.json`), `sounds.json` replacements of Java mob
sounds, `.lang` files.

## How it fits

`horizon-bedrock` is a pure crate (no game types): `pack` (folders, zips, `.brarchive`), `molang`, `geometry`,
`animation` (animations and controllers), `entity` (client entities and render controllers), `material`,
`particle`, `sounds`, `convert` and `bundle`.

1. `horizon-client-render/src/packs/bedrock.rs`: when `resourcepacks/` is listed, a Bedrock pack is converted
   to a Java pack folder in `.horizon-cache/bedrock/` (again only when the file changes) and listed like any
   other pack (`file/<name>`), so selecting it in the pack screen works as usual. The folder holds
   `assets/minecraft/textures/entity/bedrock/` (PNG, TGA converted), `assets/<ns>/sounds.json` and `sounds/`,
   and the pack's own JSON under `assets/horizon_bedrock/`.
2. `mobs/bedrock.rs` loads `assets/horizon_bedrock/` from the resource root (`BedrockMobs`, swapped when the
   packs change, like the OptiFine CEM models). `MobHost` answers the pack's `query.*` from the `Mob`.
3. `mobs/render.rs`: `render_visible` draws the Bedrock model for a mob the pack dresses, and the vanilla
   one otherwise. With no pack the bundle is empty and nothing changes.
4. `game/bedrock.rs`: the sounds and effects the frames queue are played and drawn once per frame.

## Testing it

`cargo run --release -p horizon-bedrock --example scan_pack -- <pack>` parses every file of a pack, runs every
Molang expression once and resolves every entity, and prints what failed. `--example convert_pack -- <pack>
<out>` writes the Java form. Third-party packs are never part of the repository: tests build their own
fixtures.

`--example inspect_entity -- <pack> minecraft:spider [name tag]` prints what the pack draws for one entity
(layers, geometry and texture found or missing, bones shown, scale, pose); `all` lists the ids.
`--example render_entity -- <pack> minecraft:player out.png` with `HOLD=minecraft:mace` (`SLOT=`, `ANCHOR=pivot|frame`) draws the holder with the attachable in hand and prints where it sits relative to the hand. `crates/horizon-bedrock/tests/synthetic_pack.rs` builds a pack in the test and runs it through open, convert, load and play. The vanilla resource pack (`Mojang/bedrock-samples`) is the second reference pack for `scan_pack`.
In game, `HORIZON_BEDROCK_DEBUG=1` logs once per entity: geometry, texture (pack or the game's own),
vertices, and `top`/`feet` (the model's highest and lowest point in blocks; `feet` is 1.501 on the ground).
A texture the pack names under `textures/entity/` and does not ship is looked up in the game's own
`entity/` textures.

Known limits: the Bedrock to Java rotation signs and the emitter's locator placement were not checked against
a running game; read a mob's model first with a pack you know.

## Pack options (the gear)

* Resource Packs screen: a Bedrock pack with subpacks or manifest `settings` shows a **Settings** button.
* Rows: **Mode** (subpack: Default = base pack, then each subpack), toggles, dropdowns, multiselect options, sliders.
* Saved in `config/horizon-addons.json` by pack file name:
  `{"Pack.mcpack": {"subpack": "SP2", "settings": {"ns:flag": true}}}`. Edit by hand if needed.
* Applied by **Done** (resources reload). A changed subpack converts the pack again into its own cache folder.
* Expressions: `query.is_pack_setting_enabled('ns:flag')`, `query.is_pack_setting_selected('ns:mode', 'option')`.
  Unset values read the manifest defaults.

## Held items, effects and texture animation

- Items held by a mob dressed by a pack sit on the pack's `rightitem` / `leftitem` bone (the arm bone when missing), as the animation posed it.
- `query.equipped_item_any_tag` reads a tag as the kind of item its name says (`oreville:is_sword` matches `*_sword`); tiers (`minecraft:golden_tier`) match the item's material prefix.
- A particle effect a controller state sets off runs for as long as the state does and stops after it (fire on a burning mob).
- `uv_anim` moves a layer's texture only when its material defines `USE_UV_ANIM`; the charged creeper's `charged_creeper` material is the game's own energy swirl.
- `variable.attack_time` and `variable.attack_animation_tick` carry the swing of zombies, golems and the like.

## Texture meshes, curves, world queries

- `texture_meshes`: each texture a bone's sprites name becomes a model of the same bones with thin boxes (`geometry.x#mesh:<texture>`), drawn as one more layer with that texture. Sprites are 16 units wide (times `scale`); the texture's real size is not read; the edge of the sprite's silhouette is not extruded.
- `blend_via_shortest_path` on a controller state: rotations blend the shorter way round.
- Particles: `emitter_initialization`, `particle_initialization` (creation, per update, per render), `curves` (`linear`, `catmull_rom`, `bezier`, `bezier_chain`), `particle_kill_plane`, `emitter_shape_entity_aabb`, collision `collision_radius` and `events` (`min_speed`), billboard `direction` (`derive_from_velocity`, `custom`, `min_speed_threshold`).
- Render controllers: `on_fire_color`, `is_hurt_color`; `rebuild_animation_matrices` and `filter_lighting` are read and have no effect.
- World queries (`relative_block_has_any_tag` within 2 blocks, `entity_biome_has_any_identifier`, `time_of_day`, `moon_phase`, `is_raining`, `is_thundering`): the mob tick samples the world once a pack asks; the first ask answers 0.
- `get_animation_frame`: a held clock gives its 64 frames from the day time; other items 0.
- Item use: `main_hand_item_use_duration` ticks left (0 when not in use), `main_hand_item_max_duration` ticks, `item_remaining_use_duration` seconds.
- `bone_origin`, `bone_rotation`, `get_default_bone_pivot`: from the previous frame's pose.

## Audit log, delays, sprites, per-material passes

- `HORIZON_BEDROCK_DEBUG=1` logs every key a pack carries that the engine does not read, with a count (`audit.rs`; matrix in `bedrock-coverage.md`).
- `HORIZON_BEDROCK_DUMP=skeleton,creeper` (with `HORIZON_BEDROCK_DEBUG=1`): once a second per mob within 12 blocks, logs `bedrock-dump <kind>#<id>` with the answers of the main queries, the running animations and controller states (`name[state s]{children}`, `!` = finished), every bone the pose moves (rotation, entity-space rotation, position, scale) and each layer (geometry, texture, material, colour, overlay, uv). `all` dumps every kind. The first Bedrock frame also logs `bedrock: engine build <tag>`.
- `HORIZON_BEDROCK_DUMP_VARS=acloko,habcyx` adds a `bedrock-dump-vars` line per dump with those `v.` variables of the entity.
- Stencil: `StencilWrite`/`Replace` materials and `NotEqual` stencil-test materials carry their reference in `MaterialFlags`; a `NotEqual` layer drawn after a layer that wrote the same reference is pushed 0.1 blocks away from the camera, so an outline hull shows only past the body's silhouette. An additive layer tinted black (overlay black at alpha 1) is not drawn.
- `query.bone_orientation_trs(bone)` answers a `Trs` value (translation in pixels with y up from the feet, rotation, scale); `v.a = q.bone_orientation_trs(..)` exposes `v.a.t.x`, `v.a.r.y`, `v.a.s.z` and so on.
- Faint alpha: a texture whose texels are alpha 1–8 (the villager's eye white and pupil are 2) and that has no other translucency gets those texels made solid when it is converted; the game's alpha test would drop them, Bedrock shows them.
- Animations: `start_delay`, `loop_delay`. Entity `description.animation_controllers` merge into `animations`. Particles: `emitter_lifetime_events` (creation, expiration, timeline).
- `texture_meshes`: sprite size = the texture's PNG; `use_pixel_depth` = one box per run of opaque pixels, `scale[2]` deep; otherwise a flat plane. Built after the textures convert (`Bundle::fit_sprites`).
- `materials` per bone: one pass per distinct material among a layer's bones, each with its flags (`uv_anim` only on the bones whose material has `USE_UV_ANIM`).
- `filter_lighting`: flat lighting on the layer. `rebuild_animation_matrices`: read, no-op.
- Particle variables set by particle expressions stay with that particle; `emitter_shape_entity_aabb` uses the carrying mob's box.

## v12

- `query.rotation_to_camera(axis)`, `query.camera_rotation(axis)`, `query.distance_from_camera`: valores reais (antes 0). Morte do A&S usa a direção até a câmera.
- Molang `this` nos canais de animação (acumulado até o canal); `pose_changed` mantém `bone_origin` na pose parcial.
- `any_animation_finished` / `all_animations_finished`: animação em loop termina a cada ciclo; animação sem comprimento ou inexistente conta como terminada (piscar não trava mais).
- `on_fire_color`: alpha = intensidade da mistura, não transparência (esqueleto deixava de piscar).
- Morte: giro do Java desfeito de forma exata; corpo ancorado no chão e no centro dos pés.
- Exemplos: `probe_death`, `probe_layers` (camadas por tick).
- Passe por material: bone de outro material só deixa de desenhar os próprios cubos (filhos continuam); antes escondia a subárvore (olhos do villager, casca do slime).
- `relative_to: { rotation: "entity" }`: `resolve_entity_rotations` gira o bone no espaço da entidade (ignora a rotação dos bones acima). Braço do esqueleto ao mirar.
- `scalex/y/z`, `held_item_scale`, `held_item_ignores_lighting`, `hide_held_items` lidos e desenhados; `hide_armor`, `models.bone.reset` sem efeito (documentado).
- `overlay_color` e `on_fire_color` entram na cor da camada (`Layer::tint`); `is_hurt_color` fica com o flash vermelho do jogo.
- `query.standing_scale`: escala real do urso polar.
