# M7 Code graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal (spec "Selesai bila"):** code graph project dibangun tanpa token AI dari tree-sitter (server +
CLI), diperkaya SCIP dari laptop user, diperbarui inkremental per file, dicocokkan lintas repo, dan
dipakai AI lewat MCP (`ag_search`, `ag_trace`, `ag_snippet`, + `ag_impact`, `ag_graph`). Benchmark
recall/presisi call graph terhadap ground truth dijalankan di CI dan dibandingkan dengan
codebase-memory pada repo yang sama.

**Spec:** `docs/spec.md` §"Code graph", §"Penghemat token" (code graph wajib), baris M7. Viewer visual
(zoom project → fungsi, sorot tiket) = M10. Prototype: `screens-m7-m8-m10.js` "Status index code
graph" (Repo & index, Lintas repo, Riwayat git, Kualitas kode, Benchmark, dialog Index ulang).

## Cakupan M7 kali ini

- Bahasa prioritas spec: TS/JS (+TSX), Python, Go, Rust, Java, Kotlin, PHP, C#, Dart — semua lewat
  crate grammar tree-sitter resmi (`tree-sitter-language`, tree-sitter 0.25+).
- Embedding `fastembed-rs` **tidak** di M7 (ONNX runtime + unduh model ratusan MB di build/CI).
  Pencarian = leksikal sadar-identifier (camelCase/snake_case dipecah, doc comment ikut diindeks,
  Postgres `tsvector` `simple`). Dicatat sebagai keputusan sementara + utang.
- Lintas repo: HTTP call ↔ route, event/queue (publish ↔ subscribe literal), shared package
  (nama package.json/go.mod/Cargo.toml/composer.json/pubspec). gRPC = utang.
- Query ala Cypher: subset satu pola (`MATCH (a:Kind {..})-[:CALLS*1..3]->(b) WHERE … RETURN … LIMIT`).
- "Minta SCIP dari runner" belum dikirim lewat runner: UI menampilkan perintah `agentgate graph scip`.
- Dokumen non-kode sebagai node + keterlacakan ke tiket/knowledge = M8/M10 (UI "segera").

## Architecture

- **Crate `graph`** (murni, tanpa DB):
  - `lang`: deteksi bahasa dari ekstensi → grammar tree-sitter.
  - `extract`: `FileFacts` per file (simbol + rentang baris + exported + doc + signature, call
    reference + receiver + simbol pembungkus, import, inheritance, route, HTTP call, event
    publish/subscribe, TODO, kompleksitas kasar). Query tags bawaan grammar + query sendiri.
  - `build`: `RepoGraph` dari semua `FileFacts` (+ edge SCIP opsional): node file + simbol, edge
    `calls`/`inherits`/`imports` dengan keyakinan **pasti** (file yang sama, `self/this`, nama yang
    diimpor, SCIP) atau **dugaan** (nama unik di repo / kandidat satu direktori). Modul per direktori
    + node pusat (derajat tertinggi), dead code (tanpa pemanggil, bukan entry/test/route/exported →
    pasti; exported → dugaan), `RepoSurface` untuk lintas repo.
  - `analysis`: trace (BFS pemanggil/dipanggil, kedalaman ≤ 3), dampak (dari simbol atau unified
    diff → simbol berubah → pemanggil transitif + file tes), lintas repo (`link_surfaces`).
  - `query`: parser + eksekutor subset Cypher di atas graph di memori.
  - `walk`: jalan repo (`ignore`: .gitignore), alasan dilewati (rahasia, generate, > 1 MB, biner,
    gagal parsing), sha256 isi.
  - `history`: parse `git log --numstat` → hotspot, coupling, pemilik per modul (90 hari).
  - `scip` (feature `scip`, dipakai CLI): baca `index.scip` → edge pemanggil→definisi per baris.
  - `bench`: korpus fixture per bahasa + ground truth edge → recall/presisi; tes CI wajib lulus
    (recall ≥ 95%, presisi ≥ 90%).
- **Server**:
  - Migrasi M7: `graph_repos` (status, sumber, commit, statistik, bahasa, file dilewati, statistik
    git, permukaan lintas repo, edge SCIP, error), `graph_files` (path, bahasa, sha, isi, facts
    jsonb), `graph_symbols` (nama, kind, file, baris, exported, doc, modul, pusat, dead, derajat,
    `search tsvector`), `graph_edges` (from/to simbol, kind, keyakinan, sumber, baris), `graph_links`
    (lintas repo per project).
  - `services::graph`: `ingest` (manifest → file yang perlu dikirim; upload batch → parse di
    `spawn_blocking`; SCIP; riwayat git), `rebuild` (facts tersimpan → `RepoGraph` → tulis ulang
    simbol/edge dalam satu transaksi → statistik → link lintas repo project), `query` (search,
    trace, snippet, impact, dead code, overview, cypher), `clone` (index dari server: `git clone`
    dangkal 90 hari dengan token koneksi M6 → walk → ingest penuh). Worker `GraphIndexWorker`
    (tombol Index ulang + webhook PR merged ke branch default).
  - API CLI (bearer): `POST /api/cli/graph/manifest`, `/upload`, `/finish`, `/scip`.
  - MCP: `ag_search`, `ag_trace`, `ag_snippet`, `ag_impact`, `ag_graph`; `ag_context` tiket +
    prompt job memuat "Konteks code graph" (repo terindeks + titik awal dari judul tiket).
- **CLI**: `agentgate graph index [--full]` (walk, manifest, upload, riwayat git, finish),
  `agentgate graph scip` (jalankan indexer SCIP yang terpasang: scip-typescript, scip-python,
  scip-go, rust-analyzer, scip-java; unggah edge), `agentgate graph status`.
- **UI** (tab project `graph`, ikon `topology-star-3`): `/w/{slug}/p/{KEY}/graph` = **Cari simbol**
  (datatable hasil), `/graph/symbol/{id}` (detail: properti, pemanggil, dipanggil, potongan kode,
  dampak), tab Viewer "segera" (M10), `/graph/index/{repo|cross|git|quality|bench}` = subpage E
  sesuai prototype; dialog Index ulang (cakupan repo, inkremental/penuh). Komponen baru:
  `confidence` (badge pasti/dugaan/tidak cocok), `lang_bar` (bar komposisi bahasa), `code_view`
  (potongan kode bernomor baris) → `/_dev/ui` + design-system.md.

## Global Constraints

- Aturan M1–M6 tetap (UUID v7, `timestamptz`, enum text + CHECK + tes `EnumIter`, 404 lintas
  workspace, PRG, Fluent "Anda", audit di transaksi yang sama).
- File rahasia tidak pernah dikirim CLI (dicek sebelum upload) dan ditolak server bila tetap
  dikirim. Isi file > 1 MB tidak disimpan.
- Membaca graph butuh `ViewProject`; mengunggah index/Index ulang butuh `EditProject`.
- Tes tidak memanggil jaringan; clone diuji dengan repo git lokal.

## Review Focus

1. Resolusi call graph & keyakinan (benchmark) → Task 3–4.
2. Tenancy: query graph selalu difilter workspace + project yang bisa dibuka → Task 6–8.
3. Rahasia tidak masuk graph → Task 5, 7.
4. Inkremental: file tidak berubah tidak di-parse ulang; file dihapus hilang → Task 6.

## Tasks

- [ ] **1. Plan** (dokumen ini) — commit `docs: plan M7`.
- [ ] **2. Crate graph: bahasa + extract** (tree-sitter 10 grammar, `FileFacts`, tes per bahasa).
- [ ] **3. build + analysis** (resolusi, keyakinan, modul, dead code, trace, dampak dari diff).
- [ ] **4. bench + lintas repo + query Cypher + walk + history + scip** (tes unit; bench lulus).
- [ ] **5. Skema server**: migrasi, entity, model + tes enum.
- [ ] **6. services::graph**: ingest, rebuild inkremental, link lintas repo, query; tes.
- [ ] **7. API CLI + CLI `graph index|scip|status`**; tes server + tes CLI dengan server palsu.
- [ ] **8. MCP tool + konteks job/ag_context**; tes.
- [ ] **9. Clone server + worker + webhook merge → reindex**; tes dengan repo lokal.
- [ ] **10. UI**: Cari simbol, detail simbol, Status index (5 panel), dialog Index ulang; komponen
  di `/_dev/ui` + design-system.md; seed dev (`seed_graph`).
- [ ] **11. Benchmark vs codebase-memory** pada repo ini → `docs/benchmarks/2026-11-13-code-graph.md`.
- [ ] **12. Visual check** Playwright (desktop, 390px, gelap) vs prototype.
- [ ] **13. bin/ci**, merge `--no-ff` ke `staging`, `bin/ci`, PROGRESS.md.
