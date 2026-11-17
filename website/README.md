# Cosmic Conflict – Website

Statische Website (HTML + Bootstrap 5.3 + three.js), keine Build-Tools nötig. Die 3D-Modelle
sind die echten Modelle aus dem Spiel und werden im Browser in Echtzeit gerendert.

## Öffnen

Die 3D-Modelle werden per `fetch` geladen – das funktioniert **nur über einen Webserver**, nicht
mit Doppelklick auf die HTML-Datei (dann steht in den Modell-Rahmen ein Hinweis).

```sh
cd website
python3 -m http.server 8000     # → http://localhost:8000
```

Bootstrap, three.js und die Schriften (Archivo, IBM Plex Mono) kommen per CDN – ohne Internet
fehlen sie.

## Aufbau

```
website/
├── index.html          Start
├── download.html       Download
├── erklaerung.html     Erklärung (Übersicht)
├── konzept.html          ├─ Konzept        ┐
├── entwicklung.html      ├─ Entwicklung    │ im Menü unter „Erklärung“
├── combat.html           ├─ Combat         │
├── hosting.html          └─ Hosting        ┘
├── community.html      Community
├── team.html           Team
├── css/style.css       eigenes Stylesheet (Farben & Schriften ganz oben unter :root)
├── js/main.js          Bild-Slots, Jahr im Footer
├── js/showroom.js      3D-Ansichten (three.js)
├── assets/3d/          Modelle, Texturen, manifest.json – ERZEUGT, nicht von Hand ändern
├── tools/sync_assets.py  kopiert die Modelle aus dem Spiel nach assets/3d/
└── img/                eigene Bilder → Liste aller Bildplätze in img/README.md
```

Navigation und Footer stehen in **jeder** HTML-Datei. Wer dort etwas ändert, muss die Änderung
in alle 9 Dateien übernehmen (Suchen & Ersetzen im Editor).

## Gestaltung

Technisches Datenblatt statt Hochglanz: Haarlinien, eckige Kanten, nummerierte Abschnitte,
Monospace für Daten und genau eine Signalfarbe (`--signal`, Orange = Spielerfarbe 7 im Spiel).

| Baustein | Klasse | Wofür |
|---|---|---|
| Abschnitt | `sec` + `sec-head` (`sec-label` + `sec-title`) | jede Sektion: links Nummer + Name, rechts Überschrift |
| Abbildung | `fig` (mit `figcaption`) | Bild mit Schnittmarken an den Ecken und Bildunterschrift |
| Spalten | `cols` mit `style="--n: 4"` | nummerierte Spalten mit Trennlinien |
| Zeilen-Liste | `rows` + `row-link` (`has-thumb` = mit Vorschaubild) | Themen, News, Kanäle, Events |
| Daten | `spec` (dl), `tbl` (Tabelle), `ticks` (Liste), `legend` (nummeriert) | |
| Aufklappen | `qa` mit `<details>` | FAQ, Changelog |
| Buttons | `btn btn-signal`, `btn btn-line`, Zusatz `arrow` | |

## Bilder einfügen

```html
<div class="media r-43 bg-slot" data-bg="img/start/intro.jpg" role="img" aria-label="…"></div>
```

- Datei mit genau diesem Namen in den Ordner legen → wird automatisch angezeigt.
- Fehlt sie, sieht man eine Schraffur und den Dateinamen. Alle Hinweise ausblenden:
  `<body class="… hide-slot-labels">`.
- Alle 84 Bildplätze mit empfohlenen Größen: [img/README.md](img/README.md).
- Seitenverhältnis: `r-video` (16:9), `r-wide` (21:9), `r-43`, `r-square`, `r-portrait` (4:5).
- Abdunkeln für Text darauf: `shade`, `shade-left`, `shade-head`, `shade-foot`.

## 3D-Modelle

### Einbauen

```html
<div class="model-view" data-model="tank"></div>
```

| Attribut | Standard | Bedeutung |
|---|---|---|
| `data-model` | – | Modellname, z. B. `tank`, `battlecruiser`, `command_center`, `deposit_crystal` |
| `data-spin` | `0.35` | Drehgeschwindigkeit (Umdrehungen pro 10 s), `0` = steht still |
| `data-angle` | `35` | Startwinkel in Grad |
| `data-pitch` | `16` | Kamerahöhe in Grad |
| `data-zoom` | `1.15` | größer / kleiner |
| `data-team` | `6` | Spielerfarbe 0–7 (wie im Spiel) |

Alle Modellnamen stehen in `assets/3d/manifest.json`. Man kann die Modelle mit der Maus drehen.

Weitere Bausteine (Beispiele in den Seiten):

- **Hangar** (Start, Combat): Liste + großes Modell + Werte aus den Spieldaten.
  `<div class="hangar" data-keys="tank,fighter">` (feste Auswahl) oder
  `data-categories="ground,ship,defence,sensors"` (alle einer Kategorie, mit Tabs).
- **Spielerfarben**: Buttons `data-team-swatch="0…7"` im Hangar.
- **Darstellung umschalten** (Entwicklung): `<button data-view-mode="pbr|clay|wire" data-for="ID">`.
- **Tabelle „Wer trifft wen“** (Combat): `<tbody data-target-matrix>` wird aus den Spieldaten gefüllt.

### Modelle aktualisieren

Wenn sich Modelle oder Werte im Spiel ändern (neue Einheiten, `data/*.ron`):

```sh
python3 website/tools/sync_assets.py     # braucht Pillow: pip install pillow
```

Das Skript kopiert `crates/game/assets/models/*.glb`, verkleinert AO-Maps und Texturen für das
Web, liest Namen und Werte aus `data/units.ron` und `data/buildings.ron` und schreibt
`assets/3d/manifest.json`. Die Website zeigt die Modelle mit denselben Material-Slots
(`materials.ron`) wie das Spiel.

Lizenzen: Die Modelle gehören zum Projekt (GPL-3.0-or-later), die Texturen sind CC0 – Details in
`assets/3d/models/CREDITS.md` und `assets/3d/textures/CREDITS.md`.

## Farben & Schriften ändern

Ganz oben in `css/style.css` unter `:root` – z. B. `--signal` (Orange), `--ink` (Hintergrund),
`--paper` (Text). Die Modellfarbe im 3D-Viewer ist die Spielerfarbe (`data-team`).

## Vor dem Veröffentlichen

- Alle `href="#"` durch echte Links ersetzen (Downloads, Social Media, Impressum, Datenschutz).
- In Österreich braucht eine öffentliche Website ein Impressum (Offenlegung nach § 25 MedienG).
- `img/shared/og-image.jpg` ist das Vorschaubild beim Teilen in Discord, WhatsApp usw.
- `assets/3d/` (~20 MB) muss mit hochgeladen werden, sonst fehlen die Modelle.
