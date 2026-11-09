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

- **Ground units** (Drone, Trooper, Hover Tank, Flak Walker) walk along great circles on the
  planet they stand on. They cannot leave it.
- **Ships** (Fighter, Orbital Bomber, Colony Ship) fly through space, slide around planets and the
  sun, and hover in low orbit when sent to a planet. They can attack ground targets from orbit.
- **Drones** build anywhere on their planet. A **Colony Ship** flies to any planet and founds an
  **Outpost** there (expansion).
- Buildings: Command Center, Outpost, Extractor, Habitat Dome (+10 population), Factory (ground
  forces), Orbital Shipyard (ships, needs a Factory), Defence Turret (hits ground and low orbit).
- Weapons say what they can hit (`ground`, `space`): troopers and tanks only hit the surface,
  flak only hits ships, fighters and turrets hit both, bombers only bombard the surface.

## 4. Winning

A player without any core building (Command Center / Outpost) and without a colony ship is
defeated. The last player standing wins. Surrender and disconnecting count as defeat.

## 5. Ideas / roadmap

See [STATUS.md](STATUS.md).
