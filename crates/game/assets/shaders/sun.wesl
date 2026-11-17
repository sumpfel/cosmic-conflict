// The sun. Surface mode: boiling granulation, slow dark spots and limb darkening on a hot
// emissive sphere. Corona mode (a large additive shell around it): glow that falls off with the
// view ray's closest distance to the sun's centre, with faint streamers. Bloom does the rest.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, globals},
};
import super::planet_common::{gnoise, fbm};

struct SunParams {
    // rgb: emissive colour (HDR), w: surface detail (0 = plain)
    color: vec4<f32>,
    // x: 0 = surface, 1 = corona; y: sun radius; z: corona strength
    shape: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> sun: SunParams;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    let t = globals.time;
    if (sun.shape.x > 0.5) {
        // Corona: the sun sits at the origin.
        let shell = length(in.world_position.xyz);
        let inside = length(view.world_position) < shell;
        if (inside == is_front) {
            discard;
        }
        let ro = view.world_position;
        let rd = normalize(in.world_position.xyz - ro);
        let tca = max(-dot(ro, rd), 0.0);
        let closest = ro + rd * tca;
        let r = length(closest) / sun.shape.y;
        let x = max(r - 1.0, 0.0);
        let dir = normalize(closest);
        let streamers = fbm(dir * 5.0 + vec3<f32>(0.0, t * 0.01, 0.0), 3);
        let glow = exp(-x * 5.5) * 0.8 + exp(-x * 1.6) * 0.2 * (0.6 + 0.8 * max(streamers, 0.0));
        // Fade at the shell so its edge never shows.
        let edge = 1.0 - smoothstep(0.65, 1.0, length(closest) / shell);
        let c = sun.color.rgb * vec3<f32>(1.0, 0.8, 0.6) * glow * edge * sun.shape.z;
        // Bevy draws `AlphaMode::Add` with premultiplied blending: alpha 0 = purely additive.
        return vec4<f32>(c, 0.0);
    }
    let n = normalize(in.world_normal);
    let v = normalize(view.world_position - in.world_position.xyz);
    let mu = clamp(dot(n, v), 0.0, 1.0);
    // Limb darkening (the edge is cooler and redder).
    let limb = 0.35 + 0.65 * pow(mu, 0.55);
    var c = sun.color.rgb * limb * mix(vec3<f32>(1.0, 0.7, 0.45), vec3<f32>(1.0), mu);
    if (sun.color.w > 0.0) {
        let gran = fbm(n * 18.0 + vec3<f32>(0.0, t * 0.05, t * 0.03), 4);
        let cells = gnoise(n * 55.0 + vec3<f32>(t * 0.12, 0.0, 0.0));
        let spots = smoothstep(0.32, 0.42, gnoise(n * 3.2 + vec3<f32>(t * 0.004, 0.0, 0.0)));
        c *= (0.82 + 0.35 * gran + 0.08 * cells) * (1.0 - 0.6 * spots);
    }
    return vec4<f32>(c, 1.0);
}
