/* Helper SVG bersama + karakter chibi (dipakai V1 dan V3).
   Warna lewat CSS custom property (var(--x)) supaya tema terang/gelap tidak perlu gambar ulang. */
(() => {
  const NS = "http://www.w3.org/2000/svg";
  const STYLE = new Set(["fill", "stroke", "opacity", "fill-opacity", "stroke-opacity", "stop-color", "stop-opacity", "filter", "mix-blend-mode"]);
  function S(tag, a = {}, parent) {
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
  const pts = (p) => p.map((q) => `${q[0].toFixed(2)},${q[1].toFixed(2)}`).join(" ");
  const poly = (p, a, parent) => S("polygon", { points: pts(p), ...a }, parent);
  const mixC = (c, o, pct) => `color-mix(in oklab, ${c}, ${o} ${pct}%)`;
  const dk = (c, p) => mixC(c, "black", p);
  const lt = (c, p) => mixC(c, "white", p);
  const g = (parent, cls, extra = {}) => S("g", { cls, ...extra }, parent);
  const rr = (x, y, w, h, r, a, parent) => S("rect", { x, y, width: w, height: h, rx: r, ...a }, parent);

  /* Gradien linear sederhana ke <defs>. */
  let gid = 0;
  function grad(defs, stops, { x1 = 0, y1 = 0, x2 = 0, y2 = 1, id } = {}) {
    id = id || `gr${++gid}`;
    const lg = S("linearGradient", { id, x1, y1, x2, y2 }, defs);
    stops.forEach(([o, c, op]) => S("stop", { offset: o, "stop-color": c, "stop-opacity": op ?? 1 }, lg));
    return `url(#${id})`;
  }
  function radial(defs, stops, id) {
    id = id || `rg${++gid}`;
    const lg = S("radialGradient", { id }, defs);
    stops.forEach(([o, c, op]) => S("stop", { offset: o, "stop-color": c, "stop-opacity": op ?? 1 }, lg));
    return `url(#${id})`;
  }

  /* ---------------------------------------------------------------
     Chibi menghadap depan. Titik (0,0) = tengah kaki. Tinggi ±56.
     o: { outline, hideLegs, pose: "type"|"stand"|"mug"|"talk"|"sit" }
     --------------------------------------------------------------- */
  function chibi(parent, ch, o = {}) {
    const root = S("g", { cls: `chibi st-${ch.st} pose-${o.pose || "stand"}` }, parent);
    const ol = o.outline ? { stroke: "var(--ink)", "stroke-width": o.outline, "stroke-linejoin": "round" } : {};
    const col = ch.color;
    S("ellipse", { cx: 0, cy: 0, rx: 12.5, ry: 4.4, fill: "var(--ch-shadow)" }, root);
    const bob = S("g", { cls: "bob" }, root);
    if (!o.hideLegs) {
      const legs = S("g", { cls: "legs" }, bob);
      rr(-6.2, -13, 5.2, 12.5, 2.4, { fill: ch.bot ? "var(--bot-joint)" : "var(--pants)", ...ol }, legs);
      rr(1, -13, 5.2, 12.5, 2.4, { fill: ch.bot ? "var(--bot-joint)" : "var(--pants)", ...ol }, legs);
      S("ellipse", { cx: -3.6, cy: -0.8, rx: 3.8, ry: 2, fill: "var(--shoes)" }, legs);
      S("ellipse", { cx: 3.6, cy: -0.8, rx: 3.8, ry: 2, fill: "var(--shoes)" }, legs);
    }
    // Lengan belakang badan
    const armL = rr(-12.6, -27, 5, 13, 2.5, { fill: dk(col, 14), ...ol, cls: "arm arm-l" }, bob);
    const armR = rr(7.6, -27, 5, 13, 2.5, { fill: dk(col, 22), ...ol, cls: "arm arm-r" }, bob);
    // Badan
    S("path", { d: "M-9.5 -28 Q-10.5 -12 -9 -11 L9 -11 Q10.5 -12 9.5 -28 Q0 -31.5 -9.5 -28Z", fill: col, ...ol }, bob);
    S("path", { d: "M1.5 -29.8 Q6 -29.5 9.5 -28 Q10.5 -12 9 -11 L1.5 -11Z", fill: "#000", opacity: 0.1 }, bob);
    if (ch.bot) {
      S("circle", { cx: 0, cy: -20, r: 3.3, fill: "var(--bot-shell)", opacity: 0.95 }, bob);
      S("circle", { cx: 0, cy: -20, r: 1.5, fill: lt(col, 10) }, bob);
    } else {
      S("path", { d: "M-4 -29.5 L0 -25 L4 -29.5", fill: "none", stroke: lt(col, 55), "stroke-width": 1.4, "stroke-linecap": "round" }, bob);
    }
    if (o.pose === "mug") {
      rr(6.5, -21, 6, 6.5, 1.6, { fill: "var(--mug)", ...ol }, bob);
      S("path", { d: "M12.5 -19.5 q2.6 1.4 0 3.4", fill: "none", stroke: "var(--mug-handle)", "stroke-width": 1.3 }, bob);
    }
    const head = S("g", { cls: "head" }, bob);
    if (ch.bot) {
      S("line", { x1: 0, y1: -50, x2: 0, y2: -56, stroke: "var(--bot-joint)", "stroke-width": 1.6 }, head);
      S("circle", { cx: 0, cy: -57, r: 2.6, fill: col, cls: "antenna" }, head);
      rr(-13.5, -51, 27, 22, 8, { fill: "var(--bot-shell)", ...ol }, head);
      S("path", { d: "M-13.5 -38 Q-13.5 -29 -5 -29 L5 -29 Q13.5 -29 13.5 -38 Z", fill: "var(--bot-shell-d)" }, head);
      rr(-10, -46.5, 20, 11.5, 5.5, { fill: "var(--bot-visor)" }, head);
      const ey = S("g", { cls: "eyes" }, head);
      rr(-6.2, -43.5, 4, 5.2, 2, { fill: lt(col, 35) }, ey);
      rr(2.2, -43.5, 4, 5.2, 2, { fill: lt(col, 35) }, ey);
      S("circle", { cx: -14.4, cy: -40, r: 2.2, fill: col }, head);
      S("circle", { cx: 14.4, cy: -40, r: 2.2, fill: col }, head);
    } else {
      const skin = ch.skin || "#efc3a0", hair = ch.hair || "#2a1d16";
      if (ch.style === "hijab") {
        const hj = ch.hijab || "#6b8f7f";
        S("path", { d: "M-13.5 -38 Q-14 -53 0 -53.5 Q14 -53 13.5 -38 Q14 -28 9.5 -26 L-9.5 -26 Q-14 -28 -13.5 -38Z", fill: hj, ...ol }, head);
        S("ellipse", { cx: 0, cy: -38, rx: 8.6, ry: 9.4, fill: skin }, head);
        S("path", { d: "M-8.6 -41 Q0 -49 8.6 -41 Q8 -46.5 0 -47 Q-8 -46.5 -8.6 -41Z", fill: hj }, head);
      } else {
        S("circle", { cx: 0, cy: -39, r: 11.5, fill: skin, ...ol }, head);
        S("path", { d: "M-12 -38.5 Q-13 -53 0 -52.5 Q13 -53 12 -38.5 Q10 -45 3 -45.5 Q-5 -44 -8 -47 Q-9.5 -42 -12 -38.5Z", fill: hair, ...ol }, head);
      }
      S("ellipse", { cx: -4, cy: -37.5, rx: 1.35, ry: 1.7, fill: "var(--eye)" }, head);
      S("ellipse", { cx: 4, cy: -37.5, rx: 1.35, ry: 1.7, fill: "var(--eye)" }, head);
      S("circle", { cx: -6.8, cy: -33.8, r: 2, fill: "#ff8f8f", opacity: 0.32 }, head);
      S("circle", { cx: 6.8, cy: -33.8, r: 2, fill: "#ff8f8f", opacity: 0.32 }, head);
      S("path", { d: "M-1.6 -32.6 Q0 -31.4 1.6 -32.6", fill: "none", stroke: "var(--eye)", "stroke-width": 1, "stroke-linecap": "round" }, head);
      if (ch.acc === "glasses") {
        S("circle", { cx: -4, cy: -37.5, r: 3.4, fill: "none", stroke: "var(--eye)", "stroke-width": 0.9 }, head);
        S("circle", { cx: 4, cy: -37.5, r: 3.4, fill: "none", stroke: "var(--eye)", "stroke-width": 0.9 }, head);
        S("line", { x1: -0.6, y1: -37.5, x2: 0.6, y2: -37.5, stroke: "var(--eye)", "stroke-width": 0.9 }, head);
      }
    }
    void armL; void armR;
    return root;
  }

  window.SV = { NS, S, poly, pts, mixC, dk, lt, g, rr, grad, radial, chibi };
})();
