## M6: Git & PR

## Audit
audit-action-git-connected = Provider git dihubungkan
audit-action-git-updated = Token provider git diganti
audit-action-git-disconnected = Provider git diputus
audit-action-repo-linked = Repo ditautkan ke provider
audit-action-project-git-updated = Konvensi git project diubah
audit-action-pull-request-merged = PR di-merge otomatis
audit-subject-git-connection = Provider git
audit-subject-pull-request = Pull request

## Notifikasi
notification-pr_opened = membuka PR untuk tiket ini
notification-ci_failed = CI PR tiket ini gagal
notification-pr_merged = PR tiket ini sudah di-merge
notification-merge_failed = Merge otomatis gagal. Merge PR secara manual di provider

## Validasi
git-branch-pattern-invalid = Pola wajib memuat {"{"}kode{"}"} dan hanya huruf, angka, / - _ . serta variabel {"{"}kode{"}"} {"{"}slug{"}"} {"{"}tipe{"}"}.
git-check-invalid = Maksimal 20 nama check, masing-masing maksimal 100 karakter.

## Izin
permission-manage-integrations = Atur integrasi
permission-manage-integrations-desc = Menghubungkan provider git dan integrasi luar ke workspace.

## Error provider
git-token-invalid = Token ditolak provider. Pastikan token masih berlaku dan punya scope repo.
git-unreachable = Provider git tidak bisa dihubungi. Coba lagi beberapa saat lagi.
git-url-invalid = URL server harus https dan berisi host, misalnya https://github.perusahaan.local.
git-not-connected = Repo ini belum terhubung ke provider git. Hubungkan provider di Pengaturan → Git provider.
git-provider-rejected = Provider menolak permintaan. Periksa izin token dan pengaturan repo.
git-app-not-configured = GitHub App belum diatur di server ini atau kunci privatnya tidak valid.
git-app-install-invalid = Instalasi GitHub App tidak bisa diverifikasi. Mulai lagi dari tombol Pasang di GitHub.
git-app-suspended = Instalasi GitHub App ditangguhkan pemilik akun GitHub.
git-app-repo-not-granted = Repo ini belum diberi akses ke GitHub App. Tambahkan di pengaturan instalasi App di GitHub.
git-repo-not-found = Repo tidak ditemukan di provider atau token tidak punya akses ke repo itu.

## Status PR dan CI
pr-state-open = Terbuka
pr-state-draft = Draft
pr-state-closed = Ditutup
pr-state-merged = Merged
ci-state-none = Belum ada CI
ci-state-pending = Berjalan
ci-state-success = Lulus
ci-state-failure = Gagal
pr-more = +{ $n } PR

## Pengaturan → Git provider
git-title = Git provider
git-description = Hubungkan provider tempat repo project disimpan. Satu workspace boleh memakai beberapa provider.
git-panel-cloud = Cloud
git-panel-cloud-desc = GitHub, GitLab, Bitbucket
git-panel-enterprise = Versi perusahaan
git-panel-enterprise-desc = Server milik perusahaan
git-panel-repos = Repo terhubung
git-panel-repos-desc = Webhook per repo
git-stat-providers = Provider terhubung
git-stat-repos = Repo GitHub
git-stat-webhooks = Webhook aktif
git-stat-broken = Webhook bermasalah
git-cloud-description = Server Agent Gate menerima webhook PR dan CI langsung dari provider.
git-enterprise-description = Server Git milik perusahaan. Server Agent Gate harus bisa menjangkau server itu, dan sebaliknya untuk webhook.
git-repos-description = Webhook dan event terakhir per repo. Repo ditambahkan dari pengaturan project.
git-repos-empty = Belum ada repo GitHub
git-repos-empty-description = Tambahkan repo di Pengaturan project → Repo. Repo langsung terhubung bila provider-nya sudah dihubungkan.
git-repos-search = Cari repo atau project…
git-col-repo = Repo
git-col-project = Project
git-col-provider = Provider
git-col-webhook = Webhook
git-col-last-event = Event terakhir
git-col-actions = Aksi
git-webhook-active = Aktif
git-webhook-error = Gagal dipasang
git-webhook-none = Belum terhubung
git-no-event = Belum ada event
git-relink = Pasang ulang
git-connected = Terhubung
git-connected-detail = Akun { $account } · { $host } · { $n } repo
git-last-event = event terakhir
git-provider-github-hint = github.com, lewat GitHub App (disarankan) atau personal access token
git-provider-github_enterprise-hint = GitHub Enterprise Server di jaringan perusahaan
git-connect = Hubungkan
git-connect-title = Hubungkan { $provider }
git-replace-token = Ganti token
git-replace-token-title = Ganti token { $provider }
git-connect-description = Token disimpan terenkripsi dan tidak pernah ditampilkan lagi. Repo di host ini langsung terhubung dan webhook dipasang otomatis.
git-connect-submit = Hubungkan
git-server-url = URL server
git-server-url-hint = Alamat web GitHub Enterprise Server. API dipanggil di /api/v3.
git-token = Personal access token
git-token-hint = Fine-grained token atau token classic dengan scope repo dan admin:repo_hook.
git-scopes = Yang dipakai Agent Gate
git-scope-read = Baca metadata repo
git-scope-pr = Buka dan merge pull request
git-scope-ci = Baca status CI dan check
git-scope-hooks = Pasang webhook PR dan CI
git-webhook-manual = Webhook manual
git-webhook-manual-description = Bila webhook gagal dipasang otomatis, pasang sendiri di pengaturan repo provider dengan data ini.
git-webhook-url = Payload URL
git-webhook-secret = Secret
git-webhook-events = Event
git-webhook-type = Content type
git-disconnect = Putuskan
git-disconnect-title = Putuskan { $provider }?
git-disconnect-description = Token dihapus dan repo tidak lagi menerima status PR dan CI. Webhook di provider tetap ada sampai Anda menghapusnya; event dari sana ditolak.
git-flash-connected = Provider git terhubung. Webhook dipasang untuk repo yang cocok.
git-flash-disconnected = Provider git diputus.
git-flash-relinked = Webhook terpasang.
git-flash-saved = Konvensi git disimpan. Berlaku untuk job berikutnya.
git-flash-app-connected = GitHub App terpasang. Repo yang diberi akses langsung terhubung.
git-flash-app-refreshed = Izin dan daftar repo GitHub App diperbarui.

## GitHub App (R2)
git-connect-ways = Pilih cara menghubungkan. GitHub App disarankan: token berumur pendek, izin per repo, dan satu webhook untuk semua repo.
git-way-app = GitHub App
git-way-recommended = Disarankan
git-way-app-desc = Pasang App Agent Gate di akun atau organisasi GitHub Anda, lalu pilih repo yang boleh diakses. Tidak ada token yang perlu disimpan.
git-way-token = Personal access token
git-way-token-desc = Cadangan bila App tidak bisa dipasang. Token disimpan terenkripsi dan webhook dipasang per repo.
git-app-install = Pasang di GitHub
git-app-unavailable = GitHub App belum diatur di server ini. Minta admin instance mengikuti panduan "GitHub App" di dokumentasi admin, atau pakai personal access token.
git-app-connected-detail = Akun { $account } · { $n } repo dapat diakses
git-via-app = GitHub App
git-via-token = Token
git-app-manage = Kelola di GitHub
git-app-refresh = Segarkan
git-app-installation = Instalasi
git-app-repos = Repo
git-app-repos-all = Semua repo akun ({ $n })
git-app-repos-selected = { $n } repo dipilih
git-app-permissions = Izin
git-app-suspended-badge = Ditangguhkan
git-app-suspended-note = Instalasi GitHub App ditangguhkan pemilik akun GitHub. Agent Gate tidak bisa membaca repo sampai instalasi diaktifkan lagi.
git-webhook-app = Lewat App

## Pengaturan project → Konvensi git
project-git-title = Konvensi git
project-git-description = Aturan git, PR, dan CI untuk { $project }. Berlaku untuk semua workflow project ini.
project-git-panel-git = Konvensi git
project-git-panel-git-desc = Branch, commit, bahasa
project-git-panel-out = Hasil kode & merge
project-git-panel-out-desc = PR otomatis dan mode merge
project-git-panel-ci = Gate CI
project-git-panel-ci-desc = Check wajib
project-git-panel-env = Environment testing
project-git-panel-env-desc = Per repo
project-git-panel-deploy = Stage deploy
project-git-panel-deploy-desc = Opsional
project-git-panel-path = Jalur kode ke server
project-git-panel-path-desc = Clone atau upload
project-git-git-description = Dipakai AI saat membuat branch, commit, dan PR.
project-git-out-description = Cara hasil kode stage Development sampai ke branch utama.
project-git-ci-description = Dicek gate engine sebelum tiket boleh pindah stage.
project-git-note = Berlaku untuk job berikutnya
project-git-no-repos = Project ini belum punya repo. Tambahkan repo di Pengaturan project → Repo supaya runner bisa membuat PR.
git-branch-pattern = Pola nama branch
git-branch-pattern-hint = Variabel: {"{"}kode{"}"}, {"{"}slug{"}"}, {"{"}tipe{"}"}. {"{"}kode{"}"} wajib, dipakai untuk menautkan PR ke tiket.
git-branch-example = Contoh: { $example }
git-commit-style = Pesan commit
git-commit-style-hint = Format pesan commit dari AI.
git-commit-style-conventional = Conventional Commits
git-commit-style-free = Bebas
git-commit-language = Bahasa commit message
git-pr-language = Bahasa deskripsi PR
git-code-language = Bahasa komentar kode
git-language-en = Bahasa Inggris
git-language-id = Bahasa Indonesia
git-code-output = Hasil kode stage Development
git-code-output-manual = Manual
git-code-output-manual-hint = AI hanya mengubah file. Commit, push, dan PR dikerjakan manusia.
git-code-output-pr = PR otomatis
git-code-output-pr-hint = Runner membuat branch, commit, push, lalu membuka PR. Link PR menempel di tiket.
git-merge-mode = Mode merge
git-merge-mode-manual = Tombol Merge (default)
git-merge-mode-manual-hint = Setelah stage Code Reviewer di-approve dan CI hijau, tombol Merge muncul di tab PR & CI tiket. Merge dikerjakan lewat provider.
git-merge-mode-auto-on-review = Otomatis saat review di-approve
git-merge-mode-auto-on-review-hint = PR di-merge otomatis begitu stage ber-persona Code Reviewer di-approve dan CI hijau.
git-require-ci = Wajib CI hijau
git-require-ci-hint = Gate "CI hijau" ditolak bila ada check yang gagal atau belum selesai.
git-required-checks = Check wajib
git-required-checks-hint = Nama check yang harus lulus, dipisah koma. Kosong = semua check yang berjalan.
git-ai-review = Komentar review AI per baris
git-ai-review-hint = Ditulis per baris di PR dan sebagai artifact review di tiket.

## Project → Pull request
project-view-prs = Pull request
prs-description = Semua PR dari tiket project ini. Status CI diperbarui lewat webhook provider.
prs-no-repos = Project ini belum punya repo. PR muncul di sini setelah runner membuka PR untuk tiket.
prs-empty = Belum ada PR
prs-empty-description = PR dibuat runner saat job Development selesai, atau ditautkan dari webhook bila nama branch memuat kode tiket.
pr-search = Cari PR, tiket, atau branch…
pr-col-pr = PR
pr-col-ticket = Tiket
pr-col-branch = Branch
pr-col-ci = CI
pr-col-state = Status
pr-col-updated = Diperbarui
pr-filter-repo = Repo
pr-tab-all = Semua
pr-tab-open = Terbuka
pr-tab-merged = Merged
pr-stat-open = Terbuka
pr-stat-failing = CI gagal
pr-stat-merged = Merged
pr-stat-repos = Repo

## Tiket → PR & CI
ticket-tab-pr = PR & CI
ticket-pr-title = Pull request
ticket-pr-description = PR dibuat runner di repo tiket. CI berjalan di setiap push.
ticket-pr-empty = Belum ada PR
ticket-pr-empty-auto = PR dibuka otomatis oleh runner saat job Development selesai.
ticket-pr-empty-manual = Hasil kode project ini manual: buat PR sendiri dengan nama branch yang memuat kode tiket, lalu PR tertaut lewat webhook.
pr-open-provider = Buka di provider
pr-ci-label = CI
pr-checks-empty = Belum ada check yang dilaporkan untuk commit ini.
pr-from-job = Dibuka oleh runner.
pr-view-job = Lihat job

## R1: tombol Merge dan cara merge
git-merge-method = Cara merge
git-merge-method-hint = Dipakai tombol Merge maupun merge otomatis.
git-merge-method-squash = Squash
git-merge-method-squash-hint = Semua commit digabung menjadi satu.
git-merge-method-merge = Merge commit
git-merge-method-merge-hint = Riwayat commit dipertahankan dengan commit merge.
git-merge-method-rebase = Rebase
git-merge-method-rebase-hint = Commit ditempatkan ulang di atas branch tujuan.
pr-merge = Merge
pr-flash-merged = PR di-merge lewat provider.
git-merge-not-ready = PR belum bisa di-merge: review harus di-approve dan CI hijau.
