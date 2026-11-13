// Tinjau usulan knowledge (M8): pintasan papan ketik di halaman `[data-kb-review]`.
// A = approve (submit form `#kb-approve`), R = buka dialog tolak, J/K = usulan berikutnya/
// sebelumnya (tautan `[data-kb-next]` / `[data-kb-prev]`). Diabaikan saat mengetik atau saat
// dialog terbuka.
(() => {
  document.addEventListener("keydown", (e) => {
    const root = document.querySelector("[data-kb-review]");
    if (!root || e.ctrlKey || e.metaKey || e.altKey || e.repeat) return;
    const el = document.activeElement;
    if (el && (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.isContentEditable)) return;
    if (document.querySelector("dialog[open]")) return;
    const k = e.key.toLowerCase();
    if (k === "a") {
      const form = document.getElementById("kb-approve");
      if (form) { e.preventDefault(); form.requestSubmit(); }
    } else if (k === "r") {
      const btn = root.querySelector('[data-dialog-open="kb-reject"]');
      if (btn) { e.preventDefault(); btn.click(); }
    } else if (k === "j" || k === "k") {
      const link = root.querySelector(k === "j" ? "a[data-kb-next]" : "a[data-kb-prev]");
      if (link) { e.preventDefault(); link.click(); }
    }
  });
})();
