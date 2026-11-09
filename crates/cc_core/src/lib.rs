//! # cc_core — Cosmic Conflict's shared rules
//!
//! Engine-independent vocabulary used by the server, the simulation and the client:
//! resources, the content database (`data/*.ron`), match settings and the deterministic
//! star-system generator (planets are globes; every position on a planet is a unit direction
//! from its centre).

pub mod content;
pub mod geo;
pub mod rng;
pub mod settings;
pub mod system;

pub use content::ContentDb;
pub use glam::{Quat, Vec3};

use serde::{Deserialize, Serialize};

pub type PlayerId = u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Res {
    Metal,
    Crystal,
    Gas,
}

impl Res {
    pub const ALL: [Res; 3] = [Res::Metal, Res::Crystal, Res::Gas];

    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Res::Metal => "Metal",
            Res::Crystal => "Crystal",
            Res::Gas => "Gas",
        }
    }
}

/// Player colours (sRGB), picked in the lobby.
pub const PLAYER_COLORS: [(u8, u8, u8); 8] = [
    (60, 140, 255),
    (235, 70, 60),
    (70, 210, 110),
    (245, 200, 50),
    (180, 90, 235),
    (40, 215, 215),
    (250, 140, 40),
    (235, 235, 235),
];
