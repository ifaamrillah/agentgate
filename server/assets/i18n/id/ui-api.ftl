## M11: API & webhook

## Audit dan notifikasi
audit-action-api-token-created = Token API dibuat
audit-action-api-token-revoked = Token API dicabut
audit-action-webhook-created = Webhook dibuat
audit-action-webhook-updated = Webhook diubah
audit-action-webhook-deleted = Webhook dihapus
audit-action-webhook-secret-rotated = Secret webhook dirotasi
audit-subject-api-token = Token API
audit-subject-webhook = Webhook
notification-webhook_disabled = Webhook dinonaktifkan otomatis setelah 5 kali gagal kirim. Periksa tujuannya lalu aktifkan lagi.

## Komponen copy_value
copy-action = Salin
copy-done = Tersalin

## Halaman
api-title = API & webhook
api-description = REST API JSON untuk integrasi pihak ketiga, dan webhook untuk menerima event dari Agent Gate.
api-panel-tokens = Token API
api-panel-tokens-desc = Akses REST API
api-panel-hooks = Webhook
api-panel-hooks-desc = Event keluar
api-panel-log = Log pengiriman
api-panel-log-desc = 30 hari terakhir
api-panel-docs = Dokumentasi
api-panel-docs-desc = Endpoint dan contoh

api-col-name = Nama
api-col-scopes = Cakupan
api-col-used = Terakhir dipakai
api-col-expires = Kedaluwarsa
api-col-actions = Aksi
api-col-target = Tujuan
api-col-last = Pengiriman terakhir
api-col-active = Aktif
api-col-time = Waktu
api-col-event = Event
api-col-status = Status
api-col-duration = Durasi
api-filter-creator = Pembuat
api-filter-result = Hasil
api-result-ok = Berhasil
api-result-fail = Gagal
api-result-pending = Menunggu
api-tokens-search = Cari token…
api-log-search = Cari event atau URL…
api-project = Project

## Token
api-tokens-description = Cakupan membatasi apa yang bisa dilakukan token. Token ikut role pembuatnya dan hanya disimpan sebagai digest.
api-tokens-empty = Belum ada token API
api-tokens-empty-description = Buat token untuk menghubungkan skrip, dashboard, atau layanan lain ke REST API.
api-token-new = Token baru
api-token-new-description = Token bertindak atas nama Anda, dengan izin role Anda di workspace ini.
api-token-name-placeholder = Misalnya: Dashboard BI toko
api-token-create = Buat token
api-token-created = Token dibuat
api-token-created-description = Salin sekarang. Token ini tidak akan ditampilkan lagi.
api-token-created-warning = Simpan di secret manager. Kalau hilang, cabut token ini lalu buat yang baru.
api-token-saved = Sudah saya simpan
api-copy-token = Salin token
api-never-used = Belum pernah
api-expired = Kedaluwarsa
api-expiry-30 = 30 hari
api-expiry-90 = 90 hari
api-expiry-365 = 1 tahun
api-expiry-never = Tanpa batas
api-revoke = Cabut
api-revoke-title = Cabut token “{ $name }”?
api-revoke-description = Integrasi yang memakai token ini langsung berhenti. Tindakan ini tidak bisa dibatalkan.
api-revoke-confirm = Cabut token
api-scope-tiket-baca = Membaca tiket, komentar, dan riwayat
api-scope-tiket-tulis = Membuat dan mengubah tiket (tetap lewat gate)
api-scope-komentar-tulis = Menulis komentar
api-scope-laporan-baca = Membaca laporan dan sprint
api-scope-knowledge-baca = Membaca knowledge base
api-scope-webhook-kelola = Mengatur webhook
api-token-scopes-required = Pilih minimal satu cakupan.
api-token-expiry-invalid = Pilih masa berlaku token.
api-token-project-invalid = Project tidak ditemukan atau tidak bisa Anda akses.

## Webhook
api-hooks-description = Payload ditandatangani HMAC-SHA256 di header X-AgentGate-Signature. Pengiriman yang gagal diulang hingga 5 kali.
api-hooks-empty = Belum ada webhook
api-hooks-empty-description = Tambahkan URL tujuan untuk menerima event tiket, sprint, rilis, dan job.
api-hook-new = Webhook baru
api-hook-new-description = Secret penanda tangan dibuat otomatis dan ditampilkan sekali setelah webhook dibuat.
api-hook-create = Buat webhook
api-hook-edit-title = Ubah webhook
api-hook-edit = Ubah webhook { $host }
api-hook-menu = Opsi webhook { $host }
api-hook-toggle = Aktifkan webhook { $host }
api-hook-url = URL tujuan
api-hook-url-hint = Harus HTTPS dan bisa dijangkau dari internet.
api-hook-events-placeholder = Pilih event…
api-hook-projects-hint = Pilih "Semua project" atau batasi ke project tertentu.
api-hook-off = Dinonaktifkan
api-hook-none = Belum ada pengiriman
api-hook-failures = gagal { $n }×
api-no-response = Tanpa respons
api-hook-history = Riwayat pengiriman
api-hook-ping = Kirim event uji
api-hook-rotate = Rotasi secret
api-hook-delete = Hapus webhook
api-hook-delete-title = Hapus webhook { $host }?
api-hook-delete-description = Event tidak lagi dikirim ke URL ini dan riwayat pengirimannya ikut terhapus.
api-hook-created = Webhook dibuat
api-hook-rotated = Secret baru dibuat
api-secret-description = Pakai secret ini untuk memverifikasi header X-AgentGate-Signature. Secret tidak akan ditampilkan lagi.
api-secret-warning = Secret lama langsung tidak berlaku. Perbarui penerima Anda sebelum event berikutnya terkirim.
api-copy-secret = Salin secret
webhook-url-invalid = Masukkan URL HTTPS yang valid, tanpa nama pengguna atau kata sandi.
webhook-url-forbidden = Alamat ini mengarah ke jaringan internal sehingga tidak diizinkan.
webhook-events-required = Pilih minimal satu event.
webhook-project-invalid = Project tidak ditemukan atau tidak bisa Anda akses.
webhook-secret-unavailable = Secret tidak bisa dibuat karena kunci enkripsi server belum diatur.
webhook-inactive = Webhook sedang nonaktif. Aktifkan dulu untuk mengirim event.

## Log pengiriman
api-log-description = Pengiriman 30 hari terakhir. Buka baris untuk melihat payload dan respons.
api-log-empty = Belum ada pengiriman
api-log-empty-description = Event yang dikirim ke webhook Anda akan muncul di sini.
api-view = Lihat
api-payload = Payload
api-response = Respons
api-attempts = Percobaan ke-{ $n } dari 5.
api-retry-note = Pengiriman diulang otomatis dengan jeda bertahap. Setelah 5 kali gagal beruntun, webhook dinonaktifkan dan Owner serta Admin diberi tahu.
api-resend = Kirim ulang
api-error-timeout = Tujuan tidak merespons sebelum batas waktu.
api-error-connect-failed = Tidak bisa terhubung ke tujuan.
api-error-request-failed = Permintaan gagal dikirim.
api-error-client-error = Klien HTTP gagal disiapkan.
api-error-secret-unavailable = Secret webhook tidak bisa dibaca.
api-error-dns-failed = Nama host tujuan tidak bisa di-resolve.
api-error-webhook-disabled = Webhook nonaktif, pengiriman dihentikan.
api-error-url-invalid = URL tujuan tidak valid.
api-error-url-forbidden = Alamat tujuan mengarah ke jaringan internal dan diblokir.
api-error-other = Pengiriman gagal.

## Flash
api-flash-token-revoked = Token dicabut.
api-flash-hook-created = Webhook dibuat.
api-flash-hook-updated = Webhook disimpan.
api-flash-hook-deleted = Webhook dihapus.
api-flash-hook-on = Webhook diaktifkan.
api-flash-hook-off = Webhook dinonaktifkan.
api-flash-ping = Event uji dijadwalkan. Hasilnya muncul di log dalam beberapa detik.
api-flash-resent = Pengiriman dijadwalkan ulang.

## Dokumentasi
api-docs-description = REST API JSON. Spesifikasi lengkap tersedia dalam format OpenAPI 3.
api-openapi = Unduh OpenAPI
api-endpoint = Endpoint
api-base-url = Base URL
api-copy-base = Salin base URL
api-auth = Autentikasi
api-auth-hint = Header
api-limit = Batas pemakaian
api-limit-hint = { $n } request per menit per token · header
api-signature = Tanda tangan webhook
api-signature-hint = Verifikasi di penerima:
api-clients = Client AI dan runner
api-clients-hint = Codex, Gemini CLI, Cursor, OpenCode, dan Copilot CLI memakai MCP dan runner, bukan token API.
api-clients-link = Hubungkan client
api-example = Contoh request
