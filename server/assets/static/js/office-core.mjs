// AI Office (R5): logika murni tanpa DOM, dipakai office-engine.mjs dan diuji
// server/tests/js/office_core.test.mjs. Isinya: status → tampilan, tata letak ruangan dari
// kolom server, kursi per jenis ruangan + penempatan karakter yang stabil antar polling,
// penghindaran tumpukan label HTML, LOD zoom, matematika kamera, dan kunci mode per user.
// Satuan dunia = ubin (tile); renderer Isometrik/Denah memproyeksikan sendiri.

/* ---------------------------------------------------------------- status */
export const STATUSES = ["working", "review", "decision", "meeting", "idle", "online", "paused"];
const SV = {
  working: { icon: "terminal-2", badge: null, act: false, halo: true },
  review: { icon: "eye", badge: "warn", act: true, halo: true },
  decision: { icon: "alert-triangle", badge: "danger", act: true, halo: true },
  meeting: { icon: "message-2", badge: null, act: false, halo: true },
  idle: { icon: "coffee", badge: null, act: false, halo: false },
  online: { icon: "user", badge: null, act: false, halo: true },
  paused: { icon: "player-pause", badge: "muted", act: false, halo: true },
};
/** Ikon yang harus tersedia di `<template data-office-icons>`. */
export const STATUS_ICONS = [...new Set(Object.values(SV).map((s) => s.icon))];
/** Tampilan status: badge (`warn`/`danger`/`muted`/null), ikon, butuh tindakan, lingkaran lantai. */
export function statusView(st) {
  const known = Object.hasOwn(SV, st);
  return { ...(known ? SV[st] : SV.idle), st: known ? st : "idle" };
}
const PRIO = { decision: 0, review: 1, working: 2, paused: 3, meeting: 4, online: 5, idle: 6 };
/** Urgensi (kecil = lebih penting): yang tampil dulu saat ruangan penuh. */
export const priority = (st) => PRIO[st] ?? 7;

/* ---------------------------------------------------------------- tata letak */
/** Ukuran ruangan (ubin) dan lorong antar-ruangan. */
export const RW = 11, RD = 9, GAP = 1.4;

/** Ruangan server `{k, kind, n, c, col, front}` → grid kolom × (belakang, depan). */
export function layout(rooms) {
  const R = (rooms || []).map((r) => {
    const c0 = Math.max(0, r.col | 0), r0 = r.front ? 1 : 0;
    const x0 = c0 * (RW + GAP), y0 = r0 * (RD + GAP);
    return { ...r, c0, r0, x0, y0, x1: x0 + RW, y1: y0 + RD };
  }).sort((a, b) => a.c0 + a.r0 - (b.c0 + b.r0) || a.c0 - b.c0);
  const cols = R.length ? Math.max(...R.map((r) => r.c0)) + 1 : 0;
  const rows = R.length ? Math.max(...R.map((r) => r.r0)) + 1 : 0;
  return {
    rooms: R,
    byKey: Object.fromEntries(R.map((r) => [r.k, r])),
    cols,
    rows,
    W: cols ? cols * RW + (cols - 1) * GAP : 0,
    H: rows ? rows * RD + (rows - 1) * GAP : 0,
    sig: JSON.stringify(R.map((r) => [r.k, r.kind, r.n, r.c, r.c0, r.r0])),
  };
}

/* ---------------------------------------------------------------- kursi */
export const POSES = ["desk", "chair", "sofa", "stand", "talk", "mug"];
/* Koordinat lokal ruangan (ubin). Urutan = urutan isi. `desk` = indeks meja (monitor menyala). */
const SEATS = {
  project: [
    ["d0", 2.35, 2.55, "desk", 0], ["d2", 6.75, 2.55, "desk", 2], ["d1", 4.35, 2.55, "desk", 1], ["d3", 8.75, 2.55, "desk", 3],
    ["s0", 8.9, 6.0, "sofa"], ["s1", 7.4, 6.0, "sofa"], ["b0", 1.9, 7.2, "stand"],
    ["x0", 3.6, 4.9, "stand"], ["x1", 5.0, 4.9, "stand"], ["x2", 4.4, 8.0, "stand"], ["x3", 10.0, 4.0, "stand"],
  ],
  meeting: [
    ["m0", 5.5, 2.9, "chair"], ["m1", 7.1, 2.9, "chair"], ["m2", 3.9, 2.9, "chair"],
    ["t0", 2.2, 1.5, "talk"], ["t1", 3.2, 1.5, "talk"], ["t2", 4.2, 1.5, "talk"],
    ["x0", 9.0, 5.0, "stand"], ["x1", 1.8, 6.8, "stand"], ["x2", 5.5, 6.9, "stand"], ["x3", 8.8, 6.8, "stand"],
  ],
  pantry: [
    ["g0", 2.6, 1.9, "mug"], ["s0", 8.6, 5.8, "sofa"], ["s1", 7.4, 5.8, "sofa"],
    ["k0", 1.6, 4.4, "stand"], ["k1", 3.0, 6.4, "stand"], ["k2", 4.5, 5.2, "stand"], ["f0", 7.2, 2.0, "mug"],
    ["x0", 9.6, 2.6, "stand"], ["x1", 5.2, 2.6, "stand"], ["x2", 1.4, 7.4, "stand"], ["x3", 4.8, 7.8, "stand"], ["x4", 9.6, 7.2, "stand"],
  ],
  lounge: [
    ["s0", 2.2, 2.6, "sofa"], ["s1", 3.6, 2.6, "sofa"], ["s2", 7.3, 5.8, "sofa"], ["s3", 8.7, 5.8, "sofa"],
    ["x0", 5.0, 4.0, "stand"], ["x1", 3.0, 6.8, "stand"],
  ],
};
const SEAT_CACHE = {};
/** Kursi lokal satu ruangan: `{id, x, y, pose, desk?}`. */
export function seatsFor(room) {
  const kind = SEATS[room.kind] ? room.kind : "lounge";
  return (SEAT_CACHE[kind] ||= SEATS[kind].map(([id, x, y, pose, desk]) => ({ id, x, y, pose, desk: desk ?? null })));
}
/* Urutan kursi yang dicoba per karakter: di Ruang rapat manusia duduk, agent berdiri di layar. */
const order = (room, c) => {
  const seats = seatsFor(room);
  if (room.kind !== "meeting") return seats;
  const first = c.bot ? "talk" : "chair";
  return [...seats.filter((s) => s.pose === first), ...seats.filter((s) => s.pose !== first)];
};

/**
 * Tempatkan karakter di kursi. `prev` = hasil sebelumnya (kursi lama dipertahankan selama
 * karakter masih di ruangan yang sama). Ruangan penuh: yang paling mendesak tampil.
 * Hasil `{ seats: {charId: {id, room, pose, desk, x, y}}, hidden: [charId], overflow: {room: n} }`;
 * `x`/`y` = koordinat dunia (ubin).
 */
export function place(chars, L, prev) {
  const out = { seats: {}, hidden: [], overflow: {} };
  const groups = new Map();
  (chars || []).forEach((c, i) => {
    if (!L.byKey[c.room]) { out.hidden.push(c.id); return; }
    if (!groups.has(c.room)) groups.set(c.room, []);
    groups.get(c.room).push({ c, i });
  });
  for (const [k, list] of groups) {
    const room = L.byKey[k], cap = seatsFor(room).length;
    list.sort((a, b) => priority(a.c.st) - priority(b.c.st) || a.i - b.i);
    const shown = list.slice(0, cap), rest = list.slice(cap);
    if (rest.length) {
      out.overflow[k] = rest.length;
      rest.forEach(({ c }) => out.hidden.push(c.id));
    }
    const taken = new Set(), at = {};
    const valid = new Set(seatsFor(room).map((s) => s.id));
    shown.forEach(({ c }) => {
      const p = prev?.seats?.[c.id];
      if (p && p.room === k && valid.has(p.local) && !taken.has(p.local)) { taken.add(p.local); at[c.id] = p.local; }
    });
    shown.forEach(({ c }) => {
      if (at[c.id]) return;
      const s = order(room, c).find((x) => !taken.has(x.id));
      taken.add(s.id); at[c.id] = s.id;
    });
    shown.forEach(({ c }) => {
      const s = seatsFor(room).find((x) => x.id === at[c.id]);
      out.seats[c.id] = { id: `${k}:${s.id}`, local: s.id, room: k, pose: s.pose, desk: s.desk, x: room.x0 + s.x, y: room.y0 + s.y };
    });
  }
  return out;
}

/** Tanda tangan isi gambar per ruangan: ruangan digambar ulang hanya bila berubah. */
export function roomSigs(chars, P) {
  const by = {};
  (chars || []).forEach((c) => {
    const s = P.seats[c.id];
    if (!s) return;
    (by[s.room] ||= []).push([c.id, s.local, c.st, !!c.bot, c.icon || "", c.short || c.name || "", c.look || null]);
  });
  const out = {};
  for (const k in by) out[k] = JSON.stringify(by[k].sort((a, b) => (a[0] < b[0] ? -1 : 1)));
  return out;
}

/* ---------------------------------------------------------------- label */
/**
 * Penghindaran tumpukan label HTML (koordinat layar, px). Item:
 * `{id, x, y, w, h, mode: "tag"|"above"|"below", prio?, hideOnHit?}`; `x,y` = jangkar
 * (tag: tengah-bawah label; above: label di atas titik; below: di bawah titik).
 * Prio tinggi ditempatkan dulu (tidak digeser); sisanya digeser menjauh (ke atas, atau ke
 * bawah untuk `below`) dan disembunyikan bila tetap bertabrakan atau geser > `maxShift`.
 * `hideOnHit`: sembunyikan bila menabrak `obstacles` (HUD) atau keluar `view`.
 * Hasil: `[{id, l, t, w, h, hidden}]` dengan urutan masukan.
 */
export function declutter(items, { gap = 3, obstacles = [], view = null, maxShift = Infinity } = {}) {
  const all = items.map((a, i) => {
    const t = a.mode === "below" ? a.y + 8 : a.mode === "above" ? a.y - a.h - 6 : a.y - a.h;
    return { ...a, i, l: a.x - a.w / 2, t, t0: t, prio: a.prio || 0, hidden: false };
  });
  const hit = (a, b) => a.l < b.l + b.w + gap && b.l < a.l + a.w + gap && a.t < b.t + b.h + gap && b.t < a.t + a.h + gap;
  const key = (a) => (a.mode === "below" ? a.t : -(a.t + a.h));
  const placed = [];
  [...all].sort((a, b) => b.prio - a.prio || key(a) - key(b) || a.i - b.i).forEach((a) => {
    const down = a.mode === "below";
    for (let n = 0; n < 24; n++) {
      const o = placed.find((b) => hit(a, b));
      if (!o) break;
      a.t = down ? o.t + o.h + gap : o.t - a.h - gap;
    }
    if (placed.some((b) => hit(a, b)) || Math.abs(a.t - a.t0) > maxShift) a.hidden = true;
    if (!a.hidden && a.hideOnHit) {
      const out = view && (a.l < 0 || a.t < 0 || a.l + a.w > view.w || a.t + a.h > view.h);
      if (out || obstacles.some((o) => hit(a, o))) a.hidden = true;
    }
    if (!a.hidden) placed.push(a);
  });
  return all.map(({ id, l, t, w, h, hidden }) => ({ id, l, t, w, h, hidden }));
}

/** Tingkat detail dari zoom: `far` (badge ikon saja), `mid`, `near`. */
export function lodOf(z, mode = "iso") {
  if (z < (mode === "plan" ? 0.55 : 0.5)) return "far";
  return z < 0.85 ? "mid" : "near";
}
/** Isi label karakter pada LOD: `full` (nama + badge), `icon` (badge ikon), `none`. */
export function tagShows(lod, st, selected) {
  if (selected) return "full";
  const v = statusView(st);
  if (lod === "far") return v.badge ? "icon" : "none";
  if (lod === "mid" && v.st === "idle") return "none";
  return "full";
}

/* ---------------------------------------------------------------- ruangan */
/** Subjudul label ruangan (kunci i18n halaman + angka). */
export function roomSub(room, here) {
  if (room.kind === "pantry") return here.length ? { k: "sub-idle", n: here.length } : { k: "sub-quiet" };
  if (room.kind === "meeting") return here.some((c) => c.st === "meeting") ? { k: "sub-meeting" } : { k: "sub-quiet" };
  if (room.kind !== "project") return { k: "sub-quiet" };
  if (!here.length) return { k: "sub-empty" };
  const out = { k: "sub-agents", n: here.filter((c) => c.bot).length };
  const todo = here.filter((c) => statusView(c.st).act).length;
  if (todo) out.todo = todo;
  return out;
}

/* ---------------------------------------------------------------- kamera */
/** Kamera `{z, tx, ty}` (layar = dunia × z + t) yang memuat kotak `b` di area pandang. */
export function fitView(b, W, H, { pad = 0.94, top = 52, bottom = 44, min = 0.05, max = 2.6 } = {}) {
  const h = Math.max(1, H - top - bottom);
  const z = Math.max(min, Math.min(max, Math.min(W / Math.max(1, b.x1 - b.x0), h / Math.max(1, b.y1 - b.y0)) * pad));
  return { z, tx: W / 2 - ((b.x0 + b.x1) / 2) * z, ty: top + h / 2 - ((b.y0 + b.y1) / 2) * z };
}
/** Zoom faktor `k` di titik layar `(sx, sy)`: titik dunia di bawahnya tidak bergeser. */
export function zoomAround(cam, k, sx, sy, min, max) {
  const z = Math.max(min, Math.min(max, cam.z * k));
  const wx = (sx - cam.tx) / cam.z, wy = (sy - cam.ty) / cam.z;
  return { z, tx: sx - wx * z, ty: sy - wy * z };
}

/* ---------------------------------------------------------------- mode */
/** Kunci localStorage mode tampilan per user (pid). */
export const modeKey = (me) => (me ? `ag-office-view:${me}` : "ag-office-view");
export function readMode(store, me) {
  try { return store?.getItem(modeKey(me)) === "plan" ? "plan" : "iso"; } catch { return "iso"; }
}
export function writeMode(store, me, mode) {
  try { store?.setItem(modeKey(me), mode === "plan" ? "plan" : "iso"); } catch { /* mode tetap berlaku di halaman ini */ }
}
