/* V3 · Diorama hangat: potongan gedung 2.5D (dollhouse) dengan perspektif satu titik per ruangan.
   Ruangan berjajar per lantai; project baru menambah lantai. Karakter menghadap depan dan lebih besar,
   ilustrasi hangat bergaris tinta, lampu gantung yang menyala di mode gelap. */
(() => {
  const { S, dk, lt, mixC, chibi, grad, radial } = window.SV;
  const W = 400, H = 260, GX = 16, SLAB = 50, MX = 26, TOPM = 62;
  const IN = 60, CD = 18, FD = 60; // inset samping, tinggi plafon, kedalaman lantai (perspektif)
  const OL = { stroke: "var(--ink)", "stroke-width": 1.3, "stroke-linejoin": "round" };
  const poly = (p, a, g) => S("polygon", { points: p.map((q) => q.join(",")).join(" "), ...a }, g);
  const R = (g, x, y, w, h, r, fill, ol = true, extra = {}) => S("rect", { x, y, width: w, height: h, rx: r, fill, ...(ol ? OL : {}), ...extra }, g);

  function render({ world, defs, data }) {
    const cols = 3, rows = Math.ceil(data.rooms.length / cols);
    const BW = MX * 2 + cols * W + (cols - 1) * GX, BH = TOPM + rows * H + (rows - 1) * SLAB + 30;
    const out = { chars: {}, rooms: {} };
    grad(defs, [[0, "var(--sky1)"], [1, "var(--sky2)"]], { id: "v3sky" });
    radial(defs, [[0, "var(--lamp)", 0.85], [0.5, "var(--lamp)", 0.25], [1, "var(--lamp)", 0]], "v3glow");
    grad(defs, [[0, "#000", 0.16], [1, "#000", 0]], { id: "v3ao", x1: 0, y1: 0, x2: 0, y2: 1 });

    // tanah + jalan + bayangan gedung
    R(world, -600, BH - 4, BW + 1200, 300, 0, "var(--ground)", false);
    R(world, -600, BH + 26, BW + 1200, 34, 0, "var(--path)", false);
    S("ellipse", { cx: BW / 2, cy: BH + 2, rx: BW / 2 + 40, ry: 12, fill: "var(--drop)" }, world);
    tree(world, -70, BH + 2, 1.1); tree(world, BW + 70, BH + 2, 1.25); tree(world, -150, BH + 4, 0.8); bush(world, BW + 150, BH + 4);
    // fasad
    R(world, 0, 18, BW, BH - 18, 6, "var(--facade)");
    // pita papan nama per lantai
    for (let rw = 0; rw < rows; rw++) { const y = TOPM + rw * (H + SLAB) - SLAB + (rw === 0 ? 0 : 0); R(world, 10, y + 10, BW - 20, SLAB - 18, 3, "var(--slab)", false); }
    R(world, -10, 6, BW + 20, 18, 4, "var(--roof)");
    // tanaman di atap + papan nama
    [60, 140, BW - 140, BW - 60].forEach((x, i) => { R(world, x - 22, -12, 44, 18, 3, "var(--planter)"); bushTop(world, x, -12, i); });
    const sign = S("g", {}, world);
    R(sign, BW / 2 - 92, -34, 184, 30, 8, "var(--sign)");
    S("text", { x: BW / 2, y: -14, "text-anchor": "middle", cls: "v3-sign", text: `${data.workspace} · AI Office` }, sign);
    S("line", { x1: BW / 2 - 60, y1: -4, x2: BW / 2 - 60, y2: 6, ...OL }, sign);
    S("line", { x1: BW / 2 + 60, y1: -4, x2: BW / 2 + 60, y2: 6, ...OL }, sign);
    // pintu masuk di lantai dasar (antara ruangan kolom 0 dan 1 tidak ada ruang: taruh di tengah bawah fasad)
    data.rooms.forEach((r, i) => {
      const c = i % cols, rw = Math.floor(i / cols);
      const x0 = MX + c * (W + GX), y0 = TOPM + rw * (H + SLAB);
      drawRoom(world, { ...r, col: c, row: rw, x0, y0 }, data.chars.filter((ch) => ch.room === r.k), out);
    });
    out.bounds = { x0: -60, y0: -60, x1: BW + 60, y1: BH + 70 };
    return out;
  }

  function drawRoom(world, r, chars, out) {
    const { x0, y0 } = r, x1 = x0 + W, y1 = y0 + H;
    const bx0 = x0 + IN, bx1 = x1 - IN, by0 = y0 + CD, by1 = y1 - FD, bw = bx1 - bx0, k = bw / W;
    const at = (u, d) => { const fx = x0 + u * W, bx = bx0 + u * bw; return [bx + (fx - bx) * d, by1 + (y1 - by1) * d, k + (1 - k) * d]; };
    const g = S("g", { cls: `room k-${r.kind}` }, world);
    const wallC = r.kind === "pantry" ? "var(--wall-pantry)" : r.kind === "meeting" ? "var(--wall-meet)" : mixC("var(--wall)", r.c, 10);
    // kotak ruangan
    poly([[x0, y0], [x1, y0], [bx1, by0], [bx0, by0]], { fill: "var(--ceil)" }, g);
    poly([[bx0, by0], [bx1, by0], [bx1, by1], [bx0, by1]], { fill: wallC }, g);
    poly([[x0, y0], [bx0, by0], [bx0, by1], [x0, y1]], { fill: dk(wallC, 9) }, g);
    poly([[x1, y0], [bx1, by0], [bx1, by1], [x1, y1]], { fill: dk(wallC, 14) }, g);
    // wallpaper garis halus
    const wp = S("g", { opacity: 0.5 }, g);
    for (let x = bx0 + 14; x < bx1; x += 14) S("line", { x1: x, y1: by0, x2: x, y2: by1, stroke: dk(wallC, 4), "stroke-width": 1 }, wp);
    // lantai
    const floorC = r.kind === "pantry" ? "var(--floor-pantry)" : r.kind === "meeting" ? "var(--floor-meet)" : "var(--floor)";
    poly([[bx0, by1], [bx1, by1], [x1, y1], [x0, y1]], { fill: floorC }, g);
    const pl = S("g", { stroke: "var(--plank)", "stroke-width": 1 }, g);
    if (r.kind === "pantry") {
      for (let i = 1; i < 10; i++) S("line", { x1: bx0 + (bw * i) / 10, y1: by1, x2: x0 + (W * i) / 10, y2: y1 }, pl);
      [0.18, 0.4, 0.68].forEach((d) => { const a = at(0, d), b = at(1, d); S("line", { x1: a[0], y1: a[1], x2: b[0], y2: b[1] }, pl); });
    } else {
      for (let i = 1; i < 14; i++) S("line", { x1: bx0 + (bw * i) / 14, y1: by1, x2: x0 + (W * i) / 14, y2: y1 }, pl);
    }
    // plint + AO
    S("line", { x1: bx0, y1: by1, x2: bx1, y2: by1, stroke: dk(wallC, 25), "stroke-width": 2 }, g);
    poly([[bx0, by1 - 26], [bx1, by1 - 26], [bx1, by1], [bx0, by1]], { fill: "url(#v3ao)", opacity: 0.6 }, g);

    const wall = S("g", {}, g);
    const items = [];
    const add = (d, f) => items.push({ d, f });
    const put = (u, d, fn) => add(d, (gg) => { const [X, Y, s] = at(u, d); const t = S("g", { transform: `translate(${X.toFixed(1)} ${Y.toFixed(1)}) scale(${s.toFixed(3)})` }, gg); fn(t, X, Y, s); });

    // lampu gantung
    const lamps = r.kind === "meeting" ? [0.5] : [0.3, 0.7];
    lamps.forEach((u) => {
      const X = x0 + u * W, Y = y0 + CD + 34;
      S("ellipse", { cx: X, cy: Y + 70, rx: 120, ry: 90, fill: "url(#v3glow)", cls: "v3-glow" }, g);
      S("line", { x1: X, y1: y0 + 6, x2: X, y2: Y - 10, stroke: "var(--ink)", "stroke-width": 1.2 }, g);
      S("path", { d: `M${X - 15} ${Y + 2} Q${X} ${Y - 22} ${X + 15} ${Y + 2}Z`, fill: "var(--lampshade)", ...OL }, g);
      S("ellipse", { cx: X, cy: Y + 3, rx: 6, ry: 2.6, fill: "var(--bulb)" }, g);
    });

    if (r.kind === "project") {
      // jendela + kanban + rak + jam
      win(wall, bx0 + 16, by0 + 22, 92, 72);
      kanban(wall, bx0 + 126, by0 + 22, 118, 66, r.c);
      shelf(wall, bx0 + 126, by0 + 104, 118, r.c);
      clock(wall, bx1 - 22, by0 + 34);
      // karpet
      const a = at(0.08, 0.28), b = at(0.92, 0.28), c2 = at(0.98, 0.78), d2 = at(0.02, 0.78);
      poly([a, b, c2, d2].map((p) => [p[0], p[1]]), { fill: mixC("var(--rug)", r.c, 22), ...OL, "stroke-width": 0.8 }, g);
      const seats = [0.2, 0.5, 0.8], ORDER = [0, 2, 1];
      const active = chars.filter((c) => c.st !== "idle" && c.st !== "meeting");
      seats.forEach((u, i) => {
        const ch = active[ORDER.indexOf(i)];
        put(u, 0.4, (t) => chairBack(t));
        if (ch) put(u, 0.43, (t, X, Y, s) => person(t, ch, out, X, Y, s, { seated: true }));
        put(u, 0.5, (t) => desk(t, ch));
      });
      put(0.04, 0.92, (t) => plant(t, 1.1, true));
      put(0.96, 0.9, (t) => plant(t, 0.9));
      if (!active.length) put(0.5, 0.75, (t) => box(t));
    } else if (r.kind === "meeting") {
      tv(wall, bx0 + bw / 2 - 74, by0 + 18, 148, 84);
      win(wall, bx0 + 12, by0 + 24, 56, 70);
      const a = at(0.1, 0.25), b = at(0.9, 0.25), c2 = at(0.95, 0.85), d2 = at(0.05, 0.85);
      poly([a, b, c2, d2].map((p) => [p[0], p[1]]), { fill: mixC("var(--rug)", r.c, 18), ...OL, "stroke-width": 0.8 }, g);
      const standing = chars.filter((c) => c.bot), seated = chars.filter((c) => !c.bot);
      standing.forEach((c) => put(0.2, 0.2, (t, X, Y, s) => person(t, c, out, X, Y, s, { pose: "talk" })));
      [0.38, 0.55, 0.72].forEach((u) => put(u, 0.45, (t) => chairBack(t)));
      seated.forEach((c, i) => put(0.55 + i * 0.17, 0.47, (t, X, Y, s) => person(t, c, out, X, Y, s, { seated: true })));
      put(0.55, 0.55, (t) => {
        R(t, -130, -56, 260, 10, 4, "var(--wood)");
        R(t, -118, -46, 8, 46, 2, dk("var(--wood)", 18));
        R(t, 110, -46, 8, 46, 2, dk("var(--wood)", 18));
        R(t, -60, -66, 30, 10, 2, "var(--paper)");
        R(t, 40, -68, 10, 12, 2, "var(--mug)");
      });
      put(0.95, 0.88, (t) => plant(t, 1.1, true));
    } else if (r.kind === "pantry") {
      win(wall, bx1 - 76, by0 + 22, 60, 66);
      // kabinet atas
      R(wall, bx0 + 6, by0 + 14, 150, 34, 3, "var(--cabinet)");
      [0, 1, 2].forEach((i) => S("line", { x1: bx0 + 6 + 50 * (i + 1), y1: by0 + 14, x2: bx0 + 6 + 50 * (i + 1), y2: by0 + 48, ...OL }, wall));
      put(0.27, 0.02, (t) => {
        R(t, -110, -62, 220, 62, 3, "var(--cabinet)");
        R(t, -114, -68, 228, 9, 3, "var(--counter)");
        [-73, -36, 1, 38, 75].forEach((x) => S("line", { x1: x, y1: -58, x2: x, y2: -3, ...OL, "stroke-width": 0.9 }, t));
        R(t, -80, -98, 34, 30, 4, "var(--machine)"); S("circle", { cx: -63, cy: -84, r: 4, fill: "var(--accent-c)" }, t);
        R(t, 10, -76, 26, 8, 3, "var(--metal)");
        R(t, 60, -82, 10, 14, 2, "var(--mug)"); R(t, 76, -80, 10, 12, 2, "var(--mug)");
      });
      put(0.62, 0.04, (t) => { R(t, -26, -124, 52, 124, 6, "var(--fridge)"); S("line", { x1: -26, y1: -78, x2: 26, y2: -78, ...OL }, t); R(t, 16, -110, 4, 22, 2, "var(--metal)"); R(t, 16, -66, 4, 22, 2, "var(--metal)"); });
      const idle = chars.filter((c) => c.st === "idle");
      if (idle[0]) put(0.32, 0.3, (t, X, Y, s) => person(t, idle[0], out, X, Y, s, { pose: "mug" }));
      put(0.28, 0.62, (t) => { R(t, -4, -50, 8, 50, 2, "var(--metal)"); S("ellipse", { cx: 0, cy: -52, rx: 46, ry: 10, fill: "var(--wood)", ...OL }, t); R(t, -16, -66, 10, 12, 2, "var(--mug)"); });
      [0.12, 0.44].forEach((u) => put(u, 0.66, (t) => { R(t, -2, -30, 4, 30, 1, "var(--metal)"); S("ellipse", { cx: 0, cy: -31, rx: 14, ry: 5, fill: "var(--cushion)", ...OL }, t); }));
      // sofa + penghuni
      put(0.77, 0.5, (t) => sofaBack(t, 150));
      if (idle[1]) put(0.79, 0.53, (t, X, Y, s) => person(t, idle[1], out, X, Y, s, { seated: true, sofa: true }));
      put(0.77, 0.56, (t) => sofaFront(t, 150));
      put(0.96, 0.92, (t) => plant(t, 1, true));
    }

    items.sort((a, b) => a.d - b.d).forEach((it) => it.f(g));
    // bingkai depan ruangan (tepi slab tipis)
    S("rect", { x: x0, y: y0, width: W, height: H, fill: "none", ...OL, "stroke-width": 1.6 }, g);
    out.rooms[r.k] = { box: { x0: x0 - 16, y0: y0 - 60, x1: x1 + 16, y1: y1 + 16 }, label: [x0 + W / 2, y0 - SLAB / 2 - (r.row === 0 ? 8 : 0)], pos: "center" };
  }

  /* ----- perabot (koordinat lokal: (0,0) = titik pijak tengah, y ke atas negatif) ----- */
  function desk(g, ch) {
    R(g, -48, -54, 8, 54, 2, dk("var(--wood)", 18));
    R(g, 40, -54, 8, 54, 2, dk("var(--wood)", 18));
    R(g, -42, -48, 84, 26, 3, dk("var(--wood)", 8));
    R(g, -54, -60, 108, 9, 3, "var(--wood)");
    // laptop (punggung layar menghadap kita)
    const on = ch && ch.st !== "offline";
    R(g, -46, -82, 32, 22, 4, "var(--laptop)");
    S("circle", { cx: -30, cy: -71, r: 3.2, fill: on ? ch.color : "var(--laptop-off)", cls: on ? "screen-glow" : "" }, g);
    R(g, 30, -72, 11, 12, 2, "var(--mug)");
    if (!ch) R(g, -54, -66, 26, 6, 1.5, "var(--paper)");
  }
  function chairBack(g) { R(g, -22, -118, 44, 58, 11, "var(--chair)"); R(g, -3, -58, 6, 20, 1, "var(--chair-leg)", false); }
  function sofaBack(g, w) { R(g, -w / 2, -74, w, 44, 14, "var(--sofa)"); }
  function sofaFront(g, w) {
    R(g, -w / 2 + 4, -40, w - 8, 26, 8, lt("var(--sofa)", 14));
    R(g, -w / 2 - 8, -54, 20, 50, 8, dk("var(--sofa)", 6));
    R(g, w / 2 - 12, -54, 20, 50, 8, dk("var(--sofa)", 6));
    R(g, -w / 2 + 10, -14, 6, 14, 1, "var(--chair-leg)", false); R(g, w / 2 - 16, -14, 6, 14, 1, "var(--chair-leg)", false);
  }
  function plant(g, s = 1, tall = false) {
    const t = S("g", { transform: `scale(${s})` }, g);
    const h = tall ? 34 : 26;
    if (tall) { [[-14, -96, -30], [12, -104, 25], [0, -118, 0], [-20, -70, -50], [18, -76, 45]].forEach(([x, y, rot], i) => S("ellipse", { cx: x, cy: y, rx: 9, ry: 22, transform: `rotate(${rot} ${x} ${y})`, fill: i % 2 ? "var(--leaf2)" : "var(--leaf1)", ...OL, "stroke-width": 1 }, t)); S("line", { x1: 0, y1: -h, x2: 0, y2: -110, stroke: "var(--ink)", "stroke-width": 1.2 }, t); }
    else[[-10, -42, -35], [9, -44, 35], [0, -52, 0]].forEach(([x, y, rot], i) => S("ellipse", { cx: x, cy: y, rx: 7, ry: 15, transform: `rotate(${rot} ${x} ${y})`, fill: i % 2 ? "var(--leaf2)" : "var(--leaf1)", ...OL, "stroke-width": 1 }, t));
    S("path", { d: `M-15 ${-h} L15 ${-h} L11 0 L-11 0Z`, fill: "var(--pot)", ...OL }, t);
    R(t, -17, -h - 5, 34, 7, 2, lt("var(--pot)", 12));
  }
  function box(g) { R(g, -22, -30, 44, 30, 2, "var(--cardboard)"); S("line", { x1: -22, y1: -22, x2: 22, y2: -22, ...OL, "stroke-width": 0.8 }, g); }
  function win(g, x, y, w, h) {
    R(g, x - 4, y - 4, w + 8, h + 8, 4, "var(--frame)");
    R(g, x, y, w, h, 2, "url(#v3sky)", false);
    S("circle", { cx: x + w * 0.72, cy: y + h * 0.3, r: 6, fill: "var(--sun)", cls: "v3-sun" }, g);
    S("ellipse", { cx: x + w * 0.3, cy: y + h * 0.42, rx: 11, ry: 4, fill: "var(--cloud)", cls: "v3-cloud" }, g);
    S("line", { x1: x + w / 2, y1: y, x2: x + w / 2, y2: y + h, stroke: "var(--frame)", "stroke-width": 3 }, g);
    S("line", { x1: x, y1: y + h / 2, x2: x + w, y2: y + h / 2, stroke: "var(--frame)", "stroke-width": 3 }, g);
    R(g, x - 8, y + h + 3, w + 16, 6, 2, "var(--frame)");
    // tirai
    S("path", { d: `M${x - 10} ${y - 8} Q${x - 2} ${y + h / 2} ${x - 8} ${y + h + 4} L${x - 16} ${y + h + 4} L${x - 16} ${y - 8}Z`, fill: "var(--curtain)", ...OL, "stroke-width": 1 }, g);
    S("path", { d: `M${x + w + 10} ${y - 8} Q${x + w + 2} ${y + h / 2} ${x + w + 8} ${y + h + 4} L${x + w + 16} ${y + h + 4} L${x + w + 16} ${y - 8}Z`, fill: "var(--curtain)", ...OL, "stroke-width": 1 }, g);
    S("rect", { x: x - 18, y: y - 11, width: w + 36, height: 4, rx: 2, fill: "var(--ink)" }, g);
  }
  function kanban(g, x, y, w, h, c) {
    R(g, x, y, w, h, 4, "var(--board)");
    const cw = (w - 16) / 3;
    [0, 1, 2].forEach((i) => {
      R(g, x + 6 + i * (cw + 2), y + 6, cw, 5, 2, i === 0 ? c : i === 1 ? "var(--st-review)" : "var(--st-online)", false);
      for (let n = 0; n < 3 - (i === 2 ? 1 : 0); n++) R(g, x + 9 + i * (cw + 2), y + 16 + n * 15, cw - 6, 11, 2, ["var(--note1)", "var(--note2)", "var(--note3)"][(n + i) % 3], true, { "stroke-width": 0.7 });
    });
  }
  function shelf(g, x, y, w, c) {
    R(g, x, y + 22, w, 5, 2, "var(--wood)");
    const cols = [c, "var(--book1)", "var(--book2)", "var(--book3)", lt(c, 40)];
    for (let i = 0; i < 9; i++) { const bh = 14 + (i * 7) % 8; R(g, x + 6 + i * 8.5, y + 22 - bh, 7, bh, 1, cols[i % 5], true, { "stroke-width": 0.7 }); }
    const px = x + w - 22; S("path", { d: `M${px - 8} ${y + 8} L${px + 8} ${y + 8} L${px + 6} ${y + 22} L${px - 6} ${y + 22}Z`, fill: "var(--pot)", ...OL, "stroke-width": 0.8 }, g);
    [[-6, 2], [0, -2], [6, 2]].forEach(([dx, dy]) => S("circle", { cx: px + dx, cy: y + dy, r: 6, fill: "var(--leaf1)", ...OL, "stroke-width": 0.8 }, g));
  }
  function clock(g, x, y) { S("circle", { cx: x, cy: y, r: 11, fill: "var(--paper)", ...OL }, g); S("path", { d: `M${x} ${y - 7} L${x} ${y} L${x + 5} ${y + 2}`, fill: "none", stroke: "var(--ink)", "stroke-width": 1.4, "stroke-linecap": "round" }, g); }
  function tv(g, x, y, w, h) {
    R(g, x, y, w, h, 5, "var(--tv)");
    R(g, x + 6, y + 6, w - 12, h - 12, 2, "var(--tv-screen)", false);
    R(g, x + 16, y + 16, 60, 8, 2, "#fff", false, { opacity: 0.9 });
    ["#8ec5ff", "#ffb4c8", "#a6e3c4"].forEach((c, i) => R(g, x + 16 + i * 40, y + 34, 32, 30, 3, c, false, { opacity: 0.9 }));
  }
  function tree(g, x, y, s) {
    const t = S("g", { transform: `translate(${x} ${y}) scale(${s})` }, g);
    R(t, -5, -60, 10, 60, 3, "var(--trunk)");
    [[-22, -78, 26], [20, -82, 24], [0, -108, 30], [0, -76, 22]].forEach(([dx, dy, r], i) => S("circle", { cx: dx, cy: dy, r, fill: i % 2 ? "var(--leaf2)" : "var(--leaf1)", ...OL }, t));
  }
  function bush(g, x, y) { [[-16, -14, 16], [14, -14, 15], [0, -24, 18]].forEach(([dx, dy, r], i) => S("circle", { cx: x + dx, cy: y + dy, r, fill: i % 2 ? "var(--leaf2)" : "var(--leaf1)", ...OL }, g)); }
  function bushTop(g, x, y, i) { [[-12, -6, 10], [10, -7, 10], [0, -14, 12]].forEach(([dx, dy, r], j) => S("circle", { cx: x + dx, cy: y + dy, r: r * (i % 2 ? 0.9 : 1), fill: j % 2 ? "var(--leaf2)" : "var(--leaf1)", ...OL, "stroke-width": 1 }, g)); }

  function person(t, ch, out, X, Y, s, o) {
    const gg = S("g", { cls: "ch", "data-ch": ch.id, transform: `translate(0 ${o.seated ? (o.sofa ? -26 : -40) : 0})` }, t);
    if (ch.st !== "idle") S("ellipse", { cx: 0, cy: 0, rx: 26, ry: 6.5, cls: `halo st-${ch.st}` }, gg);
    S("ellipse", { cx: 0, cy: 0, rx: 30, ry: 8, cls: "selring" }, gg);
    const sc = S("g", { transform: "scale(1.5)" }, gg);
    chibi(sc, ch, { outline: 0.9, hideLegs: o.seated, pose: o.pose || (o.seated && ch.st === "working" ? "type" : "stand") });
    S("rect", { x: -26, y: -92, width: 52, height: 94, fill: "transparent", cls: "hit" }, gg);
    const lift = o.seated ? (o.sofa ? -26 : -40) : 0;
    out.chars[ch.id] = [X, Y + (lift - 90) * s];
  }

  window.VARIANTS = window.VARIANTS || {};
  window.VARIANTS.v3 = { cls: "v3", name: "V3 · Diorama hangat", render, lod: 0.5, zoom: { min: 0.3, max: 2.4, focus: 1.3 } };
})();
