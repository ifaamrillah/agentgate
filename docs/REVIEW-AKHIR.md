# Review akhir Agent Gate (M0–M12)

Ringkasan untuk pemilik produk: apa yang sudah dibangun, cara mencobanya, semua keputusan sementara
yang menunggu persetujuan, apa yang dibutuhkan dari Anda, dan utang teknis yang tersisa.
Detail per milestone: `docs/PROGRESS.md`. Spec: `docs/spec.md`.

## 1. Yang sudah dibangun

| Milestone | Isi utama |
|---|---|
| M0 Fondasi | Workspace Cargo (server, cli, core, graph), dev full Docker (`bin/*`), CI, 3 spike (WYSIWYG, hook client, terminal per OS) |
| M1 Identitas | Daftar/masuk, verifikasi email, reset, 2FA TOTP, login Google/GitHub, workspace + role + undangan + tim + project, audit log, hapus workspace (tenggang 30 hari) |
| UI "Tenang" | Sistem desain gaya Linear, mode gelap, pustaka komponen Tera dipakai semua halaman |
| M2 Tiket & board | Tipe + field custom, kode tiket (alias), board/daftar, detail, komentar @mention, kotak masuk, lampiran, relasi, sampah, pencarian |
| M3 Workflow & gate | Builder workflow + YAML, 9 template, versi + migrasi tiket, gate, approve/reject, "Butuh keputusan", lompat/paksa Admin, artifact + diff |
| M4 MCP & persona | OAuth 2.1 (DCR, PKCE) untuk `/mcp`, 7 tool MCP + slash command per stage, persona, skill pack (scan + review manusia), draft tiket |
| M5 CLI & runner | `agentgate` login device flow, `init`/`uninstall`, runner (WebSocket, worktree per job, headless/terminal), evidence bertanda tangan Ed25519, izin aksi berbahaya |
| M6 Git & PR | GitHub/GHES (PAT terenkripsi), webhook PR/CI, gate `pr_open`/`prs_merged`/`ci_green`, merge otomatis, konvensi git, sinkron aturan lewat PR |
| M7 Code graph | tree-sitter 10 bahasa, call/import/route, dampak diff, SCIP, riwayat git, relasi lintas repo, tool MCP `ag_search`/`ag_trace`/…; recall 100% di benchmark |
| M8 Knowledge | Dokumen project/workspace berversi, usulan AI + tinjau side-by-side, pemilik knowledge, `agentgate knowledge pull|sync` |
| M9 Planning | Backlog/sprint, estimasi AI + kalibrasi, burndown/burnup/velocity/CFD, roadmap, kalender, rilis + release notes, JQL + saved filter + edit massal |
| M10 Visual | AI Office isometrik live, graph viewer berlapis, pratinjau mockup di domain sandbox, Penghemat token |
| M11 Integrasi | 6 client AI (Claude Code, Codex, Gemini, Cursor, OpenCode, Copilot), API REST `/api/v1` + webhook keluar, ekspor ZIP, impor CSV, connector Jira |
| M12 Rilis | Realtime runner `LISTEN/NOTIFY` (diuji 10.000 listener), file di S3 + dihapus saat purge, hardening (rate limit DCR, kode OAuth hangus, Permissions-Policy, SSRF GHES), sapuan otorisasi semua route, `/_metrics` + `/_readiness`, halaman **Status sistem**, deploy Compose satu VPS + backup ke luar VPS, rilis CLI `dist` + `/install` + `self-update`, audit a11y + visual, panduan admin |

Tes: lihat angka terakhir di `docs/PROGRESS.md` (baris M12). Benchmark: `docs/benchmarks/`.

## 2. Cara menjalankan dan mencoba

```sh
bin/dev                     # server + worker di http://localhost:5150 (Mailpit http://localhost:8025)
bin/loco db seed            # data contoh (sekali)
bin/loco task dev_login email:owner@agentgate.test   # cetak link masuk tanpa password (10 menit)
bin/test                    # semua tes;  bin/ci = fmt, clippy, layer, deny, tes
```

Akun seed (tanpa password, workspace **PT Contoh** `/w/pt-contoh`, project **KSR**):

| Email | Role |
|---|---|
| `owner@agentgate.test` | Owner (juga admin instance di dev → `/system`) |
| `admin@agentgate.test` | Admin |
| `dev@agentgate.test` | Member (Dev) |
| `qa@agentgate.test` | Member (QA) |
| `viewer@agentgate.test` | Viewer |

Beri password: `bin/loco task user:set_password email:dev@agentgate.test`.

Yang layak dicoba: board `/w/pt-contoh/p/KSR/board`, tiket KSR-11 (review + mockup), KSR-13 (job + evidence),
AI Office `/w/pt-contoh/office`, code graph `/w/pt-contoh/p/KSR/graph`, knowledge, laporan, Status sistem
`/system`, halaman instal CLI `/install`. CLI: `cargo run -p agentgate-cli -- login --server http://localhost:5150`.
MCP: `claude mcp add --transport http agentgate http://localhost:5150/mcp`.

Production: `deploy/README.md` dan `docs/admin/` (deploy, konfigurasi, operasi & alert, backup/pulihkan, upgrade,
keamanan). Panduan pemakai: `docs/panduan/mulai.md`.

## 3. Keputusan sementara (perlu disetujui atau diubah)

Format: **keputusan** — default sekarang.

### Akun, akses, keamanan

- Sapaan UI — "Anda".
- Login sosial — sesi mengikuti batas aplikasi (idle 1 hari, mutlak 30 hari), bukan sesi provider.
- Logo SVG — ditolak sampai ada sanitasi SVG.
- Tim/Jabatan/Role — daftar sederhana tanpa bulk action.
- Wajib 2FA dan hapus workspace — hanya Owner bawaan.
- Admin instance (`/system`) — dari env `SYSTEM_ADMIN_EMAILS`, bukan Owner workspace (data lintas tenant). (M12)
- `/_metrics` — butuh token bearer `METRICS_TOKEN`, 404 tanpa token, diblok Caddy dari internet. (M12)
- Pendaftaran client OAuth dinamis (DCR) — terbuka untuk siapa pun, dibatasi 20 per IP per jam; kode otorisasi
  hangus setelah 1 percobaan gagal. (M12)
- Redirect URI OAuth — https, loopback port bebas, atau skema privat.
- URL GitHub Enterprise — harus https ke alamat publik (alamat privat/loopback ditolak, kecuali dev/tes). GHES di
  jaringan internal belum didukung. (M12)
- Izin knowledge — ikut `manage_ticket_settings` (tanpa `manage_knowledge`).
- Integrasi (Git, API, Jira, impor) — izin `manage_integrations` (Owner/Admin bawaan); ekspor hanya Owner/Admin bawaan.

### Tiket, workflow, planning

- Detail tiket — halaman sendiri, bukan panel samping board.
- Tombol Approve — varian primer (aksen), bukan hijau.
- Builder workflow — daftar vertikal + panel, bukan kanvas graf.
- Workflow Subtask — "Subtask ringkas" (Dev → Review → Selesai).
- Gate evidence — hanya lewat runner/git atau lompat/paksa Admin (tidak bisa dicentang manual).
- Estimasi AI — lewat komentar `@agent` atau MCP; kalibrasi = rasio aktual/estimasi per stage.
- Rilis — tidak membuat git tag; satu component per tiket.
- Roadmap — dikelompokkan per sprint (bukan per epic); board Scrum default sprint aktif.
- Batas WIP — hanya penanda (tidak menolak pindah kartu). JQL hanya `AND`.

### Agent, MCP, runner, git

- Persona di DB per workspace; skill pack wajib disetujui manusia.
- Server MCP stateless (JSON, tanpa sesi SSE); slash command = prompt MCP `ag-<stage>`.
- Login CLI — device flow, kredensial di file 0600 (bukan keychain OS).
- Evidence — Ed25519 per runner; job sukses walau tes gagal, gate yang memutuskan.
- Runner — tidak pernah auto-ambil kecuali diaktifkan; halaman Runner untuk semua anggota.
- Runner online — 60 detik (spec menyebut 2 menit).
- Git — personal access token (bukan GitHub App), satu koneksi per jenis provider per workspace.
- Konvensi git per project (bukan per stage); PR ditautkan lewat kode tiket di branch/judul.
- Merge otomatis — squash saat stage reviewer di-approve dan CI hijau.
- Index code graph server — arsip tarball (tanpa riwayat git); SCIP hanya dari laptop; semantic search ditunda.
- Realtime runner — satu koneksi `LISTEN` per proses web, kanal per workspace, tick cadangan 30 dtk. Browser
  (board, AI Office, log job) masih polling HTMX 2–3 dtk. (M12)
- `agentgate init` menulis config untuk 6 client sekaligus.

### Visual, integrasi

- AI Office — Canvas 2D vanilla (bukan PixiJS), maks. 8 ruangan, antrean job tidak tampil.
- Pratinjau mockup — domain terpisah `PREVIEW_URL`, token HMAC 1 jam.
- Biaya token — USD (belum Rupiah); "token ≈ byte/4" untuk estimasi.
- API publik — token bertindak sebagai pembuatnya + scope; rate limit 600/menit.
- Webhook keluar — retry 5×, nonaktif otomatis setelah 5 gagal beruntun.
- Jira — satu koneksi per workspace, dua arah lewat polling JQL 5 menit, gate tetap ditegakkan.
- Notifikasi — hanya in-app (email/Slack di luar cakupan).

### Operasi & rilis (M12)

- Deploy — satu VPS Docker Compose: migrate (sekali), web, worker+scheduler (satu proses), Postgres, SeaweedFS,
  Caddy (TLS otomatis), backup.
- Backup — harian 02.00 WIB, `pg_dump` + salinan bucket file ke S3 **di luar VPS**, retensi 14 hari, riwayat di
  Status sistem; dianggap basi > 26 jam.
- `statement_timeout` 30 dtk per role DB (migrasi dan restore tanpa batas).
- Rilis CLI — `dist` lewat GitHub Actions saat tag `v*`: 5 target (macOS arm/x86, Linux x86/arm, Windows), installer
  shell + PowerShell, sha256, atestasi GitHub; formula Homebrew dibuat tapi belum dipublikasikan ke tap.
- `/install.sh` dan `/install.ps1` di server mengalihkan ke rilis (`CLI_RELEASE_URL`); `agentgate self-update`
  menjalankan ulang installer lewat server.
- Error tracking (Sentry) — belum dipasang; butuh DSN.
- Token kontras: warna teks "faint" dan tombol primer/bahaya mode gelap digelapkan agar lolos WCAG AA.

## 4. Perlu dari Anda

Paling penting dulu:

1. **Remote GitHub** untuk repo ini. CI, rilis CLI, dan atestasi baru jalan setelah di-push.
2. **Domain + `APP_URL` https** (issuer OAuth MCP, link email, webhook GitHub) dan **`PREVIEW_URL`** di domain lain.
3. **VPS** (2 vCPU/4 GB minimal) dan **bucket S3 di luar VPS** untuk backup (B2/Wasabi/S3).
4. **SMTP** (`SMTP_*`, `MAIL_FROM`).
5. **OAuth app Google dan GitHub** (callback `{APP_URL}/auth/google/callback`, `/auth/github/callback`).
6. **Token GitHub** (PAT fine-grained: contents, pull requests, checks/statuses, webhooks) atau keputusan membuat
   **GitHub App**. Semua tes memakai GitHub palsu.
7. **Distribusi CLI**: konfirmasi org/repo rilis → isi `CLI_RELEASE_URL`; repo tap Homebrew + token bila ingin
   `brew install`; keputusan code signing macOS/Windows (sekarang tidak).
8. **Uji dengan client AI asli** (Claude Code sudah sesuai spike; Codex, Gemini, Cursor, OpenCode, Copilot baru
   diuji dengan binary palsu). Putuskan apakah Gemini CLI tetap didukung (digantikan Antigravity CLI).
9. **Kredensial Jira** asli (Cloud: site + email + API token; atau DC + PAT) dengan project uji.
10. **Kurs USD → Rupiah** dan tabel harga model bila biaya token perlu tampil dalam Rupiah / untuk Codex/Gemini/Cursor.
11. `SYSTEM_ADMIN_EMAILS`, `METRICS_TOKEN`, dan Prometheus/Grafana bila ingin alert (contoh aturan di
    `docs/admin/operasi.md`); DSN Sentry bila ingin error tracking.
12. Konfirmasi kebijakan: merge otomatis, retry webhook, rate limit API 600/mnt, DCR 20/IP/jam, auto-ambil runner
    default mati, batas job paralel default, online runner 60 dtk.
13. Konfirmasi fitur: JQL `OR`/kurung, WIP ditegakkan, git tag saat rilis, roadmap per epic, izin `manage_knowledge`,
    server boleh commit/PR knowledge sendiri, GitLab/Bitbucket sebelum rilis, index server boleh `git clone`.
14. Lain-lain: GHES untuk uji nyata (bila ada pelanggan), Graphify + repo pelanggan untuk benchmark code graph,
    pilihan model embedding untuk semantic search.

## 5. Utang teknis tersisa (urut risiko)

**Tinggi**

- Keamanan akun: event keamanan (2FA, password, OAuth) belum diaudit per user; aktifkan 2FA/buat ulang kode pemulihan
  belum minta autentikasi ulang dan belum mencabut sesi lain; reset password belum menghapus TOTP.
- Adapter 5 client AI selain Claude Code belum diuji dengan client asli.
- Restore backup belum diuji di stack production sungguhan (sudah diuji ke database smoke lokal).
- GHES: cek DNS dilakukan saat connect; panggilan berikutnya belum dijaga dari DNS rebinding.
- Webhook GitHub masuk diproses sinkron (belum antrean + retry); merge otomatis tidak dicoba ulang bila gagal.

**Sedang**

- Browser masih polling (board, AI Office 3 dtk, log job 2 dtk); SSE di atas hub `LISTEN/NOTIFY` adalah langkah
  berikutnya. Runner tidak mencoba WebSocket lagi setelah jatuh ke polling.
- Mengganti template kode tiket dan migrasi workflow berjalan dalam satu transaksi (lambat untuk ribuan tiket).
- Status CI dihitung dari check terakhir, belum dari check wajib branch protection.
- Token GitHub yang dicabut baru ketahuan saat panggilan gagal; belum GitHub App.
- Index code graph server: repo sangat besar bisa timeout unduh arsip.
- Daftar `mcp_tools` persona belum ditegakkan per sesi MCP.
- Metrik per proses (tiap replika web di-scrape sendiri); backup basi belum jadi metrik Prometheus.
- Sisa temuan a11y: daftar di editor Milkdown (pihak ketiga), runner offline di AI Office (sengaja redup).

**Rendah**

- Komentar belum punya balasan/reaksi; field custom belum multi-pilih; label tanpa warna.
- Teks tooltip grafik/roadmap dan error JQL masih di Rust (belum Fluent).
- Tabel datatable di 390px digulir horizontal.
- Antrean job dan daftar token/webhook disortir di memori (cukup sampai ratusan/ribuan baris).
- Event knowledge dan planning belum masuk audit log.
- Builder workflow belum kanvas graf; AI Office tanpa animasi pulang; graph viewer layout sederhana.
- Kredensial CLI di file 0600, belum keychain OS.
