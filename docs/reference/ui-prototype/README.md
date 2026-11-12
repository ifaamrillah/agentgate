# Prototype UI Agent Gate (disetujui 2026-11-12)

Prototype klik untuk semua layar M1–M12. Disetujui pemilik produk sebagai acuan UI dan UX untuk kode asli.
Versi publik: https://claude.ai/artifact/GEWs27yzY5pqm9nRzhqHnZ (privat milik pemilik produk).

Untuk membukanya secara lokal, buka `index.html` di browser. Navigasi lewat tombol "Peta layar" atau
tombol `[` dan `]`. Layar dibagi per milestone di `screens-*.js`. Komponen dan token ada di `core.js` dan
`style.css`.

## Varian yang dipilih

| Bagian | Pilihan | Catatan |
|---|---|---|
| Kerangka (shell) | A · Panel melayang | Sidebar netral hangat `#f7f7f5`, area kerja putih membulat dengan border dan bayangan tipis |
| Panel kiri halaman masuk | B · Alur 4 langkah | Latar gelap, tiket → agent → gate → approve |
| Kartu angka | A · Ringkas | |
| Menu tab vertikal | E · Satu panel menyatu | Menu + isi dalam satu kartu; menu berlatar abu dengan item rapat (tidak meregang) |
| Kartu bagian | A · Header bergaris | Judul + deskripsi + aksi di dalam kartu, dipisah garis tipis |
| Tabel data | A · Filter di menu | Tombol Filter + chip filter aktif; sort, pagination dengan per halaman, total, nomor halaman |
| Kolom board | C · Garis putus | |
| Ikon prioritas | C · Label P1–P4 | |
| Knowledge | B · Beranda + kartu kelompok | Tinjau usulan mode fokus berdampingan |
| AI Office | A1 · Isometrik vektor lembut | Kanvas geser/zoom + mini-map |

## Aturan UX yang ditetapkan pemilik produk

1. Halaman padat dipecah menjadi beberapa halaman atau sub-halaman. Banyak halaman tidak masalah, asal UX-nya bagus.
2. Judul dan deskripsi bagian berada **di dalam** kartu. Aksi "lihat lainnya" berupa tautan "Lihat semua →" di header kartu, bukan tombol di footer.
3. Batal/Simpan ada di footer kartu formnya: tidak melayang, tanpa bayangan, bukan kartu sendiri.
4. Konten di kanan menu tab vertikal adalah satu kartu, dari judul sampai Batal/Simpan.
5. Tidak ada tombol panah kembali di samping judul halaman.
6. Tidak ada simpan otomatis: setiap editor yang mengubah data punya Batal + Simpan.
7. Alert dan info diletakkan di atas konten terkait.
8. Angka di tab berbentuk badge.
9. Topbar hanya berisi breadcrumb. Tab tampilan project dan tombol aksi ada di bar di bawahnya. Tab tampilan project bisa diatur (pin/urut) dan sisanya masuk menu "Lainnya".
10. Semua tab memakai gaya garis bawah. `.seg` hanya untuk sakelar mode kecil.
11. Judul, tab, dan tombol tidak boleh sejajar dalam satu baris.
12. Semua pilihan panjang (orang, tim, tiket, model) memakai combobox yang bisa dicari.
13. Editor teks panjang memakai editor lengkap (toolbar, Markdown, tabel, Mermaid, mention, emoji, layar penuh). Dokumen panjang punya halaman editor sendiri, bukan modal.
14. Warna (project, persona, label) memakai palet 18 warna + warna custom.
15. Sidebar tidak memiliki item "Pengaturan" di bawah; pengaturan dibuka dari menu workspace.
16. Item menu sidebar 34px / 14px; sidebar tidak boleh menyusutkan item (`flex-shrink: 0`).
17. Checkbox tercentang memakai centang putih tebal.
