//! Planet globes: a displaced icosphere with vertex colours from the biome palette, plus a thin
//! atmosphere shell. Heights come from `cc_core::system::Planet` so units and buildings stand
//! exactly on the drawn terrain.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use cc_core::rng::{Noise, smoothstep};
use cc_core::system::{Biome, Planet};

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn scale(c: [f32; 3], f: f32) -> [f32; 3] {
    [c[0] * f, c[1] * f, c[2] * f]
}

/// Biome palette: (deep sea, shallow sea, shore, lowland, highland, peak, polar).
struct Palette {
    deep: [f32; 3],
    shallow: [f32; 3],
    shore: [f32; 3],
    low: [f32; 3],
    high: [f32; 3],
    peak: [f32; 3],
    polar: [f32; 3],
}

fn palette(b: Biome) -> Palette {
    match b {
        Biome::Terran => Palette {
            deep: [0.02, 0.07, 0.22],
            shallow: [0.05, 0.22, 0.40],
            shore: [0.70, 0.64, 0.44],
            low: [0.16, 0.36, 0.11],
            high: [0.36, 0.38, 0.20],
            peak: [0.42, 0.38, 0.34],
            polar: [0.92, 0.94, 0.97],
        },
        Biome::Jungle => Palette {
            deep: [0.02, 0.14, 0.18],
            shallow: [0.05, 0.32, 0.32],
            shore: [0.45, 0.42, 0.25],
            low: [0.06, 0.26, 0.06],
            high: [0.10, 0.34, 0.08],
            peak: [0.30, 0.32, 0.22],
            polar: [0.55, 0.65, 0.50],
        },
        Biome::Desert => Palette {
            deep: [0.42, 0.24, 0.13],
            shallow: [0.55, 0.33, 0.18],
            shore: [0.80, 0.60, 0.36],
            low: [0.84, 0.64, 0.40],
            high: [0.70, 0.45, 0.26],
            peak: [0.52, 0.32, 0.20],
            polar: [0.90, 0.80, 0.65],
        },
        Biome::Ice => Palette {
            deep: [0.30, 0.45, 0.60],
            shallow: [0.55, 0.72, 0.84],
            shore: [0.80, 0.88, 0.94],
            low: [0.88, 0.92, 0.97],
            high: [0.62, 0.68, 0.76],
            peak: [0.45, 0.50, 0.58],
            polar: [0.97, 0.98, 1.0],
        },
        Biome::Volcanic => Palette {
            deep: [1.0, 0.30, 0.02],
            shallow: [0.95, 0.50, 0.05],
            shore: [0.10, 0.07, 0.06],
            low: [0.16, 0.13, 0.12],
            high: [0.26, 0.22, 0.20],
            peak: [0.36, 0.32, 0.30],
            polar: [0.45, 0.42, 0.40],
        },
        Biome::Barren => Palette {
            deep: [0.25, 0.24, 0.23],
            shallow: [0.30, 0.29, 0.28],
            shore: [0.38, 0.37, 0.35],
            low: [0.46, 0.45, 0.43],
            high: [0.56, 0.55, 0.52],
            peak: [0.66, 0.64, 0.61],
            polar: [0.75, 0.74, 0.72],
        },
        Biome::Toxic => Palette {
            deep: [0.25, 0.55, 0.05],
            shallow: [0.45, 0.80, 0.10],
            shore: [0.40, 0.38, 0.12],
            low: [0.40, 0.34, 0.16],
            high: [0.32, 0.22, 0.30],
            peak: [0.42, 0.32, 0.42],
            polar: [0.70, 0.75, 0.55],
        },
    }
}

/// Atmosphere tint per biome (None = no atmosphere).
pub fn atmosphere(b: Biome) -> Option<[f32; 3]> {
    match b {
        Biome::Terran => Some([0.35, 0.6, 1.0]),
        Biome::Jungle => Some([0.4, 0.85, 0.7]),
        Biome::Ice => Some([0.7, 0.85, 1.0]),
        Biome::Toxic => Some([0.6, 0.95, 0.3]),
        Biome::Volcanic => Some([1.0, 0.45, 0.2]),
        Biome::Desert => Some([1.0, 0.75, 0.5]),
        Biome::Barren => None,
    }
}

/// Surface colour for a direction.
fn color(p: &Planet, pal: &Palette, detail: &Noise, dir: Vec3, e: f32) -> [f32; 3] {
    let lat = dir.y.abs();
    let wobble = detail.fbm(dir.x * 9.0, dir.y * 9.0, dir.z * 9.0, 3);
    let mut c = match p.biome.sea_level() {
        Some(sea) if e < sea => mix(pal.shallow, pal.deep, (sea - e) * 3.0),
        Some(sea) if e < sea + 0.05 => pal.shore,
        _ => {
            let land = mix(pal.low, pal.high, smoothstep(0.0, 0.45, e + wobble * 0.15));
            mix(land, pal.peak, smoothstep(0.4, 0.75, e))
        }
    };
    // Polar caps and snowy peaks.
    let polar = smoothstep(0.80, 0.9, lat + wobble * 0.08) + smoothstep(0.75, 0.9, e) * 0.7;
    if !matches!(p.biome, Biome::Volcanic) || e > 0.0 {
        c = mix(c, pal.polar, polar);
    }
    // Craters / dark patches on barren worlds, colour variation everywhere.
    let var = 1.0 + wobble * 0.18;
    scale(c, var)
}

/// The displaced, coloured globe (positions relative to the planet's centre).
pub fn planet_mesh(p: &Planet, subdivisions: u32) -> Mesh {
    let mut mesh = Sphere::new(1.0).mesh().ico(subdivisions).expect("icosphere");
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        unreachable!("icosphere has positions")
    };
    let pal = palette(p.biome);
    let detail = Noise::new(p.seed ^ 0xC0105);
    let mut pos = Vec::with_capacity(positions.len());
    let mut col = Vec::with_capacity(positions.len());
    for v in positions {
        let dir = Vec3::from_array(*v).normalize();
        let e = p.elevation(dir);
        pos.push((dir * (p.radius + p.height(dir))).to_array());
        let c = color(p, &pal, &detail, dir, e);
        col.push([c[0], c[1], c[2], 1.0]);
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.compute_smooth_normals();
    mesh.asset_usage = RenderAssetUsages::RENDER_WORLD;
    mesh
}

/// Icosphere detail (Bevy subdivides each edge linearly; 79 is the maximum ≈ 64k vertices).
pub fn subdivisions_for(radius: f32) -> u32 {
    if radius >= 50.0 { 79 } else { 64 }
}
