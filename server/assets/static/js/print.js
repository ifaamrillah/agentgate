// Tombol cetak (`[data-print]`): membuka dialog cetak peramban (simpan sebagai PDF). Inline
// onclick dilarang CSP, jadi lewat event delegasi.
document.addEventListener("click", (e) => {
  if (e.target.closest?.("[data-print]")) { e.preventDefault(); window.print(); }
});
