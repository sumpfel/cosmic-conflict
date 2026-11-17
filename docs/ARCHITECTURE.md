# Cosmic Conflict — Architecture

The structure follows the owner's MMM project (`sumpfel/MMM` + `sumpfel/MMM-backend`), merged
into one Cargo workspace.

## Crates

| Crate | Depends on | Purpose |
|---|---|---|
| `cc_core` | serde, ron, libm, glam | Resources, content database (`data/*.ron`, embedded with `include_str!`, validated, FNV-1a hash), research tree + effective stats (`research.rs`), combat rules (`CombatRules::hit`), match settings, **star-system generation** (`system.rs`), sphere geometry (`geo.rs`), deterministic RNG + 3D noise (`rng.rs`). No engine. |
| `cc_protocol` | cc_core, postcard | `ClientMsg` / `ServerMsg`, `GameCommand`, `Snapshot`; TCP transport with `[u32 len][postcard]` frames and a thread per direction (taken from MMM). |
| `cc_sim` | cc_core, cc_protocol | The authoritative simulation. |
| `cc_server` | core, protocol, sim | Lobby + match lifecycle. Library (the client embeds it for "Host Game") and binary `cc-server`. |
| `cosmic-conflict` (`crates/game`) | bevy 0.20, bevy_egui 0.43, all of the above | Rendering, UI, input, the client-side mirror of the match. |

## Networking

- **Server-authoritative**, 20 Hz ticks (`TICK_RATE`). Clients only send intents; `Sim::command`
  validates ownership, costs, requirements and placement.
- A full `Snapshot` (players with their finished research, units and buildings with shields,
  lab research queues, changed deposits, shots) goes to everyone every 2 ticks (10 Hz).
  Per-player messages (`Notice`) go only to that player. No fog of war yet.
- `PROTOCOL_VERSION` 2: `GameCommand::Research { building, tech }` / `CancelResearch`,
  `MAX_RESEARCH_QUEUE`.
- The star system is generated from the seed on server and clients; `MapReady { map_hash }`
  checks they agree. `PROTOCOL_VERSION` and the content hash are checked on `Hello`.
- Lifecycle: `Lobby → Loading → Running → GameOver → Lobby`.

## Positions on globes

- A planet is a centre + radius. Anything on its surface is a **unit direction** from the centre
  (`Loc::Ground { planet, dir }`); ships use world positions (`Loc::Space { pos }`).
- Ground units move with `geo::step_towards` (great circle, angle = metres / radius). Ships fly
  straight and are pushed out of planet and sun spheres (`Sim::move_towards`).
- The simulation uses the base sphere; the terrain relief (`Planet::height`, 3D noise sampled on
  the unit sphere) is cosmetic and identical on every client, so units and buildings stand on the
  drawn ground.
- Determinism rules for `cc_core::system` / `rng`: only `+ - * /`, `sqrt`, `floor` and `libm`
  (never `f32::sin` etc.), own RNG and noise.

## Simulation (`cc_sim`)

`Sim::step()`: economy (income, extraction, population) → training (housing check) → research
(lab queues) → detection (who can target which stealthed unit) → units (orders: Idle with
auto-targeting, Move, Attack, Build, Found) → building weapons → separation (ground units along
the surface, ships in space) → hits (shields, then hull) → recovery (shield recharge, auras,
self-repair, stealth timers) → deaths → defeat/victory (2 Hz).

Entities live in `BTreeMap`s (deterministic order). Tests in `crates/cc_sim/src/tests.rs` cover
walking around a globe, mining on the far side, placement, training/housing, colonising,
flying around planets, combat and victory, research (unlocks, prerequisites, queue, refunds,
upgrades), armour vs. bunkers, shields and ion damage, splash, stealth and detection, repair
auras and the sensor API.

### Research, stats and combat

- `data/research.ron` → `TechDef { field, tier, cost, time, requires, effects }`; units and
  buildings list the techs they need in `tech`. Validation: prerequisites come from lower tiers
  (no cycles), effects must match some unit or building (tests), weapon filters only on weapon
  stats.
- **Every number the simulation uses comes from `research::Stats`** — the player's effective
  `UnitStats` / `BuildingStats` per kind (`(base + Σ add) × Π mul` over the finished techs whose
  effect tags match). `Sim` keeps one `Arc<Stats>` per player and rebuilds it in
  `complete_research`; the client builds the same from `PlayerSnap::techs` (`ClientWorld::stats_of`).
  Implicit tags: units get `unit` + `ground`/`ship`, buildings get `building`.
- `data/combat.ron` → `CombatRules`: multipliers per damage kind vs armour class and shields,
  `min_damage`, `shield_delay`, `splash_damage`, `reveal_time`. `CombatRules::hit` is the single
  damage formula (unit-tested in `cc_core`). Hits are collected during the step (`Hit { target,
  damage, kind }`, splash adds more) and applied after movement.
- Auras (`Aura { radius, shield, shield_regen, repair }`) on units / finished buildings affect the
  owner's buildings and units on the same planet; the strongest value per component counts.

### Sensors for fog of war (interface for the fog-of-war work)

- `Sensors { sight, radar, seismic, deep, detect }` on every unit and building (data), scaled by
  research. Intended meaning: see GAME_DESIGN.md §6. Only `detect` is used by the simulation so
  far (stealth targeting); hiding things from clients is the fog-of-war feature.
- `Sim::sensors_of(id) -> Option<(owner, world_pos, Sensors)>`: effective ranges (buildings under
  construction only have half their sight). `Sim::targetable_by(id, player)`: false for a
  stealthed enemy unit no detector of `player` sees (and that has not fired in the last 2 s).
- Ranges are straight-line distances in metres (home planet radius 60 m: radar 80 covers ~45 %
  of the globe, seismic 100 ~70 %); the client draws them as rings on the surface for selected
  own things (`input::draw_overlays`).
- Snapshots still go to everyone; enemy cloaked units are therefore drawn by clients until the
  server filters per player.

## Client (`crates/game/src`)

| Module | Role |
|---|---|
| `main.rs` | app, states `Menu → Loading → InGame`, resources |
| `net.rs` | connection thread, message pump, embedded server, `CC_AUTOSTART` |
| `settings.rs` | persisted settings (RON in the OS config dir): name, address, colour, volume, camera speed, `Graphics` (preset + options) |
| `audio.rs` | sound effects through the `Sound` resource |
| `ui/menu.rs` | main menu, host/join, lobby (colours, settings, chat), loading screen |
| `ui/settings_window.rs` | settings window (graphics presets and options, volume, camera speed), from the main menu and the in-game menu |
| `ui/hud.rs` | resources, planet list, selection panel (build menu grouped by category, train, lab research, unit stats: armour, shields, weapon, sensors, auras), research tree window (tiers × field lanes, prerequisite paths painted with egui), notices, planet labels, end screen |
| `game/scene.rs` | camera (HDR, TonyMcMapface), star field, sun (surface + corona shader), point light and directional shadow light, menu backdrop |
| `game/planet_mesh.rs` | displaced icosphere (40–79 subdivisions by setting) with biome vertex colours; UV0 = (cavity, sea mask) |
| `game/planet_render.rs` | planet / atmosphere / sun materials and their per-planet parameters, planet spawning, the sun light rig (point ↔ directional shadow light, cascades), applying the graphics settings (MSAA / FXAA, bloom, shadow map, depth prepass) |
| `game/models.rs` | `Models`: parts (mesh + one material or one per player colour) per model key; primitive fallbacks built at start-up |
| `game/model_assets.rs` | loads the Blender GLBs + baked AO maps, decodes/mip-maps the CC0 textures on a thread, builds slot materials from `models/materials.ron`, swaps them into `Models` (`ready`) |
| `game/world.rs` | `ClientWorld`: applies snapshots, effective stats per player, spawns/despawns visuals, interpolates movement |
| `game/loading.rs` | regenerates the system on a background thread, spawns planets and deposits, sends the hash |
| `game/camera.rs` | planet view pivoting on a surface point (heading + tilt, parallel-transport panning, stays out of terrain and buildings), system view around the sun, free-flying camera; eased mode switches |
| `game/input.rs` | picking (screen-space, occluded by planets), selection, orders, placement ghost, overlays (health and shield bars, sensor range rings) |
| `game/effects.rs` | tracers per damage kind (kinetic streaks, laser beams, ion arcs, plasma bolts; explosive / plasma impacts), explosions, order pings |

Bevy notes: `Sphere::mesh().ico(n)` subdivides each edge **linearly** (n ≤ 79); systems may have
at most 16 parameters (group them in tuples); `egui::TopBottomPanel` no longer exists in egui
0.36 (use an `Area`).

### Planet rendering (`game/planet_render.rs`, `assets/shaders/*.wesl`)

- Shaders are **WESL** (Bevy 0.20): `import bevy_pbr::render::...`, the material group is
  `@group(constants::MATERIAL_BIND_GROUP)`, `@if(DEF)` for shader defs, `super::planet_common`
  for the shared module (noise with analytic derivatives, ray / sphere, the cloud field).
- `PlanetParams` (11 × vec4, Rust and WGSL must match) per planet: centre + radius, rotation,
  relief / sea height / sea kind, sun direction + illuminance, shadow mix, atmosphere tint /
  density / top / scale height / haze, rock colour, cloud colour / coverage / height / opacity,
  quality (surface level, view and light steps, cloud octaves). `base_params` fills the static
  part from the biome; `update_planet_materials` writes the rest every frame (only uploads when
  something changed noticeably).
- **Lighting split**: planet surfaces, atmospheres and clouds light themselves from the sun at
  the origin (correct for every planet at once). Units / buildings / ships use Bevy lights:
  `SunRig` cross-fades the point light (no shadows) and the `SunShadowLight` directional light
  (sun → focused planet, cascades fitted from the camera to past the horizon) by camera
  distance; both give `FOCUS_LUX` there. The planet shader samples the same shadow map
  (`fetch_directional_shadow`) on the focused planet.
- **Atmosphere shell**: a sphere 1 % larger than the air, `cull_mode: None`; the shader keeps
  front faces from outside and back faces from inside, marches from the camera to the ground /
  scene depth (`DepthPrepass` on the camera while the atmosphere or clouds are on) and writes
  premultiplied colour (alpha = cloud cover and extinction).
- Things that must not cast sun shadows: everything near the sun (corona, star field),
  atmosphere shells. `AlphaMode::Add` shaders must return alpha 0 (premultiplied blending).

## 3D asset pipeline (Blender + Python)

```bash
python3 tools/fetch_textures.py                       # CC0 texture sets -> crates/game/assets/textures (+ CREDITS.md)
blender -b --python tools/blender/build_models.py     # all 40 models -> crates/game/assets/models (~2.5 min, CPU)
blender -b --python tools/blender/build_models.py -- tank fighter               # some
blender -b --python tools/blender/build_models.py -- --preview /tmp/m.png tank  # + Cycles contact sheet
blender -b --python tools/blender/build_models.py -- --variants 6 --seed 3 --out /tmp/v --preview /tmp/v.png fighter
blender -b --python tools/blender/preview.py -- /tmp/all.png crates/game/assets/models --ships fighter,bomber,colony_ship
```

- **Arsenal generators** (`tools/blender/models_arsenal.py`, merged into `MODELS`): the 23
  research-tree models — Lancer, Scout Rover, Siege Mortar, Sentinel, Mender, Wraith
  Infiltrator, Juggernaut, Recon Probe, Missile Frigate, Ion Disruptor, Surveyor, Battlecruiser,
  Research Lab, Robotics Bay, Laser Turret, Missile Battery, Bunker, Plasma Cannon, Shield
  Generator, Radar Station, Seismic Sensor, Deep-Space Array, Gravimetric Beacon. Extra slots for
  them: `Plasma`, `Laser`, `Ion` (emissive) and `Shield` (additive dome).
- **Generators** (`tools/blender/build_models.py`): one function per model key (the `model` names
  in `data/*.ron`, deposits are `deposit_<resource>`), all randomness from a seeded RNG
  (`zlib.crc32(key)`), so the game's files are reproducible (AO maps bit-identical, a few UV
  floats differ in the last bits); `--variants` shows what a generator can vary. Never hand-edit a GLB.
- **Kit** (`tools/blender/cc_kit.py`): bmesh primitives in **Blender coordinates — x right, y
  forward (nose / front door), z up** — box (taper, slope, wedge), beam, lathe, cylinder, sphere,
  torus, loft (superellipse fuselages), plate (extruded outlines: wings, fins), tube, noise
  rocks, lattice towers, crystals, greebles, vents, bolts; `sym()` mirrors, `around(n)` repeats.
  UVs are per loop in metres and scaled to each texture's physical size (`meta.json`).
  The glTF exporter turns this into y-up with the nose at −Z = the game's forward. Origin: ground
  contact (ground units, buildings, deposits; foundations go 0.3 m below) or centre (ships).
- **Finish** (`Model.finish`): angle-limited bevel (40°, 2 segments) + weighted normals on hard
  surfaces → join → second UV map (smart project) → **Cycles AO bake** (with a ground plane for
  things on a planet) → `<key>_ao.png` (8-bit grey) → split by material → `<key>_<Slot>` meshes →
  GLB (placeholder materials, no textures, no tangents).
- **Slots** (`cc_kit.MATERIALS`, written to `models/materials.ron`): the contract between Blender
  and the game. `Team` = player colour tint, `Glow` = player colour light; `Window`/`Light`/
  `Engine`/`Crystal`/`Gas` emit; `Haze` is additive. Add a slot there, never in Rust.
- **Client** (`game/model_assets.rs`): textures are decoded to `CC_TEXTURE_SIZE` (default 512)
  with CPU mip chains (sRGB-correct albedo, renormalised normals), AO as R8 on `UvChannel::Uv1`,
  tangents generated at load. `poll_loading` waits for `Models::ready`; missing GLBs keep the
  primitive model. `model_assets::tests` checks that every model key has a GLB, an AO map and
  known slots, and that every texture set is complete.
- **Credits**: every downloaded asset is listed with author, licence and link in
  `assets/textures/CREDITS.md` (written from the APIs by `fetch_textures.py`); only CC0 sources.

## Development helpers

```bash
cargo run                                          # play
CC_AUTOSTART=1 cargo run                           # host on port 47999 and start a match at once
CC_AUTOSTART=1 CC_AUTOPLAY=1 cargo run             # ... and build extractors, a habitat, train drones
CC_AUTOSTART=lobby cargo run                       # host and stay in the lobby
CC_SEED=5 CC_SIZE=small|large CC_START=high        # match settings for CC_AUTOSTART
CC_SYSTEM_VIEW=1 CC_CAM_DIST=1500                  # start in system view / at that camera distance
CC_CAM_TILT=72 CC_CAM_HEADING=35                   # planet view tilt / heading in degrees (0 = top view)
CC_SHOWCASE=1                                      # server: all research + one of every unit / building at the start
CC_SHOWCASE=research CC_SELECT=research_lab        # only tier 1 researched, research tree open, lab selected
CC_SELECT=radar_station                            # select my first unit / building of that id (rings, stats)
CC_SCREENSHOT=/tmp/x.png CC_SCREENSHOT_AFTER=10    # screenshot and quit
CC_WINDOW_SCALE=1 CC_MUTE=1 XDG_CONFIG_HOME=/tmp/cfg   # cheap, silent test runs with their own settings
CC_GRAPHICS=low|medium|high|ultra                  # graphics preset for this run (overrides settings.ron)
CC_SETTINGS=1                                      # open the settings window at start
CC_FOCUS=2                                         # start the planet view at planet id 2 instead of home
CC_FPS=1                                           # log the frame rate every 2 s (asks for no vsync)
cargo run -p cc_server -- --port 47900             # dedicated server
```

Run the client through `cargo run` (the `dev` feature links Bevy dynamically; the binary alone
does not find `libbevy_dylib`). Crashes are written to `crash.log` in the settings folder.
