//! Deterministic star-system generation.
//!
//! A sun sits at the origin. Every player gets an identical **home planet** on a ring around
//! it; neutral planets are generated for one sector and rotated into every other sector, so
//! the system is rotationally symmetric and fair. Resource deposits cover the **whole globe**
//! of every planet (Fibonacci-sphere spread); home planets additionally get a guaranteed
//! cluster next to the start.
//!
//! Generated identically on the server and on every client from the seed (see `rng.rs` for
//! the determinism rules); `StarSystem::hash` is compared after loading.

use std::f32::consts::{PI, TAU};

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::content::{FNV_OFFSET, fnv1a};
use crate::geo;
use crate::rng::{Noise, Rng};
use crate::settings::MatchSettings;
use crate::Res;

pub const SUN_RADIUS: f32 = 110.0;
/// Height above the surface at which ships hover over a planet.
pub const ORBIT_ALTITUDE: f32 = 16.0;
pub const HOME_RADIUS: f32 = 60.0;
pub const NO_PLANET: u16 = u16::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    Terran,
    Desert,
    Ice,
    Volcanic,
    Barren,
    Jungle,
    Toxic,
}

impl Biome {
    pub const ALL: [Biome; 7] =
        [Biome::Terran, Biome::Desert, Biome::Ice, Biome::Volcanic, Biome::Barren, Biome::Jungle, Biome::Toxic];

    pub fn label(self) -> &'static str {
        match self {
            Biome::Terran => "Terran",
            Biome::Desert => "Desert",
            Biome::Ice => "Ice",
            Biome::Volcanic => "Volcanic",
            Biome::Barren => "Barren",
            Biome::Jungle => "Jungle",
            Biome::Toxic => "Toxic",
        }
    }

    /// Elevation (in [-1, 1]) below which the surface is a flat sea (water, ice, lava, acid).
    pub fn sea_level(self) -> Option<f32> {
        match self {
            Biome::Terran => Some(-0.05),
            Biome::Jungle => Some(-0.12),
            Biome::Ice => Some(-0.1),
            Biome::Volcanic => Some(-0.25),
            Biome::Toxic => Some(-0.15),
            Biome::Desert | Biome::Barren => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Planet {
    pub id: u16,
    pub name: String,
    pub center: Vec3,
    pub radius: f32,
    pub biome: Biome,
    /// Seed for the terrain noise.
    pub seed: u64,
    /// Index of the player (in match order) whose home this is.
    pub home: Option<u8>,
    /// Where the command center of the home player stands (unit direction).
    pub start_dir: Vec3,
    /// Height of the terrain relief in metres.
    pub relief: f32,
}

impl Planet {
    /// Terrain elevation in [-1, 1] for a unit direction (smooth, seamless on the sphere).
    pub fn elevation(&self, dir: Vec3) -> f32 {
        let n = Noise::new(self.seed);
        let p = dir * 2.3;
        let base = n.fbm(p.x + 11.0, p.y - 3.0, p.z + 7.0, 6);
        let q = dir * 3.1;
        let ridges = n.ridged(q.x - 5.0, q.y + 9.0, q.z + 2.0, 4);
        let e = base * 0.75 + (ridges - 0.35) * 0.6;
        // Flatten the start area so the base sits on even ground.
        let flat = if self.home.is_some() {
            crate::rng::smoothstep(0.10, 0.30, geo::angle_between(dir, self.start_dir))
        } else {
            1.0
        };
        (e * flat).clamp(-1.0, 1.0)
    }

    /// Height above the base radius in metres.
    pub fn height(&self, dir: Vec3) -> f32 {
        let e = self.elevation(dir);
        match self.biome.sea_level() {
            Some(sea) => e.max(sea) * self.relief,
            None => e * self.relief,
        }
    }

    /// World position of the terrain surface in direction `dir`.
    pub fn surface(&self, dir: Vec3) -> Vec3 {
        self.center + dir * (self.radius + self.height(dir))
    }

    /// World position on the base sphere (used by the simulation; the terrain is cosmetic).
    pub fn ground(&self, dir: Vec3) -> Vec3 {
        self.center + dir * self.radius
    }

    /// Where ships hover over `dir`.
    pub fn orbit_point(&self, dir: Vec3) -> Vec3 {
        self.center + dir * (self.radius + self.relief + ORBIT_ALTITUDE)
    }

    /// Surface distance in metres between two directions.
    pub fn arc(&self, a: Vec3, b: Vec3) -> f32 {
        geo::angle_between(a, b) * self.radius
    }
}

#[derive(Clone, Debug)]
pub struct Deposit {
    pub id: u32,
    pub planet: u16,
    pub dir: Vec3,
    pub kind: Res,
    pub amount: f32,
}

#[derive(Clone, Debug)]
pub struct StarSystem {
    pub planets: Vec<Planet>,
    pub deposits: Vec<Deposit>,
    pub hash: u64,
}

impl StarSystem {
    pub fn home_of(&self, slot: u8) -> Option<&Planet> {
        self.planets.iter().find(|p| p.home == Some(slot))
    }

    pub fn planet(&self, id: u16) -> Option<&Planet> {
        self.planets.get(id as usize)
    }

    /// The planet whose sphere (including relief) the ray hits first, and the hit direction.
    pub fn pick(&self, origin: Vec3, ray: Vec3) -> Option<(u16, Vec3)> {
        let mut best: Option<(f32, u16)> = None;
        for p in &self.planets {
            if let Some(t) = geo::ray_sphere(origin, ray, p.center, p.radius + p.relief * 0.3)
                && best.is_none_or(|(bt, _)| t < bt)
            {
                best = Some((t, p.id));
            }
        }
        best.map(|(t, id)| {
            let p = &self.planets[id as usize];
            (id, (origin + ray * t - p.center).normalize())
        })
    }
}

const NAMES: [&str; 32] = [
    "Aurelia", "Boreas", "Caldera", "Draxis", "Elysium", "Fenris", "Gaia Prime", "Helion", "Icarus", "Juno",
    "Kepler", "Lumen", "Midas", "Nyx", "Orion", "Pyra", "Quorra", "Rhea", "Solace", "Tethys", "Umbra", "Vesta",
    "Wraith", "Xandar", "Ymir", "Zephyr", "Arcadia", "Brontes", "Cygnus", "Delphi", "Erebus", "Faros",
];

/// A planet before it is copied into each sector.
struct Template {
    center: Vec3,
    radius: f32,
    biome: Biome,
    seed: u64,
    /// Relative deposit layout (canonical frame).
    deposits: Vec<(Vec3, Res, f32)>,
}

fn rich_kind(rng: &mut Rng, weights: [f32; 3]) -> Res {
    let total: f32 = weights.iter().sum();
    let mut x = rng.f32() * total;
    for (i, w) in weights.iter().enumerate() {
        if x < *w {
            return Res::ALL[i];
        }
        x -= w;
    }
    Res::Metal
}

fn base_amount(kind: Res) -> f32 {
    match kind {
        Res::Metal => 2500.0,
        Res::Crystal => 1800.0,
        Res::Gas => 1500.0,
    }
}

/// Deposits spread over the whole globe. `start` (canonical +Y) keeps its surroundings free
/// for the base and gets a guaranteed cluster.
fn deposit_pattern(rng: &mut Rng, radius: f32, weights: [f32; 3], abundance: f32, home: bool) -> Vec<(Vec3, Res, f32)> {
    let mut out = Vec::new();
    let count = ((radius * radius * 0.0125) as usize).max(8);
    let golden = PI * (3.0 - 5.0f32.sqrt());
    for i in 0..count {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let th = golden * i as f32;
        let jitter = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * 0.12;
        let dir = (Vec3::new(libm::cosf(th) * r, y, libm::sinf(th) * r) + jitter).normalize();
        if home && geo::angle_between(dir, Vec3::Y) < 0.42 {
            continue;
        }
        let kind = rich_kind(rng, weights);
        let amount = base_amount(kind) * abundance * rng.range(0.8, 1.25);
        out.push((dir, kind, amount));
    }
    if home {
        // The guaranteed cluster around the start: 3 metal, 2 crystal, 1 gas.
        let ring = [
            (0.0, Res::Metal),
            (0.9, Res::Metal),
            (1.8, Res::Metal),
            (2.9, Res::Crystal),
            (3.9, Res::Crystal),
            (5.1, Res::Gas),
        ];
        for (az, kind) in ring {
            let ang = 0.26 + rng.range(0.0, 0.05);
            let dir = (Vec3::Y * libm::cosf(ang)
                + (Vec3::X * libm::cosf(az) + Vec3::Z * libm::sinf(az)) * libm::sinf(ang))
            .normalize();
            out.push((dir, kind, base_amount(kind) * abundance * 1.2));
        }
    }
    out
}

/// Generates the star system for `players` players (at least one).
pub fn generate(settings: &MatchSettings, players: usize) -> StarSystem {
    let p = players.max(1);
    let root = Rng::new(settings.seed);
    let abundance = settings.abundance.factor();
    let ring = 520.0 + 110.0 * p as f32;
    let rot = root.fork(1).angle();
    let sector = TAU / p as f32;

    let mut names: Vec<&str> = NAMES.to_vec();
    let mut nr = root.fork(9);
    for i in (1..names.len()).rev() {
        let j = nr.range_u32(0, i as u32 + 1) as usize;
        names.swap(i, j);
    }
    let mut biomes: Vec<Biome> = Biome::ALL.to_vec();
    for i in (1..biomes.len()).rev() {
        let j = nr.range_u32(0, i as u32 + 1) as usize;
        biomes.swap(i, j);
    }

    // Home planet in sector 0 (all others are rotated copies).
    let home_center = Vec3::new(libm::cosf(rot) * ring, 0.0, libm::sinf(rot) * ring);
    let mut hr = root.fork(2);
    let home_deposits = deposit_pattern(&mut hr, HOME_RADIUS, [0.5, 0.3, 0.2], abundance, true);
    let home_seed = hr.next_u64();

    // Neutral planets for sector 0.
    let mut tr = root.fork(3);
    let mut templates: Vec<Template> = Vec::new();
    let per = settings.size.neutrals_per_player();
    let mut tries = 0;
    while templates.len() < per && tries < 400 {
        tries += 1;
        let dist = ring * tr.range(0.45, 1.55);
        let ang = rot + sector * tr.range(0.22, 0.78) + if p == 1 { tr.range(0.0, TAU) } else { 0.0 };
        let y = tr.range(-70.0, 70.0);
        let radius = tr.range(26.0, 52.0);
        let center = Vec3::new(libm::cosf(ang) * dist, y, libm::sinf(ang) * dist);
        if dist - radius < SUN_RADIUS * 2.2 {
            continue;
        }
        // Keep clear of every home, every other neutral and their rotated copies.
        let mut ok = true;
        for s in 0..p {
            let q = geo::rot_y(sector * s as f32);
            if (q * home_center).distance(center) < HOME_RADIUS + radius + 140.0 {
                ok = false;
            }
            for t in &templates {
                if (q * t.center).distance(center) < t.radius + radius + 110.0 {
                    ok = false;
                }
            }
        }
        // A neutral must not collide with its own copies either.
        for s in 1..p {
            if (geo::rot_y(sector * s as f32) * center).distance(center) < radius * 2.0 + 110.0 {
                ok = false;
            }
        }
        if !ok {
            continue;
        }
        let biome = Biome::ALL[tr.range_u32(0, Biome::ALL.len() as u32) as usize];
        // Neutral planets are specialised: rich in one resource.
        let mut weights = [0.25, 0.25, 0.25];
        weights[tr.range_u32(0, 3) as usize] = 0.9;
        let deposits = deposit_pattern(&mut tr, radius, weights, abundance * 1.3, false);
        templates.push(Template { center, radius, biome, seed: tr.next_u64(), deposits });
    }

    let mut planets = Vec::new();
    let mut deposits = Vec::new();
    let add_deposits = |planet: u16, q: Quat, list: &[(Vec3, Res, f32)], deposits: &mut Vec<Deposit>| {
        for (d, kind, amount) in list {
            let id = deposits.len() as u32;
            deposits.push(Deposit { id, planet, dir: (q * *d).normalize(), kind: *kind, amount: amount.floor() });
        }
    };
    for s in 0..p {
        let q = geo::rot_y(sector * s as f32);
        let center = q * home_center;
        // The base faces the sun (lit side), tilted towards the north.
        let to_sun = -center.normalize();
        let start_dir = (to_sun * libm::cosf(0.45) + Vec3::Y * libm::sinf(0.45)).normalize();
        let id = planets.len() as u16;
        planets.push(Planet {
            id,
            name: names[s % names.len()].to_string(),
            center,
            radius: HOME_RADIUS,
            biome: biomes[s % biomes.len()],
            seed: home_seed,
            home: Some(s as u8),
            start_dir,
            relief: HOME_RADIUS * 0.035,
        });
        add_deposits(id, Quat::from_rotation_arc(Vec3::Y, start_dir), &home_deposits, &mut deposits);
    }
    for s in 0..p {
        let q = geo::rot_y(sector * s as f32);
        for t in &templates {
            let id = planets.len() as u16;
            planets.push(Planet {
                id,
                name: names[(p + id as usize) % names.len()].to_string(),
                center: q * t.center,
                radius: t.radius,
                biome: t.biome,
                seed: t.seed,
                home: None,
                start_dir: Vec3::Y,
                relief: t.radius * 0.045,
            });
            add_deposits(id, q, &t.deposits, &mut deposits);
        }
    }

    let mut h = FNV_OFFSET;
    for pl in &planets {
        for v in [pl.center.x, pl.center.y, pl.center.z, pl.radius] {
            h = fnv1a(h, &v.to_bits().to_le_bytes());
        }
    }
    for d in &deposits {
        for v in [d.dir.x, d.dir.y, d.dir.z, d.amount] {
            h = fnv1a(h, &v.to_bits().to_le_bytes());
        }
        h = fnv1a(h, &[d.kind as u8]);
    }
    StarSystem { planets, deposits, hash: h }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SystemSize;

    fn settings(seed: u64, size: SystemSize) -> MatchSettings {
        MatchSettings { seed, size, ..Default::default() }
    }

    #[test]
    fn deterministic() {
        let a = generate(&settings(5, SystemSize::Medium), 3);
        let b = generate(&settings(5, SystemSize::Medium), 3);
        assert_eq!(a.hash, b.hash);
        let c = generate(&settings(6, SystemSize::Medium), 3);
        assert_ne!(a.hash, c.hash);
    }

    #[test]
    fn every_player_has_a_home_with_resources_all_over() {
        for players in 1..=8 {
            for size in SystemSize::ALL {
                let sys = generate(&settings(42 + players as u64, size), players);
                for slot in 0..players as u8 {
                    let home = sys.home_of(slot).expect("home planet");
                    let deps: Vec<_> = sys.deposits.iter().filter(|d| d.planet == home.id).collect();
                    assert!(deps.len() >= 30, "{} deposits", deps.len());
                    // Near the start: every resource.
                    for r in Res::ALL {
                        assert!(
                            deps.iter().any(|d| d.kind == r && geo::angle_between(d.dir, home.start_dir) < 0.4),
                            "no {r:?} near the start"
                        );
                    }
                    // And on the far side of the globe too.
                    assert!(deps.iter().any(|d| d.dir.dot(home.start_dir) < -0.5));
                    // The start faces the sun.
                    assert!(home.start_dir.dot(-home.center.normalize()) > 0.5);
                }
                assert_eq!(sys.planets.len(), players * (1 + size.neutrals_per_player()), "{players} {size:?}");
            }
        }
    }

    #[test]
    fn planets_do_not_overlap() {
        for seed in 0..30 {
            for players in [2, 4, 8] {
                let sys = generate(&settings(seed, SystemSize::Large), players);
                for a in &sys.planets {
                    assert!(a.center.length() - a.radius > SUN_RADIUS * 1.5, "planet in the sun");
                    for b in &sys.planets {
                        if a.id < b.id {
                            assert!(a.center.distance(b.center) > a.radius + b.radius + 40.0, "seed {seed}: overlap");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn homes_are_identical() {
        let sys = generate(&settings(9, SystemSize::Small), 4);
        let count = |slot: u8| {
            let id = sys.home_of(slot).unwrap().id;
            let mut c = [0usize; 3];
            let mut amount = 0.0;
            for d in sys.deposits.iter().filter(|d| d.planet == id) {
                c[d.kind.idx()] += 1;
                amount += d.amount;
            }
            (c, amount as u64)
        };
        let first = count(0);
        for s in 1..4 {
            assert_eq!(count(s), first);
        }
    }
}
