// Graph viewer (M10, `[data-gv]`): geser (seret) dan zoom (roda/pinch) kanvas SVG lewat viewBox,
// tombol "Reset tampilan", klik ganda node ber-`data-drill` = zoom masuk, `/` = fokus cari.
// Tanpa JS kanvas tetap bisa digulir dan semua aksi berupa tautan.
(() => {
  const MIN = 0.25, MAX = 4;

  const init = (root) => {
    if (root.dataset.gvReady) return;
    root.dataset.gvReady = "1";
    const stage = root.querySelector("[data-gv-stage]");
    const svg = stage?.querySelector("svg[data-gv-svg]");
    if (!stage || !svg) return;
    const vb0 = svg.viewBox.baseVal;
    const full = { x: vb0.x, y: vb0.y, w: vb0.width, h: vb0.height };
    stage.dataset.js = "";
    svg.removeAttribute("width");
    svg.removeAttribute("height");
    let view = { ...full };

    const fit = () => {
      // Sisakan ruang untuk overlay atas/bawah (banner, filter).
      const r = stage.getBoundingClientRect();
      const padTop = 56, padBottom = 64, padX = 24;
      const sx = (r.width - 2 * padX) / full.w;
      const sy = (r.height - padTop - padBottom) / full.h;
      const s = Math.min(Math.max(Math.min(sx, sy), MIN), 1.4);
      const w = r.width / s, h = r.height / s;
      view = {
        x: full.x + full.w / 2 - w / 2,
        y: full.y + full.h / 2 - h / 2 - (padTop - padBottom) / 2 / s,
        w, h,
      };
      apply();
    };
    const apply = () => svg.setAttribute("viewBox", `${view.x} ${view.y} ${view.w} ${view.h}`);
    const scale = () => stage.getBoundingClientRect().width / view.w;

    const zoomAt = (factor, cx, cy) => {
      const r = stage.getBoundingClientRect();
      const s = scale();
      const next = Math.min(Math.max(s * factor, MIN), MAX);
      const k = s / next;
      const px = view.x + (cx - r.left) / s;
      const py = view.y + (cy - r.top) / s;
      view = { x: px - (px - view.x) * k, y: py - (py - view.y) * k, w: view.w * k, h: view.h * k };
      apply();
    };

    stage.addEventListener("wheel", (e) => {
      e.preventDefault();
      zoomAt(Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015)), e.clientX, e.clientY);
    }, { passive: false });

    // Seret untuk geser; klik pendek tetap membuka tautan node.
    const pointers = new Map();
    let drag = null, moved = false, pinch = null;
    stage.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        pinch = { d: Math.hypot(a.x - b.x, a.y - b.y) };
        drag = null;
        return;
      }
      drag = { x: e.clientX, y: e.clientY, vx: view.x, vy: view.y };
      moved = false;
    });
    stage.addEventListener("pointermove", (e) => {
      if (!pointers.has(e.pointerId)) return;
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (pinch && pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        const d = Math.hypot(a.x - b.x, a.y - b.y);
        zoomAt(d / pinch.d, (a.x + b.x) / 2, (a.y + b.y) / 2);
        pinch.d = d;
        return;
      }
      if (!drag) return;
      const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
      if (!moved && Math.hypot(dx, dy) < 4) return;
      if (!moved) { moved = true; stage.setPointerCapture(e.pointerId); stage.dataset.dragging = ""; }
      const s = scale();
      view.x = drag.vx - dx / s;
      view.y = drag.vy - dy / s;
      apply();
    });
    const end = (e) => {
      pointers.delete(e.pointerId);
      if (pointers.size < 2) pinch = null;
      if (pointers.size === 0) { drag = null; delete stage.dataset.dragging; }
    };
    stage.addEventListener("pointerup", end);
    stage.addEventListener("pointercancel", end);
    // Klik node = pilih (tautan biasa, tetap lewat hx-boost), ditunda sebentar supaya klik ganda
    // bisa menjadi zoom masuk.
    const go = (href) => {
      const a = document.createElement("a");
      a.href = href;
      a.dataset.gvGo = "";
      a.hidden = true;
      stage.appendChild(a);
      a.click();
      a.remove();
    };
    let timer = null;
    stage.addEventListener("click", (e) => {
      if (moved) { e.preventDefault(); e.stopPropagation(); moved = false; return; }
      if (e.target.closest?.("[data-gv-go]")) return;
      const node = e.target.closest?.("a.app-gv-n");
      if (!node || e.ctrlKey || e.metaKey || e.shiftKey) return;
      e.preventDefault();
      e.stopPropagation();
      const href = node.getAttribute("href");
      clearTimeout(timer);
      timer = setTimeout(() => go(href), node.hasAttribute("data-drill") ? 260 : 0);
    }, true);
    stage.addEventListener("dblclick", (e) => {
      const node = e.target.closest?.("[data-drill]");
      if (!node) return;
      e.preventDefault();
      clearTimeout(timer);
      go(node.getAttribute("data-drill"));
    });
    stage.addEventListener("keydown", (e) => {
      const node = e.target.closest?.("a.app-gv-n[data-drill]");
      if (node && e.key === "Enter" && e.shiftKey) { e.preventDefault(); go(node.getAttribute("data-drill")); }
    });

    root.querySelector("[data-gv-fit]")?.addEventListener("click", fit);
    fit();
    new ResizeObserver(() => fit()).observe(stage);
  };

  document.addEventListener("keydown", (e) => {
    if (e.key !== "/" || e.ctrlKey || e.metaKey || e.altKey) return;
    const q = document.querySelector("[data-gv-q]");
    const tag = document.activeElement?.tagName;
    if (!q || /INPUT|TEXTAREA|SELECT/.test(tag) || document.activeElement?.isContentEditable) return;
    e.preventDefault();
    q.focus();
    q.select();
  });

  const scan = (root) => {
    const scope = root instanceof Element || root instanceof Document ? root : document;
    if (scope.matches?.("[data-gv]")) init(scope);
    scope.querySelectorAll?.("[data-gv]").forEach(init);
  };
  // htmx bisa sudah memproses <body> sebelum skrip ini terdaftar: pindai dokumen juga sekali.
  if (window.htmx?.onLoad) htmx.onLoad(scan);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => scan(document));
  else scan(document);
})();
