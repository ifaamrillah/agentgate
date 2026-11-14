# Progres Pengerjaan Agent Gate

Sumber kebenaran status milestone. Diperbarui setiap milestone selesai atau ada keputusan baru.

## Status milestone

| Milestone | Status | Catatan |
|---|---|---|
| M0 Fondasi | selesai (2026-11-11) | `bin/ci` hijau, `/_health` jalan, 3 spike tertulis |
| M1 Identitas & tenancy | selesai (2026-11-11) | Akun, 2FA, OAuth (fake di test), workspace/role/undangan/tim/project/repo, audit log, hapus workspace |
| UI "Tenang" | selesai (2026-11-12) | Desain prototype diterapkan ke semua halaman M0–M1; pustaka komponen untuk M2–M12 (`docs/reference/design-system.md`) |
| M2 Tiket & board | selesai (2026-11-12) | Tipe + field custom, template kode (alias), board/daftar, detail, komentar + @mention, kotak masuk, lampiran S3, relasi (blocks ditegakkan), riwayat, sampah 30 hari, pencarian; 396 tes |
| M3 Workflow & gate | selesai (2026-11-12) | Builder workflow + YAML impor/ekspor, 9 template, versi + migrasi tiket, workflow per tipe/project, stage per tiket (stepper, gate, approve/reject, batas gagal → Butuh keputusan, 5 keputusan, lompat/paksa Admin, penugasan), artifact + versi + diff, tab kotak masuk "Butuh keputusan", board per stage; 430 tes |
| M4 MCP & persona | selesai (2026-11-13) | OAuth 2.1 (DCR, PKCE S256, consent pilih workspace, refresh rotasi, revoke) untuk `/mcp`; server MCP rmcp (7 tool, slash command = prompt MCP per stage + utilitas); persona DB (CRUD, warna/ikon, instruksi, skill, tools, model per client); skill pack (impor GitHub + pemindaian statis + review manusia); review draft tiket brainstorming; Akun → Hubungkan client AI; 467 tes |
| M5 CLI & runner | selesai (2026-11-13) | Crate `cli` (`agentgate`): login device flow, `init`/`uninstall`, aturan + hook Claude Code, `jobs`, `runner start` (WS kontrol + fallback polling, worktree per job, headless/terminal, laporan langkah/log), `doctor`; server: job + klaim/lock, API runner (bearer CLI + header runner), evidence bertanda tangan Ed25519 memenuhi gate `tests_passed`/`screenshot_exists`, izin aksi berbahaya; UI Runner & CLI (izin, antrean, runner saya/tim, pengaturan, drift, instal), halaman job + log live, job + evidence di kartu stage, job dari komentar `@agent`; 533 tes |
| M6 Git & PR | selesai (2026-11-13) | Abstraksi provider git + klien GitHub/GHES (PAT terenkripsi, validasi token, daftar repo, webhook dipasang otomatis); Pengaturan → Git provider (Cloud/Enterprise/Repo tertaut); Konvensi git per project (branch, commit, PR, base, hasil kode, CI wajib); runner: worktree dari base branch, branch per tiket, push + PR otomatis; webhook bertanda tangan HMAC melacak PR/merge/CI dan memenuhi gate `pr_open`/`prs_merged`/`ci_green`; merge otomatis (squash) saat stage reviewer di-approve dan CI hijau; daftar PR project, tab "PR & CI" tiket, badge PR di header tiket dan kartu board; sinkron aturan lewat PR (`agentgate sync`); batas job paralel ditegakkan; 558 tes |
| M7 Code graph | selesai (2026-11-13) | Crate `graph`: tree-sitter 10 bahasa, fakta per file, build graph (call/import/inherit/route, keyakinan pasti/dugaan), modul + node pusat, kode mati, kompleksitas, trace/dampak dari diff, kueri Cypher-subset, SCIP (`agentgate graph scip`), riwayat git (hotspot/coupling/pemilik), relasi lintas repo HTTP/event/package; index inkremental dari CLI (`agentgate graph index|scip|status`, file rahasia tidak pernah dibaca) dan dari server (arsip tarball provider, tombol Index ulang, otomatis setelah PR di-merge ke branch utama); MCP `ag_search`/`ag_trace`/`ag_snippet`/`ag_impact`/`ag_graph` + bagian `code_graph` di `ag_context` dan prompt job; UI Cari simbol, detail simbol, Status index (repo, lintas repo, git, kualitas, benchmark); benchmark vs codebase-memory di `docs/benchmarks/2026-11-13-code-graph.md` (recall/presisi 100%/100% vs codebase-memory 89,9%/93,7% pada 149 pasangan pemanggil); 601 tes |
| M8 Knowledge base | selesai (2026-11-13) | Dokumen project per kelompok/jenis (bawaan + custom), versi + diff + pulihkan, editor md per dokumen, cari; usulan AI (scan ulang, minta AI perbarui, tiket Done, graph di-index ulang, audit mingguan, repo) dengan Tinjau usulan side-by-side (A/R/J/K) dan batalkan; pemilik knowledge + notifikasi kotak masuk; dokumen workspace (berlaku per project/persona), jenis custom, update otomatis; MCP `ag_knowledge` + konteks job + knowledge per persona; CLI `agentgate knowledge pull|status|sync`, backup `init` diunggah ke server (panel drift); komponen `diff_sbs` dan `donut`; 626 tes |
| M9 Planning & laporan | selesai (2026-11-13) | Backlog & sprint (rencana/mulai/selesaikan dengan pemindahan sisa, kapasitas dari velocity, seret-lepas, log scope per tiket), story point + estimasi AI per stage (draft → tiket, MCP `ag_planning`, minta estimasi, kalibrasi), laporan burndown/burnup/velocity/sprint report/CFD/kalibrasi/cycle time (SVG server-side, cetak/PDF), Roadmap (Gantt sprint/progres/estimasi/rilis/blocks) dan Kalender (tenggat, perkiraan selesai AI, rilis), Rilis + release notes berversi (susun AI, terbitkan, pulihkan, rilis sekarang), Components (modul code graph, pemilik, penerima default), board Scrum (sprint aktif, swimlane penerima/epic, batas WIP), pencarian lanjutan ala JQL + saved filter (pribadi/tim/workspace) + edit massal; 656 tes |
| M10 Visual | selesai (2026-11-13) | AI Office isometrik A1 live dari job/runner/tiket (kanvas Canvas 2D tanpa PixiJS, denah parametris per project + Ruang rapat/Pantry/Lounge, geser/zoom/pinch/keyboard, mini-map, label, tab ruangan, panel ringkasan/detail dengan log live, approve/reject kembali ke kantor, kustom karakter teraudit); graph viewer berlapis Project → Repo → Modul → File → Fungsi (SVG server-side, sorot tiket dari file berubah job, analisis dampak, filter layer/bahasa, cari); pratinjau mockup HTML di domain sandbox terpisah (token HMAC 1 jam, CSP sandbox, Web/Mobile, versi, bandingkan dengan screenshot QA, approve/revisi); Penghemat token (ringkasan, fitur wajib/opsional dengan toggle teraudit, per tiket, per job; rincian token + biaya + file berubah dari runner, pencatat penghematan di tool MCP); 693 tes |
| M11 Multi-client & integrasi | selesai (2026-11-14) | Aturan repo v2 untuk 6 client (Claude Code, Codex, Gemini CLI, Cursor, OpenCode, Copilot CLI): config MCP + hook tipis per client, `agentgate hook --client`, adapter runner headless/interaktif + parser JSONL per client, `doctor` 6 client, halaman Hubungkan client; API publik REST `/api/v1` (token `ag_pat_` ber-scope, rate limit 600/mnt, OpenAPI) + webhook keluar (outbox transaksional, HMAC, retry 5×, auto-nonaktif, log pengiriman); Ekspor data ZIP (JSON + Markdown/YAML, worker, 7 hari, audit); Impor CSV wizard 4 langkah (pemetaan otomatis Jira/Trello/Linear, alias kode lama, laporan baris dilewati); Connector Jira Cloud/DC (impor sekali, mirror AG→Jira dari outbox, dua arah lewat polling JQL dengan gate ditegakkan, pemetaan status/field/user, log sinkron); race init htmx JS diperbaiki + tes `node --test`; 786 tes Rust + 8 tes JS |
| M12 Rilis | selesai (2026-11-14) | Realtime runner `LISTEN/NOTIFY` (kanal per workspace; uji beban 10.000 listener p99 4,8 ms, `docs/benchmarks/2026-11-14-listen-notify.md`); file staging/prod di S3 + berkas ikut dihapus saat purge workspace; hardening (rate limit DCR per IP, kode OAuth hangus setelah gagal, Permissions-Policy/Referrer-Policy, SSRF URL GHES + redirect tarball); sapuan otorisasi semua route `/w/{slug}` (lintas workspace 404, tanpa login → /login); `/_metrics` Prometheus (token) + `/_readiness`; halaman Status sistem `/system` (admin instance); deploy Compose satu VPS (`docker/prod/Dockerfile`, `deploy/`: Caddy, SeaweedFS, backup pg_dump + rclone ke luar VPS, diuji smoke + restore); rilis CLI `dist` (workflow tag `v*`, `/install`, `/install.sh|ps1`, `agentgate self-update`, `bin/release-cli`); audit a11y axe + sweep visual 3 mode; `docs/admin/`, `docs/panduan/`, `docs/REVIEW-AKHIR.md`; 806 tes Rust + 8 tes JS |
| R1 Revisi keputusan pemilik | selesai (2026-11-14) | Semua baris R1 di `REVIEW-AKHIR.md` §3.0: bukti gate manual (screenshot/log/teks berlabel Manual, gate terpenuhi setelah approver stage setuju, audit + riwayat + notifikasi), merge otomatis default mati + tombol Merge + cara merge squash/merge/rebase (project `auto_on_review` dimigrasi ke manual), client OAuth wajib disetujui user sebelum dapat token + cleanup client yatim 24 jam, runner tim khusus Owner/Admin (auto-ambil pribadi mati/tim nyala, batas paralel per runner), online 90 dtk/heartbeat 30 dtk (konstanta bersama core), izin `manage_knowledge`, rate limit per token API (60–6.000) + aktifkan ulang webhook, retensi backup harian 14 hari + bulanan 6 bulan (diuji), logo SVG tersanitasi + CSP ketat (uji XSS), Sentry server + browser (mati tanpa DSN, data sensitif dibuang), varian tombol `success` (Approve hijau, Reject merah); 826 tes Rust + 8 tes JS + 6 tes skrip bin |
| R5 AI Office baru | selesai (2026-11-14) | Kanvas Canvas 2D diganti panggung SVG + vanilla JS sesuai prototype `office-v1`: Isometrik Studio (V1) sebagai tampilan utama + sakelar Denah (V2) yang disimpan per user di localStorage, kolam cahaya hangat V3 (multiply terang/screen gelap), chibi dengan aksesori/gaya rambut/hijab dari `look`, tema gelap lewat token CSS tanpa gambar ulang, label HTML yang menghindari tumpukan (geser naik/samping, jangkar cadangan, hindari HUD), zoom jauh = badge ikon saja, label karakter berupa tombol berurutan logis (Tab/Enter/Escape, `aria-label`), animasi hanya tanpa `prefers-reduced-motion`; semua fungsi M10 tetap (ruangan per project + Ruang rapat/Pantry, karakter job/persona/tiket/runner online, geser/zoom/pinch/keyboard, mini-map, tab ruangan, panel log live + Approve/Reject + Kustom teraudit, polling 3 dtk); 826 tes Rust + 26 tes JS (18 baru untuk logika murni `office-core.mjs`) |
| R2 Revisi git & code graph | selesai (2026-11-14) | Semua baris R2 di `REVIEW-AKHIR.md` §3.0: GitHub App sebagai cara utama (alur pasang + verifikasi OAuth `/user/installations`, state HMAC, token instalasi dari JWT RS256 di-cache, izin/repo dari instalasi, satu webhook `/webhooks/github-app` untuk installation/installation_repositories/PR/check/push, Segarkan, ditangguhkan), PAT + GHES tetap; clone dangkal `--shallow-since` 12 bulan di volume cache + fetch inkremental, statistik git dihitung server, kredensial lewat header env git (tidak di URL/config/log), batas waktu/ukuran/disk (`df`), tarball cadangan, clone yatim dibersihkan cleanup; embedding lokal fastembed (feature `semantic`, `multilingual-e5-small`, ONNX Runtime dinamis) inkremental per hash teks simbol ke pgvector, pencarian hibrid RRF di Cari simbol + MCP `ag_search mode`, bisa dimatikan; PR knowledge otomatis setelah approve (branch `agentgate/knowledge-*`, commit tambahan selama PR terbuka, status di halaman dokumen); git tag saat Rilis sekarang (HEAD branch utama atau commit, peringatan bila provider tidak tertaut); 844 tes Rust (18 baru; GitHub palsu + kunci RSA tes, repo bare lokal untuk clone, provider embedding `fake`) + 26 tes JS |
| R3 Revisi tiket, workflow, planning | selesai (2026-11-14) | Semua baris R3 di `REVIEW-AKHIR.md` §3.0: panel samping detail tiket dari board/daftar/backlog/roadmap (`<dialog>` modal native di kanan, fokus terkunci, `pushState` ke URL tiket sehingga Back menutup/Forward membuka, Esc/tutup/klik latar, "Buka halaman penuh", 390px layar penuh; partial `_summary.html` dipakai bersama halaman penuh; header `X-Ticket-Panel`), builder workflow sebagai kanvas graf (`workflow-core.mjs` murni + `workflow-canvas.js`: tata letak otomatis, panah maju + lencana transisi, panah `gagal_ke` merah putus-putus berlabel, kotak paralel, geser/zoom/pinch/keyboard, kamera per workflow; sakelar Kanvas/Daftar diingat), izin aksi berbahaya per stage (utang M3; `Stage.danger`, YAML `izin_bahaya`, menimpa persona di JobSpec), JQL `OR`/`NOT`/kurung dengan presedensi Jira + error Fluent (utang M9) + autocomplete field/operator/nilai (endpoint saran, combobox ARIA), batas WIP ditegakkan di semua jalur pindah kartu (board drag/menu, form detail, "tandai selesai", API, MCP, sinkron Jira) dengan paksa Admin beralasan (audit `ticket.wip_overridden`) dan penolakan di browser tanpa memindah kartu, roadmap per epic (batang rentang anak + progres, buka/tutup, grup Tanpa epic, panah blocks dari epic yang tertutup; tooltip roadmap ke Fluent); 855 tes Rust (11 baru + kasus baru di tes lama: parser/konteks JQL, saran, WIP semua jalur, panel, roadmap epic, izin stage) + 38 tes JS (12 baru: `jql-core`, `workflow-core`) |

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
| 2026-11-12 | ~~Tata letak per halaman 1A…8B~~ digantikan desain **Tenang** (prototype `docs/reference/ui-prototype/`, varian shell A, sec A, subnav E, table A, stat A, board C, prio C, auth B); font Geist/Geist Mono di-vendor, mode gelap via `html.dark` (`theme.js`) | Dipilih pemilik produk |
| 2026-11-12 | Kontrol form memakai class sendiri (`app-input`, `app-check`, `app-switch`, `app-radio`), bukan `.input`/`.field` Basecoat; Basecoat tetap untuk tombol, dropdown, dialog, toast, alert, tabel | Selector Basecoat terlalu spesifik untuk ditimpa rapi |
| 2026-11-12 | Akun, pengaturan workspace, dan pengaturan project berbagi satu menu Pengaturan di sidebar (dibuka dari menu profil); sub-halaman subpage E adalah rute server (`/settings`, `/settings/security`, `/settings/danger`) | Prototype `NAV_SETTINGS`; tanpa panel JS |
| 2026-11-12 | Flash sukses tampil sebagai toast; info/peringatan/error sebagai alert | Prototype |
| 2026-11-12 | Tabel besar server-side (`views::datatable`): form GET biasa, HTMX menukar region (`hx-select` + `outerMorph`), URL ikut berubah; anggota memakai limit/offset + count di DB. Audit log tetap pagination keyset (lebih lama/terbaru) | Bekerja tanpa JS; audit tidak bergeser saat kejadian baru masuk |
| 2026-11-12 | Tab tampilan project disematkan/diurutkan per user (`users.project_views`, POST `/account/project-views`) | Aturan UX 9 |
| 2026-11-12 | Logo project lewat Loco storage (disk di dev/staging/prod, memori di tes), PNG/JPEG/WebP ≤ 1 MB dicek dari isi berkas, SVG ditolak; disajikan controller dengan cek akses project (404); warna project `projects.color` (hex) | Aturan §8; SVG butuh sanitasi |
| 2026-11-12 | Endpoint combobox `GET /w/{slug}/people?q=` membalas `<option>` (partial) | Pilihan orang yang bisa dicari tanpa memuat semua anggota |
| 2026-11-12 | Sapaan UI "Anda" (bukan "kamu") di semua teks Fluent | Mengikuti prototype |
| 2026-11-12 | `connect_timeout` pool DB tes 5 detik (sebelumnya 500ms) | Tes throttle burst 12 login gagal 500 karena acquire koneksi habis waktu |
| 2026-11-12 | M2: `tickets.status` 4 kategori tetap (backlog/todo/in_progress/done) = kolom board; stage M3 akan memetakan ke kategori ini | Aturan blocks, filter, dan pencarian tidak berubah di M3 |
| 2026-11-12 | Blocks: tiket yang diblokir tiket belum Selesai tidak bisa pindah ke Dikerjakan/Selesai lewat jalur mana pun (board, form, menu); 422 menyebut kode pemblokir | Spec "tidak bisa mulai" |
| 2026-11-12 | Izin tiket: lihat `ViewProject`; ubah/komentar/lampiran/relasi/buang/pulihkan `EditProject`; hapus permanen `AdministerProject` atau pembuat; tipe/field/kode workspace = izin baru `ManageTicketSettings` (Owner, Admin); override kode per project `AdministerProject` | Spec tidak merinci |
| 2026-11-12 | Kode tiket unik per workspace (tanpa beda huruf besar) via `ticket_codes`; mengganti template menghitung ulang semua kode, kode lama jadi alias (GET → 301) | Tautan lama tidak putus |
| 2026-11-12 | Detail tiket = halaman sendiri `/w/{slug}/t/{code}` (tab Ringkasan/Aktivitas), bukan panel kanan board; judul dan properti diubah lewat dialog "Ubah detail", deskripsi langsung di `md_editor` | Bekerja tanpa JS, URL bisa dibagikan |
| 2026-11-12 | Board: drag-drop HTML5 native (`board-dnd.js`) mengisi form tersembunyi lalu submit biasa; tanpa JS ada menu ⋯ pindah status. Board tanpa kepala halaman (prototype); maksimal 500 kartu | Tanpa library JS |
| 2026-11-12 | Pencarian: Postgres full-text `simple` + cocok kode/ILIKE, hanya project yang bisa dibuka user; `Tiket saya` tab Ditugaskan/Dilaporkan/Diikuti | Spec "pencarian dasar" |
| 2026-11-12 | Notifikasi in-app = baris `notifications` di transaksi yang sama: disebut (@nama-depan/@email, hanya orang dengan akses project), ditugaskan, komentar dan status untuk watcher | Tidak membocorkan tiket |
| 2026-11-12 | Lampiran ≤ 10 MB, diunduh `attachment` + `nosniff` + `no-store`; key `tickets/<ticket>/<uuid>`. **Dev memakai S3 SeaweedFS** (`storage_aws_s3`, bucket `agentgate-dev` dibuat `bin/_lib.sh`); tes memakai storage memori | Brief M2 |
| 2026-11-12 | Sampah: `deleted_at`, 404 di semua halaman kecuali Sampah, purge > 30 hari oleh langkah `tickets` di task `cleanup` (berkas lampiran dihapus setelah commit) | Spec |
| 2026-11-12 | Seed dev menambah 10 tiket contoh KSR (epic, story, subtask, bug, relasi blocks, komentar @mention) lewat service produksi; dilewati bila KSR sudah punya tiket | Layar bisa dicek tanpa input manual |
| 2026-11-12 | M3: definisi workflow disimpan JSON per versi (`workflow_versions`); `workflows.version` = versi aktif, `workflows.draft` = draf builder (selalu valid). Terbitkan = versi baru; stage dihapus yang masih berisi tiket wajib dipetakan di halaman migrasi | Tiket yang berjalan tetap memakai aturan versi saat masuk stage-nya (`tickets.stage_version`) |
| 2026-11-12 | M3: tiket menyimpan `workflow_id`, `stage_key`, `stage_version`, `stage_state` (active/review/decision), `fail_count`, `stage_entered_at`, `cancelled_at`; `tickets.status` tetap = kategori stage | Laporan/filter M2 tetap bekerja per kategori |
| 2026-11-12 | M3: workflow default per tipe tiket (`ticket_types.workflow_id`), override per project (`project_workflows`). Workspace baru: Kanban untuk semua tipe, "Subtask ringkas" untuk Subtask. Ganti workflow → tiket pindah ke stage dengan ID sama, lalu kategori sama | Spec "workflow per tipe tiket" |
| 2026-11-12 | M3: kelola workflow memakai izin `ManageTicketSettings`; workflow project memakai `AdministerProject` | Tidak menambah izin baru |
| 2026-11-12 | M3: maju ke stage berikut butuh stage aktif, bukan stage approval, gate terpenuhi, dan tidak melewati stage yang menegakkan gate/approval; selain itu = lompat (Admin, alasan wajib, audit). Paksa transisi butuh `projects.allow_force_transition`. Mundur bebas bagi yang bisa mengubah tiket | Server menegakkan gate (spec) |
| 2026-11-12 | M3: approve/reject oleh approver stage; bila stage tanpa approver, siapa pun yang bisa mengubah tiket. Reject wajib catatan, menambah putaran gagal dan kembali ke `gagal_ke`; mencapai batas → Butuh keputusan | |
| 2026-11-12 | M3: lima keputusan (petunjuk, perbaiki sendiri, ubah requirement, terima dengan catatan → tiket Bug baru berelasi "terkait", batalkan → stage terakhir + `cancelled_at`) | Spec "Butuh keputusan" |
| 2026-11-12 | M3: kolom board = gabungan stage semua workflow di project, urut (kategori, indeks); drag-drop mengirim `stage` | Board satu project bisa berisi beberapa workflow |
| 2026-11-12 | M3: artifact per stage, setiap simpan = versi baru (draft/pending/approved/rejected); versi yang disimpan saat stage menunggu review langsung `pending`; approve/reject memutuskan versi terbaru stage itu. Diff = LCS baris sederhana | |
| 2026-11-12 | M3: event `status_changed` M2 diganti `stage_changed`; tiket baru tidak boleh mulai di stage yang didahului stage approval/gate (seed subtask diubah ke Dikerjakan) | |
| 2026-11-14 | Tes paralel lewat **cargo nextest** dengan database per slot (`<db test>_s<slot>`, slot 0 dimigrasi, slot lain di-clone tiap run); Postgres bersama `max_connections=300` | Suite penuh 806 tes turun dari ~6 menit ke ~72 dtk (tes berurutan 228 dtk untuk 395 tes) |
| 2026-11-14 | **cargo hakari** (`workspace-hack`) menyeragamkan feature dependency; CLI/core/graph tidak memakainya, image produksi mengosongkannya | Feature tokio/clap/loco berbeda antara `bin/dev`, `bin/test`, dan clippy memaksa compile ulang tiap pindah perintah |
| 2026-11-14 | `common/` + `models/` dipindah ke crate `agentgate-models` (`server/models`), di-re-export server; `server/src/models` symlink untuk `bin/loco db entities` | Build tes setelah mengubah satu service ~21 dtk (sebelumnya ~3 menit, bersama kenaikan job) |
| 2026-11-14 | `CARGO_BUILD_JOBS` image dev 4 → 8 (`AG_CARGO_JOBS` untuk menimpa); `sccache` tetap; linker tetap bawaan (`rust-lld`) | Job juga membatasi thread codegen rustc; sccache dibutuhkan worktree baru; rust-lld sudah default sejak Rust 1.90 |
| 2026-11-14 | R1: bukti gate manual = tabel `manual_evidence` terpisah dari `job_evidence` (runner, bertanda tangan); `screenshot_exists` hanya menerima screenshot; yang memutuskan = aturan approve stage | Evidence runner append-only dan wajib tanda tangan |
| 2026-11-14 | R1: tombol Merge muncul untuk PR yang ditandai `merge_ready_at` saat stage Code Reviewer di-approve dan CI lulus (termasuk check wajib); izin `EditProject` | Approve dan merge tetap dua langkah manusia |
| 2026-11-14 | R1: runner tim = runner yang ditandai Owner/Admin; auto-ambil/batas paralel per runner. Runner tim juga boleh auto-ambil job knowledge; aturan executor job tidak berubah | Tidak mengubah siapa yang boleh menjalankan job |
| 2026-11-14 | R1: Sentry browser memakai reporter kecil `static/js/sentry.js` (envelope API), bukan SDK Sentry JS | CSP `script-src 'self'` tanpa bundel pihak ketiga; kontrol penuh atas data yang dikirim |
| 2026-11-14 | R1: sanitasi SVG allowlist dengan quick-xml (bukan blocklist); webhook gagal beruntun hanya diaktifkan ulang Owner/Admin bawaan; dialog desktop dibatasi tinggi layar (isi bergulir) | Keamanan dan temuan cek visual |
| 2026-11-14 | R2: instalasi GitHub App diverifikasi lewat OAuth user (`/user/installations`), bukan hanya `installation_id` dari query | Cegah menautkan instalasi milik orang lain ke workspace |
| 2026-11-14 | R2: ONNX Runtime dimuat dinamis (`ORT_DYLIB_PATH`, rilis resmi Microsoft di image) | Biner statis crate `ort` butuh libstdc++ GCC 13+, gagal link di bookworm |
| 2026-11-14 | R2: tes dan `bin/dev` tanpa feature `semantic` (provider `fake`); `bin/ci` meng-clippy feature itu | Loop dev tidak bertambah lambat |
| 2026-11-14 | R2: PR knowledge ke repo project pertama yang tertaut (urut dibuat) | Belum ada pilihan repo knowledge per project |
| 2026-11-14 | R5: AI Office dipecah `office-core.mjs` (logika murni, diuji node), `office-svg.mjs` (gambar), `office-engine.mjs` (kamera/input/label); denah tetap mengikuti kolom server (2 project per kolom, Ruang rapat + Pantry di kolom terakhir, Lounge pengisi slot) | Server dan JSON `/office/state` tidak berubah; logika label/penempatan bisa diuji tanpa browser |
| 2026-11-14 | R5: Isometrik dan Denah memakai satu daftar kursi (satuan ubin); Denah = tampak atas dari tata letak yang sama | Karakter berada di tempat yang sama saat beralih mode |
| 2026-11-14 | R5: label karakter = tombol HTML (urutan Tab per ruangan), SVG `aria-hidden`; label yang disembunyikan LOD/declutter tetap bisa difokus (`.clip`) | Karakter terbaca screen reader tanpa elemen SVG yang bisa difokus |
| 2026-11-14 | R3: panel tiket = `<dialog>` modal native + `fetch` dengan header `X-Ticket-Panel: 1` (bukan swap HTMX); `history.pushState({agPanel})` sendiri | HTMX 4 hanya memulihkan entri history bertanda `htmx`; dialog modal memberi fokus terkunci + latar inert tanpa skrip trap; URL tiket yang dibuka langsung tetap halaman penuh |
| 2026-11-14 | R3: aksi di dalam panel (komentar, ubah detail, pindah stage) berjalan seperti di halaman penuh (redirect ke halaman tiket) | Satu partial dan satu set handler; panel tidak bercabang dari halaman penuh |
| 2026-11-14 | R3: batas WIP dihitung atas beban project per stage (tiket aktif, bukan sampah/dibatalkan), bukan kartu yang tampil di filter/sprint/lajur; kepala kolom menampilkan angka yang sama | Penegakan konsisten di board, API, MCP, dan Jira |
| 2026-11-14 | R3: WIP dicek di jalur pindah kartu (`move_in`, form detail, "tandai selesai"); approve, keputusan, lompat/paksa Admin tidak dicek | Alur gate tidak boleh macet karena kolom penuh; Admin punya jalur paksa sendiri |
| 2026-11-14 | R3: kanvas builder memakai node HTML (tautan ke panel stage) + SVG garis, tata letak otomatis dari urutan definisi; posisi tidak disimpan | Tanpa kolom posisi di definisi; node tetap tautan biasa (keyboard, screen reader, tanpa JS) |
| 2026-11-14 | R3: autocomplete JQL — konteks kursor dihitung server (`jql::context`, tokenizer yang sama dengan parser), JS hanya menyisipkan | Satu tata bahasa; nilai saran dari data yang boleh dilihat pelaku |
| 2026-11-14 | R3: roadmap epic = epic terdekat di rantai induk (maks. 5 tingkat); anak epic yang tertutup diwakili baris epic untuk panah blocks | Subtask story ikut epic-nya; panah tidak hilang saat epic ditutup |

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

- ~~**M4/M5**: layer `html_errors` mengubah semua error non-HTML menjadi HTML~~ selesai di M4: path API
  (`controllers::api_routes::is_api`: `/mcp`, `/oauth/*`, `/.well-known/*`, API publik) selalu dibalas JSON.
- Daftar pengecualian CSRF ada di `server/src/controllers/middleware/csrf.rs` (`EXEMPT_PATHS`), bukan di `app.rs`
  seperti di rencana M0 — satu tempat yang sama dengan aturannya.

- **Keamanan akun (setelah M1)**: event keamanan akun (aktif/nonaktif 2FA, ganti/reset password, tautkan
  OAuth) belum diaudit karena audit log per workspace; perlu log keamanan per user. Mengaktifkan 2FA dan
  membuat ulang kode pemulihan belum meminta autentikasi ulang dan belum mencabut sesi lain; reset password
  belum menghapus TOTP. Undangan (kirim/ulang/cabut) masih memakai role pelaku dari awal request.
- **Deploy**: lockout login per IP memakai IP klien dari proxy; pastikan reverse proxy meneruskan IP asli,
  kalau tidak satu IP proxy bisa mengunci login semua orang.
- **UI Tenang (2026-11-12)**: logo project di staging/prod masih disk lokal (`STORAGE_DIR`), S3 (`storage_aws_s3`)
  belum diaktifkan (M12). SVG logo ditolak sampai ada sanitasi. Akses project belum memakai combobox `src`; tabel
  Tim/Jabatan/Role belum datatable (data kecil), tanpa bulk action. Detail audit hanya tampil bila baris
  terpilih ada di halaman yang dibuka. Profil akun belum punya foto/sesi aktif/akun terhubung; workspace belum
  punya logo dan "Metode masuk" per workspace (tidak ada di backend M1). Tombol salin/unduh kode pemulihan
  belum ada. Editor `md_editor` (Crepe, ~2,8 MB, dimuat lazy) belum dipakai halaman mana pun sampai M2.
- **M2 Tiket (2026-11-12)**: deskripsi dan komentar tampil sebagai teks pre-wrap (belum ada renderer
  Markdown server-side yang disanitasi). Komentar belum punya balasan/reaksi/pin. Board memuat maksimal 500
  kartu per board tanpa pagination kolom. Mengganti template kode menghitung ulang semua tiket workspace dalam
  satu transaksi (lambat untuk ribuan tiket; pindahkan ke job bila perlu). Purge workspace (cleanup) belum
  menghapus berkas lampiran tiket di storage. Logo project di dev kini ikut S3: logo lama yang tersimpan di
  disk dev tidak muncul lagi (unggah ulang). Notifikasi belum dikirim lewat email/push. Field custom belum
  bisa diurutkan dan belum ada jenis multi-pilih/orang. Label tiket teks bebas tanpa warna.
- **M3 Workflow (2026-11-12)**: gate evidence (screenshot, tes, PR, merged, CI) selalu belum terpenuhi sampai
  runner (M5) dan provider git (M6) ada; stage dengan gate itu hanya bisa dilewati lompat/paksa Admin. Stage
  paralel dijalani berurutan (flag `paralel` hanya disimpan dan ditampilkan). Persona agent tetap 9 bawaan
  sampai M4; prompt/skill/tooling belum dipakai. Migrasi tiket saat terbitkan berjalan dalam satu transaksi
  (pindahkan ke job bila ribuan tiket). Artifact tampil teks pre-wrap (belum render Markdown) dan editor
  artifact memakai textarea `md_editor` tanpa unggah gambar. ~~Builder berupa daftar vertikal~~ (R3: kanvas graf +
  izin aksi berbahaya per stage); belum ada perkiraan "N tiket ikut dimigrasi" di bar simpan. Impor YAML lewat textarea (belum unggah berkas).
  Notifikasi review/keputusan hanya in-app.
- **M4 MCP & persona (2026-11-13)**: DCR (`/oauth/register`) terbuka tanpa rate limit dan tanpa CORS
  (cukup untuk client CLI; perlu rate limit sebelum produksi). Kode otorisasi tidak dibatalkan setelah percobaan
  PKCE salah (tetap kedaluwarsa 10 menit, sekali pakai). Daftar `mcp_tools` persona belum ditegakkan per sesi
  MCP (semua tool tersedia; izin tetap dicek per role/project). Skill pack yang sudah disetujui belum bisa
  diperbarui dari repo (impor ulang = pack baru); pack dari repo tidak bisa diedit. Halaman Hubungkan client
  belum punya menu per client (ganti nama, lihat token/refresh). Halaman draft tiket belum punya kunci sesi,
  pecah/gabung draft, chip repo, dan panel estimasi AI seperti prototype. "File aturan di repo" dan Knowledge
  persona tampil "segera" (M5/M8). Tool `ag_ask` hanya menulis komentar + notifikasi in-app (belum menunggu
  jawaban secara sinkron).
- **M5 CLI & runner (2026-11-13)**: runner menerima event lewat `/api/runner/events` yang bisa mengulang event
  yang sama (runner harus idempoten; belum ada cursor). WebSocket hanya kanal kontrol; server mem-polling DB tiap
  2 detik (ganti ke LISTEN/NOTIFY di M12). Runner tidak mencoba WebSocket lagi setelah jatuh ke polling.
  `RunnerInfo` belum membawa preferensi terminal dari server (sementara env `AGENTGATE_TERMINAL`); `terminal-failed`
  hanya bisa dilaporkan setelah `start`. `JobSpec` belum membawa base branch (worktree dari HEAD default).
  Backup `.agentgate/backup` belum diunggah ke server (M8). Sinkron aturan lewat PR belum ada (M6); halaman
  "Aturan & drift" hanya membandingkan checksum yang dilaporkan runner. Antrean job difilter di memori (cukup
  untuk ratusan job; pindah ke query SQL bila besar). Log live lewat polling HTMX 2 detik (belum SSE). Tabel
  antrean di 390px digulir horizontal (badge status terpotong di tepi). Kredensial CLI disimpan di file 0600,
  belum keychain OS. Install CLI masih `cargo install --git` (belum ada binary rilis/skrip install).
  ~~Pengaturan "Job paralel maksimal" belum ditegakkan~~ (selesai di M6: runner membaca `max_parallel` saat
  start dan server menolak klaim di atas batas). Auto-ambil hanya berlaku saat runner menyala.
  ~~`JobSpec` belum membawa base branch~~ dan ~~sinkron aturan lewat PR~~ selesai di M6.
- **M6 Git & PR (2026-11-13)**: hanya GitHub.com dan GitHub Enterprise Server; GitLab, Bitbucket, dan runner
  jembatan tampil "segera". Autentikasi memakai personal access token, belum GitHub App (izin per repo, token
  berumur pendek). Satu repo per job runner (tiket lintas repo belum didukung). Panel Konvensi git "Environment
  testing", "Deploy", dan "Jalur kode" tampil "segera". Belum ada komentar review per baris, review AI atas diff,
  dan tombol "Hubungkan PR manual" seperti prototype. Webhook diproses sinkron di request (belum antrean + retry);
  event tidak dikenal dijawab 202 dan diabaikan. Status CI dihitung dari check run/status terakhir per commit
  head, belum per check wajib dari branch protection. Merge otomatis tidak mencoba ulang bila gagal (hanya notif
  `merge_failed`). Token baru divalidasi saat Hubungkan; token yang kemudian dicabut baru ketahuan saat panggilan
  API gagal. Semua dialog sempat rusak karena Basecoat 1.1 memosisikan isi dialog dengan `position: fixed`;
  diperbaiki di `components/dialog.css` (cek ulang bila Basecoat di-upgrade).
- Role custom tidak pernah menjadi "admin di semua project": hanya Owner/Admin bawaan yang boleh menugaskan
  role Admin bawaan (termasuk di grant project) dan mengubah izin role yang dipakai grant project.

- **M7 Code graph (2026-11-13)**: belum ada embedding/semantic search (pencarian leksikal sadar-identifier +
  prefix); relasi gRPC belum dicocokkan; dokumen sebagai node graph menunggu M8/M10; viewer graph visual = M10
  (tab "Viewer" tampil "segera"). Index server memakai arsip tarball (tanpa riwayat git, jadi statistik git hanya
  dari CLI) dan berjalan di worker Loco dengan direktori sementara; unduhan arsip memakai timeout provider yang
  sama (repo sangat besar bisa timeout). Request CLI graph memakai timeout 60 detik per panggilan. Pencarian
  edge duplikat di build `O(E²)` kasus terburuk (cukup untuk ribuan edge per file; ganti ke indeks bila lambat).
  Call di argumen makro Rust dideteksi heuristik (`ident(` di token tree). Panggilan lewat trait object/closure
  tersimpan belum ter-resolve tanpa SCIP. Jumlah pemanggil di daftar hasil cari hanya edge `calls` (route handler
  dan HTTP lintas repo tampil di detail simbol). Fakta lama diparse ulang lewat `FileFacts.version`
  (`EXTRACTOR_VERSION`) — naikkan setiap kali hasil extractor berubah.

- **M8 Knowledge base (2026-11-13)**: editor dokumen belum punya komentar inline + @mention dan highlight kode di
  blok kode. Sinkron ke repo (`agentgate knowledge sync`) meng-commit ke branch aktif; commit + PR otomatis oleh server
  lewat provider git belum ada. Instruksi persona `writer` di DB belum diperbarui untuk alur knowledge (job knowledge
  memakai prompt bawaan). Prompt MCP `ag-knowledge-sync` hanya prompt interaktif. Diff panjang di Tinjau usulan belum
  dilipat (semua baris tampil). Versi dokumen workspace hanya daftar, belum ada halaman diff seperti dokumen project.
  Kandidat dokumen untuk pemicu "tiket Done" heuristik: file sumber berubah di repo PR tiket, isi menyebut kode tiket,
  atau jenis dengan pemicu Done aktif (belum menelusuri dampak lewat code graph).
  Event knowledge belum masuk audit log. Estimasi biaya scan hanya token (tanpa Rupiah).

- **M10 Visual (2026-11-13)**: AI Office: pekerjaan antre (`queued`) tidak tampil; persona yang punya beberapa job
  muncul sebagai beberapa karakter; maks. 8 ruangan project (sisanya dihitung "tersembunyi") dan 12 tiket menunggu
  per project; semua ruangan project memakai satu template perabot; label ruangan bisa tersembunyi bila bertabrakan
  dengan overlay (aturan prototype); kanvas tetap terang di mode gelap (hanya latar dan label yang gelap, sama dengan
  prototype A1); state dipolling 3 dtk (belum lewat WebSocket/SSE); karakter yang hilang saat jalan (job selesai)
  langsung lenyap tanpa animasi pulang. Lantai kantor tidak bisa diubah (prototype punya opsi gaya A2/A3; hanya A1
  yang dipilih). Graph viewer: layout berlapis sederhana (bisa bersilang pada graf padat), maks. 40 node per level.
  Penghemat token: atribusi penghematan ke tiket/job heuristik (job terbuka terakhir pelaku), biaya dalam USD,
  "Token ≈ byte/4". Mockup: screenshot otomatis = evidence screenshot stage (bukan render server), bandingkan memakai
  screenshot QA terbaru. ~~`markdown.js` dan `md-editor.js` masih memakai pola `htmx.onLoad` saja~~ (selesai di M11, juga
  `combobox.js`/`dialog.js`; tes `bin/tests/js_test.sh`).

- **M9 Planning & laporan (2026-11-13)**: teks tooltip grafik dan kalender ditulis bahasa Indonesia langsung di Rust
  (belum Fluent; roadmap dan error JQL sudah Fluent sejak R3). ~~Query JQL hanya `AND`~~ (R3: `OR`/`NOT`/kurung +
  autocomplete); evaluasi di memori atas maks. 5.000 tiket terbaru, tampil 200.
  Edit massal belum bisa memindah stage, menghapus, atau menaruh di menu per kolom seperti prototype (satu dialog).
  ~~Batas WIP hanya ditandai~~ (R3: ditegakkan, paksa Admin). Label kode di batang roadmap yang sangat
  pendek terpotong. Event planning (sprint, rilis, component, saved filter) belum masuk audit log. Burnup memakai
  poin tiket saat ini, bukan poin historis. Komitmen sprint menghitung poin tiket yang sudah selesai sebelum mulai.

- **M12 Rilis (2026-11-14)** — selesai di M12: DCR rate limit + kode OAuth hangus (utang M4), runner WebSocket lewat
  `LISTEN/NOTIFY` (utang M5), logo/lampiran di S3 di staging/prod (utang UI Tenang), purge workspace menghapus berkas
  storage (utang M2), binary rilis + skrip instal CLI (utang M5). Sisa: browser masih polling; GHES belum dijaga dari
  DNS rebinding setelah connect; restore backup baru diuji ke database smoke lokal; metrik per proses dan backup basi
  belum jadi metrik; Milkdown (daftar) dan runner offline di AI Office masih ditandai axe. Daftar lengkap urut risiko:
  `docs/REVIEW-AKHIR.md` §5.

- **M11 Multi-client & integrasi (2026-11-14)**: adapter client selain Claude Code hanya diuji dengan binary palsu
  (format JSONL dari spike); isolasi config global "best effort" di Gemini/Cursor/OpenCode/Copilot; biaya token Codex
  kosong (tanpa tabel harga); bila hook project Codex tidak dimuat, pembatas hanya sandbox `workspace-write`; parser
  Copilot dan field usage Cursor/Gemini toleran (belum terverifikasi); plugin OpenCode untuk API V2 tanpa session-start.
  API: daftar token/webhook disortir di memori, cursor tiket berbasis offset (bukan keyset), kirim ulang hanya untuk
  webhook aktif; perubahan di luar request (worker/task) menunggu sapuan webhook per menit. Tabel datatable di 390px
  digulir horizontal (pola lama). Impor CSV sinkron di worker dengan batas 5.000 baris, belum membuat field custom baru.
  Jira: satu koneksi per workspace (satu pasangan project), dua arah memakai polling (belum webhook Jira masuk),
  deskripsi ADF ↔ Markdown sederhana, sprint lewat agile API best effort; semua diuji dengan server Jira palsu.
  Notifikasi M11 (ekspor siap/gagal, webhook nonaktif) hanya in-app.

## Keputusan sementara (dipakai sampai review akhir pemilik produk)

Rangkuman terkelompok semua keputusan M0–M12: `docs/REVIEW-AKHIR.md` §3.

- M12: admin instance (halaman Status sistem `/system`) = email di `SYSTEM_ADMIN_EMAILS`, bukan Owner workspace;
  bukan admin → 404. Dev: `owner@agentgate.test`.
- M12: `/_metrics` butuh bearer `METRICS_TOKEN` (kosong/salah → 404) dan diblok Caddy dari internet; `/_readiness`
  publik (bawaan Loco).
- M12: DCR dibatasi 20 pendaftaran per IP per jam (429 + `Retry-After`); kode otorisasi dihapus setelah satu percobaan
  tukar gagal.
- M12: URL GitHub Enterprise hanya https ke alamat publik (dicek literal + DNS saat connect); redirect tarball hanya ke
  api_base yang sama atau https host publik.
- M12: realtime runner = satu `LISTEN agentgate_jobs` per proses web, kanal broadcast per workspace, tick cadangan
  30 dtk; browser tetap polling (SSE belum).
- M12: deploy satu VPS Compose (`agentgate-rs-prod`); worker + scheduler satu proses; `statement_timeout` 30 dtk per role
  DB; backup harian 19.00 UTC ke S3 luar VPS, retensi 14 hari, riwayat di tabel `system_backups`.
- M12: rilis CLI dengan `dist` 0.30.2 (5 target, installer shell/PowerShell/Homebrew, sha256, atestasi), workflow
  ditulis tangan; tap Homebrew belum dipublikasikan; `self-update` menjalankan ulang installer lewat server.
- M12: Sentry/error tracking belum dipasang (butuh DSN); error terlihat di log JSON, `/_metrics`, dan Status sistem.
- M12: token warna `faint` (terang `#6e6e7a`, gelap `#85859a`), `danger` gelap `#f47c7c`, tombol primer/bahaya mode
  gelap lebih pekat (`--ag-accent-btn`, `--ag-danger-btn`) agar lolos WCAG AA; inisial logo project hitam/putih
  mengikuti terang warna; chip tautan memakai `aria-current`.

- Sapaan UI memakai "Anda" (mengikuti prototype).
- Logo project disimpan di disk server sampai abstraksi storage S3 dikerjakan (paling lambat M12).
- Logo SVG ditolak sampai ada sanitasi SVG.
- Tim, Jabatan, dan Role tetap daftar sederhana (tanpa bulk action) sampai datanya besar.
- M2: detail tiket sebagai halaman sendiri (bukan panel samping board); header detail memakai ikon tipe +
  judul + kode/status, belum stepper stage (M3) dan estimasi AI (M9).
- ~~M2: status/kolom board tetap 4~~ digantikan kolom per stage workflow (M3).
- M3: header tiket menampilkan stepper stage di bawah judul; badge "Menunggu review"/PR dan estimasi AI
  di header prototype belum ada (PR = M6, estimasi = M9).
- M3: tombol Approve memakai varian primer (aksen), bukan hijau seperti prototype, supaya konsisten dengan
  sistem desain (satu tombol primer per area).
- M3: builder workflow memakai daftar vertikal + panel konfigurasi (bukan kanvas graf); tab Riwayat versi
  di dalam builder.
- M3: workflow bawaan "Subtask ringkas" (Dev → Review → Selesai) untuk tipe Subtask.

- M4: persona disimpan di DB per workspace dan dirujuk lewat `key`; hapus = soft delete dengan persona
  pengganti untuk stage yang memakainya. Avatar builder prototype diganti pilihan warna + ikon.
- M4: impor skill pack lewat arsip GitHub (codeload) + pemindaian statis (skrip, perintah berbahaya, URL);
  pack wajib disetujui manusia sebelum bisa dipasang ke persona.
- M4: redirect URI client OAuth hanya https, loopback (`127.0.0.1`/`localhost`/`[::1]`, port bebas), atau skema
  privat (`com.example:/cb`). CSP halaman consent mengizinkan `form-action` ke origin redirect client itu saja.
- M4: "Hubungkan client AI" ada di menu Akun (token milik user, bukan workspace); satu token terikat satu
  workspace yang dipilih di consent.
- M4: server MCP stateless dengan respons JSON (tanpa sesi SSE); validasi Host dimatikan karena auth memakai
  bearer token. Nama, deskripsi, dan teks tool/prompt berbahasa Indonesia.
- M4: slash command = prompt MCP `ag-<key-stage>` (unik per workspace) + utilitas (`ag-status`, `ag-next`, dst.);
  di Claude Code tampil sebagai `/mcp__agentgate__ag-…`.
- M4: draft dari `ag_draft_tickets` tercentang secara default; halaman draft dibuka dari tautan di board
  (bukan menu sendiri). Membuat tiket mengikuti urutan induk → anak dalam satu transaksi.

- M5: login CLI memakai device flow RFC 8628 dengan client bawaan `agentgate-cli`; halaman `/device` memakai satu
  field kode (format `ABCD-EFGH`), bukan 8 kotak seperti prototype. Token CLI tampil dan bisa dicabut di Akun →
  Hubungkan client AI.
- M5: kredensial CLI di `~/.config/agentgate/credentials.json` (mode 0600), bukan keychain.
- M5: evidence ditandatangani Ed25519 dengan kunci per runner (didaftarkan saat `runner start`); gate
  `tests_passed` dipenuhi hasil `[test] command` di `.agentgate/project.toml` (JUnit/exit code), `screenshot_exists`
  dipenuhi berkas di `.agentgate/evidence/`. Evidence tanpa tanda tangan valid ditolak.
- M5: job selesai sukses memanggil `stages::complete` sebagai executor; job sukses walau tes gagal, gate yang
  menentukan lolos/tidak. Runner tidak pernah menyala otomatis dan tidak pernah mengambil job tanpa auto-ambil.
- M5: halaman Runner & CLI di `/w/{slug}/runner/{panel}`, masuk menu Pengaturan untuk semua anggota (bukan hanya
  Admin). Tanda "menunggu runner" di komentar `@agent` hanya tampil untuk penulis komentar.
- M5: job dari komentar `@agent` diklaim untuk penulis komentar dan membalas sebagai komentar persona; judul
  job memakai nama persona (job stage memakai nama stage).
- M5: `agentgate init` meng-commit backup `.agentgate/backup` dan hanya meng-gitignore `.agentgate/evidence/`;
  commit CLI memakai `--no-verify`. Hook PreToolUse di luar job tidak memutuskan apa pun (timeout 1800 detik saat
  menunggu izin). RTK hanya dicek di `doctor` dan hook rewrite.

- M6: provider git memakai personal access token (terenkripsi XChaCha20-Poly1305 seperti secret lain), bukan GitHub App.
  Satu koneksi per jenis provider per workspace; GHES boleh http hanya untuk host loopback (dev/tes).
- M6: izin baru `manage_integrations` (Owner/Admin bawaan) untuk Pengaturan → Git provider; Konvensi git project
  memakai `administer_project`.
- M6: konvensi git disimpan per project (bukan per stage): pola branch (default `ag/{kode}-{slug}`), format commit, judul
  PR, base branch, hasil kode (PR otomatis / manual: AI hanya mengubah file), mode merge, check CI wajib. Spec job lama tanpa field
  `git` dianggap "PR otomatis".
- M6: PR ditautkan ke tiket lewat kode tiket di nama branch atau judul PR. Gate PR/CI terpenuhi lewat webhook;
  stage diselesaikan otomatis hanya bila sudah ada job sukses di stage itu.
- M6: merge otomatis (squash) dipicu approve stage ber-persona `reviewer` bila CI hijau; gagal → notifikasi
  `merge_failed`, stage tetap di-approve.
- M6: secret webhook ditampilkan di dialog "Webhook manual" (untuk repo yang webhook-nya gagal dipasang otomatis).
- M6: `agentgate sync` mendorong branch `agentgate/sync-rules` lalu membuka PR; nama repo diambil dari remote
  `origin`. Bila repo belum tertaut, CLI menampilkan instruksi manual.
- M6: seed dev (`seed_git`) memakai token palsu; tombol yang memanggil GitHub (pasang ulang webhook, merge) akan
  gagal di dev sampai token asli dipasang.

- M7: index oleh server mengunduh arsip tarball lewat koneksi git M6 (bukan `git clone`): tanpa riwayat git, jadi
  hotspot/coupling/pemilik hanya dari `agentgate graph index` di CLI dan dipertahankan di antara index server.
- M7: semantic search/embedding ditunda; pencarian simbol leksikal (tsvector kata identifier + prefix nama).
- M7: SCIP hanya dikirim dari laptop user (`agentgate graph scip`); server cukup tree-sitter. Panel status hanya
  menampilkan perintahnya (belum ada "minta SCIP" ke runner).
- M7: tab Code graph berisi "Cari simbol" (halaman default) + "Status index"; Viewer graph visual prototype = M10.
  Tombol Index ulang butuh izin `edit_project`; repo tanpa koneksi git langsung ditandai gagal dengan petunjuk CLI.
- M7: reindex otomatis hanya untuk PR yang baru di-merge ke branch default repo yang sudah pernah di-index.
- M7: relasi lintas repo: HTTP (route ↔ call literal), event/queue (topik sama), package (dependensi repo lain
  di project). gRPC belum. Keyakinan "tidak cocok" untuk call HTTP tanpa route.
- M7: seed dev (`seed_graph`) meng-index `server/src/fixtures/graph/{kasir-api,kasir-web}` lewat jalur index server
  + statistik git sintetis untuk kasir-api.

- M8: izin pengaturan knowledge workspace memakai `manage_ticket_settings` (belum ada izin `manage_knowledge`);
  approve usulan oleh pemilik knowledge project (selama masih boleh mengubah project) atau admin project.
- M8: job knowledge (scan, minta AI perbarui, audit) tanpa tiket, memakai persona `writer`, branch
  `agentgate/knowledge-xxxx`; auto-ambil hanya oleh peminta/pemilik knowledge.
- M8: setiap graph di-index ulang menandai basi dokumen yang file sumbernya berubah; audit mingguan berjalan pukul
  02.00 UTC hanya pada hari yang dipilih.
- M8: persona dengan `knowledge_types` kosong membaca semua jenis dokumen.
- M8: dokumen workspace memakai jenis `konvensi`; berlaku untuk semua project/persona bila daftar kosong.
- M8: `agentgate knowledge sync`: file repo yang berbeda dari versi server menjadi usulan (pemicu Repo); file yang
  sama dengan isi versi dasar dianggap "tertinggal" (pakai `pull`). Backup `init` diunggah ke server (maks 5 MB)
  dan menghasilkan usulan dokumen `konvensi-coding-style`; gagal unggah hanya peringatan.
- M8: Tinjau usulan memakai varian B prototype (Beranda + kartu kelompok, fokus side-by-side); di layar < 640px
  diff tampil satu kolom (hanya sisi sesudah + baris dihapus).

- M9: estimasi AI diminta lewat komentar `@agent` (job estimasi persona) atau MCP `ag_planning estimate`; estimasi
  draft disalin ke tiket saat draft dibuat. Kalibrasi = rasio aktual/estimasi per stage dari tiket selesai.
- M9: release notes disusun job knowledge tanpa tiket (stage `release_notes`); rilis tidak membuat git tag; "Rilis
  sekarang" memindah tiket belum selesai ke versi lain atau melepasnya (tanpa opsi "tetap di versi ini").
- M9: satu component per tiket; aturan penerima default = pemilik component (bila aktif dan masih punya akses).
- M9: roadmap dikelompokkan per sprint (lalu "Tanpa sprint"), bukan per epic; mulai batang tiket yang sudah jalan =
  mulai sprint atau tanggal dibuat, selesai = tenggat atau perkiraan dari sisa estimasi.
- M9: board Scrum default menampilkan sprint aktif; sprint selesai tidak muncul di chip.
- M9: saved filter dibagikan ke satu tim (pemilik harus anggota) atau seluruh workspace; hanya pemilik yang menghapus.
  Pencarian lanjutan lintas semua project yang boleh dibuka; edit massal lewat jalur ubah tiket biasa (izin per
  tiket, sprint/versi harus milik project tiketnya, yang gagal dilewati dan dilaporkan).
- M9: burndown sprint yang dimulai setelah tanggal mulainya memakai keadaan saat mulai untuk hari-hari sebelumnya.

- M10: AI Office digambar Canvas 2D vanilla (mesin prototype A1 di-port ke `static/js/office-engine.mjs`, dimuat lazy),
  bukan PixiJS seperti di spec. Denah parametris: kolom 17 ubin per dua project (paling ramai dulu), kolom terakhir
  Ruang rapat + Pantry, slot kosong jadi Lounge; satu karakter per job terbuka (persona bisa muncul lebih dari sekali).
- M10: manusia di kantor = pemilik runner online (pakai `runners::is_online`, 60 dtk; spec menyebut 2 menit) yang
  memegang job terbuka; duduk di Ruang rapat bila ada job interaktif, selain itu di ruangan dengan job terbanyak.
- M10: "Butuh approval" = tiket `review` dengan job terakhir stage itu sukses, atau job dengan izin aksi `pending`
  (detail mengarah ke halaman job untuk menjawab izin); "Butuh keputusan" = tiket `decision`. Aktivitas terbaru = 6
  job terakhir yang berubah. Kustom karakter diaudit sebagai `office_look_updated` dengan subjek Workspace; persona
  butuh `manage_ticket_settings`, manusia dirinya sendiri atau Admin di semua project.
- M10: graph viewer dirender SVG server-side (model level di crate `graph`), tanpa library graf; layer node ditebak
  dari path/jenis. Sorot tiket = `jobs.changed_files` (git diff base…HEAD dari runner).
- M10: mockup = artifact `*.html`; pratinjau di `PREVIEW_URL` (`settings.preview_base_url`, dev
  `http://preview.localhost:<PORT>`) dengan token HMAC 1 jam; CSP aplikasi menambah `frame-src` hanya di halaman
  mockup; Approve/Minta revisi memakai endpoint stage dengan `return_to` (path relatif workspace yang sama saja).
- M10: penghematan token terukur server dicatat per kejadian (`token_savings`) dari tool MCP; RTK dan prefix stabil
  tampil "tidak terukur"; toggle per project hanya fitur opsional (Gaya ringkas, Model per persona).

- M11: notifikasi email/Slack/Teams aktivitas tiket tetap di luar cakupan (spec "Di luar cakupan"); OAuth Google/GitHub
  sudah selesai di M1. Notifikasi baru M11 hanya in-app.
- M11: `agentgate init` menulis config untuk keenam client sekaligus (`RULES_VERSION = 2`, satu repo dipakai tim dengan
  client berbeda); slash command tetap prompt MCP (tanpa file skill per stage). Hook memakai string perintah
  `agentgate hook <event> --client <c>`; tanpa `--client` runner memakai client default di pengaturan runner web. Codex
  menerima URL MCP lewat `-c mcp_servers.agentgate.url`; Gemini/Cursor mengabaikan effort; OpenCode `model#effort` hanya
  bila model diisi.
- M11: token API bertindak sebagai pembuatnya (izin role tetap dicek) dibatasi scope (`tiket:baca`, `tiket:tulis`,
  `komentar:tulis`, `laporan:baca`, `knowledge:baca`, `webhook:kelola`) dan project; halaman API & webhook butuh
  `manage_integrations`; API webhook butuh scope `webhook:kelola` + `manage_integrations` (tanpa itu 404). Token dan
  secret webhook dirender sekali di respons POST (bukan PRG), secret webhook dibuat otomatis.
- M11: `outbox_events` (ditulis di transaksi perubahan) = sumber event webhook dan feed mirror Jira; `ping` hanya delivery.
  Maks. 5 percobaan per delivery, 5 gagal beruntun → webhook nonaktif + notifikasi Owner/Admin; kirim ulang = delivery
  baru. URL webhook https; IP privat/loopback ditolak kecuali setting dev/test.
- M11: ekspor hanya Owner/Admin bawaan, ZIP di storage 7 hari, unduhan lewat POST (diaudit); impor CSV memakai
  `manage_integrations`, kode lama jadi alias, status yang butuh gate → stage awal + peringatan.
- M11: connector Jira satu per workspace; dua arah menarik perubahan lewat polling JQL (tanpa butuh URL publik); status dari
  Jira yang melanggar gate ditolak dan status Jira dikembalikan; field lain perubahan terakhir menang; seed dev membuat
  koneksi **dijeda** agar dev tidak memanggil Jira sungguhan.

## Perlu dari user (dikumpulkan untuk akhir)

Daftar terkonsolidasi dan berurutan prioritas: `docs/REVIEW-AKHIR.md` §4.

- M12: VPS + domain aplikasi dan domain pratinjau, bucket S3 di luar VPS untuk backup, `SYSTEM_ADMIN_EMAILS`,
  `METRICS_TOKEN` + Prometheus bila ingin alert, DSN Sentry bila ingin error tracking, org/repo rilis CLI
  (`CLI_RELEASE_URL`), repo tap Homebrew + token, keputusan code signing.

- M11: verifikasi adapter dengan client asli (terpasang + login): Codex (`.codex/` project tetap dimuat dengan
  `--ignore-user-config`? `-a never` di exec? timeout hook), Gemini (hook jalan dengan `--skip-trust`? sub-field `stats`,
  `/mcp auth agentgate`), Cursor (nama binary `agent`/`cursor-agent`, field usage), OpenCode V2 (API plugin, key
  `mcp.servers`), Copilot (`--trust-folder`, format JSONL, `modifiedArgs`, `~/.copilot/mcp-config.json`), dan perintah
  login di halaman Hubungkan. Juga keputusan apakah Gemini CLI tetap didukung (digantikan Antigravity CLI).
- M11: kredensial Jira asli untuk uji nyata (site Jira Cloud + email + API token, dan/atau Jira Data Center + PAT) dengan
  project uji; konfirmasi satu koneksi per workspace cukup dan polling 5 menit untuk dua arah.
- M11: setujui kebijakan webhook (retry 5×, auto-nonaktif setelah 5 gagal beruntun) dan rate limit API 600/menit; pastikan
  `public_base_url` produksi benar (dipakai halaman Dokumentasi API).
- M11: tabel harga per model bila biaya token Codex/Gemini/Cursor perlu tampil.
- Disk host: sempat tersisa ±12 GB (bahkan penuh) saat M11 berjalan; setelah worktree ditutup ±88 GB (volume `agentgate-rs-target-main` ±115 GB; build gagal sekali karena disk
  penuh). Worktree M11 sudah ditutup; pertimbangkan `cargo clean` berkala atau membersihkan build cache Docker.

- M10: konfirmasi PixiJS tidak wajib (AI Office memakai Canvas 2D, ringan dan tanpa dependensi); batas online
  runner 60 dtk vs 2 menit di spec; kurs USD → Rupiah bila biaya token perlu tampil dalam Rupiah; apakah antrean job
  perlu tampil di kantor (mis. karakter menunggu di lorong) dan apakah perlu lebih dari 8 ruangan project; domain
  pratinjau produksi (`PREVIEW_URL`, harus origin berbeda dari aplikasi, idealnya domain terpisah dengan TLS).

- M9: konfirmasi apakah JQL perlu `OR`/kurung dan autocomplete sebelum rilis; apakah batas WIP perlu ditegakkan
  (menolak pindah kartu) atau cukup penanda; apakah rilis perlu membuat git tag lewat provider git; apakah roadmap
  perlu pengelompokan per epic selain per sprint.

- M8: konfirmasi apakah perlu izin terpisah `manage_knowledge` (sekarang ikut `manage_ticket_settings`); tarif token → Rupiah
  bila estimasi biaya scan perlu tampil dalam Rupiah; apakah server boleh membuat commit + PR knowledge sendiri lewat
  provider git (sekarang hanya lewat `agentgate knowledge sync` di laptop).

- M7: Graphify (atau akses ke instalasinya) untuk benchmark pembanding kedua; repo pelanggan nyata (TS/Go/Java/PHP)
  untuk benchmark di luar repo ini; pilihan model embedding (mis. fastembed-rs lokal vs API) sebelum semantic search
  dikerjakan; konfirmasi apakah index server boleh memakai `git clone` (butuh git di image server) agar statistik
  git tidak bergantung CLI.

- M6: token GitHub asli (PAT fine-grained dengan izin contents, pull requests, checks/statuses, webhooks) atau
  keputusan membuat GitHub App; semua tes memakai server GitHub palsu.
- M6: URL publik (`public_base_url`) yang bisa dijangkau GitHub agar webhook PR/CI masuk; tanpa itu webhook harus
  dipasang manual dan gate PR/CI tidak terpenuhi otomatis.
- M6: konfirmasi kebijakan merge otomatis (squash saat stage reviewer di-approve dan CI hijau) dan apakah
  GitLab/Bitbucket dibutuhkan sebelum rilis.
- M6: server GitHub Enterprise Server untuk uji nyata (bila ada pelanggan GHES).

- Remote GitHub untuk repo ini (CI GitHub Actions baru jalan setelah di-push).
- OAuth app **Google** dan **GitHub** (client ID/secret, callback `{APP_URL}/auth/google/callback` dan
  `{APP_URL}/auth/github/callback`). Selama M1 login sosial hanya diuji dengan provider palsu.
- Server SMTP untuk staging/production (`MAIL_FROM`, `SMTP_*`).
- M5: distribusi CLI (binary rilis per OS, Homebrew/skrip install, signing macOS) dan URL repo publik untuk
  `cargo install --git`.
- M5: konfirmasi kebijakan: apakah runner boleh auto-ambil job default-nya mati (sekarang mati), batas job paralel
  default (sekarang tanpa batas), dan apakah halaman Runner perlu dibatasi ke role tertentu.
- M3: konfirmasi apakah gate evidence boleh dipenuhi manual (centang + unggah bukti) sebelum runner/git
  tersedia, atau tetap hanya lewat lompat/paksa Admin seperti sekarang.
- M4: URL publik produksi (`APP_URL` → `settings.public_base_url`) dipakai sebagai issuer OAuth dan resource MCP; harus https
  di staging/prod agar Claude Code/Codex/Gemini bisa login.
- M4: konfirmasi daftar redirect URI yang diizinkan dan apakah DCR perlu dibatasi (mis. hanya user login /
  allowlist nama client).

## Catatan untuk sesi berikutnya

- **R3 selesai dan di-merge ke `staging` (2026-11-14)**, `bin/ci` hijau sebelum dan sesudah merge. Rencana:
  `docs/superpowers/plans/2026-11-14-r3-revisi.md`. Database dev perlu `bin/loco db migrate`
  (`m20261128_000001_r3_revisi`, aksi audit `ticket.wip_overridden`). Screenshot cek visual (desktop, 390px, gelap:
  panel tiket, kanvas + paralel disuntik lewat `page.route`, autocomplete JQL, penolakan WIP disimulasikan dengan
  menandai kolom penuh di DOM karena seed tidak punya batas WIP, roadmap epic) di scratchpad sesi (`shots-r3/`).
  Utang R3: posisi node kanvas tidak bisa diseret/disimpan; aksi di dalam panel tiket berlanjut ke halaman penuh;
  label panah gagal kanvas bisa bertumpuk bila banyak `gagal_ke` ke stage yang berdekatan; nilai saran JQL `stage`
  menampilkan kunci sebagai keterangan (bukan nama kategori); pinch kanvas belum diuji otomatis.

- **R2 selesai (2026-11-14)**. Rencana: `docs/superpowers/plans/2026-11-14-r2-revisi.md`. Panduan GitHub App:
  `docs/admin/github-app.md`; benchmark embedding: `docs/benchmarks/2026-11-14-embedding.md`. Screenshot cek visual
  (desktop, 390px, gelap) di scratchpad sesi (`shots-r2/`). Database dev perlu `bin/loco db migrate`
  (`m20261127_000001_r2_revisi`). Pencarian semantik nyata di dev: build dengan `--features semantic` dan
  `ORT_DYLIB_PATH` (lihat benchmark); tanpa itu Cari simbol menampilkan catatan "model tidak tersedia".

- **R5 selesai dan di-merge ke `staging` (2026-11-14)**, `bin/ci` hijau sebelum dan sesudah merge. Rencana:
  `docs/superpowers/plans/2026-11-14-r5-ai-office.md`; acuan visual permanen `docs/reference/ui-prototype/office-v1/`.
  Screenshot cek visual (desktop, 390px, gelap; Isometrik + Denah; zoom dekat/jauh; data seed dan data prototype
  yang disuntik lewat `page.route`) di scratchpad sesi (`shots-r5/`). Utang R5: animasi berjalan antar-kursi M10
  tidak dibawa; karakter melebihi kapasitas kursi (project 11, rapat 10, pantry 12) disembunyikan dengan catatan
  "+N tidak muat" di label ruangan; ruangan yang isinya berubah digambar ulang utuh (animasi CSS ruangan itu mulai
  ulang); pinch belum diuji otomatis.

- **R1 selesai dan di-merge ke `staging` (2026-11-14)**, `bin/ci` hijau sebelum dan sesudah merge. Rencana:
  `docs/superpowers/plans/2026-11-14-r1-revisi.md`. Screenshot cek visual (desktop, 390px, gelap) di scratchpad sesi
  (`shots-r1/`, 42 file). Batch berikutnya: R2–R4, lalu R5 (AI Office V1 Isometrik Studio + Denah V2 + pencahayaan
  hangat V3, SVG + vanilla JS). Database dev perlu `bin/loco db migrate` (migrasi `m20261126_000001_r1_revisi`).

- **Cara kerja cepat (2026-11-14):** selama mengerjakan fitur jalankan tes yang relevan saja
  (`bin/test <filter>`, mis. `bin/test tickets::mockup`) atau `bin/sh cargo check -p agentgate-server --tests`.
  Suite penuh (`bin/test`, ~72 dtk) dan `bin/ci` hanya sebelum commit akhir/merge. Setelah mengubah dependency:
  `bin/sh cargo hakari generate && bin/sh cargo hakari manage-deps`. Bila `target/` > ~30 GB: `bin/sh cargo clean`.
  Rincian di `docs/reference/loco-project-rules.md` §9 dan §14.

- **M12 selesai dan di-merge ke `staging` (2026-11-14), `bin/ci` hijau sebelum dan sesudah merge.** Belum di-merge ke
  `main`/`release` dan belum di-push (tanpa remote). Langkah berikutnya menunggu review pemilik produk atas
  `docs/REVIEW-AKHIR.md` (keputusan sementara §3, kebutuhan §4).
- Image production: `docker build -f docker/prod/Dockerfile -t agentgate-rs-server:<sha> .` (±287 MB; build pertama
  ±15 menit). Smoke test M12: migrate + web + worker + backup/list/restore terhadap database `agentgate_smoke`
  sementara di Postgres bersama (sudah dihapus). Screenshot sweep M12 di scratchpad sesi (`shots-m12/`, 113 file).
- Seed dev M12 (`seed_system`): 6 baris riwayat backup (satu gagal) untuk `/system/backups`; dilewati bila tabel berisi.

- Seed dev M9 (`seed_planning`): Sprint 4 aktif (5 tiket berpoin, Fix version v1.4, component Pembayaran), Sprint 5
  (rencana), component "Struk & printer". Dilewati bila KSR sudah punya sprint. Buka `/w/pt-contoh/p/KSR/backlog`,
  `/reports`, `/roadmap`, `/calendar`, `/releases`, dan `/w/pt-contoh/search/advanced`.

- Fork/subagen yang men-stub template sementara untuk menjalankan tes **tidak boleh** me-restore file milik orang
  lain yang belum di-commit (katalog `/_dev/ui` sempat hilang karena ini). Commit dulu sebelum men-stub.
- Tera memuat semua template saat start: satu template yang merujuk komponen/layout yang sudah dihapus membuat
  seluruh tes gagal.

- Migration M1 diedit di tempat selama branch belum di-merge. Database dev yang dibuat sebelum M1 di-merge
  perlu `bin/loco db reset` lalu `bin/loco db seed`.

- Rencana milestone: `docs/superpowers/plans/`. Sumber dependency Loco/Tera untuk referensi API ada di `tmp/src/`
  (gitignored; salin ulang dari `/usr/local/cargo/registry/src` di container bila hilang).
- Screenshot UI memakai Playwright dari npm di scratchpad (Playwright MCP tidak bisa start: Chrome tidak terpasang).
- Seed dev M3 (`seed_workflows`) memakai template Superpowers untuk tipe Story di KSR dan membuat KSR-11..13
  (menunggu review, butuh keputusan, eksekusi). Dilewati bila workflow Superpowers sudah ada.
- Seed dev M4 (`seed_agents`): skill pack "Konvensi kasir" (disetujui, terpasang di persona Developer) dan 6 draft
  tiket di KSR (`/w/pt-contoh/p/KSR/drafts`). Uji MCP manual: `claude mcp add --transport http agentgate
  http://localhost:5150/mcp`, lalu `/mcp` → Authenticate.
- Seed dev M5 (`seed_runner`): runner "MacBook Olivia" (owner) dan "ThinkPad Dewi" (dev, offline), KSR-13 di
  stage Eksekusi dengan job berjalan + log + 2 evidence bertanda tangan, job komentar `@agent-qa` yang menunggu
  izin, dan KSR-14 "Struk digital via WhatsApp" dengan job antre. Dilewati bila workspace sudah punya runner.
  Status online runner dihitung dari `last_seen_at`, jadi beberapa menit setelah seed semuanya tampil offline.
- Seed dev M6 (`seed_git`): koneksi GitHub (token palsu) dengan repo `kasir-api` dan `kasir-web` tertaut ke KSR,
  PR #142 (open, CI lulus), #87 (open, CI gagal), #131 (merged) pada tiket KSR-2..4. Uji webhook manual:
  kirim payload bertanda tangan `X-Hub-Signature-256` (HMAC-SHA256 secret koneksi) ke `/webhooks/github/{id}`.
- Seed dev M7 (`seed_graph`): code graph kasir-api (TS, 6 file) dan kasir-web (TS/TSX, 3 file) dengan 3 relasi
  HTTP lintas repo; buka `/w/pt-contoh/p/KSR/graph`. Uji CLI: `agentgate graph index` di repo mana pun yang
  terhubung project. Benchmark repo nyata: `cargo run --release -p agentgate-graph --example repo_graph -- <root> <fungsi…>`.
- Uji CLI manual: `cargo run -p agentgate-cli -- login --server http://127.0.0.1:5150`, lalu `init` di repo
  contoh dan `runner start`.
- Seed dev M8 (`seed_knowledge`): 6 dokumen KSR (dokumen API ditandai basi), 2 usulan (tiket Done untuk Aturan
  Bisnis Refund, scan "Glosarium Retail") dan dokumen workspace "Standar coding perusahaan". Buka
  `/w/pt-contoh/p/KSR/knowledge`. Uji CLI: `agentgate knowledge pull|status|sync` di repo yang terhubung KSR.
- CSS hasil Tailwind (`server/assets/static/css/app.css`) di-gitignore; setelah menambah file CSS komponen jalankan
  `bin/tailwind` (atau `bin/dev` yang watch) sebelum screenshot.
- Seed dev M10 (`seed_visual`): 9 job selesai/gagal 4 minggu terakhir dengan rincian token, biaya, dan file berubah,
  36 baris penghematan, job terakhir KSR-11 (review) dan KSR-12 (keputusan), mockup `checkout-qris.html` di KSR-11.
  Dilewati bila workspace sudah punya data penghematan. Buka `/w/pt-contoh/office`, `/w/pt-contoh/p/KSR/settings/tokens`,
  `/w/pt-contoh/p/KSR/graph/viewer`, dan tab Artifact KSR-11. Pratinjau mockup harus dibuka lewat
  `http://localhost:5150` (bukan `127.0.0.1`), karena `frame-ancestors` domain pratinjau memakai origin aplikasi.
- Seed dev M11: `seed_jira` (koneksi Jira Cloud contoh SR ↔ KSR, dua arah, **dijeda**, status Blocked belum dipetakan,
  8 tautan, 12 log) dan `seed_integrations` (2 token API, 2 webhook + 8 log pengiriman, 1 ekspor siap). Buka
  `/w/pt-contoh/settings/{api,jira,export,import-csv}`. Uji API: buat token di halaman API lalu
  `curl -H "Authorization: Bearer ag_pat_…" http://localhost:5150/api/v1/tickets?project=KSR`. Tes JS: `bin/tests/js_test.sh`
  (butuh node ≥ 18 di host). Screenshot Playwright dari npm di scratchpad (`npm i playwright`).
