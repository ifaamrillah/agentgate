# Agent Gate

Manajemen proyek ala Jira yang sekaligus menjadi MCP server dan orkestrator AI agent untuk seluruh
siklus pengembangan produk. Spesifikasi produk: [`docs/spec.md`](docs/spec.md). Konvensi kode:
[`docs/reference/loco-project-rules.md`](docs/reference/loco-project-rules.md). Status pengerjaan:
[`docs/PROGRESS.md`](docs/PROGRESS.md).

## Prasyarat

Cukup **Docker** (dengan Compose v2), `git`, `bash`, `curl`, `perl`, dan `python3` (untuk tes skrip `bin/`) di host. Rust, Tailwind, `sea-orm-cli`,
dan tool lain berjalan di container `app`; skrip `bin/*` meneruskan perintah ke sana.

## Perintah sehari-hari

| Perintah | Fungsi |
|---|---|
| `bin/dev` | Server development di <http://localhost:5150> (Tailwind `--watch` + restart otomatis) |
| `bin/test [filter]` | Semua tes workspace (database test, satu thread) |
| `bin/loco db migrate` | Migrasi database dev |
| `bin/loco <perintah>` | Perintah Loco lain (`routes`, `doctor`, `g migration ...`) |
| `bin/ci` | Pemeriksaan lengkap sebelum merge (fmt, clippy, deny, arah layer, CSS, tes) |
| `bin/sh [perintah]` | Shell atau satu perintah di container `app` |
| `bin/worktree new <branch>` | Mulai kerja paralel di worktree sendiri (lihat di bawah) |

Pertama kali, `bin/*` membangun image `agentgate-rs-dev` dan menyalakan layanan bersama
(project Compose `agentgate-rs-shared`):

| Layanan | Alamat dari host | Keterangan |
|---|---|---|
| Postgres 18 + pgvector | `127.0.0.1:54329` (user/password `agentgate`) | database `agentgate_dev`, `agentgate_test` |
| SeaweedFS (S3) | <http://127.0.0.1:8333> (key `agentgate` / `agentgate-dev-secret`) | storage file |
| Mailpit | <http://localhost:8025> (SMTP 1025) | menampung semua email dev |

Katalog komponen UI ada di <http://localhost:5150/_dev/ui> (hanya development).

## Data seed dan login development

`bin/loco db seed` mengisi database dev dengan lima user (terverifikasi, **tanpa password**):
`owner@agentgate.test`, `admin@agentgate.test`, `dev@agentgate.test`, `qa@agentgate.test`, dan
`viewer@agentgate.test`, semuanya anggota workspace **PT Contoh** (`/w/pt-contoh`) dengan role
Owner, Admin, Member, Member, dan Viewer (jabatan: owner PM, dev Dev, qa QA). Workspace itu punya
project **KSR** "Kasir Online" (`/w/pt-contoh/p/KSR`) dengan akses: tim Backend (dev) dan tim QA
(qa) ber-role Member, viewer langsung ber-role Viewer; owner dan admin otomatis Admin di semua
project. Menjalankannya lagi
tidak menggandakan data; untuk mengulang dari nol pakai
`bin/loco db reset` lalu seed.

- **Masuk tanpa password**: `bin/loco task dev_login email:owner@agentgate.test` mencetak link
  `http://localhost:<PORT>/_dev/login?token=...` yang berlaku 10 menit. Buka di browser. Link ini
  setara password: jangan dibagikan atau di-commit. Hanya jalan di environment development.
- **Mencoba form login**: beri password ke user seed dengan
  `bin/loco task user:set_password email:dev@agentgate.test` (password dibaca dari prompt tanpa echo,
  dua kali, minimal 12 karakter; semua sesi user itu dicabut).
- **User baru**: `bin/loco task user:create email:ana@contoh.test name:"Ana"` (terverifikasi, tanpa password).
- **Email** (verifikasi, reset password, undangan) dikirim oleh worker antrean yang ikut jalan di
  `bin/dev`; buka hasilnya di Mailpit.
- **Pembersihan harian** (sesi kedaluwarsa, token lama, undangan selesai, workspace yang lewat masa
  tenggat 30 hari): `bin/loco task cleanup` (dry run) lalu `bin/loco task cleanup confirm:yes`.
  Di staging/production dijadwalkan scheduler pukul 03:00 UTC.

## Environment variable (staging/production)

Development dan test memakai nilai bawaan di `server/config/*.yaml`. Staging dan production wajib
mengisi:

| Variabel | Isi |
|---|---|
| `DATABASE_URL` | URL Postgres |
| `SESSION_SECRET` | kunci tanda tangan cookie sesi, 64+ karakter acak (`openssl rand -base64 64`); mengganti me-logout semua user |
| `ENCRYPTION_KEY` | base64 dari tepat 32 byte (`openssl rand -base64 32`) untuk enkripsi secret TOTP; mengganti membuat 2FA aktif tak bisa dibuka |
| `S3_BUCKET`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY` | storage file (lampiran, logo, evidence, ekspor, impor) di S3-compatible; `S3_ENDPOINT` (kosong = AWS) dan `S3_REGION` (default `us-east-1`) opsional |
| `APP_URL` | URL publik, dipakai untuk link di email dan callback OAuth |
| `PREVIEW_URL` | URL domain pratinjau mockup HTML (M10), mis. `https://preview.example.com`; host wajib berbeda dari `APP_URL` dan diarahkan ke server yang sama |
| `MAIL_FROM`, `SMTP_HOST`, `SMTP_PORT`, `SMTP_SECURE`, `SMTP_USER`, `SMTP_PASSWORD` | pengiriman email |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` | login Google/GitHub; provider yang `CLIENT_ID`-nya kosong tidak diaktifkan |
| `PORT`, `BINDING`, `LOG_LEVEL`, `DB_*`, `AUTH_MAX_CONCURRENT_HASHES` | opsional, ada default |
| `SYSTEM_ADMIN_EMAILS`, `METRICS_TOKEN`, `CLI_RELEASE_URL`, `CLI_MIN_VERSION`, `BACKUP_TARGET`, `BACKUP_SCHEDULE_LABEL` | operasi instance (M12), opsional; kosong = fiturnya mati |

Deploy satu VPS dengan Docker Compose: `deploy/` dan `docs/admin/deploy.md`. Panduan admin lain
(konfigurasi, operasi & alert, backup/pulihkan, upgrade, keamanan) di `docs/admin/`; panduan
pemakai di `docs/panduan/`.

## Kerja paralel dengan worktree

```bash
bin/worktree new feature/nama-fitur      # folder, database, container, dan port sendiri
bin/worktree list
bin/worktree close feature/nama-fitur    # setelah di-merge ke staging
```

Checkout utama selalu di `staging`. Detail aturan: §11 dokumen konvensi.

## Editor

`.devcontainer/devcontainer.json` menempelkan VS Code/Cursor ke container `app` yang sama dengan
`bin/*`, sehingga rust-analyzer berjalan di dalam container.

## Struktur

| Folder | Isi |
|---|---|
| `server/` | Aplikasi Loco (`agentgate-server`): web app, API, gate engine, MCP, worker |
| `cli/` | Binary `agentgate`: installer aturan, indexer, runner |
| `core/` | Type bersama, parser workflow, protokol runner |
| `graph/` | Parser tree-sitter dan pembangun code graph |
| `docker/` | Image dev dan file Compose |
| `bin/` | Skrip resmi development |
| `docs/` | Spesifikasi, rencana per milestone, hasil spike |
