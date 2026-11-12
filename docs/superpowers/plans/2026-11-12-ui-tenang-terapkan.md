# UI "Tenang" Diterapkan ke M0–M1

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Langkah memakai checkbox (`- [ ]`).

**Goal:** Semua halaman M0–M1 memakai desain "Tenang" yang disetujui pemilik produk (prototype `docs/reference/ui-prototype/`, varian shell A, sec A, subnav E, table A, stat A, board C, prio C, auth B), dan pustaka komponen Tera siap dipakai M2–M12.

**Architecture:** Token `--ag-*` di `assets/css/app.css` (terang + `html.dark`) dipetakan ke variabel Basecoat; setiap komponen punya satu file CSS di `assets/css/components/` dan satu komponen Tera di `assets/views/components/`. Kerangka halaman di `assets/views/layouts/` (`app` → `page` → `project`; `settings`; `auth`; `public`). Data menu dihitung di Rust (`views::nav::side_nav`, `views::nav::project_views`), tabel server-side di `views::datatable` (spec → state → view), palet di `views::palette`. JS vanilla satu file per perilaku di `assets/static/js/`.

**Tech Stack:** Loco 1.3.1, Tera 2 (components), HTMX 4 (`outerMorph`, `hx-select`), Tailwind standalone + Basecoat 1.1, Geist/Geist Mono (vendor), ikon Tabler 3.51.0, Loco storage (disk/mem/S3) untuk logo.

## Global Constraints

- Tanpa Node/bundler; CSP `script-src 'self'` (tanpa inline script, `onclick`, `hx-on`).
- Semua teks lewat Fluent `id`; kunci UI baru di `assets/i18n/id/ui*.ftl`.
- Class project berawalan `app-`; file vendor tidak diedit; `| safe` tidak dipakai untuk konten user.
- GET tidak mengubah data; preferensi tab tampilan dan logo lewat POST.
- Fitur milestone berikutnya tidak dipalsukan: tampil sebagai empty state jujur atau item "segera".
- Komponen baru masuk katalog `/_dev/ui` di commit yang sama.

## Tasks

- [x] **1. Token dan CSS.** Geist + Geist Mono di-vendor (sha256 di `assets/vendor.sha256`); token prototype terang/gelap; override Basecoat (btn, badge, alert, dropdown, dialog, toast, table) dan CSS komponen baru.
- [x] **2. Data menu dan palet di Rust.** `views::palette` (18 warna, warna dari seed, inisial), `views::nav::side_nav` (menu aplikasi / menu Pengaturan), `views::nav::project_views`, `views::datatable`, `views::ui::{Stat, ComboOption}`.
- [x] **3. Komponen Tera.** button, field, textarea_field, select_field, combo (+ combo_options), card, savebar, subpage, stats, tabs, empty_state, dialog, flash, avatar, project_logo, person, setting_row, action_menu, color_picker, uploader, datatable, project_tabs, dot, crumb.
- [x] **4. Layout.** base (theme.js di head), app (shell A), page, project (bar tab tampilan), settings (menu Pengaturan), auth (split B), public (solo).
- [x] **5. Halaman autentikasi, solo, error, akun** → `layouts/auth.html`, `layouts/public.html`, `layouts/settings.html`.
- [x] **6. Beranda, project, pengaturan project.** Stats + card; logo project (unggah/hapus, Loco storage) + warna (color picker); tab tampilan per user (kolom `users.project_views`, POST `/account/project-views`); akses project dengan combobox.
- [x] **7. Pengaturan workspace.** Umum (subpage E), Anggota (datatable server-side, kolom Tim, filter role/tim/2FA), Tim, Jabatan, Role, Audit log (datatable); endpoint combobox orang/tim.
- [x] **8. Katalog `/_dev/ui`** memuat semua komponen; tes layout (`dev_ui`, `contracts/views`, `home`) diperbarui.
- [x] **9. Tes baru:** unggah logo (tipe dari isi berkas, batas ukuran, akses), pagination/sort/filter tabel, endpoint combobox, simpan tab tampilan.
- [x] **10. Screenshot Playwright** desktop 1440px, 390px, mode gelap; bandingkan dengan prototype dan perbaiki.
- [x] **11. Dokumentasi:** `docs/reference/design-system.md` (pemakaian komponen), `docs/superpowers/specs/2026-11-12-pustaka-komponen-design.md`.
- [x] **12. `bin/ci` hijau**, merge `--no-ff` ke `staging`, `docs/PROGRESS.md` (keputusan + utang teknis), commit.

## Keputusan desain

- Kontrol form memakai class sendiri (`app-input`, `app-field`, `app-check`, `app-switch`, `app-radio`, `app-choice`), bukan `.input`/`.field` Basecoat: selector Basecoat terlalu spesifik untuk ditimpa dengan rapi.
- Akun, pengaturan workspace, dan pengaturan project berbagi satu menu Pengaturan di sidebar (prototype `NAV_SETTINGS`); sub-halaman subpage E adalah rute server terpisah, bukan panel JS.
- Flash sukses tampil sebagai toast; info/peringatan/error sebagai alert di atas konten.
- Tabel server-side: form GET biasa (berfungsi tanpa JS), HTMX menukar region tabel saja (`hx-select` + `outerMorph`) dan memperbarui URL.
