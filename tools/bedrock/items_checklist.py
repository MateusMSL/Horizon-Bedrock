#!/usr/bin/env python3
"""Checklist of every item a Bedrock pack dresses (attachables), for testing in Horizon.

    python tools/bedrock/items_checklist.py <unpacked pack dir> > docs/bedrock-itens-checklist.md

Per item: the attachable that draws it, how many bones carry cubes, whether it has first-person
animations, animation controllers, particle effects, sounds and a stencil material.
"""
import json
import re
import sys
from pathlib import Path


def load(p):
    try:
        text = p.read_text(encoding="utf-8-sig")
    except OSError:
        return None
    text = re.sub(r"^\s*//.*$", "", text, flags=re.M)
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return None


def main():
    root = Path(sys.argv[1])
    geos = {}
    for f in root.glob("models/**/*.json"):
        d = load(f) or {}
        for g in d.get("minecraft:geometry", []) or []:
            gid = g.get("description", {}).get("identifier", "").lower()
            geos[gid] = sum(1 for b in g.get("bones", []) if b.get("cubes") or b.get("texture_meshes"))
    controllers = set()
    for f in root.glob("animation_controllers/**/*.json"):
        controllers |= set((load(f) or {}).get("animation_controllers", {}))
    stencil = set()
    for f in root.glob("materials/*.material"):
        for k, v in ((load(f) or {}).get("materials", {}) or {}).items():
            if isinstance(v, dict) and "EnableStencilTest" in json.dumps(v):
                stencil.add(k.split(":")[0])

    rows = {}
    for f in sorted(root.glob("attachables/**/*.json")):
        d = (load(f) or {}).get("minecraft:attachable", {}).get("description", {})
        ident = d.get("identifier", "")
        items = list((d.get("item") or {ident: None}).keys())
        anims = d.get("animations", {}) or {}
        fp = [k for k in anims if "first" in k]
        ctrls = [k for k, v in anims.items() if v in controllers]
        cubes = sum(geos.get(str(g).lower(), 0) for g in (d.get("geometry") or {}).values())
        mats = [str(m) for m in (d.get("materials") or {}).values()]
        info = {
            "file": f.name,
            "attachable": ident,
            "bones": cubes,
            "fp": bool(fp),
            "anims": len(anims),
            "ctrls": len(ctrls),
            "particles": len(d.get("particle_effects") or {}),
            "sounds": len(d.get("sound_effects") or {}),
            "stencil": any(m in stencil for m in mats),
        }
        for item in items:
            name = item.split(":")[-1]
            if not name or name.startswith("oreville"):
                continue
            rows.setdefault(name, []).append(info)

    print("# Checklist de itens do pack (attachables)\n")
    print(f"Gerado por `tools/bedrock/items_checklist.py` ({len(rows)} itens). Marcar no jogo:")
    print("**2D** sprite certo · **3D** modelo na mão · **1P/3P** primeira/terceira pessoa ·")
    print("**Anim** animação (parado, ataque, uso) · **Part** partículas · **Som** sons.\n")
    print("| ok | item | attachable | ossos | 1P | anims | ctrl | part | sons | stencil |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for name in sorted(rows):
        for r in rows[name]:
            print(
                f"| [ ] | `{name}` | `{r['attachable']}` ({r['file']}) | {r['bones']} | "
                f"{'sim' if r['fp'] else '-'} | {r['anims']} | {r['ctrls']} | {r['particles'] or '-'} | "
                f"{r['sounds'] or '-'} | {'sim' if r['stencil'] else '-'} |"
            )


if __name__ == "__main__":
    main()
