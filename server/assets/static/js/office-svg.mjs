// AI Office (R5): gambar SVG. Helper + karakter chibi (tampilan dari `look` server) + dua
// renderer dunia: Isometrik Studio (V1) dan Denah (V2), dengan cahaya hangat V3.
// Semua warna adegan lewat `var(--of-*)` (office.css), jadi tema terang/gelap berganti tanpa
// menggambar ulang. Kursi/penempatan dari office-core.mjs; satuan masukan = ubin.
// Acuan: docs/reference/ui-prototype/office-v1/js/{svg,v1-iso,v2-plan,v3-diorama}.js.
import { RW, RD, GAP, statusView } from "./office-core.mjs";

export const NS = "http://www.w3.org/2000/svg";
const STYLE = new Set(["fill", "stroke", "opacity", "stop-color", "stop-opacity", "filter"]);
export function S(tag, a = {}, parent) {
  const e = document.createElementNS(NS, tag);
  for (const k in a) {
    const v = a[k];
    if (v == null || v === false) continue;
    if (k === "cls") e.setAttribute("class", v);
    else if (k === "text") e.textContent = v;
    else if (STYLE.has(k)) e.style.setProperty(k, String(v));
    else e.setAttribute(k, v);
  }
  if (parent) parent.appendChild(e);
  return e;
}
const v = (n) => `var(--of-${n})`;
const pts = (p) => p.map((q) => `${q[0].toFixed(1)},${q[1].toFixed(1)}`).join(" ");
const poly = (p, a, parent) => S("polygon", { points: pts(p), ...a }, parent);
const mixC = (c, o, pct) => `color-mix(in oklab, ${c}, ${o} ${pct}%)`;
const dk = (c, p) => mixC(c, "black", p);
const lt = (c, p) => mixC(c, "white", p);
const rr = (x, y, w, h, r, a, parent) => S("rect", { x, y, width: w, height: h, rx: r, ...a }, parent);
const HEX = /^#[0-9a-fA-F]{6}$/;
const hex = (c, d) => (HEX.test(c || "") ? c : d);

/* Gradien/filter bersama (sekali per <svg> utama). */
export function defs(d) {
  const lin = (id, stops, o = {}) => { const g = S("linearGradient", { id, x1: 0, y1: 0, x2: o.x2 ?? 0, y2: o.y2 ?? 1 }, d); stops.forEach(([off, c, op]) => S("stop", { offset: off, "stop-color": c, "stop-opacity": op ?? 1 }, g)); };
  const rad = (id, stops) => { const g = S("radialGradient", { id }, d); stops.forEach(([off, c, op]) => S("stop", { offset: off, "stop-color": c, "stop-opacity": op ?? 1 }, g)); };
  lin("of-sky", [[0, v("sky1")], [1, v("sky2")]]);
  lin("of-sun", [[0, v("sun"), 1], [1, v("sun"), 0]]);
  lin("of-ao", [[0, v("ao"), 1], [1, v("ao"), 0]]);
  rad("of-glow", [[0, v("lamp"), 0.9], [1, v("lamp"), 0]]);
  // Cahaya hangat V3: kolam cahaya lampu (multiply terang, screen gelap; lihat office.css).
  rad("of-warm", [[0, v("lamp"), 0.85], [0.5, v("lamp"), 0.25], [1, v("lamp"), 0]]);
  const f = (id, sd) => S("filter", { id, x: "-20%", y: "-20%", width: "140%", height: "140%" }, d).appendChild(S("feGaussianBlur", { stdDeviation: sd }));
  f("of-soft", 3.2); f("of-big", 18); f("of-blur", 10);
  const pat = S("pattern", { id: "of-dots", width: 24, height: 24, patternUnits: "userSpaceOnUse" }, d);
  S("circle", { cx: 1.5, cy: 1.5, r: 1.1, fill: v("dot") }, pat);
}

/* ===================================================================== */
/* Chibi menghadap depan. (0,0) = tengah kaki, tinggi ±57.               */
/* ch: {bot, st, look:{shirt, acc, skin, hair, style}}                     */
/* ===================================================================== */
export function chibi(parent, ch, o = {}) {
  const look = ch.look || {};
  const st = statusView(ch.st).st;
  const root = S("g", { cls: `of-chibi st-${st} pose-${o.pose || "stand"}` }, parent);
  const col = hex(look.shirt, "#5b5bd6");
  S("ellipse", { cx: 0, cy: 0, rx: 12.5, ry: 4.4, fill: v("ch-shadow") }, root);
  const bob = S("g", { cls: "bob" }, root);
  if (!o.hideLegs) {
    const legs = S("g", {}, bob);
    const lc = ch.bot ? v("bot-joint") : v("pants");
    rr(-6.2, -13, 5.2, 12.5, 2.4, { fill: lc }, legs);
    rr(1, -13, 5.2, 12.5, 2.4, { fill: lc }, legs);
    S("ellipse", { cx: -3.6, cy: -0.8, rx: 3.8, ry: 2, fill: v("shoes") }, legs);
    S("ellipse", { cx: 3.6, cy: -0.8, rx: 3.8, ry: 2, fill: v("shoes") }, legs);
  }
  const human = !ch.bot, skin = hex(look.skin, "#efc3a0"), hair = hex(look.hair, "#2a1d16");
  // Rambut panjang/bob di belakang badan dan kepala.
  if (human && (look.style === "long" || look.style === "bob")) {
    const bottom = look.style === "long" ? -21 : -31;
    S("path", { d: `M-13 -40 Q-14.5 -54 0 -53.5 Q14.5 -54 13 -40 L13.5 ${bottom + 3} Q13 ${bottom} 10 ${bottom} L-10 ${bottom} Q-13 ${bottom} -13.5 ${bottom + 3} Z`, fill: hair }, bob);
  }
  rr(-12.6, -27, 5, 13, 2.5, { fill: dk(col, 14), cls: "arm arm-l" }, bob);
  rr(7.6, -27, 5, 13, 2.5, { fill: dk(col, 22), cls: "arm arm-r" }, bob);
  S("path", { d: "M-9.5 -28 Q-10.5 -12 -9 -11 L9 -11 Q10.5 -12 9.5 -28 Q0 -31.5 -9.5 -28Z", fill: col }, bob);
  S("path", { d: "M1.5 -29.8 Q6 -29.5 9.5 -28 Q10.5 -12 9 -11 L1.5 -11Z", fill: "#000", opacity: 0.1 }, bob);
  if (ch.bot) {
    S("circle", { cx: 0, cy: -20, r: 3.3, fill: v("bot-shell"), opacity: 0.95 }, bob);
    S("circle", { cx: 0, cy: -20, r: 1.5, fill: lt(col, 10) }, bob);
  } else if (look.style !== "hijab") {
    S("path", { d: "M-4 -29.5 L0 -25 L4 -29.5", fill: "none", stroke: lt(col, 55), "stroke-width": 1.4, "stroke-linecap": "round" }, bob);
  }
  if (look.acc === "scarf") {
    S("path", { d: "M-9.8 -29.5 Q0 -25 9.8 -29.5 L9.6 -26 Q0 -21.5 -9.6 -26Z", fill: dk(col, 30) }, bob);
    rr(3, -26, 3.6, 9, 1.6, { fill: dk(col, 30) }, bob);
  }
  if (o.pose === "mug") {
    rr(6.5, -21, 6, 6.5, 1.6, { fill: v("mug") }, bob);
    S("path", { d: "M12.5 -19.5 q2.6 1.4 0 3.4", fill: "none", stroke: v("mug-handle"), "stroke-width": 1.3 }, bob);
  }
  const head = S("g", { cls: "head" }, bob);
  if (ch.bot) {
    S("line", { x1: 0, y1: -50, x2: 0, y2: -56, stroke: v("bot-joint"), "stroke-width": 1.6 }, head);
    S("circle", { cx: 0, cy: -57, r: 2.6, fill: col, cls: "antenna" }, head);
    rr(-13.5, -51, 27, 22, 8, { fill: v("bot-shell") }, head);
    S("path", { d: "M-13.5 -38 Q-13.5 -29 -5 -29 L5 -29 Q13.5 -29 13.5 -38 Z", fill: v("bot-shell-d") }, head);
    rr(-10, -46.5, 20, 11.5, 5.5, { fill: v("bot-visor") }, head);
    const ey = S("g", { cls: "eyes" }, head);
    rr(-6.2, -43.5, 4, 5.2, 2, { fill: lt(col, 35) }, ey);
    rr(2.2, -43.5, 4, 5.2, 2, { fill: lt(col, 35) }, ey);
    S("circle", { cx: -14.4, cy: -40, r: 2.2, fill: col }, head);
    S("circle", { cx: 14.4, cy: -40, r: 2.2, fill: col }, head);
  } else {
    if (look.style === "hijab") {
      const hj = dk(col, 10);
      S("path", { d: "M-13.5 -38 Q-14 -53 0 -53.5 Q14 -53 13.5 -38 Q14 -28 9.5 -26 L-9.5 -26 Q-14 -28 -13.5 -38Z", fill: hj }, head);
      S("ellipse", { cx: 0, cy: -38, rx: 8.6, ry: 9.4, fill: skin }, head);
      S("path", { d: "M-8.6 -41 Q0 -49 8.6 -41 Q8 -46.5 0 -47 Q-8 -46.5 -8.6 -41Z", fill: hj }, head);
    } else {
      S("circle", { cx: 0, cy: -39, r: 11.5, fill: skin }, head);
      S("path", { d: "M-12 -38.5 Q-13 -53 0 -52.5 Q13 -53 12 -38.5 Q10 -45 3 -45.5 Q-5 -44 -8 -47 Q-9.5 -42 -12 -38.5Z", fill: hair }, head);
    }
    S("ellipse", { cx: -4, cy: -37.5, rx: 1.35, ry: 1.7, fill: v("eye") }, head);
    S("ellipse", { cx: 4, cy: -37.5, rx: 1.35, ry: 1.7, fill: v("eye") }, head);
    S("circle", { cx: -6.8, cy: -33.8, r: 2, fill: "#ff8f8f", opacity: 0.32 }, head);
    S("circle", { cx: 6.8, cy: -33.8, r: 2, fill: "#ff8f8f", opacity: 0.32 }, head);
    S("path", { d: "M-1.6 -32.6 Q0 -31.4 1.6 -32.6", fill: "none", stroke: v("eye"), "stroke-width": 1, "stroke-linecap": "round" }, head);
  }
  accessory(head, look.acc, ch.bot, col);
  return root;
}

function accessory(g, acc, bot, col) {
  const top = bot ? -51 : -52.5, w = bot ? 14.2 : 12.6, ey = bot ? -41 : -37.5, ex = bot ? 4.2 : 4;
  if (acc === "glasses") {
    [-ex, ex].forEach((x) => S("circle", { cx: x, cy: ey, r: 3.4, fill: bot ? "rgba(255,255,255,.08)" : "none", stroke: bot ? v("bot-shell") : v("eye"), "stroke-width": 0.9 }, g));
    S("line", { x1: -ex + 3.4, y1: ey, x2: ex - 3.4, y2: ey, stroke: bot ? v("bot-shell") : v("eye"), "stroke-width": 0.9 }, g);
  } else if (acc === "headset") {
    S("path", { d: `M${-w} ${top + 12} Q${-w} ${top - 3} 0 ${top - 3} Q${w} ${top - 3} ${w} ${top + 12}`, fill: "none", stroke: v("headset"), "stroke-width": 2.2, "stroke-linecap": "round" }, g);
    rr(-w - 2.4, top + 9, 4.4, 8, 2, { fill: v("headset") }, g);
    rr(w - 2, top + 9, 4.4, 8, 2, { fill: v("headset") }, g);
    S("path", { d: `M${w - 0.5} ${top + 16} Q${w - 1} ${top + 21} ${w - 7} ${top + 21.5}`, fill: "none", stroke: v("headset"), "stroke-width": 1.4, "stroke-linecap": "round" }, g);
    S("circle", { cx: w - 7.5, cy: top + 21.5, r: 1.6, fill: col }, g);
  } else if (acc === "cap") {
    S("path", { d: `M${-w + 0.5} ${top + 7} Q${-w} ${top - 4} 0 ${top - 4} Q${w} ${top - 4} ${w - 0.5} ${top + 7}Z`, fill: col }, g);
    S("path", { d: `M${-w + 2} ${top + 7} Q0 ${top + 4.5} ${w + 6} ${top + 7.5} Q${w + 4} ${top + 10} ${w - 2} ${top + 9} Q0 ${top + 8} ${-w + 2} ${top + 9.5}Z`, fill: dk(col, 22) }, g);
    S("circle", { cx: 0, cy: top - 3.6, r: 1.4, fill: dk(col, 22) }, g);
  } else if (acc === "beanie") {
    S("path", { d: `M${-w + 0.5} ${top + 7} Q${-w} ${top - 5} 0 ${top - 5} Q${w} ${top - 5} ${w - 0.5} ${top + 7}Z`, fill: col }, g);
    rr(-w, top + 4.5, w * 2, 4.4, 2, { fill: dk(col, 18) }, g);
    S("circle", { cx: 0, cy: top - 5.5, r: 3, fill: lt(col, 30) }, g);
  } else if (acc === "beret") {
    S("ellipse", { cx: -2, cy: top + 1.5, rx: w + 1.5, ry: 4.6, fill: col, transform: `rotate(-8 -2 ${top + 1.5})` }, g);
    S("circle", { cx: -1, cy: top - 3, r: 1.3, fill: dk(col, 25) }, g);
  }
}

/* ===================================================================== */
/* Isometrik Studio (V1)                                                 */
/* ===================================================================== */
const T = 30, WH = 2.5, M = 0.7;
const P = (x, y, z = 0) => [(x - y) * T, ((x + y) * T) / 2 - z * T];
const quad = (x0, y0, x1, y1, z = 0) => [P(x0, y0, z), P(x1, y0, z), P(x1, y1, z), P(x0, y1, z)];

function box(g, x, y, z, w, d, h, c, o = {}) {
  const gg = S("g", o.cls ? { cls: o.cls } : {}, g);
  const L = [P(x, y + d, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x, y + d, z + h)];
  const R = [P(x + w, y, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x + w, y, z + h)];
  const Tp = [P(x, y, z + h), P(x + w, y, z + h), P(x + w, y + d, z + h), P(x, y + d, z + h)];
  if (!o.noL) poly(L, { fill: o.l || dk(c, 8) }, gg);
  if (!o.noR) poly(R, { fill: o.r || dk(c, 17) }, gg);
  if (!o.noT) poly(Tp, { fill: o.t || c }, gg);
  if (o.edge !== false && h > 0.03) S("polyline", { points: pts([Tp[3], Tp[2], Tp[1]]), fill: "none", stroke: v("edge"), "stroke-width": 1 }, gg);
  return gg;
}
const shadowQ = (sh, x, y, w, d, a = 1) => poly(quad(x + 0.06, y + 0.1, x + w + 0.16, y + d + 0.2), { fill: v("shadow"), opacity: a }, sh);
function pot(g, x, y, s, tall) {
  const [X, Y] = P(x, y), rx = 0.32 * T * s, ry = rx * 0.5, h = (tall ? 0.55 : 0.4) * T * s;
  S("path", { d: `M${X - rx} ${Y - h} L${X - rx * 0.82} ${Y} A${rx * 0.82} ${ry * 0.82} 0 0 0 ${X + rx * 0.82} ${Y} L${X + rx} ${Y - h} Z`, fill: v("pot") }, g);
  S("path", { d: `M${X} ${Y - h} L${X + rx} ${Y - h} L${X + rx * 0.82} ${Y} A${rx * 0.82} ${ry * 0.82} 0 0 1 ${X} ${Y + ry * 0.82}Z`, fill: "#000", opacity: 0.08 }, g);
  S("ellipse", { cx: X, cy: Y - h, rx, ry, fill: v("soil") }, g);
  return [X, Y - h];
}
function plant(g, x, y, s = 1, tall = false) {
  const [X, Y] = pot(g, x, y, s, tall);
  if (tall) {
    S("line", { x1: X, y1: Y, x2: X, y2: Y - 38 * s, stroke: v("trunk"), "stroke-width": 2.2 * s }, g);
    [[-9, -44, 12], [8, -48, 12], [0, -58, 12], [-4, -36, 9], [9, -36, 8]].forEach(([dx, dy, r], i) => S("circle", { cx: X + dx * s, cy: Y + dy * s, r: r * s, fill: i % 2 ? v("leaf2") : v("leaf1") }, g));
    S("circle", { cx: X - 3 * s, cy: Y - 54 * s, r: 5 * s, fill: v("leaf3"), opacity: 0.7 }, g);
  } else {
    [[-6, -6, 7], [6, -7, 7], [0, -13, 8], [-1, -5, 6]].forEach(([dx, dy, r], i) => S("circle", { cx: X + dx * s, cy: Y + dy * s, r: r * s, fill: i % 2 ? v("leaf2") : v("leaf1") }, g));
    S("circle", { cx: X - 2 * s, cy: Y - 15 * s, r: 3.4 * s, fill: v("leaf3"), opacity: 0.75 }, g);
  }
}
function sofaBack(g, x, y, w, c) {
  box(g, x, y, 0, w, 0.3, 0.85, c);
  box(g, x, y + 0.3, 0, w, 0.6, 0.38, lt(c, 8));
  box(g, x - 0.05, y, 0, 0.3, 0.95, 0.6, dk(c, 5));
}
function sofaFront(g, x, y, w, c, pillow) {
  if (pillow) box(g, x + 0.45, y + 0.3, 0.38, 0.5, 0.18, 0.35, v("cushion"), { edge: false });
  box(g, x + w - 0.25, y, 0, 0.3, 0.95, 0.6, dk(c, 5));
  box(g, x + 0.25, y + 0.75, 0, w - 0.5, 0.15, 0.4, lt(c, 4), { edge: false });
}
function lamp(g, x, y) {
  const [X, Y] = P(x, y);
  S("ellipse", { cx: X, cy: Y + 6, rx: 60, ry: 30, fill: "url(#of-glow)", cls: "of-lampglow" }, g);
  S("ellipse", { cx: X, cy: Y, rx: 6, ry: 3, fill: v("desk-leg") }, g);
  S("rect", { x: X - 1, y: Y - 48, width: 2, height: 48, fill: v("desk-leg") }, g);
  S("path", { d: `M${X - 8} ${Y - 44} L${X - 5} ${Y - 58} L${X + 5} ${Y - 58} L${X + 8} ${Y - 44}Z`, fill: v("shade") }, g);
  S("ellipse", { cx: X, cy: Y - 44, rx: 5, ry: 1.6, fill: v("bulb"), cls: "of-bulb" }, g);
}
function shelf(g, x, y, c) {
  box(g, x, y, 0, 0.45, 1.6, 1.7, v("wood"), { r: dk(v("wood"), 30) });
  [0.15, 0.7, 1.25].forEach((z, k) => {
    for (let i = 0; i < 5; i++) {
      const yy = y + 0.12 + i * 0.28, hh = 0.38 + ((i + k) % 3) * 0.04;
      poly([P(x + 0.45, yy, z), P(x + 0.45, yy + 0.2, z), P(x + 0.45, yy + 0.2, z + hh), P(x + 0.45, yy, z + hh)], { fill: [c, v("book1"), v("book2"), v("book3"), lt(c, 40)][(i + k * 2) % 5] }, g);
    }
  });
}
/* Kolam cahaya hangat (V3) di lantai. */
const warm = (g, x, y, rx = 150, ry = 78) => { const [X, Y] = P(x, y); S("ellipse", { cx: X, cy: Y, rx, ry, fill: "url(#of-warm)", cls: "of-warm" }, g); };

/* Karakter isometrik: lingkaran status + chibi + area klik. Kembalikan jangkar label (dunia). */
function isoPerson(g, ch, seat) {
  const seated = seat.pose === "desk" || seat.pose === "chair" || seat.pose === "sofa";
  const [X, Y0] = P(seat.x, seat.y), Y = Y0 + (seated ? 6 : 0);
  const sv = statusView(ch.st);
  const gg = S("g", { cls: `of-ch st-${sv.st}`, "data-ch": ch.id, transform: `translate(${X.toFixed(1)} ${Y.toFixed(1)})` }, g);
  const cy = seated ? -4 : 0;
  if (sv.halo) S("ellipse", { cx: 0, cy, rx: 19, ry: 9.5, cls: `of-halo${ch.bot ? "" : " human"}` }, gg);
  S("ellipse", { cx: 0, cy, rx: 22, ry: 11, cls: "of-selring" }, gg);
  const sc = S("g", { transform: "scale(1.18)" }, gg);
  const pose = seat.pose === "desk" ? (sv.st === "working" ? "type" : "sit") : seat.pose === "chair" || seat.pose === "sofa" ? "sit" : seat.pose === "mug" ? "mug" : seat.pose === "talk" ? "talk" : "stand";
  chibi(sc, ch, { hideLegs: seated, pose });
  S("rect", { x: -18, y: -72, width: 36, height: 74, fill: "transparent", cls: "of-hit" }, gg);
  return [X, Y - 74];
}

export const iso = {
  lod: "iso",
  /** Lantai dasar: alas, lorong, dinding luar belakang/kiri, jendela. */
  base(g, L) {
    const WT = L.W, HT = L.H;
    poly([P(-M + 1, HT + M + 1.4), P(WT + M + 1.4, HT + M + 1.4), P(WT + M + 1.4, -M + 1), P(-M + 1, -M)].map(([a, b]) => [a, b + 18]), { fill: v("drop"), filter: "url(#of-big)" }, g);
    box(g, -M, -M, -0.55, WT + 2 * M, HT + 2 * M, 0.55, v("slab"), { l: v("slab-l"), r: v("slab-r"), edge: false });
    poly(quad(-M, -M, WT + M, HT + M), { fill: v("corridor") }, g);
    const cor = S("g", { opacity: 0.9 }, g);
    for (let c = 1; c < L.cols; c++) { const x = c * (RW + GAP) - GAP / 2; poly(quad(x - 0.35, 0, x + 0.35, HT), { fill: v("runner") }, cor); }
    if (L.rows > 1) poly(quad(0, RD + GAP / 2 - 0.35, WT, RD + GAP / 2 + 0.35), { fill: v("runner") }, cor);
    const wall = S("g", {}, g);
    box(wall, -M - 0.3, -M - 0.3, 0, 0.3, HT + 2 * M + 0.3, WH, v("wall"), { r: v("wall-in"), t: v("wall-cap"), l: v("wall-cap") });
    box(wall, -M - 0.3, -M - 0.3, 0, WT + 2 * M + 0.3, 0.3, WH, v("wall"), { l: v("wall-in2"), t: v("wall-cap"), r: v("wall-cap") });
    poly([P(-M, -M, 0), P(WT + M, -M, 0), P(WT + M, -M, 0.12), P(-M, -M, 0.12)], { fill: v("skirt") }, wall);
    poly([P(-M, -M, 0), P(-M, HT + M, 0), P(-M, HT + M, 0.12), P(-M, -M, 0.12)], { fill: v("skirt") }, wall);
    const win = (x) => {
      const y = -M, a = x, b = x + 2.2;
      poly([P(a, y, 0.95), P(b, y, 0.95), P(b, y, 2.05), P(a, y, 2.05)], { fill: v("frame") }, wall);
      poly([P(a + 0.1, y, 1.05), P(b - 0.1, y, 1.05), P(b - 0.1, y, 1.95), P(a + 0.1, y, 1.95)], { fill: "url(#of-sky)" }, wall);
      const m0 = P(a + 1.1, y, 1.05), m1 = P(a + 1.1, y, 1.95);
      S("line", { x1: m0[0], y1: m0[1], x2: m1[0], y2: m1[1], stroke: v("frame"), "stroke-width": 2 }, wall);
      const cl = P(a + 0.6, y, 1.7);
      S("ellipse", { cx: cl[0], cy: cl[1], rx: 7, ry: 3, fill: v("cloud"), cls: "of-cloud" }, wall);
      poly([P(a, y + 0.05), P(b, y + 0.05), P(b + 1.2, y + 2.6), P(a + 1.2, y + 2.6)], { fill: "url(#of-sun)", cls: "of-sunpatch" }, g);
    };
    const winW = (y) => {
      const x = -M, a = y, b = y + 2.2;
      poly([P(x, a, 0.95), P(x, b, 0.95), P(x, b, 2.05), P(x, a, 2.05)], { fill: v("frame") }, wall);
      poly([P(x, a + 0.1, 1.05), P(x, b - 0.1, 1.05), P(x, b - 0.1, 1.95), P(x, a + 0.1, 1.95)], { fill: "url(#of-sky)" }, wall);
    };
    L.rooms.filter((r) => r.r0 === 0).forEach((r) => { win(r.x0 + 1.2); if (r.kind !== "meeting") win(r.x0 + 7.4); });
    L.rooms.filter((r) => r.c0 === 0).forEach((r) => winW(r.y0 + 4.2));
  },
  bounds(L) {
    const top = P(-M - 0.3, -M - 0.3, WH)[1];
    return { x0: P(-M, L.H + M)[0] - 20, x1: P(L.W + M, -M)[0] + 20, y0: top - 20, y1: P(L.W + M, L.H + M)[1] + 40 };
  },
  /** Satu ruangan + penghuninya (`occ` = [{c, seat}]). */
  room(r, occ) {
    const { x0, y0, x1, y1 } = r;
    const g = S("g", { cls: `of-room k-${r.kind}`, "data-room": r.k });
    const anchors = {};
    const fl = r.kind === "pantry" ? v("floor-pantry") : r.kind === "meeting" ? v("floor-meet") : r.kind === "lounge" ? v("floor-lounge") : v("floor");
    poly(quad(x0, y0, x1, y1), { fill: mixC(fl, r.c, r.kind === "project" ? 6 : 3) }, g);
    const pat = S("g", { stroke: v("plank"), "stroke-width": 1, fill: "none" }, g);
    if (r.kind === "pantry") {
      for (let i = 1; i < RW; i++) S("polyline", { points: pts([P(x0 + i, y0), P(x0 + i, y1)]) }, pat);
      for (let j = 1; j < RD; j++) S("polyline", { points: pts([P(x0, y0 + j), P(x1, y0 + j)]) }, pat);
    } else if (r.kind !== "meeting") {
      for (let j = 0.5; j < RD; j += 0.5) S("polyline", { points: pts([P(x0, y0 + j), P(x1, y0 + j)]) }, pat);
    }
    poly(quad(x0, y0, x1, y0 + 0.9), { fill: "url(#of-ao)" }, g);
    const lowc = mixC(v("low"), r.c, 8);
    if (r.r0 > 0) box(g, x0, y0 - 0.16, 0, RW, 0.16, 0.4, lowc);
    if (r.c0 > 0) { box(g, x0 - 0.16, y0, 0, 0.16, 3.4, 0.4, lowc); box(g, x0 - 0.16, y0 + 5, 0, 0.16, RD - 5, 0.4, lowc); }

    const sh = S("g", { filter: "url(#of-soft)" }, g);
    const items = [];
    const add = (k, f) => items.push({ k, f });
    const byLocal = {};
    occ.forEach((o) => { byLocal[o.seat.local] = o; });
    const person = (o) => add(o.seat.x + o.seat.y, (gg) => { anchors[o.c.id] = isoPerson(gg, o.c, o.seat); });
    const seatAt = (id) => byLocal[id];
    // Kursi non-meja (berdiri/sofa) digambar dengan kunci kedalaman posisi; sofa diatur khusus.
    const sofaSeats = new Set();

    if (r.r0 === 0 && r.kind === "project") {
      add(-99, (gg) => {
        const y = -0.7, a = x0 + 4.0, b = x0 + 6.8;
        poly([P(a, y, 0.9), P(b, y, 0.9), P(b, y, 2.0), P(a, y, 2.0)], { fill: v("board") }, gg);
        [0, 1, 2].forEach((i) => {
          const ca = a + 0.15 + i * 0.9;
          poly([P(ca, y, 1.82), P(ca + 0.75, y, 1.82), P(ca + 0.75, y, 1.9), P(ca, y, 1.9)], { fill: dk(r.c, 10), opacity: 0.6 }, gg);
          for (let n = 0; n < 3 - (i % 2); n++) poly([P(ca + 0.08, y, 1.6 - n * 0.22), P(ca + 0.62, y, 1.6 - n * 0.22), P(ca + 0.62, y, 1.75 - n * 0.22), P(ca + 0.08, y, 1.75 - n * 0.22)], { fill: [v("note1"), v("note2"), v("note3")][(n + i) % 3] }, gg);
        });
      });
    }
    if (r.kind === "meeting" && r.r0 === 0) {
      add(-99, (gg) => {
        const y = -0.7, a = x0 + 3.3, b = x0 + 7.5;
        poly([P(a, y, 0.85), P(b, y, 0.85), P(b, y, 2.15), P(a, y, 2.15)], { fill: v("tv") }, gg);
        poly([P(a + 0.12, y, 0.95), P(b - 0.12, y, 0.95), P(b - 0.12, y, 2.05), P(a + 0.12, y, 2.05)], { fill: v("tv-screen"), cls: "of-screen" }, gg);
        poly([P(a + 0.4, y, 1.75), P(a + 2.2, y, 1.75), P(a + 2.2, y, 1.88), P(a + 0.4, y, 1.88)], { fill: "#fff", opacity: 0.85 }, gg);
        [0, 1, 2].forEach((i) => poly([P(a + 0.4 + i * 1.2, y, 1.1), P(a + 1.4 + i * 1.2, y, 1.1), P(a + 1.4 + i * 1.2, y, 1.55), P(a + 0.4 + i * 1.2, y, 1.55)], { fill: ["#8ec5ff", "#ffb4c8", "#a6e3c4"][i], opacity: 0.9 }, gg));
      });
    }

    /* Sofa dengan penghuni di antara sandaran dan lengan depan. */
    const sofaWith = (sx, sy, w, c, seatIds, pillow) => {
      shadowQ(sh, sx, sy, w, 0.95);
      const k = sx + w / 2 + sy + 0.2;
      add(k, (gg) => sofaBack(gg, sx, sy, w, c));
      seatIds.forEach((id, i) => { sofaSeats.add(id); const o = seatAt(id); if (o) add(k + 0.1 + i * 0.01, (gg) => { anchors[o.c.id] = isoPerson(gg, o.c, o.seat); }); });
      add(k + 0.3, (gg) => sofaFront(gg, sx, sy, w, c, pillow && !seatIds.some((id) => seatAt(id))));
    };

    if (r.kind === "project") {
      warm(g, x0 + 5.5, y0 + 3.4, 170, 80);
      [2.35, 4.35, 6.75, 8.75].forEach((dx, i) => {
        const cx = x0 + dx, y = y0 + 3.0, x = cx - 0.95;
        const o = seatAt(`d${i}`);
        shadowQ(sh, x, y, 1.9, 0.9, 0.9);
        add(cx + y - 0.85, (gg) => box(gg, cx - 0.3, y - 0.95, 0.1, 0.6, 0.12, 0.85, v("chair")));
        add(cx + y - 0.6, (gg) => { box(gg, cx - 0.32, y - 0.85, 0.42, 0.64, 0.6, 0.08, v("chair")); box(gg, cx - 0.04, y - 0.6, 0, 0.08, 0.08, 0.42, v("chair-leg"), { edge: false }); });
        if (o) add(cx + y - 0.5, (gg) => { anchors[o.c.id] = isoPerson(gg, o.c, o.seat); });
        add(cx + y + 0.45, (gg) => {
          box(gg, x + 0.06, y + 0.06, 0, 0.08, 0.78, 0.72, v("desk-leg"), { edge: false });
          box(gg, x + 1.76, y + 0.06, 0, 0.08, 0.78, 0.72, v("desk-leg"), { edge: false });
          box(gg, x + 0.14, y + 0.7, 0.2, 1.62, 0.04, 0.5, v("desk-panel"), { edge: false });
          box(gg, x, y, 0.72, 1.9, 0.9, 0.07, v("desk"));
          box(gg, x + 0.22, y + 0.2, 0.79, 0.1, 0.1, 0.16, v("mon"), { edge: false });
          box(gg, x + 0.05, y + 0.12, 0.92, 0.85, 0.07, 0.46, v("mon"), { edge: false });
          if (o && o.c.st !== "paused") poly([P(x + 0.12, y + 0.12, 1.4), P(x + 0.83, y + 0.12, 1.4), P(x + 0.83, y + 0.12, 1.44), P(x + 0.12, y + 0.12, 1.44)], { fill: hex(o.c.look?.shirt, "#5b5bd6"), opacity: 0.85, cls: "of-screen-glow" }, gg);
          if (i % 2 === 0) { const [X, Y] = P(x + 1.55, y + 0.6, 0.79); S("rect", { x: X - 3, y: Y - 7, width: 6, height: 7, rx: 1.5, fill: v("mug") }, gg); }
          else box(gg, x + 1.2, y + 0.45, 0.79, 0.5, 0.35, 0.05, v("paper"), { edge: false });
          if (!o && i === 3) plant(gg, x + 1.6, y + 0.3, 0.45);
        });
      });
      const rx0 = x0 + 5.6, ry0 = y0 + 5.4;
      poly(quad(rx0, ry0, rx0 + 4.6, ry0 + 3.0), { fill: mixC(v("rug"), r.c, 18) }, g);
      poly(quad(rx0 + 0.2, ry0 + 0.2, rx0 + 4.4, ry0 + 2.8), { fill: "none", stroke: mixC(v("rug"), r.c, 35), "stroke-width": 1.2, "stroke-dasharray": "4 3" }, g);
      warm(g, rx0 + 2.6, ry0 + 1.4, 110, 56);
      sofaWith(rx0 + 0.9, ry0 + 0.3, 2.8, mixC(v("sofa"), r.c, 25), ["s1", "s0"], true);
      add(rx0 + 2.3 + ry0 + 1.9, (gg) => box(gg, rx0 + 1.6, ry0 + 1.7, 0, 1.4, 0.7, 0.3, v("wood")));
      add(rx0 + 4.3 + ry0 + 0.4, (gg) => lamp(gg, rx0 + 4.2, ry0 + 0.35));
      add(x0 + 2.0 + y0 + 6.3, (gg) => {
        box(gg, x0 + 1.0, y0 + 6.2, 0, 0.06, 0.06, 0.4, v("desk-leg"), { edge: false });
        box(gg, x0 + 2.9, y0 + 6.2, 0, 0.06, 0.06, 0.4, v("desk-leg"), { edge: false });
        box(gg, x0 + 0.9, y0 + 6.2, 0.4, 2.1, 0.07, 1.2, v("board"), { l: v("board") });
        [[0.2, 1.3, "note1"], [0.7, 1.3, "note2"], [1.3, 1.25, "note3"], [0.2, 0.95, "note2"], [1.3, 0.9, "note1"]].forEach(([dx, z, c]) =>
          poly([P(x0 + 0.9 + dx, y0 + 6.27, z), P(x0 + 1.3 + dx, y0 + 6.27, z), P(x0 + 1.3 + dx, y0 + 6.27, z + 0.28), P(x0 + 0.9 + dx, y0 + 6.27, z + 0.28)], { fill: v(c) }, gg));
        poly([P(x0 + 0.95, y0 + 6.27, 1.53), P(x0 + 2.95, y0 + 6.27, 1.53), P(x0 + 2.95, y0 + 6.27, 1.58), P(x0 + 0.95, y0 + 6.27, 1.58)], { fill: r.c }, gg);
      });
      add(x0 + 0.5 + y0 + 4.2, (gg) => shelf(gg, x0 + 0.15, y0 + 3.6, r.c));
      add(x0 + 10.4 + y0 + 0.6, (gg) => plant(gg, x0 + 10.35, y0 + 0.6, 1, true));
      add(x0 + 0.6 + y0 + 8.4, (gg) => plant(gg, x0 + 0.6, y0 + 8.4, 0.9));
    } else if (r.kind === "meeting") {
      const tx = x0 + 3.0, ty = y0 + 3.3;
      poly(quad(x0 + 1.4, y0 + 1.8, x0 + 9.6, y0 + 7.4), { fill: mixC(v("rug"), r.c, 22) }, g);
      warm(g, tx + 2.5, ty + 0.9, 170, 85);
      shadowQ(sh, tx, ty, 5, 1.7);
      [0, 1, 2].forEach((i) => add(tx + 0.9 + i * 1.6 + ty - 0.6, (gg) => box(gg, tx + 0.6 + i * 1.6, ty - 0.85, 0.1, 0.6, 0.12, 0.85, v("chair"))));
      add(tx + 2.5 + ty + 0.85, (gg) => {
        box(gg, tx + 0.4, ty + 0.4, 0, 0.12, 0.9, 0.72, v("desk-leg"), { edge: false });
        box(gg, tx + 4.5, ty + 0.4, 0, 0.12, 0.9, 0.72, v("desk-leg"), { edge: false });
        box(gg, tx, ty, 0.72, 5, 1.7, 0.08, v("wood"));
        box(gg, tx + 1.2, ty + 0.5, 0.8, 0.8, 0.55, 0.04, v("paper"), { edge: false });
        const [X, Y] = P(tx + 3.6, ty + 0.9, 0.8); S("rect", { x: X - 3, y: Y - 7, width: 6, height: 7, rx: 1.5, fill: v("mug") }, gg);
      });
      [0, 1, 2].forEach((i) => add(tx + 0.9 + i * 1.6 + ty + 2.2, (gg) => box(gg, tx + 0.6 + i * 1.6, ty + 1.95, 0.42, 0.6, 0.55, 0.08, v("chair"))));
      add(x0 + 10.2 + y0 + 0.7, (gg) => plant(gg, x0 + 10.2, y0 + 0.7, 1, true));
      add(x0 + 10.2 + y0 + 8.2, (gg) => plant(gg, x0 + 10.2, y0 + 8.2, 0.9));
      add(x0 + 0.7 + y0 + 8.3, (gg) => lamp(gg, x0 + 0.7, y0 + 8.3));
    } else if (r.kind === "pantry") {
      shadowQ(sh, x0 + 0.3, y0 + 0.25, 5.8, 0.9);
      add(x0 + 3 + y0 + 0.7, (gg) => {
        box(gg, x0 + 0.3, y0 + 0.25, 0, 5.8, 0.9, 0.85, v("cabinet"));
        box(gg, x0 + 0.25, y0 + 0.2, 0.85, 5.9, 1.0, 0.08, v("counter"));
        [1, 2, 3, 4, 5].forEach((i) => S("polyline", { points: pts([P(x0 + 0.3 + i * 1.16, y0 + 1.15, 0.05), P(x0 + 0.3 + i * 1.16, y0 + 1.15, 0.78)]), stroke: v("plank"), "stroke-width": 1 }, gg));
        box(gg, x0 + 1.0, y0 + 0.35, 0.93, 0.55, 0.5, 0.6, v("machine"));
        box(gg, x0 + 1.1, y0 + 0.75, 0.93, 0.35, 0.25, 0.12, v("mon"), { edge: false });
        box(gg, x0 + 3.2, y0 + 0.4, 0.93, 0.9, 0.5, 0.05, v("metal"), { edge: false });
        plant(gg, x0 + 5.5, y0 + 0.6, 0.55);
      });
      shadowQ(sh, x0 + 6.6, y0 + 0.3, 1.2, 1.0);
      add(x0 + 7.2 + y0 + 0.8, (gg) => { box(gg, x0 + 6.6, y0 + 0.3, 0, 1.2, 1.0, 2.0, v("fridge")); box(gg, x0 + 7.62, y0 + 0.5, 1.1, 0.05, 0.05, 0.5, v("metal"), { edge: false }); });
      add(x0 + 8.6 + y0 + 0.6, (gg) => { box(gg, x0 + 8.3, y0 + 0.35, 0, 0.6, 0.6, 1.0, v("cabinet")); const [X, Y] = P(x0 + 8.6, y0 + 0.65, 1.0); S("rect", { x: X - 7, y: Y - 18, width: 14, height: 18, rx: 6, fill: v("water"), opacity: 0.85 }, gg); });
      const cx = x0 + 3.0, cy = y0 + 4.8;
      poly(quad(cx - 1.6, cy - 1.4, cx + 1.8, cy + 1.6), { fill: mixC(v("rug"), r.c, 14) }, g);
      warm(g, cx, cy, 130, 66);
      add(cx + cy, (gg) => {
        const [X, Y] = P(cx, cy);
        S("ellipse", { cx: X, cy: Y, rx: 10, ry: 5, fill: v("shadow") }, gg);
        S("rect", { x: X - 2, y: Y - 22, width: 4, height: 22, fill: v("desk-leg") }, gg);
        S("ellipse", { cx: X, cy: Y - 22, rx: 30, ry: 15, fill: dk(v("wood"), 15) }, gg);
        S("ellipse", { cx: X, cy: Y - 24, rx: 30, ry: 15, fill: v("wood") }, gg);
        S("rect", { x: X - 8, y: Y - 31, width: 6, height: 7, rx: 1.5, fill: v("mug") }, gg);
      });
      [[-1.1, 0.2], [0.3, 1.2], [1.2, -0.4]].forEach(([dx, dy]) => add(cx + dx + cy + dy, (gg) => { const [X, Y] = P(cx + dx, cy + dy); S("rect", { x: X - 1.5, y: Y - 12, width: 3, height: 12, fill: v("desk-leg") }, gg); S("ellipse", { cx: X, cy: Y - 12, rx: 8, ry: 4, fill: v("cushion") }, gg); }));
      const sx = x0 + 6.4, sy = y0 + 5.2;
      poly(quad(sx - 0.5, sy - 0.4, sx + 4.2, sy + 3.2), { fill: mixC(v("rug"), "#e8a87c", 25) }, g);
      warm(g, sx + 1.8, sy + 1.2, 120, 60);
      sofaWith(sx, sy, 3.2, v("sofa"), ["s1", "s0"], false);
      add(sx + 1.6 + sy + 2.2, (gg) => box(gg, sx + 0.8, sy + 1.9, 0, 1.4, 0.7, 0.3, v("wood")));
      add(x0 + 10.4 + y0 + 8.4, (gg) => plant(gg, x0 + 10.4, y0 + 8.4, 1, true));
      add(x0 + 0.6 + y0 + 8.4, (gg) => plant(gg, x0 + 0.6, y0 + 8.4, 0.9));
      add(x0 + 10.4 + y0 + 4.4, (gg) => lamp(gg, x0 + 10.4, y0 + 4.4));
    } else {
      // Lounge: dua sofa, meja kopi, rak, tanaman.
      poly(quad(x0 + 1.0, y0 + 1.5, x0 + 5.2, y0 + 4.6), { fill: mixC(v("rug"), r.c, 16) }, g);
      poly(quad(x0 + 5.9, y0 + 4.8, x0 + 10.4, y0 + 8.4), { fill: mixC(v("rug"), "#e8a87c", 22) }, g);
      warm(g, x0 + 3.2, y0 + 3.0, 120, 60); warm(g, x0 + 8.0, y0 + 6.4, 120, 60);
      sofaWith(x0 + 1.5, y0 + 2.0, 3.0, mixC(v("sofa"), r.c, 20), ["s0", "s1"], true);
      add(x0 + 3 + y0 + 3.9, (gg) => box(gg, x0 + 2.3, y0 + 3.6, 0, 1.4, 0.7, 0.3, v("wood")));
      sofaWith(x0 + 6.4, y0 + 5.2, 3.2, v("sofa"), ["s2", "s3"], true);
      add(x0 + 8 + y0 + 7.4, (gg) => box(gg, x0 + 7.2, y0 + 7.1, 0, 1.4, 0.7, 0.3, v("wood")));
      add(x0 + 0.5 + y0 + 6.2, (gg) => shelf(gg, x0 + 0.15, y0 + 5.6, r.c));
      add(x0 + 10.3 + y0 + 0.7, (gg) => plant(gg, x0 + 10.3, y0 + 0.7, 1, true));
      add(x0 + 10.2 + y0 + 3.6, (gg) => lamp(gg, x0 + 10.2, y0 + 3.6));
      add(x0 + 0.7 + y0 + 8.4, (gg) => plant(gg, x0 + 0.7, y0 + 8.4, 0.9));
    }
    // Penghuni yang belum digambar (berdiri, kursi rapat).
    occ.forEach((o) => { if (o.seat.pose !== "desk" && !sofaSeats.has(o.seat.local)) person(o); });

    items.sort((a, b) => a.k - b.k).forEach((it) => it.f(g));
    box(g, x0, y1, 0, 4.2, 0.16, 0.4, lowc);
    box(g, x0 + 5.8, y1, 0, RW - 5.8, 0.16, 0.4, lowc);
    box(g, x1, y0, 0, 0.16, 3.4, 0.4, lowc);
    box(g, x1, y0 + 5, 0, 0.16, RD - 5 + 0.16, 0.4, lowc);

    const n = P(x0, y0), e = P(x1, y0), w = P(x0, y1), s = P(x1, y1);
    return {
      g,
      chars: anchors,
      box: { x0: w[0] - 10, x1: e[0] + 10, y0: n[1] - (r.r0 === 0 ? WH * T + 30 : 70), y1: s[1] + 50 },
      label: r.r0 === 0 ? P(r.x0 + 1.6, -0.7, WH + 0.1) : P(r.x0 + RW / 2, r.y1 + 0.7, -0.55),
      pos: r.r0 === 0 ? "above" : "below",
      // Cadangan di dalam ruangan sendiri (sisi depan) bila posisi utama tertutup/di luar layar.
      alt: { at: P(r.x0 + RW / 2, r.y1 - 0.6), mode: "above" },
    };
  },
  /** Titik dunia sebuah kursi (mini-map). */
  at: (seat) => P(seat.x, seat.y),
};

/* ===================================================================== */
/* Denah (V2)                                                            */
/* ===================================================================== */
const K = 38, PAD = 18;
const X = (x) => PAD + x * K;
const rect = (g, x, y, w, h, r, fill, stroke) => S("rect", { x: X(x), y: X(y), width: w * K, height: h * K, rx: r, fill, stroke, "stroke-width": stroke ? 1 : null }, g);

function planToken(g, ch, seat, iconSvg) {
  const sv = statusView(ch.st);
  const x = X(seat.x), y = X(seat.y), look = ch.look || {};
  const gg = S("g", { cls: `of-ch of-tok st-${sv.st}`, "data-ch": ch.id, transform: `translate(${x.toFixed(1)} ${y.toFixed(1)})` }, g);
  S("circle", { r: 27, cls: "of-pulse" }, gg);
  S("circle", { r: 21, cls: "of-ring" }, gg);
  S("circle", { r: 25, cls: "of-selring" }, gg);
  S("circle", { r: 17, fill: v("tok-shadow"), cy: 2 }, gg);
  const col = hex(look.shirt, "#5b5bd6");
  if (ch.bot) {
    S("circle", { r: 17.5, fill: col, cls: "of-face" }, gg);
    const markup = iconSvg?.(ch.icon) || iconSvg?.("robot") || "";
    if (markup) {
      const wrap = S("g", { cls: "of-tok-ic" }, gg);
      wrap.innerHTML = markup;
      const svg = wrap.querySelector("svg");
      if (svg) { svg.setAttribute("x", -10); svg.setAttribute("y", -10); svg.setAttribute("width", 20); svg.setAttribute("height", 20); }
    }
  } else {
    S("circle", { r: 16.5, fill: hex(look.skin, "#e8b48f"), cls: "of-face" }, gg);
    if (look.style === "hijab") S("path", { d: "M-16.5 0 A16.5 16.5 0 0 1 16.5 0 L16.5 6 Q0 -6 -16.5 6Z", fill: dk(col, 10) }, gg);
    else S("path", { d: "M-16 -4 A16.5 16.5 0 0 1 16 -4 Q8 -9 0 -6 Q-8 -10 -16 -4Z", fill: hex(look.hair, "#2a1d16") }, gg);
    S("text", { y: 7.5, "text-anchor": "middle", cls: "of-ini", text: (ch.short || ch.name || "?").trim().charAt(0).toUpperCase() }, gg);
    S("circle", { cx: 12, cy: 12, r: 4.5, cls: "of-online" }, gg);
  }
  S("circle", { r: 28, fill: "transparent", cls: "of-hit" }, gg);
  return [x, y - 28];
}

export const plan = {
  lod: "plan",
  base(g, L) {
    const BW = L.W * K + PAD * 2, BH = L.H * K + PAD * 2;
    S("rect", { x: -3000, y: -3000, width: BW + 6000, height: BH + 6000, fill: "url(#of-dots)" }, g);
    S("rect", { x: 6, y: 10, width: BW, height: BH, rx: 14, fill: v("drop"), filter: "url(#of-blur)" }, g);
    S("rect", { x: 0, y: 0, width: BW, height: BH, rx: 12, fill: v("corridor") }, g);
    if (L.rows > 1) S("line", { x1: PAD + 20, y1: X(RD + GAP / 2), x2: BW - PAD - 20, y2: X(RD + GAP / 2), stroke: v("guide"), "stroke-width": 1.2, "stroke-dasharray": "2 6", "stroke-linecap": "round" }, g);
  },
  /** Dinding luar + jendela digambar di atas ruangan. */
  top(g, L) {
    const BW = L.W * K + PAD * 2, BH = L.H * K + PAD * 2;
    S("rect", { x: 0, y: 0, width: BW, height: BH, rx: 12, fill: "none", stroke: v("wall"), "stroke-width": 7 }, g);
    const win = (x, y, len, horiz) => {
      S("rect", { ...(horiz ? { x, y, width: len, height: 7 } : { x, y, width: 7, height: len }), fill: v("glass") }, g);
      S("line", horiz ? { x1: x, y1: y + 3.5, x2: x + len, y2: y + 3.5, stroke: v("glass-line"), "stroke-width": 1.2 } : { x1: x + 3.5, y1: y, x2: x + 3.5, y2: y + len, stroke: v("glass-line"), "stroke-width": 1.2 }, g);
    };
    L.rooms.forEach((r) => {
      const rx = X(r.x0);
      if (r.r0 === 0) [1.4, 6.8].forEach((dx) => win(rx + dx * K, -3.5, 2.8 * K, true));
      else [1.4, 6.8].forEach((dx) => win(rx + dx * K, BH - 3.5, 2.8 * K, true));
      if (r.c0 === 0) win(-3.5, X(r.y0) + 3 * K, 2.8 * K, false);
      if (r.c0 === L.cols - 1) win(BW - 3.5, X(r.y0) + 3 * K, 2.8 * K, false);
    });
  },
  bounds(L) { return { x0: -30, y0: -70, x1: L.W * K + PAD * 2 + 30, y1: L.H * K + PAD * 2 + 70 }; },
  room(r, occ, L, iconSvg) {
    const { x0, y0 } = r, x1 = x0 + RW, y1 = y0 + RD;
    const g = S("g", { cls: `of-room k-${r.kind}`, "data-room": r.k });
    const anchors = {};
    const floor = r.kind === "pantry" ? v("floor-pantry") : r.kind === "meeting" ? v("floor-meet") : r.kind === "lounge" ? v("floor-lounge") : v("floor");
    rect(g, x0, y0, RW, RD, 0, mixC(floor, r.c, r.kind === "project" ? 5 : 3));
    if (r.kind === "pantry") {
      const tp = S("g", { stroke: v("tile"), "stroke-width": 1 }, g);
      for (let i = 1; i < RW; i++) S("line", { x1: X(x0 + i), y1: X(y0), x2: X(x0 + i), y2: X(y1) }, tp);
      for (let j = 1; j < RD; j++) S("line", { x1: X(x0), y1: X(y0 + j), x2: X(x1), y2: X(y0 + j) }, tp);
    }
    if (r.kind === "project") S("rect", { x: X(x0), y: X(y0), width: 4, height: RD * K, fill: r.c, opacity: 0.55 }, g);
    const fx = S("g", { cls: "of-furn" }, g);
    const glow = (x, y, rad) => S("circle", { cx: X(x0 + x), cy: X(y0 + y), r: rad * K, fill: "url(#of-warm)", cls: "of-warm" }, g);
    const R = (x, y, w, h, rad, fill, line = true) => rect(fx, x0 + x, y0 + y, w, h, rad, fill, line ? v("line-f") : null);
    const plant = (x, y, s = 1) => {
      const cx = X(x0 + x), cy = X(y0 + y);
      S("circle", { cx, cy, r: 14 * s, fill: v("pot"), stroke: v("line-f") }, fx);
      for (let i = 0; i < 6; i++) { const a = (i / 6) * Math.PI * 2, ex = cx + Math.cos(a) * 7 * s, ey = cy + Math.sin(a) * 7 * s; S("ellipse", { cx: ex, cy: ey, rx: 7 * s, ry: 4 * s, transform: `rotate(${((a * 180) / Math.PI).toFixed(1)} ${ex.toFixed(1)} ${ey.toFixed(1)})`, fill: i % 2 ? v("leaf1") : v("leaf2") }, fx); }
      S("circle", { cx, cy, r: 3.5 * s, fill: v("leaf3") }, fx);
    };
    const rug = (x, y, w, h, c) => {
      R(x, y, w, h, 10, mixC(v("rug"), c, 14), false);
      S("rect", { x: X(x0 + x + 0.16), y: X(y0 + y + 0.16), width: (w - 0.32) * K, height: (h - 0.32) * K, rx: 6, fill: "none", stroke: mixC(v("rug"), c, 40), "stroke-width": 1, "stroke-dasharray": "3 3" }, fx);
    };
    const sofa = (x, y, w, h, c) => { R(x, y, w, h, 9, c); R(x + 0.2, y + 0.28, w - 0.4, h - 0.36, 6, lt(c, 18), false); };
    const lampTop = (x, y) => { glow(x, y, 1.6); const cx = X(x0 + x), cy = X(y0 + y); S("circle", { cx, cy, r: 8, fill: v("shade"), stroke: v("line-f") }, fx); S("circle", { cx, cy, r: 3, fill: v("bulb") }, fx); };
    const chair = (x, y, back) => { R(x - 0.3, y - 0.3, 0.6, 0.6, 6, v("chair")); R(x - 0.27, back === "top" ? y - 0.36 : y + 0.24, 0.54, 0.12, 2, dk(v("chair"), 20), false); };

    if (r.kind === "project") {
      glow(5.5, 3.2, 3.6);
      R(4.0, 0.12, 2.8, 0.3, 3, v("board"));
      ["note1", "note2", "note3", "note1", "note2"].forEach((c, i) => R(4.12 + i * 0.52, 0.17, 0.38, 0.2, 1.5, v(c), false));
      R(0.15, 3.6, 0.45, 1.6, 3, v("wood"));
      for (let i = 0; i < 5; i++) R(0.2, 3.7 + i * 0.29, 0.35, 0.22, 1.5, [r.c, v("book1"), v("book2"), v("book3"), v("book1")][i], false);
      [2.35, 4.35, 6.75, 8.75].forEach((dx, i) => {
        const o = occ.find((q) => q.seat.local === `d${i}`);
        chair(dx, 2.55, "top");
        R(dx - 0.95, 3.0, 1.9, 0.9, 5, v("desk"));
        R(dx - 0.45, 3.68, 0.9, 0.11, 2, o && o.c.st !== "paused" ? hex(o.c.look?.shirt, "#5b5bd6") : v("mon"), false);
        R(dx - 0.3, 3.18, 0.6, 0.16, 1.5, v("kb"), false);
        if (!o && i === 3) plant(dx + 0.55, 3.45, 0.55);
      });
      rug(5.6, 5.4, 4.6, 3.0, r.c);
      glow(7.9, 6.6, 2.2);
      sofa(6.5, 5.65, 2.8, 0.95, mixC(v("sofa"), r.c, 25));
      R(7.2, 7.1, 1.4, 0.7, 8, v("wood"));
      R(7.4, 7.25, 0.35, 0.25, 2, v("paper"), false);
      lampTop(9.8, 5.75);
      R(0.9, 6.2, 2.1, 0.22, 3, v("board"));
      ["note1", "note2", "note3"].forEach((c, i) => R(1.05 + i * 0.6, 6.25, 0.4, 0.12, 1, v(c), false));
      plant(10.35, 0.6); plant(0.6, 8.4, 0.9);
    } else if (r.kind === "meeting") {
      glow(5.5, 4.2, 3.6);
      rug(1.4, 1.8, 8.2, 5.6, r.c);
      R(3.0, 3.3, 5.0, 1.7, 26, v("wood"));
      S("circle", { cx: X(x0 + 5.5), cy: X(y0 + 4.15), r: 7, fill: v("paper"), stroke: v("line-f") }, fx);
      [0, 1, 2].forEach((i) => { chair(3.9 + i * 1.6, 2.9, "top"); chair(3.9 + i * 1.6, 5.6, "bottom"); });
      R(3.3, 0.12, 4.2, 0.26, 3, v("tv"), false);
      plant(10.2, 0.7); plant(10.2, 8.2, 0.9); lampTop(0.7, 8.3);
    } else if (r.kind === "pantry") {
      R(0.3, 0.25, 5.8, 0.9, 5, v("counter"));
      R(0.3, 0.25, 0.9, 3.0, 5, v("counter"));
      S("ellipse", { cx: X(x0 + 3.65), cy: X(y0 + 0.7), rx: 18, ry: 11, fill: v("sink"), stroke: v("line-f") }, fx);
      R(1.0, 0.35, 0.6, 0.6, 5, v("machine"), false);
      S("circle", { cx: X(x0 + 1.3), cy: X(y0 + 0.65), r: 4, fill: v("accent-c") }, fx);
      R(6.6, 0.3, 1.2, 1.0, 5, v("fridge"));
      R(8.3, 0.35, 0.6, 0.6, 8, v("water"));
      glow(3.0, 4.8, 2.4);
      S("circle", { cx: X(x0 + 3.0), cy: X(y0 + 4.8), r: 0.85 * K, fill: v("wood"), stroke: v("line-f") }, fx);
      [[-1.1, 0.2], [0.3, 1.2], [1.2, -0.4]].forEach(([dx, dy]) => S("circle", { cx: X(x0 + 3.0 + dx), cy: X(y0 + 4.8 + dy), r: 9, fill: v("cushion"), stroke: v("line-f") }, fx));
      rug(5.9, 4.8, 4.7, 3.6, "#e8a87c");
      glow(8.0, 6.4, 2.2);
      sofa(6.4, 5.2, 3.2, 0.95, v("sofa"));
      R(7.2, 7.1, 1.4, 0.7, 8, v("wood"));
      plant(10.4, 8.4); plant(0.6, 8.4, 0.9); lampTop(10.4, 4.4);
    } else {
      glow(3.2, 3.0, 2.4); glow(8.0, 6.4, 2.4);
      rug(1.0, 1.5, 4.2, 3.1, r.c);
      rug(5.9, 4.8, 4.5, 3.6, "#e8a87c");
      sofa(1.5, 2.0, 3.0, 0.95, mixC(v("sofa"), r.c, 20));
      sofa(6.4, 5.2, 3.2, 0.95, v("sofa"));
      R(2.3, 3.6, 1.4, 0.7, 8, v("wood"));
      R(7.2, 7.1, 1.4, 0.7, 8, v("wood"));
      R(0.15, 5.6, 0.45, 1.6, 3, v("wood"));
      plant(10.3, 0.7); plant(0.7, 8.4, 0.9); lampTop(10.2, 3.6);
    }
    // Dinding ruangan + pintu ke lorong (belakang: bawah; depan: atas) dengan busur ayun.
    const wallC = { fill: "none", stroke: v("wall-in"), "stroke-width": 4, "stroke-linecap": "square" };
    S("rect", { x: X(x0), y: X(y0), width: RW * K, height: RD * K, ...wallC }, g);
    const dx = X(x0 + 4.2), dW = 1.6 * K, doorY = r.r0 === 0 ? X(y1) : X(y0), sw = r.r0 === 0 ? -1 : 1;
    S("rect", { x: dx, y: doorY - 4, width: dW, height: 8, fill: v("corridor") }, g);
    S("path", { d: `M${dx} ${doorY} L${dx} ${doorY + sw * dW} A${dW} ${dW} 0 0 ${sw > 0 ? 0 : 1} ${dx + dW} ${doorY}`, fill: "none", stroke: v("door"), "stroke-width": 1 }, g);
    S("line", { x1: dx, y1: doorY, x2: dx, y2: doorY + sw * dW, stroke: v("wall-in"), "stroke-width": 2 }, g);

    const ppl = S("g", {}, g);
    occ.slice().sort((a, b) => a.seat.y - b.seat.y).forEach((o) => { anchors[o.c.id] = planToken(ppl, o.c, o.seat, iconSvg); });

    const BH = L.H * K + PAD * 2;
    return {
      g,
      chars: anchors,
      box: { x0: X(x0) - 20, y0: r.r0 === 0 ? X(y0) - 80 : X(y0) - 30, x1: X(x1) + 20, y1: r.r0 === 0 ? X(y1) + 20 : X(y1) + 80 },
      label: r.r0 === 0 ? [X(x0) + (RW * K) / 2, -6] : [X(x0) + (RW * K) / 2, BH + 4],
      pos: r.r0 === 0 ? "above" : "below",
      // Cadangan di dalam ruangan sendiri (sisi bawah) bila posisi utama tertutup/di luar layar.
      alt: { at: [X(x0) + (RW * K) / 2, X(y1) - 6], mode: "above" },
    };
  },
  at: (seat) => [X(seat.x), X(seat.y)],
};

/** Pratinjau karakter berdiri di `<svg>` (panel detail, dialog Kustom). */
export function preview(svg, ch) {
  if (!svg || !ch) return;
  svg.replaceChildren();
  svg.setAttribute("viewBox", "-30 -68 60 74");
  S("ellipse", { cx: 0, cy: 0, rx: 20, ry: 6, fill: v("ch-shadow"), opacity: 0.5 }, svg);
  chibi(svg, { ...ch, st: ch.st === "working" ? "online" : ch.st }, { pose: "stand" });
}
