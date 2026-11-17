/* ==========================================================================
   Cosmic Conflict – Website-Skript
   1. Bild-Slots laden (data-bg)
   2. Hinweis, wenn die 3D-Ansicht nicht starten kann
   3. Aktuelles Jahr im Footer
   Die 3D-Modelle selbst: js/showroom.js
   ========================================================================== */
(() => {
  "use strict";

  /* 1. Bild-Slots
     <div class="bg-slot" data-bg="img/start/hero.jpg"></div>
     Klappt das Laden, wird das Bild als Hintergrund gesetzt; fehlt es, bekommt
     das Element .slot-empty und zeigt den erwarteten Dateinamen. */
  document.querySelectorAll("[data-bg]").forEach((el) => {
    const src = el.dataset.bg;
    if (!src) return;
    const img = new Image();
    img.onload = () => {
      el.style.setProperty("--bg-img", `url("${src}")`);
      el.classList.add("slot-loaded");
    };
    img.onerror = () => el.classList.add("slot-empty");
    img.src = src;
  });

  /* 2. 3D: Modelle werden per fetch geladen – das geht nicht über file:// */
  const views = document.querySelectorAll(".model-view");
  if (views.length) {
    const fail = (msg) => {
      if (window.__ccShowroom === "ready") return;
      document.documentElement.classList.add("no-3d");
      views.forEach((v) => v.dataset.status || (v.dataset.status = msg));
    };
    if (location.protocol === "file:") {
      fail("3D-Ansicht braucht einen Webserver – siehe README");
    } else {
      setTimeout(() => fail("3D-Ansicht konnte nicht starten"), 8000);
    }
  }

  /* 3. Jahr */
  document.querySelectorAll("[data-year]").forEach((el) => {
    el.textContent = new Date().getFullYear();
  });
})();
