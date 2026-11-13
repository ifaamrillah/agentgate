# Sistem Desain Agent Gate — "Tenang"

Acuan wajib untuk semua layar. Katalog hidup: `/_dev/ui` (development; tabelnya bisa di-sort,
difilter, dan dipindah halaman). Prototype yang disetujui pemilik produk:
`docs/reference/ui-prototype/` (buka `index.html`). Varian terpilih: shell A, judul seksi A,
sub-navigasi E, tabel A, stat A, board C, prioritas C, layar masuk B. Rancangan pustaka:
`docs/superpowers/specs/2026-11-12-pustaka-komponen-design.md`.

## Aturan

1. **Tidak ada warna, font, atau radius ditulis langsung di template.** Halaman hanya mengisi block
   layout dan merangkai komponen (`server/assets/views/components/`). Pola tata letak dibuat sebagai
   class `app-*` di `server/assets/css/components/`, bukan utilitas Tailwind di halaman.
2. Token hanya di `server/assets/css/app.css` (`--ag-*`, terang di `:root`, gelap di `html.dark`);
   variabel Basecoat dipetakan ke token. CSS komponen satu file per komponen; Basecoat tidak diedit,
   hanya ditimpa di `@layer components`.
3. Kontras teks minimal 4.5:1 di kedua mode. Mode gelap dipasang `static/js/theme.js` (ikut sistem
   sampai user memilih lewat menu profil "Ganti tema"; tersimpan di `localStorage` `ag-theme`).
4. Kontrol form memakai class sendiri: `app-input` (input/select/textarea, `data-group` untuk
   prefix), `app-check`, `app-switch` (`role="switch"`), `app-radio`, `app-choice`, `app-otp`.
   Bukan `.input`/`.field` Basecoat.
5. JS: satu file per perilaku di `static/js/`, dipanggil lewat atribut `data-*`; tanpa inline script.
6. Fitur milestone berikutnya tidak dipalsukan: tampilkan `empty_state` yang jujur atau item
   navigasi "segera" (`aria-disabled`, tanpa `href`).
7. Setelah mengubah CSS komponen, jalankan `bin/tailwind`.

## Token

| Bagian | Nilai |
|---|---|
| Permukaan | `bg` #fff, `side` #f7f7f5 (sidebar hangat), `page` #fff, `hover` #f0f0ee, `active` #e9e9e6, `muted-bg` #fafafb |
| Garis | `line` #ebebef, `line2` #e1e1e8, `input` #d9d9e1 |
| Teks | `fg` #17171c, `fg2` #3d3d47, `muted` #63636f, `faint` #8d8d99 |
| Aksen | `accent` #5b5bd6, `accent-soft` #f0f0fe, `accent-fg` #4747c2 |
| Status | `success` #1c7f45, `warn` #a35407, `danger` #cf2e2e, `info` #1d63c9 (masing-masing `-soft`) |
| Prioritas (C) | P1 merah solid, P2 oranye lembut, P3 kuning lembut, P4 abu (`.app-prio[data-p]`) |
| Font | Geist 400/500/600, Geist Mono 400/500 (kode, kunci, slug). Teks dasar 13px |
| Radius | tombol/input 6px, kartu 10px, panel utama 12px |
| Ikon | Tabler 3.51.0 outline, 16px, stroke 1.75 (`icon(name=...)`) |
| Palet project | 18 warna `views::palette::COLORS` + warna custom hex; warna bawaan dari kunci (`color_for`) |

## Kerangka halaman

```
sidebar 256px │ topbar 48px: ☰ workspace / ● project / … (breadcrumb)
(menu app atau│ pbar: tab tampilan project ………………………… aksi
 menu         │ konten (narrow 760 | mid 1040 | wide):
 Pengaturan)  │   kepala halaman: [logo] judul + deskripsi        [aksi]
              │   stats · card · grid · subpage
```

| Layout | Block | Dipakai |
|---|---|---|
| `layouts/base.html` | `title`, `body`; memuat theme.js (head) dan semua JS `defer` | — |
| `layouts/app.html` | `title`, `crumb`, `tabs`, `mtabs`, `actions`, `width`, `content`; sidebar dari `side` (`views::nav::side_nav`), banner hapus workspace, flash (sukses → toast) | jangan langsung |
| `layouts/page.html` | + `leading`, `heading`, `description`, `head_actions`, `main`, `page_attrs` | beranda, halaman umum |
| `layouts/project.html` | page + `project_tabs` di pbar, logo project | ringkasan project (dan tampilan M2+) |
| `layouts/settings.html` | `crumb_label`, `settings`, `page_attrs`; sidebar menjadi menu Pengaturan (Akun, Workspace, Project) | semua pengaturan dan akun |
| `layouts/auth.html` | `heading`, `description`, `card`, `below`; panel kiri gelap alur 4 langkah (B) | masuk, daftar, 2FA, reset, undangan |
| `layouts/public.html` | `content`, `foot`; satu kolom di tengah | pilih/buat workspace, error |

Sidebar dan tab hanya dibangun di `server/src/views/nav.rs` (`side_nav`, `settings_nav`,
`account_nav`, `project_settings_nav`, `project_views`). Item dirender dengan `data-nav="<key>"` dan
`aria-current="page"`. Pengaturan tidak muncul sebagai item sidebar: dibuka dari menu profil, lalu
sidebar berganti menjadi menu Pengaturan dengan "Kembali ke aplikasi".

## Komponen

Semua di `server/assets/views/components/`; parameter lengkap ada di komentar kepala tiap file.

| Komponen | Pakai untuk | Data Rust |
|---|---|---|
| `button` | tombol/tautan/pembuka dialog (`dialog="id"`); varian primary, outline, secondary, ghost, destructive, danger, link; ukuran sm, lg, icon, icon-sm | — |
| `field`, `textarea_field`, `select_field` | isian dengan label, hint, error Fluent (`error`, `error_min/max`), `prefix` | — |
| `combo` (+ `combo_options`) | pilihan orang/tim yang bisa dicari, satu atau banyak (`multiple`), opsi dari server (`src` → GET `?q=` membalas `combo_options`) | `views::ui::ComboOption` |
| `color_picker` | 18 warna palet + warna lain (hex) | `palette()` |
| `uploader` | unggah logo/gambar (klik atau tarik-lepas, langsung submit), hapus | — |
| `md_editor` | editor markdown Milkdown Crepe dengan Batal/Simpan; tanpa JS jadi textarea | — |
| `card` | kartu seksi: judul, deskripsi, `count`, tautan "lihat semua", varian danger/accent, `flush` untuk daftar/tabel; `form="<action>"` membungkus kartu dengan `savebar` | — |
| `savebar` | footer Batal + Simpan perubahan | — |
| `setting_row` | baris pengaturan (judul + penjelasan kiri, kontrol kanan) di card flush | — |
| `subpage` | halaman bertab vertikal (varian E): menu kiri + isi satu panel; tiap item = rute server | `[NavItem]` (`desc_key`, `count`, `alert`) |
| `stats` | 2–4 kartu angka (stat A) dengan tone, penyebut, petunjuk, tautan | `[views::ui::Stat]` |
| `tabs` | tab garis bawah dengan badge angka | `[NavItem]` |
| `project_tabs` | tab tampilan project + menu "Lainnya" + dialog "Atur tab" (sematkan, urutkan; tersimpan per user) | `views::nav::ProjectViews` |
| `datatable` | tabel server-side: cari, filter (menu checkbox + chip), sort, per halaman, nomor halaman, total; body = baris `<tr>` | `views::datatable::{TableSpec, TableView}` |
| `empty_state` | keadaan kosong jujur (dashed / plain) | — |
| `dialog` | dialog native; isi `<form><section/><footer/></form>` | — |
| `action_menu` | menu ⋯ per baris (`app-menu-item`, `data-variant="destructive"`) | — |
| `flash` | alert info/sukses/peringatan/error di atas konten | — |
| `avatar`, `person`, `project_logo`, `dot` | identitas orang/project (inisial di atas warna palet, atau gambar) | `palette::initials`, `color_for` |
| `badge` | status pill (info, ai, gate, success, neutral, destructive, primary, secondary, outline; `code`) | — |
| `crumb` | bagian breadcrumb di block `crumb` | — |
| `type_icon` | ikon tipe tiket berwarna (`tile` = kotak lembut) | `views::tickets::TypeView` |
| `prio` | label prioritas C (P1–P4), `full` menampilkan nama | `views::tickets::PriorityView` |
| `ticket_card` | kartu tiket board C: tipe + kode, judul, prioritas, label, poin, tenggat, penerima; menu ⋯ pindah status tanpa JS, drag-drop (`board-dnd.js`); `stages` = kolom stage, badge Menunggu review / Butuh keputusan (`app-tcard-state`) | `views::tickets::TicketRow` |
| `stage_stepper` | chip per stage workflow (selesai hijau bercentang, sekarang aksen, `aria-current=step`), gulir horizontal | `views::workflow::stepper` → `StageView` |
| `gate_list` | daftar gate keluar stage: centang hijau terpenuhi, silang merah belum, dengan petunjuk | `views::workflow::GateView` |
| `live_dot` | titik status hidup: `online` hijau, `busy` aksen, `offline` abu, dengan label | `views::runner::RunnerView::state` |
| `job_steps` | bar progres + daftar langkah job (ok/now/wait/fail, durasi) di `card flush` | `views::runner::StepsView` |
| `log_view` | log job gaya terminal (gelap di dua tema), warna per aliran, kursor saat berjalan; `job-log.js` menjaga gulir di bawah | `[views::runner::LogLineView]` |
| `evidence_item` | evidence bertanda tangan runner: ikon jenis, tautan berkas, lulus/gagal + jumlah tes atau ukuran, thumbnail screenshot | `views::runner::EvidenceView` |
| `pr_state` | status PR (Terbuka hijau, Draft abu, Merged info, Ditutup merah) dengan ikon git, nomor opsional, tautan ke provider; `compact` = ikon + nomor (kartu board) | `views::git::StateView::pr` |
| `ci_state` | status CI atau satu check (ikon berwarna tanpa latar: lulus, gagal, berjalan berputar, belum ada); `label` = nama check, `compact` = ikon saja | `views::git::StateView::ci` |

Pola isi (class): `app-grid[data-cols=duo|two|three]`, `app-stack-v`, `app-form`, `app-form-row`,
`app-blk`, `app-lrow` (baris daftar), `app-srow`, `app-props` (dl properti), `app-feed`/`app-fe`
(aktivitas), `app-who`, `app-bar`, `app-link`, `app-code`, `app-hint`, `app-muted`, `app-prio`,
`app-count`.

Tiket (M2, `components/ticket.css`): `app-status`/`app-status-pill[data-status]` (backlog abu, todo
info, dikerjakan aksen, selesai sukses), `app-chip[aria-pressed]` (filter cepat), `app-label-pill`,
`app-points`, `app-due[data-overdue]`, `app-av-empty` (belum ada penerima), `app-tref` (tipe +
kode), `app-board` > `app-bcol` (kolom, `data-drop` saat diseret), `app-ticket` (detail dua
kolom), `app-prose` (teks pre-wrap), `app-inline-form`, `app-drop` (area unggah lampiran),
`app-meter` (progres per status).

Workflow (M3, `components/workflow.css`): `app-stepper`/`app-stage-step[data-state]`, `app-gates`
(`li[data-ok]`), kartu stage tiket (`app-stage-head`, `app-stage-persona`, `app-stage-props`,
`app-stage-acts`, `app-stage-pair`), `app-choices[data-cols=one]` (radio kartu satu kolom:
transisi keluar, keputusan), `app-rounds` (tabel putaran gagal), artifact (`app-artifacts` =
navigator 300px | pratinjau, `app-versions`/`app-version[aria-current]`, `app-artifact-decide`),
`app-diff` > `span[data-line=add|del|same]`, builder (`app-builder` = kanvas | konfigurasi,
`app-flow` > `app-node[data-parallel]`, `app-node-note[data-tone=danger]`, `app-section-label`),
`app-template-stages` (chip nama stage), `app-change-list`, `app-event-note` (catatan di riwayat).
Halaman tiket memuat `tickets/_stage_head.html` (stepper + alert keadaan stage) di atas tab, dan
`tickets/_stage_dialogs.html` (reject, lompat/paksa, penugasan) sekali per halaman.

### Tabel server-side

1. Definisikan `TableSpec` statis (kolom `ColumnSpec::new(key).sort(..).hide("md").right()`,
   `FilterSpec`, sort bawaan, per halaman bawaan, kunci placeholder cari).
2. Controller menerima `Query<Vec<(String, String)>>`, `spec.parse(&query)` → `TableState`
   (`q`, `sort`, `desc`, `page`, `per`, `filter(key)`), jalankan query DB dengan `offset()`/`per`,
   hitung total, lalu `TableView::new(&spec, &state, base_path, total).with_filter_options(..)`.
3. Template: `{% <datatable table={table} empty_title=... > %}<tr>…</tr>{% </datatable> %}` di dalam
   `card flush={true}`. Semua kontrol adalah GET biasa (jalan tanpa JS); HTMX menukar `#<id>` saja
   (`hx-select` + `outerMorph`) dan memperbarui URL.

### Combobox dengan opsi dari server

`{{ <combo id=... name=... label=... options={[]} src="/w/acme/people"/> }}`; endpoint GET membaca
`q`, membalas partial `components/combo_options.html` (hanya `<option>`). GET tidak mengubah data.

## Status (badge `variant`)

| Variant | Arti |
|---|---|
| `info` | Siap dikerjakan, informasi |
| `ai` | Dikerjakan AI |
| `gate` | Menunggu gate/persetujuan |
| `success` | Selesai, aktif |
| `neutral` | Diarsipkan, nonaktif, label umum |
| `destructive` | Gagal, kedaluwarsa |
| `primary`, `secondary`, `outline` | Varian Basecoat (role, label umum) |

## M2+ (dari prototype)

Board varian C sudah dibangun di M2 (`ticket_card`, `tickets/board.html`), prioritas C
(`prio`). Detail tiket di M2 berupa halaman sendiri (`/w/{slug}/t/{code}`), bukan panel kanan. Pakai `screens-m2-m3.js` dst. sebagai acuan markup; komponen baru masuk
katalog `/_dev/ui` di commit yang sama.

## M4 (agent, MCP)

- **Persona**: tanda `app-pav` (`--pc` = warna persona, ikon Tabler; `data-size="sm"` 24px),
  kartu `app-pcard` di daftar, Edit persona = `subpage` varian E satu bagian per halaman.
- **Skill pack**: daftar `app-lrow` + badge status/risiko; detail = pohon file `app-ftree` +
  textarea (`app-codeview` untuk pratinjau), laporan pindai `app-check-item`/`app-finding`.
- **Hubungkan client AI** (Akun): `subpage` dengan `app-steps` (langkah bernomor + `app-codeblock`).
- **Persetujuan OAuth**: halaman solo (`layouts/public.html`) kartu `app-consent` (pasangan logo,
  user, combobox workspace, daftar scope, footer Tolak/Izinkan). Navigasi penuh (`hx-boost="false"`).
- **Draft tiket**: `app-lrow app-draft` dengan indentasi per kedalaman, checkbox `form=` ke form
  "Buat tiket" di kepala halaman (tanpa form bersarang), ubah lewat `dialog` per draft.

## M5 (runner & CLI)

- **Login perangkat** (`/device`): halaman solo tiga langkah (`app-device-*`): kode → setujui
  (pilih workspace) → selesai. Satu isian kode (bukan 8 kotak seperti prototype).
- **Runner & CLI** (`/w/{slug}/runner/{panel}`, menu Pengaturan untuk semua anggota): `subpage`
  varian E; antrean job memakai `datatable` (filter/sort di memori), runner = `app-lrow` +
  `live_dot`, pengaturan = `card form` + `setting_row` + `app-choices`.
- **Log job** (`/w/{slug}/jobs/{id}`): `app-job` (kolom utama | samping 320px, menumpuk < 900px);
  bagian `#job-live` memuat dirinya ulang lewat HTMX (`hx-trigger="every 2s"`, `outerMorph`) selama
  job terbuka. Aksi Lepas/Batalkan di bar aksi (`runner/_job_actions.html` + dialognya).
- **Kartu stage tiket**: blok `app-stage-job` (persona, status job, Lihat job, Ambil) dan evidence
  stage sekarang (`evidence_item`). Komentar `@agent` menampilkan status job dan penanda
  "menunggu runner Anda online" (`app-agent-wait`).
- **Izin aksi**: baris `app-lrow` + dialog Tinjau dengan `app-term-cmd` (perintah gaya terminal),
  satu form dengan tiga tombol submit (`decision=deny|allow|allow_ticket`).

## M6 (Git & PR)

- **Git provider** (`/w/{slug}/settings/git/{cloud|enterprise|repos}`, `ManageIntegrations`):
  `subpage` varian E + `stats`; provider = `app-lrow` dengan `app-prov-logo`, dialog Hubungkan
  (token, URL server GHES, daftar akses `app-consent-scopes`), Webhook manual (`app-props`),
  Putuskan (danger). Repo terhubung = `datatable` (filter/sort di memori).
- **Konvensi git project** (`/w/{slug}/p/{KEY}/settings/git/{panel}`): `subpage` (env/deploy/jalur
  kode "segera"), satu `card form` per panel dengan `setting_row` dan `app-choices`.
- **Pull request project** (`/w/{slug}/p/{KEY}/prs`, tab project `prs`): `stats`, `tabs`
  Semua/Terbuka/Merged, `datatable` dengan sel `app-pr-cell`, `ci_state`, `pr_state`.
- **Tab tiket PR & CI** (`/w/{slug}/t/{code}/pr`): satu `app-pr` per PR (kepala `app-pr-head` +
  daftar check `app-pr-checks`/`app-pr-check`), ukuran diff `app-pr-diff`.
- **Badge PR**: header tiket (`app-pr-link` → tab PR & CI, + CI) dan kartu board
  (`app-tcard-pr`, `pr_state compact` + `ci_state compact`). CSS: `components/git.css`.

## M7 (Code graph)

- **Komponen baru** (katalog `/_dev/ui`, kartu "Code graph (M7)"):
  - `confidence level=certain|inferred|unmatched|external`: badge keyakinan relasi
    (success/gate/destructive/neutral + ikon). Dipakai di trace, relasi lintas repo, ringkasan.
  - `lang_bar parts=[LangPart]`: bar bertumpuk komposisi bahasa; warna dari data
    (`views::graph::lang_color`), lebar lewat `--w`, label bahasa terbesar.
  - `code_view lines=[CodeLine]`: potongan kode bernomor baris (gulir horizontal, tinggi maks.),
    tanpa pewarnaan sintaks. Dipakai di detail simbol.
- **Cari simbol** (`/w/{slug}/p/{KEY}/graph`, tab project `graph`): `stats` + `tabs` (Cari simbol /
  Status index / Viewer M10 "segera"), form `app-graph-search`, hasil = `app-lrow app-sym`
  (ikon jenis simbol, path:baris, jumlah pemanggil/dipanggil, badge mati/tes). Tanpa kueri:
  daftar modul + node pusat.
- **Detail simbol** (`/graph/symbol/{id}`): kepala `app-symbol`, `code_view`, panel trace
  pemanggil/dipanggil (`confidence` per langkah), dampak perubahan (risiko rendah/sedang/tinggi),
  relasi lintas repo.
- **Status index** (`/graph/index/{repo|cross|git|quality|bench}`): `subpage` + panel; repo =
  `datatable` (status, sumber CLI/server, commit, `lang_bar`, cakupan `app-gi-*`), lintas repo dan
  kode mati = `datatable`, git = hotspot/coupling/pemilik, benchmark = hasil korpus bawaan.
  Index ulang lewat dialog `graph-reindex` (`EditProject`). CSS: `components/graph.css`.
