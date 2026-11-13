# M8 Knowledge base Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal (spec "Selesai bila"):** scan project contoh menghasilkan dokumen knowledge, dan update setelah
tiket Done muncul sebagai usulan yang harus disetujui pemilik knowledge. Plus utang: knowledge persona
(M4), backup aturan lama ke server (M5), dan render Markdown aman untuk deskripsi tiket, komentar,
artifact, dan dokumen knowledge (M2–M3).

**Spec:** `docs/spec.md` §"Knowledge base", §"Persona" (dokumen yang boleh dibaca), §"MCP" (`/ag-search`,
`/ag-knowledge-sync`, `agentgate init` menyerap aturan lama ke "Konvensi Project", backup di server),
§"Runner" (job tanpa tiket di runner knowledge owner), baris M8. Prototype: varian **B · Beranda + kartu
kelompok** di `screens-m7-m8-m10.js` (Beranda, Tinjau usulan mode fokus berdampingan, Dokumen per
kelompok, Editor dokumen, Knowledge workspace, Dokumen workspace, Jenis dokumen custom) dan panel
Knowledge di Edit persona (`screens-m4-m6.js`).

## Keputusan desain

- **AI tetap milik user.** Server tidak menulis isi dokumen. Scan/update = **job knowledge** (tanpa
  tiket) di runner: `jobs.ticket_id` boleh NULL untuk `kind = 'knowledge'`. Prompt job berisi tugas
  (scan kedalaman X / perbarui dokumen Y / audit), katalog relevan, dan instruksi jenis dokumen; agent
  menulis lewat MCP `ag_knowledge` `propose`, jadi **semua** hasil AI masuk sebagai usulan.
- **Executor job knowledge** = pemilik knowledge project (notifikasi + auto-ambil runner-nya). Siapa pun
  dengan `EditProject` boleh mengambil (model klaim bila runner pemilik offline).
- **Server = mirror yang bisa diedit; repo disinkronkan lewat CLI.** Approve/simpan membuat versi di
  server. `agentgate knowledge sync` menulis `.agentgate/knowledge/<slug>.md` (front matter: judul,
  kelompok, jenis, versi), mengunggah perubahan lokal (versi dasar sama → versi baru; berbeda → usulan
  `repo`), lalu commit di branch `agentgate/knowledge-sync` + PR lewat jalur PR M6. Commit otomatis dari
  server lewat API provider = utang.
- **Status dokumen** = `basi` (kolom `stale_reason`) / `ada usulan` (usulan pending) / `terbaru`.
- **Pemicu update:** tiket masuk kategori Done → dokumen kandidat (sumber dokumen ∩ file repo tiket yang
  berubah sejak versi dokumen, atau menyebut kode tiket) → run `update`; graph di-rebuild setelah merge →
  dokumen yang sumbernya berubah ditandai basi + run `update`; audit mingguan (task terjadwal) → tandai
  basi + run `audit`; manual (tombol / `/ag-knowledge-sync`). Semua bisa dimatikan per workspace.
- **Sumber dokumen** (`knowledge_sources`: repo + path/prefix) diisi agent saat `propose` (`sources`),
  dipakai untuk deteksi basi dan panel "Terhubung ke graph".
- **Estimasi scan** dari ukuran graph (file, simbol) × kedalaman → token + jumlah dokumen relevan + menit.
  Rupiah tidak ditampilkan (harga tergantung langganan user).
- **Katalog bawaan** (10 kelompok spec) ada di kode; relevansi kelompok dari bahasa/path graph
  (Frontend bila ada TSX/JSX/Vue/Svelte/CSS, Mobile bila Dart/Kotlin/Swift, Data bila SQL/notebook/dbt).
  Jenis custom per workspace (template + instruksi AI + project + kelompok).
- **Knowledge workspace**: dokumen dengan `project_id NULL`, berlaku untuk project terpilih (kosong =
  semua), dibaca persona terpilih (kosong = semua). Kelola = izin baru `manage_knowledge` (Owner/Admin).
- **Persona**: `knowledge_types text[]` (kosong = semua jenis). `ag_context`/prompt job hanya menyebut
  dokumen yang boleh dibaca persona stage; `ag_knowledge read` menolak dokumen di luar daftar bila
  dipanggil dengan `persona`.
- **Approve** oleh pemilik knowledge project (fallback `AdministerProject` bila belum ada pemilik).
  Edit langsung oleh manusia: `EditProject`, dengan cek versi dasar (konflik → 409 + pesan).
- **Markdown aman:** `common::markdown::render` (pulldown-cmark GFM: tabel, checklist, coret, footnote)
  → sanitasi `ammonia` (tanpa `style`, `on*`, `javascript:`; tautan eksternal `rel=noopener
  nofollow`), blok ```mermaid → `<pre data-mermaid>` yang dirender Mermaid 12.3.0 (di-vendor, lazy,
  `securityLevel: strict`) oleh `static/js/markdown.js`. Fungsi Tera `markdown(text=...)`.
- **Backup aturan:** `agentgate init` mengunggah arsip + manifest + isi teks aturan lama ke
  `POST /api/cli/backups` (storage `backups/…`), server menyimpan `rule_backups` dan membuat usulan
  `init` untuk dokumen "Konvensi & Coding Style" berisi aturan lama. Daftar + unduh di Runner & CLI →
  Aturan & drift.

## Architecture

- **Migrasi M8**: `knowledge_types`, `knowledge_docs` (+ GIN FTS), `knowledge_versions`,
  `knowledge_proposals`, `knowledge_sources`, `knowledge_runs`, `knowledge_settings`, `rule_backups`;
  `projects.knowledge_owner_id`; `personas.knowledge_types`; `jobs.ticket_id` NULL + kind `knowledge`
  + `knowledge_run_id`; izin `manage_knowledge` ke Owner/Admin bawaan.
- **`services::knowledge`**: `catalog`, `docs` (buat/ubah/arsip/pulihkan versi/basi), `proposals`
  (propose/approve/reject/undo), `runs` (estimasi, buat job, prompt, selesai), `triggers` (Done,
  rebuild graph, audit), `search`, `sync` (ekspor/impor CLI), `backups`, `context` (MCP/prompt).
- **Runner**: `JobKind::Knowledge` di core; spec job tanpa tiket (`ticket = ""`, branch kosong, hasil
  kode manual); selesai sukses tidak memanggil `stages::complete`, menutup run.
- **MCP**: tool `ag_knowledge` (`list`, `read`, `propose`) → 13 tool; `ag_search` menyertakan dokumen
  knowledge; `ag_context` bagian `knowledge`; prompt `ag-knowledge-sync` meminta run manual.
- **CLI**: `agentgate knowledge sync|pull|status`; `init` mengunggah backup.
- **UI**: tab project `knowledge` aktif. Halaman: Beranda (`/knowledge`, + `?q=` hasil cari), Kelompok
  (`/knowledge/g/{group}`), Tinjau usulan (`/knowledge/proposals?p=`), Editor (`/knowledge/{slug}`),
  Riwayat/diff versi (`/knowledge/{slug}/versions[/{n}]`), dialog Scan, Dokumen baru, Tolak. Pengaturan
  → Knowledge workspace (`/settings/knowledge/{docs|types|auto}`, dokumen + jenis = halaman sendiri).
  Persona → panel Knowledge. Komponen baru: `md_view` (Markdown tersanitasi + Mermaid), `diff_sbs`
  (diff berdampingan), `donut` (kesehatan) → `/_dev/ui` + design-system.md.

## Global Constraints

- Aturan M1–M7 tetap (UUID v7, `timestamptz`, enum text + CHECK + tes, 404 lintas workspace, PRG,
  Fluent "Anda", audit di transaksi yang sama, tanpa inline script, CSP `script-src 'self'`).
- Usulan AI tidak pernah menimpa dokumen tanpa approve manusia.
- Isi Markdown dari user/AI selalu disanitasi sebelum tampil (tes XSS).
- Tes tidak memanggil jaringan.

## Tasks

- [ ] **1. Plan** (dokumen ini) — commit `docs: plan M8`.
- [ ] **2. Markdown aman**: `common::markdown` + fungsi Tera + `md_view` + Mermaid vendor + `markdown.js`;
  pakai di deskripsi, komentar, artifact, ringkasan job; tes XSS.
- [ ] **3. Skema**: migrasi, entity, model, enum + tes.
- [ ] **4. Job tanpa tiket**: `ticket_id` opsional di job/runner/UI runner; `JobKind::Knowledge`; tes.
- [ ] **5. services::knowledge** inti: katalog, dokumen, versi, usulan, cari; tes.
- [ ] **6. Run + pemicu**: estimasi, job knowledge + prompt, Done/merge/audit, selesai job; tes.
- [ ] **7. MCP**: `ag_knowledge`, `ag_search` + knowledge, `ag_context`, prompt sync; persona knowledge; tes.
- [ ] **8. UI project**: beranda, kelompok, tinjau usulan (+ `kb-review.js`), editor, versi, dialog; tes.
- [ ] **9. UI workspace + persona**: Knowledge workspace (3 panel), dokumen workspace, jenis custom,
  panel Knowledge persona; tes.
- [ ] **10. CLI + API**: ekspor/impor/PR knowledge, `agentgate knowledge`, unggah backup `init`,
  daftar backup di Runner & CLI; tes.
- [ ] **11. Seed dev** (`seed_knowledge`), komponen di `/_dev/ui`, design-system.md.
- [ ] **12. Visual check** Playwright (desktop, 390px, gelap) vs prototype.
- [ ] **13. bin/ci**, merge `--no-ff` ke `staging`, `bin/ci`, PROGRESS.md.
