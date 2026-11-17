//! The 3D world: space scene, planets, the client mirror of the match, camera and input.

pub mod camera;
pub mod effects;
pub mod input;
pub mod loading;
pub mod model_assets;
pub mod models;
pub mod planet_mesh;
pub mod planet_render;
pub mod scene;
pub mod world;

use bevy::prelude::*;

use crate::AppState;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(planet_render::PlanetRenderPlugin)
            .init_resource::<input::Selection>()
            .init_resource::<input::Placement>()
            .init_resource::<input::Drag>()
            .init_resource::<input::Hover>()
            .init_resource::<effects::Effects>()
            .add_systems(Startup, (scene::setup_scene, models::init_models, model_assets::start_model_load, effects::init_effects))
            .add_systems(Update, model_assets::finish_model_load)
            .add_systems(OnEnter(AppState::Menu), loading::teardown)
            .add_systems(OnExit(AppState::Menu), scene::despawn_menu_backdrop)
            .add_systems(Update, (scene::ensure_menu_backdrop, scene::spin_backdrop).run_if(in_state(AppState::Menu)))
            .add_systems(OnEnter(AppState::Loading), loading::begin_loading)
            .add_systems(Update, loading::poll_loading.run_if(in_state(AppState::Loading)))
            .add_systems(
                Update,
                (
                    world::apply_snapshots,
                    world::interpolate,
                    camera::orbit_camera,
                    input::dev_autoplay,
                    input::handle_input,
                    input::draw_overlays,
                    effects::update_effects,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(PostUpdate, scene::follow_stars.after(TransformSystems::Propagate))
            .add_systems(Update, (screenshot_hook, fps_log));
    }
}

/// Development helper: `CC_SCREENSHOT=/tmp/x.png CC_SCREENSHOT_AFTER=8` saves a screenshot
/// after that many seconds and quits.
fn screenshot_hook(mut commands: Commands, time: Res<Time>, mut done: Local<u8>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("CC_SCREENSHOT") else { return };
    let after: f32 = std::env::var("CC_SCREENSHOT_AFTER").ok().and_then(|v| v.parse().ok()).unwrap_or(8.0);
    let t = time.elapsed_secs();
    match *done {
        0 if t > after => {
            commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(path));
            *done = 1;
        }
        1 if t > after + 1.5 => {
            exit.write(AppExit::Success);
            *done = 2;
        }
        _ => {}
    }
}

/// Development helper: `CC_FPS=1` logs the frame rate every 2 s (vsync is off then, see main).
fn fps_log(time: Res<Time>, mut acc: Local<(f32, u32)>) {
    if std::env::var_os("CC_FPS").is_none() {
        return;
    }
    acc.0 += time.delta_secs();
    acc.1 += 1;
    if acc.0 >= 2.0 {
        info!("fps: {:.1} ({:.2} ms)", acc.1 as f32 / acc.0, acc.0 * 1000.0 / acc.1 as f32);
        *acc = (0.0, 0);
    }
}
