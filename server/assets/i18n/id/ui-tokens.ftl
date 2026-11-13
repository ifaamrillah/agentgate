## M10: Penghemat token

## Audit
audit-action-project-token-saver-updated = Penghemat token project diubah
audit-action-office-look-updated = Tampilan karakter AI Office diubah

## Halaman
tokens-title = Penghemat token
tokens-description = Semua penghematan token untuk { $project }. Pemakaian dan perkiraan biaya berasal dari laporan client user lewat runner.
tokens-saved = Pengaturan penghemat token disimpan.
tokens-panel-summary = Ringkasan
tokens-panel-summary-desc = 30 hari dan tren mingguan
tokens-panel-features = Fitur penghemat
tokens-panel-features-desc = Wajib dan opsional
tokens-panel-tickets = Per tiket
tokens-panel-tickets-desc = Tiket paling boros
tokens-panel-jobs = Per job
tokens-panel-jobs-desc = Dilaporkan client
tokens-summary-description = Penghematan token 30 hari terakhir dan tren 8 minggu.
tokens-top = { $feature } menyumbang penghematan terbesar.
tokens-top-detail = { $value } token dari { $total } yang dihemat 30 hari terakhir.
tokens-see-features = Lihat fitur
tokens-stat-saved = Token dihemat
tokens-stat-saved-hint = { $value }% dari pemakaian tanpa penghemat
tokens-stat-saved-cost = Biaya dihemat
tokens-stat-saved-cost-hint = Perkiraan dari tarif rata-rata
tokens-stat-used = Token terpakai
tokens-stat-used-hint = Input + output + cache
tokens-stat-cost = Perkiraan biaya
tokens-stat-cost-hint = { $value }
tokens-week-title = Token per minggu
tokens-legend-used = Terpakai
tokens-legend-saved = Dihemat (perkiraan tanpa penghemat)
tokens-empty = Belum ada pemakaian
tokens-empty-description = Angka muncul setelah runner menyelesaikan job dan client melaporkan pemakaian token.
tokens-features-description = Fitur wajib selalu aktif. Fitur opsional bisa dimatikan per project. Gaya ringkas dan model diatur per persona.
tokens-personas = Persona
tokens-not-used = Tidak dipakai: proxy kompresi level API dan anggaran token per job.
tokens-required = Wajib
tokens-optional = Opsional
tokens-required-on = { $feature } wajib aktif dan tidak bisa dimatikan
tokens-saved-30 = hemat
tokens-unmeasured = tidak terukur
tokens-tickets-description = Tiket dengan pemakaian terbesar 30 hari terakhir.
tokens-jobs-description = Dilaporkan client (Claude Code, Codex CLI, Gemini CLI) lewat runner. 50 job terbaru.
tokens-input = Input
tokens-output = Output
tokens-cache = Cache
tokens-ticket-tip = { $code }: input { $input }, output { $output }, cache { $cache } · { $cost }
tokens-col-job = Job
tokens-col-ticket = Tiket
tokens-col-client = Persona · client
tokens-col-saved = Dihemat
tokens-col-cost = Biaya
tokens-running = Sedang berjalan
tokens-failed = gagal

## Fitur
tokens-feat-code-graph = Code graph built-in
tokens-feat-code-graph-desc = AI menjelajah lewat MCP graph, bukan grep atau membaca file utuh.
tokens-feat-rtk = RTK
tokens-feat-rtk-desc = Output perintah (test log, git status, build log) dikompresi lewat hook. Bila hook client tidak mendukung, AI memakai rtk lewat aturan MCP.
tokens-feat-mcp-rules = Aturan lewat MCP
tokens-feat-mcp-rules-desc = Instruksi diambil per stage, bukan semua aturan sekaligus.
tokens-feat-handoff = Serah terima ringkas
tokens-feat-handoff-desc = Agent berikutnya menerima ringkasan artifact stage sebelumnya, bukan transkrip.
tokens-feat-layered = Respons MCP berlapis
tokens-feat-layered-desc = Ringkasan dulu, detail diambil hanya saat diminta.
tokens-feat-stable-prefix = Prefix konteks stabil
tokens-feat-stable-prefix-desc = Supaya prompt cache client sering kena.
tokens-feat-concise = Gaya ringkas
tokens-feat-concise-desc = Per persona. Hanya untuk chat dan log; artifact tetap ditulis lengkap.
tokens-feat-persona-models = Model per persona
tokens-feat-persona-models-desc = Model murah untuk tugas ringan, sesuai pilihan model di tiap persona.
