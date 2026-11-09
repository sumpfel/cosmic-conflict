//! The client's mirror of the server state: applies snapshots, spawns/despawns visuals and
//! interpolates movement between snapshots (10 Hz → every frame).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use bevy::prelude::*;
use cc_core::content::Domain;
use cc_core::system::StarSystem;
use cc_core::PlayerId;
use cc_protocol::*;

use super::effects::Effects;
use super::models::{Models, UNIT_SCALE};
use crate::net::IncomingSnapshots;
use crate::Content;

/// Everything spawned for a match (despawned when it ends).
#[derive(Component)]
pub struct WorldEntity;

#[derive(Component)]
pub struct DepositVisual;

pub struct UnitView {
    pub snap: UnitSnap,
    pub entity: Entity,
    from: Vec3,
    to: Vec3,
    /// Interpolated world position (updated every frame).
    pub pos: Vec3,
    /// Surface normal (ground) or world up (ships).
    pub up: Vec3,
    forward: Vec3,
}

pub struct BuildingView {
    pub snap: BuildingSnap,
    pub entity: Entity,
    pub pos: Vec3,
    pub up: Vec3,
}

#[derive(Resource)]
pub struct ClientWorld {
    pub system: Arc<StarSystem>,
    pub me: PlayerId,
    pub players: Vec<MatchPlayer>,
    pub stats: HashMap<PlayerId, PlayerSnap>,
    pub units: BTreeMap<EntityId, UnitView>,
    pub buildings: BTreeMap<EntityId, BuildingView>,
    pub deposits: Vec<f32>,
    pub deposit_entities: Vec<Entity>,
    since_snapshot: f32,
}

impl ClientWorld {
    pub fn new(system: Arc<StarSystem>, start: &GameStart, deposit_entities: Vec<Entity>) -> Self {
        let deposits = system.deposits.iter().map(|d| d.amount).collect();
        ClientWorld {
            system,
            me: start.you,
            players: start.players.clone(),
            stats: HashMap::new(),
            units: BTreeMap::new(),
            buildings: BTreeMap::new(),
            deposits,
            deposit_entities,
            since_snapshot: 0.0,
        }
    }

    pub fn color_of(&self, owner: PlayerId) -> u8 {
        self.players.iter().find(|p| p.id == owner).map(|p| p.color).unwrap_or(7)
    }

    pub fn name_of(&self, owner: PlayerId) -> &str {
        self.players.iter().find(|p| p.id == owner).map(|p| p.name.as_str()).unwrap_or("?")
    }

    pub fn my_stats(&self) -> Option<&PlayerSnap> {
        self.stats.get(&self.me)
    }

    pub fn home_planet(&self) -> Option<u16> {
        let slot = self.players.iter().find(|p| p.id == self.me)?.slot;
        self.system.home_of(slot).map(|p| p.id)
    }

    /// World position and up vector of a unit snapshot.
    fn unit_world(&self, s: &UnitSnap) -> (Vec3, Vec3) {
        let v = Vec3::from_array(s.pos);
        if s.planet == NO_PLANET {
            (v, Vec3::Y)
        } else {
            let p = &self.system.planets[s.planet as usize];
            (p.surface(v), v)
        }
    }

    pub fn building_world(&self, s: &BuildingSnap) -> (Vec3, Vec3) {
        let p = &self.system.planets[s.planet as usize];
        let d = Vec3::from_array(s.dir);
        (p.center + d * (p.radius + p.height(d) - 0.3), d)
    }

    pub fn entity_pos(&self, id: EntityId) -> Option<Vec3> {
        self.units.get(&id).map(|u| u.pos).or_else(|| self.buildings.get(&id).map(|b| b.pos))
    }

    pub fn owner_of(&self, id: EntityId) -> Option<PlayerId> {
        self.units.get(&id).map(|u| u.snap.owner).or_else(|| self.buildings.get(&id).map(|b| b.snap.owner))
    }

    /// Planets where this player has a building.
    pub fn presence(&self, player: PlayerId, planet: u16) -> bool {
        self.buildings.values().any(|b| b.snap.owner == player && b.snap.planet == planet)
    }
}

fn spawn_visual(commands: &mut Commands, models: &Models, key: &str, team: u8, scale: f32, t: Transform) -> Entity {
    let e = commands.spawn((WorldEntity, t, Visibility::default())).id();
    models.spawn_parts(commands, e, models.parts(key), team, scale, None);
    e
}

pub fn apply_snapshots(
    mut commands: Commands,
    mut incoming: ResMut<IncomingSnapshots>,
    world: Option<ResMut<ClientWorld>>,
    models: Res<Models>,
    content: Res<Content>,
    mut effects: ResMut<Effects>,
    mut deposit_q: Query<(&mut Transform, &mut Visibility), With<DepositVisual>>,
) {
    let Some(mut world) = world else { return };
    let content = &content.0;
    while let Some(snap) = incoming.0.pop_front() {
        world.since_snapshot = 0.0;
        world.stats = snap.players.iter().map(|p| (p.id, p.clone())).collect();

        // Units.
        let mut seen = Vec::with_capacity(snap.units.len());
        for s in snap.units {
            seen.push(s.id);
            let (pos, up) = world.unit_world(&s);
            if let Some(v) = world.units.get_mut(&s.id) {
                v.from = v.pos;
                v.to = pos;
                v.snap = s;
            } else {
                let def = content.unit(s.kind);
                let team = world.color_of(s.owner);
                let entity = spawn_visual(&mut commands, &models, &def.model, team, UNIT_SCALE, Transform::from_translation(pos));
                let forward = up.any_orthonormal_vector();
                world.units.insert(s.id, UnitView { snap: s, entity, from: pos, to: pos, pos, up, forward });
            }
        }
        seen.sort_unstable();
        let gone: Vec<EntityId> = world.units.keys().copied().filter(|id| seen.binary_search(id).is_err()).collect();
        for id in gone {
            if let Some(v) = world.units.remove(&id) {
                commands.entity(v.entity).despawn();
                let r = content.unit(v.snap.kind).radius;
                // Colony ships "disappear" when they land; everything else explodes.
                if !content.unit(v.snap.kind).colonizer || v.snap.hp < content.unit(v.snap.kind).hp * 0.5 {
                    effects.explosion(v.pos, r * 2.5);
                }
            }
        }

        // Buildings.
        let mut seen = Vec::with_capacity(snap.buildings.len());
        for s in snap.buildings {
            seen.push(s.id);
            if let Some(v) = world.buildings.get_mut(&s.id) {
                v.snap = s;
            } else {
                let (pos, up) = world.building_world(&s);
                let def = content.building(s.kind);
                let team = world.color_of(s.owner);
                let t = Transform::from_translation(pos).with_rotation(Quat::from_rotation_arc(Vec3::Y, up));
                let entity = spawn_visual(&mut commands, &models, &def.model, team, 1.0, t);
                world.buildings.insert(s.id, BuildingView { snap: s, entity, pos, up });
            }
        }
        seen.sort_unstable();
        let gone: Vec<EntityId> = world.buildings.keys().copied().filter(|id| seen.binary_search(id).is_err()).collect();
        for id in gone {
            if let Some(v) = world.buildings.remove(&id) {
                commands.entity(v.entity).despawn();
                effects.explosion(v.pos, content.building(v.snap.kind).radius * 1.6);
            }
        }

        // Deposits shrink as they are mined.
        for (id, amount) in snap.deposits {
            let Some(initial) = world.system.deposits.get(id as usize).map(|d| d.amount) else { continue };
            world.deposits[id as usize] = amount;
            if let Some(e) = world.deposit_entities.get(id as usize)
                && let Ok((mut t, mut vis)) = deposit_q.get_mut(*e)
            {
                let f = (amount / initial).clamp(0.0, 1.0);
                t.scale = Vec3::splat(0.45 + 0.55 * f.sqrt());
                *vis = if amount <= 0.0 { Visibility::Hidden } else { Visibility::Inherited };
            }
        }

        for shot in snap.shots {
            if let (Some(a), Some(b), Some(owner)) = (world.entity_pos(shot.from), world.entity_pos(shot.to), world.owner_of(shot.from)) {
                effects.shot(a, b, world.color_of(owner));
            }
        }
    }
}

/// Smooth movement between snapshots, units standing on the terrain and facing where they go.
pub fn interpolate(
    time: Res<Time>,
    world: Option<ResMut<ClientWorld>>,
    content: Res<Content>,
    mut q: Query<&mut Transform>,
) {
    let Some(mut world) = world else { return };
    world.since_snapshot += time.delta_secs();
    let t = (world.since_snapshot / (SNAPSHOT_EVERY as f32 * TICK_DT)).min(1.0);
    let system = world.system.clone();
    for v in world.units.values_mut() {
        let p = v.from.lerp(v.to, t);
        let moving = v.to - v.from;
        let def = content.0.unit(v.snap.kind);
        if v.snap.planet != NO_PLANET {
            let planet = &system.planets[v.snap.planet as usize];
            let dir = (p - planet.center).normalize_or(Vec3::Y);
            v.pos = planet.surface(dir);
            v.up = dir;
            let tangent = moving - dir * moving.dot(dir);
            if tangent.length_squared() > 1e-6 {
                v.forward = tangent.normalize();
            } else {
                // Keep the old heading, re-projected onto the new tangent plane.
                let f = v.forward - dir * v.forward.dot(dir);
                v.forward = f.try_normalize().unwrap_or_else(|| dir.any_orthonormal_vector());
            }
        } else {
            v.pos = p;
            v.up = Vec3::Y;
            if moving.length_squared() > 1e-6 {
                v.forward = v.forward.lerp(moving.normalize(), 0.25).normalize_or(moving.normalize());
            }
        }
        if let Ok(mut tr) = q.get_mut(v.entity) {
            let bob = if def.domain == Domain::Space { (time.elapsed_secs() * 1.3 + v.snap.id as f32).sin() * 0.15 } else { 0.0 };
            *tr = Transform::from_translation(v.pos + v.up * bob).looking_to(v.forward, v.up);
        }
    }
    for v in world.buildings.values() {
        if let Ok(mut tr) = q.get_mut(v.entity) {
            let p = v.snap.progress.clamp(0.0, 1.0);
            tr.scale = Vec3::new(1.0, 0.15 + 0.85 * p, 1.0);
        }
    }
}
