/* V1 · Isometrik Studio: A1 yang dipoles.
   - Diorama di atas alas (slab) dengan dinding luar hanya di belakang/kiri, partisi rendah antar-ruangan
     (tidak ada kaca yang menutupi isi ruangan).
   - Lebih sedikit meja tetapi furnitur lebih kaya: rak, papan kanban, sofa, lampu, tanaman.
   - Karakter chibi ±1,9 ubin (sebelumnya ±0,9) dengan lingkaran status di lantai.
   - Bayangan kontak lembut (satu filter blur per ruangan), cahaya jendela siang / lampu hangat malam. */
(() => {
  const { S, poly, pts, dk, lt, mixC, chibi, grad } = window.SV;
  const T = 30, WH = 2.5, RW = 11, RD = 9, GAP = 1.4;
  const P = (x, y, z = 0) => [(x - y) * T, ((x + y) * T) / 2 - z * T];
  const quad = (x0, y0, x1, y1, z = 0) => [P(x0, y0, z), P(x1, y0, z), P(x1, y1, z), P(x0, y1, z)];

  function box(g, x, y, z, w, d, h, c, o = {}) {
    const gg = S("g", { cls: o.cls }, g);
    const L = [P(x, y + d, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x, y + d, z + h)];
    const R = [P(x + w, y, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x + w, y, z + h)];
    const Tp = [P(x, y, z + h), P(x + w, y, z + h), P(x + w, y + d, z + h), P(x, y + d, z + h)];
    if (!o.noL) poly(L, { fill: o.l || dk(c, 8) }, gg);
    if (!o.noR) poly(R, { fill: o.r || dk(c, 17) }, gg);
    if (!o.noT) poly(Tp, { fill: o.t || c }, gg);
    if (o.edge !== false && h > 0.03) S("polyline", { points: pts([Tp[3], Tp[2], Tp[1]]), fill: "none", stroke: "var(--edge)", "stroke-width": 1 }, gg);
    return gg;
  }
  const shadowQ = (sh, x, y, w, d, a = 1) => poly(quad(x + 0.06, y + 0.1, x + w + 0.16, y + d + 0.2), { fill: "var(--shadow)", opacity: a }, sh);
  function pot(g, x, y, s = 1, tall = false) {
    const [X, Y] = P(x, y), rx = 0.32 * T * s, ry = rx * 0.5, h = (tall ? 0.55 : 0.4) * T * s;
    S("path", { d: `M${X - rx} ${Y - h} L${X - rx * 0.82} ${Y} A${rx * 0.82} ${ry * 0.82} 0 0 0 ${X + rx * 0.82} ${Y} L${X + rx} ${Y - h} Z`, fill: "var(--pot)" }, g);
    S("path", { d: `M${X} ${Y - h} L${X + rx} ${Y - h} L${X + rx * 0.82} ${Y} A${rx * 0.82} ${ry * 0.82} 0 0 1 ${X} ${Y + ry * 0.82}Z`, fill: "#000", opacity: 0.08 }, g);
    S("ellipse", { cx: X, cy: Y - h, rx, ry, fill: "var(--soil)" }, g);
    return [X, Y - h];
  }
  function plant(g, x, y, s = 1, tall = false) {
    const [X, Y] = pot(g, x, y, s, tall);
    if (tall) {
      S("line", { x1: X, y1: Y, x2: X, y2: Y - 38 * s, stroke: "var(--trunk)", "stroke-width": 2.2 * s }, g);
      [[-9, -44, 12], [8, -48, 12], [0, -58, 12], [-4, -36, 9], [9, -36, 8]].forEach(([dx, dy, r], i) =>
        S("circle", { cx: X + dx * s, cy: Y + dy * s, r: r * s, fill: i % 2 ? "var(--leaf2)" : "var(--leaf1)" }, g));
      S("circle", { cx: X - 3 * s, cy: Y - 54 * s, r: 5 * s, fill: "var(--leaf3)", opacity: 0.7 }, g);
    } else {
      [[-6, -6, 7], [6, -7, 7], [0, -13, 8], [-1, -5, 6]].forEach(([dx, dy, r], i) =>
        S("circle", { cx: X + dx * s, cy: Y + dy * s, r: r * s, fill: i % 2 ? "var(--leaf2)" : "var(--leaf1)" }, g));
      S("circle", { cx: X - 2 * s, cy: Y - 15 * s, r: 3.4 * s, fill: "var(--leaf3)", opacity: 0.75 }, g);
    }
  }

  function render({ world, defs, data }) {
    S("filter", { id: "v1soft", x: "-20%", y: "-20%", width: "140%", height: "140%" }, defs).appendChild(
      S("feGaussianBlur", { stdDeviation: 3.2 }));
    S("filter", { id: "v1big", x: "-20%", y: "-20%", width: "140%", height: "140%" }, defs).appendChild(
      S("feGaussianBlur", { stdDeviation: 18 }));
    const sky = grad(defs, [[0, "var(--sky1)"], [1, "var(--sky2)"]], { id: "v1sky" });
    const sun = grad(defs, [[0, "var(--sun)", 1], [1, "var(--sun)", 0]], { id: "v1sun" });
    const glow = window.SV.radial(defs, [[0, "var(--lamp)", 0.9], [1, "var(--lamp)", 0]], "v1glow");

    const cols = 3, rows = 2;
    const WT = cols * RW + (cols - 1) * GAP, HT = rows * RD + (rows - 1) * GAP, M = 0.7;
    const out = { chars: {}, rooms: {} };

    /* Alas + bayangan diorama */
    poly([P(-M + 1, HT + M + 1.4), P(WT + M + 1.4, HT + M + 1.4), P(WT + M + 1.4, -M + 1), P(-M + 1, -M)].map(([a, b]) => [a, b + 18]), { fill: "var(--drop)", filter: "url(#v1big)" }, world);
    box(world, -M, -M, -0.55, WT + 2 * M, HT + 2 * M, 0.55, "var(--slab)", { l: "var(--slab-l)", r: "var(--slab-r)", edge: false });
    poly(quad(-M, -M, WT + M, HT + M), { fill: "var(--corridor)" }, world);
    // garis lorong
    const cor = S("g", { opacity: 0.9 }, world);
    for (let c = 1; c < cols; c++) { const x = c * (RW + GAP) - GAP / 2; poly(quad(x - 0.35, 0, x + 0.35, HT), { fill: "var(--runner)" }, cor); }
    poly(quad(0, RD + GAP / 2 - 0.35, WT, RD + GAP / 2 + 0.35), { fill: "var(--runner)" }, cor);

    /* Dinding luar belakang (utara) dan kiri (barat) */
    const wall = S("g", { cls: "walls" }, world);
    box(wall, -M - 0.3, -M - 0.3, 0, 0.3, HT + 2 * M + 0.3, WH, "var(--wall)", { r: "var(--wall-in)", t: "var(--wall-cap)", noL: false, l: "var(--wall-cap)" });
    box(wall, -M - 0.3, -M - 0.3, 0, WT + 2 * M + 0.3, 0.3, WH, "var(--wall)", { l: "var(--wall-in2)", t: "var(--wall-cap)", r: "var(--wall-cap)" });
    // plint
    poly([P(-M, -M, 0), P(WT + M, -M, 0), P(WT + M, -M, 0.12), P(-M, -M, 0.12)], { fill: "var(--skirt)" }, wall);
    poly([P(-M, -M, 0), P(-M, HT + M, 0), P(-M, HT + M, 0.12), P(-M, -M, 0.12)], { fill: "var(--skirt)" }, wall);
    // jendela di dinding utara + cahaya di lantai
    const win = (x) => {
      const y = -M, a = x, b = x + 2.2;
      poly([P(a, y, 0.95), P(b, y, 0.95), P(b, y, 2.05), P(a, y, 2.05)], { fill: "var(--frame)" }, wall);
      poly([P(a + 0.1, y, 1.05), P(b - 0.1, y, 1.05), P(b - 0.1, y, 1.95), P(a + 0.1, y, 1.95)], { fill: sky }, wall);
      S("line", { x1: P(a + 1.1, y, 1.05)[0], y1: P(a + 1.1, y, 1.05)[1], x2: P(a + 1.1, y, 1.95)[0], y2: P(a + 1.1, y, 1.95)[1], stroke: "var(--frame)", "stroke-width": 2 }, wall);
      S("ellipse", { cx: P(a + 0.6, y, 1.7)[0], cy: P(a + 0.6, y, 1.7)[1], rx: 7, ry: 3, fill: "var(--cloud)", cls: "cloud" }, wall);
      poly([P(a, y + 0.05), P(b, y + 0.05), P(b + 1.2, y + 2.6), P(a + 1.2, y + 2.6)], { fill: sun, cls: "sunpatch" }, world);
    };
    const winW = (y) => {
      const x = -M, a = y, b = y + 2.2;
      poly([P(x, a, 0.95), P(x, b, 0.95), P(x, b, 2.05), P(x, a, 2.05)], { fill: "var(--frame)" }, wall);
      poly([P(x, a + 0.1, 1.05), P(x, b - 0.1, 1.05), P(x, b - 0.1, 1.95), P(x, a + 0.1, 1.95)], { fill: sky }, wall);
    };

    const ROOMS = data.rooms.map((r, i) => ({ ...r, c0: i % cols, r0: Math.floor(i / cols) }))
      .map((r) => ({ ...r, x0: r.c0 * (RW + GAP), y0: r.r0 * (RD + GAP) }))
      .map((r) => ({ ...r, x1: r.x0 + RW, y1: r.y0 + RD }));

    ROOMS.filter((r) => r.r0 === 0).forEach((r) => { win(r.x0 + 1.2); if (r.kind !== "meeting") win(r.x0 + 7.4); });
    ROOMS.filter((r) => r.c0 === 0).forEach((r) => winW(r.y0 + 4.2));

    ROOMS.sort((a, b) => a.c0 + a.r0 - (b.c0 + b.r0)).forEach((r) => drawRoom(world, r, data.chars.filter((c) => c.room === r.k), out));

    const top = P(-M - 0.3, -M - 0.3, WH)[1];
    out.bounds = { x0: P(-M, HT + M)[0] - 20, x1: P(WT + M, -M)[0] + 20, y0: top - 20, y1: P(WT + M, HT + M)[1] + 40 };
    return out;
  }

  function drawRoom(world, r, chars, out) {
    const { x0, y0, x1, y1 } = r;
    const g = S("g", { cls: `room k-${r.kind}` }, world);
    const fl = r.kind === "pantry" ? "var(--floor-pantry)" : r.kind === "meeting" ? "var(--floor-meet)" : "var(--floor)";
    poly(quad(x0, y0, x1, y1), { fill: mixC(fl, r.c, r.kind === "project" ? 6 : 3) }, g);
    // pola lantai
    const pat = S("g", { stroke: "var(--plank)", "stroke-width": 1, fill: "none" }, g);
    if (r.kind === "pantry") {
      for (let i = 1; i < RW; i++) S("polyline", { points: pts([P(x0 + i, y0), P(x0 + i, y1)]) }, pat);
      for (let j = 1; j < RD; j++) S("polyline", { points: pts([P(x0, y0 + j), P(x1, y0 + j)]) }, pat);
    } else if (r.kind === "project") {
      for (let j = 0.5; j < RD; j += 0.5) S("polyline", { points: pts([P(x0, y0 + j), P(x1, y0 + j)]) }, pat);
    }
    // AO di kaki dinding belakang
    poly(quad(x0, y0, x1, y0 + 0.9), { fill: "url(#v1ao)" }, g);
    // tepi ruangan (partisi rendah)
    const low = (pts4, side) => poly(pts4, { fill: side === "L" ? "var(--low-l)" : "var(--low-r)" }, g);
    const H = 0.4;
    const wallN = () => { if (r.r0 > 0) { box(g, x0, y0 - 0.16, 0, RW, 0.16, H, mixC("var(--low)", r.c, 8), { edge: true }); } };
    const wallW = () => { if (r.c0 > 0) { box(g, x0 - 0.16, y0, 0, 0.16, 3.4, H, mixC("var(--low)", r.c, 8)); box(g, x0 - 0.16, y0 + 5, 0, 0.16, RD - 5, H, mixC("var(--low)", r.c, 8)); } };
    void low;
    wallN(); wallW();

    const sh = S("g", { filter: "url(#v1soft)" }, g);
    const items = [];
    const add = (k, f) => items.push({ k, f });

    /* Dekor dinding belakang (hanya baris belakang = dinding luar) */
    if (r.r0 === 0 && r.kind === "project") {
      add(-99, (gg) => {
        const y = -0.7, a = x0 + 4.0, b = x0 + 6.8;
        poly([P(a, y, 0.9), P(b, y, 0.9), P(b, y, 2.0), P(a, y, 2.0)], { fill: "var(--board)" }, gg);
        [0, 1, 2].forEach((i) => {
          const ca = a + 0.15 + i * 0.9;
          poly([P(ca, y, 1.82), P(ca + 0.75, y, 1.82), P(ca + 0.75, y, 1.9), P(ca, y, 1.9)], { fill: dk(r.c, 10), opacity: 0.6 }, gg);
          for (let n = 0; n < 3 - i % 2; n++) poly([P(ca + 0.08, y, 1.6 - n * 0.22), P(ca + 0.62, y, 1.6 - n * 0.22), P(ca + 0.62, y, 1.75 - n * 0.22), P(ca + 0.08, y, 1.75 - n * 0.22)], { fill: ["var(--note1)", "var(--note2)", "var(--note3)"][(n + i) % 3] }, gg);
        });
      });
    }
    if (r.kind === "meeting" && r.r0 === 0) {
      add(-99, (gg) => {
        const y = -0.7, a = x0 + 3.3, b = x0 + 7.5;
        poly([P(a, y, 0.85), P(b, y, 0.85), P(b, y, 2.15), P(a, y, 2.15)], { fill: "var(--tv)" }, gg);
        poly([P(a + 0.12, y, 0.95), P(b - 0.12, y, 0.95), P(b - 0.12, y, 2.05), P(a + 0.12, y, 2.05)], { fill: "var(--tv-screen)", cls: "screen" }, gg);
        poly([P(a + 0.4, y, 1.75), P(a + 2.2, y, 1.75), P(a + 2.2, y, 1.88), P(a + 0.4, y, 1.88)], { fill: "#fff", opacity: 0.85 }, gg);
        [0, 1, 2].forEach((i) => poly([P(a + 0.4 + i * 1.2, y, 1.1), P(a + 1.4 + i * 1.2, y, 1.1), P(a + 1.4 + i * 1.2, y, 1.55), P(a + 0.4 + i * 1.2, y, 1.55)], { fill: ["#8ec5ff", "#ffb4c8", "#a6e3c4"][i], opacity: 0.9 }, gg));
      });
    }

    const seatOf = (i) => [x0 + 2.35 + (i % 2) * 2.0 + Math.floor(i / 2) * 4.4, y0 + 3.0];
    const ORDER = [0, 2, 1, 3];
    const byChar = (i) => chars.filter((c) => c.st !== "meeting" && c.st !== "idle")[ORDER.indexOf(i)];

    if (r.kind === "project") {
      for (let i = 0; i < 4; i++) {
        const [cx, dy] = seatOf(i), x = cx - 0.95, y = dy;
        const ch = byChar(i);
        shadowQ(sh, x, y, 1.9, 0.9, 0.9);
        // kursi
        add(cx + y - 0.85, (gg) => { box(gg, cx - 0.3, y - 0.95, 0.1, 0.6, 0.12, 0.85, "var(--chair)"); });
        add(cx + y - 0.6, (gg) => { box(gg, cx - 0.32, y - 0.85, 0.42, 0.64, 0.6, 0.08, "var(--chair)"); box(gg, cx - 0.04, y - 0.6, 0, 0.08, 0.08, 0.42, "var(--chair-leg)", { edge: false }); });
        if (ch) add(cx + y - 0.5, (gg) => person(gg, ch, P(cx, y - 0.45), { seated: true }, out));
        // meja
        add(cx + y + 0.45, (gg) => {
          box(gg, x + 0.06, y + 0.06, 0, 0.08, 0.78, 0.72, "var(--desk-leg)", { edge: false });
          box(gg, x + 1.76, y + 0.06, 0, 0.08, 0.78, 0.72, "var(--desk-leg)", { edge: false });
          box(gg, x + 0.14, y + 0.7, 0.2, 1.62, 0.04, 0.5, "var(--desk-panel)", { edge: false });
          box(gg, x, y, 0.72, 1.9, 0.9, 0.07, "var(--desk)");
          // monitor (punggung menghadap kamera) + lampu status kecil
          const on = ch && ch.st !== "offline";
          box(gg, x + 0.22, y + 0.2, 0.79, 0.1, 0.1, 0.16, "var(--mon)", { edge: false });
          box(gg, x + 0.05, y + 0.12, 0.92, 0.85, 0.07, 0.46, "var(--mon)", { edge: false });
          if (on) poly([P(x + 0.12, y + 0.12, 1.4), P(x + 0.83, y + 0.12, 1.4), P(x + 0.83, y + 0.12, 1.44), P(x + 0.12, y + 0.12, 1.44)], { fill: ch.color, opacity: 0.85, cls: "screen-glow" }, gg);
          // properti meja
          if (i % 2 === 0) { const [X, Y] = P(x + 1.55, y + 0.6, 0.79); S("rect", { x: X - 3, y: Y - 7, width: 6, height: 7, rx: 1.5, fill: "var(--mug)" }, gg); }
          else { box(gg, x + 1.2, y + 0.45, 0.79, 0.5, 0.35, 0.05, "var(--paper)", { edge: false }); }
          if (!ch && i === 3) plant(gg, x + 1.6, y + 0.3, 0.45);
        });
      }
      // area santai depan: karpet + sofa + meja kopi + papan kanban berdiri
      const rx0 = x0 + 5.6, ry0 = y0 + 5.4;
      poly(quad(rx0, ry0, rx0 + 4.6, ry0 + 3.0), { fill: mixC("var(--rug)", r.c, 18) }, g);
      poly(quad(rx0 + 0.2, ry0 + 0.2, rx0 + 4.4, ry0 + 2.8), { fill: "none", stroke: mixC("var(--rug)", r.c, 35), "stroke-width": 1.2, "stroke-dasharray": "4 3" }, g);
      shadowQ(sh, rx0 + 0.9, ry0 + 0.3, 2.8, 0.9);
      add(rx0 + 2.3 + ry0 + 0.6, (gg) => sofa(gg, rx0 + 0.9, ry0 + 0.3, 2.8, mixC("var(--sofa)", r.c, 25)));
      add(rx0 + 2.3 + ry0 + 1.9, (gg) => { box(gg, rx0 + 1.6, ry0 + 1.7, 0, 1.4, 0.7, 0.3, "var(--wood)"); });
      add(rx0 + 4.3 + ry0 + 0.4, (gg) => lamp(gg, rx0 + 4.2, ry0 + 0.35));
      // papan tulis berdiri
      add(x0 + 2.0 + y0 + 6.3, (gg) => {
        box(gg, x0 + 1.0, y0 + 6.2, 0, 0.06, 0.06, 0.4, "var(--desk-leg)", { edge: false });
        box(gg, x0 + 2.9, y0 + 6.2, 0, 0.06, 0.06, 0.4, "var(--desk-leg)", { edge: false });
        box(gg, x0 + 0.9, y0 + 6.2, 0.4, 2.1, 0.07, 1.2, "var(--board)", { l: "var(--board)" });
        [[0.2, 1.3, "var(--note1)"], [0.7, 1.3, "var(--note2)"], [1.3, 1.25, "var(--note3)"], [0.2, 0.95, "var(--note2)"], [1.3, 0.9, "var(--note1)"]].forEach(([dx, z, c]) =>
          poly([P(x0 + 0.9 + dx, y0 + 6.27, z), P(x0 + 1.3 + dx, y0 + 6.27, z), P(x0 + 1.3 + dx, y0 + 6.27, z + 0.28), P(x0 + 0.9 + dx, y0 + 6.27, z + 0.28)], { fill: c }, gg));
        poly([P(x0 + 0.95, y0 + 6.27, 1.53), P(x0 + 2.95, y0 + 6.27, 1.53), P(x0 + 2.95, y0 + 6.27, 1.58), P(x0 + 0.95, y0 + 6.27, 1.58)], { fill: r.c }, gg);
      });
      // rak buku di sisi kiri
      add(x0 + 0.5 + y0 + 4.2, (gg) => shelf(gg, x0 + 0.15, y0 + 3.6, r.c));
      add(x0 + 10.4 + y0 + 0.6, (gg) => plant(gg, x0 + 10.35, y0 + 0.6, 1, true));
      add(x0 + 0.6 + y0 + 8.4, (gg) => plant(gg, x0 + 0.6, y0 + 8.4, 0.9));
    } else if (r.kind === "meeting") {
      const tx = x0 + 3.0, ty = y0 + 3.3;
      poly(quad(x0 + 1.4, y0 + 1.8, x0 + 9.6, y0 + 7.4), { fill: mixC("var(--rug)", r.c, 22) }, g);
      shadowQ(sh, tx, ty, 5, 1.7);
      const seated = chars.filter((c) => c.bot === false), standing = chars.filter((c) => c.bot);
      [0, 1, 2].forEach((i) => add(tx + 0.9 + i * 1.6 + ty - 0.6, (gg) => box(gg, tx + 0.6 + i * 1.6, ty - 0.85, 0.1, 0.6, 0.12, 0.85, "var(--chair)")));
      seated.forEach((c, i) => add(tx + 2.5 + i * 1.6 + ty - 0.4, (gg) => person(gg, c, P(tx + 2.5 + i * 1.6, ty - 0.4), { seated: true }, out)));
      add(tx + 2.5 + ty + 0.85, (gg) => {
        box(gg, tx + 0.4, ty + 0.4, 0, 0.12, 0.9, 0.72, "var(--desk-leg)", { edge: false });
        box(gg, tx + 4.5, ty + 0.4, 0, 0.12, 0.9, 0.72, "var(--desk-leg)", { edge: false });
        box(gg, tx, ty, 0.72, 5, 1.7, 0.08, "var(--wood)");
        box(gg, tx + 1.2, ty + 0.5, 0.8, 0.8, 0.55, 0.04, "var(--paper)", { edge: false });
        const [X, Y] = P(tx + 3.6, ty + 0.9, 0.8); S("rect", { x: X - 3, y: Y - 7, width: 6, height: 7, rx: 1.5, fill: "var(--mug)" }, gg);
      });
      [0, 1, 2].forEach((i) => add(tx + 0.9 + i * 1.6 + ty + 2.2, (gg) => box(gg, tx + 0.6 + i * 1.6, ty + 1.95, 0.42, 0.6, 0.55, 0.08, "var(--chair)")));
      standing.forEach((c, i) => add(x0 + 2.2 + y0 + 1.5, (gg) => person(gg, c, P(x0 + 2.2 + i, y0 + 1.5), { pose: "talk" }, out)));
      add(x0 + 10.2 + y0 + 0.7, (gg) => plant(gg, x0 + 10.2, y0 + 0.7, 1, true));
      add(x0 + 10.2 + y0 + 8.2, (gg) => plant(gg, x0 + 10.2, y0 + 8.2, 0.9));
      add(x0 + 0.7 + y0 + 8.3, (gg) => lamp(gg, x0 + 0.7, y0 + 8.3));
    } else if (r.kind === "pantry") {
      // konter dapur di belakang
      shadowQ(sh, x0 + 0.3, y0 + 0.25, 5.8, 0.9);
      add(x0 + 3 + y0 + 0.7, (gg) => {
        box(gg, x0 + 0.3, y0 + 0.25, 0, 5.8, 0.9, 0.85, "var(--cabinet)");
        box(gg, x0 + 0.25, y0 + 0.2, 0.85, 5.9, 1.0, 0.08, "var(--counter)");
        [1, 2, 3, 4, 5].forEach((i) => S("polyline", { points: pts([P(x0 + 0.3 + i * 1.16, y0 + 1.15, 0.05), P(x0 + 0.3 + i * 1.16, y0 + 1.15, 0.78)]), stroke: "var(--plank)", "stroke-width": 1 }, gg));
        box(gg, x0 + 1.0, y0 + 0.35, 0.93, 0.55, 0.5, 0.6, "var(--machine)");
        box(gg, x0 + 1.1, y0 + 0.75, 0.93, 0.35, 0.25, 0.12, "var(--mon)", { edge: false });
        box(gg, x0 + 3.2, y0 + 0.4, 0.93, 0.9, 0.5, 0.05, "var(--metal)", { edge: false });
        plant(gg, x0 + 5.5, y0 + 0.6, 0.55);
      });
      shadowQ(sh, x0 + 6.6, y0 + 0.3, 1.2, 1.0);
      add(x0 + 7.2 + y0 + 0.8, (gg) => { box(gg, x0 + 6.6, y0 + 0.3, 0, 1.2, 1.0, 2.0, "var(--fridge)"); box(gg, x0 + 7.62, y0 + 0.5, 1.1, 0.05, 0.05, 0.5, "var(--metal)", { edge: false }); });
      add(x0 + 8.6 + y0 + 0.6, (gg) => { box(gg, x0 + 8.3, y0 + 0.35, 0, 0.6, 0.6, 1.0, "var(--cabinet)"); const [X, Y] = P(x0 + 8.6, y0 + 0.65, 1.0); S("rect", { x: X - 7, y: Y - 18, width: 14, height: 18, rx: 6, fill: "var(--water)", opacity: 0.85 }, gg); });
      // meja bundar + bangku
      const [cx, cy] = [x0 + 3.0, y0 + 4.8];
      poly(quad(cx - 1.6, cy - 1.4, cx + 1.8, cy + 1.6), { fill: mixC("var(--rug)", r.c, 14) }, g);
      add(cx + cy, (gg) => {
        const [X, Y] = P(cx, cy);
        S("ellipse", { cx: X, cy: Y, rx: 10, ry: 5, fill: "var(--shadow)" }, gg);
        S("rect", { x: X - 2, y: Y - 22, width: 4, height: 22, fill: "var(--desk-leg)" }, gg);
        S("ellipse", { cx: X, cy: Y - 22, rx: 30, ry: 15, fill: dk("var(--wood)", 15) }, gg);
        S("ellipse", { cx: X, cy: Y - 24, rx: 30, ry: 15, fill: "var(--wood)" }, gg);
        S("rect", { x: X - 8, y: Y - 31, width: 6, height: 7, rx: 1.5, fill: "var(--mug)" }, gg);
      });
      [[-1.1, 0.2], [0.3, 1.2], [1.2, -0.4]].forEach(([dx, dy]) => add(cx + dx + cy + dy, (gg) => { const [X, Y] = P(cx + dx, cy + dy); S("rect", { x: X - 1.5, y: Y - 12, width: 3, height: 12, fill: "var(--desk-leg)" }, gg); S("ellipse", { cx: X, cy: Y - 12, rx: 8, ry: 4, fill: "var(--cushion)" }, gg); }));
      // sofa santai
      const sx = x0 + 6.4, sy = y0 + 5.2;
      poly(quad(sx - 0.5, sy - 0.4, sx + 4.2, sy + 3.2), { fill: mixC("var(--rug)", "#e8a87c", 25) }, g);
      shadowQ(sh, sx, sy, 3.2, 0.95);
      const sofaC = chars.filter((c) => c.st === "idle");
      add(sx + 1.6 + sy + 0.2, (gg) => sofa(gg, sx, sy, 3.2, "var(--sofa)", true));
      if (sofaC[1]) add(sx + 2.2 + sy + 0.55, (gg) => person(gg, sofaC[1], P(sx + 2.2, sy + 0.6), { seated: true, sofa: true }, out));
      add(sx + 1.6 + sy + 0.9, (gg) => sofaFront(gg, sx, sy, 3.2, "var(--sofa)"));
      add(sx + 1.6 + sy + 2.2, (gg) => box(gg, sx + 0.8, sy + 1.9, 0, 1.4, 0.7, 0.3, "var(--wood)"));
      if (sofaC[0]) add(x0 + 2.6 + y0 + 1.9, (gg) => person(gg, sofaC[0], P(x0 + 2.6, y0 + 1.9), { pose: "mug" }, out));
      add(x0 + 10.4 + y0 + 8.4, (gg) => plant(gg, x0 + 10.4, y0 + 8.4, 1, true));
      add(x0 + 0.6 + y0 + 8.4, (gg) => plant(gg, x0 + 0.6, y0 + 8.4, 0.9));
      add(x0 + 10.4 + y0 + 4.4, (gg) => lamp(gg, x0 + 10.4, y0 + 4.4));
    }

    items.sort((a, b) => a.k - b.k).forEach((it) => it.f(g));

    // partisi rendah depan (selatan + timur) dengan bukaan pintu
    const lowc = mixC("var(--low)", r.c, 8);
    box(g, x0, y1, 0, 4.2, 0.16, 0.4, lowc);
    box(g, x0 + 5.8, y1, 0, RW - 5.8, 0.16, 0.4, lowc);
    box(g, x1, y0, 0, 0.16, 3.4, 0.4, lowc);
    box(g, x1, y0 + 5, 0, 0.16, RD - 5 + 0.16, 0.4, lowc);

    const n = P(x0, y0, 0), e = P(x1, y0, 0), w = P(x0, y1, 0), s = P(x1, y1, 0);
    out.rooms[r.k] = {
      box: { x0: w[0] - 10, x1: e[0] + 10, y0: n[1] - (r.r0 === 0 ? WH * T + 30 : 70), y1: s[1] + 50 },
      label: r.r0 === 0 ? P(x0 + 1.6, -0.7, WH + 0.1) : P(x0 + RW / 2, y1 + 0.7, -0.55),
      pos: r.r0 === 0 ? "above" : "below",
    };
  }

  function sofa(g, x, y, w, c, deep) {
    box(g, x, y, 0, w, 0.3, 0.85, c);
    box(g, x, y + 0.3, 0, w, 0.6, 0.38, lt(c, 8));
    box(g, x - 0.05, y, 0, 0.3, 0.95, 0.6, dk(c, 5));
    if (!deep) { box(g, x + w - 0.25, y, 0, 0.3, 0.95, 0.6, dk(c, 5)); sofaPillow(g, x + 0.45, y + 0.3); }
  }
  function sofaFront(g, x, y, w, c) { box(g, x + w - 0.25, y, 0, 0.3, 0.95, 0.6, dk(c, 5)); box(g, x + 0.25, y + 0.75, 0, w - 0.5, 0.15, 0.4, lt(c, 4), { edge: false }); }
  function sofaPillow(g, x, y) { box(g, x, y, 0.38, 0.5, 0.18, 0.35, "var(--cushion)", { edge: false }); }
  function lamp(g, x, y) {
    const [X, Y] = P(x, y);
    S("ellipse", { cx: X, cy: Y + 6, rx: 60, ry: 30, fill: "url(#v1glow)", cls: "lampglow" }, g);
    S("ellipse", { cx: X, cy: Y, rx: 6, ry: 3, fill: "var(--desk-leg)" }, g);
    S("rect", { x: X - 1, y: Y - 48, width: 2, height: 48, fill: "var(--desk-leg)" }, g);
    S("path", { d: `M${X - 8} ${Y - 44} L${X - 5} ${Y - 58} L${X + 5} ${Y - 58} L${X + 8} ${Y - 44}Z`, fill: "var(--shade)", cls: "lampshade" }, g);
  }
  function shelf(g, x, y, c) {
    box(g, x, y, 0, 0.45, 1.6, 1.7, "var(--wood)", { r: dk("var(--wood)", 30) });
    [0.15, 0.7, 1.25].forEach((z, k) => {
      for (let i = 0; i < 5; i++) {
        const yy = y + 0.12 + i * 0.28;
        poly([P(x + 0.45, yy, z), P(x + 0.45, yy + 0.2, z), P(x + 0.45, yy + 0.2, z + 0.38 + ((i + k) % 3) * 0.04), P(x + 0.45, yy, z + 0.38 + ((i + k) % 3) * 0.04)],
          { fill: [c, "var(--book1)", "var(--book2)", "var(--book3)", lt(c, 40)][(i + k * 2) % 5] }, g);
      }
    });
  }
  function person(g, ch, [X, Y], o, out) {
    const gg = S("g", { cls: "ch", "data-ch": ch.id, transform: `translate(${X.toFixed(1)} ${(Y + (o.seated ? 6 : 0)).toFixed(1)})` }, g);
    // lingkaran status di lantai
    if (ch.st !== "idle") S("ellipse", { cx: 0, cy: o.seated ? -4 : 0, rx: 19, ry: 9.5, cls: `halo st-${ch.st}` }, gg);
    S("ellipse", { cx: 0, cy: o.seated ? -4 : 0, rx: 22, ry: 11, cls: "selring" }, gg);
    const sc = S("g", { transform: "scale(1.18)" }, gg);
    chibi(sc, ch, { hideLegs: o.seated, pose: o.pose || (o.seated && ch.st === "working" ? "type" : o.sofa ? "sit" : "stand") });
    S("rect", { x: -18, y: -72, width: 36, height: 74, fill: "transparent", cls: "hit" }, gg);
    out.chars[ch.id] = [X, Y + (o.seated ? 6 : 0) - 74];
  }

  window.VARIANTS = window.VARIANTS || {};
  window.VARIANTS.v1 = {
    cls: "v1", name: "V1 · Isometrik Studio", render, lod: 0.5, zoom: { min: 0.3, max: 2.4, focus: 1.4 },
    setup(defs) { void defs; },
  };
  // gradien AO dipasang sekali per render lewat defs global
  const orig = render;
  window.VARIANTS.v1.render = (a) => {
    const ao = S("linearGradient", { id: "v1ao", x1: 0, y1: 0, x2: 0, y2: 1 }, a.defs);
    S("stop", { offset: 0, "stop-color": "var(--ao)", "stop-opacity": 1 }, ao);
    S("stop", { offset: 1, "stop-color": "var(--ao)", "stop-opacity": 0 }, ao);
    return orig(a);
  };
})();
