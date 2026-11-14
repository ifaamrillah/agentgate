// Board drag-drop (`[data-board-dnd]`): kartu `[draggable][data-move-action]` diseret ke kolom
// `[data-column]`; saat dilepas, form tersembunyi `[data-board-move-form]` diisi (stage kolom,
// `before` = id kartu di bawah titik lepas) lalu dikirim biasa. Server memvalidasi (blok, izin,
// batas WIP) dan mengalihkan kembali ke board; tanpa JS, menu ⋯ di kartu tetap bisa memindahkan.
//
// Batas WIP (R3): kolom `[data-wip-full]` menolak kartu dari kolom lain langsung di browser
// (kartu tidak berpindah, kolom bergetar, pesan `[data-board-reject]` + live region). Pilihan
// menu ⋯ ke kolom penuh ditolak dengan cara yang sama. Admin project (`#wip-force`) bisa memaksa
// dengan alasan (POST …/move-over-wip). Server tetap otoritas: penolakan dari server kembali ke
// board dengan flash, kartu di posisi semula.
(() => {
  let dragged = null;
  let pending = null; // { action, stage, name, msg } untuk dialog paksa
  const columnOf = (el) => el?.closest?.("[data-board-dnd] [data-column]");
  const clear = () => document.querySelectorAll("[data-board-dnd] [data-drop]").forEach((c) => c.removeAttribute("data-drop"));
  // Kartu pertama yang titik tengahnya di bawah posisi kursor.
  const cardBelow = (column, y) =>
    [...column.querySelectorAll("[data-ticket-id]")]
      .filter((c) => c !== dragged)
      .find((c) => { const r = c.getBoundingClientRect(); return y < r.top + r.height / 2; });

  const page = (el) => el.closest("[data-board-page]") || document;
  const reject = (column, card, statusAction) => {
    const root = page(column);
    const box = root.querySelector("[data-board-reject]");
    const msg = column.dataset.wipMsg || "";
    if (box) {
      box.querySelector("[data-board-reject-msg]").textContent = msg;
      box.hidden = false;
    }
    column.removeAttribute("data-wip-reject");
    void column.offsetWidth; // mulai ulang animasi
    column.setAttribute("data-wip-reject", "");
    setTimeout(() => column.removeAttribute("data-wip-reject"), 700);
    pending = {
      action: statusAction.replace(/\/status$/, "/move-over-wip"),
      stage: column.dataset.column,
      msg,
      card,
    };
    // Fokus kembali ke judul kartu agar pengguna keyboard tidak kehilangan posisi.
    card?.querySelector("a")?.focus({ preventScroll: true });
  };

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
    const blocked = column.hasAttribute("data-wip-full") && dragged.closest("[data-column]") !== column;
    e.dataTransfer.dropEffect = blocked ? "none" : "move";
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
    // Kolom penuh: kartu tetap di kolom asalnya.
    if (!same && column.hasAttribute("data-wip-full")) {
      clear();
      reject(column, dragged, dragged.dataset.moveAction);
      return;
    }
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

  // Menu ⋯ "Pindah ke …" ke kolom penuh: tolak sebelum dikirim.
  document.addEventListener("submit", (e) => {
    const form = e.target;
    const card = form.closest?.("[data-board-dnd] [data-ticket-id]");
    const stage = form.elements?.stage?.value;
    if (!card || !stage || form.matches("[data-board-move-form]")) return;
    const board = card.closest("[data-board-dnd]");
    const column = [...board.querySelectorAll("[data-column]")].find((c) => c.dataset.column === stage);
    if (!column || !column.hasAttribute("data-wip-full") || card.closest("[data-column]") === column) return;
    e.preventDefault();
    e.stopPropagation();
    reject(column, card, form.action);
  }, true);

  document.addEventListener("click", (e) => {
    if (!e.target.closest?.("[data-wip-force-open]")) return;
    const dialog = document.getElementById("wip-force");
    if (!dialog || !pending) return;
    e.preventDefault();
    const form = dialog.querySelector("[data-wip-force-form]");
    form.action = pending.action;
    form.elements.stage.value = pending.stage;
    dialog.querySelector("[data-wip-force-msg]").textContent = pending.msg;
    dialog.showModal();
    form.elements.reason.focus();
  });
})();
