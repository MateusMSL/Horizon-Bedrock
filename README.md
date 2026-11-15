<p align="center">
  <img src="docs/media/banner.webp" alt="Horizon: Minecraft Java rebuilt in Rust, with Bedrock add-on support" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Minecraft-Java%2026.3-5D9C3F" alt="Minecraft Java 26.3">
  <img src="https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Bedrock%20add--ons-in%20progress-F0795A" alt="Bedrock add-ons in progress">
  <img src="https://img.shields.io/badge/engine-v196-2A2470" alt="Engine build v196">
  <img src="https://img.shields.io/badge/platform-Windows-2A2470" alt="Windows">
  <a href="#project-status"><img src="https://img.shields.io/badge/player--visible%20gaps-0%20open-F0795A" alt="Player-visible gaps open"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
</p>

<p align="center">
  <a href="#download"><b>Download</b></a> ·
  <a href="docs/bedrock.md"><b>Documentation</b></a> ·
  <a href="https://github.com/MateusMSL/BR-.mcpack-reader/releases"><b>Release notes</b></a> ·
  <a href="docs/bedrock-roadmap.md"><b>Roadmap</b></a> ·
  <a href="#project-status"><b>Project status</b></a> ·
  <a href="#contributing"><b>Contributing</b></a>
</p>

# Horizon: Bedrock add-ons

A fork of [Horizon](https://github.com/horizon-rust/horizon), Minecraft: Java Edition 26.3 rewritten in Rust, focused on one thing: a **generic Bedrock add-on engine**. Drop a `.mcpack` / `.mcaddon` resource pack into `resourcepacks/` and Java mobs, held items, first-person hands, particles and sounds take the pack's look and animations. Free, open source and unofficial.

> Not an official Minecraft product, nor approved by or associated with Mojang or Microsoft. This project contains no Minecraft code, assets or data: you need your own copy of Java Edition 26.3. Third-party packs are never part of the repository.

## Download

Get the latest build from [Releases](https://github.com/MateusMSL/BR-.mcpack-reader/releases) (Windows x64), or [build from source](#build-from-source).

**You need** a Microsoft account that owns Java Edition and a GPU with Vulkan or DirectX 12. An existing Minecraft install is reused.

## What works today

- **Entities:** client entities, geometry, render controllers, materials, animations and animation controllers, all driven by the pack's Molang (every expression of the test pack is interpreted).
- **Held items (attachables):** items in the main and off hand, head, torso and other slots, with the pack's bindings, animations and per-bone materials; the holder's arm follows the attachable's animation.
- **First person:** the pack's own first-person skeleton, camera at the eye, walk sway, item swings and use animations (tools, buckets, flint and steel in progress).
- **Particles and sounds:** the pack's particles simulated on the CPU, sound definitions played by the animations.
- **Debugging:** a hot-reloaded `bedrock-debug.json` (bone dumps, hide/solo/move/rotate bones, per-frame first-person trace) and the [`scan_pack`](docs/bedrock.md#testing-it) tool.

Rules are generic and Bedrock-faithful: nothing is adjusted per pack or per item.

Test pack: **Actions & Stuff 1.1.68** (Oreville).

| Measure | Value |
| --- | --- |
| Pack files read (4,483 JSON) | 100%, 0 failures |
| Molang (199,430 expressions) | 100% interpreted, 0 failures |
| Ignored keys (`audit_pack`) | 0 |
| Client entities checked in game | 18 of 128 |
| Client entities with a known error | 20 of 128 |

Details: [docs/bedrock-status.md](docs/bedrock-status.md), [docs/bedrock-coverage.md](docs/bedrock-coverage.md), [docs/bedrock-validation.md](docs/bedrock-validation.md).

## Using a pack

1. Put the `.mcpack`, `.mcaddon`, folder or zip in `resourcepacks/`.
2. Select it in the pack screen.
3. The pack is converted once to `.horizon-cache/bedrock/` (again only when the file changes).

How it fits together: [docs/bedrock.md](docs/bedrock.md).

## Build from source

Needs Rust 1.90+, Python 3.10+ and JDK 25. `horizon-prepare` builds the game's resources from **your own copy** of Java Edition 26.3 and never downloads it:

1. **Client:** play 26.3 once in the official launcher (Horizon reads `~/.minecraft`, or `--minecraft-dir` / `HORIZON_MINECRAFT_DIR`).
2. **Server jar:** the 26.3 server jar, passed once with `--server-jar` or `HORIZON_SERVER_JAR`.

```sh
git clone https://github.com/MateusMSL/BR-.mcpack-reader.git
cd BR-.mcpack-reader
cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
cargo run --release -p horizon-client
```

Check a pack without the game:

```sh
cargo run --release -p horizon-bedrock --example scan_pack -- <pack>
cargo run --release -p horizon-bedrock --example convert_pack -- <pack> <out>
```

Prepared files land in the gitignored `.local/`: never commit or share them.

## Roadmap

Pending items and every engine build (v1xx) are in [docs/bedrock-roadmap.md](docs/bedrock-roadmap.md). Open work:

- Flint and steel use animation (first and third person)
- Invisible or badly textured items (tools, fishing rod), checked in first person and third person from behind and the front
- Items swinging forever in the off hand
- Missing particles (rain on the ground, water splashes, ground fire) and ambient mechanics
- Checklist of every animation in the pack, with its variables
- Java fire block using the pack's animated texture
- Armor stand with the pack's armours, minecart, boat and lever models, ghast harness glow, sulfur cube

## Contributing

Issues and pull requests are welcome: read [CONTRIBUTING.md](CONTRIBUTING.md). Fixes must be generic engine rules, never per-pack or per-item.

```sh
git config core.hooksPath .githooks
python3 tools/pr_ready.py
```

Never commit game files, decompiled source, prepared resources or third-party packs.

## Project status

**Player-visible gaps: 0 open** of the 833 the [port ledger](tools/port_ledger.py) found, reading every vanilla 26.3 method against Horizon's code: vanilla behaviour a player would notice that Horizon doesn't have yet. By area: Animals [0](https://github.com/horizon-rust/horizon/issues/3512); Monsters and bosses [0](https://github.com/horizon-rust/horizon/issues/3513); Entities, projectiles, vehicles and decorations [0](https://github.com/horizon-rust/horizon/issues/3514); Items, enchantments and inventories [0](https://github.com/horizon-rust/horizon/issues/3515); Blocks, redstone and fluids [0](https://github.com/horizon-rust/horizon/issues/3516); World, world generation and saves [0](https://github.com/horizon-rust/horizon/issues/3517); Screens, HUD and client packets [0](https://github.com/horizon-rust/horizon/issues/3518); Entity and block rendering, models [0](https://github.com/horizon-rust/horizon/issues/3519); Server and players on the server [0](https://github.com/horizon-rust/horizon/issues/3520); Mob AI, villagers and raids [0](https://github.com/horizon-rust/horizon/issues/3521). This number goes down as gaps close (`tools/parity_gaps.py`); the table below says what the harnesses compare. These are upstream Horizon's numbers for the Java game this fork builds on.

Early. Parsing and Molang are complete for the test pack; in-game validation is in progress (see the table above and the error list in [docs/bedrock-status.md](docs/bedrock-status.md)). The Java game itself comes from upstream Horizon; this fork does not track its parity numbers.

The table is the project's own audit of what the harnesses compare, subsystem by subsystem: a score says how much of that subsystem the harnesses cover and pass, not that nothing else differs from vanilla. What players still run into: [known issues](docs/known-issues.md).

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

### Known issues

See [docs/bedrock-status.md](docs/bedrock-status.md): mobs (slime shell, mooshroom, sulfur cube, ghast animations, breeze, phantom smoke...), glow (spider and enderman eyes, harness, torch, armour glint) and engine gaps (inventory/GUI items, spectator and emote queries, non-pack entities).

## Licence

[MIT](LICENSE), for the project's original work; dependencies keep their own licences ([notices](THIRD_PARTY_NOTICES.md)). Minecraft is a trademark of Mojang Synergies AB; its code and assets belong to Mojang and Microsoft under the [Minecraft EULA](https://www.minecraft.net/en-us/eula).

## Credits

Horizon by [Ludovic Marie](https://lu4ovic.com) ([@ludovic111](https://github.com/ludovic111)). Bedrock add-on engine by [Mateus](https://github.com/MateusMSL).
