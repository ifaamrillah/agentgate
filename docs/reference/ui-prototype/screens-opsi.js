/* Opsi desain: pemilik produk memilih varian komponen global. Pilihan disimpan (localStorage) dan langsung berlaku di semua layar. */
(() => {
const { ic, btn, esc } = AG;

/* Gaya kecil khusus layar opsi */
const css = document.createElement("style");
css.textContent = `
.op-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
@media (max-width: 899px) { .op-grid { grid-template-columns: 1fr; } }
.op-card { text-align: left; display: grid; gap: 10px; align-content: start; padding: 14px; border-radius: 10px; background: var(--raised); border: 1px solid var(--line); min-width: 0; cursor: pointer; transition: border-color .15s, box-shadow .15s; }
.op-card:hover { border-color: var(--input); box-shadow: var(--shadow-sm); }
.op-card.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
.op-card .op-h { display: flex; align-items: center; gap: 8px; }
.op-card .op-l { width: 24px; height: 24px; border-radius: 6px; background: var(--hover); display: inline-flex; align-items: center; justify-content: center; font-weight: 600; font-size: 12px; color: var(--fg2); }
.op-card.on .op-l { background: var(--accent); color: var(--on-accent, #fff); }
.op-card p { color: var(--muted); font-size: 13px; margin: 0; }
.op-mini { border-radius: 8px; background: var(--page); border: 1px solid var(--line); padding: 10px; min-height: 64px; display: grid; align-items: center; overflow: hidden; pointer-events: none; }
.op-live { border: 1px solid var(--line); border-radius: 12px; background: var(--page); padding: 20px; min-width: 0; overflow: hidden; }
.op-live-h { display: flex; align-items: center; gap: 8px; margin-bottom: 14px; color: var(--muted); font-size: 12.5px; flex-wrap: wrap; }
.op-auth { border: 1px solid var(--line); border-radius: 12px; overflow: hidden; height: 600px; position: relative; }
.op-auth .split { min-height: 600px; height: 600px; }
.op-auth .brand { display: flex !important; }
@media (max-width: 899px) { .op-auth .formside-x { display: none; } .op-auth .brand { width: 100%; max-width: none; } }
.op-hub { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
@media (max-width: 899px) { .op-hub { grid-template-columns: 1fr; } }
.op-hub a { display: grid; gap: 6px; padding: 16px; border-radius: 10px; background: var(--raised); border: 1px solid var(--line); color: inherit; text-decoration: none; }
.op-hub a:hover { border-color: var(--input); box-shadow: var(--shadow-sm); }
.op-hub .row b { flex: 1; }
.op-board { display: grid; grid-template-columns: repeat(4, minmax(220px, 1fr)); gap: 10px; overflow-x: auto; padding-bottom: 4px; }
.op-board .bcol { display: grid; gap: 8px; align-content: start; min-height: 260px; }
.op-board .bh { display: flex; align-items: center; gap: 8px; font-weight: 500; padding: 4px 4px 2px; }
`;
document.head.appendChild(css);

/* Render html dengan varian sementara (untuk pratinjau kecil di kartu opsi) */
const withUI = (k, v, fn) => { const old = AG.UI[k]; AG.UI[k] = v; try { return fn(); } finally { AG.UI[k] = old; } };

const DECISIONS = [
  { k: "shell", n: "Warna latar aplikasi", i: "layout-sidebar", d: "Warna sidebar dan area kerja di semua halaman. Seluruh halaman ini ikut berubah saat Anda memilih.", go: "#/home",
    opts: { A: ["Panel melayang", "Sidebar netral hangat, area kerja putih membulat di atasnya. Mirip Linear dan Notion Calendar."], B: ["Putih bersih", "Semua putih; kartu dipisah garis dan bayangan halus. Mirip GitHub dan Vercel."], C: ["Sidebar gelap", "Sidebar arang dengan teks terang, area kerja putih. Mirip ClickUp dan Supabase."] } },
  { k: "auth", n: "Panel kiri halaman masuk", i: "lock", d: "Layar 1–5: masuk, daftar, lupa sandi, 2FA, undangan.", go: "#/login",
    opts: { A: ["Cuplikan produk", "Kalimat utama + cuplikan board yang sedang dikerjakan agent."], B: ["Alur 4 langkah", "Latar gelap, menjelaskan cara kerja: tiket → agent → gate → approve."], C: ["Testimoni + angka", "Kutipan pengguna dan tiga angka hasil."] } },
  { k: "stat", n: "Kartu angka", i: "chart-bar", d: "Strip angka seperti Perlu tindakan / Ditugaskan / Dilaporkan / Diikuti.", go: "#/my",
    opts: { A: ["Ringkas", "Label kecil di atas, angka di bawah. Paling tenang."], B: ["Dengan ikon", "Ikon besar berwarna di kiri, angka dan label di kanan."], C: ["Garis warna", "Garis warna di atas sesuai status, angka lebih besar, tanpa ikon."] } },
  { k: "subnav", n: "Menu tab vertikal", i: "layout-sidebar", d: "Halaman pengaturan seperti Pengaturan project · Umum, Persona, Runner.", go: "#/settings/p/KSR/general",
    opts: { A: ["Daftar polos", "Item aktif berbentuk kartu putih kecil. Mirip Linear."], B: ["Kartu dengan deskripsi", "Menu di dalam kartu, tiap item punya deskripsi singkat. Mirip Vercel. (Pilihan Anda sekarang)"], C: ["Garis tepi", "Garis vertikal tipis, item aktif ditandai garis aksen. Mirip GitHub."], D: ["Ikon berkotak", "Ikon di kotak kecil berwarna, item aktif ikonnya menyala aksen, dengan deskripsi. Mirip Stripe."], E: ["Satu panel menyatu", "Menu dan isi dalam satu kartu besar: menu berlatar abu di kiri, isi di kanan. Mirip pengaturan macOS dan Notion."] } },
  { k: "sec", n: "Kartu bagian", i: "layout-rows", d: "Judul, deskripsi, dan tautan “Lihat semua” berada di dalam kartu. Berlaku untuk semua bagian seperti Perlu tindakan, Runner tim, Properti.", go: "#/home",
    opts: { A: ["Header bergaris", "Judul dan tautan di atas, dipisah garis tipis dari isi. Paling umum (Jira, GitHub)."], B: ["Header berlatar", "Judul huruf kapital kecil di pita abu lembut. Tegas memisahkan bagian (Stripe)."], C: ["Tanpa garis", "Judul lebih besar, isi langsung di bawahnya tanpa pemisah. Paling ringan (Linear)."] } },
  { k: "table", n: "Tabel data", i: "table", d: "Semua tabel: anggota, audit, daftar tiket, PR, pencarian, webhook.", go: "#/p/KSR/list",
    opts: { A: ["Filter di menu", "Satu tombol Filter + chip filter aktif di bawahnya. Rapi walau filter banyak."], B: ["Chip sebaris", "Setiap filter tampil sebagai chip di bar. Cepat diklik, header tabel tegas, baris belang."], C: ["Panel facet kiri", "Filter berupa daftar centang di kiri tabel, seperti toko online. Cocok untuk data besar."] } },
  { k: "board", n: "Kolom board", i: "layout-kanban", d: "Pembungkus kolom di board dan swimlane, plus bar filter.", go: "#/p/KSR/board",
    opts: { A: ["Latar abu lembut", "Kolom berlatar abu tipis, kartu putih. Mirip Jira."], B: ["Kolom berbingkai", "Kolom kartu putih berbingkai, kartu di dalamnya abu. Mirip Linear."], C: ["Garis putus", "Kolom berbingkai putus-putus dengan semburat aksen. Terasa ringan."] } },
  { k: "prio", n: "Ikon prioritas", i: "flag", d: "Ikon prioritas di kartu board, daftar, dan detail tiket.", go: "#/p/KSR/board",
    opts: { A: ["Chevron", "Dua chevron = mendesak, chevron = tinggi, sama dengan = sedang, turun = rendah. Mirip Jira."], B: ["Batang sinyal", "Kotak merah bertanda seru untuk mendesak, 3 / 2 / 1 batang terisi untuk tinggi, sedang, rendah. Mirip Linear."], C: ["Label P1–P4", "Pil berwarna berisi teks P1–P4. Paling mudah dibedakan, tapi lebih ramai."] } },
];
AG.OPSI = DECISIONS;

/* ---------- Pratinjau ---------- */
const sampleStats = () => AG.stats([
  { label: "Perlu tindakan", value: 4, icon: "alert-circle", tone: "dng", hint: "2 review, 2 keputusan", go: "#/my" },
  { label: "Ditugaskan", value: 11, icon: "user-check", tone: "acc", hint: "3 jatuh tempo minggu ini" },
  { label: "Dilaporkan", value: 7, icon: "message-2", tone: "warn", delta: "+2" },
  { label: "Diikuti", value: 23, icon: "eye", tone: "ok" },
]);
const miniStats = () => AG.stats([{ label: "Perlu tindakan", value: 4, icon: "alert-circle", tone: "dng" }, { label: "Ditugaskan", value: 11, icon: "user-check", tone: "acc" }]);

const samplePrio = () => `<div class="col g12">
  <div class="row wrap g16">${[1, 2, 3, 4].map((p) => `<span class="row g8">${AG.prioIcon(p, "s16", true)}</span>`).join("")}</div>
  <div class="box" style="padding:0"><table class="tbl"><thead><tr><th>Tiket</th><th>Judul</th><th>Prioritas</th></tr></thead><tbody>
    ${AG.TICKETS.slice(0, 5).map((t) => `<tr><td class="mono faint">${t.k}</td><td>${esc(t.t)}</td><td>${AG.prioIcon(t.p, "s14", true)}</td></tr>`).join("")}</tbody></table></div></div>`;
const miniPrio = () => `<div class="row wrap g12">${[1, 2, 3, 4].map((p) => AG.prioIcon(p, "s16")).join("")}</div>`;

const sampleSubnav = () => AG.subpage({
  active: "umum",
  nav: [
    { k: "umum", label: "Umum", icon: "settings", desc: "Nama, kode, logo, warna", group: "Project" },
    { k: "akses", label: "Akses", icon: "lock", desc: "Siapa yang bisa melihat" },
    { k: "repo", label: "Repo", icon: "git-branch", desc: "Repo terhubung", count: 2 },
    { k: "wf", label: "Workflow", icon: "route", desc: "Tahap dan gate", group: "Proses" },
    { k: "bahaya", label: "Zona berbahaya", icon: "alert-triangle", desc: "Arsip dan hapus" },
  ],
  body: AG.panel("umum", AG.card({ title: "Umum", desc: "Nama dan kode tiket project.", body: `${AG.fld("Nama project", AG.inp("Kasir Online"))}${AG.fld("Kode tiket", AG.inp("KSR"), "Dipakai sebagai awalan tiket, misalnya KSR-24.")}`, foot: AG.savebar("Simpan perubahan") }), true)
    + AG.panel("akses", AG.card({ title: "Akses", body: AG.empty("lock", "Akses project", "Contoh panel kedua. Klik item menu untuk berpindah.") }))
    + AG.panel("repo", `<div class="box">${AG.empty("git-branch", "2 repo terhubung", "kasir-web, kasir-api")}</div>`)
    + AG.panel("wf", `<div class="box">${AG.empty("route", "Workflow project", "Memakai workflow “Standar produk”.")}</div>`)
    + AG.panel("bahaya", `<div class="box">${AG.empty("alert-triangle", "Zona berbahaya", "Arsipkan atau hapus project.")}</div>`),
});

const NAMES = ["Oki Oktaviani", "Andi Ahmad", "Dewi Damayanti", "Qori Qurniawan", "Vina Valentina", "Bagas Pratama", "Citra Lestari", "Eko Saputra", "Fajar Nugroho", "Gita Maharani", "Hendra Wijaya", "Indah Permata", "Joko Susilo", "Kartika Sari", "Lukman Hakim", "Maya Anggraini", "Nanda Putri", "Rizky Ramadhan", "Sinta Ayu", "Taufik Hidayat", "Utami Rahayu", "Wahyu Setiawan", "Yoga Prasetyo", "Zahra Aulia"];
const ROLES = ["Owner", "Admin", "Member", "Member", "Viewer"], TEAMS = ["Produk", "Backend", "Frontend", "QA", "Desain"];
const sampleTable = () => AG.datatable({
  id: "op-dt", pageSize: 10, select: true, search: "Cari nama atau email…",
  filters: [{ k: "role", label: "Role", icon: "shield", options: ["Owner", "Admin", "Member", "Viewer"] }, { k: "tim", label: "Tim", icon: "users", options: TEAMS }, { k: "fa", label: "2FA", icon: "lock", options: ["Aktif", "Belum"] }],
  columns: [{ label: "Nama" }, { label: "Tim" }, { label: "Role" }, { label: "2FA", hide: "sm" }, { label: "Terakhir aktif", hide: "sm" }],
  rows: NAMES.map((n, i) => {
    const r = ROLES[i % 5], t = TEAMS[(i * 3) % 5], fa = i % 4 ? "Aktif" : "Belum", h = (i * 7) % 48 + 1;
    const ini = n.split(" ").map((x) => x[0]).join("");
    return { cells: [`<span class="row g8"><span class="av" style="width:24px;height:24px;font-size:10px;background:hsl(${i * 37 % 360} 45% 45%)">${ini}</span><span class="col"><b class="b5">${n}</b><span class="faint sm">${n.split(" ")[0].toLowerCase()}@tokomaju.id</span></span></span>`, t, r, fa === "Aktif" ? AG.pill("Aktif", "ok") : AG.pill("Belum", "warn"), `${h} jam lalu`], sort: [n, t, r, fa, h], text: n, filter: { role: r, tim: t, fa } };
  }),
});

const sampleBoard = () => {
  const cols = ["prd", "design", "dev", "review"].map((k) => AG.STAGES.find((s) => s.k === k)).filter(Boolean);
  const bar = AG.filterBar({ id: "op-fb", search: "Cari tiket…", filters: [
    { k: "who", label: "Penerima", icon: "user", options: Object.values(AG.PEOPLE).map((p) => p.n) },
    { k: "lbl", label: "Label", icon: "tag", options: Object.keys(AG.LABELS) },
    { k: "type", label: "Tipe", icon: "bookmark", options: ["Story", "Bug", "Task", "Epic"] },
    { k: "prio", label: "Prioritas", icon: "flag", options: ["Mendesak", "Tinggi", "Sedang", "Rendah"] },
    { k: "sprint", label: "Sprint", icon: "run", options: ["Sprint 14", "Sprint 15"] },
  ], quick: [{ label: "Saya", icon: "user" }, { label: "Butuh review", icon: "eye" }, { label: "Dikerjakan AI", icon: "sparkles" }], right: btn("Tampilan", "sm", "adjustments-horizontal") });
  return `<div class="col g12">${bar}<div class="op-board">${cols.map((s) => {
    const items = AG.TICKETS.filter((t) => t.stage === s.k).slice(0, 3);
    return `<div class="bcol"><div class="bh">${AG.stageIcon(s.k)}${s.n}<span class="faint">${items.length}</span></div>${items.map((t) => AG.ticketCard(t, { drag: false })).join("") || `<div class="faint sm" style="padding:8px">Kosong</div>`}</div>`;
  }).join("")}</div></div>`;
};
const miniBoard = (v) => {
  const s = {
    A: "background:var(--hover);border-radius:8px;padding:5px",
    B: "background:var(--raised);border:1px solid var(--line);border-top:3px solid var(--line2);border-radius:8px;padding:5px",
    C: "background:color-mix(in srgb,var(--accent) 4%,var(--page));border:1px dashed var(--line2);border-radius:9px;padding:5px",
  }[v];
  const card = v === "B" ? "background:var(--muted-bg);border:1px solid var(--line)" : "background:var(--raised);border:1px solid var(--line);box-shadow:var(--shadow-sm)";
  return `<div style="display:grid;grid-template-columns:repeat(3,1fr);gap:6px">${[2, 3, 1].map((n) => `<div style="${s};display:grid;gap:4px;align-content:start;min-height:60px">${Array.from({ length: n }, () => `<div style="${card};border-radius:5px;height:14px"></div>`).join("")}</div>`).join("")}</div>`;
};
const miniSubnav = (v) => {
  if (v === "D") return `<div style="display:grid;grid-template-columns:110px 1fr;gap:8px"><div style="display:grid;gap:3px">${["Umum", "Akses", "Repo"].map((t, i) => `<div style="display:flex;align-items:center;gap:6px;padding:3px 5px;border-radius:6px;font-size:11.5px;${i ? "" : "background:var(--raised);border:1px solid var(--line);box-shadow:var(--shadow-sm)"}"><span style="width:16px;height:16px;border-radius:4px;background:${i ? "var(--hover)" : "var(--accent)"}"></span>${t}</div>`).join("")}</div><div style="background:var(--raised);border:1px solid var(--line);border-radius:6px"></div></div>`;
  if (v === "E") return `<div style="display:grid;grid-template-columns:90px 1fr;border:1px solid var(--line);border-radius:8px;overflow:hidden;background:var(--raised);min-height:76px"><div style="background:var(--muted-bg);border-right:1px solid var(--line);padding:5px;display:grid;gap:2px;align-content:start">${["Umum", "Akses", "Repo"].map((t, i) => `<div style="height:20px;padding:0 6px;display:flex;align-items:center;font-size:11px;border-radius:4px;${i ? "" : "background:var(--raised);border:1px solid var(--line)"}">${t}</div>`).join("")}</div><div style="padding:8px;display:grid;gap:5px;align-content:start"><div style="height:6px;width:50%;background:var(--line2);border-radius:3px"></div><div style="height:16px;border:1px solid var(--line2);border-radius:4px"></div></div></div>`;
  const item = (on, t) => {
    const base = "display:flex;align-items:center;gap:6px;height:22px;padding:0 7px;font-size:11.5px;border-radius:5px;color:var(--fg2)";
    const onS = { A: "background:var(--raised);border:1px solid var(--line);box-shadow:var(--shadow-sm);color:var(--fg)", B: "background:var(--accent-soft);color:var(--accent-fg)", C: "border-left:2px solid var(--accent);border-radius:0;color:var(--fg);margin-left:-1px" }[v];
    return `<div style="${base};${on ? onS : v === "C" ? "border-left:2px solid transparent;border-radius:0;margin-left:-1px" : ""}">${t}${v === "B" ? `<span class="faint" style="font-size:10px;margin-left:auto">…</span>` : ""}</div>`;
  };
  const wrap = { A: "", B: "background:var(--raised);border:1px solid var(--line);border-radius:8px;padding:4px", C: "border-left:1px solid var(--line2)" }[v];
  return `<div style="display:grid;grid-template-columns:110px 1fr;gap:8px"><div style="${wrap};display:grid;gap:2px">${item(1, "Umum")}${item(0, "Akses")}${item(0, "Repo")}</div><div style="background:var(--raised);border:1px solid var(--line);border-radius:6px"></div></div>`;
};
const miniTable = (v) => {
  const line = (w) => `<div style="height:6px;border-radius:3px;background:var(--line2);width:${w}%"></div>`;
  const bar = v === "A" ? `<div class="row g4"><span style="height:16px;width:60px;border:1px solid var(--line2);border-radius:4px"></span><span style="height:16px;width:36px;border:1px solid var(--line2);border-radius:4px;background:var(--raised)"></span></div>`
    : v === "B" ? `<div class="row g4">${[50, 34, 34, 34].map((w) => `<span style="height:16px;width:${w}px;border:1px solid var(--line2);border-radius:4px;background:var(--raised)"></span>`).join("")}</div>` : "";
  const tbl = `<div style="background:var(--raised);border:1px solid var(--line);border-radius:6px;padding:6px;display:grid;gap:6px">${[0, 1, 2].map((i) => `<div style="${v === "B" && i % 2 ? "background:var(--muted-bg);" : ""}padding:2px">${line(90 - i * 15)}</div>`).join("")}</div>`;
  if (v === "C") return `<div style="display:grid;grid-template-columns:48px 1fr;gap:6px"><div style="background:var(--raised);border:1px solid var(--line);border-radius:6px;padding:5px;display:grid;gap:4px">${[0, 1, 2, 3].map(() => line(80)).join("")}</div>${tbl}</div>`;
  return `<div class="col g4">${bar}${tbl}</div>`;
};
const miniAuth = (v) => {
  const bg = v === "B" ? "#16161d" : "var(--side)";
  const inner = v === "A" ? `<div style="height:6px;width:70%;background:var(--line2);border-radius:3px"></div><div style="margin-top:auto;margin-right:-8px;background:var(--raised);border:1px solid var(--line2);border-radius:5px 0 0 0;height:30px"></div>`
    : v === "B" ? [0, 1, 2, 3].map(() => `<div class="row g4"><span style="width:8px;height:8px;border-radius:50%;background:#5b5bd6"></span><span style="height:4px;width:50%;background:#3a3a46;border-radius:2px"></span></div>`).join("")
    : `<div style="height:22px;background:var(--raised);border-radius:4px"></div><div class="row g4" style="margin-top:auto">${[0, 1, 2].map(() => `<span style="flex:1;height:12px;background:var(--line2);border-radius:3px"></span>`).join("")}</div>`;
  return `<div style="display:grid;grid-template-columns:1fr 1fr;height:72px;border:1px solid var(--line);border-radius:6px;overflow:hidden"><div style="background:${bg};padding:6px;display:flex;flex-direction:column;gap:4px">${inner}</div><div style="background:var(--raised);padding:10px;display:grid;gap:5px;align-content:center">${[0, 1].map(() => `<div style="height:8px;border:1px solid var(--line2);border-radius:3px"></div>`).join("")}<div style="height:8px;background:var(--accent);border-radius:3px"></div></div></div>`;
};
const sampleAuth = () => `<div class="op-auth"><div class="split">${AG.authBrand()}
  <div class="formside formside-x" style="flex:1;display:flex;align-items:center;justify-content:center;padding:32px;background:var(--raised)"><div class="col g16" style="width:100%;max-width:360px">
    <div><h1 style="font-size:22px;margin:0">Masuk ke Agent Gate</h1><p class="mut" style="margin:4px 0 0">Lanjutkan ke workspace Anda.</p></div>
    ${AG.fld("Email", AG.inp("oki@tokomaju.id"))}${AG.fld("Kata sandi", `<input class="inp" type="password" value="rahasia123" aria-label="Kata sandi">`)}
    ${btn("Masuk", "pri", "", { style: "width:100%" })}</div></div></div></div>`;

const miniShell = (v) => {
  const side = { A: "#f7f7f5", B: "#fbfbfa", C: "#18181d" }[v], ln = v === "C" ? "#3a3a46" : "#dcdcd8";
  const main = v === "A" ? "margin:5px 5px 5px 0;border:1px solid #e4e4e0;border-radius:6px;background:#fff" : `background:${v === "C" ? "#fcfcfc" : "#fff"};${v === "B" ? "border-left:1px solid #e4e4e0" : ""}`;
  const card = `background:#fff;border:1px solid #e6e6ea;border-radius:4px;height:22px;box-shadow:0 1px 2px rgb(0 0 0 / .06)`;
  return `<div style="display:grid;grid-template-columns:62px 1fr;height:96px;border:1px solid var(--line);border-radius:6px;overflow:hidden;background:${side}">
    <div style="padding:8px 6px;display:grid;gap:5px;align-content:start">${[70, 90, 60, 80].map((w, i) => `<div style="height:5px;width:${w}%;border-radius:3px;background:${i === 0 ? (v === "C" ? "#5b5bd6" : "#5b5bd6") : ln}"></div>`).join("")}</div>
    <div style="${main};padding:8px;display:grid;gap:6px;align-content:start"><div style="height:6px;width:40%;background:#d8d8de;border-radius:3px"></div><div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:5px"><div style="${card}"></div><div style="${card}"></div><div style="${card}"></div></div><div style="${card};height:26px"></div></div></div>`;
};
const sampleShell = () => `<div class="col g16"><p class="mut" style="margin:0">Seluruh halaman ini adalah pratinjaunya: sidebar di kiri, area kerja, dan kartu di bawah. Buka juga Beranda dan Board untuk melihat halaman yang lebih ramai.</p>${sampleStats()}<div class="box pad col g8"><b>Kartu konten</b><span class="mut">Kartu putih seperti ini dipakai untuk deskripsi, properti, aktivitas, dan pengaturan.</span></div></div>`;
const sampleSec = () => `<div class="grid2" style="display:grid;grid-template-columns:minmax(0,2fr) minmax(0,1fr);gap:16px">${AG.card({ title: "Perlu tindakan Anda", desc: "Review gate, keputusan setelah gagal, dan izin agent.", action: AG.seeAll("#/inbox"), pad: false,
  body: AG.TICKETS.filter((t) => ["review", "keputusan"].includes(t.state)).slice(0, 3).map((t) => `<div class="lrow" style="padding:12px 16px;border-top:1px solid var(--line)">${AG.typeIcon(t.type)}<span class="mono faint">${t.k}</span><span class="grow b5 trunc">${esc(t.t)}</span>${AG.btn(t.state === "review" ? "Review" : "Putuskan", t.state === "review" ? "pri sm" : "sm")}</div>`).join("") })}
  ${AG.card({ title: "Runner tim", desc: "3 dari 4 online", action: AG.seeAll("#/settings/runner", "Kelola"), pad: false, body: ["MacBook Olivia", "Laptop Dewi", "PC Qori"].map((n) => `<div class="lrow" style="padding:10px 16px;border-top:1px solid var(--line)"><span class="dot" style="width:8px;height:8px;border-radius:50%;background:var(--success)"></span><span class="grow">${n}</span></div>`).join("") })}</div>`;
const miniSec = (v) => {
  const head = { A: "border-bottom:1px solid var(--line)", B: "background:var(--muted-bg);border-bottom:1px solid var(--line)", C: "" }[v];
  return `<div style="background:var(--raised);border:1px solid var(--line);border-radius:8px;overflow:hidden"><div style="${head};padding:7px 9px;display:flex;align-items:center;gap:6px"><div style="height:6px;width:${v === "C" ? 45 : 38}%;background:var(--fg2);border-radius:3px;opacity:.7"></div><span class="grow"></span><div style="height:5px;width:18%;background:var(--accent);border-radius:3px"></div></div>${[0, 1].map(() => `<div style="padding:6px 9px;${v === "C" ? "" : "border-top:1px solid var(--line)"}"><div style="height:5px;width:70%;background:var(--line2);border-radius:3px"></div></div>`).join("")}</div>`;
};
const PREVIEW = { sec: [sampleSec, miniSec], shell: [sampleShell, miniShell], auth: [sampleAuth, miniAuth], stat: [sampleStats, (v) => withUI("stat", v, miniStats)], subnav: [sampleSubnav, miniSubnav], table: [sampleTable, miniTable], board: [sampleBoard, miniBoard], prio: [samplePrio, (v) => withUI("prio", v, miniPrio)] };

/* ---------- Aksi ---------- */
AG.action("opsi-pick", (el, arg) => {
  const [k, v] = arg.split(":");
  AG.setUI(k, v);
  Object.keys(AG._dts).forEach((id) => { if (id.startsWith("op-")) delete AG._dts[id]; });
  AG.rerender();
  const d = DECISIONS.find((x) => x.k === k);
  AG.toast(`${d.n}: opsi ${v} dipakai di semua layar`);
});
const PICKED = { sec: "A", shell: "A", auth: "B", stat: "A", subnav: "E", table: "A", board: "C", prio: "C" };
AG.action("opsi-reset", () => { DECISIONS.forEach((d) => AG.setUI(d.k, PICKED[d.k])); AG.rerender(); AG.toast("Kembali ke pilihan yang sudah disetujui"); });

/* ---------- Layar ---------- */

AG.screen({
  m: "Opsi", g: "Opsi desain", n: "Daftar keputusan desain", route: "/opsi",
  render: () => AG.shell({ crumbs: ["Opsi desain"], width: "mid",
    body: `<div class="col g24">${AG.head("Opsi desain", "Pilih satu opsi untuk tiap komponen. Pilihan langsung berlaku di semua layar prototype, jadi Anda bisa membuka halaman asli untuk melihat hasilnya.", btn("Kembali ke pilihan disetujui", "", "refresh", { "data-act": "opsi-reset" }))}
      <div class="op-hub">${DECISIONS.map((d) => `<a href="#/opsi/${d.k}"><div class="row g8">${ic(d.i, "s18 mut")}<b>${d.n}</b>${AG.pill("Opsi " + AG.UI[d.k], "acc")}</div><span class="mut sm">${d.d}</span><span class="sm">${esc(d.opts[AG.UI[d.k]][0])}</span></a>`).join("")}
        <a href="#/opsi/knowledge"><div class="row g8">${ic("books", "s18 mut")}<b>Tata letak knowledge</b>${AG.pill("Opsi B", "acc")}</div><span class="mut sm">Layar 57 & 58: katalog knowledge dan usulan perubahan.</span><span class="sm">Beranda + kartu kelompok (sedang diterapkan ke halaman asli)</span></a>
        <a href="#/opsi/office"><div class="row g8">${ic("building", "s18 mut")}<b>AI Office</b>${AG.pill("Opsi A", "acc")}</div><span class="mut sm">Kantor virtual tempat agent dan tim bekerja.</span><span class="sm">Isometrik (sedang dibuat ulang dengan 3 gaya seni)</span></a>
      </div>
      ${AG.section("Pilihan Anda saat ini", "Sebutkan kode ini saat memberi keputusan, misalnya “auth B, stat A, table C”.", `<div class="box pad"><code class="mono" style="white-space:pre-wrap">${DECISIONS.map((d) => `${d.k} ${AG.UI[d.k]}`).join(", ")}</code></div>`)}</div>`,
  }),
});

DECISIONS.forEach((d, idx) => AG.screen({
  m: "Opsi", g: "Opsi desain", n: d.n, route: "/opsi/" + d.k,
  render: () => {
    const [big, mini] = PREVIEW[d.k];
    const cur = AG.UI[d.k];
    const nav = `<div class="row g8 wrap">${idx > 0 ? btn(DECISIONS[idx - 1].n, "ghost", "arrow-left", { "data-go": "#/opsi/" + DECISIONS[idx - 1].k }) : ""}<span class="grow"></span>${idx < DECISIONS.length - 1 ? btn(`Berikutnya: ${DECISIONS[idx + 1].n}`, "", "arrow-right", { "data-go": "#/opsi/" + DECISIONS[idx + 1].k }) : btn("Opsi knowledge", "", "arrow-right", { "data-go": "#/opsi/knowledge" })}</div>`;
    return AG.shell({ crumbs: [`<a href="#/opsi">Opsi desain</a>`, d.n], width: "wide",
      body: `<div class="col g24" style="max-width:1200px;margin:0 auto">
        ${AG.head(d.n, d.d)}
        <div class="op-grid" role="radiogroup" aria-label="${esc(d.n)}">${Object.entries(d.opts).map(([v, [t, desc]]) => `<button class="op-card ${v === cur ? "on" : ""}" role="radio" aria-checked="${v === cur}" data-act="opsi-pick" data-arg="${d.k}:${v}">
          <div class="op-h"><span class="op-l">${v}</span><b class="grow">${t}</b>${v === cur ? `<span class="row g4 sm" style="color:var(--accent-fg)">${ic("circle-check-filled", "s16")}Dipakai</span>` : ""}</div>
          <div class="op-mini">${mini(v)}</div><p>${desc}</p></button>`).join("")}</div>
        <div class="op-live"><div class="op-live-h">${ic("eye", "s14")}<span class="grow">Pratinjau langsung, opsi ${cur}: ${esc(d.opts[cur][0])}</span><a class="row g4" href="${d.go}">Lihat di halaman asli${ic("arrow-up-right", "s14")}</a></div>${big()}</div>
        ${nav}</div>`,
    });
  },
}));
})();
