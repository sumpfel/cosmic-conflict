#!/usr/bin/env python3
"""Downloads the CC0 PBR textures the generated models use and writes the credits.

    python3 tools/fetch_textures.py [--force] [ids...]

Sources (everything is CC0 1.0 / public domain, checked 2026-11-16):
  * Poly Haven  https://polyhaven.com/license   (API https://api.polyhaven.com)
  * ambientCG   https://ambientcg.com/license   (API https://ambientcg.com/api/v2)

Per texture set `crates/game/assets/textures/<id>/` gets `albedo.jpg`, `normal.jpg` (OpenGL
convention, +Y up) and `arm.jpg` (R = ambient occlusion, G = roughness, B = metalness — the glTF
layout Bevy reads) plus `meta.json` (name, authors, source, licence, physical size in metres).
`tools/blender/build_models.py` reads `meta.json` for the texel scale, the game reads the images.
`crates/game/assets/textures/CREDITS.md` is rewritten from `meta.json`, so credits never drift.

The preview HDRI (Blender only, not shipped with the game) goes to the git-ignored `assets_src/`.
Needs Pillow (`python3 -m pip install pillow`).
"""
import argparse
import io
import json
import os
import sys
import urllib.request
import zipfile

try:
    from PIL import Image, ImageStat
except ImportError:  # pragma: no cover
    sys.exit("fetch_textures.py needs Pillow: python3 -m pip install pillow")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "crates", "game", "assets", "textures")
SRC = os.path.join(ROOT, "assets_src")
UA = {"User-Agent": "cosmic-conflict-asset-fetch/1.0 (open source school game)"}
SIZE = 1024

# id -> (source, used for, processing, physical size in metres if the API has none)
# processing: "gray" = neutral grey albedo with a fixed mean brightness, so a material tint gives
# the final colour (hull paint, team colour); None = as downloaded.
TEXTURES = {
    "blue_metal_plate": ("polyhaven", "Hull: scratched painted hull plates (desaturated, tinted per slot)", "gray", None),
    "green_metal_rust": ("polyhaven", "Team: worn team-colour paint (desaturated, tinted with the player colour)", "gray", None),
    "concrete_floor_worn_001": ("polyhaven", "Concrete: foundations and landing pads", None, None),
    "concrete_wall_004": ("polyhaven", "Facade: painted precast concrete walls of buildings", None, None),
    "corrugated_iron_02": ("polyhaven", "Sheet: corrugated cladding (factory, shipyard)", "gray", None),
    "tiger_rock": ("polyhaven", "OreRock: iron-rich rock of metal deposits", None, None),
    "dark_rock": ("polyhaven", "Rock: crystal and gas deposits", None, None),
    "MetalPlates006": ("ambientcg", "Plates: dark metal panelling", None, 1.5),
    "Metal009": ("ambientcg", "Metal: brushed steel (barrels, nozzles, struts)", None, 1.0),
    "Metal032": ("ambientcg", "Dark: dark gunmetal machinery", None, 1.5),
    "Metal062C": ("ambientcg", "Ore: raw metal veins and nuggets", None, 1.0),
    "Rubber004": ("ambientcg", "Rubber: hover skirts, seals, tyres", None, 1.0),
    "PaintedMetal016": ("ambientcg", "Hazard: yellow/black warning stripes", None, 1.5),
    "DiamondPlate008C": ("ambientcg", "Deck: walkways and platforms", None, 1.0),
}

# Poly Haven HDRIs for the Blender preview renders (not used by the game).
HDRIS = {
    "photo_studio_01": "Blender preview renders only (tools/blender/build_models.py --preview)",
}


def get(url, timeout=120):
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()


def get_json(url):
    return json.loads(get(url))


def load_img(data, mode):
    img = Image.open(io.BytesIO(data))
    img.load()
    img = img.convert(mode)
    if img.size != (SIZE, SIZE):
        img = img.resize((SIZE, SIZE), Image.LANCZOS)
    return img


def neutral_gray(img, mean=200):
    """Grey albedo whose mean brightness is `mean` (sRGB): the material tint sets the colour."""
    g = img.convert("L")
    m = ImageStat.Stat(g).mean[0]
    k = mean / max(m, 1.0)
    g = g.point(lambda v: max(0, min(255, int(v * k + 0.5))))
    return g.convert("RGB")


def save(img, path):
    img.save(path, "JPEG", quality=90, optimize=True)
    print(f"  {os.path.relpath(path, ROOT)}  {os.path.getsize(path) // 1024} KB")


def polyhaven(tex_id, gray):
    files = get_json(f"https://api.polyhaven.com/files/{tex_id}")
    info = get_json(f"https://api.polyhaven.com/info/{tex_id}")
    maps = {}
    for key in ("Diffuse", "nor_gl", "arm"):
        entry = files.get(key, {}).get("1k", {}).get("jpg")
        if not entry:
            raise RuntimeError(f"no 1k jpg for {key}")
        maps[key] = get(entry["url"])
    d = os.path.join(OUT, tex_id)
    albedo = load_img(maps["Diffuse"], "RGB")
    save(neutral_gray(albedo) if gray else albedo, os.path.join(d, "albedo.jpg"))
    save(load_img(maps["nor_gl"], "RGB"), os.path.join(d, "normal.jpg"))
    save(load_img(maps["arm"], "RGB"), os.path.join(d, "arm.jpg"))
    dims = info.get("dimensions") or [2000, 2000]
    return {
        "name": info.get("name", tex_id),
        "authors": sorted(info.get("authors", {}).keys()),
        "source": "Poly Haven",
        "url": f"https://polyhaven.com/a/{tex_id}",
        "license": "CC0 1.0",
        "size_m": round(max(dims) / 1000.0, 3),
    }


def ambientcg(tex_id, gray, size_m):
    data = get(f"https://ambientcg.com/get?file={tex_id}_1K-JPG.zip", timeout=300)
    parts = {}
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        # Only the known map files are read (never paths from the archive).
        for n in z.namelist():
            low = os.path.basename(n).lower()
            for suffix, key in (("_color.jpg", "color"), ("_normalgl.jpg", "normal"), ("_roughness.jpg", "rough"),
                                ("_metalness.jpg", "metal"), ("_ambientocclusion.jpg", "ao")):
                if low.endswith(suffix):
                    parts[key] = z.read(n)
    if "color" not in parts or "normal" not in parts:
        raise RuntimeError("archive without colour/normal map")
    d = os.path.join(OUT, tex_id)
    albedo = load_img(parts["color"], "RGB")
    save(neutral_gray(albedo) if gray else albedo, os.path.join(d, "albedo.jpg"))
    save(load_img(parts["normal"], "RGB"), os.path.join(d, "normal.jpg"))
    white = Image.new("L", (SIZE, SIZE), 255)
    black = Image.new("L", (SIZE, SIZE), 0)
    ao = load_img(parts["ao"], "L") if "ao" in parts else white
    rough = load_img(parts["rough"], "L") if "rough" in parts else Image.new("L", (SIZE, SIZE), 128)
    metal = load_img(parts["metal"], "L") if "metal" in parts else black
    save(Image.merge("RGB", (ao, rough, metal)), os.path.join(d, "arm.jpg"))
    info = get_json(f"https://ambientcg.com/api/v2/full_json?id={tex_id}")["foundAssets"][0]
    return {
        "name": info.get("displayName", tex_id),
        # Every ambientCG asset is made by Lennart Demes (the site's author, see /license).
        "authors": ["Lennart Demes"],
        "source": "ambientCG",
        "url": f"https://ambientcg.com/a/{tex_id}",
        "license": "CC0 1.0",
        "size_m": size_m,
    }


def fetch_hdri(hdri_id):
    out = os.path.join(SRC, "hdri", f"{hdri_id}_1k.hdr")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if not os.path.exists(out):
        files = get_json(f"https://api.polyhaven.com/files/{hdri_id}")
        with open(out, "wb") as f:
            f.write(get(files["hdri"]["1k"]["hdr"]["url"]))
        print(f"  {os.path.relpath(out, ROOT)}")
    info = get_json(f"https://api.polyhaven.com/info/{hdri_id}")
    return {
        "name": info.get("name", hdri_id),
        "authors": sorted(info.get("authors", {}).keys()),
        "url": f"https://polyhaven.com/a/{hdri_id}",
    }


def write_credits():
    rows = []
    for tex_id, (_, purpose, _, _) in TEXTURES.items():
        p = os.path.join(OUT, tex_id, "meta.json")
        if not os.path.exists(p):
            continue
        m = json.load(open(p))
        rows.append(f"| `{tex_id}` | {m['name']} | {', '.join(m['authors'])} | {m['source']} | {m['license']} | "
                    f"{m['url']} | {purpose} |")
    hdri_rows = []
    for hdri_id, purpose in HDRIS.items():
        p = os.path.join(SRC, "hdri", f"{hdri_id}.json")
        if os.path.exists(p):
            m = json.load(open(p))
            hdri_rows.append(f"| `{hdri_id}` | {m['name']} | {', '.join(m['authors'])} | Poly Haven | CC0 1.0 | {m['url']} | {purpose} |")
    with open(os.path.join(OUT, "CREDITS.md"), "w") as f:
        f.write("# Texture credits\n\n")
        f.write("All textures are **CC0 1.0 (public domain)** — free to use, modify and redistribute, also "
                "commercially, without attribution. We credit the artists anyway: thank you, Poly Haven and "
                "ambientCG!\n\n")
        f.write("Downloaded and converted (1024 px JPEG; `arm.jpg` = AO / roughness / metalness; `gray` sets "
                "desaturated so a tint gives the colour) by `tools/fetch_textures.py`.\n\n")
        f.write("| Folder | Name | Author(s) | Source | License | Link | Used for |\n|---|---|---|---|---|---|---|\n")
        f.write("\n".join(rows) + "\n")
        if hdri_rows:
            f.write("\n## Preview lighting (not shipped)\n\n")
            f.write("| File | Name | Author(s) | Source | License | Link | Used for |\n|---|---|---|---|---|---|---|\n")
            f.write("\n".join(hdri_rows) + "\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--force", action="store_true", help="download again even if present")
    ap.add_argument("ids", nargs="*")
    a = ap.parse_args()
    failed = []
    for tex_id, (source, _, proc, size_m) in TEXTURES.items():
        if a.ids and tex_id not in a.ids:
            continue
        d = os.path.join(OUT, tex_id)
        meta_path = os.path.join(d, "meta.json")
        if os.path.exists(meta_path) and not a.force:
            continue
        os.makedirs(d, exist_ok=True)
        print(f"{tex_id} ({source})")
        try:
            gray = proc == "gray"
            meta = polyhaven(tex_id, gray) if source == "polyhaven" else ambientcg(tex_id, gray, size_m)
            if size_m is not None and source == "polyhaven":
                meta["size_m"] = size_m
            with open(meta_path, "w") as f:
                json.dump(meta, f, indent=1)
        except Exception as e:  # keep going: the game and Blender fall back to plain colours
            print(f"  ! {tex_id}: {e}", file=sys.stderr)
            failed.append(tex_id)
    for hdri_id in HDRIS:
        try:
            meta = fetch_hdri(hdri_id)
            with open(os.path.join(SRC, "hdri", f"{hdri_id}.json"), "w") as f:
                json.dump(meta, f, indent=1)
        except Exception as e:
            print(f"  ! hdri {hdri_id}: {e}", file=sys.stderr)
    write_credits()
    print("done" + (f", failed: {', '.join(failed)}" if failed else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
