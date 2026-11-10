# Agent Gate — Spesifikasi Produk

Nov 11, 2026 · @Muhammad Ifa Amrillah

## Ringkasan & visi

Agent Gate adalah SaaS manajemen proyek selengkap Jira yang sekaligus menjadi MCP server dan orkestrator AI agent untuk seluruh PDLC (Product Development Life Cycle) sebuah project. Untuk saat ini gratis, tanpa billing.

Bedanya dengan Jira: setiap tiket dikerjakan bersama AI agent per tahapan (brainstorming, PRD, tech spec, development, testing, deploy). Setiap tahapan menghasilkan artifact, dan aturan tiap tahapan ditegakkan keras lewat gate.

AI yang bekerja adalah AI milik masing-masing user (Claude Code, Codex, dan lainnya). Biaya token ditanggung user. Agent Gate tidak menjalankan LLM untuk pekerjaan project.

Kemampuan inti:

- Board tiket, sprint, roadmap, laporan, dan fitur manajemen proyek ala Jira.
- Workflow PDLC yang bisa dikustom penuh, dengan template metodologi populer.
- Eksekusi otomatis lewat runner lokal saat tiket digeser atau di-approve.
- Knowledge base yang selalu diperbarui (living document), dibaca manusia dan AI.
- Code graph lintas repo beserta viewer visual.
- AI Office: kantor isometrik yang memvisualisasikan agent dan manusia yang sedang bekerja.
- Aturan Agent Gate menggantikan aturan bawaan project (CLAUDE.md, skills, dan sejenisnya), supaya project dan Agent Gate selaras.

Semua fitur di dokumen ini masuk MVP. Tidak ada pembagian fase.

## Prinsip utama

1. **AI milik user (BYO AI).** Semua pekerjaan AI berjalan di client milik user. Server Agent Gate hanya menyimpan, mengatur, dan memverifikasi.
2. **Penegakan keras.** Aturan workflow ditegakkan oleh gate di server, bukan bergantung pada kepatuhan AI. Penegakan lokal (hooks, aturan) menjadi lapis tambahan.
3. **Manusia memegang kendali.** Setiap transisi bisa diatur wajib approve manusia. Hasil AI pada knowledge dan skill pack masuk sebagai usulan yang harus di-approve.
4. **Fleksibel di semua level.** Workflow, persona, role, jenis dokumen knowledge, format kode tiket, dan assignment bisa dikustom.
5. **Bisa dibaca manusia dan AI.** Tiket, artifact, dan knowledge berformat Markdown yang rapi di UI dan mudah dikonsumsi AI lewat MCP.
6. **Hemat token.** Konteks diambil seperlunya lewat MCP dan code graph, output perintah dikompresi, dan serah terima antar stage ringkas.
7. **Siap self-host.** Saat ini hanya SaaS, tetapi dependensi dibatasi ke komponen yang bisa di-self-host.

## Arsitektur & tech stack

Agent Gate dibangun dalam Rust (framework Loco) dengan frontend HTMX, terdiri dari server SaaS dan CLI `agentgate` yang berjalan di laptop user.

&#91;embedded content: arsitektur Agent Gate · server SaaS dan laptop user\]

Manusia memakai web app. Client AI di laptop user memanggil Remote MCP, dan runner menerima job serta mengirim log dan evidence lewat WebSocket. Semua transisi tiket divalidasi oleh API + Gate Engine.

Komponen server:

- **Web app**: board, dokumen, graph viewer, AI Office.
- **API + gate engine**: validasi setiap transisi tiket.
- **Event bus**: realtime ke browser dan runner.
- **Remote MCP server**: diakses client AI lewat OAuth 2.1.
- **Worker**: antrean job, indexing graph, embedding.

Komponen lokal (`agentgate` CLI):

- **Installer**: memasang dan menyinkronkan aturan ke repo.
- **Indexer**: tree-sitter + SCIP.
- **Runner**: menerima job dan menjalankan client AI secara headless atau interaktif.

| Lapisan | Teknologi |
| --- | --- |
| Backend | Rust edition 2024, Loco 1.3 (Axum + SeaORM 2.0), background worker dan scheduler Loco |
| Frontend | Tera 2 + HTMX 4 (`hx-boost`, `hx-preload`, `hx-sse`/`hx-ws`), Tailwind CSS v4 standalone CLI, UI kit Basecoat, ikon Tabler. Tanpa Node dan tanpa bundler |
| Komponen JS khusus | SortableJS (drag-drop board), Sigma.js/Cytoscape (graph), PixiJS (kantor isometrik), Mermaid, editor WYSIWYG. Semua di-vendor dengan versi dipin dan dimuat lewat import map. Interaksi kecil memakai JS vanilla per perilaku dengan atribut `data-*` |
| MCP | `rmcp` (SDK Rust resmi), transport streamable HTTP, di-mount ke router Axum |
| Database | Postgres 18 + pgvector |
| Queue & realtime | Queue Postgres bawaan Loco, realtime lewat `LISTEN/NOTIFY` Postgres. Tanpa Redis |
| Penyimpanan file | Storage kompatibel S3 lewat `ctx.storage` Loco (SeaweedFS untuk self-host) |
| Parsing kode | Crate `tree-sitter` (server + CLI), indexer SCIP (CLI) |
| Embedding | `fastembed-rs` (ONNX lokal di server) |
| Binary | `agentgate-server` (server Loco), `agentgate` (CLI/runner untuk user), single binary untuk Windows, macOS, Linux |
| Email | Mailer Loco lewat job, SMTP yang bisa dikonfigurasi |

Kode disusun sebagai Cargo workspace sejak awal, karena server dan CLI berbagi kode:

- `server`: aplikasi Loco. Aturan arah dependensi antar-layer (controllers, services, models, views, common) berlaku di dalam crate ini.
- `cli`: binary `agentgate` (installer, indexer, runner).
- `core`: type bersama, parser workflow, dan protokol runner.
- `graph`: parser tree-sitter dan pembangun graph. Dipisah karena grammar tree-sitter berat di-compile, dan dipakai server maupun CLI.

Aturan supaya siap self-host: dependensi hanya Postgres dan storage S3-compatible. Semua konfigurasi lewat environment variable. Multi-tenant lewat kolom `workspace_id` di setiap tabel domain, di-resolve sekali oleh extractor, dengan Row Level Security Postgres sebagai pertimbangan lapis kedua.

Konvensi coding mengikuti dokumen `loco-project-rules.md` milik tim, dengan penyesuaian berikut:

- Bagian yang di dokumen itu ditunda tetapi dibutuhkan Agent Gate sejak awal mengikuti panduan "Ditunda" di dokumen tersebut: multi-tenancy, audit log, realtime, login Google/GitHub (OIDC), dan 2FA.
- Modul opsional Vector (pgvector) dan Storage (S3) dipakai.
- Aturan tambahan diperlukan untuk hal yang belum dibahas dokumen itu: OAuth 2.1 server untuk MCP, device code flow untuk runner, dan API token publik. Semua token disimpan sebagai digest.

UI berbahasa Indonesia, web responsif, tanpa dark mode.

## Organisasi, tim & akses

Hierarkinya Workspace → Project → Repo. Tim adalah kelompok orang yang diberi akses ke project, mengikuti cara Jira.

- **Workspace** = perusahaan atau organisasi. Satu user bisa ikut beberapa workspace.
- **Tim** = kelompok orang, misalnya Tim Backend atau Tim QA. Satu tim bisa mengerjakan beberapa project, dan satu project bisa diakses beberapa tim.
- **Project** memberi akses ke tim atau ke orang tertentu.
- **Repo**: satu project bisa terdiri dari beberapa repo (misalnya frontend, backend, mobile).

Ada tiga jenis role, semuanya bisa dikustom:

| Jenis | Fungsi | Kustomisasi |
| --- | --- | --- |
| Hak akses | Menentukan siapa boleh melakukan apa | Role bawaan: Owner, Admin, Member, Viewer. Bisa berbeda per project. Owner/Admin workspace otomatis menjadi Admin di semua project |
| Jabatan tim | Label seperti PM, Dev, QA, Designer. Dipakai untuk default assignment | Bebas dibuat |
| Persona agent | Peran AI di tiap stage | Lihat bagian Persona agent |

Pendaftaran terbuka seperti Jira: siapa pun bisa daftar dan membuat workspace sendiri. Bergabung ke workspace orang lain hanya lewat undangan email. Login web memakai email + password, Google, atau GitHub. 2FA (authenticator app) opsional per user, dan bisa diwajibkan oleh Owner.

Semua data dibatasi per workspace (multi-tenant). Saat ini tidak ada kuota penyimpanan, jumlah project, maupun retensi evidence.

## Tiket, board & manajemen proyek

Agent Gate mencakup fitur inti Jira, ditambah estimasi dari AI dan tampilan roadmap serta kalender.

**Tiket**

- Tipe: Epic, Story, Task, Bug, Subtask, plus tipe custom.
- Field standar (judul, deskripsi rich text, assignee, reporter, prioritas, label, due date, story point) dan field custom.
- Komentar, @mention orang dan tiket, watcher, lampiran.
- Relasi antar tiket (blocks, duplicates, relates to). Relasi *blocks* ditegakkan: tiket yang diblokir tidak bisa mulai sebelum tiket pemblokir selesai.
- Riwayat aktivitas, clone, bulk edit.
- Subtask punya workflow sendiri yang lebih ringkas, misalnya Dev → Review → Done.
- Hapus tiket masuk sampah dulu, lalu terhapus permanen setelah 30 hari.

**Kode tiket** memakai template yang bisa dikustom penuh, misalnya `{PROJECT}-{NUM}` menjadi `KSR-123`, atau `{TEAM}-{YYYY}-{NUM:4}` menjadi `BE-2026-0042`. Default diatur di level workspace dan bisa ditimpa per project. Penghitung nomor bisa per project, per tipe, atau per tahun. Saat format diubah, kode lama tetap berlaku sebagai alias.

**Tampilan & perencanaan**

- Board Kanban dan Scrum dengan WIP limit, swimlane, dan quick filter.
- Backlog dan sprint (start/complete).
- Tampilan list/tabel, kalender, dan roadmap/timeline (Gantt).
- Release/version, lengkap dengan release notes yang dibuat AI.
- Components, terhubung ke code graph.
- Pencarian lanjutan dengan bahasa query ala JQL, dan saved filter.
- Laporan: burndown, burnup, velocity, sprint report, cumulative flow.
- Audit log.

**Estimasi**

- AI mengusulkan story point (kompleksitas) dan estimasi waktu (durasi) saat draft tiket dibuat. Dasarnya: kompleksitas, kode yang terdampak, dan riwayat tiket serupa.
- Estimasi waktu dipecah per stage menjadi waktu kerja AI dan waktu kerja manusia. Manusia bisa mengedit angkanya.
- Setelah tiket Done, estimasi dibandingkan dengan waktu aktual untuk mengkalibrasi estimasi berikutnya di project yang sama.
- Durasi tiap job dan lama tiket di tiap stage (cycle time) tercatat otomatis. Tidak ada time tracking manual.
- Estimasi mengisi roadmap dan kalender.

**Brainstorming menjadi tiket.** Hasil brainstorming muncul sebagai draft tiket yang harus disetujui. Hierarkinya ditentukan AI sesuai ukuran tugas: Task tunggal, Story + Subtask, atau Epic → Story → Subtask. Di layar review, draft bisa diedit, digabung, dipecah, atau dihapus. Repo yang terdampak dideteksi AI dari code graph, lalu dikonfirmasi manusia.

Notifikasi saat ini hanya di dalam aplikasi.

## Workflow, stage & gate

Workflow dibuat lewat workflow builder di web, dan setiap tipe tiket boleh punya workflow sendiri. Workflow disimpan di database dan bisa diekspor/impor sebagai YAML.

**Isi definisi tiap stage:**

- Nama dan persona agent.
- Mode **otomatis** (job jalan di runner tanpa diawasi) atau **interaktif** (butuh percakapan dengan manusia, misalnya brainstorming).
- Prompt/skill, artifact input dari stage sebelumnya, dan template artifact output.
- Gate rules dan tooling wajib (misalnya Playwright CLI).
- Default executor, approver, dan watcher.
- Mode transisi keluar: otomatis, atau tunggu approve manusia.
- Batas putaran gagal (default 3).

Stage bisa berjalan paralel, misalnya UI/UX Designer dan Architect bersamaan setelah PRD.

**Gate** dipilih dari tipe cek yang sudah disediakan, bukan skrip bebas. Contohnya: artifact ada, screenshot ada, hasil test lulus, PR terbuka, semua PR merged, CI hijau. Transisi ditolak server kalau gate belum terpenuhi. Evidence (screenshot, hasil test) dihasilkan dan ditandatangani oleh runner, bukan sekadar klaim AI.

**Assignment per stage.** Setiap stage pada tiket punya daftar executor, approver, dan watcher. Jumlahnya bisa satu atau lebih orang, dan satu orang boleh memegang semua peran. Cukup satu approver yang approve.

**Alur review.** Job selesai → artifact jadi → status "Menunggu Review". Approver bisa:

- **Approve**: tiket pindah, dan stage berikutnya langsung dipicu.
- **Edit langsung** artifact, lalu approve.
- **Reject + catatan**: AI merevisi di stage yang sama.

&#91;embedded content: siklus satu stage · gate, review, approve atau reject\]

Kalau mode transisi sebuah stage diatur otomatis, langkah approve dilewati, tetapi gate tetap dicek.

Setiap revisi artifact disimpan sebagai versi (v1, v2, v3) yang bisa dibandingkan.

**Testing gagal.** Tiket otomatis kembali ke Development dengan laporan bug (screenshot, langkah reproduksi). Setelah diperbaiki, tiket dites ulang. Setelah 3 kali gagal (bisa diubah per stage), putaran berhenti dan tiket berstatus **Butuh Keputusan**. AI menulis ringkasan penyebab, dan manusia memilih salah satu:

1. Beri petunjuk, lalu lanjutkan. Hitungan gagal kembali ke 0.
2. Perbaiki sendiri, lalu lanjut testing.
3. Ubah requirement: tiket kembali ke stage PRD.
4. Terima dengan catatan: tiket lanjut, dan tiket Bug terpisah dibuat otomatis.
5. Batalkan tiket.

**Lompat stage** hanya boleh oleh Admin, wajib dengan alasan, dan tercatat di audit. Admin juga bisa memaksa transisi dengan alasan yang tercatat di audit log.

**Migrasi workflow.** Saat workflow diubah, tiket yang berjalan ikut dimigrasi. Kalau ada stage yang dihapus, muncul layar pemetaan untuk memilih stage tujuan tiket-tiket di stage itu. Artifact lama tetap tersimpan. Gate versi baru berlaku sejak tiket masuk ke stage berikutnya.

**Template workflow bawaan** (bisa di-clone dan diubah): Scrum, Kanban, BMAD, Spec-driven, Shape Up, Bug Triage, Waterfall, Superpowers-style (Brainstorm → Plan → Eksekusi bertahap + TDD → Review), dan Hotfix/Incident (Triage → Fix → Deploy → Postmortem).

## Persona agent, skill pack & model

Persona agent adalah peran AI di tiap stage. Semua persona bisa diubah, dihapus, atau ditambah.

**Persona bawaan:**

| Persona | Tugas utama |
| --- | --- |
| Analyst | Riset dan brainstorming |
| Product Manager | PRD |
| UI/UX Designer | Alur, wireframe, dan UI mockup berupa prototype HTML |
| Architect | Tech spec |
| Developer | Implementasi kode |
| Code Reviewer | Review PR |
| QA | Testing (Playwright + screenshot) |
| DevOps | Deploy |
| Tech Writer | Knowledge dan release notes |

**Isi kustomisasi persona:** nama dan avatar karakter di kantor isometrik, instruksi/prompt, skills, tools yang diizinkan, izin aksi berbahaya, dokumen knowledge yang boleh dibaca, opsi "gaya ringkas", dan model per client.

**UI mockup** berupa prototype HTML yang bisa diklik dan dilihat langsung di tiket, plus screenshot otomatis. Mockup menyesuaikan jenis project (web, mobile, atau keduanya) dan mengikuti dokumen knowledge "Komponen UI & Design System". Mockup dipakai Agent Dev sebagai acuan, dan dipakai Agent QA untuk membandingkan screenshot hasil jadi.

**Model per persona (opsional).** Setting persona berisi model dan effort per client, dipilih dari dropdown katalog yang dikelola Agent Gate, atau diketik bebas untuk model baru. Kalau kosong, dipakai default client. Setting ini bersifat default dan rekomendasi. User boleh menurunkan model atau effort, dan penurunan itu tercatat di tiket. Tidak ada pemetaan tingkat model.

**Skill pack** bisa diimpor dari URL repo Git (format SKILL.md), misalnya skill resmi Anthropic, Superpowers, Agent Skills milik Addy Osmani, atau agent BMAD. Alur impornya:

1. Admin memasukkan URL repo.
2. Review AI otomatis berjalan di runner Admin yang mengimpor. Yang dicek: perintah berbahaya, prompt injection, permintaan akses secret, dan tools yang diminta. Hasilnya laporan risiko: Aman, Perlu Perhatian, atau Berbahaya.
3. Admin membaca laporan dan isi skill, lalu Approve atau Tolak.
4. Hanya skill yang sudah di-approve yang bisa dipasang ke persona.
5. Pembaruan dari repo sumber tidak otomatis berlaku, dan harus melalui review yang sama.

## Runner & eksekusi job

Job (AI mengerjakan satu stage untuk satu tiket) selalu berjalan di runner personal milik executor, dengan langganan AI milik executor tersebut. Tidak ada shared runner.

**Menyalakan runner.** Runner tidak pernah menyala otomatis. User menyalakannya secara manual (misalnya `agentgate runner start`), dan selalu ada konfirmasi saat menyalakan. Login runner memakai device code flow (`agentgate login`). Runner terhubung ke server lewat WebSocket.

**Model klaim.**

1. Saat stage dipicu, job diumumkan ke semua runner milik executor stage itu, dan semua executor mendapat notifikasi.
2. Executor pertama yang klik "Ambil" (dari web atau CLI) mendapat lock, dan job jalan di runner-nya.
3. Kalau tidak ada yang mengambil, job tetap di antrean dengan pengingat berkala. Kalau runner offline, job menunggu sampai runner menyala.
4. Executor bisa melepas job yang sudah diambil, dan job kembali ke antrean.

Ada setting opsional "auto-ambil" per user: selama runner menyala dan idle, job langsung diambil.

**Eksekusi.**

- Satu git worktree per tiket, supaya beberapa job bisa paralel. Tidak ada batas default jumlah job paralel. User boleh membatasinya sendiri lewat setting.
- Urutan antrean mengikuti prioritas tiket.
- Job otomatis memakai client secara headless (`claude -p`, `codex exec`, `gemini -p`, `cursor-agent`, `opencode run`, Copilot CLI). Client dipilih user di setting runner.
- Stage interaktif: runner membuka jendela terminal berisi sesi client yang sudah memuat konteks tiket. Kalau terminal gagal dibuka, executor mendapat notifikasi untuk menjalankan perintah `/ag-...` sendiri. Program terminal bisa diatur per user dan per OS.
- Sebelum job mulai, runner menulis aturan terbaru dari server ke worktree, sehingga job tidak tertahan oleh PR sinkronisasi aturan yang belum di-merge.
- Runner melaporkan progress, log, dan evidence secara realtime. Data ini dipakai oleh AI Office.
- Untuk job otomatis, runner menjalankan client dengan flag isolasi (misalnya `--setting-sources project`, `--strict-mcp-config` di Claude Code), supaya config global user tidak ikut berlaku.
- Worktree otomatis dihapus setelah PR di-merge atau tiket Done.
- Yang boleh membatalkan job yang sedang berjalan: executor dan Admin project.

**Izin aksi berbahaya.** Job otomatis tidak diawasi manusia, jadi izinnya ditentukan dari awal:

- Beberapa aksi selalu diblokir untuk semua persona, misalnya force push ke branch utama atau menghapus database production.
- Aksi lain diatur per persona di workflow builder. Contoh: QA hanya membaca kode dan menjalankan Playwright, Dev tidak boleh deploy, DevOps boleh deploy.
- Kalau AI menabrak aksi yang diblokir, job dijeda dan executor mendapat permintaan izin ("Agent Dev ingin menjalankan `prisma migrate deploy`. Izinkan atau Tolak"). Setelah dijawab dari web atau CLI, job lanjut.

**Kuota AI user habis.** Pilihannya: tunggu limit reset lalu lanjut di runner yang sama, atau lepas ke executor lain lewat model klaim. Default diatur per user dan bisa ditimpa per job.

**@agent di komentar.** Komentar seperti "@agent-qa tolong cek ulang" memicu job di runner milik penulis komentar. Hanya Member ke atas yang bisa memicu. Job memakai branch tiket di worktree terpisah, dan hasilnya dibalas sebagai komentar. Kalau runner penulis offline, komentar ditandai "menunggu runner kamu online".

**Job tanpa tiket** (update knowledge, scan ulang) dijalankan di runner knowledge owner project. Kalau runner-nya offline, job dilempar ke model klaim.

## MCP, slash command & aturan project

Aturan di repo dibuat setipis mungkin. Isi aturan disajikan dari server lewat MCP saat dibutuhkan.

**Client yang didukung:** Claude Code, Codex CLI, Gemini CLI, Cursor, OpenCode, dan GitHub Copilot CLI.

**File aturan di repo:**

- `AGENTS.md` adalah satu-satunya file utama, berisi pembuka singkat: project memakai Agent Gate, dan AI wajib memanggil `ag_context` sebelum mulai kerja. File ini dibaca langsung oleh Codex, OpenCode, Cursor, dan Copilot.
- `CLAUDE.md` hanya berisi satu baris `@AGENTS.md`.
- Gemini CLI diarahkan ke `AGENTS.md` lewat setting `contextFileName`.
- Konfigurasi koneksi MCP dan hooks per client tetap diperlukan (`.mcp.json`, `.codex/`, `.gemini/settings.json`, `.cursor/`, dan sejenisnya). Isinya kecil, di-generate otomatis, dan tanpa logika: hooks hanya memanggil `agentgate hook ...` supaya jalan di semua OS.
- Instruksi per stage, prompt persona, dan skills diambil dari server lewat MCP, tidak disimpan sebagai file.
- Di Claude Code, hook `SessionStart` menyuntikkan konteks Agent Gate secara otomatis.

**Pemasangan pertama (`agentgate init`) di project lama.** Semua aturan lama diganti total, lalu langsung di-commit:

- CLAUDE.md dan AGENTS.md di semua level folder, `.claude/skills`, `.claude/commands`, `.claude/agents`, `.claude/settings.json`, `.mcp.json`, `.cursor/rules`, dan file sejenis dari client lain dibackup lalu dihapus.
- Backup disimpan sebagai arsip `.agentgate/backup/<timestamp>.tar.gz` beserta manifest, ditambah salinan di server. Bentuk arsip mencegah CLAUDE.md lama ikut terbaca client.
- Isi aturan lama (konvensi, cara run, struktur folder) diserap ke dokumen knowledge "Konvensi Project", supaya pengetahuannya tidak hilang.
- File aturan Agent Gate dan `.agentgate/project.toml` (ID project dan repo) ikut di-commit, sehingga seluruh tim memakai aturan yang sama.
- `agentgate uninstall` mengembalikan file lama dari backup, lalu membuat commit baru.

**Sinkronisasi aturan.** Kalau file pembuka atau hooks berubah, CLI membuat PR "Update aturan Agent Gate" dari branch `agentgate/sync-rules`. Perubahan workflow biasa tidak perlu PR, karena aturannya dibaca langsung dari server. Runner mendeteksi drift lewat checksum. Job tetap memakai aturan terbaru dari server.

**Coding tanpa tiket tetap diizinkan.** Perubahannya tetap masuk ke graph dan knowledge saat di-merge.

**Slash command.** Satu command per stage dibuat otomatis dari workflow (`/ag-brainstorming`, `/ag-prd`, `/ag-testing`, dan seterusnya), ditambah command utilitas:

| Command | Fungsi |
| --- | --- |
| `/ag-status` | Status tiket |
| `/ag-ticket` | Pilih atau buat tiket aktif |
| `/ag-next` | Ambil job berikutnya |
| `/ag-search` | Cari di knowledge dan graph |
| `/ag-knowledge-sync` | Perbarui knowledge |

**Prinsip MCP.** Jumlah tool dijaga sekitar 10–15. Respons ringkas dan berlapis: ringkasan dulu, detail diambil saat diminta. Auth memakai OAuth 2.1.

## Git, code review, testing & deploy

Agent Gate terhubung ke GitHub, GitLab, dan Bitbucket, termasuk versi perusahaan: GitHub Enterprise Server, GitLab Self-Managed, dan Bitbucket Data Center.

**Hasil kode stage Development** diatur per workflow/stage:

- **Manual**: AI hanya mengubah file. Commit, push, dan PR dikerjakan manusia.
- **PR otomatis**: runner membuat branch, commit, push, lalu membuka PR. Link PR menempel di tiket.

Tiket multi-repo menghasilkan satu branch + PR per repo, dengan gate "semua PR merged".

**Merge PR** diatur per workflow: manual oleh manusia, atau otomatis saat stage Code Review di-approve.

**Konvensi git** (bisa dikustom per project):

- Nama branch: `ag/KSR-123-varian-produk`.
- Pesan commit: Conventional Commits (`feat: ...`, `fix: ...`).
- Bahasa commit message, deskripsi PR, dan komentar kode diatur per project.

**Code review oleh AI** ditulis di dua tempat: komentar per baris di PR, dan artifact review di tiket.

**Environment testing** diatur per repo atau workflow. Default-nya lokal: runner menyalakan aplikasi di laptop executor (perintah start diatur per repo), lalu Agent QA mengetes di `localhost`. Alternatifnya, testing ke URL staging atau preview per PR.

**Deploy** bersifat opsional. Workflow boleh berhenti di merge, atau punya stage Deploy ke staging/production. Stage Deploy memakai tools dan akses di laptop executor (misalnya `kubectl`, `vercel`, `docker`).

**Git versi perusahaan** biasanya berada di balik firewall dan tidak terjangkau server Agent Gate. Untuk project seperti ini, runner menjadi jembatan: runner mengecek status PR dan CI, lalu melaporkannya ke server. Gate PR/CI baru terverifikasi saat ada runner yang online.

**Kode ke server** bisa lewat dua jalur, dipilih per repo: server meng-clone langsung dari provider cloud, atau CLI meng-upload dari laptop (bisa untuk semua provider, termasuk versi perusahaan).

## Knowledge base

Knowledge adalah living document hasil scan project. Sumber kebenarannya di repo (`.agentgate/knowledge/`, ber-versi git), dengan mirror di server untuk UI dan pencarian.

**Scan.**

- Scan awal dipilih kedalamannya (cepat, standar, mendalam), dengan estimasi biaya token sebelum mulai.
- Scan ulang manual bisa dilakukan kapan saja untuk satu project, repo, atau folder, termasuk untuk menaikkan kedalaman.
- Biaya scan memakai token AI milik orang yang menjalankannya.

**Update otomatis** (kombinasi):

- Setiap tiket Done: update inkremental pada dokumen yang tersentuh, dihitung dari diff graph.
- Setiap merge ke branch utama: re-index graph, lalu update dokumen yang terdampak.
- Mingguan: audit penuh untuk mendeteksi dokumen yang basi.
- Manual: tombol di web atau `/ag-knowledge-sync`.

Semua perubahan dari AI, termasuk hasil scan ulang, masuk sebagai usulan (diff) yang harus di-approve knowledge owner sebelum menimpa dokumen. Manusia bisa mengedit dokumen secara langsung.

**Katalog dokumen.** AI hanya membuat dokumen yang relevan dengan project. Kedalaman scan menentukan tingkat detail, bukan jumlah jenis dokumen.

| Kelompok | Dokumen |
| --- | --- |
| Gambaran umum | Overview Project, Peta Repo, Tech Stack & Dependencies, Glosarium |
| Arsitektur | Arsitektur Sistem, Alur Data, Integrasi Pihak Ketiga, ADR |
| Domain & bisnis | Alur Bisnis / User Journey, Aturan Bisnis, Siklus Status, Role & Permission aplikasi |
| Backend & kode | Modul/Service (per modul), Model Data & Database, Riwayat Migration, API, Event/Queue/Job/Cron, Shared Library, Error Handling & Logging |
| Frontend | Halaman & Routing, Komponen UI & Design System, State Management, i18n |
| Mobile | Screen & Navigasi, Fitur khusus platform, Build & rilis ke store |
| Kualitas | Strategi Testing, Konvensi & Coding Style, Security, Performance, Technical Debt |
| Operasional | Setup & Cara Menjalankan, Konfigurasi & Env Var (tanpa nilai secret), Build & CI/CD, Deployment & Infrastruktur, Monitoring, Runbook |
| Data | Data Pipeline/ETL, Analytics Event, Reporting |
| Riwayat | Changelog per rilis, Peta Fitur (fitur ↔ kode ↔ tiket) |

Admin bisa menambah jenis dokumen sendiri, lengkap dengan template dan instruksi untuk AI. Ada juga knowledge level workspace untuk standar yang berlaku di semua project, misalnya standar coding perusahaan dan kebijakan keamanan.

**Format & editor.** Dokumen berformat Markdown + diagram Mermaid. Editornya mendukung:

- WYSIWYG dan mode Markdown mentah (bisa bolak-balik).
- Preview live Mermaid.
- Tabel, code block dengan highlight, checklist.
- Paste/upload gambar.
- Slash command `/` untuk menyisipkan blok.
- Komentar inline.
- @mention orang dan tiket, serta link antar dokumen.

Tidak ada edit bersama realtime.

**Bahasa** artifact dan knowledge: Indonesia. Istilah teknis yang umum tetap dalam bahasa Inggris.

## Code graph

Code graph Agent Gate dibangun sendiri di Rust dan harus minimal setara codebase-memory dan Graphify. Benchmark terhadap keduanya pada repo yang sama menjadi syarat sebelum fitur ini dianggap selesai.

**Pembangunan graph.**

- Tree-sitter dipakai untuk semua bahasa, di server dan CLI, tanpa token AI. Kualitas diprioritaskan untuk TS/JS, PHP, Python, Go, Java/Kotlin, Rust, Dart, dan C#.
- Indexer SCIP dijalankan di laptop user lewat CLI, untuk bahasa yang punya indexer SCIP. Tujuannya resolusi simbol yang presisi. Server cukup memakai tree-sitter.
- Update inkremental per file yang berubah.
- Graph lintas repo: HTTP call ↔ route, event/queue, gRPC, dan shared package.
- Embedding untuk semantic search memakai `fastembed-rs`.

**Kemampuan.**

| Kemampuan | Keterangan |
| --- | --- |
| Struktur dasar | Simbol, call graph, import, inheritance |
| Trace | Telusur pemanggil dan yang dipanggil |
| Analisis dampak | Dari git diff atau dari sebuah simbol |
| Query lanjutan | Bahasa query ala Cypher |
| Kualitas kode | Deteksi dead code, cek cakupan index |
| Clustering | Modul terdeteksi otomatis, plus node pusat |
| Keyakinan relasi | Setiap relasi ditandai pasti (hasil parsing) atau dugaan (hasil inferensi) |
| Dokumen non-kode | PRD, knowledge, dan artifact ikut menjadi node |
| Keterlacakan | Node kode terhubung ke tiket, PR, artifact, test evidence, dan dokumen knowledge (termasuk deteksi dokumen yang basi) |
| Riwayat git | Hotspot, file yang selalu berubah bersamaan (coupling), pemilik kode |
| Semantic search | Misalnya mencari "logika perhitungan diskon" walaupun nama fungsinya `calcPromo` |

Graph disajikan ke AI lewat tool MCP seperti `ag_search`, `ag_trace`, dan `ag_snippet`. Tidak perlu memasang Graphify atau codebase-memory terpisah.

**Graph viewer** di web menyediakan:

- Zoom bertingkat: project → repo → modul → file → fungsi.
- Highlight kode yang tersentuh oleh satu tiket.
- Analisis dampak.
- Pencarian.
- Filter berdasarkan bahasa atau layer.

## AI Office

AI Office adalah kantor isometrik (PixiJS): satu kantor per workspace, dengan satu ruangan per project. Datanya berasal dari laporan realtime runner.

- Yang tampil: avatar persona agent (PM, QA, dan lainnya) dan avatar manusia pemilik runner. Avatar manusia muncul di ruangan project tempat job-nya sedang berjalan.
- Klik karakter untuk melihat log kerja secara live.
- Klik karakter untuk langsung approve atau reject.
- Ruang rapat terisi saat brainstorming berlangsung.
- Agent yang menganggur duduk di pantry.
- Karakter bisa dikustom.
- Notifikasi melayang di atas kepala karakter saat butuh approval, dan tanda merah saat tiket berstatus Butuh Keputusan.

**Brainstorming beberapa orang** dilakukan bergantian. Satu orang brainstorming lebih dulu, dan hasilnya tersimpan sebagai catatan brainstorm. Orang berikutnya menjalankan `/ag-brainstorming <kode tiket>` di client-nya sendiri, dan AI melanjutkan dari catatan itu. Tiket dikunci selama satu sesi brainstorming aktif.

## Penghemat token

Semua penghematan dikumpulkan di halaman "Penghemat Token" per project, berisi toggle per fitur dan statistik penghematan. Pemakaian token dan perkiraan biaya per job dan per tiket ditampilkan, dengan data dari client user.

| Fitur | Status | Cara kerja |
| --- | --- | --- |
| Code graph built-in | Wajib | AI menjelajah lewat MCP graph, bukan grep atau membaca file utuh |
| RTK | Wajib sejak MVP | Output perintah (test log, `git status`, build log) dikompresi lewat hook. CLI memastikan binary RTK terpasang di semua OS. Kalau hook suatu client tidak mendukung, AI diinstruksikan memakai `rtk` lewat aturan MCP |
| Gaya ringkas | Opsi per persona | Hanya untuk chat dan log. Artifact (PRD, test case, knowledge) tetap ditulis lengkap |
| Aturan lewat MCP | Wajib | Instruksi diambil per stage, bukan semua aturan sekaligus |
| Serah terima ringkas | Wajib | Agent Dev menerima ringkasan PRD + AC + mockup, bukan transkrip brainstorming |
| Respons MCP berlapis | Wajib | Ringkasan dulu, detail diambil hanya saat diminta |
| Prefix konteks stabil | Wajib | Supaya prompt cache client sering kena |
| Model per persona | Opsional | Model murah untuk tugas ringan |

Tidak dipakai: Headroom atau proxy kompresi level API, dan anggaran token per job.

## Integrasi Jira & API publik

Integrasi pihak ketiga didesain sebagai sistem **Connector**. Connector pertama dan satu-satunya saat ini adalah Jira.

**Koneksi Jira** memakai email + API token untuk Jira Cloud, atau Personal Access Token untuk Jira Data Center. Ada tiga mode:

1. **Impor sekali**: tiket, komentar, lampiran, user, sprint, dan pemetaan status ke workflow dipindahkan ke Agent Gate.
2. **Mirror satu arah** (Agent Gate → Jira): Jira menjadi cermin yang hanya untuk dilihat.
3. **Sinkron dua arah**. Kalau terjadi bentrok, status/stage selalu mengikuti Agent Gate karena gate ditegakkan di sana. Field lain mengikuti perubahan terakhir.

Pemetaannya mencakup status Jira ↔ stage Agent Gate, field ↔ field, dan user ↔ user. Impor dari CSV juga didukung.

**API publik.** REST API JSON dengan API token, ditambah webhook untuk integrasi pihak ketiga.

**Ekspor data.** Seluruh data workspace (tiket, artifact, knowledge) bisa diekspor ke JSON + Markdown.

## Keamanan, privasi & akun

- **Upload kode.** CLI tidak pernah meng-upload file yang di-ignore oleh `.gitignore` maupun file rahasia (`.env`, private key, credential). Sebelum upload, ada pemindaian secret otomatis. File yang terdeteksi dilewati dan dilaporkan.
- **Remote trigger.** Runner hanya menerima job untuk stage yang executor-nya adalah pemilik runner, atau job dari komentar @agent milik pemilik runner sendiri. Semua job tercatat di audit log.
- **Evidence** dihasilkan dan ditandatangani runner, sehingga tidak bisa dipalsukan oleh AI.
- **Mockup HTML** ditampilkan di iframe sandbox dengan domain terpisah (misalnya `*.agentgate-preview.app`), supaya script di dalamnya tidak bisa mengakses session atau data Agent Gate. Domain preview ditambahkan ke frame-src di CSP aplikasi.
- **Skill pack** wajib melalui review AI + manusia sebelum aktif.
- **Config global user** diisolasi saat job otomatis lewat flag client.
- **Auth**: email + password, Google, GitHub. 2FA opsional dan bisa diwajibkan Owner. MCP memakai OAuth 2.1, runner memakai device code flow.
- **Email** (undangan, verifikasi, reset password) dikirim lewat SMTP yang bisa dikonfigurasi.
- **Hapus workspace** memakai masa tenggang 30 hari dan masih bisa dibatalkan selama masa itu. Tiket yang dihapus masuk sampah selama 30 hari.
- **Audit log** mencatat transisi paksa, lompat stage, job, approval, dan perubahan workflow.

Keamanan web mengikuti konvensi Loco tim: CSP ketat (`script-src 'self'`, tanpa inline script maupun `hx-on`), proteksi CSRF lewat pengecekan header `Sec-Fetch-Site`/`Origin`, sesi server-side di database (bukan JWT), password dengan argon2id, dan rate limit login di tabel Postgres.

## Repo, hosting & distribusi

**Repo** Agent Gate ada di GitHub. Strategi branch:

- `staging` adalah branch default dan branch integrasi, di-deploy ke staging.
- `release` hanya berisi yang sudah lolos staging, di-deploy ke production. Promosi dilakukan dengan fast-forward, sehingga image yang diuji di staging dipakai ulang di production.
- Fitur dan perbaikan di `feature/*` dan `fix/*`. Hotfix dari `release`, lalu di-merge balik ke `staging` di hari yang sama.
- Tidak ada branch `main`, dan tidak ada push langsung ke `staging` atau `release`.

**Hosting** di VPS sendiri (provider belum dipilih), memakai Docker Compose:

- Container `web` dan `worker` (plus scheduler) dari image yang sama.
- Postgres 18 + pgvector.
- Storage S3-compatible: SeaweedFS di VPS, atau layanan S3-compatible eksternal.
- Reverse proxy dengan TLS otomatis dan dukungan WebSocket/SSE.
- Domain terpisah untuk preview mockup.
- Migrasi dijalankan sebagai langkah terpisah sebelum container baru naik.
- Backup Postgres terjadwal ke storage di luar VPS.

Paket Compose yang sama nantinya menjadi paket self-host.

**Distribusi CLI `agentgate`** memakai `dist` (cargo-dist). Binary untuk Windows, macOS, dan Linux dibangun lewat GitHub Actions dan diunggah ke GitHub Releases.

- macOS/Linux: `curl -fsSL https://<domain>/install.sh | sh`
- Windows: `irm https://<domain>/install.ps1 | iex`
- Homebrew tap, dibuat otomatis oleh `dist`.
- Pembaruan lewat `agentgate self-update`.
- Installer mengecek checksum SHA-256. Rilis disertai artifact attestation GitHub.

Scoop/winget ditambahkan nanti. `cargo install` tidak dijadikan jalur utama. Code signing (Apple Developer ID + notarization, sertifikat Windows) belum dipakai, dan baru ditambahkan saat ada installer klik-dua-kali (`.pkg`/`.msi`).

**Development full Docker.** Host cukup memasang Docker. Semua toolchain berjalan di container lewat Docker Compose:

- `app`: toolchain Rust sesuai `rust-toolchain.toml`, `watchexec`, `sccache`, Tailwind standalone, `sea-orm-cli`, `cargo-deny`, `cargo-insta`.
- `db`: `pgvector/pgvector:pg18`.
- `s3`: storage S3 (SeaweedFS; image MinIO komunitas sudah tidak dirilis, diputuskan 2026-11-11).
- `mailpit`: penampung email lokal.
- Volume terpisah untuk cargo registry, `target/`, dan cache `sccache`.
- Container berjalan dengan UID/GID user host, supaya file hasil build tidak dimiliki root.
- `bin/dev`, `bin/test`, `bin/loco`, dan `bin/ci` dijalankan dari host dan meneruskan perintah ke container.
- `devcontainer.json` disediakan, supaya editor (VS Code/Cursor) dan rust-analyzer berjalan di dalam container.
- Modul Worktree dipakai: satu container `db` bersama untuk semua worktree, dan satu container `app` per worktree dengan database dan port sendiri.

Pengecualian untuk CLI/runner `agentgate`: build dan unit test di Docker, tetapi uji end-to-end menjalankan binary hasil build di host, karena runner harus memanggil client AI, membuka terminal, dan mengakses repo di host. Uji Windows dan macOS dilakukan lewat GitHub Actions.

## Urutan build MVP

MVP tetap satu rilis, tetapi dibangun dalam 13 milestone. Jalur utama M0–M6 dikerjakan berurutan dan menghasilkan alur end-to-end pertama dengan Claude Code. M7–M11 bisa dikerjakan paralel setelah dependensinya selesai. Durasi belum ditentukan karena bergantung pada ukuran tim.

Prinsipnya: satu alur utuh dulu (satu client, satu provider git), baru diperluas. Hal yang bisa mengubah arsitektur diverifikasi paling awal lewat spike di M0.

&#91;embedded content: dependensi 13 milestone MVP\]

Panah menunjukkan milestone mana yang harus selesai lebih dulu. M8 dan M10 butuh runner (M5) dan code graph (M7). M12 dimulai setelah semua milestone selesai.

| Milestone | Isi | Mulai setelah | Selesai bila |
| --- | --- | --- | --- |
| M0 Fondasi | Cargo workspace (`server`, `cli`, `core`, `graph`), Docker dev + devcontainer, `bin/*`, GitHub Actions, skeleton Loco, HTMX/Tailwind/Basecoat ter-vendor, i18n, CSP + CSRF. Spike: hooks dan flag headless tiap client, cara buka terminal per OS, editor WYSIWYG tanpa bundler | – | `bin/ci` hijau di Docker, `/_health` jalan, hasil spike tertulis |
| M1 Identitas & tenancy | Auth sesi database, Google/GitHub, 2FA, workspace, tim, project, repo, role per project, undangan email, audit log, extractor tenant | M0 | Daftar, buat workspace, undang anggota, role ditegakkan, akses lintas workspace menghasilkan 404 |
| M2 Tiket & board | Tipe tiket, field + field custom, template kode tiket, komentar, @mention, lampiran S3, relasi (blocks ditegakkan), riwayat, board Kanban, list, sampah 30 hari, pencarian dasar | M1 | Tiket bisa dikelola penuh tanpa AI |
| M3 Workflow & gate | Workflow builder, workflow per tipe, YAML impor/ekspor, assignment per stage, approve/reject, gate engine, artifact + versi + editor Markdown, Butuh Keputusan, lompat stage, migrasi workflow, 9 template | M2 | Tiket melewati workflow dengan approve/reject manual, transisi tanpa evidence ditolak |
| M4 MCP & persona | OAuth 2.1 MCP, tool MCP (`ag_context` dan lainnya), persona + model per client, slash command dari workflow, impor skill pack (review manusia) | M3 | Claude Code terhubung, `/ag-brainstorming` menghasilkan draft tiket |
| M5 CLI & runner | `agentgate login`/`init`/`uninstall`, backup + ganti aturan, runner WebSocket, model klaim, worktree, adapter Claude Code, terminal interaktif, izin aksi berbahaya, evidence ditandatangani, kuota habis, @agent, cek drift, PR sync aturan, RTK | M4 | Geser tiket, job jalan otomatis di runner, artifact + evidence masuk untuk direview |
| M6 Git & PR | GitHub, GitLab, Bitbucket (+ versi perusahaan via runner), branch/PR otomatis, multi-repo, code review AI di PR, mode merge, gate CI, testing lokal + Playwright, stage Deploy | M5 | Alur Dev, PR, Review, Testing dengan screenshot, merge berjalan end-to-end |
| M7 Code graph | Crate `graph`, SCIP di CLI, upload/clone, update inkremental, lintas repo, embedding, tool `ag_search`/`ag_trace`/`ag_snippet`, riwayat git | M1 | Lulus benchmark terhadap codebase-memory dan Graphify |
| M8 Knowledge base | Scan + estimasi biaya, katalog dokumen, jenis custom, knowledge workspace, pemicu update, usulan diff + approve, sinkron `.agentgate/knowledge/` | M5, M7 | Scan project contoh menghasilkan dokumen, update setelah tiket Done muncul sebagai usulan |
| M9 Planning & laporan | Backlog, sprint, board Scrum, story point, estimasi AI + kalibrasi, roadmap, kalender, release + release notes AI, components, query ala JQL, saved filter, laporan, bulk edit | M3 | Satu sprint bisa direncanakan, dijalankan, dan dilaporkan |
| M10 Visual | Graph viewer, kantor isometrik, preview mockup di domain sandbox, halaman Penghemat Token | M5, M7 | Aktivitas runner tampil live di AI Office, graph bisa di-zoom sampai fungsi |
| M11 Multi-client & integrasi | Adapter Codex, Gemini CLI, Cursor, OpenCode, Copilot CLI, koneksi Jira (3 mode), impor CSV, API publik + webhook, ekspor data | M5 | Alur M5 berjalan di semua client, mirror Jira berjalan |
| M12 Rilis | Hardening, uji beban `LISTEN/NOTIFY`, deploy Compose di VPS, backup, observability, pipeline rilis CLI (`dist`), security review | Semua | Production jalan dan CLI bisa di-install dari script resmi |

## Di luar cakupan & risiko

**Tidak masuk (saat ini):**

- Jira Service Management (helpdesk, SLA, customer portal).
- Marketplace app/plugin pihak ketiga.
- Aplikasi mobile native.
- Dark mode.
- Billing dan kuota penyimpanan.
- Shared runner (VPS/CI milik tim).
- Notifikasi email, Slack, Teams, Discord, atau Telegram untuk aktivitas tiket.
- Time tracking manual, dashboard custom, SAML/SCIM.
- Edit dokumen bersama secara realtime.
- Connector selain Jira.
- Headroom/proxy API dan anggaran token per job.
- Template workflow Kiro-style, TDD, Design Sprint, Dual-track Agile, RFC/ADR, Research Spike, Scrumban.
- Self-host (tetapi arsitektur disiapkan untuk itu).

**Perlu diverifikasi saat build:**

- [ ] Dukungan hooks di tiap client selain Claude Code (Codex, Gemini CLI, Cursor, OpenCode, Copilot CLI). Ini menentukan kekuatan penegakan lokal dan integrasi RTK.
- [ ] Flag isolasi config global untuk mode headless tiap client.
- [ ] Dukungan `AGENTS.md` dan `contextFileName` di tiap client.
- [ ] Cara membuka terminal interaktif dengan sesi client di Windows, macOS, dan Linux.
- [ ] Dukungan RTK di Windows dan di client selain Claude Code.
- [ ] Ketersediaan indexer SCIP untuk bahasa prioritas.
- [ ] Benchmark code graph terhadap codebase-memory dan Graphify.
- [ ] Editor WYSIWYG Markdown yang bisa dipakai tanpa bundler (lewat import map) dan lolos CSP `script-src 'self'`.
- [ ] Kapasitas `LISTEN/NOTIFY` Postgres untuk realtime board, AI Office, dan koneksi runner saat jumlah user besar.

**Risiko utama:**

- **Scope MVP sangat besar** karena semua fitur masuk satu rilis. Urutan build tetap perlu disusun mengikuti dependensi: data model dan workflow, lalu gate, runner, MCP, dan baru setelah itu fitur visual.
- **Biaya server tanpa kuota.** Penyimpanan kode, embedding, dan evidence ditanggung Agent Gate.
- **Penegakan lokal tidak merata antar client.** Gate di server tetap menjadi penjaga utama.
- **Ketergantungan pada runner online.** Job, verifikasi PR/CI untuk git versi perusahaan, dan update knowledge menunggu runner menyala, sementara runner tidak pernah menyala otomatis.
