#!/usr/bin/env bash
# Backup Agent Gate (M12). Perintah:
#   loop               backup setiap hari pukul BACKUP_HOUR_UTC (default 19 = 02.00 WIB)
#   now                satu backup sekarang
#   prune              hapus dump di luar retensi (harian + bulanan) tanpa backup baru
#   list               daftar dump di storage backup
#   restore <file>     pulihkan dump (nama dari `list`) ke DATABASE_URL; hentikan web/worker dulu
#   restore-files      salin balik file (lampiran, logo, ...) dari storage backup ke bucket aplikasi
#   init-bucket        buat bucket aplikasi bila belum ada
# Riwayat backup ditulis ke tabel system_backups (dibaca halaman Status sistem).
#
# Env: DATABASE_URL; bucket aplikasi S3_ENDPOINT/S3_REGION/S3_BUCKET/S3_ACCESS_KEY_ID/
# S3_SECRET_ACCESS_KEY; tujuan backup (DI LUAR VPS) BACKUP_REMOTE_ENDPOINT/BACKUP_REMOTE_REGION/
# BACKUP_REMOTE_BUCKET/BACKUP_REMOTE_PREFIX/BACKUP_REMOTE_ACCESS_KEY_ID/
# BACKUP_REMOTE_SECRET_ACCESS_KEY; BACKUP_RETENTION_DAYS (harian, default 14) dan
# BACKUP_MONTHLY_MONTHS (salinan bulanan, default 6), lihat retention.sh.
set -euo pipefail

# Semua remote dari env; tanpa file config rclone.
export RCLONE_CONFIG=/dev/null
export RCLONE_CONFIG_APP_TYPE=s3 RCLONE_CONFIG_APP_PROVIDER=Other
export RCLONE_CONFIG_APP_ENDPOINT="${S3_ENDPOINT:-}" RCLONE_CONFIG_APP_REGION="${S3_REGION:-us-east-1}"
export RCLONE_CONFIG_APP_ACCESS_KEY_ID="${S3_ACCESS_KEY_ID:-}" RCLONE_CONFIG_APP_SECRET_ACCESS_KEY="${S3_SECRET_ACCESS_KEY:-}"
export RCLONE_CONFIG_REMOTE_TYPE=s3 RCLONE_CONFIG_REMOTE_PROVIDER=Other
export RCLONE_CONFIG_REMOTE_ENDPOINT="${BACKUP_REMOTE_ENDPOINT:-}" RCLONE_CONFIG_REMOTE_REGION="${BACKUP_REMOTE_REGION:-us-east-1}"
export RCLONE_CONFIG_REMOTE_ACCESS_KEY_ID="${BACKUP_REMOTE_ACCESS_KEY_ID:-}" RCLONE_CONFIG_REMOTE_SECRET_ACCESS_KEY="${BACKUP_REMOTE_SECRET_ACCESS_KEY:-}"

PREFIX="${BACKUP_REMOTE_BUCKET:-}/${BACKUP_REMOTE_PREFIX:-agentgate}"
RETENTION="${BACKUP_RETENTION_DAYS:-14}"
MONTHLY="${BACKUP_MONTHLY_MONTHS:-6}"
# shellcheck source=retention.sh
. "$(dirname "$0")/retention.sh"

# Variabel psql (:'x') hanya di-interpolasi untuk SQL dari stdin, bukan dari -c.
sql() { psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -qtAX "$@"; }
# Role agentgate punya statement_timeout 30 dtk (deploy/db-init); restore besar butuh lebih.
export PGOPTIONS="-c statement_timeout=0"

backup_now() {
  [ -n "${BACKUP_REMOTE_BUCKET:-}" ] || { echo "backup: BACKUP_REMOTE_BUCKET kosong" >&2; return 1; }
  local id stamp file size started err
  started=$(date +%s)
  id=$(sql -c "INSERT INTO system_backups DEFAULT VALUES RETURNING id")
  stamp=$(date -u +%Y%m%dT%H%M%SZ)
  file="/tmp/agentgate-$stamp.dump"
  err=$(mktemp)
  if pg_dump --format=custom --no-owner --file="$file" "$DATABASE_URL" 2>"$err" \
    && rclone copyto "$file" "remote:$PREFIX/postgres/agentgate-$stamp.dump" 2>>"$err" \
    && rclone sync "app:${S3_BUCKET}" "remote:$PREFIX/files" 2>>"$err"; then
    size=$(stat -c %s "$file")
    sql -v id="$id" -v size="$size" -v loc="s3://$PREFIX/postgres/agentgate-$stamp.dump" <<<"UPDATE system_backups SET status='ok', finished_at=now(), size_bytes=:size, location=:'loc' WHERE id=:'id'"
    prune || true
    echo "backup: ok agentgate-$stamp.dump ($size byte, $(( $(date +%s) - started )) dtk)"
  else
    local msg
    msg=$(tail -c 900 "$err" | tr '\n' ' ')
    sql -v id="$id" -v msg="$msg" <<<"UPDATE system_backups SET status='failed', finished_at=now(), message=:'msg' WHERE id=:'id'" || true
    echo "backup: GAGAL: $msg" >&2
    rm -f "$file" "$err"
    return 1
  fi
  rm -f "$file" "$err"
}

# Hapus dump di luar retensi (harian + bulanan, retention.sh). Gagal hapus satu berkas tidak
# menggagalkan backup; berkas itu dicoba lagi besok.
prune() {
  local name
  rclone lsf --files-only "remote:$PREFIX/postgres" \
    | ag_backup_expired "$(date -u +%s)" "$RETENTION" "$MONTHLY" \
    | while IFS= read -r name; do
        rclone deletefile "remote:$PREFIX/postgres/$name" && echo "backup: hapus $name (retensi)"
      done
}

seconds_until_next_run() {
  local hour="${BACKUP_HOUR_UTC:-19}" now next
  now=$(date -u +%s)
  next=$(date -u -d "today ${hour}:00" +%s)
  [ "$next" -gt "$now" ] || next=$(date -u -d "tomorrow ${hour}:00" +%s)
  echo $(( next - now ))
}

case "${1:-loop}" in
  loop)
    while true; do
      wait_s=$(seconds_until_next_run)
      echo "backup: berikutnya dalam ${wait_s} dtk"
      sleep "$wait_s"
      backup_now || true
    done ;;
  now) backup_now ;;
  prune) prune ;;
  list) rclone lsl "remote:$PREFIX/postgres" | sort -k2,3 ;;
  restore)
    name="${2:?nama file dump dari 'backup.sh list'}"
    rclone copyto "remote:$PREFIX/postgres/$name" "/tmp/$name"
    pg_restore --clean --if-exists --no-owner --single-transaction --dbname="$DATABASE_URL" "/tmp/$name"
    rm -f "/tmp/$name"
    echo "restore: selesai dari $name" ;;
  restore-files) rclone copy "remote:$PREFIX/files" "app:${S3_BUCKET}" ;;
  init-bucket) rclone mkdir "app:${S3_BUCKET:?isi S3_BUCKET}" && echo "bucket ${S3_BUCKET} siap" ;;
  *) echo "pakai: backup.sh loop|now|prune|list|restore <file>|restore-files|init-bucket" >&2; exit 2 ;;
esac
