# M4 MCP & Persona Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal (spec "Selesai bila"):** Claude Code terhubung ke Agent Gate lewat Remote MCP (OAuth 2.1), dan
`/ag-brainstorming` menghasilkan draft tiket yang direview manusia. Cakupan: OAuth 2.1 untuk MCP
(discovery, dynamic client registration, PKCE, consent, token per user/workspace), tool MCP
(`ag_context` dan lainnya), persona yang bisa dikelola + model per client, slash command dari
workflow, impor skill pack dengan review manusia, dan perbaikan utang `html_errors`.

**Architecture.**

- **Error JSON** (utang M0): `controllers::api_routes::is_api(path)` = `/mcp`, `/oauth/token`,
  `/oauth/register`, `/oauth/revoke`, `/.well-known/`, `/api/`. Layer `html_errors` tidak menyentuh
  respons path itu; error tanpa body JSON di path itu diganti `{"error": "..."}` (bentuk RFC 6749).
- **Persona** (`personas`, per workspace): kunci slug stabil, nama, tugas singkat, warna, ikon,
  instruksi (Markdown), gaya ringkas, tool MCP yang diizinkan, tools client, izin aksi berbahaya
  (`allow`/`ask`/`block` per aksi; daftar "selalu diblokir" tetap di kode), model + effort per
  client (katalog statis `models::personas::CLIENTS`, boleh diketik bebas). 9 persona bawaan
  dibuat bersama workspace (dan di-backfill migrasi). Hapus = soft delete dengan **pengganti**
  (`replaced_by`): stage yang menyebut kunci persona terhapus memakai penggantinya, sehingga versi
  workflow lama tidak perlu ditulis ulang. `core::workflow::Stage.persona` menjadi kunci
  (`Option<String>`, format slug); `Persona` tinggal daftar kunci bawaan.
- **Skill pack** (`skill_packs`, `skill_files`, `persona_skills`): sumber `custom` (ditulis di
  editor) atau `repo` (diunduh server dari GitHub publik, `codeload.github.com` tarball, hanya
  `SKILL.md` + `scripts/` + `references/`, batas jumlah/ukuran; host lain → M5 lewat runner).
  Status `draft` → `pending` (kirim review) → `approved`/`rejected`. Saat dikirim, pemindaian
  statis server menghasilkan laporan risiko (Aman/Perlu perhatian/Berbahaya: perintah berbahaya,
  prompt injection, akses secret, tools yang diminta). Review AI di runner = M5. Approve/tolak
  hanya `ManageTicketSettings`, approve skill "Berbahaya" wajib ketik kunci. Hanya skill
  `approved` yang bisa dipasang ke persona. Audit di transaksi yang sama.
- **OAuth 2.1** (`oauth_clients`, `oauth_grants`, `oauth_codes`, `oauth_tokens`): metadata
  RFC 8414 (`/.well-known/oauth-authorization-server`) dan RFC 9728
  (`/.well-known/oauth-protected-resource[/mcp]`); DCR RFC 7591 (`POST /oauth/register`, klien
  publik `token_endpoint_auth_method=none`, redirect https atau loopback http);
  `GET /oauth/authorize` (login wajib lewat gate, PKCE S256 wajib, `resource` = URL MCP) →
  halaman consent (pilih workspace) → `POST /oauth/authorize` → redirect `code`/`error`;
  `POST /oauth/token` (`authorization_code` + `code_verifier`, `refresh_token` dengan rotasi;
  refresh token dipakai ulang → grant dicabut); `POST /oauth/revoke` (RFC 7009). Token opak,
  disimpan sebagai digest SHA-256; access 1 jam, refresh 30 hari, kode 10 menit sekali pakai.
  Satu grant = satu client × user × workspace ("Client terhubung", bisa dicabut).
- **MCP** (`rmcp` 3.9, streamable HTTP stateless + respons JSON, di-mount di `/mcp`): layer
  bearer memvalidasi token → `McpPrincipal` (grant, user, workspace) di extension request; 401
  membawa `WWW-Authenticate: Bearer resource_metadata=…`. Setiap panggilan memuat ulang user,
  keanggotaan, dan role (izin mengikuti role user saat itu; service tiket membaca ulang akses
  project di transaksi). Logika tool di `services::mcp` (dites langsung dan lewat HTTP).
- **Tool MCP** (7, sisanya milestone lain): `ag_context` (berlapis: ringkasan, `detail`
  deskripsi/artifact/skill), `ag_ticket` (get/search/create), `ag_draft_tickets`, `ag_artifact`
  (get/save), `ag_transition` (selesaikan stage / pindah ke stage, gate dicek server), `ag_ask`
  (pertanyaan ke executor/approver), `ag_comment`.
- **Slash command** = MCP prompts: `ag-<kunci-stage>` untuk setiap stage ber-persona di workflow
  workspace + `ag-status`, `ag-ticket`. Isi prompt dibangun dari persona (instruksi, skill,
  gaya ringkas, model) dan stage (prompt, artifact input/output). `ag-next`, `ag-search`,
  `ag-knowledge-sync` tampil "segera" (M5/M7/M8).
- **Draft tiket brainstorming** (`ticket_drafts`): `ag_draft_tickets` menyimpan hierarki draft
  per project (opsional tiket sumber). Halaman project "Draft tiket": centang/ubah/hapus, lalu
  "Buat N tiket" membuat tiket lewat service M2 (induk mengikuti hierarki), draft terhapus.
- **UI**: Pengaturan → Persona agent (kartu + dialog baru/clone), Edit persona (subpage:
  identitas, instruksi, skill, tools & izin, model per client, zona bahaya), Pengaturan →
  Skill pack (daftar, impor, buat, editor file, review), Akun → Client AI (langkah, client
  terhubung + cabut, tool MCP, slash command), consent OAuth, project → Draft tiket.

**Spec:** `docs/spec.md` §"Persona agent, skill pack & model", §"MCP, slash command & aturan project",
baris M4. Prototype: `docs/reference/ui-prototype/screens-m4-m6.js` (Persona agent, Edit persona,
Skill pack, Buat skill pack, Review skill pack, Hubungkan client AI, Persetujuan OAuth MCP, Review
draft tiket brainstorming).

## Global Constraints

- Aturan M1–M3 tetap: UUID v7, `timestamptz`, enum = text + CHECK + tes `EnumIter`,
  `lock_timeout`, 404 lintas workspace/tanpa izin, PRG, 422 dengan form terisi, teks Fluent
  ("Anda"), audit di transaksi yang sama, izin dibaca ulang di transaksi.
- Izin: persona dan skill pack = `ManageTicketSettings`; MCP mengikuti izin project user
  (lihat = `ViewProject`, ubah = `EditProject`); token tidak pernah memberi lebih dari user.
- Semua token/kode OAuth disimpan sebagai digest; token mentah hanya di respons.
- Endpoint API membalas JSON (termasuk error), tidak pernah halaman HTML.
- CSP tetap `script-src 'self'`; JS per perilaku di `static/js/`.

## Review Focus

1. PKCE wajib dan diverifikasi, kode sekali pakai, redirect_uri harus sama persis, refresh token
   berotasi dan reuse mencabut grant → Task 6.
2. Token workspace A tidak bisa membaca workspace B; Viewer tidak bisa menulis lewat MCP;
   pencabutan grant langsung menolak token → Task 7.
3. Skill yang belum di-approve tidak bisa dipasang; approve "Berbahaya" butuh konfirmasi → Task 5.
4. Hapus persona dengan pengganti; stage lama tetap punya persona → Task 4.
5. Error JSON di endpoint API → Task 1.

## File Structure

```
core/src/workflow/definition.rs (persona = kunci), yaml.rs
server/migration/src/m20261115_000001_create_mcp_personas.rs
server/src/models/{personas,skill_packs,skill_files,persona_skills,oauth_clients,oauth_grants,
                   oauth_codes,oauth_tokens,ticket_drafts}.rs (+ _entities)
server/src/services/{personas,skills/{mod,scan,fetch},oauth,mcp/{mod,tools,prompts},drafts}.rs
server/src/controllers/{api_routes,oauth,mcp}.rs, controllers/workspaces/{personas,skills}.rs,
  controllers/account (client AI), controllers/tickets/drafts.rs
server/src/views/{personas,skills,oauth,drafts}.rs
server/assets/views/{personas,skills,oauth,drafts}/*, account/connect*.html
server/tests/requests/{mcp,oauth,personas,skills,drafts}.rs
```

### Task 1: Error JSON untuk endpoint API

- [ ] Tes: 404/405/401 di `/mcp`, `/oauth/token`, `/.well-known/x` → JSON, bukan HTML; halaman
  biasa tetap HTML.
- [ ] Commit `fix(server): API paths keep JSON errors`.

### Task 2: Persona sebagai kunci di definisi workflow

- [ ] Unit test core: kunci persona custom diterima, format salah → `workflow-persona-invalid`,
  YAML round-trip kunci custom. Server: builder/YAML menolak persona yang tidak dikenal workspace.
- [ ] Commit `refactor(core): stage persona is a workspace persona key`.

### Task 3: Skema dan model

- [ ] Migrasi semua tabel M4 + CHECK enum + backfill 9 persona per workspace; workspace baru
  dibuat bersama persona bawaan. Tes enum, backfill.
- [ ] Commit `feat(server): M4 schema (personas, skills, oauth, drafts)`.

### Task 4: Persona

- [ ] Tes: daftar, buat kosong/clone, ubah tiap bagian (validasi), pasang skill approved saja,
  hapus dengan pengganti (stage memakai pengganti), anggota biasa 404, audit.
- [ ] Commit `feat(server): manageable personas`.

### Task 5: Skill pack

- [ ] Tes: buat custom + file, kirim review → laporan pindai, approve/tolak (alasan wajib),
  approve berbahaya butuh ketik kunci, impor repo lewat fetcher palsu (filter file, batas
  ukuran), URL bukan GitHub ditolak, izin.
- [ ] Commit `feat(server): skill packs with static scan and human review`.

### Task 6: OAuth 2.1

- [ ] Tes: metadata, DCR (redirect tidak valid ditolak), authorize tanpa login → login,
  consent → code, token dengan PKCE benar/salah, kode dipakai ulang, refresh rotasi + reuse,
  revoke, cabut dari halaman Client AI.
- [ ] Commit `feat(server): OAuth 2.1 authorization server for MCP`.

### Task 7: Server MCP + tools + prompts

- [ ] Tes HTTP: tanpa token 401 + `WWW-Authenticate`; initialize; tools/list; setiap tool
  (izin Viewer, lintas workspace 404, gate ditolak); prompts/list dan prompts/get
  `ag-brainstorming`; alur end-to-end OAuth → `ag_draft_tickets`.
- [ ] Commit per bagian (`mcp endpoint`, `tools`, `prompts`).

### Task 8: Draft tiket brainstorming

- [ ] Tes: halaman draft, centang/ubah/hapus, buat tiket dengan hierarki, izin.
- [ ] Commit `feat(server): brainstorm ticket drafts review`.

### Task 9: Penutup

- [ ] Seed dev (skill pack contoh, draft contoh), katalog `/_dev/ui`, design-system.md.
- [ ] Screenshot Playwright desktop/390px/dark, bandingkan prototype.
- [ ] `bin/ci` hijau, merge `--no-ff` ke `staging`, `bin/ci` lagi, PROGRESS.md.
