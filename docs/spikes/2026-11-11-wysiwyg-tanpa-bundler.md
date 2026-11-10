# Spike: editor WYSIWYG Markdown tanpa bundler

Tanggal: 2026-11-11 · Milestone: M0 Fondasi (spike) · Dipakai oleh: M3 (editor artifact), M8 (knowledge base)

Acuan: `docs/spec.md` bagian "Knowledge base → Format & editor" dan "Arsitektur & tech stack";
`docs/reference/loco-project-rules.md` §6 (CSP `script-src 'self'`, JS per perilaku lewat `data-*`, vendor tidak diedit).

Klaim yang tidak dibuktikan langsung di spike ini ditandai **belum terverifikasi**.

## Pertanyaan

1. Editor Markdown mana yang memenuhi kebutuhan spec (WYSIWYG + mode Markdown mentah bolak-balik,
   preview live Mermaid, tabel, code block dengan highlight, checklist, paste/upload gambar, slash
   menu `/`, komentar inline, @mention orang/tiket + link antar dokumen, label UI bahasa Indonesia)
   dan bisa dipakai **tanpa Node dan tanpa bundler** di proyek kita?
2. Bagaimana cara me-vendor-nya (file, URL, versi persis)?
3. Bisakah import map dipakai di bawah CSP `script-src 'self'`? Kalau bisa, bagaimana
   (hash `sha256-...` dihitung saat boot)? Kalau tidak perlu, apa alternatifnya?
4. Bagian mana yang harus kita bangun sendiri?

## Rekomendasi

**Pilihan utama: Milkdown Crepe 7.23.0** (MIT), di-vendor sebagai **satu file ESM** hasil bundle
esm.sh dengan pin `codemirror@6.0.2`, ditambah **Mermaid 12.3.0** dari tarball npm
(`mermaid.esm.min.mjs` + chunk relatif, dimuat lazy).

Alasannya:

- Markdown-native (remark/mdast); round trip yang terbukti di prototipe hanya menormalkan padding tabel.
- Hampir semua fitur spec sudah ada bawaan dan labelnya bisa diganti lewat config: slash menu
  `/` (bisa ditambah item sendiri), tabel dengan UI, code block CodeMirror 6 dengan highlight dan
  hook `renderPreview` (di sinilah Mermaid dipasang), checklist GFM, image block dengan `onUpload`.
- Bundle tidak punya import eksternal → bisa dimuat **dengan atau tanpa** import map.
- Tidak ada `eval`/`new Function` yang berjalan di jalur normal (detail di tabel CSP).
- Sudah dibuktikan jalan di Chromium 158 dan Firefox 159 di bawah CSP ketat (lihat "Hasil prototipe").

**Fallback: Tiptap 3.33.1 + `@tiptap/markdown` 3.33.1** (MIT). Ekosistemnya paling besar dan ada
extension Mention/Suggestion resmi, tetapi headless (semua UI dibangun sendiri) dan butuh
**satu bundle custom** supaya ProseMirror tidak terduplikasi. Bundle itu tidak bisa diambil dari
CDN begitu saja (lihat Risiko) → praktis butuh langkah build sekali jalan di luar repo. **Belum
terverifikasi** di spike ini.

Untuk mode Markdown mentah cukup `<textarea>` biasa (sudah diuji). Kalau nanti butuh textarea yang
lebih nyaman, **OverType 2.7.0** (MIT, 1 file ESM 272 KB, tanpa dependensi bare) layak dipertimbangkan.

## Tabel perbandingan

Versi dan tanggal diambil dari registry npm pada 2026-11-11. "Self-contained" artinya ada file ESM/UMD
yang bisa dimuat browser tanpa resolusi bare specifier.

| Kandidat | Lisensi | Versi terbaru (tgl) | Terawat? | Build tanpa bundler | Ukuran / file | Bare import | CSP (eval / Function / style) | Cakupan fitur vs spec |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Milkdown Crepe** | MIT | 7.23.0 (2026-10-27) | Ya, rilis rutin | npm hanya `lib/esm` dengan bare import (vue, codemirror, @milkdown/kit, …). **esm.sh `?bundle&deps=codemirror@6.0.2`** → 1 file tanpa import eksternal ✅ (diuji) | 2,8 MB mentah / 933 KB gzip, 1 file JS + 2 CSS (72 KB + frame) | Tidak ada (bundle) | Tanpa `eval`/`new Function`. `Function("return this")` lodash hanya fallback (tidak jalan di browser). `Function("","var x …")` di mode legacy CodeMirror (pug), di dalam `try` → hanya untuk bahasa itu. Menyisipkan `<style>`/`adoptedStyleSheets` (CodeMirror) → butuh `style-src 'unsafe-inline'` (sudah ada) | WYSIWYG ✅, tabel ✅, highlight ✅ (CodeMirror 6), checklist ✅, upload gambar ✅, slash ✅ + item custom, Mermaid via `renderPreview` ✅, label i18n ✅. Raw mode, mention, komentar: dibangun sendiri (sudah diprototipe) |
| Tiptap + `@tiptap/markdown` | MIT (extension Comments/Collab = berbayar) | core 3.33.1 (2026-11-10) | Ya, sangat aktif | npm `dist/index.js` dengan bare import `@tiptap/pm/*`. Butuh 1 bundle custom (beberapa paket + ProseMirror bersama). jsDelivr `+esm`/esm.sh per paket menduplikasi ProseMirror | core 3,3 MB unpacked (354 file); bundle akhir **belum terverifikasi** | Banyak | **Belum terverifikasi** (perkiraan bersih; ProseMirror tidak memakai eval) | Headless: semua UI (toolbar, slash, tabel, mention popup) dibangun sendiri; Mention + Suggestion resmi; Markdown parser `marked`, round trip **belum terverifikasi** |
| Toast UI Editor | MIT | 3.2.2 (2023-02-17) | **Tidak** (tanpa rilis ~3 tahun) | `dist/toastui-editor.js` UMD, tapi `require` prosemirror-* eksternal; versi "all" hanya di CDN NHN (**belum terverifikasi**) | 940 KB (UMD tanpa PM) | prosemirror-* | 1× `new Function` di dist (**konteks belum terverifikasi**) | WYSIWYG + tab Markdown bawaan, tabel, plugin syntax highlight, i18n (tanpa bahasa Indonesia, bisa custom); tanpa slash menu; Mermaid via plugin sendiri |
| ProseMirror langsung (`prosemirror-markdown`) | MIT | 1.13.9 (2026-10-22) | Ya | Paket ESM kecil-kecil, semuanya bare import → butuh import map ~15 entri atau bundle sendiri | ~165 KB (markdown) + PM | Banyak | Bersih (ProseMirror tidak memakai eval) | Hanya fondasi: semua fitur & UI dibangun sendiri |
| BlockNote | MPL-2.0 (core); sebagian paket XL berlisensi lain | 0.57.0 (2026-10-30) | Ya | UI butuh **React** (`@blocknote/react`, Mantine/shadcn) | core 4,9 MB, 383 file | Banyak | Tidak dicek | Model blok; ekspor Markdown resmi "lossy" → tidak cocok untuk Markdown sebagai sumber kebenaran |
| Lexical (+ `@lexical/markdown`) | MIT | 0.53.0 (2026-10-28) | Ya | ESM per paket, bare import; UI praktis React | 3,7 MB unpacked | Banyak | Tidak dicek | Headless; Markdown lewat transformer (round trip terbatas); semua UI sendiri |
| MDXEditor | MIT | 4.5.0 (2026-11-02) | Ya | Butuh React | — | Banyak | Tidak dicek | Lengkap tapi React-only → ditolak |
| OverType | MIT | 2.7.0 (2026-10-28) | Ya | `dist/overtype.esm.js` 1 file, tanpa bare import ✅ | 272 KB | Tidak ada | Tanpa eval; menyisipkan 1 `<style>` | **Bukan WYSIWYG** (textarea + overlay). Cocok sebagai mode Markdown mentah |
| EasyMDE | MIT | 2.21.0 (2026-05-03) | Pemeliharaan minimal | `dist/easymde.min.js` UMD 1 file ✅ | 320 KB | Tidak ada | Tanpa eval | Bukan WYSIWYG (CodeMirror 5 + preview); ditolak |
| Vditor | MIT | 4.0.1 (2026-10-18) | Ya | `dist/index.min.js` UMD; memuat Lute (Go→JS, 3,6 MB), i18n, Mermaid, dll. **secara dinamis** lewat `<script>` dari opsi `cdn` (default unpkg) → harus self-host seluruh `dist/` (653 file, 23 MB) | 292 KB + aset dinamis | Tidak ada (UMD) | Tanpa eval di index/Lute. Injeksi `<script src>` dinamis boleh selama same-origin | WYSIWYG/IR/SV, Mermaid bawaan, tabel, highlight, upload; tanpa bahasa Indonesia (i18n custom **belum terverifikasi**); tanpa slash menu & mention sesuai kebutuhan; DOM hasil Lute sulit di-extend |
| Cherry Markdown | Apache-2.0 | 0.11.13 (2026-11-10) | Ya | `dist/cherry-markdown.esm.js` 1 file ✅ | 5,2 MB | Tidak ada | `new Function` (d3-dsv, hanya saat parse CSV) | Editor split + preview, bukan WYSIWYG sejati; Mermaid bawaan |
| ProseKit | MIT | 0.23.1 (2026-11-08) | Ya, pra-1.0 | ESM per paket + Web Components (lit); bare import | — | Banyak | Tidak dicek | Toolkit ProseMirror headless; semua UI sendiri; **belum terverifikasi** |

Kandidat lain yang dicek di registry tetapi tidak relevan: `novel` (React, terakhir 2025-01), `bytemd`
(terakhir 2025-02), `ink-mde` (terakhir 2024-09), `platejs` (React).

## Resep vendoring (Crepe + Mermaid)

Semua file ditaruh di `assets/static/vendor/` dan **tidak pernah diedit** (sesuai §6). Langkah
vendoring dijalankan manual saat upgrade versi dan hasilnya di-commit beserta sha256-nya.

### 1. Crepe (JS, 1 file)

```
# Resolver esm.sh (mengembalikan stub yang menunjuk path build final):
https://esm.sh/@milkdown/crepe@7.23.0?bundle&deps=codemirror@6.0.2&target=es2022
# File final yang diunduh (path dari stub di atas):
https://esm.sh/@milkdown/crepe@7.23.0/X-ZGNvZGVtaXJyb3JANi4wLjI/es2022/crepe.bundle.mjs
→ assets/static/vendor/milkdown-crepe-7.23.0/crepe.bundle.mjs
  sha256 b590492b3c8fb6084900f0c6dc8c71582e2429ce054acaeeab7a6a74fce68fec (2.808.455 byte)
```

- `deps=codemirror@6.0.2` **wajib**. Tanpa pin itu esm.sh memilih `codemirror` **5** (UMD lama)
  untuk import `codemirror`. Akibatnya `basicSetup` undefined dan setiap code block crash
  (`Cannot read properties of undefined (reading 'extension')`). Ini terjadi nyata di spike.
  Bundle tanpa pin juga mengimpor `/node/process.mjs`, sedangkan bundle dengan pin tidak.
- Setelah unduh, cek bahwa file **tidak punya import ke path absolut**
  (`grep -oE '(from|import)\s*"/' crepe.bundle.mjs` harus kosong). Cek juga tidak ada
  `CodeMirror=e()` (tanda CM5).
- Nama `.mjs` disajikan sebagai `text/javascript` oleh server spike. MIME `.mjs` di static handler
  Loco/`tower-http` **belum terverifikasi**. Kalau ragu, simpan sebagai `crepe.bundle.js`
  (ekstensi tidak berpengaruh untuk `type="module"`).
- Lisensi pihak ketiga ikut di komentar akhir bundle (Vue, dll.). Tambahkan `LICENSE` Crepe dari tarball.

### 2. Crepe (CSS)

Sumber: tarball npm (pakai `curl`, tanpa npm):

```
https://registry.npmjs.org/@milkdown/crepe/-/crepe-7.23.0.tgz                (sha256 bc665df1…c39804)
https://registry.npmjs.org/@milkdown/prose/-/prose-7.23.0.tgz                (sha256 acb2dad1…527809)
https://registry.npmjs.org/prosemirror-virtual-cursor/-/prosemirror-virtual-cursor-0.4.2.tgz (sha256 a7ef7968…07ea16a)
```

`lib/theme/common/style.css` berisi rantai `@import` dengan bare specifier
(`@milkdown/kit/prose/...`, `prosemirror-virtual-cursor/...`, `katex/...`). Ada dua cara:

- **Disarankan:** salin file mentahnya ke `assets/css/vendor/milkdown-crepe-7.23.0/` lalu biarkan
  **Tailwind v4 standalone CLI** menggabungkan `@import` saat build `app.css`. Resolusi bare
  specifier `@milkdown/...` oleh Tailwind CLI tanpa `node_modules` **belum terverifikasi**. Kalau
  tidak bisa, ganti dengan cara kedua.
- **Dipakai di spike:** skrip kecil (Python) meratakan `@import` menjadi satu file
  `crepe-common.css` (72 KB, 22 file digabung), plus `lib/theme/frame/style.css` → `crepe-frame.css`.
  `katex.min.css` di-skip karena fitur Latex dimatikan.

Override tema (warna, font) masuk ke `assets/css/components/md-editor.css` lewat variabel
`--crepe-color-*`, bukan dengan mengedit file vendor.

### 3. Mermaid

```
https://registry.npmjs.org/mermaid/-/mermaid-12.3.0.tgz   (sha256 be0684e8d20bfc874df12c0f04927a5c482f57f2200eec54cca52a4583fd4040)
  package/dist/mermaid.esm.min.mjs                  → assets/static/vendor/mermaid-12.3.0/mermaid.esm.min.mjs
  package/dist/chunks/mermaid.esm.min/*.mjs (105)   → assets/static/vendor/mermaid-12.3.0/chunks/mermaid.esm.min/
  (jangan salin *.map; total ±5,4 MB, tetapi browser hanya mengambil chunk untuk diagram yang dipakai)
```

Semua import Mermaid relatif (`./chunks/...`), tanpa `eval`/`new Function`. Satu-satunya
`Function("return this")` ada di fallback lodash yang tidak jalan di browser.

### Catatan sha256

Hash file final yang dipakai prototipe: `crepe-common.css` 1b21b075…8e277e,
`crepe-frame.css` c6009ffe…ba0631. Simpan semua hash di `assets/static/vendor/VERSIONS.md` (atau file manifest sejenis) supaya upgrade bisa diaudit.

## Import map dan CSP

### Fakta spesifikasi

- Import map inline (`<script type="importmap">`) **tunduk pada `script-src`**. Di algoritma HTML
  "prepare the script element", tipe script ditentukan dulu (langkah 10–15). Untuk elemen tanpa
  `src`, langkah 23 lalu memanggil *"Should element's inline behavior be blocked by Content
  Security Policy?"* dengan `cspType` "script". Import map tidak dikecualikan
  ([HTML Standard §4.12.1](https://html.spec.whatwg.org/multipage/scripting.html#prepare-the-script-element)).
  Jadi hash-source `'sha256-…'` atau nonce berlaku untuknya, sama seperti inline script biasa.
- Import map **tidak bisa eksternal**: langkah 35.1 spesifikasi menyebut *"External import maps and
  speculation rules are not currently supported"*. MDN juga menyatakan atribut `src` tidak boleh
  dipakai ([MDN `<script type="importmap">`](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/script/type/importmap)).
  MDN tidak membahas CSP untuk import map; yang mengikat adalah spesifikasi HTML di atas.
- Dukungan import map Baseline "widely available" sejak Maret 2023 (MDN). Beberapa import map
  dalam satu dokumen digabung, tetapi versi browser lama hanya menerima satu import map sebelum
  module pertama dimuat. Jadi pakai **satu** import map di `<head>` layout.
- Bukti di spike: hash-source untuk import map **jalan di Chromium 158 dan Firefox 159**.
  Dengan hash yang salah, kedua browser memblokir import map. Chromium melaporkan
  `Executing inline script violates … script-src` dan `Failed to resolve module specifier
  "@milkdown/crepe"`; Firefox melaporkan `blocked an inline script (script-src-elem)`.
  WebKit/Safari **belum terverifikasi** (dependensi sistem WebKit Playwright tidak tersedia di mesin
  spike). Menurut spesifikasi seharusnya sama.

### Pendekatan A: import map + hash dihitung saat boot

1. Simpan import map sebagai file di repo, misal `assets/importmap.json`.
2. Saat boot (initializer Loco, `LazyLock`), baca file, **serialisasi ke string kanonik sekali**
   (misal JSON minified), lalu hitung `sha256` → base64 → `'sha256-<b64>'`.
3. Masukkan string yang **persis sama** ke layout Tera sebagai variabel global, dicetak dengan
   `| safe`. Ini konten statis milik kita, bukan input user, jadi tidak melanggar aturan `| safe`.
   Satu byte beda (spasi, newline dari template) membuat hash tidak cocok, jadi jangan menulis
   import map langsung di template.
4. Header CSP: `script-src 'self' 'sha256-<b64>'`. Middleware `secure_headers` Loco membaca CSP
   statis dari YAML, jadi:
   - (a) CSP di-set lewat layer Axum sendiri (`SetResponseHeaderLayer` di `after_routes`) yang
     memakai nilai hasil boot, dan key CSP dihapus dari override `secure_headers`; atau
   - (b) pakai placeholder di config yang diganti saat boot.

   Kedua opsi ini **belum terverifikasi** di Loco 1.3. Opsi (a) lebih sederhana.
5. Tes request: ambil halaman, ekstrak isi `<script type="importmap">`, hitung sha256-nya, lalu
   pastikan nilainya ada di header CSP. Tes ini melengkapi tes CSP yang sudah diwajibkan §6.

Server spike (`scripts/server.py`) menerapkan langkah 1–4 dengan cara ini. Isi import map final:

```json
{"imports":{"@milkdown/crepe":"/static/vendor/milkdown-crepe-7.23.0/crepe.bundle.mjs","mermaid":"/static/vendor/mermaid-12.3.0/mermaid.esm.min.mjs"}}
```

Bersama HTMX `hx-boost`: import map hanya diproses saat dokumen pertama dimuat. Karena itu import
map harus identik di semua halaman (satu hash global). Module `md-editor.js` hanya dieksekusi sekali
per dokumen, jadi inisialisasi harus dipicu ulang saat HTMX menukar konten (event `htmx:load` atau
`MutationObserver` untuk `[data-md-editor]`). Perilaku detail HTMX 4 di sini **belum terverifikasi**.

### Pendekatan B (tanpa import map): impor lewat URL relatif ke file vendor

Bundle Crepe tidak punya import eksternal dan Mermaid hanya punya import relatif, sehingga
**import map sama sekali tidak diperlukan**:

```js
// assets/static/js/vendor/editor-deps.js: satu-satunya tempat versi ditulis
export { Crepe } from "/static/vendor/milkdown-crepe-7.23.0/crepe.bundle.mjs";
export const loadMermaid = () => import("/static/vendor/mermaid-12.3.0/mermaid.esm.min.mjs");
```

Varian ini (`/nomap`) **diuji** di Chromium dan Firefox dengan CSP `script-src 'self'` tanpa hash
apa pun: editor jalan dan Mermaid ter-render, tanpa error.

**Saran:** mulai dengan **Pendekatan B** untuk editor, karena tanpa perubahan CSP dan tanpa kode
boot. Import map + hash (A) baru dipakai bila ada library lain yang memang butuh bare specifier
bersama (misal Sigma/Graphology, PixiJS). Mekanismenya sudah terbukti jalan.

## Hasil prototipe

Lokasi: `/tmp/claude-1000/-home-sevima-189-SEVIMA-Private-agentgate/8fe87409-86a3-4f11-afc8-f162de74b640/scratchpad/wysiwyg-spike/`
(`site/` berisi halaman + vendor, `scripts/server.py` server mirip Loco, `harness/test.mjs` skenario
Playwright). Server menyajikan header CSP sungguhan:
`default-src 'self'; script-src 'self' 'sha256-…'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'; …`.
Semua konfigurasi JS lewat atribut `data-*` dan `<script type="application/json">`. Tidak ada inline script.

| Skenario | Chromium 158 | Firefox 159 |
| --- | --- | --- |
| Hash import map salah → diblokir CSP | ✅ diblokir | ✅ diblokir |
| Hash benar → editor siap | ✅ ~320 ms (lokal) | ✅ ~420 ms |
| Tanpa import map (URL langsung, tanpa hash) | ✅ | ✅ |
| Pelanggaran CSP / error JS selama skenario | 0 | 0 |
| Mermaid live preview (SVG + label) via `renderPreview` | ✅ | ✅ |
| Code block Rust ter-highlight (CodeMirror 6) | ✅ | ✅ |
| Tabel tampil + `/Tabel` dari slash menu menyisipkan tabel 3×3 | ✅ | ✅ |
| Slash menu dengan label Indonesia + grup custom "Agent Gate" (Diagram Mermaid, Sebut orang) | ✅ | ✅ |
| Upload gambar (`onUpload` → `POST /upload` → URL) | ✅ | ✅ |
| `@bu` → popup → Enter menghasilkan `[@Budi Santoso](/u/budi)`; `#AG-3` → `[#AG-31 Gate engine](/t/AG-31)` | ✅ | ✅ |
| Komentar inline: anchor text-quote + CSS Custom Highlight API, tetap menempel setelah teks disisipkan sebelumnya | ✅ | ✅ |
| Toggle WYSIWYG → textarea Markdown → edit → WYSIWYG: suntingan bertahan | ✅ | ✅ |
| Round trip kedua stabil (md → editor → md identik) | ✅ | ✅ |

Round trip dokumen contoh (heading, bold/italic, inline code, link, Mermaid, tabel, checklist,
code block, quote, ordered list, hr) **identik kecuali padding tabel**. Remark menyejajarkan kolom
(`| Komponen | Teknologi   |`). Normalisasi ini stabil dan idempoten. Opsi `tablePipeAlign: false`
dari mdast-util-gfm-table mungkin bisa mematikannya, tetapi **belum terverifikasi** karena opsi gfm
tidak terekspos di bundle.

Temuan lain:

- Default serializer menulis bullet `*` dan rule `***`. Ini diatasi dengan
  `crepe.editor.config(ctx => ctx.update("remarkStringifyOptions", p => ({...p, bullet: "-", rule: "-"})))`.
  Slice Milkdown bisa diakses **lewat nama string**, jadi kita tidak butuh salinan kedua `@milkdown/kit`.
- Paragraf kosong diserialisasi sebagai `<br />`. Ini perlu dinormalkan sebelum disimpan, atau diterima saja.
- Image block Crepe menulis rasio ke alt text (`![1.00](url)`). Kalau kita butuh alt text yang
  bermakna (aksesibilitas), atur ulang lewat config/serializer. Caranya **belum terverifikasi**.
- Mention dan komentar dibangun **tanpa mengimpor kelas ProseMirror**. Semua lewat instance
  `EditorView` dari `ctx.get("editorView")` (`state.tr`, `schema.marks.link.create`, `coordsAtPos`,
  `domAtPos`). Ini penting karena bundle Crepe tidak mengekspor `@milkdown/kit`.

## Yang harus kita bangun sendiri

1. **Mode Markdown mentah.** Toggle `getMarkdown()` → `destroy()` → `<textarea>`, lalu kembali
   lewat `new Crepe({ defaultValue })`. Sudah ada di prototipe (±20 baris). Opsional: OverType untuk textarea yang lebih nyaman.
2. **@mention orang, #tiket, `[[` dokumen.** Deteksi trigger sebelum kursor, popup autocomplete
   dari endpoint kita (`GET /api/mentions?t=@&q=`), lalu sisipkan link Markdown biasa
   (`[@Nama](/u/id)`, `[#AG-12 Judul](/t/AG-12)`, `[Judul Dok](/k/slug)`). Server mengekstrak
   mention dari link dengan pola path tersebut untuk notifikasi dan relasi. Prototipe sudah ada;
   yang masih perlu: aksesibilitas (`aria-activedescendant`), posisi popup saat scroll, debounce.
3. **Komentar inline.** Lapisan terpisah dari Markdown. Anchor = selektor text-quote ala W3C Web
   Annotation (`exact` + `prefix`/`suffix` 32 karakter) disimpan di tabel komentar. Highlight
   memakai CSS Custom Highlight API, jadi tidak mengubah DOM ProseMirror. Komentar yang anchor-nya
   hilang ditandai "yatim". Masih perlu: UI thread, resolve, re-anchor setelah AI mengusulkan diff,
   dan fallback untuk browser tanpa `CSS.highlights` (Firefox 140+ dan Safari 17.2+ sudah
   mendukung; **belum terverifikasi** di Safari).
4. **Endpoint upload** (`POST`, simpan ke S3 lewat `ctx.storage`, kembalikan URL), validasi tipe/ukuran, CSRF.
5. **Pemetaan label i18n.** Semua label Crepe yang terlihat sudah diganti di prototipe. Masih
   perlu diaudit: tooltip, link tooltip (`editButton`, `removeButton`, `inputPlaceholder`), dan
   aria-label ikon.
6. **Integrasi HTMX.** Inisialisasi ulang editor pada swap, simpan lewat form/HTMX (`markdown` di field tersembunyi), peringatan perubahan belum disimpan.

## Risiko

- **Bundle CDN bisa salah resolusi diam-diam.** Contohnya CodeMirror 5 vs 6 di atas. Mitigasi: pin
  `deps`, cek sha256, cek tidak ada import absolut, dan jalankan tes Playwright (atau tes smoke
  manual) setiap kali upgrade.
- **Reproducibility esm.sh.** URL build esm.sh bisa menghasilkan byte berbeda bila esm.sh meng-upgrade
  build server-nya. Karena file di-commit dan hash dicatat, ini hanya berpengaruh saat upgrade.
- **Ukuran.** 933 KB gzip untuk editor, karena semua grammar `@codemirror/language-data` ikut
  di-inline (dynamic import ter-bundle) dan Vue runtime ikut serta. Dimuat hanya di halaman editor
  dan bisa di-cache `immutable`. Halaman baca (view) cukup render HTML dari server + Mermaid lazy.
- **Custom plugin ProseMirror terbatas.** Bundle tidak mengekspor `Plugin`/`DecorationSet`, sehingga
  fitur yang butuh decoration atau plugin PM sungguhan (misal komentar berbasis decoration) tidak
  bisa dibuat tanpa bundle custom (Crepe + `@milkdown/kit` dalam satu entry). Pendekatan instance +
  CSS Highlight menutup kebutuhan spike. Kalau suatu saat tidak cukup, solusinya satu langkah build
  sekali jalan (esbuild di container sementara) yang hasilnya di-vendor. Ini tetap sesuai prinsip
  "tanpa bundler di proyek".
- **CSP style.** CodeMirror dan Vue menyisipkan style. Kalau suatu hari `style-src 'unsafe-inline'`
  dicabut, editor rusak.
- **Mermaid** memakai `securityLevel: "strict"` dan output-nya disanitasi DOMPurify oleh Crepe
  (dengan aturan khusus `foreignObject`). Diagram buatan AI tetap dianggap input tidak tepercaya.
- **Bus factor Milkdown** lebih kecil daripada Tiptap. Fallback ke Tiptap tetap memakai format
  Markdown yang sama, jadi data tidak terkunci.
- **Safari/WebKit belum diuji.**

## Dampak ke milestone

**M0.** Hasil spike ini adalah keluaran yang diminta. Tambahan yang disarankan:
`assets/static/vendor/VERSIONS.md` (versi + URL + sha256), dan tes CSP yang juga memeriksa hash
import map bila Pendekatan A dipakai.

**M3 (artifact + versi + editor Markdown).**

- Komponen Tera `components/md_editor.html` (`data-md-editor`, `data-upload-url`,
  `data-mention-url`, isi awal di `<script type="application/json">`) dan
  `assets/static/js/md-editor.js` mengikuti pola prototipe.
- Artifact disimpan sebagai Markdown mentah. Versi = snapshot Markdown, sehingga diff antar-versi
  bisa berbasis teks.
- Mention di M3 cukup orang dan tiket (endpoint dari M2). Komentar inline pada artifact memakai
  lapisan anchor di atas.
- Normalisasi serializer (bullet `-`, rule `---`, `<br />`) diputuskan di sini supaya diff versi tidak berisik.

**M8 (knowledge base).**

- Editor yang sama dipakai ulang. Tambahan: trigger `[[` untuk link antar dokumen (endpoint
  pencarian dokumen), dan Mermaid sebagai diagram utama.
- Usulan AI (diff) di-review di **tampilan diff teks Markdown**, bukan di dalam editor. Crepe 7.23
  punya plugin `diff`/`streaming` untuk review perubahan AI di dalam editor; kecocokannya dengan
  alur approve knowledge owner **belum terverifikasi**.
- Sinkron ke `.agentgate/knowledge/` aman karena round trip hanya menormalkan format (padding
  tabel, gaya bullet). Jalankan normalisasi yang sama di sisi server/CLI sebelum commit supaya
  tidak terjadi "diff palsu". Normalizer Rust yang setara dengan remark-stringify **belum
  terverifikasi**. Alternatifnya: simpan selalu hasil `getMarkdown()` dan biarkan AI menulis dengan gaya yang sama.

## Ringkasan

1. Pilihan utama: Milkdown Crepe 7.23.0 (MIT). Fallback: Tiptap 3.33.1 + `@tiptap/markdown` (butuh bundle custom, belum terverifikasi).
2. Crepe di-vendor sebagai 1 file ESM hasil esm.sh `?bundle&deps=codemirror@6.0.2` (2,8 MB / 933 KB gzip, sha256 `b590492b…`), tanpa import eksternal.
3. Pin `codemirror@6.0.2` wajib. Tanpa pin, esm.sh memasukkan CodeMirror 5 dan code block crash.
4. Mermaid 12.3.0 dari tarball npm (`mermaid.esm.min.mjs` + 105 chunk relatif), lazy, dipasang lewat `renderPreview`.
5. Import map tunduk pada `script-src`. Hash `sha256` dihitung saat boot terbukti jalan di Chromium 158 dan Firefox 159; Safari belum terverifikasi.
6. Import map tidak wajib: impor URL langsung jalan dengan `script-src 'self'` murni, jadi itu yang disarankan untuk editor.
7. Prototipe lolos: Mermaid, tabel, highlight, checklist, upload, slash Indonesia, @mention/#tiket, komentar (CSS Highlight), toggle raw, tanpa error/pelanggaran CSP.
8. Yang dibangun sendiri: mode raw, mention/link dokumen, lapisan komentar, endpoint upload, integrasi HTMX. Risiko utama: resolusi bundle CDN, ukuran, Safari.
