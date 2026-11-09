//! Cosmic Conflict — a 3D real-time strategy game in space.
//!
//! Game client (Bevy + egui). The rules, the simulation and the server live in the `cc_*`
//! crates of this workspace. See `docs/ARCHITECTURE.md`.

mod audio;
mod game;
mod net;
mod settings;
mod ui;

use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;
use cc_core::ContentDb;

/// Shared, immutable game content (units, buildings).
#[derive(Resource, Clone)]
pub struct Content(pub Arc<ContentDb>);

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Menu,
    /// Generating the star system and building the world.
    Loading,
    InGame,
}

/// Which screen of the main menu is shown.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuScreen {
    #[default]
    Main,
    Host,
    Join,
    Lobby,
}

/// Writes every panic to `crash.log` next to the settings.
fn install_crash_log() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(path) = settings::config_file("crash.log") {
            let backtrace = std::backtrace::Backtrace::force_capture();
            let text = format!("Cosmic Conflict {} crashed:\n{info}\n\n{backtrace}\n", env!("CARGO_PKG_VERSION"));
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
            let _ = std::fs::write(&path, text);
            eprintln!("Crash report written to {}", path.display());
        }
        previous(info);
    }));
}

fn main() {
    install_crash_log();
    let settings = settings::Settings::load();
    let content = Arc::new(ContentDb::embedded());
    // `CC_WINDOW_SCALE=1` renders 1600x900 physical pixels on HiDPI screens (cheap test runs).
    let resolution = match std::env::var("CC_WINDOW_SCALE").ok().and_then(|v| v.parse::<f32>().ok()) {
        Some(f) => WindowResolution::new(1600, 900).with_scale_factor_override(f),
        None => WindowResolution::new(1600, 900),
    };

    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window { title: "Cosmic Conflict".into(), resolution, ..default() }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    filter: "wgpu=error,naga=warn,cosmic_conflict=info,cc_server=info".into(),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .insert_resource(settings)
        .insert_resource(Content(content))
        .init_state::<AppState>()
        .init_resource::<MenuScreen>()
        .init_resource::<net::Net>()
        .init_resource::<net::HostedServer>()
        .init_resource::<net::LobbyRes>()
        .init_resource::<net::ChatLog>()
        .init_resource::<net::MatchRes>()
        .init_resource::<net::IncomingSnapshots>()
        .init_resource::<net::MenuMessage>()
        .init_resource::<net::Notices>()
        .add_plugins((ui::UiPlugin, game::GamePlugin, audio::SoundPlugin))
        .add_systems(Update, (net::pump_network, net::dev_autostart))
        .run();
}
