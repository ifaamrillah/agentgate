// Isian `[data-submit-on-change]` (mis. pilihan role di baris tabel, jumlah per halaman) mengirim
// form induknya saat nilainya berubah. Tanpa JS, tombol submit di form tetap tersedia.
(() => {
  document.addEventListener("change", (e) => {
    const el = e.target.closest("[data-submit-on-change]");
    if (el && el.form) el.form.requestSubmit();
  });
})();
