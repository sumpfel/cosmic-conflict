# Cosmic Conflict

A multiplayer 3D real-time strategy game in space, built with Rust and [Bevy](https://bevyengine.org) 0.20.

Every commander starts on a **home planet of their own** — a full 3D globe whose entire surface
holds resources. Build extractors anywhere on your world, raise an army and a fleet, colonise
neutral planets and take the star system.

## Quick start

```sh
cargo run                        # game client (Host Game / Join Game in the menu)
cargo run -p cc_server           # dedicated server (default port 47900)
cargo test --workspace           # rules, star-system generation, simulation, full TCP match
cargo run --release -p cosmic-conflict --no-default-features   # optimised build without dynamic linking
```

Linux needs Bevy's system dependencies, see the
[Bevy setup guide](https://bevyengine.org/learn/quick-start/getting-started/setup/#installing-os-dependencies).
The toolchain (Rust 1.99.1) is pinned in `rust-toolchain.toml`.

## Controls

| Input | Action |
|---|---|
| Left click / drag | select (double click: all units of that kind on screen) |
| Right click | move, attack an enemy, help build a construction site |
| Middle drag, WASD / arrows | rotate around the planet |
| Mouse wheel, Q / E | zoom (from the surface out to the whole system) |
| Tab / Shift+Tab, H, Space | next planet, home planet, system view |
| F | focus the selection · Esc cancels placement / selection |

## Workspace

| Crate | Purpose |
|---|---|
| `crates/cc_core` | rules vocabulary, content database (`data/*.ron`), deterministic star-system generation |
| `crates/cc_protocol` | client/server messages and the TCP transport |
| `crates/cc_sim` | the authoritative simulation (runs on the server) |
| `crates/cc_server` | lobby + match server: library (embedded "Host Game") and binary `cc-server` |
| `crates/game` | the Bevy client `cosmic-conflict`: rendering, UI, input |

More: [docs/GAME_DESIGN.md](docs/GAME_DESIGN.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
[docs/STATUS.md](docs/STATUS.md).

## Licence

GPL-3.0-or-later. Asset credits: [crates/game/assets/audio/CREDITS.md](crates/game/assets/audio/CREDITS.md).
