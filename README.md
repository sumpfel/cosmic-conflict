# Cosmic Conflict
A cool game, built with Rust and [Bevy](https://bevyengine.org).

## Development
```sh
cargo run              # dev build (dynamic linking, fast iteration)
cargo run --release --no-default-features   # release build
```
Linux needs system deps: see the [Bevy setup guide](https://bevyengine.org/learn/quick-start/getting-started/setup/#installing-os-dependencies).

## Layout
- `src/main.rs` – app entry
- `src/plugins/` – game plugins
- `assets/` – models, textures, audio
