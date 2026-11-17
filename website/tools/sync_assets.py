#!/usr/bin/env python3
"""Copy the game's 3D models into the website.

    python3 website/tools/sync_assets.py

Reads crates/game/assets/models/*.glb (+ <key>_ao.png, materials.ron), the texture sets in
crates/game/assets/textures/ and the names/stats in data/units.ron + data/buildings.ron, and
writes website/assets/3d/:

    models/<key>.glb            unchanged copies
    ao/<key>.jpg                baked ambient occlusion, downscaled
    textures/<id>/*.jpg         albedo / normal / arm, downscaled for the web
    manifest.json               models, slot materials, player colours (read by js/showroom.js)

Run it again whenever the models change. Needs Pillow (pip install pillow).
"""
import json
import pathlib
import re
import shutil
import sys

from PIL import Image

WEB = pathlib.Path(__file__).resolve().parents[1]
REPO = WEB.parent
SRC = REPO / "crates" / "game" / "assets"
OUT = WEB / "assets" / "3d"
TEX_SIZE = 512
AO_SIZE = 512

CATEGORY_DE = {
    "ground": "Bodeneinheit",
    "ship": "Raumschiff",
    "economy": "Wirtschaft",
    "production": "Produktion",
    "research": "Forschung",
    "defence": "Verteidigung",
    "sensors": "Sensoren",
    "deposit": "Vorkommen",
    "other": "Sonstiges",
}
DEPOSITS = {
    "deposit_metal": "Metal Deposit",
    "deposit_crystal": "Crystal Deposit",
    "deposit_gas": "Gas Deposit",
}


def ron_entries(path):
    """Split a RON list of `( id: "...", ... )` entries into (id, text) pairs."""
    text = path.read_text(encoding="utf-8")
    starts = [m.start() for m in re.finditer(r'^\s*id:\s*"', text, re.M)]
    for i, s in enumerate(starts):
        block = text[s:starts[i + 1] if i + 1 < len(starts) else len(text)]
        yield re.search(r'id:\s*"([^"]+)"', block).group(1), block


def field(block, name, pattern=r'"([^"]*)"'):
    m = re.search(rf'\b{name}:\s*{pattern}', block)
    return m.group(1) if m else None


def number(block, name):
    v = field(block, name, r'(-?[\d.]+)')
    return float(v) if v is not None else None


def cost(block):
    m = re.search(r'\bcost:\s*\(([^)]*)\)', block)
    if not m:
        return {}
    return {k: int(float(v)) for k, v in re.findall(r'(\w+):\s*([\d.]+)', m.group(1))}


def weapon(block):
    m = re.search(r'\bweapon:\s*Some\(\((.*?)\)\)', block, re.S)
    if not m:
        return None
    w = m.group(1)
    out = {}
    for key in ("damage", "range", "cooldown"):
        v = re.search(rf'\b{key}:\s*([\d.]+)', w)
        if v:
            out[key] = float(v.group(1))
    for key in ("ground", "space"):
        v = re.search(rf'\b{key}:\s*(true|false)', w)
        if v:
            out[key] = v.group(1) == "true"
    kind = re.search(r'\bkind:\s*(\w+)', w)
    if kind:
        out["kind"] = kind.group(1)
    return out


def read_entities():
    """model key -> info from data/units.ron and data/buildings.ron."""
    entities = {}
    for path, is_unit in ((REPO / "data" / "units.ron", True), (REPO / "data" / "buildings.ron", False)):
        if not path.exists():
            continue
        for ident, block in ron_entries(path):
            if is_unit:
                category = "ship" if field(block, "domain", r'(\w+)') == "Space" else "ground"
            else:
                category = (field(block, "group", r'(\w+)') or "other").lower()
            info = {
                "name": field(block, "name") or ident,
                "category": category,
                "hp": number(block, "hp"),
                "speed": number(block, "speed"),
                "cost": cost(block),
                "weapon": weapon(block),
            }
            entities[field(block, "model") or ident] = {k: v for k, v in info.items() if v not in (None, {})}
    return entities


def read_materials():
    text = (SRC / "models" / "materials.ron").read_text(encoding="utf-8")
    mats = {}
    for name, body in re.findall(r'^\s*"(\w+)":\s*\((.*)\),\s*$', text, re.M):
        tex = re.search(r'tex:\s*(?:Some\("([^"]+)"\)|None)', body)
        tuple3 = lambda key: [float(x) for x in re.search(rf'\b{key}:\s*\(([^)]*)\)', body).group(1).split(",")]
        flt = lambda key: float(re.search(rf'\b{key}:\s*([\d.]+)', body).group(1))
        boolean = lambda key: re.search(rf'\b{key}:\s*(true|false)', body).group(1) == "true"
        mats[name] = {
            "tex": tex.group(1) if tex else None,
            "tint": [int(x) for x in tuple3("tint")],
            "rough": flt("rough"),
            "metal": flt("metal"),
            "emissive": tuple3("emissive"),
            "team": boolean("team"),
            "teamGlow": boolean("team_glow"),
            "alpha": flt("alpha"),
            "additive": boolean("additive"),
        }
    return mats


def read_player_colors():
    lib = REPO / "crates" / "cc_core" / "src" / "lib.rs"
    m = re.search(r'PLAYER_COLORS[^=]*=\s*\[(.*?)\];', lib.read_text(encoding="utf-8"), re.S)
    return [[int(a), int(b), int(c)] for a, b, c in re.findall(r'\((\d+),\s*(\d+),\s*(\d+)\)', m.group(1))]


def save_jpg(src, dst, size, mode, quality):
    dst.parent.mkdir(parents=True, exist_ok=True)
    img = Image.open(src).convert(mode)
    if img.width > size:
        img = img.resize((size, size * img.height // img.width), Image.LANCZOS)
    img.save(dst, "JPEG", quality=quality, optimize=True)


def main():
    if not (SRC / "models" / "materials.ron").exists():
        sys.exit(f"no models found in {SRC / 'models'} - generate them first (tools/blender/build_models.py)")
    entities = read_entities()
    materials = read_materials()

    glbs = {g.stem: g for g in (SRC / "models").glob("*.glb")}
    order = ([k for k in entities if k in glbs] + [k for k in DEPOSITS if k in glbs]
             + sorted(k for k in glbs if k not in entities and k not in DEPOSITS))
    models = []
    for key in order:
        glb = glbs[key]
        info = entities.get(key)
        if info is None and key in DEPOSITS:
            info = {"name": DEPOSITS[key], "category": "deposit"}
        if info is None:
            info = {"name": key.replace("_", " ").title(), "category": "other"}
        dst = OUT / "models" / glb.name
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(glb, dst)
        ao = SRC / "models" / f"{key}_ao.png"
        if ao.exists():
            save_jpg(ao, OUT / "ao" / f"{key}.jpg", AO_SIZE, "L", 88)
        models.append({"key": key, **info, "categoryLabel": CATEGORY_DE.get(info["category"], info["category"]),
                       "ao": ao.exists(), "kb": round(glb.stat().st_size / 1024)})

    used = sorted({m["tex"] for m in materials.values() if m["tex"]})
    for tex in used:
        for part, mode, quality in (("albedo", "RGB", 84), ("normal", "RGB", 90), ("arm", "RGB", 88)):
            src = SRC / "textures" / tex / f"{part}.jpg"
            if src.exists():
                save_jpg(src, OUT / "textures" / tex / f"{part}.jpg", TEX_SIZE, mode, quality)

    # remove models that no longer exist in the game
    keys = {m["key"] for m in models}
    for stale in list((OUT / "models").glob("*.glb")) + list((OUT / "ao").glob("*.jpg")):
        if stale.stem not in keys:
            stale.unlink()

    for credits in ("models/CREDITS.md", "textures/CREDITS.md"):
        if (SRC / credits).exists():
            (OUT / credits).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(SRC / credits, OUT / credits)

    manifest = {"models": models, "materials": materials, "playerColors": read_player_colors()}
    (OUT / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")
    size = sum(f.stat().st_size for f in OUT.rglob("*") if f.is_file()) / 1e6
    print(f"{len(models)} models, {len(used)} texture sets -> {OUT.relative_to(REPO)} ({size:.1f} MB)")


if __name__ == "__main__":
    main()
