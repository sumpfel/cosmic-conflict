//! Short-lived visuals: weapon tracers, explosions and order pings.

use bevy::prelude::*;

use super::models::{Models, team_color};
use super::world::WorldEntity;

const TRACER_LIFE: f32 = 0.18;
const EXPLOSION_LIFE: f32 = 0.7;
const PING_LIFE: f32 = 0.6;

#[derive(Resource, Default)]
pub struct Effects {
    shots: Vec<(Vec3, Vec3, Color, f32)>,
    pending_explosions: Vec<(Vec3, f32)>,
    pings: Vec<(Vec3, Vec3, Color, f32)>,
}

impl Effects {
    pub fn shot(&mut self, from: Vec3, to: Vec3, team: u8) {
        self.shots.push((from, to, team_color(team), 0.0));
    }

    pub fn explosion(&mut self, at: Vec3, size: f32) {
        self.pending_explosions.push((at, size));
    }

    /// A ring where an order was given (`up` = surface normal or camera direction).
    pub fn ping(&mut self, at: Vec3, up: Vec3, color: Color) {
        self.pings.push((at, up, color, 0.0));
    }
}

#[derive(Component)]
pub struct Explosion {
    age: f32,
    size: f32,
}

#[derive(Resource)]
pub struct ExplosionMat(Handle<StandardMaterial>);

pub fn init_effects(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(ExplosionMat(materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.6, 0.2, 0.6),
        emissive: LinearRgba::rgb(30.0, 12.0, 3.0),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    })));
}

pub fn update_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut fx: ResMut<Effects>,
    mut gizmos: Gizmos,
    models: Res<Models>,
    mat: Res<ExplosionMat>,
    mut explosions: Query<(Entity, &mut Explosion, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (a, b, c, age) in fx.shots.iter_mut() {
        *age += dt;
        let t = (*age / TRACER_LIFE).clamp(0.0, 1.0);
        let head = a.lerp(*b, (t * 1.6).min(1.0));
        let tail = a.lerp(*b, (t * 1.6 - 0.5).clamp(0.0, 1.0));
        let lin = c.to_linear();
        gizmos.line(tail, head, Color::linear_rgb(lin.red * 6.0 + 1.0, lin.green * 6.0 + 1.0, lin.blue * 6.0 + 1.0));
    }
    fx.shots.retain(|s| s.3 < TRACER_LIFE);
    for (at, up, c, age) in fx.pings.iter_mut() {
        *age += dt;
        let r = 1.0 + *age * 6.0;
        gizmos.circle(Isometry3d::new(*at + *up * 0.3, Quat::from_rotation_arc(Vec3::Z, *up)), r, c.with_alpha(1.0 - *age / PING_LIFE));
    }
    fx.pings.retain(|p| p.3 < PING_LIFE);
    for (at, size) in fx.pending_explosions.drain(..) {
        commands.spawn((
            WorldEntity,
            Explosion { age: 0.0, size },
            Mesh3d(models.sphere.clone()),
            MeshMaterial3d(mat.0.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(size * 0.2)),
        ));
    }
    for (e, mut ex, mut t) in &mut explosions {
        ex.age += dt;
        let k = ex.age / EXPLOSION_LIFE;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Grow fast, then collapse.
        let s = ex.size * (0.2 + 1.2 * k.sqrt()) * (1.0 - k * k);
        t.scale = Vec3::splat(s.max(0.01));
    }
}
