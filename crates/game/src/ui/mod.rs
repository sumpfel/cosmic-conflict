//! User interface (egui): main menu, host/join, lobby, loading screen and the in-game HUD.

mod hud;
mod menu;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui::{Color32, CornerRadius, Stroke};

use crate::AppState;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<menu::MenuUi>()
            .init_resource::<hud::HudState>()
            .add_systems(EguiPrimaryContextPass, setup_style)
            .add_systems(EguiPrimaryContextPass, menu::menu_ui.run_if(in_state(AppState::Menu)).after(setup_style))
            .add_systems(EguiPrimaryContextPass, menu::loading_ui.run_if(in_state(AppState::Loading)).after(setup_style))
            .add_systems(EguiPrimaryContextPass, (hud::planet_labels, hud::hud_ui).chain().run_if(in_state(AppState::InGame)).after(setup_style));
    }
}

pub const ACCENT: Color32 = Color32::from_rgb(90, 200, 255);
pub const TEXT: Color32 = Color32::from_rgb(220, 228, 240);
pub const DIM: Color32 = Color32::from_rgb(140, 152, 172);
pub const GOOD: Color32 = Color32::from_rgb(110, 230, 140);
pub const BAD: Color32 = Color32::from_rgb(255, 100, 90);
pub const PANEL: Color32 = Color32::from_rgba_premultiplied(8, 12, 22, 248);

pub fn res_color(r: cc_core::Res) -> Color32 {
    match r {
        cc_core::Res::Metal => Color32::from_rgb(200, 200, 212),
        cc_core::Res::Crystal => Color32::from_rgb(90, 225, 255),
        cc_core::Res::Gas => Color32::from_rgb(140, 255, 120),
    }
}

pub fn player_color32(i: u8) -> Color32 {
    let (r, g, b) = cc_core::PLAYER_COLORS[i as usize % cc_core::PLAYER_COLORS.len()];
    Color32::from_rgb(r, g, b)
}

pub fn panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(PANEL)
        .stroke(Stroke::new(1.0, Color32::from_rgba_premultiplied(90, 200, 255, 70)))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10))
}

fn setup_style(mut contexts: EguiContexts, mut done: Local<bool>) -> Result {
    if *done {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(1.0, Color32::from_rgba_premultiplied(90, 200, 255, 90));
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = Color32::from_rgb(30, 90, 130);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(22, 34, 52);
    v.widgets.inactive.bg_fill = Color32::from_rgb(22, 34, 52);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(32, 58, 88);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(40, 90, 130);
    v.widgets.inactive.corner_radius = CornerRadius::same(4);
    v.widgets.hovered.corner_radius = CornerRadius::same(4);
    v.widgets.active.corner_radius = CornerRadius::same(4);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        for (style, font) in s.text_styles.iter_mut() {
            font.size = match style {
                egui::TextStyle::Heading => 22.0,
                egui::TextStyle::Body | egui::TextStyle::Button => 15.0,
                egui::TextStyle::Small => 12.0,
                egui::TextStyle::Monospace => 14.0,
                _ => font.size,
            };
        }
    });
    *done = true;
    Ok(())
}
