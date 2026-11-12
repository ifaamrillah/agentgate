// Unggah gambar (komponen `uploader`): tombol `[data-uploader-pick]` membuka pemilih berkas, area
// `[data-uploader]` menerima berkas yang diseret, lalu form-nya langsung dikirim (multipart POST).
// Validasi tipe dan ukuran tetap di server.
(() => {
  const submit = (root) => root.closest("form")?.requestSubmit();
  document.addEventListener("click", (e) => {
    const pick = e.target.closest("[data-uploader-pick]");
    if (pick) { e.preventDefault(); pick.closest("[data-uploader]")?.querySelector('input[type="file"]')?.click(); }
  });
  document.addEventListener("change", (e) => {
    if (e.target.matches('[data-uploader] input[type="file"]') && e.target.files.length) submit(e.target);
  });
  document.addEventListener("dragover", (e) => {
    const root = e.target.closest?.("[data-uploader]");
    if (root) { e.preventDefault(); root.setAttribute("data-over", ""); }
  });
  document.addEventListener("dragleave", (e) => {
    const root = e.target.closest?.("[data-uploader]");
    if (root && !root.contains(e.relatedTarget)) root.removeAttribute("data-over");
  });
  document.addEventListener("drop", (e) => {
    const root = e.target.closest?.("[data-uploader]");
    if (!root) return;
    e.preventDefault();
    root.removeAttribute("data-over");
    const input = root.querySelector('input[type="file"]');
    if (!input || !e.dataTransfer.files.length) return;
    input.files = e.dataTransfer.files;
    submit(input);
  });
})();
