"""Procedural generators for every unit, building and resource deposit of Cosmic Conflict.

    blender -b --python tools/blender/build_models.py                       # all -> crates/game/assets/models
    blender -b --python tools/blender/build_models.py -- tank fighter       # some
    blender -b --python tools/blender/build_models.py -- --preview /tmp/m.png tank
    blender -b --python tools/blender/build_models.py -- --variants 6 --seed 7 --out /tmp/var --preview /tmp/v.png fighter

Every model is a function of a seeded RNG (proportions, greebles, panels, rock shapes), so
`--variants` shows how much a generator can vary; the game's files use a fixed seed per model,
so rebuilding gives the same files. Output per model: `<key>.glb` (one mesh per material slot,
named `<key>_<Slot>`) and `<key>_ao.png` (baked ambient occlusion on the second UV map).
`materials.ron` (the slot -> PBR material table) is written next to them.

Never hand-edit the GLBs: change the generator and rebuild. Conventions: see `cc_kit.py`.
"""
import math
import os
import sys
import time
import zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cc_kit  # noqa: E402
from cc_kit import TAU, Model, V, lerp  # noqa: E402

PI = math.pi
HALF = PI / 2


# =============================================================================== ground units
def tank(m):
    """Hover tank: sloped hull on four lift fans, rubber skirt, wedge turret with a long gun."""
    r = m.rng
    L = r.uniform(3.0, 3.25)
    W = r.uniform(2.15, 2.35)
    # Skirt and belly.
    m.box("Rubber", (W + 0.08, L + 0.06, 0.2), at=(0, 0, 0.36), top=(0.97, 0.97))
    m.box("Dark", (W - 0.2, L - 0.3, 0.3), at=(0, 0, 0.42))
    # Main hull: sloped glacis at the front, shorter slope at the back.
    m.box("Hull", (W, L, 0.58), at=(0, 0, 0.75), top=(0.9, 0.74), shift=(0, -0.12))
    m.box("Hull", (W * 0.86, L * 0.5, 0.2), at=(0, -0.38, 1.12), top=(0.96, 0.9))
    # Side armour skirts with team stripes.
    for s in m.sym():
        with m.at((W / 2 + 0.04, 0, 0.72), (0, 0.16, 0)):
            m.box("Panel", (0.07, L * 0.84, 0.46), top=(1, 0.96))
            m.box("Team", (0.02, L * 0.5, 0.09), at=(0.04, -0.1, 0.1))
            m.bolts("Metal", [(0.04, y, z) for y in (-L * 0.38, -L * 0.13, L * 0.13, L * 0.38) for z in (-0.17, 0.17)], normal=(1, 0, 0))
        # Lift fans in the corners.
        for y in (-L * 0.36, L * 0.33):
            with m.at((W / 2 - 0.35, y, 0.34)):
                m.lathe("Dark", [(0.36, -0.08), (0.38, 0.0), (0.38, 0.12), (0.3, 0.16)], seg=28)
                m.cyl("Engine", 0.29, 0.02, at=(0, 0, -0.085), seg=24)
        # Headlights and tow hooks.
        m.box("Dark", (0.34, 0.08, 0.14), at=(W * 0.33, L / 2 - 0.02, 0.62))
        m.box("Light", (0.12, 0.03, 0.07), at=(W * 0.33 - 0.07, L / 2 + 0.02, 0.62))
        m.box("Light", (0.12, 0.03, 0.07), at=(W * 0.33 + 0.07, L / 2 + 0.02, 0.62))
        m.lathe("Metal", [(0.05, 0), (0.05, 0.1)], at=(W * 0.18, L / 2 + 0.02, 0.5), rot=(-HALF, 0, 0), seg=10)
        # Exhausts at the back.
        with m.at((0.42, -L / 2 - 0.05, 0.9), (HALF, 0, 0)):
            m.lathe("Metal", [(0.13, -0.05), (0.15, 0.08), (0.13, 0.22), (0.1, 0.22), (0.1, 0.05)], seg=20, cap_bottom=True, cap_top=False)
            m.cyl("Engine", 0.1, 0.02, at=(0, 0, 0.06), seg=16)
        # Smoke dischargers on the turret cheeks are added with the turret below.
    # Rear engine deck: grilles and stowage.
    m.vents("Dark", at=(0, -L / 2 + 0.02, 0.72), size=(0.6, 0.22), n=4, rot=(0, 0, PI))
    m.box("Panel", (W * 0.7, 0.7, 0.06), at=(0, -L * 0.36, 1.25))
    for k in range(3):
        m.box("Dark", (0.08, 0.6, 0.03), at=(-0.4 + k * 0.4, -L * 0.36, 1.29))
    m.greebles("Dark", (-W * 0.42, -L * 0.48, 1.05), (1, 0, 0), (0, 1, 0), W * 0.84, 0.25, 6, h=(0.03, 0.08), s=(0.06, 0.2))
    m.greebles("Hull", (-W * 0.35, L * 0.05, 1.04), (1, 0, 0), (0, 1, 0), W * 0.7, L * 0.2, 5, h=(0.02, 0.05), s=(0.1, 0.3))
    # Turret.
    tz = 1.42
    ty = -0.18
    with m.at((0, ty, tz)):
        m.lathe("Dark", [(0.62, -0.24), (0.66, -0.16), (0.6, -0.12)], seg=32, cap_bottom=False, cap_top=False)
        m.box("Hull", (1.5, 1.75, 0.46), at=(0, 0, 0.0), top=(0.82, 0.86), front=0.72)
        m.box("Hull", (1.18, 0.55, 0.36), at=(0, -1.05, 0.0), top=(0.92, 0.9))
        m.box("Team", (1.52, 0.5, 0.06), at=(0, -0.35, 0.12), top=(0.92, 1.0))
        m.box("Dark", (0.52, 0.32, 0.36), at=(0, 0.88, -0.02), front=0.9)
        # Main gun with bore evacuator and muzzle brake.
        m.cyl_between("Metal", (0, 1.0, -0.02), (0, 2.85, -0.02), 0.085, 0.072, seg=20)
        m.lathe("Metal", [(0.074, 0), (0.11, 0.06), (0.11, 0.3), (0.074, 0.36)], at=(0, 1.95, -0.02), rot=(-HALF, 0, 0), seg=20, cap_bottom=False, cap_top=False)
        m.box("Dark", (0.24, 0.3, 0.17), at=(0, 2.9, -0.02))
        for s in m.sym():
            m.box("Dark", (0.06, 0.18, 0.11), at=(0.13, 2.9, -0.02))
            # Smoke dischargers.
            for k in range(3):
                m.cyl("Dark", 0.045, 0.2, at=(0.62, 0.15 + k * 0.1, 0.12), rot=(-0.6, 0.0, -0.5), seg=10)
            # Side armour plates.
            m.box("Panel", (0.05, 1.0, 0.3), at=(0.66, -0.15, -0.02), rot=(0, -0.28, 0), top=(1, 0.9))
        # Commander cupola, sensors, antenna.
        with m.at((0.32, -0.35, 0.23)):
            m.lathe("Dark", [(0.22, 0), (0.22, 0.12), (0.18, 0.16), (0.0, 0.17)], seg=20)
            for k in m.around(6):
                m.box("Glass", (0.05, 0.03, 0.04), at=(0, 0.215, 0.08))
        with m.at((-0.4, 0.1, 0.25)):
            m.box("Dark", (0.24, 0.3, 0.2), at=(0, 0, 0.08))
            m.cyl("Glass", 0.075, 0.03, at=(0, 0.16, 0.1), rot=(-HALF, 0, 0), seg=16)
        m.cyl("Dark", 0.035, 0.12, at=(-0.45, -1.1, 0.22), seg=8)
        m.cyl("Metal", 0.008, 1.1, at=(-0.45, -1.1, 0.8), seg=6)
        m.greebles("Dark", (-0.5, -1.25, 0.18), (1, 0, 0), (0, 1, 0), 1.0, 0.4, 4, h=(0.03, 0.07), s=(0.08, 0.18))


# ============================================================================== space units
def fighter(m):
    """Space superiority fighter: blended fuselage, swept wings, twin canted fins, two engines."""
    r = m.rng
    nose = r.uniform(1.65, 1.8)
    span = r.uniform(1.6, 1.75)
    sweep = r.uniform(0.75, 1.0)
    m.loft("Hull", [
        (-1.5, 0.44, 0.2, 0.2, 0.0, 4.5),
        (-1.2, 0.5, 0.25, 0.22, 0.0, 4.0),
        (-0.3, 0.44, 0.29, 0.21, 0.02, 3.5),
        (0.45, 0.3, 0.25, 0.17, 0.02, 3.0),
        (1.05, 0.17, 0.15, 0.11, 0.0, 2.6),
        (nose - 0.12, 0.05, 0.05, 0.04, -0.01, 2.2),
        (nose, 0.0, 0.0, 0.0, -0.01, 2.0),
    ], seg=28)
    # Canopy.
    m.loft("Glass", [
        (-0.25, 0.02, 0.0, 0.0, 0.2, 2.0),
        (-0.1, 0.17, 0.12, 0.02, 0.21, 2.2),
        (0.3, 0.19, 0.17, 0.02, 0.22, 2.2),
        (0.75, 0.1, 0.07, 0.02, 0.2, 2.2),
        (0.95, 0.0, 0.0, 0.0, 0.17, 2.0),
    ], seg=20)
    m.box("Dark", (0.4, 0.06, 0.05), at=(0, -0.28, 0.27))
    for s in m.sym():
        # Wing with a thick root and a thin tip.
        root_y0, root_y1 = -1.25, 0.35
        tip_y0, tip_y1 = root_y0 + 0.12, root_y0 + 0.12 + (root_y1 - root_y0) * 0.28
        tip_y0 += 0.0
        tip_y1 -= 0.0
        out = [(0.3, root_y0), (span, tip_y0 - 0.05 * sweep), (span, tip_y1 - 0.05 * sweep), (0.3, root_y1)]
        t_fn = lambda x, y: lerp(0.1, 0.03, (x - 0.3) / (span - 0.3))  # noqa: E731
        with m.at((0, 0, -0.03), (0, -0.05, 0)):
            m.plate("Hull", out, 0.0, t_fn=t_fn)
            # Team stripe near the tip (on top of the wing).
            stripe = [(span - 0.45, tip_y0 + 0.02), (span - 0.25, tip_y0 + 0.01), (span - 0.25, tip_y1 - 0.06), (span - 0.45, tip_y1 + 0.06)]
            m.plate("Team", stripe, 0.006, bottom=lerp(0.1, 0.03, (span - 0.75) / (span - 0.3)) / 2 - 0.002)
            # Wingtip cannon.
            m.cyl_between("Dark", (span, tip_y0 - 0.15, 0.0), (span, tip_y1 + 0.2, 0.0), 0.05, seg=12)
            m.cyl_between("Metal", (span, tip_y1 + 0.2, 0.0), (span, tip_y1 + 0.55, 0.0), 0.022, seg=10)
            m.box("Light", (0.02, 0.05, 0.02), at=(span + 0.05, tip_y0 - 0.1, 0.0))
        # Intakes.
        m.box("Dark", (0.16, 0.5, 0.2), at=(0.42, -0.1, -0.02), front=0.8)
        m.box("Engine", (0.1, 0.02, 0.12), at=(0.42, 0.16, -0.02))
        # Canted fins.
        fin = [(0.0, -1.5), (0.0, -0.95), (0.55, -1.32), (0.58, -1.5)]
        with m.at((0.33, 0.0, 0.16), (0, -HALF + 0.35, 0)):
            m.plate("Hull", fin, 0.035)
            m.plate("Team", [(0.42, -1.48), (0.42, -1.25), (0.52, -1.31), (0.55, -1.48)], 0.04)
        # Engines.
        with m.at((0.22, -1.52, 0.0), (HALF, 0, 0)):
            m.lathe("Metal", [(0.17, -0.05), (0.19, 0.08), (0.16, 0.24), (0.12, 0.24), (0.12, 0.06)], seg=24, cap_bottom=True, cap_top=False)
            m.cyl("Engine", 0.12, 0.02, at=(0, 0, 0.07), seg=20)
            m.torus("Glow", 0.18, 0.012, at=(0, 0, 0.0), seg=24, rseg=6)
    m.greebles("Panel", (-0.25, -1.3, 0.2), (1, 0, 0), (0, 1, 0), 0.5, 0.7, 6, h=(0.01, 0.03), s=(0.06, 0.16))
    m.cyl("Metal", 0.006, 0.25, at=(0, -0.95, 0.35), seg=6)


def drone(m):
    """Construction drone: four ducted fans, sensor head, two manipulator arms and a welder."""
    r = m.rng
    z = 0.86
    m.loft("Hull", [
        (-0.42, 0.22, 0.12, 0.1, z, 3.0),
        (-0.3, 0.3, 0.17, 0.14, z, 3.5),
        (0.25, 0.3, 0.18, 0.15, z, 3.5),
        (0.42, 0.22, 0.13, 0.12, z - 0.01, 3.0),
    ], seg=24)
    m.box("Team", (0.14, 0.62, 0.03), at=(0, -0.02, z + 0.18), top=(0.8, 0.95))
    m.box("Dark", (0.42, 0.3, 0.05), at=(0, -0.05, z - 0.15))
    m.cyl("Engine", 0.11, 0.02, at=(0, -0.05, z - 0.18), seg=20)
    # Sensor head.
    with m.at((0, 0.47, z - 0.02)):
        m.box("Dark", (0.3, 0.16, 0.17), front=0.85)
        m.cyl("Glass", 0.055, 0.04, at=(0.07, 0.08, 0.01), rot=(-HALF, 0, 0), seg=16)
        m.cyl("Glass", 0.035, 0.04, at=(-0.08, 0.08, 0.02), rot=(-HALF, 0, 0), seg=12)
        m.box("Light", (0.04, 0.02, 0.025), at=(-0.08, 0.085, -0.05))
    for s in m.sym():
        for sy in (1, -1):
            hub = V(0.6, 0.5 * sy, z + 0.03)
            m.beam("Dark", V(0.22, 0.22 * sy, z + 0.02), hub - V(0.28, 0, 0) * 0.9, 0.07, 0.05)
            with m.at(hub):
                m.lathe("Hull", [(0.29, -0.07), (0.31, -0.05), (0.31, 0.06), (0.28, 0.08), (0.25, 0.06), (0.25, -0.06)], seg=28, cap_bottom=False, cap_top=False)
                m.cyl("Dark", 0.06, 0.1, seg=12)
                for k in m.around(4, phase=r.uniform(0, TAU)):
                    m.box("Metal", (0.035, 0.2, 0.008), at=(0, 0.13, 0.0), rot=(0.35, 0, 0))
                for k in m.around(2):
                    m.beam("Dark", (0.0, 0.0, -0.03), (0.0, 0.26, -0.03), 0.02)
                m.torus("Glow", 0.27, 0.012, at=(0, 0, -0.07), seg=24, rseg=6)
        # Manipulator arm with a claw.
        sh = V(0.12, 0.28, z - 0.12)
        el = V(0.16, 0.42, z - 0.36)
        wr = V(0.12, 0.6, z - 0.42)
        m.cyl("Dark", 0.04, 0.07, at=sh, rot=(0, HALF, 0), seg=10)
        m.beam("Metal", sh, el, 0.04)
        m.beam("Metal", el, wr, 0.035)
        m.box("Dark", (0.05, 0.05, 0.05), at=wr)
        m.beam("Dark", wr, wr + V(0.03, 0.09, -0.02), 0.018)
        m.beam("Dark", wr, wr + V(-0.03, 0.09, -0.04), 0.018)
    m.cyl_between("Metal", (0.0, 0.3, z - 0.15), (0.0, 0.62, z - 0.3), 0.022, 0.012, seg=8)
    m.sphere("Light", 0.018, at=(0.0, 0.63, z - 0.305), seg=8, rings=4)
    m.cyl("Metal", 0.006, 0.35, at=(-0.12, -0.32, z + 0.32), seg=6)
    m.sphere("Light", 0.012, at=(-0.12, -0.32, z + 0.5), seg=8, rings=4)
    m.greebles("Dark", (-0.2, -0.38, z + 0.16), (1, 0, 0), (0, 1, 0), 0.4, 0.2, 4, h=(0.01, 0.03), s=(0.03, 0.08))


def trooper(m):
    """Power-armoured infantry with a rifle at the low ready."""
    r = m.rng
    k = r.uniform(0.97, 1.03)
    with m.at(scale=k):
        for s in m.sym():
            hip, knee, ankle = V(0.12, -0.02, 0.8), V(0.14, 0.05, 0.45), V(0.13, -0.01, 0.1)
            m.box("Dark", (0.13, 0.28, 0.08), at=(0.13, 0.04, 0.045), top=(0.9, 0.85))
            m.box("Hull", (0.14, 0.12, 0.06), at=(0.13, 0.13, 0.08), top=(0.8, 0.7))
            m.beam("Dark", ankle, knee, 0.11, 0.12)
            m.beam("Hull", ankle + V(0, 0.03, 0.05), knee + V(0, 0.03, -0.03), 0.14, 0.1)
            m.box("Team", (0.13, 0.08, 0.12), at=knee + V(0, 0.08, 0.0), top=(0.85, 0.8))
            m.beam("Dark", knee, hip, 0.14, 0.15)
            m.beam("Hull", knee + V(0, 0.02, 0.06), hip + V(0, 0.02, -0.04), 0.17, 0.13)
            m.cyl("Dark", 0.065, 0.1, at=hip, rot=(0, HALF, 0), seg=12)
            # Shoulder pad.
            m.box("Team", (0.17, 0.24, 0.12), at=(0.29, -0.02, 1.29), top=(0.75, 0.8), rot=(0, -0.3, 0))
        # Arms: right hand on the grip, left hand on the fore-grip.
        arms = [((0.27, -0.02, 1.22), (0.3, 0.1, 0.98), (0.12, 0.28, 1.0)),
                ((-0.27, -0.02, 1.22), (-0.26, 0.22, 1.0), (-0.03, 0.48, 1.05))]
        for sh, el, ha in arms:
            sh, el, ha = V(sh), V(el), V(ha)
            m.sphere("Dark", 0.07, at=sh, seg=12, rings=6)
            m.beam("Dark", sh, el, 0.1)
            m.sphere("Dark", 0.055, at=el, seg=10, rings=5)
            m.beam("Hull", el, ha, 0.11, 0.1)
            m.box("Dark", (0.07, 0.09, 0.08), at=ha)
        # Pelvis, torso, backpack.
        m.box("Dark", (0.32, 0.2, 0.16), at=(0, -0.02, 0.85), top=(1.05, 1.0))
        m.cyl("Dark", 0.12, 0.14, at=(0, -0.02, 0.97), seg=14)
        m.box("Hull", (0.42, 0.27, 0.34), at=(0, 0.0, 1.14), top=(1.12, 1.05), shift=(0, 0.01))
        m.box("Panel", (0.3, 0.04, 0.16), at=(0, 0.14, 1.17), top=(1.05, 1.0))
        m.box("Dark", (0.34, 0.18, 0.38), at=(0, -0.22, 1.12), top=(0.92, 0.9))
        m.vents("Dark", at=(0, -0.315, 1.2), size=(0.2, 0.12), n=3, rot=(0, 0, PI), depth=0.02)
        for s in m.sym():
            with m.at((0.1, -0.24, 0.92), (PI, 0, 0)):
                m.lathe("Metal", [(0.045, 0), (0.05, 0.05), (0.04, 0.09), (0.03, 0.09), (0.03, 0.03)], seg=12, cap_top=False)
            m.cyl("Engine", 0.03, 0.01, at=(0.1, -0.24, 0.84), seg=10)
        m.cyl("Metal", 0.005, 0.45, at=(-0.12, -0.27, 1.5), seg=5)
        # Helmet with a glowing visor.
        m.cyl("Dark", 0.06, 0.08, at=(0, 0.0, 1.35), seg=12)
        m.sphere("Hull", 0.125, at=(0, 0.02, 1.47), scale=(1.0, 1.15, 1.05), seg=18, rings=10)
        m.box("Glow", (0.17, 0.05, 0.035), at=(0, 0.14, 1.48), top=(0.9, 0.7))
        m.box("Dark", (0.2, 0.06, 0.05), at=(0, 0.13, 1.42), front=0.9)
        # Rifle (pitched down a little).
        with m.at((0.05, 0.38, 1.02), (-0.1, 0, 0)):
            m.box("Dark", (0.06, 0.5, 0.1), top=(1.0, 0.95))
            m.box("Dark", (0.05, 0.22, 0.07), at=(0, -0.32, -0.02))
            m.box("Dark", (0.04, 0.06, 0.14), at=(0, 0.05, -0.1), rot=(0.25, 0, 0))
            m.cyl_between("Metal", (0, 0.25, 0.02), (0, 0.5, 0.02), 0.017, seg=10)
            m.cyl("Dark", 0.022, 0.16, at=(0, 0.05, 0.09), rot=(-HALF, 0, 0), seg=10)
            m.box("Team", (0.062, 0.12, 0.03), at=(0, 0.17, 0.0))


def flak(m):
    """Anti-air walker: reverse-jointed legs, a cockpit pod and two quad-barrel gun pods."""
    r = m.rng
    hz = r.uniform(1.18, 1.28)
    for s in m.sym():
        hip, knee, ankle = V(0.52, 0.0, hz), V(0.58, 0.38, hz - 0.5), V(0.56, -0.12, 0.22)
        m.cyl("Dark", 0.17, 0.22, at=hip, rot=(0, HALF, 0), seg=18)
        m.cyl("Metal", 0.09, 0.3, at=hip + V(0.02, 0, 0), rot=(0, HALF, 0), seg=12)
        m.beam("Hull", hip, knee, 0.2, 0.24)
        m.box("Team", (0.04, 0.32, 0.14), at=(hip + knee) / 2 + V(0.11, 0, 0), rot=(math.atan2(0.5, 0.38) - HALF, 0, 0))
        m.cyl("Dark", 0.12, 0.24, at=knee, rot=(0, HALF, 0), seg=16)
        m.beam("Dark", knee, ankle, 0.14, 0.16)
        m.cyl_between("Metal", hip + V(0, -0.12, -0.08), ankle + V(0, -0.05, 0.25), 0.03, seg=8)
        m.sphere("Dark", 0.1, at=ankle, seg=12, rings=6)
        for a in (-0.45, 0.0, 0.45):
            d = V(math.sin(a), math.cos(a), 0)
            m.beam("Dark", ankle + V(0, 0, -0.08), ankle + d * 0.34 + V(0, 0, -0.18), 0.07, 0.05)
        m.beam("Dark", ankle, ankle + V(0, -0.24, -0.17), 0.06, 0.05)
    # Cockpit pod.
    bz = hz + 0.28
    m.box("Hull", (0.86, 1.05, 0.5), at=(0, 0.02, bz), top=(0.85, 0.82), front=0.82)
    m.box("Dark", (0.7, 0.85, 0.16), at=(0, 0.0, bz - 0.3))
    m.box("Glass", (0.5, 0.06, 0.16), at=(0, 0.52, bz + 0.03), rot=(-0.5, 0, 0))
    m.box("Team", (0.88, 0.3, 0.06), at=(0, -0.2, bz + 0.2), top=(0.85, 1.0))
    m.vents("Dark", at=(0, -0.51, bz), size=(0.4, 0.2), n=4, rot=(0, 0, PI), depth=0.03)
    # Gun pods (pitched up 35 degrees).
    for s in m.sym():
        m.cyl("Dark", 0.12, 0.2, at=(0.48, 0.05, bz + 0.05), rot=(0, HALF, 0), seg=14)
        with m.at((0.68, 0.05, bz + 0.08), (0.6, 0, 0)):
            m.box("Hull", (0.3, 0.62, 0.3), top=(0.9, 0.9))
            m.box("Team", (0.31, 0.1, 0.31), at=(0, -0.18, 0))
            m.box("Dark", (0.2, 0.3, 0.22), at=(0, -0.42, 0))
            for bx in (-0.07, 0.07):
                for bzz in (-0.07, 0.07):
                    m.cyl_between("Metal", (bx, 0.3, bzz), (bx, 1.05, bzz), 0.03, seg=10)
                    m.lathe("Dark", [(0.04, 0), (0.04, 0.08)], at=(bx, 1.0, bzz), rot=(-HALF, 0, 0), seg=10)
    # Radar.
    with m.at((-0.18, -0.25, bz + 0.27)):
        m.cyl("Metal", 0.03, 0.3, at=(0, 0, 0.15), seg=8)
        with m.at((0, 0, 0.33), (-0.9, 0, 0.4)):
            m.lathe("Metal", [(0.0, -0.02), (0.08, -0.015), (0.18, 0.02), (0.2, 0.05), (0.19, 0.055), (0.07, 0.01), (0.0, 0.0)], seg=24, smooth=True)
    m.sphere("Light", 0.03, at=(0.2, -0.3, bz + 0.28), seg=8, rings=4)
    m.greebles("Dark", (-0.32, -0.4, bz + 0.25), (1, 0, 0), (0, 1, 0), 0.64, 0.5, 5, h=(0.02, 0.05), s=(0.05, 0.14))


def bomber(m):
    """Orbital bomber: heavy fuselage, thick wings with engine nacelles, bomb bay, dorsal turret."""
    r = m.rng
    m.loft("Hull", [
        (-2.45, 0.52, 0.32, 0.3, 0.0, 4.0),
        (-1.9, 0.72, 0.45, 0.42, 0.0, 4.5),
        (0.6, 0.72, 0.48, 0.42, 0.02, 4.5),
        (1.7, 0.5, 0.38, 0.3, 0.0, 3.4),
        (2.35, 0.24, 0.2, 0.16, -0.02, 2.6),
        (2.55, 0.0, 0.0, 0.0, -0.04, 2.0),
    ], seg=32)
    m.loft("Glass", [
        (1.25, 0.02, 0.0, 0.0, 0.33, 2.0),
        (1.45, 0.3, 0.12, 0.02, 0.33, 2.4),
        (1.9, 0.22, 0.1, 0.02, 0.27, 2.4),
        (2.12, 0.0, 0.0, 0.0, 0.2, 2.0),
    ], seg=20)
    span = r.uniform(2.55, 2.7)
    for s in m.sym():
        out = [(0.6, -1.7), (span, -1.45), (span, -0.85), (0.6, 0.95)]
        t_fn = lambda x, y: lerp(0.22, 0.07, (x - 0.6) / (span - 0.6))  # noqa: E731
        with m.at((0, 0, -0.08)):
            m.plate("Hull", out, 0.0, t_fn=t_fn)
            m.plate("Team", [(span - 0.55, -1.5), (span - 0.3, -1.47), (span - 0.3, -0.95), (span - 0.55, -0.75)], 0.008,
                    bottom=lerp(0.22, 0.07, (span - 0.85) / (span - 0.6)) / 2 - 0.003)
            m.box("Light", (0.04, 0.08, 0.03), at=(span + 0.02, -1.2, 0))
        # Engine nacelle.
        nx = 1.65
        with m.at((nx, 0, -0.1)):
            m.loft("Hull", [
                (-2.0, 0.34, 0.34, 0.34, 0.0, 2.2),
                (-1.6, 0.42, 0.42, 0.42, 0.0, 2.2),
                (0.6, 0.42, 0.42, 0.42, 0.0, 2.2),
                (1.0, 0.36, 0.36, 0.36, 0.0, 2.2),
            ], seg=24, cap_front=False)
            m.lathe("Dark", [(0.36, 0), (0.3, 0.05), (0.18, -0.25), (0.0, -0.3)], at=(0, 1.0, 0), rot=(-HALF, 0, 0), seg=24, cap_bottom=False)
            m.torus("Team", 0.43, 0.035, at=(0, 0.2, 0), rot=(HALF, 0, 0), seg=28, rseg=6)
            with m.at((0, -2.0, 0), (HALF, 0, 0)):
                m.lathe("Metal", [(0.36, -0.02), (0.38, 0.12), (0.32, 0.32), (0.24, 0.32), (0.24, 0.08)], seg=24, cap_bottom=True, cap_top=False)
                m.cyl("Engine", 0.24, 0.02, at=(0, 0, 0.1), seg=20)
                m.torus("Glow", 0.37, 0.018, at=(0, 0, 0.0), seg=24, rseg=6)
        # V tail.
        with m.at((0.4, -1.85, 0.35), (0, -HALF + 0.55, 0)):
            m.plate("Hull", [(0.0, -0.55), (0.0, 0.35), (0.85, -0.45), (0.9, -0.62)], 0.06)
            m.plate("Team", [(0.62, -0.6), (0.62, -0.3), (0.82, -0.44), (0.86, -0.6)], 0.065)
    # Bomb bay doors and keel.
    m.box("Dark", (0.9, 2.2, 0.05), at=(0, -0.3, -0.43))
    m.box("Panel", (0.04, 2.3, 0.06), at=(0, -0.3, -0.42))
    # Dorsal turret.
    with m.at((0, -0.6, 0.47)):
        m.lathe("Dark", [(0.36, -0.02), (0.36, 0.05), (0.3, 0.16), (0.12, 0.24), (0.0, 0.25)], seg=24)
        for s in m.sym():
            m.cyl_between("Metal", (0.09, 0.1, 0.12), (0.09, 0.9, 0.16), 0.03, seg=10)
    m.greebles("Panel", (-0.5, -2.2, 0.42), (1, 0, 0), (0, 1, 0), 1.0, 1.3, 10, h=(0.015, 0.05), s=(0.08, 0.25))
    m.cyl("Metal", 0.01, 0.5, at=(0.3, 1.0, 0.65), seg=6)


def colony_ship(m):
    """Colony ship: command module, spinning habitat ring, cargo pods, tanks, radiators, engines."""
    r = m.rng
    # Spine.
    m.cyl_between("Panel", (0, -2.6, 0), (0, 1.8, 0), 0.36, seg=16)
    # Command module.
    m.loft("Hull", [
        (1.6, 0.9, 0.9, 0.9, 0.0, 2.2),
        (2.4, 0.95, 0.95, 0.95, 0.0, 2.2),
        (2.9, 0.7, 0.7, 0.7, 0.0, 2.2),
        (3.15, 0.25, 0.25, 0.25, 0.0, 2.0),
        (3.2, 0.0, 0.0, 0.0, 0.0, 2.0),
    ], seg=32, cap_back=True)
    for k in range(16):
        a = k / 16 * TAU
        m.box("Light", (0.1, 0.04, 0.06), at=(math.cos(a) * 0.94, 2.25, math.sin(a) * 0.94), rot=(0, -a, 0))
    m.torus("Team", 0.95, 0.06, at=(0, 1.95, 0), rot=(HALF, 0, 0), seg=40, rseg=8)
    # Habitat ring on four spokes.
    with m.at((0, 0.7, 0), (HALF, 0, 0)):
        m.torus("Hull", 2.05, 0.26, seg=56, rseg=14, sz=0.8)
        m.torus("Team", 2.05, 0.27, at=(0, 0, 0), seg=56, rseg=14, sz=0.25)
        m.cyl("Dark", 0.62, 0.55, seg=24)
        for k in m.around(4, phase=PI / 4):
            m.cyl_between("Metal", (0.55, 0, 0), (1.85, 0, 0), 0.07, seg=10)
        for k in range(24):
            a = k / 24 * TAU
            m.box("Light", (0.08, 0.05, 0.06), at=(math.cos(a) * 2.3, math.sin(a) * 2.3, 0.05), rot=(0, 0, a))
    # Cargo pods.
    for i in m.around(6, phase=r.uniform(0, 0.5)):
        for j, y in enumerate((-1.05, -0.35)):
            slot = "Team" if (i + j) % 3 == 0 else "Hull"
            m.box(slot, (0.42, 0.62, 0.42), at=(0.62, y, 0))
            m.box("Dark", (0.44, 0.06, 0.44), at=(0.62, y + 0.31, 0))
    # Propellant tanks.
    for k in m.around(3, phase=0.4):
        m.sphere("Metal", 0.55, at=(0.72, -1.95, 0), seg=24, rings=12)
        m.torus("Dark", 0.55, 0.03, at=(0.72, -1.95, 0), seg=28, rseg=6)
    # Radiators.
    for s in m.sym():
        m.beam("Metal", (0.3, -2.25, 0), (2.6, -2.25, 0), 0.06)
        m.plate("Panel", [(0.5, -2.75), (2.5, -2.65), (2.5, -1.95), (0.5, -1.85)], 0.025, at=(0, 0, 0.0))
    # Engine block.
    with m.at((0, -2.45, 0), (HALF, 0, 0)):
        m.lathe("Dark", [(0.36, -0.1), (0.62, 0.05), (0.92, 0.25), (0.92, 0.4), (0.0, 0.42)], seg=32)
    for k in range(3):
        a = PI / 2 + k * TAU / 3
        with m.at((math.cos(a) * 0.45, -2.85, math.sin(a) * 0.45), (HALF, 0, 0)):
            m.lathe("Metal", [(0.18, -0.05), (0.2, 0.1), (0.36, 0.5), (0.33, 0.5), (0.16, 0.12)], seg=24, cap_bottom=True, cap_top=False)
            m.cyl("Engine", 0.17, 0.02, at=(0, 0, 0.12), seg=18)
    m.cyl("Engine", 0.25, 0.02, at=(0, -2.86, 0), rot=(HALF, 0, 0), seg=20)
    # Antennas and a dish.
    with m.at((0.0, 2.2, 0.95), (-0.35, 0, 0)):
        m.cyl("Metal", 0.03, 0.35, at=(0, 0, 0.17), seg=8)
        with m.at((0, 0, 0.38), (-0.6, 0, 0)):
            m.lathe("Metal", [(0.0, -0.02), (0.15, -0.01), (0.33, 0.06), (0.35, 0.1), (0.33, 0.1), (0.14, 0.02), (0.0, 0.01)], seg=28)
    m.cyl("Metal", 0.01, 0.9, at=(-0.4, 2.6, 0.9), seg=6)
    m.sphere("Light", 0.03, at=(-0.4, 2.6, 1.36), seg=8, rings=4)


# ================================================================================== buildings
def concrete_pad(m, r, h=0.4, seg=8, slot="Concrete"):
    m.lathe(slot, [(r, -0.3), (r, h * 0.6), (r - 0.15, h)], seg=seg, phase=PI / seg)


def command_center(m):
    """Planetary HQ: octagonal base, office ring with window bands, control tower, comms mast."""
    r = m.rng
    concrete_pad(m, 6.7, 0.5)
    m.lathe("Hazard", [(6.56, 0.5), (6.56, 0.52), (6.2, 0.52)], seg=8, phase=PI / 8, cap_bottom=False, cap_top=False)
    # Main ring building.
    m.lathe("Facade", [(4.8, 0.4), (4.8, 3.4), (4.6, 3.6)], seg=8, phase=PI / 8)
    m.lathe("Team", [(4.88, 2.95), (4.88, 3.3), (4.82, 3.32)], seg=8, phase=PI / 8, cap_bottom=False, cap_top=False)
    m.lathe("Dark", [(4.7, 3.55), (4.9, 3.6), (4.9, 3.8), (4.7, 3.82)], seg=8, phase=PI / 8, cap_top=False)
    m.lathe("Concrete", [(4.75, 3.72), (4.75, 3.8), (0.5, 3.8)], seg=8, phase=PI / 8, cap_bottom=False)
    for k in range(8):
        a = PI / 8 + (k + 0.5) * TAU / 8
        apo = 4.8 * math.cos(PI / 8) + 0.03
        with m.at((math.cos(a) * apo, math.sin(a) * apo, 0), (0, 0, a - HALF)):
            if k == 1:  # +y side: entrance
                m.box("Dark", (2.0, 0.3, 2.4), at=(0, 0.05, 1.6))
                m.box("Window", (1.6, 0.1, 2.0), at=(0, 0.18, 1.5))
                m.box("Light", (1.8, 0.12, 0.08), at=(0, 0.25, 2.75))
                m.box("Concrete", (3.0, 1.6, 0.15), at=(0, 1.0, 0.4))
                m.box("Glow", (0.08, 0.06, 1.8), at=(1.05, 0.25, 1.5))
                m.box("Glow", (0.08, 0.06, 1.8), at=(-1.05, 0.25, 1.5))
            else:
                m.box("Window", (3.2, 0.06, 0.55), at=(0, 0.02, 2.2))
                m.box("Window", (3.2, 0.06, 0.4), at=(0, 0.02, 1.2))
                for wx in (-1.0, 0.3, 1.2):
                    if r.random() < 0.6:
                        m.box("Light", (0.5, 0.07, 0.3), at=(wx, 0.02, 2.2))
                m.box("Dark", (3.3, 0.12, 0.08), at=(0, 0.05, 2.5))
    # Roof equipment.
    for i in range(7):
        a = r.uniform(0, TAU)
        d = r.uniform(2.6, 3.8)
        if abs(math.cos(a) * d) < 2.2 and abs(math.sin(a) * d) < 2.2:
            continue
        m.box("Panel", (r.uniform(0.6, 1.2), r.uniform(0.6, 1.2), r.uniform(0.4, 0.8)), at=(math.cos(a) * d, math.sin(a) * d, 4.0), rot=(0, 0, a))
    for k in range(4):
        a = k * TAU / 4 + PI / 4
        with m.at((math.cos(a) * 3.3, math.sin(a) * 3.3, 3.8)):
            m.cyl("Dark", 0.45, 0.35, seg=16, base=True)
            m.cyl("Metal", 0.38, 0.05, at=(0, 0, 0.36), seg=16)
    # Control tower.
    m.lathe("Facade", [(2.3, 3.6), (2.3, 7.2), (2.4, 7.3)], seg=8, phase=PI / 8)
    m.lathe("Window", [(2.4, 7.3), (2.85, 8.35), (2.9, 8.4)], seg=8, phase=PI / 8, cap_bottom=False, cap_top=False)
    m.lathe("Dark", [(2.95, 8.35), (3.0, 8.55), (2.95, 8.6)], seg=8, phase=PI / 8, cap_top=False)
    m.lathe("Panel", [(2.95, 8.55), (2.7, 8.75), (1.2, 8.85)], seg=8, phase=PI / 8, cap_bottom=False)
    m.lathe("Team", [(2.36, 6.6), (2.36, 7.0), (2.32, 7.02)], seg=8, phase=PI / 8, cap_bottom=False, cap_top=False)
    for k in range(8):
        a = PI / 8 + (k + 0.5) * TAU / 8
        m.box("Light", (0.9, 0.06, 0.25), at=(math.cos(a) * 2.26, math.sin(a) * 2.26, 5.4), rot=(0, 0, a - HALF))
    # Comms mast, radar and antennas.
    m.lattice("Metal", 8.8, 12.0, 0.35, 0.12, 3, post=0.08, brace=0.035)
    m.cyl("Metal", 0.04, 1.2, at=(0, 0, 12.5), seg=8)
    m.sphere("Light", 0.12, at=(0, 0, 13.15), seg=12, rings=6)
    with m.at((1.6, -1.0, 8.85)):
        m.cyl("Dark", 0.18, 0.5, at=(0, 0, 0.25), seg=12)
        with m.at((0, 0, 0.65), (-0.75, 0, 0.6)):
            m.lathe("Metal", [(0.0, -0.05), (0.4, -0.02), (0.85, 0.12), (0.9, 0.2), (0.86, 0.2), (0.38, 0.04), (0.0, 0.0)], seg=32)
            m.cyl("Dark", 0.03, 0.6, at=(0, 0, 0.35), seg=8)
    for k in range(3):
        a = r.uniform(0, TAU)
        m.cyl("Metal", 0.02, r.uniform(1.0, 2.2), at=(math.cos(a) * 1.8, math.sin(a) * 1.8, 9.4), seg=6)
    # Side annexes with a landing pad.
    for s in m.sym():
        with m.at((5.6, -1.5, 0)):
            m.box("Facade", (2.2, 3.2, 2.0), at=(0, 0, 1.0), top=(0.95, 0.97))
            m.box("Dark", (2.3, 3.3, 0.15), at=(0, 0, 2.05))
            m.box("Team", (0.06, 3.0, 0.3), at=(1.1, 0, 1.6))
            m.greebles("Panel", (-0.9, -1.4, 2.12), (1, 0, 0), (0, 1, 0), 1.8, 2.8, 5, h=(0.15, 0.4), s=(0.3, 0.7))
    with m.at((-3.6, 4.2, 0.5)):
        m.cyl("Concrete", 1.6, 0.12, at=(0, 0, 0.06), seg=24)
        m.torus("Hazard", 1.45, 0.06, at=(0, 0, 0.12), seg=32, rseg=6, sz=0.4)
        for k in range(4):
            a = k * TAU / 4
            m.box("Light", (0.12, 0.12, 0.08), at=(math.cos(a) * 1.5, math.sin(a) * 1.5, 0.15))
    # Pipes along the wall.
    for k in (3, 5):
        a = PI / 8 + (k + 0.5) * TAU / 8
        base = V(math.cos(a) * 4.95, math.sin(a) * 4.95, 0.4)
        m.tube("Metal", [base, base + V(0, 0, 3.0), base + V(0, 0, 3.3) - base.normalized() * 0.4], 0.09, seg=10)


def outpost(m):
    """Landed colony module: octagonal lander on four legs, cargo pods, solar mast, ramp."""
    r = m.rng
    m.cyl("Concrete", 4.4, 0.15, at=(0, 0, 0.0), seg=40, base=False)
    m.torus("Hazard", 4.25, 0.07, at=(0, 0, 0.08), seg=48, rseg=6, sz=0.4)
    # Lander body.
    m.lathe("Dark", [(1.2, 0.7), (1.8, 0.9), (2.2, 1.3)], seg=8, phase=PI / 8, cap_top=False)
    m.lathe("Hull", [(2.2, 1.3), (2.35, 1.5), (2.35, 2.9), (1.8, 3.6), (0.9, 3.75)], seg=8, phase=PI / 8)
    m.lathe("Team", [(2.4, 2.3), (2.4, 2.6), (2.36, 2.62)], seg=8, phase=PI / 8, cap_bottom=False, cap_top=False)
    m.lathe("Dark", [(0.6, 3.7), (0.6, 4.0), (0.4, 4.1)], seg=16)
    for k in range(8):
        a = PI / 8 + (k + 0.5) * TAU / 8
        if k == 1:
            continue
        m.box("Light", (0.5, 0.06, 0.18), at=(math.cos(a) * 2.2, math.sin(a) * 2.2, 2.0), rot=(0, 0, a - HALF))
    # Legs.
    for k in m.around(4, phase=PI / 4):
        m.beam("Metal", (1.6, 0, 1.4), (3.1, 0, 0.15), 0.14)
        m.beam("Dark", (1.9, 0, 0.95), (2.6, 0, 0.5), 0.1)
        m.cyl("Dark", 0.35, 0.1, at=(3.15, 0, 0.12), seg=16)
    # Ramp to the front door.
    m.box("Dark", (1.0, 0.2, 1.5), at=(0, 2.2, 1.9))
    m.box("Glow", (0.9, 0.05, 0.08), at=(0, 2.31, 2.7))
    with m.at((0, 3.0, 0.75), (-0.6, 0, 0)):
        m.box("Deck", (1.0, 1.75, 0.08))
    # Cargo pods.
    for s in m.sym():
        with m.at((3.0, -1.6, 0), (0, 0, 0.5)):
            m.box("Hull", (1.0, 1.6, 1.0), at=(0, 0, 0.6))
            m.box("Team", (1.02, 0.25, 1.02), at=(0, 0.5, 0.6))
            m.box("Dark", (1.06, 0.08, 1.06), at=(0, -0.8, 0.6))
        m.tube("Metal", [(1.9, -0.8, 1.1), (2.4, -1.2, 0.9), (2.7, -1.5, 0.9)], 0.07, seg=8)
    # Solar mast and antenna.
    with m.at((-1.8, -2.6, 0.1)):
        m.cyl("Metal", 0.08, 3.4, at=(0, 0, 1.7), seg=10)
        for z in (2.6, 3.3):
            m.plate("Panel", [(-0.8, -0.35), (0.8, -0.35), (0.8, 0.35), (-0.8, 0.35)], 0.04, at=(0, 0, z), rot=(0.5, 0, 0))
    m.cyl("Metal", 0.02, 1.6, at=(0.5, 0.4, 4.8), seg=6)
    m.sphere("Light", 0.07, at=(0.5, 0.4, 5.62), seg=8, rings=4)


def extractor(m):
    """Mining derrick over a deposit: lattice tower, drill string, motor house, storage tank."""
    r = m.rng
    m.box("Concrete", (4.4, 4.4, 0.4), at=(0, 0, 0.1), top=(0.97, 0.97))
    m.box("Deck", (2.0, 2.0, 0.06), at=(0, 0, 0.33))
    for s in m.sym():
        m.box("Hazard", (0.12, 4.0, 0.03), at=(2.05, 0, 0.32))
    m.lattice("Metal", 0.3, 5.4, 1.15, 0.45, 4, post=0.14, brace=0.06)
    # Drill string and collars.
    m.cyl("Metal", 0.17, 5.6, at=(0, 0, 2.8), seg=16)
    for z in (0.6, 1.8, 3.0, 4.2):
        m.cyl("Dark", 0.27, 0.18, at=(0, 0, z), seg=16)
    m.lathe("Dark", [(0.6, 0.0), (0.5, 0.3), (0.2, 0.45)], at=(0, 0, 0.3), seg=20)
    # Motor house on top.
    with m.at((0, 0, 5.4)):
        m.box("Hull", (1.5, 1.5, 0.85), at=(0, 0, 0.42), top=(0.92, 0.92))
        m.box("Team", (1.52, 1.52, 0.18), at=(0, 0, 0.55))
        m.box("Dark", (1.2, 0.9, 0.3), at=(0, -0.2, 0.98))
        m.vents("Dark", at=(0, 0.76, 0.35), size=(0.8, 0.3), n=4)
        m.cyl("Metal", 0.1, 0.6, at=(0.45, 0.45, 1.2), seg=10)
        m.sphere("Light", 0.1, at=(-0.55, 0.55, 0.95), seg=10, rings=5)
    # Storage tank and pump.
    with m.at((1.45, -1.35, 0.35)):
        m.lathe("Hull", [(0.6, 0.0), (0.62, 0.05), (0.62, 1.6), (0.5, 1.85), (0.0, 1.95)], seg=24)
        m.torus("Team", 0.625, 0.04, at=(0, 0, 1.3), seg=32, rseg=6)
        m.box("Dark", (0.4, 0.4, 0.3), at=(-0.75, 0.3, 0.15))
    m.tube("Metal", [(0.2, -0.2, 0.7), (0.85, -0.95, 0.7), (1.05, -1.1, 0.7)], 0.07, seg=8)
    # Control booth.
    with m.at((-1.45, 1.4, 0.3)):
        m.box("Panel", (0.9, 0.9, 1.2), at=(0, 0, 0.6))
        m.box("Window", (0.7, 0.04, 0.4), at=(0, 0.46, 0.85))
        m.box("Dark", (1.0, 1.0, 0.08), at=(0, 0, 1.24))
        m.box("Glow", (0.06, 0.05, 0.7), at=(0.45, 0.46, 0.55))


def habitat(m):
    """Geodesic habitat dome on a concrete ring with an airlock tunnel."""
    import bmesh as _bm
    R = 3.3
    m.lathe("Concrete", [(3.75, -0.3), (3.75, 0.5), (3.55, 0.65), (3.3, 0.65)], seg=48)
    m.lathe("Team", [(3.78, 0.25), (3.78, 0.45), (3.76, 0.46)], seg=48, cap_bottom=False, cap_top=False)
    tmp = _bm.new()
    _bm.ops.create_icosphere(tmp, subdivisions=2, radius=R)
    zc = 0.55
    keep = lambda p: p.z > -0.05  # noqa: E731
    for e in tmp.edges:
        a, b = e.verts[0].co, e.verts[1].co
        if keep(a) and keep(b):
            m.beam("Metal", V(a.x, a.y, a.z + zc), V(b.x, b.y, b.z + zc), 0.07, roll_up=(a + b).normalized())
    for f in tmp.faces:
        pts = [v.co for v in f.verts]
        if all(keep(p) for p in pts):
            c = sum(pts, V(0, 0, 0)) / 3
            inner = [c + (p - c) * 0.97 - c.normalized() * 0.02 for p in pts]
            m.add("Window", [(p.x, p.y, p.z + zc) for p in inner], [(0, 1, 2)], None)
    tmp.free()
    # Inside: a few lit floors seen through the glass.
    m.cyl("Light", 2.4, 0.04, at=(0, 0, 1.6), seg=24)
    m.cyl("Concrete", 3.25, 0.1, at=(0, 0, 0.62), seg=40)
    # Airlock tunnel to the front.
    with m.at((0, 3.6, 0.0)):
        m.cyl_between("Hull", (0, -0.8, 1.05), (0, 1.0, 1.05), 0.75, seg=20)
        m.box("Dark", (1.7, 0.25, 1.9), at=(0, 1.05, 0.95))
        m.box("Glow", (0.9, 0.06, 1.3), at=(0, 1.19, 0.95))
        m.box("Concrete", (1.8, 1.6, 0.3), at=(0, 0.2, 0.15))
        m.box("Team", (1.52, 0.4, 0.06), at=(0, 0.3, 1.82))
        m.box("Light", (0.2, 0.06, 0.06), at=(0.6, 1.2, 1.75))


def factory(m):
    """Industrial hall with a saw-tooth roof, roller door, chimneys and external piping."""
    r = m.rng
    W, D, H = 8.0, 6.0, 3.4
    m.box("Concrete", (W + 0.5, D + 0.5, 0.5), at=(0, 0, 0.0))
    m.box("Sheet", (W, D, H), at=(0, 0, 0.25 + H / 2))
    m.box("Team", (W + 0.06, D + 0.06, 0.35), at=(0, 0, 0.25 + H - 0.45))
    m.box("Dark", (W + 0.2, D + 0.2, 0.15), at=(0, 0, 0.25 + H))
    top = 0.25 + H + 0.07
    # Saw-tooth roof with skylights facing back (-y), along x.
    n = 4
    for k in range(n):
        y0 = -D / 2 + k * D / n
        y1 = y0 + D / n
        with m.frame(cc_kit.basis(V(-W / 2 + 0.05, 0, top), (0, 1, 0), (0, 0, 1), (1, 0, 0))):
            m.plate("Sheet", [(y0, 0.0), (y1, 0.0), (y0 + 0.08, 1.1)], W - 0.1, bottom=0.0)
        m.box("Window", (W - 0.3, 0.05, 1.0), at=(0, y0 + 0.05, top + 0.53), rot=(-0.07, 0, 0))
    # Roller door (front, +y), hazard posts, light.
    with m.at((0, D / 2, 0.25)):
        m.box("Panel", (3.2, 0.12, 2.8), at=(0, 0.04, 1.4))
        for k in range(12):
            m.box("Dark", (3.1, 0.06, 0.04), at=(0, 0.11, 0.2 + k * 0.22))
        for s in m.sym():
            m.box("Hazard", (0.3, 0.3, 3.0), at=(1.8, 0.1, 1.5))
        m.box("Dark", (3.9, 0.4, 0.3), at=(0, 0.15, 3.05))
        m.box("Light", (1.0, 0.1, 0.08), at=(0, 0.36, 2.95))
        m.box("Concrete", (4.0, 1.4, 0.2), at=(0, 0.75, 0.0))
        m.box("Glow", (0.06, 0.06, 2.2), at=(2.0, 0.27, 1.3))
        m.box("Glow", (0.06, 0.06, 2.2), at=(-2.0, 0.27, 1.3))
    # Office annex with windows.
    with m.at((-W / 2 - 0.9, 1.0, 0.25)):
        m.box("Facade", (1.8, 2.6, 2.2), at=(0, 0, 1.1))
        m.box("Window", (0.05, 2.0, 0.6), at=(-0.91, 0, 1.4))
        m.box("Light", (0.06, 0.8, 0.4), at=(-0.92, -0.4, 1.4))
        m.box("Dark", (2.0, 2.8, 0.12), at=(0, 0, 2.26))
    # Chimneys.
    for k, (x, y) in enumerate(((2.4, -1.6), (1.1, -1.6))):
        h = 3.0 + k * 0.6
        with m.at((x, y, top)):
            m.cyl("Metal", 0.38, h, at=(0, 0, h / 2), seg=20)
            for z in (0.6, h * 0.5, h - 0.3):
                m.cyl("Dark", 0.43, 0.14, at=(0, 0, z), seg=20)
            m.lathe("Dark", [(0.38, h), (0.46, h + 0.1), (0.3, h + 0.12)], seg=20, cap_bottom=False)
    # Pipes along the right wall and roof units.
    for k in range(3):
        z = 1.0 + k * 0.35
        m.tube("Metal", [(W / 2 + 0.15, -D / 2 + 0.3, z), (W / 2 + 0.15, D / 2 - 1.2, z), (W / 2 + 0.6, D / 2 - 0.8, 0.25 + 0.0 * z)], 0.08, seg=10)
    m.box("Panel", (1.0, 1.2, 1.0), at=(W / 2 + 1.0, 1.6, 0.75))
    m.greebles("Panel", (-W / 2 + 0.3, -D / 2 + 0.3, top), (1, 0, 0), (0, 1, 0), 2.6, 1.2, 4, h=(0.2, 0.5), s=(0.4, 0.8))


def shipyard(m):
    """Launch pad with two lattice gantries, a bridge crane, a glowing launch ring and tanks."""
    r = m.rng
    W, D = 11.0, 8.0
    m.box("Concrete", (W, D, 0.7), at=(0, 0, 0.1), top=(0.98, 0.98))
    m.cyl("Deck", 3.2, 0.06, at=(0, 0, 0.47), seg=48)
    m.torus("Hazard", 3.35, 0.1, at=(0, 0, 0.47), seg=56, rseg=6, sz=0.4)
    for s in m.sym():
        m.box("Hazard", (0.25, D - 0.6, 0.04), at=(W / 2 - 0.4, 0, 0.46))
    for k in range(12):
        a = k / 12 * TAU
        m.box("Light", (0.15, 0.15, 0.06), at=(math.cos(a) * 3.7, math.sin(a) * 3.7, 0.48))
    # Gantry towers.
    for s in m.sym():
        with m.at((4.5, 0, 0.45)):
            m.lattice("Metal", 0.0, 7.6, 0.6, 0.5, 5, post=0.2, brace=0.08)
            m.box("Team", (1.2, 1.2, 0.5), at=(0, 0, 7.9))
            m.box("Dark", (1.4, 1.4, 0.15), at=(0, 0, 8.2))
            m.sphere("Light", 0.12, at=(0.5, 0.5, 8.35), seg=8, rings=4)
            # Cradle arm to the launch ring.
            m.beam("Dark", (-0.5, 0, 4.0), (-1.7, 0, 4.0), 0.3, 0.4)
    # Bridge between the towers.
    for y in (-0.45, 0.45):
        for z in (8.45, 9.15):
            m.beam("Metal", (-4.5, y, z), (4.5, y, z), 0.16)
    for k in range(10):
        x = -4.0 + k * 0.9
        m.beam("Metal", (x, -0.45, 8.45), (x + 0.9, -0.45, 9.15), 0.07)
        m.beam("Metal", (x, 0.45, 8.45), (x + 0.9, 0.45, 9.15), 0.07)
    m.box("Dark", (1.2, 1.4, 0.6), at=(-1.2, 0, 8.1))
    m.cyl("Metal", 0.02, 2.0, at=(-1.2, 0, 6.8), seg=6)
    m.box("Hazard", (0.3, 0.3, 0.25), at=(-1.2, 0, 5.7))
    # Launch ring.
    with m.at((0, 0, 4.45), (HALF, 0, 0)):
        m.torus("Dark", 2.8, 0.22, seg=56, rseg=10)
        m.torus("Glow", 2.55, 0.08, seg=56, rseg=8)
        for k in range(8):
            a = k / 8 * TAU
            m.box("Panel", (0.5, 0.6, 0.6), at=(math.cos(a) * 2.85, math.sin(a) * 2.85, 0), rot=(0, 0, a))
    # Fuel tanks and pipes at the back.
    for s in m.sym():
        with m.at((2.6, -3.0, 0.45)):
            m.cyl_between("Hull", (-0.8, 0, 0.75), (0.8, 0, 0.75), 0.6, seg=24)
            m.sphere("Hull", 0.6, at=(0.8, 0, 0.75), scale=(0.5, 1, 1), seg=24, rings=8)
            m.sphere("Hull", 0.6, at=(-0.8, 0, 0.75), scale=(0.5, 1, 1), seg=24, rings=8)
            m.torus("Team", 0.61, 0.04, at=(0, 0, 0.75), rot=(0, HALF, 0), seg=28, rseg=6)
            for x in (-0.6, 0.6):
                m.box("Dark", (0.15, 0.9, 0.5), at=(x, 0, 0.2))
        m.tube("Metal", [(1.8, -3.0, 1.2), (1.2, -2.0, 1.0), (0.6, -1.0, 0.55)], 0.08, seg=8)
    # Control booth on the left tower.
    with m.at((-3.5, 0.0, 3.0)):
        m.box("Panel", (1.0, 1.6, 1.1))
        m.box("Window", (0.05, 1.3, 0.45), at=(0.52, 0, 0.15))
        m.box("Light", (0.06, 0.5, 0.3), at=(0.53, 0.3, 0.15))


def turret(m):
    """Defence turret: armoured ring on a concrete base, wedge turret with twin guns, radar."""
    r = m.rng
    m.lathe("Concrete", [(2.2, -0.3), (2.2, 0.35), (1.9, 0.55)], seg=12, phase=PI / 12)
    m.lathe("Hazard", [(2.05, 0.43), (1.94, 0.56), (1.9, 0.56)], seg=12, phase=PI / 12, cap_bottom=False, cap_top=False)
    m.lathe("Dark", [(1.5, 0.5), (1.45, 1.0), (1.3, 1.1)], seg=24)
    m.torus("Metal", 1.3, 0.06, at=(0, 0, 1.12), seg=36, rseg=6)
    with m.at((0, 0, 1.55)):
        m.box("Hull", (1.7, 1.9, 0.75), top=(0.82, 0.85), front=0.72)
        m.box("Hull", (1.3, 0.6, 0.6), at=(0, -1.1, -0.02), top=(0.92, 0.9))
        m.box("Team", (1.74, 0.6, 0.07), at=(0, -0.3, 0.2), top=(0.9, 1.0))
        m.box("Dark", (0.95, 0.35, 0.5), at=(0, 0.92, 0.0), front=0.9)
        for s in m.sym():
            m.box("Panel", (0.06, 1.2, 0.45), at=(0.8, -0.2, -0.05), rot=(0, -0.25, 0))
            m.cyl_between("Dark", (0.25, 0.95, 0.02), (0.25, 1.6, 0.02), 0.11, seg=16)
            m.cyl_between("Metal", (0.25, 1.6, 0.02), (0.25, 2.9, 0.02), 0.075, 0.065, seg=16)
            m.box("Dark", (0.2, 0.28, 0.18), at=(0.25, 2.95, 0.02))
            m.cyl("Dark", 0.18, 0.4, at=(0.62, -1.15, 0.0), rot=(0, HALF, 0), seg=14)
        with m.at((-0.45, -0.7, 0.38)):
            m.cyl("Metal", 0.04, 0.5, at=(0, 0, 0.25), seg=8)
            with m.at((0, 0, 0.55), (-0.5, 0, 0.3)):
                m.lathe("Metal", [(0.0, -0.02), (0.15, -0.01), (0.32, 0.06), (0.34, 0.1), (0.31, 0.1), (0.14, 0.02), (0.0, 0.01)], seg=24)
        with m.at((0.4, 0.2, 0.36)):
            m.box("Dark", (0.3, 0.3, 0.22), at=(0, 0, 0.1))
            m.cyl("Glass", 0.09, 0.04, at=(0, 0.16, 0.12), rot=(-HALF, 0, 0), seg=16)
        m.sphere("Light", 0.05, at=(0.0, -1.3, 0.35), seg=8, rings=4)
        m.greebles("Dark", (-0.55, -1.35, 0.3), (1, 0, 0), (0, 1, 0), 1.1, 0.45, 4, h=(0.03, 0.08), s=(0.08, 0.2))


# =================================================================================== deposits
def deposit_metal(m):
    """Iron-rich boulders with exposed metallic ore."""
    r = m.rng
    for i in range(5):
        a = r.uniform(0, TAU)
        d = 0.0 if i == 0 else r.uniform(0.9, 1.6)
        size = 1.05 if i == 0 else r.uniform(0.45, 0.75)
        m.rock("OreRock", size, at=(math.cos(a) * d, math.sin(a) * d, 0.0),
               scale=(r.uniform(1.0, 1.4), r.uniform(0.9, 1.2), r.uniform(0.55, 0.8)), detail=3, rough=0.32, sharp=0.6,
               rot=(0, 0, r.uniform(0, TAU)))
    for i in range(9):
        a = r.uniform(0, TAU)
        d = r.uniform(0.3, 1.7)
        size = r.uniform(0.18, 0.38)
        z = max(0.0, 0.7 - d * 0.4) * r.uniform(0.4, 1.0)
        m.rock("Ore", size, at=(math.cos(a) * d, math.sin(a) * d, z), scale=(r.uniform(1.0, 1.6), 1.0, r.uniform(0.5, 0.9)),
               detail=2, rough=0.45, sharp=1.2, flat=-0.6, rot=(r.uniform(0, 1), r.uniform(0, 1), r.uniform(0, TAU)))


def deposit_crystal(m):
    """A cluster of glowing prism crystals breaking out of dark rock."""
    r = m.rng
    for i in range(4):
        a = r.uniform(0, TAU)
        d = 0.0 if i == 0 else r.uniform(0.7, 1.2)
        m.rock("Rock", 0.8 if i == 0 else r.uniform(0.35, 0.55), at=(math.cos(a) * d, math.sin(a) * d, 0),
               scale=(1.3, 1.1, 0.5), detail=3, rough=0.3, sharp=0.7, rot=(0, 0, r.uniform(0, TAU)))
    m.crystal("Crystal", (0, 0, 0.2), (0.05, 0.02, 1.0), 2.6, 0.32)
    for i in range(10):
        a = i / 10 * TAU + r.uniform(-0.2, 0.2)
        d = r.uniform(0.25, 0.9)
        tilt = r.uniform(0.35, 0.95)
        dirv = V(math.cos(a) * math.sin(tilt), math.sin(a) * math.sin(tilt), math.cos(tilt))
        m.crystal("Crystal", (math.cos(a) * d, math.sin(a) * d, 0.15), dirv, r.uniform(0.5, 1.6), r.uniform(0.08, 0.2))


def deposit_gas(m):
    """Fumarole: a rocky vent crater with glowing gas inside and a plume above."""
    r = m.rng
    for i in range(9):
        a = i / 9 * TAU + r.uniform(-0.15, 0.15)
        d = r.uniform(0.95, 1.15)
        m.rock("Rock", r.uniform(0.38, 0.55), at=(math.cos(a) * d, math.sin(a) * d, 0), scale=(1.2, 0.9, 0.85),
               detail=2, rough=0.3, sharp=0.6, rot=(0, 0, a))
    m.lathe("Rock", [(1.5, 0.0), (1.2, 0.25), (0.75, 0.32), (0.55, 0.1), (0.0, 0.05)], seg=20)
    m.cyl("Gas", 0.62, 0.04, at=(0, 0, 0.14), seg=24)
    for k in range(5):
        a = r.uniform(0, TAU)
        d = r.uniform(1.6, 2.2)
        m.rock("Rock", r.uniform(0.15, 0.3), at=(math.cos(a) * d, math.sin(a) * d, 0), scale=(1, 1, 0.6), detail=2, rough=0.3)
    # Plume: nested open cones, widening upwards (additive, no caps).
    m.lathe("Haze", [(0.5, 0.15), (0.65, 1.2), (0.95, 2.6), (1.1, 3.0)], seg=20, cap_bottom=False, cap_top=False)
    m.lathe("Haze", [(0.3, 0.15), (0.45, 1.5), (0.7, 3.3)], seg=16, cap_bottom=False, cap_top=False)


# ================================================================================== registry
# key -> (generator, kind, texture repeat scale, bevel width, AO map size). Keys are the
# `model` names in data/units.ron and data/buildings.ron; deposits are `deposit_<resource>`.
MODELS = {
    "drone": (drone, "unit", 0.5, 0.008, 512),
    "trooper": (trooper, "unit", 0.4, 0.006, 512),
    "tank": (tank, "unit", 0.6, 0.014, 512),
    "flak": (flak, "unit", 0.6, 0.012, 512),
    "fighter": (fighter, "ship", 0.6, 0.01, 512),
    "bomber": (bomber, "ship", 0.7, 0.014, 512),
    "colony_ship": (colony_ship, "ship", 0.8, 0.014, 1024),
    "command_center": (command_center, "building", 1.0, 0.03, 1024),
    "outpost": (outpost, "building", 1.0, 0.025, 1024),
    "extractor": (extractor, "building", 1.0, 0.02, 1024),
    "habitat": (habitat, "building", 1.0, 0.015, 1024),
    "factory": (factory, "building", 1.0, 0.03, 1024),
    "shipyard": (shipyard, "building", 1.0, 0.03, 1024),
    "turret": (turret, "building", 0.8, 0.02, 512),
    "deposit_metal": (deposit_metal, "deposit", 1.0, 0.01, 512),
    "deposit_crystal": (deposit_crystal, "deposit", 1.0, 0.01, 512),
    "deposit_gas": (deposit_gas, "deposit", 1.0, 0.01, 512),
}

# Research-tree arsenal (robots, recon, frigates, defences, sensors): tools/blender/models_arsenal.py.
import models_arsenal  # noqa: E402
MODELS.update(models_arsenal.MODELS_EXTRA)


def seed_for(key, base):
    return zlib.crc32(key.encode()) ^ base


def build(key, seed, out_dir, bake=True):
    fn, kind, tex_scale, bevel, ao = MODELS[key]
    cc_kit.reset_scene()
    m = Model(key, seed, kind)
    m.tex_scale = tex_scale
    m.bevel = bevel
    fn(m)
    raw = m.tris()
    # Ground units, buildings and deposits stand on a planet: bake the ground's occlusion too.
    ground = None if kind == "ship" else 0.0
    t0 = time.time()
    stats = m.finish(out_dir, ao_size=ao, ground=ground, bake=bake)
    print(f"== {key}: {stats['tris']} tris ({raw} before bevel), slots {', '.join(stats['slots'])}, {time.time() - t0:.1f}s")
    return stats


def main():
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opts = {"--preview": None, "--out": cc_kit.MODEL_DIR, "--seed": "0", "--variants": "0", "--size": "512"}
    flags, keys = set(), []
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
            keys.append(a)
            i += 1
    unknown = [k for k in keys if k not in MODELS]
    if unknown:
        sys.exit(f"unknown model(s): {', '.join(unknown)}; known: {', '.join(MODELS)}")
    keys = keys or list(MODELS)
    out_dir = opts["--out"]
    base = int(opts["--seed"])
    variants = int(opts["--variants"])
    bake = "--no-bake" not in flags
    files = []
    ships = []  # previewed without ground
    if variants:
        import random as _r
        rng = _r.Random(base)
        for key in keys:
            for v in range(variants):
                s = rng.randrange(1 << 30)
                fn, kind, ts, bv, ao = MODELS[key]
                cc_kit.reset_scene()
                m = Model(f"{key}_{v}", s, kind)
                m.tex_scale, m.bevel = ts, bv
                fn(m)
                m.finish(out_dir, ao_size=ao, ground=None if kind == "ship" else 0.0, bake=bake)
                files.append(os.path.join(out_dir, f"{key}_{v}.glb"))
                if kind == "ship":
                    ships.append(f"{key}_{v}")
    else:
        os.makedirs(out_dir, exist_ok=True)
        for key in keys:
            build(key, seed_for(key, base), out_dir, bake)
            files.append(os.path.join(out_dir, f"{key}.glb"))
            if MODELS[key][1] == "ship":
                ships.append(key)
        cc_kit.write_materials_ron(os.path.join(out_dir, "materials.ron"))
    if opts["--preview"]:
        import subprocess
        here = os.path.dirname(os.path.abspath(__file__))
        cmd = [sys.argv[0], "-b", "--python", os.path.join(here, "preview.py"), "--", opts["--preview"], *files,
               "--size", opts["--size"], "--ships", ",".join(ships)]
        if "--gpu" in flags:
            cmd.append("--gpu")
        subprocess.run(cmd, check=False)


main()
