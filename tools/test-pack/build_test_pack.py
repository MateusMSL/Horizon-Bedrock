#!/usr/bin/env python3
"""Builds the Horizon Bedrock test pack (`horizon-test-pack.mcpack`).

A resource pack that exercises each part of the Bedrock engine with simple, unmistakable
shapes and colours, so the same pack can be checked in Bedrock and in Horizon side by side:

  * 1st person, 3rd person and 3rd person from the front: two attachables
      - minecraft:paper  -> X/Y/Z axes (red/green/blue), white origin, yellow cube at y=24,
                            no animation (shows the bare binding rule)
      - minecraft:stick  -> a rod with a magenta tip, animated differently in 1st person
                            (tilted -45 on x, lifted 4 px) and 3rd person (pointing forward)
      - minecraft:brick  -> texture mesh (`texture_meshes`), no rotation: the bare plane a
                            sprite is extruded in (orientation sprite: red top row, green left
                            column, white diagonal, blue block at the bottom right)
      - minecraft:flint  -> texture mesh laid out as Actions & Stuff lays its 3D tools: bone
                            under `root`, mesh at [-11.31, -11.31, 0.5] turned [-90, 0, -45],
                            bone animated [90, 0, -90] / [0, -2, 4]
      - minecraft:clock  -> stencil window as A&S's clock: a frame, an invisible window that
                            writes the stencil, and a face (checker, larger than the window)
                            drawn only where the window is
      - minecraft:shears -> two-handed hold as A&S's shears: `parent_setup` sets a variable on
                            the holder, the player's first person shows and poses both arms by it
      - minecraft:fishing_rod -> holder variable read back as A&S's rod: `parent_setup` sets it,
                            the attachable reads `c.owning_entity->v...` and scales its rod by
                            it (0 hides the rod)
  * player interaction particles + sound: a splash when entering water, a ring when landing
    (also after flying in creative), both with a "blip" sound
  * custom block look: sponge with a magenta/black checker texture (blocks.json)
  * modified entity: minecraft:pig (file format 1.26.30: Bedrock keeps the newest format) with its own model (box body, head with axes, spinning
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
    # Two-handed hold (shears): reset each frame, set by the held item's `parent_setup`; the
    # first person poses and shows the left arm too.
    # As A&S's player: reset once (`initialize`), the variables the held items read back
    # declared public (`c.owning_entity->v...` reads only public ones).
    desc["scripts"].setdefault("initialize", []).append("v.fptest_two_hands = 0; v.fptest_rod = 0;")
    desc["scripts"].setdefault("variables", {}).update({"variable.fptest_two_hands": "public", "variable.fptest_rod": "public"})
    desc["animations"]["fptest_two_hands"] = "animation.fptest.two_hands"
    desc["scripts"]["animate"].append({"fptest_two_hands": "v.is_first_person && v.fptest_two_hands"})
    desc["render_controllers"].append({"controller.render.fptest.two_hands": "v.is_first_person && v.fptest_two_hands"})
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
                        # Wide bounds: Bedrock skips a model whose bounds leave the view, and
                        # an item held out in front can leave small ones.
                        "visible_bounds_width": 8, "visible_bounds_height": 8,
                        "visible_bounds_offset": [0, 2, 0]},
        "bones": bones}]}


BINDING = "c.item_slot == 'main_hand' ? 'rightitem' : 'leftitem'"


def axes(at=(0, 0, 0), length=8):
    x, y, z = at
    return [cube([x - 1, y - 1, z - 1], [2, 2, 2], WHITE),
            cube([x + 1, y - 0.5, z - 0.5], [length, 1, 1], RED),
            cube([x - 0.5, y + 1, z - 0.5], [1, length, 1], GREEN),
            cube([x - 0.5, y - 0.5, z + 1], [1, 1, length], BLUE)]


def sprite_pixels():
    """16×16 orientation sprite: red top row, green left column (over the red), white diagonal
    from the bottom left to the top right, blue 4×4 block at the bottom right; the rest clear."""
    rows = [[(0, 0, 0, 0)] * 16 for _ in range(16)]
    for x in range(16):
        rows[0][x] = (230, 30, 30, 255)
    for y in range(16):
        rows[y][0] = (30, 200, 30, 255)
    for x in range(2, 12):
        rows[15 - x][x] = (255, 255, 255, 255)
    for y in range(12, 16):
        for x in range(12, 16):
            rows[y][x] = (40, 80, 255, 255)
    return rows


def build():
    files = {}
    # Attachables.
    files["models/entity/fptest_axes.geo.json"] = geometry("geometry.fptest.axes", [{
        "name": "item", "pivot": [0, 0, 0], "binding": BINDING,
        "cubes": axes() + [cube([-1.5, 22.5, -1.5], [3, 3, 3], YELLOW),
                           cube([-0.5, 9, -0.5], [1, 13.5, 1], BLACK)]}])
    # Modelled at the hand: Bedrock puts the model's y = 24 on the hand bone, so the stick
    # starts there (cyan) and hangs down the model's -y (away from the hand, as the paper's
    # black rod does), its bone turning about the hand.
    files["models/entity/fptest_stick.geo.json"] = geometry("geometry.fptest.stick", [{
        "name": "item", "pivot": [0, 24, 0], "binding": BINDING,
        "cubes": [cube([-1, 23, -1], [2, 2, 2], CYAN),
                  cube([-0.5, 9, -0.5], [1, 14, 1], BLACK),
                  cube([-1.5, 6, -1.5], [3, 3, 3], MAGENTA)]}])
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
    # Texture meshes: the plane Bedrock extrudes a sprite in, bare (brick) and as A&S's 3D
    # tools place it (flint).
    files["models/entity/fptest_mesh.geo.json"] = {"format_version": "1.21.0", "minecraft:geometry": [
        {"description": {"identifier": "geometry.fptest.mesh", "texture_width": 16, "texture_height": 16,
                         "visible_bounds_width": 8, "visible_bounds_height": 8, "visible_bounds_offset": [0, 2, 0]},
         "bones": [{"name": "item", "pivot": [0, 24, 0], "binding": BINDING,
                    "texture_meshes": [{"texture": "sprite", "position": [0, 24, 0], "rotation": [0, 0, 0]}]}]},
        {"description": {"identifier": "geometry.fptest.mesh_ans", "texture_width": 16, "texture_height": 16,
                         "visible_bounds_width": 8, "visible_bounds_height": 8, "visible_bounds_offset": [0, 2, 0]},
         "bones": [{"name": "root", "pivot": [0, 0, 0]},
                   {"name": "sword", "parent": "root", "pivot": [0, 0, 0], "binding": BINDING,
                    "texture_meshes": [{"texture": "sprite", "position": [-11.31, -11.31, 0.5], "rotation": [-90, 0, -45]}]}]}]}
    for item, geo, extra in (("brick", "geometry.fptest.mesh", {}),
                             ("flint", "geometry.fptest.mesh_ans", {
                                 "animations": {"hold": "animation.fptest.mesh_ans.hold"},
                                 "scripts": {"animate": ["hold"]}})):
        desc = {"identifier": "minecraft:" + item,
                "materials": {"default": "entity_alphatest"},
                "textures": {"default": "textures/fptest/sprite", "sprite": "textures/fptest/sprite"},
                "geometry": {"default": geo},
                "render_controllers": ["controller.render.fptest.sprite"]}
        desc.update(extra)
        files["attachables/fptest_%s.json" % item] = {"format_version": "1.10.0", "minecraft:attachable": {"description": desc}}
    files["animations/fptest_mesh.animation.json"] = {"format_version": "1.8.0", "animations": {
        "animation.fptest.mesh_ans.hold": {"loop": True, "bones": {"sword": {"rotation": [90, 0, -90], "position": [0, -2, 4]}}}}}
    files["textures/fptest/sprite.png"] = png(16, 16, sprite_pixels())
    # A&S-like features: stencil window (clock), holder variables (shears, rod).
    files["models/entity/fptest_ans.geo.json"] = {"format_version": "1.16.0", "minecraft:geometry": [
        {"description": {"identifier": "geometry.fptest.clock", "texture_width": 9, "texture_height": 1,
                         "visible_bounds_width": 8, "visible_bounds_height": 8, "visible_bounds_offset": [0, 2, 0]},
         "bones": [{"name": "item", "pivot": [0, 24, 0], "binding": BINDING},
                   {"name": "frame", "parent": "item", "pivot": [0, 24, 0],
                    "cubes": [cube([-4, 27, -1], [8, 1, 2], YELLOW), cube([-4, 20, -1], [8, 1, 2], YELLOW),
                              cube([-4, 21, -1], [1, 6, 2], YELLOW), cube([3, 21, -1], [1, 6, 2], YELLOW)]},
                   # As A&S's clock (`ntjtva`): the face before the window in the bone list,
                   # flat planes; the window a plane in the frame, one face plane just in front
                   # of it (cyan) and one deep behind it (magenta, larger than the window).
                   {"name": "face", "parent": "item", "pivot": [0, 24, 0],
                    "cubes": [cube([-1, 23, -1.1], [2, 2, 0], CYAN), cube([-5, 19, 1.5], [10, 10, 0], MAGENTA)]},
                   {"name": "window", "parent": "item", "pivot": [0, 24, 0],
                    "cubes": [cube([-3, 21, -1], [6, 6, 0], WHITE)]}]},
        {"description": {"identifier": "geometry.fptest.shears", "texture_width": 9, "texture_height": 1,
                         "visible_bounds_width": 8, "visible_bounds_height": 8, "visible_bounds_offset": [0, 2, 0]},
         "bones": [{"name": "item", "pivot": [0, 24, 0], "binding": BINDING,
                    "cubes": [cube([-1, 23, -1], [2, 2, 2], CYAN), cube([-0.5, 15, -0.5], [1, 8, 1], RED),
                              cube([-3.5, 18.5, -0.5], [7, 1, 1], GREEN)]}]},
        {"description": {"identifier": "geometry.fptest.rod", "texture_width": 9, "texture_height": 1,
                         "visible_bounds_width": 8, "visible_bounds_height": 8, "visible_bounds_offset": [0, 2, 0]},
         "bones": [{"name": "item", "pivot": [0, 24, 0], "binding": BINDING,
                    "cubes": [cube([-1, 23, -1], [2, 2, 2], CYAN)]},
                   {"name": "rod", "parent": "item", "pivot": [0, 24, 0],
                    "cubes": [cube([-0.5, 6, -0.5], [1, 17, 1], BLUE), cube([-1.5, 3, -1.5], [3, 3, 3], YELLOW)]}]}]}
    files["materials/entity.material"] = {"materials": {"version": "1.0.0",
        # As A&S's clock (`oreville_migzni` / `oreville_vgjfwj`): the window writes stencil 3 and
        # nothing else; the face draws only where the stencil is 3.
        "fptest_window:entity_alphatest": {
            "+states": ["EnableStencilTest", "StencilWrite", "DisableColorWrite", "DisableAlphaWrite"],
            "frontFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"},
            "backFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"}, "stencilRef": 3},
        "fptest_face:entity_alphatest": {
            "+states": ["EnableStencilTest"],
            "frontFace": {"stencilFunc": "Equal"}, "backFace": {"stencilFunc": "Equal"}, "stencilRef": 3},
        # Actions & Stuff's clock, material for material: the case writes 0 with colour
        # (`oreville_khgpgi`), the window writes 3 without colour from `entity`
        # (`oreville_migzni`, here `fptest_win_e`) or `entity_alphatest` (`fptest_win_a`), the
        # dial draws where 3 is (`oreville_vgjfwj`).
        "fptest_case:entity_alphatest": {
            "+states": ["EnableStencilTest", "StencilWrite"],
            "frontFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"},
            "backFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"}, "stencilRef": 0},
        "fptest_win_e:entity": {
            "+states": ["EnableStencilTest", "StencilWrite", "DisableColorWrite", "DisableAlphaWrite"],
            "frontFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"},
            "backFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"}, "stencilRef": 3},
        "fptest_win_a:entity_alphatest": {
            "+states": ["EnableStencilTest", "StencilWrite", "DisableColorWrite", "DisableAlphaWrite"],
            "frontFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"},
            "backFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"}, "stencilRef": 3},
        "fptest_dial:entity_alphatest": {
            "+states": ["EnableStencilTest"],
            "frontFace": {"stencilFunc": "Equal"}, "backFace": {"stencilFunc": "Equal"}, "stencilRef": 3}}}
    # The A&S clock's model (`geometry.oreville_ans.ntjtva`), raised to the hand: a case 10x3x10
    # whose top is open (a yellow rim, clear inside) and bottom closed (dark), a window plane
    # 1 px under the top, two small icons (cyan) 0.1 px over the window and the dial (magenta)
    # 1.4 px under it. Three windows: `entity` with a clear texel (exactly A&S: compass),
    # `entity` with an opaque texel (recovery compass), `entity_alphatest` with a clear texel
    # (echo shard).
    def clock_rep(identifier, window_uv):
        top = {"uv": [0, 0], "uv_size": [10, 10]}
        side = {"uv": [0, 10], "uv_size": [10, 3]}
        return {"description": {"identifier": identifier, "texture_width": 32, "texture_height": 32,
                                "visible_bounds_width": 8, "visible_bounds_height": 8, "visible_bounds_offset": [0, 2, 0]},
                "bones": [
                    {"name": "item", "pivot": [0, 24, 0], "binding": BINDING},
                    {"name": "case", "parent": "item", "pivot": [0, 24, 0],
                     "cubes": [{"origin": [-5, 21, -5], "size": [10, 3, 10],
                                "uv": {"up": top, "down": {"uv": [10, 0], "uv_size": [10, 10]},
                                       "north": side, "south": side, "east": side, "west": side}}]},
                    {"name": "icons", "parent": "item", "pivot": [0, 24, 0],
                     "cubes": [{"origin": [-1.5, 23.1, 1], "size": [3, 0, 3], "uv": {"up": {"uv": [20, 10], "uv_size": [3, 3]}}},
                               {"origin": [-1.5, 23.1, -4], "size": [3, 0, 3], "uv": {"up": {"uv": [20, 10], "uv_size": [3, 3]}}},
                               {"origin": [-4.5, 21.6, -4.5], "size": [9, 0, 9], "uv": {"up": {"uv": [20, 0], "uv_size": [9, 9]}}}]},
                    {"name": "window", "parent": "item", "pivot": [0, 24, 0],
                     "cubes": [{"origin": [-4.75, 23, -4.75], "size": [9.25, 0, 9.5],
                                "uv": {"up": {"uv": window_uv, "uv_size": [1, 1]}}}]}]}
    files["models/entity/fptest_clockrep.geo.json"] = {"format_version": "1.16.0", "minecraft:geometry": [
        clock_rep("geometry.fptest.clockrep_clear", [31, 31]),
        clock_rep("geometry.fptest.clockrep_opaque", [30, 31])]}
    rep = [[(0, 0, 0, 0)] * 32 for _ in range(32)]
    for y in range(10):
        for x in range(10):
            rep[y][x] = (255, 200, 30, 255) if x in (0, 9) or y in (0, 9) else (0, 0, 0, 0)
            rep[y][10 + x] = (120, 80, 10, 255)
            if x < 9 and y < 9:
                rep[y][20 + x] = (255, 0, 255, 255)
    for y in range(10, 13):
        for x in range(10):
            rep[y][x] = (255, 200, 30, 255)
        for x in range(20, 23):
            rep[y][x] = (0, 220, 220, 255)
    rep[31][30] = (255, 255, 255, 255)
    files["textures/fptest/clockrep.png"] = png(32, 32, rep)
    for item, geo, window in (("compass", "clockrep_clear", "fptest_win_e"),
                              ("recovery_compass", "clockrep_opaque", "fptest_win_e"),
                              ("echo_shard", "clockrep_clear", "fptest_win_a")):
        files["attachables/fptest_rep_%s.json" % item] = {"format_version": "1.10.0", "minecraft:attachable": {"description": {
            "identifier": "minecraft:" + item,
            "materials": {"default": "fptest_case", "window": window, "dial": "fptest_dial"},
            "textures": {"default": "textures/fptest/clockrep"},
            "geometry": {"default": "geometry.fptest." + geo},
            "render_controllers": ["controller.render.fptest.clockrep"]}}}
    ans = {
        "clock": {"geometry": {"default": "geometry.fptest.clock"},
                  "materials": {"default": "entity_alphatest", "window": "fptest_window", "face": "fptest_face"},
                  "render_controllers": ["controller.render.fptest.clock"]},
        "shears": {"geometry": {"default": "geometry.fptest.shears"},
                   "materials": {"default": "entity_alphatest"},
                   "render_controllers": ["controller.render.fptest.simple"],
                   "scripts": {"parent_setup": "v.fptest_two_hands = 1;"}},
        "fishing_rod": {"geometry": {"default": "geometry.fptest.rod"},
                        "materials": {"default": "entity_alphatest"},
                        "render_controllers": ["controller.render.fptest.simple"],
                        "animations": {"rod": "animation.fptest.rod"},
                        "scripts": {"parent_setup": "v.fptest_rod = 1;",
                                    "pre_animation": ["v.rod = c.owning_entity->v.fptest_rod;"],
                                    "animate": ["rod"]}},
    }
    for item, extra in ans.items():
        desc = {"identifier": "minecraft:" + item, "textures": {"default": "textures/fptest/palette"}}
        desc.update(extra)
        files["attachables/fptest_%s.json" % item] = {"format_version": "1.10.0", "minecraft:attachable": {"description": desc}}
    files["animations/fptest_ans.animation.json"] = {"format_version": "1.8.0", "animations": {
        "animation.fptest.rod": {"loop": True, "bones": {"rod": {"scale": [1, "v.rod", 1]}}},
        # The left arm mirrored from vanilla's `first_person.empty_hand` right arm.
        "animation.fptest.two_hands": {"loop": True, "bones": {
            "leftarm": {"position": [-13.5, -10, 12], "rotation": [95, 45, -115]}}}}}
    files["animations/fptest_stick.animation.json"] = {"format_version": "1.8.0", "animations": {
        "animation.fptest.stick.first_person": {"loop": True, "bones": {"item": {"rotation": [-30, 0, 0]}}},
        "animation.fptest.stick.third_person": {"loop": True, "bones": {"item": {"rotation": [90, 0, 0]}}}}}
    files["render_controllers/fptest.render_controllers.json"] = {"format_version": "1.8.0", "render_controllers": {
        "controller.render.fptest.clockrep": {"geometry": "Geometry.default", "textures": ["Texture.default"],
                                              "materials": [{"*": "Material.default"}, {"window": "Material.window"},
                                                            {"icons": "Material.dial"}]},
        "controller.render.fptest.clock": {"geometry": "Geometry.default", "textures": ["Texture.default"],
                                           "materials": [{"*": "Material.default"}, {"window": "Material.window"},
                                                         {"face": "Material.face"}]},
        # Both arms in first person while the holder's `v.fptest_two_hands` is set.
        "controller.render.fptest.two_hands": {"geometry": "Geometry.default", "textures": ["Texture.default"],
                                               "materials": [{"*": "Material.default"}],
                                               "part_visibility": [{"*": False}, {"*rightarm*": True}, {"*leftarm*": True},
                                                                   {"*rightArm*": True}, {"*leftArm*": True},
                                                                   {"*Sleeve*": True}, {"*sleeve*": True}]},
        "controller.render.fptest.sprite": {"geometry": "Geometry.default", "materials": [{"*": "Material.default"}], "textures": ["Texture.sprite"]},
        "controller.render.fptest.simple": {"geometry": "Geometry.default", "materials": [{"*": "Material.default"}], "textures": ["Texture.default"]},
        "controller.render.fptest.pig": {"arrays": {"textures": {"Array.skins": ["Texture.default", "Texture.named"]}},
                                         "geometry": "Geometry.default", "materials": [{"*": "Material.default"}],
                                         "textures": ["Array.skins[q.is_name_any('Teste')]"]}}}

    # Player events: splash in water, ring on landing, each with the blip.
    files["animation_controllers/fptest_player.animation_controllers.json"] = {"format_version": "1.10.0", "animation_controllers": {
        "controller.animation.fptest.player_events": {"initial_state": "default", "states": {
            "default": {"transitions": [{"in_water": "q.is_in_water"}, {"air": "!q.is_on_ground"}]},
            "in_water": {"particle_effects": [{"effect": "fptest_splash"}], "sound_effects": [{"effect": "fptest_blip"}],
                         "transitions": [{"default": "!q.is_in_water"}]},
            "air": {"transitions": [{"in_water": "q.is_in_water"},
                                    {"landed": "q.is_on_ground && q.state_time > 1.0"},
                                    {"default": "q.is_on_ground"}]},
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
    files["entity/pig.entity.json"] = {"format_version": "1.26.30", "minecraft:client_entity": {"description": {
        "identifier": "minecraft:pig",
        # Bedrock keeps the client entity with the newest file `format_version` (vanilla's
        # pig is 1.10.0; Actions & Stuff's, which wins in the game, is 1.26.30).
        "min_engine_version": "1.8.0",
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

    # Item particles: an amethyst shard that puffs green aura from its tip every second.
    files["models/entity/fptest_shard.geo.json"] = geometry("geometry.fptest.shard", [{
        "name": "item", "pivot": [0, 24, 0], "binding": BINDING,
        "locators": {"tip": [0, 14, 0]},
        "cubes": [cube([-1, 14, -1], [2, 10, 2], MAGENTA)]}])
    files["attachables/fptest_amethyst_shard.json"] = {"format_version": "1.10.0", "minecraft:attachable": {"description": {
        "identifier": "minecraft:amethyst_shard",
        "materials": {"default": "entity_alphatest"},
        "textures": {"default": "textures/fptest/palette"},
        "geometry": {"default": "geometry.fptest.shard"},
        "particle_effects": {"aura": "fptest:aura"},
        "animations": {"puff": "animation.fptest.shard.puff"},
        "scripts": {"animate": ["puff"]},
        "render_controllers": ["controller.render.fptest.simple"]}}}
    files["animations/fptest_shard.animation.json"] = {"format_version": "1.8.0", "animations": {
        "animation.fptest.shard.puff": {"loop": True, "animation_length": 1,
                                        "particle_effects": {"0.0": {"effect": "aura", "locator": "tip"}}}}}
    # The game's own particles, replaced as Bedrock's vanilla ones (one particle per request):
    # rain splashes magenta, water splashes yellow, critical hits red.
    manual = {"minecraft:emitter_rate_manual": {"max_particles": 200},
              "minecraft:emitter_lifetime_expression": {"activation_expression": 1, "expiration_expression": 0},
              "minecraft:emitter_shape_point": {}}
    files["particles/fptest_rain.json"] = effect("minecraft:rain_splash_particle", [1.0, 0.0, 1.0, 1.0], dict(manual, **{
        "minecraft:particle_lifetime_expression": {"max_lifetime": 0.6},
        "minecraft:particle_initial_speed": 2,
        "minecraft:emitter_shape_point": {"direction": [0, 1, 0]},
        "minecraft:particle_motion_dynamic": {"linear_acceleration": [0, -9.8, 0]}}))
    files["particles/fptest_water_splash.json"] = effect("minecraft:water_splash_particle_manual", [1.0, 0.9, 0.0, 1.0], dict(manual, **{
        "minecraft:particle_lifetime_expression": {"max_lifetime": 0.8},
        "minecraft:particle_initial_speed": 3,
        "minecraft:emitter_shape_point": {"direction": ["math.random(-0.5, 0.5)", 1, "math.random(-0.5, 0.5)"]},
        "minecraft:particle_motion_dynamic": {"linear_acceleration": [0, -9.8, 0]}}))
    files["particles/fptest_crit.json"] = effect("minecraft:basic_crit_particle", [1.0, 0.1, 0.1, 1.0], dict(once, **{
        "minecraft:emitter_rate_instant": {"num_particles": 1},
        "minecraft:emitter_shape_sphere": {"radius": 0.5, "direction": "outwards"},
        "minecraft:particle_initial_speed": 1}))

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
                                 "entity.pig.name=Pig (test)\ntile.sponge.name=Sponge (test)\n"
                                 "item.brick.name=Brick (texture mesh)\nitem.flint.name=Flint (mesh as A&S)\n"
                                 "item.clock.name=Clock (stencil window)\nitem.compass.name=A&S clock (entity window, clear)\nitem.recovery_compass.name=A&S clock (entity window, opaque)\nitem.echo_shard.name=A&S clock (alphatest window, clear)\nitem.shears.name=Shears (two hands)\nitem.fishing_rod.name=Fishing Rod (holder variable)\nitem.amethyst_shard.name=Amethyst Shard (item particles)\n")
    files["texts/pt_BR.lang"] = ("item.paper.name=Papel (eixos de teste)\nitem.stick.name=Graveto (animação de teste)\n"
                                 "entity.pig.name=Porco (teste)\ntile.sponge.name=Esponja (teste)\n"
                                 "item.brick.name=Tijolo (malha de textura)\nitem.flint.name=Sílex (malha como o A&S)\n"
                                 "item.clock.name=Relógio (janela de stencil)\nitem.compass.name=Relógio A&S (janela entity, transparente)\nitem.recovery_compass.name=Relógio A&S (janela entity, opaca)\nitem.echo_shard.name=Relógio A&S (janela alphatest, transparente)\nitem.shears.name=Tesoura (duas mãos)\nitem.fishing_rod.name=Vara (variável do jogador)\nitem.amethyst_shard.name=Fragmento de Ametista (partículas do item)\n")

    # Images.
    files["textures/fptest/palette.png"] = palette(PALETTE[WHITE])
    files["textures/fptest/pig.png"] = palette(PIG_PINK)
    files["textures/fptest/pig_named.png"] = palette(PIG_CYAN)
    files["textures/fptest/particle.png"] = png(8, 8, [[(255, 255, 255, 255)] * 8 for _ in range(8)])
    files["textures/fptest/sponge.png"] = png(16, 16, [[(255, 0, 255, 255) if (x // 4 + y // 4) % 2 else (20, 20, 20, 255)
                                                         for x in range(16)] for y in range(16)])
    files["manifest.json"] = {"format_version": 2, "header": {
        "name": "Horizon Test Pack", "description": "Teste do motor Bedrock: 1a/3a pessoa, particulas, bloco, entidade, sons, etiquetas",
        "uuid": PACK_UUID, "version": [1, 0, 10], "min_engine_version": [1, 20, 0]},
        "modules": [{"type": "resources", "uuid": MODULE_UUID, "version": [1, 0, 10]}]}
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
