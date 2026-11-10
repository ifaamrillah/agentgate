# Agent Gate

Manajemen proyek ala Jira yang sekaligus menjadi MCP server dan orkestrator AI agent untuk seluruh
siklus pengembangan produk. Spesifikasi produk: [`docs/spec.md`](docs/spec.md). Konvensi kode:
[`docs/reference/loco-project-rules.md`](docs/reference/loco-project-rules.md). Status pengerjaan:
[`docs/PROGRESS.md`](docs/PROGRESS.md).

## Prasyarat

Cukup **Docker** (dengan Compose v2), `git`, `bash`, dan `curl` di host. Rust, Tailwind, `sea-orm-cli`,
dan tool lain berjalan di container `app`; skrip `bin/*` meneruskan perintah ke sana.

## Perintah sehari-hari

| Perintah | Fungsi |
|---|---|
| `bin/dev` | Server development di <http://localhost:5150> (Tailwind `--watch` + restart otomatis) |
| `bin/test [filter]` | Semua tes workspace (database test, satu thread) |
| `bin/loco db migrate` | Migrasi database dev |
| `bin/loco <perintah>` | Perintah Loco lain (`routes`, `doctor`, `g migration ...`) |
| `bin/ci` | Pemeriksaan lengkap sebelum merge (fmt, clippy, deny, arah layer, CSS, tes) |
| `bin/sh [perintah]` | Shell atau satu perintah di container `app` |
| `bin/worktree new <branch>` | Mulai kerja paralel di worktree sendiri (lihat di bawah) |

Pertama kali, `bin/*` membangun image `agentgate-rs-dev` dan menyalakan layanan bersama
(project Compose `agentgate-rs-shared`):

| Layanan | Alamat dari host | Keterangan |
|---|---|---|
| Postgres 18 + pgvector | `127.0.0.1:54329` (user/password `agentgate`) | database `agentgate_dev`, `agentgate_test` |
| SeaweedFS (S3) | <http://127.0.0.1:8333> (key `agentgate` / `agentgate-dev-secret`) | storage file |
| Mailpit | <http://localhost:8025> (SMTP 1025) | menampung semua email dev |

Katalog komponen UI ada di <http://localhost:5150/_dev/ui> (hanya development).

## Kerja paralel dengan worktree

```bash
bin/worktree new feature/nama-fitur      # folder, database, container, dan port sendiri
bin/worktree list
bin/worktree close feature/nama-fitur    # setelah di-merge ke staging
```

Checkout utama selalu di `staging`. Detail aturan: §11 dokumen konvensi.

## Editor

`.devcontainer/devcontainer.json` menempelkan VS Code/Cursor ke container `app` yang sama dengan
`bin/*`, sehingga rust-analyzer berjalan di dalam container.

## Struktur

| Folder | Isi |
|---|---|
| `server/` | Aplikasi Loco (`agentgate-server`): web app, API, gate engine, MCP, worker |
| `cli/` | Binary `agentgate`: installer aturan, indexer, runner |
| `core/` | Type bersama, parser workflow, protokol runner |
| `graph/` | Parser tree-sitter dan pembangun code graph |
| `docker/` | Image dev dan file Compose |
| `bin/` | Skrip resmi development |
| `docs/` | Spesifikasi, rencana per milestone, hasil spike |
