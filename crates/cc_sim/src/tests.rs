#![cfg(test)]

use super::*;
use cc_core::settings::{MatchSettings, StartResources};
use cc_core::system;

fn new_sim(players: u8) -> Sim {
    let settings = MatchSettings { seed: 11, start: StartResources::High, ..Default::default() };
    let content = Arc::new(ContentDb::embedded());
    let sys = Arc::new(system::generate(&settings, players as usize));
    let inits: Vec<PlayerInit> = (0..players).map(|i| PlayerInit { id: i, slot: i }).collect();
    Sim::new(content, sys, &settings, &inits)
}

fn run(sim: &mut Sim, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as usize {
        sim.step();
    }
}

fn drones(sim: &Sim, owner: PlayerId) -> Vec<EntityId> {
    let k = sim.content.unit_kind("drone").unwrap();
    sim.units.values().filter(|u| u.owner == owner && u.kind == k).map(|u| u.id).collect()
}

fn home(sim: &Sim, slot: u8) -> (u16, Vec3) {
    let p = sim.system.home_of(slot).unwrap();
    (p.id, p.start_dir)
}

#[test]
fn start_state() {
    let sim = new_sim(2);
    assert_eq!(sim.buildings.len(), 2);
    assert_eq!(drones(&sim, 0).len(), 4);
    assert_eq!(sim.player(0).unwrap().pop, 4);
    assert_eq!(sim.player(0).unwrap().pop_cap, 10);
}

#[test]
fn drones_walk_on_their_globe() {
    let mut sim = new_sim(1);
    let (planet, start) = home(&sim, 0);
    let ids = drones(&sim, 0);
    // The far side of the planet.
    let goal = -start;
    sim.command(0, GameCommand::Move { units: vec![ids[0]], to: Target::Ground { planet, dir: goal.to_array() } });
    run(&mut sim, 40.0);
    let u = &sim.units[&ids[0]];
    let Loc::Ground { planet: p, dir } = u.loc else { panic!("left the planet") };
    assert_eq!(p, planet);
    assert!(geo::angle_between(dir, goal) < 0.05, "did not arrive");
    assert_eq!(u.order, Order::Idle);
}

#[test]
fn extractor_on_a_far_deposit_mines() {
    let mut sim = new_sim(1);
    let (planet, start) = home(&sim, 0);
    // A deposit on the other half of the globe: the whole planet is usable.
    let dep = sim.system.deposits.iter().find(|d| d.planet == planet && d.dir.dot(start) < -0.3).unwrap().clone();
    let ext = sim.content.building_kind("extractor").unwrap();
    let before = sim.player(0).unwrap().stock[dep.kind.idx()];
    sim.command(0, GameCommand::Build { builders: drones(&sim, 0), kind: ext, planet, dir: dep.dir.to_array() });
    assert!(sim.buildings.values().any(|b| b.kind == ext && b.deposit == Some(dep.id)));
    run(&mut sim, 90.0);
    let b = sim.buildings.values().find(|b| b.kind == ext).unwrap();
    assert!(b.finished(), "progress {}", b.progress);
    assert!(sim.deposits[dep.id as usize] < dep.amount);
    let after = sim.player(0).unwrap().stock[dep.kind.idx()];
    assert!(after > before - 75.0, "{before} -> {after}");
}

#[test]
fn placement_rules() {
    let sim = new_sim(1);
    let (planet, start) = home(&sim, 0);
    let hab = sim.content.building_kind("habitat").unwrap();
    let ext = sim.content.building_kind("extractor").unwrap();
    assert!(sim.placement(hab, planet, start).is_err(), "on top of the command center");
    let far = -start;
    let free = (0..200)
        .map(|i| geo::offset(far, i as f32, 0.02 * i as f32))
        .find(|d| sim.placement(hab, planet, *d).is_ok());
    assert!(free.is_some());
    assert!(sim.placement(ext, planet, free.unwrap()).is_err(), "extractor away from deposits");
}

#[test]
fn training_and_housing() {
    let mut sim = new_sim(1);
    let cc = *sim.buildings.keys().next().unwrap();
    let drone = sim.content.unit_kind("drone").unwrap();
    for _ in 0..5 {
        sim.command(0, GameCommand::Train { building: cc, kind: drone });
    }
    assert_eq!(sim.buildings[&cc].queue.len(), 5);
    sim.command(0, GameCommand::Train { building: cc, kind: drone });
    assert_eq!(sim.buildings[&cc].queue.len(), 5, "queue limit");
    run(&mut sim, 70.0);
    // 4 + 5 drones would fit 10 housing.
    assert_eq!(drones(&sim, 0).len(), 9);
}

#[test]
fn colony_ship_founds_an_outpost() {
    let mut sim = new_sim(2);
    let ship_kind = sim.content.unit_kind("colony_ship").unwrap();
    let outpost = sim.content.building_kind("outpost").unwrap();
    let neutral = sim.system.planets.iter().find(|p| p.home.is_none()).unwrap().clone();
    let (home_id, start) = home(&sim, 0);
    let pos = sim.system.planets[home_id as usize].orbit_point(start);
    let ship = sim.spawn_unit(0, ship_kind, Loc::Space { pos });
    // A free spot on the neutral planet.
    let dir = (0..400)
        .map(|i| geo::offset(Vec3::Y, i as f32 * 0.7, 0.01 * i as f32))
        .find(|d| sim.placement(outpost, neutral.id, *d).is_ok())
        .unwrap();
    sim.command(0, GameCommand::Build { builders: vec![ship], kind: outpost, planet: neutral.id, dir: dir.to_array() });
    run(&mut sim, 120.0);
    assert!(!sim.units.contains_key(&ship), "ship used up");
    let b = sim.buildings.values().find(|b| b.kind == outpost).expect("outpost founded");
    assert_eq!(b.planet, neutral.id);
    assert!(b.finished());
}

#[test]
fn ships_fly_around_planets() {
    let mut sim = new_sim(2);
    let fighter = sim.content.unit_kind("fighter").unwrap();
    let a = &sim.system.planets[0];
    let b = sim.system.planets.iter().find(|p| p.id != 0).unwrap().clone();
    // Start on the far side of planet 0 so the straight line crosses it.
    let away = (a.center - b.center).normalize();
    let start = a.center + away * (a.radius + 25.0);
    let ship = sim.spawn_unit(0, fighter, Loc::Space { pos: start });
    sim.command(0, GameCommand::Move { units: vec![ship], to: Target::Ground { planet: b.id, dir: [0.0, 1.0, 0.0] } });
    let a = sim.system.planets[0].clone();
    for _ in 0..(120 * TICK_RATE) {
        sim.step();
        let pos = sim.world_pos(sim.units[&ship].loc);
        assert!(pos.distance(a.center) > a.radius, "flew through the planet");
    }
    let pos = sim.world_pos(sim.units[&ship].loc);
    assert!(pos.distance(b.orbit_point(Vec3::Y)) < 3.0, "arrived");
}

#[test]
fn combat_and_victory() {
    let mut sim = new_sim(2);
    let bomber = sim.content.unit_kind("bomber").unwrap();
    let (enemy_home, enemy_start) = home(&sim, 1);
    let pos = sim.system.planets[enemy_home as usize].orbit_point(enemy_start);
    let cc = sim.buildings.values().find(|b| b.owner == 1).unwrap().id;
    let mut fleet = Vec::new();
    for i in 0..12 {
        fleet.push(sim.spawn_unit(0, bomber, Loc::Space { pos: pos + Vec3::X * i as f32 * 3.0 }));
    }
    sim.command(0, GameCommand::Attack { units: fleet, target: cc });
    for _ in 0..(200 * TICK_RATE) {
        sim.step();
        if sim.winner.is_some() {
            break;
        }
    }
    assert!(!sim.buildings.contains_key(&cc), "command center destroyed");
    assert_eq!(sim.winner, Some(Some(0)));
    assert!(sim.player(1).unwrap().defeated);
}

#[test]
fn snapshot_contains_everything() {
    let mut sim = new_sim(2);
    run(&mut sim, 1.0);
    let s = sim.snapshot();
    assert_eq!(s.players.len(), 2);
    assert_eq!(s.units.len(), 8);
    assert_eq!(s.buildings.len(), 2);
}
