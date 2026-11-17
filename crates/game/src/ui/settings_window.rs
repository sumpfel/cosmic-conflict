//! The settings window: graphics quality (presets for weaker PCs and single options), sound and
//! camera. Opened from the main menu and the in-game menu; changes apply at once.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use egui::{Align2, RichText};

use super::DIM;
use crate::settings::{AntiAliasing, Graphics, Level, Settings};

#[derive(Resource, Default)]
pub struct SettingsWindow {
    pub open: bool,
    /// Sound / camera sliders changed: saved when the window closes.
    dirty: bool,
}

const SURFACE: [Level; 4] = [Level::Low, Level::Medium, Level::High, Level::Ultra];
const MESH: [Level; 3] = [Level::Low, Level::Medium, Level::High];

/// One option: name, a drop-down with `options`, and what the chosen step does.
fn level_row(ui: &mut egui::Ui, label: &str, value: &mut Level, options: &[Level], help: fn(Level) -> &'static str) {
    ui.label(label);
    egui::ComboBox::from_id_salt(label).selected_text(value.label()).width(110.0).show_ui(ui, |ui| {
        for l in options {
            ui.selectable_value(value, *l, l.label()).on_hover_text(help(*l));
        }
    });
    ui.label(RichText::new(help(*value)).small().color(DIM));
    ui.end_row();
}

pub fn settings_ui(
    mut contexts: EguiContexts,
    mut win: ResMut<SettingsWindow>,
    mut settings: ResMut<Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mut started: Local<bool>,
) -> Result {
    // Dev helper: `CC_SETTINGS=1` opens the window at start (screenshots).
    if !*started {
        *started = true;
        win.open |= std::env::var_os("CC_SETTINGS").is_some();
    }
    if !win.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    // Edit a copy so `Settings` only counts as changed when something did change.
    let mut s = settings.clone();
    let mut open = true;
    let mut close = keys.just_pressed(KeyCode::Escape);
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(&ctx, |ui| {
            ui.set_width(560.0);
            ui.heading("Graphics");
            ui.horizontal(|ui| {
                ui.label("Preset:");
                for l in Graphics::PRESETS {
                    if ui.selectable_label(s.graphics.preset == Some(l), l.label()).clicked() {
                        s.graphics = Graphics::preset(l);
                    }
                }
                if s.graphics.preset.is_none() {
                    ui.label(RichText::new("Custom").italics().color(DIM));
                }
            });
            ui.label(RichText::new("Low and Medium are meant for laptops and older graphics cards.").small().color(DIM));
            ui.add_space(6.0);
            let before = s.graphics;
            egui::Grid::new("graphics_grid").num_columns(3).spacing([14.0, 8.0]).show(ui, |ui| {
                let g = &mut s.graphics;
                level_row(ui, "Planet surface", &mut g.surface, &SURFACE, |l| match l {
                    Level::Off | Level::Low => "plain colours",
                    Level::Medium => "detail noise, rock slopes, ocean glint",
                    Level::High => "+ bump mapping, waves, cloud shadows",
                    Level::Ultra => "+ finest ground detail",
                });
                level_row(ui, "Shadows", &mut g.shadows, &Level::ALL, |l| match l {
                    Level::Off => "no sun shadows",
                    Level::Low => "1024 px, 2 cascades",
                    Level::Medium => "2048 px, 3 cascades, soft",
                    Level::High => "2048 px, 4 cascades, soft",
                    Level::Ultra => "4096 px, 4 cascades, soft",
                });
                level_row(ui, "Atmosphere", &mut g.atmosphere, &Level::ALL, |l| match l {
                    Level::Off => "no air glow",
                    Level::Low => "approximate scattering",
                    Level::Medium => "ray-marched, 8 steps",
                    Level::High => "ray-marched, 12 steps",
                    Level::Ultra => "ray-marched, 16 steps",
                });
                level_row(ui, "Clouds", &mut g.clouds, &Level::ALL, |l| match l {
                    Level::Off => "no clouds",
                    Level::Low => "simple clouds",
                    Level::Medium => "+ cloud shadows on the ground",
                    Level::High => "+ more detail, self-shadowing",
                    Level::Ultra => "finest cloud detail",
                });
                level_row(ui, "Planet mesh", &mut g.planet_mesh, &MESH, |_| "applies to the next match");
                ui.label("Anti-aliasing");
                egui::ComboBox::from_id_salt("aa").selected_text(g.antialiasing.label()).width(110.0).show_ui(ui, |ui| {
                    for a in AntiAliasing::ALL {
                        ui.selectable_value(&mut g.antialiasing, a, a.label());
                    }
                });
                ui.label(RichText::new("smooth edges").small().color(DIM));
                ui.end_row();
                ui.label("Bloom");
                ui.checkbox(&mut g.bloom, "");
                ui.label(RichText::new("glow around the sun, engines and lights").small().color(DIM));
                ui.end_row();
            });
            if s.graphics != before {
                s.graphics.preset = None;
            }
            ui.add_space(6.0);
            ui.separator();
            ui.heading("Sound and camera");
            egui::Grid::new("misc_grid").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                ui.label("Volume");
                ui.add(egui::Slider::new(&mut s.volume, 0.0..=1.0).show_value(true));
                ui.end_row();
                ui.label("Camera speed");
                ui.add(egui::Slider::new(&mut s.camera_speed, 0.3..=3.0));
                ui.end_row();
            });
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    let graphics_changed = s.graphics != settings.graphics;
    if graphics_changed || s.volume != settings.volume || s.camera_speed != settings.camera_speed {
        settings.graphics = s.graphics;
        settings.volume = s.volume;
        settings.camera_speed = s.camera_speed;
        if graphics_changed {
            settings.save();
        } else {
            win.dirty = true;
        }
    }
    if close || !open {
        win.open = false;
        if win.dirty {
            settings.save();
            win.dirty = false;
        }
    }
    Ok(())
}
