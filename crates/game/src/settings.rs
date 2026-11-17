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
    /// Rendering quality (planet shaders, shadows, atmosphere, clouds, anti-aliasing).
    pub graphics: Graphics,
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
            graphics: Graphics::default(),
        }
    }
}

/// One quality step. Not every option uses every step (the planet surface has no `Off`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Level {
    Off,
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

impl Level {
    pub const ALL: [Level; 5] = [Level::Off, Level::Low, Level::Medium, Level::High, Level::Ultra];

    pub fn label(self) -> &'static str {
        match self {
            Level::Off => "Off",
            Level::Low => "Low",
            Level::Medium => "Medium",
            Level::High => "High",
            Level::Ultra => "Ultra",
        }
    }

    /// 0 (Off) ..= 4 (Ultra).
    pub fn index(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AntiAliasing {
    Off,
    Fxaa,
    #[default]
    Msaa4,
}

impl AntiAliasing {
    pub const ALL: [AntiAliasing; 3] = [AntiAliasing::Off, AntiAliasing::Fxaa, AntiAliasing::Msaa4];

    pub fn label(self) -> &'static str {
        match self {
            AntiAliasing::Off => "Off",
            AntiAliasing::Fxaa => "FXAA (fast)",
            AntiAliasing::Msaa4 => "MSAA 4x",
        }
    }
}

/// Graphics options. A preset fills in all of them; changing one afterwards makes it "Custom".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Graphics {
    /// The preset the options came from (`None` = custom).
    pub preset: Option<Level>,
    /// Planet surface shader: Low = plain colours, Medium = detail noise, slope rock, ocean
    /// glint, High = + bump mapping, animated waves, cloud shadows, Ultra = + finest detail.
    pub surface: Level,
    /// Sun shadows (directional shadow map aimed at the planet in view).
    pub shadows: Level,
    /// Atmospheric scattering (ray-marched; more steps = smoother sunsets).
    pub atmosphere: Level,
    /// Cloud layers on planets with an atmosphere.
    pub clouds: Level,
    /// Planet mesh resolution (applies to the next match).
    pub planet_mesh: Level,
    pub antialiasing: AntiAliasing,
    pub bloom: bool,
}

impl Default for Graphics {
    fn default() -> Self {
        Graphics::preset(Level::High)
    }
}

impl Graphics {
    pub const PRESETS: [Level; 4] = [Level::Low, Level::Medium, Level::High, Level::Ultra];

    pub fn preset(level: Level) -> Self {
        use Level::*;
        let (surface, shadows, atmosphere, clouds, planet_mesh, antialiasing, bloom) = match level {
            Off | Low => (Low, Off, Low, Off, Low, AntiAliasing::Fxaa, false),
            Medium => (Medium, Low, Medium, Low, Medium, AntiAliasing::Msaa4, true),
            High => (High, Medium, High, Medium, High, AntiAliasing::Msaa4, true),
            Ultra => (Ultra, Ultra, Ultra, Ultra, High, AntiAliasing::Msaa4, true),
        };
        Graphics {
            preset: Some(if level == Off { Low } else { level }),
            surface,
            shadows,
            atmosphere,
            clouds,
            planet_mesh,
            antialiasing,
            bloom,
        }
    }

    /// Icosphere subdivisions for a planet of `radius` (Bevy's limit is 79 ≈ 64k vertices).
    pub fn planet_subdivisions(&self, radius: f32) -> u32 {
        let base: u32 = match self.planet_mesh {
            Level::Off | Level::Low => 40,
            Level::Medium => 60,
            Level::High | Level::Ultra => 79,
        };
        if radius >= 50.0 { base } else { (base * 4 / 5).max(24) }
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
        let mut s: Settings = config_file("settings.ron")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or_default();
        // `CC_GRAPHICS=low|medium|high|ultra` overrides the saved quality (test runs).
        if let Ok(v) = std::env::var("CC_GRAPHICS") {
            let level = match v.to_ascii_lowercase().as_str() {
                "low" => Some(Level::Low),
                "medium" => Some(Level::Medium),
                "high" => Some(Level::High),
                "ultra" => Some(Level::Ultra),
                _ => None,
            };
            if let Some(l) = level {
                s.graphics = Graphics::preset(l);
            }
        }
        s
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_files_still_load_with_default_graphics() {
        let old = r#"(name: "Ann", address: "1.2.3.4:5", port: 5, color: 2, volume: 0.3, camera_speed: 1.5)"#;
        let s: Settings = ron::from_str(old).expect("old file parses");
        assert_eq!(s.name, "Ann");
        assert_eq!(s.graphics, Graphics::default());
    }

    #[test]
    fn graphics_round_trip_and_presets_are_ordered() {
        let mut s = Settings::default();
        s.graphics = Graphics::preset(Level::Ultra);
        s.graphics.bloom = false;
        s.graphics.preset = None;
        let text = ron::ser::to_string(&s).unwrap();
        let back: Settings = ron::from_str(&text).unwrap();
        assert_eq!(back.graphics, s.graphics);
        let low = Graphics::preset(Level::Low);
        let ultra = Graphics::preset(Level::Ultra);
        assert!(low.surface < ultra.surface && low.shadows < ultra.shadows && low.atmosphere < ultra.atmosphere);
        assert!(low.planet_subdivisions(60.0) < ultra.planet_subdivisions(60.0));
        assert!(ultra.planet_subdivisions(60.0) <= 79 && low.planet_subdivisions(30.0) >= 24);
    }
}
