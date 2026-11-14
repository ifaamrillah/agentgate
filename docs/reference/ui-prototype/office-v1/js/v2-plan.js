/* V2 · Denah: floor-plan modern dari atas, gaya gambar arsitek yang tipis dan tenang.
   Dinding tebal arang + bukaan pintu dengan busur ayun, furnitur geometris, karakter = token avatar
   dengan cincin status. Tidak ada urutan kedalaman, jadi paling sederhana dan paling skalabel. */
(() => {
  const { S, dk, lt, mixC } = window.SV;
  const RW = 420, RH = 300, GAP = 40, PAD = 18;

  function render({ world, defs, data }) {
    const pat = S("pattern", { id: "v2dots", width: 24, height: 24, patternUnits: "userSpaceOnUse" }, defs);
    S("circle", { cx: 1.5, cy: 1.5, r: 1.1, fill: "var(--dot)" }, pat);
    const cols = 3;
    const BW = cols * RW + (cols - 1) * GAP + PAD * 2, BH = 2 * RH + GAP + PAD * 2;
    const out = { chars: {}, rooms: {} };
    S("rect", { x: -2000, y: -2000, width: BW + 4000, height: BH + 4000, fill: "url(#v2dots)" }, world);
    // gedung
    S("rect", { x: 6, y: 10, width: BW, height: BH, rx: 14, fill: "var(--drop)", filter: "url(#v2blur)" }, world);
    const bl = S("filter", { id: "v2blur", x: "-10%", y: "-10%", width: "120%", height: "120%" }, defs); S("feGaussianBlur", { stdDeviation: 10 }, bl);
    S("rect", { x: 0, y: 0, width: BW, height: BH, rx: 12, fill: "var(--corridor)" }, world);
    // lorong: garis penunjuk arah halus
    S("line", { x1: PAD + 20, y1: PAD + RH + GAP / 2, x2: BW - PAD - 20, y2: PAD + RH + GAP / 2, stroke: "var(--guide)", "stroke-width": 1.2, "stroke-dasharray": "2 6", "stroke-linecap": "round" }, world);

    const rooms = data.rooms.map((r, i) => {
      const cc = i % cols, rw = Math.floor(i / cols);
      return { ...r, cc, rw, x0: PAD + cc * (RW + GAP), y0: PAD + rw * (RH + GAP) };
    });
    rooms.forEach((r) => drawRoom(world, r, data.chars.filter((c) => c.room === r.k), out, BH));
    // dinding luar di atas segalanya + jendela
    S("rect", { x: 0, y: 0, width: BW, height: BH, rx: 12, fill: "none", stroke: "var(--wall)", "stroke-width": 7 }, world);
    rooms.forEach((r) => {
      if (r.rw === 0) [60, 260].forEach((dx) => win(world, r.x0 + dx, -3.5, 100, true));
      else [60, 260].forEach((dx) => win(world, r.x0 + dx, BH - 3.5, 100, true));
      if (r.cc === 0) win(world, -3.5, r.y0 + 100, 100, false);
      if (r.cc === cols - 1) win(world, BW - 3.5, r.y0 + 100, 100, false);
    });
    out.bounds = { x0: -30, y0: -70, x1: BW + 30, y1: BH + 70 };
    return out;
  }
  function win(g, x, y, len, horiz) {
    const a = horiz ? { x, y, width: len, height: 7 } : { x, y, width: 7, height: len };
    S("rect", { ...a, fill: "var(--glass)" }, g);
    S("line", horiz ? { x1: x, y1: y + 3.5, x2: x + len, y2: y + 3.5 } : { x1: x + 3.5, y1: y, x2: x + 3.5, y2: y + len }, g).style.cssText = "stroke: var(--glass-line); stroke-width: 1.2";
  }

  function drawRoom(world, r, chars, out, BH) {
    const { x0, y0 } = r, x1 = x0 + RW, y1 = y0 + RH;
    const g = S("g", { cls: `room k-${r.kind}` }, world);
    const floor = r.kind === "pantry" ? "var(--floor-pantry)" : r.kind === "meeting" ? "var(--floor-meet)" : "var(--floor)";
    S("rect", { x: x0, y: y0, width: RW, height: RH, fill: mixC(floor, r.c, r.kind === "project" ? 5 : 3) }, g);
    if (r.kind === "pantry") {
      const tp = S("g", { stroke: "var(--tile)", "stroke-width": 1 }, g);
      for (let x = x0 + 30; x < x1; x += 30) S("line", { x1: x, y1: y0, x2: x, y2: y1 }, tp);
      for (let y = y0 + 30; y < y1; y += 30) S("line", { x1: x0, y1: y, x2: x1, y2: y }, tp);
    }
    // aksen warna project di sisi kiri
    if (r.kind === "project") S("rect", { x: x0, y: y0, width: 4, height: RH, fill: r.c, opacity: 0.55 }, g);

    const fx = S("g", { cls: "furn" }, g);
    const ppl = S("g", { cls: "ppl" }, g);
    const desk = (x, y, w = 70, h = 36, monTop = true) => {
      rect(fx, x, y, w, h, 5, "var(--desk)", "var(--line-f)");
      rect(fx, x + w / 2 - 17, monTop ? y + 4 : y + h - 8, 34, 4, 2, "var(--mon)");
      rect(fx, x + w / 2 - 11, monTop ? y + 14 : y + h - 19, 22, 6, 1.5, "var(--kb)");
    };
    const chair = (x, y, back = "top") => {
      rect(fx, x - 11, y - 11, 22, 22, 6, "var(--chair)", "var(--line-f)");
      const by = back === "top" ? y - 13 : y + 9;
      rect(fx, x - 10, by, 20, 4, 2, dk("var(--chair)", 20));
    };
    const plant = (x, y, s = 1) => {
      S("circle", { cx: x, cy: y, r: 14 * s, fill: "var(--pot)", stroke: "var(--line-f)" }, fx);
      for (let i = 0; i < 6; i++) { const a = (i / 6) * Math.PI * 2; S("ellipse", { cx: x + Math.cos(a) * 7 * s, cy: y + Math.sin(a) * 7 * s, rx: 7 * s, ry: 4 * s, transform: `rotate(${(a * 180) / Math.PI} ${x + Math.cos(a) * 7 * s} ${y + Math.sin(a) * 7 * s})`, fill: i % 2 ? "var(--leaf1)" : "var(--leaf2)" }, fx); }
      S("circle", { cx: x, cy: y, r: 3.5 * s, fill: "var(--leaf3)" }, fx);
    };
    const rug = (x, y, w, h, c) => { rect(fx, x, y, w, h, 10, mixC("var(--rug)", c, 14)); S("rect", { x: x + 6, y: y + 6, width: w - 12, height: h - 12, rx: 6, fill: "none", stroke: mixC("var(--rug)", c, 40), "stroke-width": 1, "stroke-dasharray": "3 3" }, fx); };
    const sofa = (x, y, w, h, c, open = "top") => {
      rect(fx, x, y, w, h, 9, c, "var(--line-f)");
      if (open === "top") { rect(fx, x + 7, y, w - 14, h - 10, 6, lt(c, 18)); }
      else if (open === "left") { rect(fx, x, y + 7, w - 10, h - 14, 6, lt(c, 18)); }
    };

    const seats = [];
    if (r.kind === "project") {
      // pod 2×2 meja
      const px = x0 + 48, py = y0 + 92;
      desk(px, py, 72, 36, false); desk(px + 76, py, 72, 36, false);
      desk(px, py + 38, 72, 36, true); desk(px + 76, py + 38, 72, 36, true);
      [[px + 36, py - 14, "top"], [px + 112, py + 88, "bottom"], [px + 112, py - 14, "top"], [px + 36, py + 88, "bottom"]].forEach(([x, y, b]) => { chair(x, y, b); seats.push([x, y]); });
      // papan kanban di dinding atas
      const kx = x0 + 40;
      rect(fx, kx, y0 + 8, 150, 12, 3, "var(--board)", "var(--line-f)");
      ["var(--note1)", "var(--note2)", "var(--note3)", "var(--note1)", "var(--note2)"].forEach((c, i) => rect(fx, kx + 8 + i * 28, y0 + 10, 20, 8, 1.5, c));
      // area santai kanan
      rug(x0 + 238, y0 + 150, 160, 120, r.c);
      sofa(x0 + 252, y0 + 160, 132, 40, mixC("var(--sofa)", r.c, 25));
      rect(fx, x0 + 282, y0 + 218, 72, 32, 8, "var(--wood)", "var(--line-f)");
      rect(fx, x0 + 292, y0 + 226, 14, 10, 2, "var(--paper)");
      // rak + tanaman
      rect(fx, x0 + 250, y0 + 8, 130, 22, 3, "var(--wood)", "var(--line-f)");
      for (let i = 0; i < 9; i++) rect(fx, x0 + 256 + i * 13.5, y0 + 11, 10, 15, 1.5, ["var(--book1)", "var(--book2)", "var(--book3)", r.c][i % 4]);
      plant(x0 + 398, y0 + 24); plant(x0 + 26, y0 + 274, 0.9); plant(x0 + 220, y0 + 36, 0.75);
    } else if (r.kind === "meeting") {
      rug(x0 + 70, y0 + 80, 280, 160, r.c);
      rect(fx, x0 + 120, y0 + 128, 180, 64, 30, "var(--wood)", "var(--line-f)");
      [150, 210, 270].forEach((x) => { chair(x0 + x, y0 + 112, "top"); chair(x0 + x, y0 + 208, "bottom"); });
      seats.push([x0 + 150, y0 + 112], [x0 + 210, y0 + 112]);
      rect(fx, x0 + 140, y0 + 8, 140, 10, 3, "var(--tv)");
      S("circle", { cx: x0 + 210, cy: y0 + 160, r: 7, fill: "var(--paper)", stroke: "var(--line-f)" }, fx);
      plant(x0 + 30, y0 + 30); plant(x0 + 392, y0 + 30); plant(x0 + 392, y0 + 272, 0.9);
      seats.unshift([x0 + 210, y0 + 48]);
    } else if (r.kind === "pantry") {
      // konter L
      rect(fx, x0 + 20, y0 + 10, 230, 38, 5, "var(--counter)", "var(--line-f)");
      rect(fx, x0 + 20, y0 + 10, 38, 120, 5, "var(--counter)", "var(--line-f)");
      S("ellipse", { cx: x0 + 120, cy: y0 + 29, rx: 18, ry: 11, fill: "var(--sink)", stroke: "var(--line-f)" }, fx);
      rect(fx, x0 + 170, y0 + 16, 26, 26, 5, "var(--mon)");
      S("circle", { cx: x0 + 183, cy: y0 + 29, r: 5, fill: "var(--accent-c)" }, fx);
      rect(fx, x0 + 268, y0 + 10, 50, 44, 5, "var(--fridge)", "var(--line-f)");
      S("line", { x1: x0 + 268, y1: y0 + 32, x2: x0 + 318, y2: y0 + 32 }, fx).style.cssText = "stroke: var(--line-f)";
      // meja bundar
      [[x0 + 140, y0 + 150]].forEach(([x, y]) => {
        S("circle", { cx: x, cy: y, r: 30, fill: "var(--wood)", stroke: "var(--line-f)" }, fx);
        [[-44, 0], [44, 0], [0, -44], [0, 44]].forEach(([dx, dy]) => S("circle", { cx: x + dx, cy: y + dy, r: 10, fill: "var(--cushion)", stroke: "var(--line-f)" }, fx));
      });
      rug(x0 + 240, y0 + 150, 160, 130, "#e8a87c");
      sofa(x0 + 250, y0 + 160, 44, 110, "var(--sofa)", "left");
      rect(fx, x0 + 320, y0 + 190, 50, 50, 8, "var(--wood)", "var(--line-f)");
      plant(x0 + 398, y0 + 74); plant(x0 + 30, y0 + 270, 0.9);
      seats.push([x0 + 120, y0 + 72], [x0 + 272, y0 + 214]);
    }

    // pintu: bukaan + busur ayun (baris belakang: dinding bawah; baris depan: dinding atas)
    const wallC = { stroke: "var(--wall-in)", "stroke-width": 4, "stroke-linecap": "square" };
    const dx = r.kind === "pantry" ? x0 + 200 : x0 + 190, dW = 44;
    const doorY = r.rw === 0 ? y1 : y0;
    S("path", { d: `M${x0} ${y0} H${x1} V${y1} H${x0} Z`, fill: "none", ...wallC }, g);
    S("rect", { x: dx, y: doorY - 4, width: dW, height: 8, fill: "var(--corridor)" }, g);
    const sw = r.rw === 0 ? -1 : 1;
    S("path", { d: `M${dx} ${doorY} L${dx} ${doorY + sw * dW} A${dW} ${dW} 0 0 ${sw > 0 ? 0 : 1} ${dx + dW} ${doorY}`, fill: "none", stroke: "var(--door)", "stroke-width": 1, "stroke-dasharray": "0" }, g);
    S("line", { x1: dx, y1: doorY, x2: dx, y2: doorY + sw * dW, stroke: "var(--wall-in)", "stroke-width": 2 }, g);

    // karakter
    const order = r.kind === "project" ? chars : chars;
    order.forEach((c, i) => {
      const p = seats[i] || [x0 + 60 + i * 50, y0 + 250];
      token(ppl, c, p, out);
    });

    out.rooms[r.k] = { box: { x0: x0 - 20, y0: r.rw === 0 ? y0 - 80 : y0 - 30, x1: x1 + 20, y1: r.rw === 0 ? y1 + 20 : y1 + 80 }, label: r.rw === 0 ? [x0 + RW / 2, -6] : [x0 + RW / 2, BH + 4], pos: r.rw === 0 ? "above" : "below" };
  }

  function rect(g, x, y, w, h, rx, fill, stroke) { return S("rect", { x, y, width: w, height: h, rx, fill, stroke, "stroke-width": stroke ? 1 : null }, g); }

  function token(g, c, [x, y], out) {
    const gg = S("g", { cls: `ch tok st-${c.st}`, "data-ch": c.id, transform: `translate(${x} ${y})` }, g);
    S("circle", { r: 27, cls: "pulse" }, gg);
    S("circle", { r: 21, cls: "ring" }, gg);
    S("circle", { r: 25, cls: "selring" }, gg);
    S("circle", { r: 17, fill: "var(--tok-shadow)", cy: 2 }, gg);
    if (c.bot) {
      S("circle", { r: 17.5, fill: c.color, cls: "face" }, gg);
      const ic = S("svg", { x: -10, y: -10, width: 20, height: 20, viewBox: "0 0 24 24", fill: "none", stroke: "#fff", "stroke-width": 2.2, "stroke-linecap": "round", "stroke-linejoin": "round" }, gg);
      ic.innerHTML = window.ICON_PATHS[c.icon] || window.ICON_PATHS.robot;
    } else {
      S("circle", { r: 16.5, fill: c.skin || "#e8b48f", cls: "face" }, gg);
      // rambut dari atas: tudung + poni
      if (c.style === "hijab") S("path", { d: "M-16.5 0 A16.5 16.5 0 0 1 16.5 0 L16.5 6 Q0 -6 -16.5 6Z", fill: c.hijab || "#5f8f7a" }, gg);
      else S("path", { d: "M-16 -4 A16.5 16.5 0 0 1 16 -4 Q8 -9 0 -6 Q-8 -10 -16 -4Z", fill: c.hair || "#2a1d16" }, gg);
      const t = S("text", { y: 7.5, "text-anchor": "middle", cls: "ini", text: c.name[0] }, gg);
      void t;
      S("circle", { cx: 12, cy: 12, r: 4.5, fill: "var(--st-online)", stroke: "var(--floor)", "stroke-width": 2 }, gg);
    }
    S("circle", { r: 28, fill: "transparent", cls: "hit" }, gg);
    out.chars[c.id] = [x, y - 28];
  }

  window.VARIANTS = window.VARIANTS || {};
  window.VARIANTS.v2 = { cls: "v2", name: "V2 · Denah", render, lod: 0.55, zoom: { min: 0.35, max: 2.6, focus: 1.3 } };
})();
