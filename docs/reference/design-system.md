# Sistem Desain Agent Gate

Acuan wajib untuk semua layar. Katalog hidup: `/_dev/ui` (development). Keputusan dan alasannya:
`docs/superpowers/specs/2026-11-11-sistem-desain-design.md`.

## Aturan

1. **Tidak ada warna, font, atau radius ditulis langsung di template.** Pakai komponen di
   `server/assets/views/components/`, utilitas Tailwind yang memetakan token (`bg-primary`,
   `text-muted-foreground`, `bg-ag-tint`, `font-mono`, ...), atau variabel `var(--ag-*)`.
   Halaman aplikasi hanya mengisi block `layouts/page.html` dan merangkai komponen; class tata letak
   dibuat sebagai class `app-*` di `css/components/`, bukan utilitas Tailwind di halaman.
2. Token hanya didefinisikan di `server/assets/css/app.css`. CSS komponen di `server/assets/css/components/`,
   satu file per komponen. Basecoat (`css/vendor/`) tidak pernah diedit.
3. Kontras teks minimal 4.5:1. Tanpa dark mode.
4. Setelah mengubah file di `css/components/`, jalankan `bin/tailwind` (watch tidak selalu menangkap file
   yang di-`@import`).

## Ringkasan pilihan

| Bagian | Nilai |
|---|---|
| Layout | Sidebar kiri + konten; mulai M2 panel detail tiket 320px di kanan board |
| Navigasi | Sidebar indigo-900 `#312e81`, teks putih, item aktif putih 14% |
| Aksen | Indigo: 50 `#eef2ff`, 100 `#e0e7ff`, 200 `#c7d2fe`, 600 `#4f46e5`, 700 `#4338ca`, 800 `#3730a3`, 900 `#312e81` |
| Netral | Slate: 50 `#f8fafc` (kepala tabel, hover baris), 100 `#f1f5f9`, 200 `#e2e8f0` (garis), 300 `#cbd5e1` (input), 600 `#475569` (teks redup), 900 `#0f172a` (teks) |
| Font | IBM Plex Sans 400/500/600/700; IBM Plex Mono 400/500/600 untuk kode/ID (kunci tiket, slug, PR) |
| Gaya | Flat: kanvas putih, panel bergaris slate-200 tanpa bayangan; tint indigo-50 hanya untuk penanda (item aktif, kotak kunci, ringkasan) |
| Radius | Tombol/input 6px, kartu 8px, badge pill, avatar 6px (`--radius: 0.5rem`) |
| Kepadatan | Teks dasar 14px, judul halaman 20px/600, judul panel 15px/600, meta 13px, tombol 36px, baris tabel 48px |
| Ikon | Tabler, stroke 2, ukuran `size-4` (16px) di tombol/navigasi |
| Badge | Pill lembut |
| Tab tampilan | Garis bawah indigo-600 |
| Tombol utama | Solid indigo-600, teks putih |
| Avatar | Kotak membulat, inisial indigo-700 di atas indigo-100 |

## Anatomi halaman (2026-11-12)

Rancangan: `docs/superpowers/specs/2026-11-12-pustaka-komponen-design.md`. Semua halaman aplikasi:

```
sidebar │ topbar: breadcrumb Workspace / Project / Bagian
        │ [subnav] │ kepala: [leading] judul + deskripsi        [actions]
        │          │ [tabs]
        │          │ main: panel, tabel, form
```

| Layout | Isi | Dipakai |
|---|---|---|
| `layouts/app.html` | sidebar, topbar + breadcrumb (`shell.area`), banner hapus, flash | jangan langsung |
| `layouts/page.html` | block `heading`, `description`, `leading`, `actions`, `tabs`, `main`, `aside`, `width` (`wide`/`narrow` 56rem), `page_attrs` | beranda, buat workspace/project, pilih, keluar |
| `layouts/settings.html` | page + subnav Pengaturan (`shell.subnav`, `views::nav::settings_nav`), narrow | Pengaturan workspace (Anggota, Role, Audit: wide) |
| `layouts/account.html` | page + subnav Akun, narrow | Akun |
| `layouts/project.html` | page + key_tile, nama, deskripsi, tab (`views::nav::project_tabs`) | Ringkasan project |
| `layouts/project_settings.html` | project + subnav Umum/Akses/Repo di `.app-split`; block `settings_main` | Pengaturan project |

Navigasi (tab, menu kiri) hanya ditulis di `server/src/views/nav.rs`; item dirender dengan
`data-nav="<key>"` dan `aria-current="page"`.

Komponen (`server/assets/views/components/`, contoh semuanya di `/_dev/ui`): `panel` (default, danger,
tint; `flush` untuk tabel/daftar), `setting_row`, `tabs`, `subnav`, `table` (header objek `{label,
hide, sr}`; td `data-hide`, `data-actions`), `action_menu`, `disclosure`, `stat`, `key_tile`,
`meta_list`, `person`, `field`/`select_field`/`textarea_field` (`hint`, `hide_label`), `button`,
`badge`, `avatar`, `flash`, `empty_state`, `sidebar_nav`. Pola tata letak di dalam isi: `app-form`,
`app-form-actions`, `app-inline-form`, `app-toolbar`, `app-stack`, `app-button-row`, `app-columns`
(2/3 + 1/3), `app-stats`, `app-master-detail`, `app-list`, `app-list-row`, `app-muted`, `app-code`.
Halaman masuk/daftar tetap `auth/_layout_card.html` (layar terbelah, `app-auth-brand`).

## Status (badge `variant`)

| Variant | Arti | Latar / teks |
|---|---|---|
| `info` | Siap dikerjakan, informasi | `#e0f2fe` / `#075985` |
| `ai` | Dikerjakan AI | indigo-100 / indigo-700 |
| `gate` | Menunggu gate/persetujuan | `#fef3c7` / `#92400e` |
| `success` | Selesai, aktif | `#dcfce7` / `#166534` |
| `neutral` | Diarsipkan, nonaktif | slate-100 / slate-600 |
| `destructive` | Gagal, kedaluwarsa | `#fee2e2` / `#b91c1c` |
| `primary`, `secondary`, `outline` | Varian Basecoat (role, label umum) | — |

Badge berisi kode/ID memakai `code={true}` (font mono).

## Tipe tiket dan prioritas (M2+)

Story `--ag-type-story` `#15803d`, Bug `--ag-type-bug` `#b91c1c`, Task `--ag-type-task` `#1d4ed8`.
Prioritas: Tinggi `--ag-priority-high`, Sedang `--ag-priority-medium`, Rendah `--ag-priority-low`.

## Kartu tiket (M2)

Isi lengkap: baris atas kunci (mono) + tipe + prioritas; judul; chip gate (`gate`) bila menunggu;
progres stage (5 segmen indigo-600 / slate-200) + teks stage mono; baris bawah nomor PR (mono) dan
penanggung jawab (chip agent indigo-50 atau avatar). Kolom board memakai `ag-tint`.
