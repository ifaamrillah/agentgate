# Uji beban `LISTEN/NOTIFY` (M12, 2026-11-14)

Spec: "Kapasitas `LISTEN/NOTIFY` Postgres untuk realtime board, AI Office, dan koneksi runner saat jumlah
user besar." Yang diuji: jalur runner WebSocket (`services::realtime::Hub`), yang menggantikan polling DB
2 detik per koneksi sejak M5.

## Cara kerja yang diuji

- Trigger `AFTER INSERT OR UPDATE` pada `jobs`/`job_permissions` → `pg_notify('agentgate_jobs', workspace_id)`.
- Satu koneksi `LISTEN` per proses web (`Hub`), diteruskan ke kanal `broadcast` **per workspace**.
- Setiap socket runner menilai ulang event hanya saat workspace-nya berubah, plus tick cadangan 30 dtk.

## Cara menjalankan

```sh
docker exec -e DATABASE_URL=postgres://agentgate:agentgate@db:5432/agentgate_dev -w /workspace \
  agentgate-rs-main-app-1 cargo run -q -p agentgate-server --example listen_notify_load -- \
  PROCS LISTENERS WORKSPACES NOTIFIES WRITERS
```

Setiap notifikasi dikirim dalam transaksinya sendiri (seperti trigger job). Latensi = dari sebelum `BEGIN`
penulis sampai pendengar menerima (termasuk commit). Build debug, Postgres 18 di Docker lokal, laptop
16 thread / 15 GB.

## Hasil

| Proses | Pendengar | Workspace | Notifikasi | Penulis | Notif/dtk | p50 | p95 | p99 | max | Tertinggal |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 1.000 | 200 | 2.000 | 8 | 8.194 | 0,7 ms | 1,1 ms | 1,7 ms | 61 ms | 0 |
| 1 | 5.000 | 1.000 | 4.992 | 16 | 12.731 | 1,0 ms | 1,5 ms | 2,0 ms | 114 ms | 0 |
| 8 | 10.000 | 2.000 | 20.000 | 32 | 13.096 | 2,2 ms | 3,6 ms | 4,8 ms | 211 ms | 0 |
| 4 | 2.000 | 5 (padat) | 4.992 | 16 | 2.129 | 1,7 ms | 6,3 ms | 8,5 ms | 14 ms | 0 |

Nilai max adalah notifikasi pertama (listener baru menyelesaikan `LISTEN`/pemanasan).

**Versi pertama (satu kanal bersama untuk semua workspace)** gagal pada skenario 8 proses × 10.000
pendengar: p50 67 ms, p99 210 ms, max 4,6 dtk, dan 536.715 pesan tertinggal (`Lagged`), karena setiap
pendengar menyaring semua notifikasi. Perbaikannya kanal per workspace (hasil di tabel).

## Kesimpulan

- Satu koneksi `LISTEN` per proses cukup jauh di atas kebutuhan MVP (ribuan runner, belasan ribu
  perubahan job per detik). Batasnya ada di tulisan ke tabel `jobs`, bukan di `NOTIFY`.
- Biaya koneksi DB: +1 koneksi per proses web (lihat aritmetika pool di `deploy/README.md`).
- Payload hanya id workspace; notifikasi yang hilang saat listener tersambung ulang ditutup dengan
  siaran "baca ulang semua" + tick cadangan 30 dtk.
- Belum diuji: browser (board/AI Office/log job masih polling HTMX 2–3 dtk; SSE di atas hub yang sama
  adalah langkah berikutnya), dan Postgres terkelola lewat jaringan (latensi akan bertambah RTT).
