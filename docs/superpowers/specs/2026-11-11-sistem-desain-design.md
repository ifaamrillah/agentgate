# Sistem Desain Agent Gate

Tanggal: 2026-11-11 · Status: menunggu persetujuan user

## Maksud

Tampilan M0–M1 masih memakai gaya bawaan Basecoat Vega (hitam-putih, font sistem). User ingin tampilan
yang punya identitas sendiri dan **selalu konsisten** di semua milestone berikutnya. Pilihan dibuat user
lewat konfigurator interaktif (artifact "Agent Gate – Pilihan Arah UI"), satu opsi per bagian.

## Tujuan, non-tujuan, anti-tujuan

- **Tujuan**: satu sumber token (warna, font, radius, jarak, bayangan) yang dipakai semua halaman; komponen
  Tera yang sudah ada (`components/*.html`) mengikuti token itu; shell aplikasi (sidebar) memakai gaya baru;
  semua layar M1 ikut berubah tanpa mengubah perilakunya.
- **Non-tujuan**: dark mode (spec: tanpa dark mode); panel detail tiket dan board (baru ada di M2, tetapi
  token dan pola layout-nya ditetapkan sekarang); logo final.
- **Anti-tujuan**: warna/ukuran ditulis langsung di template (hex di HTML); Basecoat vendor diedit; font dari
  CDN (aplikasi tanpa CDN, CSP `self`); kontras teks di bawah 4.5:1.

## Keputusan user (15 bagian)

| # | Bagian | Pilihan |
|---|---|---|
| 1 | Layout | Sidebar + panel detail (panel detail tiket di kanan, muncul di M2) |
| 2 | Warna navigasi | Warna aksen (sidebar indigo gelap, teks putih) |
| 3 | Warna aksen | Indigo |
| 4 | Warna dasar | Slate |
| 5 | Font utama | IBM Plex Sans |
| 6 | Font kode tiket | IBM Plex Mono |
| 7 | Gaya visual | Bernuansa warna (kolom/area bertint indigo, kartu putih bergaris indigo muda) |
| 8 | Radius | Sedang (tombol 6px, kartu 8px, badge 4px) |
| 9 | Kepadatan | Normal (teks dasar 14px, tinggi tombol 36px) |
| 10 | Ikon | Tabler, stroke 2 |
| 11 | Badge status | Pill lembut |
| 12 | Kartu tiket | Lengkap (tipe, prioritas, gate, progres stage, PR) |
| 13 | Tab tampilan | Garis bawah indigo |
| 14 | Tombol utama | Solid indigo |
| 15 | Avatar | Kotak membulat |

## Token

Didefinisikan sekali di `server/assets/css/app.css` (lapisan token, `:root`) dengan menimpa variabel Basecoat
(`--primary`, `--background`, `--sidebar*`, `--radius`, `--font-sans`, `--font-mono`, dst.) plus token
tambahan `--ag-*` untuk hal yang Basecoat tidak punya. Nilai:

- **Indigo (aksen)**: 50 `#eef2ff` (tint), 100 `#e0e7ff` (soft/garis kartu), 600 `#4f46e5` (tombol, fokus),
  700 `#4338ca` (teks aksen), 900 `#312e81` (sidebar).
- **Slate (netral)**: latar `#f8fafc`, permukaan `#ffffff`, sub `#f1f5f9`, garis `#e2e8f0`, teks `#0f172a`,
  teks redup `#475569`.
- **Sidebar**: latar `#312e81`, teks `#ffffff`, teks redup `rgba(255,255,255,.78)`, label `rgba(255,255,255,.66)`,
  item aktif `rgba(255,255,255,.14)`.
- **Status (semantik, dipakai badge)**: siap = sky (`#e0f2fe`/`#075985`), AI = indigo (`#e0e7ff`/`#4338ca`),
  gate = amber (`#fef3c7`/`#92400e`), selesai = hijau (`#dcfce7`/`#166534`), bahaya = merah
  (`#fee2e2`/`#b91c1c`), netral = slate (`#f1f5f9`/`#475569`).
- **Tipe tiket**: Story hijau `#15803d`, Bug merah `#b91c1c`, Task biru `#1d4ed8`. Prioritas: Tinggi
  `#b91c1c`, Sedang `#a16207`, Rendah slate.
- **Font**: `--font-sans: "IBM Plex Sans"`, `--font-mono: "IBM Plex Mono"`; berat 400/500/600/700 (mono
  400/500/600). File woff2 di-vendor ke `assets/static/vendor/fonts/` (lisensi OFL ikut disalin), checksum
  masuk `assets/vendor.sha256`.
- **Radius**: tombol/input 6px, kartu/panel 8px, badge pill (999px), avatar 6px.
- **Kepadatan**: teks dasar 14px, judul halaman 22px/700, tombol 36px, padding kartu 12px, jarak kolom 14px.
- **Bayangan**: gaya "bernuansa warna" tidak memakai bayangan berat; kartu = garis `#e0e7ff`, area = tint
  `#eef2ff`. Bayangan hanya untuk menu/dialog melayang.

## Pola komponen

- **Shell**: sidebar 240px indigo gelap (switcher workspace, Beranda, daftar PROJECT, Pengaturan, profil di
  bawah); drawer di layar HP (perilaku yang ada dipertahankan). Area konten putih. Mulai M2: panel detail
  tiket 320px di kanan board.
- **Header halaman**: breadcrumb mono kecil + judul + aksi kanan; tab tampilan bergaris bawah indigo.
- **Tombol**: utama solid indigo-600 teks putih; sekunder putih bergaris slate; bahaya merah.
- **Badge**: pill lembut (latar 100, teks 700), satu macro `badge(kind=...)` dengan kind = status di atas.
- **Kode/ID** (KSR-24, slug, token audit, nomor PR): IBM Plex Mono.
- **Avatar**: kotak membulat 6px, inisial, warna dari palet tetap per user.
- **Form, tabel, kartu, flash, empty state**: komponen Basecoat dengan token baru; tidak ada gaya lokal per halaman.

## Pendekatan

1. **Token di atas Basecoat (dipilih)**: timpa variabel Basecoat + token `--ag-*` di lapisan token, sesuaikan
   macro di `components/`, rombak `layouts/app.html` + `sidebar_nav.html`. Perubahan terpusat, Basecoat tetap
   vendor murni, semua halaman ikut otomatis.
2. Ganti Basecoat dengan CSS sendiri: kontrol penuh, tetapi membuang komponen yang sudah jalan dan melanggar
   stack di spec.
3. Gaya per halaman (utility Tailwind langsung): cepat, tetapi tidak konsisten — persis yang ingin dihindari.

## Yang diserahkan ke builder

- Turunan warna untuk hover/fokus/disabled (mengikuti skala indigo/slate di atas, kontras ≥ 4.5:1).
- Wordmark sementara: tanda gerbang (ikon garis) + "Agent Gate" Plex Sans 600; logo final dibahas terpisah.
- Halaman `/_dev/ui` diperbarui jadi katalog sistem desain (token + semua komponen) sebagai acuan milestone
  berikutnya; dokumen ringkas di `docs/reference/design-system.md`.
- Verifikasi: `bin/ci` hijau; screenshot desktop + HP semua layar M1 sebelum/sesudah.
