// Backlog drag-drop (`[data-backlog-dnd]`): baris `[draggable][data-move-action]` diseret ke blok
// sprint/backlog `[data-drop-target]`; saat dilepas, form tersembunyi `[data-backlog-move-form]`
// diisi (`to` = id sprint atau `backlog`, `place` = keep) lalu dikirim biasa. Server memvalidasi
// dan mengalihkan kembali; tanpa JS, menu ⋯ di baris tetap bisa memindahkan tiket.
(() => {
  let dragged = null;
  const zoneOf = (el) => el?.closest?.("[data-backlog-dnd] [data-drop-target]");
  const clear = () => document.querySelectorAll("[data-backlog-dnd] [data-drop]").forEach((z) => z.removeAttribute("data-drop"));

  document.addEventListener("dragstart", (e) => {
    const row = e.target.closest?.("[data-backlog-dnd] [data-move-action]");
    if (!row) return;
    dragged = row;
    row.setAttribute("data-dragging", "");
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", row.dataset.ticketRow || "");
  });
  document.addEventListener("dragend", () => {
    dragged?.removeAttribute("data-dragging");
    dragged = null;
    clear();
  });
  document.addEventListener("dragover", (e) => {
    const zone = dragged && zoneOf(e.target);
    if (!zone) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (!zone.hasAttribute("data-drop")) { clear(); zone.setAttribute("data-drop", ""); }
  });
  document.addEventListener("drop", (e) => {
    const zone = dragged && zoneOf(e.target);
    if (!zone) return;
    e.preventDefault();
    clear();
    if (dragged.closest("[data-drop-target]") === zone) return;
    const form = document.querySelector("[data-backlog-move-form]");
    if (!form) return;
    form.action = dragged.dataset.moveAction;
    form.elements.to.value = zone.dataset.dropTarget;
    form.elements.place.value = "keep";
    // Umpan balik langsung sebelum halaman dimuat ulang.
    zone.querySelector("[data-rows]")?.appendChild(dragged);
    form.requestSubmit();
  });
})();
