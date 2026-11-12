/* M2 Tiket & board · M3 Workflow & gate. v2: filterBar, datatable, editor, sub-rute detail, editor artifact. v3: AG.card, savebar di footer kartu, tab & aksi di bar shell. */
(() => {
const { ic, av, btn, inp, sel, fld, pill, sw, srow, section, head, empty, card: xcard, seeAll, stageIcon, prioIcon, typeIcon, statePill, labelPill, tref, esc } = AG;

/* ====================== State modul ====================== */
const S = {
  f: new Set(), swim: "none", collapsed: new Set(), hideDone: false, hideAgent: false, hideMeta: false,
  sel: new Set(), ibTab: "all", actF: {}, actOld: {}, reply: null, editC: null, draft: {},
  art: {}, artV: {}, ae: {}, sub: {}, wfSel: "design", wfMode: "builder", tpl: "{PROJECT}-{NUM}", counter: "project",
};
const sIdx = (k) => AG.STAGES.findIndex((s) => s.k === k);
const P = (k) => AG.PEOPLE[k];
const WIP = { dev: 4, review: 3, test: 3 };
const wipOf = (s) => s.wip || WIP[s.k];
const si = (k, cls) => `<span class="x-si">${stageIcon(k, cls)}</span>`;
const SUB_FLOW = ["dev", "review", "done"];
const flowOf = (t) => (t.type === "subtask" ? AG.STAGES.filter((s) => SUB_FLOW.includes(s.k)) : AG.STAGES);
/* Bar simpan yang memicu aksi (bukan hanya toast). */
const sbar = (label, act, o = {}) => AG.savebar(label, { ...o, toast: "§" }).replace('data-toast="§"', `data-act="${act}"${o.arg ? ` data-arg="${esc(o.arg)}"` : ""}`);
const comboVal = (scope, q) => { const b = scope.querySelector(`${q} [data-combo]`) || scope.querySelector(`${q}[data-combo]`); return b ? AG._combos[b.dataset.combo].value : null; };
const PEOPLE_OPTS = (extra = []) => [...extra, ...Object.keys(AG.PEOPLE).map((k) => ({ v: k, l: P(k).name, sub: `${P(k).job || P(k).role} · ${P(k).email}`, lead: av(k, 20) }))];

/* Tiket lama (sprint 1–3) khusus modul ini, supaya daftar punya beberapa halaman. */
const EXTRA = [
  ["KSR-1", "story", "Login pemilik toko dengan email", "AA", ["Auth"], 3, "Sprint 1", 2],
  ["KSR-2", "task", "Struktur database produk dan stok", "AA", ["Stok"], 5, "Sprint 1", 2],
  ["KSR-3", "story", "Tambah barang ke keranjang lewat pencarian", "DD", ["Kasir"], 3, "Sprint 1", 2],
  ["KSR-4", "story", "Hitung total belanja dan kembalian", "DD", ["Kasir"], 3, "Sprint 1", 1],
  ["KSR-6", "bug", "Kembalian minus saat pelanggan bayar pas", "QQ", ["Kasir"], 1, "Sprint 1", 1],
  ["KSR-7", "story", "Cetak struk ke printer thermal", "DD", ["Struk"], 5, "Sprint 2", 2],
  ["KSR-8", "task", "Template struk dengan logo toko", "OO", ["Struk"], 2, "Sprint 2", 3],
  ["KSR-10", "story", "Daftar produk dengan kategori", "DD", ["Stok"], 3, "Sprint 2", 3],
  ["KSR-11", "bug", "Pencarian produk lambat di 5.000 SKU", "AA", ["Stok"], 2, "Sprint 2", 2],
  ["KSR-13", "story", "Stok berkurang otomatis setelah transaksi", "DD", ["Stok"], 5, "Sprint 2", 2],
  ["KSR-14", "task", "Impor produk dari Excel", "AA", ["Stok"], 3, "Sprint 3", 3],
  ["KSR-16", "story", "Laporan penjualan per hari", "OO", ["Laporan"], 5, "Sprint 3", 3],
  ["KSR-17", "bug", "Tanggal di laporan memakai zona UTC", "QQ", ["Laporan"], 1, "Sprint 3", 2],
  ["KSR-18", "story", "Riwayat transaksi per kasir", "DD", ["Laporan"], 3, "Sprint 3", 3],
].map(([k, type, t, who, lbl, pts, sprint, p]) => ({ k, type, t, who, lbl, pts, sprint, p, stage: "done", state: "siap" }));
const ALL = () => [...AG.TICKETS, ...EXTRA];
const T = (k) => AG.TICKETS.find((x) => x.k === k) || EXTRA.find((x) => x.k === k);
const num = (k) => +String(k).split("-")[1];

/* Relasi antar tiket. blocks ditegakkan. */
const REL = {
  "KSR-24": [["blocks", "KSR-36"], ["relates", "KSR-26"]],
  "KSR-36": [["blockedby", "KSR-24"]],
  "KSR-26": [["relates", "KSR-24"]],
  "KSR-30": [["relates", "KSR-29"], ["duplicates", "KSR-21"]],
  "KSR-41": [["relates", "KSR-26"]],
};
const REL_N = { blocks: "Memblokir", blockedby: "Diblokir oleh", relates: "Terkait dengan", duplicates: "Duplikat dari" };
const REL_I = { blocks: "lock", blockedby: "lock", relates: "link", duplicates: "copy" };
const blockers = (t) => (REL[t.k] || []).filter((r) => r[0] === "blockedby" && T(r[1]) && T(r[1]).stage !== "done").map((r) => r[1]);

/* Gate per tiket (override dari evidence nyata). */
const GATE_X = {
  "KSR-24": { "Review AI selesai": [true, "3 catatan, 2 sudah diperbaiki"], "CI hijau": [true, "4 dari 4 cek lulus di PR #142"] },
  "KSR-26": { "PR terbuka": [false, "Belum ada PR di kasir-api"], "Test lulus": [false, "Agent sedang menulis tes"] },
  "KSR-30": { "Hasil test lulus": [false, "Gagal 3 kali berturut-turut"], "Screenshot ada": [true, "3 screenshot dari PC Qori, ditandatangani"] },
  "KSR-29": { "Hasil test lulus": [false, "12 dari 18 skenario selesai"], "Screenshot ada": [true, "12 screenshot ditandatangani"] },
};
function gates(t) {
  const st = AG.stage(t.stage); const done = t.state === "siap" || t.state === "review";
  return (st.gate || []).map((n) => { const x = (GATE_X[t.k] || {})[n]; return { n, ok: x ? x[0] : done, d: x ? x[1] : done ? "Terpenuhi" : "Belum terpenuhi" }; });
}
function check(t, to) {
  const flow = flowOf(t).map((s) => s.k);
  const i = flow.indexOf(t.stage), j = flow.indexOf(to);
  if (j < 0) return { ok: false, why: `Stage ${AG.stage(to).n} tidak ada di workflow ${t.type === "subtask" ? "subtask" : "tiket ini"}` };
  if (i === j) return { same: true };
  if (j < i) return { ok: true, back: true };
  const bl = blockers(t);
  if (bl.length && t.stage === "backlog") return { ok: false, why: `Diblokir oleh ${bl[0]} yang belum selesai` };
  if (j > i + 1) return { ok: false, skip: true };
  if (t.state === "siap" || t.state === "review") return { ok: true };
  if (t.state === "keputusan") return { ok: false, why: "Butuh keputusan manusia lebih dulu" };
  const g = gates(t).find((x) => !x.ok);
  return { ok: false, why: `Gate belum terpenuhi: ${g ? g.n : "pekerjaan stage belum selesai"}` };
}
function applyMove(t, to) {
  const st = AG.stage(to);
  t.stage = to;
  t.state = to === "done" || to === "backlog" ? "siap" : "antre";
  t.agent = to === "done" || to === "backlog" ? undefined : st.persona ? `Job ${AG.PERSONAS[st.persona].n} menunggu runner` : undefined;
}
function flashDeny(to, why) {
  document.querySelectorAll(`.bd-cell[data-col="${to}"]`).forEach((c) => {
    c.classList.add("deny");
    const n = document.createElement("div"); n.className = "bd-why"; n.innerHTML = `${ic("lock", "s14")}<span>${esc(why)}</span>`;
    (c.querySelector(".bd-cards") || c).prepend(n);
    setTimeout(() => { c.classList.remove("deny"); n.remove(); }, 2200);
  });
}
function doMove(k, to) {
  const t = T(k); if (!t) return;
  const r = check(t, to);
  if (r.same) return;
  if (r.skip) return AG.openDialog("skip-stage", `${k}>${to}`);
  if (!r.ok) { AG.toast(r.why, "dng"); flashDeny(to, r.why); return; }
  applyMove(t, to); log(k, "route", `<b>${P(AG.ME).name}</b> memindahkan tiket ke <b>${AG.stage(to).n}</b>`);
  rr();
  const s = AG.stage(to), w = wipOf(s), n = AG.TICKETS.filter((x) => x.stage === to).length;
  if (w && n > w) AG.toast(`WIP limit ${s.n} terlampaui (${n}/${w})`, "dng");
  else AG.toast(r.back ? `${k} dikembalikan ke ${s.n}` : `${k} pindah ke ${s.n}${s.persona ? ` · job ${AG.PERSONAS[s.persona].n} dipicu` : ""}`);
}
function approve(k) {
  const t = T(k); const flow = flowOf(t); const i = flow.findIndex((s) => s.k === t.stage); const nx = flow[i + 1];
  if (!nx) return AG.toast("Tiket sudah di stage terakhir", "dng");
  applyMove(t, nx.k); log(k, "check", `<b>${P(AG.ME).name}</b> approve stage ${AG.stage(flow[i].k).n}`);
  rr(); AG.toast(`${k} disetujui, pindah ke ${nx.n}${nx.persona ? ` · job ${AG.PERSONAS[nx.persona].n} dipicu` : ""}`);
}

/* Sampah 30 hari. */
const TRASH = [
  { k: "KSR-34", type: "bug", t: "Tombol cetak struk muncul dua kali", by: "AA", at: "10 Nov 2026", left: 28, lbl: ["Struk"], p: 3 },
  { k: "KSR-46", type: "task", t: "Coba ulang warna tombol bayar", by: "OO", at: "9 Nov 2026", left: 27, lbl: ["Kasir"], p: 4 },
  { k: "KSR-45", type: "story", t: "Duplikat: struk tanpa nama kasir", by: "QQ", at: "8 Nov 2026", left: 26, lbl: ["Struk"], p: 3 },
  { k: "KSR-44", type: "task", t: "Catatan rapat vendor printer", by: "AA", at: "6 Nov 2026", left: 24, lbl: ["Struk"], p: 4 },
  { k: "KSR-43", type: "story", t: "Diskon ulang tahun pelanggan", by: "OO", at: "4 Nov 2026", left: 22, lbl: ["Promo"], p: 4 },
  { k: "KSR-42", type: "bug", t: "Ikon QRIS buram di layar retina", by: "DD", at: "3 Nov 2026", left: 21, lbl: ["Pembayaran"], p: 3 },
  { k: "KSR-32", type: "task", t: "Uji coba printer thermal 58 mm", by: "OO", at: "1 Nov 2026", left: 19, lbl: ["Struk"], p: 4 },
  { k: "KSR-15", type: "story", t: "Notifikasi stok habis lewat SMS", by: "AA", at: "28 Okt 2026", left: 15, lbl: ["Stok"], p: 3 },
  { k: "KSR-12", type: "task", t: "Uji beban 100 transaksi per menit", by: "AA", at: "24 Okt 2026", left: 11, lbl: ["Kasir"], p: 3 },
  { k: "KSR-9", type: "story", t: "Integrasi timbangan digital", by: "OO", at: "20 Okt 2026", left: 7, lbl: ["Kasir"], p: 4 },
  { k: "KSR-19", type: "story", t: "Tema gelap untuk layar kasir", by: "OO", at: "16 Okt 2026", left: 3, lbl: ["Kasir"], p: 4 },
  { k: "KSR-5", type: "story", t: "Mode kasir layar sentuh", by: "DD", at: "14 Okt 2026", left: 1, lbl: ["Kasir"], p: 4 },
];
const nextNum = () => Math.max(...ALL().map((t) => num(t.k)), ...TRASH.map((t) => num(t.k))) + 1;
function trashTickets(keys) {
  keys.forEach((k) => {
    let arr = AG.TICKETS, i = arr.findIndex((t) => t.k === k);
    if (i < 0) { arr = EXTRA; i = arr.findIndex((t) => t.k === k); }
    if (i < 0) return;
    const [t] = arr.splice(i, 1); TRASH.unshift({ ...t, by: AG.ME, at: "12 Nov 2026", left: 30, orig: t }); S.sel.delete(k);
  });
}

/* Riwayat & aktivitas agent. ts = menit yang lalu (untuk urutan). ag = aktivitas agent. */
const H = (i, h, at, ts, ag = false) => ({ i, h, at, ts, ag });
const HIST = {
  "KSR-24": [
    H("git-compare", "<b>Agent Code Reviewer</b> selesai mereview PR #142 dan #87 · 3 catatan", "40 mnt lalu", 40, true),
    H("route", "Gate Development terpenuhi (PR terbuka, Test lulus) · pindah ke <b>Code Review</b>", "1 jam lalu", 60),
    H("git-pull-request", "<b>Agent Developer</b> membuka PR #142 di kasir-api dan #87 di kasir-web", "1 jam lalu", 62, true),
    H("clock", "Job Development selesai dalam 4 j 10 mnt di runner <b>Laptop Dewi</b>", "1 jam lalu", 63, true),
    H("file-text", "<b>Agent Product Manager</b> menyimpan <b>prd.md v3</b> · menambah catatan teknis", "2 jam lalu", 120, true),
    H("check", "<b>Olivia Owner</b> approve <b>prd.md v2</b>", "kemarin", 1440),
    H("x", "<b>Olivia Owner</b> reject prd.md v1: “Tambahkan QR kedaluwarsa 15 menit”", "6 Nov", 8000),
    H("user", "<b>Olivia Owner</b> menugaskan tiket ke <b>Dewi Dev</b>", "4 Nov", 10990),
    H("sparkles", "AI mengusulkan 8 poin · 2 hari kerja (diterima)", "4 Nov", 11000, true),
    H("plus", "<b>Olivia Owner</b> membuat tiket", "4 Nov", 11001),
  ],
  "KSR-30": [
    H("alert-triangle", "Status menjadi <b>Butuh keputusan</b> · batas 3 putaran gagal tercapai", "18 mnt lalu", 18),
    H("test-pipe", "<b>Agent QA</b> menjalankan putaran #3 di runner <b>PC Qori</b> · 3 skenario gagal", "20 mnt lalu", 20, true),
    H("code", "<b>Agent Developer</b> memperbaiki template struk (putaran #3)", "1 jam lalu", 70, true),
    H("plus", "<b>Qori QA</b> membuat tiket dari hasil uji regresi KSR-29", "kemarin", 1500),
  ],
};
const histOf = (t) => HIST[t.k] || (HIST[t.k] = [H("route", `Tiket masuk stage <b>${AG.stage(t.stage).n}</b>`, "kemarin", 1440), H("sparkles", `AI mengusulkan ${t.pts || 3} poin`, "8 Nov", 5000, true), H("plus", "<b>Olivia Owner</b> membuat tiket", "8 Nov", 5001)]);
function log(k, i, h, ag = false) { const t = T(k); if (!t) return; histOf(t).unshift(H(i, h, "baru saja", 0, ag)); }

/* Komentar: thread, reaksi, sematan, lampiran, penanda diedit. */
let CID = 100;
const mt = (n) => `<span class="mention">${n}</span>`;
const C = (w, html, at, ts, x = {}) => ({ id: "c" + ++CID, w, html, at, ts, edited: false, pinned: false, deleted: false, reacts: {}, replies: [], files: [], ...x });
const COMMENTS = {
  "KSR-24": [
    C("pm", `PRD v3 menambahkan catatan teknis HMAC dan idempotency dari temuan review. Mohon dicek ${mt("@Olivia Owner")}.`, "2 jam", 118),
    C("reviewer", "Review AI selesai untuk PR #142 dan #87. Ada <b>3 catatan</b>: verifikasi signature callback sebelum parse body, polling status terlalu sering, dan satu nama variabel. Dua sudah diperbaiki Agent Developer.", "40 mnt", 40,
      { reacts: { "👍": ["DD", "AA"] }, replies: [C("DD", "Dua catatan sudah aku cek ulang di commit <code>a41f9c2</code>. Sisa satu soal nama variabel.", "35 mnt", 35)] }),
    C("DD", "Callback dari penyedia QRIS kadang datang dua kali. Sudah aku tangani pakai idempotency key per <code>order_id</code>.", "32 mnt", 32, { files: ["callback-ganda.log · 4 KB"], reacts: { "🎉": ["OO"] } }),
    C("OO", `${mt("@Adi Admin")} tolong cek juga timeout 15 menit untuk QR kedaluwarsa. ${mt("@agent-qa")} tambahkan skenario QR kedaluwarsa di Testing.`, "10 mnt", 10, { pinned: true, edited: true }),
  ],
  "KSR-30": [
    C("qa", "Putaran ke-3 gagal lagi: total di struk <b>Rp48.900</b>, di layar kasir <b>Rp49.000</b>. Screenshot dan langkah reproduksi ada di tab Evidence.", "18 mnt", 18, { reacts: { "👀": ["QQ", "OO"] } }),
    C("QQ", "Selisih muncul hanya kalau ada diskon per item. Sepertinya terkait KSR-21.", "12 mnt", 12),
  ],
  "KSR-26": [
    C("DD", `${mt("@Olivia Owner")} stok minus boleh? Ada toko yang jual barang sebelum barang masuk dicatat.`, "1 jam", 60),
    C("OO", "Tidak boleh untuk checkout biasa. Tambahkan izin khusus untuk supervisor saja.", "45 mnt", 45, { replies: [C("dev", "Siap. Saya tambahkan validasi stok dan izin <code>stok.override</code> untuk role supervisor.", "44 mnt", 44)] }),
  ],
};
const cmts = (t) => COMMENTS[t.k] || (COMMENTS[t.k] = [C("OO", "Mohon diprioritaskan setelah sprint ini.", "kemarin", 1400)]);
const countC = (t) => cmts(t).reduce((n, c) => n + (c.deleted ? 0 : 1) + c.replies.filter((r) => !r.deleted).length, 0);
function findC(k, id) {
  for (const c of COMMENTS[k] || []) { if (c.id === id) return { c, list: COMMENTS[k] }; const r = c.replies.find((x) => x.id === id); if (r) return { c: r, list: c.replies, parent: c }; }
  return {};
}

/* Artifact + versi + status review. s: ok disetujui · rev menunggu review · rej ditolak · draft draf · old diganti */
const V = (md, by, at, s, note = "") => ({ md, by, at, s, note });
const PRD = [
`# PRD: Integrasi pembayaran QRIS

## Tujuan
Kasir bisa menerima QRIS tanpa mesin EDC.

## Acceptance criteria
- [ ] Kasir memilih QRIS, layar menampilkan QR dinamis sesuai total
- [ ] Status pembayaran berubah otomatis saat callback diterima
- [ ] Struk mencetak metode bayar QRIS dan nomor referensi

## Di luar cakupan
- Refund QRIS`,
`# PRD: Integrasi pembayaran QRIS

## Tujuan
Kasir bisa menerima QRIS dari semua aplikasi bank dan e-wallet tanpa mesin EDC.

## Acceptance criteria
- [ ] Kasir memilih QRIS, layar menampilkan QR dinamis sesuai total
- [ ] Status pembayaran berubah otomatis saat callback diterima
- [ ] QR kedaluwarsa setelah 15 menit dan bisa dibuat ulang
- [ ] Struk mencetak metode bayar QRIS dan nomor referensi

## Di luar cakupan
- Refund QRIS`,
`# PRD: Integrasi pembayaran QRIS

## Tujuan
Kasir bisa menerima QRIS dari semua aplikasi bank dan e-wallet tanpa mesin EDC.

## Acceptance criteria
- [x] Kasir memilih QRIS, layar menampilkan QR dinamis sesuai total
- [x] Status pembayaran berubah otomatis saat callback diterima
- [ ] QR kedaluwarsa setelah 15 menit dan bisa dibuat ulang
- [x] Struk mencetak metode bayar QRIS dan nomor referensi

## Catatan teknis
- Callback diverifikasi dengan signature HMAC sebelum diproses
- Idempotency key per \`order_id\` supaya callback ganda aman

## Di luar cakupan
- Refund QRIS (tiket terpisah KSR-39)`,
];
const ARTS = {
  "KSR-24": [
    { id: "brainstorm", n: "brainstorm.md", st: "brainstorm", by: "analyst", v: [V(`# Brainstorming: QRIS di kasir

## Masalah
- Toko kecil tidak punya mesin EDC
- Pelanggan makin sering bayar pakai e-wallet

## Alternatif
- QR statis ditempel di meja kasir (murah, tapi tidak ada konfirmasi otomatis)
- **QR dinamis per transaksi** (dipilih)

## Risiko
- Callback terlambat saat jaringan toko lambat`, "analyst", "5 Nov", "ok", "Disetujui Olivia")] },
    { id: "prd", n: "prd.md", st: "prd", by: "pm", v: [V(PRD[0], "pm", "6 Nov", "rej", "Ditolak Olivia: tambahkan QR kedaluwarsa 15 menit"), V(PRD[1], "pm", "kemarin", "ok", "Disetujui Olivia"), V(PRD[2], "pm", "2 jam lalu", "rev", "Revisi dari temuan Code Review")] },
    { id: "flow", n: "alur-bayar.md", st: "design", by: "uiux", v: [V(`# Alur bayar QRIS

1. Kasir menekan **Bayar**, memilih QRIS
2. Layar menampilkan QR dan hitung mundur 15:00
3. Callback diterima, layar berubah hijau
4. Struk dicetak otomatis

> QR kedaluwarsa: tombol **Buat QR baru** muncul, keranjang tidak berubah`, "uiux", "7 Nov", "ok", "Disetujui Olivia dan Adi")] },
    { id: "mockup", n: "mockup-bayar.html", st: "design", by: "uiux", v: [V("```html\n<section class=\"bayar-qris\">\n  <h2>Bayar dengan QRIS</h2>\n  <img src=\"qr.png\" alt=\"QR\">\n  <p>Kedaluwarsa 14:59</p>\n</section>\n```", "uiux", "7 Nov", "old", "Diganti v2"), V("```html\n<section class=\"bayar-qris\">\n  <h2>Bayar dengan QRIS</h2>\n  <img src=\"qr.png\" alt=\"QR\">\n  <p class=\"timer\">Kedaluwarsa 14:59</p>\n  <button>Buat QR baru</button>\n</section>\n```", "uiux", "8 Nov", "ok", "Disetujui Olivia")] },
    { id: "spec", n: "tech-spec.md", st: "dev", by: "architect", v: [V(`# Tech spec: QRIS

## Endpoint
- \`POST /api/payments/qris\` membuat QR dinamis
- \`POST /api/payments/qris/callback\` menerima notifikasi

## Data
- Tabel \`payments\` menambah kolom \`qris_ref\``, "architect", "8 Nov", "old", "Diganti v2"), V(`# Tech spec: QRIS

## Endpoint
- \`POST /api/payments/qris\` membuat QR dinamis
- \`POST /api/payments/qris/callback\` menerima notifikasi
- \`GET /api/payments/:id/status\` untuk polling dengan backoff

## Data
- Tabel \`payments\` menambah kolom \`qris_ref\` dan \`expires_at\``, "architect", "9 Nov", "ok", "Disetujui Adi")] },
    { id: "plan", n: "rencana-implementasi.md", st: "dev", by: "dev", v: [V(`# Rencana implementasi

- [x] Tes callback dan signature (TDD)
- [x] Endpoint buat QR di kasir-api
- [x] Panel QRIS di kasir-web
- [x] Tes e2e bayar sampai struk`, "dev", "10 Nov", "ok", "Dibuat otomatis, tanpa review")] },
    { id: "review", n: "review-ai.md", st: "review", by: "reviewer", v: [V(`# Review AI · PR #142, #87

- **callback.rs:41** verifikasi signature sebelum parse body (sudah diperbaiki)
- **QrisPanel.tsx:88** polling tiap 1 detik terlalu sering, pakai backoff (sudah diperbaiki)
- **qris.rs:12** nama \`tmp_amt\` kurang jelas, ganti \`amount_idr\` (terbuka)`, "reviewer", "40 mnt lalu", "rev", "Menunggu review Adi atau Olivia")] },
    { id: "testplan", n: "rencana-test.md", st: "test", by: "qa", v: [V(`# Rencana test QRIS

- QR tampil sesuai total
- Callback ganda tidak membuat pembayaran ganda
- QR kedaluwarsa setelah 15 menit`, "qa", "10 mnt lalu", "draft", "Draf, belum dikirim untuk review")] },
  ],
  "KSR-28": [
    { id: "prd", n: "prd.md", st: "prd", by: "pm", v: [V(`# PRD: Diskon per item\n\n## Acceptance criteria\n- [ ] Kasir bisa memberi diskon persen atau nominal per baris\n- [ ] Diskon tidak melebihi batas per kasir`, "pm", "8 Nov", "ok", "Disetujui Olivia")] },
    { id: "mockup", n: "mockup-diskon.html", st: "design", by: "uiux", v: [V("```html\n<div class=\"baris\">Teh botol <input value=\"10%\"></div>\n```", "uiux", "10 Nov", "rej", "Ditolak: input diskon terlalu kecil di layar sentuh"), V("```html\n<div class=\"baris\">Teh botol <button class=\"diskon\">Diskon 10%</button></div>\n```", "uiux", "1 jam lalu", "rev", "Mockup v2 siap direview")] },
  ],
};
const ART_ST = { ok: ["Disetujui", "ok", "circle-check"], rev: ["Menunggu review", "warn", "eye"], rej: ["Ditolak", "dng", "circle-x"], draft: ["Draf", "gray", "pencil"], old: ["Diganti", "gray", "history"] };
const artPill = (s) => { const x = ART_ST[s]; return pill(x[0], x[1], "", x[2]); };
const lastV = (a) => a.v[a.v.length - 1];
function artsOf(t) {
  if (ARTS[t.k]) return ARTS[t.k];
  const st = t.stage === "prd" && t.state === "review" ? "rev" : sIdx(t.stage) > sIdx("prd") ? "ok" : "draft";
  return (ARTS[t.k] = [{ id: "prd", n: "prd.md", st: "prd", by: "pm", v: [V(`# PRD: ${t.t}\n\n## Tujuan\nMenjelaskan kebutuhan untuk ${t.t.toLowerCase()}.\n\n## Acceptance criteria\n- [ ] Perilaku utama berjalan di layar kasir\n- [ ] Ada pesan error yang jelas untuk kasir`, "pm", "kemarin", st, st === "rev" ? "Siap direview" : "")] }]);
}
/* Komentar inline di artifact. */
const ACM = {
  "KSR-24:prd": [
    { q: "QR kedaluwarsa setelah 15 menit dan bisa dibuat ulang", w: "OO", t: "Tambahkan: membuat QR baru tidak boleh mengosongkan keranjang.", at: "kemarin", done: false, re: [["uiux", "Sudah ada di alur-bayar.md langkah 4."]] },
    { q: "Idempotency key per", w: "reviewer", t: "Sesuai temuan PR #142. Simpan key minimal 24 jam.", at: "2 jam lalu", done: false, re: [] },
    { q: "Refund QRIS", w: "AA", t: "KSR-39 sudah dibuat di backlog.", at: "kemarin", done: true, re: [] },
  ],
};
const acmOf = (k, id) => ACM[`${k}:${id}`] || (ACM[`${k}:${id}`] = []);

/* Markdown sederhana untuk pratinjau (read-only). */
function md(s) {
  const out = []; let list = null, ol = null, code = null;
  const inl = (x) => esc(x).replace(/`([^`]+)`/g, "<code>$1</code>").replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>");
  const flush = () => { if (list) out.push(`<ul>${list.join("")}</ul>`); if (ol) out.push(`<ol>${ol.join("")}</ol>`); list = ol = null; };
  for (const line of s.split("\n")) {
    if (line.startsWith("```")) { if (code) { out.push(`<pre class="code" style="white-space:pre-wrap">${esc(code.join("\n"))}</pre>`); code = null; } else { flush(); code = []; } continue; }
    if (code) { code.push(line); continue; }
    const li = line.match(/^- (\[( |x)\] )?(.*)/);
    if (li) { (list ||= []).push(li[1] ? `<li class="td-task ${li[2] === "x" ? "on" : ""}">${ic(li[2] === "x" ? "circle-check" : "circle", "s14")}<span>${inl(li[3])}</span></li>` : `<li>${inl(li[3])}</li>`); continue; }
    const oi = line.match(/^\d+\. (.*)/); if (oi) { (ol ||= []).push(`<li>${inl(oi[1])}</li>`); continue; }
    flush();
    if (line.startsWith("## ")) out.push(`<h3>${inl(line.slice(3))}</h3>`);
    else if (line.startsWith("# ")) out.push(`<h2>${inl(line.slice(2))}</h2>`);
    else if (line.startsWith("> ")) out.push(`<blockquote>${inl(line.slice(2))}</blockquote>`);
    else if (line.trim()) out.push(`<p>${inl(line)}</p>`);
  }
  flush();
  return out.join("");
}
function diff(a, b) {
  a = a.split("\n"); b = b.split("\n"); const n = a.length, m = b.length;
  const L = Array.from({ length: n + 1 }, () => Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) L[i][j] = a[i] === b[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1]);
  const r = []; let i = 0, j = 0;
  while (i < n && j < m) { if (a[i] === b[j]) { r.push([" ", a[i]]); i++; j++; } else if (L[i + 1][j] >= L[i][j + 1]) r.push(["-", a[i++]]); else r.push(["+", b[j++]]); }
  while (i < n) r.push(["-", a[i++]]); while (j < m) r.push(["+", b[j++]]);
  return r;
}
const diffHtml = (a, b, label) => { const d = diff(a, b); const add = d.filter((x) => x[0] === "+").length, del = d.filter((x) => x[0] === "-").length;
  return `<div class="diff"><div class="hd row"><span class="trunc">${label}</span><span class="grow"></span><span class="text-ok">+${add}</span><span class="text-dng">−${del}</span></div>${d.map(([s, l]) => `<div class="${s === "+" ? "add" : s === "-" ? "del" : ""}"><span class="td-dm">${s === " " ? "" : s === "-" ? "−" : "+"}</span>${esc(l) || "&nbsp;"}</div>`).join("")}</div>`; };
/* Gambar screenshot tiruan (evidence). */
function shotSvg(kind) {
  const qr = []; let s = 7;
  for (let y = 0; y < 9; y++) for (let x = 0; x < 9; x++) { s = (s * 17 + 11) % 97; if (s % 3 && !(x < 3 && y < 3) && !(x > 5 && y < 3) && !(x < 3 && y > 5)) qr.push(`<rect x="${212 + x * 7}" y="${62 + y * 7}" width="6" height="6"/>`); }
  const corner = (x, y) => `<rect x="${x}" y="${y}" width="20" height="20" fill="none" stroke="currentColor" stroke-width="4"/>`;
  const fail = kind === "fail";
  return `<svg viewBox="0 0 320 200" class="td-svg" role="img" aria-label="Screenshot ${fail ? "struk dengan total selisih" : "halaman bayar QRIS"}">
    <rect width="320" height="200" fill="var(--bg-sub)"/><rect width="320" height="22" fill="var(--side)"/>
    <circle cx="12" cy="11" r="3" fill="var(--line2)"/><circle cx="22" cy="11" r="3" fill="var(--line2)"/><circle cx="32" cy="11" r="3" fill="var(--line2)"/>
    <text x="48" y="15" font-size="8" fill="var(--muted)">kasir.toko-sejahtera.id/${fail ? "struk/8812" : "bayar/qris"}</text>
    ${fail ? `<rect x="90" y="32" width="140" height="158" rx="3" fill="var(--raised)" stroke="var(--line2)"/>
      <text x="160" y="50" font-size="9" text-anchor="middle" fill="currentColor" font-weight="600">TOKO SEJAHTERA</text>
      ${[0, 1, 2, 3].map((i) => `<rect x="102" y="${64 + i * 14}" width="${70 - i * 6}" height="5" rx="2" fill="var(--line2)"/><rect x="190" y="${64 + i * 14}" width="28" height="5" rx="2" fill="var(--line2)"/>`).join("")}
      <text x="102" y="134" font-size="9" fill="currentColor">Total</text><text x="218" y="134" font-size="9" text-anchor="end" fill="currentColor" font-weight="600">Rp48.900</text>
      <rect x="96" y="123" width="128" height="16" rx="2" fill="none" stroke="var(--danger)" stroke-width="1.5"/>
      <text x="102" y="156" font-size="7.5" fill="var(--muted)">Kasir: (kosong)</text>
      <rect x="96" y="148" width="64" height="12" rx="2" fill="none" stroke="var(--danger)" stroke-width="1.5" stroke-dasharray="3 2"/>`
    : `<rect x="12" y="32" width="170" height="158" rx="4" fill="var(--raised)" stroke="var(--line)"/>
      ${[0, 1, 2, 3, 4].map((i) => `<rect x="22" y="${44 + i * 22}" width="${90 - i * 8}" height="6" rx="3" fill="var(--line2)"/><rect x="140" y="${44 + i * 22}" width="30" height="6" rx="3" fill="var(--line2)"/>`).join("")}
      <text x="22" y="174" font-size="10" fill="currentColor" font-weight="600">Total Rp49.000</text>
      <rect x="194" y="32" width="114" height="158" rx="4" fill="var(--raised)" stroke="var(--line)"/>
      <text x="251" y="50" font-size="8.5" text-anchor="middle" fill="currentColor" font-weight="600">Bayar dengan QRIS</text>
      <g fill="currentColor">${qr.join("")}</g>${corner(212, 62)}${corner(254, 62)}${corner(212, 104)}
      <text x="251" y="146" font-size="7.5" text-anchor="middle" fill="var(--muted)">Kedaluwarsa 14:59</text>
      <rect x="208" y="156" width="86" height="20" rx="4" fill="var(--accent)"/><text x="251" y="169" font-size="8" text-anchor="middle" fill="var(--on-accent)">Menunggu bayar</text>`}
  </svg>`;
}

/* ====================== CSS modul ====================== */
AG.css(`
@media (max-width: 899px) { .x-dtfit .dt-box table { table-layout: fixed; width: 100%; } .x-dtfit td .row { min-width: 0 !important; max-width: none !important; } .x-dtfit td { overflow: hidden; } }
.x-si { display: inline-flex; } .menu .x-si .ic { color: inherit; }
/* Board */
.bd-page { display: flex; flex-direction: column; height: calc(100dvh - 94px); min-height: 420px; }
@media (min-width: 900px) { html[data-shell-v="A"] .bd-page { height: calc(100dvh - 110px); } }
.bd-bar { padding: 12px 20px 2px; }
.bd-hint { font-size: 12px; color: var(--muted); display: inline-flex; gap: 4px; align-items: center; }
.bd-board { flex: 1; min-height: 0; align-items: stretch; padding: 10px 20px 16px; }
.bd-board .bcol { min-height: 0; }
.bd-cards { flex: 1; min-height: 40px; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; padding: 2px; margin: -2px; }
html[data-board-v="B"] .bcol[data-col] { border-top-color: var(--stc); }
.bd-colh { height: 30px; padding: 0 2px 0 6px; display: flex; align-items: center; gap: 8px; font-weight: 500; flex-shrink: 0; min-width: 0; }
.bd-colh .n { color: var(--faint); font-weight: 400; }
.bd-colh .wip { font-size: 11.5px; color: var(--muted); border: 1px solid var(--line2); border-radius: 5px; padding: 0 5px; line-height: 18px; font-variant-numeric: tabular-nums; font-weight: 400; }
.bd-colh .wip.full { color: var(--warn); border-color: transparent; background: var(--warn-soft); }
.bd-colh .wip.over { color: var(--danger); border-color: transparent; background: var(--danger-soft); }
.bd-add { width: 24px; height: 24px; border-radius: 5px; display: inline-flex; align-items: center; justify-content: center; color: var(--muted); flex-shrink: 0; }
.bd-add:hover { background: var(--hover); color: var(--fg); }
.bd-scroll { flex: 1; overflow: auto; padding: 0 20px 20px; min-height: 0; }
.bd-heads { display: flex; gap: 10px; position: sticky; top: 0; z-index: 3; background: var(--page); padding: 10px 0 4px; width: max-content; }
.bd-heads .bd-colh { width: 272px; }
.bd-lane { width: max-content; }
.bd-laneh { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 8px; margin-top: 10px; border-radius: 6px; position: sticky; left: 0; color: var(--fg2); }
.bd-laneh:hover { background: var(--hover); }
.bd-laneh .n { color: var(--faint); }
.bd-laneh .ic.chev { transition: transform 0.15s; color: var(--faint); }
.bd-lane.shut .bd-laneh .ic.chev { transform: rotate(-90deg); }
.bd-lane.shut .bd-cells { display: none; }
.bd-cells { display: flex; gap: 10px; padding-top: 4px; }
.bd-cells .bcol { min-height: 64px; }
.bcol.bd-cell.drop { background: var(--accent-soft); outline: 2px dashed var(--accent); outline-offset: -2px; }
.bcol.bd-cell.deny { background: var(--danger-soft); outline: 2px dashed var(--danger); outline-offset: -2px; }
.bd-why { display: flex; gap: 6px; align-items: center; font-size: 12px; font-weight: 500; color: var(--danger); background: var(--raised); border: 1px solid var(--danger-line); border-radius: 6px; padding: 6px 8px; }
.bd-mv { width: 22px; height: 22px; border-radius: 5px; display: inline-flex; align-items: center; justify-content: center; color: var(--muted); opacity: 0; }
.card:hover .bd-mv, .card:focus-within .bd-mv, .bd-mv:focus-visible { opacity: 1; }
.bd-mv:hover { background: var(--hover); color: var(--fg); }
@media (hover: none) { .bd-mv { opacity: 1; } }
.bd-empty { border: 1px dashed var(--line2); border-radius: 8px; color: var(--faint); font-size: 12px; padding: 14px; text-align: center; }
.bd-noagent .card .agentbox { display: none; } .bd-nometa .card > .row.wrap { display: none; }
@media (max-width: 899px) { .bd-page { height: calc(100dvh - 150px); } .bd-bar { padding: 10px 12px 0; } .bd-board, .bd-scroll { padding-left: 12px; padding-right: 12px; } .bd-heads .bd-colh { width: 84vw; } .bd-hint { display: none; } }

/* Kartu v3 */
.cardx .cx-h h2 .cx-n { color: var(--faint); font-weight: 400; margin-left: 6px; font-size: 13px; text-transform: none; letter-spacing: 0; }
.cardx .cx-h h2 { display: flex; align-items: center; gap: 0; flex-wrap: wrap; }
.cardx.dng { border-color: var(--danger-line); }
.cx-f > .x-foot { display: flex; align-items: center; gap: 8px; padding: 10px 16px; flex-wrap: wrap; }
.cx-f > .x-foot.end { justify-content: flex-end; }
.cx-b.flush > .tblwrap .tbl { border: 0; }
.td .cx-b, .ae .cx-b, .sp-panel .cx-b { grid-template-columns: minmax(0, 1fr); }
.td-desc .ed .foot { flex-wrap: wrap; row-gap: 6px; }
/* Kartu dengan kepala (lama) */
.x-box > .bh { display: flex; align-items: center; gap: 8px; padding: 10px 12px 10px 16px; border-bottom: 1px solid var(--line); min-height: 46px; }
.x-box > .bh h2 { font-size: 13px; font-weight: 600; } .x-box > .bh .n { color: var(--faint); }
.x-box > .bb { padding: 14px 16px; }

/* Detail tiket */
.td { max-width: 1240px; margin: 0 auto; width: 100%; display: grid; gap: 16px; }
.td-h { display: grid; gap: 10px; }
.td-title { font-size: 22px; font-weight: 600; letter-spacing: -0.02em; line-height: 1.3; outline: none; border-radius: 6px; margin: 0 -6px; padding: 2px 6px; }
.td-title:hover { background: var(--hover); } .td-title:focus { background: var(--raised); box-shadow: var(--ring); }
.td-body { display: grid; grid-template-columns: minmax(0, 1fr) 320px; grid-template-areas: "m s1" "m s2" "m ."; grid-template-rows: auto auto 1fr; gap: 16px 24px; align-items: start; }
.td-body.full { grid-template-columns: minmax(0, 1fr); grid-template-areas: "m"; grid-template-rows: auto; }
.td-m { grid-area: m; display: grid; grid-template-columns: minmax(0, 1fr); gap: 16px; min-width: 0; align-content: start; }
.td-s1 { grid-area: s1; min-width: 0; } .td-s2 { grid-area: s2; display: grid; grid-template-columns: minmax(0, 1fr); gap: 16px; min-width: 0; }
@media (max-width: 1099px) { .td-body { grid-template-columns: minmax(0, 1fr); grid-template-areas: "s1" "m" "s2"; grid-template-rows: none; } }
.td-desc .ed { border: 0; border-radius: 0 0 10px 10px; box-shadow: none; } .td-desc .ed .tb { border-radius: 0; top: 94px; }
@media (max-width: 899px) { .td-desc .ed .tb { top: 52px; } }
.td-desc:focus-within { border-color: var(--accent); box-shadow: var(--ring); }
.prose li.td-task, .x-prose li.task { list-style: none; margin-left: -20px; display: flex; gap: 8px; align-items: flex-start; }
.prose li.td-task .ic { margin-top: 3px; color: var(--faint); } .prose li.td-task.on .ic { color: var(--success); }
.prose blockquote { margin: 0; padding: 2px 12px; border-left: 3px solid var(--line2); color: var(--muted); }
.td-panel .pb { display: grid; gap: 14px; }
.td-panel .pa { padding: 12px 16px; display: grid; gap: 8px; background: var(--muted-bg); border-radius: 0 0 10px 10px; }
.td-lab { font-size: 11.5px; color: var(--muted); font-weight: 500; margin-bottom: 6px; display: flex; align-items: center; gap: 6px; }
.td-gate { display: flex; gap: 8px; align-items: flex-start; padding: 5px 0; }
.td-gate .gi { width: 18px; height: 18px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; }
.td-gate .gi.ok { background: var(--success-soft); color: var(--success); } .td-gate .gi.no { background: var(--danger-soft); color: var(--danger); }
.td-gate .gi .ic { width: 12px; height: 12px; stroke-width: 2.5; }
.td-gate small { display: block; color: var(--muted); font-size: 12px; }
.td-props { display: grid; grid-template-columns: 92px minmax(0, 1fr); gap: 8px 12px; align-items: center; margin: 0; }
.td-props dt { color: var(--muted); } .td-props dd { margin: 0; min-width: 0; }
.td-props .combo { height: 30px; border-color: transparent; background: none; padding: 0 6px; margin-left: -6px; width: calc(100% + 6px); }
.td-props .combo:hover { background: var(--hover); } .td-props .combo > .ic { opacity: 0; } .td-props .combo:hover > .ic { opacity: 1; }
.td-pick { display: inline-flex; align-items: center; gap: 6px; border-radius: 6px; padding: 4px 6px; margin: -4px -6px; max-width: calc(100% + 12px); }
.td-pick:hover { background: var(--hover); }
.td-live .lh { display: flex; align-items: center; gap: 10px; padding: 12px 16px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
.td-steps { display: grid; gap: 2px; padding: 12px 16px; }
.td-step { display: flex; gap: 10px; align-items: center; height: 26px; color: var(--muted); }
.td-step.done { color: var(--fg2); } .td-step.done .ic { color: var(--success); }
.td-step.now { color: var(--fg); font-weight: 500; } .td-step.now .ic { color: var(--accent); }
.td-spin { animation: tdspin 1s linear infinite; }
@keyframes tdspin { to { transform: rotate(360deg); } }
.td-live .term { margin: 0 16px 16px; max-height: 168px; overflow-y: auto; }
.td-dec button.opt { display: flex; gap: 12px; align-items: flex-start; padding: 11px 12px; border: 1px solid var(--line2); border-radius: 8px; background: var(--raised); text-align: left; width: 100%; }
.td-dec button.opt:hover { border-color: var(--input); }
.td-dec button.opt.on { border-color: var(--accent); box-shadow: var(--ring); }
.td-dec button.opt .radio { margin-top: 2px; }
.td-dec button.opt b { font-weight: 500; display: block; } .td-dec .opt small { color: var(--muted); font-size: 12px; }
.td-ai { border-left: 2px solid var(--accent); padding: 2px 0 2px 12px; display: grid; gap: 6px; }
.td-code { font-family: var(--mono); font-size: 12px; line-height: 1.7; border-top: 1px solid var(--line); overflow-x: auto; }
.td-code > div { display: flex; white-space: pre; } .td-code span.ln { width: 40px; flex-shrink: 0; text-align: right; padding-right: 10px; color: var(--faint); user-select: none; }
.td-code .add { background: var(--success-soft); } .td-code .del { background: var(--danger-soft); }
.td-code > .td-rc { margin: 8px 12px 12px 50px; border: 1px solid var(--line2); border-radius: 8px; background: var(--raised); padding: 10px 12px; display: grid; gap: 6px; font-family: var(--font); white-space: normal; }
.td-code > .td-rc .row { flex-wrap: wrap; }
@media (max-width: 899px) { .td-code > .td-rc { margin-left: 12px; } }
.td-ev { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; align-items: stretch; }
@media (max-width: 899px) { .td-ev { grid-template-columns: 1fr; } }
.td-shot { border: 1px solid var(--line); border-radius: 10px; overflow: hidden; background: var(--raised); }
.td-shot .td-svg { display: block; width: 100%; height: auto; color: var(--fg2); border-bottom: 1px solid var(--line); }
.td-shot .cap { padding: 10px 12px; display: grid; gap: 4px; }
.td-sig { display: inline-flex; gap: 5px; align-items: center; font-size: 11.5px; color: var(--success); }
.td-drop { border: 1px dashed var(--input); border-radius: 8px; padding: 12px; text-align: center; color: var(--muted); font-size: 12px; width: 100%; }
.td-drop:hover { border-color: var(--accent); color: var(--accent-fg); }
.td-dm { display: inline-block; width: 16px; opacity: .7; text-decoration: none; }
.diff .del .td-dm { text-decoration: none; }
.td-est td, .td-est th { padding: 6px 10px !important; font-size: 12px; }
.td-est td:not(:first-child), .td-est th:not(:first-child) { text-align: right; font-variant-numeric: tabular-nums; }
.td-tabs { margin-top: 2px; }

/* Komentar & aktivitas */
.tc-bar { display: flex; align-items: flex-end; gap: 12px; padding: 0 12px 0 16px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
.tc-bar .tabs { border-bottom: 0; margin: 0; flex: 1; min-width: 0; }
.tc-bar .btn { margin-bottom: 4px; }
.tc-compose { padding: 14px 16px; border-bottom: 1px solid var(--line); display: grid; gap: 8px; }
.tc-compose .cfoot, .tc-reply .cfoot, .tc-edit .cfoot { display: flex; align-items: center; gap: 8px; justify-content: flex-end; }
.tc-list { display: grid; }
.tc-pins { background: var(--muted-bg); border-bottom: 1px solid var(--line); }
.tc-pins > .lab { display: flex; gap: 6px; align-items: center; font-size: 11.5px; color: var(--muted); font-weight: 500; padding: 10px 16px 0; }
.tc-c { display: flex; gap: 10px; padding: 14px 16px; border-top: 1px solid var(--line); scroll-margin-top: 80px; }
.tc-list > .tc-c:first-child, .tc-pins > .tc-c { border-top: 0; }
.tc-c.hl { animation: tchl 2.4s ease; }
@keyframes tchl { 0%, 40% { background: var(--accent-soft); } }
.tc-main { display: grid; gap: 6px; min-width: 0; flex: 1; }
.tc-meta { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; min-width: 0; }
.tc-meta .grow { min-width: 8px; }
.tc-body { line-height: 1.6; overflow-wrap: anywhere; } .tc-body p { margin: 0 0 6px; } .tc-body p:last-child { margin: 0; } .tc-body ul, .tc-body ol { margin: 0 0 6px; padding-left: 20px; }
.tc-body .mention, .x-mention { color: var(--accent-fg); background: var(--accent-soft); border-radius: 4px; padding: 0 3px; font-weight: 500; }
.tc-body pre { font-family: var(--mono); font-size: 12px; background: var(--code-bg); border: 1px solid var(--line); border-radius: 6px; padding: 8px 10px; white-space: pre-wrap; margin: 0 0 6px; }
.tc-acts { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
.tc-react { height: 24px; padding: 0 7px; border-radius: 12px; border: 1px solid var(--line2); background: var(--raised); display: inline-flex; align-items: center; gap: 4px; font-size: 13px; }
.tc-react span { font-size: 12px; color: var(--muted); font-variant-numeric: tabular-nums; }
.tc-react:hover { border-color: var(--input); } .tc-react.on { border-color: color-mix(in srgb, var(--accent) 45%, transparent); background: var(--accent-soft); } .tc-react.on span { color: var(--accent-fg); }
.tc-react.add { color: var(--muted); padding: 0 6px; }
.tc-link { height: 24px; padding: 0 6px; border-radius: 5px; color: var(--muted); font-size: 12.5px; font-weight: 500; display: inline-flex; align-items: center; gap: 4px; }
.tc-link:hover { background: var(--hover); color: var(--fg); }
.tc-file { display: inline-flex; align-items: center; gap: 6px; height: 28px; padding: 0 10px; border: 1px solid var(--line2); border-radius: 6px; background: var(--muted-bg); font-size: 12.5px; color: var(--fg2); width: fit-content; }
.tc-file:hover { border-color: var(--input); }
.tc-replies { display: grid; gap: 0; border-left: 2px solid var(--line); margin-top: 4px; }
.tc-replies .tc-c { border-top: 0; padding: 8px 0 8px 12px; }
.tc-reply { padding: 8px 0 4px 12px; display: grid; gap: 8px; }
.tc-edit { display: grid; gap: 8px; }
.tc-del { color: var(--muted); font-style: italic; }
.tc-h { display: flex; gap: 10px; padding: 10px 16px; border-top: 1px solid var(--line); align-items: flex-start; color: var(--fg2); }
.tc-list > .tc-h:first-child { border-top: 0; }
.tc-h .tile { width: 24px; height: 24px; margin-left: 2px; }
.tc-h.ag .tile { background: var(--accent-soft); color: var(--accent-fg); }
.tc-mini { display: flex; gap: 10px; padding: 10px 16px; border-top: 1px solid var(--line); }
.tc-mini:first-child { border-top: 0; }
.tc-mini .tc-body { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }

/* Navigator artifact */
.an { display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: 16px; align-items: start; }
@media (max-width: 899px) { .an { grid-template-columns: 1fr; } }
.an-nav { position: sticky; top: 64px; display: grid; }
@media (max-width: 899px) { .an-nav { position: static; } }
.an-s { padding: 10px; border-bottom: 1px solid var(--line); }
.an-sum { display: flex; gap: 8px; align-items: center; padding: 8px 14px; border-bottom: 1px solid var(--line); font-size: 12px; color: var(--muted); }
.an-list { max-height: calc(100vh - 260px); overflow-y: auto; padding: 4px; }
@media (max-width: 899px) { .an-list { max-height: none; } }
.an-g { display: flex; align-items: center; gap: 6px; font-size: 11.5px; font-weight: 500; color: var(--muted); padding: 10px 8px 4px; }
.an-i { display: flex; gap: 10px; align-items: flex-start; padding: 7px 8px; border-radius: 7px; width: 100%; text-align: left; }
.an-i:hover { background: var(--hover); } .an-i.on { background: var(--accent-soft); }
.an-i .ic { margin-top: 2px; color: var(--faint); } .an-i.on .ic { color: var(--accent-fg); }
.an-i b { font-weight: 500; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.an-i small { display: block; color: var(--muted); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.an-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; margin-top: 6px; }
.an-dot.ok { background: var(--success); } .an-dot.rev { background: var(--warn); } .an-dot.rej { background: var(--danger); } .an-dot.draft, .an-dot.old { background: var(--input); }
.an-pv .bh { flex-wrap: wrap; }
.an-pv .prose { padding: 18px 22px; max-width: none; }
.x-vers { display: inline-flex; gap: 2px; }
.x-ver { height: 24px; min-width: 30px; padding: 0 7px; border-radius: 5px; font-family: var(--mono); font-size: 12px; color: var(--muted); border: 1px solid transparent; }
.x-ver:hover { background: var(--hover); color: var(--fg); }
.x-ver.on { border-color: var(--line2); background: var(--raised); color: var(--fg); box-shadow: var(--shadow-sm); }

/* Editor artifact */
.ae { display: grid; grid-template-columns: minmax(0, 1fr) 340px; gap: 20px; align-items: start; }
@media (max-width: 1099px) { .ae { grid-template-columns: 1fr; } }
.ae-main { display: grid; gap: 12px; min-width: 0; }
.ae-side { position: sticky; top: 110px; max-height: calc(100vh - 130px); overflow-y: auto; }
.ae-card .ed { border: 0; border-radius: 0; box-shadow: none; }
.ae-card .alert + [data-ae-ed] .ed { border-top: 1px solid var(--line); }
.ae-card .ed .tb { top: 94px; }
@media (max-width: 899px) { .ae-card .ed .tb { top: 52px; } }
.an-nav { position: sticky; top: 110px; }

@media (max-width: 1099px) { .ae-side { position: static; max-height: none; } }
.ae-side .tabs { padding: 0 16px; }
.ae-v { display: grid; gap: 4px; padding: 12px 16px; border-top: 1px solid var(--line); }
.ae-v:first-child { border-top: 0; } .ae-v.on { background: var(--accent-soft); }
.ae-v .row { flex-wrap: wrap; }
.ae-mark { background: color-mix(in srgb, var(--warn) 22%, transparent); color: inherit; border-bottom: 2px solid var(--warn); border-radius: 2px; cursor: pointer; padding: 0 1px; }
.ae-mark.on { background: color-mix(in srgb, var(--accent) 25%, transparent); border-color: var(--accent); }
.ae-ic { display: grid; gap: 6px; padding: 12px 16px; border-top: 1px solid var(--line); }
.ae-ic:first-child { border-top: 0; } .ae-ic.on { background: var(--accent-soft); } .ae-ic.done { opacity: .65; }
.ae-ic blockquote { margin: 0; padding: 2px 8px; border-left: 3px solid var(--warn); color: var(--muted); font-size: 12px; }
.ae-ic .re { display: grid; gap: 4px; padding-left: 10px; border-left: 2px solid var(--line); }

/* Daftar & settings */
.ib-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); display: inline-block; }
.ib-dot.off { background: transparent; }
.ib-t { line-height: 1.45; } .ib-t.read { color: var(--muted); }
.tc-tok { font-family: var(--mono); font-size: 12px; height: 26px; padding: 0 8px; border-radius: 6px; border: 1px solid var(--line2); background: var(--raised); color: var(--fg2); }
.tc-tok:hover { border-color: var(--accent); color: var(--accent-fg); }
.tc-prev { font-family: var(--mono); font-size: 13px; font-weight: 500; color: var(--fg); }
.tc-opt { display: flex; gap: 12px; align-items: flex-start; padding: 12px 16px; border-top: 1px solid var(--line); cursor: pointer; }
.tc-opt:first-child { border-top: 0; } .tc-opt .radio { margin-top: 1px; }
.tt-ico { width: 28px; height: 28px; border-radius: 7px; display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; }
.tt-ico.lg { width: 40px; height: 40px; border-radius: 10px; } .tt-ico.lg .ic { width: 20px; height: 20px; }
.tt-icons { display: flex; flex-wrap: wrap; gap: 6px; }
.tt-icons .btn.on { border-color: var(--accent); color: var(--accent-fg); background: var(--accent-soft); box-shadow: var(--ring); }
.tt-prev { display: flex; gap: 12px; align-items: center; padding: 12px 14px; border: 1px dashed var(--line2); border-radius: 10px; background: var(--muted-bg); }
.x-opt { display: flex; gap: 12px; align-items: flex-start; }
.x-opt .t { font-weight: 500; } .x-opt .d { color: var(--muted); font-size: 12.5px; }

/* Workflow */
.wf-tpl { padding: 14px; display: grid; gap: 10px; align-content: start; }
.wf-chain { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; font-size: 12px; color: var(--muted); }
.wf-chain .ic { width: 12px; height: 12px; color: var(--input); }
.wf-chain span { background: var(--hover); border-radius: 4px; padding: 1px 6px; color: var(--fg2); }
.wf-strip { display: flex; gap: 3px; width: 140px; justify-content: flex-end; } .wf-strip i { width: 14px; height: 6px; border-radius: 3px; display: block; }
.wf { display: grid; grid-template-columns: minmax(0, 1fr) 360px; height: calc(100dvh - 94px); }
@media (min-width: 900px) { html[data-shell-v="A"] .wf { height: calc(100dvh - 110px); } }
.wf-main { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.wf-top { padding: 0 20px; background: var(--bg); border-bottom: 1px solid var(--line); }
.wf-top .tabs { border-bottom: 0; margin: 0; }
.wf-tools { display: flex; align-items: center; gap: 12px; padding: 12px 20px 0; flex-wrap: wrap; }
.wf-canvas-wrap { flex: 1; overflow: auto; min-height: 0; }
.wf-canvas { position: relative; margin: 16px auto 32px; }
.wf-canvas svg.wf-lines { position: absolute; inset: 0; overflow: visible; pointer-events: none; }
.wf-node { position: absolute; width: 236px; height: 56px; display: flex; align-items: center; gap: 10px; padding: 0 12px; border: 1px solid var(--line2); border-radius: 10px; background: var(--raised); box-shadow: var(--shadow-sm); text-align: left; }
.wf-node:hover { border-color: var(--input); }
.wf-node.on { border-color: var(--accent); box-shadow: var(--ring); }
.wf-node b { font-weight: 500; display: block; } .wf-node small { color: var(--muted); font-size: 11.5px; display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.wf-node .wi { width: 28px; height: 28px; border-radius: 7px; background: var(--hover); display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; }
.wf-node .gc { font-size: 11px; color: var(--muted); display: inline-flex; align-items: center; gap: 3px; }
.wf-badge { position: absolute; transform: translate(-50%, -50%); width: 22px; height: 22px; border-radius: 50%; background: var(--raised); border: 1px solid var(--line2); display: inline-flex; align-items: center; justify-content: center; color: var(--muted); z-index: 1; }
.wf-badge.auto { color: var(--accent-fg); border-color: var(--accent); }
.wf-badge .ic { width: 12px; height: 12px; }
.wf-tag { position: absolute; font-size: 11.5px; color: var(--muted); background: var(--page); padding: 0 4px; white-space: nowrap; }
.wf-tag.dng { color: var(--danger); }
.wf-legend { display: flex; gap: 14px; flex-wrap: wrap; font-size: 12px; color: var(--muted); }
.wf-legend span { display: inline-flex; gap: 5px; align-items: center; }
.wf-main > .savebar { margin: 0; padding: 12px 20px; background: var(--bg); border-top: 1px solid var(--line); }
.wf-cfg { border-left: 1px solid var(--line); background: var(--bg); overflow-y: auto; min-height: 0; }
.wf-cfg .ch { padding: 12px 16px; border-bottom: 1px solid var(--line); display: flex; gap: 10px; align-items: center; position: sticky; top: 0; background: var(--bg); z-index: 2; }
.wf-cfg .cs { padding: 14px 16px; border-bottom: 1px solid var(--line); display: grid; gap: 12px; }
.wf-cfg .cs:last-child { border-bottom: 0; }
.wf-cfg .cs h3 { font-size: 12px; font-weight: 600; color: var(--fg2); }
.wf-cfg .grule { display: flex; align-items: center; gap: 8px; padding: 5px 4px 5px 10px; border: 1px solid var(--line2); border-radius: 7px; background: var(--raised); }
.wf-cfg .hint { margin-top: -4px; }
.wf-dz { display: grid; grid-template-columns: minmax(0, 1fr) 112px; gap: 10px; align-items: center; }
.wf-dz .selwrap select { height: 30px; }
.wf-opt { display: flex; gap: 10px; align-items: flex-start; padding: 9px 10px; border: 1px solid var(--line2); border-radius: 8px; background: var(--raised); width: 100%; text-align: left; }
.wf-opt:hover { border-color: var(--input); } .wf-opt.on { border-color: var(--accent); box-shadow: var(--ring); }
.wf-opt .radio { margin-top: 1px; } .wf-opt b { font-weight: 500; display: block; } .wf-opt small { color: var(--muted); font-size: 12px; display: block; }
@media (max-width: 1099px) { .wf { grid-template-columns: 1fr; height: auto; } .wf-cfg { border-left: 0; border-top: 1px solid var(--line); } .wf-canvas-wrap { max-height: 70vh; } .wf-main > .savebar { position: static; } }
.x-subh { display: grid; gap: 2px; padding: 14px 16px 10px; border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); background: var(--muted-bg); }
.mg-map { display: grid;
 grid-template-columns: minmax(0, 1fr) 28px minmax(0, 1fr); gap: 12px; align-items: center; padding: 14px 16px; }
@media (max-width: 899px) { .mg-map { grid-template-columns: 1fr; } .mg-map > .arr { transform: rotate(90deg); justify-self: start; } }
`);

/* Render ulang yang menyimpan draf komentar. */
function stashDrafts() {
  document.querySelectorAll("[data-draft]").forEach((w) => { const a = w.querySelector(".ed .area"); if (a) S.draft[w.dataset.draft] = a.innerHTML; });
}
function rr() { stashDrafts(); AG.rerender(); }
/* Menu vertikal subpage: cegah href="#" berpindah ke beranda, dan ingat panel aktif. */
function subpageKeep(root, key) {
  root.addEventListener("click", (e) => { const a = e.target.closest('.sp-nav a[data-tab]'); if (!a) return; e.preventDefault(); S.sub[key] = a.dataset.tab; });
}
const SRC = {};
const dtKeys = (id) => [...(AG._dts[id]?.sel || [])].map((i) => SRC[id]?.[i]).filter(Boolean);

/* ====================== Komponen modul ====================== */
const card = (t) => AG.ticketCard(t)
  .replace(`<span class="mono faint grow">${t.k}</span>`, `<span class="mono faint grow">${t.k}</span><button type="button" class="bd-mv" data-menu="bd-move" data-arg="${t.k}" aria-label="Pindahkan ${t.k} ke stage lain" title="Pindahkan ke…">${ic("dots", "s14")}</button>`)
  .replace('<article class="card"', '<article class="card" tabindex="0"');
const stageStrip = (t) => {
  const flow = flowOf(t); const i = flow.findIndex((s) => s.k === t.stage);
  return `<div class="stages" aria-label="Alur stage">${flow.map((s, j) => `${j ? `<span class="arr">${ic("chevron-right", "s14")}</span>` : ""}<span class="stg ${j < i ? "done" : j === i ? (t.state === "keputusan" ? "dng" : "now") : ""}" ${j === i ? 'aria-current="step"' : ""}>${j < i ? ic("check", "s14") : ic(s.i, "s14")}${s.n}</span>`).join("")}</div>`;
};
const personName = (k) => (k ? P(k).name : "Belum ada penerima");
const stagePill = (k) => { const s = AG.stage(k); return `<span class="pill">${stageIcon(k, "s14")}${s.n}</span>`; };
/* Kartu bagian v3: judul (+ jumlah) di header kartu, foot opsional. */
const xbox = (title, body, { n = "", act = "", cls = "", pad = false, desc = "", foot = "" } = {}) => xcard({ title: `${title}${n !== "" ? `<span class="cx-n">${n}</span>` : ""}`, desc, action: act, body, foot, pad, cls });
const whoCell = (k) => `<span class="row">${av(k, 20)}<span class="trunc ${k ? "" : "mut"}">${k ? P(k).name : "Kosong"}</span></span>`;
const keyCell = (t) => `<span class="row g4 nowrap">${typeIcon(t.type)}<span class="mono mut">${t.k}</span></span>`;
const dueSort = (d) => (d ? parseInt(d) + (/Des/.test(d) ? 30 : 0) : 999);

/* ====================== M2 · Board ====================== */
const EPICS = () => AG.TICKETS.filter((t) => t.type === "epic");
const epicOf = (t) => t.epic || (t.parent && T(t.parent)?.epic) || (t.type === "epic" ? t.k : null);
const epicLabel = (k) => `${k} · ${T(k).t}`;
const BOARD_FILTERS = () => [
  { k: "who", label: "Penerima", icon: "users", options: [...Object.values(AG.PEOPLE).map((p) => p.name), "Belum ada penerima"] },
  { k: "lbl", label: "Label", icon: "tag", options: Object.keys(AG.LABELS) },
  { k: "type", label: "Tipe", icon: "category-2", options: Object.values(AG.TYPES).map((x) => x.n) },
  { k: "prio", label: "Prioritas", icon: "antenna-bars-5", options: [1, 2, 3, 4].map((p) => AG.PRIORITY[p].n) },
  { k: "epic", label: "Epic", icon: "bolt", options: [...EPICS().map((e) => epicLabel(e.k)), "Tanpa epic"] },
  { k: "sprint", label: "Sprint", icon: "calendar", options: ["Sprint 4", "Sprint 3", "Tanpa sprint"] },
];
const fv = (id, k) => AG.filterValue(id, k);
function matchFilters(t, id) {
  const has = (k, v) => { const x = fv(id, k); return !x.length || x.includes(v); };
  const q = AG.filterQuery(id);
  return has("who", t.who ? P(t.who).name : "Belum ada penerima") && (!fv(id, "lbl").length || t.lbl.some((l) => fv(id, "lbl").includes(l)))
    && has("type", AG.TYPES[t.type].n) && has("prio", AG.PRIORITY[t.p].n) && has("epic", epicOf(t) ? epicLabel(epicOf(t)) : "Tanpa epic") && has("sprint", t.sprint || "Tanpa sprint")
    && (!q || `${t.k} ${t.t}`.toLowerCase().includes(q));
}
function filtered() {
  return AG.TICKETS.filter((t) => (!S.f.has("me") || t.who === AG.ME) && (!S.f.has("review") || ["review", "keputusan"].includes(t.state)) && (!S.f.has("ai") || t.state === "jalan") && matchFilters(t, "bd"));
}
function lanes(list) {
  if (S.swim === "epic") {
    return [...EPICS().map((e) => ({ k: e.k, h: `${typeIcon("epic")}<span class="mono mut">${e.k}</span><b class="b5">${e.t}</b>`, items: list.filter((t) => epicOf(t) === e.k) })),
      { k: "none", h: `<b class="b5">Tanpa epic</b>`, items: list.filter((t) => !epicOf(t)) }];
  }
  if (S.swim === "who") {
    return [...Object.keys(AG.PEOPLE).map((p) => ({ k: p, h: `${av(p, 20)}<b class="b5">${P(p).name}</b>`, items: list.filter((t) => t.who === p) })), { k: "none", h: `${av(null, 20)}<b class="b5">Belum ada penerima</b>`, items: list.filter((t) => !t.who) }].filter((l) => l.items.length);
  }
  return [{ k: "all", items: list }];
}
function colHead(s, list) {
  const n = list.filter((t) => t.stage === s.k).length, all = AG.TICKETS.filter((t) => t.stage === s.k).length, w = wipOf(s);
  return `<div class="bd-colh" data-colh="${s.k}">${stageIcon(s.k)}<span class="trunc">${s.n}</span><span class="n">${n}</span><span class="grow"></span>
    ${s.persona ? av(s.persona, 18) : ""}${w ? `<span class="wip ${all > w ? "over" : all === w ? "full" : ""}" title="WIP limit ${w}">${all}/${w}</span>` : ""}
    <button type="button" class="bd-add" data-dialog="new-ticket" data-arg="${s.k}" aria-label="Tiket baru di ${s.n}">${ic("plus", "s14")}</button></div>`;
}
function board(k) {
  const list = filtered();
  const stages = AG.STAGES.filter((s) => !(S.hideDone && s.k === "done"));
  const bar = AG.filterBar({ id: "bd", search: "Cari tiket di board…", filters: BOARD_FILTERS(),
    quick: [{ label: "Saya", icon: "user", on: S.f.has("me") }, { label: "Butuh review", icon: "eye", on: S.f.has("review") }, { label: "Dikerjakan AI", icon: "sparkles", on: S.f.has("ai") }],
    count: `${list.length} dari ${AG.TICKETS.length} tiket`,
    right: `<span class="bd-hint">${ic("hand-stop", "s14")}Seret kartu antar kolom</span>${btn(`Tampilan${S.swim !== "none" ? ` <span class="fcount">1</span>` : ""}`, "", "adjustments-horizontal", { "data-menu": "bd-view" })}` });
  const cls = `${S.hideAgent ? "bd-noagent" : ""} ${S.hideMeta ? "bd-nometa" : ""}`;
  let main;
  if (!list.length) main = `<div style="padding:24px 20px;max-width:520px">${empty("filter", "Tidak ada tiket yang cocok", "Ubah atau hapus filter untuk melihat tiket lain.", btn("Hapus filter", "", "x", { "data-act": "bd-clear" }))}</div>`;
  else if (S.swim === "none") {
    main = `<div class="board bd-board ${cls}">${stages.map((s) => { const items = list.filter((t) => t.stage === s.k);
      return `<div class="bcol bd-cell" data-col="${s.k}" data-lane="all" style="--stc:${s.c}">${colHead(s, list)}<div class="bd-cards">${items.map(card).join("") || `<div class="bd-empty">Belum ada tiket</div>`}</div></div>`; }).join("")}</div>`;
  } else {
    main = `<div class="bd-scroll ${cls}"><div class="bd-heads">${stages.map((s) => colHead(s, list)).join("")}</div>
      ${lanes(list).map((l) => `<div class="bd-lane ${S.collapsed.has(l.k) ? "shut" : ""}">
        <button type="button" class="bd-laneh" data-act="bd-lane" data-arg="${l.k}" aria-expanded="${!S.collapsed.has(l.k)}">${ic("chevron-down", "s14 chev")}${l.h}<span class="n">${l.items.length}</span></button>
        <div class="bd-cells">${stages.map((s) => `<div class="bcol bd-cell" data-col="${s.k}" data-lane="${l.k}" style="--stc:${s.c}"><div class="bd-cards">${l.items.filter((t) => t.stage === s.k).map(card).join("")}</div></div>`).join("")}</div></div>`).join("")}</div>`;
  }
  return AG.shell({ nav: "app", project: k, crumbs: AG.projectCrumb(k, "Board"), ...AG.projectTabs(k), flush: true,
    actions: btn("Tiket baru", "pri", "plus", { "data-dialog": "new-ticket" }),
    body: `<div class="bd-page"><div class="bd-bar">${bar}</div>${main}</div>` });
}
function boardAfter(root) {
  let drag = null, over = null;
  const clear = () => root.querySelectorAll(".bd-cell.drop,.bd-cell.deny").forEach((c) => { if (!c.querySelector(".bd-why")) c.classList.remove("drop", "deny"); });
  root.querySelectorAll("[data-drag]").forEach((c) => {
    c.addEventListener("dragstart", (e) => { drag = c.dataset.drag; e.dataTransfer.setData("text/plain", drag); e.dataTransfer.effectAllowed = "move"; requestAnimationFrame(() => c.classList.add("dragging")); });
    c.addEventListener("dragend", () => { c.classList.remove("dragging"); drag = null; over = null; clear(); });
    c.addEventListener("keydown", (e) => { if (e.target !== c) return; if (e.key === "Enter") location.hash = c.dataset.go; if (e.key === "m" || e.key === "M") { e.preventDefault(); c.querySelector(".bd-mv")?.click(); } });
  });
  root.querySelectorAll(".bd-cell").forEach((col) => {
    col.addEventListener("dragover", (e) => {
      const k = drag || e.dataTransfer.getData("text/plain"); if (!k) return; e.preventDefault();
      const key = col.dataset.col + "|" + col.dataset.lane; if (over === key) return; over = key; clear();
      const r = check(T(k), col.dataset.col); if (r.same) return;
      root.querySelectorAll(`.bd-cell[data-col="${col.dataset.col}"]`).forEach((c) => c.classList.add(r.ok || r.skip ? "drop" : "deny"));
    });
    col.addEventListener("drop", (e) => {
      e.preventDefault(); const k = drag || e.dataTransfer.getData("text/plain"); if (!k) return;
      const t = T(k), lane = col.dataset.lane; over = null; clear();
      if (S.swim === "who" && t.stage === col.dataset.col && lane !== (t.who || "none")) { t.who = lane === "none" ? null : lane; rr(); return AG.toast(`${k} ditugaskan ke ${personName(t.who)}`); }
      doMove(k, col.dataset.col);
    });
  });
  /* Quick filter dan pencarian bar filter. */
  const keys = ["me", "review", "ai"];
  root.querySelectorAll('[data-fbar="bd"] .fquick').forEach((b, i) => b.addEventListener("click", () => { S.f.has(keys[i]) ? S.f.delete(keys[i]) : S.f.add(keys[i]); setTimeout(rr, 0); }));
  const q = root.querySelector('[data-fsearch="bd"]'); let tm;
  if (q) q.addEventListener("input", () => { clearTimeout(tm); tm = setTimeout(() => { AG.rerender(); const n = document.querySelector('[data-fsearch="bd"]'); if (n) { n.focus(); n.selectionStart = n.selectionEnd = n.value.length; } }, 220); });
}
AG.screen({ m: "M2", g: "Tiket & board", n: "Board Kanban", route: "/p/:k/board", path: "/p/KSR/board", render: (p) => board(p.k), after: boardAfter });

AG.action("bd-clear", () => { S.f.clear(); AG._filters.bd?.filters.forEach((f) => (f.value = [])); if (AG._filters.bd) AG._filters.bd.q = ""; rr(); });
AG.action("bd-swim", (el, a) => { S.swim = a; rr(); });
AG.action("bd-opt", (el, a) => { S[a] = !S[a]; rr(); });
AG.action("bd-lane", (el, a) => { S.collapsed.has(a) ? S.collapsed.delete(a) : S.collapsed.add(a); el.closest(".bd-lane").classList.toggle("shut"); el.setAttribute("aria-expanded", !S.collapsed.has(a)); });
AG.action("bd-move", (el, a) => { const [k, to] = a.split(">"); doMove(k, to); });
AG.menu("bd-view", () => {
  const opt = (key, label, inverted = true) => { const on = inverted ? !S[key] : S[key]; return `<button class="mi" data-act="bd-opt" data-arg="${key}"><span class="cb ${on ? "on" : ""}">${ic("check")}</span>${label}</button>`; };
  return `<div class="mlab">Swimlane</div>${[["none", "Tanpa swimlane", "layout-columns"], ["epic", "Per epic", "bolt"], ["who", "Per penerima", "users"]].map(([k, n, i]) => `<button class="mi" data-act="bd-swim" data-arg="${k}">${ic(i)}${n}${S.swim === k ? `<span class="tail">${ic("check", "s14")}</span>` : ""}</button>`).join("")}
    <div class="msep"></div><div class="mlab">Tampilkan</div>${opt("hideDone", "Kolom Selesai")}${opt("hideAgent", "Status agent di kartu")}${opt("hideMeta", "Prioritas, label, poin, tenggat")}
    <div class="msep"></div><button class="mi" data-dialog="views" data-arg="KSR">${ic("layout-navbar")}Atur tab tampilan…</button><a class="mi" href="#/settings/p/KSR/workflow">${ic("route")}Atur WIP limit & workflow</a>`;
});
AG.menu("bd-move", (k) => {
  const t = T(k);
  return `<div class="mlab">Pindahkan ${k} ke…</div>${flowOf(t).map((s) => { const r = check(t, s.k);
    const tail = r.same ? ic("check", "s14") : r.skip ? "Lompat" : r.ok ? (r.back ? "Kembali" : "Berikutnya") : ic("lock", "s14");
    return `<button class="mi" ${r.same ? 'disabled aria-disabled="true" style="opacity:.55"' : `data-act="bd-move" data-arg="${k}>${s.k}"`} title="${esc(r.why || "")}">${si(s.k)}${s.n}<span class="tail">${tail}</span></button>`; }).join("")}
    <div class="msep"></div><a class="mi" href="#/t/${k}">${ic("external-link")}Buka tiket</a>`;
});

/* ====================== M2 · Daftar ====================== */
const LIST_FILTERS = () => [
  { k: "stage", label: "Stage", icon: "route", options: AG.STAGES.map((s) => s.n) },
  { k: "state", label: "Status", icon: "progress", options: Object.values(AG.STATES).map((s) => s.n) },
  ...BOARD_FILTERS().filter((f) => f.k !== "epic").map((f) => (f.k === "sprint" ? { ...f, options: ["Sprint 4", "Sprint 3", "Sprint 2", "Sprint 1", "Tanpa sprint"] } : f)),
];
const ticketRow = (t) => ({
  cells: [keyCell(t), `<div class="row" style="min-width:200px;max-width:290px"><span class="trunc" style="min-width:0" title="${esc(t.t)}">${esc(t.t)}</span><span class="row g4 hide-sm" style="flex:none">${t.lbl.slice(0, 1).map(labelPill).join("")}</span></div>`, stagePill(t.stage), statePill(t.state), whoCell(t.who), AG.prioLabel(t.p), `<span class="mono">${t.pts ?? "–"}</span>` ],
  sort: [num(t.k), t.t, sIdx(t.stage), Object.keys(AG.STATES).indexOf(t.state), t.who ? P(t.who).name : "~", t.p, t.pts ?? 999],
  text: `${t.k} ${t.t} ${t.lbl.join(" ")}`,
  filter: { stage: AG.stage(t.stage).n, state: AG.STATES[t.state].n, who: t.who ? P(t.who).name : "Belum ada penerima", lbl: t.lbl[0] || "", type: AG.TYPES[t.type].n, prio: AG.PRIORITY[t.p].n, sprint: t.sprint || "Tanpa sprint" },
});
const TICKET_COLS = [{ label: "Kode", w: "104px" }, { label: "Judul" }, { label: "Stage", hide: "sm" }, { label: "Status", hide: "sm" }, { label: "Penerima", hide: "sm" }, { label: "Prioritas", w: "96px" }, { label: "Poin", hide: "sm", align: "right", w: "56px" }];
function listScreen(k) {
  const items = ALL().sort((a, b) => num(b.k) - num(a.k)); SRC.kl = items.map((t) => t.k);
  return AG.shell({ nav: "app", project: k, crumbs: AG.projectCrumb(k, "Daftar"), ...AG.projectTabs(k),
    actions: btn("Tiket baru", "pri", "plus", { "data-dialog": "new-ticket" }),
    body: `<div class="x-dtfit">` + AG.datatable({ id: "kl", columns: TICKET_COLS, rows: items.map(ticketRow), filters: LIST_FILTERS(), search: "Cari kode atau judul…", pageSize: 10, select: true,
      right: btn("Ekspor CSV", "", "file-export", { "data-toast": `CSV ${items.length} tiket diunduh` }),
      bulk: `${btn("Ubah stage", "sm ghost", "route", { "data-act": "kl-bulk", "data-arg": "edit" })}${btn("Penerima", "sm ghost", "user", { "data-act": "kl-bulk", "data-arg": "edit" })}${btn("Prioritas", "sm ghost", "antenna-bars-5", { "data-act": "kl-bulk", "data-arg": "edit" })}${btn("Hapus", "sm ghost dng", "trash", { "data-act": "kl-bulk", "data-arg": "del" })}`,
      rowGo: (i) => `#/t/${items[i].k}` }) + `</div>` });
}
AG.screen({ m: "M2", g: "Tiket & board", n: "Daftar tiket", route: "/p/:k/list", path: "/p/KSR/list", render: (p) => listScreen(p.k) });
AG.action("kl-bulk", (el, a) => { const ks = dtKeys("kl"); if (!ks.length) return; S.sel = new Set(ks); AG.openDialog(a === "del" ? "delete-ticket" : "bulk-edit", ks.join(",")); });

/* ====================== M2 · Detail tiket ====================== */
const DESC = {
  "KSR-24": `Kasir perlu menerima pembayaran **QRIS** langsung dari layar kasir, tanpa mesin EDC terpisah. QR dibuat dinamis sesuai total belanja dan status bayar diperbarui otomatis.

### Acceptance criteria
- [x] QR dinamis tampil saat kasir memilih QRIS
- [x] Status berubah otomatis saat callback diterima
- [ ] QR kedaluwarsa setelah 15 menit dan bisa dibuat ulang
- [x] Struk mencetak metode bayar dan nomor referensi

Repo terdampak: \`kasir-api\`, \`kasir-web\` (dideteksi dari code graph, dikonfirmasi Olivia).`,
  "KSR-30": `Nama kasir tidak muncul di struk untuk transaksi yang memakai diskon per item. Ditemukan saat uji regresi KSR-29.

### Langkah reproduksi
1. Login sebagai kasir Rina (PIN 6 digit)
2. Tambah 3 barang, beri diskon 10% pada satu barang
3. Bayar tunai, cetak struk

**Hasil:** nama kasir kosong dan total Rp48.900. **Seharusnya:** nama Rina dan total Rp49.000.`,
  "KSR-26": `Checkout harus ditolak bila stok barang di toko tidak cukup. Saat ini transaksi tetap jalan dan stok menjadi minus.

### Acceptance criteria
- [ ] Pesan “Stok tidak cukup” tampil per baris barang
- [ ] Supervisor bisa mengizinkan stok minus dengan PIN
- [ ] Validasi juga berjalan di API, bukan hanya di layar`,
  "KSR-36": `Setiap toko bisa mengatur tarif PPN sendiri (0%, 11%, atau 12%) dan pilihan harga sudah termasuk pajak atau belum. Pajak tampil terpisah di struk.

> Menunggu KSR-24 selesai karena perhitungan total QRIS harus sudah memakai pajak.`,
};
const descOf = (t) => AG.mdToHtml(DESC[t.k] || `${t.t}. Tiket ini bagian dari pengembangan aplikasi Kasir Online untuk toko retail.\n\n### Acceptance criteria\n- [ ] Perilaku utama berjalan di layar kasir\n- [ ] Pesan error jelas untuk kasir`);
const CUSTOM = { "KSR-24": [["Toko pilot", "Cabang Sudirman"], ["Penyedia QRIS", "Xendit"]], "KSR-30": [["Versi aplikasi", "2.4.0"], ["Dampak pelanggan", "38 toko"]] };
const WATCH = {};
const watchOf = (t) => WATCH[t.k] || (WATCH[t.k] = ["DD", "QQ", "VV"]);
const ATTACH = { "KSR-24": [["paperclip", "callback-sample.json", "2 KB", "DD"], ["photo", "alur-bayar-qris.png", "184 KB", "OO"]] };
const attOf = (t) => ATTACH[t.k] || (ATTACH[t.k] = []);

function stagePanel(t) {
  const st = AG.stage(t.stage), per = st.persona ? AG.PERSONAS[st.persona] : null, gs = gates(t);
  const mode = st.mode === "interaktif" ? pill("Interaktif", "info", "", "message-2") : per ? pill("Otomatis", "acc", "", "bolt") : pill("Manual", "gray");
  const appr = t.stage === "review" ? ["AA", "OO"] : t.stage === "test" ? ["QQ", "OO"] : ["OO"];
  const ex = [st.persona, t.who].filter(Boolean);
  const stack = (ks) => `<span class="stack">${ks.map((k) => av(k, 22)).join("")}</span>`;
  let acts = "";
  if (t.state === "review") acts = `${btn("Approve", "ok full", "check", { "data-act": "td-approve", "data-arg": t.k })}
    <div class="grid2" style="gap:8px">${btn("Edit lalu approve", "", "pencil", { "data-act": "td-editapprove", "data-arg": t.k })}${btn("Reject", "dng", "x", { "data-dialog": "reject", "data-arg": t.k })}</div>
    <span class="mut xs">Cukup satu approver. Approve memicu job ${AG.stage(AG.STAGES[sIdx(t.stage) + 1]?.k || "done").n} otomatis.</span>`;
  else if (t.state === "keputusan") acts = `${btn("Ambil keputusan", "dngs full", "alert-triangle", { "data-dialog": "decision", "data-arg": t.k })}<span class="mut xs">Putaran gagal mencapai batas 3. Agent berhenti sampai ada keputusan.</span>`;
  else if (t.state === "jalan") acts = `<div class="grid2" style="gap:8px">${btn("Ambil alih", "", "user", { "data-act": "td-takeover", "data-arg": t.k })}${btn("Hentikan job", "dng", "player-stop", { "data-act": "td-stop", "data-arg": t.k })}</div><span class="mut xs">Gate dicek ulang otomatis saat job selesai.</span>`;
  else if (t.state === "antre") acts = `${btn("Jalankan di runner saya", "full", "player-play", { "data-act": "td-run", "data-arg": t.k })}<span class="mut xs">${t.agent || "Menunggu runner yang cocok online"}</span>`;
  else if (t.state === "manusia") acts = btn("Tandai selesai, minta review", "pri full", "check", { "data-act": "td-human-done", "data-arg": t.k });
  else if (t.stage === "backlog") { const bl = blockers(t); acts = bl.length ? `${btn(`Mulai ${AG.STAGES[1].n}`, "full dis", "lock", { "aria-disabled": "true" })}<span class="text-dng xs row g4">${ic("lock", "s14")}Diblokir oleh ${bl.join(", ")}. Mulai setelah tiket itu selesai.</span>` : btn(`Mulai ${AG.STAGES[1].n}`, "pri full", "player-play", { "data-act": "bd-move", "data-arg": `${t.k}>${AG.STAGES[1].k}` }); }
  else if (t.stage !== "done") { const nx = flowOf(t)[flowOf(t).findIndex((s) => s.k === t.stage) + 1]; acts = btn(`Pindah ke ${nx.n}`, "pri full", "arrow-right", { "data-act": "bd-move", "data-arg": `${t.k}>${nx.k}` }); }
  else acts = `<span class="row text-ok">${ic("circle-check")}Selesai · semua PR merged</span>`;
  return xcard({ cls: "td-panel", title: `<span class="row g8"><span style="color:${st.c};display:inline-flex">${ic(st.i, "s16")}</span>${st.n}</span>`, desc: "Stage sekarang", action: mode,
    foot: `<div class="pa">${acts}</div>`, body: `
    <div class="pb">
      ${per ? `<div class="row">${av(st.persona, 24)}<div class="grow"><b class="b5">Agent ${per.n}</b><div class="mut xs">${per.t} · Claude Code</div></div>${btn("", "ghost sm icon", "settings", { "data-go": "#/settings/workflows/superpowers", "aria-label": "Atur stage di workflow" })}</div>` : `<div class="mut sm">Stage ini tanpa agent. Dikerjakan manusia.</div>`}
      ${gs.length ? `<div><div class="td-lab">${ic("shield-check", "s14")}Gate keluar · ${gs.filter((g) => g.ok).length}/${gs.length} terpenuhi</div>${gs.map((g) => `<div class="td-gate"><span class="gi ${g.ok ? "ok" : "no"}">${ic(g.ok ? "check" : "x")}</span><span>${g.n}<small>${g.d}</small></span></div>`).join("")}</div>` : ""}
      <dl class="props" style="grid-template-columns:76px minmax(0,1fr)">
        <dt>Executor</dt><dd>${stack(ex.length ? ex : ["OO"])}</dd>
        <dt>Approver</dt><dd class="row">${stack(appr)}<span class="mut xs trunc">${appr.includes(AG.ME) ? "Termasuk Anda" : ""}</span></dd>
        <dt>Transisi</dt><dd class="sm">${per && st.k !== "brainstorm" ? "Tunggu approve manusia" : "Manual"}</dd>
      </dl>
    </div>` });
}
function livePanel(t) {
  const steps = ["Membaca konteks tiket dan PRD", "Menelusuri code graph: modul stok", "Menulis rencana implementasi", "Menulis tes validasi stok", "Implementasi di kasir-api", "Menjalankan test dan lint", "Membuka PR"];
  const cur = t.k === "KSR-26" ? 3 : 1;
  return xcard({ cls: "td-live", pad: false, title: `<span class="row g8"><span class="live busy"></span>Agent ${AG.PERSONAS[AG.stage(t.stage).persona || "dev"].n} sedang bekerja</span>`,
    desc: `Langkah ${cur + 1} dari ${steps.length} · <span class="tab-num" data-elapsed="761">12:41</span>`,
    action: btn("Terminal interaktif", "sm", "terminal-2", { "data-toast": "Terminal interaktif dibuka di runner Laptop Dewi" }), body: `
    <div style="padding:12px 16px 0"><div class="bar"><i style="width:${Math.round(((cur + 0.5) / steps.length) * 100)}%"></i></div></div>
    <div class="td-steps">${steps.map((s, i) => `<div class="td-step ${i < cur ? "done" : i === cur ? "now" : ""}">${i < cur ? ic("circle-check", "s14") : i === cur ? ic("loader-2", "s14 td-spin") : ic("circle", "s14")}<span class="trunc">${s}</span></div>`).join("")}</div>
    <div class="row wrap sm mut" style="padding:0 16px 10px;gap:6px 14px">${ic("device-laptop", "s14")}<span><b class="b5 fg">${t.runner || "Laptop Dewi"}</b> · Ubuntu 24.04 · Claude Code</span><span class="row g4">${ic("git-branch", "s14")}<code>feat/${t.k.toLowerCase()}-validasi-stok</code></span><span class="row g4">${ic("folder", "s14")}<code>.agentgate/wt/${t.k.toLowerCase()}</code></span></div>
    <div class="term" data-log><span class="d">14:02:11</span> baca <span class="b">src/inventory/stock.rs</span>
<span class="d">14:02:14</span> ag_trace <span class="b">checkout::submit</span> → 6 pemanggil
<span class="d">14:03:40</span> tulis tests/checkout_stock.rs <span class="g">+84</span>
<span class="d">14:04:02</span> cargo test checkout_stock <span class="y">· 3 gagal (diharapkan, TDD)</span></div>` });
}
const DEC_OPTS = [
  ["hint", "Beri petunjuk lalu lanjutkan", "Agent memperbaiki dengan petunjuk Anda. Hitungan gagal kembali ke 0.", "message-2"],
  ["self", "Perbaiki sendiri", "Anda mengerjakan perbaikan di Development, lalu tiket dites ulang.", "user"],
  ["req", "Ubah requirement", "Tiket kembali ke stage PRD untuk merevisi acceptance criteria.", "file-text"],
  ["accept", "Terima dengan catatan", "Tiket lanjut dan tiket Bug terpisah dibuat otomatis.", "circle-check"],
  ["cancel", "Batalkan tiket", "Tiket ditutup tanpa diselesaikan. Artifact tetap tersimpan.", "circle-x"],
];
const decOptions = (g) => `<div class="col">${DEC_OPTS.map(([v, n, d, i], x) => `<button type="button" class="opt ${x === 0 ? "on" : ""}" data-radio="${g}" data-v="${v}"><span class="radio ${x === 0 ? "on" : ""}"></span><span class="grow"><b>${x + 1}. ${n}</b><small>${d}</small></span><span class="faint">${ic(i)}</span></button>`).join("")}</div>`;
const aiCause = () => `<div class="td-ai"><div class="row">${av("qa", 20)}<b class="b5">Ringkasan penyebab dari Agent QA</b></div>
  <p>Total di struk dihitung ulang di <code>kasir-web</code> dengan pembulatan per item, sedangkan layar kasir memakai total dari <code>kasir-api</code> yang dibulatkan per transaksi. Tiga perbaikan Developer hanya mengubah template struk, jadi selisih Rp100 tetap muncul. Nama kasir kosong karena struk memakai sesi lama setelah diskon diterapkan.</p>
  <p class="mut sm">Saran: pakai satu sumber total dari API, lalu tes ulang 18 skenario struk.</p></div>`;
function decisionPanel(t) {
  return xcard({ cls: "dng td-dec", title: `<span class="row g8"><span class="text-dng">${ic("alert-triangle", "s16")}</span>Butuh keputusan Anda</span>`, desc: "Testing gagal 3 kali (batas 3). Putaran berhenti sampai Anda memilih tindakan.",
    foot: `<div class="x-foot end">${btn("Kirim keputusan", "pri", "send", { "data-act": "dec-submit", "data-arg": t.k })}</div>`, body: `
    <div class="col g16">${aiCause()}
      <div class="tblwrap"><table class="tbl dense"><thead><tr><th>Putaran</th><th>Hasil</th><th class="hide-sm">Diperbaiki oleh</th><th></th></tr></thead><tbody>
        ${[["1", "Total Rp48.900, seharusnya Rp49.000", "Agent Developer", "09:14"], ["2", "Total masih selisih, nama kasir kosong", "Agent Developer", "10:02"], ["3", "Total masih selisih Rp100", "Agent Developer", "10:51"]].map(([n, r, w, at]) => `<tr><td class="mono">#${n}</td><td><span class="row g4 text-dng">${ic("circle-x", "s14")}<span class="fg">${r}</span></span></td><td class="hide-sm mut">${w}</td><td class="mut nowrap">${at}</td></tr>`).join("")}</tbody></table></div>
      <div><div class="td-lab">Pilih tindakan</div>${decOptions("dec-inline")}</div>
      ${fld('Petunjuk atau catatan <span class="opt">(wajib untuk pilihan 1 dan 4)</span>', inp("", { area: true, ph: "Contoh: pakai total dari endpoint /api/cart/:id/total, jangan hitung ulang di web.", a: { "data-dec-note": "" } }))}</div>` });
}
function relSection(t) {
  const rs = REL[t.k] || [];
  const parent = t.parent ? T(t.parent) : null;
  const rows = `${parent ? `<div class="lrow" data-go="#/t/${parent.k}" style="min-height:44px"><span class="mut sm" style="width:110px;flex-shrink:0">Induk</span>${tref(parent.k)}<span class="grow trunc">${parent.t}</span>${statePill(parent.state)}</div>` : ""}
    ${rs.map(([r, k]) => { const x = T(k) || TRASH.find((y) => y.k === k); if (!x) return ""; const blocked = r === "blockedby" && x.stage !== "done";
      return `<div class="lrow" data-go="#/t/${k}" style="min-height:44px"><span class="row g4 sm ${blocked ? "text-dng" : "mut"}" style="width:110px;flex-shrink:0">${ic(REL_I[r], "s14")}${REL_N[r]}</span>${tref(k)}<span class="grow trunc">${x.t}</span><span class="hide-sm">${x.stage ? stagePill(x.stage) : ""}</span>${btn("", "ghost sm icon", "x", { "data-act": "rel-del", "data-arg": `${t.k}|${k}`, "aria-label": `Hapus relasi ke ${k}` })}</div>`; }).join("")}
    ${!rs.length && !parent ? `<div class="lrow mut" style="min-height:44px">Belum ada relasi. Relasi “memblokir” ditegakkan saat tiket dipindah.</div>` : ""}`;
  return xbox("Relasi", rows, { n: rs.length + (parent ? 1 : 0), act: btn("Tambah", "ghost sm", "plus", { "data-dialog": "relation-add", "data-arg": t.k }) });
}
function subSection(t) {
  if (t.type === "subtask") return "";
  const subs = AG.TICKETS.filter((x) => x.parent === t.k); const done = subs.filter((x) => x.stage === "done").length;
  return xbox("Subtask", `${subs.map((x) => `<div class="lrow" data-go="#/t/${x.k}" style="min-height:44px">${typeIcon("subtask")}<span class="mono mut">${x.k}</span><span class="grow trunc">${x.t}</span><span class="mut xs hide-sm">Dev → Review → Selesai</span>${stagePill(x.stage)}${av(x.who, 20)}</div>`).join("")}
    <button type="button" class="lrow add" style="min-height:42px;width:100%" data-dialog="new-ticket" data-arg="sub:${t.k}">${ic("plus", "s14")}Tambah subtask · workflow ringkas Dev → Review → Selesai</button>`, { n: `${done}/${subs.length}` });
}
function attSection(t) {
  const a = attOf(t);
  return xbox("Lampiran", `${a.map(([i, n, s, by]) => `<div class="lrow" style="min-height:42px">${ic(i, "s14 faint")}<span class="grow trunc">${n}</span><span class="mut xs hide-sm">${P(by).name.split(" ")[0]}</span><span class="mut xs">${s}</span>${btn("", "ghost sm icon", "download", { "data-toast": `${n} diunduh`, "aria-label": `Unduh ${n}` })}</div>`).join("")}
    <div style="padding:10px 12px;${a.length ? "border-top:1px solid var(--line)" : ""}"><button type="button" class="td-drop" data-act="att-add" data-arg="${t.k}">${ic("cloud-upload", "s14")} Seret berkas ke sini atau pilih berkas · disimpan di S3</button></div>`, { n: a.length });
}
function fieldsBox(t) {
  const cf = CUSTOM[t.k] || [["Toko pilot", "Semua toko"]];
  const est = { "KSR-24": ["8", "8", "2 hari"], "KSR-26": ["5", "5", "1,5 hari"], "KSR-30": ["2", "3", "4 jam"], "KSR-36": ["5", "5", "1,5 hari"] }[t.k] || [t.pts ?? "–", t.pts ?? "–", "1 hari"];
  const c = (f, o) => `<div data-fset="${f}">${AG.combo(o)}</div>`;
  const epics = ALL().filter((x) => x.type === "epic" || (t.type === "subtask" && x.type !== "subtask" && x.k !== t.k));
  return `${xbox("Detail", `<dl class="td-props">
    <dt>Penerima</dt><dd>${c("who", { options: PEOPLE_OPTS([{ v: "", l: "Belum ada penerima", lead: av(null, 20) }]), value: t.who || "", ph: "Pilih penerima", w: 300 })}</dd>
    <dt>Reporter</dt><dd class="row" style="padding-left:0">${av("OO", 20)}<span class="trunc">Olivia Owner</span></dd>
    <dt>Prioritas</dt><dd>${c("p", { options: [1, 2, 3, 4].map((p) => ({ v: p, l: AG.PRIORITY[p].n, lead: prioIcon(p) })), value: t.p, w: 220 })}</dd>
    <dt>Label</dt><dd>${c("lbl", { options: Object.keys(AG.LABELS).map((l) => ({ v: l, l, lead: `<span class="dot round" style="background:${AG.LABELS[l]}"></span>` })), value: t.lbl, multi: true, ph: "Tambah label", w: 240 })}</dd>
    <dt>Story point</dt><dd class="row"><span class="pill gray mono">${est[0]}</span><span class="mut xs row g4">${ic("sparkles", "s14")}usulan AI ${est[1]}</span></dd>
    <dt>Tenggat</dt><dd><button type="button" class="td-pick" data-toast="Pilih tanggal tenggat">${ic("calendar", "s14 faint")}${t.due || "Belum diatur"}</button></dd>
    <dt>Sprint</dt><dd>${c("sprint", { options: ["Backlog", "Sprint 3", "Sprint 4", "Sprint 5"].map((s) => ({ v: s, l: s, sub: s === "Sprint 4" ? "Aktif · 4–18 Nov" : s === "Sprint 5" ? "Mulai 19 Nov" : "" })), value: t.sprint || "Backlog", w: 240 })}</dd>
    <dt>${t.type === "subtask" ? "Induk" : "Epic"}</dt><dd>${c("epic", { options: [{ v: "", l: "Tanpa epic" }, ...epics.map((e) => ({ v: e.k, l: `${e.k} ${e.t}`, lead: typeIcon(e.type) }))], value: (t.type === "subtask" ? t.parent : t.epic) || "", w: 320 })}</dd>
    ${cf.map(([k, v]) => `<dt>${k}</dt><dd class="trunc">${v}</dd>`).join("")}
    <dt>Watcher</dt><dd>${c("watch", { options: PEOPLE_OPTS(), value: watchOf(t), multi: true, ph: "Tambah watcher", w: 300 })}</dd>
  </dl>`, { pad: true })}
  ${xbox("Estimasi AI vs aktual", `<div class="tblwrap"><table class="tbl td-est"><thead><tr><th>Stage</th><th>AI</th><th>Manusia</th><th>Aktual</th></tr></thead><tbody>
    ${[["PRD", "20 m", "30 m", "45 m"], ["Desain UI", "40 m", "1 j", "1 j 20 m"], ["Development", "3 j", "2 j", "4 j 10 m"], ["Code Review", "15 m", "45 m", "berjalan"], ["Testing", "30 m", "30 m", "–"]].map((r) => `<tr>${r.map((x, i) => `<td class="${i === 3 && x === "berjalan" ? "text-acc" : ""}">${x}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`,
    { n: est[2], desc: "Dikalibrasi dari 14 tiket serupa.", act: btn("Ubah", "ghost sm", "pencil", { "data-toast": "Angka estimasi bisa diedit per stage" }) })}`;
}

/* Sub-rute detail. */
const TD_TABS = [["", "Ringkasan", "file-text"], ["activity", "Aktivitas", "message-2"], ["artifacts", "Artifact", "folders"], ["pr", "PR & CI", "git-pull-request"], ["evidence", "Evidence", "photo"]];
const evCount = (t) => (t.k === "KSR-24" ? 2 : t.k === "KSR-30" ? 4 : 0);
function tdShell(t, tab, main, { full = false, alert = "" } = {}) {
  const bl = blockers(t);
  const pendArt = artsOf(t).find((a) => lastV(a).s === "rev");
  const rev = t.state === "review" ? `<div class="alert warn">${ic("eye")}<div class="grow"><b>Menunggu review Anda.</b> ${t.k === "KSR-24" ? "Review AI selesai dengan 3 catatan, CI hijau di 2 PR. Periksa artifact dan PR, lalu approve atau reject." : `${t.agent || "Artifact siap direview"}.`} ${pendArt ? `<a class="link" href="#/t/${t.k}/artifact/${pendArt.id}">Buka ${pendArt.n}</a>` : `<a class="link" href="#/t/${t.k}/artifacts">Lihat artifact</a>`}</div></div>` : "";
  const counts = { activity: countC(t), artifacts: artsOf(t).length, pr: t.k === "KSR-24" ? 2 : 0, evidence: evCount(t) };
  const pend = artsOf(t).filter((a) => lastV(a).s === "rev").length;
  return AG.shell({ nav: "app", project: "KSR", crumbs: AG.projectCrumb("KSR", t.k),
    actions: `${btn("", "ghost icon", "link", { "data-toast": `Link ${t.k} disalin`, "aria-label": "Salin link" })}${btn(watchOf(t).includes(AG.ME) ? "Diikuti" : "Ikuti", "hide-sm", "eye", { "data-act": "td-follow", "data-arg": t.k })}${btn("", "icon", "dots", { "data-menu": "td-more", "data-arg": t.k, "aria-label": "Aksi lain" })}`,
    body: `<div class="td">
      <div class="td-h">
        <div class="row wrap">${typeIcon(t.type)}<span class="mono mut">${t.k}</span>${t.parent ? `<span class="faint">·</span><span class="mut sm">subtask dari</span>${tref(t.parent)}` : t.epic ? `<span class="faint">·</span>${tref(t.epic)}` : ""}${statePill(t.state)}${t.pr ? `<span class="pill gray">${ic("git-pull-request")}PR ${t.pr}</span>` : ""}</div>
        <h1 class="td-title" contenteditable="true" spellcheck="false" aria-label="Judul tiket">${t.t}</h1>
        ${stageStrip(t)}
        ${bl.length ? `<div class="alert dng">${ic("lock")}<div class="grow"><b>Diblokir oleh ${bl.map((k) => `<a class="link" href="#/t/${k}">${k}</a>`).join(", ")}</b> · ${T(bl[0]).t} (${AG.stage(T(bl[0]).stage).n}). Tiket ini tidak bisa mulai sebelum tiket pemblokir selesai.</div></div>` : ""}
        ${rev}${alert}
      </div>
      <nav class="tabs td-tabs" aria-label="Bagian tiket">${TD_TABS.map(([k, n, i]) => `<a href="#/t/${t.k}${k ? "/" + k : ""}" class="${k === tab ? "on" : ""}" ${k === tab ? 'aria-current="page"' : ""}>${ic(i, "s14")}${n}${counts[k] ? `<span class="n">${counts[k]}</span>` : ""}${k === "artifacts" && pend ? `<span class="pill warn" style="height:18px;padding:0 6px">${pend} review</span>` : ""}</a>`).join("")}</nav>
      <div class="td-body ${full ? "full" : ""}">
        <div class="td-m">${main}</div>
        ${full ? "" : `<div class="td-s1">${stagePanel(t)}</div><div class="td-s2">${fieldsBox(t)}</div>`}
      </div></div>` });
}
function notFound(id) { return AG.shell({ nav: "app", project: "KSR", crumbs: AG.projectCrumb("KSR", id), body: empty("trash", `${id} tidak ditemukan`, "Tiket mungkin sudah dipindah ke sampah.", btn("Buka sampah", "", "trash", { "data-go": "#/p/KSR/trash" })) }); }

function overview(id) {
  const t = T(id); if (!t) return notFound(id);
  const top = t.state === "keputusan" ? decisionPanel(t) : t.state === "jalan" ? livePanel(t) : "";
  const est = t.stage === "backlog" ? `<div class="alert">${ic("sparkles")}<div class="grow"><b>Usulan estimasi AI: ${t.pts || 3} poin · sekitar 1,5 hari.</b> <span>AI 4 j, manusia 5 j. Dasar: 3 tiket serupa (KSR-21, KSR-22) dan 6 berkas di <code>kasir-api/src/tax</code> dari code graph.</span></div>${btn("Terima", "sm", "check", { "data-toast": "Estimasi AI dipakai" })}</div>` : "";
  const recent = actItems(t, "c").slice(0, 2);
  return tdShell(t, "", `${top}
    ${xbox("Deskripsi", `<div data-draft="desc:${t.k}">${AG.editor({ html: S.draft["desc:" + t.k] ?? descOf(t), h: "120px", save: true, footer: `<span class="mut hide-sm">Diubah Olivia Owner · 2 jam lalu</span>` })}</div>`, { cls: "td-desc" })}
    ${subSection(t)}
    ${relSection(t)}
    ${attSection(t)}
    ${xbox("Komentar terbaru", recent.map((x) => { const c = x.c; return `<div class="tc-mini">${av(c.w, 24)}<div class="tc-main"><div class="tc-meta"><b class="b5">${authorName(c.w)}</b><span class="faint xs">${c.at} lalu</span></div><div class="tc-body">${c.html}</div></div></div>`; }).join("") || `<div class="lrow mut">Belum ada komentar.</div>`,
      { n: countC(t), act: seeAll(`#/t/${t.k}/activity`) })}`, { alert: est });
}

/* ---------- Aktivitas & komentar ---------- */
const authorName = (w) => (AG.PERSONAS[w] ? `Agent ${AG.PERSONAS[w].n}` : P(w)?.name || w);
function actItems(t, f = "all") {
  const cs = cmts(t).filter((c) => !c.pinned || f !== "all").map((c) => ({ kind: AG.PERSONAS[c.w] ? "ca" : "c", ts: c.ts, c }));
  const hs = histOf(t).map((h) => ({ kind: h.ag ? "a" : "h", ts: h.ts, h }));
  const keep = { all: () => true, c: (x) => x.kind === "c" || x.kind === "ca", h: (x) => x.kind === "h", a: (x) => x.kind === "a" || x.kind === "ca" }[f];
  const old = S.actOld[t.k];
  return [...cs, ...hs].filter(keep).sort((a, b) => (old ? b.ts - a.ts : a.ts - b.ts));
}
const edArea = (draftKey, o) => `<div data-draft="${draftKey}" class="tc-ed">${AG.editor({ compact: true, ...o, html: S.draft[draftKey] ?? o.html ?? "" })}</div>`;
function cmtHtml(t, c, depth = 0) {
  const ai = !!AG.PERSONAS[c.w];
  if (c.deleted) return `<div class="tc-c" id="${c.id}">${av(null, depth ? 24 : 28)}<div class="tc-main"><span class="tc-del">Komentar dihapus</span>${c.replies.length ? `<div class="tc-replies">${c.replies.map((r) => cmtHtml(t, r, 1)).join("")}</div>` : ""}</div></div>`;
  const arg = `${t.k}|${c.id}`;
  const body = S.editC === c.id
    ? `<div class="tc-edit">${edArea("edit:" + c.id, { html: c.html })}<div class="cfoot">${btn("Batal", "sm", "", { "data-act": "cm-edit-cancel" })}${btn("Simpan", "pri sm", "check", { "data-act": "cm-edit-save", "data-arg": arg })}</div></div>`
    : `<div class="tc-body">${c.html}</div>`;
  const reacts = Object.entries(c.reacts).filter(([, v]) => v.length).map(([e, v]) => `<button type="button" class="tc-react ${v.includes(AG.ME) ? "on" : ""}" data-act="cm-react" data-arg="${arg}|${e}" title="${esc(v.map((p) => authorName(p)).join(", "))}" aria-label="Reaksi ${e}, ${v.length} orang">${e}<span>${v.length}</span></button>`).join("");
  return `<div class="tc-c" id="${c.id}">${av(c.w, depth ? 24 : 28)}<div class="tc-main">
    <div class="tc-meta"><b class="b5">${authorName(c.w)}</b>${ai ? pill("AI", "acc") : ""}<span class="faint xs">${c.at}${c.at === "baru saja" ? "" : " lalu"}</span>${c.edited ? `<span class="faint xs">· diedit</span>` : ""}${c.pinned ? `<span class="row g4 xs text-acc">${ic("pinned", "s12")}Disematkan</span>` : ""}<span class="grow"></span>${btn("", "ghost sm icon", "dots", { "data-menu": "cm-more", "data-arg": arg, "aria-label": "Aksi komentar" })}</div>
    ${body}
    ${c.files.length ? `<div class="row wrap g4">${c.files.map((f) => `<button type="button" class="tc-file" data-toast="${esc(f.split(" ·")[0])} diunduh">${ic("paperclip", "s14")}${f}</button>`).join("")}</div>` : ""}
    <div class="tc-acts">${reacts}<button type="button" class="tc-react add" data-menu="cm-emoji" data-arg="${arg}" aria-label="Tambah reaksi">${ic("mood-plus", "s14")}</button>
      ${depth ? "" : `<button type="button" class="tc-link" data-act="cm-reply" data-arg="${arg}">${ic("message-reply", "s14")}Balas${c.replies.length ? ` · ${c.replies.length}` : ""}</button>`}
      <button type="button" class="tc-link" data-act="cm-link" data-arg="${arg}">${ic("link", "s14")}Salin tautan</button></div>
    ${!depth && (c.replies.length || S.reply === c.id) ? `<div class="tc-replies">${c.replies.map((r) => cmtHtml(t, r, 1)).join("")}
      ${S.reply === c.id ? `<div class="tc-reply">${edArea("reply:" + c.id, { ph: `Balas ${authorName(c.w)}…` })}<div class="cfoot">${btn("Batal", "sm", "", { "data-act": "cm-reply-cancel" })}${btn("Balas", "pri sm", "send", { "data-act": "cm-reply-send", "data-arg": arg })}</div></div>` : ""}</div>` : ""}
  </div></div>`;
}
const histHtml = (h) => `<div class="tc-h ${h.ag ? "ag" : ""}"><span class="tile">${ic(h.i, "s14")}</span><span class="grow" style="padding-top:2px">${h.h}</span><span class="faint xs nowrap" style="padding-top:4px">${h.at}</span></div>`;
function activity(id) {
  const t = T(id); if (!t) return notFound(id);
  const f = S.actF[t.k] || "all"; const items = actItems(t, f); const pins = cmts(t).filter((c) => c.pinned && !c.deleted);
  const F = [["all", "Semua"], ["c", "Komentar", cmts(t).length], ["h", "Riwayat", histOf(t).filter((h) => !h.ag).length], ["a", "Aktivitas agent", histOf(t).filter((h) => h.ag).length + cmts(t).filter((c) => AG.PERSONAS[c.w]).length]];
  return tdShell(t, "activity", xbox("Aktivitas & komentar", `
    <div class="tc-compose" data-compose="${t.k}">${edArea("new:" + t.k, { ph: "Tulis komentar… Ketik @ untuk menyebut orang, tiket, atau @agent untuk meminta agent." })}
      <div class="cfoot"><span class="mut xs grow hide-sm">${ic("info-circle", "s14")} Menyebut <span class="x-mention">@agent-qa</span> mengirim permintaan ke agent di runner tim.</span><span class="faint xs hide-sm">Ctrl Enter</span>${btn("Kirim", "pri sm", "send", { "data-act": "cm-send", "data-arg": t.k })}</div></div>
    <div class="tc-bar"><nav class="tabs" aria-label="Filter aktivitas">${F.map(([k, n, c]) => `<button type="button" class="${f === k ? "on" : ""}" data-act="act-f" data-arg="${t.k}|${k}">${n}${c != null ? `<span class="n">${c}</span>` : ""}</button>`).join("")}</nav>
      ${btn(S.actOld[t.k] ? "Terlama dulu" : "Terbaru dulu", "ghost sm", S.actOld[t.k] ? "sort-ascending" : "sort-descending", { "data-act": "act-sort", "data-arg": t.k })}</div>
    ${f === "all" && pins.length ? `<div class="tc-pins"><div class="lab">${ic("pinned", "s14")}Disematkan</div>${pins.map((c) => cmtHtml(t, c)).join("")}</div>` : ""}
    <div class="tc-list">${items.map((x) => (x.c ? cmtHtml(t, x.c) : histHtml(x.h))).join("") || `<div style="padding:16px">${empty("message-2", "Belum ada aktivitas", "Komentar dan perubahan tiket muncul di sini.")}</div>`}</div>`,
    { desc: "Komentar, riwayat perubahan, dan aktivitas agent di tiket ini." }));
}
function sendComment(k, html, text) {
  const t = T(k); const c = C(AG.ME, html, "baru saja", 0); cmts(t).push(c);
  const ag = text.match(/@agent-(\w+)/);
  if (ag && AG.PERSONAS[ag[1]]) { cmts(t).push(C(ag[1], "Diterima. Saya kerjakan di runner Laptop Dewi dan membalas di sini setelah selesai.", "baru saja", 0)); log(k, "robot", `<b>Agent ${AG.PERSONAS[ag[1]].n}</b> menerima permintaan dari komentar`, true); }
  return { c, ag: ag && AG.PERSONAS[ag[1]] ? AG.PERSONAS[ag[1]].n : null };
}
const edOf = (el, sel) => el.closest(sel)?.querySelector(".ed");
const edHtml = (ed) => (ed.classList.contains("mdmode") ? AG.mdToHtml(ed.querySelector(".md").value) : ed.querySelector(".area").innerHTML);
const edText = (ed) => (ed.classList.contains("mdmode") ? ed.querySelector(".md").value : ed.querySelector(".area").textContent);
const edEmpty = (ed) => !edText(ed).trim() && !ed.querySelector(".area .pill");
function activityAfter(root, p) {
  const t = T(p.id); if (!t) return; root = root.querySelector("#app");
  /* Ketik @ di editor komentar: buka saran sebutan. */
  root.addEventListener("input", (e) => {
    const area = e.target.closest(".tc-ed .area"); if (!area || e.data !== "@") return;
    document.execCommand("delete"); area.closest(".ed").querySelector('[data-ed="mention"]').click();
  });
  root.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" || !(e.ctrlKey || e.metaKey)) return;
    const w = e.target.closest(".tc-compose,.tc-reply,.tc-edit"); if (!w) return; e.preventDefault();
    w.querySelector(".cfoot .btn.pri")?.click();
  });
  const id = (location.hash.match(/[?&]c=(c\d+)/) || [])[1];
  if (id) { const el = root.querySelector("#" + id); if (el) { el.scrollIntoView({ block: "center" }); el.classList.add("hl"); } }
  if (S.focusNew) { S.focusNew = false; root.querySelector(".tc-reply .area,.tc-edit .area")?.focus(); }
}
AG.action("act-f", (el, a) => { const [k, f] = a.split("|"); S.actF[k] = f; rr(); });
AG.action("act-sort", (el, k) => { S.actOld[k] = !S.actOld[k]; rr(); });
AG.action("cm-send", (el, k) => {
  const ed = edOf(el, ".tc-compose"); if (edEmpty(ed)) { ed.querySelector(".area").focus(); return AG.toast("Komentar masih kosong", "dng"); }
  const r = sendComment(k, edHtml(ed), edText(ed)); S.draft["new:" + k] = ""; S.actF[k] = S.actF[k] === "h" ? "all" : S.actF[k];
  AG.rerender(); AG.toast(r.ag ? `Permintaan dikirim ke Agent ${r.ag}` : "Komentar terkirim");
});
AG.action("cm-reply", (el, a) => { const [, id] = a.split("|"); S.reply = id; S.focusNew = true; rr(); });
AG.action("cm-reply-cancel", () => { S.reply = null; AG.rerender(); });
AG.action("cm-reply-send", (el, a) => {
  const [k, id] = a.split("|"); const ed = edOf(el, ".tc-reply"); if (edEmpty(ed)) return AG.toast("Balasan masih kosong", "dng");
  const { c } = findC(k, id); c.replies.push(C(AG.ME, edHtml(ed), "baru saja", 0)); S.reply = null; S.draft["reply:" + id] = "";
  const ag = edText(ed).match(/@agent-(\w+)/); if (ag && AG.PERSONAS[ag[1]]) c.replies.push(C(ag[1], "Siap, saya tindak lanjuti di thread ini.", "baru saja", 0));
  rr(); AG.toast(`Balasan untuk ${authorName(c.w)} terkirim`);
});
AG.action("cm-react", (el, a, e) => {
  e.stopPropagation(); const [k, id, emo] = a.split("|"); const { c } = findC(k, id); const list = (c.reacts[emo] ||= []);
  const i = list.indexOf(AG.ME); i < 0 ? list.push(AG.ME) : list.splice(i, 1); rr();
});
AG.menu("cm-emoji", (arg) => `<div class="mlab">Reaksi</div><div style="display:grid;grid-template-columns:repeat(6,32px);gap:2px;padding:2px 4px 4px">${AG.EMOJI.map((e) => `<button class="mi" style="justify-content:center;padding:0;font-size:16px" data-act="cm-react" data-arg="${arg}|${e}" aria-label="Reaksi ${e}">${e}</button>`).join("")}</div>`);
AG.menu("cm-more", (arg) => {
  const [k, id] = arg.split("|"); const { c, parent } = findC(k, id); if (!c) return "";
  return `${parent ? "" : `<button class="mi" data-act="cm-pin" data-arg="${arg}">${ic(c.pinned ? "pin" : "pinned")}${c.pinned ? "Lepas sematan" : "Sematkan di atas"}</button>`}
    <button class="mi" data-act="cm-link" data-arg="${arg}">${ic("link")}Salin tautan komentar</button>
    <button class="mi" data-act="cm-quote" data-arg="${arg}">${ic("blockquote")}Kutip di komentar baru</button>
    ${c.w === AG.ME ? `<button class="mi" data-act="cm-edit" data-arg="${arg}">${ic("pencil")}Edit</button>` : ""}
    <div class="msep"></div><button class="mi dng" data-dialog="cm-del" data-arg="${arg}">${ic("trash")}Hapus${c.w === AG.ME ? "" : '<span class="tail">Owner</span>'}</button>`;
});
AG.action("cm-pin", (el, a) => { const [k, id] = a.split("|"); const { c } = findC(k, id); c.pinned = !c.pinned; rr(); AG.toast(c.pinned ? "Komentar disematkan di atas" : "Sematan dilepas"); });
AG.action("cm-link", (el, a) => {
  const [k, id] = a.split("|"); const url = `${location.href.split("#")[0]}#/t/${k}/activity?c=${id}`;
  try { navigator.clipboard?.writeText(url).catch(() => {}); } catch {}
  AG.toast("Tautan komentar disalin"); const n = document.getElementById(id); if (n) { n.classList.remove("hl"); void n.offsetWidth; n.classList.add("hl"); }
});
AG.action("cm-quote", (el, a) => { const [k, id] = a.split("|"); const { c } = findC(k, id); S.draft["new:" + k] = `<blockquote>${authorName(c.w)}: ${c.html.replace(/<[^>]+>/g, "").slice(0, 120)}</blockquote><p></p>`; AG.rerender(); document.querySelector(".tc-compose .area")?.focus(); });
AG.action("cm-edit", (el, a) => { const [, id] = a.split("|"); S.editC = id; S.focusNew = true; rr(); });
AG.action("cm-edit-cancel", () => { S.editC = null; AG.rerender(); });
AG.action("cm-edit-save", (el, a) => { const [k, id] = a.split("|"); const ed = edOf(el, ".tc-edit"); if (edEmpty(ed)) return AG.toast("Komentar tidak boleh kosong", "dng"); const { c } = findC(k, id); c.html = edHtml(ed); c.edited = true; S.editC = null; delete S.draft["edit:" + id]; rr(); AG.toast("Komentar diperbarui"); });
AG.dialog("cm-del", (arg) => { const [k, id] = arg.split("|"); const { c } = findC(k, id); return { icon: "trash", tone: "dng", title: "Hapus komentar?", desc: c?.replies.length ? "Balasan tetap tampil. Komentar diganti tulisan “Komentar dihapus”." : "Komentar dihapus untuk semua orang dan tercatat di riwayat.",
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Hapus", "dngs", "trash", { "data-act": "cm-del-go", "data-arg": arg })}` }; });
AG.action("cm-del-go", (el, a) => { const [k, id] = a.split("|"); const { c, list } = findC(k, id); if (c.replies.length) { c.deleted = true; c.pinned = false; } else list.splice(list.indexOf(c), 1); log(k, "trash", `<b>${P(AG.ME).name}</b> menghapus komentar ${authorName(c.w)}`); AG.closeDialog(); rr(); AG.toast("Komentar dihapus"); });

/* ---------- Artifact: navigator ---------- */
const artIcon = (a) => (a.n.endsWith(".html") ? "file-code" : a.id === "review" ? "git-compare" : a.id === "brainstorm" ? "bulb" : "file-text");
function artifacts(id) {
  const t = T(id); if (!t) return notFound(id);
  const arts = artsOf(t);
  const cur = arts.find((a) => a.id === S.art[t.k]) || arts.find((a) => lastV(a).s === "rev") || arts[0];
  const vi = Math.min(S.artV[`${t.k}:${cur.id}`] ?? cur.v.length - 1, cur.v.length - 1); const v = cur.v[vi];
  const pend = arts.filter((a) => lastV(a).s === "rev").length;
  const groups = AG.STAGES.filter((s) => arts.some((a) => a.st === s.k));
  const nav = xbox("Artifact", `
    <div class="an-s"><label class="fsearch" style="min-width:0;width:100%;flex:1">${ic("search", "s14")}<input placeholder="Cari artifact…" data-an-q aria-label="Cari artifact"></label></div>
    <div class="an-list">${groups.map((s) => `<div class="an-grp"><div class="an-g">${stageIcon(s.k, "s14")}${s.n}<span class="faint">${arts.filter((a) => a.st === s.k).length}</span></div>
      ${arts.filter((a) => a.st === s.k).map((a) => { const l = lastV(a); return `<button type="button" class="an-i ${a === cur ? "on" : ""}" data-act="an-pick" data-arg="${t.k}|${a.id}" data-q="${esc(a.n)}">${ic(artIcon(a), "s14")}<span class="grow" style="min-width:0"><b>${a.n}</b><small>v${a.v.length} · ${authorName(l.by)} · ${l.at}</small></span><span class="an-dot ${l.s}" title="${ART_ST[l.s][0]}"></span></button>`; }).join("")}</div>`).join("")}
      <div class="mut sm" data-an-none style="padding:12px;display:none">Tidak ada artifact yang cocok.</div></div>`,
    { n: arts.length, cls: "an-nav", desc: pend ? `<span class="row g4 text-warn">${ic("eye", "s14")}${pend} menunggu review</span>` : "Semua sudah direview" });
  const isCode = cur.n.endsWith(".html");
  const pv = xcard({ cls: "an-pv", pad: false, title: `<span class="row g8">${ic(artIcon(cur), "s16")}${cur.n}</span>`, desc: `Stage ${AG.stage(cur.st).n} · ${authorName(v.by)} · ${v.at}`,
    action: `<span class="x-vers" role="group" aria-label="Versi">${cur.v.map((_, i) => `<button type="button" class="x-ver ${i === vi ? "on" : ""}" data-act="an-ver" data-arg="${t.k}|${cur.id}|${i}">v${i + 1}</button>`).join("")}</span>${artPill(v.s)}`,
    body: `${v.s === "rev" ? `<div class="alert warn" style="margin:12px 16px 0">${ic("eye")}<div class="grow"><b>v${vi + 1} menunggu review Anda.</b> ${v.note}</div></div>` : v.note ? `<div class="row sm mut" style="padding:10px 16px 0">${ic("info-circle", "s14")}${v.note}</div>` : ""}
    ${isCode ? `<div style="padding:16px">${md(v.md)}</div>` : `<div class="prose">${md(v.md)}</div>`}`,
    foot: `<div class="x-foot">${btn("Buka editor", "pri", "pencil", { "data-go": `#/t/${t.k}/artifact/${cur.id}` })}
      ${cur.v.length > 1 ? btn(`Bandingkan v${Math.max(1, vi)} → v${vi + 1}`, "", "git-compare", { "data-act": "an-diff", "data-arg": `${t.k}|${cur.id}|${vi}` }) : ""}
      ${btn("", "ghost icon", "download", { "data-toast": `${cur.n} v${vi + 1} diunduh`, "aria-label": "Unduh artifact" })}<span class="grow"></span>
      ${v.s === "rev" ? `${btn("Reject", "dng", "x", { "data-dialog": "art-reject", "data-arg": `${t.k}|${cur.id}|${vi}` })}${btn("Approve", "ok", "check", { "data-act": "art-approve", "data-arg": `${t.k}|${cur.id}|${vi}` })}` : ""}</div>` });

  return tdShell(t, "artifacts", `<div class="an">${nav}${pv}</div>`, { full: true });
}
function artifactsAfter(root) {
  const q = root.querySelector("[data-an-q]"); if (!q) return;
  q.addEventListener("input", () => {
    const v = q.value.toLowerCase().trim(); let any = false;
    root.querySelectorAll(".an-grp").forEach((g) => { let n = 0; g.querySelectorAll(".an-i").forEach((i) => { const ok = !v || i.dataset.q.toLowerCase().includes(v); i.style.display = ok ? "" : "none"; n += ok; }); g.style.display = n ? "" : "none"; any ||= n > 0; });
    root.querySelector("[data-an-none]").style.display = any ? "none" : "";
  });
}
AG.action("an-pick", (el, a) => { const [k, id] = a.split("|"); S.art[k] = id; rr(); });
AG.action("an-ver", (el, a) => { const [k, id, v] = a.split("|"); S.art[k] = id; S.artV[`${k}:${id}`] = +v; rr(); });
AG.action("an-diff", (el, a) => { const [k, id, v] = a.split("|"); S.ae[`${k}:${id}`] = { v: +v, diff: Math.max(0, +v - 1), side: "ver" }; location.hash = `#/t/${k}/artifact/${id}`; });
function artSetStatus(k, id, vi, s, note) {
  const a = artsOf(T(k)).find((x) => x.id === id); const v = a.v[vi]; v.s = s; v.note = note;
  log(k, s === "ok" ? "check" : "x", `<b>${P(AG.ME).name}</b> ${s === "ok" ? "approve" : "reject"} <b>${a.n} v${vi + 1}</b>${s === "rej" && note ? `: “${esc(note.replace(/^Ditolak [^:]+: /, ""))}”` : ""}`);
  return a;
}
AG.action("art-approve", (el, a) => { const [k, id, vi] = a.split("|"); const x = artSetStatus(k, id, +vi, "ok", `Disetujui ${P(AG.ME).name} · baru saja`); rr(); AG.toast(`${x.n} v${+vi + 1} disetujui`); });
AG.dialog("art-reject", (arg) => { const [k, id, vi] = arg.split("|"); const a = artsOf(T(k)).find((x) => x.id === id); return { icon: "x", tone: "dng", title: `Reject ${a.n} v${+vi + 1}`, desc: `Agent ${AG.PERSONAS[a.by]?.n || ""} merevisi berdasarkan catatan Anda dan menyimpan v${a.v.length + 1}.`,
  body: fld("Catatan untuk agent", inp("", { area: true, ph: "Contoh: tambahkan skenario callback ganda dan batas waktu 15 menit.", a: { "data-rej": "" } }), "Wajib diisi. Komentar inline yang terbuka ikut dikirim."),
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Reject dan minta revisi", "dngs", "x", { "data-act": "art-reject-go", "data-arg": arg })}` }; });
AG.action("art-reject-go", (el, a) => {
  const [k, id, vi] = a.split("|"); const ta = el.closest(".dialog").querySelector("[data-rej]");
  if (!ta.value.trim()) { ta.classList.add("err"); ta.focus(); return AG.toast("Catatan reject wajib diisi", "dng"); }
  const x = artSetStatus(k, id, +vi, "rej", `Ditolak ${P(AG.ME).name}: ${ta.value.trim()}`);
  AG.closeDialog(); rr(); AG.toast(`${x.n} v${+vi + 1} ditolak · agent merevisi`);
});

/* ---------- Editor artifact (halaman sendiri) ---------- */
function markup(html, list) {
  return list.reduce((h, c, i) => (c.done ? h : h.replace(esc(c.q), `<mark class="ae-mark" data-ci="${i}" title="Komentar inline">${esc(c.q)}</mark>`)), html);
}
function aeSide(t, a, st) {
  const key = `${t.k}:${a.id}`; const cm = acmOf(t.k, a.id); const open = cm.filter((c) => !c.done).length;
  const vers = a.v.map((v, i) => i).reverse().map((i) => { const v = a.v[i];
    return `<div class="ae-v ${i === st.v ? "on" : ""}"><div class="row"><b class="mono b5">v${i + 1}</b>${artPill(v.s)}<span class="grow"></span><span class="mut xs">${v.at}</span></div>
      <div class="row sm mut">${av(v.by, 18)}<span class="trunc">${authorName(v.by)}</span></div>${v.note ? `<div class="mut sm">${esc(v.note)}</div>` : ""}
      <div class="row g4">${i === st.v ? `<span class="mut xs">Sedang dibuka</span>` : btn("Buka", "sm", "eye", { "data-act": "ae-ver", "data-arg": `${key}|${i}` })}${i !== st.v ? btn(`Bandingkan dengan v${st.v + 1}`, "ghost sm", "git-compare", { "data-act": "ae-diff", "data-arg": `${key}|${i}` }) : ""}</div></div>`; }).join("");
  const cms = cm.map((c, i) => `<div class="ae-ic ${c.done ? "done" : ""}" data-ci="${i}"><blockquote>${esc(c.q)}</blockquote>
      <div class="row">${av(c.w, 20)}<b class="b5 trunc">${authorName(c.w)}</b><span class="faint xs">${c.at}</span><span class="grow"></span>${c.done ? pill("Selesai", "ok") : ""}</div><div>${esc(c.t)}</div>
      ${c.re.length ? `<div class="re">${c.re.map(([w, x]) => `<div class="sm"><b class="b5">${authorName(w)}</b> <span class="mut">${esc(x)}</span></div>`).join("")}</div>` : ""}
      <div class="row g4">${btn(c.done ? "Buka lagi" : "Selesaikan", "ghost sm", c.done ? "arrow-back-up" : "check", { "data-act": "ae-ic-done", "data-arg": `${key}|${i}` })}${btn("Balas", "ghost sm", "message-reply", { "data-act": "ae-ic-reply", "data-arg": `${key}|${i}` })}</div>
      <div class="row g4" data-ic-reply="${i}" style="display:none">${inp("", { ph: "Tulis balasan…", a: { "aria-label": "Balasan" } })}${btn("Kirim", "pri sm", "send", { "data-act": "ae-ic-send", "data-arg": `${key}|${i}` })}</div></div>`).join("");
  return `<div data-tabs-root><nav class="tabs" aria-label="Panel artifact"><button type="button" class="${st.side !== "cm" ? "on" : ""}" data-tab="ver">${ic("versions", "s14")}Versi<span class="n">${a.v.length}</span></button><button type="button" class="${st.side === "cm" ? "on" : ""}" data-tab="cm">${ic("message-2", "s14")}Komentar inline<span class="n">${open}</span></button></nav>
    <div data-panel="ver" class="${st.side !== "cm" ? "on" : ""}" style="gap:0">${vers}</div>
    <div data-panel="cm" class="${st.side === "cm" ? "on" : ""}" style="gap:0">
      <div style="padding:12px 16px;border-bottom:1px solid var(--line)" class="col g8">${btn("Komentari teks terpilih", "full", "message-circle", { "data-act": "ae-ic-new", "data-arg": key })}<span class="mut xs">Pilih teks di editor, lalu klik tombol ini.</span><div data-ic-form></div></div>
      ${cms || `<div style="padding:16px" class="mut sm">Belum ada komentar inline.</div>`}</div></div>`;
}
function artEditor(id, name) {
  const t = T(id); if (!t) return notFound(id);
  const a = artsOf(t).find((x) => x.id === name);
  if (!a) return AG.shell({ nav: "app", project: "KSR", crumbs: AG.projectCrumb("KSR", id), body: empty("file-search", `Artifact ${esc(name)} tidak ada`, `Tiket ${id} tidak punya artifact dengan nama itu.`, btn("Lihat semua artifact", "", "folders", { "data-go": `#/t/${id}/artifacts` })) });
  const key = `${t.k}:${a.id}`; const st = (S.ae[key] ||= { v: a.v.length - 1, diff: null, side: "ver" }); st.v = Math.min(st.v, a.v.length - 1);
  const v = a.v[st.v]; const latest = st.v === a.v.length - 1; const nx = a.v.length + 1;
  let main;
  if (st.diff != null) {
    main = `<div class="alert gray">${ic("git-compare")}<div class="grow">Membandingkan <b>v${st.diff + 1}</b> dengan <b>v${st.v + 1}</b>. Baris hijau ditambahkan, merah dihapus.</div>${btn("Tutup perbandingan", "sm", "x", { "data-act": "ae-diff-off", "data-arg": key })}</div>
      <div class="row wrap sm mut">Bandingkan dari ${a.v.map((_, i) => (i !== st.v ? `<button type="button" class="x-ver ${i === st.diff ? "on" : ""}" data-act="ae-diff" data-arg="${key}|${i}">v${i + 1}</button>` : "")).join("")} ke <b class="mono fg">v${st.v + 1}</b></div>
      ${diffHtml(a.v[st.diff].md, v.md, `${a.n} · v${st.diff + 1} → v${st.v + 1}`)}`;
  } else {
    main = `${!latest ? `<div class="alert warn" style="margin:12px 16px">${ic("history")}<div class="grow">Anda melihat <b>v${st.v + 1}</b> (${ART_ST[v.s][0].toLowerCase()}). Menyimpan akan membuat <b>v${nx}</b> dari versi ini.</div>${btn(`Ke v${a.v.length}`, "sm", "", { "data-act": "ae-ver", "data-arg": `${key}|${a.v.length - 1}` })}</div>` : ""}
      <div data-ae-ed>${AG.editor({ full: true, html: markup(AG.mdToHtml(v.md), latest ? acmOf(t.k, a.id) : []) })}</div>`;
  }
  const rev = v.s === "rev";
  const bar = sbar(rev ? `Approve v${st.v + 1}` : `Simpan sebagai v${nx}`, rev ? "ae-approve" : "ae-save", { arg: `${key}|${st.v}`, cancel: false,
    msg: `<span data-ae-msg>${rev ? `v${st.v + 1} menunggu review Anda · cukup satu approver` : `v${st.v + 1} ${ART_ST[v.s][0].toLowerCase()}`}</span>`,
    extra: `${btn(rev ? "Reject" : "Minta revisi agent", "dng", "x", { "data-dialog": "art-reject", "data-arg": `${key.replace(":", "|")}|${st.v}` })}${rev ? btn(`Simpan sebagai v${nx}`, "", "check", { "data-act": "ae-save", "data-arg": `${key}|${st.v}` }) : ""}` });
  return AG.shell({ nav: "app", project: "KSR", crumbs: [AG.projectCrumb("KSR", "")[0], `<a href="#/t/${t.k}">${t.k}</a>`, `<a href="#/t/${t.k}/artifacts">Artifact</a>`, a.n],
    actions: `${btn("", "ghost icon", "download", { "data-toast": `${a.n} v${st.v + 1} diunduh`, "aria-label": "Unduh" })}${btn("Semua artifact", "", "folders", { "data-go": `#/t/${t.k}/artifacts` })}`,
    body: `${head(`${a.n} <span class="mono mut" style="font-size:14px;font-weight:500">v${st.v + 1}</span>`, `Artifact stage ${AG.stage(a.st).n} untuk ${t.k} · ${esc(t.t)} · dibuat ${authorName(a.by)}`, "", `<span class="tile lg">${ic(artIcon(a), "s18")}</span>`)}
      <div class="ae"><div class="ae-main">${xcard({ cls: `ae-card ${st.diff != null ? "diff" : ""}`, pad: st.diff != null, title: st.diff != null ? "Perbandingan versi" : "Isi artifact",
        desc: `<span class="row wrap g8" style="margin-top:4px">${artPill(v.s)}${stagePill(a.st)}<span class="row g4">${av(v.by, 18)}${authorName(v.by)} · ${v.at}</span></span>`, body: main, foot: bar })}</div>
      ${xcard({ cls: "ae-side", pad: false, title: "Versi & komentar", body: aeSide(t, a, st) })}</div>` });
}
function artEditorAfter(root, p) {
  const t = T(p.id); const a = t && artsOf(t).find((x) => x.id === p.name); if (!a) return;
  const app = root.querySelector("#app"); const key = `${t.k}:${a.id}`;
  app.addEventListener("click", (e) => {
    const tb = e.target.closest(".ae-side [data-tab]"); if (tb) S.ae[key].side = tb.dataset.tab;
    const mk = e.target.closest(".ae-mark");
    if (mk) { S.ae[key].side = "cm"; app.querySelector('.ae-side [data-tab="cm"]')?.click(); app.querySelectorAll(".ae-ic,.ae-mark").forEach((x) => x.classList.remove("on")); mk.classList.add("on"); const c = app.querySelector(`.ae-ic[data-ci="${mk.dataset.ci}"]`); c?.classList.add("on"); c?.scrollIntoView({ block: "nearest" }); }
    const ci = e.target.closest(".ae-ic"); if (ci && !e.target.closest("button,input")) { const m = app.querySelector(`.ae-mark[data-ci="${ci.dataset.ci}"]`); app.querySelectorAll(".ae-ic,.ae-mark").forEach((x) => x.classList.remove("on")); ci.classList.add("on"); if (m) { m.classList.add("on"); m.scrollIntoView({ block: "center" }); } }
  });
  const area = app.querySelector("[data-ae-ed] .area"), msg = app.querySelector("[data-ae-msg]");
  if (area && msg) { const on = () => { msg.textContent = "Ada perubahan yang belum disimpan"; msg.parentElement.classList.add("text-warn"); }; area.addEventListener("input", on); app.querySelector("[data-ae-ed] .md")?.addEventListener("input", on); }
}
const aeRefreshSide = (key) => { const [k, id] = key.split(":"); const t = T(k), a = artsOf(t).find((x) => x.id === id); const el = document.querySelector(".ae-side > .cx-b"); if (el) el.innerHTML = aeSide(t, a, S.ae[key]); };
function edMarkdown(ed) {
  if (ed.classList.contains("mdmode")) return ed.querySelector(".md").value;
  const b = ed.querySelector('[data-ed="md"]'); b.click(); const v = ed.querySelector(".md").value; b.click(); return v;
}
AG.action("ae-ver", (el, a) => { const [key, i] = a.split("|"); S.ae[key].v = +i; S.ae[key].diff = null; AG.rerender(); });
AG.action("ae-diff", (el, a) => { const [key, i] = a.split("|"); S.ae[key].diff = +i; AG.rerender(); });
AG.action("ae-diff-off", (el, key) => { S.ae[key].diff = null; AG.rerender(); });
AG.action("ae-save", (el, a) => {
  const [key, vi] = a.split("|"); const [k, id] = key.split(":"); const x = artsOf(T(k)).find((y) => y.id === id);
  const ed = document.querySelector("[data-ae-ed] .ed"); if (!ed) return AG.toast("Tutup perbandingan dulu untuk menyimpan", "dng");
  const mdv = edMarkdown(ed).trim(); if (mdv === x.v[+vi].md.trim()) return AG.toast("Belum ada perubahan untuk disimpan");
  x.v.push(V(mdv, AG.ME, "baru saja", "rev", `Diedit manual oleh ${P(AG.ME).name} dari v${+vi + 1}`)); S.ae[key] = { v: x.v.length - 1, diff: +vi, side: "ver" };
  log(k, "pencil", `<b>${P(AG.ME).name}</b> menyimpan <b>${x.n} v${x.v.length}</b>`); AG.rerender(); AG.toast(`${x.n} v${x.v.length} disimpan · perbedaan dengan v${+vi + 1} ditampilkan`);
});
AG.action("ae-approve", (el, a) => { const [key, vi] = a.split("|"); const [k, id] = key.split(":"); const x = artSetStatus(k, id, +vi, "ok", `Disetujui ${P(AG.ME).name} · baru saja`); AG.rerender(); AG.toast(`${x.n} v${+vi + 1} disetujui`); });
AG.action("ae-ic-done", (el, a) => { const [key, i] = a.split("|"); const [k, id] = key.split(":"); const c = acmOf(k, id)[+i]; c.done = !c.done; S.ae[key].side = "cm"; AG.toast(c.done ? "Komentar inline diselesaikan" : "Komentar dibuka lagi"); const m = document.querySelector(`.ae-mark[data-ci="${i}"]`); if (c.done && m) m.replaceWith(document.createTextNode(m.textContent)); aeRefreshSide(key); });
AG.action("ae-ic-reply", (el, a) => { const [, i] = a.split("|"); const r = el.closest(".ae-ic").querySelector(`[data-ic-reply="${i}"]`); r.style.display = r.style.display === "none" ? "flex" : "none"; r.querySelector("input")?.focus(); });
AG.action("ae-ic-send", (el, a) => { const [key, i] = a.split("|"); const [k, id] = key.split(":"); const v = el.closest("[data-ic-reply]").querySelector("input").value.trim(); if (!v) return AG.toast("Balasan masih kosong", "dng"); acmOf(k, id)[+i].re.push([AG.ME, v]); S.ae[key].side = "cm"; aeRefreshSide(key); AG.toast("Balasan terkirim"); });
AG.action("ae-ic-new", (el, key) => {
  const area = document.querySelector("[data-ae-ed] .area"); const sel = getSelection();
  if (!area || !sel.rangeCount || sel.isCollapsed || !area.contains(sel.anchorNode)) return AG.toast("Pilih teks di editor dulu", "dng");
  const q = sel.toString().trim().slice(0, 140); const [k, id] = key.split(":"); const i = acmOf(k, id).length;
  const mk = document.createElement("mark"); mk.className = "ae-mark on"; mk.dataset.ci = i;
  try { sel.getRangeAt(0).surroundContents(mk); } catch { mk.textContent = q; const r = sel.getRangeAt(0); r.deleteContents(); r.insertNode(mk); }
  sel.removeAllRanges();
  const f = el.parentElement.querySelector("[data-ic-form]");
  f.innerHTML = `<div class="col g8"><blockquote style="margin:0;padding:2px 8px;border-left:3px solid var(--warn);color:var(--muted);font-size:12px">${esc(q)}</blockquote>${inp("", { area: true, ph: "Tulis komentar untuk teks ini…", a: { "data-ic-text": "" } })}<div class="row end g4">${btn("Batal", "sm", "", { "data-act": "ae-ic-cancel", "data-arg": i })}${btn("Kirim", "pri sm", "send", { "data-act": "ae-ic-save", "data-arg": `${key}|${esc(q)}` })}</div></div>`;
  f.querySelector("textarea").focus();
});
AG.action("ae-ic-cancel", (el, i) => { const m = document.querySelector(`.ae-mark[data-ci="${i}"]`); if (m) m.replaceWith(document.createTextNode(m.textContent)); el.closest("[data-ic-form]").innerHTML = ""; });
AG.action("ae-ic-save", (el, a) => {
  const [key, q] = a.split("|"); const [k, id] = key.split(":"); const tx = el.closest("[data-ic-form]").querySelector("[data-ic-text]");
  if (!tx.value.trim()) { tx.classList.add("err"); return AG.toast("Komentar masih kosong", "dng"); }
  acmOf(k, id).push({ q: q.replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"'), w: AG.ME, t: tx.value.trim(), at: "baru saja", done: false, re: [] }); S.ae[key].side = "cm";
  aeRefreshSide(key); document.querySelectorAll(".ae-mark.on").forEach((m) => m.classList.remove("on")); AG.toast("Komentar inline ditambahkan");
});

/* ---------- PR & Evidence ---------- */
function prBody(t) {
  if (t.k !== "KSR-24") return xbox("Pull request", empty("git-pull-request", "Belum ada PR", t.stage === "dev" ? "PR dibuat otomatis oleh agent saat gate “PR terbuka” dikerjakan." : "PR dibuat saat tiket masuk stage Development.", btn("Hubungkan PR manual", "", "link", { "data-toast": "Tempel URL PR untuk menautkan" })), { pad: true });
  const chk = (ok, n, d) => `<div class="row" style="height:26px">${ok === 1 ? `<span class="text-ok">${ic("circle-check", "s14")}</span>` : ok === 0 ? `<span class="text-dng">${ic("circle-x", "s14")}</span>` : `<span class="text-acc">${ic("loader-2", "s14 td-spin")}</span>`}<span class="grow">${n}</span><span class="mut xs">${d}</span></div>`;
  const pr = (repo, no, title, br, checks, n) => `<div class="box"><div class="lrow" style="border:0;flex-wrap:wrap"><span class="tile ok">${ic("git-pull-request")}</span><div class="grow" style="min-width:200px"><b class="b5">${title}</b><div class="mut sm row g4 wrap"><span class="mono">${repo} ${no}</span>·<code>${br}</code> → <code>main</code></div></div>${pill("Terbuka", "ok")}${btn("Buka di GitHub", "sm", "external-link", { "data-toast": `Membuka ${repo} ${no} di GitHub` })}</div>
    <div style="padding:4px 16px 12px;border-top:1px solid var(--line)"><div class="td-lab" style="margin-top:8px">CI · ${n}</div>${checks}</div></div>`;
  const rc = (file, start, lines, body, done) => `<div class="box"><div class="row" style="padding:8px 12px;font-family:var(--mono);font-size:12px">${ic("file-code", "s14 faint")}<span class="grow trunc">${file}</span>${done ? pill("Selesai", "ok") : pill("Terbuka", "warn")}</div>
    <div class="td-code">${lines.map(([s, l], i) => `<div class="${s === "+" ? "add" : s === "-" ? "del" : ""}"><span class="ln">${start + i}</span>${s} ${esc(l)}</div>`).join("")}
    <div class="td-rc"><div class="row">${av("reviewer", 20)}<b class="b5">Agent Code Reviewer</b><span class="faint xs">baris ${start + lines.length - 1}</span></div><div>${body}</div><div class="row">${btn("Balas", "ghost sm", "message-2", { "data-toast": "Balasan dikirim ke thread review" })}${done ? "" : btn("Minta agent perbaiki", "ghost sm", "sparkles", { "data-toast": "Agent Developer diminta memperbaiki catatan ini" })}${btn(done ? "Buka lagi" : "Selesaikan", "ghost sm", done ? "arrow-back-up" : "check", { "data-toast": done ? "Catatan dibuka lagi" : "Catatan ditandai selesai" })}</div></div></div></div>`;
  return `${xbox("Pull request", `${pr("kasir-api", "#142", "feat(pembayaran): integrasi QRIS", "feat/ksr-24-qris", `${chk(1, "build", "1 m 12 d")}${chk(1, "cargo test · 128 lulus", "41 d")}${chk(1, "clippy", "22 d")}${chk(1, "e2e playwright · 6 lulus", "2 m 4 d")}`, "4/4 hijau")}
    ${pr("kasir-web", "#87", "feat(bayar): panel QRIS di layar kasir", "feat/ksr-24-qris", `${chk(1, "build", "48 d")}${chk(1, "vitest · 64 lulus", "19 d")}${chk(1, "lint", "9 d")}`, "3/3 hijau")}`, { n: 2, pad: true, desc: "PR dibuat agent di setiap repo terdampak. CI berjalan di setiap push." })}
    ${xbox("Komentar review AI per baris", `${rc("kasir-api/src/payments/qris/callback.rs", 38, [[" ", "pub async fn callback(body: Bytes, sig: Header) -> Result<()> {"], ["-", "    let evt: QrisEvent = serde_json::from_slice(&body)?;"], ["+", "    verify_hmac(&body, &sig, &cfg.qris_secret)?;"], ["+", "    let evt: QrisEvent = serde_json::from_slice(&body)?;"]], "Signature sebaiknya diverifikasi <b>sebelum</b> body di-parse, supaya payload palsu yang besar tidak sempat diproses.", true)}
    ${rc("kasir-api/src/payments/qris.rs", 12, [[" ", "let fee = calc_fee(&order);"], ["+", "let tmp_amt = order.total - fee;"]], "Nama <code>tmp_amt</code> kurang jelas. Usul: <code>amount_idr</code>.", false)}`, { n: 3, pad: true, desc: "Catatan Agent Code Reviewer. 2 sudah diperbaiki." })}`;
}
function evBody(t) {
  const evDesc = "Screenshot dan hasil test dibuat dan ditandatangani runner saat job berjalan, bukan klaim AI.";
  if (t.k === "KSR-30") return xbox("Evidence", `<div class="td-ev">${[1, 2, 3].map((n) => `<div class="td-shot">${shotSvg("fail")}<div class="cap"><div class="row"><b class="b5 grow">Putaran #${n} · struk transaksi 8812</b>${pill("Gagal", "dng")}</div><span class="td-sig">${ic("shield-check", "s14")}Ditandatangani runner PC Qori · sha256 4be1…${n}c0</span></div></div>`).join("")}
    <div class="box pad col"><b class="b5">Hasil test terakhir</b><div class="row text-dng">${ic("circle-x", "s14")}<span class="fg">playwright struk.spec.ts · 15 lulus, 3 gagal</span></div><div class="code" style="white-space:pre-wrap">✖ struk › diskon per item › total sama dengan layar
  Expected: "Rp49.000"
  Received: "Rp48.900"</div><span class="td-sig">${ic("shield-check", "s14")}Ditandatangani runner PC Qori · 10:51</span></div></div>`, { n: 4, pad: true, desc: evDesc });
  if (t.k !== "KSR-24") return xbox("Evidence", empty("photo", "Belum ada evidence", evDesc), { pad: true });
  return xbox("Evidence", `<div class="td-ev"><div class="td-shot">${shotSvg("ok")}<div class="cap"><div class="row"><b class="b5 grow">Halaman bayar QRIS · 1440 px</b>${pill("Lulus", "ok")}</div><span class="td-sig">${ic("shield-check", "s14")}Ditandatangani runner Laptop Dewi · sha256 9f2c…a71e</span><span class="mut xs">Playwright · 12 Nov 13:58 · stage Development</span></div></div>
    <div class="box"><div class="lrow" style="min-height:44px"><b class="b5 grow">Hasil test</b><span class="td-sig">${ic("shield-check", "s14")}Tanda tangan valid</span></div>
      ${[["cargo test", "128/128", "41 d"], ["vitest kasir-web", "64/64", "19 d"], ["playwright e2e", "6/6", "2 m 4 d"], ["coverage modul pembayaran", "82%", ""]].map(([n, r, d]) => `<div class="lrow" style="min-height:40px"><span class="text-ok">${ic("circle-check", "s14")}</span><span class="grow">${n}</span><span class="mono">${r}</span><span class="mut xs" style="width:48px;text-align:right">${d}</span></div>`).join("")}
      <div class="boxfoot" style="justify-content:flex-start">${btn("Unduh log", "sm", "download", { "data-toast": "Log test diunduh" })}${btn("Verifikasi tanda tangan", "sm ghost", "shield-check", { "data-toast": "Tanda tangan cocok dengan kunci publik runner Laptop Dewi" })}</div></div></div>`, { n: 2, pad: true, desc: evDesc });

}
const prScreen = (id) => { const t = T(id); return t ? tdShell(t, "pr", prBody(t)) : notFound(id); };
const evScreen = (id) => { const t = T(id); return t ? tdShell(t, "evidence", evBody(t)) : notFound(id); };

/* Interaksi bersama halaman detail. */
function detailAfter(root, p) {
  const t = T(p.id); if (!t) return;
  const app = root.querySelector("#app");
  root.querySelector(".stg.now,.stg.dng")?.scrollIntoView({ block: "nearest", inline: "center" });
  app.addEventListener("combo-change", (e) => {
    const f = e.target.closest("[data-fset]")?.dataset.fset; if (!f) return; const v = e.detail;
    if (f === "who") { t.who = v || null; log(t.k, "user", `<b>${P(AG.ME).name}</b> menugaskan ke <b>${personName(t.who)}</b>`); AG.toast(`${t.k} ditugaskan ke ${personName(t.who)}`); }
    if (f === "p") { t.p = +v; AG.toast(`Prioritas ${t.k}: ${AG.PRIORITY[t.p].n}`); }
    if (f === "lbl") t.lbl = [...v];
    if (f === "sprint") { t.sprint = v === "Backlog" ? undefined : v; AG.toast(`${t.k} masuk ${v}`); }
    if (f === "epic") { if (t.type === "subtask") t.parent = v || undefined; else t.epic = v || undefined; AG.toast(v ? `${t.k} ditautkan ke ${v}` : "Tautan epic dilepas"); }
    if (f === "watch") WATCH[t.k] = [...v];
  });
  const log2 = root.querySelector("[data-log]"), el = root.querySelector("[data-elapsed]");
  if (log2) {
    const pool = ['<span class="d">14:04:31</span> tulis src/checkout/validate.rs <span class="g">+41 −3</span>', '<span class="d">14:04:52</span> cargo test checkout_stock <span class="y">· 1 gagal</span>', '<span class="d">14:05:10</span> perbaiki pesan error per baris barang', '<span class="d">14:05:33</span> cargo test checkout_stock <span class="g">· 3 lulus</span>', '<span class="d">14:05:40</span> ag_snippet <span class="b">inventory::reserve</span>'];
    let i = 0, sec = +el.dataset.elapsed;
    const iv = setInterval(() => {
      if (!log2.isConnected) return clearInterval(iv);
      sec++; el.textContent = `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, "0")}`;
      if (sec % 2 === 0 && i < pool.length) { log2.insertAdjacentHTML("beforeend", "\n" + pool[i++]); log2.scrollTop = log2.scrollHeight; }
    }, 1000);
  }
}
const DETAIL_VARIANTS = [["KSR-24", "Detail tiket · menunggu review"], ["KSR-30", "Detail tiket · butuh keputusan"], ["KSR-26", "Detail tiket · dikerjakan AI"], ["KSR-36", "Detail tiket · backlog diblokir"]];
DETAIL_VARIANTS.forEach(([k, n]) => AG.screen({ m: "M2", g: "Tiket & board", n, route: `/t/${k}`, render: () => overview(k), after: (r) => detailAfter(r, { id: k }) }));
AG.screen({ m: "M2", g: "Tiket & board", n: "Detail subtask", route: "/t/:id", path: "/t/KSR-41", render: (p) => overview(p.id), after: detailAfter });
AG.screen({ m: "M2", g: "Tiket & board", n: "Detail tiket · aktivitas & komentar", route: "/t/:id/activity", path: "/t/KSR-24/activity", render: (p) => activity(p.id), after: (r, p) => { detailAfter(r, p); activityAfter(r, p); } });
AG.screen({ m: "M3", g: "Workflow & gate", n: "Detail tiket · navigator artifact", route: "/t/:id/artifacts", path: "/t/KSR-24/artifacts", render: (p) => artifacts(p.id), after: (r, p) => { detailAfter(r, p); artifactsAfter(r); } });
AG.screen({ m: "M3", g: "Workflow & gate", n: "Editor artifact", route: "/t/:id/artifact/:name", path: "/t/KSR-24/artifact/prd", render: (p) => artEditor(p.id, p.name), after: artEditorAfter });
AG.screen({ m: "M2", g: "Tiket & board", n: "Detail tiket · PR & CI", route: "/t/:id/pr", path: "/t/KSR-24/pr", render: (p) => prScreen(p.id), after: detailAfter });
AG.screen({ m: "M3", g: "Workflow & gate", n: "Detail tiket · evidence", route: "/t/:id/evidence", path: "/t/KSR-30/evidence", render: (p) => evScreen(p.id), after: detailAfter });

AG.action("td-approve", (el, k) => approve(k));
AG.action("td-editapprove", (el, k) => { const t = T(k); const arts = artsOf(t); const a = arts.find((x) => lastV(x).s === "rev") || arts.find((x) => x.st === t.stage) || arts[0]; location.hash = `#/t/${k}/artifact/${a.id}`; });
AG.action("td-takeover", (el, k) => { const t = T(k); t.state = "manusia"; t.agent = `Diambil alih ${P(AG.ME).name}`; log(k, "user", `<b>${P(AG.ME).name}</b> mengambil alih job`); rr(); AG.toast(`Job ${k} dihentikan, Anda mengerjakan secara manual`); });
AG.action("td-stop", (el, k) => { const t = T(k); t.state = "antre"; t.agent = "Job dihentikan, menunggu dijalankan lagi"; log(k, "player-stop", `<b>${P(AG.ME).name}</b> menghentikan job`, true); rr(); AG.toast(`Job ${k} dihentikan`, "dng"); });
AG.action("td-run", (el, k) => { const t = T(k); t.state = "jalan"; t.agent = "Job dimulai di MacBook Olivia"; log(k, "player-play", "Job diklaim runner <b>MacBook Olivia</b>", true); rr(); AG.toast(`${k} diklaim runner MacBook Olivia`); });
AG.action("td-human-done", (el, k) => { const t = T(k); t.state = "review"; t.agent = "Menunggu review"; rr(); AG.toast(`${k} siap direview`); });
AG.action("td-follow", (el, k) => { const w = watchOf(T(k)); const i = w.indexOf(AG.ME); i < 0 ? w.push(AG.ME) : w.splice(i, 1); rr(); AG.toast(i < 0 ? `Anda mengikuti ${k}` : `Berhenti mengikuti ${k}`); });
AG.action("att-add", (el, k) => { attOf(T(k)).push(["paperclip", `lampiran-${attOf(T(k)).length + 1}.pdf`, "120 KB", AG.ME]); log(k, "paperclip", `<b>${P(AG.ME).name}</b> menambah lampiran`); rr(); AG.toast("Berkas diunggah ke penyimpanan S3"); });
AG.action("rel-del", (el, a) => { const [k, o] = a.split("|"); REL[k] = (REL[k] || []).filter((r) => r[1] !== o); REL[o] = (REL[o] || []).filter((r) => r[1] !== k); rr(); AG.toast(`Relasi ${k} dan ${o} dihapus`); });
AG.menu("td-more", (k) => `<div class="mlab">${k}</div>
  <button class="mi" data-act="td-clone" data-arg="${k}">${ic("copy")}Clone tiket</button>
  <button class="mi" data-toast="Link ${k} disalin">${ic("link")}Salin link</button>
  <button class="mi" data-dialog="relation-add" data-arg="${k}">${ic("link")}Tambah relasi</button>
  <div class="msep"></div><div class="mlab">Khusus Admin</div>
  <button class="mi" data-dialog="skip-stage" data-arg="${k}">${ic("player-skip-forward")}Lompat stage<span class="tail">Admin</span></button>
  <button class="mi" data-dialog="force-transition" data-arg="${k}">${ic("bolt")}Paksa transisi<span class="tail">Admin</span></button>
  <div class="msep"></div>
  <button class="mi dng" data-dialog="delete-ticket" data-arg="${k}">${ic("trash")}Hapus ke sampah</button>`);
AG.action("td-clone", (el, k) => { const t = T(k); const n = `KSR-${nextNum()}`; AG.TICKETS.push({ ...t, lbl: [...t.lbl], k: n, t: `${t.t} (salinan)`, stage: "backlog", state: "siap", agent: undefined, pr: undefined }); location.hash = `#/t/${n}`; AG.toast(`${n} dibuat dari clone ${k}`); });

/* ====================== M2 · Tiket saya ====================== */
const MY_REPORTED = ["KSR-24", "KSR-36", "KSR-38", "KSR-40", "KSR-33", "KSR-8", "KSR-16", "KSR-1"];
const MY_WATCH = ["KSR-26", "KSR-29", "KSR-30", "KSR-27"];
const MY_COLS = [{ label: "Kode", w: "104px" }, { label: "Judul" }, { label: "Status", hide: "sm" }, { label: "Stage", hide: "sm" }, { label: "Prioritas", w: "96px" }, { label: "Tenggat", hide: "sm" }, { label: "Penerima", hide: "sm" }];
const myRow = (t) => ({ cells: [keyCell(t), `<span class="trunc" style="display:block;min-width:200px">${esc(t.t)}</span>`, statePill(t.state), stagePill(t.stage), AG.prioLabel(t.p), `<span class="mut nowrap">${t.due || "–"}</span>`, whoCell(t.who)],
  sort: [num(t.k), t.t, Object.keys(AG.STATES).indexOf(t.state), sIdx(t.stage), t.p, dueSort(t.due), t.who ? P(t.who).name : "~"], text: `${t.k} ${t.t}`,
  filter: { state: AG.STATES[t.state].n, prio: AG.PRIORITY[t.p].n, stage: AG.stage(t.stage).n } });
function myScreen() {
  const me = AG.ME; const tab = (location.hash.match(/[?&]g=(\w+)/) || [])[1] || "aksi";
  const G = {
    aksi: { n: "Perlu tindakan", items: AG.TICKETS.filter((t) => ["review", "keputusan"].includes(t.state)), d: "Anda approver atau pengambil keputusan" },
    tugas: { n: "Ditugaskan", items: ALL().filter((t) => t.who === me), d: "Anda penerima tiket" },
    lapor: { n: "Dilaporkan", items: MY_REPORTED.map(T).filter(Boolean), d: "Anda yang membuat" },
    ikut: { n: "Diikuti", items: MY_WATCH.map(T).filter(Boolean), d: "Anda watcher" },
  };
  const g = G[tab] || G.aksi; const id = "my-" + tab; SRC[id] = g.items.map((t) => t.k);
  const ai = ALL().filter((t) => t.state === "jalan" && (t.who === me || MY_REPORTED.includes(t.k))).length;
  return AG.shell({ nav: "app", crumbs: ["Tiket saya"], width: "mid", actions: btn("Tiket baru", "pri", "plus", { "data-dialog": "new-ticket" }),
    body: `${head("Tiket saya", "Semua tiket yang menunggu Anda, lintas project.")}
      ${AG.stats([
        { label: "Perlu tindakan", value: G.aksi.items.length, icon: "eye", tone: "warn", hint: "Review atau keputusan", go: "#/my?g=aksi" },
        { label: "Butuh keputusan", value: AG.TICKETS.filter((t) => t.state === "keputusan").length, icon: "alert-triangle", tone: "dng", hint: "Gagal 3 kali di gate", go: "#/t/KSR-30" },
        { label: "Dikerjakan AI", value: ai, icon: "sparkles", tone: "acc", hint: "Untuk tiket Anda", go: "#/my?g=lapor" },
        { label: "Ditugaskan ke saya", value: G.tugas.items.filter((t) => t.stage !== "done").length, icon: "user", hint: `${G.tugas.items.filter((t) => t.stage === "done").length} sudah selesai`, go: "#/my?g=tugas" },
      ])}
      <nav class="tabs" aria-label="Kelompok tiket">${Object.entries(G).map(([k, x]) => `<a href="#/my?g=${k}" class="${k === tab ? "on" : ""}">${x.n}<span class="n">${x.items.length}</span></a>`).join("")}</nav>
      <div class="x-dtfit">${AG.datatable({ id, columns: MY_COLS, rows: g.items.map(myRow), search: "Cari di tiket saya…", pageSize: 10,
        filters: [{ k: "state", label: "Status", icon: "progress", options: Object.values(AG.STATES).map((s) => s.n) }, { k: "stage", label: "Stage", icon: "route", options: AG.STAGES.map((s) => s.n) }, { k: "prio", label: "Prioritas", icon: "antenna-bars-5", options: [1, 2, 3, 4].map((p) => AG.PRIORITY[p].n) }],
        right: `<span class="mut sm hide-sm">${g.d}</span>`, rowGo: (i) => `#/t/${g.items[i].k}`, empty: AG.empty("circle-check", "Tidak ada tiket", "Semua beres untuk kelompok ini.") })}</div>` });
}
AG.screen({ m: "M2", g: "Tiket & board", n: "Tiket saya", route: "/my", render: myScreen });

/* ====================== M2 · Kotak masuk ====================== */
const N = (i, tone, t, w, ts, unread, go, kind) => ({ i, tone, t, w, ts, unread, go, kind });
const INBOX = [
  ...AG.NOTIFS.map((n, x) => ({ ...n, src: n, ts: [5, 18, 22, 60, 120][x] ?? 30, kind: ["Review", "Keputusan", "Izin agent", "Sebutan", "Knowledge"][x] || "Lainnya" })),
  N("message-reply", "gray", "<b>Dewi Dev</b> membalas komentar Anda di <b>KSR-24</b>", "3 jam", 180, true, "#/t/KSR-24/activity", "Sebutan"),
  N("file-text", "warn", "<b>Agent Product Manager</b> menyimpan <b>prd.md v3</b> di KSR-24 · menunggu review", "3 jam", 185, true, "#/t/KSR-24/artifact/prd", "Review"),
  N("mood-smile", "gray", "<b>Adi Admin</b> memberi reaksi 👍 pada komentar Anda", "4 jam", 240, false, "#/t/KSR-24/activity", "Sebutan"),
  N("player-skip-forward", "gray", "<b>Adi Admin</b> melompati stage Desain UI pada <b>KSR-35</b> · alasan: perubahan teks saja", "kemarin", 1440, false, "#/t/KSR-35", "Workflow"),
  N("circle-check", "ok", "<b>KSR-22</b> selesai · estimasi 2 hari, aktual 1,5 hari", "kemarin", 1500, false, "#/t/KSR-22", "Selesai"),
  N("user", "gray", "<b>Qori QA</b> menugaskan <b>KSR-37</b> kepada Anda", "kemarin", 1560, true, "#/t/KSR-37", "Penugasan"),
  N("route", "gray", "Workflow <b>Superpowers · Kasir Online</b> diperbarui ke v7 oleh Adi Admin", "2 hari", 2900, false, "#/settings/workflows/superpowers", "Workflow"),
  N("eye", "warn", "<b>KSR-28</b> mockup v2 siap direview di stage Desain UI", "2 hari", 3000, false, "#/t/KSR-28/artifacts", "Review"),
  N("circle-check", "ok", "<b>KSR-21</b> selesai · PR #131 merged", "3 hari", 4300, false, "#/t/KSR-21", "Selesai"),
  N("at", "gray", "<b>Qori QA</b> menyebut Anda di <b>KSR-30</b>: “selisih Rp100 lagi”", "3 hari", 4400, false, "#/t/KSR-30/activity", "Sebutan"),
];
const markRead = (n, v = false) => { n.unread = v; if (n.src) n.src.unread = v; };
const IB_COLS = [{ label: "", w: "28px", sort: false }, { label: "Notifikasi" }, { label: "Jenis", hide: "sm" }, { label: "Waktu", w: "96px" }];
function inbox() {
  const unread = INBOX.filter((n) => n.unread).length;
  const tabs = { all: ["Semua", INBOX], unread: ["Belum dibaca", INBOX.filter((n) => n.unread)], mention: ["Sebutan & balasan", INBOX.filter((n) => n.kind === "Sebutan")] };
  const [, list] = tabs[S.ibTab]; const id = "ib-" + S.ibTab; SRC[id] = list.map((n) => INBOX.indexOf(n));
  return AG.shell({ nav: "app", crumbs: ["Kotak masuk"], width: "mid",
    actions: btn("Atur notifikasi", "ghost", "settings", { "data-go": "#/settings/notifications" }),
    body: `${head("Kotak masuk", `${unread} belum dibaca. Notifikasi hanya di dalam aplikasi.`)}
      <nav class="tabs" aria-label="Filter kotak masuk">${Object.entries(tabs).map(([k, [n, l]]) => `<button type="button" class="${S.ibTab === k ? "on" : ""}" data-act="ib-tab" data-arg="${k}">${n}<span class="n">${k === "all" ? l.length : l.length}</span></button>`).join("")}</nav>
      <div class="x-dtfit">${AG.datatable({ id, columns: IB_COLS, select: true, pageSize: 10, search: "Cari notifikasi…",
        filters: [{ k: "kind", label: "Jenis", icon: "category-2", options: ["Review", "Keputusan", "Izin agent", "Sebutan", "Penugasan", "Knowledge", "Workflow", "Selesai"] }],
        right: btn("Tandai semua dibaca", "", "check", { "data-act": "ib-all" }),
        bulk: `${btn("Tandai dibaca", "sm ghost", "check", { "data-act": "ib-bulk", "data-arg": `${id}|read` })}${btn("Tandai belum dibaca", "sm ghost", "mail", { "data-act": "ib-bulk", "data-arg": `${id}|unread` })}`,
        rows: list.map((n) => ({ cells: [`<span class="ib-dot ${n.unread ? "" : "off"}" ${n.unread ? 'aria-label="Belum dibaca"' : ""}></span>`, `<div class="row top" style="min-width:240px"><span class="tile ${n.tone === "gray" ? "" : n.tone}">${ic(n.i)}</span><span class="ib-t ${n.unread ? "" : "read"}" style="padding-top:4px">${n.t}</span></div>`, `<span class="mut">${n.kind}</span>`, `<span class="mut nowrap">${n.w}${/lalu|kemarin/.test(n.w) ? "" : " lalu"}</span>`],
          sort: [n.unread ? 0 : 1, n.t.replace(/<[^>]+>/g, ""), n.kind, n.ts], text: n.t.replace(/<[^>]+>/g, ""), filter: { kind: n.kind } })),
        rowGo: (i) => `${list[i].go}?n=${INBOX.indexOf(list[i])}`,
        empty: AG.empty("inbox", "Semua sudah dibaca", "Notifikasi baru muncul di sini.", btn("Tampilkan semua", "", "", { "data-act": "ib-tab", "data-arg": "all" })) })}</div>` });
}
AG.screen({ m: "M2", g: "Tiket & board", n: "Kotak masuk", route: "/inbox", render: inbox,
  after: (root) => root.querySelector("#app").addEventListener("click", (e) => { const tr = e.target.closest("tr[data-go]"); if (!tr || e.target.closest("[data-act]")) return; const n = +(tr.dataset.go.match(/\?n=(\d+)/) || [])[1]; if (INBOX[n]) markRead(INBOX[n]); }, true) });
AG.action("ib-tab", (el, a) => { S.ibTab = a; AG.rerender(); });
AG.action("ib-all", () => { INBOX.forEach((n) => markRead(n)); AG.rerender(); AG.toast("Semua notifikasi ditandai dibaca"); });
AG.action("ib-bulk", (el, a) => { const [id, how] = a.split("|"); const ix = dtKeys(id); ix.forEach((i) => markRead(INBOX[i], how === "unread")); AG._dts[id].sel.clear(); AG.rerender(); AG.toast(`${ix.length} notifikasi ditandai ${how === "unread" ? "belum dibaca" : "dibaca"}`); });

/* ====================== M2 · Sampah ====================== */
function trash(k) {
  SRC.trash = TRASH.map((t) => t.k);
  return AG.shell({ nav: "app", project: k, crumbs: AG.projectCrumb(k, "Sampah"), width: "mid",
    actions: TRASH.length ? btn("Kosongkan sampah", "dng", "trash-x", { "data-dialog": "purge", "data-arg": "all" }) : "",
    body: `${head("Sampah", "Tiket yang dihapus disimpan 30 hari, lalu terhapus permanen. Komentar, artifact, dan riwayat ikut dipulihkan.")}
      ${TRASH.length ? `<div class="alert gray">${ic("info-circle")}<span>Hanya Admin dan pembuat tiket yang bisa menghapus permanen. Semua penghapusan tercatat di <a class="link" href="#/settings/audit">audit log</a>.</span></div><div class="x-dtfit">` + AG.datatable({ id: "trash", select: true, pageSize: 10, search: "Cari tiket di sampah…",
        columns: [{ label: "Tiket" }, { label: "Dihapus oleh", hide: "sm" }, { label: "Tanggal", hide: "sm" }, { label: "Sisa waktu", w: "96px" }, { label: "", sort: false, align: "right", w: "84px" }],
        filters: [{ k: "by", label: "Dihapus oleh", icon: "user", options: Object.values(AG.PEOPLE).map((p) => p.name) }, { k: "type", label: "Tipe", icon: "category-2", options: Object.values(AG.TYPES).map((x) => x.n) }],
        bulk: `${btn("Pulihkan", "sm ghost", "arrow-back-up", { "data-act": "trash-bulk", "data-arg": "restore" })}${btn("Hapus permanen", "sm ghost dng", "trash-x", { "data-act": "trash-bulk", "data-arg": "purge" })}`,
        rows: TRASH.map((t) => ({ cells: [`<div class="row" style="min-width:220px">${typeIcon(t.type)}<span class="mono mut">${t.k}</span><span class="trunc">${esc(t.t)}</span></div>`, whoCell(t.by), `<span class="mut nowrap">${t.at}</span>`,
            t.left <= 3 ? pill(`${t.left} hari`, "dng", "", "clock") : `<span class="mut nowrap">${t.left} hari</span>`,
            `<div class="row end g4">${btn("Pulihkan", "sm hide-sm", "arrow-back-up", { "data-act": "trash-restore", "data-arg": t.k })}${btn("", "sm icon only-sm", "arrow-back-up", { "data-act": "trash-restore", "data-arg": t.k, "aria-label": `Pulihkan ${t.k}` })}${btn("", "sm ghost icon dng", "trash-x", { "data-dialog": "purge", "data-arg": t.k, "aria-label": `Hapus permanen ${t.k}` })}</div>`],
          sort: [num(t.k), P(t.by).name, 30 - t.left, t.left, 0], text: `${t.k} ${t.t}`, filter: { by: P(t.by).name, type: AG.TYPES[t.type].n } })) }) + `</div>`
      : empty("trash", "Sampah kosong", "Tiket yang dihapus akan muncul di sini selama 30 hari.", btn("Kembali ke board", "", "layout-kanban", { "data-go": `#/p/${k}/board` }))}` });
}
AG.screen({ m: "M2", g: "Tiket & board", n: "Sampah", route: "/p/:k/trash", path: "/p/KSR/trash", render: (p) => trash(p.k) });
function restore(k) { const i = TRASH.findIndex((t) => t.k === k); if (i < 0) return; const [x] = TRASH.splice(i, 1); const t = x.orig || { k: x.k, type: x.type, t: x.t, stage: "backlog", state: "siap", p: x.p, who: null, lbl: x.lbl, pts: null }; (t.stage === "done" && t.sprint && !t.agent && num(t.k) < 19 ? EXTRA : AG.TICKETS).push(t); return t; }
AG.action("trash-restore", (el, k) => { const t = restore(k); AG._dts.trash?.sel.clear(); AG.rerender(); AG.toast(`${k} dipulihkan ke ${AG.stage(t.stage).n}`); });
AG.action("trash-bulk", (el, a) => { const ks = dtKeys("trash"); if (!ks.length) return; if (a === "purge") return AG.openDialog("purge", ks.join(",")); ks.forEach(restore); AG._dts.trash.sel.clear(); AG.rerender(); AG.toast(`${ks.length} tiket dipulihkan`); });
AG.action("purge-confirm", (el, a) => { const ks = a === "all" ? TRASH.map((t) => t.k) : a.split(","); ks.forEach((k) => { const i = TRASH.findIndex((t) => t.k === k); if (i >= 0) TRASH.splice(i, 1); }); if (AG._dts.trash) AG._dts.trash.sel.clear(); AG.closeDialog(); AG.rerender(); AG.toast(a === "all" ? "Sampah dikosongkan" : ks.length > 1 ? `${ks.length} tiket dihapus permanen` : `${ks[0]} dihapus permanen`); });
AG.dialog("purge", (a) => { const n = a === "all" ? TRASH.length : a.split(",").length; return { icon: "trash-x", tone: "dng", title: a === "all" ? `Kosongkan sampah (${n} tiket)?` : n > 1 ? `Hapus permanen ${n} tiket?` : `Hapus permanen ${a}?`, desc: "Tiket, komentar, artifact, dan lampiran di S3 dihapus selamanya. Tindakan ini tidak bisa dibatalkan.",
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Hapus permanen", "dngs", "trash-x", { "data-act": "purge-confirm", "data-arg": a })}` }; });

/* ====================== M2 · Tipe tiket & field ====================== */
const TYPE_LIST = [
  ...Object.entries(AG.TYPES).map(([k, x]) => ({ k, ...x, builtin: true, d: { epic: "Kumpulan story besar lintas sprint", story: "Kebutuhan dari sisi pengguna toko", task: "Pekerjaan teknis atau operasional", bug: "Perilaku yang salah di aplikasi", subtask: "Pecahan kecil dari tiket lain" }[k] })),
  { k: "insiden", n: "Insiden", i: "flag", c: "#e11d48", d: "Gangguan di toko yang sedang berjalan" },
  { k: "spike", n: "Spike", i: "bulb", c: "#0891b2", d: "Riset singkat sebelum estimasi" },
];
const TYPE_WF = { epic: "Epic ringkas", story: "Superpowers", task: "Superpowers", bug: "Bug Triage", subtask: "Subtask ringkas", insiden: "Hotfix/Incident", spike: "Kanban" };
const WF_NAMES = ["Superpowers", "Scrum", "Kanban", "Bug Triage", "Subtask ringkas", "Epic ringkas", "Hotfix/Incident"];
const STD_FIELDS = [["Judul", "Teks", true], ["Deskripsi", "Rich text", false], ["Penerima", "Orang", false], ["Reporter", "Orang", true], ["Prioritas", "Pilihan", true], ["Label", "Multi pilihan", false], ["Tenggat", "Tanggal", false], ["Story point", "Angka", false], ["Sprint", "Sprint", false]];
const CF = [
  { n: "Toko pilot", kind: "Pilihan", opts: "Sudirman, Kemang, BSD", types: ["story", "task", "bug"], req: false, card: false },
  { n: "Versi aplikasi", kind: "Teks", types: ["bug", "insiden"], req: true, card: true },
  { n: "Dampak pelanggan", kind: "Angka", types: ["bug", "insiden"], req: false, card: false },
  { n: "Tautan desain", kind: "URL", types: ["story", "epic"], req: false, card: false },
  { n: "Penyedia pembayaran", kind: "Pilihan", opts: "Xendit, Midtrans, DOKU", types: ["story", "bug"], req: false, card: false },
];
const ICONS_T = ["bookmark", "checkbox", "bug", "bolt", "subtask", "flag", "bulb", "target", "book", "rocket", "shield-check", "star", "test-pipe", "palette"];
const tico = (x, lg = false) => `<span class="tt-ico ${lg ? "lg" : ""}" style="background:color-mix(in srgb, ${x.c} 14%, transparent);color:${x.c}">${ic(x.i)}</span>`;
const typeOf = (k) => TYPE_LIST.find((x) => x.k === k);
function typesScreen() {
  SRC["cf-all"] = CF.map((f) => f.n);
  return AG.shell({ nav: "settings", width: "narrow", crumbs: ["Pengaturan", "Tipe tiket & field"],
    body: `${head("Tipe tiket & field", "Setiap tipe punya ikon, field, dan workflow sendiri. Pilih tipe untuk mengaturnya.", btn("Tipe baru", "pri", "plus", { "data-dialog": "type-add" }))}
      ${section("Tipe tiket", `${TYPE_LIST.length} tipe · ${TYPE_LIST.filter((x) => !x.builtin).length} custom`, `<div class="box">${TYPE_LIST.map((x) => `<a class="lrow" href="#/settings/ticket-types/${x.k}">${tico(x)}<div class="grow"><div class="row g4"><b class="b5">${x.n}</b>${x.builtin ? "" : pill("Custom", "acc")}</div><div class="mut sm trunc">${x.d || ""}</div></div>
        <span class="mut sm hide-sm nowrap" style="width:150px">${ic("route", "s14")} ${TYPE_WF[x.k]}</span><span class="mut sm hide-sm nowrap" style="width:70px">${CF.filter((f) => f.types.includes(x.k)).length + STD_FIELDS.length} field</span><span class="mut sm nowrap" style="width:64px;text-align:right">${ALL().filter((t) => t.type === x.k).length} tiket</span>${ic("chevron-right", "s14 faint")}</a>`).join("")}</div>`)}
      ${section("Pustaka field custom", "Field custom bisa dipakai di beberapa tipe sekaligus.", AG.datatable({ id: "cf-all", pageSize: 10, search: "Cari field…",
        columns: [{ label: "Field" }, { label: "Jenis" }, { label: "Dipakai di", sort: false, hide: "sm" }, { label: "Wajib", w: "80px" }, { label: "", sort: false, w: "44px" }],
        right: btn("Field baru", "", "plus", { "data-dialog": "field-add", "data-arg": "story|" }),
        rows: CF.map((f) => ({ cells: [`<b class="b5">${f.n}</b>`, `<span class="mut">${f.kind}</span>`, `<div class="row wrap g4">${f.types.map((t) => typeOf(t) ? `<span class="pill gray">${ic(typeOf(t).i)}${typeOf(t).n}</span>` : "").join("")}</div>`, f.req ? pill("Wajib", "warn") : `<span class="mut">Tidak</span>`, btn("", "ghost sm icon", "pencil", { "data-dialog": "field-add", "data-arg": `${f.types[0]}|${f.n}`, "aria-label": `Edit ${f.n}` })],
          sort: [f.n, f.kind, 0, f.req ? 0 : 1, 0], text: `${f.n} ${f.kind}` })) }))}` });
}
AG.screen({ m: "M2", g: "Pengaturan tiket", n: "Tipe tiket & field", route: "/settings/ticket-types", render: typesScreen });

function typeDetail(k) {
  const ty = typeOf(k); if (!ty) return AG.shell({ nav: "settings", width: "narrow", crumbs: ["Pengaturan", "Tipe tiket"], body: empty("category-2", "Tipe tidak ditemukan", "Tipe ini mungkin sudah dihapus.", btn("Semua tipe", "", "arrow-left", { "data-go": "#/settings/ticket-types" })) });
  const cfs = CF.filter((f) => f.types.includes(ty.k)); const id = "tt-f-" + ty.k; SRC[id] = [...STD_FIELDS.map((f) => f[0]), ...cfs.map((f) => f.n)];
  const active = S.sub["tt:" + k] || "umum";
  const nTk = ALL().filter((t) => t.type === ty.k).length;
  const save = (label = "Simpan perubahan") => AG.savebar(label, { msg: `Berlaku untuk ${nTk} tiket ${ty.n}`, toast: `Tipe ${ty.n} disimpan` });
  const umum = `${xcard({ title: "Identitas", desc: "Nama, ikon, dan warna tampil di board, daftar, dan detail tiket.", foot: save(), body: `${ty.builtin ? `<div class="alert gray">${ic("lock")}<span>Tipe bawaan tidak bisa dihapus. Nama, ikon, warna, field, dan workflow tetap bisa diubah.</span></div>` : ""}<div class="col g16">
      <div class="grid2">${fld("Nama tipe", inp(ty.n, { a: { "data-tt-name": "" } }))}${fld("Kunci", inp(ty.k, { cls: "mono ro", a: { readonly: true, "aria-label": "Kunci tipe" } }), "Dipakai di API dan token {TYPE}.")}</div>
      ${fld("Deskripsi", inp(ty.d || "", { ph: "Kapan tipe ini dipakai" }))}
      ${fld("Ikon", `<div class="tt-icons" role="radiogroup" aria-label="Ikon tipe">${ICONS_T.map((i) => `<button type="button" class="btn icon ${i === ty.i ? "on" : ""}" data-radio="tt-icon" data-icon="${i}" aria-label="Ikon ${i}">${ic(i)}</button>`).join("")}</div>`)}
      ${fld("Warna", AG.colorPicker(ty.c))}
      <div class="tt-prev" data-tt-prev>${tico(ty, true)}<div class="grow"><b class="b5" data-tt-pn>${ty.n}</b><div class="mut sm">Pratinjau di kartu board</div></div><span class="row g4" data-tt-pi style="color:${ty.c}">${ic(ty.i, "s14")}</span><span class="mono faint">KSR-42</span></div></div>` })}
    ${ty.builtin ? "" : section("Zona berbahaya", "", `<div class="box dng">${srow("Hapus tipe", `${ALL().filter((t) => t.type === ty.k).length} tiket memakai tipe ini. Tipe hanya bisa dihapus bila tidak ada tiket.`, btn("Hapus tipe", "dng", "trash", { "data-act": "tt-del", "data-arg": ty.k }))}</div>`)}`;
  const fields = xcard({ title: `Field untuk ${ty.n}`, desc: "Field standar selalu ada. Atur mana yang wajib dan tampil di kartu board.", foot: save(), body: AG.datatable({ id, pageSize: 25, search: "Cari field…",
    columns: [{ label: "Field" }, { label: "Jenis", hide: "sm" }, { label: "Wajib", sort: false, w: "72px" }, { label: "Di kartu", sort: false, w: "80px" }, { label: "", sort: false, w: "44px" }],
    right: btn("Tambah field", "", "plus", { "data-dialog": "field-add", "data-arg": `${ty.k}|` }),
    rows: [...STD_FIELDS.map(([n, kind, req]) => ({ cells: [`<span class="row">${n}<span class="faint" title="Field standar">${ic("lock", "s14")}</span></span>`, `<span class="mut">${kind}</span>`, req ? `<span class="mut sm">Selalu</span>` : sw(n === "Penerima" && ty.k === "bug", `${n} wajib`), sw(["Prioritas", "Label", "Story point", "Penerima", "Tenggat"].includes(n), `${n} di kartu`), ""], sort: [n, kind], text: n })),
      ...cfs.map((f) => ({ cells: [`<span class="row">${f.n}${pill("Custom", "acc")}</span>`, `<span class="mut">${f.kind}${f.opts ? ` · ${f.opts}` : ""}</span>`, sw(f.req, `${f.n} wajib`), sw(f.card, `${f.n} di kartu`), btn("", "ghost sm icon", "dots", { "data-menu": "tt-field", "data-arg": `${ty.k}|${f.n}`, "aria-label": `Aksi ${f.n}` })], sort: [f.n, f.kind], text: f.n }))] }) });
  const wf = xcard({ title: "Workflow", desc: "Alur stage untuk tiket bertipe ini. Mengganti workflow membuka layar migrasi untuk tiket yang sedang berjalan.", action: seeAll("#/settings/workflows/superpowers", "Buka workflow"), foot: save(), body: `${fld("Workflow default", sel(WF_NAMES, TYPE_WF[ty.k]), "Dipakai di semua project, kecuali project yang menimpa di bawah.")}
    <div class="col g4"><b class="b5">Timpa per project</b><span class="mut sm">Project boleh memakai workflow lain untuk tipe ini.</span></div>
    <div class="box">${AG.PROJECTS.map((p) => { const w = p.k === "KSR" ? TYPE_WF[ty.k] : ty.k === "bug" ? "Bug Triage" : p.wf; const own = p.k !== "KSR" && w !== TYPE_WF[ty.k];
      return `<div class="lrow" style="flex-wrap:wrap">${AG.projDot(p.k)}<div class="grow" style="min-width:140px"><b class="b5">${p.n}</b><div class="mut sm">${own ? "Menimpa default" : "Ikut default"}</div></div><div style="width:200px">${sel(WF_NAMES, w)}</div>${btn("", "ghost sm icon", "external-link", { "data-go": `#/settings/p/${p.k}/workflow`, "aria-label": `Workflow ${p.n}` })}</div>`; }).join("")}</div>` });
  return AG.shell({ nav: "settings", width: "mid", crumbs: [`<a href="#/settings/ticket-types">Tipe tiket & field</a>`, ty.n],
    body: AG.subpage({ title: `<span class="row g12">${tico(ty, true)}<span>${ty.n}</span></span>`, desc: `${ty.builtin ? "Tipe bawaan" : "Tipe custom"} · ${ALL().filter((t) => t.type === ty.k).length} tiket · workflow ${TYPE_WF[ty.k]}`,
      active,
      nav: [{ k: "umum", label: "Umum", icon: "settings", desc: "Nama, ikon, warna" }, { k: "field", label: "Field", icon: "list-details", count: STD_FIELDS.length + cfs.length, desc: "Standar dan custom" }, { k: "wf", label: "Workflow", icon: "route", desc: TYPE_WF[ty.k] }],
      body: AG.panel("umum", umum, active === "umum") + AG.panel("field", fields, active === "field") + AG.panel("wf", wf, active === "wf") }) });

}
AG.screen({ m: "M2", g: "Pengaturan tiket", n: "Detail tipe tiket", route: "/settings/ticket-types/:type", path: "/settings/ticket-types/story", render: (p) => typeDetail(p.type),
  after: (root, p) => {
    const app = root.querySelector("#app"); subpageKeep(app, "tt:" + p.type); const ty = typeOf(p.type); if (!ty) return;
    const prev = app.querySelector("[data-tt-prev]"); if (!prev) return;
    const paint = () => { prev.querySelector(".tt-ico").outerHTML; prev.querySelector(".tt-ico").style.cssText = `background:color-mix(in srgb, ${ty.c} 14%, transparent);color:${ty.c}`; prev.querySelector(".tt-ico").innerHTML = ic(ty.i); prev.querySelector("[data-tt-pi]").style.color = ty.c; prev.querySelector("[data-tt-pi]").innerHTML = ic(ty.i, "s14"); };
    app.addEventListener("color-change", (e) => { ty.c = e.detail; paint(); });
    app.addEventListener("click", (e) => { const b = e.target.closest("[data-icon]"); if (b) { app.querySelectorAll("[data-icon]").forEach((x) => x.classList.toggle("on", x === b)); ty.i = b.dataset.icon; paint(); } });
    app.querySelector("[data-tt-name]")?.addEventListener("input", (e) => { prev.querySelector("[data-tt-pn]").textContent = e.target.value || "Tanpa nama"; });
  } });
AG.action("tt-del", (el, k) => { if (ALL().some((t) => t.type === k)) return AG.toast("Tipe masih dipakai tiket. Pindahkan tiketnya dulu.", "dng"); TYPE_LIST.splice(TYPE_LIST.findIndex((x) => x.k === k), 1); location.hash = "#/settings/ticket-types"; AG.toast("Tipe dihapus"); });
AG.menu("tt-field", (a) => { const [k, n] = a.split("|"); return `<button class="mi" data-dialog="field-add" data-arg="${k}|${esc(n)}">${ic("pencil")}Edit field</button><button class="mi dng" data-act="tt-field-del" data-arg="${esc(n)}">${ic("trash")}Hapus field</button>`; });
AG.action("tt-field-del", (el, n) => { CF.splice(CF.findIndex((f) => f.n === n), 1); AG.rerender(); AG.toast(`Field “${n}” dihapus. Nilai lama tetap di riwayat tiket.`); });
const KINDS = ["Teks", "Angka", "Pilihan", "Multi pilihan", "Tanggal", "Orang", "URL"];
const optRow = (on, t, d, label) => `<div class="x-opt">${sw(on, label)}<div><div class="t">${t}</div><div class="d">${d}</div></div></div>`;
AG.dialog("field-add", (arg = "story|") => { const [ty, name] = arg.split("|"); const f = CF.find((x) => x.n === name); const types = f ? f.types : [ty];
  return { w: "w560", icon: f ? "pencil" : "table-plus", tone: "acc", title: f ? `Edit field “${f.n}”` : "Field custom baru", desc: "Field tampil di detail tiket dan bisa dipakai di pencarian.",
  body: `${fld("Nama field", inp(f?.n || "", { ph: "Contoh: Cabang toko", a: { "data-fname": "" } }))}
    <div class="grid2">${fld("Jenis", sel(KINDS, f?.kind || "Pilihan"))}${fld('Nilai default <span class="opt">(opsional)</span>', inp("", { ph: "Kosong" }))}</div>
    ${fld("Opsi pilihan", inp(f?.opts || "Sudirman, Kemang, BSD"), "Pisahkan dengan koma. Hanya untuk jenis Pilihan dan Multi pilihan.")}
    ${fld("Berlaku untuk tipe", `<div class="row wrap g4">${TYPE_LIST.map((x) => `<button type="button" class="chip ${types.includes(x.k) ? "on" : ""}" data-toggle data-tk="${x.k}">${ic(x.i, "s14")}${x.n}</button>`).join("")}</div>`)}
    <div class="col g12">${optRow(f?.req || false, "Wajib diisi", "Tiket tidak bisa dibuat tanpa field ini.", "Wajib")}${optRow(f?.card || false, "Tampil di kartu board", "Nilai muncul di bawah judul kartu.", "Tampil di kartu")}</div>`,
  foot: `${f ? `<button type="button" class="btn ghost dng" style="margin-right:auto" data-act="tt-field-del" data-arg="${esc(f.n)}" data-close>${ic("trash")}Hapus</button>` : ""}${btn("Batal", "", "", { "data-close": true })}${btn(f ? "Simpan field" : "Tambah field", "pri", "", { "data-act": "field-save", "data-arg": arg })}` }; });
AG.action("field-save", (el, arg) => {
  const [ty, name] = arg.split("|"); const d = el.closest(".dialog"); const n = d.querySelector("[data-fname]").value.trim();
  if (!n) { d.querySelector("[data-fname]").classList.add("err"); return AG.toast("Nama field wajib diisi", "dng"); }
  const types = [...d.querySelectorAll(".chip.on[data-tk]")].map((c) => c.dataset.tk); const [req, card] = [...d.querySelectorAll(".x-opt .sw")].map((s) => s.classList.contains("on"));
  const [kind] = [...d.querySelectorAll("select")].map((s) => s.value); const opts = d.querySelectorAll("input.inp")[2]?.value;
  const old = CF.find((x) => x.n === name); const rec = { n, kind, opts: /Pilihan/.test(kind) ? opts : undefined, types: types.length ? types : [ty], req, card };
  if (old) Object.assign(old, rec); else CF.push(rec);
  AG.closeDialog(); AG.rerender(); AG.toast(old ? `Field “${n}” diperbarui` : `Field “${n}” ditambahkan`);
});
AG.dialog("type-add", () => ({ w: "w560", icon: "category-2", tone: "acc", title: "Tipe tiket baru", desc: "Misalnya Insiden, Spike, atau Riset.",
  body: `${fld("Nama", inp("", { ph: "Contoh: Riset", a: { "data-tname": "" } }))}
    ${fld("Ikon", `<div class="tt-icons">${ICONS_T.slice(5).map((i, x) => `<button type="button" class="btn icon ${x === 2 ? "on" : ""}" data-radio="ticon" data-icon="${i}" aria-label="Ikon ${i}">${ic(i)}</button>`).join("")}</div>`)}
    ${fld("Warna", AG.colorPicker("#4f46e5"))}
    <div class="grid2">${fld("Workflow", sel(WF_NAMES, "Kanban"))}${fld("Salin field dari", sel(["Story", "Task", "Bug", "Tidak ada"], "Task"))}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Buat tipe", "pri", "", { "data-act": "type-save" })}`,
  after(el) { let c = "#4f46e5"; el.addEventListener("color-change", (e) => (el.dataset.c = e.detail)); el.addEventListener("click", (e) => { const b = e.target.closest("[data-icon]"); if (b) el.querySelectorAll("[data-icon]").forEach((x) => x.classList.toggle("on", x === b)); }); el.dataset.c = c; } }));
AG.action("type-save", (el) => {
  const d = el.closest(".dialog"); const n = d.querySelector("[data-tname]").value.trim(); if (!n) { d.querySelector("[data-tname]").classList.add("err"); return AG.toast("Nama tipe wajib diisi", "dng"); }
  const k = n.toLowerCase().replace(/\W+/g, "-"); if (typeOf(k)) return AG.toast("Tipe dengan nama itu sudah ada", "dng");
  TYPE_LIST.push({ k, n, i: d.querySelector("[data-icon].on")?.dataset.icon || "target", c: d.closest(".scrim").dataset.c || "#4f46e5", d: "" }); TYPE_WF[k] = d.querySelector("select").value;
  AG.closeDialog(); location.hash = `#/settings/ticket-types/${k}`; AG.toast(`Tipe “${n}” dibuat`);
});

/* ====================== M2 · Kode tiket ====================== */
function fmtCode(tpl, x) {
  return tpl.replace(/\{PROJECT\}/g, x.p).replace(/\{TEAM\}/g, x.team).replace(/\{TYPE\}/g, x.type).replace(/\{YYYY\}/g, "2026").replace(/\{YY\}/g, "26")
    .replace(/\{NUM:(\d)\}/g, (_, n) => String(x.n).padStart(+n, "0")).replace(/\{NUM\}/g, String(x.n));
}
const TC_SAMPLES = [{ p: "KSR", team: "FE", type: "STR", n: 42, d: "Story baru di Kasir Online" }, { p: "GDG", team: "BE", type: "BUG", n: 118, d: "Bug di Aplikasi Gudang · tim Backend" }, { p: "PPL", team: "MOB", type: "TSK", n: 7, d: "Task di Portal Pelanggan · tim Mobile" }];
function tcPreview(tpl) {
  const err = !/\{NUM(:\d)?\}/.test(tpl) ? "Template wajib memuat {NUM} atau {NUM:n}." : /\{(?!PROJECT\}|TEAM\}|TYPE\}|YYYY\}|YY\}|NUM\}|NUM:\d\})[^}]*\}/.test(tpl) ? "Ada token yang tidak dikenal." : "";
  return { err, html: TC_SAMPLES.map((x) => `<div class="lrow" style="min-height:44px"><span class="grow mut">${x.d}</span>${ic("arrow-right", "s14 faint")}<span class="tc-prev">${err ? "–" : esc(fmtCode(tpl, x))}</span></div>`).join("") };
}
function codeScreen() {
  const pv = tcPreview(S.tpl); const active = S.sub.tc || "tpl";
  const opt = (k, n, d) => `<div class="tc-opt ${S.counter === k ? "on" : ""}" data-radio="tc-counter" data-act="tc-counter" data-arg="${k}" role="radio" aria-checked="${S.counter === k}"><span class="radio ${S.counter === k ? "on" : ""}"></span><div><b class="b5">${n}</b><div class="mut sm">${d}</div></div></div>`;
  const save = (label = "Simpan template") => sbar(label, "tc-save", { msg: "Tiket baru langsung memakai format baru" });
  const tpl = xcard({ title: "Format kode", desc: "Kode muncul di board, commit, branch, dan URL tiket.", foot: save(), body: `<div class="alert">${ic("info-circle")}<div><b>Kode lama tetap berlaku sebagai alias.</b> Saat format diubah, link, pencarian, dan commit lama yang menyebut <code>GDG-118</code> tetap mengarah ke <code>BE-2026-0118</code>.</div></div>
      <div class="col g12">
      ${fld("Template", inp(S.tpl, { cls: "mono", a: { "data-tc": "", "aria-label": "Template kode tiket", spellcheck: "false" } }), "", `<span data-tc-err>${pv.err}</span>`)}
      <div class="col g4"><span class="mut sm">Sisipkan token</span><div class="row wrap">${["{PROJECT}", "{TEAM}", "{TYPE}", "{YYYY}", "{YY}", "{NUM}", "{NUM:4}"].map((x) => `<button type="button" class="tc-tok" data-act="tc-tok" data-arg="${x}">${x}</button>`).join("")}</div></div>
      <div class="row wrap g4 sm mut">Contoh siap pakai: <button type="button" class="link" data-act="tc-set" data-arg="{PROJECT}-{NUM}">{PROJECT}-{NUM}</button>·<button type="button" class="link" data-act="tc-set" data-arg="{TEAM}-{YYYY}-{NUM:4}">{TEAM}-{YYYY}-{NUM:4}</button>·<button type="button" class="link" data-act="tc-set" data-arg="{PROJECT}-{TYPE}-{NUM}">{PROJECT}-{TYPE}-{NUM}</button></div></div>
      <div class="col g8"><b class="b5">Pratinjau langsung</b><div class="box" data-tc-prev>${pv.html}</div></div>` });
  const cnt = xcard({ title: "Penghitung nomor", desc: "Kapan nomor mulai dari 1 lagi.", pad: false, foot: save("Simpan penghitung"), body: `<div class="box">${opt("project", "Per project", "KSR-1, KSR-2, … satu urutan untuk semua tipe di project.")}${opt("type", "Per tipe", "Story dan Bug punya urutan masing-masing, cocok dengan {TYPE}.")}${opt("year", "Per tahun", "Urutan mulai dari 1 setiap 1 Januari, cocok dengan {YYYY}.")}</div>` });
  SRC["tc-proj"] = AG.PROJECTS.map((p) => p.k);
  const proj = section("Timpa per project", "Project boleh memakai format sendiri. Kosong berarti ikut default workspace.", AG.datatable({ id: "tc-proj", pageSize: 10,
    columns: [{ label: "Project" }, { label: "Template" }, { label: "Contoh", hide: "sm" }, { label: "", sort: false, align: "right" }],
    rows: [["KSR", null], ["GDG", "{TEAM}-{YYYY}-{NUM:4}"], ["PPL", null]].map(([k, tp]) => ({ cells: [`<span class="row">${AG.projDot(k)}${AG.project(k).n}</span>`, tp ? `<code>${tp}</code>` : `<span class="mut">Default workspace</span>`, `<span class="mono">${fmtCode(tp || S.tpl, { p: k, team: "BE", type: "STR", n: k === "GDG" ? 42 : 12 })}</span>`, btn(tp ? "Ubah" : "Timpa", "sm", "", { "data-go": `#/settings/p/${k}/general`, "data-toast": "Atur di pengaturan project" })], sort: [AG.project(k).n, tp || ""] })) }));
  return AG.shell({ nav: "settings", width: "mid", crumbs: ["Pengaturan", "Kode tiket"],
    body: AG.subpage({ title: "Kode tiket", desc: "Template kode default untuk workspace. Bisa ditimpa per project.", active,
      nav: [{ k: "tpl", label: "Format & pratinjau", icon: "hash", desc: S.tpl }, { k: "cnt", label: "Penghitung nomor", icon: "list-numbers", desc: { project: "Per project", type: "Per tipe", year: "Per tahun" }[S.counter] }, { k: "proj", label: "Per project", icon: "folders", count: 1, desc: "1 project menimpa" }],
      body: AG.panel("tpl", tpl, active === "tpl") + AG.panel("cnt", cnt, active === "cnt") + AG.panel("proj", proj, active === "proj") }) });

}
AG.screen({ m: "M2", g: "Pengaturan tiket", n: "Kode tiket", route: "/settings/ticket-code", render: codeScreen,
  after: (root) => { const app = root.querySelector("#app"); subpageKeep(app, "tc"); const i = root.querySelector("[data-tc]"); i.addEventListener("input", () => { S.tpl = i.value; const pv = tcPreview(i.value); root.querySelector("[data-tc-prev]").innerHTML = pv.html; root.querySelector("[data-tc-err]").textContent = pv.err; i.classList.toggle("err", !!pv.err); }); } });
AG.action("tc-tok", (el, a) => { const i = document.querySelector("[data-tc]"); const p = i.selectionStart ?? i.value.length; i.value = i.value.slice(0, p) + a + i.value.slice(i.selectionEnd ?? p); i.focus(); i.selectionStart = i.selectionEnd = p + a.length; i.dispatchEvent(new Event("input")); });
AG.action("tc-set", (el, a) => { S.tpl = a; AG.rerender(); });
AG.action("tc-counter", (el, a) => { S.counter = a; });
AG.action("tc-save", () => { if (tcPreview(S.tpl).err) return AG.toast("Template belum valid", "dng"); AG.toast(`Template ${S.tpl} disimpan · kode lama tetap jadi alias`); });

/* ====================== M2 · Dialog ====================== */
AG.dialog("new-ticket", (arg = "") => {
  const sub = arg.startsWith("sub:") ? arg.slice(4) : null; const stg = !sub && arg ? arg : "backlog";
  const types = sub ? ["subtask"] : ["story", "task", "bug", "epic"];
  return { w: "w640", title: sub ? `Subtask baru untuk ${sub}` : "Tiket baru", desc: sub ? "Subtask memakai workflow ringkas: Dev → Review → Selesai." : "Kasir Online · AI mengusulkan estimasi saat judul diisi.",
    body: `${fld("Tipe", `<div class="row wrap g4" role="radiogroup" aria-label="Tipe tiket">${types.map((k, i) => `<button type="button" class="chip ${i === 0 ? "on" : ""}" data-radio="nt-type" data-v="${k}">${typeIcon(k)}${AG.TYPES[k].n}</button>`).join("")}</div>`)}
      ${fld("Judul", inp("", { ph: "Contoh: Kasir bisa membatalkan transaksi sebelum dibayar", a: { "data-nt-title": "", autofocus: true } }))}
      ${fld('Deskripsi <span class="opt">(opsional)</span>', AG.editor({ compact: true, h: "96px", ph: "Tujuan, acceptance criteria, catatan. @ untuk menyebut orang atau tiket." }), "Deskripsi panjang bisa dilanjutkan di halaman tiket dengan editor lengkap.")}
      <div class="grid3" style="gap:12px">${fld("Stage awal", sel(AG.STAGES.filter((s) => s.k !== "done").map((s) => [s.k, s.n]), sub ? "dev" : stg))}<div data-nt-who>${fld("Penerima", AG.combo({ options: PEOPLE_OPTS([{ v: "", l: "Belum ada", lead: av(null, 20) }]), value: "", ph: "Belum ada", w: 300 }))}</div>${fld("Prioritas", sel([1, 2, 3, 4].map((p) => [String(p), AG.PRIORITY[p].n]), "3"))}</div>
      <div class="grid3" style="gap:12px"><div data-nt-lbl>${fld("Label", AG.combo({ options: Object.keys(AG.LABELS).map((l) => ({ v: l, l, lead: `<span class="dot round" style="background:${AG.LABELS[l]}"></span>` })), value: [], multi: true, ph: "Tanpa label", w: 240 }))}</div>${fld("Sprint", sel(["Backlog", "Sprint 4", "Sprint 5"], "Backlog"))}<div data-nt-epic>${fld(sub ? "Induk" : "Epic", AG.combo({ options: [{ v: "", l: "Tanpa epic" }, ...ALL().filter((t) => (sub ? t.type !== "subtask" : t.type === "epic")).map((t) => ({ v: t.k, l: `${t.k} ${t.t}`, lead: typeIcon(t.type) }))], value: sub || "", w: 320 }))}</div></div>
      <div class="box soft" data-nt-est style="padding:12px 14px"><div class="row mut sm">${ic("sparkles", "s14")}Isi judul untuk mendapat usulan story point dan estimasi waktu dari AI.</div></div>`,
    foot: `<label class="row sm mut" style="margin-right:auto">${sw(false, "Buat lagi")}Buat lagi</label>${btn("Batal", "", "", { "data-close": true })}${btn(sub ? "Buat subtask" : "Buat tiket", "pri", "", { "data-act": "nt-create", "data-arg": sub || "" })}`,
    after(el) {
      const ti = el.querySelector("[data-nt-title]"), est = el.querySelector("[data-nt-est]"); let tm;
      ti.focus();
      ti.addEventListener("input", () => { clearTimeout(tm); tm = setTimeout(() => {
        const v = ti.value.trim(); if (v.length < 6) return;
        const pts = Math.min(13, [1, 2, 3, 5, 8][Math.min(4, Math.floor(v.length / 12))]);
        est.innerHTML = `<div class="row top" style="gap:10px"><span class="tile acc">${ic("sparkles")}</span><div class="grow col g4"><b class="b5">Usulan AI: ${pts} poin · sekitar ${pts > 3 ? "1,5 hari" : "4 jam"}</b><span class="mut sm">AI ${pts > 3 ? "4 j" : "1 j 30 m"}, manusia ${pts > 3 ? "5 j" : "2 j"}. Mirip KSR-22 dan KSR-26 · menyentuh <code>kasir-api/src/checkout</code>.</span></div>${btn("Pakai", "sm", "check", { "data-toast": "Estimasi AI dipakai" })}</div>`;
      }, 250); });
    } };
});
AG.action("nt-create", (el, parent) => {
  const d = el.closest(".dialog"); const ti = d.querySelector("[data-nt-title]"); const v = ti.value.trim();
  if (!v) { ti.classList.add("err"); ti.focus(); if (!d.querySelector(".nt-err")) ti.insertAdjacentHTML("afterend", `<span class="errt nt-err">Judul wajib diisi.</span>`); return; }
  const [stage, p, sprint] = [...d.querySelectorAll("select")].map((s) => s.value);
  const who = comboVal(d, "[data-nt-who]"), lbl = comboVal(d, "[data-nt-lbl]") || [], ep = comboVal(d, "[data-nt-epic]");
  const type = d.querySelector('[data-radio="nt-type"].on')?.dataset.v || "story"; const k = `KSR-${nextNum()}`;
  const par = parent || (type === "subtask" ? ep : null);
  AG.TICKETS.push({ k, type, stage: par ? "dev" : stage, state: par ? "siap" : stage === "backlog" ? "siap" : "antre", t: v, p: +p, who: who || null, lbl: [...lbl], pts: 3, parent: par || undefined, epic: !par && ep ? ep : undefined, sprint: sprint === "Backlog" ? undefined : sprint, agent: stage !== "backlog" && !par ? "Menunggu runner" : undefined });
  const ed = d.querySelector(".ed"); if (ed && !edEmpty(ed)) DESC[k] = edText(ed);
  const again = d.querySelector(".df .sw.on");
  if (again) { ti.value = ""; ti.focus(); AG.toast(`${k} dibuat`); return; }
  AG.closeDialog(); AG.rerender(); AG.toast(`${k} dibuat${par ? ` sebagai subtask ${par}` : ""}`);
});
AG.dialog("delete-ticket", (arg = "") => { const ks = arg.split(",").filter(Boolean); return { icon: "trash", tone: "dng", title: ks.length > 1 ? `Hapus ${ks.length} tiket?` : `Hapus ${ks[0]}?`, desc: "Tiket dipindah ke sampah dan bisa dipulihkan selama 30 hari. Job yang berjalan dihentikan.",
  body: `<div class="box">${ks.slice(0, 5).map((k) => T(k) ? `<div class="lrow" style="min-height:40px">${typeIcon(T(k).type)}<span class="mono mut">${k}</span><span class="trunc">${T(k).t}</span></div>` : "").join("")}${ks.length > 5 ? `<div class="lrow mut" style="min-height:36px">dan ${ks.length - 5} lainnya</div>` : ""}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Pindah ke sampah", "dngs", "trash", { "data-act": "del-confirm", "data-arg": arg })}` }; });
AG.action("del-confirm", (el, a) => { const ks = a.split(","); trashTickets(ks); if (AG._dts.kl) AG._dts.kl.sel.clear(); AG.closeDialog(); if (location.hash.startsWith("#/t/")) location.hash = "#/p/KSR/trash"; else AG.rerender(); AG.toast(`${ks.length > 1 ? ks.length + " tiket" : ks[0]} dipindah ke sampah`); });
AG.dialog("relation-add", (k) => ({ icon: "link", tone: "acc", title: `Tambah relasi untuk ${k}`, desc: "Relasi “memblokir” ditegakkan: tiket yang diblokir tidak bisa mulai sebelum pemblokir selesai.",
  body: `${fld("Jenis relasi", sel([["blocks", "Memblokir"], ["blockedby", "Diblokir oleh"], ["relates", "Terkait dengan"], ["duplicates", "Duplikat dari"]], "blocks"))}
    <div data-rel-t>${fld("Tiket", AG.combo({ options: ALL().filter((t) => t.k !== k).map((t) => ({ v: t.k, l: `${t.k} ${t.t}`, sub: `${AG.stage(t.stage).n} · ${AG.STATES[t.state].n}`, lead: typeIcon(t.type), group: t.stage === "done" ? "Selesai" : "Aktif" })).sort((a, b) => (a.group > b.group ? 1 : -1)), ph: "Cari kode atau judul…", w: 400 }))}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Tambah relasi", "pri", "", { "data-act": "rel-save", "data-arg": k })}` }));
AG.action("rel-save", (el, k) => {
  const d = el.closest(".dialog"); const ty = d.querySelector("select").value; const o = comboVal(d, "[data-rel-t]"); if (!o) return AG.toast("Pilih tiket dulu", "dng");
  const inv = { blocks: "blockedby", blockedby: "blocks", relates: "relates", duplicates: "relates" }[ty];
  (REL[k] ||= []).push([ty, o]); (REL[o] ||= []).push([inv, k]); AG.closeDialog(); rr(); AG.toast(`${k} ${REL_N[ty].toLowerCase()} ${o}`);
});
AG.dialog("bulk-edit", () => { const n = S.sel.size; return { w: "w560", icon: "adjustments-horizontal", tone: "acc", title: n ? `Ubah ${n} tiket` : "Ubah tiket terpilih", desc: "Field yang dibiarkan “Tidak diubah” tetap seperti semula. Perubahan stage tetap dicek gate per tiket.",
  body: `<div class="grid2">${fld("Stage", sel([["", "Tidak diubah"], ...AG.STAGES.map((s) => [s.k, s.n])], ""))}<div data-bk-who>${fld("Penerima", AG.combo({ options: PEOPLE_OPTS([{ v: "", l: "Tidak diubah" }, { v: "-", l: "Kosongkan", lead: av(null, 20) }]), value: "", w: 300 }))}</div>${fld("Prioritas", sel([["", "Tidak diubah"], ...[1, 2, 3, 4].map((p) => [String(p), AG.PRIORITY[p].n])], ""))}${fld("Tambah label", sel([["", "Tidak diubah"], ...Object.keys(AG.LABELS).map((l) => [l, l])], ""))}${fld("Sprint", sel([["", "Tidak diubah"], ["Sprint 4", "Sprint 4"], ["Sprint 5", "Sprint 5"]], ""))}</div>
    <div class="row wrap g4">${[...S.sel].map((k) => `<span class="pill gray mono">${k}</span>`).join("")}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn(n ? `Terapkan ke ${n} tiket` : "Terapkan", "pri", "", { "data-act": "bulk-apply" })}` }; });
AG.action("bulk-apply", (el) => {
  const d = el.closest(".dialog"); const [stage, p, lbl, sprint] = [...d.querySelectorAll("select")].map((s) => s.value); const who = comboVal(d, "[data-bk-who]");
  let moved = 0; const denied = [];
  [...S.sel].forEach((k) => { const t = T(k); if (!t) return; if (who) t.who = who === "-" ? null : who; if (p) t.p = +p; if (lbl && !t.lbl.includes(lbl)) t.lbl.push(lbl); if (sprint) t.sprint = sprint;
    if (stage) { const r = check(t, stage); if (r.same) return; if (r.ok) { applyMove(t, stage); moved++; } else denied.push(k); } });
  const n = S.sel.size; if (AG._dts.kl) AG._dts.kl.sel.clear(); AG.closeDialog(); AG.rerender();
  AG.toast(denied.length ? `${moved} tiket dipindah, ${denied.length} ditolak gate (${denied.join(", ")})` : `${n} tiket diperbarui`, denied.length ? "dng" : "");
});

/* ====================== M3 · Workflow & gate ====================== */
const TEMPLATES = [
  { k: "scrum", n: "Scrum", d: "Sprint dengan review dan retrospektif.", s: ["Backlog", "Sprint", "Development", "Review", "Selesai"] },
  { k: "kanban", n: "Kanban", d: "Alur kontinu dengan WIP limit.", s: ["To do", "Dikerjakan", "Review", "Selesai"] },
  { k: "bmad", n: "BMAD", d: "Agent berperan: analis, PM, architect, dev, QA.", s: ["Analyst", "PRD", "Architect", "Story", "Dev", "QA"] },
  { k: "spec", n: "Spec-driven", d: "Spesifikasi dulu, lalu rencana dan tugas.", s: ["Spec", "Plan", "Tasks", "Implement", "Verify"] },
  { k: "shapeup", n: "Shape Up", d: "Siklus 6 minggu dengan betting table.", s: ["Shaping", "Betting", "Building", "Cooldown"] },
  { k: "bug", n: "Bug Triage", d: "Laporan bug sampai verifikasi perbaikan.", s: ["Laporan", "Triage", "Reproduksi", "Fix", "Verifikasi"] },
  { k: "waterfall", n: "Waterfall", d: "Tahap berurutan dengan approve di tiap gerbang.", s: ["Requirement", "Desain", "Implementasi", "Testing", "Deploy"] },
  { k: "superpowers", n: "Superpowers-style", d: "Brainstorm, rencana, eksekusi bertahap dengan TDD.", s: ["Brainstorm", "Plan", "Eksekusi + TDD", "Review"] },
  { k: "hotfix", n: "Hotfix/Incident", d: "Respons cepat untuk insiden produksi.", s: ["Triage", "Fix", "Deploy", "Postmortem"] },
];
const WFS = [
  { k: "superpowers", n: "Superpowers · Kasir Online", from: "Superpowers-style", used: "Kasir Online · Story, Task", st: 9, ver: 7, at: "2 hari lalu", cols: AG.STAGES.map((s) => s.c) },
  { k: "scrum-gdg", n: "Scrum · Gudang", from: "Scrum", used: "Aplikasi Gudang", st: 5, ver: 3, at: "1 minggu lalu", cols: ["#8d8d99", "#0891b2", "#0f8a5f", "#2563eb", "#1c7f45"] },
  { k: "kanban", n: "Kanban", from: "Kanban", used: "Portal Pelanggan", st: 4, ver: 1, at: "3 minggu lalu", cols: ["#8d8d99", "#0f8a5f", "#2563eb", "#1c7f45"] },
  { k: "bug", n: "Bug Triage", from: "Bug Triage", used: "Semua project · Bug", st: 5, ver: 2, at: "5 hari lalu", cols: ["#cf2e2e", "#c2410c", "#a16207", "#0f8a5f", "#1c7f45"] },
  { k: "subtask", n: "Subtask ringkas", from: "Bawaan", used: "Semua project · Subtask", st: 3, ver: 1, at: "11 Nov", cols: ["#0f8a5f", "#2563eb", "#1c7f45"] },
];
function wfList() {
  return AG.shell({ nav: "settings", width: "narrow", crumbs: ["Pengaturan", "Workflow"],
    body: `${head("Workflow", "Alur stage, persona, dan gate. Setiap tipe tiket boleh punya workflow sendiri.", `${btn("Impor YAML", "", "upload", { "data-dialog": "yaml-import" })}${btn("Workflow baru", "pri", "plus", { "data-dialog": "wf-clone", "data-arg": "kosong" })}`)}
      <div data-tabs-root class="col g20"><nav class="tabs" role="tablist"><button type="button" class="on" data-tab="mine">Workflow di workspace<span class="n">${WFS.length}</span></button><button type="button" data-tab="tpl">Template bawaan<span class="n">${TEMPLATES.length}</span></button><button type="button" data-tab="yaml">Ekspor & impor</button></nav>
      <div data-panel="mine" class="on col g12">${xcard({ title: "Workflow di workspace", desc: "Buka workflow untuk mengubah stage, persona, dan gate di builder.", pad: false, body: `<div class="box">${WFS.map((w) => `<div class="lrow" data-go="#/settings/workflows/${w.k}" style="flex-wrap:wrap">
        <span class="tile">${ic("route")}</span><div class="grow" style="min-width:180px"><b class="b5">${w.n}</b><div class="mut xs">${w.st} stage · dari ${w.from} · v${w.ver} · diubah ${w.at}</div></div>
        <span class="wf-strip hide-sm" aria-hidden="true">${w.cols.map((c) => `<i style="background:${c}"></i>`).join("")}</span>
        <span class="mut sm hide-sm" style="width:170px">${w.used}</span>${btn("", "ghost sm icon", "dots", { "data-menu": "wf-row", "data-arg": w.k, "aria-label": `Aksi ${w.n}` })}</div>`).join("")}</div>` })}</div>
      <div data-panel="tpl" class="col g12">${xcard({ title: "Template bawaan", desc: "Clone template lalu ubah sesuai tim Anda. Template asli tidak berubah.", body: `<div class="grid2">${TEMPLATES.map((x) => `<div class="box wf-tpl"><div class="row"><b class="b5 grow">${x.n}</b>${btn("Clone", "sm", "copy", { "data-dialog": "wf-clone", "data-arg": x.k })}</div><p class="mut sm">${x.d}</p>
        <div class="wf-chain">${x.s.map((s, i) => `${i ? ic("chevron-right") : ""}<span>${s}</span>`).join("")}</div></div>`).join("")}</div>` })}</div>
      <div data-panel="yaml" class="col g12">${xcard({ title: "Ekspor & impor", desc: "Workflow disimpan di database. YAML dipakai untuk berbagi antar workspace atau disimpan di repo.", pad: false, body: `<div class="box">${srow("Ekspor semua workflow", "Satu berkas YAML berisi stage, gate, persona, dan penugasan default.", btn("Ekspor YAML", "", "download", { "data-dialog": "wf-yaml", "data-arg": "superpowers" }))}${srow("Impor dari YAML", "Divalidasi dulu sebelum disimpan. Persona yang tidak dikenal bisa dipetakan.", btn("Impor YAML", "", "upload", { "data-dialog": "yaml-import" }))}</div>` })}</div></div>` });
}
AG.screen({ m: "M3", g: "Workflow & gate", n: "Daftar workflow", route: "/settings/workflows", render: wfList });
AG.menu("wf-row", (k) => `<a class="mi" href="#/settings/workflows/${k}">${ic("pencil")}Buka builder</a><button class="mi" data-dialog="wf-clone" data-arg="${k}">${ic("copy")}Duplikat</button><button class="mi" data-dialog="wf-yaml" data-arg="${k}">${ic("download")}Ekspor YAML</button><div class="msep"></div><button class="mi dng" data-toast="Workflow yang masih dipakai tidak bisa dihapus" data-tone="dng">${ic("trash")}Hapus</button>`);
AG.dialog("wf-clone", (k) => { const tp = TEMPLATES.find((x) => x.k === k) || WFS.find((x) => x.k === k); const blank = k === "kosong"; return { icon: blank ? "route" : "copy", tone: "acc", title: blank ? "Workflow baru" : `Clone ${tp?.n}`, desc: blank ? "Mulai dari dua stage, lalu tambah stage di builder." : "Salinan bisa diubah tanpa memengaruhi template asli.",
  body: `${fld("Nama", inp(blank ? "" : `${tp?.n} · salinan`, { a: { "data-wf-name": "" }, ph: "Contoh: Alur tim mobile" }))}${fld("Pakai untuk", sel(["Belum dipakai", "Kasir Online · Bug", "Aplikasi Gudang · semua tipe", "Portal Pelanggan · Story"], "Belum dipakai"), "Bisa diatur nanti di pengaturan workflow project.")}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn(blank ? "Buat" : "Clone", "pri", "", { "data-act": "wf-clone-save", "data-arg": k })}` }; });
AG.action("wf-clone-save", (el, k) => { const n = el.closest(".dialog").querySelector("[data-wf-name]").value.trim() || "Workflow baru"; const tp = TEMPLATES.find((x) => x.k === k); WFS.push({ k: "superpowers", n, from: tp ? tp.n : "Kosong", used: "Belum dipakai", st: tp ? tp.s.length : 2, ver: 1, at: "baru saja", cols: (tp ? tp.s : [1, 2]).map((_, i) => ["#0891b2", "#7c3aed", "#0f8a5f", "#2563eb", "#c2410c", "#1c7f45"][i % 6]) }); AG.closeDialog(); AG.rerender(); AG.toast(`Workflow “${n}” dibuat`); });

/* Definisi stage workflow Superpowers (dengan cabang paralel). */
const WF = {};
const defStage = (s, x = {}) => (WF[s.k] = { k: s.k, n: s.n, i: s.i, c: s.c, persona: s.persona || null, mode: s.mode || (s.persona ? "otomatis" : "manual"), gates: [...(s.gate || [])], skill: "", prompt: "", in: [], out: "", tools: [], exec: [], appr: ["OO"], watch: [], trans: s.persona ? "approve" : "manual", fail: 3, code: "pr", danger: {}, ...x });
AG.STAGES.forEach((s) => defStage(s));
defStage({ k: "architect", n: "Architect", persona: "architect", i: "hierarchy-2", c: "#4f46e5", gate: ["Artifact ada"] }, { skill: "bmad:architect", out: "tech-spec.md", in: ["prd.md"], appr: ["AA"], tools: [] });
Object.assign(WF.brainstorm, { skill: "superpowers:brainstorming", out: "brainstorm.md", exec: ["OO"], trans: "manual", prompt: "Gali masalah toko, alternatif solusi, dan risiko. Hasilkan draft tiket." });
Object.assign(WF.prd, { skill: "bmad:pm", in: ["brainstorm.md"], out: "prd.md", prompt: "Tulis PRD dengan acceptance criteria yang bisa diuji." });
Object.assign(WF.design, { skill: "ui-ux-pro-max", in: ["prd.md"], out: "mockup.html", tools: ["Playwright CLI"], appr: ["OO", "AA"], watch: ["DD"], prompt: "Buat alur dan mockup HTML dengan design system Kasir Online. Sertakan screenshot 1440 dan 390 px." });
Object.assign(WF.dev, { skill: "superpowers:test-driven-development", in: ["prd.md", "tech-spec.md", "mockup.html"], out: "PR", tools: ["cargo", "pnpm"], exec: ["DD"], appr: ["AA"], watch: ["OO"], prompt: "Kerjakan bertahap dengan TDD. Buka PR per repo." });
Object.assign(WF.review, { skill: "code-review", in: ["PR"], out: "review-ai.md", appr: ["AA", "OO"], prompt: "Review PR per baris: keamanan, kebenaran, konsistensi." });
Object.assign(WF.test, { skill: "playwright-qa", in: ["prd.md", "PR"], out: "laporan-test.md", tools: ["Playwright CLI"], exec: ["QQ"], appr: ["QQ"], prompt: "Uji acceptance criteria dengan Playwright, ambil screenshot tiap skenario." });
const WF_ROWS = [["backlog"], ["brainstorm"], ["prd"], ["design", "architect"], ["dev"], ["review"], ["test"], ["done"]];
const GATE_TYPES = [["Artifact ada", "file-text"], ["Screenshot ada", "photo"], ["Hasil test lulus", "test-pipe"], ["PR terbuka", "git-pull-request"], ["Semua PR merged", "git-merge"], ["CI hijau", "circle-check"], ["Review AI selesai", "git-compare"], ["Acceptance criteria terisi", "list-check"], ["Mockup HTML ada", "layout-sidebar"], ["Catatan brainstorm ada", "bulb"], ["Test lulus", "test-pipe"], ["Artifact PRD ada", "file-text"]];
const gIcon = (n) => (GATE_TYPES.find((g) => g[0] === n) || [0, "shield-check"])[1];
const DANGER = [["Migrasi database", "ask"], ["git push --force", "deny"], ["Hapus berkas di luar worktree", "deny"], ["Install dependensi baru", "allow"], ["Akses jaringan ke luar", "ask"]];
const SKILLS = ["superpowers:brainstorming", "bmad:pm", "bmad:architect", "ui-ux-pro-max", "superpowers:test-driven-development", "code-review", "playwright-qa", "devops:deploy-staging"];
let wfDirty = 0;

function wfCanvas() {
  const W = 600, NW = 236, NH = 56, RH = 100, cx = (W - NW) / 2;
  const pos = {};
  WF_ROWS.forEach((row, r) => { const y = 24 + r * RH; if (row.length === 1) pos[row[0]] = { x: cx, y }; else { const tot = NW * 2 + 32; row.forEach((k, i) => (pos[k] = { x: (W - tot) / 2 + i * (NW + 32), y })); } });
  const H = 24 + WF_ROWS.length * RH;
  const paths = [], badges = [];
  for (let r = 0; r < WF_ROWS.length - 1; r++) for (const a of WF_ROWS[r]) for (const b of WF_ROWS[r + 1]) {
    const A = pos[a], B = pos[b]; const x1 = A.x + NW / 2, y1 = A.y + NH, x2 = B.x + NW / 2, y2 = B.y; const my = (y1 + y2) / 2;
    paths.push(`<path d="M${x1} ${y1} C ${x1} ${my}, ${x2} ${my}, ${x2} ${y2 - 6}" fill="none" stroke="var(--line2)" stroke-width="1.5"/><path d="M${x2 - 4} ${y2 - 8} L${x2} ${y2 - 2} L${x2 + 4} ${y2 - 8}" fill="none" stroke="var(--input)" stroke-width="1.5"/>`);
  }
  WF_ROWS.slice(0, -1).flat().forEach((k) => { const A = pos[k], s = WF[k]; const auto = s.trans === "auto"; badges.push(`<span class="wf-badge ${auto ? "auto" : ""}" style="left:${A.x + NW / 2}px;top:${A.y + NH + 14}px" title="${auto ? "Transisi otomatis (gate tetap dicek)" : s.trans === "manual" ? "Dipindah manual" : "Tunggu approve manusia"}">${ic(auto ? "bolt" : s.trans === "manual" ? "hand-stop" : "user-check")}</span>`); });
  const T1 = pos.test, D1 = pos.dev; const lx = T1.x + NW, ly1 = T1.y + NH / 2, ly2 = D1.y + NH / 2;
  paths.push(`<path d="M${lx} ${ly1} C ${lx + 70} ${ly1}, ${lx + 70} ${ly2}, ${lx + 6} ${ly2}" fill="none" stroke="var(--danger)" stroke-width="1.5" stroke-dasharray="4 4"/><path d="M${lx + 12} ${ly2 - 4} L${lx + 5} ${ly2} L${lx + 12} ${ly2 + 4}" fill="none" stroke="var(--danger)" stroke-width="1.5"/>`);
  const P1 = pos.design, P2 = pos.architect;
  paths.push(`<rect x="${P1.x - 10}" y="${P1.y - 12}" width="${P2.x + NW - P1.x + 20}" height="${NH + 24}" rx="14" fill="none" stroke="var(--accent)" stroke-opacity=".45" stroke-dasharray="3 4"/>`);
  const nodes = Object.keys(pos).map((k) => { const s = WF[k], p = pos[k]; const per = s.persona ? AG.PERSONAS[s.persona] : null;
    return `<button type="button" class="wf-node ${S.wfSel === k ? "on" : ""}" style="left:${p.x}px;top:${p.y}px" data-act="wf-sel" data-arg="${k}" aria-pressed="${S.wfSel === k}"><span class="wi" style="color:${s.c}">${ic(s.i)}</span><span class="grow" style="min-width:0"><b>${s.n}</b><small>${per ? per.n : "Tanpa agent"} · ${s.mode === "interaktif" ? "Interaktif" : s.mode === "otomatis" ? "Otomatis" : "Manual"}</small></span>${s.gates.length ? `<span class="gc" title="${s.gates.length} gate">${ic("shield-check", "s14")}${s.gates.length}</span>` : ""}</button>`; }).join("");
  return `<div class="wf-canvas" style="width:${W + 110}px;height:${H}px"><svg class="wf-lines" width="${W + 110}" height="${H}" aria-hidden="true">${paths.join("")}</svg>${badges.join("")}${nodes}
    <span class="wf-tag" style="left:${P2.x + NW - 92}px;top:${P1.y - 21}px">Paralel setelah PRD</span>
    <span class="wf-tag dng" style="left:${lx + 30}px;top:${(ly1 + ly2) / 2 - 8}px">Gagal, maks 3×</span></div>`;
}
function wfYaml() {
  return `workflow: superpowers-kasir
versi: 7
stages:
${WF_ROWS.map((row) => row.map((k) => { const s = WF[k]; return `  - id: ${k}${row.length > 1 ? `\n    paralel_dengan: ${row.find((x) => x !== k)}` : ""}
    nama: ${s.n}${s.persona ? `\n    persona: ${s.persona}\n    mode: ${s.mode}` : ""}${s.skill ? `\n    skill: ${s.skill}` : ""}${s.gates.length ? `\n    gate: [${s.gates.map((g) => `"${g}"`).join(", ")}]` : ""}
    transisi: ${s.trans === "auto" ? "otomatis" : s.trans === "approve" ? "tunggu_approve" : "manual"}${s.persona ? `\n    batas_gagal: ${s.fail}` : ""}`; }).join("\n")).join("\n")}`;
}
function wfConfig() {
  const s = WF[S.wfSel]; const per = s.persona;
  const c = (f, o) => `<div data-wfset="${f}">${AG.combo(o)}</div>`;
  const segOf = (opts, cur, act) => `<div class="seg" role="group">${opts.map(([v, l, i]) => `<button type="button" class="${cur === v ? "on" : ""}" data-act="${act}" data-arg="${s.k}|${v}">${i ? ic(i, "s14") : ""}${l}</button>`).join("")}</div>`;
  const trans = [["approve", "Tunggu approve", "Setelah job selesai, tiket “Menunggu review” sampai approver menyetujui.", "user-check"], ["auto", "Otomatis", "Approve dilewati, tetapi gate tetap dicek server.", "bolt"], ["manual", "Manual", "Tiket dipindah manusia lewat board.", "hand-stop"]];
  return `<div class="ch"><span class="tile" style="color:${s.c}">${ic(s.i, "s18")}</span><div class="grow"><div class="mut xs">Stage</div><b class="md">${s.n}</b></div>${btn("", "ghost sm icon", "trash", { "data-dialog": "wf-del-stage", "data-arg": s.k, "aria-label": `Hapus stage ${s.n}` })}</div>
    <div class="cs"><h3>Umum</h3>${fld("Nama stage", inp(s.n, { a: { "data-wf-dirty": "" } }))}
      ${fld("Persona agent", c("persona", { options: [{ v: "", l: "Tanpa agent (manual)", lead: av(null, 20) }, ...Object.entries(AG.PERSONAS).map(([k, p]) => ({ v: k, l: p.n, sub: p.t, lead: av(k, 20) }))], value: per || "", w: 320 }))}
      ${per ? fld("Mode", segOf([["otomatis", "Otomatis", "bolt"], ["interaktif", "Interaktif", "message-2"]], s.mode, "wf-mode"), s.mode === "interaktif" ? "Butuh percakapan dengan manusia, misalnya brainstorming." : "Job jalan di runner tanpa diawasi.") : ""}</div>
    ${per ? `<div class="cs"><h3>Instruksi</h3>${fld("Skill", c("skill", { options: SKILLS.map((x) => ({ v: x, l: x, sub: x.split(":")[0] })), value: s.skill || null, ph: "Pilih skill", w: 320 }))}
      ${fld("Prompt tambahan", inp(s.prompt, { area: true, ph: "Instruksi khusus untuk stage ini", a: { "data-wf-dirty": "" } }))}
      ${fld("Artifact input", `<div class="row wrap g4">${s.in.map((x) => `<span class="pill gray mono">${x}</span>`).join("") || `<span class="mut sm">Tidak ada</span>`}</div>`)}
      ${fld("Template output", `<div class="row"><span class="pill mono">${ic("file-text")}${s.out || "–"}</span></div>`)}</div>` : ""}
    <div class="cs"><div class="row"><h3 class="grow">Gate rules</h3><button type="button" class="chip" data-menu="wf-gate-add" data-arg="${s.k}">${ic("plus", "s14")}Tambah cek</button></div>
      ${s.gates.length ? `<div class="col g4">${s.gates.map((g) => `<div class="grule">${ic(gIcon(g), "s14 faint")}<span class="grow">${g}</span>${btn("", "ghost sm icon", "x", { "data-act": "wf-gate-del", "data-arg": `${s.k}|${g}`, "aria-label": `Hapus gate ${g}` })}</div>`).join("")}</div>` : `<span class="mut sm">Belum ada gate. Tiket bisa keluar tanpa cek.</span>`}
      <span class="hint">Transisi ditolak server bila gate belum terpenuhi. Evidence ditandatangani runner.</span>
      ${fld("Tooling wajib di runner", `<div class="row wrap g4">${s.tools.map((x) => `<span class="pill">${ic("terminal-2")}${x}</span>`).join("")}<button type="button" class="chip" data-toast="Pilih tool yang wajib ada di runner">${ic("plus", "s14")}Tool</button></div>`)}</div>
    <div class="cs"><h3>Penugasan default</h3>
      ${fld("Executor", c("exec", { options: PEOPLE_OPTS(), value: s.exec, multi: true, ph: "Belum ada", w: 300 }))}
      ${fld("Approver", c("appr", { options: PEOPLE_OPTS(), value: s.appr, multi: true, ph: "Belum ada", w: 300 }), "Cukup satu approver yang approve.")}
      ${fld("Watcher", c("watch", { options: PEOPLE_OPTS(), value: s.watch, multi: true, ph: "Belum ada", w: 300 }))}</div>
    <div class="cs"><h3>Transisi keluar</h3><div class="col g4" role="radiogroup" aria-label="Transisi keluar">${trans.map(([v, l, d, i]) => `<button type="button" class="wf-opt ${s.trans === v ? "on" : ""}" data-act="wf-trans" data-arg="${s.k}|${v}" role="radio" aria-checked="${s.trans === v}"><span class="radio ${s.trans === v ? "on" : ""}"></span><span class="grow"><b>${l}</b><small>${d}</small></span><span class="faint">${ic(i, "s14")}</span></button>`).join("")}</div>
      ${per ? fld("Batas putaran gagal", `<div style="max-width:96px">${inp(String(s.fail), { type: "number", a: { min: 1, max: 9, "aria-label": "Batas putaran gagal", "data-wf-dirty": "" } })}</div>`, "Lewat batas, tiket berstatus Butuh keputusan.") : ""}</div>
    ${per ? `<div class="cs"><h3>Izin aksi berbahaya</h3><span class="hint" style="margin-top:-6px">Berlaku untuk Agent ${AG.PERSONAS[per].n} di stage ini.</span>${DANGER.map(([n, d]) => `<div class="wf-dz"><span>${n}</span>${sel([["ask", "Tanya"], ["allow", "Izinkan"], ["deny", "Tolak"]], s.danger[n] || (s.k === "dev" && n === "Migrasi database" ? "ask" : d))}</div>`).join("")}</div>` : ""}
    ${["dev", "review", "test"].includes(s.k) ? `<div class="cs"><h3>Hasil kode</h3>${segOf([["pr", "PR otomatis", "git-pull-request"], ["manual", "Manual", "user"]], s.code, "wf-code")}<span class="hint">${s.code === "pr" ? "Agent membuat branch dan PR per repo yang terdampak." : "Perubahan tinggal di worktree, manusia yang membuat commit dan PR."}</span></div>` : ""}`;
}
function builder(id) {
  const tabs = [["builder", "Builder", "sitemap"], ["yaml", "YAML", "code"]];
  return AG.shell({ nav: "settings", crumbs: [`<a href="#/settings/workflows">Workflow</a>`, "Superpowers · Kasir Online"], flush: true,
    tabs: `<nav class="vtabs" aria-label="Mode workflow">${tabs.map(([k, l, i]) => `<button type="button" class="vtab ${S.wfMode === k ? "on" : ""}" data-act="wf-view" data-arg="${k}">${ic(i, "s14")}${l}</button>`).join("")}</nav>`,
    mtabs: `<nav class="mtabs" aria-label="Mode workflow">${tabs.map(([k, l]) => `<button type="button" class="vtab ${S.wfMode === k ? "on" : ""}" data-act="wf-view" data-arg="${k}">${l}</button>`).join("")}</nav>`,
    actions: `${pill("Draf v8", "warn")}${btn("Riwayat versi", "ghost hide-sm", "history", { "data-toast": "v7 aktif sejak 10 Nov · 7 versi tersimpan" })}`,
    body: `<div class="wf"><div class="wf-main">
      ${S.wfMode === "yaml" ? "" : `<div class="wf-tools">${btn("Tambah stage", "sm", "plus", { "data-dialog": "wf-stage-add" })}<span class="grow"></span><div class="wf-legend hide-sm"><span>${ic("user-check", "s14")}Tunggu approve</span><span class="text-acc">${ic("bolt", "s14")}Otomatis</span><span>${ic("hand-stop", "s14")}Manual</span><span>${ic("shield-check", "s14")}Jumlah gate</span></div></div>`}
      <div class="wf-canvas-wrap">${S.wfMode === "yaml" ? `<div style="padding:20px"><div class="code">${esc(wfYaml())}</div></div>` : wfCanvas()}</div>
      ${sbar("Simpan workflow", "wf-save", { msg: `<span data-wf-msg>Draf v8 · ${wfDirty ? `${wfDirty} perubahan belum disimpan` : "12 tiket berjalan ikut dimigrasi saat disimpan"}</span>`, extra: btn("Ekspor YAML", "ghost", "download", { "data-dialog": "wf-yaml", "data-arg": id }), cancel: false })}</div>
      <aside class="wf-cfg" aria-label="Konfigurasi stage">${wfConfig()}</aside></div>` });
}
const wfTouch = () => { wfDirty++; const m = document.querySelector("[data-wf-msg]"); if (m) m.textContent = `Draf v8 · ${wfDirty} perubahan belum disimpan`; };
AG.screen({ m: "M3", g: "Workflow & gate", n: "Workflow builder", route: "/settings/workflows/:id", path: "/settings/workflows/superpowers", render: (p) => builder(p.id),
  after: (root) => {
    const app = root.querySelector("#app");
    root.querySelector(".wf-node.on")?.scrollIntoView({ block: "nearest", inline: "nearest" });
    app.addEventListener("combo-change", (e) => { const f = e.target.closest("[data-wfset]")?.dataset.wfset; if (!f) return; const s = WF[S.wfSel];
      if (f === "persona") { s.persona = e.detail || null; s.mode = s.persona ? (s.mode === "manual" ? "otomatis" : s.mode) : "manual"; wfDirty++; rerenderWf(); return; }
      if (f === "skill") s.skill = e.detail; else s[f] = [...e.detail]; wfTouch(); });
    app.addEventListener("input", (e) => { if (e.target.closest("[data-wf-dirty]")) wfTouch(); });
    app.addEventListener("change", (e) => { const z = e.target.closest(".wf-dz"); if (z) { WF[S.wfSel].danger[z.querySelector("span").textContent] = e.target.value; wfTouch(); } });
  } });
function rerenderWf() { const w = document.querySelector(".wf-canvas-wrap"), c = document.querySelector(".wf-cfg"); const y = w?.scrollTop, cy = c?.scrollTop; AG.rerender(); const w2 = document.querySelector(".wf-canvas-wrap"), c2 = document.querySelector(".wf-cfg"); if (w2) w2.scrollTop = y; if (c2 && cy != null) c2.scrollTop = cy; }
AG.action("wf-sel", (el, k) => { S.wfSel = k; const w = document.querySelector(".wf-canvas-wrap"); const y = w?.scrollTop; AG.rerender(); const w2 = document.querySelector(".wf-canvas-wrap"); if (w2) w2.scrollTop = y; });
AG.action("wf-view", (el, a) => { S.wfMode = a; AG.rerender(); });
AG.action("wf-mode", (el, a) => { const [k, v] = a.split("|"); WF[k].mode = v; wfDirty++; rerenderWf(); });
AG.action("wf-trans", (el, a) => { const [k, v] = a.split("|"); WF[k].trans = v; wfDirty++; rerenderWf(); });
AG.action("wf-code", (el, a) => { const [k, v] = a.split("|"); WF[k].code = v; wfDirty++; rerenderWf(); });
AG.action("wf-gate-del", (el, a) => { const [k, g] = a.split("|"); WF[k].gates = WF[k].gates.filter((x) => x !== g); wfDirty++; rerenderWf(); });
AG.action("wf-gate-add", (el, a) => { const [k, g] = a.split("|"); if (!WF[k].gates.includes(g)) WF[k].gates.push(g); wfDirty++; rerenderWf(); AG.toast(`Gate “${g}” ditambahkan ke ${WF[k].n}`); });
AG.action("wf-save", () => { wfDirty = 0; AG.rerender(); AG.toast("Workflow disimpan sebagai v8 · 12 tiket berjalan ikut dimigrasi"); });
AG.menu("wf-gate-add", (k) => `<div class="mlab">Tipe cek tersedia</div>${GATE_TYPES.map(([n, i]) => `<button class="mi" data-act="wf-gate-add" data-arg="${k}|${n}" ${WF[k].gates.includes(n) ? 'style="opacity:.5"' : ""}>${ic(i)}${n}${WF[k].gates.includes(n) ? `<span class="tail">${ic("check", "s14")}</span>` : ""}</button>`).join("")}`);
AG.dialog("wf-stage-add", () => ({ icon: "plus", tone: "acc", title: "Tambah stage", desc: "Stage baru berlaku untuk tiket yang belum melewati posisinya.",
  body: `${fld("Nama", inp("", { ph: "Contoh: Deploy staging" }))}<div class="grid2">${fld("Persona", sel([["devops", "DevOps"], ["writer", "Tech Writer"], ["", "Tanpa agent"]], "devops"))}${fld("Sisipkan setelah", sel(AG.STAGES.map((s) => [s.k, s.n]), "test"))}</div>
    ${optRow(false, "Jalan paralel", "Berjalan bersamaan dengan stage lain di posisi yang sama.", "Paralel")}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Tambah", "pri", "", { "data-close": true, "data-toast": "Stage ditambahkan ke draf v8" })}` }));
AG.dialog("wf-del-stage", (k) => { const n = AG.TICKETS.filter((t) => t.stage === k).length; return { icon: "trash", tone: "dng", title: `Hapus stage ${WF[k].n}?`, desc: n ? `${n} tiket sedang di stage ini. Anda akan memilih stage tujuan untuk tiket-tiket itu.` : "Tidak ada tiket di stage ini.",
  body: `<div class="alert gray">${ic("info-circle")}<span>Artifact lama tetap tersimpan. Gate versi baru berlaku sejak tiket masuk ke stage berikutnya.</span></div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn(n ? "Lanjut ke pemetaan" : "Hapus stage", "dngs", "", { "data-close": true, "data-go": "#/settings/workflows/superpowers/migrate" })}` }; });
AG.dialog("wf-yaml", () => ({ w: "w640", icon: "file-export", tone: "acc", title: "Ekspor YAML", desc: "superpowers-kasir.yaml · versi 7",
  body: `<div class="code" style="max-height:52vh">${esc(wfYaml())}</div>`,
  foot: `${btn("Salin", "", "copy", { "data-toast": "YAML disalin" })}${btn("Unduh", "pri", "download", { "data-close": true, "data-toast": "superpowers-kasir.yaml diunduh" })}` }));

/* Migrasi workflow */
function migrate() {
  const removed = [["design", "prd"], ["brainstorm", "backlog"]];
  const targets = [...AG.STAGES.filter((s) => !removed.some((r) => r[0] === s.k)), WF.architect];
  const need = AG.TICKETS.filter((t) => removed.some((r) => r[0] === t.stage)).length;
  return AG.shell({ nav: "settings", width: "mid", crumbs: [`<a href="#/settings/workflows">Workflow</a>`, `<a href="#/settings/workflows/superpowers">Superpowers</a>`, "Migrasi"],
    body: `${head("Migrasi ke workflow v8", "Ada stage yang dihapus. Pilih stage tujuan untuk tiket yang sedang berada di stage itu.", "", `<span class="tile lg warn">${ic("arrows-exchange", "s18")}</span>`)}

      ${AG.stats([{ label: "Stage dihapus", value: 2, icon: "trash", tone: "dng", hint: "Desain UI, Brainstorming" }, { label: "Tiket perlu dipetakan", value: need, icon: "arrows-exchange", tone: "warn", hint: "Pilih stage tujuan di bawah" }, { label: "Tiket lain ikut v8", value: AG.TICKETS.length - need, icon: "circle-check", tone: "ok", hint: "Tanpa perubahan stage" }])}
      ${section("Perubahan lain di v8", "", `<div class="box">${[["plus", "ok", "Stage <b>Architect</b> ditambahkan, paralel dengan Desain UI", "Tiket yang sudah lewat PRD tidak ikut stage ini."], ["shield-check", "", "Gate <b>Screenshot ada</b> ditambahkan ke Testing", "Berlaku sejak tiket masuk Testing berikutnya. KSR-29 dan KSR-30 memakai gate lama."], ["file-text", "", "Artifact <b>brainstorm.md</b> dan <b>mockup.html</b> tetap tersimpan di tiket", "Masih bisa dibuka dari navigator artifact."]].map(([i, tone, t, d]) => `<div class="lrow"><span class="tile ${tone}">${ic(i)}</span><div class="grow"><div>${t}</div><div class="mut sm">${d}</div></div></div>`).join("")}</div>`)}
      ${xcard({ title: "Pemetaan stage", desc: "Tiket mengikuti pemetaan stage, atau Anda pilih satu per satu.", body: `<div class="col g12">${removed.map(([from, to]) => { const tk = AG.TICKETS.filter((t) => t.stage === from); return `<div class="box"><div class="mg-map"><div class="row"><span class="stg dng">${ic(AG.stage(from).i, "s14")}${AG.stage(from).n}</span><span class="mut sm">dihapus · ${tk.length} tiket</span></div><span class="arr faint">${ic("arrow-right")}</span>${sel(targets.map((s) => [s.k, s.n]), to)}</div>
        ${tk.map((t) => `<div class="lrow">${tref(t.k)}<span class="grow trunc">${t.t}</span><span class="hide-sm">${statePill(t.state)}</span><div style="width:170px" class="hide-sm">${sel([["", "Ikuti pemetaan"], ...targets.map((s) => [s.k, s.n])], "")}</div></div>`).join("") || `<div class="lrow mut">Tidak ada tiket.</div>`}</div>`; }).join("")}</div>`, foot: sbar("Terapkan migrasi", "mg-apply", { msg: `${need} tiket dipindah · tercatat di audit log`, cancel: false, extra: btn("Batal", "ghost", "", { "data-go": "#/settings/workflows/superpowers" }) }) })}` });
}
AG.screen({ m: "M3", g: "Workflow & gate", n: "Migrasi workflow", route: "/settings/workflows/:id/migrate", path: "/settings/workflows/superpowers/migrate", render: migrate });
AG.action("mg-apply", () => { location.hash = "#/settings/workflows"; AG.toast("Migrasi diterapkan · tercatat di audit log"); });

/* Workflow per tipe tiket untuk project */
function projWf(k) {
  const opts = ["Superpowers · Kasir Online", "Scrum · Gudang", "Kanban", "Bug Triage", "Subtask ringkas", "Epic ringkas", "Hotfix/Incident"];
  const norm = (w) => ({ Superpowers: "Superpowers · Kasir Online", Scrum: "Scrum · Gudang" }[w] || w);
  const rows = TYPE_LIST.map((x) => [x, norm(k === "KSR" ? TYPE_WF[x.k] : x.k === "subtask" ? "Subtask ringkas" : x.k === "bug" ? "Bug Triage" : AG.project(k).wf)]);
  return AG.shell({ nav: "settings", project: k, width: "narrow", crumbs: [AG.project(k).n, "Workflow"],
    body: `${head("Workflow project", `Workflow per tipe tiket di ${AG.project(k).n}. Mengganti workflow membuka layar migrasi untuk tiket yang sedang berjalan.`)}
      ${xcard({ title: "Workflow per tipe tiket", desc: "Pilih workflow untuk setiap tipe tiket di project ini.", pad: false, foot: AG.savebar("Simpan", { msg: "Perubahan workflow tipe tiket memicu migrasi", toast: "Workflow project disimpan" }), body: `<div class="box">${rows.map(([x, w]) => `<div class="lrow" style="flex-wrap:wrap">${tico(x)}<div class="grow" style="min-width:120px"><b class="b5">${x.n}</b><div class="mut xs">${k === "KSR" ? ALL().filter((t) => t.type === x.k && t.stage !== "done").length : 0} tiket aktif</div></div>
        <div style="width:220px">${sel(opts, w)}</div>${btn("", "ghost sm icon", "external-link", { "data-go": "#/settings/workflows/superpowers", "aria-label": `Buka workflow ${x.n}` })}</div>`).join("")}</div>
      <div class="x-subh"><b class="b5">Aturan transisi</b><span class="mut sm">Siapa yang boleh melompati stage atau memaksa transisi.</span></div><div class="box">${srow("Lompat stage", "Hanya Admin, wajib alasan, tercatat di audit log.", pill("Admin", "acc", "", "lock"))}${srow("Paksa transisi", "Admin bisa memindahkan tiket walau gate belum terpenuhi, dengan alasan.", sw(true, "Izinkan paksa transisi"))}${srow("Batas WIP Development", "Kolom diberi tanda saat melewati batas.", `<div style="width:80px">${inp("4", { type: "number", a: { "aria-label": "WIP limit" } })}</div>`)}</div>` })}` });
}
AG.screen({ m: "M3", g: "Workflow & gate", n: "Workflow project", route: "/settings/p/:k/workflow", path: "/settings/p/KSR/workflow", render: (p) => projWf(p.k) });

/* ====================== M3 · Dialog ====================== */
AG.dialog("reject", (k) => ({ icon: "x", tone: "dng", title: `Reject ${k}`, desc: "Agent merevisi di stage yang sama berdasarkan catatan Anda. Artifact baru disimpan sebagai versi berikutnya.",
  body: `${fld("Catatan untuk agent", inp("", { area: true, ph: "Contoh: Tambahkan skenario QR kedaluwarsa dan uji callback ganda.", a: { "data-rej": "" } }), "Wajib diisi.")}
    <div class="row sm">${AG.cb(true, "Lampirkan komentar review AI")}<span>Lampirkan 1 catatan review AI yang masih terbuka</span></div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Reject dan minta revisi", "dngs", "x", { "data-act": "rej-submit", "data-arg": k })}` }));
AG.action("rej-submit", (el, k) => {
  const ta = el.closest(".dialog").querySelector("[data-rej]"); if (!ta.value.trim()) { ta.classList.add("err"); ta.focus(); return AG.toast("Catatan reject wajib diisi", "dng"); }
  const t = T(k); t.state = "jalan"; t.agent = "Merevisi sesuai catatan reject"; log(k, "x", `<b>${P(AG.ME).name}</b> reject: “${esc(ta.value.trim())}”`);
  AG.closeDialog(); rr(); AG.toast(`${k} dikembalikan ke agent untuk revisi`);
});
AG.dialog("decision", (k) => ({ w: "w640", icon: "alert-triangle", tone: "dng", title: `Butuh keputusan · ${k}`, desc: "Testing gagal 3 kali. Pilih satu tindakan.",
  body: `<div class="td-dec col g16">${aiCause()}${decOptions("dec-dlg")}${fld("Petunjuk atau catatan", inp("", { area: true, ph: "Wajib untuk pilihan 1 dan 4", a: { "data-dec-note": "" } }))}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Kirim keputusan", "pri", "send", { "data-act": "dec-submit", "data-arg": k })}` }));
AG.action("dec-submit", (el, k) => {
  const scope = el.closest(".dialog") || el.closest(".td-dec"); const v = scope.querySelector("[data-radio].on")?.dataset.v || "hint"; const note = scope.querySelector("[data-dec-note]");
  if ((v === "hint" || v === "accept") && !note.value.trim()) { note.classList.add("err"); note.focus(); return AG.toast("Isi petunjuk atau catatan dulu", "dng"); }
  const t = T(k); const msg = {
    hint: () => { t.stage = "dev"; t.state = "jalan"; t.agent = "Memperbaiki dengan petunjuk · putaran 1"; return `${k} kembali ke Development dengan petunjuk · hitungan gagal 0`; },
    self: () => { t.stage = "dev"; t.state = "manusia"; t.agent = "Diperbaiki Olivia"; return `${k} di Development, Anda yang memperbaiki`; },
    req: () => { t.stage = "prd"; t.state = "antre"; t.agent = "Revisi requirement"; return `${k} kembali ke PRD`; },
    accept: () => { t.stage = "done"; t.state = "siap"; const n = `KSR-${nextNum()}`; AG.TICKETS.push({ k: n, type: "bug", stage: "backlog", state: "siap", t: `Lanjutan ${k}: ${note.value.trim().slice(0, 60)}`, p: 2, who: "QQ", lbl: ["Struk"], pts: 2 }); return `${k} diterima dengan catatan · ${n} Bug dibuat`; },
    cancel: () => { t.stage = "done"; t.state = "siap"; t.agent = undefined; return `${k} dibatalkan · artifact tetap tersimpan`; },
  }[v]();
  log(k, "alert-triangle", `<b>${P(AG.ME).name}</b> memutuskan: ${DEC_OPTS.find((o) => o[0] === v)[1]}`);
  AG.closeDialog(); rr(); AG.toast(msg);
});
const transBody = (k, to, force) => { const t = T(k); return `<div class="row">${tref(k)}<span class="trunc">${t.t}</span></div>
  <div class="grid2">${fld("Dari", `<span class="stg now" style="width:max-content">${ic(AG.stage(t.stage).i, "s14")}${AG.stage(t.stage).n}</span>`)}${fld("Ke stage", sel(flowOf(t).filter((s) => s.k !== t.stage).map((s) => [s.k, s.n]), to || flowOf(t)[Math.min(flowOf(t).length - 1, flowOf(t).findIndex((s) => s.k === t.stage) + (force ? 1 : 2))].k))}</div>
  ${force ? `<div><div class="td-lab">Gate yang dilewati</div>${gates(t).map((g) => `<div class="td-gate"><span class="gi ${g.ok ? "ok" : "no"}">${ic(g.ok ? "check" : "x")}</span><span>${g.n}<small>${g.d}</small></span></div>`).join("") || `<span class="mut sm">Stage ini tanpa gate.</span>`}</div>` : ""}
  ${fld("Alasan", inp("", { area: true, ph: force ? "Contoh: hotfix produksi, PR sudah di-merge manual." : "Contoh: perubahan teks saja, tidak perlu desain.", a: { "data-reason": "" } }), "Wajib. Tercatat di audit log bersama nama Anda.")}
  ${force ? `<label class="row sm">${AG.cb(false, "Saya paham")}<span>Saya paham gate dilewati dan evidence tidak lengkap</span></label>` : ""}
  <div class="alert gray">${ic("shield-lock")}<span>Hanya Admin dan Owner. Anda masuk sebagai <b>Owner</b>.</span></div>`; };
AG.dialog("skip-stage", (arg) => { const [k, to] = arg.split(">"); return { w: "w560", icon: "player-skip-forward", tone: "warn", title: `Lompat stage · ${k}`, desc: "Melewati satu atau lebih stage. Gate stage yang dilewati tidak dicek.",
  body: transBody(k, to, false), foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Lompat stage", "pri", "player-skip-forward", { "data-act": "trans-go", "data-arg": `${k}|skip` })}` }; });
AG.dialog("force-transition", (k) => ({ w: "w560", icon: "bolt", tone: "dng", title: `Paksa transisi · ${k}`, desc: "Memindahkan tiket walaupun gate belum terpenuhi.",
  body: transBody(k, "", true), foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Paksa transisi", "dngs", "bolt", { "data-act": "trans-go", "data-arg": `${k}|force` })}` }));
AG.action("trans-go", (el, a) => {
  const [k, kind] = a.split("|"); const d = el.closest(".dialog"); const r = d.querySelector("[data-reason]"); const to = d.querySelector("select").value;
  if (!r.value.trim()) { r.classList.add("err"); r.focus(); return AG.toast("Alasan wajib diisi", "dng"); }
  if (kind === "force" && !d.querySelector(".cb.on")) return AG.toast("Centang konfirmasi dulu", "dng");
  const t = T(k); applyMove(t, to); log(k, kind === "skip" ? "player-skip-forward" : "bolt", `<b>${P(AG.ME).name}</b> ${kind === "skip" ? "melompati stage" : "memaksa transisi"} ke ${AG.stage(to).n}: “${esc(r.value.trim())}”`);
  AG.closeDialog(); rr(); AG.toast(`${k} ${kind === "skip" ? "lompat" : "dipaksa pindah"} ke ${AG.stage(to).n} · tercatat di audit log`);
});
const YAML_SAMPLE = `workflow: mobile-release
versi: 1
stages:
  - id: spec
    nama: Spec
    persona: pm
    gate: ["Artifact ada"]
  - id: build
    nama: Build
    persona: mobile-dev
    gate: ["PR terbuka", "Test lulus"]
  - id: store
    nama: Rilis ke store
    transisi: tunggu_approve`;
AG.dialog("yaml-import", () => ({ w: "w640", icon: "file-import", tone: "acc", title: "Impor workflow dari YAML", desc: "Tempel isi YAML atau pilih berkas. Divalidasi sebelum disimpan.",
  body: `<button type="button" class="td-drop" data-toast="Pilih berkas .yaml">${ic("upload", "s14")} Seret berkas .yaml ke sini atau pilih berkas</button>
    ${fld("Isi YAML", `<textarea class="inp mono" style="min-height:200px;font-size:12px" data-yaml>${esc(YAML_SAMPLE)}</textarea>`)}<div data-yaml-res></div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Validasi", "", "check", { "data-act": "yaml-check" })}${btn("Impor", "pri", "", { "data-act": "yaml-go" })}` }));
AG.action("yaml-check", (el) => {
  const d = el.closest(".dialog"); const v = d.querySelector("[data-yaml]").value; const n = (v.match(/- id:/g) || []).length;
  d.querySelector("[data-yaml-res]").innerHTML = !/^workflow:/m.test(v) || !n ? `<div class="alert dng">${ic("alert-circle")}<span>Kunci <code>workflow</code> atau daftar <code>stages</code> tidak ditemukan.</span></div>`
    : `<div class="col"><div class="alert ok">${ic("circle-check")}<span>${n} stage dan ${(v.match(/gate:/g) || []).length} aturan gate valid.</span></div>${/mobile-dev/.test(v) ? `<div class="alert warn">${ic("alert-triangle")}<div class="grow col g8"><span>Persona <code>mobile-dev</code> tidak dikenal. Petakan ke:</span>${AG.combo({ options: Object.entries(AG.PERSONAS).map(([k, p]) => ({ v: k, l: p.n, sub: p.t, lead: av(k, 20) })), value: "dev", w: 280 })}</div></div>` : ""}</div>`;
});
AG.action("yaml-go", (el) => { el.closest(".dialog").querySelector('[data-act="yaml-check"]').click(); if (el.closest(".dialog").querySelector(".alert.dng")) return; AG.closeDialog(); AG.toast("Workflow “mobile-release” diimpor sebagai draf"); });
})();
