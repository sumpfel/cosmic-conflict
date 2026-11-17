//! In-game HUD: resources, planet list, selection panel with build / train / research commands
//! and unit stats, the research tree window, notices, box selection and the end-of-match screen.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use cc_core::content::{ArmorClass, Aura, BuildGroup, ContentDb, Cost, Sensors, Weapon};
use cc_core::research::{self, TechField, TechKind};
use cc_core::Res as Kind;
use cc_protocol::{ClientMsg, EntityId, GameCommand, LobbyCmd, MAX_RESEARCH_QUEUE, NO_PLANET};
use egui::{Align2, Color32, RichText};

use super::settings_window::SettingsWindow;
use super::{ACCENT, BAD, DIM, GOOD, panel_frame, player_color32, res_color};
use crate::audio::{Sfx, Sound};
use crate::game::camera::OrbitCam;
use crate::game::input::{Drag, Placement, SENSOR_COLORS, Selection, my_builders, sensor_ranges};
use crate::game::world::ClientWorld;
use crate::net::{ChatLine, ChatLog, HostedServer, MatchRes, Net, Notices};
use crate::{AppState, Content, MenuScreen};

#[derive(Resource, Default)]
pub struct HudState {
    menu_open: bool,
    research_open: bool,
    seen_notices: usize,
    chat_open: bool,
    /// Give the chat input keyboard focus on the next frame.
    chat_focus: bool,
    seen_chat: u64,
    started: bool,
    dev_selected: bool,
}

/// Seconds a chat line stays visible while the chat is closed.
const CHAT_FADE: f64 = 10.0;
const CHAT_WIDTH: f32 = 300.0;
const CHAT_RECENT: usize = 6;

fn chat_line(ui: &mut egui::Ui, world: &ClientWorld, line: &ChatLine, alpha: f32, backdrop: bool) {
    use egui::text::{LayoutJob, TextFormat};
    let font = egui::FontId::proportional(14.0);
    let background = if backdrop { Color32::from_black_alpha((150.0 * alpha) as u8) } else { Color32::TRANSPARENT };
    let fmt = |color: Color32, italics: bool| TextFormat { font_id: font.clone(), color: color.gamma_multiply(alpha), background, italics, ..Default::default() };
    let mut job = LayoutJob::default();
    match line.from {
        Some(f) => {
            job.append(&format!("{}: ", world.name_of(f)), 0.0, fmt(player_color32(world.color_of(f)), false));
            job.append(&line.text, 0.0, fmt(Color32::WHITE, false));
        }
        None => job.append(&line.text, 0.0, fmt(DIM, true)),
    }
    ui.label(job);
}

/// Bottom-left chat. Closed: the last few lines fade out and clicks pass through.
/// Enter opens it with the full history and an input; Enter sends, Esc / click outside closes
/// (an unsent draft is kept).
fn chat_ui(ctx: &egui::Context, world: &ClientWorld, chat: &mut ChatLog, hud: &mut HudState, net: &Net, sound: &mut Sound, now: f64) {
    if chat.received != hud.seen_chat {
        if chat.lines.last().is_some_and(|l| l.from.is_some_and(|f| f != world.me)) {
            sound.ui(Sfx::Click);
        }
        hud.seen_chat = chat.received;
    }
    // `egui_wants_keyboard_input` is still last frame's state: no other text field has focus.
    if !hud.chat_open && !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        hud.chat_open = true;
        hud.chat_focus = true;
    }
    let open = hud.chat_open;
    egui::Area::new("chat".into()).anchor(Align2::LEFT_BOTTOM, egui::vec2(10.0, -10.0)).interactable(open).show(ctx, |ui| {
        if !open {
            ui.set_width(CHAT_WIDTH);
            let start = chat.lines.len().saturating_sub(CHAT_RECENT);
            for line in chat.lines[start..].iter().filter(|l| now - l.at < CHAT_FADE) {
                let alpha = (CHAT_FADE - (now - line.at)).clamp(0.0, 1.0) as f32;
                chat_line(ui, world, line, alpha, true);
            }
            return;
        }
        panel_frame().show(ui, |ui| {
            ui.set_width(CHAT_WIDTH);
            egui::ScrollArea::vertical().max_height(220.0).stick_to_bottom(true).show(ui, |ui| {
                for line in &chat.lines {
                    chat_line(ui, world, line, 1.0, false);
                }
            });
            let resp = ui.add(egui::TextEdit::singleline(&mut chat.input).hint_text("Message to everyone …").desired_width(f32::INFINITY));
            if hud.chat_focus {
                resp.request_focus();
                hud.chat_focus = false;
            } else if resp.lost_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !chat.input.trim().is_empty() {
                    net.send(ClientMsg::Lobby(LobbyCmd::Chat(std::mem::take(&mut chat.input))));
                }
                hud.chat_open = false;
            }
            ui.label(RichText::new("Enter: send · Esc: close").small().color(DIM));
        });
    });
}

/// Where a tech stands for me.
#[derive(Clone, Debug, PartialEq)]
enum TechState {
    Done,
    /// Running in a lab (progress 0..1).
    Running(f32),
    Queued,
    /// Can be queued in this lab.
    Available(EntityId),
    /// Why not (missing prerequisite, no lab, full queues).
    Locked(String),
}

/// The state of `tech` and, if it can be started, the best lab for it (`prefer`: a selected lab).
fn tech_state(world: &ClientWorld, content: &ContentDb, tech: TechKind, prefer: Option<EntityId>) -> TechState {
    let mut done = vec![false; content.techs.len()];
    for t in world.my_techs() {
        if let Some(d) = done.get_mut(*t as usize) {
            *d = true;
        }
    }
    if done[tech as usize] {
        return TechState::Done;
    }
    let labs: Vec<&crate::game::world::BuildingView> = world
        .buildings
        .values()
        .filter(|b| b.snap.owner == world.me && b.snap.progress >= 1.0 && content.building(b.snap.kind).research)
        .collect();
    for b in &labs {
        if b.snap.research.first() == Some(&tech) {
            return TechState::Running(b.snap.research_progress);
        }
        if b.snap.research.contains(&tech) {
            return TechState::Queued;
        }
    }
    if labs.is_empty() {
        return TechState::Locked("Build a Research Lab first.".into());
    }
    // A lab whose queue (with the finished research) satisfies the prerequisites.
    let mut best: Option<(&crate::game::world::BuildingView, usize)> = None;
    let mut why = String::new();
    for b in &labs {
        if b.snap.research.len() >= MAX_RESEARCH_QUEUE {
            why = "All research queues are full.".into();
            continue;
        }
        match research::blocked(content, tech, &done, &b.snap.research) {
            None => {
                let score = b.snap.research.len() * 2 + usize::from(Some(b.snap.id) != prefer);
                if best.is_none_or(|(_, s)| score < s) {
                    best = Some((b, score));
                }
            }
            Some(w) => why = w,
        }
    }
    match best {
        Some((b, _)) => TechState::Available(b.snap.id),
        None => TechState::Locked(why),
    }
}

fn tech_tooltip(content: &ContentDb, tech: TechKind) -> String {
    let t = content.tech(tech);
    let mut s = format!("{} (tier {}, {:.0} s)\n{}", t.name, t.tier, t.time, t.description);
    for e in &t.effects {
        s.push_str(&format!("\n• {}", e.describe()));
    }
    let (units, buildings) = content.unlocked_by(tech);
    let names: Vec<&str> = units.iter().map(|u| content.unit(*u).name.as_str()).chain(buildings.iter().map(|b| content.building(*b).name.as_str())).collect();
    if !names.is_empty() {
        s.push_str(&format!("\nUnlocks: {}", names.join(", ")));
    }
    let req: Vec<&str> = content.tech_requires[tech as usize].iter().map(|r| content.tech(*r).name.as_str()).collect();
    if !req.is_empty() {
        s.push_str(&format!("\nRequires: {}", req.join(", ")));
    }
    s
}

fn armor_text(class: ArmorClass, armor: f32, shield: f32, regen: f32) -> String {
    let mut s = format!("Armour: {} {:.0}", class.label(), armor);
    if shield > 0.0 {
        s.push_str(&format!(" · Shield {:.0} (+{:.0}/s)", shield, regen));
    }
    s
}

fn weapon_text(w: &Weapon) -> String {
    let hits = match (w.ground, w.space) {
        (true, true) => "ground and ships",
        (true, false) => "ground",
        _ => "ships",
    };
    let mut s = format!("{} weapon: {:.0} damage every {:.1} s, range {:.0} m", w.kind.label(), w.damage, w.cooldown, w.range);
    if w.splash > 0.0 {
        s.push_str(&format!(", splash {:.1} m", w.splash));
    }
    s.push_str(&format!(" — hits {hits}"));
    s
}

fn sensor_line(ui: &mut egui::Ui, s: &Sensors) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new("Sensors:").small());
        for ((name, c), range) in SENSOR_COLORS.iter().zip(sensor_ranges(s)) {
            if range > 0.0 {
                let col = Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8);
                ui.label(RichText::new(format!("{name} {range:.0} m")).small().color(col));
            }
        }
    });
}

fn aura_text(a: &Aura) -> String {
    let mut parts = Vec::new();
    if a.shield > 0.0 {
        parts.push(format!("+{:.0} shields (+{:.0}/s)", a.shield, a.shield_regen));
    }
    if a.repair > 0.0 {
        parts.push(format!("repairs {:.0} hp/s", a.repair));
    }
    format!("Field: {} for own units and buildings within {:.0} m", parts.join(", "), a.radius)
}

/// Why I cannot use something that needs these techs (first missing one).
fn missing_tech(world: &ClientWorld, content: &ContentDb, techs: &[TechKind]) -> Option<String> {
    techs.iter().find(|t| !world.has_tech(**t)).map(|t| format!("Requires research: {}.", content.tech(*t).name))
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

// ------------------------------------------------------------------------------ research tree

const RESEARCHING: Color32 = Color32::from_rgb(200, 150, 255);
const NODE_W: f32 = 178.0;
const NODE_H: f32 = 40.0;
/// Horizontal distance between tiers (the gap holds the connectors).
const TIER_W: f32 = 240.0;
const ROW_H: f32 = 50.0;
const LANE_GAP: f32 = 16.0;
const LANE_LABEL_W: f32 = 96.0;
const HEADER_H: f32 = 26.0;

/// Node positions of the research tree: tiers from left to right, one lane per field; inside a
/// lane each tech sits near the row of its prerequisites so the paths stay short and straight.
struct TreeLayout {
    /// Top-left corner of every tech's node (relative to the tree's origin).
    pos: Vec<egui::Pos2>,
    /// (field, top, bottom) of every lane.
    lanes: Vec<(TechField, f32, f32)>,
    tiers: u8,
    size: egui::Vec2,
}

fn tree_layout(content: &ContentDb) -> TreeLayout {
    let n = content.techs.len();
    let tiers = content.techs.iter().map(|t| t.tier).max().unwrap_or(1);
    let mut row = vec![0.0f32; n];
    let mut pos = vec![egui::Pos2::ZERO; n];
    let mut lanes = Vec::new();
    let mut y = HEADER_H;
    for field in TechField::ALL {
        let mut rows = 0usize;
        let mut placed: Vec<TechKind> = Vec::new();
        for tier in 1..=tiers {
            // Desired row: the average row of the prerequisites in this lane.
            let mut col: Vec<(f32, TechKind)> = (0..n as TechKind)
                .filter(|t| content.tech(*t).field == field && content.tech(*t).tier == tier)
                .map(|t| {
                    let same: Vec<f32> = content.tech_requires[t as usize].iter().filter(|p| placed.contains(p)).map(|p| row[*p as usize]).collect();
                    let want = if same.is_empty() { -1.0 } else { same.iter().sum::<f32>() / same.len() as f32 };
                    (want, t)
                })
                .collect();
            col.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let mut next = 0.0f32;
            for (want, t) in col {
                let r = want.round().max(next);
                row[t as usize] = r;
                next = r + 1.0;
                placed.push(t);
            }
            rows = rows.max(next as usize);
        }
        if rows == 0 {
            continue;
        }
        for t in &placed {
            let tier = content.tech(*t).tier;
            pos[*t as usize] = egui::pos2(LANE_LABEL_W + (tier - 1) as f32 * TIER_W, y + row[*t as usize] * ROW_H + (ROW_H - NODE_H) / 2.0);
        }
        let bottom = y + rows as f32 * ROW_H;
        lanes.push((field, y, bottom));
        y = bottom + LANE_GAP;
    }
    let size = egui::vec2(LANE_LABEL_W + (tiers - 1) as f32 * TIER_W + NODE_W + 12.0, y);
    TreeLayout { pos, lanes, tiers, size }
}

/// Paints text segments one after another; returns the end x.
fn paint_segments(p: &egui::Painter, mut at: egui::Pos2, parts: &[(String, Color32)], size: f32) -> f32 {
    for (text, color) in parts {
        let g = p.layout_no_wrap(text.clone(), egui::FontId::proportional(size), *color);
        let w = g.size().x;
        p.galley(at, g, *color);
        at.x += w + 5.0;
    }
    at.x
}

/// Draws the tree; returns a click on a tech: (lab, tech, affordable).
fn research_tree(
    ui: &mut egui::Ui,
    tree: &TreeLayout,
    world: &ClientWorld,
    content: &ContentDb,
    stock: [f32; 3],
    selected_lab: Option<EntityId>,
) -> Option<(EntityId, TechKind, bool)> {
    let (rect, _) = ui.allocate_exact_size(tree.size, egui::Sense::hover());
    let o = rect.min.to_vec2();
    let n = content.techs.len();
    let states: Vec<TechState> = (0..n as TechKind).map(|t| tech_state(world, content, t, selected_lab)).collect();
    let node = |t: usize| egui::Rect::from_min_size(tree.pos[t] + o, egui::vec2(NODE_W, NODE_H));
    let hovered = ui.input(|i| i.pointer.hover_pos()).and_then(|p| (0..n).find(|t| node(*t).contains(p)));
    // The hovered tech and everything it needs.
    let mut path = vec![false; n];
    if let Some(h) = hovered {
        let mut stack = vec![h];
        while let Some(t) = stack.pop() {
            if !path[t] {
                path[t] = true;
                stack.extend(content.tech_requires[t].iter().map(|r| *r as usize));
            }
        }
    }
    let p = ui.painter().clone();

    // Lanes and tier headers.
    for (i, (field, top, bottom)) in tree.lanes.iter().enumerate() {
        let band = egui::Rect::from_min_max(egui::pos2(rect.left(), top + o.y - 4.0), egui::pos2(rect.right(), bottom + o.y + 4.0));
        p.rect_filled(band, 6.0, if i % 2 == 0 { Color32::from_rgba_unmultiplied(255, 255, 255, 6) } else { Color32::from_rgba_unmultiplied(0, 0, 0, 30) });
        p.text(egui::pos2(rect.left() + 8.0, (top + bottom) / 2.0 + o.y), Align2::LEFT_CENTER, field.label().to_uppercase(), egui::FontId::proportional(13.0), ACCENT);
    }
    for tier in 1..=tree.tiers {
        let x = rect.left() + LANE_LABEL_W + (tier - 1) as f32 * TIER_W + NODE_W / 2.0;
        let label = ["I", "II", "III", "IV", "V", "VI"].get(tier as usize - 1).copied().unwrap_or("?");
        p.text(egui::pos2(x, rect.top() + 4.0), Align2::CENTER_TOP, format!("TIER {label}"), egui::FontId::proportional(12.0), DIM);
    }

    // Paths: prerequisite (right edge) → tech (left edge).
    let mut highlighted = Vec::new();
    for t in 0..n {
        for r in &content.tech_requires[t] {
            let r = *r as usize;
            let a = node(r).right_center();
            let b = node(t).left_center();
            let dx = ((b.x - a.x) * 0.5).max(30.0);
            let pts = [a, a + egui::vec2(dx, 0.0), b - egui::vec2(dx, 0.0), b];
            if path[t] && path[r] {
                highlighted.push(pts);
                continue;
            }
            let color = match (&states[r], &states[t]) {
                (TechState::Done, TechState::Done) => Color32::from_rgb(60, 130, 80),
                (TechState::Done, _) => Color32::from_rgb(130, 150, 185),
                _ => Color32::from_rgb(62, 68, 82),
            };
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(pts, false, Color32::TRANSPARENT, egui::Stroke::new(1.6, color)));
            p.circle_filled(b, 3.0, color);
        }
    }
    for pts in highlighted {
        p.add(egui::epaint::CubicBezierShape::from_points_stroke(pts, false, Color32::TRANSPARENT, egui::Stroke::new(3.0, ACCENT)));
        p.circle_filled(pts[3], 4.0, ACCENT);
    }

    // Nodes.
    let mut clicked = None;
    for t in 0..n {
        let r = node(t);
        let def = content.tech(t as TechKind);
        let (fill, stroke, text) = match &states[t] {
            TechState::Done => (Color32::from_rgb(22, 50, 34), GOOD, Color32::from_rgb(200, 240, 210)),
            TechState::Running(_) => (Color32::from_rgb(40, 28, 62), RESEARCHING, Color32::WHITE),
            TechState::Queued => (Color32::from_rgb(32, 26, 46), RESEARCHING.gamma_multiply(0.6), Color32::WHITE),
            TechState::Available(_) => (Color32::from_rgb(22, 34, 54), ACCENT, Color32::WHITE),
            TechState::Locked(_) => (Color32::from_rgb(20, 22, 28), Color32::from_rgb(58, 62, 72), DIM),
        };
        let width = if path[t] || hovered == Some(t) { 2.5 } else { 1.2 };
        p.rect_filled(r, 6.0, fill);
        p.rect_stroke(r, 6.0, egui::Stroke::new(width, if path[t] { ACCENT } else { stroke }), egui::StrokeKind::Inside);
        let galley = p.layout(def.name.clone(), egui::FontId::proportional(13.5), text, NODE_W - 12.0);
        let name_h = galley.size().y.min(18.0);
        p.galley(r.left_top() + egui::vec2(7.0, 4.0), galley, text);
        let line = egui::pos2(r.left() + 7.0, r.top() + 6.0 + name_h);
        match &states[t] {
            TechState::Done => {
                paint_segments(&p, line, &[("✔ researched".into(), GOOD)], 11.0);
            }
            TechState::Running(f) => {
                let bar = egui::Rect::from_min_size(egui::pos2(r.left() + 6.0, r.bottom() - 7.0), egui::vec2((NODE_W - 12.0) * f.clamp(0.0, 1.0), 3.0));
                p.rect_filled(bar, 1.5, RESEARCHING);
                let left = def.time * (1.0 - f);
                paint_segments(&p, line, &[(format!("researching {:.0}% · {:.0} s", f * 100.0, left), RESEARCHING)], 11.0);
            }
            TechState::Queued => {
                paint_segments(&p, line, &[("queued".into(), RESEARCHING)], 11.0);
            }
            TechState::Available(_) | TechState::Locked(_) => {
                let mut parts: Vec<(String, Color32)> = Kind::ALL
                    .iter()
                    .filter(|k| def.cost.get(**k) > 0.0)
                    .map(|k| {
                        let v = def.cost.get(*k);
                        (format!("{}{}", &k.label()[..1], v as u32), if stock[k.idx()] >= v { res_color(*k) } else { BAD })
                    })
                    .collect();
                parts.push((format!("{:.0}s", def.time), DIM));
                paint_segments(&p, line, &parts, 11.0);
            }
        }
        // Unlocks: a small marker per unit / building at the right.
        let (u, b) = content.unlocked_by(t as TechKind);
        if !u.is_empty() || !b.is_empty() {
            p.text(r.right_top() + egui::vec2(-6.0, 4.0), Align2::RIGHT_TOP, format!("+{}", u.len() + b.len()), egui::FontId::proportional(11.0), Color32::from_rgb(140, 200, 255));
        }
        let resp = ui.interact(r, ui.id().with(("tech", t)), egui::Sense::click());
        let mut tip = tech_tooltip(content, t as TechKind);
        if let TechState::Locked(why) = &states[t] {
            tip.push_str(&format!("\n\n{why}"));
        }
        let resp = resp.on_hover_text(tip);
        if let TechState::Available(lab) = states[t]
            && resp.clicked()
        {
            clicked = Some((lab, t as TechKind, affordable(&def.cost, stock)));
        }
    }
    clicked
}

#[allow(clippy::too_many_arguments)]
pub fn hud_ui(
    mut contexts: EguiContexts,
    world: Option<Res<ClientWorld>>,
    content: Res<Content>,
    (mut net, mut hosted, mut chat): (ResMut<Net>, ResMut<HostedServer>, ResMut<ChatLog>),
    mres: Res<MatchRes>,
    mut notices: ResMut<Notices>,
    mut selection: ResMut<Selection>,
    mut placement: ResMut<Placement>,
    drag: Res<Drag>,
    mut orbit: Option<ResMut<OrbitCam>>,
    (mut hud, mut settings_window): (ResMut<HudState>, ResMut<SettingsWindow>),
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

    if !hud.started {
        hud.started = true;
        // `CC_SHOWCASE=research` (dev helper) opens the research tree for screenshots.
        hud.research_open = std::env::var("CC_SHOWCASE").as_deref() == Ok("research");
    }
    // `CC_SELECT=<unit or building id>` (dev helper) selects my first one, e.g. for screenshots.
    if !hud.dev_selected
        && let Ok(id) = std::env::var("CC_SELECT")
    {
        let unit = content.unit_kind(&id);
        let building = content.building_kind(&id);
        let found = world
            .units
            .values()
            .find(|u| u.snap.owner == world.me && Some(u.snap.kind) == unit)
            .map(|u| u.snap.id)
            .or_else(|| world.buildings.values().find(|b| b.snap.owner == world.me && Some(b.snap.kind) == building).map(|b| b.snap.id));
        if let Some(e) = found {
            selection.0 = vec![e];
            hud.dev_selected = true;
        }
    }
    if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::R)) {
        hud.research_open = !hud.research_open;
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
                let done = world.my_techs().len();
                if ui.button(format!("Research {done}/{} (R)", content.techs.len())).clicked() {
                    hud.research_open = !hud.research_open;
                }
                ui.label(RichText::new(format!("{:.0} ms", net.ping_ms)).small().color(DIM));
                if let Some(o) = orbit.as_ref() {
                    if o.is_free() {
                        ui.label(RichText::new("Free camera · C: back").small().color(DIM));
                    }
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
                     WASD: move · Q/E: turn · Middle drag: turn & tilt (Shift: drag the ground) · Wheel: zoom (Ctrl: tilt)\n\
                     T: top / tilted / low view · C: free camera · Tab: next planet · H: home · Space: system · F: focus selection · Enter: chat",
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
                let st = world.stats_of(b.snap.owner).map(|s| s.building(b.snap.kind).clone());
                let max_hp = st.as_ref().map(|s| s.hp).unwrap_or(def.hp);
                if b.snap.progress < 1.0 {
                    bar(ui, b.snap.progress, Color32::from_rgb(220, 180, 40), 300.0, format!("Under construction {:.0}%", b.snap.progress * 100.0));
                } else {
                    bar(ui, b.snap.hp / max_hp, Color32::from_rgb(60, 180, 90), 300.0, format!("{:.0} / {:.0}", b.snap.hp, max_hp));
                }
                if let Some(st) = &st
                    && (st.shield > 0.0 || b.snap.shield > 0.5)
                {
                    let cap = st.shield.max(b.snap.shield);
                    bar(ui, b.snap.shield / cap, Color32::from_rgb(70, 130, 230), 300.0, if st.shield > 0.0 { format!("Shield {:.0} / {:.0}", b.snap.shield, cap) } else { format!("Shield {:.0} (generator)", b.snap.shield) });
                }
                ui.label(RichText::new(&def.description).small().color(DIM));
                if let Some(st) = &st {
                    ui.label(RichText::new(armor_text(def.armor_class, st.armor, st.shield, st.shield_regen)).small());
                    if let Some(w) = &st.weapon {
                        ui.label(RichText::new(weapon_text(w)).small());
                    }
                    if let Some(a) = &st.aura {
                        ui.label(RichText::new(aura_text(a)).small().color(Color32::from_rgb(120, 200, 255)));
                    }
                    sensor_line(ui, &st.sensors);
                }
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
                                    let locked = missing_tech(&world, content, &content.unit_tech[*k as usize]);
                                    let can = affordable(&u.cost, stock) && b.snap.queue.len() < 5 && locked.is_none();
                                    let tip = match &locked {
                                        Some(why) => format!("{}\n{why}", u.description),
                                        None => u.description.clone(),
                                    };
                                    let label = if locked.is_some() { format!("🔒 {}", u.name) } else { u.name.clone() };
                                    let btn = ui.add_enabled(can, egui::Button::new(label).min_size(egui::vec2(110.0, 28.0)));
                                    if btn.on_hover_text(&tip).on_disabled_hover_text(&tip).clicked() {
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
                    if def.research {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("RESEARCH").small().color(DIM));
                            if ui.small_button("Open research tree (R)").clicked() {
                                hud.research_open = true;
                            }
                        });
                        if !b.snap.research.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                for (i, t) in b.snap.research.iter().enumerate() {
                                    let name = &content.tech(*t).name;
                                    if i == 0 {
                                        bar(ui, b.snap.research_progress, Color32::from_rgb(170, 110, 230), 170.0, name.clone());
                                    } else {
                                        ui.label(RichText::new(name).small());
                                    }
                                }
                                if ui.small_button("cancel last").clicked() {
                                    net.command(GameCommand::CancelResearch { building: b.snap.id });
                                }
                            });
                        }
                        // What this lab can start now, cheapest tiers first.
                        let mut avail: Vec<TechKind> = (0..content.techs.len() as TechKind)
                            .filter(|t| tech_state(&world, content, *t, Some(b.snap.id)) == TechState::Available(b.snap.id))
                            .collect();
                        avail.sort_by_key(|t| (content.tech(*t).tier, content.tech(*t).field));
                        egui::ScrollArea::vertical().max_height(130.0).show(ui, |ui| {
                            ui.set_max_width(600.0);
                            ui.horizontal_wrapped(|ui| {
                                for t in avail {
                                    let def = content.tech(t);
                                    ui.vertical(|ui| {
                                        let btn = ui.add_enabled(affordable(&def.cost, stock), egui::Button::new(RichText::new(&def.name).small()).min_size(egui::vec2(130.0, 24.0)));
                                        let tip = tech_tooltip(content, t);
                                        if btn.on_hover_text(&tip).on_disabled_hover_text(&tip).clicked() {
                                            net.command(GameCommand::Research { building: b.snap.id, tech: t });
                                        }
                                        cost_text(ui, &def.cost, stock);
                                    });
                                }
                            });
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
                let max = world.stats_of(u.snap.owner).map(|s| s.unit(u.snap.kind).hp).unwrap_or(content.unit(u.snap.kind).hp);
                let g = groups.entry(u.snap.kind).or_default();
                g.0 += 1;
                g.1 += u.snap.hp;
                g.2 += max;
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
                let st = world.stats_of(first.snap.owner).map(|s| s.unit(first.snap.kind).clone());
                let max_hp = st.as_ref().map(|s| s.hp).unwrap_or(def.hp);
                bar(ui, first.snap.hp / max_hp, Color32::from_rgb(60, 180, 90), 300.0, format!("{:.0} / {:.0}", first.snap.hp, max_hp));
                if let Some(st) = &st
                    && (st.shield > 0.0 || first.snap.shield > 0.5)
                {
                    let cap = st.shield.max(first.snap.shield);
                    bar(ui, first.snap.shield / cap, Color32::from_rgb(70, 130, 230), 300.0, if st.shield > 0.0 { format!("Shield {:.0} / {:.0}", first.snap.shield, cap) } else { format!("Shield {:.0} (generator)", first.snap.shield) });
                }
                ui.label(RichText::new(&def.description).small().color(DIM));
                if let Some(st) = &st {
                    ui.label(RichText::new(format!("{} · speed {:.1} m/s", armor_text(def.armor_class, st.armor, st.shield, st.shield_regen), st.speed)).small());
                    if let Some(w) = &st.weapon {
                        ui.label(RichText::new(weapon_text(w)).small());
                    }
                    if let Some(a) = &st.aura {
                        ui.label(RichText::new(aura_text(a)).small().color(Color32::from_rgb(120, 200, 255)));
                    }
                    if def.stealth {
                        ui.label(RichText::new("Cloaked: enemy weapons need a detector in range (firing reveals it briefly). Holds fire unless ordered to attack.").small().color(Color32::from_rgb(170, 140, 255)));
                    }
                    sensor_line(ui, &st.sensors);
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
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    ui.set_max_width(600.0);
                    for group in BuildGroup::ALL {
                        let in_group: Vec<u16> = kinds.iter().copied().filter(|k| content.building(*k).group == group).collect();
                        if in_group.is_empty() {
                            continue;
                        }
                        ui.label(RichText::new(group.label().to_uppercase()).small().color(DIM));
                        ui.horizontal_wrapped(|ui| {
                            for k in in_group {
                                let b = content.building(k);
                                let missing = content.building_requires[k as usize]
                                    .iter()
                                    .find(|r| !world.buildings.values().any(|x| x.snap.owner == world.me && x.snap.kind == **r && x.snap.progress >= 1.0))
                                    .map(|r| format!("Requires a finished {}.", content.building(*r).name))
                                    .or_else(|| missing_tech(&world, content, &content.building_tech[k as usize]));
                                ui.vertical(|ui| {
                                    let can = affordable(&b.cost, stock) && missing.is_none();
                                    let tip = match &missing {
                                        Some(why) => format!("{}\n{why}", b.description),
                                        None => b.description.clone(),
                                    };
                                    let label = if missing.is_some() { format!("🔒 {}", b.name) } else { b.name.clone() };
                                    let btn = ui.add_enabled(can, egui::Button::new(label).min_size(egui::vec2(110.0, 26.0)));
                                    if btn.on_hover_text(&tip).on_disabled_hover_text(&tip).clicked() {
                                        placement.kind = Some(k);
                                    }
                                    cost_text(ui, &b.cost, stock);
                                });
                            }
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

    // --- Chat ------------------------------------------------------------------------------
    chat_ui(&ctx, &world, &mut chat, &mut hud, &net, &mut sound, time.elapsed_secs_f64());

    // --- Research tree ---------------------------------------------------------------------
    if hud.research_open {
        let selected_lab = sel.first().copied().filter(|id| world.buildings.get(id).is_some_and(|b| content.building(b.snap.kind).research));
        let tree = tree_layout(content);
        let mut open = true;
        let mut clicked: Option<(EntityId, TechKind, bool)> = None;
        egui::Window::new("Research")
            .frame(egui::Frame::window(&ctx.global_style()).fill(Color32::from_rgb(11, 15, 23)))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .min_width((screen_w - 60.0).min(tree.size.x + 30.0))
            .default_height((ctx.content_rect().height() - 300.0).clamp(300.0, tree.size.y + 60.0))
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, -30.0))
            .show(&ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Click a tech to research it (in the selected lab if it can) · hover shows its path and details").small().color(DIM));
                    for (c, t) in [(GOOD, "researched"), (RESEARCHING, "in progress / queued"), (ACCENT, "available"), (DIM, "locked")] {
                        ui.label(RichText::new(format!("■ {t}")).small().color(c));
                    }
                });
                ui.separator();
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    clicked = research_tree(ui, &tree, &world, content, stock, selected_lab);
                });
            });
        if let Some((lab, tech, can_pay)) = clicked {
            if can_pay {
                net.command(GameCommand::Research { building: lab, tech });
                sound.ui(Sfx::Click);
            } else {
                sound.ui(Sfx::Error);
            }
        }
        if !open {
            hud.research_open = false;
        }
    }

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
                if ui.button("Settings").clicked() {
                    settings_window.open = true;
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
