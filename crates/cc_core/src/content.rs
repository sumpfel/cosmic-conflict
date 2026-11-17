//! Game content (units, buildings, research, combat rules), defined in `data/*.ron` and embedded
//! at compile time.
//!
//! String ids in the files are resolved to compact numeric kinds (`u16` indices) and validated
//! when the database is loaded. The content hash is compared between client and server.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::Res;
use crate::research::{Stat, TechDef, TechKind};

pub type UnitKind = u16;
pub type BuildingKind = u16;

/// An amount of every resource (costs, income per second).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cost {
    pub metal: f32,
    pub crystal: f32,
    pub gas: f32,
}

impl Cost {
    pub fn get(&self, r: Res) -> f32 {
        match r {
            Res::Metal => self.metal,
            Res::Crystal => self.crystal,
            Res::Gas => self.gas,
        }
    }

    pub fn arr(&self) -> [f32; 3] {
        [self.metal, self.crystal, self.gas]
    }

    pub fn is_zero(&self) -> bool {
        self.metal == 0.0 && self.crystal == 0.0 && self.gas == 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Domain {
    /// Walks on a planet's surface.
    Ground,
    /// Flies through space (and hovers in low orbit over planets).
    Space,
}

/// What a weapon does to armour and shields (multipliers in `data/combat.ron`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DamageKind {
    /// Bullets, cannon shells, railgun slugs.
    #[default]
    Kinetic,
    /// Beams: precise, good against light targets, reflected by heavy plating.
    Laser,
    /// Missiles, bombs, shells: cracks buildings, usually with splash.
    Explosive,
    /// Superheated plasma: melts heavy armour.
    Plasma,
    /// Ion / EMP: tears down shields, weak against bare hulls.
    Ion,
}

impl DamageKind {
    pub const ALL: [DamageKind; 5] = [DamageKind::Kinetic, DamageKind::Laser, DamageKind::Explosive, DamageKind::Plasma, DamageKind::Ion];

    pub fn label(self) -> &'static str {
        match self {
            DamageKind::Kinetic => "Kinetic",
            DamageKind::Laser => "Laser",
            DamageKind::Explosive => "Explosive",
            DamageKind::Plasma => "Plasma",
            DamageKind::Ion => "Ion",
        }
    }
}

/// How a target's hull takes damage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ArmorClass {
    /// Infantry, drones, scouts, probes.
    #[default]
    Light,
    /// Walkers, robots, fighters, frigates.
    Medium,
    /// Tanks, heavy mechs, capital ships.
    Heavy,
    /// Buildings.
    Structure,
}

impl ArmorClass {
    pub const ALL: [ArmorClass; 4] = [ArmorClass::Light, ArmorClass::Medium, ArmorClass::Heavy, ArmorClass::Structure];

    pub fn label(self) -> &'static str {
        match self {
            ArmorClass::Light => "Light",
            ArmorClass::Medium => "Medium",
            ArmorClass::Heavy => "Heavy",
            ArmorClass::Structure => "Structure",
        }
    }
}

fn structure() -> ArmorClass {
    ArmorClass::Structure
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weapon {
    pub damage: f32,
    pub range: f32,
    /// Seconds between shots.
    pub cooldown: f32,
    /// Can hit targets on a planet's surface.
    pub ground: bool,
    /// Can hit ships.
    pub space: bool,
    #[serde(default)]
    pub kind: DamageKind,
    /// Radius (m) around the target in which other enemies take `CombatRules::splash_damage`.
    #[serde(default)]
    pub splash: f32,
}

/// Detection ranges in metres. The simulation does not hide anything yet (fog of war is a
/// separate feature); these values say what each sensor is meant to reveal:
///
/// * `sight`: optical — full information within range, blocked by the planet's horizon.
/// * `radar`: contacts on the planet's surface and in low orbit, through the horizon, without
///   unit types (blips).
/// * `seismic`: moving ground units on the *same planet* through the horizon.
/// * `deep`: ships in open space (long range, no ground contacts).
/// * `detect`: reveals stealthed units. The simulation already uses this one: a stealthed unit
///   cannot be targeted by a player unless one of their detectors is in range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sensors {
    pub sight: f32,
    pub radar: f32,
    pub seismic: f32,
    pub deep: f32,
    pub detect: f32,
}

impl Sensors {
    /// The largest range of any sensor.
    pub fn max_range(&self) -> f32 {
        self.sight.max(self.radar).max(self.seismic).max(self.deep).max(self.detect)
    }
}

/// An effect on the owner's own things nearby (shield generators, repair robots).
/// Affects buildings and ground units on the same planet (ships only when the source flies).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Aura {
    pub radius: f32,
    /// Extra shield capacity for everything in range (the strongest aura counts, no stacking).
    pub shield: f32,
    /// Shield points per second recharged by the aura.
    pub shield_regen: f32,
    /// Hit points per second restored to damaged units and finished buildings in range.
    pub repair: f32,
}

/// Where a building is listed in the build menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BuildGroup {
    #[default]
    Economy,
    Production,
    Research,
    Defence,
    Sensors,
}

impl BuildGroup {
    pub const ALL: [BuildGroup; 5] = [BuildGroup::Economy, BuildGroup::Production, BuildGroup::Research, BuildGroup::Defence, BuildGroup::Sensors];

    pub fn label(self) -> &'static str {
        match self {
            BuildGroup::Economy => "Economy",
            BuildGroup::Production => "Production",
            BuildGroup::Research => "Research",
            BuildGroup::Defence => "Defence",
            BuildGroup::Sensors => "Sensors",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub domain: Domain,
    pub cost: Cost,
    /// Training time in seconds.
    pub time: f32,
    pub pop: u32,
    pub hp: f32,
    #[serde(default)]
    pub armor_class: ArmorClass,
    /// Subtracted from every hit on the hull (see `CombatRules::min_damage`).
    #[serde(default)]
    pub armor: f32,
    /// Shield points that absorb damage before the hull.
    #[serde(default)]
    pub shield: f32,
    /// Shield points per second, after `CombatRules::shield_delay` seconds without a hit.
    #[serde(default)]
    pub shield_regen: f32,
    /// Metres per second.
    pub speed: f32,
    /// Collision / selection radius in metres.
    pub radius: f32,
    #[serde(default)]
    pub weapon: Option<Weapon>,
    #[serde(default)]
    pub sensors: Sensors,
    /// Cannot be targeted unless an enemy detector is in range (firing reveals it briefly).
    #[serde(default)]
    pub stealth: bool,
    #[serde(default)]
    pub aura: Option<Aura>,
    #[serde(default)]
    pub builds: Vec<String>,
    /// Founds its building on arrival and is used up.
    #[serde(default)]
    pub colonizer: bool,
    /// Research needed before it can be trained.
    #[serde(default)]
    pub tech: Vec<String>,
    /// Tags research effects can target. "unit" and "ground" / "ship" are added on load.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Model key for the client.
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub group: BuildGroup,
    pub cost: Cost,
    /// Construction time in seconds (one builder).
    pub time: f32,
    pub hp: f32,
    #[serde(default = "structure")]
    pub armor_class: ArmorClass,
    #[serde(default)]
    pub armor: f32,
    #[serde(default)]
    pub shield: f32,
    #[serde(default)]
    pub shield_regen: f32,
    pub radius: f32,
    #[serde(default)]
    pub pop: u32,
    #[serde(default)]
    pub income: Cost,
    #[serde(default)]
    pub trains: Vec<String>,
    /// Finished buildings the owner needs before this one can be started.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Research needed before it can be built.
    #[serde(default)]
    pub tech: Vec<String>,
    /// Runs research projects (`data/research.ron`).
    #[serde(default)]
    pub research: bool,
    #[serde(default)]
    pub extracts: bool,
    #[serde(default)]
    pub rate: f32,
    #[serde(default)]
    pub weapon: Option<Weapon>,
    #[serde(default)]
    pub sensors: Sensors,
    #[serde(default)]
    pub aura: Option<Aura>,
    #[serde(default)]
    pub core: bool,
    /// Tags research effects can target. "building" is added on load.
    #[serde(default)]
    pub tags: Vec<String>,
    pub model: String,
}

/// Damage multipliers of one damage kind against each armour class and against shields.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Multipliers {
    pub light: f32,
    pub medium: f32,
    pub heavy: f32,
    pub structure: f32,
    pub shield: f32,
}

impl Multipliers {
    pub fn vs(&self, class: ArmorClass) -> f32 {
        match class {
            ArmorClass::Light => self.light,
            ArmorClass::Medium => self.medium,
            ArmorClass::Heavy => self.heavy,
            ArmorClass::Structure => self.structure,
        }
    }
}

/// `data/combat.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatRules {
    pub multipliers: BTreeMap<DamageKind, Multipliers>,
    /// A hit always does at least this fraction of its (multiplied) damage, whatever the armour.
    pub min_damage: f32,
    /// Seconds without being hit before shields recharge.
    pub shield_delay: f32,
    /// Fraction of a weapon's damage dealt to the other enemies in its splash radius.
    pub splash_damage: f32,
    /// Seconds a stealthed unit can be targeted by everyone after it fired.
    pub reveal_time: f32,
}

impl CombatRules {
    pub fn mult(&self, kind: DamageKind) -> Multipliers {
        self.multipliers[&kind]
    }

    /// Applies a hit of `damage` to a target with `shield` points left (reduced in place).
    /// Returns the damage that reaches the hull.
    pub fn hit(&self, damage: f32, kind: DamageKind, class: ArmorClass, armor: f32, shield: &mut f32) -> f32 {
        let m = self.mult(kind);
        let mut rest = damage;
        if *shield > 0.0 && m.shield > 0.0 {
            let against_shield = damage * m.shield;
            let absorbed = against_shield.min(*shield);
            *shield -= absorbed;
            rest = damage * (1.0 - absorbed / against_shield);
        }
        if rest <= 0.0 {
            return 0.0;
        }
        let hull = rest * m.vs(class);
        (hull - armor).max(hull * self.min_damage)
    }
}

/// The loaded, validated content with resolved references.
#[derive(Clone, Debug)]
pub struct ContentDb {
    pub units: Vec<UnitDef>,
    pub buildings: Vec<BuildingDef>,
    pub techs: Vec<TechDef>,
    pub combat: CombatRules,
    /// Per unit: the building kinds it can build.
    pub unit_builds: Vec<Vec<BuildingKind>>,
    /// Per building: the unit kinds it trains.
    pub building_trains: Vec<Vec<UnitKind>>,
    /// Per building: the buildings it requires.
    pub building_requires: Vec<Vec<BuildingKind>>,
    /// Per unit / building / tech: the research it needs.
    pub unit_tech: Vec<Vec<TechKind>>,
    pub building_tech: Vec<Vec<TechKind>>,
    pub tech_requires: Vec<Vec<TechKind>>,
    unit_ids: HashMap<String, UnitKind>,
    building_ids: HashMap<String, BuildingKind>,
    tech_ids: HashMap<String, TechKind>,
    /// FNV-1a over all data files (client and server must agree).
    pub hash: u64,
}

const UNITS_RON: &str = include_str!("../../../data/units.ron");
const BUILDINGS_RON: &str = include_str!("../../../data/buildings.ron");
const RESEARCH_RON: &str = include_str!("../../../data/research.ron");
const COMBAT_RON: &str = include_str!("../../../data/combat.ron");

pub fn fnv1a(hash: u64, bytes: &[u8]) -> u64 {
    let mut h = hash;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn index<T>(items: &[T], id: impl Fn(&T) -> &str, what: &str) -> Result<HashMap<String, u16>, String> {
    let mut ids = HashMap::new();
    for (i, x) in items.iter().enumerate() {
        if ids.insert(id(x).to_string(), i as u16).is_some() {
            return Err(format!("duplicate {what} id {}", id(x)));
        }
    }
    Ok(ids)
}

fn resolve(ids: &HashMap<String, u16>, owner: &str, what: &str, list: &[String]) -> Result<Vec<u16>, String> {
    list.iter().map(|id| ids.get(id).copied().ok_or_else(|| format!("{owner}: unknown {what} {id}"))).collect()
}

fn check_weapon(owner: &str, w: &Option<Weapon>) -> Result<(), String> {
    if let Some(w) = w
        && (w.damage <= 0.0 || w.range <= 0.0 || w.cooldown <= 0.0 || w.splash < 0.0 || !(w.ground || w.space))
    {
        return Err(format!("{owner}: a weapon needs positive damage, range and cooldown and something to hit"));
    }
    Ok(())
}

impl ContentDb {
    /// The content compiled into the binary.
    pub fn embedded() -> Self {
        Self::load(UNITS_RON, BUILDINGS_RON, RESEARCH_RON, COMBAT_RON).expect("embedded content is valid")
    }

    pub fn load(units_ron: &str, buildings_ron: &str, research_ron: &str, combat_ron: &str) -> Result<Self, String> {
        let mut units: Vec<UnitDef> = ron::from_str(units_ron).map_err(|e| format!("units.ron: {e}"))?;
        let mut buildings: Vec<BuildingDef> = ron::from_str(buildings_ron).map_err(|e| format!("buildings.ron: {e}"))?;
        let techs: Vec<TechDef> = ron::from_str(research_ron).map_err(|e| format!("research.ron: {e}"))?;
        let combat: CombatRules = ron::from_str(combat_ron).map_err(|e| format!("combat.ron: {e}"))?;
        let unit_ids = index(&units, |u| &u.id, "unit")?;
        let building_ids = index(&buildings, |b| &b.id, "building")?;
        let tech_ids = index(&techs, |t| &t.id, "tech")?;

        let mut unit_builds = Vec::new();
        let mut unit_tech = Vec::new();
        for u in &units {
            unit_builds.push(resolve(&building_ids, &u.id, "building", &u.builds)?);
            unit_tech.push(resolve(&tech_ids, &u.id, "tech", &u.tech)?);
        }
        let mut building_trains = Vec::new();
        let mut building_requires = Vec::new();
        let mut building_tech = Vec::new();
        for b in &buildings {
            building_trains.push(resolve(&unit_ids, &b.id, "unit", &b.trains)?);
            building_requires.push(resolve(&building_ids, &b.id, "building", &b.requires)?);
            building_tech.push(resolve(&tech_ids, &b.id, "tech", &b.tech)?);
        }
        let tech_requires = techs.iter().map(|t| resolve(&tech_ids, &t.id, "tech", &t.requires)).collect::<Result<Vec<_>, _>>()?;

        for u in &units {
            if u.hp <= 0.0 || u.speed <= 0.0 || u.time <= 0.0 {
                return Err(format!("unit {}: hp, speed and time must be positive", u.id));
            }
            if u.colonizer && u.domain != Domain::Space {
                return Err(format!("unit {}: colonizers must fly", u.id));
            }
            if u.sensors.sight <= 0.0 {
                return Err(format!("unit {}: needs a sight range", u.id));
            }
            if u.armor < 0.0 || u.shield < 0.0 || u.shield_regen < 0.0 {
                return Err(format!("unit {}: armour and shields cannot be negative", u.id));
            }
            check_weapon(&u.id, &u.weapon)?;
        }
        for b in &buildings {
            if b.hp <= 0.0 || b.time <= 0.0 || b.radius <= 0.0 {
                return Err(format!("building {}: hp, time and radius must be positive", b.id));
            }
            if b.extracts && b.rate <= 0.0 {
                return Err(format!("building {}: an extractor needs a rate", b.id));
            }
            if b.sensors.sight <= 0.0 {
                return Err(format!("building {}: needs a sight range", b.id));
            }
            if b.armor < 0.0 || b.shield < 0.0 || b.shield_regen < 0.0 {
                return Err(format!("building {}: armour and shields cannot be negative", b.id));
            }
            check_weapon(&b.id, &b.weapon)?;
        }
        for t in &techs {
            if t.time <= 0.0 || t.tier == 0 {
                return Err(format!("tech {}: time and tier must be positive", t.id));
            }
            for e in &t.effects {
                if e.mul < 0.0 || !e.add.is_finite() || !e.mul.is_finite() {
                    return Err(format!("tech {}: invalid effect on {:?}", t.id, e.stat));
                }
                if e.damage.is_some() && !matches!(e.stat, Stat::Damage | Stat::Range | Stat::Cooldown | Stat::Splash) {
                    return Err(format!("tech {}: `damage` only filters weapon stats", t.id));
                }
            }
        }
        // Prerequisites come from lower tiers (so the tree has no cycles).
        for (k, t) in techs.iter().enumerate() {
            for r in &tech_requires[k] {
                if techs[*r as usize].tier >= t.tier {
                    return Err(format!("tech {}: requires {} of the same or a higher tier", t.id, techs[*r as usize].id));
                }
            }
        }
        for k in DamageKind::ALL {
            if !combat.multipliers.contains_key(&k) {
                return Err(format!("combat.ron: no multipliers for {k:?}"));
            }
        }
        if !(0.0..=1.0).contains(&combat.min_damage) || !(0.0..=1.0).contains(&combat.splash_damage) {
            return Err("combat.ron: min_damage and splash_damage are fractions".into());
        }
        if !building_ids.contains_key("command_center") {
            return Err("there must be a command_center building".into());
        }
        if !unit_ids.contains_key("drone") {
            return Err("there must be a drone unit".into());
        }

        // Implicit tags for research effects.
        for u in &mut units {
            u.tags.push("unit".into());
            u.tags.push(if u.domain == Domain::Ground { "ground" } else { "ship" }.into());
        }
        for b in &mut buildings {
            b.tags.push("building".into());
        }
        let mut hash = FNV_OFFSET;
        for file in [units_ron, buildings_ron, research_ron, combat_ron] {
            hash = fnv1a(hash, file.as_bytes());
        }
        Ok(ContentDb {
            units,
            buildings,
            techs,
            combat,
            unit_builds,
            building_trains,
            building_requires,
            unit_tech,
            building_tech,
            tech_requires,
            unit_ids,
            building_ids,
            tech_ids,
            hash,
        })
    }

    pub fn unit_kind(&self, id: &str) -> Option<UnitKind> {
        self.unit_ids.get(id).copied()
    }

    pub fn building_kind(&self, id: &str) -> Option<BuildingKind> {
        self.building_ids.get(id).copied()
    }

    pub fn tech_kind(&self, id: &str) -> Option<TechKind> {
        self.tech_ids.get(id).copied()
    }

    pub fn unit(&self, k: UnitKind) -> &UnitDef {
        &self.units[k as usize]
    }

    pub fn building(&self, k: BuildingKind) -> &BuildingDef {
        &self.buildings[k as usize]
    }

    pub fn tech(&self, k: TechKind) -> &TechDef {
        &self.techs[k as usize]
    }

    /// Units and buildings that need this tech (for the research screen).
    pub fn unlocked_by(&self, tech: TechKind) -> (Vec<UnitKind>, Vec<BuildingKind>) {
        let units = (0..self.units.len() as UnitKind).filter(|k| self.unit_tech[*k as usize].contains(&tech)).collect();
        let buildings = (0..self.buildings.len() as BuildingKind).filter(|k| self.building_tech[*k as usize].contains(&tech)).collect();
        (units, buildings)
    }

    /// Buildings that can run research.
    pub fn labs(&self) -> impl Iterator<Item = BuildingKind> + '_ {
        (0..self.buildings.len() as BuildingKind).filter(|k| self.building(*k).research)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_content_loads() {
        let db = ContentDb::embedded();
        assert!(db.units.len() >= 5);
        let cc = db.building_kind("command_center").unwrap();
        assert!(db.building(cc).core);
        assert!(!db.building_trains[cc as usize].is_empty());
        let drone = db.unit_kind("drone").unwrap();
        assert!(db.unit_builds[drone as usize].contains(&db.building_kind("extractor").unwrap()));
        assert!(db.labs().count() >= 1, "research needs a lab");
    }

    #[test]
    fn every_unit_can_be_trained() {
        let db = ContentDb::embedded();
        for (k, u) in db.units.iter().enumerate() {
            assert!(db.building_trains.iter().any(|t| t.contains(&(k as UnitKind))), "{} is never trained", u.id);
        }
    }

    #[test]
    fn every_building_can_be_built() {
        let db = ContentDb::embedded();
        for (k, b) in db.buildings.iter().enumerate() {
            assert!(db.unit_builds.iter().any(|t| t.contains(&(k as BuildingKind))), "{} is never built", b.id);
        }
    }

    #[test]
    fn every_tech_does_something() {
        let db = ContentDb::embedded();
        for (k, t) in db.techs.iter().enumerate() {
            let (u, b) = db.unlocked_by(k as TechKind);
            let required = db.tech_requires.iter().any(|r| r.contains(&(k as TechKind)));
            assert!(!t.effects.is_empty() || !u.is_empty() || !b.is_empty() || required, "{} has no effect", t.id);
        }
    }

    #[test]
    fn every_effect_matches_something() {
        let db = ContentDb::embedded();
        for t in &db.techs {
            for e in &t.effects {
                let units = db.units.iter().any(|u| e.matches(&u.tags, u.weapon.as_ref()));
                let buildings = db.buildings.iter().any(|b| e.matches(&b.tags, b.weapon.as_ref()));
                assert!(units || buildings, "{}: the {:?} effect matches nothing", t.id, e.stat);
            }
        }
    }

    #[test]
    fn armour_shields_and_damage_kinds() {
        let db = ContentDb::embedded();
        let c = &db.combat;
        // Armour subtracts, but never below the minimum fraction.
        let mut s = 0.0;
        let light = c.hit(10.0, DamageKind::Kinetic, ArmorClass::Light, 0.0, &mut s);
        let armoured = c.hit(10.0, DamageKind::Kinetic, ArmorClass::Light, 4.0, &mut s);
        let bunker = c.hit(10.0, DamageKind::Kinetic, ArmorClass::Light, 100.0, &mut s);
        assert!(armoured < light && bunker > 0.0 && (bunker - light * c.min_damage).abs() < 1e-4);
        // A shield takes the hit first; ion is better at it than kinetic.
        let mut shield = 50.0;
        assert_eq!(c.hit(10.0, DamageKind::Kinetic, ArmorClass::Heavy, 0.0, &mut shield), 0.0);
        let after_kinetic = shield;
        let mut shield = 50.0;
        c.hit(10.0, DamageKind::Ion, ArmorClass::Heavy, 0.0, &mut shield);
        assert!(shield < after_kinetic);
        // An almost empty shield lets the rest through.
        let mut shield = 1.0;
        assert!(c.hit(20.0, DamageKind::Kinetic, ArmorClass::Medium, 0.0, &mut shield) > 0.0);
        assert_eq!(shield, 0.0);
        // Plasma is the anti-armour weapon, explosives crack buildings.
        assert!(c.mult(DamageKind::Plasma).heavy > c.mult(DamageKind::Kinetic).heavy);
        assert!(c.mult(DamageKind::Explosive).structure > c.mult(DamageKind::Laser).structure);
    }
}
