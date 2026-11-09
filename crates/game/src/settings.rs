//! Persisted player settings (RON in the OS config directory, e.g. `~/.config/cosmic-conflict/`).

use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub name: String,
    /// Last address used in "Join".
    pub address: String,
    pub port: u16,
    pub color: u8,
    /// Master volume 0..1.
    pub volume: f32,
    /// Camera rotation speed factor.
    pub camera_speed: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            name: "Commander".into(),
            address: format!("127.0.0.1:{}", cc_protocol::DEFAULT_PORT),
            port: cc_protocol::DEFAULT_PORT,
            color: 0,
            volume: 0.6,
            camera_speed: 1.0,
        }
    }
}

pub fn config_file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(|p| PathBuf::from(p).join("cosmic-conflict"))
        .or_else(|| directories::ProjectDirs::from("", "", "cosmic-conflict").map(|d| d.config_dir().to_path_buf()))?;
    Some(base.join(name))
}

impl Settings {
    pub fn load() -> Self {
        config_file("settings.ron")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = config_file("settings.ron") else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(path, s);
        }
    }
}
