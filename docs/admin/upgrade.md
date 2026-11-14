# Upgrade

```sh
git pull                                   # atau checkout tag rilis
SHA=$(git rev-parse --short HEAD)
docker build -f docker/prod/Dockerfile --build-arg BUILD_SHA=$SHA \
  -t agentgate-rs-server:$SHA -t agentgate-rs-server:latest .
C="docker compose -f deploy/compose.yaml --env-file deploy/.env"
$C exec backup backup.sh now               # backup dulu
$C up -d                                   # migrate jalan dulu, lalu web/worker diganti
$C logs --tail=50 migrate web
```

- Migrasi bersifat maju; rollback = pulihkan backup sebelum upgrade lalu jalankan image lama.
- Migrasi yang mengubah tabel besar ditulis kompatibel dua arah (kolom baru nullable dulu), sehingga
  container lama tetap jalan selama migrasi.
- Runner CLI lama tetap bisa tersambung selama versinya ≥ `CLI_MIN_VERSION`; minta user
  `agentgate self-update` setelah rilis CLI baru.
