//! Match loading: regenerate the star system from the seed, build the planet meshes on a
//! background thread, spawn planets and deposits, report the hash, then enter the game.

use std::sync::Arc;

use bevy::prelude::*;
use cc_core::system::{self, StarSystem};
use cc_protocol::ClientMsg;
use crossbeam_channel::Receiver;

use super::camera::OrbitCam;
use super::input::{Placement, Selection};
use super::models::Models;
use super::planet_mesh::{planet_mesh, subdivisions_for};
use super::scene::spawn_planet_with_mesh;
use super::world::{ClientWorld, DepositVisual, WorldEntity};
use crate::net::{MatchRes, Net};
use crate::AppState;

#[derive(Resource)]
pub struct LoadingTask {
    rx: Receiver<(Arc<StarSystem>, Vec<Mesh>)>,
    /// Map hash sent; waiting for the server to start.
    pub sent: bool,
}

#[derive(Component)]
pub struct PlanetVisual;

pub fn begin_loading(mut commands: Commands, mres: Res<MatchRes>) {
    let Some(start) = mres.start.clone() else { return };
    let (tx, rx) = crossbeam_channel::bounded(1);
    std::thread::spawn(move || {
        let sys = Arc::new(system::generate(&start.settings, start.players.len()));
        let meshes: Vec<Mesh> = std::thread::scope(|s| {
            let handles: Vec<_> =
                sys.planets.iter().map(|p| s.spawn(move || planet_mesh(p, subdivisions_for(p.radius)))).collect();
            handles.into_iter().map(|h| h.join().expect("planet mesh")).collect()
        });
        let _ = tx.send((sys, meshes));
    });
    commands.insert_resource(LoadingTask { rx, sent: false });
}

#[allow(clippy::too_many_arguments)]
pub fn poll_loading(
    mut commands: Commands,
    task: Option<ResMut<LoadingTask>>,
    mres: Res<MatchRes>,
    net: Res<Net>,
    models: Res<Models>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(mut task) = task else { return };
    let Some(start) = mres.start.as_ref() else { return };
    if !task.sent {
        let Ok((sys, planet_meshes)) = task.rx.try_recv() else { return };
        for (p, mesh) in sys.planets.iter().zip(planet_meshes) {
            let handle = meshes.add(mesh);
            let e = spawn_planet_with_mesh(&mut commands, &mut materials, &models, p, handle, PlanetVisual);
            commands.entity(e).insert(WorldEntity);
        }
        let mut deposit_entities = Vec::with_capacity(sys.deposits.len());
        for d in &sys.deposits {
            let p = &sys.planets[d.planet as usize];
            let pos = p.center + d.dir * (p.radius + p.height(d.dir) - 0.2);
            let spin = Quat::from_axis_angle(d.dir, d.id as f32 * 1.7);
            let t = Transform::from_translation(pos).with_rotation(spin * Quat::from_rotation_arc(Vec3::Y, d.dir));
            let e = commands.spawn((WorldEntity, DepositVisual, t, Visibility::default())).id();
            models.spawn_parts(&mut commands, e, models.deposit_parts(d.kind), 0, 1.0, None);
            deposit_entities.push(e);
        }
        if sys.hash != start.map_hash {
            warn!("star system hash differs from the server's: {:016x} vs {:016x}", sys.hash, start.map_hash);
        }
        net.send(ClientMsg::MapReady { map_hash: sys.hash });
        let world = ClientWorld::new(sys.clone(), start, deposit_entities);
        let home = world.home_planet();
        commands.insert_resource(OrbitCam::looking_at_home(&sys, home));
        commands.insert_resource(world);
        task.sent = true;
    }
    if mres.running {
        commands.remove_resource::<LoadingTask>();
        next.set(AppState::InGame);
    }
}

/// Removes everything of the last match (runs when the menu opens).
pub fn teardown(
    mut commands: Commands,
    q: Query<Entity, With<WorldEntity>>,
    mut selection: ResMut<Selection>,
    mut placement: ResMut<Placement>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<ClientWorld>();
    commands.remove_resource::<LoadingTask>();
    selection.0.clear();
    placement.kind = None;
}
