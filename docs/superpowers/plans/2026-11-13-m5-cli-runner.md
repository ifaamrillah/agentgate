# M5 CLI & Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal (spec "Selesai bila"):** geser tiket ke stage ber-persona → job diumumkan ke executor →
executor klik **Ambil** (web/CLI, atau auto-ambil) → job jalan otomatis di runner miliknya
(`claude -p` di worktree tiket, aturan terbaru dari server) → log, langkah, dan **evidence yang
ditandatangani runner** masuk ke server → gate evidence (`tests_passed`, `screenshot_exists`)
terpenuhi → stage diselesaikan (Menunggu review) untuk direview manusia.

Cakupan M5 dari spec: `agentgate login`/`init`/`uninstall`, backup + ganti aturan, runner
WebSocket, model klaim, worktree, adapter Claude Code, terminal interaktif, izin aksi berbahaya,
evidence ditandatangani, kuota habis, @agent, cek drift, PR sync aturan, RTK.

**Spec:** `docs/spec.md` §"Runner & eksekusi job", §"MCP, slash command & aturan project"
(file aturan, `agentgate init`, sinkronisasi), §"Penghemat token" (RTK), baris M5. Spike:
`docs/spikes/2026-11-11-client-hooks-headless.md`, `docs/spikes/2026-11-11-terminal-per-os.md`.
Prototype: `screens-m4-m6.js` (Runner & antrean job, Log job live, Login perangkat).

## Architecture

- **Login CLI = device code flow (RFC 8628)** di atas authorization server M4. Client publik
  bawaan `agentgate-cli` (baris `oauth_clients` dibuat saat pertama dipakai). `POST
  /oauth/device_authorization` → `device_code` (digest) + `user_code` 8 huruf (`WDJB-MJHT`,
  tanpa huruf mirip) berlaku 15 menit; halaman `/device` (login wajib): masukkan kode → setujui
  (pilih workspace) / tolak → selesai. `POST /oauth/token` grant
  `urn:ietf:params:oauth:grant-type:device_code` → `authorization_pending` / `slow_down` /
  `access_denied` / `expired_token` / token (grant = client × user × workspace, sama dengan MCP).
- **Runner** (`runners`): perangkat milik user di satu workspace, didaftarkan CLI dengan nama,
  OS, client, versi, dan **kunci publik Ed25519** (kunci privat tidak pernah meninggalkan
  laptop). Online = heartbeat < 60 dtk. Bisa dicabut. Pengaturan runner per user
  (`runner_prefs`): auto-ambil, batas paralel, client default, kebijakan kuota, terminal per OS.
- **Job** (`jobs`): satu stage × satu tiket (atau `@agent` dari komentar). Dibuat di
  `stages::enter_stage` (satu titik untuk semua jalur masuk stage) bila stage ber-persona dan
  mode otomatis/interaktif; job terbuka lama milik tiket dibatalkan. Status:
  `queued → claimed → running → (paused | quota) → succeeded | failed | cancelled`.
  Klaim = `UPDATE … WHERE status='queued'` (lock pertama menang). Urutan antrean = prioritas
  tiket, lalu umur. Lepas → `queued`. Batal: executor (pemegang lock) atau `AdministerProject`.
- **Protokol runner** (`core::runner`, dipakai server dan CLI): WebSocket `GET /api/runner/ws`
  (bearer) untuk kontrol — runner mengirim `hello`/`heartbeat`, server mendorong
  `job_ready` (job untuk runner ini), `job_cancelled`, `permission_decided`. Server menilai
  event dari DB tiap 2 dtk per koneksi (LISTEN/NOTIFY = M12). Data lewat HTTP JSON
  `/api/runner/…`: daftar/claim/release/cancel job, `start` (→ `JobSpec`: tiket, stage, persona,
  prompt dari `services::mcp::prompts`, model, branch `ag/KSR-26-slug`, file aturan, izin aksi),
  `logs`, `steps`, `evidence`, `permissions`, `finish`, `drift`.
- **Evidence ditandatangani**: runner menghasilkan sendiri (bukan klaim AI): `test_report` dari
  menjalankan `[test] command` di `.agentgate/project.toml` (exit code + ringkasan + output
  dipangkas), `screenshot` dari berkas `.agentgate/evidence/*.png|jpg` di worktree, `log`.
  Tanda tangan Ed25519 atas payload kanonik (`core::runner::evidence_payload`: job, kind, nama,
  SHA-256 isi, ringkasan). Server memverifikasi dengan kunci runner pemegang job; berkas ke
  `ctx.storage` (`evidence/<job>/<uuid>`), ≤ 10 MB.
- **Gate evidence**: `tests_passed` = evidence `test_report` terbaru tiket di stage itu sejak
  `stage_entered_at` lulus; `screenshot_exists` = ada `screenshot` sejak `stage_entered_at`.
  PR/CI tetap M6.
- **Job selesai**: `succeeded` → server menjalankan `stages::complete` sebagai executor (minta
  review / maju bila gate terpenuhi); gagal gate → tetap aktif + baris log alasan.
  `quota` → executor memilih tunggu reset / lepas (dialog "Kuota AI habis").
- **Izin aksi berbahaya**: `agentgate hook pre-tool` (hook `PreToolUse` Claude Code) mengklasifikasi
  perintah (`core::runner::classify_command`) → `ALWAYS_BLOCKED` ditolak; tingkat persona
  `allow`/`ask`/`block`; `ask` → `job_permissions` (job `paused`), menunggu jawaban dari web
  (Tolak / Izinkan sekali / Selalu untuk tiket ini) atau CLI. RTK: hook memanggil
  `rtk rewrite "<cmd>"` bila `rtk` terpasang.
- **@agent**: komentar `@agent-<persona>` oleh Member+ → job `comment` yang langsung terkunci ke
  penulis; runner penulis offline → komentar ditandai "menunggu runner Anda online". Hasil
  dibalas sebagai komentar.
- **CLI** (`cli`): `login`, `logout`, `status`, `init`, `uninstall`, `rules check|sync`, `jobs`,
  `job claim|release|cancel|attach`, `runner start [--yes]`, `hook <event>`, `doctor`.
  Kredensial `~/.config/agentgate/credentials.json` (0600). Worktree
  `~/.local/state/agentgate/wt/<repo>/<branch>`. Adapter Claude Code: `claude -p … --output-format
  stream-json --verbose --setting-sources project --strict-mcp-config --mcp-config .mcp.json
  --permission-mode acceptEdits [--model]`. Interaktif: launcher di state dir → terminal per OS
  → `agentgate job attach <id>`; gagal → notifikasi perintah `/ag-…`.
- **`agentgate init`**: backup semua file aturan lama (`CLAUDE.md`/`AGENTS.md` semua level,
  `.claude/{skills,commands,agents,settings.json}`, `.mcp.json`, `.cursor/rules`, `.codex/`,
  `.gemini/`, `GEMINI.md`, `.github/copilot-instructions.md`) ke
  `.agentgate/backup/<ts>.tar.gz` + manifest, hapus, tulis file aturan dari server
  (`GET /api/cli/rules`), commit. `uninstall` memulihkan backup terbaru + commit. Drift =
  checksum file aturan vs server; `rules sync` membuat branch `agentgate/sync-rules` + commit.
- **UI**: `/device`; Pengaturan → **Runner & CLI** (`/w/{slug}/runner/{panel}` subpage E: Permintaan
  izin, Antrean job, Runner saya, Runner tim, Pengaturan runner, Aturan & drift, Instal CLI);
  **Log job** `/w/{slug}/jobs/{id}` (langkah, log live HTMX polling, detail, evidence, izin, Lepas/
  Batalkan); kartu job + evidence di stage tiket; dialog Ambil job, Permintaan izin, Kuota habis,
  Lepas, Batalkan.

## Global Constraints

- Aturan M1–M4 tetap (UUID v7, `timestamptz`, enum text + CHECK + tes `EnumIter`, 404 lintas
  workspace, PRG, Fluent "Anda", audit di transaksi yang sama, izin dibaca ulang).
- API runner hanya bearer (token device flow / OAuth M4), selalu JSON, CSRF tidak berlaku (tanpa
  cookie). Token tidak memberi lebih dari user.
- Evidence tanpa tanda tangan valid dari runner pemegang job ditolak 422.
- Prompt tiket tidak pernah lewat shell (argv array).

## Review Focus

1. Device flow: kode sekali pakai, kedaluwarsa, `slow_down`, tolak; user lain tidak bisa
   menyetujui kode yang sudah dipakai → Task 3.
2. Klaim: dua klaim bersamaan → satu menang; hanya executor; runner orang lain tidak bisa
   mengirim log/evidence → Task 5.
3. Tanda tangan evidence diverifikasi; gate evidence hanya dari evidence sejak masuk stage → Task 6.
4. Izin aksi: `ALWAYS_BLOCKED` tidak bisa diizinkan → Task 2, 7.

## Tasks

- [ ] **1. Plan** (dokumen ini) — commit `docs: plan M5`.
- [ ] **2. core::runner**: protokol (JobSpec, pesan WS, evidence), `classify_command`,
  `evidence_payload` + sign/verify Ed25519, `branch_name`, file aturan + checksum. Unit test.
- [ ] **3. Skema + device flow**: migrasi M5 (device codes, runners, prefs, jobs, logs, evidence,
  permissions, kind notifikasi/audit), entity; device authorization + `/device` + token grant.
- [ ] **4. Job dari stage**: `enter_stage` → job; antrean, klaim/lepas/batal dari web.
- [ ] **5. API runner**: register, heartbeat, ws, jobs, start, logs, steps, finish, permissions,
  drift; tes HTTP (izin, lintas workspace, klaim bersamaan).
- [ ] **6. Evidence + gate**: unggah bertanda tangan, gate `tests_passed`/`screenshot_exists`,
  job sukses → `stages::complete`.
- [ ] **7. Izin aksi, kuota, @agent**.
- [ ] **8. UI**: Runner & CLI, Log job, kartu job di tiket, dialog; komponen baru di `/_dev/ui`.
- [ ] **9. CLI**: login/logout/status, init/uninstall/rules, jobs/job, runner start (adapter
  Claude, worktree, evidence, terminal interaktif), hook, doctor. Tes unit + tes integrasi CLI
  terhadap server palsu.
- [ ] **10. Penutup**: seed dev (runner + job contoh), design-system.md, screenshot Playwright
  (desktop/390/dark), `bin/ci`, merge `--no-ff` ke `staging`, `bin/ci`, PROGRESS.md.
