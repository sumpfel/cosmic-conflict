//! # cc_sim — the authoritative simulation
//!
//! Runs on the server only. Ground units live on a planet's surface (a unit direction from its
//! centre) and walk along great circles; ships fly in straight lines through space, slide around
//! planets and the sun, and hover in low orbit when sent to a planet. The terrain relief is
//! cosmetic: the simulation uses each planet's base sphere.
//!
//! `Sim::step()` order: economy (income, extraction, population) → training → units (orders,
//! movement, weapons) → building weapons → separation → damage → deaths → defeat / victory.

mod tests;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use cc_core::content::{BuildingKind, Domain, UnitKind, Weapon};
use cc_core::settings::MatchSettings;
use cc_core::system::{ORBIT_ALTITUDE, SUN_RADIUS, StarSystem};
use cc_core::{ContentDb, PlayerId, Vec3, geo};
use cc_protocol::*;

pub const MAX_QUEUE: usize = 5;
pub const POP_LIMIT: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Loc {
    Ground { planet: u16, dir: Vec3 },
    Space { pos: Vec3 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    Idle,
    Move(Loc),
    Attack(EntityId),
    /// Help constructing this building.
    Build(EntityId),
    /// Colonizer: fly to the planet and found the building there.
    Found { kind: BuildingKind, planet: u16, dir: Vec3 },
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: UnitKind,
    pub loc: Loc,
    pub hp: f32,
    pub order: Order,
    pub cooldown: f32,
}

#[derive(Clone, Debug)]
pub struct Building {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: BuildingKind,
    pub planet: u16,
    pub dir: Vec3,
    pub hp: f32,
    pub progress: f32,
    pub queue: Vec<UnitKind>,
    pub train_time: f32,
    pub cooldown: f32,
    /// Extractors: the deposit they mine.
    pub deposit: Option<u32>,
    /// Training is waiting for housing (notice sent once).
    pub stalled: bool,
}

impl Building {
    pub fn finished(&self) -> bool {
        self.progress >= 1.0
    }
}

#[derive(Clone, Debug)]
pub struct PlayerState {
    pub id: PlayerId,
    pub slot: u8,
    pub stock: [f32; 3],
    pub income: [f32; 3],
    pub pop: u32,
    pub pop_cap: u32,
    pub defeated: bool,
    pub surrendered: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerInit {
    pub id: PlayerId,
    /// Which home planet of the star system.
    pub slot: u8,
}

pub struct Sim {
    pub content: Arc<ContentDb>,
    pub system: Arc<StarSystem>,
    pub tick: u64,
    pub players: Vec<PlayerState>,
    pub units: BTreeMap<EntityId, Unit>,
    pub buildings: BTreeMap<EntityId, Building>,
    /// Amount left per deposit (index = deposit id).
    pub deposits: Vec<f32>,
    next_id: EntityId,
    shots: Vec<Shot>,
    changed_deposits: BTreeSet<u32>,
    notices: Vec<(PlayerId, String)>,
    /// Set when the match is over: `Some(winner)`.
    pub winner: Option<Option<PlayerId>>,
}

/// Something that can be shot at.
#[derive(Clone, Copy, Debug)]
struct Body {
    id: EntityId,
    owner: PlayerId,
    pos: Vec3,
    /// A ship (in space) rather than something on a surface.
    space: bool,
    planet: u16,
    radius: f32,
}

fn can_hit(w: &Weapon, attacker_planet: Option<u16>, target: &Body) -> bool {
    if target.space {
        w.space
    } else {
        // Ground weapons only reach the surface of the planet they stand on.
        w.ground && attacker_planet.is_none_or(|p| p == target.planet)
    }
}

fn arr(v: Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn vec(a: [f32; 3]) -> Option<Vec3> {
    let v = Vec3::from_array(a);
    (v.is_finite() && v.length_squared() > 1e-8).then_some(v)
}

/// Formation offset for the i-th unit of a group (sunflower spiral), in metres.
fn spread(i: usize) -> (f32, f32) {
    if i == 0 { (0.0, 0.0) } else { (i as f32 * 2.399_963, 2.6 * (i as f32).sqrt()) }
}

impl Sim {
    pub fn new(content: Arc<ContentDb>, system: Arc<StarSystem>, settings: &MatchSettings, players: &[PlayerInit]) -> Self {
        let deposits = system.deposits.iter().map(|d| d.amount).collect();
        let mut sim = Sim {
            content,
            system,
            tick: 0,
            players: Vec::new(),
            units: BTreeMap::new(),
            buildings: BTreeMap::new(),
            deposits,
            next_id: 1,
            shots: Vec::new(),
            changed_deposits: BTreeSet::new(),
            notices: Vec::new(),
            winner: None,
        };
        let cc = sim.content.building_kind("command_center").expect("command_center");
        let drone = sim.content.unit_kind("drone").expect("drone");
        for p in players {
            sim.players.push(PlayerState {
                id: p.id,
                slot: p.slot,
                stock: settings.start.amounts(),
                income: [0.0; 3],
                pop: 0,
                pop_cap: 0,
                defeated: false,
                surrendered: false,
            });
            let Some(home) = sim.system.home_of(p.slot) else { continue };
            let (planet, start, radius) = (home.id, home.start_dir, home.radius);
            let id = sim.spawn_building(p.id, cc, planet, start, None);
            let b = sim.buildings.get_mut(&id).expect("just spawned");
            b.progress = 1.0;
            b.hp = sim.content.building(cc).hp;
            let ring = (sim.content.building(cc).radius + 3.0) / radius;
            for i in 0..4 {
                let dir = geo::offset(start, 0.4 + i as f32 * 1.5, ring);
                sim.spawn_unit(p.id, drone, Loc::Ground { planet, dir });
            }
        }
        sim.update_population();
        sim
    }

    fn alloc(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn spawn_unit(&mut self, owner: PlayerId, kind: UnitKind, loc: Loc) -> EntityId {
        let id = self.alloc();
        let hp = self.content.unit(kind).hp;
        self.units.insert(id, Unit { id, owner, kind, loc, hp, order: Order::Idle, cooldown: 0.0 });
        id
    }

    fn spawn_building(&mut self, owner: PlayerId, kind: BuildingKind, planet: u16, dir: Vec3, deposit: Option<u32>) -> EntityId {
        let id = self.alloc();
        let hp = self.content.building(kind).hp * 0.1;
        self.buildings.insert(
            id,
            Building {
                id,
                owner,
                kind,
                planet,
                dir,
                hp,
                progress: 0.0,
                queue: Vec::new(),
                train_time: 0.0,
                cooldown: 0.0,
                deposit,
                stalled: false,
            },
        );
        id
    }

    pub fn player(&self, id: PlayerId) -> Option<&PlayerState> {
        self.players.iter().find(|p| p.id == id)
    }

    fn player_mut(&mut self, id: PlayerId) -> Option<&mut PlayerState> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    fn notice(&mut self, player: PlayerId, text: impl Into<String>) {
        self.notices.push((player, text.into()));
    }

    /// Messages for single players produced since the last call.
    pub fn take_notices(&mut self) -> Vec<(PlayerId, String)> {
        std::mem::take(&mut self.notices)
    }

    pub fn world_pos(&self, loc: Loc) -> Vec3 {
        match loc {
            Loc::Ground { planet, dir } => self.system.planets[planet as usize].ground(dir),
            Loc::Space { pos } => pos,
        }
    }

    /// Checks a building site. Returns the (possibly snapped) direction and the deposit an
    /// extractor would mine.
    pub fn placement(&self, kind: BuildingKind, planet: u16, dir: Vec3) -> Result<(Vec3, Option<u32>), String> {
        let def = self.content.building(kind);
        let Some(p) = self.system.planet(planet) else { return Err("No such planet.".into()) };
        let mut dir = dir.normalize();
        let mut deposit = None;
        if def.extracts {
            let best = self
                .system
                .deposits
                .iter()
                .filter(|d| d.planet == planet && self.deposits[d.id as usize] > 0.0)
                .map(|d| (p.arc(d.dir, dir), d))
                .filter(|(a, _)| *a < def.radius + 2.5)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((_, d)) = best else { return Err("Extractors must be built on a resource deposit.".into()) };
            if self.buildings.values().any(|b| b.deposit == Some(d.id)) {
                return Err("This deposit is already being mined.".into());
            }
            dir = d.dir;
            deposit = Some(d.id);
        } else if self
            .system
            .deposits
            .iter()
            .any(|d| d.planet == planet && self.deposits[d.id as usize] > 0.0 && p.arc(d.dir, dir) < def.radius + 1.5)
        {
            return Err("Too close to a resource deposit.".into());
        }
        for b in self.buildings.values().filter(|b| b.planet == planet) {
            let r = self.content.building(b.kind).radius;
            if p.arc(b.dir, dir) < r + def.radius + 1.0 {
                return Err("Blocked by another building.".into());
            }
        }
        Ok((dir, deposit))
    }

    fn has_finished(&self, player: PlayerId, kind: BuildingKind) -> bool {
        self.buildings.values().any(|b| b.owner == player && b.kind == kind && b.finished())
    }

    /// Missing requirement for `kind`, if any.
    pub fn missing_requirement(&self, player: PlayerId, kind: BuildingKind) -> Option<BuildingKind> {
        self.content.building_requires[kind as usize].iter().copied().find(|r| !self.has_finished(player, *r))
    }

    fn pay(&mut self, player: PlayerId, cost: [f32; 3]) -> bool {
        let Some(p) = self.player_mut(player) else { return false };
        if p.stock.iter().zip(cost).all(|(s, c)| *s >= c) {
            for (s, c) in p.stock.iter_mut().zip(cost) {
                *s -= c;
            }
            true
        } else {
            let missing = cc_core::Res::ALL.iter().find(|r| p.stock[r.idx()] < cost[r.idx()]).map(|r| r.label()).unwrap_or("resources");
            self.notice(player, format!("Not enough {}.", missing.to_lowercase()));
            false
        }
    }

    fn refund(&mut self, player: PlayerId, cost: [f32; 3], factor: f32) {
        if let Some(p) = self.player_mut(player) {
            for (s, c) in p.stock.iter_mut().zip(cost) {
                *s += c * factor;
            }
        }
    }

    fn owned_units(&self, player: PlayerId, ids: &[EntityId]) -> Vec<EntityId> {
        let mut v: Vec<EntityId> = ids.iter().copied().filter(|id| self.units.get(id).is_some_and(|u| u.owner == player)).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Applies a player's command (everything is validated here).
    pub fn command(&mut self, player: PlayerId, cmd: GameCommand) {
        if self.player(player).is_none_or(|p| p.defeated) || self.winner.is_some() {
            return;
        }
        match cmd {
            GameCommand::Move { units, to } => {
                let ids = self.owned_units(player, &units);
                let mut stranded = false;
                for (i, id) in ids.iter().enumerate() {
                    let u = &self.units[id];
                    let domain = self.content.unit(u.kind).domain;
                    let (az, dist) = spread(i);
                    let goal = match (domain, to) {
                        (Domain::Ground, Target::Ground { planet, dir }) => {
                            let Some(dir) = vec(dir) else { continue };
                            match u.loc {
                                Loc::Ground { planet: here, .. } if here == planet => {
                                    let r = self.system.planets[planet as usize].radius;
                                    Some(Loc::Ground { planet, dir: geo::offset(dir.normalize(), az, dist / r) })
                                }
                                _ => {
                                    stranded = true;
                                    None
                                }
                            }
                        }
                        (Domain::Ground, Target::Space { .. }) => {
                            stranded = true;
                            None
                        }
                        (Domain::Space, Target::Ground { planet, dir }) => {
                            let (Some(p), Some(dir)) = (self.system.planet(planet), vec(dir)) else { continue };
                            let d = geo::offset(dir.normalize(), az, dist * 1.6 / p.radius);
                            Some(Loc::Space { pos: p.orbit_point(d) })
                        }
                        (Domain::Space, Target::Space { pos }) => {
                            let Some(pos) = Vec3::from_array(pos).is_finite().then(|| Vec3::from_array(pos)) else { continue };
                            let off = Vec3::new(az.cos(), 0.0, az.sin()) * dist * 1.6;
                            Some(Loc::Space { pos: pos + off })
                        }
                    };
                    if let Some(g) = goal {
                        self.units.get_mut(id).expect("owned").order = Order::Move(g);
                    }
                }
                if stranded {
                    self.notice(player, "Ground units cannot leave their planet.");
                }
            }
            GameCommand::Attack { units, target } => {
                let Some(body) = self.body(target) else { return };
                if body.owner == player {
                    return;
                }
                for id in self.owned_units(player, &units) {
                    let u = &self.units[&id];
                    let planet = match u.loc {
                        Loc::Ground { planet, .. } => Some(planet),
                        Loc::Space { .. } => None,
                    };
                    if let Some(w) = &self.content.unit(u.kind).weapon
                        && can_hit(w, planet, &body)
                    {
                        self.units.get_mut(&id).expect("owned").order = Order::Attack(target);
                    }
                }
            }
            GameCommand::Stop { units } => {
                for id in self.owned_units(player, &units) {
                    self.units.get_mut(&id).expect("owned").order = Order::Idle;
                }
            }
            GameCommand::Build { builders, kind, planet, dir } => {
                if kind as usize >= self.content.buildings.len() {
                    return;
                }
                let Some(dir) = vec(dir) else { return };
                if self.system.planet(planet).is_none() {
                    return;
                }
                let ids: Vec<EntityId> = self
                    .owned_units(player, &builders)
                    .into_iter()
                    .filter(|id| self.content.unit_builds[self.units[id].kind as usize].contains(&kind))
                    .collect();
                if ids.is_empty() {
                    self.notice(player, "None of the selected units can build that.");
                    return;
                }
                if let Some(r) = self.missing_requirement(player, kind) {
                    let name = self.content.building(r).name.clone();
                    self.notice(player, format!("Requires a finished {name}."));
                    return;
                }
                // A colonizer founds the building itself (one ship per order).
                if let Some(ship) = ids.iter().copied().find(|id| self.content.unit(self.units[id].kind).colonizer) {
                    if let Err(e) = self.placement(kind, planet, dir) {
                        self.notice(player, e);
                        return;
                    }
                    self.units.get_mut(&ship).expect("owned").order = Order::Found { kind, planet, dir: dir.normalize() };
                    return;
                }
                let ids: Vec<EntityId> = ids
                    .into_iter()
                    .filter(|id| matches!(self.units[id].loc, Loc::Ground { planet: p, .. } if p == planet))
                    .collect();
                if ids.is_empty() {
                    self.notice(player, "Builders can only build on the planet they stand on.");
                    return;
                }
                let (dir, deposit) = match self.placement(kind, planet, dir) {
                    Ok(x) => x,
                    Err(e) => {
                        self.notice(player, e);
                        return;
                    }
                };
                if !self.pay(player, self.content.building(kind).cost.arr()) {
                    return;
                }
                let b = self.spawn_building(player, kind, planet, dir, deposit);
                for id in ids {
                    self.units.get_mut(&id).expect("owned").order = Order::Build(b);
                }
            }
            GameCommand::Assist { builders, building } => {
                let Some(b) = self.buildings.get(&building) else { return };
                if b.owner != player || b.finished() {
                    return;
                }
                let planet = b.planet;
                for id in self.owned_units(player, &builders) {
                    let u = &self.units[&id];
                    if !self.content.unit_builds[u.kind as usize].is_empty()
                        && matches!(u.loc, Loc::Ground { planet: p, .. } if p == planet)
                    {
                        self.units.get_mut(&id).expect("owned").order = Order::Build(building);
                    }
                }
            }
            GameCommand::Train { building, kind } => {
                let Some(b) = self.buildings.get(&building) else { return };
                if b.owner != player || !b.finished() || !self.content.building_trains[b.kind as usize].contains(&kind) {
                    return;
                }
                if b.queue.len() >= MAX_QUEUE {
                    self.notice(player, "The queue is full.");
                    return;
                }
                if self.pay(player, self.content.unit(kind).cost.arr()) {
                    self.buildings.get_mut(&building).expect("exists").queue.push(kind);
                }
            }
            GameCommand::CancelTrain { building } => {
                let Some(b) = self.buildings.get_mut(&building) else { return };
                if b.owner != player {
                    return;
                }
                if let Some(k) = b.queue.pop() {
                    if b.queue.is_empty() {
                        b.train_time = 0.0;
                        b.stalled = false;
                    }
                    let cost = self.content.unit(k).cost.arr();
                    self.refund(player, cost, 1.0);
                }
            }
            GameCommand::Demolish { building } => {
                let Some(b) = self.buildings.get_mut(&building) else { return };
                if b.owner != player {
                    return;
                }
                b.hp = 0.0;
                let (kind, finished, queue) = (b.kind, b.finished(), std::mem::take(&mut b.queue));
                if !finished {
                    let cost = self.content.building(kind).cost.arr();
                    self.refund(player, cost, 0.75);
                }
                for k in queue {
                    let cost = self.content.unit(k).cost.arr();
                    self.refund(player, cost, 1.0);
                }
            }
            GameCommand::Surrender => {
                if let Some(p) = self.player_mut(player) {
                    p.surrendered = true;
                }
            }
        }
    }

    fn body(&self, id: EntityId) -> Option<Body> {
        if let Some(u) = self.units.get(&id) {
            let def = self.content.unit(u.kind);
            let (space, planet) = match u.loc {
                Loc::Ground { planet, .. } => (false, planet),
                Loc::Space { .. } => (true, NO_PLANET),
            };
            return Some(Body { id, owner: u.owner, pos: self.world_pos(u.loc), space, planet, radius: def.radius });
        }
        self.buildings.get(&id).map(|b| Body {
            id,
            owner: b.owner,
            pos: self.system.planets[b.planet as usize].ground(b.dir),
            space: false,
            planet: b.planet,
            radius: self.content.building(b.kind).radius,
        })
    }

    pub fn step(&mut self) {
        self.tick += 1;
        let dt = TICK_DT;
        self.economy(dt);
        self.training(dt);
        let bodies: Vec<Body> = self.units.keys().chain(self.buildings.keys()).filter_map(|id| self.body(*id)).collect();
        let index: HashMap<EntityId, usize> = bodies.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        let mut damage: Vec<(EntityId, f32)> = Vec::new();
        self.update_units(dt, &bodies, &index, &mut damage);
        self.building_weapons(dt, &bodies, &mut damage);
        self.separate();
        for (id, d) in damage {
            if let Some(u) = self.units.get_mut(&id) {
                u.hp -= d;
            } else if let Some(b) = self.buildings.get_mut(&id) {
                b.hp -= d;
            }
        }
        self.cleanup_dead();
        if self.tick.is_multiple_of(10) {
            self.check_victory();
        }
    }

    fn update_population(&mut self) {
        for p in &mut self.players {
            p.pop = 0;
            p.pop_cap = 0;
        }
        for u in self.units.values() {
            let pop = self.content.unit(u.kind).pop;
            if let Some(p) = self.players.iter_mut().find(|p| p.id == u.owner) {
                p.pop += pop;
            }
        }
        for b in self.buildings.values().filter(|b| b.finished()) {
            let pop = self.content.building(b.kind).pop;
            if let Some(p) = self.players.iter_mut().find(|p| p.id == b.owner) {
                p.pop_cap = (p.pop_cap + pop).min(POP_LIMIT);
            }
        }
    }

    fn economy(&mut self, dt: f32) {
        for p in &mut self.players {
            p.income = [0.0; 3];
        }
        let mut gains: Vec<(PlayerId, [f32; 3])> = Vec::new();
        for b in self.buildings.values().filter(|b| b.finished()) {
            let def = self.content.building(b.kind);
            let mut rate = def.income.arr();
            if let Some(d) = b.deposit {
                let left = self.deposits[d as usize];
                if left > 0.0 {
                    let take = (def.rate * dt).min(left);
                    self.deposits[d as usize] = left - take;
                    self.changed_deposits.insert(d);
                    rate[self.system.deposits[d as usize].kind.idx()] += take / dt;
                }
            }
            gains.push((b.owner, rate));
        }
        for (owner, rate) in gains {
            if let Some(p) = self.player_mut(owner) {
                for (i, r) in rate.iter().enumerate() {
                    p.stock[i] += r * dt;
                    p.income[i] += r;
                }
            }
        }
        self.update_population();
    }

    fn training(&mut self, dt: f32) {
        let ids: Vec<EntityId> = self.buildings.values().filter(|b| b.finished() && !b.queue.is_empty()).map(|b| b.id).collect();
        for id in ids {
            let b = &self.buildings[&id];
            let (owner, kind) = (b.owner, b.queue[0]);
            let def = self.content.unit(kind);
            let (pop, time) = (def.pop, def.time);
            let p = self.player(owner).expect("owner exists");
            if p.pop + pop > p.pop_cap {
                let b = self.buildings.get_mut(&id).expect("exists");
                if !b.stalled {
                    b.stalled = true;
                    self.notice(owner, "Not enough housing — build a Habitat Dome.");
                }
                continue;
            }
            let b = self.buildings.get_mut(&id).expect("exists");
            b.stalled = false;
            b.train_time += dt;
            if b.train_time < time {
                continue;
            }
            b.train_time = 0.0;
            b.queue.remove(0);
            let (planet, dir, bkind) = (b.planet, b.dir, b.kind);
            let p = &self.system.planets[planet as usize];
            let around = id as f32 * 2.4 + self.tick as f32 * 0.37;
            let loc = match self.content.unit(kind).domain {
                Domain::Ground => {
                    let r = self.content.building(bkind).radius + 2.5;
                    Loc::Ground { planet, dir: geo::offset(dir, around, r / p.radius) }
                }
                Domain::Space => Loc::Space { pos: p.orbit_point(geo::offset(dir, around, 4.0 / p.radius)) },
            };
            self.spawn_unit(owner, kind, loc);
            if let Some(pl) = self.player_mut(owner) {
                pl.pop += pop;
            }
            let name = self.content.unit(kind).name.clone();
            self.notice(owner, format!("{name} ready."));
        }
    }

    fn update_units(&mut self, dt: f32, bodies: &[Body], index: &HashMap<EntityId, usize>, damage: &mut Vec<(EntityId, f32)>) {
        let ids: Vec<EntityId> = self.units.keys().copied().collect();
        let mut founded: Vec<(EntityId, PlayerId, BuildingKind, u16, Vec3)> = Vec::new();
        let mut progress: Vec<(EntityId, f32)> = Vec::new();
        for id in ids {
            let u = self.units[&id].clone();
            let def = self.content.unit(u.kind);
            let speed = def.speed * dt;
            let here_planet = match u.loc {
                Loc::Ground { planet, .. } => Some(planet),
                Loc::Space { .. } => None,
            };
            let my_pos = self.world_pos(u.loc);
            let mut loc = u.loc;
            let mut order = u.order.clone();
            let mut cooldown = (u.cooldown - dt).max(0.0);
            let mut fire_at: Option<Body> = None;

            match &u.order {
                Order::Idle => {
                    if let Some(w) = &def.weapon {
                        fire_at = nearest_enemy(bodies, u.owner, my_pos, w, here_planet);
                    }
                }
                Order::Move(goal) => {
                    if self.move_towards(&mut loc, *goal, speed, 0.0) {
                        order = Order::Idle;
                    }
                }
                Order::Attack(t) => match (index.get(t).map(|i| bodies[*i]), &def.weapon) {
                    (Some(target), Some(w)) if can_hit(w, here_planet, &target) => {
                        if my_pos.distance(target.pos) <= w.range + target.radius {
                            fire_at = Some(target);
                        } else {
                            let goal = match (here_planet, target.space) {
                                (Some(_), false) => {
                                    let p = &self.system.planets[target.planet as usize];
                                    Some(Loc::Ground { planet: target.planet, dir: (target.pos - p.center).normalize() })
                                }
                                // Ground units cannot chase ships.
                                (Some(_), true) => None,
                                (None, true) => Some(Loc::Space { pos: target.pos }),
                                (None, false) => {
                                    let p = &self.system.planets[target.planet as usize];
                                    Some(Loc::Space { pos: p.orbit_point((target.pos - p.center).normalize()) })
                                }
                            };
                            match goal {
                                Some(g) => {
                                    self.move_towards(&mut loc, g, speed, w.range * 0.7);
                                }
                                None => order = Order::Idle,
                            }
                        }
                    }
                    _ => order = Order::Idle,
                },
                Order::Build(bid) => match self.buildings.get(bid) {
                    Some(b) if !b.finished() && b.owner == u.owner && here_planet == Some(b.planet) => {
                        let reach = self.content.building(b.kind).radius + def.radius + 1.5;
                        let goal = Loc::Ground { planet: b.planet, dir: b.dir };
                        if self.move_towards(&mut loc, goal, speed, reach) {
                            progress.push((*bid, dt / self.content.building(b.kind).time));
                        }
                    }
                    _ => order = Order::Idle,
                },
                Order::Found { kind, planet, dir } => {
                    let p = &self.system.planets[*planet as usize];
                    if self.move_towards(&mut loc, Loc::Space { pos: p.orbit_point(*dir) }, speed, 1.0) {
                        founded.push((id, u.owner, *kind, *planet, *dir));
                        order = Order::Idle;
                    }
                }
            }

            if let (Some(target), Some(w)) = (fire_at, &def.weapon)
                && cooldown <= 0.0
            {
                damage.push((target.id, w.damage));
                self.shots.push(Shot { from: id, to: target.id });
                cooldown = w.cooldown;
            }
            let u = self.units.get_mut(&id).expect("exists");
            u.loc = loc;
            u.order = order;
            u.cooldown = cooldown;
        }
        for (bid, amount) in progress {
            if let Some(b) = self.buildings.get_mut(&bid) {
                let max = self.content.building(b.kind).hp;
                let before = b.progress;
                b.progress = (b.progress + amount).min(1.0);
                b.hp = (b.hp + max * 0.9 * (b.progress - before)).min(max);
                if before < 1.0 && b.finished() {
                    let (owner, name) = (b.owner, self.content.building(b.kind).name.clone());
                    self.notice(owner, format!("{name} complete."));
                }
            }
        }
        for (ship, owner, kind, planet, dir) in founded {
            match self.placement(kind, planet, dir) {
                Ok((dir, deposit)) => {
                    let id = self.spawn_building(owner, kind, planet, dir, deposit);
                    let b = self.buildings.get_mut(&id).expect("just spawned");
                    b.progress = 1.0;
                    b.hp = self.content.building(kind).hp;
                    self.units.remove(&ship);
                    let (name, pname) = (self.content.building(kind).name.clone(), self.system.planets[planet as usize].name.clone());
                    self.notice(owner, format!("{name} founded on {pname}."));
                }
                Err(e) => self.notice(owner, format!("Cannot land: {e}")),
            }
        }
    }

    /// Moves `loc` towards `goal` by `step` metres, stopping `stop` metres short.
    /// Returns true when arrived.
    fn move_towards(&self, loc: &mut Loc, goal: Loc, step: f32, stop: f32) -> bool {
        match (*loc, goal) {
            (Loc::Ground { planet, dir }, Loc::Ground { planet: gp, dir: gdir }) if planet == gp => {
                let r = self.system.planets[planet as usize].radius;
                let remaining = geo::angle_between(dir, gdir) * r - stop;
                if remaining <= 0.0 {
                    return true;
                }
                let (d, done) = geo::step_towards(dir, gdir, step.min(remaining) / r);
                *loc = Loc::Ground { planet, dir: d };
                done || remaining <= step
            }
            (Loc::Space { pos }, Loc::Space { pos: gpos }) => {
                let to = gpos - pos;
                let remaining = to.length() - stop;
                if remaining <= 0.0 {
                    return true;
                }
                let mut next = pos + to.normalize() * step.min(remaining);
                // Slide around planets and the sun instead of flying through them.
                for p in &self.system.planets {
                    let keep = p.radius + p.relief + ORBIT_ALTITUDE * 0.6;
                    let off = next - p.center;
                    if off.length() < keep {
                        next = p.center + off.normalize_or(Vec3::Y) * keep;
                    }
                }
                let keep = SUN_RADIUS + 25.0;
                if next.length() < keep {
                    next = next.normalize_or(Vec3::Y) * keep;
                }
                *loc = Loc::Space { pos: next };
                remaining <= step
            }
            _ => true,
        }
    }

    fn building_weapons(&mut self, dt: f32, bodies: &[Body], damage: &mut Vec<(EntityId, f32)>) {
        let system = self.system.clone();
        for b in self.buildings.values_mut() {
            let Some(w) = &self.content.building(b.kind).weapon else { continue };
            b.cooldown = (b.cooldown - dt).max(0.0);
            if !b.finished() || b.cooldown > 0.0 {
                continue;
            }
            let pos = system.planets[b.planet as usize].ground(b.dir);
            if let Some(t) = nearest_enemy(bodies, b.owner, pos, w, Some(b.planet)) {
                damage.push((t.id, w.damage));
                self.shots.push(Shot { from: b.id, to: t.id });
                b.cooldown = w.cooldown;
            }
        }
    }

    /// Pushes overlapping ground units apart along the surface, and ships apart in space.
    fn separate(&mut self) {
        let list: Vec<(EntityId, Loc, f32)> =
            self.units.values().map(|u| (u.id, u.loc, self.content.unit(u.kind).radius)).collect();
        let mut push: HashMap<EntityId, Vec3> = HashMap::new();
        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                let (a, b) = (&list[i], &list[j]);
                match (a.1, b.1) {
                    (Loc::Ground { planet: pa, dir: da }, Loc::Ground { planet: pb, dir: db }) if pa == pb => {
                        let r = self.system.planets[pa as usize].radius;
                        let d = (db - da) * r;
                        let min = a.2 + b.2;
                        let len = d.length();
                        if len < min {
                            let n = if len > 1e-4 { d / len } else { geo::any_perpendicular(da) };
                            let f = n * (min - len) * 0.25 / r;
                            *push.entry(a.0).or_default() -= f;
                            *push.entry(b.0).or_default() += f;
                        }
                    }
                    (Loc::Space { pos: pa }, Loc::Space { pos: pb }) => {
                        let d = pb - pa;
                        let min = a.2 + b.2 + 1.0;
                        let len = d.length();
                        if len < min {
                            let n = if len > 1e-4 { d / len } else { Vec3::X };
                            let f = n * (min - len) * 0.25;
                            *push.entry(a.0).or_default() -= f;
                            *push.entry(b.0).or_default() += f;
                        }
                    }
                    _ => {}
                }
            }
        }
        for (id, p) in push {
            if let Some(u) = self.units.get_mut(&id) {
                u.loc = match u.loc {
                    Loc::Ground { planet, dir } => Loc::Ground { planet, dir: (dir + p).normalize() },
                    Loc::Space { pos } => Loc::Space { pos: pos + p },
                };
            }
        }
    }

    fn cleanup_dead(&mut self) {
        self.units.retain(|_, u| u.hp > 0.0);
        let dead: Vec<(PlayerId, BuildingKind, Vec<UnitKind>)> =
            self.buildings.values().filter(|b| b.hp <= 0.0).map(|b| (b.owner, b.kind, b.queue.clone())).collect();
        self.buildings.retain(|_, b| b.hp > 0.0);
        for (owner, kind, _) in dead {
            let name = self.content.building(kind).name.clone();
            self.notice(owner, format!("{name} destroyed."));
        }
    }

    fn check_victory(&mut self) {
        if self.winner.is_some() {
            return;
        }
        for i in 0..self.players.len() {
            let id = self.players[i].id;
            if self.players[i].defeated {
                continue;
            }
            let has_core = self.buildings.values().any(|b| b.owner == id && self.content.building(b.kind).core);
            let has_colonizer = self.units.values().any(|u| u.owner == id && self.content.unit(u.kind).colonizer);
            if self.players[i].surrendered || (!has_core && !has_colonizer) {
                self.players[i].defeated = true;
                self.units.retain(|_, u| u.owner != id);
                self.buildings.retain(|_, b| b.owner != id);
            }
        }
        let alive: Vec<PlayerId> = self.players.iter().filter(|p| !p.defeated).map(|p| p.id).collect();
        if self.players.len() >= 2 && alive.len() <= 1 {
            self.winner = Some(alive.first().copied());
        } else if alive.is_empty() {
            self.winner = Some(None);
        }
    }

    /// Removes a player who left (counts as surrender).
    pub fn remove_player(&mut self, id: PlayerId) {
        if let Some(p) = self.player_mut(id) {
            p.surrendered = true;
        }
        self.check_victory();
    }

    /// The state for the clients; drains the shots and changed deposits since the last call.
    pub fn snapshot(&mut self) -> Snapshot {
        let units = self
            .units
            .values()
            .map(|u| {
                let (planet, pos) = match u.loc {
                    Loc::Ground { planet, dir } => (planet, arr(dir)),
                    Loc::Space { pos } => (NO_PLANET, arr(pos)),
                };
                let activity = match u.order {
                    Order::Idle => Activity::Idle,
                    Order::Move(_) | Order::Found { .. } => Activity::Moving,
                    Order::Attack(_) => Activity::Attacking,
                    Order::Build(_) => Activity::Building,
                };
                UnitSnap { id: u.id, owner: u.owner, kind: u.kind, planet, pos, hp: u.hp, activity }
            })
            .collect();
        let buildings = self
            .buildings
            .values()
            .map(|b| BuildingSnap {
                id: b.id,
                owner: b.owner,
                kind: b.kind,
                planet: b.planet,
                dir: arr(b.dir),
                hp: b.hp,
                progress: b.progress,
                queue: b.queue.clone(),
                train_progress: b.queue.first().map(|k| b.train_time / self.content.unit(*k).time).unwrap_or(0.0),
            })
            .collect();
        let players = self
            .players
            .iter()
            .map(|p| PlayerSnap { id: p.id, stock: p.stock, income: p.income, pop: p.pop, pop_cap: p.pop_cap, defeated: p.defeated })
            .collect();
        let deposits = std::mem::take(&mut self.changed_deposits).into_iter().map(|d| (d, self.deposits[d as usize])).collect();
        Snapshot { tick: self.tick, players, units, buildings, deposits, shots: std::mem::take(&mut self.shots) }
    }
}

fn nearest_enemy(bodies: &[Body], owner: PlayerId, pos: Vec3, w: &Weapon, planet: Option<u16>) -> Option<Body> {
    bodies
        .iter()
        .filter(|b| b.owner != owner && can_hit(w, planet, b))
        .map(|b| (b.pos.distance(pos) - b.radius, b))
        .filter(|(d, _)| *d <= w.range)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, b)| *b)
}
