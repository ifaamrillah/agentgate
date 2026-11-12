// Pemilih warna (komponen `color_picker`): input warna custom dan isian hex saling mengikuti,
// mengisi radio "custom" (value = hex), dan memperbarui pratinjau `[data-color-preview]` di form
// yang sama (custom property `--c`). Tanpa JS, radio palet dan isian hex tetap terkirim biasa.
(() => {
  const HEX = /^#[0-9a-f]{6}$/i;
  const preview = (root, color) => {
    root.closest("form")?.querySelectorAll("[data-color-preview]").forEach((el) => el.style.setProperty("--c", color));
  };
  document.addEventListener("input", (e) => {
    const root = e.target.closest("[data-color-picker]");
    if (!root) return;
    const custom = root.querySelector("[data-color-custom]");
    const picker = root.querySelector('input[type="color"]');
    const text = root.querySelector('input[type="text"]');
    if (e.target === picker) text.value = picker.value;
    else if (e.target === text) { if (!HEX.test(text.value)) return; picker.value = text.value; }
    else return;
    custom.value = picker.value;
    custom.checked = true;
    preview(root, picker.value);
  });
  document.addEventListener("change", (e) => {
    const root = e.target.closest("[data-color-picker]");
    if (root && e.target.matches('input[type="radio"]')) preview(root, e.target.value);
  });
})();
