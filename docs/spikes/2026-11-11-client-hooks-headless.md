# Spike: hooks & flag headless tiap client

Tanggal: 2026-11-11 · Milestone: M0 Fondasi (spike) · Terkait: `docs/spec.md` bagian "Runner & eksekusi job", "MCP, slash command & aturan project", "Penghemat token".

Konvensi penandaan:

- **[verified-lokal]**: dicoba langsung di host ini.
- **[docs]**: diambil dari dokumentasi resmi atau repo resmi. URL ada di bagian sumber tiap client.
- **belum terverifikasi**: tidak dikonfirmasi dari sumber primer, atau hanya berupa inferensi.

Versi yang diamati:

| Client | Versi |
| --- | --- |
| Claude Code | 2.1.319 (lokal) |
| Codex CLI | 0.167.0 (2026-11-06) |
| Copilot CLI | 1.0.109 (2026-11-10) |
| Gemini CLI | v0.67.0 (2026-11-04) |
| OpenCode | v2.2.7 (2026-11-10, V2) |
| Cursor CLI | v2026.10.22 |
| RTK | v1.2.0 (2026-11-08) |

## Pertanyaan

Untuk setiap client (Claude Code, Codex CLI, Gemini CLI, Cursor CLI, OpenCode, Copilot CLI):

1. Perintah headless beserta flag-nya: input prompt, mode auto-approve/permission, working dir, model, dan reasoning effort.
2. Isolasi dari config global user. Apa yang masih bocor, dan apakah auth tetap jalan.
3. Hooks: event apa saja yang tersedia, lokasi dan format config, apakah hook bisa memblokir tool call, dan apakah bisa me-rewrite input/output (dibutuhkan RTK). Juga dukungan RTK per client dan di Windows.
4. File instruksi project: apakah `AGENTS.md` dibaca native, dan setting `contextFileName` di Gemini.
5. Config MCP level project, dukungan remote Streamable HTTP, dan OAuth 2.1.
6. Slash command custom per project (untuk `/ag-<stage>`).
7. Output machine-readable, serta pelaporan token usage dan biaya.
8. Resume session, dan membuka sesi interaktif yang sudah dimuati prompt.

## Ringkasan rekomendasi

1. **Claude Code (M5) siap dipakai sesuai spec, dengan satu koreksi penting.** `--strict-mcp-config` mengabaikan **semua** MCP selain yang dioper lewat `--mcp-config`, termasuk `.mcp.json` di project.
   - Runner wajib memanggil: `claude -p ... --setting-sources project --strict-mcp-config --mcp-config <worktree>/.mcp.json`.
   - Auth tetap jalan dengan `--setting-sources project` [verified-lokal]. Credentials ada di `$CLAUDE_CONFIG_DIR/.credentials.json` (Linux), bukan di settings.
   - Runner **tidak boleh** mengubah `CLAUDE_CONFIG_DIR`/`HOME` milik user.
2. **Semua 6 client punya mode headless, output JSON/JSONL, dan resume.** Yang melaporkan token usage dengan field yang jelas adalah Claude (plus `total_cost_usd`), Codex (tanpa cost), OpenCode (`part.cost`/`part.tokens`), dan Gemini (`stats`). Field usage di Cursor dan Copilot belum terverifikasi.
3. **Lima dari enam client punya hooks dengan PreToolUse yang bisa block dan rewrite input.** Ini cukup untuk RTK, yang me-rewrite *perintah input* (`git status` → `rtk git status`), bukan output.
   - Claude Code, Codex, Gemini, Cursor, dan Copilot memakai hooks dengan format JSON.
   - OpenCode memakai plugin TypeScript, bukan hooks.
   - Semua bisa diisi `agentgate hook <event>` sebagai satu-satunya command, kecuali OpenCode yang butuh file plugin `.ts` tipis yang memanggil `agentgate hook`.
4. **Workspace trust adalah jebakan utama untuk job headless.** Codex, Gemini, Copilot, dan Cursor mengabaikan config project (MCP, hooks, commands) bila folder belum di-trust. Adapter wajib meng-trust worktree per run:
   - Gemini: `--skip-trust` / `GEMINI_CLI_TRUST_WORKSPACE=true`.
   - Copilot: `--trust-folder` + `GITHUB_COPILOT_PROMPT_MODE_WORKSPACE_MCP=true`.
   - Cursor: `--trust`.
   - Codex: entri `projects."<path>".trust_level` + `--dangerously-bypass-hook-trust`.
   - Claude `-p` melewati dialog trust [verified-lokal, `claude --help`].
5. **Isolasi penuh hanya bersih di Claude Code dan Codex.**
   - Claude: `--setting-sources project` + `--strict-mcp-config`.
   - Codex: `--ignore-user-config`, sementara auth tetap dibaca dari `CODEX_HOME`.
   - Client lain hanya punya opsi "ganti seluruh home dir" (`GEMINI_CLI_HOME`, `COPILOT_HOME`, `CURSOR_CONFIG_DIR`, `OPENCODE_CONFIG_DIR`). Opsi ini memutus auth berbasis file kecuali credentials disalin/symlink, atau memakai token via env.
   - Rekomendasi M11: isolasi "best effort" per client yang didokumentasikan, bukan janji isolasi total.
6. **AGENTS.md dibaca native** oleh Codex, OpenCode, Cursor, dan Copilot (Copilot juga membaca CLAUDE.md/GEMINI.md).
   - Gemini butuh `{"context":{"fileName":["AGENTS.md"]}}`. Key-nya sekarang `context.fileName`, bukan `contextFileName` top-level.
   - Spec perlu dikoreksi untuk nama key ini.
   - OpenCode **tidak** fallback ke CLAUDE.md, jadi tidak ada masalah.
7. **Slash command `/ag-<stage>` tidak seragam.** Rekomendasi: generate skill di dua lokasi, ditambah TOML khusus Gemini.
   - `.claude/skills/ag-<stage>/SKILL.md` (Claude, juga dibaca Copilot).
   - `.agents/skills/ag-<stage>/SKILL.md` (Codex, Cursor, Copilot, OpenCode).
   - `.gemini/commands/ag-<stage>.toml` (Gemini).
   - `.opencode/commands/ag-<stage>.md` (OpenCode, untuk `/ag-x` sejati).
   - Codex tidak punya slash command project. Custom prompts sudah deprecated sejak 2026-01-22, sehingga di Codex pemanggilannya `$ag-<stage>` atau `/skills`.
8. **RTK punya integrasi resmi untuk keenam client.** Di Windows, hook native binary jalan sejak v0.37.2, **kecuali Gemini** yang hook-nya masih butuh bash. Karena spec mewajibkan hook memanggil `agentgate hook ...`, Agent Gate sebaiknya **memanggil `rtk rewrite "<cmd>"` dari dalam `agentgate hook`**, bukan memasang hook RTK global (`rtk init -g` menulis ke config global user, bertentangan dengan isolasi).
9. **Risiko produk besar dari dua client:**
   - Gemini CLI sudah digantikan "Antigravity CLI" (2026-06-18) untuk user gratis/Google One.
   - OpenCode pindah ke V2 dengan server bersama per user, sehingga wajib `--standalone` agar env per job berlaku.
   - Keduanya perlu diputuskan ulang sebelum M11.

## Tabel perbandingan

| # | Claude Code 2.1.319 | Codex CLI 0.167 | Gemini CLI 0.67 | Cursor CLI 2026.10.22 | OpenCode 2.2.7 (V2) | Copilot CLI 1.0.109 |
| --- | --- | --- | --- | --- | --- | --- |
| 1. Headless | `claude -p "<p>"` / stdin; `--permission-mode` (acceptEdits/auto/bypassPermissions/dontAsk/plan/manual), `--allowedTools`/`--disallowedTools`, `--model`, `--effort low..max`; cwd = proses (+`--add-dir`) | `codex exec "<p>"` / `-`; `-s workspace-write`, `-a never`, `--yolo`; `-C dir`, `--add-dir`; `-m`, `-c model_reasoning_effort=high` | `gemini -p "<p>"` / stdin; `--approval-mode default\|auto_edit\|yolo\|plan`; `-m`; cwd = proses; thinking hanya lewat settings | `agent -p "<p>"`; `-f/--force`; `--workspace`, `--add-dir`; `--model <slug>` (effort lewat slug); `--trust` | `opencode run --standalone "<p>"`; `--auto`; `-m provider/model#variant` (variant = effort); `--agent`; cwd = proses | `copilot -p "<p>"`; `--allow-all-tools`/`--allow-tool`/`--deny-tool`, `--yolo`; `-C dir`; `--model`, `--reasoning-effort` |
| 2. Isolasi | `--setting-sources project` + `--strict-mcp-config --mcp-config` ✔; auth tetap ✔ [verified-lokal]; bocor: managed settings, `~/.claude.json`, skill/CLAUDE.md user (belum terverifikasi) | `--ignore-user-config` (auth tetap dari `CODEX_HOME`) ✔; `--ignore-rules`, `--ephemeral` | Tidak ada "skip user"; `GEMINI_CLI_HOME` ganti seluruh home (OAuth putus kecuali disalin); override via `GEMINI_CLI_SYSTEM_SETTINGS_PATH` | Tidak ada flag; `CURSOR_CONFIG_DIR` hanya cli-config (sisanya belum terverifikasi); bocor `~/.cursor/mcp.json`, hooks, `~/.claude` hooks, skills | `OPENCODE_CONFIG_DIR` ganti dir global; auth di DB data dir tetap ✔; wajib `--standalone` | `COPILOT_HOME` ganti `~/.copilot`; auth keychain/`COPILOT_GITHUB_TOKEN`; bocor `.claude/settings.json` hooks, policy |
| 3. Hooks | 33 event (SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, …); `.claude/settings.json`; block ✔ (exit 2/deny); rewrite input ✔ `updatedInput`; rewrite output ✔ `updatedToolOutput` | 14 event (SessionStart, PreToolUse, PostToolUse, UserPromptSubmit, Stop, …); `.codex/hooks.json`; block ✔; input ✔ `updatedInput`; output ✘ (hanya block/additionalContext); perlu trust hook | 11 event (SessionStart, BeforeTool, AfterTool, BeforeAgent, AfterAgent, …); `.gemini/settings.json`; block ✔; input ✔ `tool_input`; output ✘ (deny→reason) | 19 event (sessionStart, preToolUse, beforeShellExecution, stop, …); `.cursor/hooks.json`; block ✔; input ✔ `updated_input`; output hanya MCP | Plugin TS `.opencode/plugins/*.ts`; input ✔, output ✔ (`e.result`), block lewat `permission.hook` | 14 event (sessionStart, preToolUse, postToolUse, agentStop, …); `.github/hooks/*.json`; block ✔; input ✔ `modifiedArgs`; output ✔ `modifiedResult` |
| 3b. RTK | ✔ hook resmi, Windows ✔ | ✔ `rtk init --codex`, Windows ✔ | ✔ global, Windows ✘ (butuh bash) | ✔ global, Windows ✔ | ✔ plugin (≥1.18.29; kompat V2 belum terverifikasi) | ✔ `rtk init --copilot`, Windows ✔ |
| 4. Instruksi | `CLAUDE.md` (`@AGENTS.md` import) | `AGENTS.md` native (+`AGENTS.override.md`), maks 32 KiB | `GEMINI.md`; `context.fileName: ["AGENTS.md"]` | `AGENTS.md` + `CLAUDE.md` + `.cursor/rules/*.mdc` | `AGENTS.md` saja | `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md` |
| 5. MCP project | `.mcp.json` `type:"http"`; OAuth DCR + CIMD + client-id statis; `headersHelper` | `.codex/config.toml` `[mcp_servers.x] url=`; OAuth CIMD/DCR (`codex mcp login`); butuh trust | `.gemini/settings.json` `mcpServers.x.httpUrl`; OAuth DCR auto saat 401 | `.cursor/mcp.json` `url`; OAuth PKCE (`agent mcp login`); `--approve-mcps` | `opencode.json` `mcp.servers.x.type:"remote"`; OAuth PKCE, client_id→CIMD→DCR | `.mcp.json`/`.github/mcp.json` `type:"http"`; OAuth DCR ✔, CIMD ✘; butuh trust |
| 6. Slash cmd | `.claude/skills/<n>/SKILL.md` atau `.claude/commands/<n>.md` → `/n` | Tidak ada per project; skill `.agents/skills/<n>/SKILL.md` → `$n` | `.gemini/commands/<n>.toml` → `/n` | Skill `.cursor/skills/` atau `.agents/skills/` → `/n` (`.cursor/commands` belum terverifikasi) | `.opencode/commands/<n>.md` → `/n` | `.claude/commands/*.md`, skill `.github/skills`, `.agents/skills` |
| 7. Output & usage | `--output-format json\|stream-json`; `usage` + `modelUsage` + `total_cost_usd` ✔ [verified-lokal] | `--json` JSONL; `turn.completed.usage` (tanpa cost) | `-o json\|stream-json`; `stats` (sub-field belum terverifikasi) | `--output-format json\|stream-json`; token per turn (field belum terverifikasi), tanpa cost | `--format json`; `step_finish.part.tokens` + `part.cost` | `--output-format json` JSONL; field usage belum terverifikasi |
| 8. Resume/interaktif | `-r <id>`, `-c`, `--session-id <uuid>`, `--fork-session`; `claude "<p>"` | `codex exec resume --last\|<id>`; `codex "<p>"` | `--resume latest\|<uuid>`; `-i "<p>"` | `--resume [id]`, `--continue`; `agent "<p>"` | `run -c` / `-s <id>`; `opencode --prompt "<p>"` | `--continue`, `--resume <id>`, `--session-id`; `-i "<p>"` |

## Claude Code (2.1.319)

### 1. Headless [verified-lokal: `claude --help`]

```bash
cd <worktree>
claude -p "$(cat prompt.md)" \
  --output-format stream-json --verbose --include-hook-events \
  --setting-sources project \
  --strict-mcp-config --mcp-config .mcp.json \
  --permission-mode acceptEdits \
  --allowedTools "Bash(npm test *) Edit Read" --disallowedTools "Bash(git push --force *)" \
  --model opus --effort high \
  --session-id <uuid-dari-runner>
```

**Input prompt.** Prompt bisa diberikan sebagai argumen posisional, lewat stdin (`cat x | claude -p "q"`), atau dengan `--input-format stream-json` untuk input streaming.

**Permission.** `--permission-mode` menerima `acceptEdits`, `auto`, `bypassPermissions`, `manual`, `dontAsk`, `plan`.

- `--permission-prompts none` membuat semua aksi yang butuh prompt otomatis ditolak.
- `--dangerously-skip-permissions` mem-bypass semua permission check.
- Di docs, `permissions.defaultMode` bernilai `auto`/`bypassPermissions` tidak berlaku dari project settings. Nilai ini harus dioper lewat flag.
- Untuk spec "izin aksi berbahaya", ada dua opsi:
  - Hook `PreToolUse` yang memanggil `agentgate hook pre-tool`, lalu mengembalikan `deny`, atau menahan job menunggu persetujuan executor.
  - `PermissionRequest` hook. Di `-p` hook ini tetap jalan, dan bila tidak ada keputusan, tool di-deny [docs].

**Working dir.** Mengikuti cwd proses. Direktori tambahan lewat `--add-dir`. Ada juga `-w/--worktree`, tetapi Agent Gate mengelola worktree sendiri.

**Model dan effort.** `--model` (alias `opus`/`sonnet`/`fable`/`haiku`, atau nama penuh) dan `--effort low|medium|high|xhigh|max` (docs juga menyebut `ultracode`). `--fallback-model` tersedia.

**Flag lain yang relevan:**

- `--max-budget-usd` (hanya `--print`).
- `--json-schema` untuk structured output.
- `--no-session-persistence`.
- `--exclude-dynamic-system-prompt-sections`, yang mendukung prinsip "prefix konteks stabil" di spec.
- `--append-system-prompt`.

### 2. Isolasi

**`--setting-sources project`.** Hanya `.claude/settings.json` project yang dimuat. User dan local settings dilewati.

- Diuji dengan `claude -p "say ok" --output-format json --setting-sources project --strict-mcp-config --model haiku --effort low --no-session-persistence`. Hasilnya sukses, auth OAuth langganan tetap jalan [verified-lokal].
- Credentials di Linux ada di `$CLAUDE_CONFIG_DIR/.credentials.json` (default `~/.claude/.credentials.json`) [verified-lokal]. Di macOS ada di keychain [docs settings: "system keychain (macOS) or a credentials file"].
- Alternatif token: `claude setup-token` lalu env `CLAUDE_CODE_OAUTH_TOKEN` (nama env belum terverifikasi dari halaman docs yang dibaca), atau `ANTHROPIC_API_KEY`.

**`--strict-mcp-config`.** "Only use MCP servers from `--mcp-config`, ignoring all other MCP configurations" [docs cli-reference]. Artinya `.mcp.json` di worktree **juga diabaikan** kecuali dioper eksplisit lewat `--mcp-config .mcp.json`.

- Di `-p`, server project dari `.mcp.json` dimuat tanpa approval prompt [docs mcp].

**Masih bocor:**

- Managed settings/policy, yang memang tidak bisa dan tidak boleh dilewati.
- `~/.claude.json`, berisi account, state per project, dan MCP user. MCP user sudah dinetralkan oleh `--strict-mcp-config`.
- **Belum terverifikasi:** apakah `--setting-sources project` juga melewati `~/.claude/CLAUDE.md`, `~/.claude/skills`, `~/.claude/agents`, plugin yang di-enable di user settings, dan auto-memory.
  - Plugin kemungkinan ikut hilang karena `enabledPlugins` adalah key settings user (inferensi).
  - Uji yang diusulkan: jalankan `-p` dengan `--output-format stream-json --verbose`, lalu lihat daftar `skills`/`slash_commands`/`plugins` di event `system/init`.

**Opsi lebih keras.** Sebaiknya tidak dipakai untuk job biasa.

- `--safe-mode`: semua kustomisasi mati, termasuk CLAUDE.md, hooks, dan MCP. Ini terlalu keras, karena hooks dan MCP Agent Gate ikut mati.
- `--bare`: hooks dan CLAUDE.md mati, dan **OAuth/keychain tidak dibaca** (hanya `ANTHROPIC_API_KEY`). Tidak cocok untuk langganan user.

### 3. Hooks [docs hooks]

**Lokasi.**

- `.claude/settings.json` (project, di-commit) dan `.claude/settings.local.json`.
- User, managed, plugin, frontmatter skill/subagent.

**Event (33).** `SessionStart`, `Setup`, `UserPromptSubmit`, `UserPromptExpansion`, `PreToolUse`, `PermissionRequest`, `PermissionDenied`, `PostToolUse`, `PostToolUseFailure`, `PostToolBatch`, `Notification`, `MessageDisplay`, `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted`, `Stop`, `StopFailure`, `TeammateIdle`, `InstructionsLoaded`, `ConfigChange`, `CwdChanged`, `DirectoryAdded`, `FileChanged`, `WorktreeCreate`, `WorktreeRemove`, `PreCompact`, `PostCompact`, `PreModelSwitch`, `PostModelSwitch`, `Elicitation`, `ElicitationResult`, `SessionEnd`.

**Format** (handler `type`: `command|http|mcp_tool|prompt|agent`):

```json
{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command", "command": "agentgate", "args": ["hook", "session-start", "--client", "claude"] }] }],
    "PreToolUse":   [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "agentgate", "args": ["hook", "pre-tool", "--client", "claude"] }] }],
    "PostToolUse":  [{ "hooks": [{ "type": "command", "command": "agentgate", "args": ["hook", "post-tool", "--client", "claude"] }] }],
    "Stop":         [{ "hooks": [{ "type": "command", "command": "agentgate", "args": ["hook", "stop", "--client", "claude"] }] }]
  }
}
```

Bentuk exec (`command` + `args`) penting di Windows. Command harus berupa executable nyata (`.exe`). Shim `.cmd` tidak bisa dipakai. Shell default di Windows adalah Git Bash, dengan fallback PowerShell [docs hooks].

**Block.** Exit 2, atau `hookSpecificOutput.permissionDecision: "deny"` (`allow|deny|ask|defer`). `defer` hanya berlaku di `-p`.

**Rewrite input.** `hookSpecificOutput.updatedInput` mengganti seluruh objek input. Contoh: `{"permissionDecision":"allow","updatedInput":{"command":"rtk git status"}}`.

**Rewrite output.** `hookSpecificOutput.updatedToolOutput` berlaku juga untuk Bash, dengan shape `{"stdout","stderr","interrupted","isImage"}`. `updatedMCPToolOutput` khusus MCP. Rewrite output ini hanya mengubah apa yang dilihat model.

**SessionStart.** `hookSpecificOutput.additionalContext` disuntikkan sebagai system reminder. Batasnya 10.000 karakter, lebih dari itu dialihkan ke file. `source` bernilai `startup|resume|clear|compact|fork`. Ini sesuai spec ("hook SessionStart menyuntikkan konteks").

**Stream event.** `--include-hook-events` (stream-json) memunculkan event hook di stream, berguna untuk log runner.

### 4. Instruksi

`CLAUDE.md` berisi `@AGENTS.md`, yaitu sintaks import CLAUDE.md. Pola ini sudah dipakai luas. Di sesi ini tidak diuji ulang secara terpisah.

### 5. MCP [docs mcp]

```json
{ "mcpServers": { "agentgate": { "type": "http", "url": "https://<server>/mcp" } } }
```

- OAuth: discovery RFC 9728 lalu RFC 8414. Mendukung DCR, CIMD (otomatis), dan client-id statis (`--client-id`, `--callback-port`).
- Docs menyebut "OAuth 2.0". Kata "2.1" tidak disebut secara harfiah, tetapi PKCE, discovery, dan DCR/CIMD adalah profil MCP auth.
- Token disimpan "securely" (keychain atau credentials file).
- Ekspansi env `${VAR}` / `${VAR:-default}` didukung.
- `headersHelper` bisa dipakai bila Agent Gate ingin memberi token runner tanpa flow browser. Ini opsi bagus untuk job headless: `agentgate mcp-token` mencetak `{"Authorization":"Bearer ..."}`.

### 6. Slash command [docs skills]

- `.claude/skills/ag-<stage>/SKILL.md` (disarankan) atau `.claude/commands/ag-<stage>.md` (format lama, masih jalan).
- Frontmatter: `description`, `argument-hint`, `allowed-tools`, `model`, `disable-model-invocation`. Argumen lewat `$ARGUMENTS` / `$N`.
- Di `claude -p "/ag-dev TICKET-12"`, skill pertama dimuat dan sisa baris menjadi argumen.

### 7. Output & usage [verified-lokal]

`--output-format json` menghasilkan satu objek `type:"result"`. Field relevan yang teramati:

- `session_id`, `num_turns`, `duration_ms`, `duration_api_ms`, `is_error`, `subtype`, `result`, `permission_denials`, `terminal_reason`, `stop_reason`.
- `total_cost_usd` (contoh: `0.0017216`).
- `usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens, output_tokens_details.thinking_tokens, server_tool_use}`.
- `modelUsage.<model>.{inputTokens, outputTokens, cacheReadInputTokens, cacheCreationInputTokens, costUSD, costBasis:"list", contextWindow}`.

Catatan:

- Untuk akun langganan, `costUSD` tetap diisi, dengan `costBasis: "list"`. Artinya perkiraan harga list, bukan tagihan. Halaman "Penghemat Token" perlu menyebut ini sebagai "perkiraan".
- `stream-json` memberi event realtime (`system/init`, assistant, tool use, `result`). Formatnya cocok untuk progress dan log runner.

### 8. Resume & interaktif

- `claude -r <session-id|nama> "<lanjutan>"`, `-c` (terakhir di cwd), `--fork-session`. `--session-id <uuid>` membuat runner bisa menentukan ID sejak awal, sehingga mudah di-resume setelah kuota reset.
- Stage interaktif: `claude "<prompt konteks tiket>"` membuka TUI dengan prompt awal. Alternatifnya `claude "/ag-<stage> <tiket>"`.

**Sumber:** `claude --help` lokal (2.1.319); https://code.claude.com/docs/en/cli-reference; https://code.claude.com/docs/en/hooks; https://code.claude.com/docs/en/settings; https://code.claude.com/docs/en/mcp; https://code.claude.com/docs/en/skills (diakses 2026-11-11).

## OpenAI Codex CLI (0.167.0, 2026-11-06)

Catatan sumber: `developers.openai.com/codex/*` kini redirect (308) ke `learn.chatgpt.com/docs/...`.

### 1. Headless

```bash
codex exec --json --ignore-user-config --ephemeral \
  -C <worktree> -s workspace-write -a never \
  -m <model> -c model_reasoning_effort=high \
  --dangerously-bypass-hook-trust \
  -o last.md - < prompt.md
```

- Prompt diberikan sebagai argumen, atau lewat stdin dengan `-`. Bila ada pipe dan argumen sekaligus, argumen menjadi instruksi dan pipe menjadi konteks.
- Sandbox: `-s read-only|workspace-write|danger-full-access`. Default exec adalah read-only.
- Approval: `-a on-request|never`. `--yolo` sama dengan `--dangerously-bypass-approvals-and-sandbox`.
- **`--full-auto` sudah tidak didukung.**
- Effort: `low|medium|high|xhigh|max|ultra`, tergantung model.

### 2. Isolasi

**`--ignore-user-config`** melewati `$CODEX_HOME/config.toml`. "auth still uses `CODEX_HOME`". Ini kandidat terbaik.

**Flag pendukung.** `--ignore-rules` melewati execpolicy `.rules` milik user dan project. `--strict-config` juga tersedia.

**Presedensi** (tinggi ke rendah): CLI/`-c`, lalu `.codex/config.toml` project (hanya project trusted), lalu profile, lalu `~/.codex/config.toml`, lalu `/etc/codex/config.toml`.

**Trust project.** Diatur di user config: `projects."<path>".trust_level="trusted"`. Bila folder untrusted, `.codex/` (config, hooks, rules) diabaikan.

- **Belum terverifikasi:** apakah dengan `--ignore-user-config` layer `.codex/` di worktree masih dimuat.
- **Belum terverifikasi:** apakah `-c 'projects."/wt".trust_level="trusted"'` bisa dipakai.
- Ini harus dites di M11.

**Auth.** `cli_auth_credentials_store = file|keyring|auto|ephemeral`. `CODEX_API_KEY` berlaku untuk exec. Mengganti `CODEX_HOME` memutus auth berbasis file.

### 3. Hooks

**Status dan lokasi.** Aktif default. Lokasinya `.codex/hooks.json` atau `[hooks]` di `.codex/config.toml` (project trusted), plus user, plugin, dan managed.

**Event.** `SessionStart`, `SessionEnd`, `SubagentStart`, `SubagentStop`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreModelRequest`, `PostModelRequest`, `PreCompact`, `PostCompact`, `UserPromptSubmit`, `Stop`, `Interrupt`.

**Format.** Mirip Claude: `{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"agentgate hook pre-tool --client codex"}]}]}`. Payload dikirim lewat stdin, berisi `session_id, cwd, hook_event_name, tool_name, tool_input, ...`.

**Block.** Exit 2, atau `permissionDecision:"deny"`.

**Rewrite input.** `permissionDecision:"allow"` + `updatedInput` (Bash wajib menyertakan `command`). `ask` belum didukung.

**Output.** Tidak bisa diganti. Yang tersedia hanya `additionalContext`, block feedback, dan `continue:false`. `updatedMCPToolOutput` tidak didukung.

**Trust hook.** Hook non-managed harus di-review lewat `/hooks` (trust per hash). **Headless wajib `--dangerously-bypass-hook-trust`**, kalau tidak hook tidak jalan.

### 4. Instruksi

`AGENTS.md` dibaca native. Codex menelusuri dari git root sampai cwd, satu file per direktori (`AGENTS.override.md` > `AGENTS.md` > `project_doc_fallback_filenames`), dengan batas default 32 KiB.

### 5. MCP

```toml
# .codex/config.toml (project, harus trusted)
[mcp_servers.agentgate]
url = "https://<server>/mcp"
# auth default "oauth"; alternatif bearer_token_env_var = "AGENTGATE_MCP_TOKEN"
```

- Login lewat `codex mcp login agentgate [--oauth-client-registration auto|cimd|dcr]`.
- Token disimpan di `mcp_oauth_credentials_store = auto|file|keyring`.
- Untuk headless, `bearer_token_env_var` atau `http_headers_helper` lebih aman daripada flow browser.

### 6. Slash command

- Custom prompts (`~/.codex/prompts`, hanya level user) **deprecated 2026-01-22**.
- Penggantinya skills di `.agents/skills/<n>/SKILL.md` (frontmatter `name`, `description`), dipanggil lewat `$n` atau `/skills`.
- `/ag-<stage>` dalam bentuk slash literal **tidak tersedia** di Codex.

### 7. Output

- `--json` menghasilkan JSONL: `thread.started`, `turn.started`, `turn.completed`, `turn.failed`, `item.*`, `error`.
- Usage ada di `turn.completed.usage{input_tokens, cached_input_tokens, output_tokens, reasoning_output_tokens}`. **Tanpa cost**, sehingga Agent Gate harus menghitung dari tabel harga.
- `-o <file>` untuk pesan terakhir, `--output-schema` untuk structured output.

### 8. Resume

- `codex exec resume --last "<lanjutan>"` atau `codex exec resume <id>`, dan `codex exec fork <id>`.
- Interaktif: `codex "<prompt>"`, `codex resume`.

**Sumber:** https://github.com/openai/codex/releases; https://learn.chatgpt.com/docs/non-interactive-mode; https://learn.chatgpt.com/docs/config-file/config-reference; https://learn.chatgpt.com/docs/hooks; https://learn.chatgpt.com/docs/agent-configuration/agents-md; https://learn.chatgpt.com/docs/extend/mcp?surface=cli; https://learn.chatgpt.com/docs/auth; https://developers.openai.com/codex/changelog; https://developers.openai.com/codex/skills.

## Google Gemini CLI (v0.67.0, 2026-11-04)

**Peringatan:** semua halaman geminicli.com menyatakan Gemini CLI "was replaced by Antigravity CLI on June 18th, 2026" untuk user unpaid/Google One, dan tidak dioptimasi untuk Gemini 4. Rilis masih berjalan (v0.68.0-preview.0).

### 1. Headless

```bash
cd <worktree>
GEMINI_CLI_TRUST_WORKSPACE=true gemini -p "$(cat prompt.md)" \
  --approval-mode yolo -m pro -o stream-json --skip-trust
```

- `-y/--yolo` deprecated, diganti `--approval-mode`. `--allowed-tools` deprecated, diganti Policy Engine.
- Working dir mengikuti cwd. `--include-directories` tersedia.
- Exit code: 0 ok, 1 error, 42 input salah, 53 batas turn.
- Thinking tidak punya flag. Diatur lewat settings `modelConfigs.aliases.<alias>.generateContentConfig.thinkingConfig` (`thinkingBudget` / `thinkingLevel`).
- **Folder trust aktif default.** Di folder untrusted, `.gemini/settings.json`, MCP, dan commands diabaikan, dan headless gagal dengan `FatalUntrustedWorkspaceError`.

### 2. Isolasi

- Presedensi: defaults, lalu system-defaults, lalu user `~/.gemini/settings.json`, lalu project, lalu system override (`GEMINI_CLI_SYSTEM_SETTINGS_PATH`), lalu env, lalu args.
- Runner bisa mengarahkan `GEMINI_CLI_SYSTEM_SETTINGS_PATH` ke file miliknya untuk **menimpa** key user. Cara ini tidak menghapus key user yang tidak ditimpa.
- `GEMINI_CLI_HOME` mengganti home (dikonfirmasi dari source `paths.ts`). Akibatnya `oauth_creds.json` ikut pindah, sehingga login Google putus kecuali disalin. `GEMINI_API_KEY` atau Vertex tidak terpengaruh.
- Tidak ada flag "skip user settings".
- Extension bisa dibatasi dengan `-e`. Apakah `-e none` bekerja belum terverifikasi.

### 3. Hooks

- Aktif default (`hooksConfig.enabled: true`).
- Event: `SessionStart`, `SessionEnd`, `BeforeAgent`, `AfterAgent`, `BeforeModel`, `AfterModel`, `BeforeToolSelection`, `BeforeTool`, `AfterTool`, `PreCompress`, `Notification`.
- Format:

  ```json
  {"hooks":{"BeforeTool":[{"matcher":"run_shell_command","hooks":[{"name":"agentgate","type":"command","command":"agentgate hook pre-tool --client gemini","timeout":5000}]}]}}
  ```

- Block: exit 2 (alasan dari stderr), atau `{"decision":"deny","reason":...}`.
- Rewrite input: `hookSpecificOutput.tool_input` di-merge ke argumen tool.
- Output tidak bisa diganti, kecuali lewat deny (yang menjadikan `reason` sebagai hasil), atau `additionalContext`.
- Env hook: `GEMINI_PROJECT_DIR`, `GEMINI_SESSION_ID`, `CLAUDE_PROJECT_DIR`.
- Apakah hook berjalan di folder untrusted belum terverifikasi.

### 4. Instruksi

`{"context":{"fileName":["AGENTS.md"]}}` di `.gemini/settings.json`. Key lama `contextFileName` sudah berubah menjadi `context.fileName`.

### 5. MCP

```json
{"mcpServers":{"agentgate":{"httpUrl":"https://<server>/mcp"}}}
```

- Transport ditentukan key: `httpUrl` untuk Streamable HTTP, `url` untuk SSE.
- OAuth terdeteksi otomatis saat menerima 401, dengan discovery dan DCR. Konfigurasi manual ada di blok `oauth{...}`.
- Token disimpan di `~/.gemini/mcp-oauth-tokens.json`.
- Login butuh browser (`/mcp auth`), jadi harus dilakukan interaktif sebelum headless. Alternatif: `headers` statis berisi token dari runner.

### 6. Slash command

- File `.gemini/commands/ag-<stage>.toml` menjadi `/ag-<stage>`. Subfolder menjadi namespace `/a:b`.

  ```toml
  description = "Agent Gate: stage Dev"
  prompt = "Panggil ag_context untuk stage dev, tiket {{args}}."
  ```

### 7. Output

- `-o json` menghasilkan `{response, stats, error?}`.
- `-o stream-json` menghasilkan event `init`, `message`, `tool_use`, `tool_result`, `error`, `result`. Statistik token per model ada di event `result`.
- Detail sub-field `stats` belum terverifikasi.

### 8. Resume

- `--resume latest|<idx>|<uuid>`, `--list-sessions`. Apakah bisa dikombinasikan dengan `-p` belum terverifikasi.
- Interaktif dengan prompt: `-i "<prompt>"`.

**Sumber:** https://github.com/google-gemini/gemini-cli/releases; https://geminicli.com/docs/cli/cli-reference; https://geminicli.com/docs/cli/headless; https://geminicli.com/docs/cli/trusted-folders; https://geminicli.com/docs/reference/configuration; https://geminicli.com/docs/hooks dan /hooks/reference; https://geminicli.com/docs/cli/gemini-md; https://geminicli.com/docs/tools/mcp-server; https://geminicli.com/docs/cli/custom-commands; https://geminicli.com/docs/cli/session-management; source `packages/core/src/utils/paths.ts`, `config/storage.ts`.

## Cursor CLI (v2026.10.22)

Docs memakai nama binary `agent`. Apakah alias `cursor-agent` masih ada belum terverifikasi. Adapter perlu mendeteksi keduanya.

### 1. Headless

```bash
agent -p "$(cat prompt.md)" --workspace <worktree> --trust --force \
  --approve-mcps --output-format stream-json --model <slug>
```

- Tanpa `--force`, `-p` hanya mengusulkan perubahan.
- Effort tidak punya flag tersendiri. Effort dipilih lewat slug model (mis. `...-thinking-max`), dan sejak 2026-05 slug tersebut dipertahankan di `-p`.
- Flag lain: `--sandbox enabled|disabled`, `--mode plan|ask`, `--add-dir`.
- Auth: `CURSOR_API_KEY` atau `--api-key`.

### 2. Isolasi

- Global config: `~/.cursor/cli-config.json`. Project hanya bisa mengatur permission lewat `.cursor/cli.json`.
- `CURSOR_CONFIG_DIR` dan `XDG_CONFIG_HOME` memindahkan cli-config. Apakah juga memindahkan `mcp.json`/`hooks.json`/skills belum terverifikasi.
- **Yang bocor:**
  - `~/.cursor/mcp.json` (tanpa approval sejak April 2026).
  - `~/.cursor/hooks.json`.
  - Hooks dari `~/.claude/settings.json` lewat fitur "Third-Party Imports" (aktif default).
  - Skills di `~/.cursor|.agents|.claude|.codex/skills`.
  - User Rules, serta team rules/hooks dari dashboard.
- Tidak ada flag untuk mengabaikan config global.
- Lokasi credential tidak didokumentasikan ("securely stored locally"). `CURSOR_API_KEY` menghindari ketergantungan pada login tersimpan.

### 3. Hooks

- Lokasi: `.cursor/hooks.json` (project), `~/.cursor/hooks.json`, `/etc/cursor/hooks.json`.
- Event: `sessionStart`, `sessionEnd`, `preToolUse`, `postToolUse`, `postToolUseFailure`, `subagentStart`, `subagentStop`, `beforeShellExecution`, `afterShellExecution`, `beforeMCPExecution`, `afterMCPExecution`, `beforeReadFile`, `afterFileEdit`, `beforeSubmitPrompt`, `preCompact`, `stop`, `afterAgentResponse`, `afterAgentThought`, `workspaceOpen`.
- Format:

  ```json
  {"version":1,"hooks":{"preToolUse":[{"command":"agentgate hook pre-tool --client cursor","matcher":"Shell","failClosed":false}]}}
  ```

- Block: exit 2, atau `permission: allow|deny|ask`. `ask` tidak dihormati di `preToolUse`, tetapi dihormati di `beforeShellExecution`.
- Rewrite input: `updated_input` lewat `preToolUse`.
- Rewrite output: hanya untuk MCP (`updated_mcp_tool_output`).
- `sessionStart` bisa mengembalikan `env` dan `additional_context`. `stop` bisa mengembalikan `followup_message`.
- Format output Claude (`hookSpecificOutput.updatedInput`) juga diterima.
- Hooks dihormati oleh CLI (changelog Januari dan April 2026, v2026.10.17).

### 4. Instruksi

CLI membaca `AGENTS.md` dan `CLAUDE.md` di root, ditambah `.cursor/rules/*.mdc`. `AGENTS.md` bertingkat didukung.

**Catatan:** karena Cursor membaca `CLAUDE.md` (berisi `@AGENTS.md`) sekaligus `AGENTS.md`, ada kemungkinan konten terbaca dua kali. Apakah sintaks `@` di-resolve belum terverifikasi. Dampaknya kecil karena isinya tipis.

### 5. MCP

```json
{"mcpServers":{"agentgate":{"url":"https://<server>/mcp","headers":{"Authorization":"Bearer ${env:AGENTGATE_MCP_TOKEN}"}}}}
```

- Lokasi `.cursor/mcp.json`. Mendukung Streamable HTTP dan SSE.
- OAuth PKCE lewat `agent mcp login <id>`. Blok `auth` statis tersedia (CLIENT_ID).
- Interpolasi `${env:X}` dan `${workspaceFolder}`.

### 6. Slash command

- Skill di `.cursor/skills/<n>/SKILL.md` atau `.agents/skills/` menjadi `/n`.
- `.cursor/commands/*.md` di CLI belum terverifikasi. Docs mendorong migrasi ke skills.

### 7. Output

- `json`: `{type:"result", subtype, is_error, duration_ms, result, session_id, request_id}`. Bila gagal, yang keluar non-zero + stderr, bukan JSON.
- `stream-json`: `system/init`, `user`, `assistant`, `tool_call` (started/completed), `result`.
- Changelog Februari 2026 menyebut token per turn (input/output/cache) di stream-json. Nama field belum terverifikasi. Tidak ada field cost.

### 8. Resume

- `--resume [chatId]`, `--continue`, `agent resume`, `agent ls`, `agent create-chat`.
- Interaktif: `agent "<prompt>"`.

**Sumber:** https://cursor.com/docs/cli/reference/parameters; https://cursor.com/docs/cli/headless; https://cursor.com/docs/cli/reference/configuration; https://cursor.com/docs/cli/reference/permissions.md; https://cursor.com/docs/cli/reference/authentication.md; https://cursor.com/docs/hooks.md; https://cursor.com/docs/reference/third-party-hooks.md; https://cursor.com/docs/cli/using.md; https://cursor.com/docs/rules.md; https://cursor.com/docs/mcp; https://cursor.com/docs/cli/mcp; https://cursor.com/docs/skills.md; https://cursor.com/docs/cli/reference/output-format; https://cursor.com/docs/cli/changelog.md.

## OpenCode (V2, v2.2.7, 2026-11-10)

Repo `sst/opencode` kini berada di `github.com/anomalyco/opencode`. V2 mengubah flag, key config, API plugin, dan lokasi auth dibanding V1. Detail di bawah diambil dari source dan docs-source di branch `dev` (`services/site/src/docs/content/`), karena halaman opencode.ai tipis.

### 1. Headless

```bash
cd <worktree>
OPENCODE_CONFIG_DIR=<dir-runner> opencode run --standalone --auto \
  -m anthropic/<model>#high --format json "$(cat prompt.md)"
```

- **Wajib `--standalone`.** Default-nya semua client terhubung ke satu server background bersama per user, yang membaca config dan env. Tanpa flag ini, env/config per job tidak berlaku.
- `--auto` (alias tersembunyi `--yolo`, `--dangerously-skip-permissions`) meng-approve semua yang tidak di-deny eksplisit. Tanpa `--auto`, permission `ask` ditolak di mode non-interaktif.
- Permission config (V2):

  ```json
  "permissions":[{"action":"shell","resource":"git push *","effect":"deny"}]
  ```

### 2. Isolasi

- `OPENCODE_CONFIG_DIR` mengganti dir global.
- `OPENCODE_CONFIG` / `OPENCODE_CONFIG_CONTENT` menyuntikkan config.
- `OPENCODE_DISABLE_CLAUDE_CODE*` menonaktifkan pembacaan `.claude`.
- Auth disimpan di SQLite `~/.local/share/opencode/opencode.db` (data dir), sehingga tidak ikut pindah oleh `OPENCODE_CONFIG_DIR` dan auth tetap aman.
- `/etc/opencode/` mengalahkan user dan project.
- Presedensi `OPENCODE_CONFIG_CONTENT` dibanding project config belum terverifikasi.

### 3. Hooks (plugin)

Tidak ada shell-hook config. Penggantinya plugin `.opencode/plugins/agentgate.ts`:

```ts
// sketsa — API V2, belum diuji
export default Plugin.define({ id: "agentgate", async setup(ctx) {
  ctx.tool.hook("execute.before", e => { /* spawn `agentgate hook pre-tool --client opencode`, ganti e.input */ });
  ctx.tool.hook("execute.after",  e => { /* opsional ganti e.result */ });
  ctx.permission.hook("evaluate", e => { /* e.effect = "deny" untuk aksi terlarang */ });
}});
```

- Mengganti input bisa. Mengganti output bisa (`e.result`).
- Cara block yang andal adalah lewat `permission.hook("evaluate")`. Block lewat throw di `execute.before` belum terverifikasi.
- `ctx.shell.hook("create.before")` bisa mengubah command shell.
- Nama hook V1 (`tool.execute.before`) tidak diterjemahkan ke V2.

### 4. Instruksi

Hanya `AGENTS.md` ("does not use CLAUDE.md as a fallback"). Tambahan bisa lewat `"instructions":[...]`.

### 5. MCP

```json
{"mcp":{"servers":{"agentgate":{"type":"remote","url":"https://<server>/mcp"}}}}
```

- Remote hanya mendukung Streamable HTTP. OAuth aktif default (PKCE), dengan urutan client_id, lalu CIMD, lalu DCR.
- Login: `opencode mcp auth agentgate`. Lokasi token belum terverifikasi.

### 6. Slash command

- `.opencode/commands/ag-<stage>.md` menjadi `/ag-<stage>`.
- Frontmatter: `description`, `agent`, `model`, `subagent`. Body mendukung `$ARGUMENTS`, `$1`, dan `` !`cmd` ``.
- Tidak ada `run --command` di V2.

### 7. Output

- `--format json` menghasilkan JSONL `{type,timestamp,sessionID,...}`.
- `step_finish` membawa `part.tokens` dan `part.cost`.
- Juga tersedia `opencode stats --json`, `opencode session export <id>`.

### 8. Resume

- `opencode run -c` atau `-s <id>`, plus `--fork`.
- Interaktif: `opencode <dir> --prompt "<p>"`.

**Sumber:** https://github.com/anomalyco/opencode (branch `dev`): `packages/cli/src/commands/commands.ts`, `packages/cli/src/server-process.ts`, `run/noninteractive.ts`, `packages/util/src/global.ts`, `core/src/config.ts`, docs `cli/index.mdx`, `cli/commands.mdx`, `permissions.mdx`, `instructions.mdx`, `mcp-servers.mdx`, `commands.mdx`, `models.mdx`, `build/plugins/server.mdx`, `cli/providers.mdx`.

## GitHub Copilot CLI (1.0.109, 2026-11-10)

### 1. Headless

```bash
GITHUB_COPILOT_PROMPT_MODE_WORKSPACE_MCP=true \
copilot -p "$(cat prompt.md)" -C <worktree> --trust-folder \
  --allow-all-tools --deny-tool='shell(git push --force:*)' --no-ask-user \
  --model <m> --reasoning-effort high --output-format json
```

- Prompt lewat `-p`, atau lewat pipe bila `-p` tidak dipakai.
- `--allow-tool`/`--deny-tool` memakai pola `Kind(arg)`. Deny menang atas allow.
- Flag lain: `--available-tools`, `--allow-all`/`--yolo`, `--autopilot`, `--secret-env-vars`.

### 2. Isolasi

- `COPILOT_HOME` mengganti seluruh `~/.copilot` (settings, mcp-config, agents, skills, hooks, session-state). `--config-dir` deprecated.
- **Yang tetap terbaca:**
  - `.github/copilot/settings*.json`.
  - **`.claude/settings.json`** (hooks, enabledPlugins). Ini berarti hooks Claude Agent Gate di worktree juga akan dieksekusi oleh Copilot.
  - Policy `/etc/github-copilot/policy.d`.
- Presedensi auth: `COPILOT_GITHUB_TOKEN` > `GH_TOKEN` > `GITHUB_TOKEN` > keychain (`copilot-cli`) > `gh auth token`. Fallback plaintext ada di `~/.copilot/config.json`.

### 3. Hooks

- Lokasi: `.github/hooks/*.json`, `$COPILOT_HOME/hooks`, `.github/copilot/settings*.json`, `.claude/settings*.json`, dan plugin.
- Format:

  ```json
  {"version":1,"hooks":{"preToolUse":[{"command":"agentgate","args":["hook","pre-tool","--client","copilot"],"timeoutSec":30}]}}
  ```

  Bentuk `bash`/`powershell`/`command`/`exec`+`args` tersedia.
- Event: `sessionStart`, `sessionEnd`, `userPromptSubmitted`, `userPromptTransformed`, `preToolUse`, `postToolUse`, `postToolUseFailure`, `permissionRequest`, `agentStop`, `subagentStart`, `subagentStop`, `errorOccurred`, `preCompact`, `notification`.
- `preToolUse`:
  - Mengembalikan `permissionDecision: allow|deny|ask` dan `modifiedArgs`.
  - Fail-closed bila error, fail-open bila timeout.
  - Bentuk JSON `modifiedArgs` dari script CLI belum terverifikasi, karena contoh di docs memakai SDK.
- `postToolUse` mengembalikan `modifiedResult` (ganti output) atau `additionalContext`.
- `sessionStart` mengembalikan `additionalContext`.
- `agentStop` dengan `decision:"block"` memaksa turn tambahan.

### 4. Instruksi

Semua file berikut digabung: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.github/instructions/**/*.instructions.md`. Tidak ada urutan presedensi. `--no-custom-instructions` mematikan semuanya.

### 5. MCP

- Lokasi: `.mcp.json` atau `.github/mcp.json`, hanya bila folder trusted. Format `{"mcpServers":{"agentgate":{"type":"http","url":"..."}}}`.
- Di `-p`, MCP workspace hanya dimuat bila folder sudah trusted atau `GITHUB_COPILOT_PROMPT_MODE_WORKSPACE_MCP=true`.
- `--additional-mcp-config=<json>` tersedia untuk per session.
- OAuth: DCR didukung (sejak 0.0.389, komentar maintainer issue #33). CIMD belum (issue #1305).
- Bug terbuka: `oauth.clientId` diabaikan (#2717), client secret statis tidak dikirim (#5351).

**Catatan:** `.mcp.json` dipakai bersama oleh Claude dan Copilot. Format `type:"http"` sama, jadi satu file cukup untuk keduanya.

### 6. Slash command

`.claude/commands/*.md` (nama file menjadi command), skill di `.github/skills/`, `.agents/skills/`, `.claude/skills/`, dan custom agent di `.github/agents/`.

### 7. Output

- `--output-format json` menghasilkan JSONL. Sejak 1.0.109 ada record hasil akhir, termasuk saat gagal awal.
- Event `session.usage_info` ada, tetapi field usage dan cost belum terverifikasi.
- `--max-ai-credits` tersedia sebagai batas.

### 8. Resume

- `--continue`, `--resume <id|nama>`, `--session-id`.
- `-i "<prompt>"` membuka sesi interaktif dan menjalankan prompt.
- `--trust-folder` tidak bisa dikombinasikan dengan resume.

**Sumber:** https://github.com/github/copilot-cli/releases; https://raw.githubusercontent.com/github/copilot-cli/main/changelog.md; https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference; https://docs.github.com/en/copilot/how-tos/copilot-cli/automate-copilot-cli/run-cli-programmatically; https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference; https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/authenticate-copilot-cli; https://docs.github.com/en/copilot/reference/hooks-reference; https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-custom-instructions; https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers; https://github.com/github/copilot-cli/issues/33, /1305, /2717, /5351.

## RTK (rtk-ai/rtk v1.2.0, 2026-11-08)

### Cara kerja

RTK adalah binary Rust yang membungkus perintah (`rtk git status`) dan mengompresi output-nya. RTK mengklaim penghematan 60–90%, dan `rtk gain` menampilkan statistiknya.

- **Mekanisme hook:** hook memanggil `rtk rewrite "<cmd>"` lalu me-rewrite **input** command sebelum dieksekusi.
  - Claude memakai `updatedInput`.
  - Cursor memakai `updated_input` lewat `rtk hook cursor`.
- **Fail-open:** bila rtk tidak ada, perintah asli yang jalan.
- **Yang tidak di-rewrite:** perintah dengan command substitution, heredoc, atau redirect.

### Integrasi resmi per client (`rtk init`)

| Client | Perintah | Mekanisme |
| --- | --- | --- |
| Claude Code | `rtk init -g [--auto-patch]` | Mem-patch settings global |
| Codex | `rtk init --codex` | Project: `.codex/hooks.json`, `AGENTS.md`, `RTK.md`. Rewrite hanya berlaku bila disertai `allow` eksplisit |
| Gemini | `rtk init --global --gemini` | `BeforeTool` |
| Cursor | `rtk init --global --agent cursor` | Hanya global. Varian project belum terverifikasi |
| Copilot | `rtk init --copilot` | `.github/hooks/rtk-rewrite.json` |
| OpenCode | `rtk init --global --opencode` | Plugin `~/.config/opencode/plugins/rtk.ts`, butuh OpenCode ≥1.18.29. Kompatibilitas dengan API plugin V2 belum terverifikasi |

Docs RTK tidak konsisten: halaman configuration masih mendaftarkan Codex sebagai "tanpa hook", bertentangan dengan supported-agents.md.

### Windows

- Instal lewat `winget install rtk-ai.rtk` atau zip `x86_64-pc-windows-msvc`.
- Sejak v0.37.2, hook berupa native binary untuk Claude, Cursor, Copilot, Codex.
- **Hook Gemini masih butuh bash.**
- Beberapa filter butuh `rg` di PATH.

### Lisensi dan instalasi

- CLI berlisensi Apache-2.0, tetapi pipeline engine dan `rtk-proxy` berlisensi BUSL-1.1. Edisi `rtk-oss` (Apache saja) tersedia lewat `RTK_EDITION=oss`.
- **`cargo install rtk` bisa memasang crate lain ("Rust Type Kit").** Yang benar: `cargo install --git https://github.com/rtk-ai/rtk`.

### Fallback

Mode `[awareness] level = "full"` menulis instruksi agar model memakai prefix `rtk`. Ini cocok dengan fallback di spec ("AI diinstruksikan memakai `rtk` lewat aturan MCP").

### Rekomendasi untuk Agent Gate

Jangan jalankan `rtk init -g`, karena perintah itu menulis ke config global user. Sebagai gantinya, `agentgate hook pre-tool` memanggil `rtk rewrite "<cmd>"` sendiri, lalu mengembalikan field rewrite sesuai dialek client (`updatedInput` / `updated_input` / `tool_input` / `modifiedArgs` / plugin OpenCode). Satu jalur ini juga menangani Windows untuk Gemini, karena hook-nya adalah binary `agentgate`, bukan bash.

**Sumber:** https://github.com/rtk-ai/rtk (README); `docs/guide/getting-started/supported-agents.md`, `installation.md`, `configuration.md`; `docs/guide/resources/agent-coverage.md`; `hooks/cursor/README.md`, `hooks/claude/README.md`; GitHub releases API.

## Risiko / yang belum terverifikasi

1. **Claude Code: cakupan `--setting-sources project`.** Belum terverifikasi apakah flag ini juga mematikan `~/.claude/CLAUDE.md`, user skills/agents/commands, plugin, dan auto-memory. Bila tidak, preferensi pribadi user bocor ke job. Uji di awal M5 lewat event `system/init` stream-json. Mitigasi tambahan yang bisa dipakai: `--disable-slash-commands` (mematikan *semua* skill, termasuk milik Agent Gate, sehingga tidak cocok), atau `--plugin-dir` kosong.
2. **Biaya Claude untuk akun langganan.** `total_cost_usd` adalah harga list (`costBasis:"list"`), bukan tagihan riil. UI harus memberi label "perkiraan".
3. **Workspace trust per client.** Ada beberapa hal yang belum terverifikasi:
   - Codex: interaksi `--ignore-user-config` dengan trust `.codex/`.
   - Gemini: apakah hook berjalan di folder untrusted.
   - Copilot: `--trust-folder` tidak bisa dipakai bersama resume.

   Semua harus dites end-to-end di M11.
4. **Isolasi tidak setara antar client.**
   - Cursor membaca hooks `~/.claude/settings.json` user secara default (Third-Party Imports).
   - Copilot membaca `.claude/settings.json` project, sehingga hooks Claude Agent Gate akan ikut jalan dua kali bila Copilot juga punya `.github/hooks`. `agentgate hook` harus menerima flag `--client` dan idempoten, atau hooks Claude harus memeriksa env client.
   - Gemini dan Cursor tidak punya cara skip user config tanpa mengganti home.
5. **Auth saat home dipindah.** `GEMINI_CLI_HOME`, `COPILOT_HOME`, `CODEX_HOME`, dan `CURSOR_CONFIG_DIR` berisiko memutus login berbasis file. Opsi:
   - Symlink credentials. Di Windows, symlink butuh izin khusus.
   - Memakai token env (`CODEX_API_KEY`, `COPILOT_GITHUB_TOKEN`, `CURSOR_API_KEY`, `GEMINI_API_KEY`). Konsekuensinya, langganan user mungkin tidak berlaku sama untuk API key (Codex/Gemini).
6. **OAuth MCP di job headless.** Semua client butuh browser untuk login OAuth pertama. Rekomendasi: login dilakukan sekali secara interaktif saat `agentgate init`/runner start. Untuk job, pakai header bearer dari runner:
   - `headersHelper` (Claude), `http_headers_helper`/`bearer_token_env_var` (Codex), `headers` (Gemini/Cursor/OpenCode/Copilot).
   - Server MCP Agent Gate harus menerima token runner selain OAuth 2.1 user. Ini keputusan arsitektur M4.
7. **Copilot OAuth.** CIMD belum didukung, sehingga server Agent Gate wajib mendukung DCR. Bug `clientId` (#2717) dan secret statis (#5351) masih terbuka.
8. **Gemini CLI digantikan Antigravity CLI** untuk user gratis/Google One. Dukungan jangka panjang belum jelas. Antigravity CLI belum diriset.
9. **OpenCode V2** mengubah hampir semua API. Plugin RTK resmi menyasar `opencode ≥1.18.29`, dan kompatibilitasnya dengan V2 belum terverifikasi. `--standalone` wajib.
10. **Cursor:** nama binary (`agent` vs `cursor-agent`), field token di stream-json, dan `.cursor/commands` di CLI belum terverifikasi.
11. **Field usage Copilot dan Gemini** belum terverifikasi. Agent Gate mungkin harus menampilkan "tidak tersedia" untuk client tertentu.
12. **Duplikasi instruksi:** Cursor dan Copilot membaca `CLAUDE.md` *dan* `AGENTS.md`. Dampaknya kecil karena isinya satu baris, tetapi resolusi `@AGENTS.md` oleh client non-Claude belum terverifikasi.
13. **Bahaya `--dangerously-bypass-hook-trust` (Codex) dan `--trust`/`--trust-folder`.** Flag ini juga meng-trust hooks/config lain di repo, yang bisa saja dibuat oleh kontributor jahat. Runner harus memverifikasi checksum file aturan sebelum run. Ini sejalan dengan "cek drift" di spec.

## Dampak ke milestone

**M4 MCP & persona**

- Server MCP Agent Gate wajib mendukung Streamable HTTP dengan discovery RFC 9728 dan 8414, PKCE, **DCR** (wajib untuk Copilot), dan sebaiknya **CIMD** (Claude, Codex, OpenCode).
- Tambahkan jalur token bearer untuk runner (dipakai lewat `headersHelper`/`bearer_token_env_var`), supaya job headless tidak butuh browser.
- Slash command dibuat sebagai **skill** (`SKILL.md`) di `.claude/skills/` dan `.agents/skills/`, plus `.gemini/commands/*.toml` dan `.opencode/commands/*.md`. Generator perlu satu model data per command dengan beberapa renderer.
- Koreksi spec: key Gemini adalah `context.fileName`.
- Koreksi spec: Codex tidak punya slash command project, sehingga `/ag-<stage>` di Codex menjadi `$ag-<stage>`.

**M5 CLI & runner, adapter Claude Code**

- Perintah baku:

  ```bash
  claude -p --output-format stream-json --verbose --include-hook-events \
    --setting-sources project --strict-mcp-config --mcp-config .mcp.json \
    --session-id <uuid> --model ... --effort ...
  ```

  Permission ditentukan oleh hook `PreToolUse`/`PermissionRequest` → `agentgate hook`.
- Jangan menyentuh `CLAUDE_CONFIG_DIR`/`HOME`. Teruskan env user apa adanya.
- `SessionStart` mengembalikan `additionalContext` (maks 10k karakter).
- Hook berbentuk exec (`command: "agentgate"` + `args`) supaya jalan di Windows tanpa Git Bash.
- RTK: `agentgate hook pre-tool` memanggil `rtk rewrite`, lalu mengembalikan `updatedInput` + `allow`. Opsional, `updatedToolOutput` di `PostToolUse` untuk kompresi output tambahan.
- Usage dan biaya diambil dari event `result` (`usage`, `modelUsage`, `total_cost_usd`).
- Kuota habis: simpan `session_id`, lalu lanjutkan dengan `claude -r <id> -p "lanjutkan"`.
- Stage interaktif: `claude "/ag-<stage> <tiket>"` di terminal.
- Tugas pertama M5: uji kebocoran `--setting-sources project` (risiko #1).

**M11 adapter multi-client**

- Tiap adapter perlu matriks berikut:
  - Perintah headless.
  - Langkah trust: `--trust` / `--skip-trust` / `--trust-folder` / trust Codex + `--dangerously-bypass-hook-trust`.
  - Strategi isolasi (best effort, didokumentasikan ke user).
  - Strategi auth (env token vs file).
  - Dialek hook (nama event dan field rewrite).
  - Parser output (JSONL) dan normalisasi usage. Untuk Codex, cost dihitung dari tabel harga. Untuk Copilot dan Cursor, field usage masih perlu diverifikasi.
- OpenCode butuh file plugin TS yang di-generate, bukan JSON hooks, dan wajib `--standalone`.
- Gemini dan OpenCode masuk daftar risiko. Putuskan sebelum M11 apakah Gemini diganti atau ditambah Antigravity CLI (perlu spike terpisah).
- `agentgate hook` harus idempoten terhadap pemanggilan ganda, karena Copilot dan Cursor juga mengeksekusi hooks dari `.claude/settings.json`.
