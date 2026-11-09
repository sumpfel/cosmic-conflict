//! Match settings chosen by the host in the lobby.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SystemSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl SystemSize {
    pub const ALL: [SystemSize; 3] = [SystemSize::Small, SystemSize::Medium, SystemSize::Large];

    /// Neutral planets per player (the system is rotationally symmetric, so every player gets
    /// the same neighbourhood).
    pub fn neutrals_per_player(self) -> usize {
        match self {
            SystemSize::Small => 1,
            SystemSize::Medium => 2,
            SystemSize::Large => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SystemSize::Small => "Small",
            SystemSize::Medium => "Medium",
            SystemSize::Large => "Large",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Abundance {
    Scarce,
    #[default]
    Normal,
    Rich,
}

impl Abundance {
    pub const ALL: [Abundance; 3] = [Abundance::Scarce, Abundance::Normal, Abundance::Rich];

    pub fn factor(self) -> f32 {
        match self {
            Abundance::Scarce => 0.6,
            Abundance::Normal => 1.0,
            Abundance::Rich => 1.7,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Abundance::Scarce => "Scarce",
            Abundance::Normal => "Normal",
            Abundance::Rich => "Rich",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StartResources {
    Low,
    #[default]
    Normal,
    High,
}

impl StartResources {
    pub const ALL: [StartResources; 3] = [StartResources::Low, StartResources::Normal, StartResources::High];

    /// Metal, crystal, gas at the start.
    pub fn amounts(self) -> [f32; 3] {
        match self {
            StartResources::Low => [200.0, 50.0, 0.0],
            StartResources::Normal => [500.0, 150.0, 50.0],
            StartResources::High => [2000.0, 800.0, 400.0],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StartResources::Low => "Low",
            StartResources::Normal => "Normal",
            StartResources::High => "High",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchSettings {
    pub seed: u64,
    pub max_players: u8,
    pub size: SystemSize,
    pub abundance: Abundance,
    pub start: StartResources,
}

impl MatchSettings {
    pub const MAX_PLAYERS: u8 = 8;
}

impl Default for MatchSettings {
    fn default() -> Self {
        MatchSettings {
            seed: 1,
            max_players: 4,
            size: SystemSize::default(),
            abundance: Abundance::default(),
            start: StartResources::default(),
        }
    }
}
