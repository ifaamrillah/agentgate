// Board drag-drop (`[data-board-dnd]`): kartu `[draggable][data-move-action]` diseret ke kolom
// `[data-column]`; saat dilepas, form tersembunyi `[data-board-move-form]` diisi (stage kolom,
// `before` = id kartu di bawah titik lepas) lalu dikirim biasa. Server memvalidasi (blok, izin) dan
// mengalihkan kembali ke board; tanpa JS, menu ⋯ di kartu tetap bisa memindahkan status.
(() => {
  let dragged = null;
  const columnOf = (el) => el?.closest?.("[data-board-dnd] [data-column]");
  const clear = () => document.querySelectorAll("[data-board-dnd] [data-drop]").forEach((c) => c.removeAttribute("data-drop"));
  // Kartu pertama yang titik tengahnya di bawah posisi kursor.
  const cardBelow = (column, y) =>
    [...column.querySelectorAll("[data-ticket-id]")]
      .filter((c) => c !== dragged)
      .find((c) => { const r = c.getBoundingClientRect(); return y < r.top + r.height / 2; });

  document.addEventListener("dragstart", (e) => {
    const card = e.target.closest?.("[data-board-dnd] [data-move-action]");
    if (!card) return;
    dragged = card;
    card.setAttribute("data-dragging", "");
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", card.dataset.ticket || "");
  });
  document.addEventListener("dragend", () => {
    dragged?.removeAttribute("data-dragging");
    dragged = null;
    clear();
  });
  document.addEventListener("dragover", (e) => {
    const column = dragged && columnOf(e.target);
    if (!column) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (!column.hasAttribute("data-drop")) { clear(); column.setAttribute("data-drop", ""); }
  });
  document.addEventListener("drop", (e) => {
    const column = dragged && columnOf(e.target);
    if (!column) return;
    e.preventDefault();
    const form = column.closest("[data-board-dnd]").parentElement.querySelector("[data-board-move-form]");
    if (!form) return;
    const below = cardBelow(column, e.clientY);
    const same = dragged.closest("[data-column]") === column;
    // Lepas di posisi semula: tidak ada perubahan.
    if (same && (below ? below === dragged.nextElementSibling : !dragged.nextElementSibling?.matches("[data-ticket-id]"))) { clear(); return; }
    form.action = dragged.dataset.moveAction;
    form.elements.stage.value = column.dataset.column;
    form.elements.before.value = below ? below.dataset.ticketId : "";
    // Umpan balik langsung sebelum halaman dimuat ulang.
    const cards = column.querySelector("[data-cards]");
    cards.querySelector(".app-bcol-empty")?.remove();
    cards.insertBefore(dragged, below || null);
    clear();
    form.requestSubmit();
  });
})();
