// Shared by planet.wesl (the surface) and atmosphere.wesl (scattering + clouds): the per-planet
// parameters, gradient noise, ray / sphere helpers and the cloud field (the surface samples the
// same clouds for their shadows). Written by `game/planet_render.rs` (`PlanetParams`).

const PI: f32 = 3.14159265;

struct PlanetParams {
    // xyz: centre (world), w: base radius (m)
    center: vec4<f32>,
    // rotation local -> world (quaternion xyzw); only the menu globe turns
    orient: vec4<f32>,
    // x: relief (m), y: sea height above the base radius (m), z: sea kind (0 none, 1 water,
    // 2 ice, 3 lava, 4 acid), w: 1 = glowing lava cracks on land
    shape: vec4<f32>,
    // xyz: unit direction to the sun, w: sun illuminance at the planet (lux)
    sun: vec4<f32>,
    // rgb: sun colour, w: how much of the directional shadow map applies (focused planet only)
    sun_color: vec4<f32>,
    // rgb: atmosphere tint, w: density (0 = none)
    atmo: vec4<f32>,
    // x: top radius, y: Rayleigh scale height, z: haze (Mie) amount, w: brightness
    atmo2: vec4<f32>,
    // rgb: rock colour on steep slopes, w: noise seed offset
    rock: vec4<f32>,
    // rgb: cloud colour, w: coverage 0..1 (0 = no clouds)
    cloud: vec4<f32>,
    // x: cloud layer radius, y: opacity (fades out when the camera comes close), z: cloud shadow
    // strength on the ground, w: drift speed (rad/s)
    cloud2: vec4<f32>,
    // x: surface quality 0..4, y: view ray steps, z: light ray steps, w: cloud octaves
    quality: vec4<f32>,
};

fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

fn quat_rotate_inv(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return quat_rotate(vec4<f32>(-q.xyz, q.w), v);
}

// ---------------------------------------------------------------- noise

fn pcg3d(v0: vec3<u32>) -> vec3<u32> {
    var v = v0 * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v = v ^ (v >> vec3<u32>(16u));
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return v;
}

fn grad3(i: vec3<f32>) -> vec3<f32> {
    let h = pcg3d(bitcast<vec3<u32>>(vec3<i32>(i)));
    return vec3<f32>(h & vec3<u32>(0xffffu)) * (2.0 / 65535.0) - 1.0;
}

// Gradient noise (about -1..1) with its analytic derivatives: (value, d/dx, d/dy, d/dz).
fn gnoise_d(x: vec3<f32>) -> vec4<f32> {
    let i = floor(x);
    let f = x - i;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);

    let ga = grad3(i + vec3<f32>(0.0, 0.0, 0.0));
    let gb = grad3(i + vec3<f32>(1.0, 0.0, 0.0));
    let gc = grad3(i + vec3<f32>(0.0, 1.0, 0.0));
    let gd = grad3(i + vec3<f32>(1.0, 1.0, 0.0));
    let ge = grad3(i + vec3<f32>(0.0, 0.0, 1.0));
    let gf = grad3(i + vec3<f32>(1.0, 0.0, 1.0));
    let gg = grad3(i + vec3<f32>(0.0, 1.0, 1.0));
    let gh = grad3(i + vec3<f32>(1.0, 1.0, 1.0));

    let va = dot(ga, f - vec3<f32>(0.0, 0.0, 0.0));
    let vb = dot(gb, f - vec3<f32>(1.0, 0.0, 0.0));
    let vc = dot(gc, f - vec3<f32>(0.0, 1.0, 0.0));
    let vd = dot(gd, f - vec3<f32>(1.0, 1.0, 0.0));
    let ve = dot(ge, f - vec3<f32>(0.0, 0.0, 1.0));
    let vf = dot(gf, f - vec3<f32>(1.0, 0.0, 1.0));
    let vg = dot(gg, f - vec3<f32>(0.0, 1.0, 1.0));
    let vh = dot(gh, f - vec3<f32>(1.0, 1.0, 1.0));

    let v = va + u.x * (vb - va) + u.y * (vc - va) + u.z * (ve - va)
        + u.x * u.y * (va - vb - vc + vd) + u.y * u.z * (va - vc - ve + vg)
        + u.z * u.x * (va - vb - ve + vf) + u.x * u.y * u.z * (-va + vb + vc - vd + ve - vf - vg + vh);

    let d = ga + u.x * (gb - ga) + u.y * (gc - ga) + u.z * (ge - ga)
        + u.x * u.y * (ga - gb - gc + gd) + u.y * u.z * (ga - gc - ge + gg)
        + u.z * u.x * (ga - gb - ge + gf) + u.x * u.y * u.z * (-ga + gb + gc - gd + ge - gf - gg + gh)
        + du * (vec3<f32>(vb - va, vc - va, ve - va)
            + u.yzx * vec3<f32>(va - vb - vc + vd, va - vc - ve + vg, va - vb - ve + vf)
            + u.zxy * vec3<f32>(va - vb - ve + vf, va - vb - vc + vd, va - vc - ve + vg)
            + u.yzx * u.zxy * (-va + vb + vc - vd + ve - vf - vg + vh));
    return vec4<f32>(v, d);
}

fn gnoise(x: vec3<f32>) -> f32 {
    return gnoise_d(x).x;
}

// Rotates successive octaves so their lattices do not line up.
fn octave_shift(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.00 * p.x + 0.80 * p.y + 0.60 * p.z,
        -0.80 * p.x + 0.36 * p.y - 0.48 * p.z,
        -0.60 * p.x - 0.48 * p.y + 0.64 * p.z,
    ) * 2.03 + vec3<f32>(17.1, -3.7, 9.3);
}

fn fbm(p0: vec3<f32>, octaves: i32) -> f32 {
    var p = p0;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < octaves; i = i + 1) {
        s += a * gnoise(p);
        p = octave_shift(p);
        a *= 0.5;
    }
    return s;
}

// fBm with derivatives (in the coordinates of p0). Octaves finer than ~2 pixels fade out:
// `footprint` is the size of a pixel in the units of p0.
fn fbm_d(p0: vec3<f32>, octaves: i32, footprint: f32) -> vec4<f32> {
    var p = p0;
    var a = 0.5;
    var freq = 1.0;
    var s = vec4<f32>(0.0);
    for (var i = 0; i < octaves; i = i + 1) {
        let fade = 1.0 - smoothstep(0.25, 0.6, footprint * freq);
        if (fade <= 0.0) {
            break;
        }
        let n = gnoise_d(p);
        // Octave i samples p0 * freq + offset, so its derivative is scaled by freq (no rotation
        // here, unlike `fbm`, to keep the chain rule this simple).
        s += a * fade * vec4<f32>(n.x, n.yzw * freq);
        p = p * 2.03 + vec3<f32>(17.1, -3.7, 9.3);
        a *= 0.5;
        freq *= 2.03;
    }
    return s;
}

// ---------------------------------------------------------------- rays and spheres

// Distances along the ray to a sphere around the origin (x > y when it misses).
fn ray_sphere(ro: vec3<f32>, rd: vec3<f32>, r: f32) -> vec2<f32> {
    let b = dot(ro, rd);
    let c = dot(ro, ro) - r * r;
    let h = b * b - c;
    if (h < 0.0) {
        return vec2<f32>(1e9, -1e9);
    }
    let s = sqrt(h);
    return vec2<f32>(-b - s, -b + s);
}

// ---------------------------------------------------------------- clouds

// Cloud density 0..1 in a direction from the planet's centre (local frame), drifting with time.
fn cloud_density(dir: vec3<f32>, pp: PlanetParams, time: f32) -> f32 {
    let coverage = pp.cloud.w;
    if (coverage <= 0.0) {
        return 0.0;
    }
    let octaves = i32(pp.quality.w);
    // Drift: turn around the planet's axis, faster at the equator band than at the poles.
    let ang = time * pp.cloud2.w * (1.0 - 0.4 * dir.y * dir.y);
    let c = cos(ang);
    let s = sin(ang);
    let d = vec3<f32>(c * dir.x - s * dir.z, dir.y, s * dir.x + c * dir.z);
    let seed = vec3<f32>(pp.rock.w * 0.37, pp.rock.w * 0.11, -pp.rock.w * 0.23);
    let p = d * 3.4 + seed;
    // Domain warp gives swirls and fronts instead of plain blobs.
    let warp = vec3<f32>(gnoise(p * 0.7 + vec3<f32>(5.2, 1.3, 7.7)), gnoise(p * 0.7 + vec3<f32>(9.1, 4.4, 2.8)), gnoise(p * 0.7 + vec3<f32>(3.3, 8.6, 6.1)));
    let q = p + warp * 1.3 + vec3<f32>(0.0, time * 0.004, 0.0);
    // Large shapes, eroded at their edges by finer billows (crisp, cauliflower-like borders).
    let shape = fbm(q, 3);
    let detail = fbm(q * 3.3 + vec3<f32>(3.1, -7.2, 1.7), max(octaves - 1, 1));
    // Latitude belts: fewer clouds in the subtropics, more near the poles and the equator.
    let lat = abs(d.y);
    let rim = 1.0 - smoothstep(-0.15, 0.35, shape);
    let n = shape + 0.08 * cos(lat * 9.0) - 0.04 - rim * abs(detail) * 0.7 + detail * 0.12;
    let edge = (1.0 - coverage) * 0.5 - 0.22;
    // Sharp borders, denser cores.
    return smoothstep(edge, edge + 0.05, n) * (0.7 + 0.3 * smoothstep(edge + 0.05, edge + 0.3, n));
}
