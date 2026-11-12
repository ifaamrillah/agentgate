# Pustaka Komponen & Kerangka Halaman

Tanggal: 2026-11-12 · Status: revisi 2 — desain "Tenang" disetujui pemilik produk (prototype
`docs/reference/ui-prototype/`), diterapkan di `feature/ui-tenang`. Revisi 1 (sidebar indigo, panel
flat, IBM Plex) digantikan seluruhnya.

## Maksud

Revisi 1 menyatukan anatomi halaman tetapi tampilannya masih dinilai lemah. Pemilik produk memilih
arah "Tenang" yang mirip Linear: sidebar netral hangat, area kerja putih melayang, tipografi Geist
13px, aksen indigo lembut `#5b5bd6`, kartu seksi dengan judul di dalam kartu, tab garis bawah, dan
mode gelap. Pustaka komponen ditulis ulang supaya halaman M0–M1 dan M2–M12 merangkai komponen yang
sama.

## Tujuan / non-tujuan / anti-tujuan

- **Tujuan**: setiap halaman hanya merangkai komponen; navigasi (sidebar, tab, sub-halaman) dibangun
  dari data Rust; tabel besar server-side; komponen form lengkap (combobox, warna, unggah, editor).
- **Non-tujuan**: board tiket dan panel detail (M2) — pustaka menyediakan dasarnya (`app-prio`,
  badge status, card).
- **Anti-tujuan**: class tampilan di halaman; data palsu untuk fitur yang belum dibangun; panel JS
  yang menyembunyikan sub-halaman (setiap sub-halaman adalah rute server).

## Anatomi halaman

```
┌ sidebar 256 ─┬ topbar 48: ☰  workspace / ● project / bagian ────────────────┐
│ workspace ▾  ├ pbar: Ringkasan  Board  Daftar  Backlog  Lainnya ▾ ……… aksi ┤
│ Beranda      │ konten (narrow 760 | mid 1040 | wide)                        │
│ PROJECT  +   │   kepala: [logo] judul + deskripsi                  [aksi]   │
│ ● Website    │   stats                                                       │
│   Board …    │   card │ card                                                 │
│ (profil ⋯)   │                                                              │
└──────────────┴──────────────────────────────────────────────────────────────┘
```

- `layouts/app.html` (shell A): sidebar `app-side` dari `side` (`views::nav::side_nav`), topbar
  breadcrumb, pbar (tab + aksi, otomatis disembunyikan bila kosong), banner hapus workspace, flash
  (sukses → toast Basecoat), laci di < 900px.
- `layouts/page.html`: kepala halaman + `main`. `layouts/project.html`: `project_tabs` di pbar.
- `layouts/settings.html`: akun, pengaturan workspace, dan pengaturan project berbagi satu menu
  Pengaturan di sidebar (Kembali ke aplikasi · Akun · Workspace · Project). Halaman dengan beberapa
  bagian memakai `subpage` (varian E) yang item-itemnya rute server.
- `layouts/auth.html` (B): panel gelap "alur 4 langkah" + kolom form 360px.
  `layouts/public.html`: satu kolom di tengah.

## Komponen

Daftar lengkap dan cara pakai: `docs/reference/design-system.md`. Ringkasnya: `button`, `field`,
`textarea_field`, `select_field`, `combo`/`combo_options`, `color_picker`, `uploader`, `md_editor`,
`card`, `savebar`, `setting_row`, `subpage`, `stats`, `tabs`, `project_tabs`, `datatable`,
`empty_state`, `dialog`, `action_menu`, `flash`, `avatar`, `person`, `project_logo`, `dot`,
`badge`, `crumb`. Dihapus dari revisi 1: `panel`, `subnav`, `stat`, `key_tile`, `meta_list`,
`disclosure`, `sidebar_nav`, `layouts/account.html`, `layouts/project_settings.html`,
`auth/_layout_card.html`.

Data Rust: `views::nav` (sidebar, tab, sub-halaman), `views::datatable` (spec → state → view),
`views::ui::{Stat, ComboOption}`, `views::palette` (18 warna, inisial, warna dari seed; fungsi Tera
`palette()`).

## Keputusan

- Kontrol form memakai class `app-*` sendiri, bukan `.input`/`.field` Basecoat (selector Basecoat
  terlalu spesifik untuk ditimpa rapi). Basecoat tetap dipakai untuk tombol, dropdown/popover,
  dialog, toast, alert, tabel, dan ditimpa di `@layer components`.
- Tab tampilan project bisa disematkan dan diurutkan per user (disimpan di server, POST); tampilan
  yang belum ada (M2+) tampil "segera".
- Tabel memakai form GET biasa sehingga bekerja tanpa JS; HTMX hanya mempercepat (swap region).
- Mode gelap mengikuti sistem sampai user memilih.

## Verifikasi

`bin/ci` hijau; katalog `/_dev/ui` merender semua komponen (dengan tes); screenshot Playwright
desktop, 390px, dan gelap dibandingkan dengan prototype.
