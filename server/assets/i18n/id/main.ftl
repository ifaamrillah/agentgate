app-name = Agent Gate

nav-home = Beranda
nav-account = Akun
nav-logout = Keluar

home-title = Beranda
home-tagline = Manajemen proyek dengan AI agent untuk seluruh siklus pengembangan produk.

login-title = Masuk
login-description = Masuk dengan email dan password kamu.
login-email = Email
login-password = Password
login-submit = Masuk
login-forgot-password = Lupa password?
login-or = atau
login-with-google = Masuk dengan Google
login-with-github = Masuk dengan GitHub
login-no-account = Belum punya akun?
login-register = Daftar
auth-invalid-credentials = Email atau password salah.
auth-too-many-attempts = Terlalu banyak percobaan login. Coba lagi dalam 15 menit.
auth-too-many-requests = Terlalu banyak permintaan untuk email ini. Coba lagi dalam 15 menit.
flash-email-verified = Email terverifikasi. Silakan masuk.
flash-password-reset = Password berhasil diganti. Silakan masuk dengan password baru.
auth-second-factor-invalid = Kode salah. Coba lagi.
flash-second-factor-locked = Terlalu banyak kode salah. Masuk lagi dalam 15 menit.
flash-second-factor-expired = Waktu memasukkan kode habis. Silakan masuk lagi.
oauth-cancelled = Login dibatalkan.
oauth-expired = Waktu login sudah habis atau tautannya tidak berlaku. Silakan mulai lagi.
oauth-failed = Login gagal. Silakan coba lagi.
oauth-email-unverified = Email akun ini belum diverifikasi oleh Google atau GitHub. Verifikasi email di sana, atau masuk dengan email dan password.
oauth-registration-closed = Belum ada akun Agent Gate untuk email ini, dan pendaftaran sedang ditutup.
oauth-user-disabled = Akun ini dinonaktifkan.
oauth-identity-conflict = Email ini sudah tertaut ke akun lain di penyedia yang sama. Masuk dengan akun yang dulu dipakai, atau dengan email dan password.

two-factor-title = Verifikasi dua langkah
two-factor-description = Masukkan kode 6 digit dari aplikasi authenticator kamu.
two-factor-code = Kode
two-factor-recovery-hint = Tidak bisa membuka aplikasi authenticator? Masukkan salah satu kode pemulihan.
two-factor-submit = Verifikasi
two-factor-back = Batal dan kembali ke halaman masuk

verify-pending-title = Cek email untuk verifikasi
verify-pending-description = Email kamu belum diverifikasi.
verify-pending-body = Kami sudah mengirim link verifikasi ke email kamu. Buka link itu, lalu masuk lagi.
verify-pending-back = Kembali ke halaman masuk
verify-pending-resend = Kirim ulang email verifikasi

register-title = Daftar
register-description = Buat akun Agent Gate.
register-name = Nama
register-email = Email
register-password = Password
register-submit = Daftar
register-have-account = Sudah punya akun?
register-login = Masuk

check-email-title = Cek email kamu
check-email-description = Langkah berikutnya ada di inbox kamu.
check-email-body = Bila alamat itu bisa menerima email dari kami, sebentar lagi kamu akan menerima email berisi link untuk melanjutkan. Cek juga folder spam.
check-email-back = Kembali ke halaman masuk

verify-email-title = Verifikasi email
verify-email-description = Klik tombol di bawah untuk menyelesaikan verifikasi email kamu.
verify-email-submit = Verifikasi email

token-invalid-title = Link tidak berlaku
token-invalid-verify-body = Link verifikasi ini sudah tidak berlaku. Masuk dengan email dan password kamu untuk meminta link baru.
token-invalid-verify-action = Ke halaman masuk
token-invalid-reset-body = Link reset password ini sudah tidak berlaku. Link hanya bisa dipakai sekali dan berlaku dalam waktu singkat.
token-invalid-reset-action = Minta link baru

forgot-title = Lupa password
forgot-description = Masukkan email akun kamu. Kami akan mengirim link untuk membuat password baru.
forgot-email = Email
forgot-submit = Kirim link reset
forgot-back = Kembali ke halaman masuk

reset-title = Buat password baru
reset-description = Password minimal 12 karakter. Setelah diganti, semua sesi login lain berakhir.
reset-password = Password baru
reset-submit = Simpan password

account-title = Akun
account-profile = Profil
account-email = Email
account-name = Nama
account-save = Simpan
account-security = Keamanan
account-change-password = Ganti password
account-two-factor = Autentikasi dua langkah
account-two-factor-on = aktif
account-two-factor-off = belum aktif
flash-profile-saved = Profil disimpan.

change-password-title = Ganti password
change-password-description = Setelah password diganti, semua sesi login di perangkat lain berakhir.
change-password-current = Password saat ini
change-password-new = Password baru
change-password-submit = Ganti password
change-password-back = Kembali ke akun
flash-password-changed = Password berhasil diganti.

totp-setup-title = Aktifkan autentikasi dua langkah
totp-setup-description = Pindai QR code di bawah dengan aplikasi authenticator (mis. Google Authenticator, Authy, 1Password), lalu masukkan kode 6 digit yang muncul.
totp-setup-qr-label = QR code untuk aplikasi authenticator
totp-setup-manual = Tidak bisa memindai? Masukkan kunci ini secara manual:
totp-setup-code = Kode dari aplikasi
totp-setup-submit = Aktifkan
totp-code-invalid = Kode salah. Pastikan jam di ponsel kamu tepat, lalu coba lagi.
totp-back = Kembali ke akun
totp-status-title = Autentikasi dua langkah aktif
totp-recovery-title = Kode pemulihan
totp-recovery-remaining = Sisa { $count } kode pemulihan yang belum dipakai.
totp-recovery-regenerate = Buat kode pemulihan baru
totp-disable-title = Matikan autentikasi dua langkah
totp-disable-description = Masukkan password saat ini untuk mematikan autentikasi dua langkah. Semua kode pemulihan ikut dihapus.
totp-disable-submit = Matikan
totp-codes-title = Simpan kode pemulihan
totp-codes-description = Setiap kode hanya bisa dipakai sekali untuk masuk bila kamu tidak bisa membuka aplikasi authenticator. Simpan di tempat aman: kode ini hanya ditampilkan sekarang. Kode pemulihan lama tidak berlaku lagi.
totp-codes-done = Selesai, kode sudah saya simpan
flash-two-factor-disabled = Autentikasi dua langkah dimatikan.

# Email (assets/views/mailers/). Hanya karakter ASCII supaya isi email tetap 7bit.
mail-greeting = Halo { $name },
mail-footer = Email ini dikirim otomatis oleh { app-name }. Jangan membalas email ini.
mail-verify-subject = Verifikasi email kamu di { app-name }
mail-verify-body = Terima kasih sudah mendaftar. Buka link di bawah untuk memverifikasi email kamu.
mail-verify-action = Verifikasi email
mail-verify-expiry = Link ini berlaku { $hours } jam. Bila kamu tidak merasa mendaftar, abaikan email ini.
mail-already-registered-subject = Email kamu sudah terdaftar di { app-name }
mail-already-registered-body = Seseorang (mungkin kamu) mencoba mendaftar dengan email ini, padahal email ini sudah punya akun. Bila kamu lupa password, buat password baru lewat link di bawah.
mail-already-registered-action = Buat password baru
mail-already-registered-expiry = Link ini berlaku { $minutes } menit. Bila kamu tidak mencoba mendaftar, abaikan email ini; akun kamu tidak berubah.
mail-already-registered-login = Masih ingat password? Masuk di
mail-password-reset-subject = Reset password { app-name }
mail-password-reset-body = Kami menerima permintaan reset password untuk akun kamu. Buka link di bawah untuk membuat password baru.
mail-password-reset-action = Buat password baru
mail-password-reset-expiry = Link ini berlaku { $minutes } menit dan hanya bisa dipakai sekali. Bila kamu tidak meminta reset, abaikan email ini; password kamu tidak berubah.

error-code = Kode error
error-back-home = Kembali ke beranda
error-403-title = Akses ditolak
error-403-body = Kamu tidak punya izin untuk melakukan aksi ini.
error-404-title = Halaman tidak ditemukan
error-404-body = Halaman yang kamu cari tidak ada atau sudah dipindahkan.
error-422-title = Data tidak valid
error-422-body = Periksa kembali isian kamu lalu coba lagi.
error-429-title = Terlalu banyak permintaan
error-429-body = Tunggu sebentar lalu coba lagi.
error-500-title = Terjadi kesalahan
error-500-body = Ada masalah di server kami. Sebutkan kode error di bawah saat melapor.
error-503-title = Server sedang sibuk
error-503-body = Server sedang menangani banyak permintaan. Tunggu sebentar lalu coba lagi.
error-4xx-title = Permintaan tidak bisa diproses
error-4xx-body = Permintaan kamu tidak bisa diproses. Silakan coba lagi.

# Kode error validasi field (dikirim kode Rust sebagai `code`, argumen sebagai variabel).
name-required = Nama wajib diisi.
name-too-long = Nama maksimal { $max } karakter.
name-invisible-chars = Nama mengandung karakter tak terlihat.
email-invalid = Alamat email tidak valid.
email-taken = Email ini sudah terdaftar.
password-too-short = Password minimal { $min } karakter.
password-too-long = Password maksimal { $max } karakter.
password-current-wrong = Password saat ini salah.
