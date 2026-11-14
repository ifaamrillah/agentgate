# Operasi: log, metrik, alert

## Log

JSON ke stdout (`docker compose ... logs -f web worker`). Setiap request punya `request_id`; query
string disamarkan, token dan secret tidak pernah di-log. Kirim ke Loki/Vector/CloudWatch dengan
driver log Docker bila perlu.

## Halaman Status sistem (`/system`)

Untuk email di `SYSTEM_ADMIN_EMAILS`. Panel: ringkasan (peringatan), layanan (web, worker,
realtime, runner online), database (versi, ukuran, koneksi, tabel terbesar, storage per kategori),
backup (riwayat dari container backup), error (job gagal 24 jam, 5xx).

## Metrik Prometheus

`GET http://web:5150/_metrics` dengan header `Authorization: Bearer $METRICS_TOKEN`. Diblok dari
internet oleh Caddy; scrape dari jaringan `agentgate-rs-prod`. Metrik per proses (tiap replika web
di-scrape sendiri).

```yaml
scrape_configs:
  - job_name: agentgate
    authorization: { credentials: "<METRICS_TOKEN>" }
    static_configs: [{ targets: ["web:5150"] }]
```

## Alert yang disarankan

```yaml
groups:
  - name: agentgate
    rules:
      - alert: AgentGate5xxTinggi
        expr: sum(rate(agentgate_http_requests_total{status=~"5.."}[5m]))
              / sum(rate(agentgate_http_requests_total[5m])) > 0.02
        for: 10m
      - alert: AgentGateAntreanMacet
        expr: agentgate_queue_oldest_queued_seconds > 300
        for: 5m
      - alert: AgentGatePoolHabis
        expr: agentgate_db_pool_connections{state="in_use"} / agentgate_db_pool_max > 0.9
        for: 5m
      - alert: AgentGateLoginGagalMelonjak
        expr: rate(agentgate_login_failures_total[5m]) * 60 > 30
        for: 5m
      - alert: AgentGateRealtimePutus
        expr: agentgate_realtime_connected == 0
        for: 2m
```

Backup gagal/basi (> 26 jam) tampil sebagai peringatan di Status sistem; untuk alert, query
`SELECT max(finished_at) FROM system_backups WHERE status = 'ok'` dari exporter SQL.

## Error tracking (Sentry)

Mati bila `SENTRY_DSN` kosong: tidak ada request keluar, tidak ada skrip/meta Sentry di halaman, dan
CSP tidak berubah. Untuk menyalakan, buat project Sentry (platform Rust; DSN yang sama dipakai
browser), isi `SENTRY_DSN` (dan `SENTRY_ENVIRONMENT` bila perlu) di `.env` untuk container `web`
dan `worker`, lalu restart.

- **Server** (crate `sentry`): panic dan setiap respons 5xx. Event 5xx hanya membawa metode, pola
  route (`/w/{slug}/t/{code}`), dan status. Release = `agentgate@<versi>`.
- **Browser** (`/static/js/sentry.js`, tanpa SDK pihak ketiga): `window.onerror` dan promise yang
  tidak ditangani, maksimal 10 event per halaman. CSP `connect-src` ditambah origin ingest Sentry
  (mis. `https://o123.ingest.sentry.io`) hanya saat DSN diisi; `script-src 'self'` tidak berubah.
- **Data yang tidak dikirim**: cookie, header `Authorization`/`Cookie`/`X-Runner-*` (hanya
  `User-Agent`, `Content-Type`, `Accept`, `X-Request-Id` yang tersisa), body request/form, query
  string dan hash URL (bisa memuat token), data user, data breadcrumb. `send_default_pii` mati dan
  semua event melewati `services::error_tracking::scrub` sebelum dikirim.
- Sentry self-host: DSN `http(s)://<key>@<host>[:port]/<project>` didukung; origin CSP mengikuti
  host dan port DSN.

## Tugas rutin

- Scheduler di container `worker`: pembersihan harian 03.00 UTC, audit knowledge, sapuan webhook
  (tiap menit), sapuan ekspor (10 menit), sinkron Jira (5 menit).
- Manual: `docker compose ... exec worker agentgate-server task cleanup` (dry run).
