"""Generators for the research-tree arsenal: robots, recon units, frigates, the battlecruiser,
defences, sensor buildings and the research lab.

Registered into `build_models.MODELS` (same tuple format), so they build like every other model:

    blender -b --python tools/blender/build_models.py -- sentinel radar_station
    blender -b --python tools/blender/build_models.py -- --preview /tmp/a.png sentinel juggernaut

Conventions as in `cc_kit.py`: Blender coordinates (x right, y forward, z up, metres), origin at
the ground contact point (ground units, buildings) or the centre (ships). Sizes follow the
simulation radii in data/units.ron / data/buildings.ron (units are drawn 1.4x larger by the game).
"""
import math

from cc_kit import TAU, V, lerp

PI = math.pi
HALF = PI / 2


def _pad(m, r, h=0.4, seg=8, slot="Concrete"):
    m.lathe(slot, [(r, -0.3), (r, h * 0.6), (r - 0.15, h)], seg=seg, phase=PI / seg)


def _hazard_ring(m, r, z, seg=12):
    m.lathe("Hazard", [(r, z - 0.12), (r - 0.1, z + 0.01), (r - 0.14, z + 0.01)], seg=seg, phase=PI / seg, cap_bottom=False, cap_top=False)


def _dish(m, slot, r, depth, at=(0, 0, 0), rot=(0, 0, 0), seg=28):
    """Parabolic dish opening towards local +z, with a thickness."""
    prof = []
    for k in range(7):
        t = k / 6
        prof.append((r * t, depth * t * t))
    prof += [(r * 1.02, depth * 1.02), (r * 0.98, depth * 1.06)]
    for k in range(6, -1, -1):
        t = k / 6
        prof.append((r * t * 0.97, depth * t * t + depth * 0.06 + 0.01))
    with m.at(at, rot):
        m.lathe(slot, prof, seg=seg, cap_bottom=False, cap_top=False)


def _antenna(m, base, h, slot="Metal", tip="Light"):
    m.cyl(slot, 0.012, h, at=V(base) + V(0, 0, h / 2), seg=6)
    m.sphere(tip, 0.025, at=V(base) + V(0, 0, h), seg=8, rings=4)


def _infantry(m, team_pads=True):
    """Power-armoured soldier body (legs, torso, helmet) without a weapon; returns hand points."""
    for s in m.sym():
        hip, knee, ankle = V(0.12, -0.02, 0.8), V(0.14, 0.05, 0.45), V(0.13, -0.01, 0.1)
        m.box("Dark", (0.13, 0.28, 0.08), at=(0.13, 0.04, 0.045), top=(0.9, 0.85))
        m.beam("Dark", ankle, knee, 0.11, 0.12)
        m.beam("Hull", ankle + V(0, 0.03, 0.05), knee + V(0, 0.03, -0.03), 0.14, 0.1)
        m.box("Team", (0.13, 0.08, 0.12), at=knee + V(0, 0.08, 0.0), top=(0.85, 0.8))
        m.beam("Dark", knee, hip, 0.14, 0.15)
        m.beam("Hull", knee + V(0, 0.02, 0.06), hip + V(0, 0.02, -0.04), 0.17, 0.13)
        if team_pads:
            m.box("Team", (0.17, 0.24, 0.12), at=(0.29, -0.02, 1.29), top=(0.75, 0.8), rot=(0, -0.3, 0))
    m.box("Dark", (0.32, 0.2, 0.16), at=(0, -0.02, 0.85), top=(1.05, 1.0))
    m.box("Hull", (0.42, 0.27, 0.34), at=(0, 0.0, 1.14), top=(1.12, 1.05), shift=(0, 0.01))
    m.box("Panel", (0.3, 0.04, 0.16), at=(0, 0.14, 1.17), top=(1.05, 1.0))
    m.cyl("Dark", 0.06, 0.08, at=(0, 0.0, 1.35), seg=12)
    m.sphere("Hull", 0.125, at=(0, 0.02, 1.47), scale=(1.0, 1.15, 1.05), seg=18, rings=10)
    m.box("Glow", (0.17, 0.05, 0.035), at=(0, 0.14, 1.48), top=(0.9, 0.7))
    m.box("Dark", (0.2, 0.06, 0.05), at=(0, 0.13, 1.42), front=0.9)


# =============================================================================== ground units
def lancer(m):
    """Missile infantry: armoured soldier with a four-tube launcher on the right shoulder."""
    r = m.rng
    with m.at(scale=r.uniform(0.97, 1.03)):
        _infantry(m)
        # Arms holding the targeting unit.
        for sh, el, ha in (((0.27, -0.02, 1.22), (0.3, 0.12, 1.0), (0.15, 0.3, 1.08)),
                           ((-0.27, -0.02, 1.22), (-0.27, 0.15, 1.0), (-0.05, 0.32, 1.1))):
            sh, el, ha = V(sh), V(el), V(ha)
            m.sphere("Dark", 0.07, at=sh, seg=12, rings=6)
            m.beam("Dark", sh, el, 0.1)
            m.beam("Hull", el, ha, 0.11, 0.1)
            m.box("Dark", (0.07, 0.09, 0.08), at=ha)
        m.box("Dark", (0.14, 0.16, 0.1), at=(0.05, 0.34, 1.12))
        m.cyl("Glass", 0.035, 0.03, at=(0.05, 0.43, 1.14), rot=(-HALF, 0, 0), seg=12)
        # Backpack with the launcher pod on a swing arm.
        m.box("Dark", (0.36, 0.2, 0.4), at=(0, -0.23, 1.12), top=(0.92, 0.9))
        m.box("Dark", (0.16, 0.22, 0.14), at=(0.3, -0.1, 1.36))
        with m.at((0.3, -0.04, 1.5), (0.12, 0, 0)):
            m.box("Hull", (0.3, 0.75, 0.3), top=(0.95, 0.95))
            m.box("Team", (0.31, 0.12, 0.31), at=(0, -0.2, 0))
            for bx in (-0.07, 0.07):
                for bz in (-0.07, 0.07):
                    m.cyl("Dark", 0.055, 0.04, at=(bx, 0.37, bz), rot=(-HALF, 0, 0), seg=12)
                    m.cyl("Hazard", 0.04, 0.02, at=(bx, 0.39, bz), rot=(-HALF, 0, 0), seg=10)
            m.box("Dark", (0.2, 0.08, 0.2), at=(0, -0.4, 0))
        m.vents("Dark", at=(0, -0.335, 1.16), size=(0.2, 0.12), n=3, rot=(0, 0, PI), depth=0.02)
        m.cyl("Metal", 0.005, 0.45, at=(-0.12, -0.27, 1.55), seg=5)


def scout_rover(m):
    """Six-wheeled recon buggy: open roll cage, sensor mast with a seismic probe launcher."""
    r = m.rng
    L, W = r.uniform(2.1, 2.3), 1.3
    # Chassis and belly plate.
    m.box("Dark", (W * 0.7, L * 0.95, 0.18), at=(0, 0, 0.42))
    m.box("Hull", (W * 0.82, L * 0.7, 0.32), at=(0, 0.05, 0.66), top=(0.9, 0.82), front=0.85)
    m.box("Hull", (W * 0.75, L * 0.25, 0.22), at=(0, L * 0.38, 0.6), top=(0.9, 0.7), front=0.8)
    m.box("Team", (W * 0.83, 0.22, 0.05), at=(0, -0.15, 0.8), top=(0.95, 1.0))
    # Wheels on swing arms.
    for s in m.sym():
        for y in (-L * 0.36, 0.0, L * 0.36):
            with m.at((W / 2 + 0.05, y, 0.33), (0, HALF, 0)):
                m.lathe("Rubber", [(0.2, -0.13), (0.31, -0.11), (0.33, 0.0), (0.31, 0.11), (0.2, 0.13)], seg=20)
                m.cyl("Metal", 0.17, 0.27, seg=14)
                m.cyl("Dark", 0.07, 0.3, seg=8)
            m.beam("Dark", (W * 0.3, y, 0.45), (W / 2 - 0.06, y, 0.35), 0.08)
        # Roll cage.
        m.tube("Metal", [(W * 0.36, -L * 0.3, 0.8), (W * 0.3, -L * 0.1, 1.25), (W * 0.3, L * 0.12, 1.25), (W * 0.36, L * 0.3, 0.82)], 0.035, seg=8)
        m.box("Light", (0.12, 0.03, 0.06), at=(W * 0.3, L * 0.5 + 0.01, 0.62))
    m.tube("Metal", [(-W * 0.3, -L * 0.1, 1.25), (W * 0.3, -L * 0.1, 1.25)], 0.035, seg=8)
    m.tube("Metal", [(-W * 0.3, L * 0.12, 1.25), (W * 0.3, L * 0.12, 1.25)], 0.035, seg=8)
    # Seats and windscreen.
    m.box("Dark", (0.3, 0.3, 0.3), at=(-0.18, -0.05, 0.92), top=(0.9, 0.7))
    m.box("Glass", (W * 0.6, 0.04, 0.26), at=(0, L * 0.22, 0.98), rot=(-0.45, 0, 0))
    # Sensor mast: rotating optics, seismic dart launcher.
    with m.at((0.25, -L * 0.28, 0.8)):
        m.cyl("Dark", 0.05, 0.9, at=(0, 0, 0.45), seg=10)
        with m.at((0, 0, 0.98)):
            m.box("Hull", (0.26, 0.2, 0.16))
            m.cyl("Glass", 0.06, 0.04, at=(0.06, 0.11, 0), rot=(-HALF, 0, 0), seg=14)
            m.cyl("Glass", 0.04, 0.04, at=(-0.07, 0.11, 0), rot=(-HALF, 0, 0), seg=12)
            m.sphere("Light", 0.02, at=(0, 0, 0.1), seg=8, rings=4)
    with m.at((-0.3, -L * 0.36, 0.78), (0.4, 0, 0)):
        m.box("Dark", (0.28, 0.4, 0.2))
        for bx in (-0.07, 0.07):
            m.cyl("Hazard", 0.05, 0.04, at=(bx, 0.21, 0), rot=(-HALF, 0, 0), seg=10)
    m.cyl_between("Dark", (-0.1, L * 0.45, 0.72), (-0.1, L * 0.62, 0.74), 0.035, seg=8)
    _antenna(m, (-0.5, -L * 0.42, 0.8), 0.8)


def artillery(m):
    """Tracked siege mortar: low hull on tracks, a heavy breech and a long tube pitched high."""
    r = m.rng
    L, W = r.uniform(2.8, 3.0), 1.9
    for s in m.sym():
        with m.at((W / 2 - 0.22, 0, 0)):
            # Track: a rounded loop with road wheels.
            m.loft("Rubber", [(-L / 2, 0.0, 0.0, 0.0, 0.36, 2.0), (-L / 2 + 0.05, 0.24, 0.32, 0.32, 0.36, 4.0),
                              (L / 2 - 0.05, 0.24, 0.32, 0.32, 0.36, 4.0), (L / 2, 0.0, 0.0, 0.0, 0.36, 2.0)], seg=16)
            for k in range(5):
                y = lerp(-L * 0.38, L * 0.38, k / 4)
                m.cyl("Metal", 0.2, 0.06, at=(0.13, y, 0.32), rot=(0, HALF, 0), seg=16)
            m.box("Panel", (0.06, L * 0.85, 0.3), at=(0.15, 0, 0.62))
            m.box("Team", (0.02, L * 0.4, 0.08), at=(0.185, -0.2, 0.66))
    m.box("Hull", (W * 0.7, L * 0.9, 0.5), at=(0, 0, 0.82), top=(0.9, 0.85), front=0.85)
    m.box("Dark", (W * 0.5, 0.5, 0.3), at=(0, -L * 0.42, 0.95))
    m.vents("Dark", at=(0, -L * 0.45 - 0.01, 0.82), size=(0.6, 0.2), n=4, rot=(0, 0, PI))
    m.greebles("Hull", (-W * 0.3, -L * 0.3, 1.07), (1, 0, 0), (0, 1, 0), W * 0.6, 0.6, 5, h=(0.03, 0.08), s=(0.08, 0.2))
    # Mortar on a turntable, pitched at 55 degrees.
    m.cyl("Dark", 0.55, 0.15, at=(0, 0.25, 1.12), seg=24)
    with m.at((0, 0.25, 1.25)):
        for s in m.sym():
            m.box("Hull", (0.12, 0.7, 0.6), at=(0.4, 0, 0.25), top=(1.0, 0.6))
        with m.at((0, 0, 0.35), (0.95, 0, 0)):
            m.box("Hull", (0.55, 0.8, 0.55), at=(0, -0.2, 0), top=(0.9, 0.9))
            m.box("Team", (0.56, 0.15, 0.56), at=(0, -0.45, 0))
            m.cyl_between("Metal", (0, 0.2, 0), (0, 2.1, 0), 0.17, 0.15, seg=20)
            m.lathe("Dark", [(0.22, 0.0), (0.22, 0.35), (0.19, 0.4)], at=(0, 1.8, 0), rot=(-HALF, 0, 0), seg=20)
            for k in range(3):
                m.torus("Dark", 0.18, 0.025, at=(0, 0.55 + k * 0.35, 0), rot=(HALF, 0, 0), seg=20, rseg=6)
            for s in m.sym():
                m.cyl_between("Metal", (0.2, -0.3, -0.15), (0.2, 0.8, -0.15), 0.05, seg=8)
    m.box("Light", (0.1, 0.03, 0.06), at=(0.5, L * 0.45 + 0.02, 0.85))
    m.box("Light", (0.1, 0.03, 0.06), at=(-0.5, L * 0.45 + 0.02, 0.85))


def sentinel(m):
    """Bipedal combat robot: digitigrade legs, armoured torso, sensor head, twin laser arms."""
    r = m.rng
    hz = r.uniform(1.25, 1.35)
    for s in m.sym():
        hip, knee, ankle, toe = V(0.32, -0.05, hz), V(0.38, 0.28, hz - 0.55), V(0.36, -0.15, 0.25), V(0.36, 0.25, 0.05)
        m.cyl("Dark", 0.14, 0.2, at=hip, rot=(0, HALF, 0), seg=16)
        m.beam("Hull", hip, knee, 0.2, 0.22)
        m.box("Team", (0.04, 0.3, 0.12), at=(hip + knee) / 2 + V(0.11, 0, 0), rot=(math.atan2(0.55, 0.33) - HALF, 0, 0))
        m.cyl("Dark", 0.11, 0.22, at=knee, rot=(0, HALF, 0), seg=14)
        m.beam("Dark", knee, ankle, 0.13, 0.15)
        m.cyl_between("Metal", knee + V(0, -0.1, 0.0), ankle + V(0, -0.1, 0.12), 0.03, seg=8)
        m.sphere("Dark", 0.09, at=ankle, seg=12, rings=6)
        m.beam("Hull", ankle, toe, 0.18, 0.08)
        m.box("Dark", (0.2, 0.42, 0.08), at=(0.36, 0.08, 0.04), top=(0.9, 0.85))
    # Pelvis and torso.
    m.box("Dark", (0.5, 0.35, 0.25), at=(0, -0.05, hz + 0.02))
    with m.at((0, 0, hz + 0.45)):
        m.box("Hull", (0.75, 0.6, 0.55), top=(1.1, 0.9), front=0.88)
        m.box("Panel", (0.5, 0.06, 0.3), at=(0, 0.3, 0.02), rot=(-0.15, 0, 0))
        m.box("Team", (0.77, 0.15, 0.08), at=(0, -0.12, 0.24), top=(0.95, 1.0))
        m.vents("Dark", at=(0, -0.31, 0.0), size=(0.4, 0.25), n=4, rot=(0, 0, PI))
        # Head.
        m.cyl("Dark", 0.08, 0.1, at=(0, 0.05, 0.32), seg=12)
        with m.at((0, 0.08, 0.45)):
            m.box("Hull", (0.32, 0.36, 0.2), top=(0.85, 0.85), front=0.8)
            m.box("Glow", (0.24, 0.03, 0.05), at=(0, 0.17, 0.0))
            m.cyl("Glass", 0.04, 0.03, at=(0.1, 0.18, 0.06), rot=(-HALF, 0, 0), seg=10)
            _antenna(m, (-0.1, -0.12, 0.1), 0.35)
        # Arms: laser cannons.
        for s in m.sym():
            m.sphere("Dark", 0.14, at=(0.48, 0.0, 0.15), seg=14, rings=7)
            m.box("Team", (0.2, 0.3, 0.12), at=(0.5, -0.02, 0.3), top=(0.8, 0.85))
            with m.at((0.55, 0.2, -0.05)):
                m.box("Hull", (0.18, 0.55, 0.2), top=(0.9, 0.9))
                m.cyl_between("Dark", (0, 0.25, 0.0), (0, 0.65, 0.0), 0.07, seg=14)
                m.cyl_between("Metal", (0, 0.65, 0.0), (0, 0.85, 0.0), 0.045, 0.035, seg=12)
                m.cyl("Laser", 0.03, 0.02, at=(0, 0.86, 0.0), rot=(-HALF, 0, 0), seg=10)
                for k in range(2):
                    m.torus("Laser", 0.075, 0.01, at=(0, 0.35 + k * 0.15, 0), rot=(HALF, 0, 0), seg=16, rseg=5)


def repair_bot(m):
    """Mender: a hovering repair sphere with two manipulator arms and a nanite emitter ring."""
    r = m.rng
    z = 0.95
    m.sphere("Hull", 0.42, at=(0, 0, z), scale=(1.0, 1.05, 0.85), seg=24, rings=12)
    m.torus("Team", 0.43, 0.04, at=(0, 0, z + 0.05), seg=28, rseg=6)
    m.lathe("Dark", [(0.36, -0.06), (0.47, 0.0), (0.47, 0.05), (0.36, 0.09)], at=(0, 0, z - 0.12), seg=28)
    # Nanite emitter ring and the green repair glow underneath.
    m.torus("Gas", 0.33, 0.035, at=(0, 0, z - 0.4), seg=24, rseg=6)
    m.cyl("Dark", 0.18, 0.16, at=(0, 0, z - 0.32), seg=16)
    m.lathe("Haze", [(0.25, z - 0.45), (0.38, z - 0.8), (0.5, 0.02)], seg=16, cap_bottom=False, cap_top=False)
    # Front sensor eye.
    m.cyl("Dark", 0.1, 0.08, at=(0, 0.38, z + 0.05), rot=(-HALF, 0, 0), seg=16)
    m.cyl("Glass", 0.07, 0.02, at=(0, 0.43, z + 0.05), rot=(-HALF, 0, 0), seg=14)
    m.sphere("Light", 0.02, at=(0, 0.44, z + 0.05), seg=8, rings=4)
    for s in m.sym():
        sh = V(0.38, 0.1, z - 0.05)
        el = V(0.55, 0.35, z - 0.25)
        wr = V(0.35, 0.6, z - 0.3)
        m.sphere("Dark", 0.06, at=sh, seg=10, rings=5)
        m.beam("Metal", sh, el, 0.05)
        m.beam("Metal", el, wr, 0.04)
        m.cyl_between("Dark", wr, wr + V(-0.05, 0.12, -0.02), 0.03, seg=8)
        m.sphere("Gas", 0.022, at=wr + V(-0.06, 0.14, -0.02), seg=8, rings=4)
        # Hover fans.
        with m.at((0.32, -0.25, z - 0.2), (0.3, 0.3, 0)):
            m.lathe("Dark", [(0.14, -0.04), (0.15, 0.04), (0.12, 0.05)], seg=16)
            m.cyl("Engine", 0.11, 0.01, at=(0, 0, -0.04), seg=14)
    m.greebles("Panel", (-0.15, -0.3, z + 0.3), (1, 0, 0), (0, 1, 0), 0.3, 0.25, 4, h=(0.01, 0.03), s=(0.03, 0.08))
    _antenna(m, (0.12, -0.18, z + 0.3), 0.3)


def infiltrator(m):
    """Wraith infiltrator: a low four-legged stalker robot with a single eye and a laser."""
    r = m.rng
    z = 0.62
    m.loft("Dark", [
        (-0.5, 0.12, 0.08, 0.07, z, 3.0),
        (-0.35, 0.22, 0.13, 0.1, z, 3.5),
        (0.2, 0.2, 0.14, 0.1, z + 0.02, 3.5),
        (0.42, 0.12, 0.09, 0.07, z + 0.03, 3.0),
        (0.5, 0.0, 0.0, 0.0, z + 0.03, 2.0),
    ], seg=20)
    m.box("Team", (0.06, 0.6, 0.02), at=(0, -0.1, z + 0.14))
    # Head with the eye.
    with m.at((0, 0.42, z + 0.12)):
        m.box("Dark", (0.2, 0.22, 0.12), front=0.7)
        m.cyl("Glow", 0.035, 0.02, at=(0, 0.11, 0.0), rot=(-HALF, 0, 0), seg=12)
    # Laser under the chin.
    m.cyl_between("Metal", (0, 0.35, z - 0.05), (0, 0.72, z - 0.06), 0.025, seg=8)
    m.sphere("Laser", 0.018, at=(0, 0.73, z - 0.06), seg=8, rings=4)
    # Four spider legs.
    for s in m.sym():
        for y, a in ((0.25, 0.5), (-0.3, -0.5)):
            hip = V(0.16, y, z)
            knee = hip + V(0.35, 0.25 * math.copysign(1, a), 0.22)
            foot = V(0.62, y + 0.4 * math.copysign(1, a), 0.0)
            m.sphere("Metal", 0.05, at=hip, seg=10, rings=5)
            m.beam("Dark", hip, knee, 0.06, 0.07)
            m.beam("Dark", knee, foot, 0.04, 0.05)
            m.sphere("Dark", 0.04, at=knee, seg=8, rings=4)
            m.cyl("Dark", 0.035, 0.03, at=foot, seg=8)
    # Cloak emitter fins.
    for k in range(3):
        m.box("Glass", (0.3, 0.05, 0.12), at=(0, -0.25 + k * 0.18, z + 0.18), top=(0.6, 0.7))
    m.greebles("Metal", (-0.12, -0.45, z + 0.08), (1, 0, 0), (0, 1, 0), 0.24, 0.2, 3, h=(0.01, 0.02), s=(0.02, 0.05))


def juggernaut(m):
    """Assault mech: four-metre walker, heavy armour, shoulder shield emitters, plasma cannon."""
    r = m.rng
    hz = r.uniform(2.0, 2.15)
    for s in m.sym():
        hip, knee, ankle = V(0.75, 0.0, hz), V(0.9, 0.55, hz - 0.95), V(0.85, -0.05, 0.42)
        m.cyl("Dark", 0.32, 0.38, at=hip, rot=(0, HALF, 0), seg=20)
        m.beam("Hull", hip, knee, 0.42, 0.46)
        m.box("Team", (0.06, 0.55, 0.26), at=(hip + knee) / 2 + V(0.23, 0, 0), rot=(math.atan2(0.95, 0.55) - HALF, 0, 0))
        m.cyl("Dark", 0.26, 0.46, at=knee, rot=(0, HALF, 0), seg=18)
        m.box("Panel", (0.5, 0.3, 0.5), at=knee + V(0, 0.22, 0.05), top=(0.85, 0.7))
        m.beam("Dark", knee, ankle, 0.3, 0.34)
        m.cyl_between("Metal", hip + V(0, -0.3, -0.2), ankle + V(0, -0.2, 0.4), 0.07, seg=10)
        m.box("Dark", (0.7, 1.1, 0.3), at=(0.85, 0.15, 0.15), top=(0.85, 0.8))
        m.box("Hull", (0.6, 0.4, 0.22), at=(0.85, 0.55, 0.3), top=(0.8, 0.6), front=0.7)
        m.sphere("Dark", 0.22, at=ankle, seg=14, rings=7)
    m.box("Dark", (1.2, 0.7, 0.5), at=(0, -0.05, hz + 0.05))
    with m.at((0, 0, hz + 0.95)):
        # Torso: a wide armoured wedge.
        m.box("Hull", (1.8, 1.5, 1.2), top=(0.95, 0.8), front=0.8)
        m.box("Hull", (1.4, 0.6, 0.7), at=(0, 0.75, -0.25), top=(0.9, 0.7), front=0.75)
        m.box("Team", (1.82, 0.35, 0.1), at=(0, -0.2, 0.52), top=(0.95, 1.0))
        m.box("Glass", (0.7, 0.06, 0.18), at=(0, 0.72, 0.28), rot=(-0.4, 0, 0))
        m.vents("Dark", at=(0, -0.76, 0.0), size=(1.0, 0.5), n=5, rot=(0, 0, PI))
        m.greebles("Panel", (-0.7, -0.6, 0.6), (1, 0, 0), (0, 1, 0), 1.4, 0.9, 8, h=(0.03, 0.1), s=(0.1, 0.3))
        # Shield emitter pylons.
        for s in m.sym():
            m.cyl("Dark", 0.1, 0.6, at=(0.65, -0.45, 0.85), seg=10)
            m.sphere("Ion", 0.12, at=(0.65, -0.45, 1.2), seg=12, rings=6)
            m.torus("Metal", 0.16, 0.02, at=(0.65, -0.45, 1.2), seg=16, rseg=5)
        # Right arm: plasma cannon. Left arm: armoured fist / missile pod.
        with m.at((1.15, 0.1, 0.1)):
            m.sphere("Dark", 0.32, seg=16, rings=8)
            m.box("Team", (0.45, 0.7, 0.25), at=(0.05, 0, 0.35), top=(0.8, 0.85))
            with m.at((0.15, 0.4, -0.35)):
                m.box("Hull", (0.45, 1.0, 0.45), top=(0.9, 0.9))
                m.cyl_between("Dark", (0, 0.5, 0), (0, 1.6, 0), 0.18, 0.16, seg=20)
                for k in range(4):
                    m.torus("Plasma", 0.2, 0.025, at=(0, 0.7 + k * 0.22, 0), rot=(HALF, 0, 0), seg=20, rseg=6)
                m.lathe("Metal", [(0.24, 0.0), (0.26, 0.2), (0.2, 0.26), (0.12, 0.26), (0.12, 0.05)], at=(0, 1.6, 0), rot=(-HALF, 0, 0), seg=20, cap_top=False)
                m.cyl("Plasma", 0.11, 0.02, at=(0, 1.75, 0), rot=(-HALF, 0, 0), seg=16)
        with m.at((-1.15, 0.1, 0.1)):
            m.sphere("Dark", 0.32, seg=16, rings=8)
            m.box("Team", (0.45, 0.7, 0.25), at=(-0.05, 0, 0.35), top=(0.8, 0.85))
            with m.at((-0.15, 0.3, -0.35)):
                m.box("Hull", (0.5, 0.9, 0.5), top=(0.9, 0.9))
                for bx in (-0.12, 0.12):
                    for bz in (-0.12, 0.12):
                        m.cyl("Hazard", 0.07, 0.04, at=(bx, 0.46, bz), rot=(-HALF, 0, 0), seg=10)
        # Sensor head.
        with m.at((0, 0.45, 0.72)):
            m.box("Dark", (0.5, 0.45, 0.3), front=0.8)
            m.box("Glow", (0.36, 0.04, 0.06), at=(0, 0.22, 0.02))
        _antenna(m, (-0.5, -0.5, 0.6), 0.7)
        _antenna(m, (0.3, -0.6, 0.6), 0.45)


# ====================================================================================== ships
def probe(m):
    """Recon probe: a small sensor ball with an array dish, long antennae and a tiny ion engine."""
    r = m.rng
    m.sphere("Hull", 0.42, scale=(1.0, 1.2, 1.0), seg=22, rings=11)
    m.torus("Team", 0.42, 0.035, rot=(0, HALF, 0), seg=26, rseg=6)
    _dish(m, "Metal", 0.38, 0.12, at=(0, 0.42, 0.0), rot=(-HALF, 0, 0))
    m.cyl_between("Dark", (0, 0.45, 0), (0, 0.72, 0), 0.02, seg=6)
    m.sphere("Light", 0.03, at=(0, 0.74, 0), seg=8, rings=4)
    for i in m.around(3, phase=r.uniform(0, TAU)):
        m.cyl_between("Metal", (0.3, 0.0, 0.0), (1.1, -0.25, 0.0), 0.012, seg=6)
        m.sphere("Light", 0.02, at=(1.1, -0.25, 0), seg=6, rings=3)
    with m.at((0, -0.52, 0), (HALF, 0, 0)):
        m.lathe("Metal", [(0.12, -0.02), (0.15, 0.1), (0.1, 0.18), (0.08, 0.18), (0.08, 0.04)], seg=18, cap_bottom=True, cap_top=False)
        m.cyl("Engine", 0.08, 0.02, at=(0, 0, 0.06), seg=14)
    for s in m.sym():
        m.plate("Panel", [(0.4, -0.25), (0.95, -0.25), (0.95, 0.1), (0.4, 0.1)], 0.02, at=(0, 0, -0.05))
        m.box("Dark", (0.55, 0.02, 0.025), at=(0.68, -0.08, -0.05))


def _ship_engines(m, xs, y, z, r):
    for x in xs:
        with m.at((x, y, z), (HALF, 0, 0)):
            m.lathe("Metal", [(r, -0.05), (r * 1.12, r * 0.6), (r * 0.95, r * 1.3), (r * 0.7, r * 1.3), (r * 0.7, r * 0.3)], seg=22, cap_bottom=True, cap_top=False)
            m.cyl("Engine", r * 0.7, 0.02, at=(0, 0, r * 0.4), seg=18)
            m.torus("Glow", r * 1.05, 0.02, at=(0, 0, 0.0), seg=22, rseg=5)


def missile_frigate(m):
    """Missile frigate: wedge hull, a deck of vertical launch cells, bridge, two big engines."""
    r = m.rng
    L = r.uniform(5.6, 6.0)
    m.loft("Hull", [
        (-L / 2, 0.85, 0.55, 0.5, 0.0, 5.0),
        (-L * 0.3, 1.0, 0.65, 0.55, 0.0, 5.0),
        (L * 0.15, 0.85, 0.55, 0.45, 0.0, 4.5),
        (L * 0.42, 0.4, 0.3, 0.25, -0.05, 3.5),
        (L / 2, 0.08, 0.08, 0.06, -0.08, 2.5),
    ], seg=24)
    m.box("Dark", (1.4, L * 0.55, 0.12), at=(0, -L * 0.08, -0.55))
    # Launch cells: a 3 x 4 grid of hatches on the dorsal deck.
    m.box("Deck", (1.3, 1.8, 0.1), at=(0, L * 0.05, 0.62))
    for ix in range(3):
        for iy in range(4):
            x, y = -0.4 + ix * 0.4, L * 0.05 - 0.66 + iy * 0.44
            m.box("Dark", (0.32, 0.36, 0.05), at=(x, y, 0.69))
            m.box("Hazard", (0.22, 0.04, 0.02), at=(x, y + 0.13, 0.72))
    m.box("Team", (2.02, 0.4, 0.1), at=(0, -L * 0.28, 0.4), top=(0.95, 1.0))
    # Bridge tower.
    with m.at((0, -L * 0.25, 0.65)):
        m.box("Hull", (0.8, 0.9, 0.5), at=(0, 0, 0.25), top=(0.85, 0.75), front=0.85)
        m.box("Window", (0.6, 0.05, 0.12), at=(0, 0.38, 0.38), rot=(-0.3, 0, 0))
        _antenna(m, (0.2, -0.2, 0.5), 0.6)
        _dish(m, "Metal", 0.2, 0.06, at=(-0.22, -0.1, 0.55), rot=(0.3, 0, 0))
    for s in m.sym():
        # Side missile pods.
        with m.at((1.05, -L * 0.05, 0.0)):
            m.box("Panel", (0.35, L * 0.45, 0.5), top=(0.85, 0.95))
            for k in range(3):
                m.cyl("Hazard", 0.08, 0.03, at=(0.0, L * 0.22 + 0.02, -0.12 + k * 0.12), rot=(-HALF, 0, 0), seg=10)
        m.greebles("Panel", (0.3, L * 0.2, 0.4), (1, 0, 0), (0, 1, 0), 0.5, 0.8, 4, h=(0.03, 0.08), s=(0.08, 0.2))
        m.box("Light", (0.04, 0.08, 0.04), at=(1.2, L * 0.2, 0.2))
    _ship_engines(m, (-0.5, 0.5), -L / 2 - 0.05, 0.0, 0.32)


def ion_frigate(m):
    """Ion disruptor: slim hull ending in a ring-shaped ion projector with glowing coils."""
    r = m.rng
    L = r.uniform(5.2, 5.6)
    m.loft("Hull", [
        (-L / 2, 0.6, 0.45, 0.4, 0.0, 4.0),
        (-L * 0.2, 0.75, 0.5, 0.45, 0.0, 4.0),
        (L * 0.2, 0.55, 0.38, 0.35, 0.0, 3.5),
        (L * 0.35, 0.4, 0.3, 0.28, 0.0, 3.0),
    ], seg=24)
    m.box("Team", (1.52, 0.35, 0.08), at=(0, -L * 0.15, 0.35), top=(0.95, 1.0))
    # Ion projector: two prongs with coils and a ring at the front.
    for s in m.sym():
        m.box("Dark", (0.18, 1.6, 0.3), at=(0.42, L * 0.38 + 0.55, 0.0), front=0.8)
        for k in range(4):
            m.cyl("Ion", 0.2, 0.06, at=(0.42, L * 0.38 + 0.1 + k * 0.28, 0.0), rot=(-HALF, 0, 0), seg=14)
    with m.at((0, L * 0.38 + 1.35, 0.0), (HALF, 0, 0)):
        m.torus("Metal", 0.55, 0.08, seg=32, rseg=8)
        m.torus("Ion", 0.55, 0.04, at=(0, 0, 0.06), seg=32, rseg=6)
    m.sphere("Ion", 0.18, at=(0, L * 0.38 + 0.6, 0.0), seg=14, rings=7)
    # Capacitor banks and radiators.
    for s in m.sym():
        with m.at((0.75, -L * 0.1, 0.0)):
            m.cyl_between("Dark", (0, -0.8, 0), (0, 0.8, 0), 0.22, seg=16)
            for k in range(4):
                m.torus("Metal", 0.23, 0.02, at=(0, -0.6 + k * 0.4, 0), rot=(HALF, 0, 0), seg=16, rseg=5)
        m.plate("Panel", [(0.6, -L * 0.42), (1.6, -L * 0.36), (1.6, -L * 0.2), (0.6, -L * 0.16)], 0.03, at=(0, 0, 0.0))
        m.box("Light", (0.04, 0.06, 0.04), at=(1.62, -L * 0.28, 0.0))
    with m.at((0, -L * 0.15, 0.45)):
        m.box("Hull", (0.6, 0.7, 0.35), at=(0, 0, 0.17), top=(0.85, 0.8), front=0.85)
        m.box("Window", (0.45, 0.04, 0.1), at=(0, 0.3, 0.25), rot=(-0.3, 0, 0))
        _antenna(m, (0.15, -0.2, 0.35), 0.5)
    _ship_engines(m, (0.0,), -L / 2 - 0.05, 0.0, 0.38)


def surveyor(m):
    """Surveyor sensor ship: compact hull, a large rotating radar dish, sensor booms and pods."""
    r = m.rng
    L = r.uniform(4.2, 4.6)
    m.loft("Hull", [
        (-L / 2, 0.55, 0.4, 0.4, 0.0, 3.0),
        (-L * 0.2, 0.7, 0.5, 0.45, 0.0, 3.5),
        (L * 0.2, 0.6, 0.45, 0.4, 0.0, 3.0),
        (L * 0.45, 0.25, 0.22, 0.2, 0.0, 2.5),
        (L / 2, 0.0, 0.0, 0.0, 0.0, 2.0),
    ], seg=24)
    m.box("Team", (1.42, 0.3, 0.08), at=(0, -L * 0.25, 0.33), top=(0.95, 1.0))
    m.box("Window", (0.5, 0.04, 0.12), at=(0, L * 0.36, 0.2), rot=(-0.5, 0, 0))
    # Big dorsal radar dish.
    with m.at((0, -L * 0.02, 0.5)):
        m.cyl("Dark", 0.12, 0.4, at=(0, 0, 0.2), seg=12)
        _dish(m, "Metal", 0.95, 0.3, at=(0, 0, 0.45), rot=(0.5, 0, 0.3), seg=32)
        m.cyl_between("Dark", (0, 0, 0.45), (0.0, 0.25, 0.95), 0.02, seg=6)
        m.sphere("Light", 0.04, at=(0.0, 0.25, 0.97), seg=8, rings=4)
    # Ventral sensor sphere (gravimetric detector).
    m.cyl("Dark", 0.1, 0.3, at=(0, L * 0.1, -0.5), seg=10)
    m.sphere("Glass", 0.25, at=(0, L * 0.1, -0.7), seg=20, rings=10)
    m.sphere("Ion", 0.1, at=(0, L * 0.1, -0.7), seg=12, rings=6)
    for s in m.sym():
        # Sensor booms with a phased-array panel.
        m.cyl_between("Metal", (0.55, -L * 0.15, 0.0), (1.9, -L * 0.25, 0.0), 0.04, seg=8)
        m.box("Panel", (0.08, 0.9, 0.5), at=(1.95, -L * 0.25, 0.0))
        m.box("Dark", (0.02, 0.8, 0.42), at=(2.0, -L * 0.25, 0.0))
        m.box("Light", (0.04, 0.04, 0.04), at=(1.95, -L * 0.25 + 0.48, 0.0))
        m.greebles("Panel", (0.25, -L * 0.4, 0.38), (1, 0, 0), (0, 1, 0), 0.3, 0.6, 3, h=(0.02, 0.05), s=(0.06, 0.15))
    _antenna(m, (0.3, L * 0.25, 0.35), 0.8)
    _ship_engines(m, (-0.35, 0.35), -L / 2 - 0.05, 0.0, 0.24)


def battlecruiser(m):
    """Capital ship: long armoured hull with a spinal plasma lance, turrets, a command tower."""
    r = m.rng
    L = r.uniform(11.5, 12.2)
    m.loft("Hull", [
        (-L / 2, 1.4, 0.9, 0.9, 0.0, 6.0),
        (-L * 0.35, 1.8, 1.15, 1.05, 0.0, 6.0),
        (L * 0.1, 1.6, 1.0, 0.95, 0.0, 5.0),
        (L * 0.35, 1.0, 0.7, 0.7, -0.05, 4.0),
        (L * 0.48, 0.55, 0.45, 0.45, -0.05, 3.5),
    ], seg=28)
    m.box("Dark", (2.6, L * 0.7, 0.25), at=(0, -L * 0.05, -1.0))
    # Armour belts and team bands.
    for s in m.sym():
        m.box("Panel", (0.15, L * 0.6, 1.0), at=(1.75, -L * 0.08, 0.0), top=(1.0, 0.95), rot=(0, -0.12, 0))
        m.box("Team", (0.05, L * 0.3, 0.2), at=(1.84, -L * 0.15, 0.25), rot=(0, -0.12, 0))
        m.bolts("Metal", [(1.83, y, z) for y in (-L * 0.32, -L * 0.18, -L * 0.04, L * 0.1) for z in (-0.35, 0.35)], normal=(1, 0, 0))
        m.greebles("Panel", (0.4, -L * 0.4, 0.95), (1, 0, 0), (0, 1, 0), 0.9, L * 0.3, 8, h=(0.04, 0.12), s=(0.1, 0.35))
        m.box("Light", (0.05, 0.1, 0.05), at=(1.85, L * 0.15, 0.4))
    m.box("Team", (3.0, 0.6, 0.12), at=(0, -L * 0.4, 0.85), top=(0.9, 1.0))
    # Spinal plasma lance: runs through the bow, glowing coils, muzzle.
    m.cyl_between("Dark", (0, L * 0.2, 0.0), (0, L * 0.5 + 1.0, 0.0), 0.32, 0.28, seg=24)
    for k in range(6):
        m.torus("Plasma", 0.36, 0.04, at=(0, L * 0.25 + k * 0.45, 0.0), rot=(HALF, 0, 0), seg=24, rseg=6)
    m.lathe("Metal", [(0.42, 0.0), (0.46, 0.35), (0.36, 0.45), (0.22, 0.45), (0.22, 0.1)], at=(0, L * 0.5 + 0.95, 0.0), rot=(-HALF, 0, 0), seg=24, cap_top=False)
    m.cyl("Plasma", 0.2, 0.02, at=(0, L * 0.5 + 1.05, 0.0), rot=(-HALF, 0, 0), seg=18)
    # Dorsal turrets (twin barrels).
    for y in (L * 0.18, -L * 0.02):
        with m.at((0, y, 1.0)):
            m.cyl("Dark", 0.55, 0.2, at=(0, 0, 0.1), seg=20)
            m.box("Hull", (0.9, 1.0, 0.45), at=(0, 0, 0.42), top=(0.85, 0.8), front=0.8)
            for s in m.sym():
                m.cyl_between("Metal", (0.18, 0.4, 0.45), (0.18, 1.6, 0.45), 0.07, 0.06, seg=12)
    # Command tower.
    with m.at((0, -L * 0.28, 1.0)):
        m.box("Hull", (1.3, 1.6, 1.1), at=(0, 0, 0.55), top=(0.8, 0.7), front=0.85)
        m.box("Hull", (0.9, 1.0, 0.5), at=(0, 0.05, 1.3), top=(0.85, 0.8), front=0.85)
        m.box("Window", (0.8, 0.05, 0.14), at=(0, 0.52, 1.35), rot=(-0.2, 0, 0))
        m.box("Window", (1.1, 0.05, 0.12), at=(0, 0.66, 0.85), rot=(-0.3, 0, 0))
        _dish(m, "Metal", 0.35, 0.1, at=(0.35, -0.3, 1.6), rot=(0.4, 0, 0))
        _antenna(m, (-0.3, -0.3, 1.55), 1.2)
        _antenna(m, (0.0, -0.5, 1.55), 0.8)
    # Shield emitters along the flanks.
    for s in m.sym():
        for y in (-L * 0.3, L * 0.05):
            m.sphere("Ion", 0.12, at=(1.6, y, 0.85), seg=10, rings=5)
    _ship_engines(m, (-1.0, 0.0, 1.0), -L / 2 - 0.05, 0.0, 0.48)
    _ship_engines(m, (-0.6, 0.6), -L / 2 - 0.05, -0.65, 0.3)


# ================================================================================= buildings
def research_lab(m):
    """Research lab: a ring of lab wings around an observatory dome, an antenna and a test pad."""
    r = m.rng
    _pad(m, 5.1, 0.45, seg=10)
    _hazard_ring(m, 4.98, 0.45, seg=10)
    # Lab block: two wings with window bands.
    for s in m.sym():
        with m.at((2.1, 0.0, 0.4)):
            m.box("Facade", (2.2, 4.4, 2.0), at=(0, 0, 1.0))
            m.box("Window", (0.06, 3.6, 0.5), at=(1.1, 0, 1.3))
            m.box("Dark", (2.3, 4.5, 0.15), at=(0, 0, 2.05))
            m.box("Team", (0.04, 4.4, 0.25), at=(1.12, 0, 1.8))
            for k in range(3):
                if r.random() < 0.7:
                    m.box("Light", (0.07, 0.6, 0.3), at=(1.11, -1.2 + k * 1.2, 1.3))
            m.greebles("Dark", (-0.9, -1.8, 2.12), (1, 0, 0), (0, 1, 0), 1.8, 3.6, 5, h=(0.1, 0.35), s=(0.2, 0.6))
    # Central observatory with a slotted dome.
    m.cyl("Facade", 1.6, 2.6, at=(0, 0, 0.4), seg=24, base=True)
    m.torus("Team", 1.62, 0.08, at=(0, 0, 2.7), seg=32, rseg=6)
    m.sphere("Hull", 1.6, at=(0, 0, 3.0), seg=28, rings=14, cut=0.0)
    m.box("Dark", (0.5, 1.7, 1.2), at=(0, 0.6, 3.9), rot=(-0.5, 0, 0))
    with m.at((0, 0.6, 3.9), (-0.5, 0, 0)):
        m.cyl("Glass", 0.25, 0.6, at=(0, 0.2, 0.5), rot=(0.3, 0, 0), seg=16)
    # Entrance and a little particle accelerator ring on the pad.
    m.box("Dark", (1.4, 0.4, 1.8), at=(0, 2.25, 1.3))
    m.box("Window", (1.0, 0.1, 1.4), at=(0, 2.45, 1.2))
    m.box("Light", (1.2, 0.12, 0.06), at=(0, 2.5, 2.1))
    m.torus("Metal", 1.0, 0.12, at=(-3.2, -3.0, 0.75), seg=28, rseg=8)
    m.torus("Engine", 1.0, 0.04, at=(-3.2, -3.0, 0.88), seg=28, rseg=5)
    for i in m.around(4):
        m.box("Dark", (0.2, 0.2, 0.4), at=(1.0, 0, 0.55))
    _antenna(m, (1.2, -1.6, 2.5), 2.5)
    _dish(m, "Metal", 0.55, 0.18, at=(-1.4, 1.4, 2.6), rot=(0.5, -0.3, 0))


def robotics_bay(m):
    """Robotics bay: an assembly hangar with a tall door, a gantry crane and a robot arm."""
    r = m.rng
    _pad(m, 6.1, 0.45, seg=10)
    # Hangar: barrel roof.
    m.box("Facade", (7.0, 6.0, 3.0), at=(0, -0.4, 1.9))
    with m.at((0, -0.4, 3.4), (0, HALF, 0)):
        m.lathe("Sheet", [(3.45, -3.0), (3.45, 3.0)], seg=24, sx=0.45, sy=1.0, cap_bottom=True, cap_top=True)
    m.box("Team", (7.04, 0.3, 0.35), at=(0, -0.4, 3.25))
    # Big door at the front.
    m.box("Dark", (4.2, 0.3, 3.2), at=(0, 2.65, 1.9))
    for k in range(6):
        m.box("Metal", (4.0, 0.08, 0.42), at=(0, 2.82, 0.6 + k * 0.5))
    m.box("Hazard", (4.4, 0.32, 0.25), at=(0, 2.7, 3.6))
    m.box("Light", (0.2, 0.1, 0.2), at=(2.4, 2.85, 3.4))
    m.box("Light", (0.2, 0.1, 0.2), at=(-2.4, 2.85, 3.4))
    # Gantry crane over an assembly pad beside the hangar.
    m.box("Deck", (2.4, 3.0, 0.15), at=(4.6, 1.0, 0.5))
    for y in (-0.3, 2.3):
        m.beam("Metal", (3.6, y, 0.5), (3.6, y, 3.5), 0.18)
        m.beam("Metal", (5.6, y, 0.5), (5.6, y, 3.5), 0.18)
        m.beam("Hazard", (3.5, y, 3.5), (5.7, y, 3.5), 0.22)
    m.beam("Dark", (4.6, -0.3, 3.4), (4.6, 2.3, 3.4), 0.2)
    m.cyl("Metal", 0.02, 1.2, at=(4.6, 1.0, 2.8), seg=6)
    m.box("Dark", (0.4, 0.4, 0.2), at=(4.6, 1.0, 2.1))
    # Robot arm (industrial manipulator) on the other side.
    with m.at((-4.4, 1.6, 0.45)):
        m.cyl("Hazard", 0.5, 0.4, at=(0, 0, 0.2), seg=18)
        m.cyl("Dark", 0.35, 0.4, at=(0, 0, 0.6), seg=16)
        sh, el, wr = V(0, 0, 0.9), V(0.3, 0.6, 2.3), V(0.5, 1.6, 2.0)
        m.beam("Hazard", sh, el, 0.3, 0.35)
        m.sphere("Dark", 0.22, at=el, seg=12, rings=6)
        m.beam("Hazard", el, wr, 0.22, 0.25)
        m.box("Dark", (0.25, 0.3, 0.25), at=wr)
        m.cyl_between("Metal", wr, wr + V(0, 0.35, -0.2), 0.06, seg=8)
        m.sphere("Light", 0.05, at=wr + V(0, 0.4, -0.22), seg=8, rings=4)
    # Roof equipment.
    for k in range(3):
        m.cyl("Metal", 0.35, 0.8, at=(-2.0 + k * 2.0, -2.8, 3.6), seg=12, base=True)
        m.cyl("Dark", 0.4, 0.08, at=(-2.0 + k * 2.0, -2.8, 4.42), seg=12)
    m.vents("Dark", at=(0, -3.42, 1.5), size=(2.0, 0.8), n=5, rot=(0, 0, PI))


def _turret_base(m, r=2.1, h=1.0):
    m.lathe("Concrete", [(r + 0.1, -0.3), (r + 0.1, 0.35), (r - 0.2, 0.55)], seg=12, phase=PI / 12)
    _hazard_ring(m, r - 0.05, 0.56)
    m.lathe("Dark", [(r * 0.7, 0.5), (r * 0.68, h), (r * 0.6, h + 0.1)], seg=24)
    m.torus("Metal", r * 0.6, 0.06, at=(0, 0, h + 0.12), seg=36, rseg=6)


def laser_turret(m):
    """Laser turret: a slim rotating head with a lensed emitter and cooling fins."""
    r = m.rng
    _turret_base(m)
    with m.at((0, 0, 1.45)):
        m.cyl("Dark", 0.55, 0.5, seg=20)
        with m.at((0, 0, 0.55), (0.15, 0, 0)):
            m.box("Hull", (1.0, 1.7, 0.75), top=(0.8, 0.85), front=0.65)
            m.box("Team", (1.02, 0.4, 0.07), at=(0, -0.45, 0.33), top=(0.85, 1.0))
            # Emitter barrel with focusing rings and a lens.
            m.cyl_between("Dark", (0, 0.8, 0.0), (0, 1.9, 0.0), 0.2, 0.17, seg=20)
            for k in range(3):
                m.torus("Metal", 0.22, 0.03, at=(0, 1.0 + k * 0.3, 0.0), rot=(HALF, 0, 0), seg=20, rseg=6)
            m.cyl("Glass", 0.15, 0.06, at=(0, 1.93, 0.0), rot=(-HALF, 0, 0), seg=20)
            m.cyl("Laser", 0.08, 0.02, at=(0, 1.97, 0.0), rot=(-HALF, 0, 0), seg=14)
            # Cooling fins along the back.
            for k in range(6):
                m.box("Metal", (1.1, 0.04, 0.5), at=(0, -0.95 - k * 0.1, 0.0))
            for s in m.sym():
                m.box("Panel", (0.05, 1.0, 0.45), at=(0.48, 0.0, 0.0), rot=(0, -0.2, 0))
                m.cyl("Laser", 0.04, 0.02, at=(0.4, 0.7, 0.2), rot=(-HALF, 0, 0), seg=8)
    m.sphere("Light", 0.05, at=(0.0, -0.5, 2.55), seg=8, rings=4)


def missile_battery(m):
    """Surface-to-orbit missile battery: a launcher box with six cells on a raised mount."""
    r = m.rng
    _turret_base(m, 2.5, 0.9)
    m.cyl("Dark", 0.7, 0.6, at=(0, 0, 1.35), seg=20)
    with m.at((0, -0.2, 1.75)):
        for s in m.sym():
            m.box("Hull", (0.18, 1.2, 1.3), at=(0.9, 0, 0.5), top=(1.0, 0.7))
            m.cyl("Dark", 0.25, 0.2, at=(0.9, 0.0, 0.85), rot=(0, HALF, 0), seg=14)
        # Launcher pitched up 50 degrees.
        with m.at((0, 0, 0.85), (0.9, 0, 0)):
            m.box("Hull", (1.5, 2.2, 1.05), top=(0.97, 0.97))
            m.box("Team", (1.52, 0.4, 1.07), at=(0, -0.6, 0))
            for ix in range(3):
                for iz in range(2):
                    x, z = -0.45 + ix * 0.45, -0.22 + iz * 0.44
                    m.box("Dark", (0.36, 0.06, 0.36), at=(x, 1.11, z))
                    m.cyl("Hazard", 0.13, 0.04, at=(x, 1.14, z), rot=(-HALF, 0, 0), seg=14)
            m.box("Dark", (1.0, 0.3, 0.6), at=(0, -1.2, 0))
    # Fire-control radar on a mast.
    with m.at((-1.5, -1.3, 0.55)):
        m.cyl("Metal", 0.06, 1.6, at=(0, 0, 0.8), seg=8)
        m.box("Panel", (0.9, 0.08, 0.6), at=(0, 0, 1.75), rot=(-0.3, 0, 0.6))
        m.sphere("Light", 0.05, at=(0, 0, 2.15), seg=8, rings=4)
    m.box("Hazard", (0.8, 0.8, 0.5), at=(1.6, -1.2, 0.8))


def bunker(m):
    """Bunker: a low octagonal casemate of reinforced concrete with firing slits and HMG ports."""
    r = m.rng
    R = 3.2
    m.lathe("Concrete", [(R + 0.4, -0.3), (R + 0.4, 0.2), (R, 0.4)], seg=8, phase=PI / 8)
    # Sloped walls, a thick roof slab, steel reinforcement bands.
    m.lathe("Concrete", [(R, 0.3), (R * 0.85, 1.7), (R * 0.82, 1.9)], seg=8, phase=PI / 8)
    m.lathe("Concrete", [(R * 0.86, 1.85), (R * 0.88, 2.25), (R * 0.75, 2.4), (0.01, 2.45)], seg=8, phase=PI / 8)
    m.lathe("Dark", [(R * 0.97, 0.6), (R * 0.95, 0.75)], seg=8, phase=PI / 8, cap_bottom=False, cap_top=False)
    m.lathe("Metal", [(R * 0.89, 1.5), (R * 0.88, 1.6)], seg=8, phase=PI / 8, cap_bottom=False, cap_top=False)
    m.lathe("Team", [(R * 0.885, 2.1), (R * 0.88, 2.22)], seg=8, phase=PI / 8, cap_bottom=False, cap_top=False)
    for k in range(8):
        a = PI / 8 + (k + 0.5) * TAU / 8
        apo = R * 0.9 * math.cos(PI / 8)
        with m.at((math.cos(a) * apo, math.sin(a) * apo, 0), (0, 0, a - HALF)):
            # Firing slit with a machine gun on every other face.
            m.box("Dark", (1.4, 0.3, 0.22), at=(0, 0.05, 1.35), rot=(0.25, 0, 0))
            if k % 2 == 1:
                m.cyl_between("Metal", (0, 0.0, 1.35), (0, 0.85, 1.38), 0.05, seg=10)
                m.box("Dark", (0.24, 0.25, 0.2), at=(0, 0.05, 1.35))
            else:
                m.box("Light", (0.9, 0.06, 0.06), at=(0, 0.16, 1.32))
    # Roof cupola with periscopes and sandbag-like ballast blocks.
    m.cyl("Hull", 0.7, 0.5, at=(0, 0, 2.6), seg=12)
    m.cyl("Dark", 0.75, 0.1, at=(0, 0, 2.9), seg=12)
    for i in m.around(4, phase=0.4):
        m.box("Dark", (0.12, 0.12, 0.25), at=(0.5, 0, 3.0))
    m.greebles("Concrete", (-1.8, -1.8, 2.4), (1, 0, 0), (0, 1, 0), 3.6, 3.6, 10, h=(0.1, 0.25), s=(0.3, 0.6))
    # Blast door.
    m.box("Dark", (1.1, 0.4, 1.3), at=(0, -R * 0.83, 0.95))
    m.box("Hazard", (1.2, 0.42, 0.12), at=(0, -R * 0.83, 1.65))


def plasma_cannon(m):
    """Plasma cannon emplacement: armoured mount, magnetic accelerator barrel, heat sinks."""
    r = m.rng
    _turret_base(m, 3.0, 1.0)
    m.cyl("Dark", 1.2, 0.5, at=(0, 0, 1.4), seg=24)
    with m.at((0, -0.3, 1.65)):
        for s in m.sym():
            m.box("Hull", (0.35, 2.0, 1.8), at=(1.0, 0, 0.9), top=(1.0, 0.6))
            m.box("Team", (0.37, 0.8, 0.3), at=(1.0, -0.3, 1.2))
            m.cyl("Dark", 0.45, 0.4, at=(1.0, 0.1, 1.5), rot=(0, HALF, 0), seg=16)
        with m.at((0, 0.1, 1.5), (0.35, 0, 0)):
            m.box("Hull", (1.5, 2.0, 1.2), at=(0, -0.3, 0), top=(0.9, 0.9), front=0.85)
            # Accelerator barrel.
            m.cyl_between("Dark", (0, 0.6, 0), (0, 4.2, 0), 0.38, 0.32, seg=24)
            for k in range(6):
                m.torus("Plasma", 0.42, 0.05, at=(0, 1.0 + k * 0.5, 0), rot=(HALF, 0, 0), seg=24, rseg=6)
                m.torus("Metal", 0.44, 0.04, at=(0, 1.2 + k * 0.5, 0), rot=(HALF, 0, 0), seg=24, rseg=6)
            m.lathe("Metal", [(0.48, 0.0), (0.52, 0.4), (0.42, 0.5), (0.26, 0.5), (0.26, 0.1)], at=(0, 4.15, 0), rot=(-HALF, 0, 0), seg=24, cap_top=False)
            m.cyl("Plasma", 0.24, 0.02, at=(0, 4.25, 0), rot=(-HALF, 0, 0), seg=18)
            # Heat sinks and plasma tanks at the breech.
            for k in range(5):
                m.box("Metal", (1.7, 0.05, 0.9), at=(0, -1.35 - k * 0.12, 0))
            for s in m.sym():
                m.cyl_between("Hull", (0.55, -1.0, 0.65), (0.55, 0.4, 0.65), 0.22, seg=14)
                m.tube("Plasma", [(0.55, 0.4, 0.65), (0.4, 0.7, 0.45), (0.38, 1.0, 0.3)], 0.04, seg=6)
    m.sphere("Light", 0.06, at=(-1.3, -1.6, 2.8), seg=8, rings=4)


def shield_generator(m):
    """Shield generator: a pylon with field coils and an emitter sphere inside a faint dome."""
    r = m.rng
    _pad(m, 3.0, 0.4, seg=12)
    _hazard_ring(m, 2.88, 0.4)
    # Capacitor ring at the foot.
    for i in m.around(6):
        with m.at((1.9, 0, 0.4)):
            m.cyl("Hull", 0.35, 1.0, at=(0, 0, 0.5), seg=14)
            m.cyl("Team", 0.37, 0.15, at=(0, 0, 0.75), seg=14)
            m.cyl("Dark", 0.38, 0.1, at=(0, 0, 1.05), seg=14)
            m.cyl_between("Metal", (0, 0, 1.0), (-1.3, 0, 1.8), 0.05, seg=6)
    # Central pylon tapering up with field coils.
    m.lathe("Hull", [(0.9, 0.4), (0.8, 1.5), (0.45, 3.0), (0.35, 4.0)], seg=24)
    for k in range(4):
        m.torus("Metal", 0.85 - k * 0.15, 0.09, at=(0, 0, 1.4 + k * 0.7), seg=28, rseg=8)
        m.torus("Ion", 0.85 - k * 0.15, 0.04, at=(0, 0, 1.48 + k * 0.7), seg=28, rseg=5)
    # Emitter sphere held by three prongs, inside a translucent dome.
    for i in m.around(3, phase=0.3):
        m.tube("Dark", [(0.3, 0, 3.9), (0.65, 0, 4.4), (0.45, 0, 4.9)], 0.06, seg=8)
    m.sphere("Glass", 0.42, at=(0, 0, 4.6), seg=22, rings=11)
    m.sphere("Ion", 0.26, at=(0, 0, 4.6), seg=16, rings=8)
    m.sphere("Shield", 3.0, at=(0, 0, 0.4), seg=32, rings=12, cut=0.0)
    m.vents("Dark", at=(0, 0.9, 1.0), size=(0.6, 0.4), n=3)


def radar_station(m):
    """Radar station: equipment hut, lattice mast and a rotating phased-array antenna."""
    r = m.rng
    _pad(m, 2.5, 0.4, seg=10)
    m.box("Facade", (1.8, 1.4, 1.2), at=(-0.6, -0.8, 1.0))
    m.box("Dark", (1.9, 1.5, 0.12), at=(-0.6, -0.8, 1.66))
    m.box("Window", (0.06, 0.8, 0.35), at=(0.31, -0.8, 1.15))
    m.box("Team", (1.82, 0.04, 0.2), at=(-0.6, -0.08, 1.45))
    m.box("Dark", (0.6, 0.05, 0.9), at=(-0.6, -0.1, 0.85))
    m.vents("Dark", at=(-0.6, -1.52, 1.1), size=(0.8, 0.4), n=3, rot=(0, 0, PI))
    # Lattice mast.
    m.lattice("Metal", 0.4, 4.2, 0.55, 0.25, 4, post=0.1, brace=0.04)
    m.cyl("Dark", 0.35, 0.3, at=(0, 0, 4.35), seg=16)
    # Antenna: tilted rectangular array with a feed boom.
    with m.at((0, 0, 4.6), (0, 0, r.uniform(0, TAU))):
        m.cyl("Dark", 0.12, 0.4, at=(0, 0, 0.2), seg=10)
        with m.at((0, 0.1, 0.9), (-0.35, 0, 0)):
            m.box("Panel", (3.2, 0.15, 1.1))
            m.box("Dark", (3.0, 0.05, 0.95), at=(0, 0.1, 0))
            for k in range(8):
                m.box("Metal", (0.04, 0.08, 0.95), at=(-1.4 + k * 0.4, 0.14, 0))
            m.box("Team", (3.22, 0.16, 0.12), at=(0, 0, -0.6))
        m.beam("Metal", (0, -0.1, 0.4), (0, -0.6, 0.9), 0.06)
    m.sphere("Light", 0.06, at=(0, 0, 5.85), seg=8, rings=4)
    _antenna(m, (-1.3, -1.3, 1.72), 1.2)


def seismic_sensor(m):
    """Seismic sensor: a geophone spike driven into the crust, a sensor housing and cables."""
    r = m.rng
    m.lathe("Concrete", [(1.9, -0.3), (1.9, 0.15), (1.7, 0.25)], seg=10, phase=PI / 10)
    # Spike rising from the ground with clamp rings.
    m.lathe("Dark", [(0.45, 0.1), (0.38, 1.2), (0.18, 2.6), (0.0, 3.2)], seg=12, smooth=False)
    for k in range(3):
        m.torus("Metal", 0.43 - k * 0.07, 0.06, at=(0, 0, 0.4 + k * 0.6), seg=20, rseg=6)
    m.torus("Glow", 0.2, 0.03, at=(0, 0, 2.5), seg=16, rseg=5)
    # Sensor housing.
    with m.at((0.9, 0.4, 0.25)):
        m.box("Hull", (0.8, 0.9, 0.6), at=(0, 0, 0.3), top=(0.85, 0.85))
        m.box("Team", (0.82, 0.2, 0.08), at=(0, 0, 0.5))
        m.box("Light", (0.1, 0.03, 0.06), at=(0.2, 0.46, 0.4))
        _antenna(m, (-0.2, -0.2, 0.6), 1.0)
    # Geophones on the ground with cables.
    for i in m.around(4, phase=0.6):
        m.cyl("Dark", 0.12, 0.2, at=(1.5, 0, 0.2), seg=10)
        m.cyl("Hazard", 0.13, 0.05, at=(1.5, 0, 0.3), seg=10)
        m.tube("Rubber", [(0.4, 0, 0.2), (0.9, 0, 0.18), (1.4, 0, 0.2)], 0.025, seg=6)
    m.box("Hazard", (0.3, 0.3, 0.4), at=(-0.9, -0.7, 0.4))


def deep_space_array(m):
    """Deep-space array: a large steerable dish on an alt-az mount plus two smaller dishes."""
    r = m.rng
    _pad(m, 5.0, 0.4, seg=12)
    # Mount: a rotating drum and two yoke arms.
    m.cyl("Facade", 1.6, 1.4, at=(0, 0, 0.4), seg=24, base=True)
    m.cyl("Dark", 1.7, 0.15, at=(0, 0, 1.8), seg=24)
    m.torus("Team", 1.62, 0.07, at=(0, 0, 1.3), seg=28, rseg=6)
    for s in m.sym():
        m.box("Hull", (0.3, 1.0, 2.6), at=(1.1, 0, 3.1), top=(1.0, 0.6))
        m.cyl("Dark", 0.35, 0.4, at=(1.1, 0, 4.1), rot=(0, HALF, 0), seg=16)
    # Main dish tilted up towards the sky.
    with m.at((0, 0, 4.1), (-0.75, 0, 0)):
        m.cyl("Dark", 0.5, 0.6, at=(0, 0, 0.0), seg=16)
        _dish(m, "Metal", 3.6, 1.2, at=(0, 0, 0.35), seg=40)
        for i in m.around(4, phase=0.785):
            m.cyl_between("Metal", (3.3, 0, 1.4), (0, 0, 3.0), 0.05, seg=6)
        m.cyl("Dark", 0.3, 0.5, at=(0, 0, 3.1), seg=12)
        m.sphere("Light", 0.08, at=(0, 0, 3.4), seg=8, rings=4)
        # Backing truss.
        for i in m.around(6):
            m.beam("Dark", (0.4, 0, 0.1), (2.6, 0, 0.6), 0.12)
    # Two smaller dishes on the pad.
    for x, y, a in ((3.4, -2.2, 0.6), (-3.4, -2.2, -0.6)):
        m.cyl("Metal", 0.1, 1.4, at=(x, y, 1.1), seg=8)
        _dish(m, "Metal", 0.9, 0.3, at=(x, y, 1.8), rot=(-0.6, 0, a), seg=24)
    m.box("Facade", (1.6, 1.0, 1.0), at=(0, -3.6, 0.9))
    m.box("Window", (1.2, 0.05, 0.3), at=(0, -3.08, 1.1))
    _antenna(m, (0.6, -3.6, 1.4), 1.5)


def gravimetric_beacon(m):
    """Gravimetric beacon: an obelisk with a crystal core and floating measurement rings."""
    r = m.rng
    m.lathe("Concrete", [(1.9, -0.3), (1.9, 0.2), (1.7, 0.3)], seg=6, phase=PI / 6)
    _hazard_ring(m, 1.8, 0.31, seg=6)
    # Four-sided obelisk split open in the middle.
    for i in m.around(4, phase=PI / 4):
        m.box("Hull", (0.5, 0.25, 4.2), at=(0.42, 0, 2.4), top=(0.6, 0.8), rot=(0, -0.04, 0))
        m.box("Team", (0.52, 0.27, 0.25), at=(0.42, 0, 1.0))
        m.box("Dark", (0.15, 0.15, 3.0), at=(0.68, 0, 2.2))
    # Crystal core.
    m.crystal("Crystal", (0, 0, 1.6), (0, 0, 1), 2.0, 0.25, sides=6)
    m.sphere("Ion", 0.16, at=(0, 0, 2.8), seg=12, rings=6)
    # Floating rings at different tilts.
    for k, (z, tilt) in enumerate(((2.3, 0.2), (3.1, -0.3), (3.9, 0.15))):
        m.torus("Metal", 1.0 - k * 0.15, 0.05, at=(0, 0, z), rot=(tilt, tilt * 0.5, 0), seg=32, rseg=6)
        m.torus("Glow", 1.0 - k * 0.15, 0.02, at=(0, 0, z + 0.05), rot=(tilt, tilt * 0.5, 0), seg=32, rseg=5)
    m.sphere("Light", 0.07, at=(0, 0, 4.75), seg=8, rings=4)
    m.box("Hull", (0.6, 0.5, 0.6), at=(1.2, -1.0, 0.6))
    m.box("Light", (0.1, 0.03, 0.08), at=(1.2, -0.74, 0.7))


# ================================================================================== registry
# key -> (generator, kind, texture repeat scale, bevel width, AO map size), as in build_models.
MODELS_EXTRA = {
    "lancer": (lancer, "unit", 0.4, 0.006, 512),
    "scout_rover": (scout_rover, "unit", 0.5, 0.01, 512),
    "artillery": (artillery, "unit", 0.6, 0.014, 512),
    "sentinel": (sentinel, "unit", 0.5, 0.01, 512),
    "repair_bot": (repair_bot, "unit", 0.4, 0.008, 512),
    "infiltrator": (infiltrator, "unit", 0.4, 0.006, 512),
    "juggernaut": (juggernaut, "unit", 0.7, 0.018, 1024),
    "probe": (probe, "ship", 0.4, 0.006, 512),
    "missile_frigate": (missile_frigate, "ship", 0.8, 0.016, 1024),
    "ion_frigate": (ion_frigate, "ship", 0.8, 0.016, 1024),
    "surveyor": (surveyor, "ship", 0.7, 0.014, 512),
    "battlecruiser": (battlecruiser, "ship", 1.0, 0.025, 1024),
    "research_lab": (research_lab, "building", 1.0, 0.025, 1024),
    "robotics_bay": (robotics_bay, "building", 1.0, 0.03, 1024),
    "laser_turret": (laser_turret, "building", 0.8, 0.02, 512),
    "missile_battery": (missile_battery, "building", 0.8, 0.02, 512),
    "bunker": (bunker, "building", 1.0, 0.03, 1024),
    "plasma_cannon": (plasma_cannon, "building", 0.9, 0.022, 1024),
    "shield_generator": (shield_generator, "building", 0.9, 0.02, 512),
    "radar_station": (radar_station, "building", 0.8, 0.015, 512),
    "seismic_sensor": (seismic_sensor, "building", 0.8, 0.015, 512),
    "deep_space_array": (deep_space_array, "building", 1.0, 0.02, 1024),
    "gravimetric_beacon": (gravimetric_beacon, "building", 0.8, 0.015, 512),
}
