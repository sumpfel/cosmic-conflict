# Cosmic Conflict — Status & Roadmap

_Last updated: 2026-11-17 (session 2d)._

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

## Session 2 (2026-11-16) — procedural Blender models with CC0 photo materials

- **Pipeline** (see ARCHITECTURE.md → 3D asset pipeline): `tools/fetch_textures.py` downloads 14
  CC0 PBR texture sets (Poly Haven, ambientCG) and writes the credits from the APIs;
  `tools/blender/build_models.py` + `cc_kit.py` generate all 17 models (7 units, 7 buildings,
  3 deposits) from seeded Python in ~1 min: bevels, weighted normals, greebles, panels, vents,
  noise rocks, prism crystals, Cycles-baked AO, one mesh per material slot. `preview.py`
  renders Cycles contact sheets (`docs/models_preview.jpg`).
- **Client:** `game/model_assets.rs` loads the GLBs, decodes + mip-maps the textures on a thread
  (Bevy `jpeg` feature), AO on the second UV channel, team-coloured slots per player; the
  primitive models remain the fallback. Loading waits for the models. 2 new tests.
- Models: hover tank, ducted-fan drone, power-armour trooper, reverse-joint flak walker, fighter,
  bomber, colony ship (habitat ring, cargo pods, radiators); octagonal command centre with
  control tower, landed outpost module, derrick extractor, geodesic habitat, saw-tooth factory,
  gantry shipyard with launch ring, twin-gun turret; iron-ore boulders, crystal cluster, gas vent.
- Notes / ideas: the sun has no shadows yet (a directional shadow light aimed from the sun at
  the focused planet would add a lot of realism); turret heads and walker legs are static (the
  generators could export pivot nodes like MMM's horse); unique per-model texture bakes (edge
  wear, grime, decals) would be the next realism step.

### Session 2b (2026-11-16) — tilted planet camera + free camera

- The planet view no longer looks straight down at the globe's centre: it pivots around a point
  on the surface, leans ~50° towards the horizon by default and can be turned (Q / E, middle
  drag) and tilted from top view to almost ground level (middle drag, Ctrl + wheel, T presets).
  Zooming out to the whole globe straightens the view again; the system view is unchanged.
- WASD pans relative to the screen (parallel transport over the sphere); Shift + middle drag
  drags the ground. **Q / E now turn instead of zoom** (zoom: wheel, PageUp / PageDown).
- Free camera (C): fly anywhere, horizon levelled to the nearest planet, speed scales with the
  altitude, cannot enter planets or the sun; C returns to the planet view of whatever it looks
  at, without a jump. Mode switches and F (focus selection) are eased.
- The camera stays outside the terrain and outside a rough ellipsoid around every building.
- 5 camera tests (pan round trip, pan direction, tilt / no roll, free ↔ planet round trip).
- Ideas: persist the preferred tilt in `settings.ron`; edge scrolling; building heights from the
  model bounds instead of the footprint estimate.

### Session 2c (2026-11-16) — research tree, arsenal, combat model, sensors

- **Research** (`data/research.ron`, `cc_core::research`): 38 techs in 7 fields, tiers 1–4,
  researched in Research Labs (queue of 5 per lab, parallel labs, prerequisites may be queued
  earlier in the same lab, refunds). Techs unlock units / buildings and change stats through
  tag-matched effects; `research::Stats` gives the effective numbers, shared by simulation and
  client. Protocol v2 (`Research` / `CancelResearch`, techs, shields and lab queues in snapshots).
- **Combat model** (`data/combat.ron`): 5 damage kinds × 4 armour classes, flat armour with a 20 %
  floor, regenerating shields (ion tears them down), splash, shield and repair auras,
  self-repair research.
- **Arsenal**: 12 new units (Lancer, Scout Rover, Siege Mortar; robots Sentinel, Mender, Wraith
  Infiltrator, Juggernaut; ships Recon Probe, Missile Frigate, Ion Disruptor, Surveyor,
  Battlecruiser) and 11 new buildings (Research Lab, Robotics Bay; defences Laser Turret,
  Missile Battery, Bunker, Plasma Cannon, Shield Generator; sensors Radar Station, Seismic
  Sensor, Deep-Space Array, Gravimetric Beacon) — 23 Blender generators in
  `tools/blender/models_arsenal.py`, 4 new emissive / dome slots.
- **Sensors for the fog-of-war work** (done by a teammate): sight / radar / seismic / deep-space
  / detection ranges on everything, research upgrades, `Sim::sensors_of`, `Sim::targetable_by`.
  Stealth already matters in combat (only detected cloaked units can be targeted; firing
  reveals them for 2 s; cloaked units hold fire when idle).
- **Client**: research tree window (R) drawn as a graph — tiers left to right, a lane per field,
  curved prerequisite paths, hover highlights the whole path, click queues the tech in the best
  lab; lab panel with queue and quick-start buttons; build menu grouped by category with 🔒 and
  the missing tech / building; unit and building stats (armour class, shields, weapon kind,
  splash, sensors, auras, stealth); shield bars; sensor range rings on the globe for selected
  own things; tracers per damage kind. Dev helpers `CC_SHOWCASE` and `CC_SELECT`.
- Tests: 45 in the workspace (content: every tech does something, every effect matches,
  damage formula; research stats and prerequisites; simulation scenarios listed in
  ARCHITECTURE.md).
- Balance is a first guess: armour was rescaled to 0–5 after the first test showed troopers doing
  minimum damage to plain turrets. Needs play-testing (and bots).
- Ideas: carriers launching fighters, mines, a jammer that hides friendlies from radar, walls,
  orbital defence platforms, an Ion Cannon defence, research-dependent visuals (shield bubble
  flashes on hits, upgraded hull plates).

### Session 2d (2026-11-17) — realistic planet rendering, sun shadows, graphics settings

- **Planet surface shader** (`assets/shaders/planet.wesl`, own lighting from the true sun
  direction of every planet): detail noise and bump mapping in metres (octaves fade with the
  pixel size), rock with strata on steep slopes, exaggerated relief shading, baked cavity AO
  (mesh UV0), water with sun glint, scrolling waves, shore foam and sky reflection, frozen seas
  with cracks, drifting lava crust over a glowing melt, glowing acid, thin branching lava
  fissures on volcanic land, sunlight reddened near the terminator, blue sky light in shadows,
  cloud shadows, and the directional shadow map.
- **Sun shadows**: a directional light from the sun to the focused planet with cascades fitted
  to the visible part of the globe, cross-faded with the old point light (same illuminance, so
  only the shadows appear). Buildings, units, ships and mountains cast shadows; the planet's
  night side shadows its units. In system view the point light alone lights everything.
- **Atmosphere shell** (`atmosphere.wesl`) replaces the five halo spheres: ray-marched Rayleigh +
  Mie single scattering with the planet's shadow in the air, rays stopped at the scene depth
  (depth prepass), and a **cloud layer** composited inside it (domain-warped, eroded billows,
  drifting, lit with reddened sunlight, dark at night, thinning out as the camera comes down).
- **Sun**: boiling granulation, spots and limb darkening; a soft additive corona with streamers.
- **Graphics settings** (window from the main menu and the in-game menu, saved in
  `settings.ron`): presets Low / Medium / High (default) / Ultra and single options — planet
  surface, shadows (off, 1024–4096 px, 2–4 cascades), atmosphere steps, clouds, planet mesh
  resolution (next match), anti-aliasing (off / FXAA / MSAA 4x), bloom; plus volume and camera
  speed sliders (roadmap item 10). Measured on the dev GPU (shared, ~38 % busy before): Low and
  Medium add ~2 %, High ~10 %, Ultra ~20–55 % GPU load at 60 fps.
- Dev helpers: `CC_GRAPHICS=low|medium|high|ultra`, `CC_SETTINGS=1`, `CC_FOCUS=<planet id>`,
  `CC_FPS=1`. 5 new tests (settings compatibility / presets, planet layers, upload threshold,
  sea mask + cavity) — 50 in the workspace.
- Pitfalls found: Bevy draws `AlphaMode::Add` as premultiplied (an additive shader must output
  alpha 0); transparent shells around the sun must be `NotShadowCaster` (the shadow light starts
  in the sun); a material in the depth prepass hides everything behind it.
- Ideas: city lights on the night side around bases; eclipses are already physically there
  (other planets cast into the shadow map) but rare; triplanar CC0 rock textures for the
  closest zoom; a gas giant biome; quality auto-detection on first start.

### In-game chat (2026-11-16, teammate)

- **In-game text chat** (`ui/hud.rs`, `chat_ui`): bottom left. While closed the last 6 lines show
  for 10 s and fade, clicks pass through. **Enter** opens it with the full history and an input;
  Enter sends, Esc / click outside closes (the draft is kept). A click sounds on messages from
  others. Camera/order keys are blocked while typing (`egui_wants_keyboard_input`).
- No protocol change: the server already relayed `LobbyCmd::Chat` in every phase. `ChatLog` lines
  now carry an arrival time (`net::ChatLine`). `tcp_match` test checks chat during a match.
- Verified on Windows: `cargo test --workspace --no-default-features -j 2` green, tried in-game
  with two clients (host + join on 127.0.0.1).
- **Next:** proximity chat once first-person mode exists — `All` / `Nearby` channels, the server
  filters recipients by distance to the possessed unit (needs a protocol bump).

## Roadmap

1. **Computer opponents** (like `mmm_bot`): needed to play alone.
2. **Transports**: carry ground units to other planets (invasions); drop pods.
3. **Fog of war** enforced by the server (as in MMM) — in progress by a teammate; sensors,
   sensor buildings, stealth / detection and their research are in (session 2c).
4. Delta replication instead of full snapshots (MMM's `replicate.rs`).
5. Pathfinding on the sphere (impassable mountains / lava, a cube-sphere nav grid).
6. Planets orbiting the sun and rotating (positions from the tick; deterministic).
7. ~~Research tree, more units, capital ships, shields~~ (session 2c); carriers, mines,
   jammers, walls; animated turret heads.
8. Better art: ~~Blender-generated models, textures~~ (session 2); ~~sun shadows, a planet
   shader (atmospheric scattering, clouds, ocean specular)~~ (session 2d); animated parts
   (turret heads, walker legs), music.
9. Minimap / system map, control groups, rally points, attack-move.
10. ~~Settings screen~~ (session 2d); key bindings.

## Open questions for the owner

- Should ground units be able to fight ships only with dedicated anti-air, or should every
  unit shoot back?
- How large should planets feel (now 60 m home radius; units ~1–3 m)?
- Should neutral planets have native defenders?
- Should research be per player or shared by allied teammates (once there are teams)?
- Should radar contacts show the unit type, or only a blip with a size class?
