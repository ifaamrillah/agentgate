# M3 Workflow & Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal (spec "Selesai bila"):** tiket melewati workflow dengan approve/reject manual, dan transisi tanpa
evidence ditolak server. Cakupan: workflow builder, workflow per tipe tiket, YAML impor/ekspor,
penugasan per stage, approve/reject, gate engine, artifact + versi + editor Markdown, Butuh
Keputusan, lompat stage / paksa transisi, migrasi workflow, 9 template bawaan.

**Architecture.**

- **Definisi workflow** (`agentgate-core::workflow`): struct `Definition { stages: Vec<Stage> }`
  bertipe (kategori, persona, mode, gate, transisi keluar, batas gagal, `fail_to`, paralel,
  penugasan default, artifact input/output), validasi murni (kode error Fluent), dokumen YAML
  (`serde_yaml_ng`, sudah ada di dependency tree lewat Loco) dengan kunci berbahasa Indonesia
  mengikuti prototype, dan 9 template bawaan + "Subtask ringkas" sebagai berkas YAML
  (`include_str!`). Crate `core` dipakai CLI nanti (spec: "parser workflow").
- **Penyimpanan** (migrasi `m20261114_000001_create_workflows`): `workflows` (nama, kunci,
  `version` = versi aktif, `draft jsonb NULL` = draf builder), `workflow_versions` (snapshot
  definisi per versi; riwayat dan gate versi lama), `ticket_types.workflow_id` (default
  workspace per tipe), `project_workflows` (override per project per tipe). Tiket mendapat
  `workflow_id`, `stage_key`, `stage_version` (versi yang gate-nya berlaku untuk stage sekarang:
  "gate versi baru berlaku sejak tiket masuk stage berikutnya"), `stage_state`
  (`active`/`review`/`decision`), `fail_count` + `fail_stage_key`, `stage_entered_at`,
  `cancelled_at`. `tickets.status` tetap = kategori stage (rencana M2), sehingga blocks, filter,
  pencarian, dan Tiket saya tidak berubah. Penugasan per stage: `ticket_stage_people`
  (executor/approver/watcher per stage tiket, disalin dari default stage). Artifact:
  `ticket_artifacts` + `artifact_versions` (v1, v2, …; status `draft`/`pending`/`approved`/
  `rejected`).
- **Gate engine** (`services::workflow::engine`): semua perubahan stage lewat satu jalur
  (`enter_stage`) yang menulis riwayat, menyalin penugasan default, menambah watcher, dan
  memperbarui `status`. Aksi: pindah (board/tombol), minta review, approve, reject, keputusan
  (5 pilihan), lompat stage (Admin, alasan, audit), paksa transisi (Admin, alasan, audit).
  Gate dievaluasi dari data server (artifact, subtask, deskripsi). Gate evidence (screenshot,
  test, PR, CI) belum punya sumber sampai runner (M5) dan git (M6), jadi selalu belum terpenuhi:
  transisi tanpa evidence ditolak.
- **UI**: Pengaturan → Workflow (daftar, template, impor YAML), builder (alur stage kiri + form
  stage kanan, tab YAML, simpan = versi baru), migrasi (pemetaan stage yang dihapus), Pengaturan
  project → Workflow (per tipe). Board berkolom stage, detail tiket dengan stepper stage, panel
  stage (gate, penugasan, aksi), panel Butuh keputusan, tab Artifact (navigator + versi +
  bandingkan), halaman editor artifact (md_editor). Kotak masuk tab "Butuh keputusan".

**Spec:** `docs/spec.md` §"Workflow, stage & gate", baris M3 §"Urutan build MVP". Prototype:
`docs/reference/ui-prototype/screens-m2-m3.js` (Detail tiket menunggu review / butuh keputusan,
navigator artifact, editor artifact, Daftar workflow, Workflow builder, Migrasi workflow, Workflow
project).

## Global Constraints

- Aturan M1/M2 tetap: UUID v7, `timestamptz`, enum = text + CHECK + tes `EnumIter`, `lock_timeout`,
  404 lintas workspace/tanpa izin, PRG, 422 dengan form terisi, teks Fluent, audit di transaksi
  yang sama, izin dibaca ulang di transaksi (`Working::begin`).
- Izin: workflow workspace = `ManageTicketSettings` (Owner/Admin); workflow per project =
  `AdministerProject`; pindah/minta review/artifact = `EditProject`; approve/reject/keputusan =
  approver stage sekarang (bila stage tanpa approver: siapa pun dengan `EditProject`); lompat
  stage dan paksa transisi = `AdministerProject` + alasan wajib + audit log.
- "Lompat" = maju melewati stage yang menegakkan sesuatu (gate atau transisi tunggu approve).
  Melewati stage manual tanpa gate (Kanban) bebas, supaya board Kanban tetap seperti M2.
- Mundur ke stage sebelumnya boleh untuk `EditProject` (gate tidak berlaku ke belakang).

## Review Focus

1. Transisi maju tanpa gate terpenuhi ditolak di semua jalur (board, tombol, approve, buat tiket
   langsung di stage tengah) → Task 4.
2. Approve/reject hanya oleh approver; lompat/paksa hanya Admin dengan alasan + audit → Task 4.
3. Putaran gagal mencapai batas → Butuh keputusan; 5 pilihan berjalan sesuai spec → Task 5.
4. Migrasi: tiket di stage yang dihapus wajib dipetakan; gate versi lama tetap berlaku sampai
   tiket pindah stage → Task 3.
5. Artifact/versi milik tiket lain atau workspace lain → 404 → Task 6.

## File Structure

```
core/src/workflow/{mod,definition,yaml,templates}.rs, core/templates/*.yaml
server/migration/src/m20261114_000001_create_workflows.rs
server/src/models/{workflows,workflow_versions,project_workflows,ticket_stage_people,
                   ticket_artifacts,artifact_versions}.rs (+ _entities)
server/src/services/workflow/{mod,manage,migrate,engine,artifacts,gates}.rs
server/src/controllers/workflows/{mod,settings,project}.rs, controllers/tickets/{stage,artifacts}.rs
server/src/views/workflows.rs, views/tickets.rs (stage, stepper)
server/assets/views/workflows/*, tickets/{artifacts,artifact_edit,_stage_panel,_decision}.html,
  components/{stage_stepper,gate_list}.html, css/components/workflow.css
server/tests/requests/workflows/*.rs, tests/requests/tickets/{stages,artifacts}.rs
```

### Task 1: Definisi workflow di `core`

- [ ] Unit test: template valid semua; kunci stage unik; stage terakhir `done`; kategori tidak
  mundur; `fail_to` harus stage sebelumnya; YAML round-trip; YAML tak valid → error berkode.
- [ ] Commit `feat(core): workflow definition, YAML and built-in templates`.

### Task 2: Migrasi, entity, model

- [ ] Tes enum CHECK baru (`tickets.stage_state`, `artifact_versions.status`,
  `ticket_stage_people.role`, kind event/notifikasi/audit baru); workspace baru punya workflow
  Kanban + Subtask ringkas; tiket lama ter-backfill ke Kanban.
- [ ] Commit `feat(server): workflow schema and ticket stages`.

### Task 3: Kelola workflow (service + halaman)

- [ ] Tes: buat dari template, clone, ubah stage di draf, simpan → versi baru; hapus stage berisi
  tiket → wajib migrasi; migrasi memindah tiket + audit; impor/ekspor YAML; workflow per tipe per
  project; anggota biasa 404.
- [ ] Commit per bagian (`workflow settings`, `builder`, `migration`, `project workflow`).

### Task 4: Gate engine dan transisi

- [ ] Tes: board pindah maju ditolak bila gate belum terpenuhi (422 + daftar gate); stage tunggu
  approve tidak bisa digeser maju; minta review → approver dapat notifikasi; approve maju + stage
  berikutnya; reject wajib catatan; non-approver 404/422; lompat stage Admin + alasan + audit;
  paksa transisi; gate evidence selalu menolak; blocks tetap ditegakkan.
- [ ] Commit `feat(server): gate engine, review and admin transitions`.

### Task 5: Butuh keputusan

- [ ] Tes: reject ke-N (batas) → `decision`; 5 pilihan (petunjuk reset hitungan, perbaiki sendiri,
  ubah requirement, terima dengan catatan → Bug baru, batalkan); kotak masuk tab Butuh keputusan.
- [ ] Commit `feat(server): needs-decision flow and inbox tab`.

### Task 6: Artifact

- [ ] Tes: buat artifact dari template output, simpan versi baru, bandingkan versi, approve stage
  menyetujui versi pending, artifact tiket lain 404, viewer tidak bisa menyimpan.
- [ ] Commit `feat(server): ticket artifacts with versions and editor`.

### Task 7: Board + detail tiket terhubung ke stage

- [ ] Board berkolom stage (gabungan per kunci stage dari workflow yang dipakai project), kartu
  menampilkan keadaan stage, drag-drop mengirim `stage`; detail: stepper, panel stage, alert review.
- [ ] Commit `feat(server): stage board and ticket stage panel`.

### Task 8: Penutup

- [ ] Seed dev (workflow Superpowers · Kasir Online, tiket di berbagai stage/keadaan, artifact),
  katalog `/_dev/ui` (komponen baru), screenshot Playwright desktop/390/gelap vs prototype,
  `bin/ci` hijau, PROGRESS, merge `--no-ff` ke `staging`, `bin/ci` di staging.
