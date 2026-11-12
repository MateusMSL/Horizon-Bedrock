#!/usr/bin/env python3
"""Builds the Horizon Bedrock test pack (`horizon-test-pack.mcpack`).

A resource pack that exercises each part of the Bedrock engine with simple, unmistakable
shapes and colours, so the same pack can be checked in Bedrock and in Horizon side by side:

  * 1st person, 3rd person and 3rd person from the front: two attachables
      - minecraft:paper  -> X/Y/Z axes (red/green/blue), white origin, yellow cube at y=24,
                            no animation (shows the bare binding rule)
      - minecraft:stick  -> a rod with a magenta tip, animated differently in 1st person
                            (tilted -45 on x, lifted 4 px) and 3rd person (pointing forward)
  * player interaction particles + sound: a splash when entering water, a ring when landing
    (also after flying in creative), both with a "blip" sound
  * custom block look: sponge with a magenta/black checker texture (blocks.json)
  * modified entity: minecraft:pig with its own model (box body, head with axes, spinning
    head), its own particles (green aura every 4 s from the `top` locator) and sound
  * name tags: a pig named "Teste" turns cyan (render controller reads q.is_name_any)
  * texts: item/entity names changed (texts/*.lang)

The player entity is the vanilla one, read from the Bedrock install (not stored in the repo),
with the test's particle and sound events added: Horizon only draws attachables on a holder a
pack draws, so the pack must carry a player.

Usage:
  python build_test_pack.py "<Minecraft for Windows>/Content/data" <out.mcpack>
"""
import json
import os
import struct
import sys
import tempfile
import zipfile
import zlib

PACK_UUID = "5f1c2a30-7a4e-4f0e-9c1d-2b7f0e6a1001"
MODULE_UUID = "5f1c2a30-7a4e-4f0e-9c1d-2b7f0e6a1002"


# --- vanilla files from the Bedrock install -------------------------------------------------

def read_brarchive(path):
    """`{name: bytes}` of a `.brarchive` (16-byte header, 256-byte entries, then the data)."""
    d = open(path, "rb").read()
    n = struct.unpack("<I", d[8:12])[0]
    o, entries = 16, []
    for _ in range(n):
        name = d[o + 1:o + 248].split(b"\0")[0].decode("latin1")
        off, ln = struct.unpack("<II", d[o + 248:o + 256])
        entries.append((name, off, ln))
        o += 256
    return {name: d[o + off:o + off + ln] for name, off, ln in entries if name}


def vanilla(data_dir):
    base = os.path.join(data_dir, "resource_packs", "vanilla", "__brarchive")
    return {kind: read_brarchive(os.path.join(base, kind + ".brarchive"))
            for kind in ("entity", "animations", "animation_controllers", "render_controllers", "models")}


def player_files(v):
    """The vanilla player entity, its animations, controllers, render controllers and models."""
    out = {}
    player = json.loads(v["entity"]["player.entity.json"])
    desc = player["minecraft:client_entity"]["description"]
    anims = {a for a in desc["animations"].values() if a.startswith("animation.")}
    ctrls = {a for a in desc["animations"].values() if a.startswith("controller.")}
    for name, raw in v["animations"].items():
        try:
            j = json.loads(raw)
        except ValueError:
            continue
        keep = {k: a for k, a in j.get("animations", {}).items() if k in anims}
        if keep:
            out["animations/vanilla_" + name] = {"format_version": j.get("format_version", "1.8.0"), "animations": keep}
    for name, raw in v["animation_controllers"].items():
        try:
            j = json.loads(raw)
        except ValueError:
            continue
        keep = {k: a for k, a in j.get("animation_controllers", {}).items() if k in ctrls}
        if keep:
            out["animation_controllers/vanilla_" + name] = {"format_version": j.get("format_version", "1.10.0"), "animation_controllers": keep}
    out["render_controllers/vanilla_player.render_controllers.json"] = json.loads(v["render_controllers"]["player.render_controllers.json"])
    out["models/entity/vanilla_mobs.json"] = json.loads(v["models"]["mobs.json"])
    # The test's events on the player: particles and a sound on entering water and on landing.
    desc.setdefault("particle_effects", {}).update({"fptest_splash": "fptest:splash", "fptest_land": "fptest:land"})
    desc.setdefault("sound_effects", {}).update({"fptest_blip": "fptest.blip"})
    desc["animations"]["fptest_events"] = "controller.animation.fptest.player_events"
    desc["scripts"].setdefault("animate", []).append("fptest_events")
    out["entity/player.entity.json"] = player
    return out


# --- images --------------------------------------------------------------------------------

def png(w, h, pixels):
    """RGBA PNG from a list of rows of (r, g, b, a)."""
    raw = b"".join(b"\0" + b"".join(bytes(p) for p in row) for row in pixels)

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


# Palette textures: one texel per colour, faces pick a texel.
WHITE, RED, GREEN, BLUE, YELLOW, MAGENTA, CYAN, BLACK = range(8)
PALETTE = [(255, 255, 255, 255), (230, 30, 30, 255), (30, 200, 30, 255), (30, 90, 255, 255),
           (255, 220, 0, 255), (255, 0, 255, 255), (0, 220, 220, 255), (25, 25, 25, 255)]
PIG = 8  # extra texel: the pig's skin
PIG_PINK, PIG_CYAN = (240, 150, 170, 255), (0, 220, 220, 255)


def palette(extra):
    return png(9, 1, [PALETTE + [extra]])


def cube(origin, size, texel):
    face = {"uv": [texel, 0], "uv_size": [1, 1]}
    return {"origin": origin, "size": size,
            "uv": {k: face for k in ("north", "south", "east", "west", "up", "down")}}


def geometry(identifier, bones, w=9):
    return {"format_version": "1.16.0", "minecraft:geometry": [{
        "description": {"identifier": identifier, "texture_width": w, "texture_height": 1,
                        "visible_bounds_width": 4, "visible_bounds_height": 4,
                        "visible_bounds_offset": [0, 1, 0]},
        "bones": bones}]}


BINDING = "c.item_slot == 'main_hand' ? 'rightitem' : 'leftitem'"


def axes(at=(0, 0, 0), length=8):
    x, y, z = at
    return [cube([x - 1, y - 1, z - 1], [2, 2, 2], WHITE),
            cube([x + 1, y - 0.5, z - 0.5], [length, 1, 1], RED),
            cube([x - 0.5, y + 1, z - 0.5], [1, length, 1], GREEN),
            cube([x - 0.5, y - 0.5, z + 1], [1, 1, length], BLUE)]


def build():
    files = {}
    # Attachables.
    files["models/entity/fptest_axes.geo.json"] = geometry("geometry.fptest.axes", [{
        "name": "item", "pivot": [0, 0, 0], "binding": BINDING,
        "cubes": axes() + [cube([-1.5, 22.5, -1.5], [3, 3, 3], YELLOW),
                           cube([-0.5, 9, -0.5], [1, 13.5, 1], BLACK)]}])
    files["models/entity/fptest_stick.geo.json"] = geometry("geometry.fptest.stick", [{
        "name": "item", "pivot": [0, 0, 0], "binding": BINDING,
        "cubes": [cube([-1, -1, -1], [2, 2, 2], CYAN),
                  cube([-0.5, 1, -0.5], [1, 14, 1], BLACK),
                  cube([-1.5, 15, -1.5], [3, 3, 3], MAGENTA)]}])
    for item, geo, extra in (("paper", "geometry.fptest.axes", {}),
                             ("stick", "geometry.fptest.stick", {
                                 "animations": {"first_person": "animation.fptest.stick.first_person",
                                                "third_person": "animation.fptest.stick.third_person"},
                                 "scripts": {"animate": [{"first_person": "c.is_first_person"},
                                                         {"third_person": "!c.is_first_person"}]}})):
        desc = {"identifier": "minecraft:" + item,
                "materials": {"default": "entity_alphatest"},
                "textures": {"default": "textures/fptest/palette"},
                "geometry": {"default": geo},
                "render_controllers": ["controller.render.fptest.simple"]}
        desc.update(extra)
        files["attachables/fptest_%s.json" % item] = {"format_version": "1.10.0", "minecraft:attachable": {"description": desc}}
    files["animations/fptest_stick.animation.json"] = {"format_version": "1.8.0", "animations": {
        "animation.fptest.stick.first_person": {"loop": True, "bones": {"item": {"rotation": [-45, 0, 0], "position": [0, 4, 0]}}},
        "animation.fptest.stick.third_person": {"loop": True, "bones": {"item": {"rotation": [90, 0, 0]}}}}}
    files["render_controllers/fptest.render_controllers.json"] = {"format_version": "1.8.0", "render_controllers": {
        "controller.render.fptest.simple": {"geometry": "Geometry.default", "materials": [{"*": "Material.default"}], "textures": ["Texture.default"]},
        "controller.render.fptest.pig": {"geometry": "Geometry.default", "materials": [{"*": "Material.default"}],
                                         "textures": ["q.is_name_any('Teste') ? Texture.named : Texture.default"]}}}

    # Player events: splash in water, ring on landing, each with the blip.
    files["animation_controllers/fptest_player.animation_controllers.json"] = {"format_version": "1.10.0", "animation_controllers": {
        "controller.animation.fptest.player_events": {"initial_state": "default", "states": {
            "default": {"transitions": [{"in_water": "q.is_in_water"}, {"air": "!q.is_on_ground"}]},
            "in_water": {"particle_effects": [{"effect": "fptest_splash"}], "sound_effects": [{"effect": "fptest_blip"}],
                         "transitions": [{"default": "!q.is_in_water"}]},
            "air": {"transitions": [{"in_water": "q.is_in_water"}, {"landed": "q.is_on_ground"}]},
            "landed": {"particle_effects": [{"effect": "fptest_land"}], "sound_effects": [{"effect": "fptest_blip"}],
                       "transitions": [{"default": "1"}]}}}}}

    # The pig: own model, spinning head, aura particles and a blip every 4 s, cyan when named.
    pig_bones = [
        {"name": "root", "pivot": [0, 0, 0]},
        {"name": "body", "parent": "root", "pivot": [0, 13, 2], "cubes": [cube([-5, 7, -5], [10, 8, 16], PIG)]},
        {"name": "leg0", "parent": "root", "pivot": [-3, 6, 7], "cubes": [cube([-5, 0, 5], [4, 7, 4], PIG)]},
        {"name": "leg1", "parent": "root", "pivot": [3, 6, 7], "cubes": [cube([1, 0, 5], [4, 7, 4], PIG)]},
        {"name": "leg2", "parent": "root", "pivot": [-3, 6, -5], "cubes": [cube([-5, 0, -5], [4, 7, 4], PIG)]},
        {"name": "leg3", "parent": "root", "pivot": [3, 6, -5], "cubes": [cube([1, 0, -5], [4, 7, 4], PIG)]},
        {"name": "head", "parent": "root", "pivot": [0, 12, -6],
         "locators": {"top": [0, 24, -10]},
         "cubes": [cube([-4, 8, -14], [8, 8, 8], PIG)] + axes((0, 16, -10), 6)},
    ]
    files["models/entity/fptest_pig.geo.json"] = geometry("geometry.fptest.pig", pig_bones)
    files["entity/fptest_pig.entity.json"] = {"format_version": "1.10.0", "minecraft:client_entity": {"description": {
        "identifier": "minecraft:pig",
        "materials": {"default": "entity_alphatest"},
        "textures": {"default": "textures/fptest/pig", "named": "textures/fptest/pig_named"},
        "geometry": {"default": "geometry.fptest.pig"},
        "animations": {"spin": "animation.fptest.pig.spin", "aura": "animation.fptest.pig.aura"},
        "scripts": {"animate": ["spin", "aura"]},
        "particle_effects": {"aura": "fptest:aura"},
        "sound_effects": {"blip": "fptest.blip"},
        "render_controllers": ["controller.render.fptest.pig"],
        "spawn_egg": {"base_color": "#F09AAA", "overlay_color": "#1E5AFF"}}}}
    files["animations/fptest_pig.animation.json"] = {"format_version": "1.8.0", "animations": {
        "animation.fptest.pig.spin": {"loop": True, "bones": {"head": {"rotation": [0, "q.life_time * 90", 0]}}},
        "animation.fptest.pig.aura": {"loop": True, "animation_length": 4,
                                      "particle_effects": {"0.0": {"effect": "aura", "locator": "top"}},
                                      "sound_effects": {"2.0": {"effect": "blip"}}}}}

    # Particles.
    def effect(identifier, color, components):
        base = {
            "minecraft:particle_lifetime_expression": {"max_lifetime": 1.2},
            "minecraft:particle_appearance_billboard": {"size": [0.12, 0.12], "facing_camera_mode": "rotate_xyz",
                                                        "uv": {"texture_width": 8, "texture_height": 8, "uv": [0, 0], "uv_size": [8, 8]}},
            "minecraft:particle_appearance_tinting": {"color": color}}
        base.update(components)
        return {"format_version": "1.10.0", "particle_effect": {"description": {"identifier": identifier,
                "basic_render_parameters": {"material": "particles_alpha", "texture": "textures/fptest/particle"}},
                "components": base}}

    once = {"minecraft:emitter_lifetime_once": {"active_time": 0.1}}
    files["particles/fptest_splash.json"] = effect("fptest:splash", [0.2, 0.5, 1.0, 1.0], dict(once, **{
        "minecraft:emitter_rate_instant": {"num_particles": 30},
        "minecraft:emitter_shape_point": {"direction": ["math.random(-1, 1)", 1.5, "math.random(-1, 1)"]},
        "minecraft:particle_initial_speed": 4,
        "minecraft:particle_motion_dynamic": {"linear_acceleration": [0, -9.8, 0]}}))
    files["particles/fptest_land.json"] = effect("fptest:land", [1.0, 0.55, 0.0, 1.0], dict(once, **{
        "minecraft:emitter_rate_instant": {"num_particles": 24},
        "minecraft:emitter_shape_disc": {"radius": 0.3, "plane_normal": "y", "direction": "outwards"},
        "minecraft:particle_initial_speed": 2.5,
        "minecraft:particle_motion_dynamic": {"linear_drag_coefficient": 2}}))
    files["particles/fptest_aura.json"] = effect("fptest:aura", [0.2, 1.0, 0.3, 1.0], dict(once, **{
        "minecraft:emitter_rate_instant": {"num_particles": 8},
        "minecraft:emitter_shape_sphere": {"radius": 0.3, "direction": "outwards"},
        "minecraft:particle_initial_speed": 0.5,
        "minecraft:particle_motion_dynamic": {"linear_acceleration": [0, 1, 0]}}))

    # Sound.
    files["sounds/sound_definitions.json"] = {"format_version": "1.14.0", "sound_definitions": {
        "fptest.blip": {"category": "neutral", "sounds": [{"name": "sounds/fptest/blip", "volume": 1.0}]}}}

    # Block look and texts.
    files["blocks.json"] = {"format_version": [1, 1, 0], "sponge": {"textures": "fptest_sponge", "sound": "stone"}}
    files["textures/terrain_texture.json"] = {"resource_pack_name": "fptest", "texture_name": "atlas.terrain",
                                              "padding": 8, "num_mip_levels": 4,
                                              "texture_data": {"fptest_sponge": {"textures": "textures/fptest/sponge"}}}
    files["texts/languages.json"] = ["en_US", "pt_BR"]
    files["texts/en_US.lang"] = ("item.paper.name=Paper (test axes)\nitem.stick.name=Stick (test animation)\n"
                                 "entity.pig.name=Pig (test)\ntile.sponge.name=Sponge (test)\n")
    files["texts/pt_BR.lang"] = ("item.paper.name=Papel (eixos de teste)\nitem.stick.name=Graveto (animação de teste)\n"
                                 "entity.pig.name=Porco (teste)\ntile.sponge.name=Esponja (teste)\n")

    # Images.
    files["textures/fptest/palette.png"] = palette(PALETTE[WHITE])
    files["textures/fptest/pig.png"] = palette(PIG_PINK)
    files["textures/fptest/pig_named.png"] = palette(PIG_CYAN)
    files["textures/fptest/particle.png"] = png(8, 8, [[(255, 255, 255, 255)] * 8 for _ in range(8)])
    files["textures/fptest/sponge.png"] = png(16, 16, [[(255, 0, 255, 255) if (x // 4 + y // 4) % 2 else (20, 20, 20, 255)
                                                         for x in range(16)] for y in range(16)])
    files["manifest.json"] = {"format_version": 2, "header": {
        "name": "Horizon Test Pack", "description": "Teste do motor Bedrock: 1a/3a pessoa, particulas, bloco, entidade, sons, etiquetas",
        "uuid": PACK_UUID, "version": [1, 0, 0], "min_engine_version": [1, 20, 0]},
        "modules": [{"type": "resources", "uuid": MODULE_UUID, "version": [1, 0, 0]}]}
    return files


def blip_ogg(path):
    """A short two-tone beep as Ogg Vorbis (ffmpeg)."""
    cmd = ('ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=880:duration=0.12" '
           '-f lavfi -i "sine=frequency=1320:duration=0.12" -filter_complex "[0][1]concat=n=2:v=0:a=1,volume=0.6" '
           '-c:a libvorbis -q:a 4 "%s"' % path)
    if os.system(cmd) != 0:
        raise SystemExit("ffmpeg failed: the pack needs it for the blip sound")


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(2)
    data_dir, out = sys.argv[1], sys.argv[2]
    files = build()
    files.update(player_files(vanilla(data_dir)))
    tmp = os.path.join(tempfile.mkdtemp(), "blip.ogg")
    blip_ogg(tmp)
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for name, body in sorted(files.items()):
            if isinstance(body, (dict, list)):
                body = json.dumps(body, indent=1, ensure_ascii=False).encode("utf-8")
            elif isinstance(body, str):
                body = body.encode("utf-8")
            z.writestr(name, body)
        z.write(tmp, "sounds/fptest/blip.ogg")
    os.remove(tmp)
    print("wrote", out, len(files) + 1, "files")


if __name__ == "__main__":
    main()
