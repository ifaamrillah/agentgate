// Tema terang/gelap. Dimuat tanpa `defer` di <head> supaya class `dark` terpasang sebelum halaman
// dilukis (tanpa kedipan). Pilihan disimpan per browser di localStorage (`ag-theme`); tanpa pilihan
// mengikuti preferensi sistem. Tombol `[data-theme-toggle]` (menu profil) menukar tema.
(() => {
  const root = document.documentElement;
  const read = () => {
    try { return localStorage.getItem("ag-theme"); } catch { return null; }
  };
  const apply = (theme) => {
    const dark = theme ? theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
    root.classList.toggle("dark", dark);
  };
  apply(read());
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => apply(read()));
  document.addEventListener("click", (e) => {
    if (!e.target.closest("[data-theme-toggle]")) return;
    const next = root.classList.contains("dark") ? "light" : "dark";
    try { localStorage.setItem("ag-theme", next); } catch { /* tetap berlaku untuk halaman ini */ }
    apply(next);
  });
})();
