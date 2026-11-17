//! Short-lived visuals: weapon tracers (one look per damage kind), explosions and order pings.

use bevy::prelude::*;
use cc_core::content::DamageKind;

use super::models::{Models, team_color};
use super::world::WorldEntity;

const TRACER_LIFE: f32 = 0.18;
const EXPLOSION_LIFE: f32 = 0.7;
const PING_LIFE: f32 = 0.6;

struct Tracer {
    from: Vec3,
    to: Vec3,
    color: Color,
    kind: DamageKind,
    age: f32,
}

#[derive(Resource, Default)]
pub struct Effects {
    shots: Vec<Tracer>,
    pending_explosions: Vec<(Vec3, f32)>,
    pings: Vec<(Vec3, Vec3, Color, f32)>,
}

impl Effects {
    /// A shot: kinetic rounds are team-coloured streaks, lasers red beams, ion bolts violet
    /// arcs, plasma orange bolts; explosive and plasma hits burst at the target (`splash` size).
    pub fn shot(&mut self, from: Vec3, to: Vec3, team: u8, kind: DamageKind, splash: f32) {
        let color = match kind {
            DamageKind::Kinetic => team_color(team),
            DamageKind::Laser => Color::srgb(1.0, 0.15, 0.1),
            DamageKind::Explosive => Color::srgb(1.0, 0.6, 0.2),
            DamageKind::Plasma => Color::srgb(1.0, 0.45, 0.1),
            DamageKind::Ion => Color::srgb(0.55, 0.3, 1.0),
        };
        self.shots.push(Tracer { from, to, color, kind, age: 0.0 });
        if matches!(kind, DamageKind::Explosive | DamageKind::Plasma) {
            self.pending_explosions.push((to, (splash * 0.8).max(0.7)));
        }
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
    for s in fx.shots.iter_mut() {
        s.age += dt;
        let t = (s.age / TRACER_LIFE).clamp(0.0, 1.0);
        let lin = s.color.to_linear();
        let hot = |k: f32| Color::linear_rgb(lin.red * k + 1.0, lin.green * k + 1.0, lin.blue * k + 1.0);
        let (a, b) = (s.from, s.to);
        match s.kind {
            // Beams: the whole line at once, fading.
            DamageKind::Laser => gizmos.line(a, b, hot(8.0 * (1.0 - t)).with_alpha(1.0 - t)),
            // A crackling arc.
            DamageKind::Ion => {
                let side = (b - a).cross(Vec3::Y).normalize_or(Vec3::X) * 0.35;
                let up = (b - a).cross(side).normalize_or(Vec3::Z) * 0.35;
                let mut prev = a;
                for k in 1..=8 {
                    let f = k as f32 / 8.0;
                    let n = ((f * 37.0 + s.age * 91.0).sin(), (f * 53.0 - s.age * 77.0).cos());
                    let p = if k == 8 { b } else { a.lerp(b, f) + side * n.0 + up * n.1 };
                    gizmos.line(prev, p, hot(10.0).with_alpha(1.0 - t));
                    prev = p;
                }
            }
            // Projectiles: a streak travelling to the target (plasma: a fat bolt).
            _ => {
                let head = a.lerp(b, (t * 1.6).min(1.0));
                let tail = a.lerp(b, (t * 1.6 - 0.5).clamp(0.0, 1.0));
                gizmos.line(tail, head, hot(6.0));
                if s.kind == DamageKind::Plasma {
                    let off = (b - a).any_orthonormal_vector() * 0.12;
                    gizmos.line(tail + off, head + off, hot(10.0));
                    gizmos.line(tail - off, head - off, hot(10.0));
                }
            }
        }
    }
    fx.shots.retain(|s| s.age < TRACER_LIFE);
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
