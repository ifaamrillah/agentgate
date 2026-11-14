# Deploy (Docker Compose, satu VPS)

## Kebutuhan

- VPS Linux 2 vCPU / 4 GB RAM / 40 GB disk (minimum untuk tim kecil), Docker 24+ dengan Compose v2.
- Dua nama domain yang mengarah (A/AAAA) ke VPS: aplikasi (`agentgate.example.com`) dan pratinjau
  mockup di **domain lain** (`preview.agentgate-usercontent.example.com`), supaya HTML mockup
  tidak pernah berjalan di origin aplikasi.
- Port 80 dan 443 terbuka (Caddy mengurus sertifikat Let's Encrypt).
- SMTP untuk email; bucket S3 **di luar VPS** untuk backup (Backblaze B2, Wasabi, AWS S3, dsb).

## Langkah

```sh
git clone <repo> agentgate && cd agentgate
SHA=$(git rev-parse --short HEAD)
docker build -f docker/prod/Dockerfile --build-arg BUILD_SHA=$SHA \
  -t agentgate-rs-server:$SHA -t agentgate-rs-server:latest .
cp deploy/.env.example deploy/.env && chmod 600 deploy/.env
$EDITOR deploy/.env            # lihat konfigurasi.md
docker compose -f deploy/compose.yaml --env-file deploy/.env up -d
docker compose -f deploy/compose.yaml --env-file deploy/.env ps
```

Urutan otomatis: `db` sehat → `migrate` (sekali) dan `s3-init` (buat bucket) → `web`, `worker`,
`backup` → `caddy`.

## Cek setelah naik

1. `https://APP_DOMAIN/_health` → `{"ok":true}`; `/_readiness` memeriksa database dan antrean.
2. Daftar akun pertama di `/register`, buat workspace. Tambahkan email Anda ke
   `SYSTEM_ADMIN_EMAILS` lalu `up -d` lagi untuk membuka **Status sistem** (`/system`).
3. Jalankan satu backup: `docker compose ... exec backup backup.sh now`, lalu cek tab Backup di
   Status sistem.
4. Instal CLI dari `https://APP_DOMAIN/install` (butuh `CLI_RELEASE_URL`).

## Image dari registry

Bila image dibangun CI dan disimpan di registry, isi `AGENTGATE_IMAGE=registry/agentgate-server:<sha>`
lalu `docker compose ... pull && docker compose ... up -d`. Image yang sama dipakai staging lalu
production (promosi branch `release` dengan fast-forward).
