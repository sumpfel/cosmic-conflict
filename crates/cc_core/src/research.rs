//! The research tree (`data/research.ron`) and how finished research changes stats.
//!
//! A tech is researched in a lab building, costs resources and time, needs its prerequisites
//! (always from lower tiers) and either unlocks units / buildings (their `tech` lists) or has
//! effects: `(base + Σ add) × Π mul` on a stat of every unit / building whose tags match.
//! [`Stats`] resolves the effective numbers of one player for every unit and building kind; the
//! simulation and the client use the same code.

use serde::{Deserialize, Serialize};

use crate::content::{Aura, BuildingKind, ContentDb, Cost, DamageKind, Sensors, UnitKind, Weapon};

pub type TechKind = u16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TechField {
    Weapons,
    Armour,
    Shields,
    Sensors,
    Robotics,
    Propulsion,
    Economy,
}

impl TechField {
    pub const ALL: [TechField; 7] =
        [TechField::Weapons, TechField::Armour, TechField::Shields, TechField::Sensors, TechField::Robotics, TechField::Propulsion, TechField::Economy];

    pub fn label(self) -> &'static str {
        match self {
            TechField::Weapons => "Weapons",
            TechField::Armour => "Armour",
            TechField::Shields => "Shields",
            TechField::Sensors => "Sensors",
            TechField::Robotics => "Robotics",
            TechField::Propulsion => "Propulsion",
            TechField::Economy => "Economy",
        }
    }
}

/// A number research can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Stat {
    Hp,
    Armor,
    Shield,
    ShieldRegen,
    /// Self-repair in hit points per second.
    Regen,
    Damage,
    Range,
    /// Seconds between shots (a `mul` below 1 fires faster).
    Cooldown,
    Splash,
    Speed,
    Sight,
    Radar,
    Seismic,
    Deep,
    Detect,
    /// Strength of shield and repair auras.
    Aura,
    /// Extractor mining rate.
    Extraction,
    /// Passive income of buildings.
    Income,
    /// Housing of buildings.
    Pop,
}

impl Stat {
    pub const COUNT: usize = 19;

    pub fn label(self) -> &'static str {
        match self {
            Stat::Hp => "hit points",
            Stat::Armor => "armour",
            Stat::Shield => "shields",
            Stat::ShieldRegen => "shield recharge",
            Stat::Regen => "self-repair",
            Stat::Damage => "damage",
            Stat::Range => "weapon range",
            Stat::Cooldown => "reload time",
            Stat::Splash => "splash radius",
            Stat::Speed => "speed",
            Stat::Sight => "sight",
            Stat::Radar => "radar range",
            Stat::Seismic => "seismic range",
            Stat::Deep => "deep-space range",
            Stat::Detect => "stealth detection",
            Stat::Aura => "aura strength",
            Stat::Extraction => "mining rate",
            Stat::Income => "income",
            Stat::Pop => "housing",
        }
    }
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    pub stat: Stat,
    #[serde(default)]
    pub add: f32,
    #[serde(default = "one")]
    pub mul: f32,
    /// Applies to units / buildings with any of these tags (empty: everything).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Only weapons of this damage kind (weapon stats only).
    #[serde(default)]
    pub damage: Option<DamageKind>,
}

impl Effect {
    pub fn matches(&self, tags: &[String], weapon: Option<&Weapon>) -> bool {
        let tagged = self.tags.is_empty() || self.tags.iter().any(|t| tags.contains(t));
        let weapon_ok = match self.damage {
            Some(k) => weapon.is_some_and(|w| w.kind == k),
            None => true,
        };
        tagged && weapon_ok
    }

    /// "+15% damage (Kinetic) for vehicle, robot".
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.add != 0.0 {
            parts.push(format!("{:+}", self.add));
        }
        if self.mul != 1.0 {
            parts.push(format!("{:+.0}%", (self.mul - 1.0) * 100.0));
        }
        let mut s = format!("{} {}", parts.join(" "), self.stat.label());
        if let Some(k) = self.damage {
            s.push_str(&format!(" ({} weapons)", k.label()));
        }
        if !self.tags.is_empty() {
            s.push_str(&format!(" · {}", self.tags.join(", ")));
        }
        s
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TechDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub field: TechField,
    /// 1..: prerequisites always come from lower tiers.
    pub tier: u8,
    pub cost: Cost,
    /// Research time in seconds.
    pub time: f32,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

/// A stat change: `(base + add) * mul`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mod {
    pub add: f32,
    pub mul: f32,
}

impl Default for Mod {
    fn default() -> Self {
        Mod { add: 0.0, mul: 1.0 }
    }
}

impl Mod {
    pub fn apply(&self, base: f32) -> f32 {
        ((base + self.add) * self.mul).max(0.0)
    }
}

type Mods = [Mod; Stat::COUNT];

/// Effective numbers of a unit kind for one player.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitStats {
    pub hp: f32,
    pub armor: f32,
    pub shield: f32,
    pub shield_regen: f32,
    pub regen: f32,
    pub speed: f32,
    pub weapon: Option<Weapon>,
    pub sensors: Sensors,
    pub aura: Option<Aura>,
}

/// Effective numbers of a building kind for one player.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingStats {
    pub hp: f32,
    pub armor: f32,
    pub shield: f32,
    pub shield_regen: f32,
    pub regen: f32,
    pub weapon: Option<Weapon>,
    pub sensors: Sensors,
    pub aura: Option<Aura>,
    /// Extraction per second.
    pub rate: f32,
    pub income: [f32; 3],
    pub pop: u32,
}

/// One player's effective stats for every unit and building kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub units: Vec<UnitStats>,
    pub buildings: Vec<BuildingStats>,
}

fn weapon(w: &Option<Weapon>, m: &Mods) -> Option<Weapon> {
    w.map(|w| Weapon {
        damage: m[Stat::Damage as usize].apply(w.damage),
        range: m[Stat::Range as usize].apply(w.range),
        cooldown: m[Stat::Cooldown as usize].apply(w.cooldown).max(0.05),
        splash: m[Stat::Splash as usize].apply(w.splash),
        ..w
    })
}

fn sensors(s: &Sensors, m: &Mods) -> Sensors {
    // A sensor the thing does not have is not created by an upgrade (only `add` on 0 would).
    let f = |base: f32, stat: Stat| if base > 0.0 { m[stat as usize].apply(base) } else { 0.0 };
    Sensors {
        sight: f(s.sight, Stat::Sight),
        radar: f(s.radar, Stat::Radar),
        seismic: f(s.seismic, Stat::Seismic),
        deep: f(s.deep, Stat::Deep),
        detect: f(s.detect, Stat::Detect),
    }
}

fn aura(a: &Option<Aura>, m: &Mods) -> Option<Aura> {
    let k = m[Stat::Aura as usize];
    a.map(|a| Aura { shield: k.apply(a.shield), shield_regen: k.apply(a.shield_regen), repair: k.apply(a.repair), ..a })
}

fn collect(content: &ContentDb, done: &[TechKind], tags: &[String], w: Option<&Weapon>) -> Mods {
    let mut m = [Mod::default(); Stat::COUNT];
    for t in done {
        for e in &content.tech(*t).effects {
            if e.matches(tags, w) {
                let x = &mut m[e.stat as usize];
                x.add += e.add;
                x.mul *= e.mul;
            }
        }
    }
    m
}

impl Stats {
    /// Stats with the given finished research (`done`, any order).
    pub fn new(content: &ContentDb, done: &[TechKind]) -> Self {
        let units = content
            .units
            .iter()
            .map(|u| {
                let m = collect(content, done, &u.tags, u.weapon.as_ref());
                UnitStats {
                    hp: m[Stat::Hp as usize].apply(u.hp).max(1.0),
                    armor: m[Stat::Armor as usize].apply(u.armor),
                    shield: m[Stat::Shield as usize].apply(u.shield),
                    shield_regen: m[Stat::ShieldRegen as usize].apply(u.shield_regen),
                    regen: m[Stat::Regen as usize].apply(0.0),
                    speed: m[Stat::Speed as usize].apply(u.speed).max(0.1),
                    weapon: weapon(&u.weapon, &m),
                    sensors: sensors(&u.sensors, &m),
                    aura: aura(&u.aura, &m),
                }
            })
            .collect();
        let buildings = content
            .buildings
            .iter()
            .map(|b| {
                let m = collect(content, done, &b.tags, b.weapon.as_ref());
                let inc = m[Stat::Income as usize];
                BuildingStats {
                    hp: m[Stat::Hp as usize].apply(b.hp).max(1.0),
                    armor: m[Stat::Armor as usize].apply(b.armor),
                    shield: m[Stat::Shield as usize].apply(b.shield),
                    shield_regen: m[Stat::ShieldRegen as usize].apply(b.shield_regen),
                    regen: m[Stat::Regen as usize].apply(0.0),
                    weapon: weapon(&b.weapon, &m),
                    sensors: sensors(&b.sensors, &m),
                    aura: aura(&b.aura, &m),
                    rate: m[Stat::Extraction as usize].apply(b.rate),
                    income: b.income.arr().map(|v| if v > 0.0 { inc.apply(v) } else { 0.0 }),
                    pop: if b.pop > 0 { m[Stat::Pop as usize].apply(b.pop as f32).round() as u32 } else { 0 },
                }
            })
            .collect();
        Stats { units, buildings }
    }

    pub fn unit(&self, k: UnitKind) -> &UnitStats {
        &self.units[k as usize]
    }

    pub fn building(&self, k: BuildingKind) -> &BuildingStats {
        &self.buildings[k as usize]
    }
}

/// Why a tech cannot be started right now (`None`: it can).
pub fn blocked(content: &ContentDb, tech: TechKind, done: &[bool], queued: &[TechKind]) -> Option<String> {
    if done[tech as usize] {
        return Some("Already researched.".into());
    }
    if queued.contains(&tech) {
        return Some("Already being researched.".into());
    }
    content.tech_requires[tech as usize]
        .iter()
        .find(|r| !done[**r as usize] && !queued.contains(r))
        .map(|r| format!("Requires {}.", content.tech(*r).name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_change_stats() {
        let db = ContentDb::embedded();
        let base = Stats::new(&db, &[]);
        let all: Vec<TechKind> = (0..db.techs.len() as TechKind).collect();
        let maxed = Stats::new(&db, &all);
        let tank = db.unit_kind("tank").unwrap();
        assert_eq!(base.unit(tank).hp, db.unit(tank).hp, "no research = the data file");
        assert!(maxed.unit(tank).armor > base.unit(tank).armor);
        assert!(maxed.unit(tank).weapon.unwrap().damage > base.unit(tank).weapon.unwrap().damage);
        let radar = db.building_kind("radar_station").unwrap();
        assert!(maxed.building(radar).sensors.radar > base.building(radar).sensors.radar);
        // Sensors a unit does not have stay off.
        assert_eq!(maxed.unit(tank).sensors.deep, 0.0);
        let ext = db.building_kind("extractor").unwrap();
        assert!(maxed.building(ext).rate > base.building(ext).rate);
    }

    #[test]
    fn prerequisites_block() {
        let db = ContentDb::embedded();
        let mut done = vec![false; db.techs.len()];
        let (k, t) = db.techs.iter().enumerate().find(|(_, t)| !t.requires.is_empty()).unwrap();
        assert!(blocked(&db, k as TechKind, &done, &[]).is_some());
        for r in &db.tech_requires[k] {
            done[*r as usize] = true;
        }
        assert!(blocked(&db, k as TechKind, &done, &[]).is_none(), "{}", t.id);
        done[k] = true;
        assert!(blocked(&db, k as TechKind, &done, &[]).is_some());
    }
}
