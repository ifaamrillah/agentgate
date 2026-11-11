# Pustaka Komponen & Kerangka Halaman

Tanggal: 2026-11-12 · Status: dibangun di `feature/ui-library`, menunggu persetujuan user lewat screenshot

## Maksud

User menilai UI masih jauh dari Jira/ClickUp/Asana: komponen kurang reusable, layout tidak konsisten.
Akar masalahnya ada di cara halaman dibangun. Setiap halaman menyusun sendiri header, card + tabel,
baris pengaturan, dan menu kiri dengan class Tailwind lokal, dan parameter `class` dipakai untuk
mengubah tampilan (melanggar `loco-project-rules.md` §UI). Arah visual dirujuk dari skill
`ui-ux-pro-max`: untuk tool produktivitas, *Flat + Minimalism/Swiss*, informasi padat tapi terbaca,
hierarki lewat tipografi dan jarak, satu aksi utama per layar.

## Tujuan / non-tujuan / anti-tujuan

- **Tujuan**: setiap halaman aplikasi punya anatomi yang sama dan hanya *merangkai* komponen. Navigasi
  (tab, menu kiri) dibangun dari data Rust, bukan ditulis ulang di template.
- **Non-tujuan**: dark mode; board tiket (M2, tetapi panel/kolom memakai komponen di sini).
- **Anti-tujuan**: class tampilan di halaman; parameter `class` untuk warna/ukuran/padding; komponen
  dengan nama lain untuk hal yang sama; struktur halaman berbeda per jenis halaman.

## Anatomi halaman (sama untuk semua halaman aplikasi)

```
┌ sidebar ┬──────────────────────────────────────────────────────────┐
│ indigo  │ topbar 56px: breadcrumb (Workspace › Project › Bagian)   │
│         ├──────────────────────────────────────────────────────────┤
│         │ [subnav]  │ page head: judul + deskripsi      [aksi]     │
│         │ (opsional)│ tab (opsional)                               │
│         │           │ body: panel / tabel / form                   │
└─────────┴──────────────────────────────────────────────────────────┘
```

- `layouts/app.html`: sidebar + topbar (breadcrumb dari `shell`) + slot konten.
- `layouts/page.html`: block `heading`, `description`, `actions`, `tabs`, `body`, `width`
  (`wide` = penuh, untuk tabel/board; `narrow` = maks 56rem, untuk form/pengaturan), dan `aside`
  (menu kiri; bila diisi, menu berada di samping seluruh kepala + isi).
- Pengaturan workspace dan Akun memakai menu kiri dari `shell.subnav` (Rust, `views::nav`);
  halaman project memakai tab dari `views::nav::project_tabs`.

## Perubahan visual

- Kanvas putih (bukan slate-50), panel bergaris slate-200 radius 8px tanpa bayangan (gaya *flat*).
  Tint indigo hanya untuk penanda: item aktif, kolom board, kotak kunci project, area ringkasan.
- Topbar putih bergaris bawah, breadcrumb 13px. Judul halaman 20px/600, judul panel 15px/600,
  teks 14px, meta 13px, label tabel 12px/500.
- Ritme jarak 4/8: padding halaman 24px (32px ≥ lg), jarak antarpanel 24px, isi panel 20px, baris
  tabel ≥ 44px dengan hover slate-50.

## Komponen (lapisan 4) dan CSS (lapisan 3, prefiks `app-`)

| Komponen | Isi | Varian |
|---|---|---|
| `panel` | judul, deskripsi, tautan aksi, isi | `default`, `danger`, `flush` (isi tanpa padding: tabel/daftar) |
| `setting_row` | judul + penjelasan kiri, kontrol kanan | — |
| `tabs` | tab tautan dari `items` | item: aktif, `soon` (nonaktif + label "segera") |
| `subnav` | menu kiri dari `items`, judul kecil | sama |
| `stat` | label, nilai, ikon | — |
| `action_menu` | tombol ⋯ + popover berisi form | — |
| `key_tile` | kotak kode project mono | `md`, `lg` |
| `meta_list` | daftar label → nilai | nilai `mono` |
| `select_field`, `textarea_field` | label + kontrol + error (pasangan `field`) | `hide_label` |
| `table` | kini punya `flush` lewat panel | — |
| yang ada | `button`, `badge`, `avatar`, `field`, `flash`, `empty_state` | tetap |

`page_header` dan `card` dihapus setelah semua halaman pindah (digantikan layout halaman dan `panel`).

## Yang diserahkan ke builder

Nama block persis, isi breadcrumb per jenis halaman, dan pemetaan halaman lama ke komponen baru.
Verifikasi: `bin/ci` hijau, katalog `/_dev/ui` merender semua komponen + contoh halaman, screenshot
desktop 1280 dan HP 390 untuk persetujuan user sebelum diterapkan ke semua halaman.
