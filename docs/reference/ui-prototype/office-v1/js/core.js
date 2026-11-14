/* Inti bersama ketiga variasi: kamera (geser/zoom), label HTML, mini-map, tab ruangan, panel samping.
   Variasi hanya menggambar dunia SVG dan melaporkan titik jangkar karakter/ruangan:
     render({ world, defs, data }) -> { bounds:{x0,y0,x1,y1}, chars:{id:[x,y]}, rooms:{k:{box,label:[x,y]}} }
   Label (nama, badge status, nama ruangan) adalah HTML berukuran tetap, jadi tetap terbaca di zoom berapa pun. */
(() => {
  const { S } = window.SV;
  const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const ic = window.ic;
  const ST = window.ST;
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

  function mount(root, variant, data) {
    const stage = root.querySelector(".of-stage");
    const svg = root.querySelector(".of-svg");
    const labels = root.querySelector(".of-labels");
    const side = root.querySelector(".of-side");
    const tabs = root.querySelector(".of-tabs");
    const mm = root.querySelector(".of-mm");
    const pct = root.querySelector("[data-pct]");
    svg.innerHTML = ""; labels.innerHTML = ""; mm.innerHTML = "";
    stage.className = `of-stage ${variant.cls}`;
    root.dataset.variant = variant.cls;

    const defs = S("defs", {}, svg);
    const cam = S("g", { cls: "cam" }, svg);
    const world = S("g", { id: `world-${variant.cls}`, cls: "world" }, cam);
    const R = variant.render({ world, defs, data });
    const B = R.bounds;

    /* ---------- kamera ---------- */
    const Z = { min: variant.zoom?.min ?? 0.35, max: variant.zoom?.max ?? 2.6 };
    const cam0 = { z: 1, tx: 0, ty: 0 };
    let W = 0, H = 0, anim = 0;
    const size = () => { const r = stage.getBoundingClientRect(); W = r.width; H = r.height; };
    const apply = () => {
      cam.setAttribute("transform", `translate(${cam0.tx.toFixed(2)} ${cam0.ty.toFixed(2)}) scale(${cam0.z.toFixed(4)})`);
      if (pct) pct.textContent = `${Math.round(cam0.z * 100)}%`;
      stage.classList.toggle("lod-far", cam0.z < (variant.lod ?? 0.62));
      placeLabels(); drawMMView();
    };
    /* Area pandang dikurangi overlay atas (ringkasan) dan bawah (zoom). */
    const TOP = 52, BOT = 44;
    const fitOf = (b, pad = 0.9) => {
      const h = H - TOP - BOT;
      const z = Math.max(Z.min, Math.min(Z.max, Math.min(W / (b.x1 - b.x0), h / (b.y1 - b.y0)) * pad));
      return { z, tx: W / 2 - ((b.x0 + b.x1) / 2) * z, ty: TOP + h / 2 - ((b.y0 + b.y1) / 2) * z };
    };
    const go = (t, ms = 420) => {
      cancelAnimationFrame(anim);
      if (reduced() || ms === 0) { Object.assign(cam0, t); apply(); return; }
      const f = { ...cam0 }, t0 = performance.now();
      const step = (now) => {
        const k = Math.min(1, (now - t0) / ms), e = 1 - Math.pow(1 - k, 3);
        cam0.z = f.z + (t.z - f.z) * e; cam0.tx = f.tx + (t.tx - f.tx) * e; cam0.ty = f.ty + (t.ty - f.ty) * e;
        apply(); if (k < 1) anim = requestAnimationFrame(step);
      };
      anim = requestAnimationFrame(step);
    };
    const zoomAt = (k, sx = W / 2, sy = H / 2, ms = 0) => {
      const z = Math.max(Z.min, Math.min(Z.max, cam0.z * k));
      const wx = (sx - cam0.tx) / cam0.z, wy = (sy - cam0.ty) / cam0.z;
      go({ z, tx: sx - wx * z, ty: sy - wy * z }, ms);
    };
    const fit = (ms) => go(fitOf(B, 0.94), ms);
    const focusRoom = (k) => { const r = R.rooms[k]; if (r) go(fitOf(r.box, 0.86)); };
    const focusChar = (id) => { const p = R.chars[id]; if (!p) return; const z = Math.max(cam0.z, variant.zoom?.focus ?? 1.25); go({ z, tx: W * 0.5 - p[0] * z, ty: H * 0.55 - p[1] * z }); };

    /* ---------- geser / zoom input ---------- */
    let drag = null;
    stage.addEventListener("pointerdown", (e) => {
      if (e.button !== 0 || e.target.closest(".of-ov, .of-tag, .of-rl")) return;
      drag = { x: e.clientX, y: e.clientY, tx: cam0.tx, ty: cam0.ty, moved: false, id: e.pointerId, target: e.target };
      cancelAnimationFrame(anim);
    });
    window.addEventListener("pointermove", (e) => {
      if (!drag || e.pointerId !== drag.id) return;
      const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
      if (!drag.moved && Math.hypot(dx, dy) > 4) { drag.moved = true; stage.classList.add("dragging"); try { stage.setPointerCapture(drag.id); } catch { /* */ } }
      if (drag.moved) { cam0.tx = drag.tx + dx; cam0.ty = drag.ty + dy; apply(); }
    });
    window.addEventListener("pointerup", (e) => {
      if (!drag || e.pointerId !== drag.id) return;
      const d = drag; drag = null; stage.classList.remove("dragging");
      if (!d.moved) { const c = d.target.closest?.("[data-ch]"); select(c ? c.dataset.ch : null); }
    });
    stage.addEventListener("wheel", (e) => {
      e.preventDefault();
      const r = stage.getBoundingClientRect();
      zoomAt(Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0016)), e.clientX - r.left, e.clientY - r.top);
    }, { passive: false });
    stage.addEventListener("keydown", (e) => {
      const m = { ArrowLeft: [40, 0], ArrowRight: [-40, 0], ArrowUp: [0, 40], ArrowDown: [0, -40] }[e.key];
      if (m) { e.preventDefault(); cam0.tx += m[0]; cam0.ty += m[1]; apply(); }
      else if (e.key === "+" || e.key === "=") zoomAt(1.2, W / 2, H / 2, 200);
      else if (e.key === "-") zoomAt(1 / 1.2, W / 2, H / 2, 200);
      else if (e.key === "0") fit();
      else if (e.key === "Escape") select(null);
    });
    root.querySelectorAll("[data-zoom]").forEach((b) => (b.onclick = () => {
      const k = b.dataset.zoom;
      if (k === "in") zoomAt(1.25, W / 2, H / 2, 220); else if (k === "out") zoomAt(0.8, W / 2, H / 2, 220);
      else if (k === "100") zoomAt(1 / cam0.z, W / 2, H / 2, 220); else { setRoom("all"); fit(); }
    }));

    /* ---------- label HTML ---------- */
    const counts = (k) => data.chars.filter((c) => c.room === k);
    const roomSub = (r) => {
      const cs = counts(r.k), act = cs.filter((c) => ST[c.st].act).length;
      if (r.kind === "pantry") return cs.length ? `${cs.length} agent menganggur` : "Kosong";
      if (r.kind === "meeting") return cs.length ? "Sesi brainstorming" : "Kosong";
      if (!cs.length) return "Tidak ada job aktif";
      return `${cs.length} di ruangan${act ? ` · <b>${act} perlu tindakan</b>` : ""}`;
    };
    const roomEls = data.rooms.map((r) => {
      const el = document.createElement("button");
      el.type = "button"; el.className = `of-rl k-${r.kind}`;
      el.innerHTML = `<span class="t"><i style="background:${r.c}"></i>${esc(r.n)}</span><small>${roomSub(r)}</small>`;
      el.dataset.pos = R.rooms[r.k].pos || "above";
      el.onclick = () => setRoom(r.k);
      labels.appendChild(el);
      return { el, at: R.rooms[r.k].label, r };
    });
    const charEls = data.chars.map((c) => {
      const s = ST[c.st], el = document.createElement("button");
      el.type = "button"; el.className = `of-tag st-${c.st}`; el.dataset.ch = c.id;
      const badge = s.act ? `<span class="b">${ic(s.icon)}<span>${s.label}</span></span>` : c.st === "offline" ? `<span class="b b-off">${ic("plug-x")}<span>Offline</span></span>` : "";
      const tk = c.tk && c.st !== "idle" ? `<span class="tk">${esc(c.tk)}</span>` : "";
      el.innerHTML = `${badge}<span class="n"><i></i>${esc(c.name)}${tk}</span>`;
      el.title = `${c.bot ? "Agent " : ""}${c.name} · ${s.label}`;
      el.onclick = () => select(c.id, true);
      labels.appendChild(el);
      return { el, at: R.chars[c.id], c };
    });
    /* Posisi label + declutter sederhana: dari yang paling bawah di layar, label yang bertabrakan
       dengan label yang sudah ditempatkan didorong ke atas (ruangan "below" didorong ke bawah). */
    function placeLabels() {
      const { z, tx, ty } = cam0;
      const all = [];
      charEls.forEach((o) => all.push({ el: o.el, x: o.at[0] * z + tx, y: o.at[1] * z + ty, mode: "tag" }));
      roomEls.forEach((o) => all.push({ el: o.el, x: o.at[0] * z + tx, y: o.at[1] * z + ty, mode: o.el.dataset.pos }));
      all.forEach((a) => {
        const w = a.el.offsetWidth, h = a.el.offsetHeight;
        a.w = w; a.h = h;
        if (a.mode === "below") { a.t = a.y + 8; } else if (a.mode === "inside") { a.t = a.y; } else if (a.mode === "center") { a.t = a.y - h / 2; } else if (a.mode === "tag") { a.t = a.y - h; } else { a.t = a.y - h - 6; }
        a.l = a.mode === "inside" ? a.x : a.x - w / 2;
        a.fixed = a.mode === "center";
      });
      const placed = [];
      const hit = (a, b) => a.l < b.l + b.w + 3 && b.l < a.l + a.w + 3 && a.t < b.t + b.h + 2 && b.t < a.t + a.h + 2;
      all.filter((a) => a.w).sort((a, b) => (b.fixed ? 1 : 0) - (a.fixed ? 1 : 0) || (b.t + b.h) - (a.t + a.h)).forEach((a) => {
        const down = a.mode === "below";
        if (a.fixed) { placed.push(a); return; }
        for (let i = 0; i < 12; i++) {
          const o = placed.find((b) => hit(a, b));
          if (!o) break;
          a.t = down ? o.t + o.h + 3 : o.t - a.h - 3;
        }
        placed.push(a);
      });
      all.forEach((a) => {
        const dy = a.mode === "below" ? a.t - (a.y + 8) : a.mode === "inside" ? a.t - a.y : a.mode === "center" ? 0 : a.mode === "tag" ? a.t - (a.y - a.h) : a.t - (a.y - a.h - 6);
        a.el.style.transform = `translate(${Math.round(a.x)}px,${Math.round(a.y + (dy || 0))}px)`;
      });
    }

    /* ---------- mini-map ---------- */
    const pad = 20;
    const mvb = { x: B.x0 - pad, y: B.y0 - pad, w: B.x1 - B.x0 + pad * 2, h: B.y1 - B.y0 + pad * 2 };
    mm.setAttribute("viewBox", `${mvb.x} ${mvb.y} ${mvb.w} ${mvb.h}`);
    S("use", { href: `#world-${variant.cls}`, cls: "mm-world" }, mm);
    data.chars.forEach((c) => { const p = R.chars[c.id]; if (p) S("circle", { cx: p[0], cy: p[1] + (variant.mmDot ?? 0), r: mvb.w / 70, cls: `mm-dot st-${c.st}` }, mm); });
    const view = S("rect", { cls: "mm-view", rx: mvb.w / 120 }, mm);
    function drawMMView() {
      const x = -cam0.tx / cam0.z, y = -cam0.ty / cam0.z;
      view.setAttribute("x", x); view.setAttribute("y", y);
      view.setAttribute("width", Math.max(1, W / cam0.z)); view.setAttribute("height", Math.max(1, H / cam0.z));
      view.style.strokeWidth = `${mvb.w / 160}px`;
      // Seluruh kantor sudah terlihat: mini-map tidak berguna, sembunyikan supaya tidak menutupi label.
      const all = x <= B.x0 + 4 && y <= B.y0 + 4 && x + W / cam0.z >= B.x1 - 4 && y + H / cam0.z >= B.y1 - 4;
      mm.parentElement.classList.toggle("mm-hide", all);
    }
    const mmGo = (e, ms) => {
      const r = mm.getBoundingClientRect(), s = Math.min(r.width / mvb.w, r.height / mvb.h);
      const ox = (r.width - mvb.w * s) / 2, oy = (r.height - mvb.h * s) / 2;
      const wx = mvb.x + (e.clientX - r.left - ox) / s, wy = mvb.y + (e.clientY - r.top - oy) / s;
      go({ z: cam0.z, tx: W / 2 - wx * cam0.z, ty: H / 2 - wy * cam0.z }, ms);
    };
    let mmDrag = false;
    mm.addEventListener("pointerdown", (e) => { mmDrag = true; mm.setPointerCapture(e.pointerId); mmGo(e, 0); });
    mm.addEventListener("pointermove", (e) => { if (mmDrag) mmGo(e, 0); });
    mm.addEventListener("pointerup", () => { mmDrag = false; });

    /* ---------- tab ruangan ---------- */
    let room = "all";
    tabs.innerHTML = `<button type="button" data-room="all" aria-pressed="true">Semua ruangan <span class="cnt">${data.chars.length}</span></button>` +
      data.rooms.map((r) => `<button type="button" data-room="${r.k}" aria-pressed="false"><i class="dot" style="background:${r.c}"></i>${esc(r.n)} <span class="cnt">${counts(r.k).length}</span></button>`).join("");
    tabs.onclick = (e) => { const b = e.target.closest("[data-room]"); if (b) setRoom(b.dataset.room); };
    function setRoom(k) {
      room = k;
      tabs.querySelectorAll("[data-room]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.room === k)));
      roomEls.forEach(({ el, r }) => el.classList.toggle("on", r.k === k));
      if (k === "all") fit(); else focusRoom(k);
    }

    /* ---------- ringkasan ---------- */
    const sum = root.querySelector(".of-sum");
    const n = (st) => data.chars.filter((c) => c.st === st).length;
    sum.innerHTML = [["working", "bekerja"], ["review", "menunggu review"], ["decision", "butuh keputusan"], ["idle", "di pantry"], ["offline", "offline"]]
      .map(([st, l]) => `<span class="st-${st}"><i></i><b>${n(st)}</b>${l}</span>`).join("");

    /* ---------- seleksi + panel ---------- */
    let sel = null;
    function select(id, focus) {
      sel = id;
      world.querySelectorAll("[data-ch]").forEach((g) => g.classList.toggle("sel", g.dataset.ch === id));
      charEls.forEach(({ el, c }) => el.classList.toggle("sel", c.id === id));
      stage.classList.toggle("has-sel", !!id);
      if (id && focus) focusChar(id);
      renderPanel();
    }
    function avatar(c) {
      return c.bot ? `<span class="av bot" style="--c:${c.color}">${ic(c.icon)}</span>` : `<span class="av" style="--c:${c.color}">${esc(c.name[0])}</span>`;
    }
    function badge(st) { const s = ST[st]; return `<span class="pill st-${st}">${ic(s.icon)}${s.label}</span>`; }
    function renderPanel() {
      const c = data.chars.find((x) => x.id === sel);
      if (!c) {
        const act = data.chars.filter((x) => ST[x.st].act || x.st === "offline");
        const runners = [["laptop-ifa", "Ifa", "Claude Code", 2, true], ["laptop-ana", "Ana", "Codex", 1, true], ["laptop-budi", "Budi", "Claude Code", 1, true], ["pc-kantor", "Kantor", "Claude Code", 0, false]];
        side.innerHTML = `
          <section class="card"><header><h3>Perlu tindakan <span class="cnt">${act.length}</span></h3></header>
            ${act.map((x) => `<button type="button" class="row" data-sel="${x.id}">${avatar(x)}<span class="grow"><b>${x.bot ? "Agent " : ""}${esc(x.name)}</b> <span class="mono">${esc(x.tk)}</span><small>${esc(x.title)}</small></span>${badge(x.st)}</button>`).join("")}
          </section>
          <section class="card"><header><h3>Runner</h3></header>
            ${runners.map(([nm, o, cl, j, on]) => `<div class="row"><span class="rdot ${on ? "on" : ""}"></span><span class="grow"><b class="mono">${nm}</b><small>${o} · ${cl}</small></span><small class="muted">${on ? `${j} job` : "offline 12 mnt"}</small></div>`).join("")}
          </section>`;
        return;
      }
      const s = ST[c.st], pctv = c.steps ? Math.round(((c.st === "review" ? c.step : c.step - 1) / c.steps) * 100) : 0;
      const acts = c.st === "review" ? `<button class="btn pri">${ic("check")}Approve</button><button class="btn">${ic("x")}Tolak</button>`
        : c.st === "decision" ? `<button class="btn pri">${ic("alert-triangle")}Buka keputusan</button>`
        : c.st === "offline" ? `<button class="btn">${ic("plug-x")}Lihat runner</button>`
        : c.tk ? `<button class="btn">${ic("external-link")}Buka tiket</button>` : "";
      const log = c.st === "working" ? ["$ cargo nextest run diskon", "  PASS diskon::member_aktif", "  PASS diskon::batas_maks", "  RUN  diskon::kupon_ganda …"] : c.st === "review" ? ["14 tes lulus, 1 dilewati", "Bukti: 3 screenshot struk", "Menunggu approval reviewer"] : c.st === "decision" ? ["Pertanyaan ke pemilik produk:", "Cuti setengah hari memotong", "0,5 dari kuota tahunan?"] : [];
      side.innerHTML = `
        <section class="card">
          <header class="who">${avatar(c)}<span class="grow"><h3>${c.bot ? "Agent " : ""}${esc(c.full || c.name)}</h3><small>${c.bot ? "Persona AI" : "Pemilik runner"}${c.runner ? ` · <span class="mono">${esc(c.runner)}</span>` : ""}</small></span><button class="icon-btn" data-sel="" aria-label="Tutup">${ic("x")}</button></header>
          <div class="body">
            ${badge(c.st)}
            ${c.tk ? `<div class="tkline"><span class="mono">${esc(c.tk)}</span> ${esc(c.title || "")}</div>` : ""}
            ${c.doing ? `<p class="doing">${esc(c.doing)}</p>` : ""}
            ${c.jobs ? `<p class="doing">Mengawasi ${c.jobs.map((j) => `<span class="mono">${j}</span>`).join(", ")}</p>` : ""}
            ${c.steps ? `<div class="prog"><div style="width:${pctv}%"></div></div><small class="muted">Langkah ${c.step} dari ${c.steps}</small>` : ""}
            ${log.length ? `<pre class="log">${log.map(esc).join("\n")}</pre>` : ""}
            ${acts ? `<div class="acts">${acts}</div>` : ""}
          </div>
        </section>`;
    }
    side.onclick = (e) => { const b = e.target.closest("[data-sel]"); if (b) select(b.dataset.sel || null, !!b.dataset.sel); };

    /* ---------- mulai ---------- */
    const ro = new ResizeObserver(() => { const first = !W; size(); if (first) { const h = variant.home ? variant.home(R, W, H, fitOf) : fitOf(B, 0.94); go(h, 0); } else apply(); });
    ro.observe(stage);
    renderPanel();
    return { destroy() { ro.disconnect(); cancelAnimationFrame(anim); }, select, setRoom, fit, cam: cam0, zoomAt };
  }

  window.OfficeCore = { mount, esc };
})();
