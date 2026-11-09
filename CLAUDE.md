# Cosmic Conflict — notes for Claude

Read `docs/GAME_DESIGN.md` (§0 is binding), `docs/STATUS.md` and `docs/ARCHITECTURE.md` first.

- One Cargo workspace: `cc_core` (rules, content, star-system generation), `cc_protocol`,
  `cc_sim` (authoritative simulation), `cc_server`, and the Bevy 0.20 client in `crates/game`.
  Modelled on the owner's MMM project (`~/Dokumente/software/own/rts-game/`), which may be reused.
- Content changes go into `data/*.ron`; `cargo test --workspace` must stay green.
- Keep `cc_core::system` / `rng` deterministic (`libm` only, no `f32::sin` etc.).
- Bump `cc_protocol::PROTOCOL_VERSION` on incompatible message changes.
- Verify visuals with the screenshot hook (`CC_AUTOSTART=1 CC_SCREENSHOT=… CC_WINDOW_SCALE=1
  CC_MUTE=1`, run through `cargo run`); check `nvidia-smi` first, the GPU (6 GB) is shared.
- Update `docs/STATUS.md` at the end of each session.
- **Never add Claude/AI as author or co-author** — no `Co-Authored-By` lines, no "Generated with"
  footers in commits or PRs (owner rule).
