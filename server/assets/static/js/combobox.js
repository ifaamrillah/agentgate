// Combobox yang bisa dicari (komponen `combo`). Markup dari server: <select> asli (sumber nilai,
// tetap terkirim bersama form) + tombol dan panel yang masih `hidden`. Skrip ini menyembunyikan
// select, menampilkan tombol, dan mengisi daftar opsi dari <option>. Opsi boleh membawa
// data-sub (baris kedua), data-initials + data-color (avatar), data-group (judul kelompok).
// `data-combo-src`: opsi diambil dari server (GET ?q=, respons berupa <option>…) saat mengetik.
// `data-combo-submit`: form induk dikirim setelah memilih. Teks UI dari atribut data-* (i18n).
(() => {
  const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const lead = (o) => (o.dataset.initials
    ? `<span class="app-av" data-size="sm" style="--c:${esc(o.dataset.color || "#64748b")}">${esc(o.dataset.initials)}</span>`
    : o.dataset.color ? `<span class="app-dot" style="--c:${esc(o.dataset.color)}"></span>` : "");

  const init = (root) => {
    if (root.dataset.ready) return;
    root.dataset.ready = "true";
    const select = root.querySelector("select");
    const btn = root.querySelector(".app-combo-btn");
    const pop = root.querySelector(".app-combo-pop");
    const input = pop.querySelector("input");
    const list = pop.querySelector(".app-combo-list");
    const foot = pop.querySelector(".app-combo-foot");
    const check = pop.querySelector("template[data-check]")?.innerHTML || "";
    const multi = select.multiple;
    const src = root.dataset.comboSrc;
    let shown = [];
    let hover = 0;
    let timer = null;
    btn.hidden = false;

    const real = () => [...select.options].filter((o) => o.value !== "");
    const label = () => {
      const val = btn.querySelector(".app-combo-val");
      const picked = real().filter((o) => o.selected);
      if (!picked.length) {
        val.dataset.placeholder = "";
        val.textContent = root.dataset.placeholder || select.options[0]?.textContent || "";
        return;
      }
      delete val.dataset.placeholder;
      val.innerHTML = multi
        ? picked.slice(0, 3).map((o) => `<span class="badge" data-variant="neutral">${esc(o.textContent)}</span>`).join("") + (picked.length > 3 ? ` +${picked.length - 3}` : "")
        : `${lead(picked[0])}${esc(picked[0].textContent)}`;
    };
    const draw = () => {
      const q = input.value.toLowerCase().trim();
      const all = real();
      shown = src ? all : all.filter((o) => !q || `${o.textContent} ${o.dataset.sub || ""}`.toLowerCase().includes(q));
      hover = Math.min(hover, Math.max(0, shown.length - 1));
      let group = null;
      list.innerHTML = shown.map((o, i) => {
        const head = o.dataset.group && o.dataset.group !== group ? `<div class="app-combo-group">${esc((group = o.dataset.group))}</div>` : "";
        return `${head}<div class="app-combo-opt" role="option" data-i="${i}" aria-selected="${o.selected}"${i === hover ? " data-hover" : ""}>${multi ? `<input type="checkbox" class="app-check" tabindex="-1"${o.selected ? " checked" : ""}>` : ""}${lead(o)}<span>${esc(o.textContent)}${o.dataset.sub ? `<small>${esc(o.dataset.sub)}</small>` : ""}</span>${multi ? "" : check}</div>`;
      }).join("") || `<div class="app-combo-none">${esc(root.dataset.noneText || "")}</div>`;
      foot.textContent = (root.dataset.countText || "{n} / {m}").replace("{n}", shown.length).replace("{m}", src ? shown.length : all.length);
      list.querySelector("[data-hover]")?.scrollIntoView({ block: "nearest" });
    };
    const open = () => {
      document.querySelectorAll(".app-combo-pop[data-open]").forEach((p) => p !== pop && p.closest("[data-combo]")?.dispatchEvent(new Event("combo:close")));
      pop.toggleAttribute("data-open", true);
      btn.setAttribute("aria-expanded", "true");
      const r = btn.getBoundingClientRect();
      pop.toggleAttribute("data-up", r.bottom + 340 > innerHeight && r.top > 340);
      pop.toggleAttribute("data-right", r.left + 300 > innerWidth);
      input.value = "";
      hover = Math.max(0, real().findIndex((o) => o.selected));
      draw();
      input.focus();
    };
    const close = (focus = false) => {
      pop.removeAttribute("data-open");
      btn.setAttribute("aria-expanded", "false");
      if (focus) btn.focus();
    };
    const pick = (o) => {
      if (multi) o.selected = !o.selected;
      else { real().forEach((x) => (x.selected = x === o)); close(true); }
      label();
      if (multi) draw();
      select.dispatchEvent(new Event("change", { bubbles: true }));
      if (root.hasAttribute("data-combo-submit") && !multi) select.form?.requestSubmit();
    };
    const fetchRemote = () => {
      const url = new URL(src, location.href);
      url.searchParams.set("q", input.value.trim());
      fetch(url, { headers: { "HX-Request": "true", "HX-Request-Type": "partial" }, credentials: "same-origin" })
        .then((r) => (r.ok ? r.text() : ""))
        .then((html) => {
          const tmp = document.createElement("select");
          tmp.innerHTML = html;
          const keep = new Set(real().filter((o) => o.selected).map((o) => o.value));
          real().filter((o) => !o.selected).forEach((o) => o.remove());
          [...tmp.options].forEach((o) => { if (!keep.has(o.value)) select.add(o); });
          hover = 0;
          draw();
        })
        .catch(() => {});
    };

    root.addEventListener("combo:close", () => close());
    btn.addEventListener("click", () => (pop.hasAttribute("data-open") ? close() : open()));
    btn.addEventListener("keydown", (e) => { if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") { e.preventDefault(); open(); } });
    list.addEventListener("mousedown", (e) => e.preventDefault());
    list.addEventListener("click", (e) => { const el = e.target.closest("[data-i]"); if (el) pick(shown[+el.dataset.i]); });
    input.addEventListener("input", () => {
      hover = 0;
      if (src) { clearTimeout(timer); timer = setTimeout(fetchRemote, 250); } else draw();
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "ArrowDown") { hover = Math.min(hover + 1, shown.length - 1); draw(); e.preventDefault(); }
      else if (e.key === "ArrowUp") { hover = Math.max(hover - 1, 0); draw(); e.preventDefault(); }
      else if (e.key === "Enter") { e.preventDefault(); if (shown[hover]) pick(shown[hover]); }
      else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(true); }
      else if (e.key === "Tab") close();
    });
    document.addEventListener("click", (e) => { if (!root.contains(e.target)) close(); });
    select.addEventListener("change", label);
    label();
  };

  const scan = (root) => {
    if (root instanceof Element && root.matches("[data-combo]")) init(root);
    root.querySelectorAll?.("[data-combo]").forEach(init);
  };
  // htmx bisa sudah memproses <body> sebelum skrip ini terdaftar (htmx:load awal terlewat): pindai
  // dokumen sendiri saat siap, lalu onLoad untuk swap berikutnya. Upgrade idempoten.
  if (window.htmx?.onLoad) htmx.onLoad(scan);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => scan(document));
  else scan(document);
})();
