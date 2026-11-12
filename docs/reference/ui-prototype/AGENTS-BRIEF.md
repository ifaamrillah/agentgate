# Brief bersama: prototype UI Agent Gate

Kita membuat **prototype klik-able** seluruh produk Agent Gate (M1 sampai M12) untuk direview pemilik produk sebelum UI asli dibangun. Pemilik produk ingin kualitas setara Linear / Jira / ClickUp: tenang, rapi, padat tapi lega, konsisten. Arah desain sudah dipilih: **arah A "Tenang"** (mirip Linear). Pemilik produk orang Indonesia; **semua teks UI berbahasa Indonesia** (istilah teknis umum seperti PR, branch, commit, runner, gate, worktree, token tetap Inggris).

Spesifikasi produk lengkap: `/home/sevima-189/SEVIMA/Private/agentgate/docs/spec.md`. Baca bagian yang relevan dengan milestone Anda (tabel "Urutan build MVP" di akhir + bagian fiturnya). Setiap fitur yang disebut spec untuk milestone Anda harus punya tempat di UI.

## Berkas

Folder: `/tmp/claude-1000/-home-sevima-189-SEVIMA-Private-agentgate/0d219ae8-ed19-41cd-95fd-25fd4426fff9/scratchpad/proto/`

- `index.html` memuat `icons.js`, `core.js`, lalu modul layar, lalu `boot.js`.
- `style.css` dan `core.js` adalah fondasi bersama. **Baca keduanya sampai habis sebelum menulis.** Jangan ubah keduanya, dan jangan ubah modul agen lain. Kalau butuh CSS khusus, tambahkan di modul Anda lewat `AG.css(\`...\`)` dengan nama kelas berawalan unik (misalnya `kb-`, `gv-`). Kalau menurut Anda fondasi perlu diubah, tulis usulannya di laporan akhir.
- Anda hanya menulis **satu berkas modul** (disebut di tugas Anda). Bungkus dalam IIFE: `(() => { const { shell, head, btn, ... } = AG; ... })();`

## API inti (lihat core.js untuk detail)

- `AG.screen({ m: "M2", g: "Kelompok", n: "Nama layar", route: "/p/:k/board", path: "/p/KSR/board", render(params) => html, after(root, params) })`. `path` = contoh URL konkret untuk peta layar (wajib bila route punya parameter).
- `AG.shell({ nav: "app"|"settings", project, crumbs, tabs, mtabs, actions, body, flush, drawer, width: "narrow"|"mid"|"wide" })`. Halaman pengaturan memakai `nav: "settings"` + `width: "narrow"` (760px). Halaman project memakai `nav: "app", project: "KSR"`, `crumbs: AG.projectCrumb("KSR", "Nama")`, dan `...AG.projectTabs("KSR")` bila layar termasuk tampilan project (Board/Daftar/Backlog/Roadmap/Kalender/Rilis/Laporan). Untuk pengaturan project pakai `nav: "settings", project: "KSR"` (sidebar membuka sub-menu project).
- Komponen: `AG.ic(nama, kelas)`, `AG.av(kode, ukuran)` (kode orang `OO/AA/DD/QQ/VV`, atau kunci persona `dev/qa/...`), `AG.btn(label, kelas, ikon, atribut)`, `AG.inp`, `AG.sel`, `AG.fld`, `AG.pill`, `AG.sw`, `AG.cb`, `AG.srow`, `AG.section`, `AG.head`, `AG.who`, `AG.empty`, `AG.stageIcon`, `AG.prioIcon`, `AG.typeIcon`, `AG.statePill`, `AG.labelPill`, `AG.tref`, `AG.projDot`, `AG.ticketCard`.
- Kelas CSS siap pakai: `.box .srow .lrow .boxfoot .tbl .stats .stat .grid2 .grid3 .split2 .props .tabs .seg .chip .pill .alert .code .term .diff .stages .stg .bar .empty .tile .feed .fe .drawer .card .agentbox .board .bcol .toolbar .prose .row .col .grow .mut .faint .mono .trunc` dll.
- Data: `AG.PEOPLE`, `AG.PROJECTS`, `AG.PERSONAS`, `AG.STAGES` (workflow KSR), `AG.STATES`, `AG.TYPES`, `AG.PRIORITY`, `AG.LABELS`, `AG.TICKETS`, `AG.RUNNERS`, `AG.NOTIFS`. Pakai data ini supaya semua layar saling cocok (orang, kode tiket, runner, project sama di mana-mana). Data tambahan khusus modul Anda boleh didefinisikan di modul Anda.

## Interaksi (semua lewat atribut, ditangani core.js)

- Navigasi: `<a href="#/rute">` atau `data-go="#/rute"` pada elemen apa pun.
- Dialog: `AG.dialog("nama", (arg) => ({ title, desc, icon, tone: "dng"|"ok"|"acc"|"warn", w: "w560"|"w640"|"w760"|"w960", body, foot, after(el) }))`, dibuka oleh `data-dialog="nama" data-arg="..."`. Tombol `data-close` menutup dialog.
- Menu dropdown: `AG.menu("nama", (arg, el) => html item .mi/.msep/.mlab)`, dibuka oleh `data-menu="nama" data-arg="..."`.
- `data-toast="Teks"` menampilkan toast (bisa digabung dengan `data-close`).
- `data-toggle` mengganti kelas `on` (dipakai `.sw`, `.cb`, chip).
- Tab lokal: wadah `data-tabs-root`, tombol `data-tab="x"`, panel `data-panel="x"` (panel aktif diberi kelas `on`).
- `.seg > *` otomatis berpindah `on`. `data-radio="grup"` untuk pilihan radio.
- Aksi khusus: `AG.action("nama", (el, arg, event) => {...})` dipicu `data-act="nama" data-arg="..."`. Data boleh dimutasi lalu panggil `AG.rerender()`.
- Ctrl K, peta layar, tema gelap, tombol layar sebelumnya/berikutnya sudah ada di core.

**Setiap elemen yang terlihat bisa diklik harus melakukan sesuatu**: pindah layar, buka dialog/menu, toast, atau toggle. Jangan ada tombol mati. Link ke layar milik agen lain boleh (rute ada di sidebar core.js); jangan buat ulang layar milik agen lain.

## Standar kualitas

- Gaya Linear: teks 13px, judul halaman 20px/600, banyak ruang kosong, garis tipis, satu warna aksen. Ikuti pola layar yang sudah ada (lihat `screens-m1.js` milik agen M1 bila sudah ada, dan `/tmp/claude-1000/-home-sevima-189-SEVIMA-Private-agentgate/11af2280-46d4-42ff-ad94-2f8d853d571d/scratchpad/mockup-a/template.html` = mockup M1 yang sudah disetujui gayanya).
- Isi realistis dan spesifik konteks toko retail (Kasir Online, QRIS, struk, stok), bukan lorem ipsum. Teks UI singkat, kalimat biasa (bukan Huruf Kapital Semua Kata).
- Tampilkan keadaan yang menarik: ada data, ada yang menunggu review, ada yang gagal, ada empty state bila relevan.
- Warna hanya lewat token CSS (`var(--accent)` dst) supaya mode gelap jalan. Hex mentah hanya untuk warna data (label, project, persona, avatar).
- Responsif sampai lebar 390px: tidak ada scroll horizontal halaman (tabel lebar dibungkus `.tblwrap`, kolom sekunder diberi `.hide-sm`). Board/roadmap/graph boleh scroll horizontal di dalam wadahnya.
- Grafik, diagram, kantor isometrik, graph: **SVG inline buatan sendiri**, tanpa library eksternal. Teks di SVG pakai `fill="currentColor"` atau token.
- Ikon hanya dari `icons.js` (cek `Object.keys(ICONS)`; kalau tidak ada, pilih ikon lain yang ada). Jangan emoji.
- Aksesibilitas dasar: tombol ikon punya `aria-label`, kontras cukup (`--muted` untuk teks sekunder, `--faint` hanya untuk ikon/meta kecil).

## Verifikasi (wajib sebelum selesai)

Skrip screenshot: `node /tmp/claude-1000/-home-sevima-189-SEVIMA-Private-agentgate/0d219ae8-ed19-41cd-95fd-25fd4426fff9/scratchpad/shot.mjs <folder-keluaran> <path1,path2,...> [1=mobile] [1=dark]`. Simpan keluaran di folder Anda sendiri, misalnya `.../scratchpad/shots-<modul>/`. Skrip mencetak error konsol; harus `errors: []`.

1. Screenshot **setiap** layar Anda di desktop, lalu lihat gambarnya (Read) dan perbaiki yang jelek, terpotong, bertumpuk, atau tidak rata.
2. Screenshot minimal 4 layar terpenting di mobile (390px) dan 2 layar di mode gelap, lalu perbaiki.
3. Uji dialog terpenting: buka lewat Playwright sendiri bila perlu (skrip shot.mjs bisa Anda salin dan ubah ke folder Anda).

## Laporan akhir (singkat)

Daftar layar (rute + nama), dialog/menu/aksi yang dibuat, hal yang sengaja tidak dibuat, dan usulan perubahan fondasi bila ada. Jangan tempel kode.

---

# v2: revisi setelah review pemilik produk (WAJIB dibaca)

Pemilik produk menilai **UI sudah banyak yang bagus, tetapi UX kurang**: satu halaman sering memuat terlalu banyak bagian dan informasi. Pemilik produk **tidak keberatan ada banyak halaman, asal UX-nya bagus**. Pecah halaman yang padat menjadi sub-halaman (menu vertikal kiri) atau rute terpisah.

Fondasi sudah diperbarui (baca ulang bagian akhir `core.js` dan `style.css`, mulai dari komentar "v2"). Latar area konten sekarang abu-abu (`--page`), sidebar lebih besar. Cadangan versi lama ada di `../proto-v1-backup/` bila perlu membandingkan.

## Komponen baru (pakai, jangan buat versi sendiri)

- `AG.savebar(label, { msg, extra, toast, cancel })`: bar simpan mengambang di bawah halaman form.
- `AG.combo({ options: [{ v, l, sub, lead, group }], value, ph, multi, w })`: select yang bisa dicari. Wajib untuk pilihan orang, tim, tiket, repo, persona, skill, atau daftar apa pun yang bisa panjang. Memicu event `combo-change`.
- `AG.editor({ html, ph, h, full, compact, footer })`: editor rich text lengkap (toolbar, WYSIWYG ⇄ Markdown, mention, emoji, tabel, blok kode, Mermaid, lampiran). `compact: true` untuk komentar. `AG.mdToHtml(md)` untuk mengisi dari Markdown.
- `AG.colorPicker(nilai)`: banyak warna + warna custom (hex / pemilih). Event `color-change`.
- `AG.stats([{ label, value, icon, hint, tone, go, delta }])`: kartu angka. **Ganti semua strip angka lama** (`.stats/.stat`) dengan ini. Bentuknya mengikuti varian pilihan pemilik produk.
- `AG.subpage({ title, desc, head, nav: [{ k, label, icon, count, desc, go, group }], active, body })` + `AG.panel(k, html, aktif)`: halaman dengan **menu tab vertikal di kiri, konten di kanan**. Item tanpa `go` berganti panel lokal; item dengan `go` pindah rute.
- `AG.filterBar({ id, search, filters: [{ k, label, icon, options, value }], quick: [{ label, icon, on }], right, count })` + `AG.filterValue(id, k)` / `AG.filterQuery(id)`: bar filter rapi yang tetap rapi walau filternya banyak.
- `AG.datatable({ id, columns: [{ label, sort, w, hide: "sm", align }], rows: [{ cells, sort, text, filter }], filters, search, quick, right, pageSize, select, bulk, empty, rowGo })`: tabel data lengkap (filter, pencarian, sort per kolom, pilih banyak + aksi massal, pagination dengan jumlah per halaman, total data, nomor halaman, navigasi). **Semua tabel data** memakai ini. `id` harus tetap (bukan acak) supaya state halaman/sort bertahan saat render ulang.
- `AG.prioIcon(p)` dan `AG.prioLabel(p)`: ikon prioritas sesuai varian. Jangan buat ikon prioritas sendiri.
- `AG.authBrand()`: panel kiri halaman masuk sesuai varian.

## Aturan global baru

1. **Latar abu-abu**: konten yang selama ini menempel langsung di halaman (properti, aktivitas, deskripsi, feed, daftar) harus dibungkus `.box` (kartu putih) supaya konsisten.
2. **Tombol Simpan tidak boleh di samping judul.** Halaman form memakai `AG.savebar(...)` di bagian bawah. Aksi di kanan judul hanya untuk membuat/navigasi (misalnya "Undang anggota", "Tiket baru").
3. **Judul, tab, dan tombol tidak boleh berjajar dalam satu baris.** Urutan: judul + deskripsi (+ aksi buat) → di bawahnya tab → konten.
4. **Semua tab pakai gaya garis bawah `.tabs`** (seperti tab Laporan: Burndown, Burnup, Velocity…). Jangan pakai `.seg` sebagai tab atau filter. `.seg` hanya untuk sakelar mode kecil 2–3 pilihan di dalam toolbar (misalnya Web/Mobile, WYSIWYG/Markdown).
5. **Halaman yang padat dipecah.** Halaman pengaturan dengan ≥3 bagian memakai `AG.subpage` (menu vertikal kiri). Detail yang besar memakai sub-rute. Editor konten panjang adalah **halaman sendiri dengan editor lengkap**, bukan modal, dan bukan textarea.
6. **Grid beberapa kolom harus sejajar**: judul bagian di tiap kolom sejajar di atas, kartu sama tinggi (`align-items: stretch`).
7. Semua label/deskripsi di `srow` dalam panel sempit harus tetap rapi (fondasi sudah memakai container query; cek hasilnya, jangan sampai teks terlipat per kata).
8. Pertahankan semua interaksi lama yang masih relevan dan rute lama (boleh menambah rute baru). Setiap rute baru didaftarkan dengan `AG.screen`.

### Pembaruan fondasi (selama revisi v2)
- Item `AG.subpage` / `[data-tab]` dengan `href="#"` tidak lagi mengubah rute; tab dan panel aktif diingat per rute saat render ulang. Tambalan lokal di modul tidak diperlukan lagi.
- Kotak cari `AG.filterBar` memancarkan event `filter-search` (`detail: { id, q }`) untuk layar non-tabel.
- Tombol prototype "Peta layar" naik otomatis bila ada `.savebar`.

### Keputusan pemilik produk (2026-11-12)
Varian yang dipakai: shell A (panel melayang), auth B, stat A, subnav B, table A, board C, prio C. Ini sudah jadi bawaan di `AG.UI`. Knowledge memakai opsi B (beranda + kartu kelompok). AI Office memakai opsi A (isometrik), tetapi tampilannya harus jauh lebih bagus.

## v3: revisi pemilik produk (wajib, berlaku di semua layar)
Fondasi baru di core.js:
- `AG.card({ title, desc, action, body, foot, pad = true })`: kartu bagian. Judul, deskripsi, dan aksi ada **di dalam** kartu (header), `foot` menjadi footer kartu. `AG.section(t, d, body, act)` kini otomatis memakai `AG.card`. `.box` yang menjadi anak langsung isi kartu otomatis menyatu (tanpa bingkai ganda). Untuk daftar baris rapat ke tepi, pakai `pad: false`.
- `AG.seeAll(href, label = "Lihat semua")`: tautan teks dengan panah, untuk aksi header kartu.
- `AG.savebar(...)` sekarang **baris footer biasa**, bukan kartu melayang. Letakkan di `foot` kartu: `AG.card({ ..., foot: AG.savebar("Simpan") })`.
- `AG.editor({ save: true | "Label" })` menambah tombol Batal + Simpan di footer editor.
- `AG.shell`: `tabs` dan `actions` kini tampil di bar di bawah topbar. Topbar hanya berisi breadcrumb.
- `AG.head(t, d, act, lead)`: `lead` berisi panah kembali diabaikan.

Aturan:
1. **Judul + deskripsi bagian tidak boleh di luar kartu.** Setiap bagian (Perlu tindakan, Runner tim, Properti, Aktivitas, Repo, dll.) memakai `AG.card`/`AG.section`, dengan judul di dalam kartu.
2. **Tidak ada tombol di footer kartu untuk "lihat lainnya".** Contoh: tombol "Kotak masuk" atau "Runner & CLI" di bawah daftar dipindah ke header kartu sebagai `AG.seeAll(href)` dengan label "Lihat semua" (atau "Kelola" untuk halaman pengaturan).
3. **Konten di kanan menu tab vertikal (`AG.subpage`) dibungkus SATU kartu per panel**: judul panel + deskripsi di header kartu, isi di tengah, Batal/Simpan di footer kartu (`AG.card({ title, desc, body, foot: AG.savebar(...) })`). Bagian lain di panel yang sama (mis. zona berbahaya) boleh jadi kartu kedua di bawahnya, tapi savebar tetap di dalam kartu formnya, tidak melayang dan tidak berdiri sendiri.
4. **Halaman form tanpa subpage** juga sama: savebar berada di footer kartu form, bukan baris terpisah atau melayang di bawah halaman.
5. **Tidak ada tombol kembali (panah kiri) di samping judul halaman.** Breadcrumb dan sidebar sudah cukup. Hapus `lead` atau tombol panah semacam itu.
6. **Tidak ada "Disimpan otomatis".** Setiap editor yang mengubah data punya Batal + Simpan (`AG.editor({ save: true })` atau savebar di footer kartu). Hapus teks "Disimpan otomatis".
7. **Alert, info, dan peringatan (`.callout`/banner) diletakkan di ATAS konten** yang terkait (di bawah judul halaman/kartu, sebelum form/daftar), tidak di bawah.
8. Angka di tab (`.tabs`) otomatis jadi badge lewat CSS kalau memakai `<span class="n">`. Pakai pola itu untuk semua jumlah di tab.
9. Jangan menaruh tab tampilan dan tombol aksi di topbar. Kirim lewat `tabs`/`actions` ke `AG.shell`; core memindahkannya ke bar di bawah topbar.
- Tabel data (`AG.datatable`) di dalam `AG.card({ pad: false })` otomatis menempel ke tepi kartu; bar filter mendapat padding sendiri. Tidak perlu CSS modul.
