# Agent Gate — Desain Cara Pengerjaan MVP

Tanggal: 2026-11-11 · Disetujui: Muhammad Ifa Amrillah

## Maksud

Membangun Agent Gate sesuai `docs/spec.md` (keputusan produk sudah final) dari M0 sampai M12.
Konvensi kode: `docs/reference/loco-project-rules.md`, dengan penyesuaian yang tercantum di spec.

## Pendekatan

Dipertimbangkan:

1. **Berurutan per milestone (dipilih).** M0 → M12 mengikuti graf dependensi spec; tiap milestone punya plan sendiri,
   dikerjakan dengan TDD, di-merge ke `staging` setelah `bin/ci` hijau. Risiko arsitektur (spike M0) terjawab paling awal.
2. Vertikal tipis lintas semua milestone sekaligus — ditolak: spec sudah menetapkan urutan dependensi, dan fitur yang
   setengah jadi di semua area sulit direview.
3. Paralel banyak milestone sejak awal — ditolak: M1–M6 saling bergantung, dan RAM host (~3 GB bebas) membatasi build paralel.

## Cara kerja

- Satu plan per milestone di `docs/superpowers/plans/`, branch `feature/mN-<nama>`, merge ke `staging` lokal (belum ada remote).
- `docs/PROGRESS.md` selalu diperbarui: status milestone, keputusan teknis, hal yang butuh user.
- Keputusan yang tidak dijawab spec (desain UI, pilihan library, dsb.) ditanyakan ke user saat muncul.
- Integrasi yang butuh akun/infrastruktur nyata (OAuth Google/GitHub, Jira, provider git, VPS, domain) dibangun dan
  dites terhadap tiruan lokal; daftar kebutuhan dikumpulkan untuk user di akhir.
- Setiap milestone selesai: kabar singkat ke user, lalu lanjut tanpa menunggu approval. Review penuh di akhir.
- Build cargo dibatasi jumlah job-nya karena RAM host terbatas.

## Non-goal

Semua yang tercantum di "Di luar cakupan" spec.
