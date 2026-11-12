// Dialog "Atur tab tampilan" (komponen `project_tabs`): tombol naik/turun memindah baris
// `[data-view-row]`; urutan baris ikut terkirim karena setiap baris membawa input hidden `order`.
(() => {
  document.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-view-move]");
    if (!btn) return;
    e.preventDefault();
    const row = btn.closest("[data-view-row]");
    const up = btn.dataset.viewMove === "up";
    const sib = up ? row.previousElementSibling : row.nextElementSibling;
    if (sib?.matches("[data-view-row]")) row.parentElement.insertBefore(row, up ? sib : sib.nextSibling);
    btn.focus();
  });
})();
