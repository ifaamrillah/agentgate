// Autocomplete kotak query JQL (`input[data-jql-suggest="<url>"]`, R3): combobox ARIA dengan
// daftar `#<aria-controls>`. Teks sebelum kursor dikirim ke server (GET <url>?q=…, debounce
// 120 ms) yang membalas `{partial, items: [{insert, label, hint, kind}]}`; memilih saran
// menyisipkan `insert` menggantikan `partial` (jql-core.mjs) lalu meminta saran berikutnya.
// Keyboard: ↓/↑ memilih, Enter/Tab menyisipkan (Enter tanpa pilihan = jalankan query), Esc
// menutup. Tanpa JS kotak query tetap input biasa.
(() => {
  const DEBOUNCE = 120;
  let core = null;
  const loadCore = () => (core ??= import("/static/js/jql-core.mjs"));

  const init = (input) => {
    if (input.dataset.jqlReady) return;
    input.dataset.jqlReady = "1";
    const list = document.getElementById(input.getAttribute("aria-controls"));
    if (!list) return;
    const url = input.dataset.jqlSuggest;
    let items = [];
    let partial = "";
    let active = -1;
    let timer = 0;
    let seq = 0;

    const open = (on) => {
      list.hidden = !on;
      input.setAttribute("aria-expanded", on ? "true" : "false");
      if (!on) { active = -1; input.removeAttribute("aria-activedescendant"); }
    };
    const highlight = () => {
      [...list.children].forEach((li, i) => li.setAttribute("aria-selected", i === active ? "true" : "false"));
      const li = list.children[active];
      if (li) { input.setAttribute("aria-activedescendant", li.id); li.scrollIntoView({ block: "nearest" }); }
      else input.removeAttribute("aria-activedescendant");
    };
    const render = () => {
      list.replaceChildren(...items.map((it, i) => {
        const li = document.createElement("li");
        li.id = `${list.id}-${i}`;
        li.setAttribute("role", "option");
        li.dataset.kind = it.kind;
        const code = document.createElement("code");
        code.textContent = it.label;
        li.append(code);
        if (it.hint) { const s = document.createElement("span"); s.textContent = it.hint; li.append(s); }
        li.addEventListener("mousedown", (e) => { e.preventDefault(); choose(i); });
        return li;
      }));
      active = -1;
      open(items.length > 0 && document.activeElement === input);
      highlight();
    };
    const fetchNow = async () => {
      const J = await loadCore();
      const mine = ++seq;
      const q = J.prefixAt(input.value, input.selectionStart);
      let data;
      try {
        const res = await fetch(`${url}?q=${encodeURIComponent(q)}`, { headers: { Accept: "application/json" }, credentials: "same-origin" });
        if (!res.ok) return;
        data = await res.json();
      } catch { return; }
      if (mine !== seq) return; // jawaban basi
      items = data.items || [];
      partial = data.partial || "";
      render();
    };
    const schedule = () => { clearTimeout(timer); timer = setTimeout(fetchNow, DEBOUNCE); };
    const choose = async (i) => {
      const it = items[i];
      if (!it) return;
      const J = await loadCore();
      const r = J.apply(input.value, input.selectionStart, partial, it.insert);
      input.value = r.value;
      input.setSelectionRange(r.cursor, r.cursor);
      input.removeAttribute("aria-invalid");
      open(false);
      input.focus();
      schedule();
    };

    input.addEventListener("input", schedule);
    input.addEventListener("focus", schedule);
    input.addEventListener("click", schedule);
    input.addEventListener("blur", () => setTimeout(() => open(false), 100));
    input.addEventListener("keydown", async (e) => {
      const shown = !list.hidden && items.length > 0;
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        if (!shown) { await fetchNow(); return; }
        const J = await loadCore();
        active = J.move(active, items.length, e.key === "ArrowDown" ? 1 : -1);
        highlight();
      } else if ((e.key === "Enter" || e.key === "Tab") && shown && active >= 0) {
        e.preventDefault();
        choose(active);
      } else if (e.key === "Escape" && shown) {
        e.preventDefault();
        open(false);
      } else if (e.key === "ArrowLeft" || e.key === "ArrowRight" || e.key === "Home" || e.key === "End") {
        schedule();
      }
    });
  };

  const scan = (root) => {
    const found = root instanceof Element && root.matches("input[data-jql-suggest]") ? [root] : root.querySelectorAll?.("input[data-jql-suggest]") ?? [];
    found.forEach(init);
  };
  if (window.htmx?.onLoad) htmx.onLoad(scan);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => scan(document));
  else scan(document);
})();
