# R4 Revisi — Gemini CLI diganti Antigravity CLI

Sumber: `docs/REVIEW-AKHIR.md` §3.0 baris **R4** (keputusan pemilik 2026-11-14). Branch
`feature/r4-antigravity` dari `staging`. TDD, tes tertarget selama kerja; suite penuh + `bin/ci`
hanya sebelum merge.

## Ringkasan riset (2026-11-14)

Semua fakta dari dokumentasi resmi `antigravity.google/docs` kecuali ditandai lain. "Tidak
terdokumentasi" = tidak ditemukan di docs resmi; diperlakukan toleran dan masuk daftar verifikasi.

| Topik | Temuan | Sumber |
| --- | --- | --- |
| Binary & instal | `agy`. `curl -fsSL https://antigravity.google/cli/install.sh \| bash` (macOS/Linux) → `~/.local/bin/agy`; Windows `irm https://antigravity.google/cli/install.ps1 \| iex` → `%LOCALAPPDATA%\agy\bin`. Self-updater di latar; `AGY_CLI_DISABLE_AUTO_UPDATE=true` mematikannya. | https://antigravity.google/docs/getting-started/ , https://antigravity.google/docs/cli/troubleshooting/ |
| Pengganti Gemini CLI | Halaman migrasi resmi: `GEMINI.md`/`AGENTS.md` tetap dibaca, skill project pindah `.gemini/skills/` → `.agents/skills/`, MCP pindah dari `~/.gemini/settings.json` ke `mcp_config.json` (`serverUrl`, bukan `httpUrl`). | https://antigravity.google/docs/cli/gcli-migration/ |
| Headless | `agy -p "<prompt>"` (alias `--print`/`--prompt`); stateless; respons ke stdout, diagnostik ke stderr. `--output-format text\|json\|stream-json`; `--input-format stream-json` untuk multi-giliran lewat stdin; `--print-timeout`; resume `-c`/`--conversation <id>`. Login memakai kredensial cache (sekali login interaktif); tanpa login → error `authentication required`. Alternatif: `GEMINI_API_KEY` + `modelProvider: "gemini"`, atau ADC (`AGY_ADC_AUTH=true`). | https://antigravity.google/docs/cli/headless/ , https://antigravity.google/docs/install/ |
| Format stream-json | NDJSON, field `event`: `init` (`cwd`, `tools`, `permission_mode`, `model`/`agent` bila di-set), `step_update` (`step_index`, `state` `ACTIVE`/`DONE`, `step_type` `user_input`/`agent_response`/`tool`/`checkpoint`, opsional `tool_name`, `text_delta`, `usage`, `tool_info{name, parameters, output, error{type,message}}`, `subagent_info`), `result` (sama dengan amplop JSON: `conversation_id`, `status` `SUCCESS`/`ERROR`/`CANCELED`/`INTERRUPTED`/`INVALID`/`WAITING`/`RUNNING`, `response`, `error`, `duration_seconds`, `num_turns`, `usage`). | https://antigravity.google/docs/cli/headless/ |
| Token usage | `usage{input_tokens, output_tokens, thinking_tokens, cache_read_tokens, total_tokens}`; `total = input + output` (thinking termasuk output; cache_read terpisah dari input, terlihat di contoh multi-giliran). Tidak ada biaya. | https://antigravity.google/docs/cli/headless/ |
| Izin di headless | Tanpa prompt; aksi yang butuh persetujuan di-*soft-deny* (run lanjut, exit 0, catatan di stderr). `--dangerously-skip-permissions` menyetujui semua; `--mode accept-edits\|auto\|plan`; `--sandbox`. | https://antigravity.google/docs/cli/headless/ , https://antigravity.google/docs/cli/reference/ |
| Model & effort | `--model <slug>` (daftar `agy models`, mis. `gemini-4-carbon-high`, `gemini-3.8-flash-medium`, `gemini-3.1-pro-high`); `--effort low\|medium\|high\|xhigh\|max` (tergantung model). Model tak dikenal di headless → exit 1, status `ERROR`, "invalid model selection". | https://antigravity.google/docs/cli/headless/ |
| MCP | Project `.agents/mcp_config.json`, global `~/.gemini/config/mcp_config.json`, `{"mcpServers": {"x": {"serverUrl": "…"}}}` (Streamable HTTP; `url` juga diterima; `httpUrl`/SSE tidak). OAuth 2.0 + PKCE otomatis: client id manual → CIMD (`https://antigravity.google/oauth/client-metadata.json`) → DCR; redirect `https://antigravity.google/oauth-callback`. Login: tombol Authenticate (App) lalu tempel kode; di CLI lewat `/mcp` (overlay manajer MCP). Tidak ada `agy mcp add`/`login`. | https://antigravity.google/docs/mcp/ |
| Hook | Project `.agents/hooks.json`, global `~/.gemini/config/hooks.json`. Bentuk `{ "<nama>": { "PreToolUse": [{matcher, hooks:[{type, command, timeout(detik, bawaan 30)}]}], "SessionStart": [{type, command}] } }`. Event: `SessionStart`, `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop`. Payload camelCase: `toolCall{name, args}`, `stepIdx`, `conversationId`, `workspacePaths`, … Shell = `run_command` (`args.CommandLine`, `args.Cwd`). Jawaban `decision` `allow`/`auto_approve`/`deny`/`ask`/`force_ask`/`deny_unless_prior_grant` + `reason`; tulis ulang input lewat `overwrite` (merge dangkal ke `args`). `SessionStart` → `injectSteps: [{systemMessage: {systemMessage}}]`. Exit code hook tidak terdokumentasi. | https://antigravity.google/docs/hooks/ |
| File aturan | `AGENTS.md`/`GEMINI.md` (juga `.agents/AGENTS.md`, `.agents/rules/*.md` dengan frontmatter) di folder kerja dan induknya sampai root repo; kumulatif. Global `~/.gemini/AGENTS.md`, `~/.gemini/GEMINI.md`, `~/.gemini/config/…`. Batas 24 KB per file. | https://antigravity.google/docs/rules/ , https://antigravity.google/docs/customization-discovery/ |
| Skill / slash command | `.agents/skills/<n>/SKILL.md` (sudah dibaca Codex/Cursor/Copilot/OpenCode) → muncul sebagai slash command. `.gemini/commands/*.toml` tidak dibaca. | https://antigravity.google/docs/skills/ , https://antigravity.google/docs/cli/gcli-migration/ |
| Interaktif | `agy -i "<prompt>"` (`--prompt-interactive`): prompt awal lalu sesi berlanjut. | https://antigravity.google/docs/cli/reference/ |
| Trust | Saat pertama dijalankan, CLI menanyakan trust folder; disimpan di `trustedWorkspaces` (`~/.gemini/antigravity-cli/settings.json`). Tidak ada flag trust; perilaku headless pada folder belum trusted **tidak terdokumentasi**. | https://antigravity.google/docs/getting-started/ , https://antigravity.google/docs/settings/ |
| Kuota habis | **Tidak terdokumentasi resmi.** Laporan pengguna di forum Google: HTTP 429 `RESOURCE_EXHAUSTED` / "Resource has been exhausted (e.g. check quota)". Setting `useG1Credits` memakai AI credits setelah kuota plan habis. | https://discuss.ai.google.dev/t/antigravity-agent-execution-termination-error-http-429-resource-exhausted/181274 , https://antigravity.google/docs/cli/credits/ |
| Rencana/todo | Tidak ada tool todo yang terdokumentasi (daftar tool di halaman hook); rencana berupa artifact Markdown. | https://antigravity.google/docs/hooks/ , https://antigravity.google/docs/cli/artifacts/ |

## Keputusan desain

1. Kunci client `antigravity`, nama "Antigravity CLI", binary `agy`
   (`AGENTGATE_ANTIGRAVITY_BIN`). Variant `ClientKind::Gemini` dihapus (bukan alias).
2. **Headless**: `agy -p <prompt> --output-format stream-json --dangerously-skip-permissions
   [--model M] [--effort E]`, env `AGY_CLI_DISABLE_AUTO_UPDATE=true`. Izin aksi berbahaya tetap
   lewat hook `PreToolUse` (`deny` = blok keras). Interaktif: `agy [--model M] [--effort E] -i <prompt>`.
3. **Parser**: teks dari `text_delta` `agent_response` (di-flush saat `DONE`), tool dicatat
   sekali per `step_index` (`call_mcp_tool` → `server · tool`), error tool dari
   `tool_info.error`, hasil dari `result` (`status != SUCCESS` = error; `response` = teks akhir),
   usage dari `result.usage` (cadangan: jumlah usage per langkah). Tidak ada langkah rencana.
4. **Aturan v3**: `.gemini/settings.json` diganti `.agents/mcp_config.json` + `.agents/hooks.json`.
   `init` ulang mengenali `.gemini/settings.json` milik Agent Gate v2 (isi persis sama untuk URL
   MCP mana pun) → dihapus tanpa backup; isi lain (diubah user) → backup seperti aturan lama.
   `uninstall`, `rules check`, `rules sync` juga membersihkan file usang milik Agent Gate.
   `.agents/rules/`, `.agent/rules/`, `.agents/hooks.json`, `.agents/mcp_config.json`,
   `.agents/rules.json` ikut dianggap aturan lama milik user.
5. **Hook**: payload `toolCall.name`/`toolCall.args` dinormalkan; perintah dari `CommandLine`;
   jawaban `decision` (`auto_approve` untuk izinkan, `deny` + `reason`), RTK lewat
   `overwrite.CommandLine`; `session-start` → `injectSteps`.
6. **Hubungkan**: isi file `~/.gemini/config/mcp_config.json`; login lewat `/mcp` di sesi
   Antigravity. OAuth server sudah mendukung DCR + redirect https → kompatibel tanpa perubahan.
7. **Migrasi DB** `m20261129_000001_r4_antigravity`: `runners.client`,
   `runner_prefs.default_client`, kunci `personas.models` `gemini` → `antigravity` (model katalog
   Gemini CLI lama `gemini-2.5-pro`/`gemini-2.5-flash` dikosongkan karena bukan slug Antigravity;
   model lain dipertahankan). Riwayat `jobs.client` tidak diubah (yang benar-benar dijalankan).
   `down` membalik pemetaan.
8. Kuota: tambah pola "resource has been exhausted".

## Urutan kerja

1. core: katalog client + aturan v3 (tes dulu).
2. cli: adapter `antigravity.rs` (tes parser dari contoh docs), dialek hook, install/uninstall/drift.
3. server: Hubungkan, Fluent, migrasi + tes, tes runner.
4. E2E CLI (`runner_runs_jobs_with_every_client`, hook, install).
5. Dokumen, cek visual Playwright, `bin/ci`, merge `--no-ff`, `bin/ci`.
