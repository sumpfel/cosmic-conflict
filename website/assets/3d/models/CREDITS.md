# 3D model credits

All models in this folder (`<key>.glb`, `<key>_ao.png`, `materials.ron`) are **generated** for
Cosmic Conflict by `tools/blender/build_models.py`, `tools/blender/models_arsenal.py` (research-tree
units, defences and sensors) and `tools/blender/cc_kit.py`. Geometry, panel
details, rocks and crystals are procedural (seeded Python + Blender's bevel, weighted-normal
and Cycles AO-bake tools). No third-party meshes are used. They are part of this project and
share its licence (GPL-3.0-or-later, see `LICENSE`).

Their surfaces use these **CC0 1.0 (public domain)** photo textures, mapped per material slot
(full list with authors and links in [`../textures/CREDITS.md`](../textures/CREDITS.md)):

| Slot | Texture | Author(s) | Source |
|---|---|---|---|
| Hull | Blue Metal Plate (`blue_metal_plate`, desaturated) | Rob Tuytel | [Poly Haven](https://polyhaven.com/a/blue_metal_plate) |
| Team | Green Metal Rust (`green_metal_rust`, desaturated, tinted with the player colour) | Rob Tuytel | [Poly Haven](https://polyhaven.com/a/green_metal_rust) |
| Concrete | Concrete Floor Worn 001 | Dimitrios Savva, Rico Cilliers | [Poly Haven](https://polyhaven.com/a/concrete_floor_worn_001) |
| Facade | Concrete Wall 004 | Charlotte Baglioni, Dario Barresi | [Poly Haven](https://polyhaven.com/a/concrete_wall_004) |
| Sheet | Corrugated Iron 02 (desaturated) | Jenelle van Heerden, Sergej Majboroda | [Poly Haven](https://polyhaven.com/a/corrugated_iron_02) |
| OreRock | Tiger Rock | Dario Barresi | [Poly Haven](https://polyhaven.com/a/tiger_rock) |
| Rock | Dark Rock | Amal Kumar | [Poly Haven](https://polyhaven.com/a/dark_rock) |
| Panel, Metal, Dark, Ore, Rubber, Hazard, Deck | Metal Plates 006, Metal 009, Metal 032, Metal 062 C, Rubber 004, Painted Metal 016, Diamond Plate 008 C | Lennart Demes | [ambientCG](https://ambientcg.com) |

Glass, Window, Glow, Engine, Light, Plasma, Laser, Ion, Crystal, Gas, Haze and Shield are plain
materials without textures.

The preview renders (`tools/blender/preview.py`) light the models with the CC0 HDRI
*Photo Studio 01* by Sergej Majboroda ([Poly Haven](https://polyhaven.com/a/photo_studio_01)),
which is downloaded to the git-ignored `assets_src/` and not shipped with the game.

Made with [Blender](https://www.blender.org) 5.2 (GPL; files made with Blender are not covered
by Blender's licence).
