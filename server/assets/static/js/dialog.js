// Dialog native (<dialog class="dialog">). `[data-dialog-open="<id>"]` membuka dengan showModal(),
// `[data-dialog-close]` menutup, klik di latar menutup. Dialog bertanda `data-open-on-load`
// (mis. form di dialog yang gagal validasi dan dirender ulang dengan 422) langsung dibuka.
(() => {
  document.addEventListener("click", (e) => {
    const opener = e.target.closest("[data-dialog-open]");
    if (opener) {
      const d = document.getElementById(opener.dataset.dialogOpen);
      if (d && typeof d.showModal === "function") {
        e.preventDefault();
        d.showModal();
        d.querySelector("[autofocus], input:not([type=hidden]), select, textarea")?.focus();
      }
      return;
    }
    if (e.target.closest("[data-dialog-close]")) {
      e.target.closest("dialog")?.close();
      return;
    }
    if (e.target instanceof HTMLDialogElement && e.target.matches(".dialog")) e.target.close();
  });
  const scan = (root) => {
    const found = root instanceof Element && root.matches("dialog[data-open-on-load]") ? [root] : root.querySelectorAll?.("dialog[data-open-on-load]") ?? [];
    found.forEach((d) => { if (!d.open) d.showModal(); });
  };
  // htmx bisa sudah memproses <body> sebelum skrip ini terdaftar (htmx:load awal terlewat): pindai
  // dokumen sendiri saat siap, lalu onLoad untuk swap berikutnya. Upgrade idempoten.
  if (window.htmx?.onLoad) htmx.onLoad(scan);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => scan(document));
  else scan(document);
})();
