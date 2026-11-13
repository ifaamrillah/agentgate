// Tombol `[data-copy]` (komponen copy_value): salin nilainya ke clipboard, lalu ganti label
// sementara menjadi `data-copied`. Tombol tersembunyi sampai skrip ini jalan (tanpa JS, nilai
// tetap bisa dipilih manual). Konten hasil swap HTMX ikut ditangani (htmx.onLoad).
(() => {
  htmx.onLoad((root) => {
    if (!(root instanceof Element) || !navigator.clipboard) return;
    root.querySelectorAll("[data-copy][hidden]").forEach((b) => { b.hidden = false; });
  });
  document.addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-copy]");
    if (!btn) return;
    try {
      await navigator.clipboard.writeText(btn.dataset.copy);
    } catch {
      return;
    }
    const text = btn.querySelector("span");
    const label = btn.getAttribute("aria-label");
    btn.dataset.done = "";
    if (text) {
      const before = text.textContent;
      text.textContent = btn.dataset.copied;
      setTimeout(() => { text.textContent = before; delete btn.dataset.done; }, 1600);
    } else {
      btn.setAttribute("aria-label", btn.dataset.copied);
      setTimeout(() => { btn.setAttribute("aria-label", label); delete btn.dataset.done; }, 1600);
    }
  });
})();
