// Kanvas graf builder workflow (`[data-wf-canvas="<id workflow>"]`, R3, prototype "Workflow
// builder"). Node stage = tautan HTML yang dirender server (ikon, nama, persona, gate); skrip ini
// menata posisinya lewat workflow-core.mjs dan menggambar SVG: panah maju + lencana transisi
// (tunggu approve / otomatis / manual), panah gagal (`gagal_ke`) merah putus-putus berlabel, dan
// kotak kelompok paralel. Kamera: seret latar (pointer) untuk menggeser, Ctrl/⌘ + gulir atau
// pinch untuk zoom di titik, gulir biasa untuk menggeser; tombol +/−/Rapikan; keyboard di kanvas:
// panah geser, + − zoom, 0 rapikan. Kamera disimpan sessionStorage per workflow sehingga memilih
// stage (navigasi biasa) tidak memindah pandangan. Tanpa JS node tampil berurutan seperti daftar.
// Pilihan tampilan Kanvas/Daftar diingat localStorage.
(() => {
  const SVGNS = "http://www.w3.org/2000/svg";
  let core = null;
  const loadCore = () => (core ??= import("/static/js/workflow-core.mjs"));
  const store = (s) => { try { return window[s]; } catch { return null; } };

  // Ingat pilihan Kanvas/Daftar; buka builder tanpa `view` → pakai pilihan terakhir.
  document.addEventListener("click", (e) => {
    const a = e.target.closest?.("[data-wf-view]");
    if (a) try { store("localStorage")?.setItem("ag-wf-view", a.dataset.wfView); } catch { /* abaikan */ }
  });

  const el = (name, attrs) => {
    const n = document.createElementNS(SVGNS, name);
    for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, v);
    return n;
  };

  async function init(root) {
    if (root.dataset.wfReady) return;
    root.dataset.wfReady = "1";
    try {
      const u = new URL(location.href);
      if (!u.searchParams.has("view") && store("localStorage")?.getItem("ag-wf-view") === "list") {
        u.searchParams.set("view", "list");
        location.replace(u);
        return;
      }
    } catch { /* abaikan */ }
    const W = await loadCore();
    const world = root.querySelector("[data-wf-world]");
    const svg = root.querySelector("[data-wf-edges]");
    const nodes = [...world.querySelectorAll("[data-wf-node]")];
    const icons = {};
    root.querySelector("template[data-wf-icons]")?.content.querySelectorAll("[data-i]").forEach((s) => { icons[s.dataset.i] = s.innerHTML; });
    const stages = nodes.map((n) => ({
      key: n.dataset.wfNode, exit: n.dataset.exit, parallel: n.hasAttribute("data-parallel"), failTo: n.dataset.failTo || "",
    }));
    const page = root.closest(".app-card") || document;
    const pct = page.querySelector("[data-wf-pct]");
    const key = `ag-wf-cam-${root.dataset.wfCanvas}`;
    let L = null;
    let cam = { x: 0, y: 0, k: 1 };

    const viewport = () => ({ w: root.clientWidth, h: root.clientHeight });
    const apply = () => {
      world.style.transform = `translate(${cam.x}px, ${cam.y}px) scale(${cam.k})`;
      if (pct) pct.textContent = `${Math.round(cam.k * 100)}%`;
      try { store("sessionStorage")?.setItem(key, JSON.stringify(cam)); } catch { /* abaikan */ }
    };
    const set = (c) => { cam = c; apply(); };

    const draw = () => {
      L = W.layout(stages);
      world.style.width = `${L.width}px`;
      world.style.height = `${L.height}px`;
      for (const n of nodes) {
        const p = L.pos[n.dataset.wfNode];
        n.style.left = `${p.x}px`;
        n.style.top = `${p.y}px`;
      }
      svg.setAttribute("width", L.width);
      svg.setAttribute("height", L.height);
      svg.setAttribute("viewBox", `0 0 ${L.width} ${L.height}`);
      svg.replaceChildren();
      const defs = el("defs", {});
      for (const [id, cls] of [["wf-arrow", "app-wf-arrow"], ["wf-arrow-fail", "app-wf-arrow-fail"]]) {
        const m = el("marker", { id, viewBox: "0 0 8 8", refX: "7", refY: "4", markerWidth: "7", markerHeight: "7", orient: "auto-start-reverse" });
        m.append(el("path", { d: "M0,0 L8,4 L0,8 Z", class: cls }));
        defs.append(m);
      }
      svg.append(defs);
      for (const g of L.groups) svg.append(el("rect", { x: g.x, y: g.y, width: g.w, height: g.h, rx: 14, class: "app-wf-group" }));
      for (const e of L.edges) svg.append(el("path", { d: e.d, class: "app-wf-edge", "marker-end": "url(#wf-arrow)" }));
      for (const f of L.fails) svg.append(el("path", { d: f.d, class: "app-wf-fail", "marker-end": "url(#wf-arrow-fail)" }));
      // Lencana transisi dan label (HTML di atas SVG).
      world.querySelectorAll("[data-wf-deco]").forEach((d) => d.remove());
      for (const b of L.badges) {
        const s = document.createElement("span");
        s.className = "app-wf-badge";
        s.dataset.wfDeco = "";
        s.dataset.exit = b.exit;
        s.setAttribute("aria-hidden", "true");
        s.innerHTML = icons[b.exit] || "";
        s.style.left = `${b.x}px`;
        s.style.top = `${b.y}px`;
        world.append(s);
      }
      for (const f of L.fails) {
        const n = nodes.find((x) => x.dataset.wfNode === f.from);
        const s = document.createElement("span");
        s.className = "app-wf-tag";
        s.dataset.wfDeco = "";
        s.dataset.tone = "danger";
        s.setAttribute("aria-hidden", "true");
        s.textContent = n?.dataset.failLabel || "";
        s.style.left = `${f.label.x}px`;
        s.style.top = `${f.label.y}px`;
        world.append(s);
      }
      for (const g of L.groups) {
        const s = document.createElement("span");
        s.className = "app-wf-tag";
        s.dataset.wfDeco = "";
        s.dataset.tone = "accent";
        s.setAttribute("aria-hidden", "true");
        s.textContent = root.dataset.parallelLabel || "";
        s.style.left = `${g.x + g.w - 8}px`;
        s.style.top = `${g.y}px`;
        s.dataset.anchor = "end";
        world.append(s);
      }
      root.setAttribute("data-wf-laid", "");
    };

    const fitAll = () => set(W.fit({ width: L.width, height: L.height }, viewport()));
    draw();
    let saved = null;
    try { saved = JSON.parse(store("sessionStorage")?.getItem(key) || "null"); } catch { saved = null; }
    if (saved && Number.isFinite(saved.k)) set({ x: saved.x, y: saved.y, k: W.clampZoom(saved.k) });
    else fitAll();
    // Stage terpilih harus terlihat.
    const current = nodes.find((n) => n.getAttribute("aria-current") === "true");
    if (current) set(W.focus(cam, L.pos[current.dataset.wfNode], viewport()));

    /* ---- input ---- */
    const center = () => { const v = viewport(); return [v.w / 2, v.h / 2]; };
    const zoomBy = (f, px, py) => { const [cx, cy] = center(); set(W.zoomAt(cam, f, px ?? cx, py ?? cy)); };
    page.querySelector("[data-wf-zoom-in]")?.addEventListener("click", () => zoomBy(1.2));
    page.querySelector("[data-wf-zoom-out]")?.addEventListener("click", () => zoomBy(1 / 1.2));
    page.querySelector("[data-wf-fit]")?.addEventListener("click", () => { draw(); fitAll(); });

    root.addEventListener("wheel", (e) => {
      e.preventDefault();
      const r = root.getBoundingClientRect();
      if (e.ctrlKey || e.metaKey) zoomBy(Math.exp(-e.deltaY * 0.0015), e.clientX - r.left, e.clientY - r.top);
      else set({ ...cam, x: cam.x - e.deltaX, y: cam.y - e.deltaY });
    }, { passive: false });

    const pointers = new Map();
    let pan = null;
    let pinch = null;
    root.addEventListener("pointerdown", (e) => {
      if (e.button !== 0 || e.target.closest("[data-wf-node]")) return;
      root.setPointerCapture(e.pointerId);
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (pointers.size === 1) pan = { x: e.clientX, y: e.clientY, cam: { ...cam } };
      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        pinch = { d: Math.hypot(a.x - b.x, a.y - b.y), cam: { ...cam } };
        pan = null;
      }
      root.setAttribute("data-panning", "");
    });
    root.addEventListener("pointermove", (e) => {
      if (!pointers.has(e.pointerId)) return;
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (pinch && pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        const r = root.getBoundingClientRect();
        const f = Math.hypot(a.x - b.x, a.y - b.y) / (pinch.d || 1);
        set(W.zoomAt(pinch.cam, f, (a.x + b.x) / 2 - r.left, (a.y + b.y) / 2 - r.top));
      } else if (pan) {
        set({ ...pan.cam, x: pan.cam.x + e.clientX - pan.x, y: pan.cam.y + e.clientY - pan.y });
      }
    });
    const up = (e) => {
      pointers.delete(e.pointerId);
      if (pointers.size < 2) pinch = null;
      if (pointers.size === 0) { pan = null; root.removeAttribute("data-panning"); }
    };
    root.addEventListener("pointerup", up);
    root.addEventListener("pointercancel", up);

    root.addEventListener("keydown", (e) => {
      if (e.target !== root && !e.target.closest("[data-wf-node]")) return;
      const step = 48;
      const moves = { ArrowLeft: [step, 0], ArrowRight: [-step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
      if (e.target === root && moves[e.key]) {
        e.preventDefault();
        set({ ...cam, x: cam.x + moves[e.key][0], y: cam.y + moves[e.key][1] });
      } else if (e.key === "+" || e.key === "=") { e.preventDefault(); zoomBy(1.2); }
      else if (e.key === "-" || e.key === "_") { e.preventDefault(); zoomBy(1 / 1.2); }
      else if (e.key === "0") { e.preventDefault(); draw(); fitAll(); }
    });
    // Node yang difokus lewat Tab dibawa ke layar.
    root.addEventListener("focusin", (e) => {
      const n = e.target.closest?.("[data-wf-node]");
      if (n) { root.scrollTop = 0; root.scrollLeft = 0; set(W.focus(cam, L.pos[n.dataset.wfNode], viewport())); }
    });
  }

  const scan = (rootEl) => {
    const found = rootEl instanceof Element && rootEl.matches("[data-wf-canvas]") ? [rootEl] : rootEl.querySelectorAll?.("[data-wf-canvas]") ?? [];
    found.forEach(init);
  };
  if (window.htmx?.onLoad) htmx.onLoad(scan);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => scan(document));
  else scan(document);
})();
