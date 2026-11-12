// Tabel data (komponen `datatable`): pilih baris lewat checkbox `[data-dt-row]`, "pilih semua"
// `[data-dt-all]`, dan bar aksi massal `[data-dt-bulkbar]` yang menampilkan jumlah terpilih
// (`[data-dt-selected]`). Filter, sort, dan halaman adalah tautan/isian GET (HTMX), bukan JS.
(() => {
  const sync = (dt) => {
    const rows = [...dt.querySelectorAll("[data-dt-row]")];
    const picked = rows.filter((r) => r.checked);
    const bar = dt.querySelector("[data-dt-bulkbar]");
    const all = dt.querySelector("[data-dt-all]");
    if (all) { all.checked = rows.length > 0 && picked.length === rows.length; all.indeterminate = picked.length > 0 && picked.length < rows.length; }
    rows.forEach((r) => r.closest("tr")?.setAttribute("aria-selected", r.checked));
    if (bar) {
      bar.toggleAttribute("data-open", picked.length > 0);
      const n = bar.querySelector("[data-dt-selected]");
      if (n) n.textContent = picked.length;
    }
  };
  document.addEventListener("change", (e) => {
    const dt = e.target.closest("[data-dt]");
    if (!dt) return;
    if (e.target.matches("[data-dt-all]")) dt.querySelectorAll("[data-dt-row]").forEach((r) => (r.checked = e.target.checked));
    if (e.target.matches("[data-dt-all], [data-dt-row]")) sync(dt);
  });
  document.addEventListener("click", (e) => {
    const clear = e.target.closest("[data-dt-clear]");
    if (!clear) return;
    const dt = clear.closest("[data-dt]");
    dt.querySelectorAll("[data-dt-row]").forEach((r) => (r.checked = false));
    sync(dt);
  });
})();
