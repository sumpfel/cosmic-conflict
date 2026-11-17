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

/// A finished building of `owner` on its home planet, `dist` metres from the start.
fn place(sim: &mut Sim, owner: PlayerId, id: &str, az: f32, dist: f32) -> EntityId {
    let kind = sim.content.building_kind(id).unwrap();
    let (planet, start) = home(sim, owner);
    let r = sim.system.planets[planet as usize].radius;
    let dir = geo::offset(start, az, dist / r);
    let b = sim.spawn_building(owner, kind, planet, dir, None);
    let st = sim.stats_of(owner).building(kind).clone();
    let b2 = sim.buildings.get_mut(&b).unwrap();
    b2.progress = 1.0;
    b2.hp = st.hp;
    b2.shield = st.shield;
    b
}

fn ground_unit(sim: &mut Sim, owner: PlayerId, id: &str, planet: u16, dir: Vec3) -> EntityId {
    let kind = sim.content.unit_kind(id).unwrap();
    sim.spawn_unit(owner, kind, Loc::Ground { planet, dir })
}

#[test]
fn research_unlocks_units_and_buildings() {
    let mut sim = new_sim(1);
    let lab = place(&mut sim, 0, "research_lab", 1.0, 20.0);
    place(&mut sim, 0, "factory", 3.0, 22.0);
    let robotics = sim.content.tech_kind("robotics").unwrap();
    let bay = sim.content.building_kind("robotics_bay").unwrap();
    let (planet, start) = home(&sim, 0);
    let site = (0..300)
        .map(|i| geo::offset(-start, i as f32 * 0.7, 0.01 * i as f32))
        .find(|d| sim.placement(bay, planet, *d).is_ok())
        .unwrap();
    // Locked before the research.
    sim.command(0, GameCommand::Build { builders: drones(&sim, 0), kind: bay, planet, dir: site.to_array() });
    assert!(!sim.buildings.values().any(|b| b.kind == bay));
    let before = sim.player(0).unwrap().stock;
    sim.command(0, GameCommand::Research { building: lab, tech: robotics });
    assert_eq!(sim.buildings[&lab].research, vec![robotics]);
    assert!(sim.player(0).unwrap().stock[0] < before[0], "paid");
    sim.command(0, GameCommand::Research { building: lab, tech: robotics });
    assert_eq!(sim.buildings[&lab].research.len(), 1, "no double research");
    let t = sim.content.tech(robotics).time;
    run(&mut sim, t + 1.0);
    assert!(sim.has_tech(0, robotics));
    assert!(sim.snapshot().players[0].techs.contains(&robotics));
    sim.command(0, GameCommand::Build { builders: drones(&sim, 0), kind: bay, planet, dir: site.to_array() });
    assert!(sim.buildings.values().any(|b| b.kind == bay), "unlocked");
}

#[test]
fn research_prerequisites_queue_and_refund() {
    let mut sim = new_sim(1);
    let lab = place(&mut sim, 0, "research_lab", 1.0, 20.0);
    let other = place(&mut sim, 0, "research_lab", 3.0, 22.0);
    let du = sim.content.tech_kind("depleted_uranium").unwrap();
    let ballistics = sim.content.tech_kind("ballistics").unwrap();
    sim.command(0, GameCommand::Research { building: lab, tech: du });
    assert!(sim.buildings[&lab].research.is_empty(), "prerequisite missing");
    // Chained in the same lab is fine, in another lab it has to wait for the result.
    sim.command(0, GameCommand::Research { building: lab, tech: ballistics });
    sim.command(0, GameCommand::Research { building: other, tech: du });
    assert!(sim.buildings[&other].research.is_empty());
    sim.command(0, GameCommand::Research { building: lab, tech: du });
    assert_eq!(sim.buildings[&lab].research, vec![ballistics, du]);
    let stock = sim.player(0).unwrap().stock;
    sim.command(0, GameCommand::CancelResearch { building: lab });
    let refunded = sim.player(0).unwrap().stock;
    assert_eq!(refunded[0], stock[0] + sim.content.tech(du).cost.metal);
    let t = sim.content.tech(ballistics).time;
    run(&mut sim, t + 1.0);
    assert!(sim.has_tech(0, ballistics) && !sim.has_tech(0, du));
}

#[test]
fn upgrades_raise_stats_and_heal_the_difference() {
    let mut sim = new_sim(1);
    let cc = *sim.buildings.keys().next().unwrap();
    let hp = sim.buildings[&cc].hp;
    let t = sim.content.tech_kind("reinforced_structures").unwrap();
    sim.complete_research(0, t);
    assert!((sim.buildings[&cc].hp - hp * 1.25).abs() < 1.0, "{} -> {}", hp, sim.buildings[&cc].hp);
    let ext = sim.content.building_kind("extractor").unwrap();
    let drills = sim.content.tech_kind("deep_core_drills").unwrap();
    let base = sim.stats_of(0).building(ext).rate;
    sim.complete_research(0, drills);
    assert!(sim.stats_of(0).building(ext).rate > base);
}

#[test]
fn armour_and_shields_soak_damage() {
    let mut sim = new_sim(2);
    let (planet, start) = home(&sim, 0);
    let r = sim.system.planets[planet as usize].radius;
    // Troopers against a bare turret and against a bunker: armour makes the difference.
    let turret = place(&mut sim, 0, "turret", 0.5, 25.0);
    let bunker = place(&mut sim, 0, "bunker", 2.5, 25.0);
    for (b, az) in [(turret, 0.5_f32), (bunker, 2.5)] {
        let dir = geo::offset(start, az, 37.0 / r);
        for i in 0..3 {
            let u = ground_unit(&mut sim, 1, "trooper", planet, geo::offset(dir, i as f32 * 2.0, 2.0 / r));
            sim.units.get_mut(&u).unwrap().hp = 1e6;
            sim.command(1, GameCommand::Attack { units: vec![u], target: b });
        }
    }
    // Keep the defences from shooting back.
    for b in [turret, bunker] {
        sim.buildings.get_mut(&b).unwrap().cooldown = 1e9;
    }
    let (t0, b0) = (sim.buildings[&turret].hp, sim.buildings[&bunker].hp);
    run(&mut sim, 20.0);
    let lost_turret = t0 - sim.buildings[&turret].hp;
    let lost_bunker = b0 - sim.buildings[&bunker].hp;
    assert!(lost_turret > 0.0 && lost_bunker > 0.0);
    assert!(lost_bunker < lost_turret * 0.6, "bunker {lost_bunker} vs turret {lost_turret}");

    // A shield takes hits first and recharges after a pause.
    let shield_gen = place(&mut sim, 0, "shield_generator", 4.0, 25.0);
    run(&mut sim, 25.0);
    let s = sim.buildings[&shield_gen].shield;
    assert!(s > 100.0, "aura shield charged: {s}");
    sim.apply_hits(&[Hit { target: shield_gen, damage: 50.0, kind: DamageKind::Kinetic }]);
    let hp = sim.buildings[&shield_gen].hp;
    assert_eq!(sim.buildings[&shield_gen].shield, s - 50.0);
    run(&mut sim, 1.0);
    assert_eq!(sim.buildings[&shield_gen].hp, hp, "hull untouched");
    assert!(sim.buildings[&shield_gen].shield <= s - 50.0 + 1e-3, "no recharge yet");
    run(&mut sim, 5.0);
    assert!(sim.buildings[&shield_gen].shield > s - 50.0, "recharging");
    // Ion damage tears shields down much faster.
    let before = sim.buildings[&shield_gen].shield;
    sim.apply_hits(&[Hit { target: shield_gen, damage: 20.0, kind: DamageKind::Ion }]);
    assert!(before - sim.buildings[&shield_gen].shield > 40.0);
}

#[test]
fn splash_hits_neighbours() {
    let mut sim = new_sim(2);
    let (planet, start) = home(&sim, 1);
    let r = sim.system.planets[planet as usize].radius;
    let far = -start;
    let a = ground_unit(&mut sim, 1, "trooper", planet, far);
    let b = ground_unit(&mut sim, 1, "trooper", planet, geo::offset(far, 0.0, 1.6 / r));
    let c = ground_unit(&mut sim, 1, "trooper", planet, geo::offset(far, 1.0, 12.0 / r));
    let mortar = ground_unit(&mut sim, 0, "artillery", planet, geo::offset(far, 3.0, 40.0 / r));
    sim.command(0, GameCommand::Attack { units: vec![mortar], target: a });
    // The troopers stay put (no weapons range) for the test.
    for id in [a, b, c] {
        sim.units.get_mut(&id).unwrap().cooldown = 1e9;
    }
    run(&mut sim, 0.5);
    let full = sim.content.unit(sim.units[&a].kind).hp;
    assert!(sim.units[&a].hp < full);
    assert!(sim.units[&b].hp < full, "splash");
    assert_eq!(sim.units[&c].hp, full, "out of the blast");
}

#[test]
fn stealth_needs_a_detector() {
    let mut sim = new_sim(2);
    let (planet, start) = home(&sim, 0);
    let r = sim.system.planets[planet as usize].radius;
    let turret = place(&mut sim, 0, "turret", 1.0, 30.0);
    let tdir = geo::offset(start, 1.0, 30.0 / r);
    let ghost = ground_unit(&mut sim, 1, "infiltrator", planet, geo::offset(tdir, 0.0, 10.0 / r));
    let full = sim.units[&ghost].hp;
    run(&mut sim, 3.0);
    assert_eq!(sim.units[&ghost].hp, full, "the turret cannot see it");
    assert!(!sim.targetable_by(ghost, 0));
    sim.command(0, GameCommand::Attack { units: drones(&sim, 0), target: ghost });
    // A gravimetric beacon next to the turret reveals it.
    place(&mut sim, 0, "gravimetric_beacon", 1.3, 34.0);
    run(&mut sim, 3.0);
    assert!(sim.targetable_by(ghost, 0));
    assert!(sim.units.get(&ghost).is_none_or(|u| u.hp < full), "shot once detected");
    let _ = turret;
}

#[test]
fn firing_reveals_a_stealthed_unit() {
    let mut sim = new_sim(2);
    let (planet, start) = home(&sim, 0);
    let r = sim.system.planets[planet as usize].radius;
    let ghost = ground_unit(&mut sim, 1, "infiltrator", planet, geo::offset(start, 0.0, 14.0 / r));
    let cc = sim.buildings.values().find(|b| b.owner == 0).unwrap().id;
    sim.command(1, GameCommand::Attack { units: vec![ghost], target: cc });
    for _ in 0..40 {
        sim.step();
        if sim.units[&ghost].revealed > 0.0 {
            break;
        }
    }
    assert!(sim.targetable_by(ghost, 0), "fired, so it can be shot back");
}

#[test]
fn repair_aura_mends() {
    let mut sim = new_sim(1);
    let (planet, start) = home(&sim, 0);
    let r = sim.system.planets[planet as usize].radius;
    let tank = ground_unit(&mut sim, 0, "tank", planet, geo::offset(start, 2.0, 20.0 / r));
    sim.units.get_mut(&tank).unwrap().hp = 50.0;
    run(&mut sim, 2.0);
    assert_eq!(sim.units[&tank].hp, 50.0, "no self-repair without research");
    ground_unit(&mut sim, 0, "repair_bot", planet, geo::offset(start, 2.0, 24.0 / r));
    run(&mut sim, 5.0);
    assert!(sim.units[&tank].hp > 70.0, "{}", sim.units[&tank].hp);
}

#[test]
fn sensors_for_fog_of_war() {
    let mut sim = new_sim(1);
    let radar = place(&mut sim, 0, "radar_station", 1.0, 20.0);
    let (_, _, s) = sim.sensors_of(radar).unwrap();
    assert!(s.radar > s.sight && s.sight > 0.0);
    let t = sim.content.tech_kind("phased_array").unwrap();
    sim.complete_research(0, t);
    let (_, _, upgraded) = sim.sensors_of(radar).unwrap();
    assert!(upgraded.radar > s.radar);
}
