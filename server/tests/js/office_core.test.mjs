// Logika murni AI Office (R5): tata letak ruangan, kursi + penempatan karakter, penghindaran
// tumpukan label, pemetaan status, LOD zoom, kamera, dan kunci mode per user.
// Dijalankan `bin/tests/js_test.sh` (node --test), tanpa browser.
import { test } from "node:test";
import assert from "node:assert/strict";
import * as C from "../../assets/static/js/office-core.mjs";

// Bentuk ruangan dari GET /w/{slug}/office/state (models::office::plan_rooms).
const ROOMS = [
  { k: "KSR", kind: "project", n: "Kasir", c: "#5b5bd6", col: 0, front: false },
  { k: "LMS", kind: "project", n: "Kampus", c: "#d6409f", col: 0, front: true },
  { k: "HRM", kind: "project", n: "HR", c: "#12a594", col: 1, front: false },
  { k: "LOUNGE", kind: "lounge", n: "", c: "#a16207", col: 1, front: true },
  { k: "MEET", kind: "meeting", n: "", c: "#0891b2", col: 2, front: false },
  { k: "PANTRY", kind: "pantry", n: "", c: "#a16207", col: 2, front: true },
];
const ch = (id, room, st, extra = {}) => ({ id, room, st, bot: true, key: id, ...extra });

/* ---------- status ---------- */
test("status: aksi manusia jadi badge, paused abu-abu, sisanya tanpa badge", () => {
  assert.equal(C.statusView("review").badge, "warn");
  assert.equal(C.statusView("review").act, true);
  assert.equal(C.statusView("decision").badge, "danger");
  assert.equal(C.statusView("decision").icon, "alert-triangle");
  assert.equal(C.statusView("paused").badge, "muted");
  assert.equal(C.statusView("working").badge, null);
  assert.equal(C.statusView("idle").halo, false);
  assert.equal(C.statusView("online").halo, true);
  // Status tak dikenal tidak membuat kode gambar gagal.
  assert.equal(C.statusView("???").badge, null);
  assert.equal(C.statusView("???").icon, C.statusView("idle").icon);
  // Semua status server punya ikon (dimuat lewat <template data-office-icons>).
  for (const st of C.STATUSES) assert.ok(C.statusView(st).icon, st);
  assert.deepEqual([...new Set(C.STATUSES.map((s) => C.statusView(s).icon))].sort(), C.STATUS_ICONS.slice().sort());
});

test("status: prioritas tampil saat ruangan penuh mengikuti urgensi", () => {
  const order = ["online", "idle", "working", "decision", "paused", "review"].sort((a, b) => C.priority(a) - C.priority(b));
  assert.deepEqual(order.slice(0, 3), ["decision", "review", "working"]);
});

/* ---------- tata letak ---------- */
test("layout: kolom server jadi grid, depan = baris kedua, tanpa tumpang tindih", () => {
  const L = C.layout(ROOMS);
  assert.equal(L.cols, 3);
  assert.equal(L.rows, 2);
  const by = Object.fromEntries(L.rooms.map((r) => [r.k, r]));
  assert.equal(by.KSR.c0, 0); assert.equal(by.KSR.r0, 0);
  assert.equal(by.LMS.c0, 0); assert.equal(by.LMS.r0, 1);
  assert.equal(by.PANTRY.c0, 2); assert.equal(by.PANTRY.r0, 1);
  assert.equal(by.KSR.x1 - by.KSR.x0, C.RW);
  assert.equal(by.KSR.y1 - by.KSR.y0, C.RD);
  assert.ok(by.LMS.y0 >= by.KSR.y1, "baris depan di bawah baris belakang");
  assert.ok(by.HRM.x0 >= by.KSR.x1, "kolom berikutnya di kanan");
  for (const a of L.rooms) for (const b of L.rooms) {
    if (a === b) continue;
    const overlap = a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1;
    assert.ok(!overlap, `${a.k} vs ${b.k}`);
  }
  assert.equal(L.W, by.PANTRY.x1);
  assert.equal(L.H, by.PANTRY.y1);
  // Urutan gambar isometrik: belakang-kiri dulu.
  const order = L.rooms.map((r) => r.c0 + r.r0);
  assert.deepEqual(order, order.slice().sort((a, b) => a - b));
});

test("layout: tanpa ruangan tetap valid, dan tanda tangan berubah bila ruangan berubah", () => {
  const E = C.layout([]);
  assert.equal(E.rooms.length, 0);
  assert.ok(E.W >= 0 && E.H >= 0);
  const a = C.layout(ROOMS).sig, b = C.layout(ROOMS.map((r) => (r.k === "KSR" ? { ...r, n: "Kasir Online" } : r))).sig;
  assert.notEqual(a, b);
  assert.equal(a, C.layout(ROOMS.slice()).sig);
});

/* ---------- kursi + penempatan ---------- */
test("kursi: tiap jenis ruangan punya kursi di dalam ruangan, id unik", () => {
  for (const kind of ["project", "meeting", "pantry", "lounge"]) {
    const seats = C.seatsFor({ k: "R", kind });
    assert.ok(seats.length >= 6, kind);
    assert.equal(new Set(seats.map((s) => s.id)).size, seats.length, kind);
    for (const s of seats) {
      assert.ok(s.x > 0 && s.x < C.RW && s.y > 0 && s.y < C.RD, `${kind} ${s.id}`);
      assert.ok(C.POSES.includes(s.pose), s.pose);
    }
  }
  assert.equal(C.seatsFor({ k: "R", kind: "project" }).filter((s) => s.pose === "desk").length, 4);
});

test("penempatan: agent bekerja di meja, manusia rapat di kursi, bot rapat berdiri", () => {
  const L = C.layout(ROOMS);
  const chars = [
    ch("j1", "KSR", "working"),
    ch("u1", "MEET", "meeting", { bot: false }),
    ch("j2", "MEET", "meeting"),
    ch("p1", "PANTRY", "idle"),
  ];
  const P = C.place(chars, L);
  assert.equal(P.seats.j1.pose, "desk");
  assert.equal(P.seats.j1.room, "KSR");
  assert.equal(P.seats.u1.pose, "chair");
  assert.equal(P.seats.j2.pose, "talk");
  assert.ok(P.seats.p1.room === "PANTRY");
  assert.deepEqual(P.hidden, []);
  // Koordinat dunia = koordinat ruangan + kursi.
  const ksr = L.rooms.find((r) => r.k === "KSR");
  assert.ok(P.seats.j1.x > ksr.x0 && P.seats.j1.x < ksr.x1);
});

test("penempatan: stabil antar polling, tidak ada dua karakter di satu kursi", () => {
  const L = C.layout(ROOMS);
  const a = [ch("j1", "KSR", "working"), ch("j2", "KSR", "working"), ch("j3", "KSR", "review")];
  const P1 = C.place(a, L);
  // j1 selesai; j4 baru datang: j2/j3 tetap di kursinya.
  const b = [ch("j2", "KSR", "working"), ch("j3", "KSR", "review"), ch("j4", "KSR", "working")];
  const P2 = C.place(b, L, P1);
  assert.equal(P2.seats.j2.id, P1.seats.j2.id);
  assert.equal(P2.seats.j3.id, P1.seats.j3.id);
  const ids = Object.values(P2.seats).map((s) => s.id);
  assert.equal(new Set(ids).size, ids.length);
});

test("penempatan: ruangan penuh menyembunyikan yang paling tidak mendesak", () => {
  const L = C.layout(ROOMS);
  const cap = C.seatsFor({ k: "KSR", kind: "project" }).length;
  const many = Array.from({ length: cap + 2 }, (_, i) => ch(`o${i}`, "KSR", "online", { bot: false }));
  many.push(ch("d1", "KSR", "decision"));
  const P = C.place(many, L);
  assert.ok(P.seats.d1, "yang butuh keputusan selalu tampil");
  assert.equal(P.hidden.length, 3);
  assert.equal(P.overflow.KSR, 3);
  // Karakter di ruangan yang tidak ada di denah disembunyikan.
  const Q = C.place([ch("x", "NOPE", "working")], L);
  assert.deepEqual(Q.hidden, ["x"]);
});

test("isi ruangan: tanda tangan berubah hanya bila penghuni/status/tampilan berubah", () => {
  const L = C.layout(ROOMS);
  const a = [ch("j1", "KSR", "working", { look: { shirt: "#111111" } }), ch("p1", "PANTRY", "idle")];
  const P = C.place(a, L);
  const s1 = C.roomSigs(a, P);
  const s2 = C.roomSigs(a.map((c) => ({ ...c, doing: "langkah lain" })), P);
  assert.equal(s1.KSR, s2.KSR, "teks 'sedang apa' tidak mengubah gambar");
  const s3 = C.roomSigs(a.map((c) => (c.id === "j1" ? { ...c, st: "review" } : c)), P);
  assert.notEqual(s1.KSR, s3.KSR);
  assert.equal(s1.PANTRY, s3.PANTRY, "ruangan lain tidak digambar ulang");
});

/* ---------- label ---------- */
const overlaps = (a, b, gap = 0) => a.l < b.l + b.w + gap && b.l < a.l + a.w + gap && a.t < b.t + b.h + gap && b.t < a.t + a.h + gap;

test("label: yang bertumpuk didorong ke atas tanpa saling menutupi", () => {
  const items = [
    { id: "a", x: 100, y: 200, w: 60, h: 20, mode: "tag" },
    { id: "b", x: 110, y: 205, w: 60, h: 20, mode: "tag" },
    { id: "c", x: 105, y: 198, w: 60, h: 20, mode: "tag" },
    { id: "far", x: 600, y: 200, w: 60, h: 20, mode: "tag" },
  ];
  const out = C.declutter(items);
  const vis = out.filter((o) => !o.hidden);
  assert.equal(vis.length, 4);
  for (const a of vis) for (const b of vis) if (a !== b) assert.ok(!overlaps(a, b), `${a.id} vs ${b.id}`);
  const by = Object.fromEntries(out.map((o) => [o.id, o]));
  // Label paling bawah (b) tetap di jangkarnya; yang lain naik.
  assert.equal(by.b.t, 205 - 20);
  assert.ok(by.a.t < by.b.t && by.c.t < by.b.t);
  // Label tanpa tabrakan tidak bergeser; jangkar tag = tengah bawah.
  assert.equal(by.far.l, 600 - 30);
  assert.equal(by.far.t, 180);
});

test("label: ruangan bawah didorong ke bawah; tabrakan HUD atau keluar layar disembunyikan", () => {
  const items = [
    { id: "r1", x: 200, y: 300, w: 80, h: 30, mode: "below" },
    { id: "r2", x: 210, y: 300, w: 80, h: 30, mode: "below" },
    { id: "hud", x: 30, y: 30, w: 80, h: 30, mode: "above", hideOnHit: true },
    { id: "off", x: -500, y: 100, w: 80, h: 30, mode: "above", hideOnHit: true },
  ];
  const out = C.declutter(items, { obstacles: [{ l: 0, t: 0, w: 200, h: 40 }], view: { w: 800, h: 600 } });
  const by = Object.fromEntries(out.map((o) => [o.id, o]));
  assert.ok(by.r2.t > by.r1.t || by.r1.t > by.r2.t);
  assert.ok(!overlaps(by.r1, by.r2));
  assert.ok(Math.min(by.r1.t, by.r2.t) >= 300, "ruangan 'below' tidak naik ke atas jangkar");
  assert.equal(by.hud.hidden, true);
  assert.equal(by.off.hidden, true);
  // Jangkar cadangan dipakai bila posisi utama tertutup HUD.
  const alt = C.declutter([{ id: "r", x: 30, y: 30, w: 80, h: 30, mode: "above", hideOnHit: true, alt: { x: 300, y: 300, mode: "below" } }], { obstacles: [{ l: 0, t: 0, w: 200, h: 40 }], view: { w: 800, h: 600 } });
  assert.deepEqual([alt[0].hidden, alt[0].l, alt[0].t], [false, 260, 308]);
});

test("label: prioritas lebih tinggi ditempatkan dulu; label kalah disembunyikan setelah batas geser", () => {
  // Tumpukan rapat: hanya 2 label muat di bawah plafon (maxShift) → sisanya disembunyikan,
  // tetapi yang butuh tindakan (prio tinggi) selalu tampil.
  const items = Array.from({ length: 6 }, (_, i) => ({ id: `n${i}`, x: 100, y: 200, w: 60, h: 20, mode: "tag", prio: 0 }));
  items.push({ id: "act", x: 100, y: 200, w: 60, h: 20, mode: "tag", prio: 2 });
  const out = C.declutter(items, { maxShift: 50 });
  const by = Object.fromEntries(out.map((o) => [o.id, o]));
  assert.equal(by.act.hidden, false);
  assert.equal(by.act.t, 180, "prio tinggi tetap di jangkarnya");
  assert.ok(out.filter((o) => o.hidden).length >= 3);
  const vis = out.filter((o) => !o.hidden);
  for (const a of vis) for (const b of vis) if (a !== b) assert.ok(!overlaps(a, b));
  // Batas geser per item menimpa opsi umum.
  const two = C.declutter([
    { id: "a", x: 0, y: 100, w: 40, h: 20, mode: "tag", prio: 1 },
    { id: "b", x: 0, y: 100, w: 40, h: 20, mode: "tag", maxShift: 5 },
  ]);
  assert.equal(two[1].hidden, true);
});

test("label: HUD dihindari dengan geser ke samping; terpaksa menimpa HUD lebih baik daripada hilang", () => {
  const hud = { l: 0, t: 140, w: 400, h: 30 };
  const out = C.declutter([
    { id: "a", x: 100, y: 200, w: 60, h: 20, mode: "tag", prio: 2 },
    { id: "b", x: 100, y: 200, w: 60, h: 20, mode: "tag", prio: 1 },
  ], { obstacles: [hud] });
  const [a, b] = out;
  assert.equal(b.hidden, false);
  assert.equal(b.t, a.t, "geser ke samping pada baris yang sama");
  assert.ok(!overlaps(a, b) && !overlaps(b, hud));
  // Tidak ada tempat bebas sama sekali: label biasa tetap tampil (menimpa HUD), label ruangan hilang.
  const wall = { l: -1000, t: -1000, w: 3000, h: 3000 };
  const forced = C.declutter([{ id: "t", x: 100, y: 200, w: 60, h: 20, mode: "tag" }, { id: "r", x: 300, y: 300, w: 60, h: 20, mode: "below", hideOnHit: true }], { obstacles: [wall] });
  assert.deepEqual(forced.map((o) => o.hidden), [false, true]);
});

test("LOD: zoom jauh hanya badge ikon, dekat semua label", () => {
  assert.equal(C.lodOf(0.3), "far");
  assert.equal(C.lodOf(0.7), "mid");
  assert.equal(C.lodOf(1.2), "near");
  assert.equal(C.lodOf(0.38, "plan"), "far");
  assert.equal(C.lodOf(0.38), "mid");
  assert.equal(C.tagShows("far", "working", false), "none");
  assert.equal(C.tagShows("far", "review", false), "icon");
  assert.equal(C.tagShows("far", "working", true), "full", "yang dipilih tetap berlabel");
  assert.equal(C.tagShows("mid", "idle", false), "none", "pantry berdesakan: nama muncul saat diperbesar");
  assert.equal(C.tagShows("mid", "working", false), "full");
  assert.equal(C.tagShows("near", "idle", false), "full");
});

/* ---------- ringkasan ruangan ---------- */
test("subjudul ruangan mengikuti jenis dan isi", () => {
  const r = (kind) => ({ k: "R", kind });
  assert.deepEqual(C.roomSub(r("pantry"), []), { k: "sub-quiet" });
  assert.deepEqual(C.roomSub(r("pantry"), [ch("a", "R", "idle"), ch("b", "R", "idle")]), { k: "sub-idle", n: 2 });
  assert.deepEqual(C.roomSub(r("meeting"), [ch("a", "R", "meeting")]), { k: "sub-meeting" });
  assert.deepEqual(C.roomSub(r("project"), []), { k: "sub-empty" });
  assert.deepEqual(C.roomSub(r("project"), [ch("a", "R", "working"), ch("b", "R", "review"), ch("u", "R", "online", { bot: false })]), { k: "sub-agents", n: 2, todo: 1 });
  assert.deepEqual(C.roomSub(r("lounge"), []), { k: "sub-quiet" });
});

/* ---------- kamera ---------- */
test("kamera: pas layar memusatkan kotak dan menghormati batas zoom", () => {
  const v = C.fitView({ x0: 0, y0: 0, x1: 1000, y1: 500 }, 500, 400, { pad: 1, top: 0, bottom: 0 });
  assert.equal(v.z, 0.5);
  assert.equal(v.tx, 0);
  assert.equal(v.ty, (400 - 250) / 2);
  const tiny = C.fitView({ x0: 0, y0: 0, x1: 10, y1: 10 }, 500, 400, { max: 2 });
  assert.equal(tiny.z, 2);
});

test("kamera: zoom di titik layar menjaga titik dunia di bawah kursor", () => {
  const cam = { z: 1, tx: 10, ty: 20 };
  const n = C.zoomAround(cam, 2, 110, 120, 0.2, 3);
  // Titik dunia (100,100) tetap di layar (110,120).
  assert.equal(100 * n.z + n.tx, 110);
  assert.equal(100 * n.z + n.ty, 120);
  assert.equal(C.zoomAround(cam, 100, 0, 0, 0.2, 3).z, 3);
});

/* ---------- mode per user ---------- */
test("mode: default isometrik, disimpan per user, storage rusak tidak memecahkan halaman", () => {
  const mem = new Map();
  const store = { getItem: (k) => mem.get(k) ?? null, setItem: (k, v) => mem.set(k, v) };
  assert.equal(C.readMode(store, "u1"), "iso");
  C.writeMode(store, "u1", "plan");
  assert.equal(C.readMode(store, "u1"), "plan");
  assert.equal(C.readMode(store, "u2"), "iso", "user lain di browser yang sama tidak terpengaruh");
  assert.equal(C.modeKey("u1"), "ag-office-view:u1");
  mem.set(C.modeKey("u1"), "<script>");
  assert.equal(C.readMode(store, "u1"), "iso");
  const broken = { getItem() { throw new Error("blocked"); }, setItem() { throw new Error("blocked"); } };
  assert.equal(C.readMode(broken, "u1"), "iso");
  assert.doesNotThrow(() => C.writeMode(broken, "u1", "plan"));
  assert.equal(C.readMode(null, "u1"), "iso");
});
