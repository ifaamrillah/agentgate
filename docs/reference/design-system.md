# Sistem Desain Agent Gate

Acuan wajib untuk semua layar. Katalog hidup: `/_dev/ui` (development). Keputusan dan alasannya:
`docs/superpowers/specs/2026-11-11-sistem-desain-design.md`.

## Aturan

1. **Tidak ada warna, font, atau radius ditulis langsung di template.** Pakai komponen di
   `server/assets/views/components/`, utilitas Tailwind yang memetakan token (`bg-primary`,
   `text-muted-foreground`, `bg-ag-tint`, `font-mono`, ...), atau variabel `var(--ag-*)`.
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
| Netral | Slate: 50 `#f8fafc` (latar halaman), 100 `#f1f5f9`, 200 `#e2e8f0` (garis), 300 `#cbd5e1` (input), 600 `#475569` (teks redup), 900 `#0f172a` (teks) |
| Font | IBM Plex Sans 400/500/600/700; IBM Plex Mono 400/500/600 untuk kode/ID (kunci tiket, slug, PR) |
| Gaya | Bernuansa warna: kartu putih bergaris indigo-100 tanpa bayangan; area pengelompok `ag-tint` (indigo-50) |
| Radius | Tombol/input 6px, kartu 8px, badge pill, avatar 6px (`--radius: 0.5rem`) |
| Kepadatan | Teks dasar 14px, judul halaman 22px/700, tombol 36px |
| Ikon | Tabler, stroke 2, ukuran `size-4` (16px) di tombol/navigasi |
| Badge | Pill lembut |
| Tab tampilan | Garis bawah indigo-600 |
| Tombol utama | Solid indigo-600, teks putih |
| Avatar | Kotak membulat, inisial indigo-700 di atas indigo-100 |

## Tata letak per halaman (pilihan user, 2026-11-12)

Opsi dari artifact "Agent Gate – Opsi Layout per Halaman". Halaman baru mengikuti pola yang sama.

| Halaman | Pola | Kerangka |
|---|---|---|
| Halaman project | Kepala project (kunci di kotak tint, nama, deskripsi, akses) + tab bergaris bawah (Ringkasan, Board/Backlog/Knowledge/AI Office, Pengaturan); Ringkasan = board 2/3 + kolom Repo & Tentang project | `layouts/project.html` |
| Pengaturan project | Tab project + menu kiri (Umum, Akses, Repo); isi dibungkus satu card putih dengan baris label (`ag-setting-row`: judul + penjelasan kiri, isian kanan); zona bahaya card merah (`ag-card-danger`) | `layouts/project_settings.html` |
| Beranda workspace | Kartu statistik + tabel project dalam card | `workspaces/home.html` |
| Pengaturan workspace (dan Anggota, Tim, Jabatan, Role, Audit log) | Menu kiri (`ag-subnav`) dari `shell.settings`, bagian aktif `shell.section` | `layouts/settings.html` |
| Anggota | Tabel dalam card + menu aksi per baris (popover Basecoat: role, jabatan, keluarkan); undang = panel lipat `<details>` | `workspaces/members/index.html` |
| Audit log | Tabel + panel detail kejadian terpilih (`?event=`, default baris pertama) | `workspaces/audit.html` |
| Akun | Menu kiri (Profil, Password, Autentikasi dua langkah) | `layouts/account.html` |
| Masuk/daftar/undangan/reset | Layar terbelah: panel merek indigo-900 (layar lebar) + form | `auth/_layout_card.html` |

Komponen pola ada di `css/components/layout.css`: `ag-tabs` (tab link, `aria-current="page"`),
`ag-subnav` (menu kiri, jadi baris geser di HP), `ag-setting-row`, `tr[aria-selected]`. Tabel di dalam
card memakai `card py-0` (padding kolom tepi otomatis); card berisi popover diberi `overflow-visible`.

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
