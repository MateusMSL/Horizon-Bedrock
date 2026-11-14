#!/usr/bin/env python3
"""Checklist of a Bedrock pack's particle effects and who sets each off, for Horizon.

    python tools/bedrock/particles_checklist.py <unpacked pack dir> <horizon repo> > docs/bedrock-particulas.md

Per effect: whether it replaces a vanilla Bedrock particle (`minecraft:*`) and, if so, the Java
particle Horizon hands it (`bedrock_particles.rs`); which entities, attachables, animations and
controllers name it; its emitter rate (manual effects make one particle per request).
"""
import json
import re
import sys
from collections import defaultdict
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
    root, repo = Path(sys.argv[1]), Path(sys.argv[2])
    redirects = {}
    src = (repo / "crates/horizon-client-render/src/bedrock_particles.rs").read_text()
    for java, ids in re.findall(r"P::(\w+) => &\[([^\]]+)\]", src):
        for i in re.findall(r'"([^"]+)"', ids):
            redirects[i] = java

    effects = {}
    for f in sorted(root.glob("particles/**/*.json")):
        pe = (load(f) or {}).get("particle_effect", {})
        ident = pe.get("description", {}).get("identifier")
        if not ident:
            continue
        comps = pe.get("components", {})
        rate = next((k.split("emitter_rate_")[1] for k in comps if "emitter_rate_" in k), "instant")
        effects[ident.lower()] = {"file": f.name, "rate": rate}

    # Who names each effect: short names in client entities and attachables.
    users = defaultdict(set)
    for kind, folder, root_key in [
        ("entidade", "entity", "minecraft:client_entity"),
        ("attachable", "attachables", "minecraft:attachable"),
    ]:
        for f in root.glob(f"{folder}/**/*.json"):
            d = (load(f) or {}).get(root_key, {}).get("description", {})
            for full in (d.get("particle_effects") or {}).values():
                users[str(full).lower()].add(f"{kind} `{d.get('identifier', f.name)}`")
    for folder in ["particles"]:
        for f in root.glob(f"{folder}/**/*.json"):
            text = f.read_text(encoding="utf-8-sig", errors="ignore")
            for ident in re.findall(r'"effect"\s*:\s*"([^"]+)"', text):
                users[ident.lower()].add(f"partícula `{f.name}`")

    vanilla = sorted(i for i in effects if i.startswith("minecraft:"))
    pack = sorted(i for i in effects if not i.startswith("minecraft:"))
    print("# Partículas do pack no Horizon\n")
    print(f"Gerado por `tools/bedrock/particles_checklist.py`: {len(effects)} efeitos, "
          f"{len(vanilla)} substituem partículas vanilla do Bedrock, {len(pack)} do próprio pack.\n")
    print("## Substitutas de partículas vanilla\n")
    print("O Horizon entrega ao efeito do pack a partícula Java da coluna *Java* (`bedrock_particles.rs`), quando o efeito faz uma partícula por pedido (`manual` ou rajada pequena).\n")
    print("| ok | efeito | Java | taxa | arquivo |")
    print("| --- | --- | --- | --- | --- |")
    for i in vanilla:
        e = effects[i]
        java = redirects.get(i)
        if java and e["rate"] == "steady":
            col = f"`{java}` não: o efeito é um fluxo, a partícula do jogo fica"
        else:
            col = f"`{java}`" if java else "sem equivalente"
        print(f"| [ ] | `{i}` | {col} | {e['rate']} | {e['file']} |")
    print("\n## Efeitos do pack\n")
    print("Disparados por animações/controladores das entidades e attachables que os nomeiam.\n")
    print("| ok | efeito | taxa | usado por |")
    print("| --- | --- | --- | --- |")
    for i in pack:
        e = effects[i]
        who = sorted(users.get(i, []))
        shown = ", ".join(who[:4]) + (f" e mais {len(who) - 4}" if len(who) > 4 else "")
        print(f"| [ ] | `{i}` | {e['rate']} | {shown or '-'} |")


if __name__ == "__main__":
    main()
