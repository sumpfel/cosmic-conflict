//! The space scene: camera, star field, the sun and its light, and the home-screen backdrop.

use bevy::asset::RenderAssetUsages;
use bevy::camera::{Exposure, Hdr};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{CascadeShadowConfigBuilder, ShadowFilteringMethod};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use cc_core::rng::Rng;
use cc_core::system::{Biome, Planet, SUN_RADIUS};

use super::models::Models;
use super::planet_mesh::planet_mesh;
use super::planet_render::{PlanetAssets, SUN_COLOR, SunMaterial, SunShadowLight, spawn_planet_visual};
use crate::settings::Settings;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct StarField;

#[derive(Component)]
pub struct MenuBackdrop;

#[derive(Component)]
pub struct SunLight;

pub const STAR_DISTANCE: f32 = 40_000.0;

/// Thousands of small, coloured, unlit star quads on a huge sphere that follows the camera,
/// denser along a tilted "galactic" band.
fn star_mesh() -> Mesh {
    let mut rng = Rng::new(0x57A2);
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut col: Vec<[f32; 4]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let band = Vec3::new(0.25, 1.0, 0.35).normalize();
    for i in 0..9000 {
        let mut d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        if d.length_squared() > 1.0 || d.length_squared() < 1e-4 {
            continue;
        }
        d = d.normalize();
        if i % 5 < 2 {
            // Pull towards the band plane.
            let off = d.dot(band);
            d = (d - band * off * 0.85).normalize();
        }
        let c = d * STAR_DISTANCE;
        let t = d.any_orthonormal_vector();
        let b = d.cross(t);
        let bright = rng.f32().powf(6.0);
        let size = 10.0 + bright * 22.0;
        let temp = rng.f32();
        let tint = if temp < 0.2 {
            [1.0, 0.75, 0.55]
        } else if temp < 0.45 {
            [0.7, 0.8, 1.0]
        } else {
            [1.0, 0.97, 0.92]
        };
        let lum = 0.5 + bright * 25.0;
        let base = pos.len() as u32;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            pos.push((c + (t * sx + b * sy) * size).to_array());
            col.push([tint[0] * lum, tint[1] * lum, tint[2] * lum, 1.0]);
        }
        idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sun_materials: ResMut<Assets<SunMaterial>>,
) {
    // Anti-aliasing, bloom and the shadow filter follow the graphics settings
    // (`planet_render::apply_graphics`).
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Hdr,
        ShadowFilteringMethod::Gaussian,
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Projection::Perspective(PerspectiveProjection { fov: 0.8, near: 0.2, far: STAR_DISTANCE * 1.5, ..default() }),
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Exposure { ev100: 9.7 },
        Msaa::Sample4,
        Transform::from_xyz(0.0, 300.0, 1200.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.55, 0.65, 1.0), brightness: 120.0, ..default() });

    commands.spawn((
        StarField,
        Mesh3d(meshes.add(star_mesh())),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() })),
        Transform::default(),
        bevy::light::NotShadowCaster,
    ));

    // The sun: a boiling emissive sphere, a soft corona and the light of the whole system.
    let sun_color = Vec3::new(90.0, 55.0, 20.0);
    let sun_mat = sun_materials.add(SunMaterial::surface(sun_color));
    let corona = sun_materials.add(SunMaterial::corona(sun_color, SUN_RADIUS, 0.035));
    let sphere = meshes.add(Sphere::new(1.0).mesh().ico(48).expect("ico"));
    commands
        .spawn((
            Mesh3d(sphere.clone()),
            MeshMaterial3d(sun_mat),
            Transform::from_scale(Vec3::splat(SUN_RADIUS)),
            bevy::light::NotShadowCaster,
        ))
        .with_children(|c| {
            // Must not cast: the directional shadow light starts in the sun.
            c.spawn((Mesh3d(sphere), MeshMaterial3d(corona), Transform::from_scale(Vec3::splat(3.2)), bevy::light::NotShadowCaster));
        });
    commands.spawn((
        SunLight,
        PointLight {
            color: SUN_COLOR,
            intensity: 1.6e10,
            range: 100_000.0,
            radius: SUN_RADIUS,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default(),
    ));
    // Sunlight with shadows for the planet in view (aimed and faded by `planet_render`).
    commands.spawn((
        SunShadowLight,
        DirectionalLight { color: SUN_COLOR, illuminance: 0.0, shadow_maps_enabled: false, ..default() },
        Transform::default(),
        CascadeShadowConfigBuilder { num_cascades: 3, minimum_distance: 0.1, maximum_distance: 200.0, first_cascade_far_bound: 40.0, overlap_proportion: 0.25 }
            .build(),
    ));
}

/// The star field is "infinitely" far away: it follows the camera.
pub fn follow_stars(cam: Query<&GlobalTransform, With<MainCamera>>, mut stars: Query<&mut Transform, With<StarField>>) {
    let Ok(cam) = cam.single() else { return };
    for mut t in &mut stars {
        t.translation = cam.translation();
    }
}

const BACKDROP: Vec3 = Vec3::new(1100.0, 0.0, 0.0);

/// A lone home world filling the right half of the home screen. Runs every frame on the home
/// screen and spawns the backdrop when it is missing (the models are created at start-up).
#[allow(clippy::too_many_arguments)]
pub fn ensure_menu_backdrop(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut planet_assets: PlanetAssets,
    models: Option<Res<Models>>,
    settings: Res<Settings>,
    existing: Query<Entity, With<MenuBackdrop>>,
    mut built_with: Local<u32>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(models) = models else { return };
    let subdivisions = settings.graphics.planet_subdivisions(60.0);
    if !existing.is_empty() {
        // Rebuild when the planet mesh setting changes.
        if *built_with != subdivisions {
            for e in &existing {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    let planet = Planet {
        id: 0,
        name: "Backdrop".into(),
        center: BACKDROP,
        radius: 60.0,
        biome: Biome::Terran,
        seed: 1234,
        home: None,
        start_dir: Vec3::Y,
        relief: 2.4,
    };
    let mesh = meshes.add(planet_mesh(&planet, subdivisions));
    spawn_planet_visual(&mut commands, &mut planet_assets, models.sphere.clone(), &planet, mesh, MenuBackdrop);
    *built_with = subdivisions;
    if let Ok(mut t) = cam.single_mut() {
        *t = Transform::from_translation(BACKDROP + Vec3::new(10.0, 55.0, 245.0)).looking_at(BACKDROP + Vec3::new(-85.0, 10.0, 0.0), Vec3::Y);
    }
}

pub fn despawn_menu_backdrop(mut commands: Commands, q: Query<Entity, With<MenuBackdrop>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

pub fn spin_backdrop(time: Res<Time>, mut q: Query<&mut Transform, With<MenuBackdrop>>) {
    for mut t in &mut q {
        t.rotate_y(time.delta_secs() * 0.02);
    }
}
