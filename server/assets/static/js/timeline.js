// Roadmap (`[data-tl-scroll]`): gulir trek ke posisi "hari ini" saat halaman dibuka dan saat
// tombol `[data-tl-today]` ditekan. Nilai atribut = posisi x hari ini di trek (px).
(() => {
  const go = (smooth) => {
    const tl = document.querySelector("[data-tl-scroll]");
    if (!tl) return;
    const x = Number(tl.dataset.tlScroll) || 0;
    const lead = window.innerWidth < 900 ? 60 : 220;
    tl.scrollTo({ left: Math.max(0, x - lead), behavior: smooth ? "smooth" : "auto" });
  };
  document.addEventListener("DOMContentLoaded", () => go(false));
  document.addEventListener("click", (e) => {
    if (e.target.closest?.("[data-tl-today]")) { e.preventDefault(); go(true); }
  });
})();
