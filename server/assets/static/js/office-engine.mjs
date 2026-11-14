// AI Office (R5): panggung SVG (Isometrik Studio / Denah) yang dimuat lazy oleh office.js.
// Kamera = transform <g> (geser, zoom, pinch, keyboard, mini-map `<use>`), dunia digambar
// office-svg.mjs dan hanya ruangan yang isinya berubah yang digambar ulang tiap polling.
// Label nama/ruangan adalah tombol HTML berurutan logis (bisa difokus dan dibaca screen
// reader), posisinya dihitung office-core.mjs `declutter` supaya tidak bertumpuk.
// Tema gelap = token CSS (office.css), tanpa gambar ulang. Animasi hanya lewat CSS dan
// dimatikan oleh `prefers-reduced-motion`.
import * as C from "./office-core.mjs";
import { S, defs, iso, plan, preview as drawPreview } from "./office-svg.mjs";

export const { readMode, writeMode } = C;
/** Pratinjau karakter di `<svg>` (panel detail, dialog Kustom). */
export const preview = drawPreview;

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (m) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[m]);

/**
 * `opts`: { stage, labels, mm, i18n, selected, mode, iconSvg(name), onSelect(id, focus), onRoom(k) }.
 * API: setState, select, char, fit, zoom, zoomTo, room, focusChar, names, mode, destroy.
 */
export function mount(opts) {
  const { stage, labels: lab, mm, i18n } = opts;
  const svg = stage.querySelector("[data-office-svg]");
  const tr = (k, n) => (i18n[k] || k).replace("{n}", n ?? "");
  let mode = opts.mode === "plan" ? "plan" : "iso";
  const R = () => (mode === "plan" ? plan : iso);
  stage.dataset.mode = mode;

  defs(S("defs", {}, svg));
  const camG = S("g", { cls: "of-cam" }, svg);
  let world = null, roomsG = null, built = "";
  let L = C.layout([]), P = { seats: {}, hidden: [], overflow: {} }, sigs = {}, RG = {}, B = { x0: 0, y0: 0, x1: 1, y1: 1 };
  const st = { chars: [], sel: opts.selected || null, room: "all", names: true, focus: null };
  const cam = { z: 1, tx: 0, ty: 0 };
  let W = 0, H = 0, TOP = 56, BOT = 52, moved = false, anim = 0, raf = 0, alive = true;
  const zMin = () => Math.min(0.3, C.fitView(B, W, H, { top: TOP, bottom: BOT }).z * 0.9);
  const Z_MAX = 2.6;

  /* ---------------------------------------------------------------- gambar */
  const build = (full) => {
    if (full) {
      world?.remove();
      world = S("g", { id: "of-world", cls: "of-world" }, camG);
      R().base(S("g", {}, world), L);
      roomsG = S("g", {}, world);
      R().top?.(S("g", {}, world), L);
      RG = {}; sigs = {};
      B = R().bounds(L);
      built = L.sig + mode;
      setupMini();
    }
    const next = C.roomSigs(st.chars, P);
    L.rooms.forEach((r) => {
      const s = next[r.k] || "";
      if (RG[r.k] && sigs[r.k] === s) return;
      const occ = st.chars.filter((c) => P.seats[c.id]?.room === r.k).map((c) => ({ c, seat: P.seats[c.id] }));
      const out = R().room(r, occ, L, opts.iconSvg);
      if (RG[r.k]) RG[r.k].g.replaceWith(out.g); else roomsG.appendChild(out.g);
      RG[r.k] = out; sigs[r.k] = s;
    });
    markSel();
    drawDots();
  };
  const anchor = (id) => { const s = P.seats[id]; return s && RG[s.room]?.chars[id]; };
  const markSel = () => {
    world?.querySelectorAll(".of-ch").forEach((g) => g.classList.toggle("sel", g.dataset.ch === st.sel));
  };

  /* ---------------------------------------------------------------- label */
  const LB = new Map(), RL = new Map();
  const charName = (c) => (c.bot ? tr("agent", c.name) : c.name);
  const tagHtml = (c) => {
    const v = C.statusView(c.st);
    const badge = v.badge ? `<span class="b" data-tone="${v.badge}">${opts.iconSvg?.(v.icon) || ""}<span>${esc(tr("st-" + v.st))}</span></span>` : "";
    const tk = c.tk && v.st !== "idle" ? `<span class="tk">${esc(c.tk)}</span>` : "";
    return `${badge}<span class="n"><i></i><span class="nm">${esc(c.short || c.name)}</span>${tk}</span>`;
  };
  const roomName = (r) => (r.kind === "meeting" ? tr("room-meeting") : r.kind === "pantry" ? tr("room-pantry") : r.kind === "lounge" ? tr("room-lounge") : r.n);
  const roomSubText = (r) => {
    const here = st.chars.filter((c) => P.seats[c.id]?.room === r.k);
    const s = C.roomSub(r, here);
    let t = esc(tr(s.k, s.n));
    if (s.todo) t += ` · <b>${esc(tr("sub-todo", s.todo))}</b>`;
    if (P.overflow[r.k]) t += ` · ${esc(tr("sub-more", P.overflow[r.k]))}`;
    return t;
  };
  const syncLabels = () => {
    const seen = new Set(), want = [];
    // Urutan DOM = urutan Tab: per ruangan (urutan denah), lalu urutan kursi.
    const order = st.chars.filter((c) => P.seats[c.id]).sort((a, b) => {
      const ra = L.rooms.indexOf(L.byKey[P.seats[a.id].room]), rb = L.rooms.indexOf(L.byKey[P.seats[b.id].room]);
      return ra - rb || P.seats[a.id].y - P.seats[b.id].y || P.seats[a.id].x - P.seats[b.id].x;
    });
    order.forEach((c) => {
      seen.add(c.id);
      let el = LB.get(c.id);
      if (!el) {
        el = document.createElement("button");
        el.type = "button";
        el.dataset.officeLabel = c.id;
        el.addEventListener("click", () => opts.onSelect?.(c.id, true));
        el.addEventListener("focus", () => focusTag(c.id, true));
        el.addEventListener("blur", () => focusTag(c.id, false));
        LB.set(c.id, el);
      }
      const v = C.statusView(c.st), sig = tagHtml(c);
      if (el._sig !== sig) {
        el._sig = sig; el._w = 0;
        el.innerHTML = sig;
        el.className = `app-of-tag st-${v.st}`;
        el.setAttribute("aria-label", `${charName(c)}, ${tr("st-" + v.st)}${c.tk ? ", " + c.tk : ""}`);
      }
      el._c = c;
      want.push(el);
    });
    for (const [id, el] of LB) if (!seen.has(id)) { el.remove(); LB.delete(id); }
    L.rooms.forEach((r) => {
      let el = RL.get(r.k);
      if (!el) {
        el = document.createElement("button");
        el.type = "button";
        el.className = "app-of-rl";
        el.dataset.officeRoomLabel = r.k;
        el.addEventListener("click", () => opts.onRoom?.(r.k));
        RL.set(r.k, el);
      }
      const sig = `<span class="t"><i style="background:${esc(r.c)}"></i>${esc(roomName(r))}</span><small>${roomSubText(r)}</small>`;
      if (el._sig !== sig) { el._sig = sig; el._w = 0; el.innerHTML = sig; el.setAttribute("aria-label", tr("focus", roomName(r))); }
      el.classList.toggle("on", st.room === r.k);
      want.push(el);
    });
    for (const [k, el] of RL) if (!L.byKey[k]) { el.remove(); RL.delete(k); }
    // Susun ulang hanya bila urutan berubah (memindahkan elemen yang sedang fokus membuatnya blur).
    const cur = [...lab.children];
    if (cur.length !== want.length || want.some((el, i) => cur[i] !== el)) {
      const active = document.activeElement;
      want.forEach((el) => lab.appendChild(el));
      if (active && lab.contains(active) && document.activeElement !== active) active.focus({ preventScroll: true });
    }
    markLabels();
  };
  const markLabels = () => { for (const [id, el] of LB) { el.classList.toggle("sel", id === st.sel); el.setAttribute("aria-pressed", String(id === st.sel)); } };

  let hud = null;
  const hudRects = () => {
    const now = performance.now();
    if (hud && now - hud.t < 500) return hud.r;
    const o = stage.getBoundingClientRect();
    hud = { t: now, r: [...stage.querySelectorAll(".app-of-ov > *, .app-of-hint:not(.gone)")].map((e) => e.getBoundingClientRect()).filter((q) => q.width > 0).map((q) => ({ l: q.left - o.left, t: q.top - o.top, w: q.width, h: q.height })) };
    return hud.r;
  };
  const measure = (el, show) => {
    const key = el._sig + show;
    if (el._w && el._mk === key) return;
    el.classList.remove("clip");
    el.dataset.show = show;
    el._w = el.offsetWidth; el._h = el.offsetHeight || 22; el._mk = key;
  };
  const placeLabels = () => {
    const lod = C.lodOf(cam.z, mode);
    stage.dataset.lod = lod;
    const items = [];
    for (const [id, el] of LB) {
      const a = anchor(id), c = el._c;
      const pinned = id === st.sel || id === st.focus;
      let show = C.tagShows(lod, c.st, pinned);
      if (!st.names && !pinned && show === "full" && !C.statusView(c.st).badge) show = "none";
      el.dataset.show = show;
      if (!a || show === "none") { el.classList.add("clip"); continue; }
      measure(el, show);
      const v = C.statusView(c.st);
      items.push({ id, el, x: a[0] * cam.z + cam.tx, y: a[1] * cam.z + cam.ty, w: el._w, h: el._h, mode: "tag", prio: pinned ? 3 : v.act ? 2 : 0, maxShift: pinned || v.act ? Infinity : 90 });
    }
    for (const [k, el] of RL) {
      const o = RG[k];
      if (!o) { el.classList.add("clip"); continue; }
      measure(el, lod);
      items.push({ id: "rm:" + k, el, x: o.label[0] * cam.z + cam.tx, y: o.label[1] * cam.z + cam.ty, w: el._w, h: el._h, mode: o.pos, prio: 2.5, hideOnHit: true, maxShift: 40, alt: o.alt && { x: o.alt.at[0] * cam.z + cam.tx, y: o.alt.at[1] * cam.z + cam.ty, mode: o.alt.mode } });
    }
    const out = C.declutter(items, { obstacles: hudRects(), view: { w: W, h: H } });
    out.forEach((p, i) => {
      const el = items[i].el;
      el.classList.toggle("clip", p.hidden);
      if (!p.hidden) el.style.transform = `translate(${Math.round(p.l)}px,${Math.round(p.t)}px)`;
    });
  };
  /* Fokus keyboard pada label: sorot karakter dan pastikan terlihat. */
  const focusTag = (id, on) => {
    world?.querySelectorAll(".of-ch").forEach((g) => g.classList.toggle("focus", on && g.dataset.ch === id));
    st.focus = on ? id : st.focus === id ? null : st.focus;
    if (!on) { schedule(); return; }
    const a = anchor(id);
    if (!a) return;
    const sx = a[0] * cam.z + cam.tx, sy = a[1] * cam.z + cam.ty;
    if (sx < 60 || sx > W - 60 || sy < TOP + 30 || sy > H - BOT - 10) go({ z: cam.z, tx: W / 2 - a[0] * cam.z, ty: H * 0.55 - a[1] * cam.z }, 260);
    else schedule();
  };

  /* ---------------------------------------------------------------- mini-map */
  let mmUse = null, mmDots = null, mmView = null, mmBox = null;
  function setupMini() {
    if (!mm) return;
    mm.replaceChildren();
    const pad = 20;
    mmBox = { x: B.x0 - pad, y: B.y0 - pad, w: B.x1 - B.x0 + pad * 2, h: B.y1 - B.y0 + pad * 2 };
    mm.setAttribute("viewBox", `${mmBox.x} ${mmBox.y} ${mmBox.w} ${mmBox.h}`);
    mmUse = S("use", { href: "#of-world", cls: "of-mm-world" }, mm);
    mmDots = S("g", {}, mm);
    mmView = S("rect", { cls: "of-mm-view", rx: mmBox.w / 120 }, mm);
  }
  function drawDots() {
    if (!mmDots) return;
    mmDots.replaceChildren();
    st.chars.forEach((c) => {
      const s = P.seats[c.id]; if (!s) return;
      const [x, y] = R().at(s);
      S("circle", { cx: x, cy: y - (mode === "iso" ? 20 : 0), r: mmBox.w / 70, cls: `of-mm-dot st-${C.statusView(c.st).st}` }, mmDots);
    });
  }
  function drawMiniView() {
    if (!mmView) return;
    const x = -cam.tx / cam.z, y = -cam.ty / cam.z, w = W / cam.z, h = H / cam.z;
    mmView.setAttribute("x", x); mmView.setAttribute("y", y);
    mmView.setAttribute("width", Math.max(1, w)); mmView.setAttribute("height", Math.max(1, h));
    mmView.style.strokeWidth = `${mmBox.w / 160}px`;
    const all = x <= B.x0 + 4 && y <= B.y0 + 4 && x + w >= B.x1 - 4 && y + h >= B.y1 - 4;
    mm.parentElement?.classList.toggle("mm-hide", all);
  }
  if (mm) {
    let down = false;
    const goMini = (e) => {
      const r = mm.getBoundingClientRect(), s = Math.min(r.width / mmBox.w, r.height / mmBox.h);
      const ox = (r.width - mmBox.w * s) / 2, oy = (r.height - mmBox.h * s) / 2;
      const wx = mmBox.x + (e.clientX - r.left - ox) / s, wy = mmBox.y + (e.clientY - r.top - oy) / s;
      moved = true; cancelAnimationFrame(anim);
      Object.assign(cam, { tx: W / 2 - wx * cam.z, ty: H / 2 - wy * cam.z }); schedule();
    };
    mm.addEventListener("pointerdown", (e) => { e.stopPropagation(); down = true; mm.setPointerCapture(e.pointerId); goMini(e); });
    mm.addEventListener("pointermove", (e) => { if (down) goMini(e); });
    mm.addEventListener("pointerup", () => { down = false; });
  }

  /* ---------------------------------------------------------------- kamera */
  const pct = stage.querySelector("[data-office-pct]");
  const apply = () => {
    raf = 0;
    if (!alive) return;
    camG.setAttribute("transform", `translate(${cam.tx.toFixed(2)} ${cam.ty.toFixed(2)}) scale(${cam.z.toFixed(4)})`);
    if (pct) pct.textContent = `${Math.round(cam.z * 100)}%`;
    placeLabels();
    drawMiniView();
  };
  const schedule = () => { if (!raf) raf = requestAnimationFrame(apply); };
  const go = (t, ms = 420) => {
    cancelAnimationFrame(anim);
    if (reduced() || !ms) { Object.assign(cam, t); schedule(); return; }
    const f = { ...cam }, t0 = performance.now();
    const step = (now) => {
      const k = Math.min(1, (now - t0) / ms), e = 1 - Math.pow(1 - k, 3);
      cam.z = f.z + (t.z - f.z) * e; cam.tx = f.tx + (t.tx - f.tx) * e; cam.ty = f.ty + (t.ty - f.ty) * e;
      apply();
      if (k < 1 && alive) anim = requestAnimationFrame(step);
    };
    anim = requestAnimationFrame(step);
  };
  const fitOf = (b, pad = 0.94) => C.fitView(b, W, H, { pad, top: TOP, bottom: BOT, min: zMin(), max: Z_MAX });
  /* Tampilan awal: seluruh kantor; layar sempit yang membuat kantor terlalu kecil → kolom pertama. */
  const home = () => {
    const f = fitOf(B);
    if (f.z >= 0.3 || !L.rooms.length) return f;
    const first = L.rooms.filter((r) => r.c0 === 0).map((r) => RG[r.k]?.box).filter(Boolean);
    if (!first.length) return f;
    const b = { x0: Math.min(...first.map((q) => q.x0)), y0: Math.min(...first.map((q) => q.y0)), x1: Math.max(...first.map((q) => q.x1)), y1: Math.max(...first.map((q) => q.y1)) };
    return fitOf(b, 0.96);
  };
  const zoomAt = (k, sx = W / 2, sy = H / 2, ms = 0) => { moved = true; hideHint(); go(C.zoomAround(cam, k, sx, sy, zMin(), Z_MAX), ms); };
  const pan = (dx, dy) => { moved = true; hideHint(); cancelAnimationFrame(anim); cam.tx += dx; cam.ty += dy; schedule(); };
  const hint = stage.querySelector("[data-office-hint]");
  const hideHint = () => { if (hint && !hint.classList.contains("gone")) { hint.classList.add("gone"); hud = null; schedule(); } };
  const hintTimer = setTimeout(hideHint, 8000);

  /* ---------------------------------------------------------------- input */
  const ptrs = new Map();
  let down = null, pinch = null;
  stage.addEventListener("pointerdown", (e) => {
    if (e.button || e.target.closest(".app-of-ov, .app-of-tag, .app-of-rl")) return;
    try { stage.setPointerCapture(e.pointerId); } catch { /* */ }
    ptrs.set(e.pointerId, [e.clientX, e.clientY]);
    cancelAnimationFrame(anim);
    if (ptrs.size === 2) { const [a, b] = [...ptrs.values()]; pinch = { d: Math.hypot(a[0] - b[0], a[1] - b[1]) || 1, z: cam.z }; down = null; return; }
    down = { x: e.clientX, y: e.clientY, moved: false, target: e.target };
  });
  stage.addEventListener("pointermove", (e) => {
    if (!ptrs.has(e.pointerId)) return;
    const prev = ptrs.get(e.pointerId);
    ptrs.set(e.pointerId, [e.clientX, e.clientY]);
    const r = stage.getBoundingClientRect();
    if (pinch && ptrs.size === 2) {
      const [a, b] = [...ptrs.values()], d = Math.hypot(a[0] - b[0], a[1] - b[1]);
      zoomAt((pinch.z * (d / pinch.d)) / cam.z, (a[0] + b[0]) / 2 - r.left, (a[1] + b[1]) / 2 - r.top);
      return;
    }
    if (!down) return;
    if (!down.moved && Math.hypot(e.clientX - down.x, e.clientY - down.y) > 4) { down.moved = true; stage.classList.add("drag"); }
    if (down.moved) pan(e.clientX - prev[0], e.clientY - prev[1]);
  });
  const up = (e) => {
    if (down && !down.moved && ptrs.has(e.pointerId)) {
      const g = down.target.closest?.("[data-ch]");
      opts.onSelect?.(g ? g.dataset.ch : null, false);
    }
    ptrs.delete(e.pointerId);
    if (ptrs.size < 2) pinch = null;
    down = null;
    stage.classList.remove("drag");
  };
  stage.addEventListener("pointerup", up);
  stage.addEventListener("pointercancel", up);
  stage.addEventListener("wheel", (e) => {
    e.preventDefault();
    const r = stage.getBoundingClientRect();
    if (e.ctrlKey || e.metaKey) zoomAt(Math.exp(-e.deltaY * 0.0022), e.clientX - r.left, e.clientY - r.top);
    else pan(-e.deltaX, -e.deltaY);
  }, { passive: false });
  stage.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && st.sel) { opts.onSelect?.(null, false); e.preventDefault(); return; }
    if (e.target !== stage) return;
    const s = 60, k = e.key;
    if (k === "ArrowLeft") pan(s, 0); else if (k === "ArrowRight") pan(-s, 0);
    else if (k === "ArrowUp") pan(0, s); else if (k === "ArrowDown") pan(0, -s);
    else if (k === "+" || k === "=") zoomAt(1.2, W / 2, H / 2, 200);
    else if (k === "-") zoomAt(1 / 1.2, W / 2, H / 2, 200);
    else if (k === "0") api.fit();
    else return;
    e.preventDefault();
  });

  /* ---------------------------------------------------------------- ukuran */
  /* Area pandang = panggung dikurangi overlay atas (ringkasan, sakelar) dan bawah (zoom). */
  const size = () => {
    const r = stage.getBoundingClientRect();
    W = Math.max(1, r.width); H = Math.max(1, r.height); hud = null;
    const edge = (sel, f) => [...stage.querySelectorAll(sel)].map((e) => e.getBoundingClientRect()).filter((q) => q.height > 0).map(f);
    TOP = Math.max(16, ...edge(".app-of-ov[data-pos='tl'] > *, .app-of-ov[data-pos='tr'] > *", (q) => (q.top - r.top < H / 2 ? q.bottom - r.top + 6 : 0)));
    BOT = Math.max(16, ...edge(".app-of-ov[data-pos='br'] > *", (q) => r.bottom - q.top + 6));
  };
  const ro = new ResizeObserver(() => {
    size();
    if (!moved) go(home(), 0); else schedule();
  });
  ro.observe(stage);
  size();

  const api = {
    destroy() { alive = false; clearTimeout(hintTimer); ro.disconnect(); cancelAnimationFrame(anim); cancelAnimationFrame(raf); },
    setState(data) {
      const next = C.layout(data.rooms || []);
      st.chars = data.chars || [];
      const full = next.sig + mode !== built;
      L = next;
      P = C.place(st.chars, L, full ? null : P);
      build(full);
      syncLabels();
      if (full && !moved) go(home(), 0); else schedule();
    },
    select(id) { st.sel = id || null; markSel(); markLabels(); schedule(); },
    char(id) { return st.chars.find((c) => c.id === id); },
    fit() { moved = true; st.room = "all"; RL.forEach((el) => el.classList.remove("on")); go(fitOf(B)); },
    zoom(f) { zoomAt(f, W / 2, H / 2, 220); },
    zoomTo(z) { zoomAt(z / cam.z, W / 2, H / 2, 220); },
    room(k) {
      st.room = k;
      RL.forEach((el, key) => el.classList.toggle("on", key === k));
      moved = true;
      go(k === "all" || !RG[k] ? fitOf(B) : fitOf(RG[k].box, 0.86));
    },
    focusChar(id) {
      const a = anchor(id);
      if (!a) return;
      const sx = a[0] * cam.z + cam.tx, sy = a[1] * cam.z + cam.ty;
      if (cam.z >= 0.85 && sx > 80 && sx < W - 80 && sy > TOP + 40 && sy < H - BOT - 40) return;
      const z = Math.min(Z_MAX, Math.max(cam.z, mode === "plan" ? 1.1 : 1.25));
      moved = true;
      go({ z, tx: W / 2 - a[0] * z, ty: H * 0.55 - a[1] * z });
    },
    names(on) { st.names = !!on; stage.classList.toggle("nonames", !on); schedule(); },
    mode(m) {
      const next = m === "plan" ? "plan" : "iso";
      if (next === mode) return;
      mode = next;
      stage.dataset.mode = mode;
      moved = false;
      P = C.place(st.chars, L, null);
      build(true);
      syncLabels();
      go(home(), 0);
    },
    get current() { return mode; },
  };
  return api;
}
