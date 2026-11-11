# Bedrock hooks

Original Horizon files touched by the Bedrock add-on. Everything else lives in new files (`crates/horizon-bedrock`, `*/bedrock*.rs`, `mobs/bedrock_render/`, `game/pack_proxy.rs`).

- Check after every upstream sync: `.\tools\check-hooks.ps1`
- Manifest: `tools/bedrock-hooks.txt` (`file|line that must exist`)
- Reapply lost hooks: `git apply --3way patches/bedrock-hooks.patch`
-

| File | Added lines | Hook lines |
|---|---|---|
| `crates/horizon-client-render/src/pack_screen.rs` | 421 | `subpacks: Vec<crate::packs::bedrock::Subpack>,`<br>`defs: Vec<horizon_bedrock::settings::Def>,`<br>`values: HashMap<String, horizon_bedrock::settings::Choice>,` |
| `crates/horizon-client/src/mobs/render.rs` | 236 | `include!("bedrock_render/render_01.rs");`<br>`include!("bedrock_render/render_02.rs");`<br>`} else if m.extra.swim.amount(alpha) > 0.0 && !super::bedrock::pack_lies_down(m.id) {` |
| `crates/horizon-client-render/src/packs.rs` | 171 | `pub mod bedrock;`<br>`pub fn folder_with(dir: &Path, format: (i64, i64), bedrock: bool) -> Vec<Pack> {`<br>`} else if bedrock::is_bedrock_file(&name) {` |
| `crates/horizon-client-render/src/skins.rs` | 121 | `offline_skins().as_ref().map(\|(skin, _)\| skin)`<br>`pub fn offline_skin_raw() -> Option<&'static [u8]> {`<br>`offline_skins().as_ref().map(\|(_, raw)\| raw.as_slice())` |
| `crates/horizon-client-render/src/gfx.rs` | 114 | `ADDITIVE, ADDITIVE_PARTICLE_OVERLAY, DISSOLVE_OVERLAY, DRAGON_RAYS_OVERLAY, ENERGY_SWIRL_OVERLAY,`<br>`FLUID_OVERLAY, NO_DEPTH_TEST, NO_DEPTH_WRITE, OIT_ENTITY, SHADOW_OVERLAY, TERRAIN_MODEL_CUTOUT_MIPPED_OVERLAY,`<br>`no_depth_write_pipeline: wgpu::RenderPipeline,` |
| `crates/horizon-client/src/item_render.rs` | 104 | `let wanted = crate::mobs::bedrock_debug::settings().items.clone();`<br>`log::info!("bedrock-debug: {line}");` |
| `crates/horizon-client-render/src/gfx/entity_batch.rs` | 69 | `fn bedrock_materials_sit_between_the_swirls_and_the_shadows() {` |
| `crates/horizon-client-render/src/options_ui.rs` | 64 | `OpenAddonFolder,`<br>`pub addon_infos: Vec<crate::pack_screen::PackInfo>,`<br>`addons_page: bool,` |
| `crates/horizon-client/src/game/armor_stands.rs` | 59 | `if !a.invisible && self.pack_stand(a, camera, alpha, wobble, packed, out) {`<br>`fn pack_stand(`<br>`a: &ArmorStand,` |
| `crates/horizon-client/src/game/dragon.rs` | 49 | `let pack = {`<br>`let proxy = super::pack_proxy::Proxy {`<br>`entity: "minecraft:ender_dragon",` |
| `crates/horizon-client/src/game/minecarts.rs` | 46 | `const PASSENGER_Y: f64 = 0.1875 + 0.47;`<br>`self.previous_position = Vec3::new(c.previous.x, c.previous.y + SEAT, c.previous.z);`<br>`let pack_block = c.custom_display.is_none()` |
| `crates/horizon-assets/src/item_model/tests.rs` | 43 | `fn a_potion_contents_select_matches_the_bare_and_the_object_form() {` |
| `crates/horizon-client/src/game/perspective.rs` | 42 | `if horizon_client::mobs::bedrock::wants_surroundings() {`<br>`horizon_client::mobs::bedrock::sample_surroundings(m, &self.mob_world_base(&[]))`<br>`bedrock: Default::default(),` |
| `crates/horizon-client/src/mobs/riders.rs` | 38 | `static MOUNT_KINDS: std::sync::RwLock<Vec<(u32, String)>> = std::sync::RwLock::new(Vec::new());`<br>`pub fn set_mount_kinds(kinds: Vec<(u32, String)>) {`<br>`if let Ok(mut k) = MOUNT_KINDS.write() {` |
| `crates/horizon-client/src/mobs.rs` | 37 | `pub mod bedrock;`<br>`pub mod bedrock_debug;`<br>`pub surroundings: Option<Arc<bedrock::Surroundings>>,` |
| `crates/horizon-client/src/game/packs.rs` | 33 | `.unwrap_or_else(std::sync::PoisonError::into_inner) = bedrock;`<br>`let bedrock = Arc::new(horizon_client::mobs::bedrock::BedrockMobs::load(&root));`<br>`pub bedrock: Arc<horizon_client::mobs::bedrock::BedrockMobs>,` |
| `crates/horizon-client/src/game/passengers.rs` | 30 | `pub(super) fn vehicle_kind(&self, id: u32) -> String {`<br>`if let Some(b) = self.boats.iter().find(\|b\| b.id == id) {`<br>`return if b.chest { "minecraft:chest_boat" } else { "minecraft:boat" }.to_owned();` |
| `crates/horizon-client/src/game/remote.rs` | 25 | `if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)`<br>`&& m.species() == horizon_client::mobs::Species::Blaze`<br>`for (index, _, value) in &p.values {` |
| `crates/horizon-client/src/mobs/nether.rs` | 25 | `pub fn set_blaze_charged(m: &mut Mob, on: bool) {`<br>`if m.extra.flag == on {`<br>`m.extra.flag = on;` |
| `crates/horizon-client/src/mobs/camel.rs` | 23 | `static MODEL_DROP: std::sync::Mutex<Vec<(u32, f32)>> = std::sync::Mutex::new(Vec::new());`<br>`pub fn set_model_drop(id: u32, drop: f32) {`<br>`if let Ok(mut all) = MODEL_DROP.lock() {` |
| `crates/horizon-client/src/game/sulfur_cubes.rs` | 22 | `let transform = match horizon_client::mobs::render::bedrock_bone_frame(` |
| `crates/horizon-client/src/game.rs` | 21 | `mod bedrock;`<br>`let bedrock = self`<br>`if bedrock.replaces_flames(&m.kind.id) {` |
| `crates/horizon-client-render/src/entity.wgsl` | 18 | `return entity_color_cut(in, front, 0.1);`<br>`fn entity_color_cut(in: Out, front: bool, cut: f32) -> vec4<f32> {`<br>`if color.a < cut \|\| (in.cull != 0u && !front) { discard; }` |
| `crates/horizon-client/src/game/creatures/mob_geometry.rs` | 17 | `let pack_dresses = is_player`<br>`.mob_data()`<br>`.unwrap_or_else(std::sync::PoisonError::into_inner)` |
| `crates/horizon-client/src/game/options.rs` | 16 | `let addons = self.ui.options.addons_open();`<br>`if !self.ui.options.resource_packs_open() && !addons {`<br>`std::fs::read_dir(if addons {` |
| `crates/horizon-assets/src/item_model.rs` | 15 | `Self::Component(component) => canonical_component(component, v),`<br>`fn canonical_component(component: &str, v: &Value) -> Value {`<br>`Value::String(potion) if component == "minecraft:potion_contents" => {` |
| `crates/horizon-client/src/game/mob_blocks.rs` | 15 | `if species == Species::SnowGolem`<br>`.mob_data()`<br>`.unwrap_or_else(std::sync::PoisonError::into_inner)` |
| `crates/horizon-client/src/game/frame_geometry.rs` | 11 | `let rest = to_world`<br>`* horizon_client::first_person::item_pose(left, inverse_height, 0.0, ItemPose::Using);`<br>`let mut dressed = Vec::new();` |
| `crates/horizon-client/src/game/startup_menu.rs` | 6 | `self.menu.options.addon_infos = packs::addon_infos();`<br>`Event::OpenAddonFolder => {`<br>`let dir = packs::addons_dir();` |
| `crates/horizon-client-render/src/hud.rs` | 5 | `\|\| (gh <= 1024 && name.starts_with("entity/bedrock/"))))` |
| `crates/horizon-client/assets/horizon/lang/en_us.json` | 5 | `"horizon.pages.addons.tooltip": "Bedrock add-ons, from the addons folder. They apply on top of the resource packs.",` |
| `crates/horizon-client/src/mobs/cem_anim.rs` | 5 | `pub fn is_in_gui() -> bool {`<br>`IN_GUI.get()` |
| `crates/horizon-client/src/mobs/data.rs` | 4 | `pub bedrock: std::sync::RwLock<std::sync::Arc<super::bedrock::BedrockMobs>>,`<br>`bedrock: Default::default(),` |
| `crates/horizon-client-render/Cargo.toml` | 3 | `# Bedrock resource packs in 'resourcepacks/', converted to Java packs when discovered.`<br>`horizon-bedrock = { path = "../horizon-bedrock" }` |
| `.gitignore` | 2 | `/resourcepacks` |
| `crates/horizon-client-lib/Cargo.toml` | 2 | `# Bedrock resource packs: the look of mobs, sounds and particles (docs/bedrock.md).`<br>`horizon-bedrock = { path = "../horizon-bedrock" }` |
| `crates/horizon-client/Cargo.toml` | 2 | `# Bedrock resource packs: the look of mobs, sounds and particles (docs/bedrock.md).`<br>`horizon-bedrock = { path = "../horizon-bedrock" }` |
| `crates/horizon-client/assets/horizon/lang/pt_br.json` | 2 | `"horizon.pages.addons.tooltip": "Add-ons do Bedrock, da pasta addons. Valem por cima dos pacotes de recursos.",` |
| `crates/horizon-client/assets/horizon/lang/ru_ru.json` | 2 | `"horizon.pages.addons.tooltip": "Аддоны Bedrock из папки addons. Применяются поверх наборов ресурсов.",` |
| `crates/horizon-client/src/game/mounts.rs` | 2 | `self.previous_position = if first { at } else { self.rider_seat };`<br>`self.rider_seat = at;` |
| `crates/horizon-client/src/game/remote_vehicles.rs` | 2 | `self.previous_position = if first { v } else { self.rider_seat };`<br>`self.rider_seat = v;` |
| `Cargo.toml` | 1 | `"crates/horizon-bedrock",` |
| `crates/horizon-client-render/src/options_ui_pages.rs` | 1 | `Page::Addons => "horizon.pages.addons.tooltip",` |
| `crates/horizon-client/src/game/native_projectiles.rs` | 1 | `pub(super) fn projectile_type(p: &Projectile) -> &'static str {` |
| `crates/horizon-client/src/game/occlusion_culling_tests.rs` | 1 | `bedrock: Default::default(),` |
| `crates/horizon-client/src/mobs/cem.rs` | 1 | `bedrock: Default::default(),` |
| `crates/horizon-client/src/mobs/random_textures.rs` | 1 | `bedrock: Default::default(),` |
| `crates/horizon-client/src/mobs/skin.rs` | 1 | `bedrock: Default::default(),` |
