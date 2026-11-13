## Planning (M9): backlog, sprint, estimasi, laporan, roadmap, kalender, rilis, components.

## Kode error
planning-name-invalid = Nama wajib diisi dan tidak boleh terlalu panjang.
planning-name-taken = Nama ini sudah dipakai di project ini.
planning-text-too-long = Teks terlalu panjang.
planning-dates-invalid = Tanggal tidak valid, atau tanggal selesai lebih awal dari tanggal mulai.
planning-capacity-invalid = Kapasitas harus angka 0–999.
planning-sprint-state = Sprint tidak bisa diubah pada keadaannya sekarang.
planning-target-invalid = Pilihan tidak valid untuk project ini.
planning-estimate-invalid = Estimasi tidak valid. Periksa poin dan jam per stage.
planning-notes-too-long = Release notes terlalu panjang.
planning-unchanged = Tidak ada perubahan dari versi sekarang.
planning-release-state = Rilis ini sudah dirilis atau diarsipkan.
planning-owner-invalid = Pemilik harus anggota yang bisa membuka project ini.
planning-query-invalid = Query tidak bisa dibaca. Periksa penulisannya.
planning-team-invalid = Pilih tim tempat Anda menjadi anggota.
planning-bulk-empty = Pilih minimal satu tiket dan satu perubahan.

## Backlog
backlog-title = Backlog
backlog-description = Rencanakan sprint. Seret tiket antar sprint atau pakai menu di ujung baris. Angka AI adalah usulan story point, angka di kotak adalah yang disepakati.
backlog-search = Cari tiket di sprint dan backlog…
backlog-count = { $total } tiket
backlog-count-filtered = { $shown } dari { $total } tiket
backlog-drag-hint = Seret untuk memindah
backlog-summary = { $count } tiket · { $points } poin
backlog-move-to = Pindahkan { $code } ke
backlog-move-top = Ke paling atas
backlog-move-bottom = Ke paling bawah
backlog-open-ticket = Buka tiket
backlog-empty = Backlog kosong
backlog-empty-description = Semua tiket sudah masuk sprint.
backlog-no-match = Tidak ada tiket yang cocok
backlog-no-match-description = Ubah kata kunci atau hapus pencarian.
backlog-stat-active = Sprint aktif selesai
backlog-stat-active-hint = { $value }, dalam poin
backlog-stat-no-active = Belum ada sprint aktif
backlog-stat-velocity = Velocity rata-rata
backlog-stat-velocity-hint = { $value } sprint terakhir
backlog-stat-velocity-none = Muncul setelah sprint pertama selesai
backlog-stat-accuracy = Akurasi estimasi AI
backlog-stat-accuracy-hint = Dari { $value } tiket selesai · lihat kalibrasi
backlog-stat-forecast = Poin tersisa
backlog-stat-forecast-hint = Perkiraan selesai { $value }
backlog-stat-forecast-late = Perkiraan selesai { $value }, melewati akhir sprint

## Sprint
sprint-new = Sprint baru
sprint-new-description = Durasi bawaan dua minggu, mulai setelah sprint terakhir.
sprint-create = Buat sprint
sprint-edit = Ubah nama, tujuan, tanggal
sprint-report = Laporan sprint
sprint-delete = Hapus sprint
sprint-delete-confirm = Hapus { $name }? Tiketnya kembali ke backlog.
sprint-options = Opsi { $name }
sprint-start = Mulai sprint
sprint-start-title = Mulai { $name }
sprint-start-description = { $count } tiket · { $points } dari { $capacity } poin kapasitas
sprint-start-description-plain = { $count } tiket · { $points } poin
sprint-start-parallel = { $name } masih aktif. Sprint ini akan berjalan paralel, atau selesaikan { $name } dulu.
sprint-start-unestimated = { $count } tiket belum diestimasi. Tiket tetap bisa masuk sprint, estimasi AI menyusul setelah brainstorming selesai.
sprint-complete = Selesaikan sprint
sprint-complete-title = Selesaikan { $name }
sprint-complete-description = Jadwal sprint { $end }.
sprint-complete-done = Selesai
sprint-complete-left = Belum selesai
sprint-complete-target = Pindahkan tiket yang belum selesai ke
sprint-complete-new = Sprint baru
sprint-complete-new-hint = Dibuat otomatis dengan durasi 2 minggu
sprint-complete-backlog-hint = Dikeluarkan dari semua sprint
sprint-complete-note = Job agent yang sedang berjalan tidak dihentikan, hanya sprint-nya yang berpindah. Laporan sprint ini tersimpan di Laporan.
sprint-points-hint = { $value } poin
sprint-state-active = Aktif
sprint-state-planned = Berikutnya
sprint-state-completed = Selesai
sprint-days-left = sisa { $days } hari ({ $workdays } hari kerja)
sprint-ticket-count = { $count } tiket
sprint-points = { $points } poin
sprint-points-of = { $points } / { $capacity } poin
sprint-done-points = { $done } dari { $points } poin selesai
sprint-over-capacity = Melebihi kapasitas { $over } poin. Pindahkan sebagian tiket ke sprint berikutnya.
sprint-unestimated = { $count } tiket belum punya estimasi. AI mengusulkan setelah brainstorming selesai.
sprint-empty = Sprint masih kosong
sprint-empty-description = Seret tiket dari backlog ke sini.
sprint-field-name = Nama sprint
sprint-field-goal = Tujuan sprint
sprint-field-start = Mulai
sprint-field-end = Selesai
sprint-field-capacity = Kapasitas (poin)
sprint-flash-created = Sprint dibuat.
sprint-flash-saved = Sprint disimpan.
sprint-flash-deleted = Sprint dihapus, tiketnya kembali ke backlog.
sprint-flash-started = Sprint dimulai.
sprint-flash-completed = Sprint selesai. Laporannya tersimpan di Laporan.
sprint-flash-moved = Tiket dipindahkan.

## Estimasi
estimate-ai-none = AI belum mengusulkan estimasi
estimate-ai-changed = AI mengusulkan { $ai } poin, disepakati { $points }
estimate-ai-accepted = Usulan AI { $ai } poin diterima
estimate-ai-hours = Durasi: { $ai } jam AI + { $human } jam manusia
estimate-missing = Belum diestimasi

## Field tiket (riwayat)
ticket-field-sprint = Sprint
ticket-field-release = Fix version
ticket-field-component = Component
