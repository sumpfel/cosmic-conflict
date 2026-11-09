//! Orbit camera: circles the focused planet (or the sun in system view).
//!
//! Middle mouse drag / WASD / arrow keys rotate around the globe, the wheel zooms from the
//! surface out to the whole system. Tab cycles planets, H jumps home, Space shows the system,
//! a double click on a planet focuses it.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use cc_core::system::StarSystem;

use super::scene::MainCamera;
use super::world::ClientWorld;
use crate::settings::Settings;

pub const SYSTEM_VIEW_DIST: f32 = 2600.0;

#[derive(Resource, Debug)]
pub struct OrbitCam {
    /// The planet the camera circles (`None` = the sun / whole system).
    pub focus: Option<u16>,
    /// Smoothed centre of rotation.
    pub center: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub target_dist: f32,
}

impl OrbitCam {
    pub fn looking_at_home(sys: &StarSystem, home: Option<u16>) -> Self {
        let mut cam = OrbitCam { focus: None, center: Vec3::ZERO, yaw: 0.0, pitch: 0.4, dist: SYSTEM_VIEW_DIST, target_dist: SYSTEM_VIEW_DIST };
        if let Some(h) = home.and_then(|h| sys.planet(h)) {
            cam.focus = Some(h.id);
            cam.center = h.center;
            // Look straight at the base.
            let d = h.start_dir;
            cam.yaw = d.x.atan2(d.z);
            cam.pitch = d.y.clamp(-0.99, 0.99).asin();
            cam.dist = h.radius * 2.6;
            cam.target_dist = cam.dist;
        }
        if std::env::var("CC_SYSTEM_VIEW").is_ok() {
            cam.system_view();
            cam.dist = cam.target_dist;
            cam.center = Vec3::ZERO;
        }
        if let Ok(v) = std::env::var("CC_CAM_DIST")
            && let Ok(d) = v.parse::<f32>()
        {
            cam.dist = d;
            cam.target_dist = d;
        }
        cam
    }

    /// Direction from the centre to the camera.
    pub fn offset_dir(&self) -> Vec3 {
        Vec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos())
    }

    pub fn focus_planet(&mut self, sys: &StarSystem, id: u16) {
        if let Some(p) = sys.planet(id) {
            if self.focus != Some(id) {
                self.target_dist = p.radius * 2.8;
            }
            self.focus = Some(id);
        }
    }

    /// Turns the globe so that `dir` (from the planet's centre) faces the camera.
    pub fn look_at_dir(&mut self, dir: Vec3) {
        self.yaw = dir.x.atan2(dir.z);
        self.pitch = dir.y.clamp(-0.99, 0.99).asin().clamp(-1.45, 1.45);
    }

    pub fn system_view(&mut self) {
        self.focus = None;
        self.target_dist = SYSTEM_VIEW_DIST;
        self.pitch = self.pitch.max(0.5);
    }

    fn min_dist(&self, sys: &StarSystem) -> f32 {
        match self.focus.and_then(|f| sys.planet(f)) {
            Some(p) => p.radius + p.relief + 9.0,
            None => 400.0,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn orbit_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    settings: Res<Settings>,
    world: Option<Res<ClientWorld>>,
    cam: Option<ResMut<OrbitCam>>,
    mut q: Query<&mut Transform, With<MainCamera>>,
    mut egui: bevy_egui::EguiContexts,
) {
    let (Some(world), Some(mut cam)) = (world, cam) else { return };
    let sys = &world.system;
    let dt = time.delta_secs().min(0.1);
    let (ui_pointer, ui_keys) = match egui.ctx_mut() {
        Ok(ctx) => (ctx.egui_wants_pointer_input() || ctx.is_pointer_over_egui(), ctx.egui_wants_keyboard_input()),
        Err(_) => (false, false),
    };

    let target_center = cam.focus.and_then(|f| sys.planet(f)).map(|p| p.center).unwrap_or(Vec3::ZERO);
    let radius = cam.focus.and_then(|f| sys.planet(f)).map(|p| p.radius).unwrap_or(400.0);
    // Rotation speed: slower close to the surface.
    let altitude = (cam.dist - radius).max(1.0);
    let rot_speed = (altitude / (radius * 1.5)).clamp(0.08, 1.0) * 1.6 * settings.camera_speed;

    if !ui_keys {
        let mut dyaw = 0.0;
        let mut dpitch = 0.0;
        if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
            dyaw -= 1.0;
        }
        if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
            dyaw += 1.0;
        }
        if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
            dpitch += 1.0;
        }
        if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
            dpitch -= 1.0;
        }
        cam.yaw += dyaw * rot_speed * dt;
        cam.pitch = (cam.pitch + dpitch * rot_speed * dt).clamp(-1.45, 1.45);
        if keys.any_pressed([KeyCode::KeyQ, KeyCode::PageUp]) {
            cam.target_dist *= 1.0 + dt * 1.5;
        }
        if keys.any_pressed([KeyCode::KeyE, KeyCode::PageDown]) {
            cam.target_dist /= 1.0 + dt * 1.5;
        }
        if keys.just_pressed(KeyCode::Space) {
            cam.system_view();
        }
        if keys.just_pressed(KeyCode::KeyH)
            && let Some(h) = world.home_planet()
        {
            cam.focus_planet(sys, h);
        }
        if keys.just_pressed(KeyCode::Tab) && !sys.planets.is_empty() {
            let n = sys.planets.len() as u16;
            let next = match cam.focus {
                Some(f) if keys.pressed(KeyCode::ShiftLeft) => (f + n - 1) % n,
                Some(f) => (f + 1) % n,
                None => 0,
            };
            cam.focus_planet(sys, next);
        }
    }
    if !ui_pointer || buttons.pressed(MouseButton::Middle) {
        if buttons.pressed(MouseButton::Middle) {
            let d = motion.delta;
            cam.yaw -= d.x * 0.004 * rot_speed;
            cam.pitch = (cam.pitch + d.y * 0.004 * rot_speed).clamp(-1.45, 1.45);
        }
        let s = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        if s != 0.0 && !ui_pointer {
            // Zoom by altitude so the surface can be approached smoothly.
            let min = cam.min_dist(sys);
            let alt = (cam.target_dist - min).max(0.5);
            cam.target_dist = min + alt * (1.0 - s * 0.15).clamp(0.5, 1.6);
        }
    }
    let min = cam.min_dist(sys);
    cam.target_dist = cam.target_dist.clamp(min, 9000.0);
    let k = 1.0 - (-dt * 8.0).exp();
    cam.dist += (cam.target_dist - cam.dist) * k;
    cam.dist = cam.dist.max(min);
    cam.center = cam.center.lerp(target_center, 1.0 - (-dt * 6.0).exp());

    if let Ok(mut t) = q.single_mut() {
        let eye = cam.center + cam.offset_dir() * cam.dist;
        // Close to the surface, tilt the view a little towards the horizon.
        let near = 1.0 - ((cam.dist - min) / (radius * 0.8)).clamp(0.0, 1.0);
        let up_hint = Vec3::Y;
        let look = cam.center + up_hint * near * radius * 0.25;
        *t = Transform::from_translation(eye).looking_at(look, up_hint);
    }
}
