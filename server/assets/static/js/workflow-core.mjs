// Logika murni kanvas builder workflow (R3), dipakai workflow-canvas.js dan diuji node
// (server/tests/js/workflow_core.test.mjs). Tanpa DOM.
//
// Tata letak otomatis mengikuti urutan stage di definisi (prototype "Workflow builder"):
// satu baris per stage, stage `paralel` bergabung dengan baris sebelumnya (berdampingan, dibungkus
// kotak kelompok). Panah maju dari setiap node ke setiap node baris berikutnya; panah gagal
// (`gagal_ke`) dari sisi kanan stage ke stage sebelumnya, di lajur kanan (beberapa panah gagal
// memakai lajur berbeda). Koordinat dunia dalam px; kamera = { x, y, k } (translate lalu scale).

export const NODE = { w: 236, h: 56 };
export const GAP_X = 32;
export const ROW_H = 100;
export const PAD = 28;
export const FAIL_LANE = 120;
export const LANE_STEP = 22;
/** Ruang label panah gagal di kanan lajur ("Gagal → Development, maks 3×"). */
export const LABEL_W = 200;
export const ZOOM_MIN = 0.35;
export const ZOOM_MAX = 2;

/** Baris tata letak: daftar kunci per baris. */
export function rows(stages) {
  const out = [];
  stages.forEach((s, i) => {
    if (i > 0 && s.parallel && out.length) out[out.length - 1].push(s.key);
    else out.push([s.key]);
  });
  return out;
}

/**
 * Tata letak graf.
 * @param {{key: string, exit?: string, parallel?: boolean, failTo?: string}[]} stages urut definisi
 * @returns {{ pos: Record<string,{x:number,y:number}>, width: number, height: number,
 *   edges: {from:string,to:string,d:string}[], badges: {key:string,x:number,y:number,exit:string}[],
 *   fails: {from:string,to:string,d:string,lane:number,label:{x:number,y:number}}[],
 *   groups: {keys:string[],x:number,y:number,w:number,h:number}[] }}
 */
export function layout(stages) {
  const R = rows(stages);
  const rowWidth = (n) => n * NODE.w + (n - 1) * GAP_X;
  const inner = Math.max(NODE.w, ...R.map((r) => rowWidth(r.length)));
  const pos = {};
  R.forEach((r, ri) => {
    const left = PAD + (inner - rowWidth(r.length)) / 2;
    r.forEach((k, i) => { pos[k] = { x: left + i * (NODE.w + GAP_X), y: PAD + ri * ROW_H }; });
  });
  const byKey = Object.fromEntries(stages.map((s) => [s.key, s]));
  const index = Object.fromEntries(R.flatMap((r, ri) => r.map((k) => [k, ri])));

  const edges = [];
  for (let ri = 0; ri < R.length - 1; ri++) {
    for (const a of R[ri]) for (const b of R[ri + 1]) {
      const A = pos[a], B = pos[b];
      const x1 = A.x + NODE.w / 2, y1 = A.y + NODE.h, x2 = B.x + NODE.w / 2, y2 = B.y - 4;
      const my = (y1 + y2) / 2;
      edges.push({ from: a, to: b, d: `M${x1} ${y1} C ${x1} ${my}, ${x2} ${my}, ${x2} ${y2}` });
    }
  }
  const badges = R.slice(0, -1).flat().map((k) => ({
    key: k, x: pos[k].x + NODE.w / 2, y: pos[k].y + NODE.h + 16, exit: byKey[k]?.exit || "manual",
  }));

  const right = PAD + inner;
  const fails = [];
  for (const s of stages) {
    const t = s.failTo;
    if (!t || !(t in pos) || !(s.key in pos) || index[t] >= index[s.key]) continue;
    const lane = right + 36 + fails.length * LANE_STEP;
    const A = pos[s.key], B = pos[t];
    const x1 = A.x + NODE.w, y1 = A.y + NODE.h / 2, x2 = B.x + NODE.w + 4, y2 = B.y + NODE.h / 2;
    const d = `M${x1} ${y1} C ${lane} ${y1}, ${lane} ${y2}, ${x2} ${y2}`;
    fails.push({ from: s.key, to: t, d, lane, label: { x: lane - 8, y: (y1 + y2) / 2 } });
  }

  const groups = R.filter((r) => r.length > 1).map((r) => {
    const first = pos[r[0]], last = pos[r[r.length - 1]];
    return { keys: [...r], x: first.x - 12, y: first.y - 14, w: last.x + NODE.w - first.x + 24, h: NODE.h + 28 };
  });

  const width = right + Math.max(FAIL_LANE, fails.length ? 36 + fails.length * LANE_STEP + LABEL_W : 0) + PAD;
  const height = PAD * 2 + (R.length ? (R.length - 1) * ROW_H + NODE.h : 0);
  return { pos, width, height, edges, badges, fails, groups };
}

/** Batasi zoom. */
export const clampZoom = (k) => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, k));

/** Zoom sebesar `factor` dengan titik layar (px, py) tetap di tempat. */
export function zoomAt(cam, factor, px, py) {
  const k = clampZoom(cam.k * factor);
  const r = k / cam.k;
  return { x: px - (px - cam.x) * r, y: py - (py - cam.y) * r, k };
}

/** Kamera yang menampilkan seluruh graf `size` di viewport, terpusat, maksimal 100%. */
export function fit(size, viewport, pad = 16) {
  const k = clampZoom(Math.min(1, (viewport.w - pad * 2) / size.width, (viewport.h - pad * 2) / size.height));
  return { x: (viewport.w - size.width * k) / 2, y: Math.max(pad, (viewport.h - size.height * k) / 2), k };
}

/** Geser kamera seperlunya agar node di `p` (koordinat dunia) terlihat penuh. */
export function focus(cam, p, viewport, margin = 24) {
  const left = p.x * cam.k + cam.x, top = p.y * cam.k + cam.y;
  const w = NODE.w * cam.k, h = NODE.h * cam.k;
  let { x, y } = cam;
  if (left < margin) x += margin - left;
  else if (left + w > viewport.w - margin) x -= left + w - (viewport.w - margin);
  if (top < margin) y += margin - top;
  else if (top + h > viewport.h - margin) y -= top + h - (viewport.h - margin);
  return x === cam.x && y === cam.y ? cam : { x, y, k: cam.k };
}
