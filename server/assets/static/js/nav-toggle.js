// Laci sidebar di layar kecil: tombol `[data-nav-toggle]` menukar `data-nav-open` pada
// `[data-shell]`; klik di luar sidebar atau Escape menutupnya. Listener didelegasikan ke document,
// jadi tetap berlaku setelah swap HTMX tanpa inisialisasi ulang.
(() => {
  const shell = () => document.querySelector("[data-shell]");
  const close = () => shell()?.removeAttribute("data-nav-open");
  document.addEventListener("click", (e) => {
    const s = shell();
    if (!s) return;
    if (e.target.closest("[data-nav-toggle]")) {
      s.toggleAttribute("data-nav-open");
      return;
    }
    if (s.hasAttribute("data-nav-open") && !e.target.closest(".app-side")) close();
  });
  document.addEventListener("keydown", (e) => { if (e.key === "Escape") close(); });
  document.addEventListener("htmx:after:swap", close);
})();
