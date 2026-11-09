# Cosmic Conflict
A cool game, built with Rust and [Bevy](https://bevyengine.org).

## Development
```sh
cargo run                     # game client (default member, dynamic linking)
cargo run -p cosmic-server    # multiplayer server (TCP, port 7777)
cargo run --release -p cosmic-conflict --no-default-features   # release client (no dynamic linking)
```
Linux needs system deps: see the [Bevy setup guide](https://bevyengine.org/learn/quick-start/getting-started/setup/#installing-os-dependencies).

## Layout (Cargo workspace)
- `crates/game` – Bevy client (`src/plugins/` game plugins, `assets/`)
- `crates/server` – multiplayer backend (tokio, newline-delimited JSON over TCP)
- `crates/shared` – network protocol shared by both
