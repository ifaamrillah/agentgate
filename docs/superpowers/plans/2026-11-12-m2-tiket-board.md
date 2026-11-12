# M2 Tiket & Board Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tiket bisa dikelola penuh tanpa AI: dibuat dengan tipe (bawaan + custom) dan field custom, diberi
kode dari template, dipindah di board Kanban, dicari, dikomentari dengan @mention (masuk kotak masuk),
diberi lampiran (S3), dihubungkan (blocks ditegakkan), dilacak riwayatnya, dan dihapus ke sampah 30 hari.

**Architecture:** Tiket milik project (`tickets.project_id`), diakses lewat extractor `TicketAccess`
(kode tiket → tiket → `ProjectAccess` project-nya; tanpa akses/terhapus/workspace lain → 404). Semua
mutasi tiket lewat `services::tickets::*`, yang membaca ulang izin di transaksi (pola `Administering`
M1), menulis riwayat `ticket_events` di transaksi yang sama, lalu notifikasi (`notifications`) juga di
transaksi yang sama (notifikasi in-app = baris DB, bukan efek samping keluar proses). Lampiran: upload
ke `ctx.storage` dulu, baris DB di transaksi, file dihapus bila transaksi gagal (pola logo M1).

**Status sekarang vs stage M3.** M2 memakai kolom `tickets.status` dengan 4 nilai tetap
(`backlog`, `todo`, `in_progress`, `done`) sebagai kolom board. Di M3, setiap stage workflow punya
`category` dari himpunan yang sama, dan tiket mendapat `stage_id` nullable; `status` tetap diisi
dengan kategori stage-nya (seperti *status category* Jira). Akibatnya: aturan *blocks* ("tidak bisa
mulai" = pindah dari `backlog`/`todo` ke `in_progress`/`done`), "Tiket saya", filter, dan pencarian
tidak berubah di M3, dan migrasi M3 hanya menambah tabel workflow + kolom `stage_id`, bukan mengubah
data tiket.

**Tech Stack:** Loco 1.3.1 (+ feature `storage_aws_s3` untuk SeaweedFS di dev), SeaORM 2.0, Postgres
full-text (`to_tsvector('simple', …)` + index GIN ekspresi), Tera 2, HTMX 4, drag-drop HTML5 native
(file `board-dnd.js`, tanpa SortableJS: drop mengisi form tersembunyi lalu submit biasa).

**Spec:** `docs/spec.md` §"Tiket, board & manajemen proyek", baris M2 di §"Urutan build MVP".
Prototype: `docs/reference/ui-prototype/screens-m2-m3.js` (Board Kanban, Daftar tiket, Detail tiket,
aktivitas & komentar, Tiket saya, Kotak masuk, Sampah, Tipe tiket & field, Kode tiket).

## Global Constraints

- Semua aturan M1 tetap: UUID v7, `timestamptz`, enum = text + CHECK + tes `EnumIter`, `lock_timeout`,
  404 untuk akses lintas workspace / tanpa izin, POST-redirect-GET, 422 dengan form terisi, teks lewat
  Fluent, audit di transaksi yang sama.
- Izin tiket: lihat = `ViewProject`; buat, ubah, pindah, komentar, lampiran, relasi, hapus ke sampah,
  pulihkan = `EditProject`; hapus permanen dari sampah = `AdministerProject` atau pembuat tiket.
  Tipe tiket, field custom, dan template kode workspace = izin baru `ManageTicketSettings` (Owner,
  Admin); override kode per project = `AdministerProject`.
- Kode tiket unik per workspace tanpa membedakan huruf besar, lewat tabel `ticket_codes` (kode sekarang
  + alias lama). Template harus memuat `{NUM}`; penghitung per tipe wajib `{TYPE}`, per tahun wajib
  `{YYYY}`/`{YY}`. Bentrok kode saat membuat tiket → nomor berikutnya (loop terbatas).
- Lampiran maksimal 10 MB, nama file disanitasi, diunduh dengan `Content-Disposition: attachment` dan
  `nosniff`; key storage `tickets/<ticket>/<uuid>`.
- Sampah: `tickets.deleted_at`; tiket di sampah 404 di semua halaman kecuali Sampah; purge setelah 30
  hari oleh job `cleanup` (langkah baru), berkas lampirannya dihapus setelah commit.

## Review Focus

1. Relasi *blocks* tidak bisa dilewati lewat jalur mana pun (board drag, form detail, bulk) → tes Task 5.
2. Kode tiket alias: kode lama me-redirect ke kode baru; kode lama tidak bisa dipakai tiket lain → Task 3.
3. @mention hanya memberi notifikasi ke orang yang punya akses project (tidak membocorkan tiket) → Task 6.
4. Lampiran tiket di sampah/workspace lain → 404; purge menghapus file storage → Task 7, Task 8.
5. Pencarian hanya mengembalikan tiket dari project yang bisa dibuka user → Task 9.

---

## File Structure

```
server/migration/src/m20261113_000001_create_tickets.rs   tipe, field, tiket, kode, relasi, komentar,
                                                          riwayat, lampiran, watcher, notifikasi, izin baru
server/src/models/{ticket_types,custom_fields,tickets,ticket_codes,ticket_links,ticket_comments,
                   ticket_events,ticket_attachments,ticket_watchers,notifications,ticket_counters}.rs
server/src/models/ticket_code.rs        parser/render template kode (unit test)
server/src/services/tickets/            create, update, transition, comments, links, attachments,
                                        trash, mentions, settings (tipe/field/kode), search
server/src/controllers/extractors/ticket_access.rs
server/src/controllers/tickets/         board, list, detail, comments, links, attachments, trash
server/src/controllers/workspaces/{inbox,my_tickets,search,ticket_settings}.rs
server/src/views/tickets.rs, views/ticket_settings.rs, views/inbox.rs
server/assets/views/tickets/ inbox/ ticket_settings/ ; components/{ticket_card,prio,ticket_type_icon}.html
server/assets/static/js/board-dnd.js
server/tests/requests/tickets/*.rs, tests/models/{ticket_code,tickets}.rs, tests/workers/cleanup.rs
```

### Task 1: Migrasi, entity, izin `ManageTicketSettings`, tipe bawaan

- [ ] Tes: enum CHECK baru (`tickets.status`, `tickets.priority`, `ticket_links.kind`, `custom_fields.kind`,
  `ticket_events.kind`, `notifications.kind`, `workspaces.ticket_counter_scope`, aksi/subjek audit baru)
  di `tests/models/enums.rs`; `workspace_gets_builtin_ticket_types`; permission baru tersimpan.
- [ ] Migrasi + `bin/loco db entities`; backfill tipe bawaan untuk workspace lama (raw SQL di migrasi);
  `create::run_in` membuat Epic/Story/Task/Bug/Subtask.
- [ ] Commit `feat(server): add ticket schema and built-in ticket types`.

### Task 2: Template kode tiket

- [ ] Unit test `ticket_code`: render `{PROJECT}-{NUM}`, `{TEAM}-{YYYY}-{NUM:4}` → `BE-2026-0042`,
  `{TYPE}`, `{YY}`; tolak template tanpa `{NUM}`, token tak dikenal, karakter di luar `[A-Za-z0-9-_./]`;
  skop `type` tanpa `{TYPE}` ditolak.
- [ ] Commit `feat(server): ticket code templates`.

### Task 3: Buat, lihat, ubah tiket

- [ ] Tes request: `member_creates_ticket_with_next_code`, `viewer_cannot_create_ticket_gets_404`,
  `ticket_of_other_workspace_gets_404`, `old_code_alias_redirects_to_current_code`,
  `required_custom_field_enforced`, `edit_records_history`, `assignee_must_have_project_access`.
- [ ] `services::tickets::{create, update}`, extractor `TicketAccess`, halaman Tiket baru dan Detail.
- [ ] Commit `feat(server): create, view and edit tickets`.

### Task 4: Board Kanban dan Daftar

- [ ] Tes: board menampilkan kolom status dan kartu; filter `q`/`assignee=me`; daftar datatable dengan sort,
  filter status/prioritas/tipe, pagination; tab project Board/Daftar aktif.
- [ ] Commit `feat(server): kanban board and ticket list`.

### Task 5: Status dan relasi (blocks ditegakkan)

- [ ] Tes: `blocked_ticket_cannot_start_until_blocker_done` (POST status dari board dan detail → 422 +
  pesan), `relation_to_self_or_other_project_rejected`, `duplicates_and_relates_do_not_block`,
  `unlink_records_history`.
- [ ] Commit `feat(server): ticket links with enforced blocks`.

### Task 6: Komentar, @mention, watcher, kotak masuk

- [ ] Tes: `mention_creates_inbox_notification`, `mention_of_user_without_project_access_is_ignored`,
  `assigning_notifies_assignee`, `comment_notifies_watchers_except_author`, `mark_read_is_post_only`,
  `edit_own_comment_only`.
- [ ] Commit `feat(server): comments, mentions, watchers and inbox`.

### Task 7: Lampiran S3

- [ ] Tes (storage memori): upload → baris + file; unduh butuh akses; > 10 MB → 422; hapus menghapus file;
  dev memakai `kind: Aws` ke SeaweedFS (`http://s3:8333`, bucket dibuat `bin/_lib.sh`).
- [ ] Commit `feat(server): ticket attachments on S3 storage`.

### Task 8: Sampah 30 hari

- [ ] Tes: hapus → tiket hilang dari board/detail (404), muncul di Sampah; pulihkan; hapus permanen hanya
  Admin/pembuat; `cleanup_purges_tickets_after_30_days_only` (termasuk file lampiran), idempoten.
- [ ] Commit `feat(server): ticket trash with 30-day purge`.

### Task 9: Pencarian dasar, Tiket saya

- [ ] Tes: `search_finds_by_code_title_and_description`, `search_excludes_projects_without_access`,
  `search_excludes_trashed`, Tiket saya per tab (ditugaskan, dilaporkan, diikuti).
- [ ] Commit `feat(server): basic search and my tickets`.

### Task 10: Pengaturan tipe tiket, field custom, kode tiket

- [ ] Tes: buat/ubah tipe custom, tipe bawaan tidak bisa dihapus, tipe dipakai tiket tidak bisa dihapus;
  field custom per tipe; ubah template workspace mengubah kode tiket lama menjadi alias; override
  project; member tanpa izin → 404.
- [ ] Commit `feat(server): ticket type, field and code settings`.

### Task 11: Penutup

- [ ] Screenshot Playwright desktop, 390 px, gelap; bandingkan dengan prototype; katalog `/_dev/ui`
  (komponen baru); seed tiket contoh; `bin/ci` hijau; PROGRESS; merge `--no-ff` ke `staging`.
