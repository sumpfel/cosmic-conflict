/* ==========================================================================
   Cosmic Conflict – 3D-Showroom
   Zeigt die echten Spielmodelle (assets/3d/, erzeugt von tools/sync_assets.py)
   mit denselben Material-Slots wie das Spiel.

   Einzelnes Modell:
     <div class="model-view" data-model="tank"></div>
       data-spin="0.3"   Drehgeschwindigkeit (Umdrehungen pro 10 s), 0 = steht
       data-team="6"     Spielerfarbe (Index in PLAYER_COLORS)
       data-angle="35"   Startwinkel in Grad
       data-pitch="18"   Kamerahöhe in Grad
       data-zoom="1.3"   größer/kleiner (Standard 1.15)
   Hangar (Liste + großes Modell + Werte):
     <div class="hangar" data-categories="ground,ship" data-keys="tank,fighter"> … </div>

   Alle Ansichten teilen sich EINEN WebGL-Canvas (fixiert über der Seite, ohne
   Mausereignisse); jede Ansicht wird per Scissor in ihren Rahmen gezeichnet.
   ========================================================================== */
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";

const BASE = "assets/3d/";
const DEFAULT_TEAM = 6;          // Orange – passt zur Akzentfarbe der Seite
const EMISSIVE_SCALE = 0.28;     // Bevy-Emissive (HDR) → three.js
const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

window.__ccShowroom = "starting";

/* ---------- Grundgerüst ---------- */
const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: "high-performance" });
// Canvas darf nicht größer als ~8192 px werden, sonst wird er gestreckt
const pixelRatio = () => Math.min(window.devicePixelRatio, 2, 8000 / Math.max(window.innerWidth, window.innerHeight));
renderer.setPixelRatio(pixelRatio());
renderer.setSize(window.innerWidth, window.innerHeight, false);
renderer.setClearColor(0x000000, 0);
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.05;
renderer.domElement.className = "showroom-canvas";
renderer.domElement.setAttribute("aria-hidden", "true");
document.body.appendChild(renderer.domElement);

const pmrem = new THREE.PMREMGenerator(renderer);
const envMap = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
const maxAniso = Math.min(8, renderer.capabilities.getMaxAnisotropy());

const manifest = await fetch(BASE + "manifest.json").then((r) => {
  if (!r.ok) throw new Error("manifest " + r.status);
  return r.json();
});
const byKey = new Map(manifest.models.map((m) => [m.key, m]));
const colors = manifest.playerColors;

/* ---------- Texturen & Materialien ---------- */
const texLoader = new THREE.TextureLoader();
const texCache = new Map();
function tex(url, srgb, channel = 0) {
  if (!texCache.has(url)) {
    const t = texLoader.load(url);
    t.flipY = false; // glTF-Konvention
    t.wrapS = t.wrapT = THREE.RepeatWrapping;
    t.anisotropy = maxAniso;
    t.colorSpace = srgb ? THREE.SRGBColorSpace : THREE.NoColorSpace;
    t.channel = channel;
    texCache.set(url, t);
  }
  return texCache.get(url);
}

const matCache = new Map();
function slotMaterial(slot, modelKey, team) {
  const spec = manifest.materials[slot] || manifest.materials.Hull;
  const usesTeam = spec && (spec.team || spec.teamGlow);
  const id = `${modelKey}|${slot}|${usesTeam ? team : "-"}`;
  if (matCache.has(id)) return matCache.get(id);

  const m = new THREE.MeshStandardMaterial({ envMapIntensity: 0.9 });
  if (spec) {
    const [r, g, b] = spec.team ? colors[team % colors.length] : spec.tint;
    m.color.setRGB(r / 255, g / 255, b / 255, THREE.SRGBColorSpace);
    m.roughness = spec.rough;
    m.metalness = spec.metal;
    if (spec.tex) {
      const dir = `${BASE}textures/${spec.tex}/`;
      m.map = tex(dir + "albedo.jpg", true);
      m.normalMap = tex(dir + "normal.jpg", false);
      m.roughnessMap = m.metalnessMap = tex(dir + "arm.jpg", false);
    }
    if (byKey.get(modelKey)?.ao && !spec.additive) {
      m.aoMap = tex(`${BASE}ao/${modelKey}.jpg`, false, 1);
    }
    let e = spec.emissive;
    if (spec.teamGlow) {
      const c = new THREE.Color().setRGB(...colors[team % colors.length].map((v) => v / 255), THREE.SRGBColorSpace);
      e = [0.6 + c.r * 6, 0.6 + c.g * 6, 0.6 + c.b * 6];
    }
    const peak = Math.max(...e);
    if (peak > 0) {
      m.emissive.setRGB(e[0] / peak, e[1] / peak, e[2] / peak, THREE.LinearSRGBColorSpace);
      m.emissiveIntensity = Math.min(peak * EMISSIVE_SCALE, 4);
    }
    if (spec.additive) {
      m.transparent = true;
      m.opacity = spec.alpha;
      m.blending = THREE.AdditiveBlending;
      m.depthWrite = false;
    }
  }
  matCache.set(id, m);
  return m;
}

const clayMat = new THREE.MeshStandardMaterial({ color: 0xbdb8ae, roughness: 0.85, metalness: 0 });
const wireMat = new THREE.MeshBasicMaterial({ color: 0xfa8c28, wireframe: true, transparent: true, opacity: 0.55 });

/* ---------- Modelle laden ---------- */
const gltfLoader = new GLTFLoader();
const modelCache = new Map();
function loadModel(key) {
  if (!modelCache.has(key)) {
    modelCache.set(key, gltfLoader.loadAsync(`${BASE}models/${key}.glb`).then((g) => g.scene));
  }
  return modelCache.get(key);
}

// Weicher Schatten unter dem Modell
const shadowTex = (() => {
  const c = document.createElement("canvas");
  c.width = c.height = 128;
  const g = c.getContext("2d");
  const grd = g.createRadialGradient(64, 64, 0, 64, 64, 64);
  grd.addColorStop(0, "rgba(0,0,0,0.55)");
  grd.addColorStop(1, "rgba(0,0,0,0)");
  g.fillStyle = grd;
  g.fillRect(0, 0, 128, 128);
  return new THREE.CanvasTexture(c);
})();

/* ---------- Eine Ansicht ---------- */
class View {
  constructor(el) {
    this.el = el;
    this.spin = reduceMotion ? 0 : parseFloat(el.dataset.spin ?? "0.35");
    this.team = parseInt(el.dataset.team ?? DEFAULT_TEAM, 10);
    this.angle = THREE.MathUtils.degToRad(parseFloat(el.dataset.angle ?? "35"));
    this.pitch = THREE.MathUtils.degToRad(parseFloat(el.dataset.pitch ?? "16"));
    this.zoom = parseFloat(el.dataset.zoom ?? "1.15");
    this.mode = "pbr";
    this.visible = false;
    this.lastInteract = 0;

    this.scene = new THREE.Scene();
    this.scene.environment = envMap;
    const key = new THREE.DirectionalLight(0xfff0dc, 2.4);
    key.position.set(5, 8, 6);
    const rim = new THREE.DirectionalLight(0x9fc2ff, 1.8);
    rim.position.set(-6, 3, -7);
    this.scene.add(key, rim, new THREE.HemisphereLight(0xc8d6ff, 0x1a1612, 0.35));

    this.pivot = new THREE.Group();
    this.scene.add(this.pivot);
    this.camera = new THREE.PerspectiveCamera(28, 1, 0.01, 1000);
    this.shadow = new THREE.Mesh(
      new THREE.PlaneGeometry(1, 1),
      new THREE.MeshBasicMaterial({ map: shadowTex, transparent: true, depthWrite: false })
    );
    this.shadow.rotation.x = -Math.PI / 2;
    this.scene.add(this.shadow);

    this.bindDrag();
    if (el.dataset.model) this.setModel(el.dataset.model);
  }

  async setModel(key) {
    this.key = key;
    const info = byKey.get(key);
    this.el.classList.remove("is-loaded", "is-missing");
    this.el.classList.add("is-loading");
    document.querySelectorAll(`[data-model-name="${this.el.id}"]`).forEach((n) => (n.textContent = info?.name ?? key));
    if (!info) {
      this.el.classList.replace("is-loading", "is-missing");
      this.el.dataset.status = `Modell „${key}“ fehlt – sync_assets.py ausführen`;
      return;
    }
    let src;
    try {
      src = await loadModel(key);
    } catch (err) {
      console.warn(err);
      this.el.classList.replace("is-loading", "is-missing");
      this.el.dataset.status = `Modell „${key}“ konnte nicht geladen werden`;
      return;
    }
    if (this.key !== key) return; // inzwischen anderes Modell gewählt
    const obj = src.clone(true);
    this.pivot.clear();
    this.pivot.rotation.set(0, this.angle, 0);
    this.pivot.add(obj);
    this.meshes = [];
    obj.traverse((o) => {
      if (o.isMesh) {
        o.userData.slot = o.name.split("_").pop();
        this.meshes.push(o);
      }
    });
    this.applyMaterials();
    this.frame(obj);
    this.el.classList.replace("is-loading", "is-loaded");
  }

  applyMaterials() {
    for (const o of this.meshes || []) {
      if (this.mode === "clay") o.material = clayMat;
      else if (this.mode === "wire") o.material = wireMat;
      else o.material = slotMaterial(o.userData.slot, this.key, this.team);
    }
  }

  setMode(mode) { this.mode = mode; this.applyMaterials(); }
  setTeam(team) { this.team = team; this.applyMaterials(); }

  frame(obj) {
    const box = new THREE.Box3().setFromObject(obj);
    const center = box.getCenter(new THREE.Vector3());
    obj.position.sub(center); // Drehachse = Mitte des Modells
    const sphere = box.getBoundingSphere(new THREE.Sphere());
    this.radius = sphere.radius;
    const s = this.radius * 2.1;
    this.shadow.scale.set(s, s, 1);
    this.shadow.position.y = box.min.y - center.y - this.radius * 0.02;
    this.updateCamera();
  }

  updateCamera() {
    if (!this.radius) return;
    const rect = this.el.getBoundingClientRect();
    const aspect = Math.max(rect.width, 1) / Math.max(rect.height, 1);
    this.camera.aspect = aspect;
    const vfov = THREE.MathUtils.degToRad(this.camera.fov);
    const hfov = 2 * Math.atan(Math.tan(vfov / 2) * aspect);
    const dist = (this.radius / Math.sin(Math.min(vfov, hfov) / 2)) / this.zoom;
    this.camera.position.set(0, Math.sin(this.pitch) * dist, Math.cos(this.pitch) * dist);
    this.camera.near = dist / 100;
    this.camera.far = dist * 10;
    this.camera.lookAt(0, 0, 0);
    this.camera.updateProjectionMatrix();
  }

  bindDrag() {
    let lastX = null;
    const el = this.el;
    el.addEventListener("pointerdown", (e) => {
      lastX = e.clientX;
      el.setPointerCapture(e.pointerId);
      el.classList.add("is-dragging");
    });
    el.addEventListener("pointermove", (e) => {
      if (lastX === null) return;
      this.pivot.rotation.y += (e.clientX - lastX) * 0.012;
      lastX = e.clientX;
      this.lastInteract = performance.now();
    });
    const end = () => { lastX = null; el.classList.remove("is-dragging"); this.lastInteract = performance.now(); };
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
  }

  tick(dt, now) {
    if (this.spin && now - this.lastInteract > 1500) {
      this.pivot.rotation.y += dt * this.spin * (Math.PI * 2) / 10;
    }
  }
}

/* ---------- Hangar: Liste + Modell + Werte ---------- */
const fmt = (n) => (n == null ? "–" : Number.isInteger(n) ? String(n) : n.toFixed(1));
const RES_DE = { metal: "Metall", crystal: "Kristall", gas: "Gas" };
const KIND_DE = { Kinetic: "Kinetisch", Laser: "Laser", Explosive: "Explosiv", Plasma: "Plasma", Ion: "Ionen" };

function setupHangar(root, viewsByEl) {
  const cats = (root.dataset.categories || "").split(",").filter(Boolean);
  const keys = (root.dataset.keys || "").split(",").filter(Boolean);
  const list = root.querySelector(".hangar-list");
  const stage = root.querySelector(".model-view");
  const specs = root.querySelector(".hangar-specs");
  const view = viewsByEl.get(stage);
  const models = keys.length
    ? keys.map((k) => byKey.get(k)).filter(Boolean)
    : manifest.models.filter((m) => cats.includes(m.category));
  const tabs = root.querySelectorAll("[data-filter]");

  function show(m) {
    view.setModel(m.key);
    list.querySelectorAll("button").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.key === m.key)));
    const cost = m.cost ? ["metal", "crystal", "gas"].filter((r) => m.cost[r]).map((r) => `${m.cost[r]} ${RES_DE[r]}`).join(" · ") : "–";
    const w = m.weapon;
    const rows = [
      ["Typ", m.categoryLabel],
      ["Hülle", fmt(m.hp)],
      ["Tempo", m.speed ? fmt(m.speed) + " m/s" : "–"],
      ["Waffe", w ? `${KIND_DE[w.kind] ?? w.kind ?? ""} ${fmt(w.damage)} / ${fmt(w.range)} m` : "–"],
      ["Ziele", w ? [w.ground && "Boden", w.space && "All"].filter(Boolean).join(" + ") : "–"],
      ["Kosten", cost],
    ];
    specs.innerHTML = `<h3 class="hangar-name">${m.name}</h3><dl class="spec">${rows
      .map(([k, v]) => `<div><dt>${k}</dt><dd>${v}</dd></div>`)
      .join("")}</dl>`;
  }

  function render(filter) {
    const shown = filter ? models.filter((m) => m.category === filter) : models;
    list.innerHTML = shown
      .map((m, i) => `<li><button type="button" data-key="${m.key}"><span class="num">${String(i + 1).padStart(2, "0")}</span>${m.name}</button></li>`)
      .join("");
    list.querySelectorAll("button").forEach((b) => b.addEventListener("click", () => show(byKey.get(b.dataset.key))));
    const start = shown.find((m) => m.key === root.dataset.start) || shown[0];
    if (start) show(start);
  }

  tabs.forEach((t) =>
    t.addEventListener("click", () => {
      tabs.forEach((x) => x.setAttribute("aria-selected", String(x === t)));
      render(t.dataset.filter);
    })
  );
  const active = [...tabs].find((t) => t.getAttribute("aria-selected") === "true");
  render(active ? active.dataset.filter : null);

  root.querySelectorAll("[data-team-swatch]").forEach((b) => {
    const c = colors[+b.dataset.teamSwatch % colors.length];
    b.style.setProperty("--swatch", `rgb(${c.join(",")})`);
    b.addEventListener("click", () => {
      root.querySelectorAll("[data-team-swatch]").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
      view.setTeam(+b.dataset.teamSwatch);
    });
  });
}

/* ---------- Ansichten anlegen (lazy) ---------- */
const views = [];
const viewsByEl = new Map();
const io = new IntersectionObserver(
  (entries) => entries.forEach((e) => { const v = viewsByEl.get(e.target); if (v) v.visible = e.isIntersecting; }),
  { rootMargin: "200px 0px" }
);

function makeView(el, lazyModel) {
  const model = el.dataset.model;
  if (lazyModel) delete el.dataset.model; // erst laden, wenn sichtbar
  const v = new View(el);
  views.push(v);
  viewsByEl.set(el, v);
  if (lazyModel && model) {
    const once = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) { once.disconnect(); v.setModel(model); }
    }, { rootMargin: "400px 0px" });
    once.observe(el);
  }
  io.observe(el);
  return v;
}

document.querySelectorAll(".hangar .model-view").forEach((el) => makeView(el, false));
document.querySelectorAll(".model-view:not(.hangar .model-view)").forEach((el) => makeView(el, true));
document.querySelectorAll(".hangar").forEach((h) => setupHangar(h, viewsByEl));

// Tabelle „Wer trifft wen“: <tbody data-target-matrix> wird aus den Spieldaten gefüllt
document.querySelectorAll("[data-target-matrix]").forEach((tbody) => {
  const armed = manifest.models.filter((m) => m.weapon);
  if (!armed.length) return;
  const yes = (v) => (v ? '<td class="yes">●</td>' : '<td class="no">–</td>');
  tbody.innerHTML = armed
    .map((m) => `<tr><th scope="row">${m.name}</th><td>${m.categoryLabel}</td><td>${fmt(m.weapon.damage)}</td>` +
      `<td>${fmt(m.weapon.range)} m</td>${yes(m.weapon.ground)}${yes(m.weapon.space)}</tr>`)
    .join("");
});

// Darstellungs-Umschalter: <button data-view-mode="clay" data-for="view-id">
document.querySelectorAll("[data-view-mode]").forEach((b) =>
  b.addEventListener("click", () => {
    const v = viewsByEl.get(document.getElementById(b.dataset.for));
    if (!v) return;
    v.setMode(b.dataset.viewMode);
    document.querySelectorAll(`[data-for="${b.dataset.for}"]`).forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
  })
);

/* ---------- Render-Schleife ---------- */
function resize() {
  renderer.setPixelRatio(pixelRatio());
  renderer.setSize(window.innerWidth, window.innerHeight, false);
  views.forEach((v) => v.updateCamera());
}
window.addEventListener("resize", resize);
new ResizeObserver(() => views.forEach((v) => v.updateCamera())).observe(document.body);

let last = performance.now();
renderer.setAnimationLoop((now) => {
  const dt = Math.min((now - last) / 1000, 0.1);
  last = now;
  const H = window.innerHeight;
  const W = window.innerWidth;
  renderer.setScissorTest(false);
  renderer.clear();
  renderer.setScissorTest(true);
  for (const v of views) {
    if (!v.visible || !v.radius) continue;
    v.tick(dt, now);
    const r = v.el.getBoundingClientRect();
    if (r.bottom < 0 || r.top > H || r.right < 0 || r.left > W || r.width === 0) continue;
    const y = H - r.bottom;
    renderer.setViewport(r.left, y, r.width, r.height);
    renderer.setScissor(r.left, y, r.width, r.height);
    renderer.render(v.scene, v.camera);
  }
});

document.documentElement.classList.add("has-3d");
window.__ccShowroom = "ready";
