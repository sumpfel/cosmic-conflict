//! In-game HUD: resources, planet list, selection panel with build / train commands, notices,
//! box selection and the end-of-match screen.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use cc_core::content::{ContentDb, Cost};
use cc_core::Res as Kind;
use cc_protocol::{GameCommand, NO_PLANET};
use egui::{Align2, Color32, RichText};

use super::{ACCENT, BAD, DIM, GOOD, panel_frame, player_color32, res_color};
use crate::audio::{Sfx, Sound};
use crate::game::camera::OrbitCam;
use crate::game::input::{Drag, Placement, Selection, my_builders};
use crate::game::world::ClientWorld;
use crate::net::{HostedServer, MatchRes, Net, Notices};
use crate::{AppState, Content, MenuScreen};

#[derive(Resource, Default)]
pub struct HudState {
    menu_open: bool,
    seen_notices: usize,
}

fn cost_text(ui: &mut egui::Ui, cost: &Cost, stock: [f32; 3]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for r in Kind::ALL {
            let v = cost.get(r);
            if v > 0.0 {
                let c = if stock[r.idx()] >= v { res_color(r) } else { BAD };
                ui.label(RichText::new(format!("{} {}", &r.label()[..1], v as u32)).small().color(c));
            }
        }
    });
}

fn affordable(cost: &Cost, stock: [f32; 3]) -> bool {
    Kind::ALL.iter().all(|r| stock[r.idx()] >= cost.get(*r))
}

fn bar(ui: &mut egui::Ui, frac: f32, color: Color32, width: f32, text: String) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 16.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, Color32::from_rgb(20, 26, 38));
    let mut fill = rect;
    fill.set_width(rect.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(fill, 3.0, color);
    p.text(rect.center(), Align2::CENTER_CENTER, text, egui::FontId::proportional(12.0), Color32::WHITE);
}

#[allow(clippy::too_many_arguments)]
pub fn hud_ui(
    mut contexts: EguiContexts,
    world: Option<Res<ClientWorld>>,
    content: Res<Content>,
    (mut net, mut hosted): (ResMut<Net>, ResMut<HostedServer>),
    mres: Res<MatchRes>,
    mut notices: ResMut<Notices>,
    mut selection: ResMut<Selection>,
    mut placement: ResMut<Placement>,
    drag: Res<Drag>,
    mut orbit: Option<ResMut<OrbitCam>>,
    mut hud: ResMut<HudState>,
    mut sound: ResMut<Sound>,
    time: Res<Time>,
    mut next: ResMut<NextState<AppState>>,
    mut screen: ResMut<MenuScreen>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let Some(world) = world else { return Ok(()) };
    let content: &ContentDb = &content.0;
    let stock = world.my_stats().map(|s| s.stock).unwrap_or([0.0; 3]);

    // Sounds for new notices.
    if notices.0.len() != hud.seen_notices {
        if let Some((last, _)) = notices.0.last() {
            if last.ends_with("complete.") || last.contains("founded") {
                sound.ui(Sfx::BuildingDone);
            } else if last.ends_with("ready.") {
                sound.ui(Sfx::UnitReady);
            } else if last.ends_with("destroyed.") {
                sound.ui(Sfx::Explosion);
            } else {
                sound.ui(Sfx::Error);
            }
        }
        hud.seen_notices = notices.0.len();
    }
    for n in notices.0.iter_mut() {
        n.1 -= time.delta_secs();
    }
    let before = notices.0.len();
    notices.0.retain(|n| n.1 > 0.0);
    if notices.0.len() != before {
        hud.seen_notices = notices.0.len();
    }

    // --- Top bar ---------------------------------------------------------------------------
    let screen_w = ctx.content_rect().width();
    egui::Area::new("top".into()).anchor(Align2::LEFT_TOP, egui::Vec2::ZERO).show(&ctx, |ui| {
        panel_frame().corner_radius(0).inner_margin(egui::Margin::symmetric(12, 6)).show(ui, |ui| {
        ui.set_width(screen_w - 24.0);
        ui.horizontal(|ui| {
            if let Some(s) = world.my_stats() {
                for r in Kind::ALL {
                    ui.label(RichText::new(r.label()).color(res_color(r)).strong());
                    ui.label(RichText::new(format!("{:.0}", s.stock[r.idx()])).size(17.0));
                    ui.label(RichText::new(format!("+{:.1}/s", s.income[r.idx()])).small().color(DIM));
                    ui.add_space(14.0);
                }
                let pc = if s.pop >= s.pop_cap { BAD } else { Color32::WHITE };
                ui.label(RichText::new("Population").strong());
                ui.label(RichText::new(format!("{}/{}", s.pop, s.pop_cap)).size(17.0).color(pc));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Menu").clicked() {
                    hud.menu_open = !hud.menu_open;
                }
                ui.label(RichText::new(format!("{:.0} ms", net.ping_ms)).small().color(DIM));
                if let Some(o) = orbit.as_ref() {
                    let name = o.focus.and_then(|f| world.system.planet(f)).map(|p| p.name.as_str()).unwrap_or("System view");
                    ui.label(RichText::new(name).color(ACCENT).strong());
                }
            });
        });
        });
    });

    // --- Planet list -----------------------------------------------------------------------
    egui::Area::new("planets".into()).anchor(Align2::LEFT_TOP, egui::vec2(10.0, 50.0)).show(&ctx, |ui| {
        panel_frame().show(ui, |ui| {
            ui.set_width(190.0);
            ui.label(RichText::new("PLANETS").small().color(DIM));
            if ui.selectable_label(orbit.as_ref().is_some_and(|o| o.focus.is_none()), "☉ System view").clicked()
                && let Some(o) = orbit.as_mut()
            {
                o.system_view();
            }
            for p in &world.system.planets {
                let focused = orbit.as_ref().is_some_and(|o| o.focus == Some(p.id));
                ui.horizontal(|ui| {
                    // Colour dots of everyone with buildings there.
                    let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, 14.0), egui::Sense::hover());
                    let mut x = r.left() + 5.0;
                    for pl in &world.players {
                        if world.presence(pl.id, p.id) {
                            ui.painter().circle_filled(egui::pos2(x, r.center().y), 4.5, player_color32(pl.color));
                            x += 9.0;
                        }
                    }
                    let mut text = RichText::new(&p.name);
                    if p.home.is_some() {
                        text = text.strong();
                    }
                    if ui.selectable_label(focused, text).on_hover_text(format!("{} · radius {:.0} m", p.biome.label(), p.radius)).clicked()
                        && let Some(o) = orbit.as_mut()
                    {
                        o.focus_planet(&world.system, p.id);
                    }
                });
            }
        });
    });

    // --- Selection panel -------------------------------------------------------------------
    let sel: Vec<_> = selection.0.clone();
    egui::Area::new("selection".into()).anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0)).show(&ctx, |ui| {
        panel_frame().show(ui, |ui| {
            ui.set_width(620.0);
            ui.set_min_height(120.0);
            if let Some(kind) = placement.kind {
                let def = content.building(kind);
                ui.label(RichText::new(format!("Placing {}", def.name)).strong().color(ACCENT));
                ui.label("Left click on the planet to build · Shift keeps placing · Right click / Esc cancels");
                if let Some(p) = &placement.problem {
                    ui.label(RichText::new(p).color(BAD));
                }
                return;
            }
            if sel.is_empty() {
                ui.label(RichText::new("Nothing selected").color(DIM));
                ui.label(RichText::new(
                    "Left click / drag: select · Right click: move, attack, help build · Double click a unit: all of its kind\n\
                     Middle drag / WASD: rotate · Wheel: zoom · Tab: next planet · H: home · Space: system · F: focus selection",
                ).small().color(DIM));
                return;
            }
            // A single building.
            if let [id] = sel.as_slice()
                && let Some(b) = world.buildings.get(id)
            {
                let def = content.building(b.snap.kind);
                let mine = b.snap.owner == world.me;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&def.name).size(19.0).strong());
                    if !mine {
                        ui.label(RichText::new(world.name_of(b.snap.owner)).color(player_color32(world.color_of(b.snap.owner))));
                    }
                    let planet = &world.system.planets[b.snap.planet as usize].name;
                    ui.label(RichText::new(format!("on {planet}")).color(DIM));
                });
                if b.snap.progress < 1.0 {
                    bar(ui, b.snap.progress, Color32::from_rgb(220, 180, 40), 300.0, format!("Under construction {:.0}%", b.snap.progress * 100.0));
                } else {
                    bar(ui, b.snap.hp / def.hp, Color32::from_rgb(60, 180, 90), 300.0, format!("{:.0} / {:.0}", b.snap.hp, def.hp));
                }
                ui.label(RichText::new(&def.description).small().color(DIM));
                if let Some(d) = world.system.deposits.iter().find(|d| def.extracts && d.planet == b.snap.planet && Vec3::from_array(b.snap.dir).dot(d.dir) > 0.99995) {
                    ui.label(RichText::new(format!("{} left: {:.0}", d.kind.label(), world.deposits[d.id as usize])).color(res_color(d.kind)));
                }
                if mine && b.snap.progress >= 1.0 {
                    let trains = &content.building_trains[b.snap.kind as usize];
                    if !trains.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for k in trains {
                                let u = content.unit(*k);
                                ui.vertical(|ui| {
                                    let can = affordable(&u.cost, stock) && b.snap.queue.len() < 5;
                                    let btn = ui.add_enabled(can, egui::Button::new(&u.name).min_size(egui::vec2(110.0, 28.0)));
                                    if btn.on_hover_text(&u.description).on_disabled_hover_text(&u.description).clicked() {
                                        net.command(GameCommand::Train { building: b.snap.id, kind: *k });
                                    }
                                    cost_text(ui, &u.cost, stock);
                                });
                            }
                        });
                    }
                    if !b.snap.queue.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Queue:");
                            for (i, k) in b.snap.queue.iter().enumerate() {
                                let name = &content.unit(*k).name;
                                if i == 0 {
                                    bar(ui, b.snap.train_progress, ACCENT.gamma_multiply(0.7), 140.0, name.clone());
                                } else {
                                    ui.label(RichText::new(name).small());
                                }
                            }
                            if ui.small_button("cancel last").clicked() {
                                net.command(GameCommand::CancelTrain { building: b.snap.id });
                            }
                        });
                    }
                }
                if mine {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.small_button(RichText::new("Demolish").color(BAD)).clicked() {
                            net.command(GameCommand::Demolish { building: b.snap.id });
                            selection.0.clear();
                        }
                    });
                }
                return;
            }
            // Units.
            let units: Vec<_> = sel.iter().filter_map(|id| world.units.get(id)).collect();
            if units.is_empty() {
                return;
            }
            let mut groups: BTreeMap<u16, (usize, f32, f32)> = BTreeMap::new();
            for u in &units {
                let def = content.unit(u.snap.kind);
                let g = groups.entry(u.snap.kind).or_default();
                g.0 += 1;
                g.1 += u.snap.hp;
                g.2 += def.hp;
            }
            let first = units[0];
            let mine = first.snap.owner == world.me;
            if units.len() == 1 {
                let def = content.unit(first.snap.kind);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&def.name).size(19.0).strong());
                    if !mine {
                        ui.label(RichText::new(world.name_of(first.snap.owner)).color(player_color32(world.color_of(first.snap.owner))));
                    }
                    let place = if first.snap.planet == NO_PLANET {
                        "in space".to_string()
                    } else {
                        format!("on {}", world.system.planets[first.snap.planet as usize].name)
                    };
                    ui.label(RichText::new(place).color(DIM));
                    ui.label(RichText::new(format!("{:?}", first.snap.activity)).small().color(DIM));
                });
                bar(ui, first.snap.hp / def.hp, Color32::from_rgb(60, 180, 90), 300.0, format!("{:.0} / {:.0}", first.snap.hp, def.hp));
                ui.label(RichText::new(&def.description).small().color(DIM));
                if let Some(w) = &def.weapon {
                    ui.label(RichText::new(format!(
                        "Weapon: {:.0} damage every {:.1} s, range {:.0} m — hits {}",
                        w.damage,
                        w.cooldown,
                        w.range,
                        match (w.ground, w.space) {
                            (true, true) => "ground and ships",
                            (true, false) => "ground",
                            _ => "ships",
                        }
                    )).small());
                }
            } else {
                ui.horizontal_wrapped(|ui| {
                    for (k, (n, hp, max)) in &groups {
                        ui.label(RichText::new(format!("{} × {}", n, content.unit(*k).name)).strong());
                        ui.label(RichText::new(format!("{:.0}%", hp / max * 100.0)).small().color(DIM));
                        ui.add_space(10.0);
                    }
                });
            }
            if !mine {
                return;
            }
            // Build menu of the selected builders.
            let builders = my_builders(&world, content, &selection, None);
            if !builders.is_empty() {
                let mut kinds: Vec<u16> = Vec::new();
                for id in &builders {
                    for k in &content.unit_builds[world.units[id].snap.kind as usize] {
                        if !kinds.contains(k) {
                            kinds.push(*k);
                        }
                    }
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    for k in kinds {
                        let b = content.building(k);
                        let missing = content.building_requires[k as usize]
                            .iter()
                            .find(|r| !world.buildings.values().any(|x| x.snap.owner == world.me && x.snap.kind == **r && x.snap.progress >= 1.0));
                        ui.vertical(|ui| {
                            let can = affordable(&b.cost, stock) && missing.is_none();
                            let tip = match missing {
                                Some(r) => format!("{}\nRequires a finished {}.", b.description, content.building(*r).name),
                                None => b.description.clone(),
                            };
                            let btn = ui.add_enabled(can, egui::Button::new(&b.name).min_size(egui::vec2(110.0, 28.0)));
                            if btn.on_hover_text(&tip).on_disabled_hover_text(&tip).clicked() {
                                placement.kind = Some(k);
                            }
                            cost_text(ui, &b.cost, stock);
                        });
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui.small_button("Stop").clicked() {
                    net.command(GameCommand::Stop { units: sel.clone() });
                }
                if first.snap.planet != NO_PLANET
                    && let Some(o) = orbit.as_mut()
                    && ui.small_button("Focus (F)").clicked()
                {
                    o.focus_planet(&world.system, first.snap.planet);
                    o.look_at_dir(first.up);
                }
            });
        });
    });

    // --- Notices ---------------------------------------------------------------------------
    egui::Area::new("notices".into()).anchor(Align2::RIGHT_BOTTOM, egui::vec2(-14.0, -14.0)).show(&ctx, |ui| {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            for (text, life) in notices.0.iter().rev() {
                let a = (life / 1.0).clamp(0.0, 1.0);
                ui.label(RichText::new(text).size(15.0).color(Color32::WHITE.gamma_multiply(a)).background_color(Color32::from_black_alpha((150.0 * a) as u8)));
            }
        });
    });

    // --- Box selection ---------------------------------------------------------------------
    if drag.is_box()
        && let Some(start) = drag.start
    {
        let rect = egui::Rect::from_two_pos(egui::pos2(start.x, start.y), egui::pos2(drag.current.x, drag.current.y));
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, "box".into()));
        p.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(90, 200, 255, 25));
        p.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, ACCENT), egui::StrokeKind::Inside);
    }

    // --- Menu & end of match ---------------------------------------------------------------
    let defeated = world.my_stats().is_some_and(|s| s.defeated);
    if let Some(winner) = mres.game_over {
        egui::Window::new("match_over").title_bar(false).resizable(false).anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO).show(&ctx, |ui| {
            let (text, color) = match winner {
                Some(w) if w == world.me => ("VICTORY".to_string(), GOOD),
                Some(w) => (format!("{} rules the system", world.name_of(w)), BAD),
                None => ("The match is over".to_string(), DIM),
            };
            ui.label(RichText::new(text).size(34.0).strong().color(color));
            ui.add_space(10.0);
            if ui.button("Back to the lobby").clicked() {
                *screen = MenuScreen::Lobby;
                next.set(AppState::Menu);
            }
        });
    } else if defeated || hud.menu_open {
        egui::Window::new("game_menu").title_bar(false).resizable(false).anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO).show(&ctx, |ui| {
            ui.set_width(240.0);
            if defeated {
                ui.label(RichText::new("DEFEATED").size(30.0).strong().color(BAD));
                ui.label("You can keep watching or leave.");
            } else {
                ui.heading("Menu");
                if ui.button("Resume").clicked() {
                    hud.menu_open = false;
                }
                if ui.button(RichText::new("Surrender").color(BAD)).clicked() {
                    net.command(GameCommand::Surrender);
                    hud.menu_open = false;
                }
            }
            if ui.button("Leave match").clicked() {
                hud.menu_open = false;
                // Leaving counts as surrender on the server; a host also closes its server.
                net.disconnect();
                hosted.stop();
                next.set(AppState::Menu);
                *screen = MenuScreen::Main;
            }
        });
    }
    Ok(())
}

/// Planet names (with the colours of everyone who has built there) floating over the globes.
pub fn planet_labels(
    mut contexts: EguiContexts,
    world: Option<Res<ClientWorld>>,
    orbit: Option<Res<OrbitCam>>,
    cam_q: Query<(&Camera, &GlobalTransform), With<crate::game::scene::MainCamera>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let (Some(world), Ok((cam, cam_t))) = (world, cam_q.single()) else { return Ok(()) };
    let painter = ctx.layer_painter(egui::LayerId::background());
    for p in &world.system.planets {
        let focused = orbit.as_ref().is_some_and(|o| o.focus == Some(p.id) && o.dist < p.radius * 4.0);
        if focused {
            continue;
        }
        let Ok(pos) = cam.world_to_viewport(cam_t, p.center + Vec3::Y * (p.radius + 6.0)) else { continue };
        let owners: Vec<u8> = world.players.iter().filter(|pl| world.presence(pl.id, p.id)).map(|pl| pl.color).collect();
        let color = owners.first().map(|c| player_color32(*c)).unwrap_or(DIM);
        let at = egui::pos2(pos.x, pos.y - 8.0);
        painter.text(at + egui::vec2(1.0, 1.0), Align2::CENTER_BOTTOM, &p.name, egui::FontId::proportional(14.0), Color32::BLACK);
        painter.text(at, Align2::CENTER_BOTTOM, &p.name, egui::FontId::proportional(14.0), color);
    }
    Ok(())
}
