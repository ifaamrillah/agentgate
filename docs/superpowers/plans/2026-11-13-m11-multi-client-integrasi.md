# M11 Multi-client & Integrasi Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal (spec "Selesai bila"):** alur M5 berjalan di semua client, mirror Jira berjalan. Cakupan baris
M11: adapter Codex, Gemini CLI, Cursor, OpenCode, Copilot CLI; koneksi Jira (3 mode); impor CSV; API
publik + webhook; ekspor data. Utang yang ditutup: race init `markdown.js`/`md-editor.js` (M10).

**Spec:** `docs/spec.md` §"Runner & eksekusi job", §"MCP, slash command & aturan project", §"Integrasi
Jira & API publik", baris M11. Spike: `docs/spikes/2026-11-11-client-hooks-headless.md`. Prototype:
`screens-m9-m11-m12.js` (Connector Jira, Impor CSV, API & webhook, Ekspor data).

**Di luar cakupan (spec §"Di luar cakupan"):** notifikasi email/Slack/Teams/Discord/Telegram untuk
aktivitas tiket, connector selain Jira. OAuth Google/GitHub login sudah selesai di M1. Notifikasi M11
(ekspor siap, webhook dinonaktifkan, Jira gagal) tetap in-app.

## Keputusan desain

### Multi-client (crate `core` + `cli`)

- **File aturan v2** (`core::runner::rules`, `RULES_VERSION = 2`): satu repo dipakai tim dengan client
  berbeda, jadi `init` menulis config tipis untuk keenam client sekaligus: `AGENTS.md`, `CLAUDE.md`
  (`@AGENTS.md`), `.mcp.json` (Claude + Copilot), `.claude/settings.json`, `.codex/config.toml` +
  `.codex/hooks.json`, `.gemini/settings.json` (`context.fileName`, `mcpServers.httpUrl`, hooks),
  `.cursor/mcp.json` + `.cursor/hooks.json`, `opencode.json` + `.opencode/plugins/agentgate.ts`
  (plugin tipis yang memanggil `agentgate hook`), `.github/hooks/agentgate.json` (Copilot). Hook
  selalu `agentgate hook <event> --client <c>`.
- **Slash command** tetap prompt MCP (M4); tidak ada file skill per stage (spec: instruksi per stage
  tidak disimpan sebagai file).
- **Hook per client** (`cli::hook`): baca payload stdin sesuai client, jawab dengan bentuk keputusan
  client itu (Claude `hookSpecificOutput`, Codex sama dengan Claude, Gemini `decision`, Cursor
  `permission`, Copilot `permissionDecision`, OpenCode lewat plugin).
- **Adapter runner** (`cli::runner::clients`): satu adapter per client (program + override env
  `AGENTGATE_<CLIENT>_BIN`, argumen headless + flag trust/isolasi dari spike, argumen interaktif, env,
  parser JSONL → log/langkah/hasil/usage, deteksi kuota). Client job = `JobSpec.client` (server memilih
  dari client default runner/persona). Isolasi "best effort" per client, didokumentasikan.
- **doctor** memeriksa keenam binary.
- **Tes**: argumen + parser per client dari fixture, rules v2 valid (JSON/TOML), alur runner end-to-end
  dengan binary palsu per client.

### API publik + webhook (server)

- **Token API** (`api_tokens`): pemilik user, nama, prefix tampil (`ag_pat_XXXX`), digest SHA-256,
  scope (`tiket:baca`, `tiket:tulis`, `komentar:tulis`, `laporan:baca`, `knowledge:baca`,
  `webhook:kelola`), project (kosong = semua), `expires_at`, `last_used_at`, `revoked_at`. Token
  bertindak sebagai pembuatnya (izin role tetap dicek) dibatasi scope. Rate limit 600/menit per token
  (Postgres), header `X-RateLimit-Remaining`.
- **REST `/api/v1`** JSON: projects, tickets (list/get/create/patch/transition lewat gate), comments,
  sprints, knowledge, webhooks; error JSON `{error: code}`; OpenAPI di `/api/v1/openapi.yaml`.
- **Webhook keluar** (`webhooks`, `webhook_deliveries`): URL https (http hanya loopback bila diizinkan
  config), secret terenkripsi, event + project. Outbox: baris delivery ditulis di transaksi yang sama
  dengan perubahan; worker mengirim setelah commit; scheduler menyapu yang tertahan. Tanda tangan
  `X-AgentGate-Signature: sha256=<hmac(timestamp.body)>`, `X-AgentGate-Timestamp`,
  `X-AgentGate-Event`, `X-AgentGate-Delivery`. Gagal diulang sampai 5× dengan jeda bertahap; 5 gagal
  beruntun → webhook nonaktif + notifikasi Owner/Admin. Kirim ulang, kirim event uji, rotasi secret.
- UI `/w/{slug}/settings/api/{tokens,hooks,log,docs}` (izin `manage_integrations`).

### Ekspor data

- `data_exports`: pilihan isi (tiket+komentar+riwayat, artifact, knowledge, lampiran+evidence,
  workflow/persona/skill pack, audit log), project, penerima link, status, progres, ukuran, key storage,
  `expires_at` (7 hari). Worker menyusun ZIP (JSON + Markdown/YAML). Hanya Owner/Admin; ekspor dan
  unduhan diaudit; notifikasi in-app saat siap. UI `/w/{slug}/settings/export/{new,hist}`.

### Impor CSV

- `ticket_imports`: 4 langkah (Unggah ≤ 20 MB UTF-8 `,`/`;` → Pemetaan kolom otomatis (ekspor
  Jira/Trello/Linear) → Pratinjau siap/peringatan/dilewati → Impor). Kode lama jadi alias, status →
  stage lewat nama, penerima lewat email anggota, label baru dibuat. Laporan baris dilewati CSV
  disimpan 30 hari. UI `/w/{slug}/settings/import-csv/{new,hist}`.

### Connector Jira

- `jira_connections` (satu per workspace di MVP): Cloud (email + API token, REST v3) atau Data Center
  (PAT, REST v2), token terenkripsi, project Jira ↔ project Agent Gate, mode `once|mirror|two_way`,
  pemetaan status ↔ stage, field ↔ field, user ↔ user (otomatis lewat email), jeda.
  `jira_links` (tiket ↔ issue), `jira_sync_logs` (arah, hasil ok/gate/fail, pesan).
- Klien `JiraApi` (trait) + implementasi reqwest; tes memakai server Jira palsu (axum).
- **Impor sekali**: issue, komentar, lampiran (≤ 25 MB), sprint, pemetaan status; kode Jira = alias.
- **Mirror** (Agent Gate → Jira): perubahan tiket (outbox yang sama dengan webhook) didorong ke Jira;
  sinkron penuh berkala menimpa perubahan di Jira.
- **Dua arah**: tarik perubahan Jira lewat JQL `updated >=` (polling, tanpa butuh URL publik).
  Status Jira → transisi stage lewat gate; ditolak → log "Ditolak gate" dan status Jira dikembalikan.
  Field lain: perubahan terakhir menang.
- UI `/w/{slug}/settings/jira/{overview,conn,mode,status,fields,log}`.

## Global Constraints

- Aturan M1–M10 tetap (UUID v7, `timestamptz`, enum text + CHECK + tes, 404 lintas workspace, PRG,
  Fluent "Anda", tanpa inline script, CSP `script-src 'self'`, GET tidak menulis, efek samping setelah
  commit, secret terenkripsi XChaCha20-Poly1305).
- Tes tidak memanggil jaringan sungguhan (server palsu lokal). Komponen baru masuk `/_dev/ui` +
  `design-system.md`.

## Tasks

- [x] **1. Plan** (dokumen ini).
- [x] **2. Race init JS**: `markdown.js`/`md-editor.js` memindai dokumen saat siap + `htmx.onLoad`;
  tes `node --test` lewat `bin/tests`.
- [x] **3. Multi-client** (worktree terpisah): rules v2, hook per client, adapter runner + parser,
  doctor, halaman Hubungkan client; tes.
- [x] **4. API publik + webhook**: migrasi, token + auth + rate limit, REST v1 + OpenAPI, outbox +
  worker + retry, UI; tes.
- [x] **5. Ekspor data + impor CSV**: migrasi, worker ZIP, wizard CSV, UI; tes.
- [x] **6. Connector Jira**: migrasi, klien + server palsu, 3 mode, pemetaan, log, UI; tes.
- [x] **7. Seed dev** (`seed_integrations`), `/_dev/ui`, design-system.md.
- [x] **8. Visual check** Playwright (desktop, 390px, gelap) vs prototype.
- [x] **9. bin/ci**, merge `--no-ff` ke `staging`, `bin/ci`, PROGRESS.md.
