# Cosmic Conflict — Game Design

## 0. Owner requirements (binding)

1. The game is a 3D RTS **in space**, built like the owner's MMM project (server-authoritative
   multiplayer, Bevy client, egui UI); MMM's code and assets may be reused.
2. **Every player starts on their own planet.**
3. **The entire planet can be used for resources**: deposits cover the whole globe, not just the
   area around the start.
4. Planets are **3D globes**.

## 1. The star system

- A sun in the centre; one home planet per player on a ring around it; neutral planets in
  between. The system is **rotationally symmetric**: neutral planets are generated for one
  sector and copied into every sector, so all players get the same neighbourhood.
- Generated deterministically from the match seed on the server and every client
  (`cc_core::system`); the clients report a hash after loading.
- Sizes: Small / Medium / Large = 1 / 2 / 3 neutral planets per player.
- Biomes (look only, for now): Terran, Jungle, Desert, Ice, Volcanic, Barren, Toxic. Seas (water,
  ice, lava, acid) are flat and walkable.

## 2. Resources

| Resource | Used for |
|---|---|
| Metal | almost everything |
| Crystal | advanced buildings, ships |
| Gas | heavy units, colony ships |

- Deposits are spread over the whole globe (Fibonacci sphere, about 50 on a home planet). Each home
  planet also has a guaranteed cluster next to the start (3 metal, 2 crystal, 1 gas). Neutral
  planets are richer and specialise in one resource.
- An **Extractor** built on a deposit mines it (2.5/s) until it is empty. Command Centers and
  Outposts give a small income of their own, so nobody can get stuck.

## 3. Units and buildings

Defined in `data/units.ron` and `data/buildings.ron` (no code needed to change numbers).

- **Ground units** walk along great circles on the planet they stand on and cannot leave it.
  **Ships** fly through space, slide around planets and the sun, hover in low orbit when sent to
  a planet and can attack ground targets from orbit.
- **Drones** build anywhere on their planet. A **Colony Ship** flies to any planet and founds an
  **Outpost** there (expansion).
- Weapons say what they can hit (`ground`, `space`), their damage kind and splash (§5).

| Built in | Units (research needed) |
|---|---|
| Command Center / Outpost | Drone |
| Factory | Trooper, Scout Rover, Hover Tank, Flak Walker, Lancer (Guided Missiles), Siege Mortar (Advanced Ballistics) |
| Robotics Bay | Sentinel (Combat Robotics), Mender repair robot (Nanite Self-Repair), Wraith Infiltrator — cloaked (Active Camouflage), Juggernaut (Heavy Robotics + Plasma Containment) |
| Orbital Shipyard | Recon Probe, Fighter, Orbital Bomber, Colony Ship, Missile Frigate (Guided Missiles), Ion Disruptor (Ion Disruption), Surveyor (Deep-Space Scanning), Battlecruiser (Capital Ship Engineering + Plasma Containment) |

| Group | Buildings (research needed) |
|---|---|
| Economy | Command Center, Outpost, Extractor, Habitat Dome |
| Production | Factory, Robotics Bay (Combat Robotics), Orbital Shipyard |
| Research | Research Lab |
| Defence | Defence Turret (kinetic), Laser Turret (Focused Lasers), Missile Battery — anti-orbit (Guided Missiles), Bunker — armour 5 (Fortification Doctrine), Plasma Cannon (Plasma Containment), Shield Generator — shield dome (Deflector Shields) |
| Sensors | Radar Station, Seismic Sensor (Seismic Mapping), Deep-Space Array (Deep-Space Scanning), Gravimetric Beacon — reveals cloaked units (Gravimetric Detection) |

## 4. Research

`data/research.ron`: 38 techs in 7 fields (Weapons, Armour, Shields, Sensors, Robotics,
Propulsion, Economy), tiers 1–4.

- Researched in a **Research Lab**: one project at a time per lab, up to 5 queued, the cost is
  paid when queuing (refunded on cancel); several labs research in parallel. A tech needs its
  prerequisites (always from lower tiers) finished — or queued earlier in the same lab.
- A tech unlocks units / buildings (their `tech` lists) and/or has **effects**:
  `value = (base + Σ add) × Π mul` on a stat (hit points, armour, shields, damage, range, reload,
  splash, speed, every sensor range, aura strength, mining, income, housing) of everything whose
  tags match (`infantry`, `vehicle`, `robot`, `ship`, `building`, `defence`, …), optionally only
  for one damage kind. Hit point upgrades also heal by the difference.
- Upgrades never add a sensor that a unit does not have; they scale the ones it has.
- In the game: the **research tree** window (R or the top bar button) shows the tiers from left
  to right and one lane per field with the prerequisite paths; hovering a tech highlights its
  whole path, clicking queues it in a lab. A selected lab lists what it can start.

## 5. Combat

`data/combat.ron`. Every weapon has a **damage kind**, every target an **armour class**
(Light, Medium, Heavy, Structure), flat **armour** and optionally a **shield**.

| Kind | Good against | Weak against |
|---|---|---|
| Kinetic | light targets, shields | buildings, heavy armour |
| Laser | light targets (×1.3) | heavy armour, buildings |
| Explosive (usually splash) | buildings (×1.5), heavy armour | light targets |
| Plasma | heavy armour (×1.45) | shields |
| Ion | shields (×2.75) | bare hulls (×0.5) |

- A hit first drains the shield (damage × the kind's shield multiplier); the rest hits the hull:
  damage × class multiplier − armour, but never below 20 % of it. Armour values are small (0–5),
  so a point of armour matters most against fast, weak weapons (troopers barely scratch a Bunker).
- Shields recharge after 3 s without a hit. **Splash** deals 50 % to other enemies in the radius.
- **Auras**: the Shield Generator gives own buildings and ground units within 16 m up to +300
  shields; the Mender repairs 6 hp/s within 10 m (the strongest aura counts, no stacking).
  Nanite Self-Repair gives everything a slow self-repair.

## 6. Sensors, scouting and stealth

Every unit and building has **sensors** (ranges in metres, straight-line distance):

| Sensor | Meant to reveal |
|---|---|
| Sight | everything nearby, with full information (blocked by the horizon) |
| Radar | contacts on the surface and in low orbit beyond the horizon (no unit types) |
| Seismic | moving ground units on the same planet, through the horizon |
| Deep space | ships in open space, long range (probes, Surveyor, Deep-Space Array: 900 m) |
| Detection | stealthed units |

- **Fog of war itself is a separate feature (a teammate is building it).** The data, research
  upgrades, models and the client's range rings exist already; `Sim::sensors_of(id)` gives the
  effective ranges for the visibility code.
- **Stealth** already works in combat: the Wraith Infiltrator cannot be targeted (by units,
  defences or orders) unless a detector of that player is in range; firing reveals it for 2 s,
  so it holds fire unless ordered to attack. Splash still hurts it.
- Scouting kit: Scout Rover (sight + seismic + short detection), Recon Probe (fast, cheap,
  deep-space scanner), Surveyor (radar, deep space, detection), Infiltrator (cloaked), and the
  four sensor buildings.

## 7. Winning

A player without any core building (Command Center / Outpost) and without a colony ship is
defeated. The last player standing wins. Surrender and disconnecting count as defeat.

## 8. Ideas / roadmap

See [STATUS.md](STATUS.md).
