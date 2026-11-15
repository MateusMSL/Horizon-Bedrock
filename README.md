<p align="center">
  <img src="docs/media/banner.webp" alt="Horizon Bedrock: Minecraft Java with Bedrock add-ons" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Minecraft-Java%2026.3-5D9C3F" alt="Minecraft Java 26.3">
  <img src="https://img.shields.io/badge/Bedrock%20.mcpack-supported-F0795A" alt=".mcpack supported">
  <img src="https://img.shields.io/badge/.mcaddon-coming%20later-8A8A8A" alt=".mcaddon coming later">
  <img src="https://img.shields.io/badge/platform-Windows-2A2470" alt="Windows">
  <a href="#project-status"><img src="https://img.shields.io/badge/player--visible%20gaps-0%20open-F0795A" alt="Player-visible gaps open"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
</p>

<p align="center">
  <a href="#-what-is-this"><b>What is this</b></a> ·
  <a href="#-how-to-play"><b>How to play</b></a> ·
  <a href="#-what-already-works"><b>What works</b></a> ·
  <a href="#-known-problems"><b>Known problems</b></a> ·
  <a href="docs/bedrock-roadmap.md"><b>Changelog</b></a>
</p>

# Horizon: Bedrock add-ons

## 🧩 What is this

**Horizon Bedrock** is a fork (a copy with extra features) of [Horizon](https://github.com/horizon-rust/horizon), a version of **Minecraft: Java Edition 26.3** rebuilt from scratch.

This fork adds one thing on top: it lets the Java game **use Bedrock resource packs** (`.mcpack`). Mobs, armour, items in your hand, animations, particles and sounds take the look of the pack, the same way they do in Bedrock.

| Today | Later |
| --- | --- |
| ✅ Bedrock resource packs (`.mcpack`) | 🔜 Full add-ons (`.mcaddon`) with behaviour packs: new mobs, items and game rules |

The main test pack is **[Actions & Stuff](https://guides.orevillestudios.com/actions-and-stuff)** (Oreville Studios), one of the biggest animation packs for Bedrock. If it works with Actions & Stuff, most simpler packs work too.

> Free and unofficial. Not made by, approved by or connected to Mojang or Microsoft. You need your own copy of Minecraft: Java Edition. Packs from other creators are never included here: get them from their official pages.

<p align="center">
  <img src="docs/media/readme/survival.webp" alt="Horizon in survival" width="49%">
  <img src="docs/media/readme/valley.webp" alt="A valley in Horizon" width="49%">
</p>

## 🎮 How to play

**You need:** a Windows PC, a Microsoft account that owns Minecraft: Java Edition, and the official [Horizon launcher](https://playhorizon.dev).

1. Install the **Horizon launcher** and let it install a version once (tab **Versions**).
2. Get the **Horizon Bedrock** game build and add it in the launcher: **Versions → Import a game build**. Ready-made downloads will be on the [Releases](https://github.com/MateusMSL/Horizon-Bedrock/releases) page; until then, the [installation guide](docs/bedrock-instalacao.md) (in Portuguese) shows how to build it.
3. Put your `.mcpack` in the `addons` folder of that version and turn it on in **Options → Add-ons**.
4. Press **Play**. The first start with a new pack takes about a minute (the pack is prepared once).

## ✅ What already works

Checked in game with Actions & Stuff, side by side with Bedrock.

**Your character**
- Walking, tools, spyglass, brush, eating and drinking animations
- Sword combat stance and attacks, in first and third person
- Riding: leaning into turns and the first-person bob
- Special moves: elytra barrel roll, superhero landing, diving, water bucket clutch
- Elytra flight with armour on

**Mobs**
- New movement, idle, attack and hurt animations
- Faces that follow you with their eyes and move their eyebrows
- 3D armour, tools and weapons on zombies, skeletons and others
- Mounts leaning into turns, with the rider leaning too
- The reworked witch

**Armour and items**
- 3D armour for every material, including the living turtle helmet
- The armour shimmer, and 3D armour, elytra and shields in your hand
- The custom enchantment outline and glint, in the colour picked in the pack's settings

**Decorations (props on armour stands)**
- Most static props: skeleton remains, cake, carrot pile, cookie jar, pillow fort, sleigh, tents and more
- Many interactive props: vampire coffin, windmill, toy train, snow globe, Jack in the Box, wagon, weapon rack, presents
- Switching variants by crouching and using: carpets and party balloons
- Armour stand poses: crouch and use a stand to go through its 13 poses

**Sounds and more**
- New sword swings, the axolotl bucket companion and footsteps in step with the animations
- Morphs: Mini Me, Iris, Goober, Bandit and Rusty

Full list, item by item: [Actions & Stuff checklist](docs/bedrock-as-checklist.md).

## 🛠️ Known problems

Things that don't work yet and will be fixed in future updates:

- **Death animations** of players and mobs are not played yet
- **Weapons on your back and belt** (Holstered Weapons) don't show
- **Fishing rod** cast and reel, the **map** held in first person and **climbing ladders** have no animation
- **Beds** and **costumes** (leather armour with trims) keep the normal look
- Some **props** don't appear (chimney, pumpkin patch, snowman, Backrooms door…), and flames, glows and particles of some props sit in the wrong place
- **Placeable items** on armour stands appear away from the stand
- **Interactive props** don't react when punched (piñatas, big pumpkin, mushrooms)
- **Morphs:** with an empty hand, first person shows your normal skin's hand, and tools and armour don't fit the morph's body
- **Pumpkins** and snow golems always have the same face
- **Wolves** howl on any night, not only at full moon
- **Glows with shaders on:** glowing eyes, torches, allays and other glows look wrong when a shader pack is active
- **Vanilla Experience** with another resource pack on top breaks some items
- **Name tag variants** are still being checked one by one (for example, *Halloween* doesn't turn a slime into its skull variant)

Everything still open: [pending list](docs/bedrock-pendencias.md) (in Portuguese).

## 👩‍💻 For developers

How the add-on engine works: [docs/bedrock.md](docs/bedrock.md). Build from source:

```sh
git clone https://github.com/MateusMSL/Horizon-Bedrock.git
cd Horizon-Bedrock
cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
cargo run --release -p horizon-client
```

Contributions are welcome: read [CONTRIBUTING.md](CONTRIBUTING.md). Fixes must be general rules of the engine, never made for one pack or one item. Never commit game files or packs from other creators.

## Project status

**Player-visible gaps: 0 open** of the 833 the [port ledger](tools/port_ledger.py) found, reading every vanilla 26.3 method against Horizon's code: vanilla behaviour a player would notice that Horizon doesn't have yet. By area: Animals [0](https://github.com/horizon-rust/horizon/issues/3512); Monsters and bosses [0](https://github.com/horizon-rust/horizon/issues/3513); Entities, projectiles, vehicles and decorations [0](https://github.com/horizon-rust/horizon/issues/3514); Items, enchantments and inventories [0](https://github.com/horizon-rust/horizon/issues/3515); Blocks, redstone and fluids [0](https://github.com/horizon-rust/horizon/issues/3516); World, world generation and saves [0](https://github.com/horizon-rust/horizon/issues/3517); Screens, HUD and client packets [0](https://github.com/horizon-rust/horizon/issues/3518); Entity and block rendering, models [0](https://github.com/horizon-rust/horizon/issues/3519); Server and players on the server [0](https://github.com/horizon-rust/horizon/issues/3520); Mob AI, villagers and raids [0](https://github.com/horizon-rust/horizon/issues/3521). This number goes down as gaps close (`tools/parity_gaps.py`); the table below says what the harnesses compare. These are upstream Horizon's numbers for the Java game this fork builds on.

<details>
<summary>What the upstream harnesses compare (technical)</summary>

### What the harnesses compare

Scores: 0 (absent), 0.25 (started), 0.5 (playable, major features missing), 0.75 (mostly complete) or 1 (its harnesses cover the subsystem and pass). Last audited 2026-10-13: the harnesses pass on Linux and macOS. The [port ledger](tools/port_ledger.py) adds a method-by-method view of what is ported.

| Subsystem | Score | Main gaps |
| --- | ---: | --- |
| Window, input and game loop | 1 | None: game loop, options, camera, auto-jump, toggle sprint/sneak and frame limiter match |
| Assets and resources | 1 | None: item models match over 196,290 stacks; the resource pack screen over 3,000 sessions |
| Network protocol and multiplayer | 1 | None: joins vanilla servers; the dedicated server matches vanilla's files, login, configuration, play and command tree; LAN discovery over 462 cases (offline mode only) |
| Chunk storage and palettes | 1 | None: client and saves share paletted containers, identical to vanilla over 266 scenarios |
| Section meshing | 1 | None: 112 scenes and over a million quads identical to vanilla |
| Rendering | 1 | None: entity models, block entity renderers (407 cases) and animations (296), HUD overlays (2,200 states), the recipe book (600), player list, subtitles and world border match |
| Lighting | 1 | None: saved and relit light matches vanilla, slabs and stairs included |
| World generation | 1 | None: 2,352 whole chunks over 225 structure sites, in all three dimensions, identical to vanilla |
| Region files and saves | 1 | None: saves round-trip with vanilla; worlds from 1.8 to 26.2 upgrade as vanilla's DataFixers do |
| Entities and physics | 1 | None: attributes, spawn rules, natural spawning (15,500 cases), raids and patrols (1,900), `finalizeSpawn` (24,000) and mob AI (92 scenarios) match |
| Interaction and inventory | 1 | None: menu clicks over 8,000 sequences; recipe book placement (6,000), buttons (1,200), saved book (1,500), loom and crafter screens (1,600) |
| Menus, HUD and chat | 1 | None: every command (758 over 39,685 inputs), scoreboards (81,000 operations), the player list (872,000 draw calls), chat and its suggestions (3,000 sessions) match |
| Audio | 1 | None: the sound engine, sound events, music, situational music, ambience and subtitles match vanilla |
| Block behaviours and ticking | 1 | None: block updates, random ticks, redstone, dispensers (6,000 cases), crafters (4,000), container spills (4,000), copper golems and the sculk family (435 scenarios) match |

</details>

## Licence

[MIT](LICENSE), for the project's original work; dependencies keep their own licences ([notices](THIRD_PARTY_NOTICES.md)). Minecraft is a trademark of Mojang Synergies AB; its code and assets belong to Mojang and Microsoft under the [Minecraft EULA](https://www.minecraft.net/en-us/eula).

## Credits

Horizon by [Ludovic Marie](https://lu4ovic.com) ([@ludovic111](https://github.com/ludovic111)). Bedrock add-on engine by [Mateus](https://github.com/MateusMSL).
