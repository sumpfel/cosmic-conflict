// Atmosphere shell: single scattering (Rayleigh + Mie) ray-marched from the camera through the
// air in front of the planet, with the planet's shadow in the air, sunlight reddened on its way
// in, and the cloud layer composited at its height. Drawn on a sphere slightly larger than the
// air (front faces from outside, back faces from inside) with premultiplied alpha: the surface
// behind is dimmed by the transmittance and the clouds.
//
// Steps (`pp.quality.y` view, `pp.quality.z` light): 0 light steps = analytic approximation.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, globals, lights},
    view_transformations::{frag_coord_to_ndc, position_ndc_to_world},
};
import bevy_pbr::prepass::utils as prepass_utils;
import super::planet_common::{PlanetParams, PI, quat_rotate_inv, ray_sphere, cloud_density};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> pp: PlanetParams;

// Optical depth (Rayleigh, Mie) from `p` (relative to the centre) towards the sun.
fn light_depth(p: vec3<f32>, l: vec3<f32>, radius: f32, top: f32, h_r: f32, h_m: f32, steps: i32) -> vec2<f32> {
    let h0 = max(length(p) - radius, 0.0);
    if (steps <= 0) {
        // Exponential atmosphere, airmass from the sun's elevation.
        let mu = dot(normalize(p), l);
        let airmass = 1.0 / (max(mu, 0.0) * 0.9 + 0.08);
        return vec2<f32>(h_r * exp(-h0 / h_r), h_m * exp(-h0 / h_m)) * airmass;
    }
    let t_out = ray_sphere(p, l, top).y;
    let dt = max(t_out, 0.0) / f32(steps);
    var d = vec2<f32>(0.0);
    for (var j = 0; j < steps; j = j + 1) {
        let q = p + l * (f32(j) + 0.5) * dt;
        let h = max(length(q) - radius, 0.0);
        d += vec2<f32>(exp(-h / h_r), exp(-h / h_m)) * dt;
    }
    return d;
}

// 0 where the planet blocks the sun (soft edge), 1 in sunlight.
fn sun_visible(p: vec3<f32>, l: vec3<f32>, radius: f32) -> f32 {
    let along = dot(p, l);
    if (along >= 0.0) {
        return 1.0;
    }
    let perp = sqrt(max(dot(p, p) - along * along, 0.0));
    return smoothstep(radius * 0.985, radius * 1.03, perp);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    let center = pp.center.xyz;
    let radius = pp.center.w;
    let top = pp.atmo2.x;
    let cam = view.world_position - center;
    // The mesh is 1 % larger than the air: from outside only its front faces count, from
    // inside only its back faces, so every pixel is shaded once.
    let inside = length(cam) < top * 1.01;
    if (inside == is_front) {
        discard;
    }
    let rd = normalize(in.world_position.xyz - view.world_position);
    let l = pp.sun.xyz;

    let air = ray_sphere(cam, rd, top);
    var t0 = max(air.x, 0.0);
    var t1 = air.y;
    let ground = ray_sphere(cam, rd, radius + pp.shape.x * 0.25);
    let hits_ground = ground.x < ground.y && ground.x > 0.0;
    if (hits_ground) {
        t1 = min(t1, ground.x);
    }
    // Stop at whatever stands in front (buildings, units, ships, mountains): without this,
    // a tower sticking up over the horizon would get the haze of the whole limb.
    @if(DEPTH_PREPASS) {
        let depth = prepass_utils::prepass_depth(in.position, 0u);
        if (depth > 0.0) {
            let scene = position_ndc_to_world(vec3<f32>(frag_coord_to_ndc(in.position).xy, depth));
            t1 = min(t1, distance(view.world_position, scene));
        }
    }
    if (t1 <= t0) {
        discard;
    }

    let time = globals.time;
    let density = pp.atmo.w;
    let h_r = pp.atmo2.y;
    let h_m = h_r * 0.55;
    let beta_r = pp.atmo.rgb * pp.atmo.rgb * 0.07 * density;
    let beta_m = vec3<f32>(pp.atmo2.z * 0.03 * density);
    let sun = pp.sun_color.rgb * pp.sun.w * pp.atmo2.w;

    // Cloud layer hit in front of the ground (from above or, below the layer, from beneath).
    var cloud_alpha = 0.0;
    var cloud_col = vec3<f32>(0.0);
    var t_cloud = 1e9;
    if (pp.cloud.w > 0.0 && pp.cloud2.y > 0.001) {
        let cs = ray_sphere(cam, rd, pp.cloud2.x);
        var tc = cs.x;
        if (tc < 0.0) {
            tc = cs.y;
        }
        if (cs.x < cs.y && tc > 0.0 && tc < t1) {
            let pc = cam + rd * tc;
            let dir = normalize(pc);
            let local = quat_rotate_inv(pp.orient, dir);
            let dens = cloud_density(local, pp, time);
            if (dens > 0.0) {
                t_cloud = tc;
                // A thin layer seen edge-on (at the limb) would smear into streaks: fade it.
                let edge_on = smoothstep(0.03, 0.4, abs(dot(dir, rd)));
                cloud_alpha = clamp(dens * pp.cloud2.y * edge_on, 0.0, 0.97);
                let mu = dot(dir, l);
                // Thick parts are darker on the side away from the sun (cheap self-shadow).
                var self_shadow = 1.0;
                if (pp.quality.w >= 5.0) {
                    let ahead = quat_rotate_inv(pp.orient, normalize(dir + (l - dir * mu) * 0.04));
                    self_shadow = exp(-cloud_density(ahead, pp, time) * 1.1);
                }
                let lit = smoothstep(-0.12, 0.35, mu) * (0.45 + 0.55 * max(mu, 0.0));
                // Sunlight reaching the cloud tops is reddened near the terminator.
                let lds = light_depth(pc, l, radius, top, h_r, h_m, 0);
                let tr = exp(-(beta_r * lds.x + beta_m * 1.1 * lds.y));
                let silver = pow(max(dot(rd, l), 0.0), 8.0) * 0.6;
                // Sky light only on the day side; at night a little of the scene's ambient.
                let sky = pp.atmo.rgb * pp.sun.w * 0.025 * smoothstep(-0.25, 0.3, mu);
                cloud_col = pp.cloud.rgb * (sun * tr * (lit * self_shadow + silver) / PI + sky + lights.ambient_color.rgb * 0.35);
                // Thin parts are greyer than the dense cores.
                cloud_col *= 0.72 + 0.28 * dens;
            }
        }
    }

    // March the view ray.
    let steps = max(i32(pp.quality.y), 1);
    let lsteps = i32(pp.quality.z);
    let dt = (t1 - t0) / f32(steps);
    var od = vec2<f32>(0.0);
    var sum_r = vec3<f32>(0.0);
    var sum_m = vec3<f32>(0.0);
    var od_front = vec2<f32>(0.0);
    for (var i = 0; i < steps; i = i + 1) {
        let t = t0 + (f32(i) + 0.5) * dt;
        let p = cam + rd * t;
        let h = max(length(p) - radius, -pp.shape.x);
        let dr = exp(-h / h_r) * dt;
        let dm = exp(-h / h_m) * dt;
        od += vec2<f32>(dr, dm) * 0.5;
        let behind_cloud = select(1.0, 1.0 - cloud_alpha, t > t_cloud);
        if (t <= t_cloud) {
            od_front = od;
        }
        let vis = sun_visible(p, l, radius) * behind_cloud;
        if (vis > 0.0) {
            let ld = light_depth(p, l, radius, top, h_r, h_m, lsteps);
            let tau = beta_r * (od.x + ld.x) + beta_m * 1.1 * (od.y + ld.y);
            let att = exp(-tau) * vis;
            sum_r += att * dr;
            sum_m += att * dm;
        }
        od += vec2<f32>(dr, dm) * 0.5;
    }

    let mu = dot(rd, l);
    let phase_r = 3.0 / (16.0 * PI) * (1.0 + mu * mu);
    let g = 0.72;
    let phase_m = 3.0 / (8.0 * PI) * ((1.0 - g * g) * (1.0 + mu * mu)) / ((2.0 + g * g) * pow(1.0 + g * g - 2.0 * g * mu, 1.5));
    var inscatter = sun * (sum_r * beta_r * phase_r + sum_m * beta_m * phase_m);

    let trans = exp(-(beta_r * od.x + beta_m * 1.1 * od.y));
    let trans_front = exp(-(beta_r * od_front.x + beta_m * 1.1 * od_front.y));
    if (cloud_alpha > 0.0) {
        inscatter += cloud_col * cloud_alpha * trans_front;
    }
    let t_avg = dot(trans, vec3<f32>(0.3333));
    let alpha = 1.0 - (1.0 - cloud_alpha) * t_avg;
    return vec4<f32>(inscatter * view.exposure, alpha);
}
