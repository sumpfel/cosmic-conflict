# Cosmic Conflict — Architecture

The structure follows the owner's MMM project (`sumpfel/MMM` + `sumpfel/MMM-backend`), merged
into one Cargo workspace.

## Crates

| Crate | Depends on | Purpose |
|---|---|---|
| `cc_core` | serde, ron, libm, glam | Resources, content database (`data/*.ron`, embedded with `include_str!`, validated, FNV-1a hash), match settings, **star-system generation** (`system.rs`), sphere geometry (`geo.rs`), deterministic RNG + 3D noise (`rng.rs`). No engine. |
| `cc_protocol` | cc_core, postcard | `ClientMsg` / `ServerMsg`, `GameCommand`, `Snapshot`; TCP transport with `[u32 len][postcard]` frames and a thread per direction (taken from MMM). |
| `cc_sim` | cc_core, cc_protocol | The authoritative simulation. |
| `cc_server` | core, protocol, sim | Lobby + match lifecycle. Library (the client embeds it for "Host Game") and binary `cc-server`. |
| `cosmic-conflict` (`crates/game`) | bevy 0.20, bevy_egui 0.43, all of the above | Rendering, UI, input, the client-side mirror of the match. |

## Networking

- **Server-authoritative**, 20 Hz ticks (`TICK_RATE`). Clients only send intents; `Sim::command`
  validates ownership, costs, requirements and placement.
- A full `Snapshot` (players, units, buildings, changed deposits, shots) goes to everyone every
  2 ticks (10 Hz). Per-player messages (`Notice`) go only to that player. No fog of war yet.
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

`Sim::step()`: economy (income, extraction, population) → training (housing check) → units
(orders: Idle with auto-targeting, Move, Attack, Build, Found) → building weapons → separation
(ground units along the surface, ships in space) → damage → deaths → defeat/victory (2 Hz).

Entities live in `BTreeMap`s (deterministic order). Tests in `crates/cc_sim/src/tests.rs` cover
walking around a globe, mining on the far side, placement, training/housing, colonising,
flying around planets, combat and victory.

## Client (`crates/game/src`)

| Module | Role |
|---|---|
| `main.rs` | app, states `Menu → Loading → InGame`, resources |
| `net.rs` | connection thread, message pump, embedded server, `CC_AUTOSTART` |
| `settings.rs` | persisted settings (RON in the OS config dir) |
| `audio.rs` | sound effects through the `Sound` resource |
| `ui/menu.rs` | main menu, host/join, lobby (colours, settings, chat), loading screen |
| `ui/hud.rs` | resources, planet list, selection panel (build/train), notices, planet labels, end screen |
| `game/scene.rs` | camera (HDR, bloom, TonyMcMapface), star field, sun + point light (scaled to the focused planet's distance), menu backdrop, planet + atmosphere spawning |
| `game/planet_mesh.rs` | displaced icosphere (79 subdivisions = 64k vertices) with biome vertex colours |
| `game/models.rs` | procedural unit/building/deposit models from Bevy primitives; material slots (`Team` = owner colour) |
| `game/world.rs` | `ClientWorld`: applies snapshots, spawns/despawns visuals, interpolates movement |
| `game/loading.rs` | regenerates the system on a background thread, spawns planets and deposits, sends the hash |
| `game/camera.rs` | orbit camera around the focused planet / the sun |
| `game/input.rs` | picking (screen-space, occluded by planets), selection, orders, placement ghost, overlays |
| `game/effects.rs` | tracers, explosions, order pings |

Bevy notes: `Sphere::mesh().ico(n)` subdivides each edge **linearly** (n ≤ 79); systems may have
at most 16 parameters (group them in tuples); `egui::TopBottomPanel` no longer exists in egui
0.36 (use an `Area`).

## Development helpers

```bash
cargo run                                          # play
CC_AUTOSTART=1 cargo run                           # host on port 47999 and start a match at once
CC_AUTOSTART=1 CC_AUTOPLAY=1 cargo run             # ... and build extractors, a habitat, train drones
CC_AUTOSTART=lobby cargo run                       # host and stay in the lobby
CC_SEED=5 CC_SIZE=small|large CC_START=high        # match settings for CC_AUTOSTART
CC_SYSTEM_VIEW=1 CC_CAM_DIST=1500                  # start in system view / at that camera distance
CC_SCREENSHOT=/tmp/x.png CC_SCREENSHOT_AFTER=10    # screenshot and quit
CC_WINDOW_SCALE=1 CC_MUTE=1 XDG_CONFIG_HOME=/tmp/cfg   # cheap, silent test runs with their own settings
cargo run -p cc_server -- --port 47900             # dedicated server
```

Run the client through `cargo run` (the `dev` feature links Bevy dynamically; the binary alone
does not find `libbevy_dylib`). Crashes are written to `crash.log` in the settings folder.
