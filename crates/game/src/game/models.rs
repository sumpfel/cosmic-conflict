//! Procedural models for units, buildings and resource deposits, built from Bevy primitives.
//!
//! Local frame: +Y is "up" (the surface normal for things on a planet), −Z is forward.
//! Every part uses a material *slot*; `Team` becomes the owner's colour at spawn time.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use cc_core::{PLAYER_COLORS, Res};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Hull,
    Dark,
    Glass,
    Glow,
    Team,
    Ore,
    Crystal,
    Gas,
    Haze,
}

#[derive(Clone)]
pub struct Part {
    pub mesh: Handle<Mesh>,
    pub slot: Slot,
    pub transform: Transform,
}

#[derive(Resource)]
pub struct Models {
    mats: HashMap<Slot, Handle<StandardMaterial>>,
    team: Vec<Handle<StandardMaterial>>,
    team_glow: Vec<Handle<StandardMaterial>>,
    pub ghost_ok: Handle<StandardMaterial>,
    pub ghost_bad: Handle<StandardMaterial>,
    pub planet: Handle<StandardMaterial>,
    pub sphere: Handle<Mesh>,
    models: HashMap<String, Vec<Part>>,
    deposits: HashMap<Res, Vec<Part>>,
}

/// Visual size factor for units (the simulation radii are small next to a 60 m planet).
pub const UNIT_SCALE: f32 = 1.4;

fn mat(m: &mut Assets<StandardMaterial>, base: Color, metallic: f32, rough: f32) -> Handle<StandardMaterial> {
    m.add(StandardMaterial { base_color: base, metallic, perceptual_roughness: rough, ..default() })
}

fn glow(m: &mut Assets<StandardMaterial>, c: LinearRgba) -> Handle<StandardMaterial> {
    m.add(StandardMaterial { base_color: Color::BLACK, emissive: c, unlit: false, ..default() })
}

pub fn team_color(i: u8) -> Color {
    let (r, g, b) = PLAYER_COLORS[i as usize % PLAYER_COLORS.len()];
    Color::srgb_u8(r, g, b)
}

struct Kit<'a> {
    meshes: &'a mut Assets<Mesh>,
    parts: Vec<Part>,
}

impl Kit<'_> {
    fn add(&mut self, mesh: impl Into<Mesh>, slot: Slot, transform: Transform) -> &mut Self {
        let mesh = self.meshes.add(mesh.into());
        self.parts.push(Part { mesh, slot, transform });
        self
    }
    fn at(x: f32, y: f32, z: f32) -> Transform {
        Transform::from_xyz(x, y, z)
    }
    fn take(&mut self) -> Vec<Part> {
        std::mem::take(&mut self.parts)
    }
}

fn ball(r: f32) -> Mesh {
    Sphere::new(r).mesh().ico(8).expect("ico")
}

fn facet(r: f32) -> Mesh {
    Sphere::new(r).mesh().ico(0).expect("ico")
}

/// Z-aligned cylinder transform helper.
fn along_z(t: Transform) -> Transform {
    t.with_rotation(Quat::from_rotation_x(FRAC_PI_2))
}

fn build_models(meshes: &mut Assets<Mesh>) -> HashMap<String, Vec<Part>> {
    use Slot::*;
    let at = Kit::at;
    let mut k = Kit { meshes, parts: Vec::new() };
    let mut m = HashMap::new();

    // --- Units -------------------------------------------------------------------------------
    k.add(ball(0.45), Hull, at(0.0, 0.85, 0.0))
        .add(Torus::new(0.45, 0.62), Team, at(0.0, 0.85, 0.0))
        .add(ball(0.16), Glow, at(0.0, 0.85, -0.42))
        .add(Cylinder::new(0.06, 0.5), Dark, at(0.0, 1.35, 0.0))
        .add(ball(0.18), Glow, at(0.0, 0.3, 0.0));
    m.insert("drone".into(), k.take());

    k.add(Capsule3d::new(0.22, 0.6), Dark, at(0.0, 0.62, 0.0))
        .add(Cuboid::new(0.5, 0.42, 0.32), Team, at(0.0, 0.85, 0.0))
        .add(ball(0.2), Glass, at(0.0, 1.3, 0.0))
        .add(Cuboid::new(0.08, 0.08, 0.8), Dark, at(0.27, 0.85, -0.3));
    m.insert("trooper".into(), k.take());

    k.add(Cuboid::new(2.4, 0.3, 3.2), Dark, at(0.0, 0.35, 0.0))
        .add(Cuboid::new(2.1, 0.7, 2.9), Hull, at(0.0, 0.8, 0.0))
        .add(Cuboid::new(1.3, 0.5, 1.4), Team, at(0.0, 1.4, 0.2))
        .add(Cylinder::new(0.12, 2.0), Dark, along_z(at(0.0, 1.4, -1.3)))
        .add(Cuboid::new(2.0, 0.05, 2.7), Glow, at(0.0, 0.15, 0.0));
    m.insert("tank".into(), k.take());

    k.add(Cylinder::new(0.15, 1.2), Dark, at(-0.45, 0.6, 0.0))
        .add(Cylinder::new(0.15, 1.2), Dark, at(0.45, 0.6, 0.0))
        .add(Cuboid::new(1.3, 0.7, 1.1), Team, at(0.0, 1.45, 0.0))
        .add(Cylinder::new(0.1, 1.6), Hull, at(-0.3, 2.2, -0.3).with_rotation(Quat::from_rotation_x(-0.5)))
        .add(Cylinder::new(0.1, 1.6), Hull, at(0.3, 2.2, -0.3).with_rotation(Quat::from_rotation_x(-0.5)));
    m.insert("flak".into(), k.take());

    k.add(Cuboid::new(0.7, 0.5, 3.2), Hull, at(0.0, 0.0, 0.0))
        .add(Cuboid::new(3.4, 0.1, 1.1), Team, at(0.0, 0.0, 0.45))
        .add(Cuboid::new(0.1, 0.8, 0.8), Team, at(0.0, 0.45, 1.2))
        .add(ball(0.3), Glass, at(0.0, 0.3, -0.7))
        .add(ball(0.28), Glow, at(0.0, 0.0, 1.65));
    m.insert("fighter".into(), k.take());

    k.add(Cuboid::new(1.6, 1.0, 5.0), Hull, at(0.0, 0.0, 0.0))
        .add(Cuboid::new(5.2, 0.2, 1.8), Team, at(0.0, 0.0, 0.6))
        .add(Cylinder::new(0.4, 2.2), Dark, along_z(at(-1.8, 0.0, 0.8)))
        .add(Cylinder::new(0.4, 2.2), Dark, along_z(at(1.8, 0.0, 0.8)))
        .add(ball(0.38), Glow, at(-1.8, 0.0, 1.95))
        .add(ball(0.38), Glow, at(1.8, 0.0, 1.95))
        .add(ball(0.45), Glass, at(0.0, 0.45, -1.8));
    m.insert("bomber".into(), k.take());

    k.add(Cylinder::new(1.2, 6.0), Hull, along_z(at(0.0, 0.0, 0.0)))
        .add(Torus::new(1.6, 2.3), Team, along_z(at(0.0, 0.0, 0.5)))
        .add(ball(1.15), Glass, at(0.0, 0.0, -3.0))
        .add(ball(0.7), Glow, at(0.0, 0.0, 3.1));
    m.insert("colony_ship".into(), k.take());

    // --- Buildings ---------------------------------------------------------------------------
    k.add(Cylinder::new(6.5, 1.0), Dark, at(0.0, 0.5, 0.0))
        .add(Cylinder::new(4.5, 3.0), Hull, at(0.0, 2.5, 0.0))
        .add(Cylinder::new(4.6, 0.5), Team, at(0.0, 3.3, 0.0))
        .add(Cylinder::new(1.8, 5.0), Hull, at(0.0, 6.0, 0.0))
        .add(ball(2.6), Glass, at(0.0, 4.0, 0.0))
        .add(Cylinder::new(0.15, 4.0), Dark, at(0.0, 10.0, 0.0))
        .add(ball(0.35), Glow, at(0.0, 12.1, 0.0))
        .add(Cuboid::new(1.4, 0.6, 3.0), Team, at(5.0, 1.2, 0.0))
        .add(Cuboid::new(1.4, 0.6, 3.0), Team, at(-5.0, 1.2, 0.0));
    m.insert("command_center".into(), k.take());

    k.add(Cylinder::new(4.6, 0.8), Dark, at(0.0, 0.4, 0.0))
        .add(Cylinder::new(3.0, 2.0), Hull, at(0.0, 1.8, 0.0))
        .add(Cylinder::new(3.1, 0.4), Team, at(0.0, 2.3, 0.0))
        .add(ball(2.1), Glass, at(0.0, 2.8, 0.0))
        .add(Cylinder::new(0.12, 3.0), Dark, at(0.0, 6.0, 0.0))
        .add(ball(0.3), Glow, at(0.0, 7.6, 0.0));
    m.insert("outpost".into(), k.take());

    k.add(Cylinder::new(2.2, 0.6), Dark, at(0.0, 0.3, 0.0))
        .add(Cuboid::new(1.0, 5.0, 1.0), Hull, at(0.0, 3.0, 0.0))
        .add(Cuboid::new(1.6, 0.6, 1.6), Team, at(0.0, 5.2, 0.0))
        .add(Cylinder::new(0.18, 4.0), Dark, at(1.0, 2.0, 1.0).with_rotation(Quat::from_rotation_z(0.25)))
        .add(Cylinder::new(0.18, 4.0), Dark, at(-1.0, 2.0, -1.0).with_rotation(Quat::from_rotation_z(-0.25)))
        .add(ball(0.25), Glow, at(0.0, 5.8, 0.0))
        .add(Cylinder::new(1.4, 0.4), Team, at(0.0, 0.8, 0.0));
    m.insert("extractor".into(), k.take());

    k.add(Cylinder::new(3.8, 0.6), Dark, at(0.0, 0.3, 0.0))
        .add(ball(3.3), Glass, at(0.0, 0.6, 0.0).with_scale(Vec3::new(1.0, 0.75, 1.0)))
        .add(Torus::new(3.3, 3.8), Team, at(0.0, 0.65, 0.0));
    m.insert("habitat".into(), k.take());

    k.add(Cuboid::new(8.0, 3.5, 6.0), Hull, at(0.0, 1.75, 0.0))
        .add(Cuboid::new(8.3, 0.4, 6.3), Dark, at(0.0, 3.6, 0.0))
        .add(Cuboid::new(8.1, 0.5, 6.1), Team, at(0.0, 2.6, 0.0))
        .add(Cylinder::new(0.5, 3.0), Dark, at(2.5, 4.8, 1.5))
        .add(Cylinder::new(0.5, 3.0), Dark, at(1.0, 4.8, 1.5))
        .add(Cuboid::new(3.0, 2.2, 0.2), Glow, at(0.0, 1.1, -3.0));
    m.insert("factory".into(), k.take());

    k.add(Cuboid::new(11.0, 0.8, 8.0), Dark, at(0.0, 0.4, 0.0))
        .add(Cylinder::new(3.0, 0.2), Team, at(0.0, 0.9, 0.0))
        .add(Cuboid::new(1.0, 8.0, 1.0), Hull, at(4.5, 4.4, 0.0))
        .add(Cuboid::new(1.0, 8.0, 1.0), Hull, at(-4.5, 4.4, 0.0))
        .add(Cuboid::new(10.0, 0.8, 1.0), Hull, at(0.0, 8.4, 0.0))
        .add(Torus::new(2.6, 3.0), Glow, at(0.0, 5.0, 0.0))
        .add(Cuboid::new(1.2, 0.5, 1.2), Team, at(4.5, 8.9, 0.0))
        .add(Cuboid::new(1.2, 0.5, 1.2), Team, at(-4.5, 8.9, 0.0));
    m.insert("shipyard".into(), k.take());

    k.add(Cylinder::new(2.0, 1.2), Dark, at(0.0, 0.6, 0.0))
        .add(Cylinder::new(0.8, 1.0), Hull, at(0.0, 1.7, 0.0))
        .add(ball(1.0), Team, at(0.0, 2.4, 0.0))
        .add(Cylinder::new(0.12, 2.4), Dark, along_z(at(-0.3, 2.5, -1.3)))
        .add(Cylinder::new(0.12, 2.4), Dark, along_z(at(0.3, 2.5, -1.3)));
    m.insert("turret".into(), k.take());

    m
}

fn build_deposits(meshes: &mut Assets<Mesh>) -> HashMap<Res, Vec<Part>> {
    use Slot::*;
    let at = Kit::at;
    let mut k = Kit { meshes, parts: Vec::new() };
    let mut m = HashMap::new();
    k.add(ball(1.0), Ore, at(0.0, 0.3, 0.0).with_scale(Vec3::new(1.4, 0.8, 1.1)))
        .add(ball(0.8), Ore, at(1.2, 0.2, 0.6).with_scale(Vec3::new(1.0, 0.7, 1.2)))
        .add(ball(0.6), Ore, at(-1.0, 0.15, 0.9))
        .add(ball(0.5), Ore, at(-0.4, 0.15, -1.2));
    m.insert(Res::Metal, k.take());
    let tilt = |x: f32, z: f32| Quat::from_rotation_x(x) * Quat::from_rotation_z(z);
    k.add(facet(0.5), Crystal, at(0.0, 1.0, 0.0).with_scale(Vec3::new(0.8, 3.0, 0.8)))
        .add(facet(0.4), Crystal, at(0.7, 0.6, 0.3).with_rotation(tilt(0.2, -0.5)).with_scale(Vec3::new(0.8, 2.6, 0.8)))
        .add(facet(0.35), Crystal, at(-0.6, 0.5, 0.4).with_rotation(tilt(-0.3, 0.6)).with_scale(Vec3::new(0.8, 2.4, 0.8)))
        .add(facet(0.3), Crystal, at(0.1, 0.4, -0.7).with_rotation(tilt(0.6, 0.1)).with_scale(Vec3::new(0.8, 2.2, 0.8)));
    m.insert(Res::Crystal, k.take());
    k.add(Torus::new(0.7, 1.3), Dark, at(0.0, 0.1, 0.0))
        .add(ball(0.6), Gas, at(0.0, 0.2, 0.0))
        .add(Cylinder::new(0.55, 2.6), Haze, at(0.0, 1.3, 0.0));
    m.insert(Res::Gas, k.take());
    m
}

impl Models {
    pub fn parts(&self, key: &str) -> &[Part] {
        self.models.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn deposit_parts(&self, r: Res) -> &[Part] {
        &self.deposits[&r]
    }

    pub fn material(&self, slot: Slot, team: u8) -> Handle<StandardMaterial> {
        match slot {
            Slot::Team => self.team[team as usize % self.team.len()].clone(),
            Slot::Glow => self.team_glow[team as usize % self.team_glow.len()].clone(),
            s => self.mats[&s].clone(),
        }
    }

    /// Spawns the parts as children of `parent`. `override_mat` paints everything (ghosts).
    pub fn spawn_parts(
        &self,
        commands: &mut Commands,
        parent: Entity,
        parts: &[Part],
        team: u8,
        scale: f32,
        override_mat: Option<&Handle<StandardMaterial>>,
    ) {
        commands.entity(parent).with_children(|c| {
            for p in parts {
                let mat = override_mat.cloned().unwrap_or_else(|| self.material(p.slot, team));
                let mut t = p.transform;
                t.translation *= scale;
                t.scale *= scale;
                c.spawn((Mesh3d(p.mesh.clone()), MeshMaterial3d(mat), t));
            }
        });
    }
}

pub fn init_models(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let m = &mut *materials;
    let mut mats = HashMap::new();
    mats.insert(Slot::Hull, mat(m, Color::srgb(0.62, 0.64, 0.68), 0.75, 0.35));
    mats.insert(Slot::Dark, mat(m, Color::srgb(0.12, 0.12, 0.14), 0.6, 0.5));
    mats.insert(
        Slot::Glass,
        m.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.65, 0.95, 0.75),
            alpha_mode: AlphaMode::Blend,
            metallic: 0.1,
            perceptual_roughness: 0.08,
            reflectance: 0.8,
            emissive: LinearRgba::rgb(0.02, 0.08, 0.15),
            ..default()
        }),
    );
    mats.insert(Slot::Glow, glow(m, LinearRgba::rgb(2.0, 6.0, 10.0)));
    mats.insert(Slot::Team, mat(m, Color::WHITE, 0.3, 0.45));
    mats.insert(Slot::Ore, mat(m, Color::srgb(0.38, 0.27, 0.2), 0.55, 0.6));
    mats.insert(
        Slot::Crystal,
        m.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.55, 0.75),
            emissive: LinearRgba::rgb(0.1, 0.9, 1.4),
            perceptual_roughness: 0.1,
            ..default()
        }),
    );
    mats.insert(Slot::Gas, glow(m, LinearRgba::rgb(1.5, 6.0, 1.0)));
    mats.insert(
        Slot::Haze,
        m.add(StandardMaterial {
            base_color: Color::srgba(0.5, 1.0, 0.4, 0.12),
            emissive: LinearRgba::rgb(0.1, 0.5, 0.08),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        }),
    );
    let team = (0..PLAYER_COLORS.len() as u8).map(|i| mat(m, team_color(i), 0.35, 0.4)).collect();
    let team_glow = (0..PLAYER_COLORS.len() as u8)
        .map(|i| {
            let c = team_color(i).to_linear();
            glow(m, LinearRgba::rgb(0.6 + c.red * 6.0, 0.6 + c.green * 6.0, 0.6 + c.blue * 6.0))
        })
        .collect();
    let ghost = |m: &mut Assets<StandardMaterial>, c: Color| {
        m.add(StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, unlit: true, ..default() })
    };
    let ghost_ok = ghost(m, Color::srgba(0.3, 1.0, 0.4, 0.45));
    let ghost_bad = ghost(m, Color::srgba(1.0, 0.25, 0.2, 0.45));
    let planet = m.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        metallic: 0.0,
        reflectance: 0.25,
        ..default()
    });
    let models = build_models(&mut meshes);
    let deposits = build_deposits(&mut meshes);
    let sphere = meshes.add(Sphere::new(1.0).mesh().ico(48).expect("ico"));
    commands.insert_resource(Models { mats, team, team_glow, ghost_ok, ghost_bad, planet, sphere, models, deposits });
}
