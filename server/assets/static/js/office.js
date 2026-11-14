// AI Office (`[data-office]`, R5): memuat panggung SVG (office-engine.mjs) hanya di halaman ini,
// mempolling `data-state` tiap 3 dtk (berhenti saat tab tersembunyi), dan menghubungkan panggung
// dengan HTML di sekitarnya: tab ruangan, ringkasan status, tombol zoom/label, sakelar
// Isometrik/Denah (disimpan per user di localStorage), dan panel samping (`data-panel?c=`
// diganti tanpa muat ulang; URL ikut `?c=`). Tanpa JS halaman tetap berisi panel ringkasan.
(() => {
  const POLL = 3000;

  const init = async (root) => {
    if (root.dataset.officeReady) return;
    root.dataset.officeReady = "1";
    const stage = root.querySelector("[data-office-stage]");
    const side = root.querySelector("[data-office-side]");
    if (!stage || !side) return;

    const i18n = {};
    root.querySelector("template[data-office-i18n]")?.content.querySelectorAll("[data-k]").forEach((el) => { i18n[el.dataset.k] = el.textContent.trim(); });
    const iconSvg = {};
    root.querySelector("template[data-office-icons]")?.content.querySelectorAll("[data-i]").forEach((el) => { iconSvg[el.dataset.i] = el.innerHTML; });

    let E;
    try { E = await import("/static/js/office-engine.mjs"); } catch { return; }

    const me = root.dataset.me || "";
    const store = (() => { try { return window.localStorage; } catch { return null; } })();
    const panelBase = root.dataset.panel;
    let selected = root.dataset.selected || null;
    let state = null;

    /* ---- panel samping ---- */
    const charOf = (id) => office.char(id) || state?.chars.find((c) => c.id === id);
    const paintPanel = () => {
      side.querySelectorAll("svg[data-office-pv]").forEach((svg) => { const ch = charOf(svg.dataset.officePv); if (ch) E.preview(svg, ch); });
      lookPreview();
    };
    /* Pratinjau dialog Kustom karakter mengikuti isian form secara langsung. */
    const lookPreview = () => {
      const form = side.querySelector("form[data-office-look]");
      const svg = form?.querySelector("svg[data-office-look-pv]");
      if (!svg) return;
      const base = charOf(svg.dataset.officeLookPv);
      if (!base) return;
      const v = (n) => form.querySelector(`[name="${n}"]:checked`)?.value ?? form.querySelector(`[name="${n}"]`)?.value ?? "";
      const look = { ...base.look };
      ["shirt", "skin", "hair"].forEach((k) => { const x = v(k); if (/^#[0-9a-fA-F]{6}$/.test(x)) look[k] = x; });
      ["acc", "style"].forEach((k) => { const x = v(k); if (x) look[k] = x; });
      E.preview(svg, { ...base, look });
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

    /* ---- panggung ---- */
    const tabs = document.querySelector("[data-office-tabs]");
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
      mode: E.readMode(store, me),
      iconSvg: (n) => iconSvg[n] || "",
      onSelect: (id, focus) => select(id, focus),
      onRoom: (k) => setRoom(k),
    });
    tabs?.addEventListener("click", (e) => { const b = e.target.closest("[data-office-room]"); if (b) setRoom(b.dataset.officeRoom); });
    stage.querySelectorAll("[data-office-zoom]").forEach((b) => b.addEventListener("click", () => {
      const k = b.dataset.officeZoom;
      if (k === "in") office.zoom(1.25); else if (k === "out") office.zoom(1 / 1.25); else if (k === "100") office.zoomTo(1); else setRoom("all");
    }));
    const names = stage.querySelector("[data-office-names]");
    names?.addEventListener("click", () => { const on = names.getAttribute("aria-pressed") !== "true"; names.setAttribute("aria-pressed", String(on)); office.names(on); });
    const modes = stage.querySelectorAll("[data-office-mode]");
    const markMode = () => modes.forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.officeMode === office.current)));
    modes.forEach((b) => b.addEventListener("click", () => {
      office.mode(b.dataset.officeMode);
      E.writeMode(store, me, office.current);
      markMode();
      tabs?.querySelectorAll("[data-office-room]").forEach((t) => t.setAttribute("aria-pressed", String(t.dataset.officeRoom === "all")));
    }));
    markMode();

    /* ---- data ---- */
    const sumEl = stage.querySelector("[data-office-sum]");
    const applyCounts = (d) => {
      sumEl?.querySelectorAll("[data-office-c]").forEach((b) => { b.textContent = d.counts?.[b.dataset.officeC] ?? 0; });
      if (!tabs) return;
      const total = tabs.querySelector('[data-office-n="all"]'); if (total) total.textContent = d.chars.length;
      d.rooms.forEach((r) => { const el = tabs.querySelector(`[data-office-n="${CSS.escape(r.k)}"]`); if (el) el.textContent = r.count; });
    };
    let first = true, timer = 0, failures = 0;
    const poll = async () => {
      clearTimeout(timer);
      if (!stage.isConnected) { office.destroy(); return; }
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
