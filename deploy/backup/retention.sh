#!/usr/bin/env bash
# Retensi dump Postgres (R1, keputusan pemilik): semua dump harian disimpan BACKUP_RETENTION_DAYS
# hari (default 14), ditambah satu salinan bulanan (dump PERTAMA setiap bulan kalender, UTC) yang
# disimpan BACKUP_MONTHLY_MONTHS bulan (default 6, dihitung 31 hari per bulan dari waktu dump).
#
# Fungsi murni supaya bisa diuji tanpa S3 (bin/tests/backup_retention_test.sh):
#   ag_backup_expired <now_epoch> [harian_hari] [bulanan_bulan] < daftar-nama
# membaca nama berkas (satu per baris, `agentgate-YYYYmmddTHHMMSSZ.dump`) dan mencetak nama yang
# harus dihapus. Nama lain diabaikan (tidak pernah dihapus).

ag_backup_epoch() {
  # agentgate-20261114T190000Z.dump -> detik unix; gagal = kosong.
  local s="${1#agentgate-}"
  s="${s%.dump}"
  [[ "$s" =~ ^[0-9]{8}T[0-9]{6}Z$ ]] || return 1
  date -u -d "${s:0:4}-${s:4:2}-${s:6:2}T${s:9:2}:${s:11:2}:${s:13:2}Z" +%s 2>/dev/null
}

ag_backup_expired() {
  local now="$1" daily_days="${2:-14}" monthly_months="${3:-6}"
  local daily_cut=$(( now - daily_days * 86400 ))
  local monthly_cut=$(( now - monthly_months * 31 * 86400 ))
  local -a names=() epochs=()
  local name epoch
  while IFS= read -r name; do
    name="${name##*/}"
    [ -n "$name" ] || continue
    epoch=$(ag_backup_epoch "$name") || continue
    names+=("$name")
    epochs+=("$epoch")
  done
  # Dump pertama setiap bulan: urut naik, simpan yang pertama muncul per YYYYmm.
  local -A first_of_month=()
  local sorted i month
  sorted=$(for i in "${!names[@]}"; do printf '%s %s\n' "${epochs[$i]}" "${names[$i]}"; done | sort -n)
  while read -r epoch name; do
    [ -n "$name" ] || continue
    month="${name:10:6}"
    [ -n "${first_of_month[$month]:-}" ] || first_of_month[$month]="$name"
  done <<<"$sorted"
  while read -r epoch name; do
    [ -n "$name" ] || continue
    if [ "$epoch" -ge "$daily_cut" ]; then
      continue
    fi
    month="${name:10:6}"
    if [ "${first_of_month[$month]}" = "$name" ] && [ "$epoch" -ge "$monthly_cut" ]; then
      continue
    fi
    printf '%s\n' "$name"
  done <<<"$sorted"
}
