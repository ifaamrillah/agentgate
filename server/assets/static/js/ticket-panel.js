// Panel samping detail tiket (R3). Tautan `a[data-ticket-peek]` (judul kartu board, baris daftar,
// backlog, roadmap, dan tiket terkait di dalam panel) dibuka di <dialog id="ticket-panel">
// (layouts/project.html) tanpa meninggalkan board:
// - isi dari GET <href tiket> dengan header `X-Ticket-Panel: 1` (partial tickets/panel.html, isi
//   Ringkasan yang sama dengan halaman penuh), lalu htmx.process agar perilaku JS lain ikut aktif;
// - URL diganti ke URL tiket (history.pushState), jadi Back menutup panel dan Forward membukanya
//   lagi; URL itu dibuka langsung = halaman penuh;
// - dialog modal native: fokus terkunci di panel, latar inert; Esc, tombol tutup, dan klik latar
//   menutup (lewat history.back bila panel yang menambah entri history); fokus kembali ke tautan
//   pembuka;
// - klik tengah/Ctrl/Cmd/Shift tetap membuka tab/jendela baru; tanpa dialog (halaman lain) atau
//   tanpa JS tautan berjalan biasa ke halaman penuh.
(() => {
  let opener = null; // tautan pembuka (atau href-nya bila DOM board sudah diganti)
  let openerHref = "";
  let seq = 0;

  const dialog = () => document.getElementById("ticket-panel");
  const bodyOf = (d) => d.querySelector("[data-ticket-panel-body]");

  const focusInside = (d) => {
    const target = d.querySelector("#ticket-panel-title") ? d.querySelector("[data-ticket-panel-close]") : null;
    (target || d.querySelector("[data-ticket-panel-close]"))?.focus({ preventScroll: true });
  };

  const showLoading = (d) => {
    const tpl = d.querySelector("template[data-ticket-panel-loading]");
    bodyOf(d).replaceChildren(tpl ? tpl.content.cloneNode(true) : document.createTextNode(""));
  };

  const showFailed = (d, url) => {
    const p = document.createElement("p");
    p.className = "app-hint";
    p.setAttribute("role", "alert");
    const a = document.createElement("a");
    a.className = "app-link";
    a.href = url;
    a.textContent = d.dataset.failedText || url;
    p.append(a);
    bodyOf(d).querySelector(".app-tpanel-b")?.replaceChildren(p);
  };

  async function load(url, push) {
    const d = dialog();
    if (!d) return false;
    const mine = ++seq;
    if (!d.open) {
      showLoading(d);
      d.showModal();
    }
    d.setAttribute("aria-busy", "true");
    focusInside(d);
    if (push) history.pushState({ agPanel: true, url }, "", url);
    try {
      const res = await fetch(url, {
        headers: { "X-Ticket-Panel": "1", "HX-Request": "true", Accept: "text/html" },
        credentials: "same-origin",
      });
      if (!res.ok) throw new Error(String(res.status));
      const html = await res.text();
      if (mine !== seq || !d.open) return true;
      const body = bodyOf(d);
      body.innerHTML = html;
      body.scrollTop = 0;
      window.htmx?.process?.(body);
      focusInside(d);
    } catch {
      if (mine === seq) showFailed(d, url);
    } finally {
      if (mine === seq) d.removeAttribute("aria-busy");
    }
    return true;
  }

  function finishClose() {
    const d = dialog();
    if (d?.open) d.close();
    if (d) bodyOf(d).replaceChildren();
    seq++;
    // Kembalikan fokus ke pembuka; bila board sudah dimuat ulang, cari tautan yang sama.
    const target = opener?.isConnected ? opener : document.querySelector(`a[data-ticket-peek][href="${CSS.escape(openerHref)}"]`);
    target?.focus({ preventScroll: false });
  }

  function close() {
    const d = dialog();
    if (!d?.open) return;
    // Panel yang menambah entri history ditutup lewat Back, supaya URL kembali ke board.
    if (history.state?.agPanel) history.back();
    else finishClose();
  }

  document.addEventListener("click", (e) => {
    const link = e.target.closest?.("a[data-ticket-peek]");
    if (link) {
      if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      if (!dialog() || new URL(link.href, location.href).origin !== location.origin) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      const d = dialog();
      // Tautan di dalam panel (subtask, relasi) mengganti isi panel; pembuka tetap kartu awal.
      if (!d.contains(link)) {
        opener = link;
        openerHref = link.getAttribute("href");
      }
      load(link.href, true);
      return;
    }
    if (e.target.closest?.("[data-ticket-panel-close]")) {
      e.preventDefault();
      close();
      return;
    }
    // Klik latar (di luar kotak panel) menutup.
    const d = dialog();
    if (d?.open && e.target === d) close();
  }, true);

  // Esc: dialog native memicu `cancel`; tutup lewat jalur yang sama (history).
  document.addEventListener("cancel", (e) => {
    if (e.target === dialog()) {
      e.preventDefault();
      close();
    }
  }, true);

  // Editor teks kaya (Milkdown/ProseMirror) memakai Esc sendiri dan dialog tidak menerima
  // `cancel`: Esc pertama keluar dari editor ke tombol tutup, Esc berikutnya menutup panel.
  document.addEventListener("keydown", (e) => {
    const d = dialog();
    if (e.key !== "Escape" || !d?.open) return;
    const editor = document.activeElement?.closest?.(".ProseMirror, .milkdown");
    if (editor && d.contains(editor)) {
      e.preventDefault();
      e.stopPropagation();
      d.querySelector("[data-ticket-panel-close]")?.focus();
    }
  }, true);

  window.addEventListener("popstate", (e) => {
    const d = dialog();
    if (e.state?.agPanel && d) {
      load(e.state.url, false);
    } else if (d?.open) {
      finishClose();
    }
  });
})();
