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
| `SENTRY_DSN` | tidak | error tracking Sentry server + browser; kosong = mati. Lihat operasi.md |
| `SENTRY_ENVIRONMENT` | tidak | environment Sentry (default environment Loco, mis. `production`) |
| `CLI_RELEASE_URL` | tidak | basis unduhan rilis CLI (`…/releases/latest/download`); kosong = `/install.sh` 404 |
| `CLI_MIN_VERSION` | tidak | versi CLI minimum yang ditampilkan di `/install` |
| `BACKUP_*` | ya (backup) | tujuan backup di luar VPS, jam, retensi; lihat backup.md |
| `DB_MAX_CONNECTIONS`, `DB_MIN_CONNECTIONS`, `PG_MAX_CONNECTIONS` | tidak | lihat aritmetika pool di `deploy/README.md` |
| `LOG_LEVEL` | tidak | `info` (default), `debug` hanya sementara |
| `AUTH_MAX_CONCURRENT_HASHES` | tidak | hash argon2 bersamaan (default 4 ≈ 76 MB puncak) |
| `GITHUB_APP_ID`, `GITHUB_APP_SLUG`, `GITHUB_APP_CLIENT_ID`, `GITHUB_APP_CLIENT_SECRET`, `GITHUB_APP_PRIVATE_KEY`, `GITHUB_APP_WEBHOOK_SECRET` | tidak | GitHub App instance (cara utama menautkan GitHub); semua kosong = hanya PAT. Lihat github-app.md |
| `AGENTGATE_CACHE_DIR` | tidak | volume cache clone repo + model embedding (default `/var/cache/agentgate`) |
| `GRAPH_CLONE_HISTORY_MONTHS`, `GRAPH_CLONE_MAX_MB` | tidak | riwayat clone dangkal code graph (default 12 bulan) dan batas ukuran per repo (default 1024 MB) |
| `GRAPH_EMBEDDING_ENABLED` | tidak | `false` mematikan pencarian semantik (default `true`) |

Integrasi per workspace (instalasi GitHub App atau token, Jira, webhook, token API) diatur di aplikasi, bukan env;
secret-nya dienkripsi dengan `ENCRYPTION_KEY`.
