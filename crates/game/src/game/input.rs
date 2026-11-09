//! Mouse & keyboard in the match: picking, selection (click, box, double click), context
//! commands (move / attack / help build), building placement with a live ghost, and the
//! selection rings and health bars.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use cc_core::content::{BuildingKind, ContentDb, Domain};
use cc_core::geo;
use cc_protocol::{EntityId, GameCommand, Target};

use super::camera::OrbitCam;
use super::effects::Effects;
use super::models::{Models, UNIT_SCALE};
use super::scene::MainCamera;
use super::world::{ClientWorld, WorldEntity};
use crate::audio::{Sfx, Sound};
use crate::net::Net;
use crate::Content;

#[derive(Resource, Default)]
pub struct Selection(pub Vec<EntityId>);

#[derive(Resource, Default)]
pub struct Placement {
    pub kind: Option<BuildingKind>,
    ghosts: Option<(BuildingKind, Entity, Entity)>,
    pub problem: Option<String>,
}

/// Mouse drag in progress (for the box selection drawn by the HUD).
#[derive(Resource, Default)]
pub struct Drag {
    pub start: Option<Vec2>,
    pub current: Vec2,
    last_click: Option<(f64, EntityId)>,
}

impl Drag {
    pub fn is_box(&self) -> bool {
        self.start.is_some_and(|s| s.distance(self.current) > 6.0)
    }
}

/// The entity under the cursor.
#[derive(Resource, Default)]
pub struct Hover(pub Option<EntityId>);

fn occluded(world: &ClientWorld, eye: Vec3, target: Vec3) -> bool {
    let d = target - eye;
    let len = d.length();
    if len < 1e-3 {
        return false;
    }
    let dir = d / len;
    world
        .system
        .planets
        .iter()
        .any(|p| geo::ray_sphere(eye, dir, p.center, p.radius * 0.96).is_some_and(|t| t < len - 0.5))
}

struct Pickable {
    id: EntityId,
    pos: Vec3,
    radius: f32,
}

fn pickables<'a>(world: &'a ClientWorld, content: &'a ContentDb) -> impl Iterator<Item = Pickable> + 'a {
    let units = world.units.values().map(|u| {
        let r = content.unit(u.snap.kind).radius * UNIT_SCALE;
        Pickable { id: u.snap.id, pos: u.pos + u.up * r * 0.6, radius: r }
    });
    let buildings = world.buildings.values().map(|b| {
        let r = content.building(b.snap.kind).radius;
        Pickable { id: b.snap.id, pos: b.pos + b.up * r * 0.3, radius: r }
    });
    units.chain(buildings)
}

fn pick_entity(world: &ClientWorld, content: &ContentDb, cam: &Camera, cam_t: &GlobalTransform, cursor: Vec2) -> Option<EntityId> {
    let eye = cam_t.translation();
    let right = cam_t.right().as_vec3();
    let mut best: Option<(f32, EntityId)> = None;
    for p in pickables(world, content) {
        let Ok(sp) = cam.world_to_viewport(cam_t, p.pos) else { continue };
        let Ok(edge) = cam.world_to_viewport(cam_t, p.pos + right * p.radius) else { continue };
        let r_px = sp.distance(edge).max(9.0);
        let d = sp.distance(cursor);
        if d <= r_px * 1.2 && !occluded(world, eye, p.pos) {
            let score = d / r_px;
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, p.id));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// Client-side copy of the placement rules (the server checks again).
pub fn check_site(
    world: &ClientWorld,
    content: &ContentDb,
    kind: BuildingKind,
    planet: u16,
    dir: Vec3,
    builders: &[EntityId],
) -> Result<Vec3, String> {
    let def = content.building(kind);
    let p = &world.system.planets[planet as usize];
    let colonizer = builders.iter().any(|id| world.units.get(id).is_some_and(|u| content.unit(u.snap.kind).colonizer));
    if !colonizer && !builders.iter().any(|id| world.units.get(id).is_some_and(|u| u.snap.planet == planet)) {
        return Err("Your builders are not on this planet.".into());
    }
    let mut dir = dir;
    if def.extracts {
        let best = world
            .system
            .deposits
            .iter()
            .filter(|d| d.planet == planet && world.deposits[d.id as usize] > 0.0)
            .map(|d| (p.arc(d.dir, dir), d))
            .filter(|(a, _)| *a < def.radius + 2.5)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, d)) = best else { return Err("Place extractors on a resource deposit.".into()) };
        dir = d.dir;
        if world.buildings.values().any(|b| b.snap.planet == planet && content.building(b.snap.kind).extracts && p.arc(Vec3::from_array(b.snap.dir), dir) < 0.5) {
            return Err("This deposit is already being mined.".into());
        }
    } else if world
        .system
        .deposits
        .iter()
        .any(|d| d.planet == planet && world.deposits[d.id as usize] > 0.0 && p.arc(d.dir, dir) < def.radius + 1.5)
    {
        return Err("Too close to a resource deposit.".into());
    }
    for b in world.buildings.values().filter(|b| b.snap.planet == planet) {
        let r = content.building(b.snap.kind).radius;
        if p.arc(Vec3::from_array(b.snap.dir), dir) < r + def.radius + 1.0 {
            return Err("Blocked by another building.".into());
        }
    }
    Ok(dir)
}

pub fn my_builders(world: &ClientWorld, content: &ContentDb, sel: &Selection, kind: Option<BuildingKind>) -> Vec<EntityId> {
    sel.0
        .iter()
        .copied()
        .filter(|id| {
            world.units.get(id).is_some_and(|u| {
                u.snap.owner == world.me
                    && match kind {
                        Some(k) => content.unit_builds[u.snap.kind as usize].contains(&k),
                        None => !content.unit_builds[u.snap.kind as usize].is_empty(),
                    }
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn handle_input(
    mut commands: Commands,
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    world: Option<Res<ClientWorld>>,
    content: Res<Content>,
    models: Res<Models>,
    net: Res<Net>,
    mut orbit: Option<ResMut<OrbitCam>>,
    (mut selection, mut placement, mut drag, mut hover): (ResMut<Selection>, ResMut<Placement>, ResMut<Drag>, ResMut<Hover>),
    (mut fx, mut sound): (ResMut<Effects>, ResMut<Sound>),
    mut ghost_q: Query<(&mut Transform, &mut Visibility)>,
    mut egui: bevy_egui::EguiContexts,
) {
    let Some(world) = world else { return };
    let content = &content.0;
    let Ok((cam, cam_t)) = cam_q.single() else { return };
    let ui_pointer = egui.ctx_mut().map(|c| c.is_pointer_over_egui() || c.egui_wants_pointer_input()).unwrap_or(false);
    let ui_keys = egui.ctx_mut().map(|c| c.egui_wants_keyboard_input()).unwrap_or(false);
    selection.0.retain(|id| world.units.contains_key(id) || world.buildings.contains_key(id));

    let cursor = window.cursor_position();
    let ray = cursor.and_then(|c| cam.viewport_to_world(cam_t, c).ok());
    hover.0 = match (cursor, ui_pointer) {
        (Some(c), false) => pick_entity(&world, content, cam, cam_t, c),
        _ => None,
    };
    let ground = ray.and_then(|r| world.system.pick(r.origin, r.direction.as_vec3()));

    if !ui_keys && keys.just_pressed(KeyCode::Escape) {
        if placement.kind.is_some() {
            placement.kind = None;
        } else {
            selection.0.clear();
        }
    }
    if !ui_keys && keys.just_pressed(KeyCode::KeyF)
        && let Some(id) = selection.0.first()
        && let Some(o) = orbit.as_mut()
    {
        let planet = world.units.get(id).map(|u| u.snap.planet).or_else(|| world.buildings.get(id).map(|b| b.snap.planet));
        if let Some(p) = planet.filter(|p| *p != cc_protocol::NO_PLANET) {
            o.focus_planet(&world.system, p);
            if let Some(pos) = world.entity_pos(*id) {
                o.look_at_dir((pos - world.system.planets[p as usize].center).normalize());
            }
        }
    }

    // --- Building placement --------------------------------------------------------------------
    if let Some(kind) = placement.kind {
        let builders = my_builders(&world, content, &selection, Some(kind));
        if builders.is_empty() {
            placement.kind = None;
        }
    }
    if placement.ghosts.map(|g| g.0) != placement.kind {
        if let Some((_, a, b)) = placement.ghosts.take() {
            commands.entity(a).despawn();
            commands.entity(b).despawn();
        }
        if let Some(kind) = placement.kind {
            let parts = models.parts(&content.building(kind).model);
            let mut make = |mat: &Handle<StandardMaterial>| {
                let e = commands.spawn((WorldEntity, Transform::default(), Visibility::Hidden)).id();
                models.spawn_parts(&mut commands, e, parts, 0, 1.0, Some(mat));
                e
            };
            let ok = make(&models.ghost_ok);
            let bad = make(&models.ghost_bad);
            placement.ghosts = Some((kind, ok, bad));
        }
    }
    if let (Some(kind), Some((_, ok_e, bad_e))) = (placement.kind, placement.ghosts) {
        let builders = my_builders(&world, content, &selection, Some(kind));
        let site = if ui_pointer { None } else { ground };
        let check = site.map(|(planet, dir)| (planet, check_site(&world, content, kind, planet, dir, &builders)));
        let (show_ok, show_bad) = match &check {
            Some((_, Ok(_))) => (true, false),
            Some((_, Err(_))) => (false, true),
            None => (false, false),
        };
        placement.problem = match &check {
            Some((_, Err(e))) => Some(e.clone()),
            _ => None,
        };
        if let Some((planet, dir)) = site {
            let snapped = match &check {
                Some((_, Ok(d))) => *d,
                _ => dir,
            };
            let p = &world.system.planets[planet as usize];
            let pos = p.center + snapped * (p.radius + p.height(snapped) - 0.2);
            let t = Transform::from_translation(pos).with_rotation(Quat::from_rotation_arc(Vec3::Y, snapped));
            for (e, show) in [(ok_e, show_ok), (bad_e, show_bad)] {
                if let Ok((mut tr, mut vis)) = ghost_q.get_mut(e) {
                    *tr = t;
                    *vis = if show { Visibility::Visible } else { Visibility::Hidden };
                }
            }
        } else {
            for e in [ok_e, bad_e] {
                if let Ok((_, mut vis)) = ghost_q.get_mut(e) {
                    *vis = Visibility::Hidden;
                }
            }
        }
        if !ui_pointer && buttons.just_pressed(MouseButton::Left) {
            match check {
                Some((planet, Ok(dir))) => {
                    net.command(GameCommand::Build { builders, kind, planet, dir: dir.to_array() });
                    let p = &world.system.planets[planet as usize];
                    fx.ping(p.surface(dir), dir, Color::srgb(0.3, 1.0, 0.4));
                    sound.ui(Sfx::Place);
                    if !keys.pressed(KeyCode::ShiftLeft) {
                        placement.kind = None;
                    }
                }
                Some((_, Err(_))) => sound.ui(Sfx::Error),
                None => {}
            }
            return;
        }
        if buttons.just_pressed(MouseButton::Right) {
            placement.kind = None;
            return;
        }
        return;
    }

    // --- Selection -----------------------------------------------------------------------------
    if let Some(c) = cursor {
        drag.current = c;
    }
    if buttons.just_pressed(MouseButton::Left) && !ui_pointer {
        drag.start = cursor;
    }
    if buttons.just_released(MouseButton::Left)
        && let Some(start) = drag.start.take()
    {
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let end = drag.current;
        if start.distance(end) > 6.0 {
            let (min, max) = (start.min(end), start.max(end));
            let eye = cam_t.translation();
            let inside: Vec<EntityId> = world
                .units
                .values()
                .filter(|u| u.snap.owner == world.me)
                .filter(|u| {
                    cam.world_to_viewport(cam_t, u.pos)
                        .is_ok_and(|p| p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y)
                        && !occluded(&world, eye, u.pos + u.up)
                })
                .map(|u| u.snap.id)
                .collect();
            if !shift {
                selection.0.clear();
            }
            for id in inside {
                if !selection.0.contains(&id) {
                    selection.0.push(id);
                }
            }
        } else if let Some(id) = hover.0 {
            let now = time.elapsed_secs_f64();
            let double = drag.last_click.is_some_and(|(t, last)| last == id && now - t < 0.35);
            drag.last_click = Some((now, id));
            if double && let Some(u) = world.units.get(&id).filter(|u| u.snap.owner == world.me) {
                // Everything of the same kind on screen.
                let kind = u.snap.kind;
                let eye = cam_t.translation();
                selection.0 = world
                    .units
                    .values()
                    .filter(|v| v.snap.owner == world.me && v.snap.kind == kind)
                    .filter(|v| cam.world_to_viewport(cam_t, v.pos).is_ok() && !occluded(&world, eye, v.pos + v.up))
                    .map(|v| v.snap.id)
                    .collect();
            } else if shift && world.units.get(&id).is_some_and(|u| u.snap.owner == world.me) {
                if let Some(i) = selection.0.iter().position(|s| *s == id) {
                    selection.0.remove(i);
                } else {
                    selection.0.push(id);
                }
            } else {
                selection.0 = vec![id];
            }
            sound.ui(Sfx::Select);
        } else if let Some((planet, _)) = ground {
            // Double click on a planet focuses it; a single click clears the selection.
            let now = time.elapsed_secs_f64();
            let pid = u32::MAX - planet as u32;
            if drag.last_click.is_some_and(|(t, last)| last == pid && now - t < 0.35)
                && let Some(o) = orbit.as_mut()
            {
                o.focus_planet(&world.system, planet);
            }
            drag.last_click = Some((now, pid));
            if !shift {
                selection.0.clear();
            }
        } else if !shift {
            selection.0.clear();
        }
    }

    // --- Orders --------------------------------------------------------------------------------
    if buttons.just_pressed(MouseButton::Right) && !ui_pointer {
        let mine: Vec<EntityId> =
            selection.0.iter().copied().filter(|id| world.units.get(id).is_some_and(|u| u.snap.owner == world.me)).collect();
        if mine.is_empty() {
            return;
        }
        if let Some(target) = hover.0 {
            let owner = world.owner_of(target);
            if owner.is_some_and(|o| o != world.me) {
                net.command(GameCommand::Attack { units: mine, target });
                if let Some(p) = world.entity_pos(target) {
                    fx.ping(p, (p - cam_t.translation()).normalize() * -1.0, Color::srgb(1.0, 0.3, 0.2));
                }
                sound.ui(Sfx::Attack);
                return;
            }
            if let Some(b) = world.buildings.get(&target).filter(|b| b.snap.progress < 1.0) {
                let builders = my_builders(&world, content, &selection, None);
                if !builders.is_empty() {
                    net.command(GameCommand::Assist { builders, building: b.snap.id });
                    sound.ui(Sfx::Order);
                    return;
                }
            }
        }
        if let Some((planet, dir)) = ground {
            net.command(GameCommand::Move { units: mine, to: Target::Ground { planet, dir: dir.to_array() } });
            let p = &world.system.planets[planet as usize];
            fx.ping(p.surface(dir), dir, Color::srgb(0.4, 0.9, 1.0));
            sound.ui(Sfx::Order);
        } else if let Some(r) = ray {
            // Space: a point on the horizontal plane through the camera's centre.
            let has_ships = mine.iter().any(|id| world.units.get(id).is_some_and(|u| content.unit(u.snap.kind).domain == Domain::Space));
            let center_y = orbit.as_ref().map(|o| o.center.y).unwrap_or(0.0);
            let d = r.direction.as_vec3();
            if has_ships && d.y.abs() > 1e-3 {
                let t = (center_y - r.origin.y) / d.y;
                if t > 0.0 {
                    let pos = r.origin + d * t;
                    net.command(GameCommand::Move { units: mine, to: Target::Space { pos: pos.to_array() } });
                    fx.ping(pos, Vec3::Y, Color::srgb(0.4, 0.9, 1.0));
                    sound.ui(Sfx::Order);
                }
            }
        }
    }
}

/// Selection rings, hover ring, health and construction bars.
pub fn draw_overlays(
    world: Option<Res<ClientWorld>>,
    content: Res<Content>,
    selection: Res<Selection>,
    hover: Res<Hover>,
    cam_q: Query<&GlobalTransform, With<MainCamera>>,
    mut gizmos: Gizmos,
) {
    let Some(world) = world else { return };
    let Ok(cam_t) = cam_q.single() else { return };
    let eye = cam_t.translation();
    let right = cam_t.right().as_vec3();
    let content = &content.0;
    let ring = |g: &mut Gizmos, pos: Vec3, up: Vec3, r: f32, c: Color| {
        g.circle(Isometry3d::new(pos + up * 0.25, Quat::from_rotation_arc(Vec3::Z, up)), r, c);
    };
    let bar = |g: &mut Gizmos, top: Vec3, width: f32, frac: f32, c: Color| {
        let a = top - right * width * 0.5;
        let b = top + right * width * 0.5;
        g.line(a, b, Color::srgba(0.05, 0.05, 0.05, 0.9));
        g.line(a, a.lerp(b, frac.clamp(0.0, 1.0)), c);
    };
    let health_color = |f: f32| if f > 0.6 { Color::srgb(0.2, 1.0, 0.3) } else if f > 0.3 { Color::srgb(1.0, 0.8, 0.1) } else { Color::srgb(1.0, 0.2, 0.1) };
    for u in world.units.values() {
        let def = content.unit(u.snap.kind);
        let r = def.radius * UNIT_SCALE;
        let selected = selection.0.contains(&u.snap.id);
        let hovered = hover.0 == Some(u.snap.id);
        let damaged = u.snap.hp < def.hp;
        if !(selected || hovered || damaged) || eye.distance(u.pos) > 600.0 || occluded(&world, eye, u.pos + u.up) {
            continue;
        }
        let up = if u.snap.planet == cc_protocol::NO_PLANET { (eye - u.pos).normalize() } else { u.up };
        let mine = u.snap.owner == world.me;
        if selected {
            ring(&mut gizmos, u.pos, up, r * 1.4, if mine { Color::srgb(0.3, 1.0, 0.4) } else { Color::srgb(1.0, 0.3, 0.3) });
        } else if hovered {
            ring(&mut gizmos, u.pos, up, r * 1.4, Color::srgba(1.0, 1.0, 1.0, 0.6));
        }
        if selected || damaged {
            let f = u.snap.hp / def.hp;
            bar(&mut gizmos, u.pos + u.up * (r * 2.2 + 0.6), r * 2.2, f, health_color(f));
        }
    }
    for b in world.buildings.values() {
        let def = content.building(b.snap.kind);
        let selected = selection.0.contains(&b.snap.id);
        let hovered = hover.0 == Some(b.snap.id);
        let building = b.snap.progress < 1.0;
        let damaged = b.snap.hp < def.hp && !building;
        if !(selected || hovered || damaged || building) || eye.distance(b.pos) > 900.0 || occluded(&world, eye, b.pos + b.up * 2.0) {
            continue;
        }
        let mine = b.snap.owner == world.me;
        if selected {
            ring(&mut gizmos, b.pos, b.up, def.radius + 0.8, if mine { Color::srgb(0.3, 1.0, 0.4) } else { Color::srgb(1.0, 0.3, 0.3) });
        } else if hovered {
            ring(&mut gizmos, b.pos, b.up, def.radius + 0.8, Color::srgba(1.0, 1.0, 1.0, 0.6));
        }
        let top = b.pos + b.up * (def.radius * 1.2 + 3.0);
        if building {
            bar(&mut gizmos, top, def.radius * 1.6, b.snap.progress, Color::srgb(1.0, 0.85, 0.2));
        } else if selected || damaged {
            let f = b.snap.hp / def.hp;
            bar(&mut gizmos, top, def.radius * 1.6, f, health_color(f));
        }
    }
}

/// Development helper: `CC_AUTOPLAY=1` (with `CC_AUTOSTART`) gives the first orders a few
/// seconds into the match: extractors on the nearest deposits, a habitat, two drones in
/// training, and selects the builders (for screenshots and smoke tests).
pub fn dev_autoplay(
    time: Res<Time>,
    mut stage: Local<(u8, f32)>,
    world: Option<Res<ClientWorld>>,
    content: Res<Content>,
    net: Res<Net>,
    mut selection: ResMut<Selection>,
) {
    if std::env::var("CC_AUTOPLAY").is_err() {
        return;
    }
    let Some(world) = world else { return };
    if world.units.is_empty() {
        return;
    }
    stage.1 += time.delta_secs();
    let content = &content.0;
    if stage.0 == 0 && stage.1 > 2.0 {
        stage.0 = 1;
        let Some(home) = world.home_planet() else { return };
        let drones: Vec<EntityId> = world.units.values().filter(|u| u.snap.owner == world.me).map(|u| u.snap.id).collect();
        let start = world.system.planets[home as usize].start_dir;
        let ext = content.building_kind("extractor").expect("extractor");
        let mut deps: Vec<_> = world.system.deposits.iter().filter(|d| d.planet == home).collect();
        deps.sort_by(|a, b| b.dir.dot(start).total_cmp(&a.dir.dot(start)));
        for (i, d) in deps.iter().take(3).enumerate() {
            net.command(GameCommand::Build { builders: vec![drones[i % drones.len()]], kind: ext, planet: home, dir: d.dir.to_array() });
        }
        let hab = content.building_kind("habitat").expect("habitat");
        let site = (0..400)
            .map(|i| geo::offset(start, i as f32 * 0.9, 0.2 + 0.004 * i as f32))
            .find(|d| check_site(&world, content, hab, home, *d, &drones).is_ok());
        if let Some(d) = site {
            net.command(GameCommand::Build { builders: vec![drones[3 % drones.len()]], kind: hab, planet: home, dir: d.to_array() });
        }
        if let Some(cc) = world.buildings.values().find(|b| b.snap.owner == world.me) {
            let drone = content.unit_kind("drone").expect("drone");
            for _ in 0..2 {
                net.command(GameCommand::Train { building: cc.snap.id, kind: drone });
            }
        }
        selection.0 = drones;
    }
}
