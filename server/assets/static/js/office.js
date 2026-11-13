// AI Office (M10, `[data-office]`): memuat mesin kanvas (office-engine.mjs) hanya di halaman ini,
// mempolling `data-state` tiap 3 dtk (berhenti saat tab tersembunyi), dan menghubungkan kanvas
// dengan HTML di sekitarnya: tab ruangan, ringkasan status, tombol zoom/nama, dan panel samping
// (`data-panel?c=` diganti tanpa muat ulang; URL ikut `?c=`). Tanpa JS halaman tetap berisi
// panel ringkasan dan tautan biasa.
(() => {
  const POLL = 3000;

  /* Ikon SVG di <template> → daftar path `d` untuk Path2D (dada robot, gelembung ide). */
  const svgPaths = (svg) => {
    const out = [];
    svg.querySelectorAll("path, circle, ellipse, rect, line, polyline, polygon").forEach((el) => {
      if (el.getAttribute("stroke") === "none") return;
      const a = (n) => parseFloat(el.getAttribute(n) || "0");
      switch (el.tagName.toLowerCase()) {
        case "path": out.push(el.getAttribute("d")); break;
        case "circle": { const r = a("r"); out.push(`M${a("cx") - r} ${a("cy")}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0`); break; }
        case "ellipse": { const rx = a("rx"), ry = a("ry"); out.push(`M${a("cx") - rx} ${a("cy")}a${rx} ${ry} 0 1 0 ${2 * rx} 0a${rx} ${ry} 0 1 0 ${-2 * rx} 0`); break; }
        case "rect": { const x = a("x"), y = a("y"), w = a("width"), h = a("height"); out.push(`M${x} ${y}h${w}v${h}h${-w}z`); break; }
        case "line": out.push(`M${a("x1")} ${a("y1")}L${a("x2")} ${a("y2")}`); break;
        default: { const p = (el.getAttribute("points") || "").trim(); if (p) out.push("M" + p.replace(/\s+/g, " ").replace(/ (?=[\d.-]+,)/g, "L") + (el.tagName.toLowerCase() === "polygon" ? "z" : "")); }
      }
    });
    return out;
  };

  const init = async (root) => {
    if (root.dataset.officeReady) return;
    root.dataset.officeReady = "1";
    const stage = root.querySelector("[data-office-stage]");
    const side = root.querySelector("[data-office-side]");
    if (!stage || !side) return;

    const i18n = {};
    root.querySelector("template[data-office-i18n]")?.content.querySelectorAll("[data-k]").forEach((el) => { i18n[el.dataset.k] = el.textContent.trim(); });
    const iconSvg = {}, iconMap = {};
    root.querySelector("template[data-office-icons]")?.content.querySelectorAll("[data-i]").forEach((el) => {
      iconSvg[el.dataset.i] = el.innerHTML;
      const svg = el.querySelector("svg"); if (svg) iconMap[el.dataset.i] = svgPaths(svg);
    });

    let E;
    try { E = await import("/static/js/office-engine.mjs"); } catch { return; }
    E.setIcons(iconMap);

    const home = root.dataset.home, panelBase = root.dataset.panel;
    let selected = root.dataset.selected || null;
    let state = null;

    /* ---- panel samping ---- */
    const paintPanel = () => {
      side.querySelectorAll("canvas[data-office-pv]").forEach((cv) => {
        const ch = office.char(cv.dataset.officePv) || state?.chars.find((c) => c.id === cv.dataset.officePv);
        cv.width = 104; cv.height = 116;
        if (ch) E.paintPreview(cv, ch, 1.6);
      });
      lookPreview();
    };
    /* Pratinjau dialog Kustom karakter mengikuti isian form secara langsung. */
    const lookPreview = () => {
      const form = side.querySelector("form[data-office-look]");
      const cv = form?.querySelector("canvas[data-office-look-pv]");
      if (!cv) return;
      const base = office.char(cv.dataset.officeLookPv) || state?.chars.find((c) => c.id === cv.dataset.officeLookPv);
      if (!base) return;
      const v = (n) => form.querySelector(`[name="${n}"]:checked`)?.value ?? form.querySelector(`[name="${n}"]`)?.value ?? "";
      const look = { ...base.look };
      ["shirt", "skin", "hair"].forEach((k) => { const x = v(k); if (/^#[0-9a-fA-F]{6}$/.test(x)) look[k] = x; });
      ["acc", "style"].forEach((k) => { const x = v(k); if (x) look[k] = x; });
      E.paintPreview(cv, { ...base, look }, 6.2);
    };
    side.addEventListener("input", (e) => { if (e.target.closest("form[data-office-look]")) lookPreview(); });
    side.addEventListener("change", (e) => { if (e.target.closest("form[data-office-look]")) lookPreview(); });

    let panelReq = 0;
    const loadPanel = async (id) => {
      const n = ++panelReq;
      try {
        const res = await fetch(`${panelBase}?c=${encodeURIComponent(id || "")}`, { headers: { "HX-Request": "true" }, credentials: "same-origin" });
        if (!res.ok || n !== panelReq) { if (res.status === 404 && id) select(null); return; }
        const html = await res.text();
        if (n !== panelReq) return;
        side.innerHTML = html;
        window.htmx?.process?.(side);
        paintPanel();
      } catch { /* jaringan putus: biarkan panel lama */ }
    };
    const select = (id, focus) => {
      selected = id || null;
      office.select(selected);
      const url = new URL(location.href);
      if (selected) url.searchParams.set("c", selected); else url.searchParams.delete("c");
      history.replaceState(history.state, "", url);
      if (selected && focus) office.focusChar(selected);
      loadPanel(selected);
    };
    side.addEventListener("click", (e) => {
      const a = e.target.closest("[data-office-sel]");
      if (!a || e.metaKey || e.ctrlKey || e.shiftKey || e.button) return;
      e.preventDefault();
      select(a.dataset.officeSel || null, true);
    });

    /* ---- kanvas ---- */
    const tabs = root.parentElement.querySelector("[data-office-tabs]") || document.querySelector("[data-office-tabs]");
    const setRoom = (k) => {
      office.room(k);
      tabs?.querySelectorAll("[data-office-room]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.officeRoom === k)));
    };
    const office = E.mount({
      stage,
      labels: stage.querySelector("[data-office-labels]"),
      mm: stage.querySelector("[data-office-mm]"),
      i18n,
      selected,
      iconSvg: (n) => iconSvg[n] || "",
      onSelect: (id, focus) => select(id, focus),
      onRoom: (k) => setRoom(k),
    });
    tabs?.addEventListener("click", (e) => { const b = e.target.closest("[data-office-room]"); if (b) setRoom(b.dataset.officeRoom); });
    stage.querySelectorAll("[data-office-zoom]").forEach((b) => b.addEventListener("click", () => {
      const k = b.dataset.officeZoom;
      if (k === "in") office.zoom(1.25); else if (k === "out") office.zoom(1 / 1.25); else if (k === "100") office.zoomTo(1); else { setRoom("all"); office.fit(); }
    }));
    const names = stage.querySelector("[data-office-names]");
    names?.addEventListener("click", () => { const on = names.getAttribute("aria-pressed") !== "true"; names.setAttribute("aria-pressed", String(on)); office.names(on); });

    /* ---- data ---- */
    const sumEl = stage.querySelector("[data-office-sum]");
    const applyCounts = (d) => {
      sumEl?.querySelectorAll("[data-office-c]").forEach((b) => { const st = b.dataset.officeC; b.textContent = d.counts?.[st] ?? 0; });
      if (!tabs) return;
      const total = tabs.querySelector('[data-office-n="all"]'); if (total) total.textContent = d.chars.length;
      d.rooms.forEach((r) => { const el = tabs.querySelector(`[data-office-n="${CSS.escape(r.k)}"]`); if (el) el.textContent = r.count; });
    };
    let first = true, timer = 0, failures = 0;
    const poll = async () => {
      clearTimeout(timer);
      if (!stage.isConnected) return;
      if (document.hidden) { timer = setTimeout(poll, POLL); return; }
      try {
        const res = await fetch(root.dataset.state, { headers: { Accept: "application/json" }, credentials: "same-origin" });
        if (res.ok) {
          state = await res.json(); failures = 0;
          office.setState(state);
          applyCounts(state);
          if (first) {
            first = false;
            if (selected) { office.select(selected); office.focusChar(selected); }
            paintPanel();
          }
        } else failures++;
      } catch { failures++; }
      timer = setTimeout(poll, failures ? Math.min(30000, POLL * 2 ** failures) : POLL);
    };
    document.addEventListener("visibilitychange", () => { if (!document.hidden && stage.isConnected) poll(); });
    poll();
  };

  const scan = (el) => {
    const root = el instanceof Element ? el : document;
    if (root.matches?.("[data-office]")) init(root);
    root.querySelectorAll?.("[data-office]").forEach(init);
  };
  // htmx bisa sudah memproses <body> sebelum skrip ini terdaftar: pindai dokumen juga sekali.
  if (window.htmx?.onLoad) htmx.onLoad(scan);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => scan(document));
  else scan(document);
})();
