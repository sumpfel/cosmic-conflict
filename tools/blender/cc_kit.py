"""Hard-surface geometry kit and export pipeline for Cosmic Conflict's Blender model generators.

Modelled on MMM's `mmm_geo.py`, but built on bmesh so Blender's own tools finish the models:

* Geometry is written in **Blender coordinates**: x right, **y forward** (nose / front door),
  z up, metres. The glTF exporter turns that into y-up with the nose at -Z, which is the game's
  forward (`Transform::looking_to`). Origin = ground contact point (ground units, buildings,
  deposits) or the centre of mass (ships).
* Every part goes into a *slot* (material name, see `MATERIALS`). The game maps slots to its own
  PBR materials by mesh name `<model>_<Slot>`; the GLB carries placeholders only.
* UVs are in metres (per loop, so curved surfaces have no shading seams) and are scaled to the
  physical size of each slot's photo texture (`textures/<id>/meta.json`), times `tex_scale`.
* `Model.finish()` adds the realism Blender is good at: angle-limited bevels (crisp but rounded
  edges that catch highlights), weighted normals (flat faces stay flat), a second UV map and an
  ambient-occlusion map baked with Cycles (contact shadows, cavities; with a ground plane for
  things that stand on a planet), then splits the model into slot meshes and exports a GLB.

Deterministic: all randomness comes from `Model.rng` (seeded per model).
"""
import json
import math
import os
import random
import struct
import zlib
from contextlib import contextmanager

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector, noise

TAU = math.tau
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ASSETS = os.path.join(ROOT, "crates", "game", "assets")
TEX_DIR = os.path.join(ASSETS, "textures")
MODEL_DIR = os.path.join(ASSETS, "models")

# ------------------------------------------------------------------------------------ materials
# The single source of truth for slot materials. `build_models.py` writes it to
# `crates/game/assets/models/materials.ron` for the game; the preview renderer uses it too.
#   tex      texture set in crates/game/assets/textures (None = plain colour)
#   tint     sRGB multiplier of the albedo (the grey sets get their colour from it)
#   rough    roughness factor (x texture G), metal: metalness factor (x texture B)
#   emissive linear RGB (scaled to the game's exposure); team: tint = player colour;
#   team_glow: emissive = player colour; additive: unlit, additive blending with `alpha`
MATERIALS = {
    "Hull": dict(tex="blue_metal_plate", tint=(178, 182, 188), rough=1.0, metal=1.0),
    "Team": dict(tex="green_metal_rust", tint=(255, 255, 255), rough=0.9, metal=1.0, team=True),
    "Panel": dict(tex="MetalPlates006", tint=(235, 238, 242), rough=1.0, metal=0.75),
    "Metal": dict(tex="Metal009", tint=(235, 235, 235), rough=1.0, metal=1.0),
    "Dark": dict(tex="Metal032", tint=(112, 115, 122), rough=1.0, metal=0.85),
    "Rubber": dict(tex="Rubber004", tint=(150, 150, 150), rough=1.0, metal=0.0),
    "Hazard": dict(tex="PaintedMetal016", tint=(255, 255, 255), rough=1.0, metal=1.0),
    "Deck": dict(tex="DiamondPlate008C", tint=(190, 190, 190), rough=1.0, metal=1.0),
    "Concrete": dict(tex="concrete_floor_worn_001", tint=(215, 212, 205), rough=1.0, metal=0.0),
    "Facade": dict(tex="concrete_wall_004", tint=(222, 224, 226), rough=1.0, metal=0.0),
    "Sheet": dict(tex="corrugated_iron_02", tint=(170, 176, 184), rough=1.0, metal=1.0),
    "Rock": dict(tex="dark_rock", tint=(200, 196, 192), rough=1.0, metal=0.0),
    "OreRock": dict(tex="tiger_rock", tint=(230, 220, 210), rough=1.0, metal=0.0),
    "Ore": dict(tex="Metal062C", tint=(205, 175, 140), rough=0.9, metal=1.0),
    "Glass": dict(tex=None, tint=(18, 30, 42), rough=0.05, metal=0.0, reflectance=0.9),
    "Window": dict(tex=None, tint=(26, 32, 38), rough=0.05, metal=0.0, reflectance=0.9, emissive=(1.3, 1.0, 0.65)),
    "Glow": dict(tex=None, tint=(0, 0, 0), rough=0.4, metal=0.0, team_glow=True),
    "Engine": dict(tex=None, tint=(0, 0, 0), rough=0.4, metal=0.0, emissive=(3.0, 7.0, 14.0)),
    "Light": dict(tex=None, tint=(0, 0, 0), rough=0.4, metal=0.0, emissive=(10.0, 8.0, 5.5)),
    "Crystal": dict(tex=None, tint=(26, 140, 190), rough=0.06, metal=0.0, reflectance=1.0, emissive=(0.1, 0.9, 1.4)),
    "Gas": dict(tex=None, tint=(0, 0, 0), rough=0.4, metal=0.0, emissive=(1.5, 6.0, 1.0)),
    "Haze": dict(tex=None, tint=(128, 255, 102), rough=1.0, metal=0.0, alpha=0.12, additive=True, emissive=(0.1, 0.5, 0.08)),
    # Weapon / shield emitters of the research units (tools/blender/models_arsenal.py).
    "Plasma": dict(tex=None, tint=(0, 0, 0), rough=0.4, metal=0.0, emissive=(14.0, 5.0, 1.2)),
    "Laser": dict(tex=None, tint=(0, 0, 0), rough=0.4, metal=0.0, emissive=(12.0, 1.0, 0.8)),
    "Ion": dict(tex=None, tint=(0, 0, 0), rough=0.4, metal=0.0, emissive=(7.0, 3.0, 16.0)),
    "Shield": dict(tex=None, tint=(110, 170, 255), rough=1.0, metal=0.0, alpha=0.1, additive=True, emissive=(0.15, 0.4, 1.0)),
}

# Slots that are never bevelled (soft or glowing surfaces, scanned-looking rock).
SOFT_SLOTS = {"Rock", "OreRock", "Glow", "Engine", "Light", "Gas", "Haze", "Glass", "Window", "Plasma", "Laser", "Ion", "Shield"}


def tex_meta(tex):
    p = os.path.join(TEX_DIR, tex, "meta.json")
    if os.path.exists(p):
        with open(p) as f:
            return json.load(f)
    return {"size_m": 2.0}


def write_materials_ron(path):
    """Writes MATERIALS as RON for the game (crates/game/src/game/models.rs reads it)."""
    lines = ["// Generated by tools/blender/build_models.py from cc_kit.MATERIALS - edit it there.", "{"]
    for slot, m in MATERIALS.items():
        tex = f'Some("{m["tex"]}")' if m.get("tex") else "None"
        t = m.get("tint", (255, 255, 255))
        e = m.get("emissive", (0.0, 0.0, 0.0))
        lines.append(
            f'    "{slot}": (tex: {tex}, tint: ({t[0]}, {t[1]}, {t[2]}), rough: {float(m.get("rough", 1.0))}, '
            f'metal: {float(m.get("metal", 0.0))}, reflectance: {float(m.get("reflectance", 0.5))}, '
            f'emissive: ({float(e[0])}, {float(e[1])}, {float(e[2])}), team: {str(m.get("team", False)).lower()}, '
            f'team_glow: {str(m.get("team_glow", False)).lower()}, alpha: {float(m.get("alpha", 1.0))}, '
            f'additive: {str(m.get("additive", False)).lower()}),'
        )
    lines.append("}")
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


# ------------------------------------------------------------------------------------- helpers
def V(*a):
    return Vector(a[0]) if len(a) == 1 else Vector(a)


def lerp(a, b, t):
    return a + (b - a) * t


def smoothstep(t):
    t = min(max(t, 0.0), 1.0)
    return t * t * (3 - 2 * t)


def frame_from(normal):
    """Two tangents for a planar projection with this normal (u horizontal where possible)."""
    n = V(normal).normalized()
    if abs(n.z) > 0.7:
        t = V(1, 0, 0)
    else:
        t = V(0, 0, 1).cross(n).normalized()
    b = n.cross(t).normalized()
    return t, b


def basis(origin, x, y, z):
    """4x4 matrix with these axes (columns) at origin."""
    m = Matrix.Identity(4)
    for i, ax in enumerate((x, y, z)):
        ax = V(ax)
        m[0][i], m[1][i], m[2][i] = ax.x, ax.y, ax.z
    m[0][3], m[1][3], m[2][3] = origin
    return m


def look_matrix(a, b, roll_up=(0, 0, 1)):
    """Frame at `a` whose local +Z points to `b` (for beams, tubes, barrels)."""
    a, b = V(a), V(b)
    z = (b - a).normalized()
    up = V(roll_up)
    if abs(z.dot(up)) > 0.98:
        up = V(1, 0, 0) if abs(z.x) < 0.9 else V(0, 1, 0)
    x = up.cross(z).normalized()
    y = z.cross(x)
    return basis(a, x, y, z)


def superellipse(a, n):
    """Point on a unit superellipse (exponent n: 2 = ellipse, large = rectangle)."""
    c, s = math.cos(a), math.sin(a)
    e = 2.0 / n
    return math.copysign(abs(c) ** e, c), math.copysign(abs(s) ** e, s)


# --------------------------------------------------------------------------------------- model
class Model:
    def __init__(self, name, seed=0, kind="unit"):
        self.name = name
        self.kind = kind  # "unit" (ground), "ship", "building", "deposit"
        self.rng = random.Random(seed)
        self.noise_offset = V(self.rng.uniform(-500, 500), self.rng.uniform(-500, 500), self.rng.uniform(-500, 500))
        self.bms = {}
        self.stack = [Matrix.Identity(4)]
        self.tex_scale = 1.0  # < 1: textures repeat more often (small units)
        self.bevel = 0.02
        self.no_bevel = set(SOFT_SLOTS)

    # ------------------------------------------------------------------ transforms
    def _bm(self, slot):
        if slot not in MATERIALS:
            raise KeyError(f"unknown slot {slot}")
        if slot not in self.bms:
            bm = bmesh.new()
            bm.loops.layers.uv.new("UVMap")
            self.bms[slot] = bm
        return self.bms[slot]

    @contextmanager
    def at(self, loc=(0, 0, 0), rot=(0, 0, 0), scale=1.0):
        """Translate, rotate (XYZ Euler, radians) and scale everything added inside."""
        if isinstance(scale, (int, float)):
            scale = (scale, scale, scale)
        m = Matrix.Translation(V(loc)) @ Euler(rot, "XYZ").to_matrix().to_4x4() @ Matrix.Diagonal((*scale, 1.0))
        self.stack.append(self.stack[-1] @ m)
        try:
            yield
        finally:
            self.stack.pop()

    @contextmanager
    def frame(self, m):
        self.stack.append(self.stack[-1] @ m)
        try:
            yield
        finally:
            self.stack.pop()

    def sym(self, axis=0):
        """`for s in m.sym(): ...` builds the body twice, mirrored across x = 0 (s = +1, -1)."""
        for s in (1, -1):
            d = [1.0, 1.0, 1.0, 1.0]
            d[axis] = s
            self.stack.append(self.stack[-1] @ Matrix.Diagonal(d))
            try:
                yield s
            finally:
                self.stack.pop()

    def around(self, n, phase=0.0):
        """`for i in m.around(6): ...` repeats the body rotated around z."""
        for i in range(n):
            self.stack.append(self.stack[-1] @ Matrix.Rotation(phase + i * TAU / n, 4, "Z"))
            try:
                yield i
            finally:
                self.stack.pop()

    # ------------------------------------------------------------------ raw geometry
    def add(self, slot, verts, faces, uvs=None, smooth=True):
        """Adds faces. `uvs`: per face a list of per-loop (u, v) in local metres, or None for a
        planar projection of each face in world space (neighbouring parts line up)."""
        m = self.stack[-1]
        flip = m.to_3x3().determinant() < 0
        scale = sum(abs(s) for s in m.to_scale()) / 3.0
        bm = self._bm(slot)
        uvl = bm.loops.layers.uv.active
        world = [m @ V(v) for v in verts]
        bv = [bm.verts.new(p) for p in world]
        for fi, f in enumerate(faces):
            idx = list(f)
            if len(set(idx)) < 3:
                continue
            fuv = list(uvs[fi]) if uvs is not None else None
            if flip:
                idx.reverse()
                if fuv:
                    fuv.reverse()
            try:
                face = bm.faces.new([bv[i] for i in idx])
            except ValueError:
                continue
            face.smooth = smooth
            if fuv is None:
                pts = [world[i] for i in idx]
                n = (pts[1] - pts[0]).cross(pts[2] - pts[0])
                if n.length < 1e-12:
                    n = V(0, 0, 1)
                t, b = frame_from(n)
                fuv = [(p.dot(t), p.dot(b)) for p in pts]
                for loop, uv in zip(face.loops, fuv):
                    loop[uvl].uv = uv
            else:
                for loop, uv in zip(face.loops, fuv):
                    loop[uvl].uv = (uv[0] * scale, uv[1] * scale)
        return bv

    # ------------------------------------------------------------------ primitives
    def box(self, slot, size, at=(0, 0, 0), rot=(0, 0, 0), top=(1.0, 1.0), shift=(0.0, 0.0), front=1.0):
        """Box of `size` centred at `at`. `top` scales the top face (taper), `shift` moves it
        (slope), `front` scales the +y end in x and z (wedge noses)."""
        hx, hy, hz = size[0] / 2, size[1] / 2, size[2] / 2
        c = []
        for z, k, d in ((-hz, (1.0, 1.0), (0.0, 0.0)), (hz, top, shift)):
            for x, y in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
                f = front if y > 0 else 1.0
                c.append((x * hx * k[0] * f + d[0], y * hy * k[1] + d[1], z * (f if z > 0 else 1.0)))
        faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
        with self.at(at, rot):
            self.add(slot, c, faces, None)

    def beam(self, slot, a, b, w, h=None, roll_up=(0, 0, 1)):
        """Rectangular strut from a to b."""
        a, b = V(a), V(b)
        L = (b - a).length
        if L < 1e-6:
            return
        h = w if h is None else h
        with self.frame(look_matrix(a, b, roll_up)):
            self._zbox(slot, w, h, L)

    def _zbox(self, slot, w, h, L):
        hx, hy = w / 2, h / 2
        c = [(-hx, -hy, 0), (hx, -hy, 0), (hx, hy, 0), (-hx, hy, 0), (-hx, -hy, L), (hx, -hy, L), (hx, hy, L), (-hx, hy, L)]
        faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
        self.add(slot, c, faces, None)

    def lathe(self, slot, profile, at=(0, 0, 0), rot=(0, 0, 0), seg=24, cap_bottom=True, cap_top=True,
              sx=1.0, sy=1.0, phase=0.0, smooth=True):
        """Surface of revolution around local z. profile = [(radius, z), ...] from bottom to top
        (outer side on the right). Radius 0 gives a pole."""
        verts, faces, uvs = [], [], []
        rings = []
        rmax = max(r for r, _ in profile) or 1.0
        acc, vs, prev = 0.0, [], None
        for r, z in profile:
            if prev is not None:
                acc += math.hypot(r - prev[0], z - prev[1])
            vs.append(acc)
            prev = (r, z)
        for r, z in profile:
            if r < 1e-6:
                rings.append([len(verts)] * seg)
                verts.append((0.0, 0.0, z))
            else:
                ring = []
                for i in range(seg):
                    a = phase + i / seg * TAU
                    ring.append(len(verts))
                    verts.append((math.cos(a) * r * sx, math.sin(a) * r * sy, z))
                rings.append(ring)
        du = TAU * rmax / seg
        for k in range(len(profile) - 1):
            for i in range(seg):
                j = (i + 1) % seg
                f = [rings[k][i], rings[k][j], rings[k + 1][j], rings[k + 1][i]]
                uv = [(i * du, vs[k]), ((i + 1) * du, vs[k]), ((i + 1) * du, vs[k + 1]), (i * du, vs[k + 1])]
                # collapse poles into triangles
                keep = [(v, t) for n, (v, t) in enumerate(zip(f, uv)) if n == 0 or v != f[n - 1]]
                if len(keep) > 1 and keep[-1][0] == keep[0][0]:
                    keep.pop()
                if len(keep) >= 3:
                    faces.append([v for v, _ in keep])
                    uvs.append([t for _, t in keep])
        if cap_bottom and profile[0][0] > 1e-6:
            ring = rings[0]
            faces.append(list(reversed(ring)))
            uvs.append([(verts[v][0], verts[v][1]) for v in reversed(ring)])
        if cap_top and profile[-1][0] > 1e-6:
            ring = rings[-1]
            faces.append(list(ring))
            uvs.append([(verts[v][0], verts[v][1]) for v in ring])
        with self.at(at, rot):
            self.add(slot, verts, faces, uvs, smooth=smooth)

    def cyl(self, slot, r, h, at=(0, 0, 0), rot=(0, 0, 0), seg=24, r2=None, caps=True, base=False):
        """Cylinder (cone if r2) along local z, centred (or standing on z=0 if `base`)."""
        z0 = 0.0 if base else -h / 2
        self.lathe(slot, [(r, z0), (r if r2 is None else r2, z0 + h)], at, rot, seg, caps, caps)

    def cyl_between(self, slot, a, b, r, r2=None, seg=16, caps=True):
        a, b = V(a), V(b)
        L = (b - a).length
        if L < 1e-6:
            return
        with self.frame(look_matrix(a, b)):
            self.lathe(slot, [(r, 0.0), (r if r2 is None else r2, L)], seg=seg, cap_bottom=caps, cap_top=caps)

    def sphere(self, slot, r, at=(0, 0, 0), scale=(1, 1, 1), seg=24, rings=12, rot=(0, 0, 0), cut=None):
        """UV sphere; `cut` = lowest z (fraction of r, -1..1) for domes."""
        lo = -1.0 if cut is None else cut
        prof = []
        a0 = math.asin(max(-1.0, min(1.0, lo)))
        for k in range(rings + 1):
            a = lerp(a0, math.pi / 2, k / rings)
            prof.append((math.cos(a) * r if k < rings else 0.0, math.sin(a) * r))
        if cut is None:
            prof[0] = (0.0, -r)
        with self.at(at, rot, scale):
            self.lathe(slot, prof, seg=seg, cap_bottom=cut is not None)

    def torus(self, slot, R, r, at=(0, 0, 0), rot=(0, 0, 0), seg=32, rseg=10, sz=1.0):
        prof = [(R + math.cos(a) * r, math.sin(a) * r * sz) for a in [-math.pi + k / rseg * TAU for k in range(rseg + 1)]]
        # A closed profile: the lathe welds the first and last ring visually (same positions).
        self.lathe(slot, prof, at, rot, seg, False, False)

    def loft(self, slot, sections, seg=24, cap_back=True, cap_front=True):
        """Fuselage along +y. sections = [(y, half_w, h_top, h_bottom, z_centre, n), ...] from back
        to front; n is the superellipse exponent (2 round, 6+ boxy). half_w 0 = pointed tip."""
        verts, faces, uvs, rings = [], [], [], []
        arcs = []
        for (y, w, ht, hb, cz, n) in sections:
            if w < 1e-6:
                rings.append([len(verts)] * seg)
                verts.append((0.0, y, cz))
                arcs.append([0.0] * (seg + 1))
                continue
            ring, pts = [], []
            for i in range(seg):
                a = -math.pi / 2 + i / seg * TAU
                cx, sz = superellipse(a, n)
                p = (cx * w, y, cz + sz * (ht if sz > 0 else hb))
                ring.append(len(verts))
                verts.append(p)
                pts.append(p)
            arc = [0.0]
            for i in range(seg):
                p, q = pts[i], pts[(i + 1) % seg]
                arc.append(arc[-1] + math.hypot(q[0] - p[0], q[2] - p[2]))
            rings.append(ring)
            arcs.append(arc)
        for k in range(len(sections) - 1):
            ya, yb = sections[k][0], sections[k + 1][0]
            for i in range(seg):
                j = (i + 1) % seg
                f = [rings[k][i], rings[k + 1][i], rings[k + 1][j], rings[k][j]]
                ua = arcs[k] if sections[k][1] > 1e-6 else arcs[k + 1]
                ub = arcs[k + 1] if sections[k + 1][1] > 1e-6 else arcs[k]
                uv = [(ua[i], ya), (ub[i], yb), (ub[i + 1], yb), (ua[i + 1], ya)]
                keep = [(v, t) for n_, (v, t) in enumerate(zip(f, uv)) if n_ == 0 or v != f[n_ - 1]]
                if len(keep) > 1 and keep[-1][0] == keep[0][0]:
                    keep.pop()
                if len(keep) >= 3:
                    faces.append([v for v, _ in keep])
                    uvs.append([t for _, t in keep])
        if cap_back and sections[0][1] > 1e-6:
            r = rings[0]
            faces.append(list(r))
            uvs.append([(verts[v][0], verts[v][2]) for v in r])
        if cap_front and sections[-1][1] > 1e-6:
            r = rings[-1]
            faces.append(list(reversed(r)))
            uvs.append([(verts[v][0], verts[v][2]) for v in reversed(r)])
        self.add(slot, verts, faces, uvs)

    def plate(self, slot, outline, t, at=(0, 0, 0), rot=(0, 0, 0), t_fn=None, bottom=None):
        """Extruded polygon: outline [(x, y), ...] counter-clockwise in local xy, thickness t
        along z (centred). `t_fn(x, y)` varies the thickness (airfoils, tapered fins)."""
        n = len(outline)
        area = sum(outline[i][0] * outline[(i + 1) % n][1] - outline[(i + 1) % n][0] * outline[i][1] for i in range(n))
        if area < 0:
            outline = list(reversed(outline))
        verts = []
        for (x, y) in outline:
            th = t if t_fn is None else t_fn(x, y)
            verts.append((x, y, -th / 2 if bottom is None else bottom))
        for (x, y) in outline:
            th = t if t_fn is None else t_fn(x, y)
            verts.append((x, y, th / 2 if bottom is None else bottom + th))
        faces = [list(range(n - 1, -1, -1)), list(range(n, 2 * n))]
        for i in range(n):
            j = (i + 1) % n
            faces.append([i, j, n + j, n + i])
        with self.at(at, rot):
            self.add(slot, verts, faces, None)

    def tube(self, slot, pts, r, seg=10, caps=True, taper=None):
        """Pipe along a polyline (parallel-transport frames)."""
        pts = [V(p) for p in pts]
        if len(pts) < 2:
            return
        verts, faces, uvs = [], [], []
        prev_x, acc, n = None, [0.0], len(pts)
        for k in range(1, n):
            acc.append(acc[-1] + (pts[k] - pts[k - 1]).length)
        for k in range(n):
            d = (pts[min(k + 1, n - 1)] - pts[max(k - 1, 0)]).normalized()
            if prev_x is None:
                x = d.cross(V(0, 0, 1)) if abs(d.z) < 0.95 else d.cross(V(1, 0, 0))
            else:
                x = prev_x - d * prev_x.dot(d)
            x.normalize()
            prev_x = x
            y = d.cross(x)
            rr = r * (taper(k / (n - 1)) if taper else 1.0)
            for i in range(seg):
                a = i / seg * TAU
                verts.append(tuple(pts[k] + (x * math.cos(a) + y * math.sin(a)) * rr))
        du = TAU * r / seg
        for k in range(n - 1):
            for i in range(seg):
                j = (i + 1) % seg
                faces.append([k * seg + i, k * seg + j, (k + 1) * seg + j, (k + 1) * seg + i])
                uvs.append([(i * du, acc[k]), ((i + 1) * du, acc[k]), ((i + 1) * du, acc[k + 1]), (i * du, acc[k + 1])])
        if caps:
            faces.append(list(range(seg - 1, -1, -1)))
            uvs.append([(0.0, 0.0)] * seg)
            faces.append([(n - 1) * seg + i for i in range(seg)])
            uvs.append([(0.0, 0.0)] * seg)
        self.add(slot, verts, faces, uvs)

    def rock(self, slot, r, at=(0, 0, 0), scale=(1, 1, 1), detail=3, rough=0.3, sharp=0.5, flat=-0.35, rot=(0, 0, 0)):
        """Noise-displaced icosphere (boulders). `sharp` adds ridged noise (craggy breaks),
        `flat` flattens everything below that z (fraction of r) so it sits on the ground."""
        tmp = bmesh.new()
        bmesh.ops.create_icosphere(tmp, subdivisions=detail, radius=1.0)
        off = self.noise_offset + V(self.rng.uniform(-50, 50), self.rng.uniform(-50, 50), self.rng.uniform(-50, 50))
        verts = []
        for v in tmp.verts:
            p = v.co.copy()
            q = p * 1.3 + off
            d = noise.fractal(q, 0.8, 2.1, 4, noise_basis="PERLIN_ORIGINAL") * rough
            d += -abs(noise.noise(q * 2.3 + V(7, 3, 1), noise_basis="PERLIN_ORIGINAL")) * rough * sharp
            p = p * (1.0 + d)
            p.z = max(p.z, flat)
            verts.append((p.x * r * scale[0], p.y * r * scale[1], p.z * r * scale[2]))
        idx = {v: i for i, v in enumerate(tmp.verts)}
        faces = [[idx[v] for v in f.verts] for f in tmp.faces]
        tmp.free()
        with self.at(at, rot):
            self.add(slot, verts, faces, None)

    # ------------------------------------------------------------------ details
    def greebles(self, slot, origin, u, v, size_u, size_v, count, h=(0.02, 0.08), s=(0.05, 0.25), slots=None):
        """Scatters small boxes and caps on a rectangle (origin = corner, u/v = edge directions)."""
        u, v = V(u).normalized(), V(v).normalized()
        n = u.cross(v).normalized()
        with self.frame(basis(V(origin), u, v, n)):
            for _ in range(count):
                w = self.rng.uniform(*s)
                d = self.rng.uniform(*s)
                hh = self.rng.uniform(*h)
                x = self.rng.uniform(w / 2, max(w / 2, size_u - w / 2))
                y = self.rng.uniform(d / 2, max(d / 2, size_v - d / 2))
                sl = self.rng.choice(slots) if slots else slot
                if self.rng.random() < 0.25:
                    self.cyl(sl, min(w, d) / 2, hh, at=(x, y, 0), seg=10, base=True)
                else:
                    self.box(sl, (w, d, hh), at=(x, y, hh / 2), top=(0.85, 0.85))

    def vents(self, slot, at, size, n, rot=(0, 0, 0), depth=0.03):
        """A framed grille of n slats (size = w, h of the opening), facing local +y."""
        w, h = size
        with self.at(at, rot):
            self.box(slot, (w + 0.04, depth, 0.03), at=(0, 0, h / 2 + 0.015))
            self.box(slot, (w + 0.04, depth, 0.03), at=(0, 0, -h / 2 - 0.015))
            for k in range(n):
                z = -h / 2 + (k + 0.5) * h / n
                self.box(slot, (w, depth * 0.6, h / n * 0.55), at=(0, depth * 0.2, z), rot=(0.5, 0, 0))

    def bolts(self, slot, pts, r=0.012, h=0.008, normal=(0, 0, 1)):
        n = V(normal).normalized()
        for p in pts:
            with self.frame(look_matrix(V(p), V(p) + n)):
                self.lathe(slot, [(r, 0.0), (r, h * 0.5), (r * 0.6, h)], seg=6, cap_bottom=False)

    def lattice(self, slot, base, top, half_base, half_top, levels, post=0.12, brace=0.06, brace_slot=None):
        """Square lattice tower (4 posts, X-braces per level) from z=base to z=top."""
        bs = brace_slot or slot
        def corner(sx, sy, z):
            t = (z - base) / (top - base)
            h = lerp(half_base, half_top, t)
            return V(sx * h, sy * h, z)
        cs = [(1, 1), (-1, 1), (-1, -1), (1, -1)]
        for sx, sy in cs:
            self.beam(slot, corner(sx, sy, base), corner(sx, sy, top), post)
        for k in range(levels):
            z0 = lerp(base, top, k / levels)
            z1 = lerp(base, top, (k + 1) / levels)
            for i in range(4):
                a, b = cs[i], cs[(i + 1) % 4]
                self.beam(bs, corner(*a, z0), corner(*b, z1), brace)
                self.beam(bs, corner(*b, z0), corner(*a, z1), brace)
                self.beam(bs, corner(*a, z1), corner(*b, z1), brace)

    def crystal(self, slot, base, direction, length, r, sides=6):
        """Prism crystal with a pyramid tip, from `base` (sunk a little) along `direction`."""
        d = V(direction).normalized()
        with self.frame(look_matrix(V(base) - d * r, V(base) + d * length)):
            tip = length + r
            self.lathe(slot, [(r * 0.9, 0.0), (r, tip * 0.12), (r * 0.96, tip * 0.78), (0.0, tip)],
                       seg=sides, phase=self.rng.uniform(0, TAU), smooth=False)

    # ------------------------------------------------------------------ output
    def tris(self):
        return sum(len(f.verts) - 2 for bm in self.bms.values() for f in bm.faces)

    def finish(self, out_dir, ao_size=512, ao_distance=None, ground=None, bake=True, samples=96):
        """Creates the Blender objects, applies bevel + weighted normals, bakes AO, splits into
        slot meshes and exports `<out_dir>/<name>.glb` and `<name>_ao.png`. Returns stats."""
        scene = bpy.context.scene
        coll = scene.collection
        objs = []
        for slot, bm in self.bms.items():
            if not bm.faces:
                continue
            size = tex_meta(MATERIALS[slot]["tex"])["size_m"] if MATERIALS[slot].get("tex") else 1.0
            k = 1.0 / (max(size, 0.05) * self.tex_scale)
            uvl = bm.loops.layers.uv.active
            for f in bm.faces:
                for loop in f.loops:
                    loop[uvl].uv = loop[uvl].uv * k
            bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
            me = bpy.data.meshes.new(f"{self.name}_{slot}")
            bm.to_mesh(me)
            bm.free()
            mat = bpy.data.materials.get(slot) or bpy.data.materials.new(slot)
            me.materials.append(mat)
            ob = bpy.data.objects.new(f"{self.name}_{slot}", me)
            coll.objects.link(ob)
            if slot not in self.no_bevel and self.bevel > 0:
                bv = ob.modifiers.new("bevel", "BEVEL")
                bv.width = self.bevel
                bv.segments = 2
                bv.limit_method = "ANGLE"
                bv.angle_limit = math.radians(40)
                bv.use_clamp_overlap = True
                bv.miter_outer = "MITER_ARC"
                wn = ob.modifiers.new("wn", "WEIGHTED_NORMAL")
                wn.mode = "FACE_AREA"
                wn.keep_sharp = True
                wn.weight = 50
            objs.append(ob)
        self.bms = {}
        # Apply the modifiers (custom normals survive).
        dg = bpy.context.evaluated_depsgraph_get()
        for ob in objs:
            if not ob.modifiers:
                continue
            me = bpy.data.meshes.new_from_object(ob.evaluated_get(dg), preserve_all_data_layers=True, depsgraph=dg)
            old = ob.data
            ob.modifiers.clear()
            ob.data = me
            me.name = old.name
            bpy.data.meshes.remove(old)
            me.name = ob.name
        # Join for a shared AO atlas.
        bpy.ops.object.select_all(action="DESELECT")
        for ob in objs:
            ob.select_set(True)
        bpy.context.view_layer.objects.active = objs[0]
        if len(objs) > 1:
            bpy.ops.object.join()
        model = bpy.context.view_layer.objects.active
        model.name = self.name
        me = model.data
        me.name = self.name
        # The join leaves the other slot meshes orphaned; free their names for the split.
        for orphan in [d for d in bpy.data.meshes if d.users == 0]:
            bpy.data.meshes.remove(orphan)
        ao_png = os.path.join(out_dir, f"{self.name}_ao.png")
        if bake:
            ao_uv = me.uv_layers.new(name="AO")
            me.uv_layers.active = ao_uv
            bpy.ops.object.mode_set(mode="EDIT")
            bpy.ops.mesh.select_all(action="SELECT")
            bpy.ops.uv.smart_project(angle_limit=math.radians(60), island_margin=0.004, area_weight=0.0, correct_aspect=True)
            bpy.ops.object.mode_set(mode="OBJECT")
            self._bake_ao(model, ao_png, ao_size, ao_distance, ground, samples)
            me.uv_layers.active = me.uv_layers["UVMap"]
            me.uv_layers["UVMap"].active_render = True
        # Split back into one object per slot.
        bpy.ops.object.select_all(action="DESELECT")
        model.select_set(True)
        bpy.context.view_layer.objects.active = model
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.separate(type="MATERIAL")
        bpy.ops.object.mode_set(mode="OBJECT")
        parts = [o for o in scene.objects if o.type == "MESH" and o.name != "__ground"]
        tri_count = 0
        for o in parts:
            slot = o.data.materials[0].name
            o.name = f"{self.name}_{slot}"
            o.data.name = o.name
            tri_count += sum(len(p.vertices) - 2 for p in o.data.polygons)
        g = bpy.data.objects.get("__ground")
        if g:
            bpy.data.objects.remove(g)
        os.makedirs(out_dir, exist_ok=True)
        bpy.ops.object.select_all(action="DESELECT")
        for o in parts:
            o.select_set(True)
        bpy.ops.export_scene.gltf(
            filepath=os.path.join(out_dir, f"{self.name}.glb"),
            export_format="GLB",
            use_selection=True,
            export_yup=True,
            export_apply=True,
            export_texcoords=True,
            export_normals=True,
            export_tangents=False,
            export_materials="PLACEHOLDER",
            export_animations=False,
            export_skins=False,
            export_morph=False,
            export_extras=False,
            export_cameras=False,
            export_lights=False,
        )
        return {"tris": tri_count, "slots": sorted(o.data.materials[0].name for o in parts)}

    def _bake_ao(self, ob, path, size, distance, ground, samples):
        scene = bpy.context.scene
        scene.render.engine = "CYCLES"
        scene.cycles.device = "CPU"
        scene.cycles.samples = samples
        scene.render.bake.margin = 6
        scene.render.bake.margin_type = "EXTEND"
        if scene.world is None:
            scene.world = bpy.data.worlds.new("bake")
        dims = ob.dimensions
        scene.world.light_settings.distance = distance or max(0.3, 0.25 * max(dims))
        if ground is not None:
            gm = bpy.data.meshes.new("__ground")
            s = max(dims) * 4 + 4
            gm.from_pydata([(-s, -s, ground), (s, -s, ground), (s, s, ground), (-s, s, ground)], [], [(0, 1, 2, 3)])
            g = bpy.data.objects.new("__ground", gm)
            scene.collection.objects.link(g)
        img = bpy.data.images.new(f"{self.name}_ao", size, size, alpha=False, float_buffer=False)
        img.colorspace_settings.name = "Non-Color"
        for slot_mat in ob.data.materials:
            slot_mat.use_nodes = True
            nt = slot_mat.node_tree
            node = nt.nodes.new("ShaderNodeTexImage")
            node.image = img
            nt.nodes.active = node
        bpy.ops.object.select_all(action="DESELECT")
        ob.select_set(True)
        bpy.context.view_layer.objects.active = ob
        bpy.ops.object.bake(type="AO", use_clear=True)
        write_gray_png(img, path)
        for slot_mat in ob.data.materials:
            nt = slot_mat.node_tree
            for n in [n for n in nt.nodes if n.type == "TEX_IMAGE"]:
                nt.nodes.remove(n)
        bpy.data.images.remove(img)


def write_gray_png(img, path):
    """8-bit greyscale PNG of the image's red channel, rows flipped to top-down (glTF UVs)."""
    w, h = img.size
    px = list(img.pixels)
    rows = []
    for y in range(h - 1, -1, -1):
        base = y * w * 4
        row = bytes(max(0, min(255, int(px[base + x * 4] * 255 + 0.5))) for x in range(w))
        rows.append(b"\x00" + row)
    raw = zlib.compress(b"".join(rows), 9)

    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0)))
        f.write(chunk(b"IDAT", raw))
        f.write(chunk(b"IEND", b""))


def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
