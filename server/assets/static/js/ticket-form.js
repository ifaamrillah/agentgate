// Form tiket (`[data-ticket-form]`): field custom `[data-custom-field][data-types]` hanya tampil
// untuk tipe yang dipilih di `select[name="type_id"]`. Field tersembunyi ikut nonaktif supaya
// nilainya tidak terkirim. Tanpa JS semua field tampil; server hanya memakai field tipe terpilih.
(() => {
  const sync = (form) => {
    const select = form.querySelector('select[name="type_id"]');
    if (!select) return;
    form.querySelectorAll("[data-custom-field][data-types]").forEach((field) => {
      const show = field.dataset.types.split(" ").includes(select.value);
      field.hidden = !show;
      field.querySelectorAll("input, select, textarea").forEach((el) => { el.disabled = !show; });
    });
  };
  const all = () => document.querySelectorAll("[data-ticket-form]").forEach(sync);
  document.addEventListener("change", (e) => {
    if (e.target.matches('[data-ticket-form] select[name="type_id"]')) sync(e.target.closest("[data-ticket-form]"));
  });
  document.addEventListener("DOMContentLoaded", all);
  document.addEventListener("htmx:after:swap", all);
})();
