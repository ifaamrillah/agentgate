# Benchmark code graph M7 vs codebase-memory (2026-11-13)

Pertanyaan yang diuji: **"siapa yang memanggil fungsi X?"** (pemanggil langsung, depth 1, tes
ikut dihitung). Ini pertanyaan yang paling sering dipakai agent lewat `ag_trace` dan untuk
analisis dampak.

## Setup

- Repo: Agent Gate sendiri (commit `e639aef` + perbaikan crate-name di commit benchmark ini),
  521 file yang dikenali, 457 file kode yang di-parse (Rust, TypeScript/JavaScript, dll.).
- Agent Gate: `cargo run --release -p agentgate-graph --example repo_graph -- <root> <nama…>`
  (walk + tree-sitter + build, satu thread, tanpa SCIP). Jalur ini sama dengan yang dipakai
  server (`services::graph::build` memanggil `agentgate_graph::build::build`).
- codebase-memory-mcp: `index_repository mode=full`, lalu `trace_path direction=inbound depth=1
  include_tests=true`.
- Graphify: **tidak dijalankan**, karena tool-nya tidak tersedia di mesin ini (lihat "Perlu dari owner").
- Ground truth: `git grep` semua call site `nama(` / `path::nama(` di file `.rs`, lalu
  dikelompokkan per fungsi pembungkus (skrip ada di scratchpad sesi). Ada 7 hit grep yang
  dibuang setelah dicek manual: 6 berasal dari kode di dalam string literal fixture tes
  (`graph/tests/*.rs`), dan 1 adalah closure lokal `num` di `graph/src/query.rs` yang memang
  fungsi lain.

25 fungsi target, 149 pasangan (pemanggil → fungsi) yang benar:

- **Set A (15 fungsi): set pengembangan.** Fungsi-fungsi M7. Set ini dipakai saat mencari bug
  resolusi Rust, jadi hasil akhirnya sudah "dilatih" pada set ini.
- **Set B (10 fungsi): set holdout.** Kode M1–M6 yang tidak dilihat saat perbaikan:
  - `app_shell`, `provider_for`, `connection_of`, `seed_people`, `issue_link`
  - `random_token`, `project_views`, `provider_code`, `module_of`, `is_secret_path`

## Hasil

| | Pasangan benar | Agent Gate | codebase-memory |
|---|---:|---:|---:|
| Set A (pengembangan) | 40 | 40 ditemukan, 0 salah | 34 ditemukan, 8 salah |
| Set B (holdout) | 109 | 109 ditemukan, 0 salah | 100 ditemukan, 1 salah |
| **Total recall** | 149 | **100%** | 89,9% |
| **Total presisi** | | **100%** | 93,7% |

Sebelum perbaikan resolusi Rust di commit `e639aef`, Agent Gate hanya menemukan **25 dari 40**
pasangan di set A. Yang terlewat:

- `ensure_row` 0/7: diimpor lewat `use super::…`.
- `enqueue` 0/2: dipanggil lewat `svc::` / `crate::…::`.
- `rebuild` 0/2: dipanggil lewat `build::rebuild`.
- `code_lines` dan `link_view`: dipanggil di dalam `json!`.

Selisih dengan codebase-memory per fungsi:

| Fungsi | Benar | Agent Gate | codebase-memory | Penyebab selisih codebase-memory |
|---|---:|---:|---:|---|
| `code_lines` | 2 | 2 | 1 | Call di dalam `json!({...})` tidak terlihat |
| `link_view` | 2 | 2 | 1 | Sama: di dalam makro |
| `num` | 4 | 4 | 2 (+7 salah) | Call di makro/argumen terlewat; caller fungsi JS bernama `num` di prototype ikut digabung |
| `pct` | 4 | 4 | 2 | Call di dalam `json!` |
| `unpack` | 2 | 2 | 2 (+1 salah) | Method `tar::Archive::unpack` (crate lain) dianggap memanggil fungsi kita |
| `project_views` | 11 | 11 | 3 | Call di dalam `json!` (pola umum di controller Loco) |
| `module_of` | 3 | 3 | 2 (+1 salah) | Call di `Index::new` dicatat sebagai `__file__` |
| `describe` | 4 | 4 | 4 | Perlu `qualified_name` karena nama ambigu (3 definisi) |
| 17 fungsi lain | 117 | 117 | 117 | Sama |

Kecepatan (index penuh repo ini, mesin 16 core):

| | Waktu |
|---|---|
| Agent Gate: walk + parse + build, 1 thread | ±2,1–3,4 s (parse ±2 s, build ±0,4–0,7 s) |
| codebase-memory: `mode=full` (termasuk semantic) | ±4 s |

Ukuran graph Agent Gate untuk repo ini:

- 5.054 simbol dan 15.972 relasi.
- 56.354 call di kode:
  - 11.018 ter-resolve **pasti** dan 7.718 **dugaan**.
  - 33.338 eksternal, ke std atau crate pihak ketiga.
  - 4.280 ambigu, tidak ditautkan.

## Batasan yang harus dibaca bersama angka di atas

- **Korpus kecil dan satu bahasa dominan (Rust).** 149 pasangan sudah cukup untuk melihat
  perbedaan pola (makro, `use super::`, path call), tapi belum cukup untuk klaim umum.
  - Benchmark per bahasa ada di panel *Status index → Benchmark*. Isinya korpus bawaan 10
    bahasa: recall dan presisi 100%, tetapi korpus itu ditulis tim dan kecil.
- **Set A bias.** Bug ditemukan dan diperbaiki dengan melihat set A, jadi set A bukan ukuran
  yang adil. Set B (holdout) adalah angka yang lebih jujur: Agent Gate 109/109, codebase-memory
  100/109.
- **Ground truth berbasis grep.** Call lewat trait object, closure yang disimpan ke variabel,
  atau fungsi yang dioper sebagai nilai (`.map(num)`) tidak tercatat di grep maupun di kedua
  tool. Angka di atas hanya mengukur call langsung.
- **Call di dalam makro dideteksi heuristik** (`ident(` di token tree). Kodenya bisa salah
  untuk makro yang mendefinisikan DSL sendiri. Keyakinan relasinya tetap "pasti" bila
  resolusinya lewat import.
- **Tanpa SCIP.** Untuk Rust, `rust-analyzer scip` + `agentgate graph scip` akan menutup sisa
  kasus (trait, generic), tapi itu dijalankan dari laptop user, bukan bagian benchmark ini.
- **Graphify belum dibandingkan.**

## Cara mengulang

```bash
bash -c '. bin/_lib.sh && ag_load_env && ag_exec bash -c "cd /workspace && \
  cargo run -q --release -p agentgate-graph --example repo_graph -- /workspace \
  ensure_row enqueue rebuild diff_manifest code_lines link_view lang_color kind_icon batches \
  unpack symbol_rows num pct describe words app_shell provider_for connection_of seed_people \
  issue_link random_token project_views provider_code module_of is_secret_path"'
```
