//! Main menu, host / join screens, the lobby and the loading screen.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use cc_core::settings::{Abundance, MatchSettings, StartResources, SystemSize};
use cc_core::PLAYER_COLORS;
use cc_protocol::{ClientMsg, LobbyCmd};
use egui::{Align2, Color32, RichText};

use super::{ACCENT, BAD, DIM, GOOD, panel_frame, player_color32};
use crate::audio::{Sfx, Sound};
use crate::net::{ChatLog, HostedServer, LobbyRes, MenuMessage, Net, NetStatus};
use crate::settings::Settings;
use crate::{Content, MenuScreen};

#[derive(Resource, Default)]
pub struct MenuUi {
    port_text: String,
    seed_text: String,
}

fn title(ui: &mut egui::Ui) {
    ui.label(RichText::new("COSMIC").size(64.0).strong().color(ACCENT));
    ui.label(RichText::new("CONFLICT").size(64.0).strong().color(Color32::WHITE));
    ui.label(RichText::new("Every commander starts on a world of their own. Take the system.").size(16.0).color(DIM));
    ui.add_space(24.0);
}

fn big_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add_sized([260.0, 42.0], egui::Button::new(RichText::new(text).size(19.0)))
}

#[allow(clippy::too_many_arguments)]
pub fn menu_ui(
    mut contexts: EguiContexts,
    mut screen: ResMut<MenuScreen>,
    mut settings: ResMut<Settings>,
    mut net: ResMut<Net>,
    mut hosted: ResMut<HostedServer>,
    lobby: Res<LobbyRes>,
    mut chat: ResMut<ChatLog>,
    mut msg: ResMut<MenuMessage>,
    content: Res<Content>,
    mut state: ResMut<MenuUi>,
    mut sound: ResMut<Sound>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if ctx.input(|i| i.pointer.any_click()) && ctx.is_pointer_over_egui() {
        sound.ui(Sfx::Click);
    }
    let left = ctx.content_rect().width() * 0.06;
    match *screen {
        MenuScreen::Main => {
            egui::Area::new("main_menu".into()).anchor(Align2::LEFT_TOP, egui::vec2(left, 70.0)).show(&ctx, |ui| {
                title(ui);
                ui.horizontal(|ui| {
                    ui.label("Commander name:");
                    if ui.add(egui::TextEdit::singleline(&mut settings.name).desired_width(150.0)).lost_focus() {
                        settings.save();
                    }
                });
                ui.add_space(14.0);
                if big_button(ui, "Host Game").clicked() {
                    state.port_text = settings.port.to_string();
                    *screen = MenuScreen::Host;
                }
                if big_button(ui, "Join Game").clicked() {
                    *screen = MenuScreen::Join;
                }
                if big_button(ui, "Quit").clicked() {
                    exit.write(AppExit::Success);
                }
                ui.add_space(30.0);
                ui.label(RichText::new("Controls: middle-drag / WASD rotate · wheel zoom · left select · right order").small().color(DIM));
                ui.label(RichText::new("Tab next planet · H home · Space system view · F focus selection").small().color(DIM));
            });
        }
        MenuScreen::Host => {
            egui::Area::new("host".into()).anchor(Align2::LEFT_TOP, egui::vec2(left, 70.0)).show(&ctx, |ui| {
                title(ui);
                panel_frame().show(ui, |ui| {
                    ui.set_width(330.0);
                    ui.heading("Host a game");
                    ui.horizontal(|ui| {
                        ui.label("Port:");
                        ui.add(egui::TextEdit::singleline(&mut state.port_text).desired_width(80.0));
                    });
                    ui.label(RichText::new("Other players join with your IP address and this port.").small().color(DIM));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Host").clicked() {
                            match state.port_text.trim().parse::<u16>() {
                                Ok(port) => match hosted.start(port, content.0.clone()) {
                                    Ok(()) => {
                                        settings.port = port;
                                        settings.save();
                                        net.connect(format!("127.0.0.1:{port}"));
                                    }
                                    Err(e) => msg.0 = Some(e),
                                },
                                Err(_) => msg.0 = Some("Invalid port.".into()),
                            }
                        }
                        if ui.button("Back").clicked() {
                            *screen = MenuScreen::Main;
                        }
                    });
                    if let NetStatus::Connecting(a) = &net.status {
                        ui.label(format!("Connecting to {a} …"));
                    }
                });
            });
        }
        MenuScreen::Join => {
            egui::Area::new("join".into()).anchor(Align2::LEFT_TOP, egui::vec2(left, 70.0)).show(&ctx, |ui| {
                title(ui);
                panel_frame().show(ui, |ui| {
                    ui.set_width(330.0);
                    ui.heading("Join a game");
                    ui.horizontal(|ui| {
                        ui.label("Address:");
                        ui.add(egui::TextEdit::singleline(&mut settings.address).desired_width(200.0));
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Join").clicked() {
                            let mut a = settings.address.trim().to_string();
                            if !a.contains(':') {
                                a = format!("{a}:{}", cc_protocol::DEFAULT_PORT);
                            }
                            settings.save();
                            net.connect(a);
                        }
                        if ui.button("Back").clicked() {
                            net.disconnect();
                            *screen = MenuScreen::Main;
                        }
                    });
                    if let NetStatus::Connecting(a) = &net.status {
                        ui.label(format!("Connecting to {a} …"));
                    }
                });
            });
        }
        MenuScreen::Lobby => lobby_screen(&ctx, &mut net, &mut hosted, &lobby, &mut chat, &mut settings, &mut state, &mut screen),
    }
    if let Some(text) = msg.0.clone() {
        egui::Area::new("menu_error".into())
            .anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -40.0))
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                panel_frame().stroke(egui::Stroke::new(1.2, BAD)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(text).color(BAD));
                        if ui.button("OK").clicked() {
                            msg.0 = None;
                        }
                    });
                });
            });
    }
    Ok(())
}

fn combo<T: Copy + PartialEq>(ui: &mut egui::Ui, id: &str, value: &mut T, all: &[T], label: fn(T) -> &'static str, enabled: bool) -> bool {
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(id).selected_text(label(*value)).width(120.0).show_ui(ui, |ui| {
            for v in all {
                if ui.selectable_label(*v == *value, label(*v)).clicked() && *v != *value {
                    *value = *v;
                    changed = true;
                }
            }
        });
    });
    changed
}

#[allow(clippy::too_many_arguments)]
fn lobby_screen(
    ctx: &egui::Context,
    net: &mut Net,
    hosted: &mut HostedServer,
    lobby: &LobbyRes,
    chat: &mut ChatLog,
    settings: &mut Settings,
    state: &mut MenuUi,
    screen: &mut MenuScreen,
) {
    let Some(ls) = lobby.state.as_ref() else {
        egui::Area::new("lobby_wait".into()).anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
            ui.label("Waiting for the lobby …");
        });
        return;
    };
    let me = net.me;
    let is_host = me == Some(ls.host);
    let left = ctx.content_rect().width() * 0.05;
    egui::Area::new("lobby".into()).anchor(Align2::LEFT_TOP, egui::vec2(left, 50.0)).show(ctx, |ui| {
        ui.label(RichText::new("LOBBY").size(36.0).strong().color(ACCENT));
        ui.label(RichText::new(&net.server_name).color(DIM));
        ui.add_space(10.0);
        ui.horizontal_top(|ui| {
            // Players and chat.
            panel_frame().show(ui, |ui| {
                ui.set_width(360.0);
                ui.set_min_height(420.0);
                ui.heading(format!("Commanders {}/{}", ls.players.len(), ls.settings.max_players));
                ui.separator();
                for p in &ls.players {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        ui.painter().circle_filled(r.center(), 6.0, player_color32(p.color));
                        let mut name = RichText::new(&p.name);
                        if Some(p.id) == me {
                            name = name.strong();
                        }
                        ui.label(name);
                        if p.id == ls.host {
                            ui.label(RichText::new("host").small().color(ACCENT));
                        } else if p.ready {
                            ui.label(RichText::new("ready").small().color(GOOD));
                        } else {
                            ui.label(RichText::new("not ready").small().color(DIM));
                        }
                        if is_host && Some(p.id) != me && ui.small_button("kick").clicked() {
                            net.send(ClientMsg::Lobby(LobbyCmd::Kick(p.id)));
                        }
                    });
                }
                ui.add_space(8.0);
                ui.label("Your colour:");
                ui.horizontal(|ui| {
                    for i in 0..PLAYER_COLORS.len() as u8 {
                        let taken = ls.players.iter().any(|p| p.color == i && Some(p.id) != me);
                        let mine = ls.players.iter().any(|p| p.color == i && Some(p.id) == me);
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
                        ui.painter().circle_filled(r.center(), if mine { 11.0 } else { 8.0 }, player_color32(i).gamma_multiply(if taken { 0.25 } else { 1.0 }));
                        if mine {
                            ui.painter().circle_stroke(r.center(), 11.0, egui::Stroke::new(2.0, Color32::WHITE));
                        }
                        if resp.clicked() && !taken {
                            settings.color = i;
                            settings.save();
                            net.send(ClientMsg::Lobby(LobbyCmd::SetColor(i)));
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().max_height(150.0).stick_to_bottom(true).show(ui, |ui| {
                    for (from, text) in &chat.lines {
                        match from.and_then(|f| ls.players.iter().find(|p| p.id == f)) {
                            Some(p) => {
                                ui.label(RichText::new(format!("{}: {text}", p.name)).color(player_color32(p.color)));
                            }
                            None => {
                                ui.label(RichText::new(text).italics().color(DIM));
                            }
                        }
                    }
                });
                let resp = ui.add(egui::TextEdit::singleline(&mut chat.input).hint_text("Chat …").desired_width(330.0));
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !chat.input.trim().is_empty() {
                    net.send(ClientMsg::Lobby(LobbyCmd::Chat(std::mem::take(&mut chat.input))));
                    resp.request_focus();
                }
            });
            // Match settings.
            panel_frame().show(ui, |ui| {
                ui.set_width(320.0);
                ui.set_min_height(420.0);
                ui.heading("Star system");
                ui.separator();
                let mut s: MatchSettings = ls.settings.clone();
                let mut changed = false;
                egui::Grid::new("settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Size");
                    changed |= combo(ui, "size", &mut s.size, &SystemSize::ALL, SystemSize::label, is_host);
                    ui.end_row();
                    ui.label("Resources");
                    changed |= combo(ui, "abundance", &mut s.abundance, &Abundance::ALL, Abundance::label, is_host);
                    ui.end_row();
                    ui.label("Start stock");
                    changed |= combo(ui, "start", &mut s.start, &StartResources::ALL, StartResources::label, is_host);
                    ui.end_row();
                    ui.label("Max players");
                    ui.add_enabled_ui(is_host, |ui| {
                        changed |= ui.add(egui::Slider::new(&mut s.max_players, 1..=MatchSettings::MAX_PLAYERS)).changed();
                    });
                    ui.end_row();
                    ui.label("Seed");
                    ui.horizontal(|ui| {
                        if state.seed_text.parse::<u64>().ok() != Some(s.seed) && !ui.memory(|m| m.has_focus(egui::Id::new("seed"))) {
                            state.seed_text = s.seed.to_string();
                        }
                        let r = ui.add_enabled(is_host, egui::TextEdit::singleline(&mut state.seed_text).id(egui::Id::new("seed")).desired_width(110.0));
                        if r.lost_focus()
                            && let Ok(v) = state.seed_text.trim().parse::<u64>()
                        {
                            s.seed = v;
                            changed = true;
                        }
                        if is_host && ui.small_button("random").clicked() {
                            s.seed = rand::random::<u64>() % 1_000_000_000;
                            changed = true;
                        }
                    });
                    ui.end_row();
                });
                if changed && is_host {
                    net.send(ClientMsg::Lobby(LobbyCmd::Settings(s.clone())));
                }
                ui.add_space(8.0);
                let planets = ls.players.len() * (1 + s.size.neutrals_per_player());
                ui.label(RichText::new(format!("{} home worlds + {} neutral planets", ls.players.len(), planets - ls.players.len())).color(DIM));
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    if is_host {
                        if ui.add_sized([150.0, 36.0], egui::Button::new(RichText::new("Start match").size(17.0).color(GOOD))).clicked() {
                            net.send(ClientMsg::Lobby(LobbyCmd::Start));
                        }
                    } else {
                        let ready = ls.players.iter().find(|p| Some(p.id) == me).is_some_and(|p| p.ready);
                        let text = if ready { "Not ready" } else { "Ready" };
                        if ui.add_sized([150.0, 36.0], egui::Button::new(RichText::new(text).size(17.0))).clicked() {
                            net.send(ClientMsg::Lobby(LobbyCmd::SetReady(!ready)));
                        }
                    }
                    if ui.add_sized([100.0, 36.0], egui::Button::new("Leave")).clicked() {
                        net.disconnect();
                        hosted.stop();
                        *screen = MenuScreen::Main;
                    }
                });
            });
        });
    });
}

pub fn loading_ui(mut contexts: EguiContexts, time: Res<Time>) -> Result {
    let ctx = contexts.ctx_mut()?;
    egui::Area::new("loading".into()).anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
        let dots = ".".repeat(1 + (time.elapsed_secs() * 2.0) as usize % 3);
        ui.label(RichText::new(format!("Charting the star system{dots}")).size(26.0).color(ACCENT));
    });
    Ok(())
}
