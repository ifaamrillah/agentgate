# M9 Planning & laporan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal (spec "Selesai bila"):** satu sprint bisa direncanakan (backlog → sprint, kapasitas, estimasi),
dijalankan (board Scrum, WIP, swimlane), dan dilaporkan (burndown, burnup, velocity, sprint report,
cumulative flow, kalibrasi estimasi, cycle time). Plus: roadmap, kalender, rilis + release notes AI,
components, query ala JQL + saved filter + bulk edit. Utang: estimasi AI di header tiket (M2/M3) dan
panel estimasi di draft tiket (M4); render Markdown aman untuk artifact (cek + tes XSS).

**Spec:** `docs/spec.md` §"Tiket, board & manajemen proyek" (Tampilan & perencanaan, Estimasi), baris
M9. Prototype: `screens-m9-m11-m12.js` (Backlog & sprint, Roadmap, Kalender, Rilis, Detail rilis,
Editor release notes, Laporan 7 tab, Pencarian lanjutan, Components) dan board M2 (`screens-m2-m3.js`:
WIP limit, swimlane).

## Keputusan desain

- **AI tetap milik user.** Server tidak menjalankan LLM. Estimasi AI masuk lewat MCP: tool baru
  `ag_planning` (`estimate`: story point + jam AI/manusia per stage + dasar; `release_notes`: draf
  release notes; `sprint`: baca sprint aktif + backlog) dan field `estimate` di `ag_draft_tickets`.
  "Minta estimasi AI" = komentar `@agent-analyst` berinstruksi (jalur job komentar M5). "Susun dengan
  AI" release notes = job tanpa tiket persona `writer` (jalur job tanpa tiket M8, `stage_key =
  release_notes`).
- **Estimasi** (`ticket_estimates`): satu baris per usulan/ubahan (sumber `ai`/`human`), yang terakhir
  berlaku. Story point tiket tetap `tickets.story_points` (angka yang disepakati); usulan AI
  ditampilkan di samping (diterima / diubah oleh X).
- **Kalibrasi** per project per stage: median `aktual / estimasi` dari tiket Done yang punya estimasi
  (aktual dari riwayat `stage_changed`; waktu AI = durasi job di stage itu, sisanya manusia). Faktor
  dipakai untuk perkiraan selesai (roadmap, kalender, detail rilis). Tanpa data: faktor 1.
- **Sprint** (`sprints`): planned → active → completed; boleh lebih dari satu aktif (prototype:
  "berjalan paralel"). Selesaikan sprint menyimpan laporan beku (`report` jsonb: komitmen, selesai,
  ditambah tengah sprint, tiket selesai/dipindah) lalu memindah tiket belum selesai ke sprint lain,
  sprint baru, atau backlog. Kapasitas = isian manual atau rata-rata velocity 3 sprint terakhir.
- **Log scope sprint** (`sprint_scope_events`: added/removed/done/reopened/points) ditulis di tiga
  titik (layanan planning, `enter_stage`, ubah tiket) → burndown/burnup direplay dari log, tanpa
  snapshot terjadwal.
- **Board Scrum** = board yang sama dengan pilihan sprint (`?sprint=active|<id>`), WIP limit per stage
  (field `wip` di definisi stage, builder workflow; melebihi = peringatan, tidak diblokir), swimlane
  (`?swim=epic|assignee`), filter cepat (Saya, Belum ada penerima, Sprint aktif, Belum diestimasi).
- **Backlog**: urutan `tickets.backlog_rank`; drag-drop antar sprint (`backlog-dnd.js`, submit form
  biasa) + menu ⋯ (pindah sprint, paling atas/bawah). Epic dan Subtask tidak tampil di backlog.
- **Roadmap** (Gantt) dan **Kalender** dirender server-side (posisi bar dihitung Rust, tanpa library
  JS). Mulai = masuk stage kategori bukan backlog (riwayat) atau mulai sprint; selesai = due date,
  Done, atau perkiraan dari estimasi terkalibrasi (8 jam kerja/hari, Senin–Jumat).
- **Rilis** (`releases` + `tickets.release_id` = Fix version): Rencana / Belum dirilis / Dirilis /
  Diarsipkan; release notes Markdown dengan riwayat versi (`release_note_versions`), editor halaman
  sendiri; dialog Rilis memindah tiket belum selesai.
- **Components** (`components` + `tickets.component_id`, satu per tiket): nama, deskripsi, pemilik,
  modul code graph (prefix path). Aturan: pemilik jadi penerima default tiket baru tanpa penerima.
- **JQL** (`services::planning::jql`): parser recursive-descent (AND/OR/NOT, kurung, `= != ~ !~ > >= <
  <= in`, `not in`, `is [not] EMPTY`, `ORDER BY`), fungsi `currentUser()`, `openSprints()`,
  `now()`. Field: project, type, stage, status, assignee, reporter, sprint, label, priority, points,
  text, component, fixVersion, due, created. Nama → id diresolusi dulu (kamus), lalu dikompilasi
  menjadi `sea_orm::Condition`; nilai user selalu bind. Saved filter: milik saya / tim / workspace.
- **Bulk edit** di hasil Pencarian lanjutan: prioritas, penerima, sprint, rilis, component, tambah
  label; setiap tiket lewat `services::tickets::update` (izin per project, riwayat, audit).
- **Grafik** = SVG server-side (`views::charts`: garis, batang, area bertumpuk, scatter) dengan
  `<title>` untuk tooltip; komponen `chart` di `/_dev/ui`.

## Architecture

- **Migrasi M9**: `sprints`, `sprint_scope_events`, `releases`, `release_note_versions`,
  `components`, `ticket_estimates`, `saved_filters`; `tickets.sprint_id/release_id/component_id/
  backlog_rank`; `ticket_drafts.estimate`; `projects.planning` (aturan component);
  `personas.mcp_tools` + `ag_planning`.
- **core**: `Stage.wip`, `MCP_TOOLS` + `ag_planning`.
- **`services::planning`**: `sprints`, `scope`, `estimates` (+ kalibrasi), `reports`, `timeline`
  (roadmap/kalender), `releases`, `components`, `jql`, `filters`, `bulk`.
- **UI**: tab project backlog/roadmap/calendar/releases/reports aktif; board Scrum; Pencarian
  lanjutan `/w/{slug}/search/advanced`; Pengaturan project → Components.

## Global Constraints

- Aturan M1–M8 tetap (UUID v7, `timestamptz`, enum text + CHECK + tes, 404 lintas workspace, PRG,
  Fluent "Anda", audit di transaksi yang sama, tanpa inline script, CSP `script-src 'self'`).
- Markdown user/AI selalu disanitasi (tes XSS). Tes tidak memanggil jaringan.

## Tasks

- [ ] **1. Plan** (dokumen ini).
- [ ] **2. Markdown**: cek deskripsi/komentar/artifact sudah `md_view`; tes XSS artifact.
- [ ] **3. Skema**: migrasi, entity, model, enum + tes; `Stage.wip`; `ag_planning` di MCP_TOOLS.
- [ ] **4. Sprint + log scope**: service + hook `enter_stage`/ubah tiket; tes.
- [ ] **5. Backlog UI** + dialog mulai/selesaikan sprint + `backlog-dnd.js`; tes.
- [ ] **6. Board Scrum**: sprint, WIP, swimlane, filter cepat; tes.
- [ ] **7. Estimasi**: MCP `ag_planning`, draft estimate, header tiket + panel ubah, kalibrasi,
  minta estimasi AI; tes.
- [ ] **8. Laporan** (7 tab) + `views::charts`; tes.
- [ ] **9. Roadmap + kalender**; tes.
- [ ] **10. Rilis + release notes** (editor, versi, job AI, dialog rilis, Fix version); tes.
- [ ] **11. Components** (pengaturan + aturan + field tiket); tes.
- [ ] **12. JQL + saved filter + bulk edit**; tes.
- [ ] **13. Seed dev** (`seed_planning`), `/_dev/ui`, design-system.md.
- [ ] **14. Visual check** Playwright (desktop, 390px, gelap) vs prototype.
- [ ] **15. bin/ci**, merge `--no-ff` ke `staging`, `bin/ci`, PROGRESS.md.
