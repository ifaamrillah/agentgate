# Embedding lokal code graph (R2, 2026-11-14)

Pencarian semantik simbol: setiap simbol (non-tes) di-embed dari teks
`jenis nama-lengkap / signature / docstring / path / 12 baris kode (maks. 500 karakter)`,
maks. 700 karakter. Vektor di pgvector (`graph_embeddings`), dikunci hash teks sehingga index
ulang hanya meng-embed simbol yang teksnya berubah.

## Cara menjalankan

```sh
# ONNX Runtime resmi (sama dengan image prod): tmp/ort/onnxruntime-linux-x64-1.24.1/
bin/sh bash -c 'ORT_DYLIB_PATH=/workspace/tmp/ort/onnxruntime-linux-x64-1.24.1/lib/libonnxruntime.so.1 \
  cargo run --release -p agentgate-server --features semantic --example embed_bench -- \
  multilingual-e5-small 3000'
```

Simbol diambil dari repo Agent Gate sendiri (walk + tree-sitter, rata-rata 425 karakter per teks).
Mesin: AMD Ryzen 7 5800H (16 thread), container dev Docker, build release, CPU saja, batch 64,
teks diurutkan menurut panjang sebelum di-batch.

## Hasil

| Model (`graph.embedding.model`) | Ukuran unduhan | Muat (cache) | Muat (unduh pertama) | Index simbol | Query (embed) p50 / p95 |
|---|---|---|---|---|---|
| **`multilingual-e5-small`** (default) | 465 MB | 2,1 dtk | 45 dtk | **14,6–15,3 simbol/dtk** (65–68 ms/simbol) | **5,3 / 13,9 ms** |
| `multilingual-minilm-q` (int8) | 241 MB | — | 26 dtk | 11,1 simbol/dtk | 24 / 53 ms |
| `bge-small-en-q` (int8, Inggris) | 65 MB | — | 10 dtk | 10,0 simbol/dtk | 39 / 71 ms |

Pencarian vektor pgvector (scan berurutan, tanpa index ANN), 20.000 vektor 384 dimensi,
`ORDER BY embedding <=> $q LIMIT 50`: **15 ms** (5 kali, 14,8–18,7 ms).

Jadi satu pencarian hibrid ≈ embed query 5–14 ms + vektor 15 ms + leksikal (sudah ada) untuk
project 20 ribu simbol.

Percobaan pertama memakai teks 30 baris kode / 1.600 karakter: 6,2 simbol/dtk (161 ms/simbol);
attention transformer kuadratik terhadap panjang teks, jadi teks dipangkas ke 12 baris.

## Pilihan model

`multilingual-e5-small` (intfloat, MIT, 384 dimensi, 100+ bahasa termasuk Indonesia):

- Paling cepat di CPU ini; model int8 justru lebih lambat (ONNX Runtime tanpa kernel int8 yang
  cocok untuk CPU ini).
- Multibahasa: query user/agent sering bahasa Indonesia, nama kode bahasa Inggris.
- Dilatih untuk retrieval dengan awalan `query:`/`passage:` (dipakai otomatis).
- Kekurangan: unduhan 465 MB sekali ke volume cache (`/var/cache/agentgate/models`).

## Contoh kualitas (semantik saja, 3.000 simbol)

| Query | 3 teratas |
|---|---|
| verifikasi tanda tangan webhook | `verify`, `webhook_secret`, … |
| token instalasi github app | `GithubAppSettings`, `is_app`, `for_installation` |
| parse git log jadi hotspot | `parse`, `Hotspot`, `for_host` |
| buat pull request knowledge | `PullRequestInfo`, `pull`, `PullRequestOpen` |
| kirim email undangan anggota | `render`, `NewInvitation`, `issue` |
| potong stok saat checkout | (tidak relevan: repo ini tidak punya kode stok) |
| rate limit login per ip | `miniAuth`, `miniSec`, … (meleset) |

Semantik saja masih berisik untuk konsep yang jarang muncul di nama/doc. Karena itu default UI
dan `ag_search` adalah **hibrid**: hasil leksikal dan semantik digabung dengan Reciprocal Rank
Fusion (k = 60), dan simbol yang namanya persis sama dengan query selalu di urutan pertama.

## Waktu index dalam praktik

- Index pertama repo 5.000 simbol ≈ 5,5 menit, 20.000 simbol (batas default
  `graph.embedding.max_symbols`) ≈ 22 menit, di job latar belakang `graph_embed` setelah index
  tree-sitter; pencarian leksikal sudah jalan selama itu.
- Index berikutnya hanya simbol yang berubah (hash teks): PR biasa menyentuh puluhan simbol →
  beberapa detik.

## Dampak build dan image

Feature Cargo `semantic` (fastembed 7.7.0 + ort 2.0.0-rc.15 dengan `load-dynamic`):

| | Tanpa `semantic` | Dengan `semantic` |
|---|---|---|
| Crate dependency (normal) | 665 | 720 (+55) |
| Biner release `agentgate-server` | 109,9 MB | 114,5 MB (+4,6 MB) |
| Build release ulang setelah dependency ter-cache | — | +131 dtk (link + crate server) |
| Build release pertama dependency baru (tokenizers, ort, fastembed, hf-hub) | — | ≈ 3 menit tambahan |
| Image runtime | — | + `libonnxruntime.so` 1.24.1 (≈ 22 MB) |

ONNX Runtime dimuat dinamis (`ORT_DYLIB_PATH`) dari rilis resmi Microsoft (checksum dipin di
`docker/prod/Dockerfile`), karena biner statis bawaan crate `ort` butuh libstdc++ GCC 13+ dan
gagal link di Debian bookworm. Tes dan `bin/dev` dibangun tanpa feature (provider `fake`), jadi
loop pengembangan tidak bertambah lambat. Paket `git` (clone dangkal) menambah ≈ 30–40 MB ke image (perkiraan dari ukuran paket apt, belum diukur dengan build image).
