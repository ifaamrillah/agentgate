# Backup dan pemulihan

Container `backup` menjalankan `deploy/backup/backup.sh` setiap hari pukul `BACKUP_HOUR_UTC`:

1. `pg_dump --format=custom` seluruh database → `s3://BACKUP_REMOTE_BUCKET/PREFIX/postgres/agentgate-<waktu>.dump`;
2. `rclone sync` bucket file aplikasi → `…/PREFIX/files` (salinan cermin);
3. retensi (`deploy/backup/retention.sh`): semua dump `BACKUP_RETENTION_DAYS` hari terakhir (default
   14) disimpan, ditambah **satu salinan bulanan** (dump pertama setiap bulan kalender UTC) selama
   `BACKUP_MONTHLY_MONTHS` bulan (default 6, dihitung 31 hari per bulan). Dump lain dihapus; berkas
   dengan nama di luar pola `agentgate-YYYYmmddTHHMMSSZ.dump` tidak pernah disentuh. Logika ini
   diuji `bin/tests/backup_retention_test.sh` (bagian `bin/ci`);
4. hasil (ok/gagal, ukuran, lokasi, pesan) dicatat di tabel `system_backups` → Status sistem → Backup.

Tujuan backup **harus di luar VPS** (provider berbeda lebih baik). Gunakan kunci akses khusus
backup yang hanya boleh menulis ke bucket itu.

## Perintah

```sh
C="docker compose -f deploy/compose.yaml --env-file deploy/.env"
$C exec backup backup.sh now          # backup sekarang
$C exec backup backup.sh list         # daftar dump
$C exec backup backup.sh prune        # jalankan retensi saja (tanpa backup baru)
```

## Pulihkan

```sh
$C stop web worker
$C exec backup backup.sh restore agentgate-20261114T190000Z.dump
$C exec backup backup.sh restore-files   # bila bucket file ikut hilang
$C up -d
```

`pg_restore --clean --single-transaction`: gagal = tidak ada yang berubah. Setelah pulih, sesi
login tetap berlaku sesuai isi dump; ganti `SESSION_SECRET` bila ingin memaksa semua logout.

## Uji pemulihan

Minimal tiap kuartal: pulihkan dump terbaru ke VPS/stack terpisah (`name:` compose lain), login,
buka satu tiket berlampiran. Backup yang belum pernah diuji pulih belum dianggap backup.
