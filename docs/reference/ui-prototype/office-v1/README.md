# AI Office V1 (acuan R5, disetujui 2026-11-14)

Prototype variasi visual AI Office yang dipilih pemilik produk (lihat `docs/REVIEW-AKHIR.md` §3.0):

- tampilan utama **V1 Isometrik Studio** (`js/v1-iso.js`);
- mode **Denah** dari V2 (`js/v2-plan.js`);
- **pencahayaan hangat** dari V3 (`js/v3-diorama.js`: cahaya lampu gantung, `screen` di mode gelap);
- SVG + vanilla JS (tanpa PixiJS), warna adegan lewat token CSS (`css/scenes.css`).

Buka `index.html` langsung di browser (`#v1`, `#v2`, `#v3`; `?theme=dark`). Screenshot di `shots/`.
Font memakai salinan yang sudah di-vendor di `server/assets/static/vendor/fonts/`.

Implementasi asli: `server/assets/static/js/office-*.mjs` dan `server/assets/css/components/office.css`.
