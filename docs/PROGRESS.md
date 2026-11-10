# Progres Pengerjaan Agent Gate

Sumber kebenaran status milestone. Diperbarui setiap milestone selesai atau ada keputusan baru.

## Status milestone

| Milestone | Status | Catatan |
|---|---|---|
| M0 Fondasi | selesai (2026-11-11) | `bin/ci` hijau, `/_health` jalan, 3 spike tertulis |
| M1 Identitas & tenancy | belum mulai | |
| M2 Tiket & board | belum mulai | |
| M3 Workflow & gate | belum mulai | |
| M4 MCP & persona | belum mulai | |
| M5 CLI & runner | belum mulai | |
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
| 2026-11-11 | Editor WYSIWYG: **Milkdown Crepe 7.23.0** (bundle ESM dari esm.sh, `deps=codemirror@6.0.2`), tanpa import map | Hasil spike; lihat `docs/spikes/2026-11-11-wysiwyg-tanpa-bundler.md` |

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

- **M4/M5**: layer `html_errors` mengubah *semua* respons error non-HTML menjadi halaman HTML. Sebelum endpoint
  JSON (MCP, API runner, API publik) ada, batasi layer ini ke request browser (`Accept: text/html`) atau
  kecualikan prefix API.
- Daftar pengecualian CSRF ada di `server/src/controllers/middleware/csrf.rs` (`EXEMPT_PATHS`), bukan di `app.rs`
  seperti di rencana M0 — satu tempat yang sama dengan aturannya.

## Perlu dari user (dikumpulkan untuk akhir)

- Remote GitHub untuk repo ini (CI GitHub Actions baru jalan setelah di-push).
- Pilihan gaya visual Basecoat (Vega default / Nova / Maia / Lyra / Mira / Luma / Sera / Rhea) — ditanyakan saat UI M1 dibangun.

## Catatan untuk sesi berikutnya

- Rencana milestone: `docs/superpowers/plans/`. Sumber dependency Loco/Tera untuk referensi API ada di `tmp/src/`
  (gitignored; salin ulang dari `/usr/local/cargo/registry/src` di container bila hilang).
- Screenshot UI memakai Playwright dari npm di scratchpad (Playwright MCP tidak bisa start: Chrome tidak terpasang).
