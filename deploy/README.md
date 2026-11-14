# Deploy Compose (satu VPS)

Isi folder ini: `compose.yaml` (project `agentgate-rs-prod`), `Caddyfile` (TLS otomatis, domain
aplikasi + domain pratinjau), `.env.example`, `db-init/` (dijalankan sekali saat volume Postgres
dibuat), dan `backup/` (container backup: `pg_dump` + salinan bucket file ke S3 di luar VPS).

Langkah lengkap, upgrade, dan pemulihan: `docs/admin/deploy.md`, `docs/admin/backup.md`.

```sh
docker build -f docker/prod/Dockerfile --build-arg BUILD_SHA=$(git rev-parse --short HEAD) \
  -t agentgate-rs-server:$(git rev-parse --short HEAD) -t agentgate-rs-server:latest .
cp deploy/.env.example deploy/.env    # isi
docker compose -f deploy/compose.yaml --env-file deploy/.env up -d
```

## Layanan

| Service | Perintah | Catatan |
|---|---|---|
| `migrate` | `db migrate` | sekali jalan; web/worker menunggu selesai |
| `web` | `start` | HTTP + WebSocket runner; skala dengan `WEB_REPLICAS` |
| `worker` | `start --worker --scheduler` | job antrean + jadwal harian; **scheduler hanya boleh satu** |
| `db` | Postgres 18 + pgvector | `statement_timeout` 30 dtk per role |
| `s3`, `s3-init` | SeaweedFS | lampiran, logo, evidence, ekspor; bisa diganti S3 eksternal |
| `backup` | `backup.sh loop` | harian `BACKUP_HOUR_UTC`; retensi harian `BACKUP_RETENTION_DAYS` (14) + bulanan `BACKUP_MONTHLY_MONTHS` (6) |
| `caddy` | Caddy 2 | port 80/443; `/_metrics` diblok dari luar |

## Aritmetika koneksi Postgres

Setiap proses aplikasi membuka pool sampai `DB_MAX_CONNECTIONS`. Proses web menambah **1** koneksi
`LISTEN` (realtime runner) di luar pool.

```
total = WEB_REPLICAS × (DB_MAX_CONNECTIONS + 1)   # web
      + DB_MAX_CONNECTIONS                        # worker + scheduler
      + 1                                         # backup (psql/pg_dump) saat berjalan
      + 1                                         # migrate (hanya saat deploy)
      + 3                                         # cadangan superuser Postgres
```

Default (`WEB_REPLICAS=1`, `DB_MAX_CONNECTIONS=10`): 11 + 10 + 1 + 1 + 3 = **26** dari
`PG_MAX_CONNECTIONS=100`. Batas aman: total ≤ 80% `PG_MAX_CONNECTIONS`. Contoh 3 replika web dengan
pool 20: 3 × 21 + 20 + 5 = 88, jadi naikkan `PG_MAX_CONNECTIONS` ke 120 (dan memori VPS: tiap
koneksi ±5–10 MB).

Pool habis terlihat di metrik `agentgate_db_pool_connections{state="in_use"}` mendekati
`agentgate_db_pool_max`, dan di halaman Status sistem → Database.
