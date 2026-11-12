## M3: workflow, stage, gate, artifact

## Riwayat tiket
ticket-field-stage = Stage
ticket-event-stage-changed = memindahkan stage
ticket-event-review-requested = meminta review { $detail }
ticket-event-approved = menyetujui stage
ticket-event-rejected = menolak stage
ticket-event-decision-needed = menandai tiket butuh keputusan
ticket-event-decided = mengambil keputusan
ticket-event-stage-jumped = melompati stage
ticket-event-transition-forced = memaksa transisi
ticket-event-workflow-migrated = memigrasi tiket ke workflow baru
ticket-event-artifact-saved = menyimpan artifact { $detail }
ticket-event-people-changed = mengubah penugasan stage { $detail }

## Notifikasi
notification-review_requested = meminta review Anda
notification-approved = menyetujui stage tiket Anda
notification-rejected = menolak stage tiket Anda
notification-decision_needed = menandai tiket butuh keputusan Anda

## Audit
audit-action-workflow-created = Workflow dibuat
audit-action-workflow-updated = Workflow diubah
audit-action-workflow-deleted = Workflow dihapus
audit-action-workflow-migrated = Tiket dimigrasi ke versi workflow baru
audit-action-project-workflows-changed = Workflow project diubah
audit-action-ticket-stage-jumped = Stage tiket dilompati
audit-action-ticket-transition-forced = Transisi tiket dipaksa
audit-subject-workflow = Workflow

## Error gate
ticket-stage-unknown = Stage tidak ada di workflow tiket ini.
ticket-needs-approval = Stage ini harus di-approve dulu. Minta review.
ticket-jump-admin-only = Melewati stage yang punya gate atau approval hanya untuk Admin project.
ticket-gates-unmet = Gate stage ini belum terpenuhi.
ticket-stage-busy = Stage ini sedang menunggu review atau keputusan.
ticket-not-approver = Anda bukan approver stage ini.
ticket-not-in-review = Stage ini tidak sedang menunggu review.
ticket-not-in-decision = Tiket ini tidak sedang menunggu keputusan.
ticket-no-approval-step = Stage ini tidak memakai approval.
ticket-last-stage = Tiket sudah di stage terakhir.
ticket-note-required = Tulis catatan.
ticket-note-too-long = Catatan terlalu panjang (maks. 2.000 karakter).
ticket-reason-required = Tulis alasan.
ticket-force-disabled = Paksa transisi dimatikan di project ini.
ticket-decision-invalid = Pilih salah satu keputusan.
ticket-decision-target-invalid = Pilih stage sebelum stage sekarang.
ticket-stage-people-invalid = Orang yang dipilih tidak punya akses ke project ini.

## Error workflow
workflow-name-required = Isi nama workflow.
workflow-name-too-long = Nama workflow terlalu panjang (maks. 60 karakter).
workflow-name-taken = Nama workflow sudah dipakai.
workflow-description-too-long = Deskripsi terlalu panjang (maks. 300 karakter).
workflow-in-use = Workflow masih dipakai tipe tiket, project, atau tiket.
workflow-stages-empty = Workflow butuh minimal satu stage.
workflow-stages-too-many = Terlalu banyak stage (maks. 30).
workflow-stage-key-invalid = ID stage hanya huruf kecil, angka, dan garis bawah, diawali huruf.
workflow-stage-key-taken = ID stage sudah dipakai stage lain.
workflow-stage-name-required = Isi nama stage.
workflow-stage-name-too-long = Nama stage terlalu panjang (maks. 60 karakter).
workflow-stage-name-taken = Nama stage sudah dipakai stage lain.
workflow-last-stage-not-done = Stage terakhir harus berkategori Selesai.
workflow-category-order = Kategori stage harus berurutan: Backlog, To do, Dikerjakan, Selesai.
workflow-first-stage-parallel = Stage pertama tidak bisa paralel.
workflow-fail-to-invalid = Stage tujuan saat gagal harus stage sebelumnya.
workflow-fail-limit-invalid = Batas putaran gagal 1 sampai 9.
workflow-artifact-name-invalid = Nama artifact hanya huruf, angka, titik, garis bawah, dan strip.
workflow-text-too-long = Teks terlalu panjang.
workflow-mode-needs-persona = Mode otomatis dan interaktif butuh persona.
workflow-gate-duplicate = Gate yang sama dipilih dua kali.
workflow-tool-invalid = Nama tooling tidak valid.
workflow-list-too-long = Daftar terlalu panjang.
workflow-parallel-category = Stage paralel harus sekategori dengan stage sebelumnya.
workflow-yaml-syntax = YAML tidak valid: { $detail }
workflow-yaml-unknown-value = Nilai tidak dikenal: { $detail }
workflow-yaml-key-invalid = Kunci workflow tidak valid.
workflow-yaml-name-required = Isi nama workflow di YAML.

## Error artifact
artifact-name-invalid = Nama artifact hanya huruf, angka, titik, garis bawah, dan strip (maks. 80).
artifact-name-taken = Nama artifact sudah dipakai di tiket ini.
artifact-body-too-long = Isi artifact terlalu panjang (maks. 200.000 karakter).
artifact-unchanged = Belum ada perubahan dari versi terakhir.
