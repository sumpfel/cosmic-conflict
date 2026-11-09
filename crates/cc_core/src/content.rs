//! Game content (units and buildings), defined in `data/*.ron` and embedded at compile time.
//!
//! String ids in the files are resolved to compact numeric kinds (`u16` indices) and validated
//! when the database is loaded. The content hash is compared between client and server.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::Res;

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
    /// Metres per second.
    pub speed: f32,
    /// Collision / selection radius in metres.
    pub radius: f32,
    #[serde(default)]
    pub weapon: Option<Weapon>,
    #[serde(default)]
    pub builds: Vec<String>,
    /// Founds its building on arrival and is used up.
    #[serde(default)]
    pub colonizer: bool,
    /// Model key for the client.
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub cost: Cost,
    /// Construction time in seconds (one builder).
    pub time: f32,
    pub hp: f32,
    pub radius: f32,
    #[serde(default)]
    pub pop: u32,
    #[serde(default)]
    pub income: Cost,
    #[serde(default)]
    pub trains: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub extracts: bool,
    #[serde(default)]
    pub rate: f32,
    #[serde(default)]
    pub weapon: Option<Weapon>,
    #[serde(default)]
    pub core: bool,
    pub model: String,
}

/// The loaded, validated content with resolved references.
#[derive(Clone, Debug)]
pub struct ContentDb {
    pub units: Vec<UnitDef>,
    pub buildings: Vec<BuildingDef>,
    /// Per unit: the building kinds it can build.
    pub unit_builds: Vec<Vec<BuildingKind>>,
    /// Per building: the unit kinds it trains.
    pub building_trains: Vec<Vec<UnitKind>>,
    /// Per building: the buildings it requires.
    pub building_requires: Vec<Vec<BuildingKind>>,
    unit_ids: HashMap<String, UnitKind>,
    building_ids: HashMap<String, BuildingKind>,
    /// FNV-1a over all data files (client and server must agree).
    pub hash: u64,
}

const UNITS_RON: &str = include_str!("../../../data/units.ron");
const BUILDINGS_RON: &str = include_str!("../../../data/buildings.ron");

pub fn fnv1a(hash: u64, bytes: &[u8]) -> u64 {
    let mut h = hash;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

impl ContentDb {
    /// The content compiled into the binary.
    pub fn embedded() -> Self {
        Self::load(UNITS_RON, BUILDINGS_RON).expect("embedded content is valid")
    }

    pub fn load(units_ron: &str, buildings_ron: &str) -> Result<Self, String> {
        let units: Vec<UnitDef> = ron::from_str(units_ron).map_err(|e| format!("units.ron: {e}"))?;
        let buildings: Vec<BuildingDef> = ron::from_str(buildings_ron).map_err(|e| format!("buildings.ron: {e}"))?;
        let mut unit_ids = HashMap::new();
        for (i, u) in units.iter().enumerate() {
            if unit_ids.insert(u.id.clone(), i as UnitKind).is_some() {
                return Err(format!("duplicate unit id {}", u.id));
            }
        }
        let mut building_ids = HashMap::new();
        for (i, b) in buildings.iter().enumerate() {
            if building_ids.insert(b.id.clone(), i as BuildingKind).is_some() {
                return Err(format!("duplicate building id {}", b.id));
            }
        }
        let resolve_b = |owner: &str, id: &String| {
            building_ids.get(id).copied().ok_or_else(|| format!("{owner}: unknown building {id}"))
        };
        let resolve_u =
            |owner: &str, id: &String| unit_ids.get(id).copied().ok_or_else(|| format!("{owner}: unknown unit {id}"));
        let unit_builds = units
            .iter()
            .map(|u| u.builds.iter().map(|b| resolve_b(&u.id, b)).collect::<Result<Vec<_>, _>>())
            .collect::<Result<Vec<_>, _>>()?;
        let building_trains = buildings
            .iter()
            .map(|b| b.trains.iter().map(|u| resolve_u(&b.id, u)).collect::<Result<Vec<_>, _>>())
            .collect::<Result<Vec<_>, _>>()?;
        let building_requires = buildings
            .iter()
            .map(|b| b.requires.iter().map(|r| resolve_b(&b.id, r)).collect::<Result<Vec<_>, _>>())
            .collect::<Result<Vec<_>, _>>()?;
        for u in &units {
            if u.hp <= 0.0 || u.speed <= 0.0 || u.time <= 0.0 {
                return Err(format!("unit {}: hp, speed and time must be positive", u.id));
            }
            if u.colonizer && u.domain != Domain::Space {
                return Err(format!("unit {}: colonizers must fly", u.id));
            }
        }
        for b in &buildings {
            if b.hp <= 0.0 || b.time <= 0.0 || b.radius <= 0.0 {
                return Err(format!("building {}: hp, time and radius must be positive", b.id));
            }
            if b.extracts && b.rate <= 0.0 {
                return Err(format!("building {}: an extractor needs a rate", b.id));
            }
        }
        if !building_ids.contains_key("command_center") {
            return Err("there must be a command_center building".into());
        }
        if !unit_ids.contains_key("drone") {
            return Err("there must be a drone unit".into());
        }
        let hash = fnv1a(fnv1a(FNV_OFFSET, units_ron.as_bytes()), buildings_ron.as_bytes());
        Ok(ContentDb { units, buildings, unit_builds, building_trains, building_requires, unit_ids, building_ids, hash })
    }

    pub fn unit_kind(&self, id: &str) -> Option<UnitKind> {
        self.unit_ids.get(id).copied()
    }

    pub fn building_kind(&self, id: &str) -> Option<BuildingKind> {
        self.building_ids.get(id).copied()
    }

    pub fn unit(&self, k: UnitKind) -> &UnitDef {
        &self.units[k as usize]
    }

    pub fn building(&self, k: BuildingKind) -> &BuildingDef {
        &self.buildings[k as usize]
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
}
