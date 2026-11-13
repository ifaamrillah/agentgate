// Diagram Mermaid di Markdown tersanitasi (komponen `md_view`): `<pre data-mermaid>` berisi kode
// diagram diganti SVG. Mermaid 12.3.0 (±5 MB, chunk relatif) hanya dimuat bila halaman punya
// diagram. securityLevel "strict": label HTML dan klik dimatikan, output disanitasi Mermaid.
// Tema mengikuti class `dark` di <html>. Diagram tidak valid: kode tetap tampil + atribut error.
(() => {
  const URL_MERMAID = "/static/vendor/mermaid-12.3.0/mermaid.esm.min.mjs";
  let ready = null;
  let seq = 0;
  const load = () => {
    ready ??= import(URL_MERMAID).then((m) => m.default);
    return ready;
  };
  window.agMermaid = load;

  async function render(pre) {
    if (pre.dataset.mermaidState) return;
    pre.dataset.mermaidState = "loading";
    try {
      const mermaid = await load();
      mermaid.initialize({
        startOnLoad: false,
        securityLevel: "strict",
        theme: document.documentElement.classList.contains("dark") ? "dark" : "neutral",
        fontFamily: getComputedStyle(document.body).fontFamily,
      });
      const { svg } = await mermaid.render("ag-mmd-" + ++seq, pre.textContent);
      const box = document.createElement("div");
      box.className = "app-mermaid-svg";
      box.setAttribute("role", "img");
      box.innerHTML = svg;
      pre.after(box);
      pre.dataset.mermaidReady = "";
      pre.dataset.mermaidState = "done";
    } catch (err) {
      pre.dataset.mermaidError = "";
      pre.dataset.mermaidState = "error";
      pre.title = String(err?.message || err).slice(0, 200);
    }
  }

  const scan = (root) => {
    const scope = root instanceof Element || root instanceof Document ? root : document;
    scope.querySelectorAll?.("pre[data-mermaid]").forEach(render);
  };
  // htmx bisa sudah memproses <body> sebelum skrip ini terdaftar (htmx:load awal terlewat): pindai
  // dokumen sendiri saat siap, lalu onLoad untuk swap berikutnya. Upgrade idempoten.
  if (window.htmx?.onLoad) htmx.onLoad(scan);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => scan(document));
  else scan(document);
})();
