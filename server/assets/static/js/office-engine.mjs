// AI Office (M10): mesin kanvas isometrik gaya A1 "Vektor lembut", di-port dari prototype
// (docs/reference/ui-prototype/screens-office.js) dengan denah parametris dari data nyata:
// kolom selebar 17 ubin per dua project (ruangan atas 17×11, ruangan depan 17×9), lorong 2 ubin,
// kolom terakhir Ruang rapat (atas) + Pantry (bawah), slot depan kosong jadi Lounge.
// Dimuat lazy oleh office.js hanya di halaman AI Office. Tanpa dependensi.

/* ===================================================================== */
/* Warna                                                                 */
/* ===================================================================== */
const h2r = (h) => { h = h.replace("#", ""); if (h.length === 3) h = [...h].map((c) => c + c).join(""); const n = parseInt(h, 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; };
const r2h = (a) => "#" + a.map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0")).join("");
const mix = (a, b, t) => { const A = h2r(a), B = h2r(b); return r2h(A.map((v, i) => v + (B[i] - v) * t)); };
const shade = (c, f) => (f < 0 ? mix(c, "#000000", -f) : mix(c, "#ffffff", f));
const rgba = (h, a) => { const [r, g, b] = h2r(h); return `rgba(${r},${g},${b},${a})`; };
const isHex = (v) => typeof v === "string" && /^#[0-9a-fA-F]{6}$/.test(v);

const PAL = {
  bgA: "#f1f4fb", bgB: "#e2e8f4", bgDA: "#191b24", bgDB: "#121319",
  slab: "#d6dbe8", wall: "#f6f2ec", wallL: "#efe9e1", wallCap: "#ddd5ca", base: "#d2c8bb",
  winFrame: "#ffffff", sky1: "#9fd2ff", sky2: "#e3f3ff", cloud: "#ffffff",
  glass: "#bfe4f5", glassFrame: "#b7c0cf", half: "#ebe5dd",
  fA: "#ece9f6", fA2: "#e4e0f1", fB: "#f4e9d6", fB2: "#ede0c8", fC: "#e0e6ee", fC2: "#d5dde8",
  fMEET: "#cfe6ea", fMEET2: "#b9d9de", fCOR: "#e9d2b2", fCOR2: "#e0c6a2", fPAN: "#f6efe2", fPAN2: "#cfe3dc",
  fLOU: "#eef3e6", fLOU2: "#e2ead6",
  desk: "#fbfaf7", deskLeg: "#a7afbf", mon: "#3b404d", screen: "#1f2742", screenOff: "#454b59", kb: "#e4e7ee",
  chair: "#56627f", chairLeg: "#3d4558", wood: "#d6a676", woodD: "#b88454",
  shelf: "#cfa173", board: "#ffffff", boardFrame: "#c5cbd6",
  sofa: "#7aa5cc", cushion: "#f4c98d", metal: "#c9ced6", fridge: "#f0f3f7", counter: "#fbfaf7", counterTop: "#d7c0a2",
  pot: "#ec9f7a", potB: "#f2f0ea", leaf1: "#6cc286", leaf2: "#4ea56c", leaf3: "#9adbab",
  rug: "#efd9cc", rug2: "#e5c6b3", rugB: "#dfe6f7", rugB2: "#cfd8f0",
  mug: "#ffffff", cooler: "#eaf0f6", water: "#9ad4f6", beanbag: "#f3a35f", beanbag2: "#8fb7e8",
  tv: "#2b2f3a", door: "#c99a70", doorFrame: "#ffffff", lampShade: "#f3f0e8", printer: "#e9ecf1",
  pants: "#33395a", shoes: "#3a3030", shell: "#f5f6fa", shellD: "#dfe3ec",
};
const EDGE = 0.55, SHADE = 1;
const TONES = {};
const tonesOf = (c) => TONES[c] || (TONES[c] = { t: c, l: shade(c, -0.09 * SHADE), r: shade(c, -0.2 * SHADE) });
const mat = (k) => tonesOf(PAL[k] || k);
/* Lantai project bergiliran agar ruangan bersebelahan mudah dibedakan. */
const FLOORS = [["fA", "fA2", "tile"], ["fB", "fB2", "tile"], ["fC", "fC2", "big"]];

/* Status → warna kanvas (prototype OST). */
const STC = { working: "#5b5bd6", review: "#e3962b", decision: "#e5484d", meeting: "#3b82f6", idle: "#a1a7b5", online: "#30a46c", paused: "#a1a7b5", walk: "#5b5bd6" };

/* ===================================================================== */
/* Proyeksi + denah                                                      */
/* ===================================================================== */
const T = 28, WH = 3, D = 22, COLW = 17, LASTW = 10;
const P = (x, y, z = 0) => [(x - y) * T, ((x + y) * T) / 2 - z * T];

/* Denah dari daftar ruangan server: { k, kind, n, c, col, front }. */
function planFrom(rooms) {
  const cols = Math.max(1, ...rooms.filter((r) => r.kind !== "meeting" && r.kind !== "pantry").map((r) => r.col + 1));
  const W = cols * COLW + LASTW;
  const R = rooms.map((r, i) => {
    const last = r.kind === "meeting" || r.kind === "pantry";
    const x0 = last ? cols * COLW : r.col * COLW, x1 = last ? W : x0 + COLW;
    const y0 = r.front ? 13 : 0, y1 = r.front ? D : 11;
    const fl = r.kind === "meeting" ? ["fMEET", "fMEET2", "carpet"] : r.kind === "pantry" ? ["fPAN", "fPAN2", "checker"] : r.kind === "lounge" ? ["fLOU", "fLOU2", "wood"] : FLOORS[i % FLOORS.length];
    const door = r.kind === "meeting" ? [x0 + 3.6, x0 + 5.2] : r.kind === "pantry" ? [x0 + 1.0, x0 + 2.6] : [x0 + 7.6, x0 + 9.2];
    return { ...r, x0, x1, y0, y1, top: !r.front, fl, door, doorY: r.front ? 13 : 11 };
  });
  return { W, cols, rooms: R, byKey: Object.fromEntries(R.map((r) => [r.k, r])) };
}

/* Kursi/titik berdiri per ruangan. f = arah hadap, sit = duduk, out = jalur ke pintu. */
function seatsFor(r) {
  const S = [], x0 = r.x0;
  if (r.kind === "project") {
    const dy = r.front ? 12 : 0, dX = x0 + 8.4, dY = r.doorY;
    const aN = 2.35 + dy, aS = 8.5 + dy;
    const pod = (x, n) => [
      { id: `${r.k}:a${n}`, x, y: 3.35 + dy, f: "+y", sit: 1, out: [[x, aN], [dX, aN], [dX, dY]] },
      { id: `${r.k}:b${n}`, x, y: 6.65 + dy, f: "-y", sit: 1, out: [[x, aS], [dX, aS], [dX, dY]] },
    ];
    // Urutan isi: meja kiri dulu, sisi depan-belakang bergantian.
    [[x0 + 3.2, 1], [x0 + 5.6, 2], [x0 + 11.2, 3], [x0 + 13.6, 4]].forEach(([x, n]) => S.push(...pod(x, n)));
    S.push({ id: `${r.k}:st`, x: x0 + 15.6, y: 6.6 + dy, f: "-x", sit: 0, act: "tablet", out: [[x0 + 15.6, aS], [dX, aS], [dX, dY]] });
    for (let i = 0; i < 6; i++) { const x = x0 + 2 + i * 2.3, y = 9.6 + dy; S.push({ id: `${r.k}:x${i}`, x, y, f: "+y", sit: 0, out: [[x, aS], [dX, aS], [dX, dY]] }); }
  } else if (r.kind === "meeting") {
    const d = x0 - 17, out = (x) => [[x, 3.0], [19.0 + d, 3.0], [19.0 + d, 9.3], [21.4 + d, 9.3], [21.4 + d, 11]];
    S.push({ id: "MEET:mp", x: 19.0 + d, y: 2.5, f: "+y", sit: 0, act: "point", out: [[19.0 + d, 9.3], [21.4 + d, 9.3], [21.4 + d, 11]] });
    [21.0, 22.6, 24.2].forEach((x, i) => S.push({ id: `MEET:m${i + 1}`, x: x + d, y: 3.8, f: "+y", sit: 1, out: out(x + d) }));
    [21.0, 22.6, 24.2].forEach((x, i) => S.push({ id: `MEET:s${i + 1}`, x: x + d, y: 7.15, f: "-y", sit: 1, out: [[x + d, 8.4], [21.4 + d, 8.4], [21.4 + d, 11]] }));
  } else if (r.kind === "pantry") {
    const d = x0 - 17, o = (pts) => pts.map(([a, b]) => [a + d, b]);
    S.push({ id: "PANTRY:p4", x: 22.6 + d, y: 16.0, f: "+y", sit: 0, out: o([[18.8, 15.0], [18.8, 13]]) });
    S.push({ id: "PANTRY:p5", x: 21.0 + d, y: 20.0, f: "+y", sit: 0, out: o([[18.8, 19.2], [18.8, 13]]) });
    S.push({ id: "PANTRY:p1", x: 17.95 + d, y: 17.0, f: "+x", sit: 1, sofa: 1, act: "mug", out: o([[18.8, 17.0], [18.8, 13]]) });
    S.push({ id: "PANTRY:p2", x: 21.0 + d, y: 14.75, f: "+y", sit: 0, act: "mug", out: o([[18.8, 15.0], [18.8, 13]]) });
    S.push({ id: "PANTRY:p3", x: 23.75 + d, y: 17.7, f: "+x", sit: 0, act: "tablet", out: o([[22.6, 15.4], [18.8, 15.0], [18.8, 13]]) });
    S.push({ id: "PANTRY:p6", x: 25.4 + d, y: 19.4, f: "+y", sit: 0, out: o([[22.6, 15.4], [18.8, 15.0], [18.8, 13]]) });
    S.push({ id: "PANTRY:p7", x: 17.95 + d, y: 18.3, f: "+x", sit: 1, sofa: 1, out: o([[18.8, 18.3], [18.8, 13]]) });
    for (let i = 0; i < 6; i++) { const x = 19.4 + d + (i % 3) * 2.2, y = 20.9 + Math.floor(i / 3) * -0.0; S.push({ id: `PANTRY:x${i}`, x: x + (i >= 3 ? 1.1 : 0), y: y - (i >= 3 ? 1.6 : 0), f: "+y", sit: 0, out: [[x, 20.9], [18.8 + d, 20.9], [18.8 + d, 13]] }); }
  }
  return S;
}

/* ===================================================================== */
/* Primitif gambar                                                       */
/* ===================================================================== */
let G = null; // { c, sc, t, still, dark }
const path = (pts) => { const c = G.c; c.beginPath(); c.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < pts.length; i++) c.lineTo(pts[i][0], pts[i][1]); c.closePath(); };
const fillP = (pts, f) => { path(pts); G.c.fillStyle = f; G.c.fill(); };
const lw = (n = 1) => n / G.sc;
const quad = (x0, y0, x1, y1, z = 0) => [P(x0, y0, z), P(x1, y0, z), P(x1, y1, z), P(x0, y1, z)];
function line(a, b, col, w = 1) { const c = G.c; c.beginPath(); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); c.strokeStyle = col; c.lineWidth = lw(w); c.stroke(); }
function box(x, y, z, w, d, h, m, o = {}) {
  const t = typeof m === "string" ? mat(m) : m;
  const L = [P(x, y + d, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x, y + d, z + h)];
  const R = [P(x + w, y, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x + w, y, z + h)];
  const Tp = [P(x, y, z + h), P(x + w, y, z + h), P(x + w, y + d, z + h), P(x, y + d, z + h)];
  if (!o.noL) fillP(L, t.l);
  if (!o.noR) fillP(R, t.r);
  if (!o.noT) fillP(Tp, t.t);
  if (!o.noEdge && h > 0.015) {
    const c = G.c; c.beginPath(); c.moveTo(...Tp[3]); c.lineTo(...Tp[2]); c.lineTo(...Tp[1]);
    c.strokeStyle = `rgba(255,255,255,${EDGE * (o.edge ?? 1)})`; c.lineWidth = lw(1); c.stroke();
  }
}
function cyl(cx, cy, z, r, h, m, o = {}) {
  const t = typeof m === "string" ? mat(m) : m;
  const [X, Y0] = P(cx, cy, z), Y1 = Y0 - h * T, rx = r * T * 1.414, ry = r * T * 0.707, c = G.c;
  const g = c.createLinearGradient(X - rx, 0, X + rx, 0); g.addColorStop(0, t.l); g.addColorStop(0.55, t.l); g.addColorStop(1, t.r);
  c.beginPath(); c.ellipse(X, Y0, rx, ry, 0, 0, Math.PI); c.lineTo(X - rx, Y1); c.ellipse(X, Y1, rx, ry, 0, Math.PI, 0, true); c.closePath();
  c.fillStyle = g; c.fill();
  c.beginPath(); c.ellipse(X, Y1, rx, ry, 0, 0, Math.PI * 2); c.fillStyle = o.top || t.t; c.fill();
}
function softShadow(pts, a = 1, spread = 0.18) {
  const base = 0.11 * a;
  const cx = pts.reduce((s, p) => s + p[0], 0) / pts.length, cy = pts.reduce((s, p) => s + p[1], 0) / pts.length;
  const e = spread * T;
  for (let i = 3; i >= 0; i--) {
    const k = (i * e) / 3;
    fillP(pts.map(([x, y]) => { const dx = x - cx, dy = y - cy, l = Math.hypot(dx, dy) || 1; return [x + (dx / l) * k, y + (dy / l) * k * 0.6]; }), `rgba(20,24,50,${base / 3})`);
  }
}
function blob(X, Y, rx, ry, col) { const c = G.c; c.beginPath(); c.ellipse(X, Y, rx, ry, 0, 0, Math.PI * 2); c.fillStyle = col; c.fill(); }
function footShadow(x, y, r, a = 1) {
  const [X, Y] = P(x, y), c = G.c, rx = r * T * 1.414, ry = r * T * 0.707;
  const g = c.createRadialGradient(X, Y, 0, X, Y, rx);
  g.addColorStop(0, `rgba(20,24,50,${0.2 * a})`); g.addColorStop(0.6, `rgba(20,24,50,${0.12 * a})`); g.addColorStop(1, "rgba(20,24,50,0)");
  c.save(); c.translate(X, Y); c.scale(1, ry / rx); c.translate(-X, -Y); c.beginPath(); c.arc(X, Y, rx, 0, Math.PI * 2); c.fillStyle = g; c.fill(); c.restore();
}
function onBack(x0, ztop, fn) { const [e, f] = P(x0, 0, ztop); const c = G.c; c.save(); c.transform(1, 0.5, 0, 1, e, f); fn(c); c.restore(); }
function rr(c, x, y, w, h, r) { c.beginPath(); if (c.roundRect) c.roundRect(x, y, w, h, r); else c.rect(x, y, w, h); }
const hash = (a, b = 0) => { const s = Math.sin(a * 127.1 + b * 311.7) * 43758.5453; return s - Math.floor(s); };

/* Ikon persona (Path2D dari SVG ikon di halaman). */
let ICONS = {};
const ICP = {};
function iconPaths(n) {
  if (ICP[n]) return ICP[n];
  const out = [];
  (ICONS[n] || ICONS.robot || []).forEach((d) => { try { out.push(new Path2D(d)); } catch { /* path tidak valid */ } });
  return (ICP[n] = out);
}
function drawIcon(c, n, x, y, size, col, w = 2.4) {
  c.save(); c.translate(x, y); c.scale(size / 24, size / 24); c.strokeStyle = col; c.lineWidth = w; c.lineCap = "round"; c.lineJoin = "round";
  iconPaths(n).forEach((p) => c.stroke(p)); c.restore();
}

/* ===================================================================== */
/* Lantai, dinding, dekor                                                */
/* ===================================================================== */
function floorArea(x0, y0, x1, y1, kind, ka, kb) {
  const A = PAL[ka], B = PAL[kb], c = G.c;
  fillP(quad(x0, y0, x1, y1), A);
  if (kind === "tile" || kind === "checker") {
    for (let i = Math.floor(x0); i < x1; i++) for (let j = Math.floor(y0); j < y1; j++) {
      if ((i + j) % 2 === 0) continue;
      fillP(quad(Math.max(i, x0), Math.max(j, y0), Math.min(i + 1, x1), Math.min(j + 1, y1)), B);
    }
  }
  if (kind === "tile" || kind === "checker" || kind === "big") {
    const s = kind === "big" ? 2 : 1;
    c.beginPath();
    for (let x = Math.ceil(x0 / s) * s; x < x1; x += s) { if (x <= x0) continue; const a = P(x, y0), b = P(x, y1); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); }
    for (let y = Math.ceil(y0 / s) * s; y < y1; y += s) { if (y <= y0) continue; const a = P(x0, y), b = P(x1, y); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); }
    c.strokeStyle = "rgba(0,0,0,0.07)"; c.lineWidth = lw(1); c.stroke();
  } else if (kind === "wood") {
    const st = 0.5; let r = 0;
    for (let y = y0; y < y1 - 0.001; y += st, r++) {
      if (r % 2) fillP(quad(x0, y, x1, Math.min(y + st, y1)), B);
      c.beginPath();
      for (let x = x0 + 0.4 + hash(r) * 2.2; x < x1; x += 2.6) { const a = P(x, y), b = P(x, Math.min(y + st, y1)); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); }
      const a = P(x0, y), b = P(x1, y); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]);
      c.strokeStyle = "rgba(80,40,10,0.1)"; c.lineWidth = lw(1); c.stroke();
    }
  } else if (kind === "carpet") {
    fillP(quad(x0 + 0.5, y0 + 0.5, x1 - 0.5, y1 - 0.5), B);
    fillP(quad(x0 + 0.75, y0 + 0.75, x1 - 0.75, y1 - 0.75), A);
  }
}
function rug(x0, y0, x1, y1, ka, kb) {
  softShadow(quad(x0 + 0.05, y0 + 0.08, x1 + 0.05, y1 + 0.08), 0.35, 0.05);
  fillP(quad(x0, y0, x1, y1), PAL[kb]);
  fillP(quad(x0 + 0.22, y0 + 0.22, x1 - 0.22, y1 - 0.22), PAL[ka]);
  const c = G.c; path(quad(x0 + 0.38, y0 + 0.38, x1 - 0.38, y1 - 0.38)); c.strokeStyle = rgba(PAL[kb], 0.9); c.lineWidth = lw(1.5); c.setLineDash([lw(4), lw(4)]); c.stroke(); c.setLineDash([]);
}
function windowOn(side, a, b, z0 = 1.05, z1 = 2.55) {
  const c = G.c, Pw = (u, z) => (side === "back" ? P(u, 0, z) : P(0, u, z));
  const fr = PAL.winFrame;
  fillP([Pw(a - 0.1, z0 - 0.1), Pw(b + 0.1, z0 - 0.1), Pw(b + 0.1, z1 + 0.1), Pw(a - 0.1, z1 + 0.1)], side === "back" ? shade(fr, -0.04) : shade(fr, -0.12));
  const glass = [Pw(a, z0), Pw(b, z0), Pw(b, z1), Pw(a, z1)];
  const top = Pw((a + b) / 2, z1), bot = Pw((a + b) / 2, z0);
  const g = c.createLinearGradient(top[0], top[1], bot[0], bot[1]); g.addColorStop(0, PAL.sky1); g.addColorStop(1, PAL.sky2);
  fillP(glass, g);
  c.save(); path(glass); c.clip();
  const cl = PAL.cloud;
  [[0.3, 0.72, 1], [0.7, 0.5, 0.8]].forEach(([fu, fz, s]) => { const [X, Y] = Pw(a + (b - a) * fu, z0 + (z1 - z0) * fz); blob(X, Y, 9 * s, 4 * s, cl); blob(X + 6 * s, Y - 2.5 * s, 7 * s, 4.5 * s, cl); blob(X - 6 * s, Y + 0.5, 6 * s, 3 * s, cl); });
  for (let i = 0; i < 5; i++) { const u0 = a + (i * (b - a)) / 5, hgt = 0.18 + hash(i, b) * 0.32; fillP([Pw(u0, z0), Pw(u0 + (b - a) / 5 - 0.03, z0), Pw(u0 + (b - a) / 5 - 0.03, z0 + hgt), Pw(u0, z0 + hgt)], "rgba(120,160,200,.35)"); }
  { const p1 = Pw(a + (b - a) * 0.15, z1), p2 = Pw(a + (b - a) * 0.45, z1), p3 = Pw(a + (b - a) * 0.05, z0); fillP([p1, p2, [p3[0] + (p2[0] - p1[0]), p3[1] + (p2[1] - p1[1])], p3], "rgba(255,255,255,.28)"); }
  c.restore();
  const mx = (a + b) / 2, mz = z0 + (z1 - z0) * 0.62;
  line(Pw(mx, z0), Pw(mx, z1), fr, 2.2); line(Pw(a, mz), Pw(b, mz), fr, 1.6);
  if (side === "back") box(a - 0.15, 0, z0 - 0.12, b - a + 0.3, 0.14, 0.06, "winFrame", { edge: 0.6 });
  else box(0, a - 0.15, z0 - 0.12, 0.14, b - a + 0.3, 0.06, "winFrame", { edge: 0.6 });
}
/* Papan kanban project di dinding belakang: nama project + kartu sesuai keadaan ruangan. */
function kanban(x0, x1, z0, z1, room, chars) {
  const w = (x1 - x0) * T, h = (z1 - z0) * T;
  onBack(x0, z1, (c) => {
    c.fillStyle = shade(PAL.boardFrame, -0.1); rr(c, -3, -3, w + 6, h + 6, 3); c.fill();
    c.fillStyle = PAL.board; rr(c, 0, 0, w, h, 2); c.fill();
    c.fillStyle = "#3b404d"; c.font = "600 5.6px Geist, system-ui, sans-serif";
    c.fillText(room.n.length > 34 ? room.n.slice(0, 33) + "…" : room.n, 5, 8);
    const cols = ["#8d93a3", "#5b5bd6", "#e3962b", "#0891b2", "#30a46c"], cw = (w - 8) / cols.length;
    const here = chars.filter((ch) => ch.room === room.k && ch.bot);
    const bins = [2, here.filter((c2) => c2.st === "working").length, here.filter((c2) => c2.st === "review" || c2.st === "decision").length, 1, 3];
    const tone = ["#eceef3", "#cfd0ff", "#ffe2b0", "#cfe3ff", "#d5f2df"];
    cols.forEach((col, i) => {
      const cx = 4 + i * cw;
      c.fillStyle = col; c.fillRect(cx + 1, 11, cw - 3, 2.4);
      if (i) { c.fillStyle = "rgba(0,0,0,.07)"; c.fillRect(cx - 1, 11, 0.8, h - 14); }
      for (let j = 0; j < Math.min(4, bins[i]); j++) {
        const yy = 16.5 + j * 6.6; c.fillStyle = i === 2 && here.some((x) => x.st === "decision") && j === 0 ? "#ffc6c6" : tone[i]; c.fillRect(cx + 1, yy, cw - 3, 6);
        c.fillStyle = "rgba(0,0,0,.18)"; c.fillRect(cx + 2.5, yy + 2, (cw - 8) * (0.5 + hash(i, j) * 0.4), 1);
      }
    });
  });
  box(x0 + 0.6, 0, z0 - 0.16, 1.4, 0.12, 0.05, "boardFrame");
}
function tvWall(x0, x1, z0, z1, on) {
  const w = (x1 - x0) * T, h = (z1 - z0) * T;
  onBack(x0, z1, (c) => {
    c.fillStyle = PAL.tv; rr(c, -2, -2, w + 4, h + 4, 3); c.fill();
    const g = c.createLinearGradient(0, 0, w, h); g.addColorStop(0, on ? "#3a5ea8" : PAL.screenOff); g.addColorStop(1, on ? "#24376a" : PAL.screenOff);
    c.fillStyle = g; c.fillRect(0, 0, w, h);
    if (!on) return;
    [["#ffe28a", 0], ["#b9f0c9", 1], ["#ffc0d0", 2], ["#bfe0ff", 3]].forEach(([col, i]) => { const x = 6 + (i * (w - 12)) / 4, y = 12; c.fillStyle = col; c.fillRect(x, y, (w - 12) / 4 - 4, h - y - 5); c.fillStyle = "rgba(0,0,0,.2)"; for (let k = 0; k < 2; k++) c.fillRect(x + 2, y + 3 + k * 4, (w - 12) / 4 - 9 - k * 3, 1); });
  });
}
function wallClock(x, z) {
  onBack(x, z, (c) => {
    c.fillStyle = PAL.boardFrame; c.beginPath(); c.arc(0, 0, 7.5, 0, 7); c.fill();
    c.fillStyle = PAL.board; c.beginPath(); c.arc(0, 0, 6.2, 0, 7); c.fill();
    c.strokeStyle = "#3b404d"; c.lineWidth = 1; c.lineCap = "round";
    const now = new Date(), m = now.getMinutes() / 60, hh = ((now.getHours() % 12) + m) / 12;
    c.beginPath(); c.moveTo(0, 0); c.lineTo(Math.sin(hh * 6.283) * 3.3, -Math.cos(hh * 6.283) * 3.3); c.moveTo(0, 0); c.lineTo(Math.sin(m * 6.283) * 4.8, -Math.cos(m * 6.283) * 4.8); c.stroke();
  });
}
function doorLeft(y0, y1) {
  const fr = PAL.doorFrame;
  fillP([P(0, y1 + 0.08, 0), P(0, y0 - 0.08, 0), P(0, y0 - 0.08, 2.25), P(0, y1 + 0.08, 2.25)], tonesOf(fr).r);
  fillP([P(0, y1, 0), P(0, y0, 0), P(0, y0, 2.15), P(0, y1, 2.15)], tonesOf(PAL.door).r);
  const m = (y0 + y1) / 2; line(P(0, m, 0), P(0, m, 2.15), shade(PAL.door, -0.35), 1);
  const [X, Y] = P(0, m - 0.16, 1.0); blob(X, Y, 1.6, 1.6, "#e9dcc6");
  const [X2, Y2] = P(0, m + 0.16, 1.0); blob(X2, Y2, 1.6, 1.6, "#e9dcc6");
  fillP([P(0, m + 0.35, 2.45), P(0, m - 0.35, 2.45), P(0, m - 0.35, 2.7), P(0, m + 0.35, 2.7)], "#22a565");
}

/* ===================================================================== */
/* Perabot (objek terurut kedalaman)                                     */
/* ===================================================================== */
let OBJ = [];
let OCC = {}; // kursi → karakter yang duduk
const add = (x0, y0, x1, y1, draw, k) => OBJ.push({ k: k ?? (x0 + x1 + y0 + y1) / 2, draw, fp: [x0, y0, x1, y1] });
function deskObj(x, y, w, d, mon, sid, prop = 0) {
  add(x, y, x + w, y + d, () => {
    const occ = sid ? OCC[sid] : null, on = !!occ;
    box(x + 0.08, y + 0.1, 0, 0.08, d - 0.2, 0.68, "deskLeg");
    box(x + w - 0.16, y + 0.1, 0, 0.08, d - 0.2, 0.68, "deskLeg");
    box(x + 0.16, mon === "s" ? y + 0.12 : y + d - 0.2, 0.25, w - 0.32, 0.06, 0.4, "deskLeg", { noEdge: 1 });
    box(x, y, 0.68, w, d, 0.06, "desk", { edge: 1.2 });
    const ky = mon === "s" ? y + d - 0.42 : y + 0.18;
    box(x + w / 2 - 0.4, ky, 0.74, 0.8, 0.22, 0.025, "kb");
    box(x + w / 2 + 0.55, ky + 0.04, 0.74, 0.12, 0.16, 0.03, "kb");
    const px = x + (prop % 2 ? 0.25 : w - 0.5), py = mon === "s" ? y + 0.35 : y + d - 0.55;
    if (prop % 3 === 0) cyl(px + 0.1, py + 0.1, 0.74, 0.07, 0.14, "mug", { top: shade(PAL.mug, -0.25) });
    else if (prop % 3 === 1) { cyl(px + 0.1, py + 0.12, 0.74, 0.09, 0.1, "pot"); blob(...P(px + 0.1, py + 0.12, 0.98), 4.2, 3.6, PAL.leaf1); blob(...P(px + 0.06, py + 0.1, 1.04), 2.6, 2.4, PAL.leaf3); }
    else { box(px - 0.1, py, 0.74, 0.36, 0.26, 0.05, "board"); box(px - 0.08, py + 0.02, 0.79, 0.34, 0.24, 0.02, "board"); }
    if (mon === "n" && prop % 2 === 0) deskLampDraw(x + 0.25, y + 0.25);
    const mx = x + w / 2 - 0.58, my = mon === "s" ? y + 0.1 : y + d - 0.24;
    box(mx + 0.5, my + 0.03, 0.74, 0.16, 0.08, 0.16, "mon");
    box(mx + 0.38, my - 0.06 + (mon === "s" ? 0.1 : -0.05), 0.74, 0.4, 0.22, 0.02, "mon");
    box(mx, my, 0.86, 1.16, 0.14, 0.66, "mon", { edge: 0.4 });
    if (mon === "s") {
      const yy = my + 0.14, sc = [P(mx + 0.06, yy, 0.92), P(mx + 1.1, yy, 0.92), P(mx + 1.1, yy, 1.46), P(mx + 0.06, yy, 1.46)];
      if (!on) { fillP(sc, PAL.screenOff); return; }
      fillP(sc, PAL.screen);
      const working = occ.st === "working" || occ.st === "online";
      const off = working ? Math.floor(G.t * 1.6 + occ.ph * 10) : 3;
      const cols = ["#7aa7ff", "#ffcf6b", "#7ee0b0", "#ff99c2", "#c9d1ff"];
      for (let k = 0; k < 5; k++) {
        const n = (k + off) % 9, ind = ((hash(n, sid.length) * 3) | 0) * 0.08, len = 0.22 + hash(n, 2) * 0.55;
        const z = 1.39 - k * 0.1;
        line(P(mx + 0.14 + ind, yy, z), P(mx + 0.14 + ind + len, yy, z), cols[n % 5], 1.6);
      }
      if (occ.st === "review" || occ.st === "decision") { const col = occ.st === "review" ? "#ffb347" : "#ff6b6b"; fillP([P(mx + 0.6, yy, 0.98), P(mx + 1.04, yy, 0.98), P(mx + 1.04, yy, 1.14), P(mx + 0.6, yy, 1.14)], col); }
    } else {
      const [X, Y] = P(mx + 0.58, my + 0.14, 1.2); blob(X, Y, 2.2, 2.2, shade(PAL.mon, 0.25));
    }
  });
}
function chairObj(x, y, back) {
  const seat = () => {
    footShadow(x, y, 0.32, 0.8);
    box(x - 0.04, y - 0.04, 0.06, 0.08, 0.08, 0.32, "chairLeg");
    [[-0.26, 0], [0.26, 0], [0, -0.26], [0, 0.26]].forEach(([a, b]) => box(x + a - 0.04, y + b - 0.04, 0, 0.08, 0.08, 0.06, "chairLeg"));
    box(x - 0.3, y - 0.3, 0.38, 0.6, 0.6, 0.1, "chair");
  };
  const backrest = () => {
    if (back === "n") { box(x - 0.04, y - 0.34, 0.44, 0.08, 0.06, 0.12, "chairLeg"); box(x - 0.28, y - 0.4, 0.52, 0.56, 0.1, 0.48, "chair"); }
    if (back === "s") { box(x - 0.04, y + 0.28, 0.44, 0.08, 0.06, 0.1, "chairLeg"); box(x - 0.28, y + 0.3, 0.5, 0.56, 0.1, 0.4, "chair"); }
  };
  if (back === "s") { add(x - 0.3, y - 0.3, x + 0.3, y + 0.3, seat, x + y - 0.08); add(x - 0.3, y + 0.3, x + 0.3, y + 0.4, backrest, x + y + 0.32); }
  else add(x - 0.4, y - 0.4, x + 0.3, y + 0.3, () => { backrest(); seat(); }, x + y - 0.2);
}
function plantObj(x, y, s = 1, kind = 0) {
  const r = 0.24 * s;
  add(x - r, y - r, x + r, y + r, () => {
    footShadow(x + 0.05, y + 0.06, r * 1.25, 0.9);
    cyl(x, y, 0, r, 0.42 * s, kind ? "potB" : "pot");
    const [X, Y] = P(x, y, 0.42 * s); const k = s * 1.05;
    if (kind === 2) {
      line([X, Y], [X, Y - 26 * k], shade(PAL.woodD, -0.2), 2);
      [[-7, -30, 11, 9, "leaf2"], [7, -34, 10, 9, "leaf2"], [0, -40, 12, 10, "leaf1"], [-5, -46, 8, 7, "leaf1"], [5, -47, 7, 6, "leaf3"], [-1, -36, 6, 5, "leaf3"]].forEach(([dx, dy, rx, ry, m]) => blob(X + dx * k, Y + dy * k, rx * k, ry * k, PAL[m]));
      return;
    }
    [[-6, -8, 9, 8, "leaf2"], [6, -9, 8, 7.5, "leaf2"], [0, -15, 10, 9, "leaf1"], [-4, -20, 6.5, 6, "leaf1"], [4, -19, 5.5, 5, "leaf3"], [-1, -12, 4, 3.5, "leaf3"]].forEach(([dx, dy, rx, ry, m]) => blob(X + dx * k, Y + dy * k, rx * k, ry * k, PAL[m]));
  });
}
function deskLampDraw(x, y, z = 0.74) {
  const c = G.c;
  cyl(x, y, z, 0.09, 0.03, "mon");
  const a = P(x, y, z + 0.03), b = P(x, y, z + 0.55), d = P(x + 0.22, y + 0.12, z + 0.62);
  c.beginPath(); c.moveTo(...a); c.lineTo(...b); c.lineTo(...d); c.strokeStyle = PAL.mon; c.lineWidth = lw(1.6); c.lineCap = "round"; c.stroke();
  const [X, Y] = d; c.beginPath(); c.moveTo(X - 5, Y + 3); c.lineTo(X - 2, Y - 3); c.lineTo(X + 4, Y - 2); c.lineTo(X + 6, Y + 4); c.closePath(); c.fillStyle = PAL.mon; c.fill();
  blob(X + 0.6, Y + 3.6, 3.6, 1.4, "#ffe7a3");
}
function floorLamp(x, y) {
  add(x - 0.25, y - 0.25, x + 0.25, y + 0.25, () => {
    footShadow(x, y, 0.25);
    cyl(x, y, 0, 0.18, 0.05, "deskLeg"); box(x - 0.03, y - 0.03, 0, 0.06, 0.06, 1.75, "deskLeg");
    cyl(x, y, 1.65, 0.24, 0.32, tonesOf(PAL.lampShade));
  });
}
function beanbag(x, y, k) {
  add(x - 0.6, y - 0.5, x + 0.6, y + 0.5, () => { footShadow(x, y, 0.62); const [X, Y] = P(x, y, 0); const t = tonesOf(PAL[k]); blob(X, Y - 9, 22, 13, t.l); blob(X - 3, Y - 13, 16, 9, t.t); blob(X + 9, Y - 8, 10, 8, t.r); });
}
function shelfLeft(y0) {
  add(0, y0, 0.6, y0 + 2.8, () => {
    const books = ["#e07a5f", "#3d5a80", "#81b29a", "#f2cc8f", "#9b7ede", "#ef8fa8", "#5fb0c9"];
    box(0, y0, 0, 0.55, 2.8, 1.95, "shelf");
    [0.08, 0.68, 1.28].forEach((z, li) => {
      fillP([P(0.55, y0 + 0.1, z), P(0.55, y0 + 2.7, z), P(0.55, y0 + 2.7, z + 0.55), P(0.55, y0 + 0.1, z + 0.55)], shade(PAL.shelf, -0.38));
      let yy = y0 + 0.12;
      for (let i = 0; yy < y0 + 2.6; i++) {
        const bw = 0.12 + hash(i, li) * 0.08, bh = 0.34 + hash(li, i) * 0.16, col = books[(i + li * 3) % books.length];
        if (hash(i + 7, li) > 0.86) { yy += 0.2; continue; }
        box(0.12, yy, z, 0.36, Math.min(bw, y0 + 2.68 - yy), bh, tonesOf(col), { noEdge: 1 }); yy += bw + 0.01;
      }
    });
  });
}
function steam(x, y, z) {
  const c = G.c; if (G.still) return;
  const [X, Y] = P(x, y, z);
  for (let i = 0; i < 2; i++) {
    const t = (G.t * 0.5 + i * 0.5) % 1;
    c.beginPath(); c.moveTo(X - 1.5 + i * 3, Y - 2 - t * 10); c.bezierCurveTo(X - 4 + i * 3, Y - 6 - t * 10, X + 2 + i * 3, Y - 9 - t * 10, X - 1 + i * 3, Y - 13 - t * 10);
    c.strokeStyle = `rgba(255,255,255,${0.75 * (1 - t)})`; c.lineWidth = lw(1.6); c.lineCap = "round"; c.stroke();
  }
}
function glassSeg(x, y0, y1) {
  add(x - 0.05, y0, x + 0.05, y1, () => {
    const c = G.c, H = 2.75;
    box(x - 0.05, y0, 0, 0.1, y1 - y0, 0.14, "glassFrame");
    fillP([P(x, y1, 0.14), P(x, y0, 0.14), P(x, y0, H), P(x, y1, H)], rgba(PAL.glass, 0.24));
    const a = P(x, y0 + 0.25, 0.6), b = P(x, y0 + 0.55, 2.2); c.beginPath(); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); c.strokeStyle = "rgba(255,255,255,0.55)"; c.lineWidth = lw(3); c.stroke();
    box(x - 0.04, y0, H, 0.08, y1 - y0, 0.07, "glassFrame");
    box(x - 0.035, y0, 0, 0.07, 0.06, H, "glassFrame");
  });
}
function halfSeg(y, x0, x1) {
  add(x0, y - 0.08, x1, y + 0.08, () => {
    box(x0, y - 0.08, 0, x1 - x0, 0.16, 0.5, "half", { noEdge: 1 });
    box(x0, y - 0.09, 0.5, x1 - x0, 0.18, 0.05, "glassFrame");
    fillP([P(x0, y, 0.55), P(x1, y, 0.55), P(x1, y, 1.3), P(x0, y, 1.3)], rgba(PAL.glass, 0.24));
    box(x0, y - 0.035, 1.3, x1 - x0, 0.07, 0.05, "glassFrame");
    if (Math.abs(x0 - Math.round(x0)) < 0.01) box(x0, y - 0.035, 0.55, 0.06, 0.07, 0.75, "glassFrame");
  });
}

/* Ruangan project: 4 pod (8 meja) + meja berdiri; depan = sama, digeser 12 ubin ke depan. */
function buildProject(r, i) {
  const x0 = r.x0, dy = r.front ? 12 : 0;
  [[2, 1, 0], [4.4, 2, 1], [10, 3, 2], [12.4, 4, 3]].forEach(([dx, n, k]) => {
    deskObj(x0 + dx, 4 + dy, 2.4, 1, "n", `${r.k}:a${n}`, k + i);
    deskObj(x0 + dx, 5 + dy, 2.4, 1, "s", `${r.k}:b${n}`, k + i + 1);
    chairObj(x0 + dx + 1.2, 3.35 + dy, "n");
    chairObj(x0 + dx + 1.2, 6.65 + dy, "s");
  });
  add(x0 + 15.3, 7.3 + dy, x0 + 16.6, 8.2 + dy, () => { box(x0 + 15.3, 7.3 + dy, 0, 1.3, 0.9, 0.95, "printer"); box(x0 + 15.45, 7.4 + dy, 0.95, 0.9, 0.7, 0.32, "printer", { edge: 1.2 }); box(x0 + 15.6, 7.85 + dy, 1.27, 0.6, 0.12, 0.02, "board"); });
  if (!r.front) {
    plantObj(x0 + 16.3, 0.55, 1.15, 2); plantObj(x0 + 13.3, 0.5, 0.9, 1);
    add(x0 + 15.4, 9.4, x0 + 16.6, 10.4, () => { box(x0 + 15.4, 9.4, 0, 1.2, 1.0, 0.8, "shelf"); });
    if (x0 === 0) { shelfLeft(6.6); plantObj(0.5, 10.3, 1.1); plantObj(0.5, 4.6, 0.85, 1); }
  } else {
    plantObj(x0 + 0.6, 21.3, 1.1, 2); plantObj(x0 + 16.4, 21.4, 0.85);
    if (x0 === 0) plantObj(0.5, 14.0, 0.9, 1);
  }
}
function buildMeeting(r) {
  const d = r.x0 - 17;
  add(19.8 + d, 4.6, 25.0 + d, 6.3, () => {
    softShadow(quad(19.9 + d, 4.75, 25.15 + d, 6.5), 1);
    box(20.5 + d, 5.2, 0, 0.3, 0.5, 0.66, "woodD"); box(24.0 + d, 5.2, 0, 0.3, 0.5, 0.66, "woodD");
    box(19.8 + d, 4.6, 0.66, 5.2, 1.7, 0.08, "wood", { edge: 1.3 });
    box(22.25 + d, 4.75, 0.74, 0.7, 0.45, 0.025, "mon");
    fillP([P(22.25 + d, 4.75, 0.77), P(22.95 + d, 4.75, 0.77), P(22.95 + d, 4.75, 1.18), P(22.25 + d, 4.75, 1.18)], PAL.mon);
    box(21.1 + d, 5.0, 0.74, 0.42, 0.3, 0.02, "board"); box(23.7 + d, 5.4, 0.74, 0.42, 0.3, 0.02, "board");
    cyl(24.4 + d, 4.95, 0.74, 0.1, 0.26, "glass", { top: PAL.water }); cyl(20.6 + d, 5.7, 0.74, 0.07, 0.14, "mug");
  });
  [21.0, 22.6, 24.2].forEach((x) => chairObj(x + d, 3.8, "n"));
  [21.0, 22.6, 24.2].forEach((x) => chairObj(x + d, 7.15, "s"));
  add(23.9 + d, 0.05, 26.6 + d, 0.65, () => { box(23.9 + d, 0.05, 0, 2.7, 0.6, 0.78, "woodD"); [24.55, 25.9].forEach((x) => line(P(x + d, 0.65, 0.08), P(x + d, 0.65, 0.7), rgba("#000000", 0.18), 1)); });
  plantObj(26.3 + d, 0.4, 0.75, 1); plantObj(17.6 + d, 10.3, 1.15, 2); plantObj(26.4 + d, 10.4, 1);
  floorLamp(26.45 + d, 7.65);
}
function buildPantry(r) {
  const d = r.x0 - 17;
  add(20.2 + d, 13.15, 25.0 + d, 14.05, () => {
    box(20.2 + d, 13.15, 0, 4.8, 0.88, 0.88, "counter");
    [21.4, 22.6, 23.8].forEach((x) => { line(P(x + d, 14.03, 0.08), P(x + d, 14.03, 0.8), rgba("#000000", 0.12), 1); blob(...P(x + d - 0.12, 14.03, 0.7), 0.8, 1.6, rgba("#000000", 0.35)); });
    box(20.15 + d, 13.1, 0.88, 4.9, 0.98, 0.06, "counterTop", { edge: 1 });
    box(20.45 + d, 13.25, 0.94, 0.6, 0.5, 0.62, "mon"); box(20.5 + d, 13.75, 1.1, 0.5, 0.04, 0.3, "metal"); cyl(20.75 + d, 13.62, 0.94, 0.07, 0.1, "mug");
    const [lx, ly] = P(20.55 + d, 13.75, 1.48); blob(lx, ly, 1.2, 1.2, "#30a46c");
    const [sx, sy] = P(22.0 + d, 13.6, 0.95); blob(sx, sy, 11, 5, shade(PAL.metal, -0.15)); blob(sx, sy + 0.8, 9, 4, shade(PAL.metal, -0.32));
    box(21.9 + d, 13.2, 0.94, 0.08, 0.08, 0.4, "metal");
    const [fx, fy] = P(23.0 + d, 13.6, 0.97); blob(fx, fy, 7, 3.5, "#f2e4c9"); blob(fx - 3, fy - 2.5, 2.6, 2.6, "#ff8a5c"); blob(fx + 2, fy - 2.8, 2.6, 2.6, "#ffd166"); blob(fx, fy - 4, 2.4, 2.4, "#7cc576");
    box(23.8 + d, 13.25, 0.94, 0.85, 0.55, 0.45, "printer"); fillP([P(23.88 + d, 13.8, 1.0), P(24.4 + d, 13.8, 1.0), P(24.4 + d, 13.8, 1.32), P(23.88 + d, 13.8, 1.32)], "#3b404d");
  });
  add(20.45 + d, 13.25, 21.05 + d, 13.75, () => steam(20.75 + d, 13.62, 1.06), 20.75 + d + 13.62 + 0.9);
  add(25.3 + d, 13.15, 26.4 + d, 14.1, () => { box(25.3 + d, 13.15, 0, 1.1, 0.95, 2.05, "fridge", { edge: 1.2 }); line(P(25.3 + d, 14.1, 1.3), P(26.4 + d, 14.1, 1.3), rgba("#000000", 0.14), 1.2); box(25.45 + d, 14.1, 1.4, 0.05, 0.03, 0.45, "metal"); box(25.45 + d, 14.1, 0.75, 0.05, 0.03, 0.4, "metal"); fillP([P(25.9 + d, 14.1, 1.6), P(26.2 + d, 14.1, 1.6), P(26.2 + d, 14.1, 1.85), P(25.9 + d, 14.1, 1.85)], "#ffd166"); });
  add(17.15 + d, 15.6, 18.35 + d, 19.5, () => {
    softShadow(quad(17.2 + d, 15.7, 18.6 + d, 19.6), 1);
    box(17.2 + d, 15.75, 0, 1.1, 3.6, 0.42, "sofa");
    box(17.2 + d, 15.75, 0.42, 0.32, 3.6, 0.55, "sofa", { edge: 0.8 });
    box(17.2 + d, 15.75, 0.42, 1.1, 0.3, 0.28, "sofa");
    box(17.52 + d, 16.1, 0.42, 0.75, 1.42, 0.1, "sofa", { edge: 0.8 }); box(17.52 + d, 17.56, 0.42, 0.75, 1.42, 0.1, "sofa", { edge: 0.8 });
  }, 17.95 + d + 17.0 - 0.3);
  add(17.2 + d, 19.05, 18.3 + d, 19.35, () => box(17.2 + d, 19.05, 0.42, 1.1, 0.3, 0.28, "sofa"));
  add(19.4 + d, 16.4, 20.4 + d, 18.2, () => { footShadow(19.9 + d, 17.3, 0.75, 0.9); [[19.5, 16.5], [20.2, 18.0], [20.2, 16.5], [19.5, 18.0]].forEach(([a, b]) => box(a + d, b, 0, 0.1, 0.1, 0.34, "woodD")); box(19.4 + d, 16.4, 0.34, 1.0, 1.8, 0.06, "wood", { edge: 1.2 }); cyl(19.75 + d, 16.9, 0.4, 0.07, 0.13, "mug"); box(19.6 + d, 17.4, 0.4, 0.5, 0.35, 0.02, tonesOf("#9b7ede")); });
  add(24.15 + d, 17.15, 25.05 + d, 18.05, () => { footShadow(24.6 + d, 17.6, 0.45); cyl(24.6 + d, 17.6, 0, 0.26, 0.04, "deskLeg"); cyl(24.6 + d, 17.6, 0.04, 0.05, 0.98, "deskLeg"); cyl(24.6 + d, 17.6, 1.02, 0.44, 0.06, "wood"); cyl(24.45 + d, 17.5, 1.08, 0.06, 0.12, "mug"); });
  beanbag(22.0 + d, 20.5, "beanbag");
  plantObj(26.4 + d, 21.4, 1.2, 2); plantObj(17.5 + d, 21.4, 0.9);
  floorLamp(17.45 + d, 15.35);
}
function buildLounge(r) {
  const x0 = r.x0, y0 = r.y0;
  add(x0 + 4, y0 + 3, x0 + 5, y0 + 6.6, () => {
    softShadow(quad(x0 + 4.05, y0 + 3.1, x0 + 5.3, y0 + 6.7), 1);
    box(x0 + 4.05, y0 + 3.15, 0, 1.1, 3.4, 0.42, "sofa");
    box(x0 + 4.05, y0 + 3.15, 0.42, 0.32, 3.4, 0.55, "sofa", { edge: 0.8 });
    box(x0 + 4.37, y0 + 3.5, 0.42, 0.75, 2.7, 0.1, "sofa", { edge: 0.8 });
    box(x0 + 4.4, y0 + 5.4, 0.52, 0.18, 0.5, 0.36, "cushion");
  }, x0 + 4.8 + y0 + 4.4);
  add(x0 + 6.4, y0 + 3.8, x0 + 7.6, y0 + 5.6, () => { footShadow(x0 + 7, y0 + 4.7, 0.7); [[6.5, 3.9], [7.4, 3.9], [6.5, 5.4], [7.4, 5.4]].forEach(([a, b]) => box(x0 + a, y0 + b, 0, 0.1, 0.1, 0.34, "woodD")); box(x0 + 6.4, y0 + 3.8, 0.34, 1.2, 1.8, 0.06, "wood", { edge: 1.2 }); cyl(x0 + 6.8, y0 + 4.3, 0.4, 0.07, 0.13, "mug"); });
  beanbag(x0 + 11.2, y0 + 4.6, "beanbag2"); beanbag(x0 + 13.4, y0 + 5.6, "beanbag");
  plantObj(x0 + 1.0, y0 + 7.8, 1.2, 2); plantObj(x0 + 16.2, y0 + 1.0, 1, 1); plantObj(x0 + 16.3, y0 + 8.2, 1.1, 2);
  floorLamp(x0 + 3.4, y0 + 1.4);
}
function buildScene(plan) {
  OBJ = [];
  plan.rooms.forEach((r, i) => {
    if (r.kind === "project") buildProject(r, i);
    else if (r.kind === "meeting") buildMeeting(r);
    else if (r.kind === "pantry") buildPantry(r);
    else if (r.kind === "lounge") buildLounge(r);
  });
  // Dispenser + tanaman di lorong.
  const W = plan.W;
  add(W - 1.05, 11.2, W - 0.4, 11.85, () => { footShadow(W - 0.7, 11.55, 0.4); box(W - 1.02, 11.22, 0, 0.6, 0.6, 1.0, "cooler"); cyl(W - 0.72, 11.52, 1.0, 0.22, 0.42, tonesOf(PAL.water), { top: shade(PAL.water, 0.2) }); });
  for (let c = 0; c < plan.cols; c++) plantObj(c * COLW + 16.6, 11.45, 0.8, 1);
  // Partisi kaca antar-ruangan (sumbu y) dan setengah tinggi di tepi lorong (sumbu x).
  const xs = new Set();
  plan.rooms.forEach((r) => { if (r.x0 > 0) xs.add(`${r.x0}|${r.top ? 1 : 0}`); });
  xs.forEach((key) => { const [x, top] = key.split("|").map(Number); const [a, b] = top ? [0, 11] : [13, D]; for (let j = a; j < b; j++) glassSeg(x, j, j + 1); });
  [11, 13].forEach((yy) => {
    const doors = plan.rooms.filter((r) => r.doorY === yy).map((r) => r.door);
    for (let i = 0; i < W; i++) {
      const dk = doors.find((d) => i + 1 > d[0] && i < d[1]);
      let a = i, b = i + 1;
      if (dk) { if (a < dk[0]) b = dk[0]; else if (b > dk[1]) a = dk[1]; else continue; if (b - a < 0.02) continue; }
      halfSeg(yy, a, b);
    }
  });
}

/* ===================================================================== */
/* Karakter (vektor chibi)                                               */
/* ===================================================================== */
const FACE = { "+y": ["f", false], "+x": ["f", true], "-x": ["b", false], "-y": ["b", true] };
function poseOf(ch, seat) {
  const s = seat || {};
  if (ch.path) return { pose: "walk", arms: "walk", mood: "ok" };
  const sit = !!s.sit, st = ch.st;
  if (st === "review") return { pose: sit ? "sit" : "stand", arms: "raise", mood: "ok" };
  if (st === "decision") return { pose: sit ? "sit" : "stand", arms: "head", mood: "sad" };
  if (st === "working" || st === "online") return { pose: sit ? "sit" : "stand", arms: s.act === "tablet" ? "tablet" : sit ? "type" : "idle", mood: "ok" };
  if (st === "paused") return { pose: sit ? "sit" : "stand", arms: "idle", mood: "sleepy" };
  if (st === "meeting") return { pose: sit ? "sit" : "stand", arms: s.act === "point" ? "point" : "idle", mood: "happy" };
  if (st === "idle") return { pose: sit ? "sit" : "stand", arms: s.act || "idle", mood: s.sofa ? "sleepy" : "ok" };
  return { pose: sit ? "sit" : "stand", arms: "idle", mood: "ok" };
}
function footShadow2(X, Y, r) { const c = G.c; const g = c.createRadialGradient(X, Y, 0, X, Y, r); g.addColorStop(0, "rgba(20,24,50,0.24)"); g.addColorStop(1, "rgba(20,24,50,0)"); c.save(); c.translate(X, Y); c.scale(1, 0.5); c.translate(-X, -Y); c.beginPath(); c.arc(X, Y, r, 0, 7); c.fillStyle = g; c.fill(); c.restore(); }
function eyesFor(mood, front, y, col, blink, ox = -2) {
  const c = G.c; if (!front) return;
  const L = -4.6 + ox, R = 3.4 + ox;
  c.strokeStyle = col; c.fillStyle = col; c.lineCap = "round";
  if (mood === "happy") { c.lineWidth = 1.5; c.beginPath(); c.moveTo(L - 2, y + 0.6); c.quadraticCurveTo(L, y - 2.2, L + 2, y + 0.6); c.moveTo(R - 2, y + 0.6); c.quadraticCurveTo(R, y - 2.2, R + 2, y + 0.6); c.stroke(); return; }
  if (mood === "sleepy" || blink) { c.lineWidth = 1.4; c.beginPath(); c.moveTo(L - 1.8, y); c.lineTo(L + 1.8, y); c.moveTo(R - 1.8, y); c.lineTo(R + 1.8, y); c.stroke(); return; }
  c.beginPath(); c.ellipse(L, y, 1.7, 2.1, 0, 0, 7); c.ellipse(R, y, 1.7, 2.1, 0, 0, 7); c.fill();
  if (mood === "sad") { c.lineWidth = 1.2; c.beginPath(); c.moveTo(L - 2.2, y - 4.4); c.lineTo(L + 1.6, y - 3.2); c.moveTo(R + 2.2, y - 4.4); c.lineTo(R - 1.6, y - 3.2); c.stroke(); }
}
function blushMouth(hy, mood, ox) {
  const c = G.c;
  c.fillStyle = "rgba(255,120,130,.32)"; c.beginPath(); c.ellipse(-8 + ox, hy + 5.5, 2.4, 1.4, 0, 0, 7); c.ellipse(5.6 + ox, hy + 5.5, 2.4, 1.4, 0, 0, 7); c.fill();
  c.strokeStyle = "#8a3b2c"; c.lineWidth = 1.3; c.lineCap = "round"; c.beginPath();
  if (mood === "sad") { c.moveTo(-2.5 + ox, hy + 8.2); c.quadraticCurveTo(-0.5 + ox, hy + 6.6, 1.5 + ox, hy + 8.2); }
  else if (mood === "happy") { c.moveTo(-2.8 + ox, hy + 6.4); c.quadraticCurveTo(-0.5 + ox, hy + 9.4, 1.8 + ox, hy + 6.4); }
  else { c.moveTo(-2 + ox, hy + 7); c.quadraticCurveTo(-0.5 + ox, hy + 8.2, 1 + ox, hy + 7); }
  c.stroke();
}
function accessory(a, hy, front, ox, r, bot) {
  const c = G.c;
  if (!a || a === "none") return;
  if (a === "headset") {
    c.strokeStyle = "#2b2f3a"; c.lineWidth = 2.4; c.beginPath(); c.arc(0, hy, r + 1.2, Math.PI * 1.08, Math.PI * 1.92); c.stroke();
    c.fillStyle = "#2b2f3a"; rr(c, -r - 3, hy - 2, 4.5, 8, 2); c.fill(); rr(c, r - 1.5, hy - 2, 4.5, 8, 2); c.fill();
    if (front) { c.lineWidth = 1.4; c.beginPath(); c.moveTo(-r - 0.8, hy + 5); c.quadraticCurveTo(-r + 1, hy + 10, -5 + ox, hy + 9); c.stroke(); c.beginPath(); c.arc(-5 + ox, hy + 9, 1.4, 0, 7); c.fillStyle = "#e5484d"; c.fill(); }
  } else if (a === "glasses" && front) {
    c.strokeStyle = "#2b2f3a"; c.lineWidth = 1.2;
    const y = bot ? hy - 0.8 : hy + 1.8;
    c.beginPath(); c.arc(-4.6 + ox, y, 3.4, 0, 7); c.moveTo(3.4 + ox + 3.4, y); c.arc(3.4 + ox, y, 3.4, 0, 7); c.moveTo(-1.2 + ox, y); c.lineTo(0 + ox, y); c.stroke();
    c.fillStyle = "rgba(255,255,255,.18)"; c.beginPath(); c.arc(-4.6 + ox, y, 3.2, 0, 7); c.arc(3.4 + ox, y, 3.2, 0, 7); c.fill();
  } else if (a === "cap") {
    const col = "#e5484d";
    c.fillStyle = col; c.beginPath(); c.arc(0, hy - 3, r + 0.6, Math.PI, 0); c.closePath(); c.fill();
    c.fillStyle = shade(col, -0.2); rr(c, front ? -r - 7 : -2, hy - 4.5, front ? 13 : 0.1, 3.6, 1.8); c.fill();
    c.fillStyle = shade(col, 0.4); c.beginPath(); c.arc(0, hy - r - 2, 1.6, 0, 7); c.fill();
  } else if (a === "beanie") {
    const col = "#f2a541";
    c.fillStyle = col; c.beginPath(); c.arc(0, hy - 2, r + 1, Math.PI, 0); c.closePath(); c.fill();
    c.fillStyle = shade(col, -0.18); rr(c, -r - 1.5, hy - 4.5, 2 * r + 3, 4.5, 2); c.fill();
    c.fillStyle = shade(col, 0.35); c.beginPath(); c.arc(0, hy - r - 3, 3, 0, 7); c.fill();
  } else if (a === "beret") {
    c.fillStyle = "#2b2f3a"; c.beginPath(); c.ellipse(1, hy - r + 2, r + 2, 5.5, -0.12, 0, 7); c.fill();
    c.beginPath(); c.arc(2, hy - r - 3, 1.4, 0, 7); c.fill();
  } else if (a === "scarf") {
    c.fillStyle = "#e5484d"; rr(c, -9, hy + r - 2, 18, 4.5, 2.2); c.fill(); rr(c, 3, hy + r + 1, 4, 8, 2); c.fill();
  }
}
function humanHead(ch, hy, front, po, t, skin, hair, look) {
  const c = G.c, r = 12.5, still = G.still, ox = -1.6;
  const blink = !still && (t + ch.ph * 5) % 4.2 < 0.12;
  if (look.style === "hijab") {
    const hj = hair;
    c.beginPath(); c.moveTo(-14, hy + 15); c.bezierCurveTo(-16, hy + 2, -15, hy - 14, 0, hy - 14.5); c.bezierCurveTo(15, hy - 14, 16, hy + 2, 14, hy + 15); c.closePath(); c.fillStyle = hj; c.fill();
    c.beginPath(); c.moveTo(6, hy - 12); c.bezierCurveTo(13, hy - 6, 13, hy + 6, 9, hy + 14); c.strokeStyle = shade(hj, -0.15); c.lineWidth = 2.4; c.stroke();
    if (front) {
      c.beginPath(); c.ellipse(ox - 0.6, hy + 1, 9, 9.8, 0, 0, 7); c.fillStyle = skin; c.fill();
      eyesFor(po.mood, true, hy + 1.5, "#2a1d17", blink, ox);
      blushMouth(hy, po.mood, ox);
      accessory(look.acc, hy, true, ox, r);
    }
    return;
  }
  c.beginPath(); c.arc(0, hy, r, 0, 7); c.fillStyle = front ? skin : hair; c.fill();
  if (front) {
    c.beginPath(); c.arc(r - 1.5, hy + 1.5, 2.6, 0, 7); c.fillStyle = shade(skin, -0.06); c.fill();
    c.fillStyle = hair; c.beginPath();
    if (look.style === "long") {
      c.moveTo(-r - 1.5, hy + 13); c.bezierCurveTo(-r - 3, hy - 6, -r + 1, hy - r - 2, 0, hy - r - 1.5); c.bezierCurveTo(r + 1, hy - r - 2, r + 3, hy - 4, r + 1.5, hy + 13);
      c.lineTo(r - 3, hy + 13); c.bezierCurveTo(r - 2, hy + 2, r - 3, hy - 4, r - 5, hy - 6); c.bezierCurveTo(1, hy - 4, -5, hy - 4, -9, hy - 2); c.bezierCurveTo(-10, hy + 3, -10, hy + 8, -9, hy + 13); c.closePath();
    } else if (look.style === "bob") {
      c.moveTo(-r - 1.2, hy + 5); c.bezierCurveTo(-r - 2.5, hy - 8, -r + 2, hy - r - 1.5, 1, hy - r - 1.2); c.bezierCurveTo(r + 1.5, hy - r - 1, r + 2.2, hy - 4, r + 1, hy + 5);
      c.lineTo(r - 2.5, hy + 5); c.bezierCurveTo(r - 2, hy - 3, r - 4, hy - 5, r - 7, hy - 6); c.bezierCurveTo(1, hy - 4.5, -6, hy - 3, -9.5, hy - 1); c.lineTo(-10, hy + 5); c.closePath();
    } else {
      c.moveTo(-r - 0.5, hy + 1); c.bezierCurveTo(-r - 1.5, hy - 9, -r + 2, hy - r - 1.5, 1, hy - r - 1); c.bezierCurveTo(r + 1, hy - r - 1, r + 1.5, hy - 5, r, hy); c.bezierCurveTo(r - 4, hy - 6, -4, hy - 7, -r - 0.5, hy + 1); c.closePath();
    }
    c.fill();
    c.beginPath(); c.ellipse(-3, hy - r + 3, 4.5, 1.5, -0.25, 0, 7); c.fillStyle = "rgba(255,255,255,.22)"; c.fill();
    eyesFor(po.mood, true, hy + 1.8, "#2a1d17", blink, ox);
    blushMouth(hy, po.mood, ox);
  } else {
    if (look.style === "long") { c.beginPath(); c.moveTo(-r - 1.5, hy); c.lineTo(-r, hy + 13); c.lineTo(r, hy + 13); c.lineTo(r + 1.5, hy); c.closePath(); c.fillStyle = hair; c.fill(); }
    c.beginPath(); c.ellipse(-3, hy - 6, 5, 2.4, -0.4, 0, 7); c.fillStyle = "rgba(255,255,255,.12)"; c.fill();
    c.beginPath(); c.arc(-r + 0.5, hy + 2, 2.6, 0, 7); c.fillStyle = shade(skin, -0.06); c.fill();
  }
  accessory(look.acc, hy, front, ox, r);
}
function botHead(ch, hy, front, po, t, shell, shellD, col) {
  const c = G.c, still = G.still, ox = -1.6;
  const sc = STC[ch.path ? "walk" : ch.st] || STC.idle;
  c.strokeStyle = shellD; c.lineWidth = 1.8; c.beginPath(); c.moveTo(0, hy - 11); c.lineTo(0, hy - 17); c.stroke();
  if (ch.st === "decision" || ch.st === "review") { const pul = still ? 0.5 : (Math.sin(t * 5 + ch.ph * 6) + 1) / 2; c.beginPath(); c.arc(0, hy - 18.5, 5 + pul * 2, 0, 7); c.fillStyle = rgba(sc, 0.18 + pul * 0.12); c.fill(); }
  c.beginPath(); c.arc(0, hy - 18.5, 3, 0, 7); c.fillStyle = sc; c.fill();
  c.beginPath(); c.arc(-0.9, hy - 19.4, 0.9, 0, 7); c.fillStyle = "rgba(255,255,255,.7)"; c.fill();
  c.fillStyle = col; rr(c, -15.5, hy - 4, 4, 9, 2); c.fill(); rr(c, 11.5, hy - 4, 4, 9, 2); c.fill();
  rr(c, -13.5, hy - 12, 27, 23.5, 9);
  { const g = c.createLinearGradient(-13, 0, 13, 0); g.addColorStop(0, shade(shell, 0.04)); g.addColorStop(0.65, shell); g.addColorStop(1, shellD); c.fillStyle = g; }
  c.fill();
  if (front) {
    rr(c, -10.5 + ox * 0.6, hy - 7.5, 19.5, 14, 6); c.fillStyle = shade(col, -0.5); c.fill();
    const blink = !still && (t + ch.ph * 5) % 3.6 < 0.12;
    eyesFor(po.mood, true, hy - 0.8, "#ffffff", blink, ox + 0.4);
    if (po.mood === "happy" || po.mood === "ok") { c.strokeStyle = rgba("#ffffff", 0.75); c.lineWidth = 1.1; c.beginPath(); c.moveTo(-3 + ox, hy + 4); c.quadraticCurveTo(-0.6 + ox, hy + (po.mood === "happy" ? 6 : 5), 1.8 + ox, hy + 4); c.stroke(); }
    c.beginPath(); c.ellipse(-6, hy - 10, 4, 1.2, -0.15, 0, 7); c.fillStyle = "rgba(255,255,255,.5)"; c.fill();
  } else {
    c.fillStyle = shellD; [-3, 0, 3].forEach((d) => { rr(c, -6, hy - 2 + d, 12, 1.4, 0.7); c.fill(); });
    c.fillStyle = col; rr(c, -13.5, hy + 6, 27, 3, 1.5); c.fill();
  }
  accessory(ch.look.acc, hy, front, ox, 13, true);
}
function bubble(ch, X, Y, hy, po, t) {
  const c = G.c, still = G.still, top = Y + hy - 14;
  const bx = X + 15, byy = top - 4;
  const pill = (w, h) => { rr(c, bx - 2, byy - h / 2, w, h, h / 2); c.fillStyle = "rgba(255,255,255,.96)"; c.fill(); c.strokeStyle = "rgba(30,30,60,.14)"; c.lineWidth = 1; c.stroke(); };
  if (ch.st === "working" && !ch.path && po.arms === "type" && ch.bot) {
    pill(20, 11);
    for (let i = 0; i < 3; i++) { const a = still ? 0.6 : 0.3 + 0.7 * Math.max(0, Math.sin(t * 6 - i * 0.9)); c.beginPath(); c.arc(bx + 3 + i * 5, byy, 1.6, 0, 7); c.fillStyle = `rgba(91,91,214,${a})`; c.fill(); }
  } else if (ch.st === "meeting" && ch.bot) {
    pill(15, 15); drawIcon(c, "bulb", bx - 0.5, byy - 5.5, 11, "#d38a00", 2.4);
  } else if ((ch.st === "idle" || ch.st === "paused") && po.mood === "sleepy" && !ch.path) {
    c.fillStyle = "rgba(90,95,130,.7)"; c.font = "600 8px Geist, system-ui, sans-serif";
    for (let i = 0; i < 2; i++) { const tt = still ? 0.4 + i * 0.3 : (t * 0.35 + i * 0.5) % 1; c.globalAlpha = Math.sin(tt * Math.PI); c.fillText("z", X + 9 + tt * 6 + i * 3, top + 2 - tt * 12); } c.globalAlpha = 1;
  } else if (ch.st === "decision") {
    const tt = still ? 0.3 : (t * 0.7) % 1; c.beginPath(); const sx = X + 14, sy = top + 12 + tt * 6; c.moveTo(sx, sy - 4); c.quadraticCurveTo(sx + 3, sy, sx, sy + 2); c.quadraticCurveTo(sx - 3, sy, sx, sy - 4); c.fillStyle = `rgba(120,190,255,${0.9 - tt * 0.6})`; c.fill();
  }
}
function drawChar(ch, X, Y, face, flip, po, t, sel) {
  const c = G.c, look = ch.look || {}, bot = ch.bot;
  const shirt = isHex(look.shirt) ? look.shirt : "#5b5bd6", shirtT = tonesOf(shirt);
  const skin = isHex(look.skin) ? look.skin : "#e8b996", hair = isHex(look.hair) ? look.hair : "#2a2a2a";
  const pants = PAL.pants, shoes = PAL.shoes, shell = PAL.shell, shellD = PAL.shellD;
  const ph = ch.ph * 6.283, still = G.still;
  const sit = po.pose === "sit", walk = po.pose === "walk";
  const bob = still ? 0 : walk ? Math.abs(Math.sin(t * 9 + ph)) * -1.6 : po.arms === "type" ? Math.sin(t * 3 + ph) * 0.35 : Math.sin(t * 2 + ph) * 0.6;
  const dy = (sit ? 2 : 0) + bob;
  if (sel) {
    c.save(); c.translate(X, Y); c.scale(1, 0.5);
    const pul = still ? 0 : (Math.sin(t * 4) + 1) / 2;
    c.beginPath(); c.arc(0, 0, 21 + pul * 2, 0, 7); c.fillStyle = "rgba(91,91,214,.16)"; c.fill();
    c.lineWidth = lw(2) * 2; c.strokeStyle = "#5b5bd6"; c.stroke(); c.restore();
  }
  if (ch.st === "decision" && !walk) { c.save(); c.translate(X, Y); c.scale(1, 0.5); const pul = still ? 0.5 : (t * 0.8) % 1; c.beginPath(); c.arc(0, 0, 14 + pul * 14, 0, 7); c.strokeStyle = `rgba(229,72,77,${0.7 * (1 - pul)})`; c.lineWidth = lw(2) * 2; c.stroke(); c.restore(); }
  if (!sit) footShadow2(X, Y, 11);
  c.save(); c.translate(X, Y); if (flip) c.scale(-1, 1);
  const front = face === "f";
  if (sit) {
    if (front) { rr(c, -8, -12 + dy, 16, 6, 3); c.fillStyle = pants; c.fill(); rr(c, -7, -7 + dy, 5, 7, 2); c.fill(); rr(c, 2, -7 + dy, 5, 7, 2); c.fill(); c.fillStyle = shoes; rr(c, -8, -1.5 + dy, 6.5, 3, 1.5); c.fill(); rr(c, 1.5, -1.5 + dy, 6.5, 3, 1.5); c.fill(); }
  } else {
    const sw = walk && !still ? Math.sin(t * 9 + ph) * 2.6 : 0;
    c.fillStyle = bot ? shellD : pants;
    rr(c, -6.5, -12 + dy, 5.5, 12 - dy + sw * 0.3 - 1, 2.4); c.fill();
    rr(c, 1, -12 + dy, 5.5, 12 - dy - sw * 0.3 - 1, 2.4); c.fill();
    c.fillStyle = bot ? shell : shoes;
    rr(c, -7.5 + (front ? -0.5 : 0.5), -2.4 + sw * 0.3, 7, 3.4, 1.7); c.fill();
    rr(c, 0.5 + (front ? -0.5 : 0.5), -2.4 - sw * 0.3, 7, 3.4, 1.7); c.fill();
  }
  const by = -10 + dy;
  const sh = (a) => [a > 0 ? 8.5 : -8.5, by - 13];
  const arm = (from, to, col) => { c.beginPath(); c.moveTo(from[0], from[1]); c.lineTo(to[0], to[1]); c.lineCap = "round"; c.strokeStyle = col; c.lineWidth = 5.4; c.stroke(); };
  const hand = (p, col = bot ? shell : skin) => { c.beginPath(); c.arc(p[0], p[1], 2.9, 0, 7); c.fillStyle = col; c.fill(); };
  const armCol = shirtT.l, armColB = shirtT.r;
  const wave = still ? 0 : Math.sin(t * 7 + ph) * 3.2, tap = still ? 0 : Math.sin(t * 16 + ph * 3), tap2 = still ? 0 : Math.sin(t * 16 + ph * 3 + 2.2);
  const walkSw = walk && !still ? Math.sin(t * 9 + ph) * 3 : 0;
  if (!front) { arm(sh(-1), [-10.5, by - 3 + walkSw], armColB); arm(sh(1), [10.5, by - 3 - walkSw], armColB); }
  else if (po.arms === "idle" || po.arms === "walk") arm(sh(1), [10.5, by - 2 - walkSw], armColB);
  c.beginPath(); c.moveTo(-9.5, by); c.bezierCurveTo(-10.5, by - 9, -9, by - 15, 0, by - 15); c.bezierCurveTo(9, by - 15, 10.5, by - 9, 9.5, by); c.closePath();
  { const g = c.createLinearGradient(-10, 0, 10, 0); g.addColorStop(0, shade(shirt, 0.06)); g.addColorStop(0.6, shirt); g.addColorStop(1, shirtT.r); c.fillStyle = g; }
  c.fill();
  if (front) {
    if (bot) { c.beginPath(); c.arc(-1.2, by - 7, 4.6, 0, 7); c.fillStyle = shade(shirt, 0.82); c.fill(); drawIcon(c, ch.icon || "robot", -5.2, by - 11, 8, shirt, 2.8); }
    else { c.beginPath(); c.moveTo(-4, by - 14.6); c.lineTo(-1, by - 10.5); c.lineTo(2, by - 14.6); c.fillStyle = skin; c.fill(); }
  } else if (bot) { rr(c, -4, by - 12, 8, 6, 2); c.fillStyle = shade(shirt, -0.2); c.fill(); }
  const late = [];
  if (front) {
    const a = po.arms;
    if (a === "type") { arm(sh(-1), [-6.5, by - 3 + tap * 0.9], armCol); hand([-6.5, by - 3 + tap * 0.9]); arm(sh(1), [4, by - 3 + tap2 * 0.9], armColB); hand([4, by - 3 + tap2 * 0.9]); }
    else if (a === "raise") { arm(sh(-1), [-10.5, by - 2], armCol); hand([-10.5, by - 2]); const hx = 16 + wave * 0.5, hyy = by - 31; late.push(() => { arm(sh(1), [hx - 0.5, hyy + 3], armColB); c.beginPath(); c.arc(hx, hyy, 3.6, 0, 7); c.fillStyle = bot ? shell : skin; c.fill(); c.strokeStyle = rgba("#000000", 0.12); c.lineWidth = 0.8; c.stroke(); }); }
    else if (a === "head") late.push(() => { arm(sh(-1), [-13.5, by - 21], armCol); hand([-13.5, by - 21.5]); arm(sh(1), [13.5, by - 21], armColB); hand([13.5, by - 21.5]); });
    else if (a === "mug") { arm(sh(-1), [-4.5, by - 6], armCol); rr(c, -7.5, by - 11.5, 6, 7, 1.5); c.fillStyle = "#ffffff"; c.fill(); c.fillStyle = shade(shirt, -0.1); c.fillRect(-7.5, by - 9, 6, 1.6); hand([-4.5, by - 6]); if (!still) { const tt = (t * 0.6 + ch.ph) % 1; c.beginPath(); c.moveTo(-4.5, by - 13 - tt * 7); c.quadraticCurveTo(-7, by - 16 - tt * 7, -4.5, by - 19 - tt * 7); c.strokeStyle = `rgba(255,255,255,${0.7 * (1 - tt)})`; c.lineWidth = 1.2; c.stroke(); } }
    else if (a === "tablet") { arm(sh(-1), [-5, by - 5], armCol); arm(sh(1), [5, by - 5], armColB); c.save(); c.transform(1, -0.25, 0, 1, 0, 0); rr(c, -6.5, by - 9, 13, 8, 1.5); c.fillStyle = "#2c3140"; c.fill(); c.fillStyle = "#8fc0ff"; c.fillRect(-5.3, by - 7.8, 10.6, 5.6); c.restore(); hand([-5.5, by - 5]); hand([5.5, by - 5]); }
    else if (a === "point") { arm(sh(-1), [-10, by - 2], armCol); hand([-10, by - 2]); late.push(() => { arm(sh(1), [17, by - 21 + wave * 0.2], armColB); hand([17.5, by - 21.5 + wave * 0.2]); }); }
    else { arm(sh(-1), [-10.5, by - 2 + walkSw], armCol); hand([-10.5, by - 2 + walkSw]); hand([10.5, by - 2 - walkSw]); }
  } else { hand([-10.5, by - 3 + walkSw]); hand([10.5, by - 3 - walkSw]); }
  const hy = by - 26;
  if (bot) botHead(ch, hy, front, po, t, shell, shellD, shirt);
  else humanHead(ch, hy, front, po, t, skin, hair, look);
  late.forEach((f) => f());
  c.restore();
  bubble(ch, X, Y, hy, po, t);
}

/* ===================================================================== */
/* Render adegan                                                         */
/* ===================================================================== */
function worldBox(plan) {
  const xs = [P(0, D)[0] - 12, P(plan.W, 0)[0] + 12];
  return { x0: xs[0], x1: xs[1], y0: P(0, 0, WH)[1] - 60, y1: P(plan.W, D, -0.6)[1] + 54 };
}
const RM_BOX = (r) => [r.x0, r.y0, r.x1, r.y1, r.top ? WH : 1.4];
function wallPaint(plan) {
  const seg = (side, a, b, col) => {
    const Pw = (u, z) => (side === "back" ? P(u, 0, z) : P(0, u, z));
    const t = tonesOf(col);
    fillP([Pw(a, 0), Pw(b, 0), Pw(b, 0.95), Pw(a, 0.95)], side === "back" ? t.l : t.r);
    fillP([Pw(a, 0.95), Pw(b, 0.95), Pw(b, 1.0), Pw(a, 1.0)], side === "back" ? shade(t.l, 0.25) : shade(t.r, 0.2));
    fillP([Pw(a, 0), Pw(b, 0), Pw(b, 0.1), Pw(a, 0.1)], side === "back" ? tonesOf(PAL.base).l : tonesOf(PAL.base).r);
  };
  const tint = (c) => mix(c, "#ffffff", 0.8);
  plan.rooms.filter((r) => r.top).forEach((r) => seg("back", r.x0, r.x1, tint(r.c)));
  const left = plan.rooms.filter((r) => r.x0 === 0);
  left.forEach((r) => seg("left", r.y0, r.y1, tint(r.c)));
  seg("left", 11, 13, mix(PAL.fCOR, "#ffffff", 0.2));
  line(P(0, 0, 0), P(0, 0, WH), rgba("#000000", 0.08), 1);
}
function lightPatches(wins) {
  const c = G.c, col = "rgba(255,248,215,0.55)";
  c.save(); c.globalCompositeOperation = "soft-light";
  wins.back.forEach(([a, b]) => { const pts = [P(a + 0.3, 0), P(b + 0.3, 0), P(b + 1.6, 3.4), P(a + 1.6, 3.4)]; const g = c.createLinearGradient(...P((a + b) / 2, 0), ...P((a + b) / 2 + 1, 3.4)); g.addColorStop(0, col); g.addColorStop(1, "rgba(255,248,215,0)"); fillP(pts, g); });
  wins.left.forEach(([a, b]) => { const pts = [P(0, a + 0.3), P(0, b + 0.3), P(3.4, b + 1.6), P(3.4, a + 1.6)]; const g = c.createLinearGradient(...P(0, (a + b) / 2), ...P(3.4, (a + b) / 2 + 1)); g.addColorStop(0, col); g.addColorStop(1, "rgba(255,248,215,0)"); fillP(pts, g); });
  c.restore();
}
function windowsOf(plan) {
  const back = [], left = [[1.2, 3.8], [18.6, 21.0]];
  plan.rooms.filter((r) => r.top).forEach((r) => {
    if (r.kind === "project") { back.push([r.x0 + 1.0, r.x0 + 3.6]); back.push([r.x0 + 13.9, r.x0 + 16.0]); }
    else if (r.kind === "meeting") back.push([r.x0 + 7.0, r.x0 + 9.4]);
    else back.push([r.x0 + 2.0, r.x0 + 5.0], [r.x0 + 11.0, r.x0 + 14.0]);
  });
  return { back, left };
}
function drawScene(c, V, t, S) {
  const plan = S.plan;
  G = { c, sc: V.sc, t, still: !!S.still };
  const cw = c.canvas.width, chh = c.canvas.height;
  c.setTransform(1, 0, 0, 1, 0, 0);
  const dark = !!S.dark;
  const g = c.createRadialGradient(cw * 0.5, chh * 0.42, 0, cw * 0.5, chh * 0.5, Math.max(cw, chh) * 0.75);
  g.addColorStop(0, dark ? PAL.bgDA : PAL.bgA); g.addColorStop(1, dark ? PAL.bgDB : PAL.bgB);
  c.fillStyle = g; c.fillRect(0, 0, cw, chh);
  c.setTransform(V.sc, 0, 0, V.sc, V.ox, V.oy);
  const W = plan.W;
  const slabPts = [P(-0.25, -0.25, 0), P(W, -0.25, 0), P(W, -0.25, -0.6), P(W, D, -0.6), P(-0.25, D, -0.6), P(-0.25, D, 0)];
  c.save(); c.shadowColor = dark ? "rgba(0,0,0,.5)" : "rgba(60,70,120,.22)"; c.shadowBlur = 46 * V.sc; c.shadowOffsetY = 26 * V.sc;
  fillP(slabPts, mat("slab").r); c.restore();
  box(-0.25, -0.25, -0.6, W + 0.25, D + 0.25, 0.6, "slab", { edge: 0.4 });
  box(-0.25, -0.25, 0, 0.25, D + 0.25, WH, "wallL", { edge: 0.6 });
  box(0, -0.25, 0, W, 0.25, WH, "wall", { edge: 0.6 });
  wallPaint(plan);
  plan.rooms.forEach((r) => floorArea(r.x0, r.y0, r.x1, r.y1, r.fl[2], r.fl[0], r.fl[1]));
  floorArea(0, 11, W, 13, "wood", "fCOR", "fCOR2");
  const wins = S.wins;
  lightPatches(wins);
  plan.rooms.forEach((r) => {
    if (r.kind === "project") { const dy = r.front ? 12 : 0; rug(r.x0 + 1.4, 2.3 + dy, r.x0 + 7.4, 7.7 + dy, "rugB", "rugB2"); rug(r.x0 + 9.4, 2.3 + dy, r.x0 + 15.4, 7.7 + dy, "rugB", "rugB2"); }
    else if (r.kind === "pantry") rug(r.x0 + 1.7, 15.5, r.x0 + 4.5, 19.3, "rug", "rug2");
    else if (r.kind === "lounge") rug(r.x0 + 3.6, r.y0 + 2.6, r.x0 + 9.0, r.y0 + 7.2, "rug", "rug2");
  });
  // Bayangan statis pod meja.
  plan.rooms.filter((r) => r.kind === "project").forEach((r) => { const dy = r.front ? 12 : 0; [[2, 6.8], [10, 14.8]].forEach(([a, b]) => softShadow([P(r.x0 + a + 0.12, 4.12 + dy), P(r.x0 + b + 0.35, 4.12 + dy), P(r.x0 + b + 0.35, 6.3 + dy), P(r.x0 + a + 0.12, 6.3 + dy)], 0.9, 0.12)); });
  wins.back.forEach(([a, b]) => windowOn("back", a, b));
  wins.left.forEach(([a, b]) => windowOn("left", a, b));
  plan.rooms.filter((r) => r.top).forEach((r) => {
    if (r.kind === "project") { kanban(r.x0 + 4.6, r.x0 + 12.3, 0.9, 2.5, r, S.chars); wallClock(r.x0 + 13.15, 2.0); }
    else if (r.kind === "meeting") tvWall(r.x0 + 2.5, r.x0 + 6.2, 1.05, 2.4, S.chars.some((ch) => ch.room === r.k && ch.st === "meeting"));
  });
  doorLeft(11.4, 12.6);
  const list = OBJ.slice();
  S.chars.forEach((ch) => { if (ch.hidden) return; list.push({ k: ch.x + ch.y + 0.01, ch }); });
  list.sort((a, b) => a.k - b.k);
  for (const o of list) {
    if (o.ch) {
      const ch = o.ch, seat = S.seats[ch.seat], [X, Y] = P(ch.x, ch.y, 0);
      const [face, flip] = ch.path && ch.dir ? FACE[ch.dir] : FACE[(seat && seat.f) || "+y"];
      drawChar(ch, X, Y, face, flip, poseOf(ch, seat), t, S.sel === ch.id);
    } else o.draw();
  }
  if (S.room && S.room !== "all" && plan.byKey[S.room]) {
    const [x0, y0, x1, y1, h] = RM_BOX(plan.byKey[S.room]);
    c.save(); c.setTransform(1, 0, 0, 1, 0, 0); c.beginPath(); c.rect(0, 0, cw, chh);
    c.setTransform(V.sc, 0, 0, V.sc, V.ox, V.oy);
    const hx = [P(x0, y0, h), P(x1, y0, h), P(x1, y0, 0), P(x1, y1, 0), P(x0, y1, 0), P(x0, y1, h)];
    c.moveTo(...hx[0]); hx.slice(1).forEach((p) => c.lineTo(...p)); c.closePath();
    c.fillStyle = dark ? "rgba(17,17,21,.6)" : "rgba(241,244,251,.66)"; c.fill("evenodd"); c.restore();
  }
  c.setTransform(1, 0, 0, 1, 0, 0);
}

/* Pratinjau karakter kecil (panel detail, dialog kustom). */
export function paintPreview(cv, ch, zoom = 1.05) {
  if (!cv || !ch) return;
  const c = cv.getContext("2d"), w = cv.width, h = cv.height;
  const po = { ...poseOf({ ...ch, path: null }, { sit: 0 }), pose: "stand" };
  if (po.arms === "type") po.arms = "idle";
  G = { c, sc: zoom, t: 0.8, still: true };
  c.setTransform(1, 0, 0, 1, 0, 0); c.fillStyle = "#f1f3f9"; c.fillRect(0, 0, w, h);
  c.setTransform(zoom, 0, 0, zoom, w / 2, h * 0.84);
  fillP([P(-0.9, -0.9), P(0.9, -0.9), P(0.9, 0.9), P(-0.9, 0.9)], PAL.fA);
  drawChar({ ...ch, ph: 0.3, path: null, st: ch.st === "working" ? "online" : ch.st }, 0, 0, "f", false, po, 0.8, false);
  c.setTransform(1, 0, 0, 1, 0, 0);
}

/* ===================================================================== */
/* Instans kantor                                                        */
/* ===================================================================== */
const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const isDark = () => { const r = document.documentElement; return r.dataset.theme ? r.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches; };
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (m) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[m]);

export function setIcons(map) { ICONS = map; }

/* `opts`: { stage, labels, mm, i18n, onSelect(id), onRoomsChange(rooms) } */
export function mount(opts) {
  const { stage, labels: lab, mm, i18n } = opts;
  const cv = stage.querySelector("[data-office-cv]");
  const S = { plan: planFrom([]), seats: {}, chars: [], wins: { back: [], left: [] }, sel: opts.selected || null, room: "all", dark: isDark(), still: reduced() };
  let WB = worldBox(S.plan);
  let W0 = 0, H0 = 0, dpr = 1, alive = true, last = performance.now(), lastDraw = 0, dirty = true, anim = null, sig = "";
  const cam = { z: 1, cx: 0, cy: 0, fit: true };
  const clampZ = (z) => Math.max(0.3, Math.min(2.6, z));
  const fitBox = (b, pad = 0.94) => { const z = clampZ(Math.min(W0 / (b.x1 - b.x0), H0 / (b.y1 - b.y0)) * pad); return { z, cx: (b.x0 + b.x1) / 2, cy: (b.y0 + b.y1) / 2 }; };
  const roomBox = (k) => { const r = S.plan.byKey[k]; const [x0, y0, x1, y1, h] = RM_BOX(r); const pts = [P(x0, y0, h), P(x1, y0, h), P(x1, y1, 0), P(x0, y1, 0), P(x1, y0, 0), P(x0, y1, h)]; return { x0: Math.min(...pts.map((p) => p[0])) - 30, x1: Math.max(...pts.map((p) => p[0])) + 30, y0: Math.min(...pts.map((p) => p[1])) - 70, y1: Math.max(...pts.map((p) => p[1])) + 50 }; };
  /* Tampilan awal: seluruh kantor bila muat dengan nyaman; bila tidak, zoom sedang ke ruangan pertama. */
  const homeView = () => {
    const f = fitBox(WB), want = Math.max(f.z, Math.min(1, Math.max(0.55, W0 / 1180)));
    if (want <= f.z + 0.02) return f;
    const [hx, hy] = P(8.5, 6.2, 0.9), k = Math.min(1, (want - f.z) / 0.25);
    return { z: want, cx: f.cx + (hx - f.cx) * k, cy: f.cy + (hy - f.cy) * k };
  };
  const resize = () => {
    const r = stage.getBoundingClientRect(); dpr = Math.min(2, window.devicePixelRatio || 1);
    W0 = Math.max(1, r.width); H0 = Math.max(1, r.height);
    cv.width = Math.round(W0 * dpr); cv.height = Math.round(H0 * dpr);
    if (cam.fit) Object.assign(cam, homeView(), { fit: true });
    dirty = true;
  };
  const ro = new ResizeObserver(resize); ro.observe(stage);
  const toScreen = (wx, wy) => [W0 / 2 + (wx - cam.cx) * cam.z, H0 / 2 + (wy - cam.cy) * cam.z];
  const toWorld = (sx, sy) => [cam.cx + (sx - W0 / 2) / cam.z, cam.cy + (sy - H0 / 2) / cam.z];
  const fly = (to, ms = 480) => { if (reduced()) { Object.assign(cam, to, { fit: false }); dirty = true; return; } anim = { from: { z: cam.z, cx: cam.cx, cy: cam.cy }, to, t: performance.now(), ms }; cam.fit = false; };
  const hint = stage.querySelector("[data-office-hint]");
  const hideHint = () => hint?.classList.add("gone");
  const zoomAt = (sx, sy, f) => { const [wx, wy] = toWorld(sx, sy); const z = clampZ(cam.z * f); cam.cx = wx - (sx - W0 / 2) / z; cam.cy = wy - (sy - H0 / 2) / z; cam.z = z; cam.fit = false; anim = null; dirty = true; hideHint(); };
  const pan = (dx, dy) => { cam.cx -= dx / cam.z; cam.cy -= dy / cam.z; cam.fit = false; anim = null; dirty = true; hideHint(); };
  const charAt = (sx, sy) => {
    let best = null, bk = -1;
    S.chars.forEach((ch) => { if (ch.hidden) return; const [X, Y] = toScreen(...P(ch.x, ch.y)); const z = cam.z; if (sx > X - 16 * z && sx < X + 16 * z && sy > Y - 66 * z && sy < Y + 6 * z) { const k = ch.x + ch.y; if (k > bk) { bk = k; best = ch; } } });
    return best;
  };

  /* ---- data: penempatan kursi + jalan ---- */
  const free = (roomKey, kind) => {
    const r = S.plan.byKey[roomKey]; if (!r) return null;
    const taken = new Set(S.chars.map((c) => c.seatTo || c.seat));
    const list = seatsFor(r).map((s) => s.id);
    const pref = kind === "human" ? list.filter((id) => /:b[24]$|:m2|:m3|:s/.test(id)).concat(list) : kind === "meeting-bot" ? ["MEET:mp", ...list] : list;
    return pref.find((id) => !taken.has(id)) || null;
  };
  const route = (fromSeat, toSeat, fromXY) => {
    const A = S.seats[fromSeat], B = S.seats[toSeat]; const pts = [];
    if (A && !fromXY) A.out.forEach((p) => pts.push(p));
    const ra = A && S.plan.byKey[A.room], rb = S.plan.byKey[B.room];
    if (ra !== rb && rb) {
      if (ra) pts.push([(ra.door[0] + ra.door[1]) / 2, 12]);
      pts.push([(rb.door[0] + rb.door[1]) / 2, 12]);
    }
    B.out.slice().reverse().forEach((p) => pts.push(p));
    pts.push([B.x, B.y]);
    return pts;
  };
  const walkTo = (c, toSeat) => {
    if (!toSeat || c.seat === toSeat) return;
    if (reduced() || !S.seats[c.seat]) { const s = S.seats[toSeat]; Object.assign(c, { seat: toSeat, x: s.x, y: s.y, path: null, seatTo: null }); return; }
    c.path = route(c.seat, toSeat); c.pi = 0; c.seatTo = toSeat;
  };
  const stepWalkers = (dt) => {
    let moving = false;
    S.chars.forEach((c) => {
      if (!c.path) return; moving = true;
      let rem = dt * 2.4;
      while (rem > 0 && c.path) {
        const [tx, ty] = c.path[c.pi], dx = tx - c.x, dy = ty - c.y, dist = Math.hypot(dx, dy);
        if (dist > 0.001) c.dir = Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? "+x" : "-x") : dy > 0 ? "+y" : "-y";
        if (dist <= rem) { c.x = tx; c.y = ty; rem -= dist; c.pi++; if (c.pi >= c.path.length) { c.seat = c.seatTo; c.seatTo = null; c.path = null; c.dir = null; } }
        else { c.x += (dx / dist) * rem; c.y += (dy / dist) * rem; rem = 0; }
      }
    });
    return moving;
  };
  const setState = (data) => {
    const rooms = data.rooms || [];
    const roomSig = JSON.stringify(rooms.map((r) => [r.k, r.kind, r.col, r.front, r.n, r.c]));
    if (roomSig !== sig) {
      sig = roomSig;
      S.plan = planFrom(rooms); S.wins = windowsOf(S.plan); WB = worldBox(S.plan);
      S.seats = {}; S.plan.rooms.forEach((r) => seatsFor(r).forEach((s) => { S.seats[s.id] = { ...s, room: r.k }; }));
      buildScene(S.plan);
      S.chars.forEach((c) => { if (!S.seats[c.seat]) { c.seat = null; c.path = null; } });
      if (cam.fit) Object.assign(cam, homeView(), { fit: true });
      Object.values(LB).forEach((el) => el.remove()); for (const k in LB) delete LB[k];
    }
    const prev = new Map(S.chars.map((c) => [c.id, c]));
    const gone = S.chars.filter((c) => !data.chars.some((d) => d.id === c.id));
    const next = data.chars.map((d, i) => {
      let c = prev.get(d.id);
      const placeIn = d.room;
      if (!c) {
        // Persona yang tadi menganggur muncul lagi dengan job: berjalan dari posisinya.
        const from = gone.find((g) => g.bot && d.bot && g.key === d.key);
        c = { ph: (i * 0.37 + d.id.length * 0.13) % 1, x: 0, y: 0, seat: null, path: null };
        if (from) Object.assign(c, { x: from.x, y: from.y, seat: from.seat });
      }
      Object.assign(c, d, { hidden: !S.plan.byKey[placeIn] });
      c.want = placeIn;
      return c;
    });
    S.chars = next;
    next.forEach((c) => {
      if (c.hidden) return;
      const cur = c.seatTo || c.seat;
      if (cur && S.seats[cur] && S.seats[cur].room === c.want) return;
      const kind = !c.bot ? "human" : c.st === "meeting" ? "meeting-bot" : "bot";
      const s = free(c.want, kind);
      if (!s) { c.hidden = true; return; }
      if (!c.seat) { const st = S.seats[s]; Object.assign(c, { seat: s, x: st.x, y: st.y }); }
      else walkTo(c, s);
    });
    OCC = {}; S.chars.forEach((c) => { if (!c.path && c.seat) OCC[c.seat] = c; });
    S.data = data;
    dirty = true;
  };

  /* ---- pointer: geser, klik, pinch ---- */
  const ptrs = new Map(); let down = null, pinch = null;
  stage.addEventListener("pointerdown", (e) => {
    if (e.target.closest(".app-of-ov, .app-of-lb, .app-of-rl")) return;
    stage.setPointerCapture(e.pointerId); ptrs.set(e.pointerId, [e.clientX, e.clientY]);
    if (ptrs.size === 2) { const [a, b] = [...ptrs.values()]; pinch = { d: Math.hypot(a[0] - b[0], a[1] - b[1]), z: cam.z }; down = null; return; }
    down = { x: e.clientX, y: e.clientY, moved: false };
  });
  stage.addEventListener("pointermove", (e) => {
    const r = stage.getBoundingClientRect();
    if (ptrs.has(e.pointerId)) {
      const prevP = ptrs.get(e.pointerId); ptrs.set(e.pointerId, [e.clientX, e.clientY]);
      if (pinch && ptrs.size === 2) { const [a, b] = [...ptrs.values()]; const d = Math.hypot(a[0] - b[0], a[1] - b[1]); const f = (pinch.z * (d / pinch.d)) / cam.z; zoomAt((a[0] + b[0]) / 2 - r.left, (a[1] + b[1]) / 2 - r.top, f); return; }
      if (down) { if (!down.moved && Math.hypot(e.clientX - down.x, e.clientY - down.y) > 4) { down.moved = true; stage.classList.add("drag"); } if (down.moved) pan(e.clientX - prevP[0], e.clientY - prevP[1]); }
      return;
    }
    stage.classList.toggle("hot", !!charAt(e.clientX - r.left, e.clientY - r.top));
  });
  const up = (e) => {
    const r = stage.getBoundingClientRect();
    if (down && !down.moved && ptrs.has(e.pointerId)) { const ch = charAt(e.clientX - r.left, e.clientY - r.top); opts.onSelect?.(ch ? ch.id : null, false); }
    ptrs.delete(e.pointerId); if (ptrs.size < 2) pinch = null; down = null; stage.classList.remove("drag");
  };
  stage.addEventListener("pointerup", up); stage.addEventListener("pointercancel", up);
  stage.addEventListener("wheel", (e) => {
    e.preventDefault();
    const r = stage.getBoundingClientRect();
    if (e.ctrlKey || e.metaKey) zoomAt(e.clientX - r.left, e.clientY - r.top, Math.exp(-e.deltaY * 0.0022));
    else pan(-e.deltaX, -e.deltaY);
  }, { passive: false });
  stage.addEventListener("keydown", (e) => {
    if (e.target !== stage) return;
    const k = e.key, s = 60;
    if (k === "ArrowLeft") pan(s, 0); else if (k === "ArrowRight") pan(-s, 0); else if (k === "ArrowUp") pan(0, s); else if (k === "ArrowDown") pan(0, -s);
    else if (k === "+" || k === "=") zoomAt(W0 / 2, H0 / 2, 1.2); else if (k === "-") zoomAt(W0 / 2, H0 / 2, 1 / 1.2); else if (k === "0") api.fit(); else return;
    e.preventDefault();
  });

  /* ---- mini-map ---- */
  const mmc = mm?.getContext("2d");
  const mmGeom = () => { const w = mm.clientWidth, h = mm.clientHeight, s = Math.min((w - 12) / (WB.x1 - WB.x0), (h - 12) / (WB.y1 - WB.y0)); return { w, h, s, ox: w / 2 - ((WB.x0 + WB.x1) / 2) * s, oy: h / 2 - ((WB.y0 + WB.y1) / 2) * s }; };
  const drawMini = () => {
    if (!mmc || !mm.clientWidth) return;
    const g = mmGeom(), d = Math.min(2, window.devicePixelRatio || 1);
    if (mm.width !== Math.round(g.w * d)) { mm.width = Math.round(g.w * d); mm.height = Math.round(g.h * d); }
    const c = mmc, dark = S.dark;
    c.setTransform(d, 0, 0, d, 0, 0); c.clearRect(0, 0, g.w, g.h);
    c.fillStyle = dark ? "#1c1c22" : "#ffffff"; c.fillRect(0, 0, g.w, g.h);
    const M = (x, y) => { const [a, b] = P(x, y); return [g.ox + a * g.s, g.oy + b * g.s]; };
    const q = (x0, y0, x1, y1, col) => { c.beginPath(); [M(x0, y0), M(x1, y0), M(x1, y1), M(x0, y1)].forEach((p, i) => (i ? c.lineTo(...p) : c.moveTo(...p))); c.closePath(); c.fillStyle = col; c.fill(); c.strokeStyle = dark ? "rgba(255,255,255,.18)" : "rgba(0,0,0,.14)"; c.lineWidth = 1; c.stroke(); };
    S.plan.rooms.forEach((r) => q(r.x0, r.y0, r.x1, r.y1, rgba(isHex(r.c) ? r.c : "#888888", S.room === r.k ? 0.55 : dark ? 0.3 : 0.2)));
    q(0, 11, S.plan.W, 13, dark ? "rgba(255,255,255,.06)" : "rgba(0,0,0,.05)");
    S.chars.forEach((ch) => { if (ch.hidden) return; const [x, y] = M(ch.x, ch.y); c.beginPath(); c.arc(x, y, ch.id === S.sel ? 3.4 : 2.4, 0, 7); c.fillStyle = STC[ch.st] || STC.idle; c.fill(); });
    const [a0, b0] = toWorld(0, 0), [a1, b1] = toWorld(W0, H0);
    c.strokeStyle = "#5b5bd6"; c.lineWidth = 1.5; c.strokeRect(g.ox + a0 * g.s, g.oy + b0 * g.s, (a1 - a0) * g.s, (b1 - b0) * g.s);
    c.fillStyle = "rgba(91,91,214,.08)"; c.fillRect(g.ox + a0 * g.s, g.oy + b0 * g.s, (a1 - a0) * g.s, (b1 - b0) * g.s);
  };
  if (mm) {
    let mmDown = false;
    const mmGo = (e) => { const r = mm.getBoundingClientRect(), g = mmGeom(); cam.cx = (e.clientX - r.left - g.ox) / g.s; cam.cy = (e.clientY - r.top - g.oy) / g.s; cam.fit = false; anim = null; dirty = true; };
    mm.addEventListener("pointerdown", (e) => { e.stopPropagation(); mmDown = true; mm.setPointerCapture(e.pointerId); mmGo(e); });
    mm.addEventListener("pointermove", (e) => { if (mmDown) mmGo(e); });
    mm.addEventListener("pointerup", () => { mmDown = false; });
  }

  /* ---- label HTML (nama karakter, ruangan) ---- */
  const LB = {};
  const tr = (k, n) => (i18n[k] || k).replace("{n}", n);
  const roomName = (r) => (r.kind === "meeting" ? tr("room-meeting") : r.kind === "pantry" ? tr("room-pantry") : r.kind === "lounge" ? tr("room-lounge") : r.n);
  const roomSub = (r) => {
    const here = S.chars.filter((c) => !c.path && !c.hidden && c.seat && S.seats[c.seat]?.room === r.k);
    if (r.kind === "pantry") return here.length ? tr("sub-idle", here.length) : tr("sub-quiet");
    if (r.kind === "meeting") return here.some((c) => c.st === "meeting") ? tr("sub-meeting") : tr("sub-quiet");
    if (r.kind === "lounge") return tr("sub-quiet");
    if (!here.length) return tr("sub-empty");
    const a = here.filter((c) => c.bot).length, n = here.filter((c) => c.act).length;
    return tr("sub-agents", a) + (n ? " · " + tr("sub-todo", n) : "");
  };
  const ICON_SVG = (n) => opts.iconSvg?.(n) || "";
  const labelHtml = (ch) => {
    const col = STC[ch.path ? "walk" : ch.st] || STC.idle;
    const chip = ch.st === "review" ? ["warn", "hand-stop", ch.tk] : ch.st === "decision" ? ["dng", "alert-triangle", ch.tk] : ch.st === "paused" ? ["gray", "player-pause", tr("st-paused")] : null;
    return { cls: chip ? chip[0] : "plain", html: `<span><i class="d" style="background:${col}"></i><span class="nm">${esc(ch.short)}</span>${chip ? `<span class="k">${ICON_SVG(chip[1])}${esc(chip[2])}</span>` : ""}</span>` };
  };
  const syncLabels = () => {
    const seen = new Set();
    S.chars.forEach((ch) => {
      if (ch.hidden) return;
      seen.add(ch.id);
      let el = LB[ch.id];
      const { cls, html } = labelHtml(ch), s = cls + html + (S.sel === ch.id);
      if (!el) { el = LB[ch.id] = document.createElement("button"); el.type = "button"; el.dataset.officeLabel = ch.id; el.addEventListener("click", () => opts.onSelect?.(ch.id, true)); lab.appendChild(el); }
      if (el._sig !== s) { el._sig = s; el.className = `app-of-lb ${cls}${S.sel === ch.id ? " sel" : ""}`; el.innerHTML = html; el.setAttribute("aria-label", `${ch.bot ? tr("agent", ch.name) : ch.name}, ${tr("st-" + ch.st)}${ch.tk ? ", " + ch.tk : ""}`); el._w = 0; }
    });
    S.plan.rooms.forEach((r) => {
      const id = "rm-" + r.k; seen.add(id); let el = LB[id];
      const s = roomSub(r) + S.room;
      if (!el) { el = LB[id] = document.createElement("button"); el.type = "button"; el.dataset.officeRoomLabel = r.k; el.addEventListener("click", () => opts.onRoom?.(r.k)); lab.appendChild(el); }
      if (el._sig !== s) { el._sig = s; el.className = `app-of-rl${S.room === r.k ? " on" : ""}`; el.innerHTML = `<b><i style="background:${esc(r.c)}"></i>${esc(roomName(r))}</b><small>${esc(roomSub(r))}</small>`; el.setAttribute("aria-label", tr("focus", roomName(r))); el._w = 0; }
    });
    Object.keys(LB).forEach((k) => { if (!seen.has(k)) { LB[k].remove(); delete LB[k]; } });
  };
  let hudC = null;
  const placeLabels = () => {
    const z = cam.z, cmp = z < 0.62, placed = [];
    const hitIn = (x, y, w, h, gap = 3) => placed.find((p) => x < p.x + p.w + gap && x + w + gap > p.x && y < p.y + p.h + gap && y + h + gap > p.y);
    const items = S.chars.filter((ch) => !ch.hidden && LB[ch.id]).map((ch) => {
      const el = LB[ch.id]; if (!el._w) { el._w = el.offsetWidth; el._h = el.offsetHeight || 22; }
      const po = poseOf(ch, S.seats[ch.seat]), headTop = (po.pose === "sit" ? 52 : 56) + (ch.bot ? 8 : 0);
      const [X, Y] = toScreen(...P(ch.x, ch.y));
      return { el, ch, w: el._w, h: 22, x: X - el._w / 2, y: Y - headTop * z - 26, ax: X };
    }).sort((a, b) => b.y - a.y);
    items.forEach((it) => {
      const plain = it.el.classList.contains("plain");
      // Karakter menganggur (Pantry) berdesakan: labelnya baru tampil saat diperbesar.
      const far = plain && z < 0.85 && S.sel !== it.ch.id && (it.ch.st === "idle" || it.ch.st === "paused");
      it.el.classList.toggle("cmp", cmp);
      it.el.classList.toggle("far", far);
      if ((cmp && plain) || far) return;
      const w = cmp ? Math.min(it.w, 110) : it.w, x = cmp ? it.ax - w / 2 : it.x;
      let y = it.y, p, guard = 0;
      while ((p = hitIn(x, y, w, it.h)) && guard++ < 8) y = p.y - it.h - 4;
      placed.push({ x, y, w, h: it.h });
      it.el.style.transform = `translate(${Math.round(cmp ? it.ax - 24 : x)}px,${Math.round(y)}px)`;
    });
    const now = performance.now();
    if (!hudC || now - hudC.t > 400) {
      const o = lab.getBoundingClientRect();
      hudC = { t: now, r: [...stage.querySelectorAll(".app-of-ov > *, .app-of-hint:not(.gone)")].map((e) => e.getBoundingClientRect()).filter((q) => q.width > 0).map((q) => ({ x: q.left - o.left, y: q.top - o.top, w: q.width, h: q.height })) };
    }
    const hitHud = (x, y, w, h) => hudC.r.find((p) => x < p.x + p.w + 6 && x + w + 6 > p.x && y < p.y + p.h + 6 && y + h + 6 > p.y);
    S.plan.rooms.forEach((r) => {
      const el = LB["rm-" + r.k]; if (!el) return;
      const small = z < 0.55;
      el.classList.toggle("cmp", small);
      if (!el._w || el._cmp !== small) { el._cmp = small; el._w = el.offsetWidth; el._h = el.offsetHeight; }
      const h = el._h, w = el._w;
      let X, Y;
      if (r.top) { [X, Y] = toScreen(...P((r.x0 + r.x1) / 2, 0, WH)); X -= w / 2; Y -= h + 10 * z + 8; }
      else { [X, Y] = toScreen(...P((r.x0 + r.x1) / 2, D, -0.6)); X -= w / 2; Y += 10; }
      let p, guard = 0, ok = true;
      while ((p = hitIn(X, Y, w, h, 6)) && guard++ < 4) { if (r.top) Y = p.y - h - 8; else Y = p.y + p.h + 8; }
      if (hitIn(X, Y, w, h, 6) || hitHud(X, Y, w, h) || X < 4 || Y < 4 || X + w > W0 - 4 || Y + h > H0 - 4) ok = false;
      el.style.visibility = ok ? "" : "hidden";
      if (ok) placed.push({ x: X, y: Y, w, h });
      el.style.transform = `translate(${Math.round(X)}px,${Math.round(Y)}px)`;
    });
  };

  /* ---- loop ---- */
  const pct = stage.querySelector("[data-office-pct]");
  const frame = (now) => {
    if (!alive) return;
    if (!cv.isConnected) { api.destroy(); return; }
    const dt = Math.min(0.1, (now - last) / 1000); last = now;
    if (anim) { const k = Math.min(1, (now - anim.t) / anim.ms), e = 1 - Math.pow(1 - k, 3); cam.z = anim.from.z + (anim.to.z - anim.from.z) * e; cam.cx = anim.from.cx + (anim.to.cx - anim.from.cx) * e; cam.cy = anim.from.cy + (anim.to.cy - anim.from.cy) * e; dirty = true; if (k >= 1) anim = null; }
    S.still = reduced();
    const walking = stepWalkers(dt);
    if (walking) { OCC = {}; S.chars.forEach((c) => { if (!c.path && c.seat) OCC[c.seat] = c; }); }
    const animate = !S.still || walking;
    if (dirty || (animate && now - lastDraw > 1000 / 30)) {
      lastDraw = now;
      const sc = cam.z * dpr, V = { sc, ox: cv.width / 2 - cam.cx * sc, oy: cv.height / 2 - cam.cy * sc };
      drawScene(cv.getContext("2d"), V, now / 1000, S);
      syncLabels(); placeLabels(); drawMini();
      if (pct) pct.textContent = Math.round(cam.z * 100) + "%";
      dirty = false;
    }
    requestAnimationFrame(frame);
  };
  const themeObs = new MutationObserver(() => { S.dark = isDark(); dirty = true; });
  themeObs.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "class"] });
  const api = {
    destroy() { alive = false; ro.disconnect(); themeObs.disconnect(); },
    setState,
    select(id) { S.sel = id || null; dirty = true; },
    char(id) { return S.chars.find((c) => c.id === id); },
    fit() { S.room = "all"; fly(fitBox(WB)); },
    zoom(f) { fly({ z: clampZ(cam.z * f), cx: cam.cx, cy: cam.cy }, 220); },
    zoomTo(z) { fly({ z: clampZ(z), cx: cam.cx, cy: cam.cy }, 220); },
    room(k) { S.room = k; dirty = true; fly(k === "all" || !S.plan.byKey[k] ? fitBox(WB) : fitBox(roomBox(k), 0.92)); },
    focusChar(id) { const ch = S.chars.find((c) => c.id === id); if (!ch || ch.hidden) return; const [x, y] = P(ch.x, ch.y, 1); const z = Math.max(cam.z, 1.05); const [sx, sy] = toScreen(x, y); if (sx > 80 && sx < W0 - 80 && sy > 90 && sy < H0 - 60 && cam.z >= 0.8) return; fly({ z: clampZ(z), cx: x, cy: y }); },
    names(on) { stage.classList.toggle("nonames", !on); },
  };
  resize();
  requestAnimationFrame(frame);
  return api;
}
