#!/usr/bin/env bash
# Tes retensi backup (deploy/backup/retention.sh): harian 14 hari + dump pertama tiap bulan
# disimpan 6 bulan; nama asing tidak pernah dihapus.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }
# shellcheck disable=SC1091
. "$ROOT/deploy/backup/retention.sh"

NOW=$(date -u -d "2026-11-14T19:00:00Z" +%s)
# Dump harian 2026-04-25 .. 2026-11-14 (204 hari) pukul 19.00 UTC, plus berkas asing.
LIST=$(
  d=$(date -u -d "2026-04-25" +%s)
  while [ "$d" -le "$NOW" ]; do
    printf 'agentgate-%sT190000Z.dump\n' "$(date -u -d "@$d" +%Y%m%d)"
    d=$(( d + 86400 ))
  done
  echo "catatan.txt"
  echo "agentgate-rusak.dump"
)
EXPIRED=$(printf '%s\n' "$LIST" | ag_backup_expired "$NOW")
KEPT=$(printf '%s\n' "$LIST" | grep -vxF -f <(printf '%s\n' "$EXPIRED"))

has() { printf '%s\n' "$KEPT" | grep -qx "$1"; }

# 14 hari terakhir utuh (31 Okt .. 14 Nov; 31 Okt tepat 14 hari = masih disimpan).
for day in 20261031 20261101 20261107 20261114; do
  has "agentgate-${day}T190000Z.dump" || fail "harian $day terhapus"
done
has "agentgate-20261030T190000Z.dump" && fail "harian 30 Okt (15 hari) masih disimpan"
# Salinan bulanan: dump pertama Jun..Nov (≤ 6 bulan); Mei (> 6×31 hari) dan Apr dihapus.
for day in 20260601 20260701 20260801 20260901 20261001 20261101; do
  has "agentgate-${day}T190000Z.dump" || fail "bulanan $day terhapus"
done
for day in 20260425 20260501 20260515 20260602 20261015; do
  has "agentgate-${day}T190000Z.dump" && fail "$day seharusnya dihapus"
done
# Nama asing tidak pernah dihapus.
has "catatan.txt" || fail "berkas asing terhapus"
has "agentgate-rusak.dump" || fail "nama tak dikenal terhapus"
# Jumlah tersisa: 15 harian + 5 bulanan (Jun..Okt; Nov sudah termasuk harian) + 2 asing.
COUNT=$(printf '%s\n' "$KEPT" | grep -c .)
[ "$COUNT" -eq 22 ] || fail "tersisa $COUNT, harap 22: $KEPT"

# Parameter bisa diubah: harian 3 hari, bulanan 1 bulan.
EXPIRED=$(printf '%s\n' "$LIST" | ag_backup_expired "$NOW" 3 1)
KEPT=$(printf '%s\n' "$LIST" | grep -vxF -f <(printf '%s\n' "$EXPIRED"))
has "agentgate-20261111T190000Z.dump" || fail "harian 3 hari"
has "agentgate-20261110T190000Z.dump" && fail "harian 4 hari masih ada"
has "agentgate-20261101T190000Z.dump" || fail "bulanan November"
has "agentgate-20261001T190000Z.dump" && fail "bulanan Oktober (> 1 bulan) masih ada"

echo "backup_retention_test: ok"
