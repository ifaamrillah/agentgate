/* Agent Gate prototype · inti: data contoh, komponen, kerangka, router, interaksi.
   Modul layar mendaftar lewat AG.screen({...}), AG.dialog(name, fn), AG.menu(name, fn), AG.action(name, fn). */
(() => {
const AG = (window.AG = {});

/* ====================== Data contoh ====================== */
AG.PEOPLE = {
  OO: { name: "Olivia Owner", email: "owner@agentgate.test", job: "Product Manager", role: "Owner", tfa: true, c: "#7c3aed", joined: "11 Nov 2026" },
  AA: { name: "Adi Admin", email: "admin@agentgate.test", job: "Tech Lead", role: "Admin", tfa: true, c: "#2563eb", joined: "11 Nov 2026" },
  DD: { name: "Dewi Dev", email: "dev@agentgate.test", job: "Developer", role: "Member", tfa: true, c: "#0f8a5f", joined: "12 Nov 2026" },
  QQ: { name: "Qori QA", email: "qa@agentgate.test", job: "QA Engineer", role: "Member", tfa: false, c: "#c2410c", joined: "12 Nov 2026" },
  VV: { name: "Vina Viewer", email: "viewer@agentgate.test", job: "", role: "Viewer", tfa: false, c: "#64748b", joined: "12 Nov 2026" },
};
AG.ME = "OO";
AG.PROJECTS = [
  { k: "KSR", n: "Kasir Online", c: "#7c3aed", d: "Aplikasi kasir untuk toko retail", wf: "Superpowers", repos: ["kasir-api", "kasir-web"] },
  { k: "GDG", n: "Aplikasi Gudang", c: "#0ea5e9", d: "Stok dan mutasi barang antar gudang", wf: "Scrum", repos: ["gudang-service"] },
  { k: "PPL", n: "Portal Pelanggan", c: "#f59e0b", d: "Riwayat belanja dan poin pelanggan", wf: "Kanban", repos: ["portal-web", "portal-mobile"] },
];
AG.project = (k) => AG.PROJECTS.find((p) => p.k === k) || AG.PROJECTS[0];

/* Persona agent bawaan (spec: Persona agent). */
AG.PERSONAS = {
  analyst: { n: "Analyst", t: "Riset dan brainstorming", c: "#0891b2", i: "bulb" },
  pm: { n: "Product Manager", t: "PRD", c: "#7c3aed", i: "file-text" },
  uiux: { n: "UI/UX Designer", t: "Alur, wireframe, mockup HTML", c: "#db2777", i: "layout-sidebar" },
  architect: { n: "Architect", t: "Tech spec", c: "#4f46e5", i: "hierarchy-2" },
  dev: { n: "Developer", t: "Implementasi kode", c: "#0f8a5f", i: "code" },
  reviewer: { n: "Code Reviewer", t: "Review PR", c: "#2563eb", i: "git-compare" },
  qa: { n: "QA", t: "Testing (Playwright + screenshot)", c: "#c2410c", i: "test-pipe" },
  devops: { n: "DevOps", t: "Deploy", c: "#475569", i: "rocket" },
  writer: { n: "Tech Writer", t: "Knowledge dan release notes", c: "#a16207", i: "writing" },
};

/* Workflow project KSR (template Superpowers-style yang di-clone). */
AG.STAGES = [
  { k: "backlog", n: "Backlog", i: "circle-dashed", c: "var(--faint)" },
  { k: "brainstorm", n: "Brainstorming", persona: "analyst", mode: "interaktif", i: "bulb", c: "#0891b2", gate: ["Catatan brainstorm ada"] },
  { k: "prd", n: "PRD", persona: "pm", i: "file-text", c: "#7c3aed", gate: ["Artifact PRD ada", "Acceptance criteria terisi"] },
  { k: "design", n: "Desain UI", persona: "uiux", i: "layout-sidebar", c: "#db2777", gate: ["Mockup HTML ada", "Screenshot ada"] },
  { k: "dev", n: "Development", persona: "dev", i: "code", c: "#0f8a5f", gate: ["PR terbuka", "Test lulus"], wip: 4 },
  { k: "review", n: "Code Review", persona: "reviewer", i: "git-compare", c: "#2563eb", gate: ["Review AI selesai", "CI hijau"] },
  { k: "test", n: "Testing", persona: "qa", i: "test-pipe", c: "#c2410c", gate: ["Hasil test lulus", "Screenshot ada"] },
  { k: "done", n: "Selesai", i: "circle-check-filled", c: "var(--success)", gate: ["Semua PR merged"] },
];
AG.stage = (k) => AG.STAGES.find((s) => s.k === k) || AG.STAGES[0];

/* Status tiket di dalam stage. */
AG.STATES = {
  antre: { n: "Menunggu diambil", tone: "gray", i: "hourglass" },
  jalan: { n: "Dikerjakan AI", tone: "acc", i: "sparkles" },
  review: { n: "Menunggu review", tone: "warn", i: "eye" },
  keputusan: { n: "Butuh keputusan", tone: "dng", i: "alert-triangle" },
  manusia: { n: "Dikerjakan manusia", tone: "info", i: "user" },
  siap: { n: "Siap", tone: "ok", i: "circle-check" },
};
AG.TYPES = {
  epic: { n: "Epic", i: "bolt", c: "#7c3aed" },
  story: { n: "Story", i: "bookmark", c: "#0f8a5f" },
  task: { n: "Task", i: "checkbox", c: "#2563eb" },
  bug: { n: "Bug", i: "bug", c: "#cf2e2e" },
  subtask: { n: "Subtask", i: "subtask", c: "#64748b" },
};
AG.PRIORITY = { 1: { n: "Urgent", i: "alert-triangle-filled", c: "#cf2e2e" }, 2: { n: "Tinggi", i: "antenna-bars-5", c: "var(--fg2)" }, 3: { n: "Sedang", i: "antenna-bars-4", c: "var(--fg2)" }, 4: { n: "Rendah", i: "antenna-bars-3", c: "var(--fg2)" } };
AG.LABELS = { Laporan: "#0284c7", Kasir: "#7c3aed", Struk: "#db2777", Stok: "#16a34a", Pembayaran: "#d97706", Auth: "#475569", Promo: "#e11d48" };

AG.TICKETS = [
  { k: "KSR-40", type: "epic", stage: "dev", state: "jalan", t: "Pembayaran digital di kasir", p: 2, who: "OO", lbl: ["Pembayaran"], pts: 21, sprint: "Sprint 4", due: "30 Nov", start: "4 Nov" },
  { k: "KSR-24", type: "story", stage: "review", state: "review", t: "Integrasi pembayaran QRIS", p: 1, who: "DD", lbl: ["Pembayaran"], pts: 8, epic: "KSR-40", sprint: "Sprint 4", due: "18 Nov", agent: "Review AI selesai · 3 catatan", pr: "#142" },
  { k: "KSR-26", type: "story", stage: "dev", state: "jalan", t: "Validasi stok sebelum checkout", p: 2, who: "DD", lbl: ["Stok"], pts: 5, sprint: "Sprint 4", due: "16 Nov", agent: "Menulis tes · langkah 4 dari 7", runner: "Laptop Dewi" },
  { k: "KSR-27", type: "task", stage: "dev", state: "antre", t: "Ekspor riwayat transaksi ke CSV", p: 3, who: "AA", lbl: ["Laporan"], pts: 3, sprint: "Sprint 4", due: "21 Nov", agent: "Menunggu runner Adi online" },
  { k: "KSR-30", type: "bug", stage: "test", state: "keputusan", t: "Struk tidak memuat nama kasir", p: 1, who: "QQ", lbl: ["Struk"], pts: 2, sprint: "Sprint 4", due: "14 Nov", agent: "Gagal 3 kali · total struk selisih" },
  { k: "KSR-29", type: "story", stage: "test", state: "jalan", t: "Cetak ulang struk dari riwayat", p: 3, who: "QQ", lbl: ["Struk"], pts: 3, sprint: "Sprint 4", agent: "Playwright · 12 dari 18 skenario" },
  { k: "KSR-28", type: "story", stage: "design", state: "review", t: "Diskon per item di keranjang", p: 2, who: "OO", lbl: ["Kasir", "Promo"], pts: 5, sprint: "Sprint 4", due: "20 Nov", agent: "Mockup v2 siap direview" },
  { k: "KSR-33", type: "story", stage: "prd", state: "jalan", t: "Mode offline saat koneksi toko putus", p: 2, who: "OO", lbl: ["Kasir"], pts: 13, agent: "Menyusun acceptance criteria" },
  { k: "KSR-35", type: "task", stage: "prd", state: "review", t: "Batas maksimal diskon per kasir", p: 3, who: "OO", lbl: ["Promo"], pts: 2, agent: "PRD v1 siap direview" },
  { k: "KSR-31", type: "story", stage: "brainstorm", state: "manusia", t: "Laporan penjualan harian dikirim ke email pemilik toko", p: 3, who: "OO", lbl: ["Laporan"], pts: null, agent: "Sesi brainstorming oleh Olivia" },
  { k: "KSR-36", type: "task", stage: "backlog", state: "siap", t: "Pengaturan pajak PPN per toko", p: 3, who: null, lbl: ["Kasir"], pts: 5 },
  { k: "KSR-37", type: "bug", stage: "backlog", state: "siap", t: "Scanner barcode membaca dua kali", p: 2, who: "QQ", lbl: ["Kasir"], pts: 2 },
  { k: "KSR-38", type: "story", stage: "backlog", state: "siap", t: "Program poin untuk pelanggan tetap", p: 4, who: null, lbl: ["Promo"], pts: 8 },
  { k: "KSR-22", type: "story", stage: "done", state: "siap", t: "Login kasir dengan PIN 6 digit", p: 2, who: "DD", lbl: ["Auth"], pts: 3, sprint: "Sprint 3" },
  { k: "KSR-21", type: "bug", stage: "done", state: "siap", t: "Pembulatan total belanja selisih Rp1", p: 3, who: "AA", lbl: ["Kasir"], pts: 1, sprint: "Sprint 3" },
  { k: "KSR-41", type: "subtask", stage: "dev", state: "siap", t: "Endpoint callback QRIS", p: 2, who: "DD", lbl: ["Pembayaran"], pts: 2, parent: "KSR-24" },
];
AG.ticket = (k) => AG.TICKETS.find((t) => t.k === k) || AG.TICKETS[1];

AG.RUNNERS = [
  { id: "r1", n: "MacBook Olivia", who: "OO", os: "macOS 15", client: "Claude Code", st: "idle", ver: "0.4.2", seen: "aktif", auto: false },
  { id: "r2", n: "Laptop Dewi", who: "DD", os: "Ubuntu 24.04", client: "Claude Code", st: "busy", ver: "0.4.2", seen: "aktif", job: "KSR-26", auto: true },
  { id: "r3", n: "PC Qori", who: "QQ", os: "Windows 11", client: "Codex CLI", st: "busy", ver: "0.4.1", seen: "aktif", job: "KSR-29", auto: true },
  { id: "r4", n: "ThinkPad Adi", who: "AA", os: "Fedora 41", client: "Gemini CLI", st: "off", ver: "0.4.0", seen: "3 jam lalu", auto: false },
];
AG.NOTIFS = [
  { i: "eye", tone: "warn", t: "<b>KSR-24</b> menunggu review Anda di stage Code Review", w: "5 mnt", unread: true, go: "#/t/KSR-24" },
  { i: "alert-triangle", tone: "dng", t: "<b>KSR-30</b> butuh keputusan setelah 3 kali gagal testing", w: "18 mnt", unread: true, go: "#/t/KSR-30" },
  { i: "hand-stop", tone: "acc", t: "Agent Developer minta izin menjalankan <code>prisma migrate deploy</code>", w: "22 mnt", unread: true, go: "#/settings/runner" },
  { i: "at", tone: "gray", t: "Dewi Dev menyebut Anda di <b>KSR-26</b>: “@olivia stok minus boleh?”", w: "1 jam", unread: false, go: "#/t/KSR-26" },
  { i: "book", tone: "gray", t: "3 usulan perubahan knowledge menunggu persetujuan", w: "2 jam", unread: false, go: "#/p/KSR/knowledge" },
];
AG.STATE = { theme: null };

/* ====================== Komponen ====================== */
const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
AG.esc = esc;
AG.ic = (n, cls = "") => `<svg class="ic ${n.endsWith("-filled") ? "f " : ""}${cls}" viewBox="0 0 24 24" aria-hidden="true">${(window.ICONS || {})[n] || ""}</svg>`;
const ic = AG.ic;
AG.av = (k, s = 22) => {
  if (!k) return `<span class="av none" style="width:${s}px;height:${s}px" title="Belum ada penerima"></span>`;
  if (k === "AI" || AG.PERSONAS[k]) { const p = AG.PERSONAS[k]; return `<span class="av" style="width:${s}px;height:${s}px;background:${p ? p.c : "#1f1f26"}" title="${p ? "Agent " + p.n : "Agent"}">${ic(p ? p.i : "robot", s > 24 ? "s18" : "s14")}</span>`; }
  const p = AG.PEOPLE[k] || { name: String(k), c: "#64748b" }; return `<span class="av" style="width:${s}px;height:${s}px;background:${p.c};font-size:${Math.round(s * 0.4)}px" title="${p.name}">${k}</span>`;
};
AG.attrs = (a = {}) => Object.entries(a).map(([k, v]) => (v === true ? ` ${k}` : v == null || v === false ? "" : ` ${k}="${esc(v)}"`)).join("");
/* btn(label, kelas, ikon, atribut). Contoh: btn("Simpan", "pri", "check", {"data-toast": "Tersimpan"}) */
AG.btn = (label, cls = "", icon = "", a = {}) => `<button type="button" class="btn ${cls}"${AG.attrs(a)}${!label && a["aria-label"] == null ? ` aria-label="${esc(a.title || icon)}"` : ""}>${icon ? ic(icon) : ""}${label}</button>`;
AG.inp = (value = "", { ph = "", cls = "", type = "text", area = false, pre = "", a = {} } = {}) => area
  ? `<textarea class="inp ${cls}" placeholder="${esc(ph)}"${AG.attrs(a)}>${esc(value)}</textarea>`
  : pre ? `<label class="inp ${cls}"><span class="pre">${pre}</span><input style="border:0;outline:0;background:none;flex:1;min-width:0;padding:0" type="${type}" value="${esc(value)}" placeholder="${esc(ph)}"${AG.attrs(a)}></label>`
  : `<input class="inp ${cls}" type="${type}" value="${esc(value)}" placeholder="${esc(ph)}"${AG.attrs(a)}>`;
AG.sel = (options, value) => `<span class="selwrap"><select class="inp">${options.map((o) => { const [v, l] = Array.isArray(o) ? o : [o, o]; return `<option value="${esc(v)}"${v === value ? " selected" : ""}>${esc(l)}</option>`; }).join("")}</select>${ic("selector", "s14")}</span>`;
AG.fld = (label, control, hint = "", err = "") => `<div class="fld"><label>${label}</label>${control}${hint ? `<span class="hint">${hint}</span>` : ""}${err ? `<span class="errt">${err}</span>` : ""}</div>`;
AG.pill = (t, cls = "", dot = "", icon = "") => `<span class="pill ${cls}">${dot ? `<i style="background:${dot}"></i>` : ""}${icon ? ic(icon) : ""}${t}</span>`;
AG.sw = (on = false, label = "Sakelar") => `<button type="button" class="sw ${on ? "on" : ""}" data-toggle role="switch" aria-checked="${on}" aria-label="${esc(label)}"></button>`;
AG.cb = (on = false, label = "") => `<button type="button" class="cb ${on ? "on" : ""}" data-toggle role="checkbox" aria-checked="${on}" aria-label="${esc(label || "Pilih")}">${ic("check")}</button>`;
AG.srow = (t, d, ctl, top = false) => `<div class="srow ${top ? "top" : ""}"><div><div class="t">${t}</div>${d ? `<div class="d">${d}</div>` : ""}</div><div class="ctl">${ctl}</div></div>`;
/* Kartu bagian: judul, deskripsi, dan aksi (mis. AG.seeAll) di dalam kartu; isi di badan; foot opsional (mis. AG.savebar).
   AG.card({ title, desc, action, body, foot, pad: true, id }) */
AG.card = ({ title = "", desc = "", action = "", body = "", foot = "", pad = true, id = "", cls = "" } = {}) =>
  `<section class="cardx ${cls}"${id ? ` id="${id}"` : ""}>${title || action ? `<header class="cx-h"><div class="grow"><h2>${title}</h2>${desc ? `<p>${desc}</p>` : ""}</div>${action ? `<div class="cx-a">${action}</div>` : ""}</header>` : ""}<div class="cx-b ${pad ? "" : "flush"}">${body}</div>${foot ? `<footer class="cx-f">${foot}</footer>` : ""}</section>`;
AG.seeAll = (href, label = "Lihat semua") => `<a class="seeall" href="${href}">${label}${ic("arrow-right", "s14")}</a>`;
AG.section = (t, d, body, act = "") => AG.card({ title: t, desc: d, action: act, body });
AG._sectionOld = (t, d, body, act = "") => `<section class="section"><div class="sh"><div class="grow"><h2>${t}</h2>${d ? `<p>${d}</p>` : ""}</div>${act}</div>${body}</section>`;
AG.head = (t, d = "", act = "", lead = "") => `<div class="page-head">${/arrow-left|chevron-left/.test(lead) ? "" : lead}<div class="grow"><h1>${t}</h1>${d ? `<p>${d}</p>` : ""}</div>${act ? `<div class="acts">${act}</div>` : ""}</div>`;
AG.who = (k, sub) => `<div class="who">${AG.av(k, 28)}<span><b>${AG.PEOPLE[k].name}</b><small>${sub ?? AG.PEOPLE[k].email}</small></span></div>`;
AG.empty = (icon, title, text, action = "") => `<div class="empty"><span class="tile lg">${ic(icon, "s18")}</span><b>${title}</b><span>${text}</span>${action}</div>`;
AG.stageIcon = (k, cls = "s14") => { const s = AG.stage(k); return `<span style="color:${s.c};display:inline-flex" title="${s.n}">${ic(s.i, cls)}</span>`; };
AG.prioIcon = (p, cls = "s14") => { const x = AG.PRIORITY[p]; return `<span style="color:${x.c};display:inline-flex" title="Prioritas ${x.n}">${ic(x.i, cls)}</span>`; };
AG.typeIcon = (t, cls = "s14") => { const x = AG.TYPES[t]; return `<span style="color:${x.c};display:inline-flex" title="${x.n}">${ic(x.i, cls)}</span>`; };
AG.statePill = (st) => { const s = AG.STATES[st]; return AG.pill(s.n, s.tone, "", s.i); };
AG.labelPill = (l) => AG.pill(l, "", AG.LABELS[l] || "var(--faint)");
AG.tref = (k) => { const t = AG.ticket(k); return `<a href="#/t/${t.k}" class="row g4" style="display:inline-flex;flex-shrink:0;white-space:nowrap">${AG.typeIcon(t.type)}<span class="mono mut">${t.k}</span></a>`; };
AG.projDot = (k, s = 10) => `<span class="dot" style="background:${AG.project(k).c};width:${s}px;height:${s}px"></span>`;
/* Kartu board/list tiket yang konsisten di semua layar. */
AG.ticketCard = (t, { drag = true } = {}) => {
  const st = AG.STATES[t.state];
  const box = t.state === "jalan" ? `<div class="agentbox">${ic("sparkles", "s14")}<span class="trunc">${t.agent || "Dikerjakan AI"}</span></div>`
    : t.state === "review" ? `<div class="agentbox gate">${ic("eye", "s14")}<span class="trunc">${t.agent || "Menunggu review"}</span></div>`
    : t.state === "keputusan" ? `<div class="agentbox dng">${ic("alert-triangle", "s14")}<span class="trunc">${t.agent || "Butuh keputusan"}</span></div>`
    : t.state === "antre" ? `<div class="agentbox" style="background:var(--hover);color:var(--muted)">${ic("hourglass", "s14")}<span class="trunc">${t.agent || "Menunggu diambil"}</span></div>`
    : t.state === "manusia" ? `<div class="agentbox" style="background:var(--info-soft);color:var(--info)">${ic("user", "s14")}<span class="trunc">${t.agent}</span></div>` : "";
  return `<article class="card" data-go="#/t/${t.k}" ${drag ? `draggable="true" data-drag="${t.k}"` : ""} aria-label="${esc(t.k + " " + t.t)}">
    <div class="row">${AG.typeIcon(t.type)}<span class="mono faint grow">${t.k}</span>${AG.av(t.who, 20)}</div>
    <div class="t">${t.t}</div>${box}
    <div class="row wrap">${AG.prioIcon(t.p)}${t.lbl.map(AG.labelPill).join("")}${t.pts ? `<span class="pill gray mono">${t.pts}</span>` : ""}${t.due ? `<span class="row g4 faint sm">${ic("calendar", "s14")}${t.due}</span>` : ""}</div></article>`;
};

/* ====================== Kerangka ====================== */
const NAV_APP = (proj) => {
  const P = AG.PROJECTS;
  const sub = (k) => [
    ["Ringkasan", "home", `/p/${k}`], ["Board", "layout-kanban", `/p/${k}/board`], ["Backlog", "list", `/p/${k}/backlog`],
    ["Roadmap", "timeline", `/p/${k}/roadmap`], ["Pull request", "git-pull-request", `/p/${k}/prs`], ["Knowledge", "book", `/p/${k}/knowledge`], ["Code graph", "topology-star-3", `/p/${k}/graph`],
  ];
  return `
  <button class="ws row" data-menu="workspace"><span class="wslogo">PC</span><span class="grow trunc" style="text-align:left">PT Contoh</span>${ic("chevron-down", "s14 faint")}</button>
  <button class="srch row" data-act="palette"><span class="row grow">${ic("search", "s14")}Cari atau lompat ke…</span><kbd>Ctrl K</kbd></button>
  ${navItem("Beranda", "home", "/home")}
  ${navItem("Kotak masuk", "inbox", "/inbox", (() => { const n = AG.NOTIFS.filter((x) => x.unread).length; return n ? `<span class="cnt">${n}</span>` : ""; })())}
  ${navItem("Tiket saya", "user", "/my")}
  ${navItem("AI Office", "building-store", "/office")}
  ${navItem("Pencarian", "search", "/search")}
  <div class="sec">Project<button data-dialog="new-project" aria-label="Project baru" class="faint">${ic("plus", "s14")}</button></div>
  ${P.map((p) => `${navItem(`<span class="grow trunc">${p.n}</span>${proj === p.k ? ic("chevron-down", "s14 faint") : ""}`, null, `/p/${p.k}`, "", `<span class="dot" style="background:${p.c}"></span>`, proj === p.k ? "exact" : "")}
    ${proj === p.k ? sub(p.k).slice(1).map(([n, i, r]) => navItem(n, i, r, "", "", "", "sub")).join("") : ""}`).join("")}
  <div class="foot">
    <a class="runner-pill" href="#/settings/runner"><span class="live"></span><span class="grow"><b class="b5">Runner menyala</b><br><span class="mut xs">MacBook Olivia · idle</span></span>${ic("chevron-right", "s14 faint")}</a>
    <button class="me row" data-menu="me">${AG.av(AG.ME, 24)}<span class="grow trunc b5" style="text-align:left">${AG.PEOPLE[AG.ME].name}</span>${ic("dots", "s14 faint")}</button>
  </div>`;
};
const SETTINGS_NAV = [
  ["Akun", [["Profil", "user", "/settings/profile"], ["Password", "lock", "/settings/password"], ["Autentikasi dua langkah", "shield-check", "/settings/2fa"], ["Notifikasi", "bell", "/settings/notifications"], ["Runner & CLI", "device-laptop", "/settings/runner"], ["Hubungkan client AI", "plug-connected", "/settings/connect"]]],
  ["Workspace", [["Umum", "settings", "/settings/general"], ["Anggota", "users", "/settings/members"], ["Tim", "users-group", "/settings/teams"], ["Jabatan", "id-badge-2", "/settings/jobs"], ["Role", "key", "/settings/roles"], ["Audit log", "list-details", "/settings/audit"]]],
  ["Kerja & AI", [["Workflow", "route", "/settings/workflows"], ["Tipe tiket & field", "category-2", "/settings/ticket-types"], ["Kode tiket", "hash", "/settings/ticket-code"], ["Persona agent", "robot", "/settings/personas"], ["Skill pack", "package", "/settings/skills"], ["Knowledge workspace", "book", "/settings/knowledge"]]],
  ["Integrasi", [["Git provider", "git-branch", "/settings/git"], ["Jira", "plug", "/settings/jira"], ["Impor CSV", "file-import", "/settings/import-csv"], ["API & webhook", "webhook", "/settings/api"], ["Ekspor data", "file-export", "/settings/export"]]],
  ["Sistem", [["Status sistem", "heart-rate-monitor", "/settings/system"]]],
];
const PROJECT_SETTINGS = (k) => [["Umum", "settings", `/settings/p/${k}/general`], ["Akses", "users", `/settings/p/${k}/access`], ["Repo", "git-branch", `/settings/p/${k}/repos`], ["Workflow", "route", `/settings/p/${k}/workflow`], ["Konvensi git", "git-commit", `/settings/p/${k}/git`], ["Components", "components", `/settings/p/${k}/components`], ["Penghemat token", "coins", `/settings/p/${k}/tokens`]];
const NAV_SETTINGS = (proj) => `
  <a class="ni" href="#/home" style="color:var(--muted);margin-bottom:6px">${ic("chevron-left")}Kembali ke aplikasi</a>
  ${SETTINGS_NAV.map(([g, items]) => `<div class="sec">${g === "Workspace" ? "Workspace · PT Contoh" : g}</div>${items.map(([n, i, r]) => navItem(n, i, r)).join("")}`).join("")}
  <div class="sec">Project</div>
  ${AG.PROJECTS.map((p) => `${navItem(`<span class="grow trunc">${p.n}</span>`, null, `/settings/p/${p.k}/general`, "", `<span class="dot" style="background:${p.c}"></span>`, "never")}
    ${proj === p.k ? PROJECT_SETTINGS(p.k).map(([n, i, r]) => navItem(n, i, r, "", "", "", "sub")).join("") : ""}`).join("")}`;
function navItem(label, icon, route, tail = "", lead = "", mode = "", cls = "") {
  const cur = AG.route.path;
  const on = mode === "never" ? false : mode === "exact" ? cur === route : cur === route || cur.startsWith(route + "/");
  return `<a class="ni ${cls} ${on ? "on" : ""}" href="#${route}"${on ? ' aria-current="page"' : ""}>${lead}${icon ? ic(icon) : ""}${label}${tail}</a>`;
}
AG.PROJECT_VIEWS = (k) => [["Ringkasan", "home", `/p/${k}`], ["Board", "layout-kanban", `/p/${k}/board`], ["Daftar", "list", `/p/${k}/list`], ["Backlog", "stack-2", `/p/${k}/backlog`], ["Roadmap", "timeline", `/p/${k}/roadmap`], ["Kalender", "calendar", `/p/${k}/calendar`], ["Rilis", "rocket", `/p/${k}/releases`], ["Laporan", "chart-line", `/p/${k}/reports`]];
/* Tab tampilan project di topbar. */
AG.projectTabs = (k) => {
  const cur = AG.route.path;
  const v = AG.PROJECT_VIEWS(k);
  const isOn = (r) => cur === r || (r !== `/p/${k}` && cur.startsWith(r + "/"));
  return { tabs: `<nav class="vtabs" aria-label="Tampilan project">${v.map(([n, i, r]) => `<a class="vtab ${isOn(r) ? "on" : ""}" href="#${r}">${ic(i, "s14")}${n}</a>`).join("")}</nav>`,
    mtabs: `<nav class="mtabs" aria-label="Tampilan project">${v.map(([n, , r]) => `<a class="vtab ${isOn(r) ? "on" : ""}" href="#${r}">${n}</a>`).join("")}</nav>` };
};
AG.projectCrumb = (k, last) => [`<a class="row g4" href="#/p/${k}">${AG.projDot(k)}${AG.project(k).n}</a>`, last];

/* shell({ nav: "app"|"settings", project, crumbs, tabs, mtabs, actions, body, flush, drawer, width })
   width: "narrow" (760, pengaturan) | "mid" (1040) | "wide" (penuh). flush: konten tanpa padding (board, editor). */
AG.shell = ({ nav = "app", project = null, crumbs = [], tabs = "", mtabs = "", actions = "", body = "", flush = false, drawer = "", width = "wide" }) => {
  const side = nav === "settings" ? NAV_SETTINGS(project) : NAV_APP(project);
  const cr = crumbs.length ? `<nav class="crumbs" aria-label="Breadcrumb">${crumbs.map((c, i) => (i === crumbs.length - 1 ? `<b class="trunc">${c}</b>` : `<span class="trunc">${c}</span><span class="sep">/</span>`)).join("")}</nav>` : "";
  return `<div class="app" id="app">
    <aside class="side" aria-label="Navigasi utama">${side}</aside>
    <div class="main">
      <header class="topbar">${AG.btn("", "ghost icon burger", "menu-2", { "data-act": "nav", "aria-label": "Buka menu" })}${cr}<span class="grow"></span></header>
      ${tabs || actions ? `<div class="pbar ${tabs ? "has-tabs" : ""}">${tabs}<span class="grow"></span>${actions ? `<div class="pacts">${actions}</div>` : ""}</div>` : ""}
      ${mtabs}
      <div class="body"><main class="content ${flush ? "flush" : ""}" id="content">${flush ? body : `<div class="${width}">${body}</div>`}</main>${drawer}</div>
    </div></div>`;
};

/* ====================== Registry ====================== */
AG._tabMem = {}; AG.SCREENS = []; AG.DIALOGS = {}; AG.MENUS = {}; AG.ACTIONS = {};
const MS_ORDER = ["Opsi", "M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8", "M9", "M10", "M11", "M12"];
/* screen({ m: "M2", g: "Tiket", n: "Board", route: "/p/:k/board", path: "/p/KSR/board", render(params) => html, after(root, params) }) */
AG.screen = (s) => { s.path = s.path || s.route; AG.SCREENS.push(s); };
AG.dialog = (name, fn) => { AG.DIALOGS[name] = fn; };
AG.menu = (name, fn) => { AG.MENUS[name] = fn; };
AG.action = (name, fn) => { AG.ACTIONS[name] = fn; };
AG.css = (text) => { const s = document.createElement("style"); s.textContent = text; document.head.appendChild(s); };
AG.MILESTONES = { Opsi: "Opsi desain (pilih dulu)", M1: "Identitas & tenancy", M2: "Tiket & board", M3: "Workflow & gate", M4: "MCP & persona", M5: "CLI & runner", M6: "Git & PR", M7: "Code graph", M8: "Knowledge base", M9: "Planning & laporan", M10: "Visual", M11: "Multi-client & integrasi", M12: "Rilis" };

function match(route, path) {
  const a = route.split("/"), b = path.split("/");
  if (a.length !== b.length) return null;
  const p = {};
  for (let i = 0; i < a.length; i++) { if (a[i].startsWith(":")) p[a[i].slice(1)] = decodeURIComponent(b[i]); else if (a[i] !== b[i]) return null; }
  return p;
}
AG.route = { path: "/home", params: {}, screen: null };
function ordered() { return [...AG.SCREENS].sort((x, y) => MS_ORDER.indexOf(x.m) - MS_ORDER.indexOf(y.m) || x._i - y._i); }

/* ====================== Overlay ====================== */
const layer = () => document.getElementById("overlay");
AG.openDialog = (name, arg) => {
  const fn = AG.DIALOGS[name]; if (!fn) return AG.toast(`Dialog “${name}” belum dibuat`, "dng");
  const d = fn(arg);
  const html = typeof d === "string" ? d : `<div class="dialog ${d.w || ""}" role="dialog" aria-modal="true" aria-labelledby="dlg-t">
      <div class="dh">${d.icon ? `<span class="tile lg ${d.tone || ""}">${ic(d.icon, "s18")}</span>` : ""}<div class="grow"><h2 id="dlg-t">${d.title}</h2>${d.desc ? `<p>${d.desc}</p>` : ""}</div>${AG.btn("", "ghost icon sm", "x", { "data-close": true, "aria-label": "Tutup" })}</div>
      <div class="db">${d.body || ""}</div>${d.foot === false ? "" : `<div class="df">${d.foot ?? AG.btn("Tutup", "", "", { "data-close": true })}</div>`}</div>`;
  const el = document.createElement("div"); el.className = "scrim"; el.innerHTML = html;
  el.addEventListener("mousedown", (e) => { if (e.target === el) closeTop(); });
  layer().appendChild(el);
  (el.querySelector("input,textarea,select") || el.querySelector("button"))?.focus();
  d.after?.(el);
};
function closeTop() { const l = layer(); if (l.lastElementChild) l.lastElementChild.remove(); }
AG.closeDialog = closeTop;
function closeMenus() { document.querySelectorAll(".menu").forEach((m) => m.remove()); }
function openMenu(el) {
  closeMenus();
  const fn = AG.MENUS[el.dataset.menu]; if (!fn) return AG.toast(`Menu “${el.dataset.menu}” belum dibuat`, "dng");
  const m = document.createElement("div"); m.className = "menu"; m.setAttribute("role", "menu"); m.innerHTML = fn(el.dataset.arg, el);
  document.body.appendChild(m);
  const r = el.getBoundingClientRect(), mw = m.offsetWidth, mh = m.offsetHeight;
  let x = r.left; if (x + mw > innerWidth - 8) x = Math.max(8, r.right - mw);
  let y = r.bottom + 4; if (y + mh > innerHeight - 8) y = Math.max(8, r.top - mh - 4);
  m.style.left = x + "px"; m.style.top = y + "px";
}
AG.toast = (text, tone = "") => {
  const t = document.createElement("div"); t.className = `toast ${tone}`; t.setAttribute("role", "status");
  t.innerHTML = `${ic(tone === "dng" ? "alert-circle" : "circle-check", "s14")}<span>${text}</span>`;
  document.getElementById("toasts").appendChild(t); setTimeout(() => t.remove(), 3200);
};

/* ====================== Render ====================== */
AG.render = () => {
  const path = (location.hash.replace(/^#/, "") || "/home").split("?")[0];
  let found = null, params = {};
  for (const s of AG.SCREENS) { const p = match(s.route, path); if (p) { found = s; params = p; break; } }
  if (!found) { found = AG.SCREENS.find((s) => s.route === "/404"); }
  AG.route = { path, params, screen: found };
  const root = document.getElementById("root");
  closeMenus(); layer().innerHTML = "";
  root.innerHTML = found ? found.render(params) : `<div style="padding:40px">Layar ${esc(path)} belum ada.</div>`;
  const list = ordered(), n = list.indexOf(found) + 1;
  document.getElementById("badge").textContent = found ? `Layar ${n} · ${found.m}` : "—";
  document.title = found ? `${found.n} · Agent Gate` : "Agent Gate";
  window.scrollTo(0, 0);
  found?.after?.(root, params);
};
AG.rerender = () => { const y = scrollY; AG.render(); scrollTo(0, y); };

/* ====================== Palet & peta layar ====================== */
function openPalette() {
  const items = [
    ...AG.TICKETS.map((t) => ({ l: `${t.k} ${t.t}`, h: `<span class="mono faint">${t.k}</span><span class="trunc">${t.t}</span>`, i: AG.TYPES[t.type].i, go: `#/t/${t.k}`, g: "Tiket" })),
    ...ordered().map((s) => ({ l: s.n + " " + s.g, h: `<span class="trunc">${s.n}</span>`, i: "arrow-right", go: "#" + s.path, g: s.g })),
  ];
  const el = document.createElement("div"); el.className = "scrim";
  el.innerHTML = `<div class="dialog palette" role="dialog" aria-modal="true" aria-label="Cari"><input class="inp pin" placeholder="Cari tiket, halaman, atau perintah…" aria-label="Cari"><div class="plist"></div><div class="df" style="justify-content:space-between"><span class="mut sm">↑↓ pilih · Enter buka · Esc tutup</span><span class="mut sm">Pencarian lanjutan: <a class="link" href="#/search">bahasa query</a></span></div></div>`;
  el.addEventListener("mousedown", (e) => { if (e.target === el) closeTop(); });
  layer().appendChild(el);
  const input = el.querySelector("input"), list = el.querySelector(".plist");
  let sel = 0, shown = [];
  const draw = () => {
    const q = input.value.toLowerCase().trim();
    shown = items.filter((x) => !q || x.l.toLowerCase().includes(q)).slice(0, 40);
    sel = Math.min(sel, Math.max(0, shown.length - 1));
    let g = "";
    list.innerHTML = shown.map((x, i) => { const h = x.g !== g ? `<div class="mlab">${(g = x.g)}</div>` : ""; return `${h}<a class="mi ${i === sel ? "hov" : ""}" href="${x.go}" data-i="${i}">${ic(x.i)}${x.h}</a>`; }).join("") || `<div class="mut" style="padding:16px">Tidak ada hasil.</div>`;
  };
  input.addEventListener("input", () => { sel = 0; draw(); });
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") { sel = Math.min(sel + 1, shown.length - 1); draw(); e.preventDefault(); }
    if (e.key === "ArrowUp") { sel = Math.max(sel - 1, 0); draw(); e.preventDefault(); }
    if (e.key === "Enter" && shown[sel]) { location.hash = shown[sel].go; closeTop(); }
  });
  list.addEventListener("click", () => setTimeout(closeTop, 0));
  draw(); input.focus();
}
function openMap() {
  const list = ordered(); const cur = AG.route.screen;
  const groups = {};
  list.forEach((s, i) => { (groups[s.m] ||= []).push([i + 1, s]); });
  AG.DIALOGS.__map = () => ({ w: "w960", title: "Peta layar", desc: `${list.length} layar, dikelompokkan per milestone. Sebut nomornya saat minta revisi.`, foot: false,
    body: `<div class="map">${MS_ORDER.filter((m) => groups[m]).map((m) => `<div><h3>${m} · ${AG.MILESTONES[m]}</h3><div class="items">${groups[m].map(([n, s]) => `<a href="#${s.path}" class="${s === cur ? "on" : ""}" data-close><span>${n}</span><span>${s.n}<br><span class="faint xs">${s.g}</span></span></a>`).join("")}</div></div>`).join("")}</div>` });
  AG.openDialog("__map");
}
function step(d) { const list = ordered(); const i = list.indexOf(AG.route.screen); const s = list[(i + d + list.length) % list.length]; location.hash = "#" + s.path; }

/* ====================== Interaksi global ====================== */
AG.action("nav", () => document.getElementById("app")?.classList.toggle("nav-open"));
AG.action("palette", () => openPalette());
AG.action("map", () => openMap());
AG.action("theme", () => {
  const r = document.documentElement; const dark = r.dataset.theme ? r.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
  r.dataset.theme = dark ? "light" : "dark"; try { localStorage.setItem("ag-proto-theme", r.dataset.theme); } catch {}
});
AG.action("prev", () => step(-1));
AG.action("next", () => step(1));

document.addEventListener("click", (e) => {
  const t = e.target;
  if (!t.closest(".menu") && !t.closest("[data-menu]")) closeMenus();
  const app = document.getElementById("app");
  if (app?.classList.contains("nav-open") && !t.closest(".side") && !t.closest(".burger")) app.classList.remove("nav-open");

  const tg = t.closest("[data-toggle]");
  if (tg) { tg.classList.toggle("on"); tg.setAttribute("aria-checked", tg.classList.contains("on")); if (tg.dataset.toast) AG.toast(tg.dataset.toast); return; }
  const rd = t.closest("[data-radio]");
  if (rd) { const g = rd.dataset.radio; document.querySelectorAll(`[data-radio="${g}"]`).forEach((x) => x.classList.toggle("on", x === rd)); document.querySelectorAll(`[data-radio="${g}"] .radio`).forEach((x) => x.classList.toggle("on", x.closest("[data-radio]") === rd)); }
  const sg = t.closest(".seg > *");
  if (sg && !sg.dataset.go) { [...sg.parentElement.children].forEach((x) => x.classList.toggle("on", x === sg)); }
  const tab = t.closest("[data-tab]");
  if (tab) {
    if (tab.matches('a[href="#"]')) e.preventDefault();
    const root = tab.closest("[data-tabs-root]") || document;
    const roots = [...document.querySelectorAll("[data-tabs-root]")];
    (AG._tabMem[location.hash] = AG._tabMem[location.hash] || {})[roots.indexOf(root)] = tab.dataset.tab;
    const own = (x) => root === document || x.closest("[data-tabs-root]") === root;
    root.querySelectorAll("[data-tab]").forEach((x) => own(x) && x.classList.toggle("on", x.dataset.tab === tab.dataset.tab));
    root.querySelectorAll("[data-panel]").forEach((x) => own(x) && x.classList.toggle("on", x.dataset.panel === tab.dataset.tab));
  }
  const menu = t.closest("[data-menu]");
  if (menu) { e.preventDefault(); e.stopPropagation(); const open = document.querySelector(".menu"); if (open && menu._open) { closeMenus(); menu._open = false; } else { openMenu(menu); menu._open = true; } return; }
  const act = t.closest("[data-act]");
  if (act) { e.preventDefault(); const fn = AG.ACTIONS[act.dataset.act]; fn ? fn(act, act.dataset.arg, e) : AG.toast(`Aksi “${act.dataset.act}” belum dibuat`, "dng"); }
  const dlg = t.closest("[data-dialog]");
  if (dlg) { e.preventDefault(); if (t.closest(".menu")) closeMenus(); AG.openDialog(dlg.dataset.dialog, dlg.dataset.arg); }
  const toast = t.closest("[data-toast]");
  if (toast && !tg) AG.toast(toast.dataset.toast, toast.dataset.tone || "");
  const go = t.closest("[data-go]");
  if (go) { const inner = t.closest("a[href],button,[data-toggle],[data-act],[data-dialog],input,select,textarea"); if (!inner || inner === go || !go.contains(inner)) location.hash = go.dataset.go; }
  if (t.closest("[data-close]")) { closeTop(); }
  if (t.closest(".menu .mi") && !t.closest("[data-menu]")) closeMenus();
});
document.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") { e.preventDefault(); openPalette(); return; }
  if (e.key === "Escape") { if (document.querySelector(".menu")) closeMenus(); else closeTop(); document.getElementById("app")?.classList.remove("nav-open"); return; }
  const typing = /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName) || document.activeElement?.isContentEditable;
  if (typing || layer().children.length) return;
  if (e.key === "]") step(1);
  if (e.key === "[") step(-1);
});
window.addEventListener("hashchange", AG.render);

/* Boot dipanggil setelah semua modul layar dimuat. */
AG.boot = () => {
  AG.SCREENS.forEach((s, i) => (s._i = i));
  try { const t = localStorage.getItem("ag-proto-theme"); if (t) document.documentElement.dataset.theme = t; } catch {}
  document.body.insertAdjacentHTML("beforeend", `<div id="overlay"></div><div class="toasts" id="toasts"></div>
    <div class="fab" aria-label="Kontrol prototype">
      <span class="screen-badge" id="badge"></span>
      ${AG.btn("", "icon", "chevron-left", { "data-act": "prev", "aria-label": "Layar sebelumnya ([)", title: "Layar sebelumnya ([)" })}
      ${AG.btn("", "icon", "chevron-right", { "data-act": "next", "aria-label": "Layar berikutnya (])", title: "Layar berikutnya (])" })}
      ${AG.btn("", "icon", "moon", { "data-act": "theme", "aria-label": "Ganti tema terang/gelap", title: "Ganti tema" })}
      ${AG.btn("Peta layar", "pri", "map-2", { "data-act": "map" })}
    </div>`);
  AG.render();
};
})();

/* Menu bersama di sidebar. */
(() => {
const { ic } = AG;
AG.menu("workspace", () => `<div class="mlab">Workspace</div>
  <a class="mi hov" href="#/home"><span class="wslogo">PC</span>PT Contoh<span class="tail">${ic("check", "s14")}</span></a>
  <a class="mi" href="#/workspaces"><span class="wslogo" style="background:#0f8a5f">BL</span>Beta Labs</a>
  <div class="msep"></div>
  <a class="mi" href="#/settings/general">${ic("settings")}Pengaturan workspace</a>
  <a class="mi" href="#/settings/members">${ic("users-plus")}Undang anggota</a>
  <a class="mi" href="#/workspaces/new">${ic("plus")}Buat workspace</a>`);
AG.menu("me", () => `<div class="mlab">owner@agentgate.test</div>
  <a class="mi" href="#/settings/profile">${ic("user")}Profil</a>
  <a class="mi" href="#/settings/runner">${ic("device-laptop")}Runner & CLI</a>
  <button class="mi" data-act="theme">${ic("moon")}Ganti tema</button>
  <div class="msep"></div>
  <button class="mi" data-dialog="leave-workspace">${ic("door-exit")}Keluar dari workspace</button>
  <a class="mi" href="#/login">${ic("logout")}Keluar akun</a>`);
})();

/* ====================== v2: komponen tambahan ====================== */
(() => {
const { ic, esc } = AG;
let uid = 0;

/* Bar simpan di bawah halaman. AG.savebar("Simpan perubahan", { msg: "Ada 2 perubahan belum disimpan", extra: "", toast: "Tersimpan" }) */
AG.savebar = (label = "Simpan perubahan", { msg = "", extra = "", toast = "Perubahan disimpan", cancel = true, act = "", arg = "" } = {}) =>
  `<div class="savebar">${msg ? `<span class="msg">${ic("info-circle", "s14")}${msg}</span>` : ""}${extra}${cancel ? AG.btn("Batal", "ghost", "", { "data-toast": "Perubahan dibatalkan" }) : ""}${AG.btn(label, "pri", "check", act ? { "data-act": act, "data-arg": arg } : { "data-toast": toast })}</div>`;

/* Combobox / select yang bisa dicari.
   AG.combo({ options: [{ v, l, sub, lead, group }], value, ph, multi, w, name }) */
AG._combos = {};
AG.combo = ({ options = [], value = null, ph = "Pilih…", multi = false, w = 0, empty = "Tidak ada yang cocok", footer = "" } = {}) => {
  const id = "cb" + ++uid;
  AG._combos[id] = { options: options.map((o) => (typeof o === "string" ? { v: o, l: o } : o)), value: multi ? [].concat(value || []) : value, multi, ph, empty, footer, w };
  return `<button type="button" class="inp combo" data-combo="${id}" aria-haspopup="listbox">${comboLabel(id)}${ic("selector", "s14")}</button>`;
};
function comboLabel(id) {
  const c = AG._combos[id];
  if (c.multi) {
    const sel = c.options.filter((o) => c.value.includes(o.v));
    return sel.length ? `<span class="val">${sel.slice(0, 3).map((o) => AG.pill(o.l, "gray")).join("")}${sel.length > 3 ? `<span class="mut sm">+${sel.length - 3}</span>` : ""}</span>` : `<span class="val ph">${esc(c.ph)}</span>`;
  }
  const o = c.options.find((x) => x.v === c.value);
  return o ? `<span class="val">${o.lead || ""}${esc(o.l)}</span>` : `<span class="val ph">${esc(c.ph)}</span>`;
}
function placeMenu(m, el) {
  const r = el.getBoundingClientRect(), mw = m.offsetWidth, mh = m.offsetHeight;
  let x = r.left; if (x + mw > innerWidth - 8) x = Math.max(8, r.right - mw);
  let y = r.bottom + 4; if (y + mh > innerHeight - 8) y = Math.max(8, r.top - mh - 4);
  m.style.left = x + "px"; m.style.top = y + "px";
}
function openCombo(btn) {
  document.querySelectorAll(".menu").forEach((m) => m.remove());
  const id = btn.dataset.combo, c = AG._combos[id];
  const m = document.createElement("div"); m.className = "menu cbx"; m.setAttribute("role", "listbox");
  m.style.setProperty("--w", Math.max(c.w || 0, btn.offsetWidth) + "px");
  m.innerHTML = `<div class="cs">${ic("search", "s14")}<input placeholder="Cari…" aria-label="Cari pilihan"></div><div class="cl"></div><div class="cf"><span class="cnt"></span>${c.footer || ""}</div>`;
  document.body.appendChild(m);
  const input = m.querySelector("input"), list = m.querySelector(".cl");
  let hov = 0, shown = [];
  const draw = () => {
    const q = input.value.toLowerCase().trim();
    shown = c.options.filter((o) => !q || (o.l + " " + (o.sub || "")).toLowerCase().includes(q));
    hov = Math.min(hov, Math.max(0, shown.length - 1));
    let g = null;
    list.innerHTML = shown.map((o, i) => {
      const on = c.multi ? c.value.includes(o.v) : c.value === o.v;
      const head = o.group && o.group !== g ? `<div class="mlab">${esc((g = o.group))}</div>` : "";
      return `${head}<button type="button" class="mi ${i === hov ? "hov" : ""}" data-i="${i}" role="option" aria-selected="${on}">${c.multi ? `<span class="cb ${on ? "on" : ""}">${ic("check")}</span>` : ""}${o.lead || ""}<span class="grow" style="text-align:left">${esc(o.l)}${o.sub ? `<small>${esc(o.sub)}</small>` : ""}</span>${!c.multi && on ? ic("check", "s14") : ""}</button>`;
    }).join("") || `<div class="mut" style="padding:12px">${esc(c.empty)}</div>`;
    m.querySelector(".cnt").textContent = `${shown.length} dari ${c.options.length}`;
  };
  const pick = (o) => {
    if (c.multi) { c.value = c.value.includes(o.v) ? c.value.filter((x) => x !== o.v) : [...c.value, o.v]; draw(); }
    else { c.value = o.v; m.remove(); }
    btn.innerHTML = comboLabel(id) + ic("selector", "s14");
    btn.dispatchEvent(new CustomEvent("combo-change", { bubbles: true, detail: c.value }));
  };
  list.addEventListener("click", (e) => { const b = e.target.closest("[data-i]"); if (b) { e.stopPropagation(); pick(shown[+b.dataset.i]); } });
  input.addEventListener("input", () => { hov = 0; draw(); });
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") { hov = Math.min(hov + 1, shown.length - 1); draw(); e.preventDefault(); }
    if (e.key === "ArrowUp") { hov = Math.max(hov - 1, 0); draw(); e.preventDefault(); }
    if (e.key === "Enter" && shown[hov]) { e.preventDefault(); pick(shown[hov]); }
  });
  draw(); placeMenu(m, btn); input.focus();
}

/* Pemilih warna dengan warna custom. AG.colorPicker("#7c3aed") */
AG.COLORS = ["#7c3aed", "#5b5bd6", "#2563eb", "#0ea5e9", "#0891b2", "#0d9488", "#0f8a5f", "#65a30d", "#ca8a04", "#f59e0b", "#ea580c", "#dc2626", "#e11d48", "#db2777", "#c026d3", "#64748b", "#475569", "#1f2937"];
AG.colorPicker = (value = AG.COLORS[0], colors = AG.COLORS) => {
  const custom = !colors.includes(value);
  return `<div class="cpick" data-cp>${colors.map((c) => `<button type="button" class="c ${c === value ? "on" : ""}" style="background:${c}" data-cpc="${c}" aria-label="Warna ${c}"></button>`).join("")}
    <label class="custom ${custom ? "on" : ""}" title="Warna custom"><input type="color" value="${custom ? value : "#5b5bd6"}" aria-label="Pilih warna custom"><input type="text" value="${custom ? value : "Custom"}" aria-label="Kode warna hex"></label></div>`;
};

/* Editor rich text lengkap (WYSIWYG ⇄ Markdown).
   AG.editor({ html, ph, h: "180px", full: false, compact: false, footer: "" }) */
AG.editor = ({ html = "", ph = "Tulis sesuatu… Ketik / untuk menyisipkan blok", h = "", full = false, compact = false, footer = "", save = false } = {}) => {
  if (save) footer = `${footer}${AG.btn("Batal", "ghost sm", "", { "data-toast": "Perubahan dibatalkan" })}${AG.btn(save === true ? "Simpan" : save, "pri sm", "check", { "data-toast": "Perubahan disimpan" })}`;
  const b = (cmd, icon, title, extra = "") => `<button type="button" data-ed="${cmd}" title="${title}" aria-label="${title}"${extra}>${ic(icon, "s14")}</button>`;
  const sep = `<span class="sep"></span>`;
  const tb = compact
    ? [b("bold", "bold", "Tebal (Ctrl B)"), b("italic", "italic", "Miring (Ctrl I)"), b("strikeThrough", "strikethrough", "Coret"), sep, b("insertUnorderedList", "list", "Daftar"), b("insertOrderedList", "list-numbers", "Daftar bernomor"), b("task", "list-check", "Checklist"), sep, b("code", "code", "Kode"), b("link", "link", "Tautan"), b("mention", "at", "Sebut orang atau tiket"), b("emoji", "mood-smile", "Emoji"), b("attach", "paperclip", "Lampiran")].join("")
    : [b("undo", "arrow-back-up", "Urungkan (Ctrl Z)"), b("redo", "arrow-forward-up", "Ulangi"), sep,
       `<button type="button" data-ed="block" title="Gaya paragraf" aria-label="Gaya paragraf" style="padding:0 8px">Teks${ic("chevron-down", "s12")}</button>`, sep,
       b("bold", "bold", "Tebal (Ctrl B)"), b("italic", "italic", "Miring (Ctrl I)"), b("underline", "underline", "Garis bawah (Ctrl U)"), b("strikeThrough", "strikethrough", "Coret"), b("code", "code", "Kode inline"), b("highlight", "highlight", "Sorot"), sep,
       b("insertUnorderedList", "list", "Daftar"), b("insertOrderedList", "list-numbers", "Daftar bernomor"), b("task", "list-check", "Checklist"), b("quote", "blockquote", "Kutipan"), sep,
       b("link", "link", "Tautan (Ctrl K)"), b("image", "photo", "Gambar"), b("attach", "paperclip", "Lampiran"), b("table", "table", "Tabel"), b("codeblock", "file-code", "Blok kode"), b("mermaid", "sitemap", "Diagram Mermaid"), b("hr", "separator", "Garis pemisah"), sep,
       b("mention", "at", "Sebut orang atau tiket"), b("emoji", "mood-smile", "Emoji"), b("insert", "plus", "Sisipkan blok ( / )"),
       `<span class="grow"></span>`, b("md", "markdown", "Mode Markdown"), full ? "" : b("full", "arrows-maximize", "Layar penuh")].join("");
  return `<div class="ed ${full ? "full" : ""}" style="${h ? `--h:${h}` : ""}">
    <div class="tb" role="toolbar" aria-label="Format teks">${tb}</div>
    <div class="area" contenteditable="true" data-ph="${esc(ph)}" role="textbox" aria-multiline="true">${html}</div>
    <textarea class="md" aria-label="Markdown"></textarea>
    ${compact ? "" : `<div class="foot">${ic("markdown", "s14")}Markdown didukung · ketik <kbd>/</kbd> untuk blok, <kbd>@</kbd> untuk menyebut${footer ? `<span class="grow"></span>${footer}` : ""}</div>`}
  </div>`;
};
function htmlToMd(el) {
  const walk = (n) => {
    if (n.nodeType === 3) return n.textContent;
    const k = [...n.childNodes].map(walk).join("");
    switch (n.tagName) {
      case "H1": return `# ${k}\n\n`; case "H2": return `## ${k}\n\n`; case "H3": return `### ${k}\n\n`;
      case "P": case "DIV": return `${k}\n\n`; case "BR": return "\n";
      case "STRONG": case "B": return `**${k}**`; case "EM": case "I": return `_${k}_`; case "S": case "STRIKE": return `~~${k}~~`;
      case "CODE": return n.parentElement?.tagName === "PRE" ? k : "`" + k + "`";
      case "PRE": return "```" + (n.dataset.lang || "") + "\n" + k.replace(/\n$/, "") + "\n```\n\n";
      case "BLOCKQUOTE": return k.trim().split("\n").map((l) => "> " + l).join("\n") + "\n\n";
      case "UL": return [...n.children].map((li) => (li.classList.contains("task") ? `- [${li.querySelector(".cb.on") ? "x" : " "}] ` : "- ") + walk(li).trim()).join("\n") + "\n\n";
      case "OL": return [...n.children].map((li, i) => `${i + 1}. ` + walk(li).trim()).join("\n") + "\n\n";
      case "A": return `[${k}](${n.getAttribute("href") || "#"})`;
      case "HR": return "---\n\n";
      case "TABLE": { const rows = [...n.querySelectorAll("tr")].map((tr) => "| " + [...tr.children].map((c) => c.textContent.trim()).join(" | ") + " |"); if (rows.length) rows.splice(1, 0, "|" + " --- |".repeat(n.querySelector("tr").children.length)); return rows.join("\n") + "\n\n"; }
      case "BUTTON": return "";
      default: return k;
    }
  };
  return walk(el).replace(/\n{3,}/g, "\n\n").trim() + "\n";
}
function mdToHtml(md) {
  const inline = (s) => { const codes = []; return esc(s).replace(/`(.+?)`/g, (m, c) => `\u0000${codes.push(c) - 1}\u0000`).replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>").replace(/(^|[^\w])_(.+?)_(?!\w)/g, "$1<em>$2</em>").replace(/~~(.+?)~~/g, "<s>$1</s>").replace(/\u0000(\d+)\u0000/g, (m, n) => `<code>${codes[n]}</code>`).replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2">$1</a>'); }
  const out = []; const lines = md.split("\n"); let i = 0;
  while (i < lines.length) {
    const l = lines[i];
    if (l.startsWith("```")) { const lang = l.slice(3); const buf = []; i++; while (i < lines.length && !lines[i].startsWith("```")) buf.push(lines[i++]); out.push(`<pre data-lang="${esc(lang)}"><code>${esc(buf.join("\n"))}</code></pre>`); i++; continue; }
    if (/^#{1,3} /.test(l)) { const n = l.match(/^#+/)[0].length; out.push(`<h${n}>${inline(l.slice(n + 1))}</h${n}>`); i++; continue; }
    if (/^- \[[ x]\] /.test(l)) { const buf = []; while (i < lines.length && /^- \[[ x]\] /.test(lines[i])) { const on = lines[i][3] === "x"; buf.push(`<li class="task"><button type="button" class="cb ${on ? "on" : ""}" data-toggle contenteditable="false">${ic("check")}</button><span>${inline(lines[i].slice(6))}</span></li>`); i++; } out.push(`<ul>${buf.join("")}</ul>`); continue; }
    if (/^- /.test(l)) { const buf = []; while (i < lines.length && /^- /.test(lines[i])) buf.push(`<li>${inline(lines[i++].slice(2))}</li>`); out.push(`<ul>${buf.join("")}</ul>`); continue; }
    if (/^\d+\. /.test(l)) { const buf = []; while (i < lines.length && /^\d+\. /.test(lines[i])) buf.push(`<li>${inline(lines[i++].replace(/^\d+\. /, ""))}</li>`); out.push(`<ol>${buf.join("")}</ol>`); continue; }
    if (l.startsWith("> ")) { const buf = []; while (i < lines.length && lines[i].startsWith("> ")) buf.push(inline(lines[i++].slice(2))); out.push(`<blockquote>${buf.join("<br>")}</blockquote>`); continue; }
    if (l.startsWith("|")) { const rows = []; while (i < lines.length && lines[i].startsWith("|")) { if (!/^\|\s*-/.test(lines[i])) rows.push(lines[i].split("|").slice(1, -1).map((c) => c.trim())); i++; } out.push(`<table>${rows.map((r, ri) => `<tr>${r.map((c) => (ri ? `<td>${inline(c)}</td>` : `<th>${inline(c)}</th>`)).join("")}</tr>`).join("")}</table>`); continue; }
    if (l.trim() === "---") { out.push("<hr>"); i++; continue; }
    if (l.trim() === "") { i++; continue; }
    out.push(`<p>${inline(l)}</p>`); i++;
  }
  return out.join("");
}
AG.mdToHtml = mdToHtml;
AG.htmlToMd = htmlToMd;
const EMOJI = ["👍", "🎉", "✅", "❤️", "🚀", "👀", "🙏", "😄", "🤔", "🔥", "⚠️", "💡"];
AG.EMOJI = EMOJI;
function edInsert(area, html) { area.focus(); document.execCommand("insertHTML", false, html); }
function edMenu(btn, items) {
  document.querySelectorAll(".menu").forEach((m) => m.remove());
  const m = document.createElement("div"); m.className = "menu"; m.innerHTML = items; document.body.appendChild(m); placeMenu(m, btn); return m;
}
function edCommand(ed, cmd, btn) {
  const area = ed.querySelector(".area");
  if (cmd === "md") {
    const md = ed.querySelector(".md");
    if (ed.classList.toggle("mdmode")) { md.value = htmlToMd(area); md.focus(); } else { area.innerHTML = mdToHtml(md.value); }
    btn.classList.toggle("on", ed.classList.contains("mdmode")); return;
  }
  if (ed.classList.contains("mdmode")) return AG.toast("Kembali ke mode visual untuk memakai toolbar");
  area.focus();
  const ex = (c, v = null) => document.execCommand(c, false, v);
  switch (cmd) {
    case "undo": case "redo": case "bold": case "italic": case "underline": case "strikeThrough": case "insertUnorderedList": case "insertOrderedList": ex(cmd); break;
    case "quote": ex("formatBlock", "blockquote"); break;
    case "codeblock": edInsert(area, `<pre><code>// tulis kode di sini</code></pre><p></p>`); break;
    case "code": { const s = getSelection().toString() || "kode"; edInsert(area, `<code>${esc(s)}</code>&nbsp;`); break; }
    case "highlight": ex("hiliteColor", "#fff3a3"); break;
    case "hr": ex("insertHorizontalRule"); break;
    case "task": edInsert(area, `<ul><li class="task"><button type="button" class="cb" data-toggle contenteditable="false">${ic("check")}</button><span>Tugas baru</span></li></ul>`); break;
    case "table": edInsert(area, `<table><tr><th>Kolom 1</th><th>Kolom 2</th><th>Kolom 3</th></tr><tr><td>&nbsp;</td><td>&nbsp;</td><td>&nbsp;</td></tr><tr><td>&nbsp;</td><td>&nbsp;</td><td>&nbsp;</td></tr></table><p></p>`); break;
    case "mermaid": edInsert(area, `<pre data-lang="mermaid"><code>flowchart LR\n  Kasir --> API --> Database</code></pre><p></p>`); break;
    case "link": { const s = getSelection().toString() || "tautan"; edInsert(area, `<a href="#">${esc(s)}</a>&nbsp;`); AG.toast("Tautan disisipkan. Tempel URL atau ketik kode tiket."); break; }
    case "image": edInsert(area, `<p><span class="pill gray">${ic("photo")}screenshot-kasir.png · 248 KB</span></p>`); AG.toast("Gambar diunggah (contoh)"); break;
    case "attach": edInsert(area, `<span class="pill gray">${ic("paperclip")}spesifikasi-qris.pdf</span>&nbsp;`); AG.toast("Lampiran ditambahkan (contoh)"); break;
    case "full": AG.toast("Mode layar penuh dibuka di halaman editor"); break;
    case "mention": case "emoji": case "block": case "insert": {
      const sel = getSelection(); const range = sel.rangeCount ? sel.getRangeAt(0).cloneRange() : null;
      const restore = () => { area.focus(); if (range) { sel.removeAllRanges(); sel.addRange(range); } };
      let items = "";
      if (cmd === "mention") items = `<div class="mlab">Orang</div>${Object.entries(AG.PEOPLE).map(([k, p]) => `<button class="mi" data-ins="<span class=&quot;mention&quot;>@${esc(p.name)}</span>&nbsp;">${AG.av(k, 18)}${p.name}</button>`).join("")}<div class="mlab">Agent</div>${["qa", "dev", "reviewer"].map((k) => `<button class="mi" data-ins="<span class=&quot;mention&quot;>@agent-${k}</span>&nbsp;">${AG.av(k, 18)}@agent-${k}</button>`).join("")}<div class="mlab">Tiket</div>${AG.TICKETS.slice(1, 5).map((t) => `<button class="mi" data-ins="<span class=&quot;mention&quot;>${t.k}</span>&nbsp;">${AG.typeIcon(t.type)}<span class="mono mut">${t.k}</span><span class="trunc">${esc(t.t)}</span></button>`).join("")}`;
      if (cmd === "emoji") items = `<div style="display:grid;grid-template-columns:repeat(6,32px);gap:2px;padding:4px">${EMOJI.map((e) => `<button class="mi" style="justify-content:center;padding:0;font-size:16px" data-ins="${e}">${e}</button>`).join("")}</div>`;
      if (cmd === "block") items = [["p", "Teks biasa"], ["h1", "Judul 1"], ["h2", "Judul 2"], ["h3", "Judul 3"], ["blockquote", "Kutipan"], ["pre", "Blok kode"]].map(([t, l]) => `<button class="mi" data-fmt="${t}">${l}</button>`).join("");
      if (cmd === "insert") items = [["table", "table", "Tabel"], ["task", "list-check", "Checklist"], ["codeblock", "file-code", "Blok kode"], ["mermaid", "sitemap", "Diagram Mermaid"], ["image", "photo", "Gambar"], ["attach", "paperclip", "Lampiran"], ["hr", "separator", "Garis pemisah"], ["quote", "blockquote", "Kutipan"]].map(([c, i, l]) => `<button class="mi" data-sub="${c}">${ic(i)}${l}</button>`).join("");
      const m = edMenu(btn, items);
      m.addEventListener("click", (e) => {
        const x = e.target.closest("[data-ins],[data-fmt],[data-sub]"); if (!x) return; e.stopPropagation(); m.remove(); restore();
        if (x.dataset.ins) edInsert(area, x.dataset.ins);
        if (x.dataset.fmt) document.execCommand("formatBlock", false, x.dataset.fmt);
        if (x.dataset.sub) edCommand(ed, x.dataset.sub, btn);
      });
      break;
    }
  }
}

/* Tab tampilan project: tab yang disematkan + menu "Lainnya" + atur urutan. */
const VIEW_KEY = "ag-proto-views";
const allViews = (k) => AG.PROJECT_VIEWS(k);
AG.STATE.views = (() => { try { return JSON.parse(localStorage.getItem(VIEW_KEY)) || null; } catch { return null; } })() || ["Ringkasan", "Board", "Daftar", "Backlog"];
AG.projectTabs = (k) => {
  const cur = AG.route.path;
  const isOn = (r) => cur === r || (r !== `/p/${k}` && cur.startsWith(r + "/"));
  const v = allViews(k);
  const pinned = AG.STATE.views.map((n) => v.find((x) => x[0] === n)).filter(Boolean);
  const active = v.find(([, , r]) => isOn(r));
  const shown = active && !pinned.includes(active) ? [...pinned, active] : pinned;
  const rest = v.filter((x) => !shown.includes(x));
  return {
    tabs: `<nav class="vtabs" aria-label="Tampilan project">${shown.map(([n, i, r]) => `<a class="vtab ${isOn(r) ? "on" : ""}" href="#${r}">${ic(i, "s14")}${n}</a>`).join("")}<button class="vtab" data-menu="views" data-arg="${k}">${rest.length ? `Lainnya <span class="faint">${rest.length}</span>` : ""}${ic("chevron-down", "s14")}</button></nav>`,
    mtabs: `<nav class="mtabs" aria-label="Tampilan project">${v.map(([n, , r]) => `<a class="vtab ${isOn(r) ? "on" : ""}" href="#${r}">${n}</a>`).join("")}</nav>`,
  };
};
AG.menu("views", (k) => {
  const v = allViews(k); const rest = v.filter((x) => !AG.STATE.views.includes(x[0]));
  return `${rest.length ? `<div class="mlab">Tampilan lain</div>${rest.map(([n, i, r]) => `<a class="mi" href="#${r}">${ic(i)}${n}</a>`).join("")}<div class="msep"></div>` : ""}<button class="mi" data-dialog="views" data-arg="${k}">${ic("adjustments-horizontal")}Atur tab tampilan…</button>`;
});
AG.dialog("views", (k) => {
  const v = allViews(k);
  const order = [...AG.STATE.views, ...v.map((x) => x[0]).filter((n) => !AG.STATE.views.includes(n))];
  return { title: "Atur tab tampilan", desc: "Pilih tab yang tampil di bar atas dan urutannya. Sisanya masuk menu Lainnya. Berlaku untuk akun Anda.", icon: "adjustments-horizontal", w: "w560",
    body: `<div class="box" id="views-list">${order.map((n) => { const x = v.find((y) => y[0] === n); return `<div class="lrow" data-view="${n}" draggable="true" style="min-height:44px;cursor:grab">${ic("grip-vertical", "faint")}${ic(x[1], "mut")}<span class="grow b5">${n}</span>${AG.btn("", "ghost icon sm", "chevron-up", { "data-act": "view-move", "data-arg": "-1", "aria-label": "Naikkan " + n })}${AG.btn("", "ghost icon sm", "chevron-down", { "data-act": "view-move", "data-arg": "1", "aria-label": "Turunkan " + n })}${AG.sw(AG.STATE.views.includes(n), "Tampilkan " + n)}</div>`; }).join("")}</div>`,
    foot: `${AG.btn("Batal", "", "", { "data-close": true })}${AG.btn("Simpan", "pri", "check", { "data-act": "views-save", "data-arg": k })}`,
    after: (el) => {
      const list = el.querySelector("#views-list"); let drag = null;
      list.addEventListener("dragstart", (e) => { drag = e.target.closest("[data-view]"); });
      list.addEventListener("dragover", (e) => { e.preventDefault(); const o = e.target.closest("[data-view]"); if (o && drag && o !== drag) { const r = o.getBoundingClientRect(); list.insertBefore(drag, e.clientY < r.top + r.height / 2 ? o : o.nextSibling); } });
    } };
});
AG.action("view-move", (el, d) => { const row = el.closest("[data-view]"); const sib = +d < 0 ? row.previousElementSibling : row.nextElementSibling; if (sib) row.parentElement.insertBefore(row, +d < 0 ? sib : sib.nextSibling); });
AG.action("views-save", () => {
  const rows = [...document.querySelectorAll("#views-list [data-view]")];
  const pinned = rows.filter((r) => r.querySelector(".sw.on")).map((r) => r.dataset.view);
  if (!pinned.length) return AG.toast("Pilih minimal satu tab", "dng");
  AG.STATE.views = pinned; try { localStorage.setItem(VIEW_KEY, JSON.stringify(pinned)); } catch {}
  AG.closeDialog(); AG.rerender(); AG.toast("Tab tampilan disimpan");
});

/* Event: combobox, editor, pemilih warna. */
document.addEventListener("click", (e) => {
  const cb = e.target.closest("[data-combo]"); if (cb) { e.preventDefault(); openCombo(cb); return; }
  const eb = e.target.closest(".ed .tb [data-ed]"); if (eb) { e.preventDefault(); edCommand(eb.closest(".ed"), eb.dataset.ed, eb); return; }
  const cp = e.target.closest("[data-cpc]");
  if (cp) { const w = cp.closest("[data-cp]"); w.querySelectorAll(".c").forEach((x) => x.classList.toggle("on", x === cp)); w.querySelector(".custom").classList.remove("on"); w.dispatchEvent(new CustomEvent("color-change", { bubbles: true, detail: cp.dataset.cpc })); }
});
document.addEventListener("mousedown", (e) => { if (e.target.closest(".ed .tb button")) e.preventDefault(); });
document.addEventListener("input", (e) => {
  const w = e.target.closest("[data-cp]"); if (!w) return;
  const [col, txt] = w.querySelectorAll(".custom input");
  if (e.target === col) txt.value = col.value; else if (/^#[0-9a-f]{6}$/i.test(txt.value)) col.value = txt.value; else return;
  w.querySelectorAll(".c").forEach((x) => x.classList.remove("on")); w.querySelector(".custom").classList.add("on");
  w.dispatchEvent(new CustomEvent("color-change", { bubbles: true, detail: col.value }));
});
})();

/* ====================== v2: komponen dengan varian (dipilih di layar Opsi desain) ====================== */
(() => {
const { ic, esc } = AG;
const UI_KEY = "ag-proto-ui-v4";
AG.UI = Object.assign({ sec: "A", office: "A1", shell: "A", stat: "A", subnav: "E", table: "A", board: "C", prio: "C", auth: "B" }, (() => { try { return JSON.parse(localStorage.getItem(UI_KEY)) || {}; } catch { return {}; } })());
AG.setUI = (k, v) => { AG.UI[k] = v; try { localStorage.setItem(UI_KEY, JSON.stringify(AG.UI)); } catch {} document.documentElement.dataset[k + "V"] = v; };
Object.entries(AG.UI).forEach(([k, v]) => (document.documentElement.dataset[k + "V"] = v));

/* ---------- Ikon prioritas ---------- */
const PRIO = {
  A: { 1: ["chevrons-up", "#dc2626"], 2: ["chevron-up", "#ea580c"], 3: ["equal", "#ca8a04"], 4: ["chevron-down", "#2563eb"] },
  B: { 1: ["alert-triangle-filled", "#dc2626"], 2: ["antenna-bars-5", "#ea580c"], 3: ["antenna-bars-4", "#ca8a04"], 4: ["antenna-bars-3", "#64748b"] },
};
AG.prioIcon = (p, cls = "s14", withLabel = false) => {
  const x = AG.PRIORITY[p], v = AG.UI.prio;
  if (v === "C") return `<span class="prio-pill p${p}" title="Prioritas ${x.n}">P${p}${withLabel ? " " + x.n : ""}</span>`;
  if (v === "B") {
    const c = PRIO.B[p][1];
    const svg = p === 1 ? `<svg class="prio-ic" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><rect x="1" y="1" width="14" height="14" rx="3" fill="${c}"/><rect x="7.1" y="3.6" width="1.8" height="5.6" rx=".9" fill="#fff"/><circle cx="8" cy="11.6" r="1.1" fill="#fff"/></svg>`
      : `<svg class="prio-ic" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">${[0, 1, 2].map((n) => `<rect x="${2 + n * 4.6}" y="${10 - n * 3.5}" width="3" height="${4 + n * 3.5}" rx="1" fill="${n < 5 - p ? c : "var(--line2)"}"/>`).join("")}</svg>`;
    return `<span class="row g4" style="display:inline-flex;flex-shrink:0;vertical-align:middle" title="Prioritas ${x.n}">${svg}${withLabel ? `<span style="color:var(--fg2)">${x.n}</span>` : ""}</span>`;
  }
  const [i, c] = PRIO[v][p];
  return `<span class="row g4" style="display:inline-flex;color:${c};flex-shrink:0" title="Prioritas ${x.n}">${ic(i, cls + " prio-ic")}${withLabel ? `<span style="color:var(--fg2)">${x.n}</span>` : ""}</span>`;
};
AG.prioLabel = (p) => AG.prioIcon(p, "s14", true);

/* ---------- Kartu angka ---------- */
/* AG.stats([{ label, value, icon, hint, tone: "ok|warn|dng|acc", go, delta }]) */
AG.stats = (items) => `<div class="st st-${AG.UI.stat}" style="--n:${items.length}">${items.map((x) => {
  const tag = x.go ? `a href="${x.go}"` : "div";
  const end = x.go ? "a" : "div";
  return `<${tag} class="st-i ${x.tone ? "t-" + x.tone : ""}">
    ${x.icon ? `<span class="st-ic st-ic-b">${ic(x.icon, "s20")}</span>` : ""}<div class="st-h">${x.icon ? `<span class="st-ic">${ic(x.icon, "s16")}</span>` : ""}<span class="st-l">${x.label}</span>${x.go ? ic("arrow-up-right", "s14 st-go") : ""}</div>
    <div class="st-v">${x.value}${x.delta ? `<span class="st-d">${x.delta}</span>` : ""}</div>
    ${x.hint ? `<div class="st-hint">${x.hint}</div>` : ""}</${end}>`;
}).join("")}</div>`;

/* ---------- Halaman dengan menu tab vertikal ---------- */
/* AG.subpage({ nav: [{ k, label, icon, count, desc, go, group }], active, body, title, desc, aside })
   - Item dengan `go` berpindah rute; tanpa `go` berganti panel lokal: body berisi elemen data-panel="k". */
AG.subpage = ({ nav = [], active, body = "", title = "", desc = "", head = "" }) => {
  let g = null;
  const items = nav.map((n) => {
    const grp = n.group && n.group !== g ? `<div class="sp-g">${(g = n.group)}</div>` : "";
    const attrs = n.go ? `href="${n.go}"` : `href="#" data-tab="${n.k}"`;
    return `${grp}<a class="sp-i ${n.k === active ? "on" : ""}" ${attrs}>${n.icon ? ic(n.icon, "s16") : ""}<span class="sp-t"><span>${n.label}</span>${n.desc ? `<small>${n.desc}</small>` : ""}</span>${n.count != null ? `<span class="sp-c ${n.alert ? "alert" : ""}">${n.count}</span>` : ""}</a>`;
  }).join("");
  return `<div class="sp sp-${AG.UI.subnav}" data-tabs-root>
    ${title ? `<div class="sp-head">${AG.head(title, desc, head)}</div>` : ""}
    <nav class="sp-nav" aria-label="Sub menu">${items}</nav>
    <div class="sp-body">${body}</div></div>`;
};
/* Panel untuk subpage lokal: AG.panel("k", html, aktif) */
AG.panel = (k, html, on = false) => `<div class="sp-panel col g24 ${on ? "on" : ""}" data-panel="${k}">${html}</div>`;

/* ---------- Bar filter ---------- */
/* AG.filterBar({ id, search: "Cari…", filters: [{ k, label, icon, options: [...], value: [] }], quick: [{ label, icon, on }], right: "" }) */
AG._filters = {};
AG.filterBar = ({ id = "fb" + Math.random().toString(36).slice(2, 7), search = "Cari…", filters = [], quick = [], right = "", count = "" } = {}) => {
  const prev = AG._filters[id];
  AG._filters[id] = { filters: filters.map((f) => { const o = prev?.filters.find((x) => x.k === f.k); return { ...f, value: o ? o.value : [].concat(f.value || []) }; }), q: prev?.q || "" };
  const st = AG._filters[id];
  const active = st.filters.filter((f) => f.value.length);
  const v = AG.UI.table;
  const chip = (f) => `<button class="fchip ${f.value.length ? "on" : ""}" data-menu="filter" data-arg="${id}:${f.k}">${f.icon ? ic(f.icon, "s14") : ""}${f.label}${f.value.length ? `<b>${f.value.length === 1 ? esc(f.value[0]) : f.value.length + " dipilih"}</b>` : ""}${ic("chevron-down", "s12")}</button>`;
  const searchBox = search ? `<label class="fsearch">${ic("search", "s14")}<input placeholder="${esc(search)}" value="${esc(st.q)}" data-fsearch="${id}" aria-label="${esc(search)}"></label>` : "";
  const quickHtml = quick.map((q) => `<button class="fquick ${q.on ? "on" : ""}" data-toggle>${q.icon ? ic(q.icon, "s14") : ""}${q.label}</button>`).join("");
  if (v === "B") return `<div class="fbar fbar-B" data-fbar="${id}"><div class="fr">${searchBox}${quickHtml}${st.filters.map(chip).join("")}${active.length ? `<button class="fclear" data-act="filter-clear" data-arg="${id}">${ic("x", "s14")}Hapus filter</button>` : ""}<span class="grow"></span>${count ? `<span class="mut sm">${count}</span>` : ""}${right}</div></div>`;
  return `<div class="fbar fbar-${v}" data-fbar="${id}">
    <div class="fr">${searchBox}${st.filters.length ? AG.btn(`Filter${active.length ? ` <span class="fcount">${active.length}</span>` : ""}`, "", "filter", { "data-menu": "filter-all", "data-arg": id }) : ""}${quickHtml}<span class="grow"></span>${count ? `<span class="mut sm">${count}</span>` : ""}${right}</div>
    ${active.length ? `<div class="fr fr-active">${active.map((f) => `<span class="fpill">${f.label}: <b>${esc(f.value.join(", "))}</b><button data-act="filter-remove" data-arg="${id}:${f.k}" aria-label="Hapus filter ${f.label}">${ic("x", "s12")}</button></span>`).join("")}<button class="fclear" data-act="filter-clear" data-arg="${id}">Hapus semua</button></div>` : ""}
  </div>`;
};
AG.menu("filter", (arg) => {
  const [id, k] = arg.split(":"); const f = AG._filters[id].filters.find((x) => x.k === k);
  return `<div class="mlab">${f.label}</div>${f.options.map((o) => `<button class="mi" data-act="filter-pick" data-arg="${id}:${k}:${esc(o)}"><span class="cb ${f.value.includes(o) ? "on" : ""}">${ic("check")}</span>${esc(o)}</button>`).join("")}`;
});
AG.menu("filter-all", (id) => `<div class="mlab">Tambah filter</div>${AG._filters[id].filters.map((f) => `<button class="mi" data-menu="filter" data-arg="${id}:${f.k}">${f.icon ? ic(f.icon) : ""}${f.label}<span class="tail">${f.value.length ? f.value.length + " dipilih" : ic("chevron-right", "s14")}</span></button>`).join("")}<div class="msep"></div><button class="mi" data-dialog="save-filter">${ic("bookmark")}Simpan sebagai filter…</button>`);
AG.action("filter-pick", (el, arg, e) => {
  e.stopPropagation();
  const [id, k, o] = arg.split(":"); const f = AG._filters[id].filters.find((x) => x.k === k);
  f.value = f.value.includes(o) ? f.value.filter((x) => x !== o) : [...f.value, o];
  el.querySelector(".cb").classList.toggle("on");
  AG._filterDirty = true;
});
AG.action("filter-remove", (el, arg) => { const [id, k] = arg.split(":"); AG._filters[id].filters.find((x) => x.k === k).value = []; AG.rerender(); });
AG.action("filter-clear", (el, id) => { AG._filters[id].filters.forEach((f) => (f.value = [])); AG.rerender(); });
/* Menutup menu filter setelah memilih: render ulang supaya chip/tabel ikut berubah. */
document.addEventListener("click", (e) => { if (AG._filterDirty && !e.target.closest(".menu")) { AG._filterDirty = false; AG.rerender(); } }, true);
document.addEventListener("input", (e) => { const s = e.target.closest("[data-fsearch]"); if (s) { AG._filters[s.dataset.fsearch].q = s.value; const t = document.querySelector(`[data-dt-of="${s.dataset.fsearch}"]`); if (t) AG._dtDraw(t.dataset.dt); s.dispatchEvent(new CustomEvent("filter-search", { bubbles: true, detail: { id: s.dataset.fsearch, q: s.value } })); } });
/* Nilai filter aktif untuk dipakai modul: AG.filterValue(id, k) → [] */
AG.filterValue = (id, k) => AG._filters[id]?.filters.find((f) => f.k === k)?.value || [];
AG.filterQuery = (id) => (AG._filters[id]?.q || "").toLowerCase();

/* ---------- Tabel data ---------- */
/* AG.datatable({ id, columns: [{ label, sort: true, w, hide: "sm", align }], rows: [{ cells: [html...], sort: [nilai...], text: "untuk pencarian", filter: { k: nilai } }],
     filters, search, quick, right, pageSize: 10, select: false, bulk: html, empty: html, rowGo: (i) => "#/..." }) */
AG._dts = {};
AG.datatable = (o) => {
  const id = o.id || "dt" + Math.random().toString(36).slice(2, 7);
  const prev = AG._dts[id];
  AG._dts[id] = { ...o, id, page: prev?.page || 1, size: prev?.size || o.pageSize || 10, sortBy: prev?.sortBy ?? null, dir: prev?.dir || 1, sel: prev?.sel || new Set() };
  const fid = id + "-f";
  const bar = (o.filters || o.search || o.quick) ? AG.filterBar({ id: fid, search: o.search ?? "Cari…", filters: o.filters || [], quick: o.quick || [], right: o.right || "" }) : "";
  const side = AG.UI.table === "C" && o.filters?.length;
  return `<div class="dt dt-${AG.UI.table} ${side ? "dt-side" : ""}" data-dt="${id}" data-dt-of="${fid}">
    ${side ? `<aside class="dt-facets">${facets(fid)}</aside>` : ""}
    <div class="dt-main">${bar}<div class="dt-box"><div class="dt-bulk"></div><div class="tblwrap"><table class="tbl dt-t"></table></div><div class="dt-foot"></div></div></div></div>`;
};
function facets(fid) {
  const st = AG._filters[fid]; if (!st) return "";
  return `<div class="dt-fh">${ic("filter", "s14")}Filter</div>` + st.filters.map((f) => `<div class="dt-fg"><div class="dt-fl">${f.label}</div>${f.options.map((op) => `<button class="dt-fo" data-act="facet" data-arg="${fid}:${f.k}:${esc(op)}"><span class="cb ${f.value.includes(op) ? "on" : ""}">${ic("check")}</span><span class="grow">${esc(op)}</span></button>`).join("")}</div>`).join("");
}
AG.action("facet", (el, arg) => { const [id, k, o] = arg.split(":"); const f = AG._filters[id].filters.find((x) => x.k === k); f.value = f.value.includes(o) ? f.value.filter((x) => x !== o) : [...f.value, o]; AG.rerender(); });
AG._dtDraw = (id) => {
  const d = AG._dts[id], root = document.querySelector(`[data-dt="${id}"]`); if (!d || !root) return;
  const fid = id + "-f", fs = AG._filters[fid];
  let rows = d.rows.map((r, i) => ({ ...r, _i: i }));
  if (fs) {
    const q = fs.q.toLowerCase().trim();
    if (q) rows = rows.filter((r) => (r.text || r.cells.join(" ").replace(/<[^>]+>/g, " ")).toLowerCase().includes(q));
    fs.filters.forEach((f) => { if (f.value.length) rows = rows.filter((r) => f.value.includes(String(r.filter?.[f.k] ?? ""))); });
  }
  if (d.sortBy != null) rows.sort((a, b) => { const x = a.sort?.[d.sortBy] ?? "", y = b.sort?.[d.sortBy] ?? ""; return (x > y ? 1 : x < y ? -1 : 0) * d.dir; });
  const total = rows.length, pages = Math.max(1, Math.ceil(total / d.size)); d.page = Math.min(d.page, pages);
  const from = (d.page - 1) * d.size, view = rows.slice(from, from + d.size);
  const hc = (c) => (c.hide === "sm" ? "hide-sm" : "");
  const th = d.columns.map((c, ci) => {
    const s = c.sort !== false && c.label;
    const arrow = d.sortBy === ci ? ic(d.dir > 0 ? "arrow-up" : "arrow-down", "s12") : s ? ic("selector", "s12 dt-sa") : "";
    return `<th class="${hc(c)}" style="${c.w ? `width:${c.w};` : ""}${c.align ? `text-align:${c.align}` : ""}">${s ? `<button class="dt-sort ${d.sortBy === ci ? "on" : ""}" data-act="dt-sort" data-arg="${id}:${ci}">${c.label}${arrow}</button>` : c.label || ""}</th>`;
  }).join("");
  const all = view.length && view.every((r) => d.sel.has(r._i));
  root.querySelector(".dt-t").innerHTML = `<thead><tr>${d.select ? `<th style="width:40px">${`<button class="cb ${all ? "on" : ""}" data-act="dt-all" data-arg="${id}" aria-label="Pilih semua">${ic("check")}</button>`}</th>` : ""}${th}</tr></thead><tbody>${view.map((r) => `<tr class="${d.sel.has(r._i) ? "on" : ""}" ${d.rowGo ? `data-go="${d.rowGo(r._i)}" style="cursor:pointer"` : ""}>${d.select ? `<td><button class="cb ${d.sel.has(r._i) ? "on" : ""}" data-act="dt-sel" data-arg="${id}:${r._i}" aria-label="Pilih baris">${ic("check")}</button></td>` : ""}${r.cells.map((c, ci) => `<td class="${hc(d.columns[ci] || {})}" style="${d.columns[ci]?.align ? `text-align:${d.columns[ci].align}` : ""}">${c}</td>`).join("")}</tr>`).join("") || `<tr><td colspan="${d.columns.length + (d.select ? 1 : 0)}" style="padding:0">${d.empty || AG.empty("search", "Tidak ada data yang cocok", "Ubah kata kunci atau hapus filter.")}</td></tr>`}</tbody>`;
  root.querySelector(".dt-bulk").innerHTML = d.sel.size ? `<div class="dt-bulkbar"><b>${d.sel.size} dipilih</b>${d.bulk || AG.btn("Ubah massal", "sm", "edit", { "data-dialog": "bulk-edit" })}<span class="grow"></span>${AG.btn("Batal pilih", "ghost sm", "x", { "data-act": "dt-none", "data-arg": id })}</div>` : "";
  const nums = []; for (let p = 1; p <= pages; p++) if (p === 1 || p === pages || Math.abs(p - d.page) <= 1) nums.push(p); else if (nums[nums.length - 1] !== "…") nums.push("…");
  root.querySelector(".dt-foot").innerHTML = `<span class="mut">${total ? `${from + 1}–${Math.min(from + d.size, total)} dari <b class="fg b5">${total}</b> data` : "0 data"}</span><span class="grow"></span>
    <label class="row g4 mut hide-sm">Per halaman <span class="selwrap" style="width:72px"><select class="inp" style="height:28px" data-dt-size="${id}">${[10, 25, 50, 100].map((n) => `<option ${n === d.size ? "selected" : ""}>${n}</option>`).join("")}</select>${ic("selector", "s14")}</span></label>
    <nav class="dt-pg" aria-label="Halaman">${AG.btn("", "ghost icon sm", "chevron-left", { "data-act": "dt-page", "data-arg": `${id}:${d.page - 1}`, "aria-label": "Halaman sebelumnya", ...(d.page <= 1 ? { disabled: true } : {}) })}${nums.map((n) => (n === "…" ? `<span class="faint">…</span>` : `<button class="dt-n ${n === d.page ? "on" : ""}" data-act="dt-page" data-arg="${id}:${n}" aria-label="Halaman ${n}"${n === d.page ? ' aria-current="page"' : ""}>${n}</button>`)).join("")}${AG.btn("", "ghost icon sm", "chevron-right", { "data-act": "dt-page", "data-arg": `${id}:${d.page + 1}`, "aria-label": "Halaman berikutnya", ...(d.page >= pages ? { disabled: true } : {}) })}</nav>`;
};
AG.action("dt-sort", (el, arg) => { const [id, ci] = arg.split(":"); const d = AG._dts[id]; if (d.sortBy === +ci) d.dir = -d.dir; else { d.sortBy = +ci; d.dir = 1; } AG._dtDraw(id); });
AG.action("dt-page", (el, arg) => { const [id, p] = arg.split(":"); AG._dts[id].page = Math.max(1, +p); AG._dtDraw(id); });
AG.action("dt-sel", (el, arg, e) => { e.stopPropagation(); const [id, i] = arg.split(":"); const s = AG._dts[id].sel; s.has(+i) ? s.delete(+i) : s.add(+i); AG._dtDraw(id); });
AG.action("dt-all", (el, id) => { const d = AG._dts[id]; const rows = [...document.querySelectorAll(`[data-dt="${id}"] [data-act="dt-sel"]`)].map((b) => +b.dataset.arg.split(":")[1]); const all = rows.every((i) => d.sel.has(i)); rows.forEach((i) => (all ? d.sel.delete(i) : d.sel.add(i))); AG._dtDraw(id); });
AG.action("dt-none", (el, id) => { AG._dts[id].sel.clear(); AG._dtDraw(id); });
document.addEventListener("change", (e) => { const s = e.target.closest("[data-dt-size]"); if (s) { const d = AG._dts[s.dataset.dtSize]; d.size = +s.value; d.page = 1; AG._dtDraw(d.id); } });
/* Gambar semua tabel setelah setiap render layar. */
const _render = AG.render;
/* Kembalikan tab/panel yang terakhir dipilih di rute ini (agar render ulang tidak melompat ke tab pertama). */
const restoreTabs = () => {
  const mem = AG._tabMem[location.hash]; if (!mem) return;
  [...document.querySelectorAll("[data-tabs-root]")].forEach((root, i) => {
    const k = mem[i]; if (k == null || !root.querySelector(`[data-tab="${k}"]`)) return;
    const own = (x) => x.closest("[data-tabs-root]") === root;
    root.querySelectorAll("[data-tab]").forEach((x) => own(x) && x.classList.toggle("on", x.dataset.tab === k));
    root.querySelectorAll("[data-panel]").forEach((x) => own(x) && x.classList.toggle("on", x.dataset.panel === k));
  });
};
const after = () => { restoreTabs(); document.querySelectorAll("[data-dt]").forEach((t) => AG._dtDraw(t.dataset.dt)); };
AG.render = () => { _render(); after(); };
const _rer = AG.rerender;
AG.rerender = () => { const y = scrollY; _render(); after(); scrollTo(0, y); };
window.removeEventListener("hashchange", _render);
window.addEventListener("hashchange", () => AG.render());

/* ---------- Panel kiri halaman masuk ---------- */
AG.authBrand = () => {
  const v = AG.UI.auth;
  const logo = `<span class="logo"><span class="mk">${ic("robot", "s14")}</span>Agent Gate</span>`;
  if (v === "B") return `<div class="brand ab-B">${logo}
    <div class="ab-flow">${[["ticket", "Tiket masuk", "PM menulis kebutuhan, AI menyusun draft tiket"], ["robot", "Agent mengerjakan", "PRD, desain, kode, dan test di runner tim"], ["lock", "Gate memeriksa", "Bukti test dan screenshot ditandatangani runner"], ["rocket", "Tim menyetujui", "Satu klik approve, PR merge, rilis"]].map(([i, t, d], n) => `<div class="ab-step"><span class="ab-dot">${ic(i, "s18")}</span><div><b>${t}</b><p>${d}</p></div>${n < 3 ? '<span class="ab-line"></span>' : ""}</div>`).join("")}</div>
    <p class="ab-foot">Dipakai tim produk untuk mengirim fitur lebih cepat tanpa kehilangan kendali.</p></div>`;
  if (v === "C") return `<div class="brand ab-C">${logo}
    <div class="ab-quote"><p>“Sejak pakai Agent Gate, tiket kecil selesai di hari yang sama. Kami tinggal review dan approve.”</p><div class="row g12"><span class="av" style="width:36px;height:36px;background:#0f8a5f;font-size:13px">RS</span><div><b>Rudi Santoso</b><div class="mut sm">Engineering Manager, Toko Maju</div></div></div></div>
    <div class="ab-stats">${[["3,2×", "lebih banyak tiket selesai"], ["41%", "token lebih hemat"], ["0", "rilis tanpa review"]].map(([a, b]) => `<div><b>${a}</b><span>${b}</span></div>`).join("")}</div></div>`;
  return `<div class="brand ab-A">${logo}
    <div class="ab-copy"><h2>Tiket masuk, agent AI mengerjakan, tim yang memutuskan.</h2><p>Board, workflow, dan gate review dalam satu tempat.</p></div>
    <div class="ab-shot" aria-hidden="true"><div class="ab-win"><div class="ab-bar"><i></i><i></i><i></i></div><div class="ab-cols">${[["PRD", "#7c3aed", [["KSR-33", "Mode offline", "jalan"]]], ["Development", "#0f8a5f", [["KSR-26", "Validasi stok", "jalan"], ["KSR-27", "Ekspor CSV", ""]]], ["Code Review", "#2563eb", [["KSR-24", "Integrasi QRIS", "review"]]]].map(([n, c, cards]) => `<div class="ab-col"><div class="ab-ch"><span style="background:${c}"></span>${n}</div>${cards.map(([k, t, s]) => `<div class="ab-card"><span class="mono">${k}</span><b>${t}</b>${s === "jalan" ? `<em class="j">${ic("sparkles", "s12")}AI bekerja</em>` : s === "review" ? `<em class="r">${ic("eye", "s12")}Menunggu review</em>` : ""}</div>`).join("")}</div>`).join("")}</div></div></div></div>`;
};
})();
