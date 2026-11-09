# Cosmic Conflict — Status & Roadmap

_Last updated: 2026-11-09 (session 1)._

## Session 1 (2026-11-09) — foundation modelled on MMM

- Workspace like MMM: `cc_core`, `cc_protocol`, `cc_sim`, `cc_server`, client `cosmic-conflict`.
  MMM's TCP transport, deterministic RNG, server lifecycle and client networking were reused.
- **Star system:** sun, one home globe per player, symmetric neutral planets (3 sizes),
  deposits over the whole surface of every planet, 7 biomes with terrain relief and seas.
- **Simulation:** ground units on globes (great circles), ships in space (around planets and the
  sun, low orbit over planets), extractors on deposits, income, housing, training queues,
  construction with several builders, colony ships founding outposts, weapons with ground/space
  targeting, auto-targeting, defeat/victory, surrender.
- **Client:** home screen with a lit globe, host/join, lobby (colours, system settings, seed,
  chat, ready), loading on a background thread, space scene (star field, sun with bloom, globes
  with atmospheres), orbit camera, planet list and labels, selection (click, box, double click),
  right-click orders, building placement with a green/red ghost, build/train panels, notices,
  tracers and explosions, end screen. Sounds reused from MMM (CC0 + MMM's own).
- Tests: 24 (generation determinism / fairness / no overlaps, sphere geometry, content
  validation, simulation scenarios, a full match over TCP).

## Roadmap

1. **Computer opponents** (like `mmm_bot`): needed to play alone.
2. **Transports**: carry ground units to other planets (invasions); drop pods.
3. **Fog of war** enforced by the server (as in MMM), radar/sensor buildings.
4. Delta replication instead of full snapshots (MMM's `replicate.rs`).
5. Pathfinding on the sphere (impassable mountains / lava, a cube-sphere nav grid).
6. Planets orbiting the sun and rotating (positions from the tick; deterministic).
7. Research tree (MMM-style content in RON), more units (carriers, capital ships), shields.
8. Better art: Blender-generated models like MMM's pipeline, textures, a planet shader
   (atmospheric scattering, clouds, ocean specular), music.
9. Minimap / system map, control groups, rally points, attack-move.
10. Settings screen: `volume` and `camera_speed` already exist in `settings.ron` but have no UI.

## Open questions for the owner

- Should ground units be able to fight ships only with dedicated anti-air, or should every
  unit shoot back?
- How large should planets feel (now 60 m home radius; units ~1–3 m)?
- Should neutral planets have native defenders?
