// Logika murni kanvas builder workflow (R3): baris (stage paralel bergabung dengan baris
// sebelumnya), posisi node, sisi maju, sisi gagal (`gagal_ke`), kotak kelompok paralel, ukuran
// kanvas, dan kamera (zoom di titik, batas zoom, pas ke layar, fokus ke node).
// Dijalankan bin/tests/js_test.sh (node --test), tanpa browser.
import { test } from "node:test";
import assert from "node:assert/strict";
import * as W from "../../assets/static/js/workflow-core.mjs";

// Bentuk node dari atribut data-* builder (Superpowers ringkas).
const STAGES = [
  { key: "backlog", exit: "manual" },
  { key: "prd", exit: "approval" },
  { key: "design", exit: "approval" },
  { key: "arch", exit: "approval", parallel: true },
  { key: "dev", exit: "auto" },
  { key: "test", exit: "approval", failTo: "dev" },
  { key: "done", exit: "manual" },
];

test("baris: stage paralel bergabung dengan baris sebelumnya, stage pertama tidak pernah paralel", () => {
  assert.deepEqual(W.rows(STAGES), [["backlog"], ["prd"], ["design", "arch"], ["dev"], ["test"], ["done"]]);
  assert.deepEqual(W.rows([{ key: "a", parallel: true }, { key: "b" }]), [["a"], ["b"]]);
  assert.deepEqual(W.rows([]), []);
});

test("posisi: baris berurutan ke bawah, node satu baris berdampingan dan terpusat", () => {
  const L = W.layout(STAGES);
  const { w, h } = W.NODE;
  assert.equal(L.pos.prd.y - L.pos.backlog.y, W.ROW_H);
  assert.equal(L.pos.design.y, L.pos.arch.y);
  assert.equal(L.pos.arch.x - L.pos.design.x, w + W.GAP_X);
  // Pusat baris tunggal = pusat pasangan paralel.
  const mid = (L.pos.design.x + L.pos.arch.x + w) / 2;
  assert.equal(L.pos.prd.x + w / 2, mid);
  // Kanvas memuat semua node + lajur panah gagal di kanan.
  for (const p of Object.values(L.pos)) {
    assert.ok(p.x >= 0 && p.x + w <= L.width && p.y + h <= L.height);
  }
  assert.ok(L.width >= L.pos.arch.x + w + W.FAIL_LANE);
});

test("sisi maju: setiap node ke setiap node baris berikutnya, lencana transisi per node sumber", () => {
  const L = W.layout(STAGES);
  const pairs = L.edges.map((e) => `${e.from}>${e.to}`);
  assert.deepEqual(pairs, ["backlog>prd", "prd>design", "prd>arch", "design>dev", "arch>dev", "dev>test", "test>done"]);
  assert.ok(L.edges.every((e) => e.d.startsWith("M")));
  // Lencana di bawah tiap node kecuali baris terakhir, membawa jenis transisi.
  assert.equal(L.badges.length, STAGES.length - 1);
  assert.equal(L.badges.find((b) => b.key === "dev").exit, "auto");
});

test("sisi gagal: dari kanan stage ke stage sebelumnya, di lajur kanan yang tidak menabrak node", () => {
  const L = W.layout(STAGES);
  assert.equal(L.fails.length, 1);
  const f = L.fails[0];
  assert.equal(f.from, "test");
  assert.equal(f.to, "dev");
  const right = Math.max(...Object.values(L.pos).map((p) => p.x + W.NODE.w));
  assert.ok(f.lane > right, "lajur di kanan semua node");
  assert.ok(f.label.y > L.pos.dev.y && f.label.y < L.pos.test.y + W.NODE.h);
  // gagal_ke yang tidak ada / ke depan diabaikan.
  const odd = W.layout([{ key: "a", failTo: "zz" }, { key: "b", failTo: "c" }, { key: "c" }]);
  assert.equal(odd.fails.length, 0);
  // Beberapa panah gagal memakai lajur berbeda.
  const two = W.layout([{ key: "a" }, { key: "b", failTo: "a" }, { key: "c", failTo: "a" }]);
  assert.notEqual(two.fails[0].lane, two.fails[1].lane);
});

test("kelompok paralel: kotak membungkus semua node satu baris", () => {
  const L = W.layout(STAGES);
  assert.equal(L.groups.length, 1);
  const g = L.groups[0];
  assert.deepEqual(g.keys, ["design", "arch"]);
  assert.ok(g.x < L.pos.design.x && g.x + g.w > L.pos.arch.x + W.NODE.w);
  assert.ok(g.y < L.pos.design.y && g.y + g.h > L.pos.design.y + W.NODE.h);
});

test("kamera: zoom dibatasi dan titik di bawah kursor tetap", () => {
  assert.equal(W.clampZoom(10), W.ZOOM_MAX);
  assert.equal(W.clampZoom(0.01), W.ZOOM_MIN);
  const cam = { x: 100, y: 50, k: 1 };
  const z = W.zoomAt(cam, 2, 300, 250);
  // Titik dunia di bawah (300,250) sebelum dan sesudah sama.
  const before = [(300 - cam.x) / cam.k, (250 - cam.y) / cam.k];
  const after = [(300 - z.x) / z.k, (250 - z.y) / z.k];
  assert.deepEqual(after, before);
  assert.equal(W.zoomAt({ x: 0, y: 0, k: W.ZOOM_MAX }, 2, 10, 10).k, W.ZOOM_MAX);
});

test("kamera: pas ke layar memusatkan graf tanpa memperbesar di atas 100%", () => {
  const c = W.fit({ width: 500, height: 900 }, { w: 1000, h: 600 });
  assert.ok(c.k < 1);
  assert.ok(Math.abs(c.x + (500 * c.k) / 2 - 500) < 1, "terpusat horizontal");
  const small = W.fit({ width: 200, height: 100 }, { w: 1000, h: 600 });
  assert.equal(small.k, 1);
});

test("kamera: fokus ke node yang di luar layar, diam bila sudah terlihat", () => {
  const cam = { x: 0, y: 0, k: 1 };
  const inView = W.focus(cam, { x: 100, y: 100 }, { w: 800, h: 600 });
  assert.deepEqual(inView, cam);
  const moved = W.focus(cam, { x: 100, y: 900 }, { w: 800, h: 600 });
  assert.ok(moved.y < 0);
  assert.ok(900 * moved.k + moved.y + W.NODE.h <= 600);
});
