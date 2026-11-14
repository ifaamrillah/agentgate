# Konfigurasi (environment variable)

Semua secret dibaca dari env; staging/production **tidak punya default** untuk secret dan gagal boot
bila kosong. Contoh lengkap: `deploy/.env.example`.

| Variabel | Wajib | Isi |
|---|---|---|
| `APP_URL` | ya | URL publik https, tanpa `/` di akhir (link email, callback OAuth, perintah instal CLI) |
| `PREVIEW_URL` | ya | URL domain pratinjau mockup; host harus berbeda dari `APP_URL` |
| `DATABASE_URL` | ya | diisi compose dari `POSTGRES_PASSWORD` |
| `SESSION_SECRET` | ya | `openssl rand -base64 64`; mengganti = semua user logout |
| `ENCRYPTION_KEY` | ya | `openssl rand -base64 32`; mengganti = 2FA aktif tidak bisa dibuka |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_SECURE`, `SMTP_USER`, `SMTP_PASSWORD`, `MAIL_FROM` | ya | email verifikasi, reset, undangan, notifikasi |
| `S3_BUCKET`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY` | ya | storage file aplikasi; `S3_ENDPOINT` kosong = AWS, `S3_REGION` default `us-east-1` |
| `GOOGLE_CLIENT_ID`/`_SECRET`, `GITHUB_CLIENT_ID`/`_SECRET` | tidak | login OAuth; callback `{APP_URL}/auth/google/callback` dan `/auth/github/callback` |
| `SYSTEM_ADMIN_EMAILS` | tidak | email (koma) admin instance: halaman `/system` |
| `METRICS_TOKEN` | tidak | bearer untuk `/_metrics`; kosong = 404 |
| `CLI_RELEASE_URL` | tidak | basis unduhan rilis CLI (`…/releases/latest/download`); kosong = `/install.sh` 404 |
| `CLI_MIN_VERSION` | tidak | versi CLI minimum yang ditampilkan di `/install` |
| `BACKUP_*` | ya (backup) | tujuan backup di luar VPS, jam, retensi; lihat backup.md |
| `DB_MAX_CONNECTIONS`, `DB_MIN_CONNECTIONS`, `PG_MAX_CONNECTIONS` | tidak | lihat aritmetika pool di `deploy/README.md` |
| `LOG_LEVEL` | tidak | `info` (default), `debug` hanya sementara |
| `AUTH_MAX_CONCURRENT_HASHES` | tidak | hash argon2 bersamaan (default 4 ≈ 76 MB puncak) |

Integrasi per workspace (GitHub token/App, Jira, webhook, token API) diatur di aplikasi, bukan env;
secret-nya dienkripsi dengan `ENCRYPTION_KEY`.
