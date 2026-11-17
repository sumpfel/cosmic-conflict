// Planet surface. The mesh brings the biome colours (vertex colour) and, in UV0, the baked
// cavity (x, metres: < 0 in hollows) and the sea mask (y). On top of that, per pixel:
// detail noise and bump mapping in metres, rock on steep slopes, exaggerated relief shading,
// water with sun glint and waves, ice, glowing lava, acid; lit by the real sun direction with
// atmospheric reddening near the terminator, sky light, cloud shadows and the directional
// shadow map (mountains, buildings and units cast shadows on the focused planet).
//
// Quality (`pp.quality.x`): 1 plain, 2 detail, 3 + bump / waves / cloud shadows, 4 + finest.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, lights, globals},
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    shadows::fetch_directional_shadow,
};
import super::planet_common::{
    PlanetParams, PI, quat_rotate, quat_rotate_inv, gnoise, gnoise_d, fbm, fbm_d, cloud_density,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> pp: PlanetParams;

fn d_ggx(a: f32, n_dot_h: f32) -> f32 {
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

fn v_smith(a: f32, n_dot_v: f32, n_dot_l: f32) -> f32 {
    let a2 = a * a;
    let lv = n_dot_l * sqrt((n_dot_v - a2 * n_dot_v) * n_dot_v + a2);
    let ll = n_dot_v * sqrt((n_dot_l - a2 * n_dot_l) * n_dot_l + a2);
    return 0.5 / max(lv + ll, 1e-5);
}

// Removes the component of `g` along `n` (a height gradient becomes a surface tilt).
fn tangential(g: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    return g - n * dot(g, n);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let quality = max(u32(pp.quality.x + 0.5), 1u);
    let time = globals.time;
    let pos = in.world_position.xyz;
    let center = pp.center.xyz;
    let radius = pp.center.w;
    let up = normalize(pos - center);
    // Local position in metres (stable while the menu globe turns) and the pixel footprint.
    let lp = quat_rotate_inv(pp.orient, pos - center);
    let footprint = max(length(dpdx(lp)), length(dpdy(lp)));

    let v = normalize(view.world_position - pos);
    let l = pp.sun.xyz;
    let ng = normalize(in.world_normal);
    let sea = smoothstep(0.3, 0.7, in.uv.y);
    let land = 1.0 - sea;
    let cavity = in.uv.x;
    let sea_kind = u32(pp.shape.z + 0.5);
    let seed = vec3<f32>(pp.rock.w, pp.rock.w * 0.71, -pp.rock.w * 0.43);

    var albedo = in.color.rgb;
    var n = ng;
    var rough = 0.9;
    var f0 = 0.03;
    var emissive = vec3<f32>(0.0);
    var ao = 1.0;

    // ------------------------------------------------------------ land
    if (quality >= 2u) {
        // Exaggerate the terrain's own slopes in the shading: the relief is only a few metres on
        // a 60 m globe and reads flat otherwise.
        n = normalize(up + (ng - up) * 2.2);

        let macro_n = fbm(lp * 0.045 + seed, 3);
        let octaves = select(3, select(4, 6, quality >= 4u), quality >= 3u);
        let detail = fbm_d(lp * 0.32 + seed * 1.7, octaves, footprint * 0.32);
        // Colour breakup at several scales.
        albedo *= 1.0 + 0.28 * macro_n + 0.16 * detail.x;

        // Rock on steep slopes, with darker strata.
        let slope = length(tangential(ng, up));
        let rockiness = smoothstep(0.07, 0.2, slope + detail.x * 0.04) * land;
        let h = dot(pos - center, up) - radius;
        let strata = 0.82 + 0.18 * sin(h * 4.5 + detail.x * 3.0);
        albedo = mix(albedo, pp.rock.rgb * strata * (1.0 + 0.2 * macro_n), rockiness);

        // Baked cavity: hollows darker, ridges a bit lighter.
        ao = clamp(1.0 + cavity * 0.9, 0.5, 1.12);

        if (quality >= 3u) {
            // Bump mapping from the same detail noise (height amplitude ~0.5 m, more on rock).
            let g_local = detail.yzw * 0.32 * (0.45 + 0.6 * rockiness);
            let g = quat_rotate(pp.orient, g_local);
            n = normalize(n - tangential(g, n) * land);
        }

        // Glowing fissures on volcanic land: thin, branching (domain-warped contour lines of
        // two noise octaves), uneven in heat, with a dark cooled rim; lowlands only.
        if (pp.shape.w > 0.5) {
            let q = lp * 0.07 + seed;
            let warp = vec3<f32>(gnoise(q * 1.7 + vec3<f32>(4.0, 0.0, 0.0)), gnoise(q * 1.7 + vec3<f32>(0.0, 7.0, 0.0)), 0.0);
            let field = gnoise(q + warp * 0.6) + 0.35 * gnoise(q * 3.1 - warp);
            let width = 0.012 + 0.012 * smoothstep(-0.3, 0.5, gnoise(q * 0.6 + vec3<f32>(9.0, 2.0, 5.0)));
            let line = abs(field);
            let crack = 1.0 - smoothstep(width * 0.4, width, line);
            let rim = 1.0 - smoothstep(width, width * 3.5, line);
            let low = 1.0 - smoothstep(-0.25, 0.25, h / max(pp.shape.x, 0.01));
            let heat = smoothstep(-0.2, 0.4, gnoise(q * 2.3 + vec3<f32>(0.0, time * 0.05, 0.0)));
            let pulse = 0.8 + 0.2 * sin(time * 1.1 + lp.x * 0.15 + lp.z * 0.13);
            emissive += vec3<f32>(3.0, 0.62, 0.07) * crack * low * heat * pulse * land;
            albedo *= 1.0 - 0.55 * rim * low;
        }
    }

    // ------------------------------------------------------------ seas
    if (sea > 0.0) {
        var s_albedo = in.color.rgb;
        var s_n = up;
        var s_rough = 0.08;
        var s_f0 = 0.02;
        var s_emissive = vec3<f32>(0.0);
        if (sea_kind == 1u || sea_kind == 4u) {
            // Water / acid: waves from two scrolling noise layers.
            if (quality >= 3u) {
                let w1 = gnoise_d(lp * 0.45 + vec3<f32>(time * 0.11, 0.0, time * 0.07));
                let w2 = gnoise_d(lp * 1.7 + vec3<f32>(-time * 0.23, time * 0.05, time * 0.19));
                let fade = 1.0 - smoothstep(0.3, 1.2, footprint * 1.7);
                let g = quat_rotate(pp.orient, w1.yzw * 0.45 * 0.10 + w2.yzw * 1.7 * 0.025 * fade);
                s_n = normalize(up - tangential(g, up));
            } else if (quality >= 2u) {
                let w = gnoise_d(lp * 0.45 + vec3<f32>(time * 0.11, 0.0, time * 0.07));
                s_n = normalize(up - tangential(quat_rotate(pp.orient, w.yzw * 0.045), up));
            }
            if (sea_kind == 4u) {
                s_rough = 0.14;
                let swirl = 0.5 + 0.5 * gnoise(lp * 0.2 + vec3<f32>(0.0, time * 0.05, 0.0));
                s_emissive = in.color.rgb * vec3<f32>(0.05, 0.12, 0.02) * (0.6 + swirl);
            }
            // Foam along the shore.
            if (quality >= 2u) {
                let shore = smoothstep(0.92, 0.55, in.uv.y);
                let foam = shore * smoothstep(0.1, 0.5, gnoise(lp * 2.5 + vec3<f32>(time * 0.3, 0.0, 0.0)) + 0.3);
                s_albedo = mix(s_albedo, vec3<f32>(0.8, 0.85, 0.88), foam * 0.6);
                s_rough = mix(s_rough, 0.6, foam);
            }
        } else if (sea_kind == 2u) {
            // Frozen sea: rough ice with darker cracks.
            s_rough = 0.32;
            s_f0 = 0.025;
            if (quality >= 2u) {
                let cracks = smoothstep(0.0, 0.05, abs(gnoise(lp * 0.6 + seed)));
                let cracks2 = smoothstep(0.0, 0.03, abs(gnoise(lp * 2.1 - seed)));
                s_albedo *= (0.78 + 0.22 * cracks) * (0.9 + 0.1 * cracks2);
                s_albedo *= 1.0 + 0.1 * fbm(lp * 0.2 + seed, 2);
            }
        } else if (sea_kind == 3u) {
            // Lava: dark cooling crust drifting over a glowing melt.
            let flow = vec3<f32>(time * 0.05, time * 0.013, -time * 0.03);
            var crust = 0.0;
            var heat = 1.0;
            if (quality >= 2u) {
                crust = smoothstep(-0.12, 0.22, fbm(lp * 0.22 + flow + seed, 4));
                heat = 0.7 + 0.5 * gnoise(lp * 0.8 - flow * 3.0);
            }
            s_albedo = mix(vec3<f32>(0.25, 0.06, 0.02), vec3<f32>(0.05, 0.04, 0.04), crust);
            s_rough = mix(0.45, 0.85, crust);
            s_emissive = vec3<f32>(5.0, 1.25, 0.12) * heat * (1.0 - crust * 0.92);
        }
        albedo = mix(albedo, s_albedo, sea);
        n = normalize(mix(n, s_n, sea));
        rough = mix(rough, s_rough, sea);
        f0 = mix(f0, s_f0, sea);
        emissive = mix(emissive, s_emissive, sea);
        ao = mix(ao, 1.0, sea);
    }

    // ------------------------------------------------------------ light
    let mu_s = dot(up, l);
    // The planet's own night side (bumped normals must not light it).
    let day = smoothstep(-0.03, 0.06, mu_s);
    var sun = pp.sun_color.rgb * pp.sun.w;
    let atmo = pp.atmo.w;
    if (atmo > 0.0) {
        // Sunlight crosses more air near the terminator and turns orange / red.
        let airmass = 1.0 / (max(mu_s, 0.0) * 0.9 + 0.075);
        let ext = pp.atmo.rgb * pp.atmo.rgb * 0.11 * atmo;
        sun *= exp(-ext * airmass);
    }

    var shadow = 1.0;
    let shadow_mix = pp.sun_color.w;
    if (shadow_mix > 0.001 && lights.n_directional_lights > 0u
        && (lights.directional_lights[0].flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u) {
        let view_z = dot(vec4<f32>(
            view.view_from_world[0].z,
            view.view_from_world[1].z,
            view.view_from_world[2].z,
            view.view_from_world[3].z
        ), vec4<f32>(pos, 1.0));
        let s = fetch_directional_shadow(0u, vec4<f32>(pos, 1.0), ng, view_z, in.position.xy);
        shadow = mix(1.0, s, shadow_mix);
    }
    // Cloud shadows: the cloud above this point, seen towards the sun.
    if (quality >= 3u && pp.cloud.w > 0.0 && pp.cloud2.z > 0.0 && mu_s > -0.1) {
        let h_c = pp.cloud2.x - length(pos - center);
        let towards_sun = (pos - center) + l * (h_c / max(mu_s, 0.12));
        let cd = cloud_density(quat_rotate_inv(pp.orient, normalize(towards_sun)), pp, time);
        shadow *= 1.0 - cd * pp.cloud2.z;
    }

    let n_dot_l = clamp(dot(n, l), 0.0, 1.0) * day;
    let n_dot_v = max(dot(n, v), 1e-3);
    let hv = normalize(l + v);
    let n_dot_h = clamp(dot(n, hv), 0.0, 1.0);
    let v_dot_h = clamp(dot(v, hv), 0.0, 1.0);
    let a = max(rough * rough, 0.002);
    let fresnel = f0 + (1.0 - f0) * pow(1.0 - v_dot_h, 5.0);
    let spec = d_ggx(a, n_dot_h) * v_smith(a, n_dot_v, n_dot_l) * fresnel;
    let diffuse = albedo * (1.0 - fresnel) / PI;
    let direct = (diffuse * ao + vec3<f32>(spec)) * sun * n_dot_l * shadow;

    // Sky light: on the day side of a planet with air the shadows are blue, not black.
    var sky = vec3<f32>(0.0);
    if (atmo > 0.0) {
        let sky_amount = smoothstep(-0.2, 0.5, mu_s) * min(atmo, 1.5);
        sky = albedo * ao * pp.atmo.rgb * pp.sun.w * 0.045 * sky_amount;
        // Water reflects the sky at grazing angles.
        let f_view = f0 + (1.0 - f0) * pow(1.0 - n_dot_v, 5.0);
        sky += pp.atmo.rgb * pp.sun.w * 0.03 * sky_amount * f_view * sea;
    }
    // The night side gets less of the scene's ambient than units do: a dark night makes the
    // terminator read, units and buildings stay visible on it.
    let ambient = lights.ambient_color.rgb * albedo * ao * 0.5;

    let color = (direct + sky + ambient) * view.exposure + emissive;
    return vec4<f32>(color, 1.0);
}
