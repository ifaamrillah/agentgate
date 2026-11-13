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
  artifact memakai textarea `md_editor` tanpa unggah gambar. Builder berupa daftar vertikal, belum kanvas graf
  dengan panah gagal dan kelompok paralel seperti prototype; belum ada "Izin aksi berbahaya" per stage dan
  belum ada perkiraan "N tiket ikut dimigrasi" di bar simpan. Impor YAML lewat textarea (belum unggah berkas).
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
  Pengaturan "Job paralel maksimal" (`runner_prefs.max_parallel`) disimpan tetapi belum ditegakkan: runner
  menjalankan semua job siap secara paralel. Auto-ambil hanya berlaku saat runner menyala.
- Role custom tidak pernah menjadi "admin di semua project": hanya Owner/Admin bawaan yang boleh menugaskan
  role Admin bawaan (termasuk di grant project) dan mengubah izin role yang dipakai grant project.

## Keputusan sementara (dipakai sampai review akhir pemilik produk)

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

## Perlu dari user (dikumpulkan untuk akhir)

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
- Uji CLI manual: `cargo run -p agentgate-cli -- login --server http://127.0.0.1:5150`, lalu `init` di repo
  contoh dan `runner start`.
