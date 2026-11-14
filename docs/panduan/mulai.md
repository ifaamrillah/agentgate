# Mulai memakai Agent Gate

1. **Daftar** di `https://<server>/register` (atau masuk dengan Google/GitHub), verifikasi email,
   lalu buat workspace. Undang tim dari Pengaturan → Anggota.
2. **Buat project** (kode pendek, mis. `KSR`), pilih workflow, dan hubungkan repo GitHub di
   Pengaturan project → Git.
3. **Instal CLI** dari `https://<server>/install`, lalu di folder repo:
   ```sh
   agentgate login --server https://<server>
   agentgate init --project KSR
   agentgate runner start
   ```
4. **Buat tiket** di board. Saat tiket masuk stage yang dikerjakan agent, job muncul di runner
   laptop executor stage itu; hasilnya (diff, PR, evidence tes) kembali ke tiket untuk direview.
5. **Hubungkan client AI** (Claude Code, Cursor, Codex) ke MCP server: Akun → Hubungkan client AI,
   salin URL `https://<server>/mcp`; login lewat OAuth di browser.
6. **Knowledge** project ada di tab Knowledge dan di `.agentgate/knowledge/` repo
   (`agentgate knowledge pull|sync`).

Perbarui CLI: `agentgate self-update`. Periksa perangkat: `agentgate doctor`.
