# Progres Pengerjaan Agent Gate

Sumber kebenaran status milestone. Diperbarui setiap milestone selesai atau ada keputusan baru.

## Status milestone

| Milestone | Status | Catatan |
|---|---|---|
| M0 Fondasi | selesai (2026-11-11) | `bin/ci` hijau, `/_health` jalan, 3 spike tertulis |
| M1 Identitas & tenancy | selesai (2026-11-11) | Akun, 2FA, OAuth (fake di test), workspace/role/undangan/tim/project/repo, audit log, hapus workspace |
| M2 Tiket & board | belum mulai | |
| M3 Workflow & gate | belum mulai | |
| M4 MCP & persona | belum mulai | |
| M5 CLI & runner | belum mulai | |
| M6 Git & PR | belum mulai | |
| M7 Code graph | belum mulai | |
| M8 Knowledge base | belum mulai | |
| M9 Planning & laporan | belum mulai | |
| M10 Visual | belum mulai | |
| M11 Multi-client & integrasi | belum mulai | |
| M12 Rilis | belum mulai | |

## Keputusan teknis (di luar spec)

| Tanggal | Keputusan | Alasan |
|---|---|---|
| 2026-11-11 | Storage S3 dev/self-host memakai **SeaweedFS 4.53**, bukan MinIO (spec diperbarui) | Image komunitas MinIO sudah tidak dirilis. Dipilih user. |
| 2026-11-11 | Nama Compose/volume berawalan `agentgate-rs-*` | Docker host berisi volume `agentgate_*` dari percobaan lama yang tidak boleh tersentuh |
| 2026-11-11 | Postgres dev diekspos di `127.0.0.1:54329` | Hindari bentrok dengan Postgres lain di host |
| 2026-11-11 | Error respons di-render sebagai halaman HTML lengkap lewat layer `html_errors`; fallback Loco dimatikan | Error Loco berupa JSON/halaman Inggris dan ikut di-swap HTMX |
| 2026-11-11 | `cargo deny`: lisensi yang diizinkan MIT, Apache-2.0 (+LLVM-exception), BSD-2/3, ISC, Unicode-3.0, Zlib, MPL-2.0, CC0-1.0, CDLA-Permissive-2.0, BSL-1.0, 0BSD; abaikan RUSTSEC-2024-0449 (unsafe-libyaml unmaintained, transitif lewat parser config Loco) | Tidak ada alternatif di Loco 1.3; bukan celah keamanan |
| 2026-11-11 | Tanpa remote `origin`, `bin/worktree` membuat branch dari `staging` lokal dan menganggap branch "selesai" bila sudah di-merge ke `staging` lokal | Repo belum punya remote GitHub |
| 2026-11-11 | Gaya visual Basecoat: **Vega** (default, `basecoat.css`), ditimpa token sistem desain di bawah | Dipilih user dari perbandingan 8 gaya |
| 2026-11-11 | **Sistem desain**: sidebar indigo gelap, aksen indigo, netral slate, IBM Plex Sans + Plex Mono (di-vendor), gaya bernuansa warna, radius 6/8px, badge pill, avatar kotak. Acuan: `docs/reference/design-system.md` | Dipilih user lewat konfigurator 15 bagian |
| 2026-11-11 | Editor WYSIWYG: **Milkdown Crepe 7.23.0** (bundle ESM dari esm.sh, `deps=codemirror@6.0.2`), tanpa import map | Hasil spike; lihat `docs/spikes/2026-11-11-wysiwyg-tanpa-bundler.md` |
| 2026-11-11 | URL tenant berbasis path: `/w/{slug}`, project `/w/{slug}/p/{KEY}`; non-anggota selalu 404 | Satu domain, tanpa wildcard DNS |
| 2026-11-11 | Role = himpunan permission (`text[]` + CHECK). Role bawaan Owner/Admin/Member/Viewer tidak bisa diubah; role custom tidak bisa memberi permission yang tidak dimiliki pembuatnya | Fleksibel tanpa celah eskalasi |
| 2026-11-11 | Hierarki permission project: Administer ⇒ Edit ⇒ View; akses efektif = gabungan grant langsung + grant tim; Owner/Admin workspace otomatis Admin di semua project | Spec tidak merinci; mengikuti pola Jira |
| 2026-11-11 | OAuth Google/GitHub memakai komponen Loco tetapi controller sendiri (tanpa `oauth::routes`); mulai lewat POST, cookie state terenkripsi `__Host-agentgate_oauth_{p}` | Kontrol CSRF/state dan pesan error berbahasa Indonesia |
| 2026-11-11 | Callback OAuth (GET) menulis DB: pengecualian bernama dari aturan "GET tidak menulis" | Protokol OAuth mewajibkan GET |
| 2026-11-11 | Menautkan identitas OAuth ditolak bila user sudah punya identitas provider yang sama dengan uid berbeda (`oauth-identity-conflict`) | Cegah pengambilalihan akun lewat email yang dipindah |
| 2026-11-11 | Aturan "sesi lokal ≤ sesi provider" tidak berlaku untuk login sosial; sesi mengikuti batas idle + absolut aplikasi | Google/GitHub userinfo tidak memberi sesi provider; ditinjau lagi saat SSO organisasi |
| 2026-11-11 | Sesi server-side (store database). Login terkunci → 429 `auth-too-many-attempts` | Pesan "terlalu banyak percobaan" yang jelas tanpa membocorkan apa pun |
| 2026-11-11 | Secret TOTP dienkripsi XChaCha20-Poly1305 dengan `ENCRYPTION_KEY`; kode pemulihan ditampilkan dengan 200 + `no-store` (bukan PRG) | Kode mentah tidak pernah masuk sesi |
| 2026-11-11 | Daftar lewat undangan tetap boleh saat pendaftaran umum ditutup | Undangan = izin masuk eksplisit |
| 2026-11-11 | Wajib 2FA dan hapus workspace hanya untuk Owner bawaan; hapus perlu ketik slug, tenggat 30 hari, purge oleh task `cleanup` 03:00 UTC (10:00 WIB) | Aksi paling berisiko tetap di tangan pemilik |
| 2026-11-11 | Audit log ditulis di transaksi yang sama dengan perubahannya; kolom `changes` jsonb bebas bentuk | Payload berbeda per aksi |
| 2026-11-11 | `bin/dev` menjalankan server + worker antrean (`--server-and-worker`); scheduler tidak jalan di dev | Email dev harus sampai ke Mailpit |

## Temuan spike yang mengubah rencana

- **Claude Code** (`docs/spikes/2026-11-11-client-hooks-headless.md`): `--strict-mcp-config` juga mengabaikan
  `.mcp.json` project, jadi runner wajib memberi `--mcp-config .mcp.json` eksplisit. `HOME`/`CLAUDE_CONFIG_DIR`
  tidak boleh diganti (kredensial langganan ada di sana).
- **Gemini CLI**: setting yang benar sekarang `context.fileName` (bukan `contextFileName`). Gemini CLI
  untuk pengguna gratis/Google One sudah digantikan "Antigravity CLI" (2026-06-18) — perlu dicek ulang di M11.
- **Folder trust**: Codex, Gemini, Copilot, Cursor mengabaikan config project di folder yang belum dipercaya;
  setiap adapter butuh langkah trust sendiri (M5/M11).
- **RTK**: `agentgate hook` memanggil `rtk rewrite` sendiri, bukan `rtk init -g` (yang menulis config global user).
- **Terminal per OS** (`docs/spikes/2026-11-11-terminal-per-os.md`): terminal hanya menjalankan launcher berisi
  `agentgate job attach <id>`; prompt tiket tidak pernah melewati shell. Launcher disimpan di state dir runner.

## Utang teknis yang harus ditangani di milestone tertentu

- **M4/M5**: layer `html_errors` mengubah *semua* respons error non-HTML menjadi halaman HTML. Sebelum endpoint
  JSON (MCP, API runner, API publik) ada, batasi layer ini ke request browser (`Accept: text/html`) atau
  kecualikan prefix API.
- Daftar pengecualian CSRF ada di `server/src/controllers/middleware/csrf.rs` (`EXEMPT_PATHS`), bukan di `app.rs`
  seperti di rencana M0 — satu tempat yang sama dengan aturannya.

- **Keamanan akun (setelah M1)**: event keamanan akun (aktif/nonaktif 2FA, ganti/reset password, tautkan
  OAuth) belum diaudit karena audit log per workspace; perlu log keamanan per user. Mengaktifkan 2FA dan
  membuat ulang kode pemulihan belum meminta autentikasi ulang dan belum mencabut sesi lain; reset password
  belum menghapus TOTP. Undangan (kirim/ulang/cabut) masih memakai role pelaku dari awal request.
- **Deploy**: lockout login per IP memakai IP klien dari proxy; pastikan reverse proxy meneruskan IP asli,
  kalau tidak satu IP proxy bisa mengunci login semua orang.
- Role custom tidak pernah menjadi "admin di semua project": hanya Owner/Admin bawaan yang boleh menugaskan
  role Admin bawaan (termasuk di grant project) dan mengubah izin role yang dipakai grant project.

## Perlu dari user (dikumpulkan untuk akhir)

- Remote GitHub untuk repo ini (CI GitHub Actions baru jalan setelah di-push).
- OAuth app **Google** dan **GitHub** (client ID/secret, callback `{APP_URL}/auth/google/callback` dan
  `{APP_URL}/auth/github/callback`). Selama M1 login sosial hanya diuji dengan provider palsu.
- Server SMTP untuk staging/production (`MAIL_FROM`, `SMTP_*`).

## Catatan untuk sesi berikutnya

- Migration M1 diedit di tempat selama branch belum di-merge. Database dev yang dibuat sebelum M1 di-merge
  perlu `bin/loco db reset` lalu `bin/loco db seed`.

- Rencana milestone: `docs/superpowers/plans/`. Sumber dependency Loco/Tera untuk referensi API ada di `tmp/src/`
  (gitignored; salin ulang dari `/usr/local/cargo/registry/src` di container bila hilang).
- Screenshot UI memakai Playwright dari npm di scratchpad (Playwright MCP tidak bisa start: Chrome tidak terpasang).
