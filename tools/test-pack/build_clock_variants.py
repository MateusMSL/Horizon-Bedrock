"""Builds `horizon-clock-variants.mcpack`: copies of Actions & Stuff's 3D clock, one per item, to
find the Bedrock rule that shows its dial (a plane 1.4 px behind an invisible window that writes
stencil 3 and depth) where a plainer copy (test pack 1.0.9) hides it.

V0 copies everything the clock does (the numbers of `geometry.oreville_ans.ntjtva`, the texture
layout, the material chain, the attachable's format and animations, the render controller);
each other variant is V0 with one thing changed. Load this pack alone in Bedrock, hold each
item in first person: the dial is the magenta square with a yellow cross, inside the gold rim.

Usage: python tools/test-pack/build_clock_variants.py out.mcpack

Texture: own colours; only the shapes (which texels are clear) follow the clock's.
"""
import io
import json
import struct
import sys
import zipfile
import zlib

PACK_UUID = "7c1f4a52-3b8e-4d61-9a0e-5f2c8b9d1e01"
MODULE_UUID = "7c1f4a52-3b8e-4d61-9a0e-5f2c8b9d1e02"
VERSION = [1, 0, 0]


def png(w, h, rows):
    raw = b"".join(b"\0" + b"".join(bytes(p) for p in row) for row in rows)

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


# The clock's texture is 1024x1024 with its parts around (471, 50); the same places here.
SIZE = 1024
CLEAR = (0, 0, 0, 0)
GOLD, DARK, BROWN = (240, 190, 40, 255), (150, 100, 20, 255), (110, 70, 15, 255)
MAGENTA, YELLOW, CYAN = (255, 0, 255, 255), (255, 240, 0, 255), (0, 220, 220, 255)


def texture():
    t = [[CLEAR] * SIZE for _ in range(SIZE)]

    def fill(x, y, w, h, c):
        for j in range(y, y + h):
            for i in range(x, x + w):
                t[j][i] = c

    # Case top (471, 50) 10x10: a rim, clear inside, a 2x2 knob, the lower 4 rows solid.
    top = ["##########", "#........#", "#........#", "#........#", "#...##...#",
           "#...##...#", "##########", "##########", "##########", "##########"]
    for j, row in enumerate(top):
        for i, ch in enumerate(row):
            if ch == "#":
                t[50 + j][471 + i] = GOLD
    fill(481, 50, 10, 10, BROWN)   # bottom (drawn flipped from y 60)
    fill(471, 60, 20, 6, DARK)     # sides
    fill(491, 50, 8, 1, GOLD)      # the inner ridge
    # Dial (471, 67) 12x12: magenta with a yellow cross; icons (487, 67/71) 4x4 cyan.
    fill(471, 67, 12, 12, MAGENTA)
    fill(476, 68, 2, 10, YELLOW)
    fill(472, 72, 10, 2, YELLOW)
    fill(487, 67, 4, 8, CYAN)
    # (470, 49), the window's texel, stays clear.
    return png(SIZE, SIZE, t)


def geometry(identifier, flat=False, window_first=False, window_y=2.0):
    def face(u, v, w, h):
        return {"uv": [u, v], "uv_size": [w, h]}

    case = {"name": "4dg4c", "parent": "item", "pivot": [0, 0, 0], "cubes": [
        {"origin": [-5, 0, -5], "size": [10, 3, 10], "uv": {
            "north": face(481, 60, 9.98, 2.98), "east": face(471, 60, 9.98, 2.98),
            "south": face(481, 63, 9.98, 2.98), "west": face(471, 63, 9.98, 2.98),
            "up": face(471, 50, 9.98, 9.98), "down": face(481, 59.98, 9.98, -9.98)}},
        {"origin": [-4, 2, -1], "size": [8, 1, 5], "uv": {
            "north": face(491, 50, 7.98, 0.98), "east": face(491, 50, 4.98, 0.98),
            "south": face(491, 50, 7.98, 0.98), "west": face(491, 50, 4.98, 0.98)}},
        {"origin": [-1, 2, -1], "size": [2, 1, 2], "uv": {
            "east": face(494, 50, 1.98, 0.98), "south": face(494, 50, 1.98, 0.98),
            "west": face(494, 50, 1.98, 0.98)}}]}
    dial = {"name": "khaf", "parent": "4dg4c", "pivot": [0, 0.1, 0], "cubes": [
        {"origin": [-1.5, 2.1, 1], "size": [3, 0, 3], "uv": {"up": face(487, 67, 3.98, 3.98)}},
        {"origin": [-1.5, 2.1, -4], "size": [3, 0, 3], "uv": {"up": face(487, 71, 3.98, 3.98)}},
        {"origin": [-4.5, 0.6, -4.5], "size": [9, 0, 9], "uv": {"up": face(471, 67, 11.98, 11.98)}}]}
    window = {"name": "e2kc", "parent": "4dg4c", "pivot": [0, 0, 0], "cubes": [
        {"origin": [-4.75, window_y, -4.75], "size": [9.25, 0, 9.5],
         "uv": {"up": face(470.98, 49.98, -0.02, -0.02)}}]}
    binding = "c.item_slot=='torso'?'body':q.item_slot_to_bone_name(c.item_slot)"
    if flat:
        bones = [{"name": "item", "pivot": [0, 0, 0], "binding": binding}]
        dial["pivot"] = [0, 0, 0]
    else:
        bones = [{"name": "root", "pivot": [0, 0, 0]},
                 {"name": "item", "parent": "root", "pivot": [0, 0, 0], "binding": binding}]
    bones.append(case)
    bones += [window, dial] if window_first else [dial, window]
    return {"description": {"identifier": identifier, "visible_bounds_width": 2,
                            "visible_bounds_height": 1.5, "texture_width": SIZE, "texture_height": SIZE,
                            "visible_bounds_offset": [0, 0.25, 0]},
            "bones": bones}


def materials():
    stencil = lambda ref, extra=(): {  # noqa: E731
        "+states": ["EnableStencilTest", "StencilWrite", *extra],
        "frontFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"},
        "backFace": {"stencilFunc": "Always", "stencilPassOp": "Replace"}, "stencilRef": ref}
    equal3 = {"+states": ["EnableStencilTest"], "frontFace": {"stencilFunc": "Equal"},
              "backFace": {"stencilFunc": "Equal"}, "stencilRef": 3}
    nocolor = ["DisableColorWrite", "DisableAlphaWrite"]
    return {"materials": {"version": "1.0.0",
        # A&S: oreville_klduzy, oreville_sxepuo, khgpgi (case), migzni (window), vgjfwj (dial).
        "fpclk_base:entity_alphatest": {"-defines": ["FANCY"]},
        "fpclk_plain:entity": {},
        "fpclk_case:fpclk_base": stencil(0),
        "fpclk_window:fpclk_plain": stencil(3, nocolor),
        "fpclk_dial:fpclk_base": equal3,
        # Variants.
        "fpclk_case_fancy:entity_alphatest": stencil(0),
        "fpclk_dial_fancy:entity_alphatest": equal3,
        "fpclk_window_at:entity_alphatest": stencil(3, nocolor),
        "fpclk_case_plain:fpclk_base": {},
    }}


ANIMS = {
    "animation.fpclk.first_person": {"loop": True, "bones": {"item": {
        "rotation": [-10, 180, "c.item_slot=='main_hand'?11.5:-11.5"],
        "position": [0, -3, -4.5], "scale": [0.6, 0.6, 0.6]}}},
    "animation.fpclk.third_person": {"loop": True, "bones": {"item": {
        "rotation": [25, 0, 0], "position": ["c.item_slot=='main_hand'?0.5:-0.5", -3.5, -3.85],
        "scale": [0.5, 0.5, 0.5]}}},
}

# item -> (label, overrides)
VARIANTS = [
    ("clock", "V0 igual ao A&S", {}),
    ("compass", "V1 attachable format 1.10.0", {"format": "1.10.0"}),
    ("recovery_compass", "V2 com FANCY (sem -defines)", {"case": "fpclk_case_fancy", "dial": "fpclk_dial_fancy"}),
    ("echo_shard", "V3 sem animacao de 1a pessoa", {"anims": False}),
    ("amethyst_shard", "V4 ossos sem root (item no topo)", {"flat": True}),
    ("prismarine_shard", "V5 janela de entity_alphatest", {"window": "fpclk_window_at"}),
    ("nautilus_shell", "V6 janela antes do mostrador nos ossos", {"window_first": True}),
    ("heart_of_the_sea", "V7 controller: janela e mostrador antes de *", {"rc_order": "named_first"}),
    ("ghast_tear", "V8 caixa sem stencil", {"case": "fpclk_case_plain"}),
    ("blaze_rod", "V9 janela abaixo do mostrador (y 0.5)", {"window_y": 0.5}),
    ("nether_star", "V10 so caixa e mostrador (sem janela)", {"no_window": True}),
]


def build(out):
    files = {
        "manifest.json": {"format_version": 2, "header": {
            "name": "Horizon clock variants", "description": "Relogio do A&S: variantes para achar a regra do mostrador",
            "uuid": PACK_UUID, "version": VERSION, "min_engine_version": [1, 20, 0]},
            "modules": [{"type": "resources", "uuid": MODULE_UUID, "version": VERSION}]},
        "materials/entity.material": materials(),
        "animations/fpclk.animation.json": {"format_version": "1.8.0", "animations": ANIMS},
    }
    geos = []
    rcs = {}
    lang = []
    for item, label, o in VARIANTS:
        geo = "geometry.fpclk." + item
        geos.append(geometry(geo, flat=o.get("flat", False), window_first=o.get("window_first", False),
                             window_y=o.get("window_y", 2.0)))
        rc = "controller.render.fpclk." + item
        mats = [{"*": "Material.case"}, {"e2kc": "Material.window"}, {"khaf": "Material.dial"}]
        if o.get("no_window"):
            mats = [{"*": "Material.case"}, {"e2kc": "Material.hidden"}, {"khaf": "Material.dial"}]
        if o.get("rc_order") == "named_first":
            mats = mats[1:] + mats[:1]
        r = {"geometry": "Geometry.default", "textures": ["Texture.default"], "materials": mats}
        if o.get("no_window"):
            r["part_visibility"] = [{"*": True}, {"e2kc": False}]
        rcs[rc] = r
        desc = {
            "identifier": "minecraft:" + item,
            "materials": {"default": "entity_alphatest", "case": o.get("case", "fpclk_case"),
                          "window": o.get("window", "fpclk_window"), "dial": o.get("dial", "fpclk_dial"),
                          "hidden": "entity_alphatest"},
            "textures": {"default": "textures/fpclk/clock"},
            "geometry": {"default": geo},
            "render_controllers": [rc],
        }
        if o.get("anims", True):
            desc["animations"] = {"first_person": "animation.fpclk.first_person",
                                  "third_person": "animation.fpclk.third_person"}
            desc["scripts"] = {"animate": [{"first_person": "c.is_first_person"},
                                           {"third_person": "!c.is_first_person"}]}
        files["attachables/fpclk_%s.json" % item] = {
            "format_version": o.get("format", "1.26.30"), "minecraft:attachable": {"description": desc}}
        lang.append("item.%s.name=%s" % (item, label))
    files["models/entity/fpclk.geo.json"] = {"format_version": "1.16.0", "minecraft:geometry": geos}
    files["render_controllers/fpclk.render_controllers.json"] = {"format_version": "1.10.0", "render_controllers": rcs}
    text = "\n".join(lang) + "\n"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in files.items():
            z.writestr(name, json.dumps(data, indent=1))
        z.writestr("textures/fpclk/clock.png", texture())
        z.writestr("texts/en_US.lang", text)
        z.writestr("texts/pt_BR.lang", text)
        z.writestr("texts/languages.json", json.dumps(["en_US", "pt_BR"]))
    print("wrote", out, len(files) + 4, "files,", len(VARIANTS), "variants")


if __name__ == "__main__":
    build(sys.argv[1] if len(sys.argv) > 1 else "horizon-clock-variants.mcpack")
