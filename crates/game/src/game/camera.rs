//! RTS camera for the globes, the system view and a free-flying camera.
//!
//! **Planet view**: the camera pivots around a point on the surface of the focused planet and
//! leans away from straight down by `tilt`, so models are seen from the side as well.
//! * WASD / arrows: move over the globe (relative to the screen), Shift + middle drag: drag it
//! * Q / E: turn left / right, middle drag: turn and tilt, Ctrl + wheel: tilt
//! * T: cycle top view → tilted → low view
//! * wheel / PageUp / PageDown: zoom; the tilt fades out when zooming out to the whole globe
//!
//! **System view** (Space): circles the sun; WASD / Q / E / middle drag rotate.
//!
//! **Free camera** (C): fly anywhere. WASD fly, Q / E down / up, Shift faster, middle drag looks
//! around, the wheel dashes forward. C returns to the planet the camera looks at; Tab / H / Space /
//! F / the planet list leave it as well.
//!
//! Tab cycles planets, H jumps home, Space shows the system, a double click focuses a planet.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use cc_core::system::{Planet, StarSystem, SUN_RADIUS};

use super::scene::MainCamera;
use super::world::ClientWorld;
use crate::Content;
use crate::settings::Settings;

pub const SYSTEM_VIEW_DIST: f32 = 2600.0;
/// How far the planet view leans away from straight down by default (radians, ~50°).
pub const DEFAULT_TILT: f32 = 0.87;
/// Steepest lean (~80°: looking almost along the ground).
pub const MAX_TILT: f32 = 1.40;
/// What `T` cycles through: top view, tilted, low over the ground.
const TILT_PRESETS: [f32; 3] = [0.0, DEFAULT_TILT, 1.22];
/// Latitude limit of the pivot (keeps "north" defined).
const MAX_PITCH: f32 = 1.50;
/// Closest the camera gets to the point it looks at (m).
const MIN_VIEW_DIST: f32 = 5.0;
/// Seconds a jump between camera modes is eased over.
const BLEND_TIME: f32 = 0.45;

#[derive(Debug, Clone, Copy)]
pub struct FreeCam {
    pub pos: Vec3,
    pub rot: Quat,
    /// Distance still to travel from wheel dashes (eased out).
    dash: f32,
}

#[derive(Resource, Debug)]
pub struct OrbitCam {
    /// The planet the camera circles (`None` = the sun / whole system). In free mode: the
    /// nearest planet (for lighting and labels).
    pub focus: Option<u16>,
    /// Smoothed centre of rotation.
    pub center: Vec3,
    /// Direction from the centre to the pivot on the surface (planet view) or to the camera
    /// (system view).
    pub yaw: f32,
    pub pitch: f32,
    /// Turn of the planet view around the surface normal (0 = north is up), radians.
    pub heading: f32,
    /// Lean of the planet view away from straight down (0 = top view), radians.
    pub tilt: f32,
    pub target_tilt: f32,
    /// Distance of the camera from the centre when looking straight down.
    pub dist: f32,
    pub target_dist: f32,
    /// Set while the free camera is active.
    pub free: Option<FreeCam>,
    /// Smoothed distance of the pivot from the centre (terrain height; 0 in system view).
    pivot_r: f32,
    /// 1 in planet view, 0 in system view (eased): fades tilt and heading in and out.
    planet_mix: f32,
    /// Pose to ease from after a jump between modes, and how much of it is left (1 → 0).
    blend: Option<(Transform, f32)>,
    /// Ease from the current pose next frame (the view was moved by a command, e.g. F).
    ease_next: bool,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

/// North and east tangents at `dir` (unit vector from a planet's centre).
fn north_east(dir: Vec3) -> (Vec3, Vec3) {
    let north = (Vec3::Y - dir * dir.y).normalize_or(Vec3::Z);
    (north, north.cross(dir))
}

/// Forward (up on the screen) and right tangents of the planet view at `dir`.
fn view_frame(dir: Vec3, heading: f32) -> (Vec3, Vec3) {
    let (n, e) = north_east(dir);
    let f = n * heading.cos() + e * heading.sin();
    (f, f.cross(dir))
}

fn heading_of(dir: Vec3, forward: Vec3) -> f32 {
    let (n, e) = north_east(dir);
    forward.dot(e).atan2(forward.dot(n))
}

/// The nearest planet to `pos` and the height of `pos` above its highest terrain.
fn nearest_planet(sys: &StarSystem, pos: Vec3) -> Option<(&Planet, f32)> {
    sys.planets
        .iter()
        .map(|p| (p, pos.distance(p.center) - p.radius - p.relief))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

impl OrbitCam {
    pub fn looking_at_home(sys: &StarSystem, home: Option<u16>) -> Self {
        let mut cam = OrbitCam {
            focus: None,
            center: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.4,
            heading: 0.0,
            tilt: DEFAULT_TILT,
            target_tilt: DEFAULT_TILT,
            dist: SYSTEM_VIEW_DIST,
            target_dist: SYSTEM_VIEW_DIST,
            free: None,
            pivot_r: 0.0,
            planet_mix: 0.0,
            blend: None,
            ease_next: false,
        };
        // Dev helper: `CC_FOCUS=<planet id>` starts at another planet.
        let start = std::env::var("CC_FOCUS").ok().and_then(|v| v.parse::<u16>().ok()).or(home);
        if let Some(h) = start.and_then(|h| sys.planet(h)) {
            cam.focus = Some(h.id);
            cam.center = h.center;
            // Look at the base, slanted.
            cam.set_dir(h.start_dir);
            cam.pivot_r = h.radius + h.height(cam.offset_dir());
            cam.planet_mix = 1.0;
            cam.dist = cam.pivot_r + h.radius;
            cam.target_dist = cam.dist;
        }
        if std::env::var("CC_SYSTEM_VIEW").is_ok() {
            cam.system_view();
            cam.dist = cam.target_dist;
            cam.center = Vec3::ZERO;
            cam.pivot_r = 0.0;
            cam.planet_mix = 0.0;
        }
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f32>().ok());
        if let Some(d) = env("CC_CAM_DIST") {
            cam.dist = d;
            cam.target_dist = d;
        }
        if let Some(t) = env("CC_CAM_TILT") {
            cam.tilt = t.to_radians().clamp(0.0, MAX_TILT);
            cam.target_tilt = cam.tilt;
        }
        if let Some(h) = env("CC_CAM_HEADING") {
            cam.heading = wrap_angle(h.to_radians());
        }
        cam
    }

    /// Direction from the centre to the pivot (planet view) or the camera (system view).
    pub fn offset_dir(&self) -> Vec3 {
        Vec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos())
    }

    pub fn is_free(&self) -> bool {
        self.free.is_some()
    }

    pub fn focus_planet(&mut self, sys: &StarSystem, id: u16) {
        let Some(p) = sys.planet(id) else { return };
        let changed = self.focus != Some(id);
        if let Some(f) = self.leave_free() {
            // Continue from where the free camera is.
            self.set_dir((f.pos - p.center).normalize_or(Vec3::Y));
            self.center = p.center;
            self.pivot_r = p.radius + p.height(self.offset_dir());
            self.planet_mix = 1.0;
            self.dist = f.pos.distance(p.center);
            self.target_dist = if changed { p.radius * 2.8 } else { self.dist };
        } else if changed {
            self.target_dist = p.radius * 2.8;
        }
        self.focus = Some(id);
    }

    /// Moves the pivot so that `dir` (from the planet's centre) is in the middle of the view
    /// (eased).
    pub fn look_at_dir(&mut self, dir: Vec3) {
        self.set_dir(dir);
        self.ease_next = true;
    }

    fn set_dir(&mut self, dir: Vec3) {
        self.yaw = dir.x.atan2(dir.z);
        self.pitch = dir.y.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
    }

    pub fn system_view(&mut self) {
        if let Some(f) = self.leave_free() {
            self.set_dir(f.pos.normalize_or(Vec3::Z));
            self.center = Vec3::ZERO;
            self.dist = f.pos.length();
            self.pivot_r = 0.0;
            self.planet_mix = 0.0;
        }
        self.focus = None;
        self.target_dist = SYSTEM_VIEW_DIST;
        self.pitch = self.pitch.max(0.5);
    }

    /// Ends free mode; the next frame eases from the free camera's pose.
    fn leave_free(&mut self) -> Option<FreeCam> {
        let f = self.free.take()?;
        self.blend = Some((Transform::from_translation(f.pos).with_rotation(f.rot), 1.0));
        Some(f)
    }

    fn enter_free(&mut self, pose: &Transform) {
        self.free = Some(FreeCam { pos: pose.translation, rot: pose.rotation, dash: 0.0 });
        self.blend = None;
    }

    /// Leaves free mode for the planet view of whatever the camera looks at, keeping the view.
    fn free_to_orbit(&mut self, sys: &StarSystem) {
        let Some(f) = self.leave_free() else { return };
        let fwd = f.rot * Vec3::NEG_Z;
        if let Some((id, dir)) = sys.pick(f.pos, fwd) {
            let p = &sys.planets[id as usize];
            self.focus = Some(id);
            self.center = p.center;
            self.set_dir(dir);
            let dir = self.offset_dir();
            self.pivot_r = p.radius + p.height(dir);
            self.planet_mix = 1.0;
            let to_eye = f.pos - (p.center + dir * self.pivot_r);
            let vd = to_eye.length().max(MIN_VIEW_DIST);
            self.tilt = (to_eye / vd).dot(dir).clamp(-1.0, 1.0).acos().min(MAX_TILT);
            self.target_tilt = self.tilt;
            let up = f.rot * Vec3::Y;
            let forward = (up - dir * up.dot(dir)).normalize_or(north_east(dir).0);
            self.heading = heading_of(dir, forward);
            self.dist = self.pivot_r + vd;
            self.target_dist = self.dist;
        } else if let Some((p, _)) = nearest_planet(sys, f.pos).filter(|(p, alt)| *alt < p.radius * 6.0) {
            self.focus = Some(p.id);
            self.center = p.center;
            self.set_dir((f.pos - p.center).normalize_or(Vec3::Y));
            self.pivot_r = p.radius + p.height(self.offset_dir());
            self.planet_mix = 1.0;
            self.dist = f.pos.distance(p.center);
            self.target_dist = self.dist;
        } else {
            self.set_dir(f.pos.normalize_or(Vec3::Z));
            self.focus = None;
            self.center = Vec3::ZERO;
            self.dist = f.pos.length();
            self.target_dist = self.dist.clamp(400.0, 9000.0);
            self.pivot_r = 0.0;
            self.planet_mix = 0.0;
        }
    }

    /// Slides the pivot over the globe: `forward` / `right` in radians, relative to the view.
    /// The view keeps its orientation on the screen (parallel transport along the great circle).
    fn pan(&mut self, forward: f32, right: f32) {
        let dir = self.offset_dir();
        let (f, r) = view_frame(dir, self.heading);
        let m = f * forward + r * right;
        let angle = m.length();
        if angle < 1e-7 {
            return;
        }
        let q = Quat::from_axis_angle(dir.cross(m).normalize(), angle);
        let (new_dir, new_f) = (q * dir, q * f);
        self.set_dir(new_dir);
        let dir = self.offset_dir();
        let tangent = (new_f - dir * new_f.dot(dir)).normalize_or(north_east(dir).0);
        self.heading = heading_of(dir, tangent);
    }

    /// Eye, look-at point and up vector of the planet / system view (`radius`: of the focus).
    fn pose(&self, radius: f32) -> (Vec3, Vec3, Vec3) {
        let dir = self.offset_dir();
        let vd = (self.dist - self.pivot_r).max(1.0);
        // Zoomed out to the whole globe the view straightens up so the planet stays centred.
        let tilt = self.tilt * (1.0 - smoothstep(radius * 2.0, radius * 5.0, vd)) * self.planet_mix;
        let (forward, _) = view_frame(dir, self.heading * self.planet_mix);
        let pivot = self.center + dir * self.pivot_r;
        let eye = pivot + (dir * tilt.cos() - forward * tilt.sin()) * vd;
        let up = dir * tilt.sin() + forward * tilt.cos();
        (eye, pivot, up)
    }

    fn min_dist(&self, sys: &StarSystem) -> f32 {
        match self.focus.and_then(|f| sys.planet(f)) {
            Some(p) => p.radius + p.height(self.offset_dir()) + MIN_VIEW_DIST,
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
    content: Res<Content>,
    world: Option<Res<ClientWorld>>,
    cam: Option<ResMut<OrbitCam>>,
    mut q: Query<&mut Transform, With<MainCamera>>,
    mut egui: bevy_egui::EguiContexts,
) {
    let (Some(world), Some(mut cam)) = (world, cam) else { return };
    let Ok(mut t) = q.single_mut() else { return };
    let sys = &world.system;
    let dt = time.delta_secs().min(0.1);
    let (ui_pointer, ui_keys) = match egui.ctx_mut() {
        Ok(ctx) => (ctx.egui_wants_pointer_input() || ctx.is_pointer_over_egui(), ctx.egui_wants_keyboard_input()),
        Err(_) => (false, false),
    };
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let middle = buttons.pressed(MouseButton::Middle);
    let drag = if middle { motion.delta } else { Vec2::ZERO };
    let wheel = if ui_pointer {
        0.0
    } else {
        match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        }
    };

    // --- Mode switches (work in every mode) --------------------------------------------------
    if !ui_keys {
        if keys.just_pressed(KeyCode::KeyC) {
            if cam.is_free() {
                cam.free_to_orbit(sys);
            } else {
                cam.enter_free(&t);
            }
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
                Some(f) if shift => (f + n - 1) % n,
                Some(f) => (f + 1) % n,
                None => 0,
            };
            cam.focus_planet(sys, next);
        }
    }

    if cam.is_free() {
        *t = free_fly(&mut cam, sys, &keys, ui_keys, drag, wheel, &settings, dt);
        return;
    }

    let planet = cam.focus.and_then(|f| sys.planet(f));
    let target_center = planet.map(|p| p.center).unwrap_or(Vec3::ZERO);
    let radius = planet.map(|p| p.radius).unwrap_or(400.0);

    // --- Movement ----------------------------------------------------------------------------
    let axis = |neg: &[KeyCode], pos: &[KeyCode]| -> f32 {
        if ui_keys {
            return 0.0;
        }
        keys.any_pressed(pos.iter().copied()) as i32 as f32 - keys.any_pressed(neg.iter().copied()) as i32 as f32
    };
    let move_x = axis(&[KeyCode::KeyA, KeyCode::ArrowLeft], &[KeyCode::KeyD, KeyCode::ArrowRight]);
    let move_y = axis(&[KeyCode::KeyS, KeyCode::ArrowDown], &[KeyCode::KeyW, KeyCode::ArrowUp]);
    let turn = axis(&[KeyCode::KeyQ], &[KeyCode::KeyE]);
    if planet.is_some() {
        let vd = (cam.dist - cam.pivot_r).max(MIN_VIEW_DIST);
        // Pan speed in rad/s: about the same screen speed at every zoom.
        let pan_speed = (vd / (radius * 1.5)).clamp(0.05, 1.0) * 1.6 * settings.camera_speed * if shift { 2.5 } else { 1.0 };
        let (mut fwd, mut right) = (move_y * pan_speed * dt, move_x * pan_speed * dt);
        if shift && middle {
            // Grab the ground and drag it.
            let k = 0.0009 * vd / radius;
            fwd += drag.y * k;
            right -= drag.x * k;
        }
        cam.pan(fwd, right);
        cam.heading += turn * 1.8 * settings.camera_speed * dt;
        if middle && !shift {
            cam.heading -= drag.x * 0.005;
            cam.target_tilt -= drag.y * 0.004;
        }
        if ctrl {
            cam.target_tilt += wheel * 0.1;
        }
        if !ui_keys && keys.just_pressed(KeyCode::KeyT) {
            cam.target_tilt = TILT_PRESETS.iter().copied().find(|p| *p > cam.target_tilt + 0.05).unwrap_or(TILT_PRESETS[0]);
        }
        cam.target_tilt = cam.target_tilt.clamp(0.0, MAX_TILT);
        cam.heading = wrap_angle(cam.heading);
    } else {
        let altitude = (cam.dist - radius).max(1.0);
        let rot_speed = (altitude / (radius * 1.5)).clamp(0.08, 1.0) * 1.6 * settings.camera_speed;
        cam.yaw += (move_x + turn) * rot_speed * dt;
        cam.pitch = (cam.pitch + move_y * rot_speed * dt).clamp(-MAX_PITCH, MAX_PITCH);
        cam.yaw -= drag.x * 0.004 * rot_speed;
        cam.pitch = (cam.pitch + drag.y * 0.004 * rot_speed).clamp(-MAX_PITCH, MAX_PITCH);
    }

    // --- Zoom --------------------------------------------------------------------------------
    let zoom = (if ctrl { 0.0 } else { wheel }) + axis(&[KeyCode::PageUp], &[KeyCode::PageDown]) * dt * 8.0;
    let min = cam.min_dist(sys);
    if zoom != 0.0 {
        // Zoom by altitude so the surface can be approached smoothly.
        let alt = (cam.target_dist - min).max(0.5);
        cam.target_dist = min + alt * (1.0 - zoom * 0.15).clamp(0.5, 1.6);
    }

    // --- Smoothing ---------------------------------------------------------------------------
    cam.target_dist = cam.target_dist.clamp(min, 9000.0);
    let k = 1.0 - (-dt * 8.0).exp();
    cam.dist += (cam.target_dist - cam.dist) * k;
    if planet.is_some() {
        cam.dist = cam.dist.max(min);
    }
    let k_center = 1.0 - (-dt * 6.0).exp();
    cam.center = cam.center.lerp(target_center, k_center);
    let pivot_target = planet.map(|p| p.radius + p.height(cam.offset_dir())).unwrap_or(0.0);
    cam.pivot_r += (pivot_target - cam.pivot_r) * k_center;
    let mix_target = if planet.is_some() { 1.0 } else { 0.0 };
    cam.planet_mix += (mix_target - cam.planet_mix) * (1.0 - (-dt * 5.0).exp());
    cam.tilt += (cam.target_tilt - cam.tilt) * (1.0 - (-dt * 10.0).exp());

    // --- Pose --------------------------------------------------------------------------------
    let (mut eye, pivot, up) = cam.pose(radius);
    if let Some(p) = planet {
        // Never below the terrain (a low view next to a mountain).
        let rel = eye - p.center;
        let r = rel.length();
        let ed = rel / r.max(1e-3);
        let floor = p.radius + p.height(ed) + 1.5;
        if r < floor {
            eye = p.center + ed * floor;
        }
        eye = outside_buildings(&world, &content, p.id, eye);
    }
    let mut pose = Transform::from_translation(eye).looking_at(pivot, up);
    if std::mem::take(&mut cam.ease_next) && cam.blend.is_none() {
        cam.blend = Some((*t, 1.0));
    }
    if let Some((from, w)) = cam.blend {
        let w = w - dt / BLEND_TIME;
        if w <= 0.0 {
            cam.blend = None;
        } else {
            cam.blend = Some((from, w));
            let a = smoothstep(0.0, 1.0, 1.0 - w);
            pose.translation = from.translation.lerp(pose.translation, a);
            pose.rotation = from.rotation.slerp(pose.rotation, a);
        }
    }
    *t = pose;
}

/// Pushes `eye` out of the rough shape of every building on `planet` (an upright ellipsoid sized
/// from the footprint: wide as the building, about twice as tall) so a zoomed-in camera never
/// ends up inside a wall or a tower.
fn outside_buildings(world: &ClientWorld, content: &Content, planet: u16, mut eye: Vec3) -> Vec3 {
    for b in world.buildings.values().filter(|b| b.snap.planet == planet) {
        let r = content.0.building(b.snap.kind).radius;
        let (wide, tall) = (r * 1.4 + 1.0, r * 2.6 + 2.0);
        let rel = eye - b.pos;
        if rel.length_squared() > tall * tall {
            continue;
        }
        let h = rel.dot(b.up);
        let across = (rel - b.up * h).length();
        let s = ((across / wide).powi(2) + (h.max(0.0) / tall).powi(2)).sqrt();
        if s < 1.0 {
            eye = if s > 1e-3 { b.pos + rel / s } else { b.pos + b.up * tall };
        }
    }
    eye
}

/// One frame of the free camera. Returns the camera pose.
#[allow(clippy::too_many_arguments)]
fn free_fly(
    cam: &mut OrbitCam,
    sys: &StarSystem,
    keys: &ButtonInput<KeyCode>,
    ui_keys: bool,
    drag: Vec2,
    wheel: f32,
    settings: &Settings,
    dt: f32,
) -> Transform {
    let Some(mut f) = cam.free else { return Transform::default() };
    let near = nearest_planet(sys, f.pos);
    // Height above the nearest obstacle (planet or sun): sets the speed.
    let sun_alt = f.pos.length() - SUN_RADIUS;
    let alt = near.map(|(_, a)| a).unwrap_or(f32::MAX).min(sun_alt).max(0.5);

    // "Up" is away from a nearby planet, the system's up in open space.
    let up = match near {
        Some((p, a)) => {
            let radial = (f.pos - p.center).normalize_or(Vec3::Y);
            let w = 1.0 - smoothstep(p.radius * 1.5, p.radius * 4.0, a);
            (Vec3::Y * (1.0 - w) + radial * w).normalize_or(radial)
        }
        None => Vec3::Y,
    };

    // Look around (middle drag), keep the horizon level.
    if drag != Vec2::ZERO {
        f.rot = Quat::from_axis_angle(up, -drag.x * 0.003) * f.rot;
        let pitched = f.rot * Quat::from_rotation_x(-drag.y * 0.003);
        if (pitched * Vec3::NEG_Z).dot(up).abs() < 0.995 {
            f.rot = pitched;
        }
    }
    let fwd = f.rot * Vec3::NEG_Z;
    if fwd.dot(up).abs() < 0.995 {
        let level = Transform::default().looking_to(fwd, up).rotation;
        f.rot = f.rot.slerp(level, 1.0 - (-dt * 5.0).exp());
    }

    // Fly.
    let fwd = f.rot * Vec3::NEG_Z;
    let right = f.rot * Vec3::X;
    let mut mv = Vec3::ZERO;
    if !ui_keys {
        for (ks, v) in [
            ([KeyCode::KeyW, KeyCode::ArrowUp], fwd),
            ([KeyCode::KeyS, KeyCode::ArrowDown], -fwd),
            ([KeyCode::KeyD, KeyCode::ArrowRight], right),
            ([KeyCode::KeyA, KeyCode::ArrowLeft], -right),
            ([KeyCode::KeyE, KeyCode::PageUp], up),
            ([KeyCode::KeyQ, KeyCode::PageDown], -up),
        ] {
            if keys.any_pressed(ks) {
                mv += v;
            }
        }
    }
    let boost = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) { 4.0 } else { 1.0 };
    let speed = alt.clamp(3.0, 1500.0) * settings.camera_speed * boost;
    f.pos += mv.normalize_or_zero() * speed * dt;
    f.dash += wheel * (alt * 0.25).max(1.5);
    let step = f.dash * (1.0 - (-dt * 10.0).exp());
    f.pos += fwd * step;
    f.dash -= step;

    // Stay out of planets and the sun, and inside the system.
    for p in &sys.planets {
        let rel = f.pos - p.center;
        let r = rel.length();
        if r < p.radius + p.relief + 2.0 {
            let d = rel / r.max(1e-3);
            let floor = p.radius + p.height(d) + 1.0;
            if r < floor {
                f.pos = p.center + d * floor;
                f.dash = 0.0;
            }
        }
    }
    if f.pos.length() < SUN_RADIUS * 1.2 {
        f.pos = f.pos.normalize_or(Vec3::Z) * SUN_RADIUS * 1.2;
        f.dash = 0.0;
    }
    f.pos = f.pos.clamp_length_max(12_000.0);

    // The nearest planet counts as focused (sunlight exposure, labels, Tab).
    cam.focus = near.filter(|(p, a)| *a < p.radius * 4.0).map(|(p, _)| p.id);
    if let Some(p) = cam.focus.and_then(|id| sys.planet(id)) {
        cam.center = p.center;
        cam.dist = f.pos.distance(p.center);
    }
    cam.free = Some(f);
    Transform::from_translation(f.pos).with_rotation(f.rot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::settings::MatchSettings;

    fn system() -> StarSystem {
        cc_core::system::generate(&MatchSettings::default(), 2)
    }

    /// A settled planet view of `p` (no easing left).
    fn planet_view(p: &Planet, dir: Vec3, heading: f32, tilt: f32, view_dist: f32) -> OrbitCam {
        let mut cam = OrbitCam::looking_at_home(&system(), None);
        cam.focus = Some(p.id);
        cam.center = p.center;
        cam.set_dir(dir);
        cam.heading = heading;
        cam.tilt = tilt;
        cam.target_tilt = tilt;
        cam.pivot_r = p.radius + p.height(cam.offset_dir());
        cam.planet_mix = 1.0;
        cam.dist = cam.pivot_r + view_dist;
        cam.target_dist = cam.dist;
        cam
    }

    fn transform(cam: &OrbitCam, radius: f32) -> Transform {
        let (eye, pivot, up) = cam.pose(radius);
        Transform::from_translation(eye).looking_at(pivot, up)
    }

    #[test]
    fn panning_there_and_back_restores_the_view() {
        let sys = system();
        let p = &sys.planets[0];
        for (dir, heading) in [(Vec3::Z, 0.0), (Vec3::new(0.3, 0.8, 0.5).normalize(), 2.0), (Vec3::new(-0.6, -0.7, 0.2).normalize(), -1.2)] {
            let mut cam = planet_view(p, dir, heading, DEFAULT_TILT, 30.0);
            let before = (cam.yaw, cam.pitch, cam.heading);
            cam.pan(0.15, -0.05);
            assert!((cam.offset_dir() - dir).length() > 0.05, "the pivot moved");
            cam.pan(-0.15, 0.05);
            // Second-order drift only (the two great-circle steps are not exact inverses).
            assert!((cam.offset_dir() - dir).length() < 0.02, "{dir} -> {}", cam.offset_dir());
            assert!(wrap_angle(cam.heading - before.2).abs() < 0.03, "heading {} -> {}", before.2, cam.heading);
        }
    }

    #[test]
    fn panning_forward_moves_up_the_screen() {
        let sys = system();
        let p = &sys.planets[0];
        let mut cam = planet_view(p, Vec3::new(0.2, 0.3, 0.9).normalize(), 0.7, 0.0, 30.0);
        let t = transform(&cam, p.radius);
        let screen_up = t.up().as_vec3();
        let before = p.center + cam.offset_dir() * p.radius;
        cam.pan(0.05, 0.0);
        let moved = p.center + cam.offset_dir() * p.radius - before;
        assert!(moved.normalize().dot(screen_up) > 0.99, "{moved} vs {screen_up}");
    }

    #[test]
    fn tilt_leans_the_view_and_keeps_the_horizon_level() {
        let sys = system();
        let p = &sys.planets[0];
        let dir = Vec3::new(0.5, 0.2, 0.8).normalize();
        let top = planet_view(p, dir, 0.4, 0.0, 30.0);
        let tilted = planet_view(p, dir, 0.4, 1.0, 30.0);
        let down = |c: &OrbitCam| transform(c, p.radius).forward().as_vec3().dot(-dir);
        assert!(down(&top) > 0.999, "top view looks straight down");
        assert!((down(&tilted) - 1.0f32.cos()).abs() < 0.01, "tilted by 1 rad");
        // No roll: the camera's right vector stays parallel to the ground at the pivot.
        assert!(transform(&tilted, p.radius).right().as_vec3().dot(dir).abs() < 1e-3);
    }

    #[test]
    fn free_camera_round_trip_keeps_the_view() {
        let sys = system();
        let p = &sys.planets[0];
        let dir = Vec3::new(-0.3, 0.4, 0.85).normalize();
        let mut cam = planet_view(p, dir, 1.1, 0.9, 25.0);
        let before = transform(&cam, p.radius);
        cam.enter_free(&before);
        cam.free_to_orbit(&sys);
        assert_eq!(cam.focus, Some(p.id));
        let after = transform(&cam, p.radius);
        assert!(before.translation.distance(after.translation) < 0.05, "{} vs {}", before.translation, after.translation);
        assert!(before.rotation.angle_between(after.rotation) < 0.01);
        assert!((cam.tilt - 0.9).abs() < 0.01 && wrap_angle(cam.heading - 1.1).abs() < 0.01);
    }

    #[test]
    fn leaving_the_free_camera_for_a_planet_eases_from_its_pose() {
        let sys = system();
        let p = &sys.planets[1];
        let mut cam = OrbitCam::looking_at_home(&sys, Some(0));
        let pose = Transform::from_xyz(p.center.x + p.radius * 3.0, p.center.y, p.center.z).looking_at(p.center, Vec3::Y);
        cam.enter_free(&pose);
        cam.focus_planet(&sys, p.id);
        assert!(!cam.is_free());
        assert_eq!(cam.focus, Some(p.id));
        assert!(cam.blend.is_some_and(|(from, w)| from.translation == pose.translation && w == 1.0));
        // The pivot is under the free camera.
        assert!(cam.offset_dir().dot(Vec3::X) > 0.99);
    }
}
