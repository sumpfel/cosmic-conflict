//! The space scene: camera, star field, the sun and its light, and the home-screen backdrop.

use bevy::asset::RenderAssetUsages;
use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use cc_core::rng::Rng;
use cc_core::system::{Biome, Planet, SUN_RADIUS};

use super::models::Models;
use super::planet_mesh::{atmosphere, planet_mesh};

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct StarField;

#[derive(Component)]
pub struct MenuBackdrop;

#[derive(Component)]
pub struct SunLight;

/// Illuminance (lux) the sun gives the planet in focus; other planets fall off physically.
const FOCUS_LUX: f32 = 2400.0;

/// Scales the sun so the planet the camera looks at is always well exposed (planets sit at
/// 400–1450 m from the sun; with a fixed intensity far systems would be too dark).
pub fn adjust_sunlight(
    cam: Option<Res<super::camera::OrbitCam>>,
    world: Option<Res<super::world::ClientWorld>>,
    backdrop: Query<&GlobalTransform, With<MenuBackdrop>>,
    mut light: Query<&mut PointLight, With<SunLight>>,
) {
    let dist = if let Some(t) = backdrop.iter().next() {
        t.translation().length()
    } else if let (Some(cam), Some(world)) = (cam, world) {
        match cam.focus.and_then(|f| world.system.planet(f)) {
            Some(p) => p.center.length(),
            None => {
                let n = world.system.planets.len().max(1) as f32;
                world.system.planets.iter().map(|p| p.center.length()).sum::<f32>() / n
            }
        }
    } else {
        1000.0
    };
    let target = FOCUS_LUX * 4.0 * std::f32::consts::PI * dist * dist;
    for mut l in &mut light {
        // Ease towards the target so focus changes do not flash.
        l.intensity += (target - l.intensity) * 0.08;
    }
}

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

pub fn setup_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        MainCamera,
        Camera3d::default(),
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

    // The sun: a glowing sphere, a soft corona and the light of the whole system.
    let sun_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(90.0, 55.0, 20.0),
        ..default()
    });
    let corona = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.6, 0.25, 0.08),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    });
    let sphere = meshes.add(Sphere::new(1.0).mesh().ico(48).expect("ico"));
    commands
        .spawn((
            Mesh3d(sphere.clone()),
            MeshMaterial3d(sun_mat),
            Transform::from_scale(Vec3::splat(SUN_RADIUS)),
            bevy::light::NotShadowCaster,
        ))
        .with_children(|c| {
            for (s, _) in [(1.15, 0), (1.4, 1)] {
                c.spawn((Mesh3d(sphere.clone()), MeshMaterial3d(corona.clone()), Transform::from_scale(Vec3::splat(s))));
            }
        });
    commands.spawn((
        SunLight,
        PointLight {
            color: Color::srgb(1.0, 0.93, 0.85),
            intensity: 1.6e10,
            range: 100_000.0,
            radius: SUN_RADIUS,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default(),
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
pub fn ensure_menu_backdrop(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    models: Option<Res<Models>>,
    existing: Query<(), With<MenuBackdrop>>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(models) = models else { return };
    if !existing.is_empty() {
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
    spawn_planet(&mut commands, &mut meshes, &mut materials, &models, &planet, 79, MenuBackdrop);
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

/// Spawns a planet globe (and its atmosphere) with `marker`. Returns the entity.
pub fn spawn_planet(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    models: &Models,
    p: &Planet,
    subdivisions: u32,
    marker: impl Component,
) -> Entity {
    let mesh = meshes.add(planet_mesh(p, subdivisions));
    spawn_planet_with_mesh(commands, materials, models, p, mesh, marker)
}

pub fn spawn_planet_with_mesh(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    models: &Models,
    p: &Planet,
    mesh: Handle<Mesh>,
    marker: impl Component,
) -> Entity {
    let e = commands.spawn((marker, Mesh3d(mesh), MeshMaterial3d(models.planet.clone()), Transform::from_translation(p.center))).id();
    if let Some(c) = atmosphere(p.biome) {
        // Back faces of a few slightly larger spheres: a soft halo around the silhouette that
        // fades outwards, never a film over the surface.
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(c[0] * 0.16, c[1] * 0.16, c[2] * 0.16, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: Some(bevy::render::render_resource::Face::Front),
            ..default()
        });
        commands.entity(e).with_children(|ch| {
            for k in 1..=5 {
                ch.spawn((
                    Mesh3d(models.sphere.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_scale(Vec3::splat(p.radius * (1.0 + 0.008 * k as f32) + p.relief * 0.6)),
                    bevy::light::NotShadowCaster,
                ));
            }
        });
    }
    e
}
