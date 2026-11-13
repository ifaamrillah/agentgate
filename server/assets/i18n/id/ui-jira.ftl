## M11: Connector Jira

## Audit dan notifikasi
audit-action-jira-connected = Menghubungkan Jira
audit-action-jira-updated = Mengubah koneksi Jira
audit-action-jira-disconnected = Memutus koneksi Jira
audit-subject-jira-connection = Koneksi Jira
notification-jira_failed = Sinkron Jira gagal. Periksa koneksi di Pengaturan → Jira.

## Halaman
jira-title = Jira
jira-description = Connector Jira untuk impor, mirror, atau sinkron dua arah dengan project Agent Gate.
jira-panel-overview = Ringkasan
jira-panel-overview-desc = Status dan 24 jam terakhir
jira-panel-conn = Koneksi
jira-panel-conn-desc = Kredensial dan project
jira-panel-mode = Mode sinkron
jira-panel-mode-desc = Impor, mirror, dua arah
jira-panel-status = Status ↔ stage
jira-panel-status-desc = Pemetaan status Jira
jira-panel-fields = Field & user
jira-panel-fields-desc = Pemetaan field dan orang
jira-panel-log = Log sinkron
jira-panel-log-desc = 30 hari terakhir

jira-empty = Jira belum terhubung
jira-empty-description = Hubungkan Jira Cloud atau Data Center untuk mengimpor issue, menjadikan Jira cermin, atau menyinkronkan dua arah.
jira-connect = Hubungkan Jira
jira-choose-project = Pilih project dulu
jira-not-ready = Pilih project Jira dan project Agent Gate di panel Koneksi untuk memuat pemetaan.

## Ringkasan
jira-overview-description = Status koneksi dan sinkron 24 jam terakhir.
jira-connected-to = Terhubung ke { $host }
jira-paused = dijeda
jira-running = sinkron berjalan…
jira-last-sync = terakhir { $when }
jira-never-synced = belum pernah sinkron
jira-sync-now = Sinkron sekarang
jira-menu = Opsi koneksi
jira-pause = Jeda sinkron
jira-resume = Lanjutkan sinkron
jira-resync = Sinkron ulang semua
jira-disconnect = Putuskan koneksi
jira-disconnect-title = Putuskan koneksi ke { $host }?
jira-disconnect-description = Sinkron berhenti dan pemetaan serta log sinkron dihapus. Tiket, komentar, dan lampiran yang sudah diimpor tetap ada di Agent Gate.
jira-unmapped-alert = Status Jira { $names } belum dipetakan.
jira-unmapped-fix = Atur pemetaan status
jira-last-error = Sinkron terakhir gagal: { $reason }
jira-stat-tickets = Tiket tersinkron
jira-stat-tickets-hint = { $value }
jira-stat-changes = Perubahan 24 jam
jira-stat-changes-hint = { $value } masuk dari Jira
jira-stat-gate = Ditolak gate
jira-stat-gate-hint = Lihat di log sinkron
jira-current = Konfigurasi saat ini
jira-summary-status = Status
jira-summary-fields = Field
jira-summary-users = User
jira-summary-data = Data
jira-summary-of = { $n } dari { $total } dipetakan
jira-summary-field-count = { $n } field disinkronkan
jira-summary-data-value = { $tickets } tiket, { $comments } komentar, { $attachments } lampiran, { $sprints } sprint
jira-importing = Impor berjalan · { $n } tiket sejauh ini

## Koneksi
jira-conn-description = Jenis Jira, kredensial, dan project yang disinkronkan.
jira-env = Jenis Jira
jira-env-cloud = Jira Cloud
jira-env-cloud-hint = atlassian.net, login dengan email dan API token
jira-env-data_center = Jira Data Center
jira-env-data_center-hint = Server sendiri, login dengan Personal Access Token
jira-credentials = Kredensial
jira-url = URL site atau server
jira-url-hint = Contoh: https://nama.atlassian.net atau https://jira.perusahaan.id
jira-email = Email akun
jira-email-hint = Wajib untuk Jira Cloud.
jira-token = API token atau Personal Access Token
jira-token-hint = Jira Cloud: buat di id.atlassian.com → Security → API tokens. Disimpan terenkripsi.
jira-token-kept = Tersimpan. Kosongkan untuk tetap memakai token ini
jira-test = Uji koneksi
jira-tested = Terhubung sebagai { $name } · { $n } project terlihat
jira-connected-as = Terhubung sebagai { $name }
jira-projects = Project
jira-jira-project = Project Jira
jira-project-hint = Simpan atau uji koneksi dulu untuk memuat daftar project Jira.
jira-ag-project = Project Agent Gate

## Mode
jira-mode-description = Bisa diganti kapan saja. Impor sekali tidak mendorong perubahan ke Jira.
jira-mode-once = Impor sekali
jira-mode-once-hint = Tiket, komentar, lampiran, user, sprint, dan status dipindahkan ke Agent Gate. Setelah itu tidak ada sinkron lagi.
jira-mode-mirror = Mirror satu arah
jira-mode-mirror-hint = Agent Gate → Jira. Jira menjadi cermin yang hanya untuk dilihat; perubahan di Jira ditimpa.
jira-mode-two_way = Sinkron dua arah
jira-mode-two_way-hint = Perubahan di kedua sisi disinkronkan, ditarik dari Jira sekitar tiap 5 menit.
jira-conflict = Aturan bentrok
jira-conflict-hint = Berlaku saat Jira dan Agent Gate mengubah tiket yang sama.
jira-rule-status = Status dan stage selalu mengikuti Agent Gate.
jira-rule-status-hint = Gate ditegakkan di sini, jadi perpindahan status dari Jira yang belum memenuhi gate ditolak, dikembalikan, dan dicatat di log.
jira-rule-fields = Field lain mengikuti perubahan terakhir.
jira-rule-fields-hint = Judul, deskripsi, label, prioritas, due date, dan story point memakai yang paling baru diubah. Komentar dari kedua sisi disalin.
jira-save-sync = Simpan & sinkron
jira-save-note = Perubahan berlaku pada sinkron berikutnya.

## Status
jira-status-description = Status Jira project { $jira } dipetakan ke stage workflow { $project }.
jira-status-unmapped = Status { $names } belum dipetakan. Issue dengan status ini tetap di stage-nya sekarang.
jira-status-jira = Status Jira ({ $key })
jira-status-stage = Stage Agent Gate ({ $key })
jira-status-for = Stage untuk status { $name }
jira-unmapped = Tidak dipetakan

## Field & user
jira-fields-description = Field Jira dipetakan ke field Agent Gate. User dicocokkan otomatis lewat email.
jira-fields = Field
jira-field-jira = Field Jira
jira-field-ag = Field Agent Gate
jira-field-for = Field Agent Gate untuk { $name }
jira-target-title = Judul
jira-target-description = Deskripsi
jira-target-story_points = Story point
jira-target-sprint = Sprint
jira-target-release = Rilis
jira-target-component = Component
jira-target-due_on = Due date
jira-target-labels = Label
jira-target-priority = Prioritas
jira-target-none = Tidak disinkronkan
jira-target-custom-group = Field custom
jira-users = User
jira-users-hint = Penerima issue Jira dipetakan ke anggota workspace.
jira-users-unmapped = { $names } belum dipetakan. Tiketnya diimpor tanpa penerima, atau
jira-users-invite = undang dulu
jira-users-empty = Belum ada user Jira
jira-users-empty-description = Daftar user dimuat dari project Jira saat koneksi disimpan.
jira-user-jira = User Jira
jira-user-ag = Anggota Agent Gate
jira-user-for = Anggota untuk { $name }
jira-user-unmapped = Belum dipetakan

## Log
jira-log-description = Perubahan yang ditolak gate dikembalikan ke status sebelumnya di Jira.
jira-log-download = Unduh log
jira-log-empty = Belum ada log sinkron
jira-log-empty-description = Setiap perubahan yang dikirim ke atau ditarik dari Jira dicatat di sini.
jira-log-search = Cari tiket atau pesan…
jira-col-time = Waktu
jira-col-result = Hasil
jira-col-ticket = Tiket
jira-col-message = Pesan
jira-filter-result = Hasil
jira-filter-direction = Arah
jira-result-ok = Berhasil
jira-result-gate = Ditolak gate
jira-result-fail = Gagal
jira-dir-in = Jira → Agent Gate
jira-dir-out = Agent Gate → Jira

jira-log-run-failed = Sinkron dihentikan: { $reason }
jira-log-owner-missing = Pemilik koneksi bukan anggota lagi. Simpan ulang koneksi untuk memakai akun Anda.
jira-log-push-failed = Perubahan ditolak Jira: { $detail }
jira-log-issue-created = Issue baru dibuat di Jira
jira-log-issue-updated = Diperbarui di Jira: { $detail }
jira-log-stage-unmapped = Stage { $stage } belum dipetakan ke status Jira
jira-log-status-pushed = Status Jira diubah ke { $status }
jira-log-transition-failed = Tidak ada transisi Jira ke status { $status }
jira-log-comment-failed = Komentar tidak bisa disinkronkan
jira-log-comment-pushed = Komentar dikirim ke Jira
jira-log-comment-pulled = Komentar dari Jira disalin
jira-log-search-failed = Pencarian issue Jira ditolak: { $detail }
jira-log-ticket-created = Tiket baru dari Jira dibuat di stage awal
jira-log-ticket-updated = Diperbarui dari Jira: { $detail }
jira-log-pull-failed = Perubahan dari Jira ditolak: { $reason }
jira-log-stage-moved = Status “{ $status }” dari Jira memindahkan tiket ke stage { $stage }
jira-log-gate-rejected = Status “{ $status }” dari Jira ditolak: { $reason } Dikembalikan ke { $back }.
jira-log-gate-rejected-kept = Status “{ $status }” dari Jira ditolak: { $reason } Status Jira tidak bisa dikembalikan.
jira-log-ticket-imported = Diimpor dari Jira
jira-log-user-unmapped = User { $email } belum dipetakan, penerima dikosongkan
jira-log-stage-gated = Status “{ $status }” butuh gate sebelum stage { $stage }; tiket masuk stage awal
jira-log-import-failed = Issue tidak bisa diimpor: { $reason }
jira-log-attachment-skipped = Lampiran { $detail } ({ $size } MB) melebihi batas 25 MB, dilewati
jira-log-attachment-failed = Lampiran { $detail } gagal diunduh
jira-reason-unmapped = status ini belum dipetakan ke stage.
jira-reason-no-access = pemilik koneksi tidak punya akses ke tiket ini.

## Flash
jira-flash-saved = Koneksi Jira disimpan.
jira-flash-syncing = Konfigurasi disimpan, sinkron berjalan.
jira-flash-paused = Sinkron Jira dijeda.
jira-flash-resumed = Sinkron Jira dilanjutkan.
jira-flash-full = Sinkron ulang semua dijadwalkan. Nilai di Jira akan ditimpa.
jira-flash-disconnected = Koneksi Jira diputus. Data yang sudah diimpor tetap ada.

## Error
jira-url-invalid = Masukkan URL https yang valid.
jira-url-forbidden = Alamat ini tidak diizinkan (jaringan internal atau lokal).
jira-email-required = Email akun wajib untuk Jira Cloud.
jira-token-required = Masukkan API token.
jira-token-invalid = Token ditolak Jira. Periksa email dan token.
jira-unreachable = Jira tidak bisa dijangkau. Coba lagi nanti.
jira-rejected = Jira menolak permintaan ini.
jira-env-invalid = Pilih jenis Jira.
jira-mode-invalid = Pilih mode sinkron.
jira-project-invalid = Pilih project Agent Gate yang aktif.
jira-jira-project-invalid = Pilih project Jira yang terlihat oleh akun ini.
jira-not-connected = Hubungkan Jira dan pilih project dulu.
jira-stage-invalid = Stage tidak ada di workflow project.
jira-field-invalid = Field tujuan tidak dikenal.
jira-user-invalid = User harus anggota workspace.
