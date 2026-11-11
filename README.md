<p align="center">
  <img src="docs/media/banner.webp" alt="Horizon: Minecraft Java rebuilt in Rust, with Bedrock add-on support" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Minecraft-Java%2026.3-5D9C3F" alt="Minecraft Java 26.3">
  <img src="https://img.shields.io/badge/Rust-stable-B7410E?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Bedrock%20add--ons-in%20progress-F0795A" alt="Bedrock add-ons in progress">
  <img src="https://img.shields.io/badge/engine-v196-2A2470" alt="Engine build v196">
  <img src="https://img.shields.io/badge/platform-Windows-2A2470" alt="Windows">
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

Early. Parsing and Molang are complete for the test pack; in-game validation is in progress (see the table above and the error list in [docs/bedrock-status.md](docs/bedrock-status.md)). The Java game itself comes from upstream Horizon; this fork does not track its parity numbers.

### Known issues

See [docs/bedrock-status.md](docs/bedrock-status.md): mobs (slime shell, mooshroom, sulfur cube, ghast animations, breeze, phantom smoke...), glow (spider and enderman eyes, harness, torch, armour glint) and engine gaps (inventory/GUI items, spectator and emote queries, non-pack entities).

## Licence

[MIT](LICENSE), for the project's original work; dependencies keep their own licences ([notices](THIRD_PARTY_NOTICES.md)). Minecraft is a trademark of Mojang Synergies AB; its code and assets belong to Mojang and Microsoft under the [Minecraft EULA](https://www.minecraft.net/en-us/eula).

## Credits

Horizon by [Ludovic Marie](https://lu4ovic.com) ([@ludovic111](https://github.com/ludovic111)). Bedrock add-on engine by [Mateus](https://github.com/MateusMSL).
