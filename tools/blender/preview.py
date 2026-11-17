"""Renders generated GLBs with their real PBR materials in Cycles (judge models before the game).

    blender -b --python tools/blender/preview.py -- out.png [glb or folder ...] [--size 512]
        [--samples 48] [--gpu] [--ao] [--team r,g,b] [--cols n] [--ships key,key]

Each model gets its own 3/4 "hero" shot (hard key light like a sun in space, a dim studio HDRI
for reflections, black sky; a rock ground under things that stand on a planet). The slot
materials come from `cc_kit.MATERIALS` and `crates/game/assets/textures`, exactly as the game
uses them. `--ao` shows the baked ambient-occlusion maps instead. Tiles are joined with
ImageMagick's `montage` when it is installed (otherwise they stay next to `out.png`).
"""
import glob
import math
import os
import shutil
import subprocess
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cc_kit  # noqa: E402

HDRI = os.path.join(cc_kit.ROOT, "assets_src", "hdri", "photo_studio_01_1k.hdr")


def srgb_to_linear(c):
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def tex_image(tex, name, srgb):
    path = os.path.join(cc_kit.TEX_DIR, tex, name)
    if not os.path.exists(path):
        return None
    img = bpy.data.images.load(path, check_existing=True)
    img.colorspace_settings.name = "sRGB" if srgb else "Non-Color"
    return img


def make_material(slot, team, ao_img=None, ao_uv=None, ao_only=False):
    spec = cc_kit.MATERIALS.get(slot, cc_kit.MATERIALS["Hull"])
    mat = bpy.data.materials.new(f"pv_{slot}")
    mat.use_nodes = True
    nt = mat.node_tree
    nodes, links = nt.nodes, nt.links
    bsdf = nodes["Principled BSDF"]
    out = nodes["Material Output"]
    if ao_only:
        em = nodes.new("ShaderNodeEmission")
        if ao_img is not None:
            t = nodes.new("ShaderNodeTexImage")
            t.image = ao_img
            uvn = nodes.new("ShaderNodeUVMap")
            uvn.uv_map = ao_uv
            links.new(uvn.outputs["UV"], t.inputs["Vector"])
            links.new(t.outputs["Color"], em.inputs["Color"])
        links.new(em.outputs["Emission"], out.inputs["Surface"])
        return mat
    tint = spec.get("tint", (255, 255, 255))
    if spec.get("team"):
        tint = team
    tint_lin = [srgb_to_linear(c) for c in tint] + [1.0]
    uv = nodes.new("ShaderNodeUVMap")
    uv.uv_map = "UVMap"
    tex = spec.get("tex")
    alb = tex_image(tex, "albedo.jpg", True) if tex else None
    if alb:
        ta = nodes.new("ShaderNodeTexImage")
        ta.image = alb
        links.new(uv.outputs["UV"], ta.inputs["Vector"])
        mul = nodes.new("ShaderNodeMix")
        mul.data_type = "RGBA"
        mul.blend_type = "MULTIPLY"
        mul.inputs["Factor"].default_value = 1.0
        links.new(ta.outputs["Color"], mul.inputs["A"])
        mul.inputs["B"].default_value = tint_lin
        links.new(mul.outputs["Result"], bsdf.inputs["Base Color"])
        arm = tex_image(tex, "arm.jpg", False)
        if arm:
            tr = nodes.new("ShaderNodeTexImage")
            tr.image = arm
            links.new(uv.outputs["UV"], tr.inputs["Vector"])
            sep = nodes.new("ShaderNodeSeparateColor")
            links.new(tr.outputs["Color"], sep.inputs["Color"])
            for chan, socket, f in (("Green", "Roughness", spec.get("rough", 1.0)), ("Blue", "Metallic", spec.get("metal", 0.0))):
                m = nodes.new("ShaderNodeMath")
                m.operation = "MULTIPLY"
                m.inputs[1].default_value = f
                links.new(sep.outputs[chan], m.inputs[0])
                links.new(m.outputs[0], bsdf.inputs[socket])
        nrm = tex_image(tex, "normal.jpg", False)
        if nrm:
            tn = nodes.new("ShaderNodeTexImage")
            tn.image = nrm
            links.new(uv.outputs["UV"], tn.inputs["Vector"])
            nm = nodes.new("ShaderNodeNormalMap")
            nm.uv_map = "UVMap"
            links.new(tn.outputs["Color"], nm.inputs["Color"])
            links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    else:
        bsdf.inputs["Base Color"].default_value = tint_lin
        bsdf.inputs["Roughness"].default_value = spec.get("rough", 0.5)
        bsdf.inputs["Metallic"].default_value = spec.get("metal", 0.0)
    if "IOR" in bsdf.inputs and spec.get("reflectance"):
        # Bevy: F0 = 0.16 * reflectance^2  ->  IOR
        f0 = 0.16 * spec["reflectance"] ** 2
        bsdf.inputs["IOR"].default_value = (1 + math.sqrt(f0)) / (1 - math.sqrt(f0))
    em = spec.get("emissive")
    if spec.get("team_glow"):
        em = [srgb_to_linear(c) * 8 + 0.3 for c in team]
    if em and max(em) > 0:
        peak = max(em)
        bsdf.inputs["Emission Color"].default_value = [c / peak for c in em] + [1.0]
        bsdf.inputs["Emission Strength"].default_value = min(6.0, 0.4 * peak) if not spec.get("additive") else 0.6
    if spec.get("additive"):
        mat.surface_render_method = "BLENDED"
        bsdf.inputs["Alpha"].default_value = spec.get("alpha", 0.2)
    return mat


def setup_world(scene, strength=0.35):
    world = bpy.data.worlds.new("pv")
    scene.world = world
    world.use_nodes = True
    nt = world.node_tree
    bg = nt.nodes["Background"]
    if os.path.exists(HDRI):
        env = nt.nodes.new("ShaderNodeTexEnvironment")
        env.image = bpy.data.images.load(HDRI)
        nt.links.new(env.outputs["Color"], bg.inputs["Color"])
    else:
        bg.inputs["Color"].default_value = (0.5, 0.55, 0.6, 1)
    bg.inputs["Strength"].default_value = strength
    # Black sky for camera rays (space), HDRI for reflections and fill.
    lp = nt.nodes.new("ShaderNodeLightPath")
    black = nt.nodes.new("ShaderNodeBackground")
    black.inputs["Color"].default_value = (0, 0, 0, 1)
    mix = nt.nodes.new("ShaderNodeMixShader")
    nt.links.new(lp.outputs["Is Camera Ray"], mix.inputs["Fac"])
    nt.links.new(bg.outputs["Background"], mix.inputs[1])
    nt.links.new(black.outputs["Background"], mix.inputs[2])
    nt.links.new(mix.outputs["Shader"], nt.nodes["World Output"].inputs["Surface"])


def import_glb(path, team, ao_only):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.data.objects if o not in before]
    stem = os.path.splitext(os.path.basename(path))[0]
    ao_path = os.path.join(os.path.dirname(path), f"{stem}_ao.png")
    ao_img = bpy.data.images.load(ao_path) if os.path.exists(ao_path) else None
    if ao_img:
        ao_img.colorspace_settings.name = "Non-Color"
    for o in new:
        if o.type != "MESH":
            continue
        slot = o.data.name.rsplit("_", 1)[-1]
        uvs = o.data.uv_layers
        if len(uvs) > 0:
            uvs[0].name = "UVMap"
        ao_uv = uvs[1].name if len(uvs) > 1 else "UVMap"
        o.data.materials.clear()
        o.data.materials.append(make_material(slot, team, ao_img, ao_uv, ao_only))
    return new


def bounds(objs):
    pts = [o.matrix_world @ Vector(c) for o in objs if o.type == "MESH" for c in o.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def main():
    args = sys.argv[sys.argv.index("--") + 1:]
    opts = {"--size": "512", "--samples": "48", "--team": "40,110,235", "--cols": "0", "--ships": None}
    flags = set()
    rest = []
    i = 0
    while i < len(args):
        a = args[i]
        if a in opts:
            opts[a] = args[i + 1]
            i += 2
        elif a.startswith("--"):
            flags.add(a)
            i += 1
        else:
            rest.append(a)
            i += 1
    out, inputs = rest[0], rest[1:]
    files = []
    for p in inputs:
        files += sorted(glob.glob(os.path.join(p, "*.glb"))) if os.path.isdir(p) else [p]
    size = int(opts["--size"])
    team = [int(c) for c in opts["--team"].split(",")]
    ao_only = "--ao" in flags
    # Ships float (no ground); without --ships the origin decides (ships are centred).
    ships = set(opts["--ships"].split(",")) if opts["--ships"] is not None else None

    cc_kit.reset_scene()
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = int(opts["--samples"])
    scene.cycles.use_denoising = True
    if "--gpu" in flags:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        for kind in ("OPTIX", "CUDA", "HIP"):
            try:
                prefs.compute_device_type = kind
                prefs.get_devices()
                if any(d.type == kind for d in prefs.devices):
                    for d in prefs.devices:
                        d.use = d.type == kind
                    scene.cycles.device = "GPU"
                    break
            except TypeError:
                continue
    scene.render.resolution_x = size
    scene.render.resolution_y = int(size * 0.8)
    scene.render.film_transparent = False
    scene.view_settings.view_transform = "AgX"
    scene.view_settings.look = "AgX - Medium High Contrast"
    setup_world(scene)

    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.2
    sun.data.angle = math.radians(1.5)
    sun.rotation_euler = (math.radians(48), math.radians(12), math.radians(-35))
    scene.collection.objects.link(sun)
    rim = bpy.data.objects.new("rim", bpy.data.lights.new("rim", "SUN"))
    rim.data.energy = 1.2
    rim.data.color = (0.75, 0.85, 1.0)
    rim.rotation_euler = (math.radians(-60), 0, math.radians(160))
    scene.collection.objects.link(rim)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.lens = 50
    scene.collection.objects.link(cam)
    scene.camera = cam

    ground_mat = make_material("Rock", team)
    tiles = []
    groups = []
    for f in files:
        objs = import_glb(f, team, ao_only)
        groups.append((f, objs))
        for o in objs:
            o.hide_render = True
    tmp = os.path.splitext(out)[0] + "_tiles"
    os.makedirs(tmp, exist_ok=True)
    for f, objs in groups:
        for o in objs:
            o.hide_render = False
        lo, hi = bounds(objs)
        c = (lo + hi) / 2
        radius = max((hi - lo).length / 2, 0.3)
        ground = None
        stem = os.path.splitext(os.path.basename(f))[0]
        on_ground = stem not in ships if ships is not None else lo.z > -0.45 * max(hi.z, 0.1)
        if on_ground and not ao_only:
            gm = bpy.data.meshes.new("g")
            s = radius * 6
            gm.from_pydata([(-s, -s, 0), (s, -s, 0), (s, s, 0), (-s, s, 0)], [], [(0, 1, 2, 3)])
            uvl = gm.uv_layers.new(name="UVMap")
            for li, loop in enumerate(gm.loops):
                v = gm.vertices[loop.vertex_index].co
                uvl.data[li].uv = (v.x / 2.0, v.y / 2.0)
            ground = bpy.data.objects.new("g", gm)
            ground.location = (c.x, c.y, -0.01)
            ground.data.materials.append(ground_mat)
            scene.collection.objects.link(ground)
        fov = 2 * math.atan(18 / cam.data.lens)
        d = radius / math.sin(fov / 2) * 1.05
        direction = Vector((-0.62, 0.95, 0.62)).normalized()
        cam.location = c + direction * d
        cam.rotation_euler = (-direction).to_track_quat("-Z", "Y").to_euler()
        cam.data.clip_start = d * 0.02
        cam.data.clip_end = d * 10
        tile = os.path.join(tmp, os.path.splitext(os.path.basename(f))[0] + ".png")
        scene.render.filepath = tile
        bpy.ops.render.render(write_still=True)
        tiles.append(tile)
        for o in objs:
            o.hide_render = True
        if ground:
            bpy.data.objects.remove(ground)
        print("rendered", tile)
    if shutil.which("montage") and tiles:
        cols = int(opts["--cols"]) or min(len(tiles), 4)
        subprocess.run(["montage", "-label", "%t", "-pointsize", "14", "-fill", "white", "-background", "#111",
                        "-geometry", "+2+2", "-tile", f"{cols}x", *tiles, out], check=False)
        print("contact sheet", out)


main()
