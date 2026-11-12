/* M1 · Identitas & tenancy (v2): autentikasi, workspace, beranda, ringkasan project, pengaturan workspace/akun/project. */
(() => {
const { ic, av, btn, inp, fld, pill, sw, cb, srow, section, head, empty, shell, esc } = AG;
const P = AG.PEOPLE;

AG.css(`
.m1-pw { padding-right: 4px; }
.m1-pw input, .m1-chips input { border: 0; outline: 0; background: none; flex: 1; min-width: 0; padding: 0; color: inherit; }
.m1-eye { width: 26px; height: 26px; border-radius: 5px; display: inline-flex; align-items: center; justify-content: center; color: var(--faint); flex-shrink: 0; }
.m1-eye:hover { background: var(--hover); color: var(--fg); }
label.inp:focus-within { border-color: var(--accent); box-shadow: var(--ring); }
label.inp.err:focus-within { border-color: var(--danger); }
.m1-otp { display: flex; gap: 8px; }
.m1-otp input { width: 44px; height: 48px; border: 1px solid var(--input); border-radius: 8px; text-align: center; font-family: var(--mono); font-size: 20px; background: var(--raised); color: var(--fg); outline: 0; padding: 0; }
.m1-otp input:focus { border-color: var(--accent); box-shadow: var(--ring); }
@media (max-width: 899px) { .m1-otp { gap: 6px; } .m1-otp input { width: 42px; } }
.m1-mi { height: 30px; padding: 0 8px; border-radius: 5px; display: flex; align-items: center; gap: 8px; color: var(--fg2); width: 100%; white-space: nowrap; }
.m1-mi:hover, .m1-mi.hov { background: var(--hover); color: var(--fg); }
.m1-mi .ic { color: var(--faint); }
.m1-mi .tail { margin-left: auto; color: var(--faint); display: inline-flex; }
.m1-chips { height: auto; min-height: 32px; flex-wrap: wrap; padding: 4px 6px; gap: 4px; cursor: text; }
.m1-chips input { min-width: 120px; height: 22px; }
.m1-chip { padding-right: 3px; }
.m1-chip button { width: 16px; height: 16px; border-radius: 4px; display: inline-flex; align-items: center; justify-content: center; color: var(--faint); }
.m1-chip button:hover { background: var(--hover); color: var(--fg); }
.m1-chip .ic { width: 12px; height: 12px; }
.m1-codes { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px 24px; padding: 18px 20px; }
.m1-codes span { font-family: var(--mono); font-size: 13.5px; letter-spacing: 0.02em; }
.m1-qr { width: 132px; height: 132px; border: 1px solid var(--line); border-radius: 8px; background: #fff; flex-shrink: 0; }
.m1-ptile { width: 40px; height: 40px; border-radius: 10px; display: inline-flex; align-items: center; justify-content: center; color: #fff; flex-shrink: 0; font-weight: 600; font-size: 14px; overflow: hidden; }
.m1-ptile img { width: 100%; height: 100%; object-fit: cover; }
.m1-stack { display: flex; height: 8px; border-radius: 4px; overflow: hidden; background: var(--hover); gap: 2px; }
.m1-stack i { display: block; height: 100%; }
.m1-stg { display: grid; grid-template-columns: 20px minmax(0, 1fr) 36px minmax(60px, 160px); gap: 10px; align-items: center; padding: 8px 16px; border-top: 1px solid var(--line); }
.m1-stg:hover { background: var(--muted-bg); }
.m1-stg .bar > i { background: var(--m1c, var(--accent)); }
.m1-pathcard { border: 1px solid var(--line2); border-radius: 8px; padding: 12px; display: flex; gap: 10px; align-items: flex-start; background: var(--raised); text-align: left; width: 100%; }
.m1-pathcard.on { border-color: var(--accent); box-shadow: var(--ring); }
.m1-pathcard .radio { margin-top: 2px; }
.m1-prov { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
.m1-prov .m1-pathcard { align-items: center; padding: 10px 12px; }
.m1-form { padding: 16px; display: grid; gap: 14px; }
.m1-g2 { display: grid; grid-template-columns: minmax(0, 1fr) 160px; gap: 12px; }
@media (max-width: 899px) { .m1-g2, .m1-prov { grid-template-columns: 1fr; } .m1-stg { grid-template-columns: 20px minmax(0, 1fr) 28px 70px; } }
.m1-perm td:not(:first-child), .m1-perm th:not(:first-child) { text-align: center; }
.m1-act .lrow { align-items: flex-start; }
.m1-act .lrow .acts { display: flex; gap: 6px; flex-shrink: 0; }
@media (max-width: 899px) { .m1-act .lrow { flex-wrap: wrap; } .m1-act .lrow .acts { width: 100%; padding-left: 40px; } }
.m1-sub { min-width: 180px; }
.m1-dz .btn { flex-shrink: 0; }
.fld > .seg { justify-self: start; }
.srow.m1-top { align-items: start; }
/* v2 */
.m1-duo { display: grid; grid-template-columns: minmax(0, 1fr) 340px; gap: 24px; align-items: stretch; }
.m1-duo > .section { grid-template-rows: auto 1fr; }
.m1-duo > .section > .box { display: flex; flex-direction: column; }
@media (max-width: 1099px) { .m1-duo { grid-template-columns: 1fr; } }
.m1-tabs { display: grid; gap: 12px; min-width: 0; }
.m1-tabs > .tabs { margin-top: 0; }
.m1-pad { padding: 16px; }
.m1-feed { padding: 2px 16px; }
.m1-more { margin-top: auto; }
.m1-up { display: flex; gap: 16px; align-items: center; flex-wrap: wrap; border: 1px dashed var(--input); border-radius: 10px; padding: 16px; background: var(--muted-bg); transition: background .15s, border-color .15s; }
.m1-up.over { border-color: var(--accent); background: var(--accent-soft); }
.m1-up-prev { width: 64px; height: 64px; border-radius: 14px; display: inline-flex; align-items: center; justify-content: center; color: #fff; font-weight: 600; font-size: 22px; overflow: hidden; flex-shrink: 0; box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.08); letter-spacing: -0.01em; }
.m1-up-prev.round { border-radius: 50%; }
.m1-up-prev img, .m1-mini img { width: 100%; height: 100%; object-fit: cover; display: block; }
.m1-up-body { display: grid; gap: 4px; flex: 1; min-width: 200px; }
.m1-sizes { display: flex; gap: 20px; align-items: flex-end; flex-wrap: wrap; }
.m1-sizes > div { display: grid; gap: 6px; justify-items: center; color: var(--muted); font-size: 12px; }
.m1-mini { display: inline-flex; align-items: center; justify-content: center; color: #fff; font-weight: 600; overflow: hidden; flex-shrink: 0; }
.m1-side { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border-radius: 7px; background: var(--side); border: 1px solid var(--line); font-weight: 500; }
.dt-t tr:has(.m1-cur) td { background: var(--accent-soft) !important; }
.m1-rc { width: 170px; flex-shrink: 0; }
@media (max-width: 899px) { .m1-rc { width: 132px; } }
.m1-wf { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.m1-wf .arr { color: var(--faint); }
.m1-tm { display: inline-flex; align-items: center; gap: 4px; color: var(--fg2); white-space: nowrap; } .m1-tm:hover { color: var(--accent-fg); text-decoration: underline; }
.m1-perms .lrow { align-items: flex-start; }
.m1-auth .brand { flex-shrink: 0; }
.m1-auth .ab-B .ab-step b { color: #fff; }
/* v3 */
.m1-duo > .cardx, .m1-two > .cardx { height: 100%; }
.m1-two { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 24px; align-items: stretch; }
@media (max-width: 1099px) { .m1-two { grid-template-columns: 1fr; } }
.m1-rcol { display: grid; gap: 24px; align-content: start; min-width: 0; }
.cardx.m1-acc { border-color: var(--accent); box-shadow: var(--ring); }
.cardx.m1-dng { border-color: var(--danger-line); }
.cardx.m1-dng .cx-h h2 { color: var(--danger); }
.cx-b > .dt .dt-box { border-left: 0; border-right: 0; border-bottom: 0; border-radius: 0 0 9px 9px; margin: 0 -16px -16px; box-shadow: none !important; }
.cx-b > .dt .tbl th { border-radius: 0 !important; }
.m1-sublab { font-weight: 600; font-size: 13px; }
.m1-blk { display: grid; gap: 10px; padding: 16px; border-top: 1px solid var(--line); }
.m1-blk:first-child { border-top: 0; }
.cx-b.flush > .feed { padding: 2px 16px; }
.m1-cnt { color: var(--muted); font-weight: 400; margin-left: 4px; }
@media (max-width: 1199px) { .m1-auth .brand { padding: 36px 32px; } .m1-auth .ab-shot { margin-right: -32px; } .m1-auth .ab-quote p { font-size: 19px; } .m1-auth .ab-stats b { font-size: 19px; } }
`);

/* ====================== Data lokal M1 ====================== */
const S = {
  wsDel: false, removed: [], auditSel: 0, repoForm: false, permDone: false, copied: false, forgotSent: false, recov: false,
  tab: {}, prov: "github", color: {}, archived: {},
  logo: { KSR: null, ws: null, me: null },
  invites: [{ e: "rina@agentgate.test", r: "Member", j: "QA Engineer", w: "19 Nov", by: "OO" }, { e: "galih@tokomaju.id", r: "Viewer", j: "", w: "17 Nov", by: "AA" }],
  access: [
    { t: "team", id: "engineering", r: "Member" }, { t: "team", id: "produk", r: "Viewer" }, { t: "team", id: "desain", r: "Member" },
    { t: "user", id: "OO", r: "Admin", lock: "Owner workspace" }, { t: "user", id: "AA", r: "Admin", lock: "Admin workspace" },
    { t: "user", id: "QQ", r: "Release Manager" }, { t: "user", id: "VV", r: "Viewer" },
  ],
  repos: {
    KSR: [
      { n: "kasir-api", prov: "github", u: "github.com/pt-contoh/kasir-api", b: "main", path: "clone", sync: "sinkron 5 mnt lalu" },
      { n: "kasir-web", prov: "gitlab", u: "gitlab.sevima.internal/toko/kasir-web", b: "develop", path: "upload", sync: "upload 1 jam lalu dari Laptop Dewi", ent: true },
    ],
    GDG: [{ n: "gudang-service", prov: "bitbucket", u: "bitbucket.org/pt-contoh/gudang-service", b: "main", path: "clone", sync: "sinkron 2 jam lalu" }],
    PPL: [
      { n: "portal-web", prov: "github", u: "github.com/pt-contoh/portal-web", b: "main", path: "clone", sync: "sinkron 20 mnt lalu" },
      { n: "portal-mobile", prov: "github", u: "github.com/pt-contoh/portal-mobile", b: "main", path: "clone", sync: "sinkron 20 mnt lalu" },
    ],
  },
};
const JOBS = ["Product Manager", "Tech Lead", "Developer", "QA Engineer", "UI/UX Designer", "DevOps Engineer", "Data Analyst"];

/* Anggota tambahan (lokal M1). 5 orang inti tetap dari AG.PEOPLE. */
const EXTRA = [
  ["BS", "Budi Santoso", "Developer", "Member", true, "#0d9488", "6 Okt 2026"],
  ["SR", "Siti Rahmawati", "UI/UX Designer", "Member", true, "#db2777", "6 Okt 2026"],
  ["AW", "Agus Wijaya", "Tech Lead", "Admin", true, "#4f46e5", "3 Okt 2026"],
  ["PL", "Putri Lestari", "QA Engineer", "Member", true, "#ea580c", "13 Okt 2026"],
  ["RM", "Rizky Maulana", "Developer", "Member", true, "#0891b2", "13 Okt 2026"],
  ["DK", "Dian Kusuma", "Product Manager", "Member", true, "#9333ea", "20 Okt 2026"],
  ["FN", "Fajar Nugroho", "Developer", "Member", true, "#65a30d", "20 Okt 2026"],
  ["IP", "Indah Permata", "UI/UX Designer", "Member", false, "#e11d48", "27 Okt 2026"],
  ["HG", "Hendra Gunawan", "DevOps Engineer", "Member", true, "#475569", "27 Okt 2026"],
  ["WS", "Wulan Sari", "QA Engineer", "Member", false, "#ca8a04", "29 Okt 2026"],
  ["YP", "Yoga Pratama", "Developer", "Member", true, "#2563eb", "3 Nov 2026"],
  ["NS", "Nadia Safitri", "Data Analyst", "Viewer", true, "#c026d3", "3 Nov 2026"],
  ["ES", "Eko Saputra", "Developer", "Member", false, "#0f766e", "5 Nov 2026"],
  ["MA", "Maya Anggraini", "Product Manager", "Member", true, "#be185d", "5 Nov 2026"],
  ["TP", "Teguh Prasetyo", "Developer", "Release Manager", true, "#1d4ed8", "6 Okt 2026"],
  ["AL", "Ayu Larasati", "QA Engineer", "Member", true, "#b45309", "9 Nov 2026"],
  ["BY", "Bayu Setiawan", "Developer", "Member", false, "#0369a1", "11 Nov 2026"],
  ["LM", "Lina Marlina", "", "Viewer", false, "#78716c", "11 Nov 2026"],
  ["RF", "Reza Firmansyah", "Developer", "Member", true, "#15803d", "9 Nov 2026"],
];
/* Tanggal bergabung orang inti untuk tabel anggota (workspace dibuat 2 Okt 2026). */
const JOIN = { OO: "2 Okt 2026", AA: "2 Okt 2026", DD: "3 Okt 2026", QQ: "6 Okt 2026", VV: "29 Okt 2026" };
const MEM = {};
Object.keys(P).forEach((k) => (MEM[k] = P[k]));
EXTRA.forEach(([k, name, job, role, tfa, c, joined]) => (MEM[k] = { name, email: name.split(" ")[0].toLowerCase() + "@agentgate.test", job, role, tfa, c, joined }));
const joinedOf = (k) => JOIN[k] || MEM[k].joined;
const memKeys = () => Object.keys(MEM).filter((k) => !S.removed.includes(k));

const TEAMS = [
  { id: "engineering", n: "Engineering", d: "Backend dan frontend aplikasi toko.", lead: "AA", m: ["AA", "DD", "QQ", "BS", "FN", "HG", "YP", "TP"], p: [["KSR", "Member"], ["GDG", "Member"]] },
  { id: "mobile", n: "Mobile", d: "Aplikasi kasir Android dan portal pelanggan versi mobile.", lead: "AW", m: ["AW", "RM", "ES", "RF"], p: [["PPL", "Member"]] },
  { id: "produk", n: "Produk", d: "Product manager dan analis data.", lead: "OO", m: ["OO", "DK", "MA", "NS"], p: [["KSR", "Viewer"], ["PPL", "Member"]] },
  { id: "desain", n: "Desain", d: "Alur, wireframe, dan mockup UI.", lead: "SR", m: ["SR", "IP"], p: [["KSR", "Member"]] },
  { id: "qa", n: "QA", d: "Pengujian manual dan otomatis.", lead: "PL", m: ["PL", "WS", "AL"], p: [["KSR", "Member"], ["GDG", "Member"]] },
  { id: "data", n: "Data & Laporan", d: "Belum ada anggota.", lead: null, m: [], p: [] },
];
const team = (id) => TEAMS.find((t) => t.id === id) || TEAMS[0];
const teamOf = (k) => TEAMS.find((t) => t.m.includes(k));
const PROV = { github: ["GitHub", "brand-github", "github.com/pt-contoh/"], gitlab: ["GitLab", "brand-gitlab", "gitlab.com/pt-contoh/"], bitbucket: ["Bitbucket", "brand-bitbucket", "bitbucket.org/pt-contoh/"] };

const PERMS = [
  ["ws", "Workspace", "building", [["ws.manage", "Kelola workspace", "Ubah nama, alamat URL, metode masuk, dan keamanan."], ["ws.members", "Kelola anggota", "Undang, ubah role, dan keluarkan anggota."], ["ws.teams", "Kelola tim dan jabatan", "Membuat tim dan mengatur jabatan anggota."], ["ws.audit", "Lihat audit log", "Membaca riwayat perubahan workspace."], ["ws.delete", "Hapus workspace", "Menjadwalkan atau membatalkan penghapusan workspace.", "owner"]]],
  ["project", "Project", "folder", [["p.create", "Buat project", "Membuat project baru di workspace."], ["p.access", "Kelola akses project", "Memberi akses tim atau orang ke project."], ["p.repo", "Kelola repo", "Menambah repo dan memilih jalur kode ke server."], ["p.workflow", "Kelola workflow dan gate", "Mengubah stage, gate, dan executor."]]],
  ["ticket", "Tiket dan agent", "ticket", [["t.edit", "Buat dan ubah tiket", "Mengisi tiket, komentar, dan lampiran."], ["t.gate", "Setujui gate dan review", "Menyetujui hasil agent di stage yang butuh review manusia."], ["t.force", "Paksa transisi stage", "Melewati gate. Selalu tercatat di audit log."], ["t.run", "Jalankan agent", "Memicu job agent di runner milik sendiri."]]],
];
const ALLP = PERMS.flatMap(([, , , x]) => x.map((p) => p[0]));
const ROLES = {
  owner: { n: "Owner", b: true, d: "Pemilik workspace. Semua izin, termasuk menghapus workspace.", p: ALLP },
  admin: { n: "Admin", b: true, d: "Mengelola workspace dan semua project.", p: ALLP.filter((x) => x !== "ws.delete") },
  member: { n: "Member", b: true, d: "Bekerja di tiket dan menjalankan agent.", p: ["ws.audit", "p.create", "t.edit", "t.gate", "t.run"] },
  viewer: { n: "Viewer", b: true, d: "Hanya melihat board, tiket, dan laporan.", p: [] },
  "release-manager": { n: "Release Manager", b: false, d: "Menyetujui gate rilis dan boleh memaksa transisi.", p: ["ws.audit", "p.workflow", "t.gate", "t.force"] },
};
const ROLE_ORDER = ["Owner", "Admin", "Member", "Viewer", "Release Manager"];
const roleCount = (n) => memKeys().filter((k) => MEM[k].role === n).length;

/* Audit log: template kejadian + 60 baris contoh. */
const AT = {
  repo: ["Repo ditambahkan", "project.repo_added", "project", (o) => [["Nama", `<span class=mono>${o}</span>`], ["Branch", "<span class=mono>main</span>"], ["Jalur kode", "Clone dari provider"]]],
  repo_rm: ["Repo dilepas", "project.repo_removed", "project", (o) => [["Nama", `<span class=mono>${o}</span>`], ["Alasan", "Dipindah ke project lain"]]],
  forced: ["Transisi paksa", "ticket.forced_transition", "agent", () => [["Dari", "Testing"], ["Ke", "Development"], ["Gate dilewati", "Hasil test lulus"], ["Alasan", "Total struk selisih, perlu perbaikan kode"]]],
  job: ["Job agent dijalankan", "job.started", "agent", () => [["Persona", "Developer"], ["Runner", "Laptop Dewi"], ["Client", "Claude Code"], ["Pemicu", "Stage Development"]]],
  job_fail: ["Job agent gagal", "job.failed", "agent", () => [["Persona", "QA"], ["Percobaan", "3 dari 3"], ["Penyebab", "Total struk selisih Rp1.500"]]],
  access: ["Akses project diberikan", "project.access_granted", "project", () => [["Penerima", "Tim Engineering"], ["Role", "Member"]]],
  access_rm: ["Akses project dicabut", "project.access_revoked", "project", () => [["Penerima", "Lina Marlina"], ["Role sebelumnya", "Viewer"]]],
  invited: ["Anggota diundang", "member.invited", "member", (o) => [["Email", o], ["Role", "Member"], ["Berlaku", "7 hari"]]],
  joined: ["Anggota bergabung", "member.joined", "member", (o) => [["Email", o], ["Lewat", "Undangan"]]],
  role: ["Role anggota diubah", "member.role_changed", "member", (o) => [["Anggota", o], ["Sebelum", "Member"], ["Sesudah", "Release Manager"]]],
  removed: ["Anggota dikeluarkan", "member.removed", "member", (o) => [["Anggota", o], ["Runner diputus", "Tidak ada"]]],
  gate: ["Gate disetujui", "gate.approved", "agent", () => [["Stage", "Code Review"], ["Gate", "Review AI selesai, CI hijau"]]],
  gate_no: ["Gate ditolak", "gate.rejected", "agent", () => [["Stage", "Desain UI"], ["Catatan", "Tombol bayar terlalu kecil di layar kasir"]]],
  tfa_req: ["Wajib 2FA diaktifkan", "workspace.2fa_required", "security", () => [["Sebelum", "Opsional"], ["Sesudah", "Wajib"]]],
  tfa_on: ["2FA diaktifkan", "auth.2fa_enabled", "security", (o) => [["Akun", o], ["Metode", "Aplikasi authenticator"]]],
  fail: ["Masuk gagal 3 kali", "auth.login_failed", "security", () => [["Alasan", "Password salah"], ["Tindakan", "Rate limit 5 menit"]]],
  role_new: ["Role dibuat", "role.created", "member", () => [["Izin", "4 izin"]]],
  team_new: ["Tim dibuat", "team.created", "member", (o) => [["Nama", o]]],
  team_add: ["Anggota tim ditambahkan", "team.member_added", "member", (o) => [["Tim", o], ["Jumlah", "2 orang"]]],
  wf: ["Workflow diubah", "project.workflow_updated", "project", () => [["Perubahan", "Gate “Screenshot ada” ditambahkan di Testing"]]],
  proj_new: ["Project dibuat", "project.created", "project", (o) => [["Nama", o], ["Workflow", "Dari template"]]],
  runner: ["Runner didaftarkan", "runner.registered", "agent", (o) => [["Nama", o], ["Versi CLI", "0.4.2"]]],
  perm: ["Izin perintah diberikan", "agent.permission_granted", "agent", () => [["Perintah", "<code>pnpm test:e2e</code>"], ["Berlaku", "Sekali"]]],
  token: ["Token API dibuat", "api.token_created", "security", (o) => [["Nama", o], ["Cakupan", "tickets:read, jobs:write"], ["Kedaluwarsa", "90 hari"]]],
  ws_new: ["Workspace dibuat", "workspace.created", "security", () => [["Alamat URL", "<span class=mono>agentgate.id/w/pt-contoh</span>"]]],
  ws_upd: ["Pengaturan workspace diubah", "workspace.updated", "security", () => [["Zona waktu", "Asia/Jakarta (WIB)"]]],
};
const AU = [
  ["12 Nov", "10.42", "AA", "repo", "kasir-api", "KSR"], ["12 Nov", "10.31", "OO", "forced", "KSR-30", "KSR"], ["12 Nov", "10.18", "DD", "job", "KSR-26", "KSR"],
  ["12 Nov", "10.05", "OO", "access", "Kasir Online", "KSR"], ["12 Nov", "09.58", "QQ", "job_fail", "KSR-30", "KSR"], ["12 Nov", "09.51", "OO", "invited", "rina@agentgate.test"],
  ["12 Nov", "09.44", "AA", "gate", "KSR-22", "KSR"], ["12 Nov", "09.30", "OO", "tfa_req", "PT Contoh"], ["12 Nov", "09.12", "QQ", "fail", "qa@agentgate.test"],
  ["12 Nov", "08.47", "TP", "gate", "KSR-21", "KSR"], ["12 Nov", "08.20", "RM", "job", "PPL-8", "PPL"], ["12 Nov", "08.02", "HG", "runner", "PC Qori"],
  ["11 Nov", "21.02", "OO", "role_new", "Release Manager"], ["11 Nov", "19.40", "AW", "repo", "portal-mobile", "PPL"], ["11 Nov", "17.15", "SR", "gate_no", "KSR-28", "KSR"],
  ["11 Nov", "16.02", "OO", "role", "Teguh Prasetyo"], ["11 Nov", "15.30", "AA", "wf", "Kasir Online", "KSR"], ["11 Nov", "14.11", "FN", "perm", "KSR-27", "KSR"],
  ["11 Nov", "13.25", "BY", "joined", "bayu@agentgate.test"], ["11 Nov", "11.50", "OO", "invited", "bayu@agentgate.test"], ["11 Nov", "10.12", "LM", "joined", "lina@agentgate.test"],
  ["11 Nov", "09.05", "YP", "job", "GDG-14", "GDG"], ["10 Nov", "18.33", "AA", "token", "Token CI kasir-web"], ["10 Nov", "16.20", "DK", "gate", "PPL-5", "PPL"],
  ["10 Nov", "14.48", "PL", "job_fail", "GDG-11", "GDG"], ["10 Nov", "11.02", "OO", "access_rm", "Kasir Online", "KSR"], ["10 Nov", "09.15", "ES", "fail", "eko@agentgate.test"],
  ["9 Nov", "20.10", "AL", "joined", "ayu@agentgate.test"], ["9 Nov", "17.42", "RF", "joined", "reza@agentgate.test"], ["9 Nov", "15.30", "AW", "team_add", "Mobile"],
  ["9 Nov", "13.05", "MA", "gate", "PPL-3", "PPL"], ["8 Nov", "16.40", "HG", "wf", "Aplikasi Gudang", "GDG"], ["8 Nov", "10.22", "TP", "forced", "GDG-9", "GDG"],
  ["7 Nov", "19.55", "BS", "job", "KSR-21", "KSR"], ["7 Nov", "14.30", "AA", "repo_rm", "kasir-legacy", "KSR"], ["6 Nov", "11.11", "WS", "fail", "wulan@agentgate.test"],
  ["5 Nov", "16.25", "MA", "joined", "maya@agentgate.test"], ["5 Nov", "15.40", "ES", "joined", "eko@agentgate.test"], ["5 Nov", "09.00", "OO", "invited", "maya@agentgate.test"],
  ["4 Nov", "18.12", "DD", "gate", "KSR-22", "KSR"], ["3 Nov", "13.37", "YP", "joined", "yoga@agentgate.test"], ["3 Nov", "13.20", "NS", "joined", "nadia@agentgate.test"],
  ["3 Nov", "10.05", "AW", "proj_new", "Portal Pelanggan", "PPL"], ["31 Okt", "15.48", "SR", "tfa_on", "siti@agentgate.test"], ["29 Okt", "11.20", "WS", "joined", "wulan@agentgate.test"],
  ["29 Okt", "09.44", "VV", "joined", "viewer@agentgate.test"], ["27 Okt", "16.30", "IP", "joined", "indah@agentgate.test"], ["27 Okt", "16.02", "HG", "joined", "hendra@agentgate.test"],
  ["24 Okt", "10.10", "AA", "team_new", "QA"], ["20 Okt", "14.00", "DK", "joined", "dian@agentgate.test"], ["20 Okt", "13.41", "FN", "joined", "fajar@agentgate.test"],
  ["16 Okt", "09.30", "AA", "proj_new", "Aplikasi Gudang", "GDG"], ["13 Okt", "11.15", "PL", "joined", "putri@agentgate.test"], ["13 Okt", "11.02", "RM", "joined", "rizky@agentgate.test"],
  ["9 Okt", "15.20", "OO", "team_new", "Desain"], ["6 Okt", "10.40", "BS", "joined", "budi@agentgate.test"], ["6 Okt", "10.31", "TP", "joined", "teguh@agentgate.test"],
  ["3 Okt", "08.55", "AW", "joined", "agus@agentgate.test"], ["2 Okt", "21.10", "OO", "proj_new", "Kasir Online", "KSR"], ["2 Okt", "20.37", "OO", "ws_new", "PT Contoh"],
];
const AUDIT_CAT = { member: "Anggota dan role", project: "Project dan repo", agent: "Tiket, gate, dan job", security: "Keamanan dan workspace" };
const MON = { Okt: 10, Nov: 11 };
const AUDIT = AU.map(([d, t, w, key, o, proj], i) => {
  const [a, code, cat, ch] = AT[key];
  const [dd, mm] = d.split(" "), day = +dd;
  const range = mm === "Nov" && day === 12 ? "Hari ini" : mm === "Nov" && day === 11 ? "Kemarin" : mm === "Nov" && day >= 5 ? "7 hari terakhir" : "Lebih lama";
  return { i, d, t, who: w, a, code, cat, o, proj, ch: ch(o), range, key: `2026-${MON[mm]}-${String(day).padStart(2, "0")} ${t}`, ip: ["OO", "AA", "DD"].includes(w) ? "10.0.4." + (7 + i % 20) : i % 3 ? "36.72.18." + (100 + i) : "10.0.5." + (20 + i % 30) };
});

/* ====================== Helper ====================== */
const go = (r) => (location.hash = r);
const logo = (cls = "") => `<span class="logo ${cls}"><span class="mk">${ic("robot", "s14")}</span>Agent Gate</span>`;
const pw = (value = "", { ph = "", err = false, a = {} } = {}) => `<label class="inp m1-pw ${err ? "err" : ""}"><input type="password" value="${esc(value)}" placeholder="${esc(ph)}"${AG.attrs(a)}><button type="button" class="m1-eye" data-act="m1-eye" aria-label="Tampilkan password">${ic("eye", "s14")}</button></label>`;
const otp = (vals = []) => `<div class="m1-otp" role="group" aria-label="Kode 6 digit">${[0, 1, 2, 3, 4, 5].map((i) => `<input inputmode="numeric" maxlength="1" autocomplete="one-time-code" aria-label="Digit ${i + 1}" value="${vals[i] || ""}">`).join("")}</div>`;
const wireOtp = (root) => root.querySelectorAll(".m1-otp").forEach((w) => {
  const ins = [...w.querySelectorAll("input")];
  ins.forEach((x, i) => {
    x.addEventListener("input", () => { x.value = x.value.replace(/\D/g, "").slice(-1); if (x.value && ins[i + 1]) ins[i + 1].focus(); });
    x.addEventListener("keydown", (e) => { if (e.key === "Backspace" && !x.value && ins[i - 1]) ins[i - 1].focus(); });
    x.addEventListener("paste", (e) => { const d = (e.clipboardData.getData("text") || "").replace(/\D/g, "").slice(0, 6); if (d) { e.preventDefault(); d.split("").forEach((c, j) => ins[j] && (ins[j].value = c)); ins[Math.min(d.length, 5)].focus(); } });
  });
});
/* Avatar & identitas untuk semua anggota (termasuk anggota tambahan lokal). */
const mav = (k, s = 22) => (P[k] ? av(k, s) : `<span class="av" style="width:${s}px;height:${s}px;background:${MEM[k]?.c || "#64748b"};font-size:${Math.round(s * 0.4)}px" title="${esc(MEM[k]?.name || k)}">${k}</span>`);
const mwho = (k, sub) => `<div class="who">${mav(k, 28)}<span><b class="trunc">${MEM[k].name}</b><small>${sub ?? MEM[k].email}</small></span></div>`;
const nm = (k) => (MEM[k] ? MEM[k].name : AG.PERSONAS[k] ? "Agent " + AG.PERSONAS[k].n : k);
const feed = (items) => `<div class="box m1-feed"><div class="feed">${items.map(([k, t, w]) => `<div class="fe">${MEM[k] ? mav(k, 22) : av(k, 22)}<span class="grow"><b class="b5">${nm(k)}</b> <span class="mut">${t}</span></span><span class="faint sm nowrap">${w}</span></div>`).join("")}</div></div>`;
const tile = (icon, tone = "") => `<span class="tile ${tone}">${ic(icon, "s14")}</span>`;
const card = AG.card;
const dzCard = (body, desc = "") => card({ title: "Zona berbahaya", desc, pad: false, cls: "m1-dng", body });
const footBar = (inner) => `<div class="savebar">${inner}</div>`;
const set = (title, body, width = "narrow", extra = {}) => shell({ nav: "settings", crumbs: ["Pengaturan", ...title], body, width, ...extra });
const srowT = (t, d, ctl) => srow(t, d, ctl).replace('class="srow ', 'class="srow m1-top ');
const dangerRow = (t, d, ctl) => `<div class="srow m1-dz"><div><div class="t">${t}</div><div class="d">${d}</div></div><div class="ctl">${ctl}</div></div>`;
const ticketsOf = (k) => AG.TICKETS.filter((t) => t.k.startsWith(k + "-"));
const initials = (n) => n.split(/\s+/).filter(Boolean).map((w) => w[0]).join("").slice(0, 2).toUpperCase();
const projColor = (k) => S.color[k] || AG.project(k).c;
const projLogo = (k, s = 40, r = 10) => { const p = AG.project(k), img = S.logo[k]; return `<span class="m1-mini" style="width:${s}px;height:${s}px;border-radius:${r}px;background:${img ? "transparent" : projColor(k)};font-size:${Math.round(s * 0.36)}px" data-m1-fb="${k}">${img ? `<img src="${img}" alt="">` : initials(p.n)}</span>`; };
const projPeople = (k) => {
  if (k !== "KSR") return { GDG: ["AA", "DD", "QQ", "HG", "PL", "WS", "AL", "YP"], PPL: ["OO", "AW", "RM", "ES", "RF", "DK", "MA"] }[k] || ["OO"];
  const s = new Set(); S.access.forEach((a) => (a.t === "team" ? team(a.id).m.forEach((x) => s.add(x)) : s.add(a.id))); return [...s].filter((x) => MEM[x] && !S.removed.includes(x));
};
const TZ = ["Asia/Jakarta (WIB)", "Asia/Makassar (WITA)", "Asia/Jayapura (WIT)", "Asia/Singapore (SGT)", "Asia/Kuala_Lumpur (MYT)", "Asia/Bangkok (ICT)", "Asia/Tokyo (JST)", "Australia/Sydney (AEDT)", "Europe/Amsterdam (CET)", "UTC"];
const tzCombo = (v) => AG.combo({ options: TZ.map((z) => ({ v: z, l: z })), value: v, ph: "Pilih zona waktu" });
const peopleOpts = (keys) => keys.map((k) => ({ v: k, l: MEM[k].name, sub: [MEM[k].job, MEM[k].email].filter(Boolean).join(" · "), lead: mav(k, 20) }));
const teamLead = `<span class="tile" style="width:20px;height:20px;border-radius:5px">${ic("users-group", "s12")}</span>`;
const comboOf = (sel) => { const b = document.querySelector(`${sel} [data-combo]`); return b ? AG._combos[b.dataset.combo].value : null; };

/* Tab lokal & subpage yang ingat posisinya saat render ulang. */
const tabKey = (n) => AG.route.path + "|" + n;
const tabOn = (n, def) => S.tab[tabKey(n)] || def;
document.addEventListener("click", (e) => {
  const t = e.target.closest?.("[data-tab]"); if (!t) return;
  const r = t.closest("[data-m1tabs]"); if (r) S.tab[tabKey(r.dataset.m1tabs)] = t.dataset.tab;
}, true);
const tabs = (name, items, def) => { const on = tabOn(name, def); return `<div class="m1-tabs" data-tabs-root data-m1tabs="${name}"><div class="tabs" role="tablist">${items.map(([k, l, n]) => `<button class="${k === on ? "on" : ""}" data-tab="${k}" role="tab">${l}${n != null ? ` <span class="n">${n}</span>` : ""}</button>`).join("")}</div>${items.map(([k, , , body]) => `<div data-panel="${k}" class="${k === on ? "on" : ""}">${body}</div>`).join("")}</div>`; };
const sub = (name, def, nav, panels, opts = {}, extra = "") => { const on = tabOn(name, def); return `<div data-m1tabs="${name}">${AG.subpage({ ...opts, nav, active: on, body: panels.map(([k, html]) => AG.panel(k, html, k === on)).join("") + extra })}</div>`; };

/* Unggah gambar (logo project/workspace, foto profil). */
const SAMPLE_LOGO = "data:image/svg+xml," + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#8b5cf6"/><stop offset="1" stop-color="#5b21b6"/></linearGradient></defs><rect width="64" height="64" fill="url(#g)"/><g fill="none" stroke="#fff" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17h5l4 20h22l4-14H19"/><circle cx="24" cy="45" r="3"/><circle cx="40" cy="45" r="3"/></g><path d="M30 22h10" stroke="#c4b5fd" stroke-width="3" stroke-linecap="round"/></svg>`);
S.logo.KSR = SAMPLE_LOGO;
const uploader = ({ key, name, color, round = false, label = "logo", owner = "project", hint = "PNG, SVG, atau JPG. Persegi, minimal 256 px, maksimal 1 MB." }) => {
  const img = S.logo[key];
  return `<div class="m1-up" data-m1-drop="${key}">
    <span class="m1-up-prev ${round ? "round" : ""}" data-m1-fb="${key}" style="background:${img ? "transparent" : color}">${img ? `<img src="${img}" alt="${esc(label)} ${esc(name)}">` : initials(name)}</span>
    <div class="m1-up-body"><b class="b5">${img ? `${label[0].toUpperCase() + label.slice(1)} terpasang` : `Belum ada ${label}`}</b><span class="mut sm">${img ? hint : `Sementara memakai inisial <b class="fg b5">${initials(name)}</b> dengan warna ${owner}.`}</span>
      <div class="row wrap" style="margin-top:6px">${btn(img ? `Ganti ${label}` : `Unggah ${label}`, "sm", "upload", { "data-act": "m1-up", "data-arg": key })}${img ? btn("Hapus", "ghost sm", "trash", { "data-act": "m1-up-rm", "data-arg": key }) : `<span class="mut sm">atau tarik berkas ke sini</span>`}</div></div>
    <input type="file" accept="image/png,image/svg+xml,image/jpeg,image/webp" hidden data-m1-file="${key}" aria-label="Pilih berkas ${esc(label)}"></div>`;
};
const readImg = (key, file) => {
  if (!file) return;
  if (!/^image\//.test(file.type)) return AG.toast("Berkas harus gambar PNG, SVG, atau JPG", "dng");
  if (file.size > 1024 * 1024) return AG.toast("Ukuran maksimal 1 MB", "dng");
  const r = new FileReader(); r.onload = () => { S.logo[key] = r.result; AG.rerender(); AG.toast(`${esc(file.name)} diunggah. Simpan untuk menerapkan.`); }; r.readAsDataURL(file);
};
document.addEventListener("change", (e) => { const f = e.target.closest?.("[data-m1-file]"); if (f) readImg(f.dataset.m1File, f.files[0]); });
document.addEventListener("dragover", (e) => { const d = e.target.closest?.("[data-m1-drop]"); if (d) { e.preventDefault(); d.classList.add("over"); } });
document.addEventListener("dragleave", (e) => { const d = e.target.closest?.("[data-m1-drop]"); if (d && !d.contains(e.relatedTarget)) d.classList.remove("over"); });
document.addEventListener("drop", (e) => { const d = e.target.closest?.("[data-m1-drop]"); if (d) { e.preventDefault(); d.classList.remove("over"); readImg(d.dataset.m1Drop, e.dataTransfer.files[0]); } });
AG.action("m1-up", (el, key) => {
  document.querySelector(`[data-m1-file="${key}"]`)?.click();
});
AG.action("m1-up-rm", (el, key) => { S.logo[key] = null; AG.rerender(); AG.toast(key === "me" ? "Foto dihapus. Inisial dipakai." : "Logo dihapus. Inisial dan warna dipakai."); });

function qr() {
  const n = 25, c = 4; let r = 7, out = "";
  const rnd = () => (r = (r * 9301 + 49297) % 233280) / 233280;
  const finder = (x, y) => (x < 7 && y < 7) || (x >= n - 7 && y < 7) || (x < 7 && y >= n - 7);
  for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
    let on;
    if (finder(x, y)) { const fx = x >= n - 7 ? x - (n - 7) : x, fy = y >= n - 7 ? y - (n - 7) : y; on = fx === 0 || fx === 6 || fy === 0 || fy === 6 || (fx >= 2 && fx <= 4 && fy >= 2 && fy <= 4); }
    else on = rnd() > 0.52;
    if (on) out += `<rect x="${x * c}" y="${y * c}" width="${c}" height="${c}"/>`;
  }
  return `<svg class="m1-qr" viewBox="-8 -8 ${n * c + 16} ${n * c + 16}" role="img" aria-label="Kode QR untuk aplikasi authenticator"><g fill="#17171c">${out}</g></svg>`;
}

/* ====================== Autentikasi ====================== */
/* Panel kiri memakai AG.authBrand() (varian A/B/C dipilih pemilik produk). */
const authSplit = (form) => `<div class="split m1-auth">${AG.authBrand()}
  <div class="formside"><div class="formcol"><div class="only-sm">${logo()}</div>${form}</div></div></div>`;
const oauth = (verb = "Lanjut") => `<div class="or">atau</div><div class="col">${btn(`${verb} dengan Google`, "full", "brand-google", { "data-toast": "Mengalihkan ke Google…", "data-go": "#/home" })}${btn(`${verb} dengan GitHub`, "full", "brand-github", { "data-toast": "Mengalihkan ke GitHub…", "data-go": "#/home" })}</div>`;
const ttl = (h, sub) => `<div class="col g4"><h1>${h}</h1><p class="mut">${sub}</p></div>`;

AG.screen({ m: "M1", g: "Autentikasi", n: "Masuk", route: "/login",
  render: () => authSplit(`${ttl("Masuk ke Agent Gate", `Belum punya akun? <a class="link" href="#/signup">Daftar</a>`)}
    ${fld("Email", inp("owner@agentgate.test", { type: "email", ph: "nama@perusahaan.com" }))}
    <div class="fld"><div class="row"><label class="grow lab">Password</label><a class="link sm" href="#/forgot">Lupa password?</a></div>${pw("rahasia-toko-123")}</div>
    ${btn("Masuk", "pri full", "", { "data-go": "#/login/2fa" })}${oauth("Masuk")}
    <p class="mut xs">Dengan masuk, Anda menyetujui ketentuan layanan workspace Anda.</p>`) });

AG.screen({ m: "M1", g: "Autentikasi", n: "Daftar", route: "/signup",
  render: () => authSplit(`${ttl("Buat akun", `Sudah punya akun? <a class="link" href="#/login">Masuk</a>`)}
    ${fld("Nama lengkap", inp("Rina Pratama", { ph: "Nama Anda" }))}
    <div class="fld"><label>Email</label>${inp("rina@contoh", { type: "email", cls: "err", ph: "nama@perusahaan.com", a: { "data-m1": "email" } })}<span class="errt" data-m1="email-err">Format email belum benar, contoh: nama@perusahaan.com</span></div>
    ${fld("Password", pw("", { ph: "Minimal 12 karakter" }), "Minimal 12 karakter. Boleh pakai kalimat panjang.")}
    ${btn("Buat akun", "pri full", "", { "data-act": "m1-signup" })}${oauth("Daftar")}`),
  after: (root) => {
    const e = root.querySelector('[data-m1="email"]'), er = root.querySelector('[data-m1="email-err"]');
    e.addEventListener("input", () => { const ok = /^[^@\s]+@[^@\s]+\.[a-z]{2,}$/i.test(e.value); e.classList.toggle("err", !ok); er.style.display = ok ? "none" : ""; });
  } });
AG.action("m1-signup", () => {
  const e = document.querySelector('[data-m1="email"]');
  if (e && e.classList.contains("err")) { AG.toast("Periksa kembali format email", "dng"); e.focus(); return; }
  AG.toast("Akun dibuat. Sekarang buat workspace pertama Anda."); go("#/workspaces/new");
});

AG.screen({ m: "M1", g: "Autentikasi", n: "Lupa password", route: "/forgot",
  render: () => authSplit(`${ttl("Atur ulang password", "Kami kirim tautan untuk membuat password baru. Tautan berlaku 1 jam.")}
    ${fld("Email", inp("dewi@agentgate.test", { type: "email" }))}${btn(S.forgotSent ? "Kirim ulang tautan" : "Kirim tautan", "pri full", "", { "data-act": "m1-forgot" })}
    ${S.forgotSent ? `<div class="alert ok">${ic("circle-check")}<span>Jika email terdaftar, tautan sudah dikirim. Periksa juga folder spam.</span></div>` : ""}
    <a class="link row g4" href="#/login">${ic("arrow-left", "s14")}Kembali ke halaman masuk</a>`) });
AG.action("m1-forgot", () => { S.forgotSent = true; AG.rerender(); AG.toast("Tautan atur ulang dikirim"); });

AG.screen({ m: "M1", g: "Autentikasi", n: "Kode 2FA saat masuk", route: "/login/2fa",
  render: () => authSplit(S.recov
    ? `${ttl("Pakai kode pemulihan", "Masukkan salah satu kode pemulihan yang Anda simpan saat mengaktifkan 2FA. Setiap kode hanya berlaku sekali.")}
      ${fld("Kode pemulihan", inp("", { ph: "xxxxx-xxxxx", cls: "mono" }))}
      ${btn("Verifikasi", "pri full", "", { "data-go": "#/home", "data-toast": "Masuk dengan kode pemulihan. Sisa 7 kode." })}
      <p class="mut">Ada HP? <button class="link" data-act="m1-recov">Pakai kode authenticator</button></p>`
    : `${ttl("Masukkan kode autentikasi", "Buka aplikasi authenticator di HP Anda dan masukkan 6 digit kode untuk Agent Gate.")}
      ${otp(["4", "8", "2"])}
      <label class="row" style="width:max-content">${cb(true, "Percayai perangkat ini 30 hari")}<span class="mut">Percayai perangkat ini selama 30 hari</span></label>
      ${btn("Verifikasi", "pri full", "", { "data-go": "#/home" })}
      <p class="mut">HP tidak di tangan? <button class="link" data-act="m1-recov">Pakai kode pemulihan</button></p>`),
  after: (root) => { wireOtp(root); root.querySelectorAll(".m1-otp input")[3]?.focus(); } });
AG.action("m1-recov", () => { S.recov = !S.recov; AG.rerender(); });

AG.screen({ m: "M1", g: "Autentikasi", n: "Terima undangan", route: "/invite",
  render: () => authSplit(`<div class="col g12"><span class="wslogo" style="width:40px;height:40px;font-size:15px;border-radius:10px">PC</span><h1>Bergabung ke PT Contoh</h1>
      <p class="mut">${P.OO.name} mengundang Anda sebagai <b class="fg b5">Member</b> dengan jabatan <b class="fg b5">QA Engineer</b>. Undangan berlaku sampai 19 Nov 2026.</p></div>
    ${fld("Email", `<label class="inp ro">${ic("lock", "s14 faint")}<span>rina@agentgate.test</span></label>`, "Email mengikuti undangan dan tidak bisa diganti.")}
    ${fld("Nama lengkap", inp("Rina Pratama"))}${fld("Password", pw("", { ph: "Minimal 12 karakter" }), "Minimal 12 karakter.")}
    ${btn("Terima undangan", "pri full", "", { "data-go": "#/home", "data-toast": "Selamat bergabung di PT Contoh" })}
    <p class="mut">Sudah punya akun? <a class="link" href="#/login">Masuk untuk menerima</a></p>`) });

/* ====================== Workspace ====================== */
const solo = (inner, center = false) => `<div class="solo" ${center ? 'style="justify-content:center"' : ""}>${center ? "" : logo()}<div class="inner">${inner}</div></div>`;
AG.screen({ m: "M1", g: "Workspace", n: "Pilih workspace", route: "/workspaces",
  render: () => solo(`<div class="col g4" style="text-align:center"><h1 style="font-size:22px">Pilih workspace</h1><p class="mut">Anda anggota di 2 workspace.</p></div>
    <div class="box">${[["PC", "PT Contoh", "pt-contoh", "Owner", "var(--accent)", `3 project · ${memKeys().length} anggota`, "#/home", ""], ["BL", "Beta Labs", "beta-labs", "Member", "#0f8a5f", "1 project · 12 anggota", "#/home", "Beralih ke Beta Labs"]].map(([i, n, s, r, c, d, g, t]) => `<a class="lrow" href="${g}"${t ? ` data-toast="${t}"` : ""}><span class="wslogo" style="width:32px;height:32px;font-size:12px;background:${c}">${i}</span><div class="grow"><div class="b5">${n}</div><div class="mut sm"><span class="mono">/w/${s}</span> · ${d}</div></div>${pill(r)}${ic("chevron-right", "faint")}</a>`).join("")}</div>
    ${btn("Buat workspace baru", "", "plus", { "data-go": "#/workspaces/new" })}
    <div class="alert gray">${ic("mail")}<span>Mencari workspace lain? Minta Owner-nya mengirim undangan ke <b class="b5">owner@agentgate.test</b>.</span></div>
    <p class="mut" style="text-align:center">Masuk sebagai owner@agentgate.test · <a class="link" href="#/login">Keluar</a></p>`) });

AG.screen({ m: "M1", g: "Workspace", n: "Buat workspace", route: "/workspaces/new",
  render: () => solo(`<div class="col g4" style="text-align:center"><h1 style="font-size:22px">Buat workspace</h1><p class="mut">Workspace adalah tempat tim, project, dan agent Anda bekerja.</p></div>
    <div class="box col g16" style="padding:20px">
      ${fld("Nama workspace", inp("", { ph: "Misalnya PT Contoh", a: { "data-m1": "wsname" } }))}
      ${fld("Alamat URL", inp("", { pre: "agentgate.id/w/", ph: "pt-contoh", a: { "data-m1": "wsslug" } }), "Huruf kecil, angka, dan tanda hubung. Bisa diganti nanti.")}
      ${fld("Ukuran tim", `<div class="m1-prov" role="radiogroup" aria-label="Ukuran tim">${["1–10 orang", "11–50 orang", "51+ orang"].map((x, i) => `<button class="m1-pathcard ${i === 0 ? "on" : ""}" data-radio="m1-size" role="radio"><span class="radio ${i === 0 ? "on" : ""}"></span><span class="b5">${x}</span></button>`).join("")}</div>`)}
      ${btn("Buat workspace", "pri full", "", { "data-act": "m1-ws-create" })}</div>
    <a class="link row g4" style="justify-content:center" href="#/workspaces">${ic("arrow-left", "s14")}Kembali ke daftar workspace</a>`),
  after: (root) => {
    const n = root.querySelector('[data-m1="wsname"]'), s = root.querySelector('[data-m1="wsslug"]');
    n.addEventListener("input", () => { if (!s.dataset.touched) s.value = n.value.toLowerCase().normalize("NFD").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""); });
    s.addEventListener("input", () => (s.dataset.touched = "1"));
    n.focus();
  } });
AG.action("m1-ws-create", () => {
  const n = document.querySelector('[data-m1="wsname"]').value.trim();
  if (!n) { AG.toast("Isi nama workspace dulu", "dng"); document.querySelector('[data-m1="wsname"]').focus(); return; }
  AG.toast(`Workspace ${esc(n)} dibuat`); go("#/home");
});
/* ====================== Beranda ====================== */
const scheduledBanner = () => S.wsDel ? `<div class="alert dng">${ic("alert-triangle")}<span class="grow">PT Contoh dijadwalkan hapus permanen pada <b>12 Desember 2026</b>. Hanya Owner yang masih bisa membuka workspace ini.</span>${btn("Batalkan penghapusan", "sm", "", { "data-act": "m1-ws-undel" })}</div>` : "";
AG.screen({ m: "M1", g: "Workspace", n: "Beranda", route: "/home",
  render: () => {
    const need = AG.TICKETS.filter((t) => t.state === "review" || t.state === "keputusan");
    const working = AG.TICKETS.filter((t) => t.state === "jalan" && t.type !== "epic");
    const runnerOf = (t) => AG.RUNNERS.find((r) => r.job === t.k) || (t.runner && AG.RUNNERS.find((r) => r.n === t.runner));
    const open = AG.TICKETS.filter((t) => t.stage !== "done" && t.type !== "epic").length;
    const online = AG.RUNNERS.filter((r) => r.st !== "off").length;
    const nReview = need.filter((t) => t.state === "review").length, nDec = need.length - nReview;
    const waiting = need.length + (S.permDone ? 0 : 1);
    const needRow = (t) => {
      const st = AG.stage(t.stage), dec = t.state === "keputusan";
      return `<div class="lrow" data-go="#/t/${t.k}">${AG.typeIcon(t.type, "s18")}<div class="grow col g4" style="min-width:0"><div class="row"><span class="mono faint">${t.k}</span><span class="trunc b5">${t.t}</span></div><div class="row wrap sm mut">${AG.statePill(t.state)}<span class="row g4">${AG.stageIcon(t.stage)}${st.n}</span></div></div>
        <div class="acts">${btn(dec ? "Putuskan" : "Review", dec ? "" : "pri", "", { "data-go": `#/t/${t.k}` })}</div></div>`;
    };
    const perm = S.permDone ? "" : `<div class="lrow">${tile("hand-stop", "acc")}<div class="grow col g4" style="min-width:0"><div class="b5">Agent Developer minta izin menjalankan <code>prisma migrate deploy</code></div><div class="sm mut row wrap">${AG.tref("KSR-26")}<span>Laptop Dewi · 22 mnt lalu</span></div></div>
      <div class="acts">${btn("Tolak", "", "", { "data-act": "m1-perm", "data-arg": "no" })}${btn("Izinkan sekali", "pri", "", { "data-act": "m1-perm", "data-arg": "yes" })}</div></div>`;
    const agents = `<div class="box">${working.map((t) => { const s = AG.stage(t.stage), r = runnerOf(t), pct = { "KSR-26": 57, "KSR-29": 67, "KSR-33": 40 }[t.k] || 30; return `<div class="lrow" data-go="#/t/${t.k}">${av(s.persona, 28)}<div class="grow col g4" style="min-width:0"><div class="row"><span class="mono faint">${t.k}</span><span class="trunc b5">${t.t}</span></div><div class="row sm mut"><span class="trunc">${AG.PERSONAS[s.persona].n} · ${t.agent}</span></div><div class="bar" style="max-width:280px"><i style="width:${pct}%"></i></div></div><span class="row g4 sm mut hide-sm nowrap"><span class="live busy"></span>${r ? r.n : "Sesi interaktif Olivia"}</span></div>`; }).join("")}</div>`;
    const projects = `<div class="box">${AG.PROJECTS.map((p) => { const n = ticketsOf(p.k).filter((t) => t.stage !== "done").length, ppl = projPeople(p.k); return `<a class="lrow" href="#/p/${p.k}">${projLogo(p.k, 28, 7)}<div class="grow" style="min-width:0"><div class="row"><b class="b5 trunc">${p.n}</b><span class="mono faint">${p.k}</span></div><div class="mut sm trunc">${p.wf} · ${n ? n + " tiket terbuka" : "belum ada tiket terbuka"}</div></div><span class="stack hide-sm">${ppl.slice(0, 4).map((k) => mav(k, 22)).join("")}</span><span class="mut sm hide-sm tab-num" style="width:64px;text-align:right">${ppl.length} orang</span>${ic("chevron-right", "faint")}</a>`; }).join("")}</div>`;
    const activity = feed([["reviewer", "menyelesaikan review AI <b class=fg>KSR-24</b> dengan 3 catatan", "5 mnt"], ["qa", "gagal ketiga kalinya di <b class=fg>KSR-30</b>", "18 mnt"], ["DD", "memindahkan <b class=fg>KSR-26</b> ke Development", "40 mnt"], ["AA", "menambahkan repo <span class=mono>kasir-api</span>", "1 jam"], ["OO", "mewajibkan 2FA di workspace", "2 jam"], ["TP", "menyetujui gate Code Review di <b class=fg>KSR-21</b>", "2 jam"], ["BY", "bergabung ke workspace", "kemarin"]]);
    const body = `${scheduledBanner()}
      ${head("Selamat siang, Olivia", `Kamis, 12 November 2026 · ada ${waiting} hal yang menunggu Anda.`, btn("Project baru", "pri", "plus", { "data-dialog": "new-project" }))}
      ${AG.stats([
        { label: "Tiket terbuka", value: open, icon: "ticket", hint: "di 3 project", go: "#/my" },
        { label: "Menunggu Anda", value: waiting, icon: "eye", tone: "warn", hint: `${nReview} review · ${nDec} keputusan${S.permDone ? "" : " · 1 izin"}`, go: "#/inbox" },
        { label: "Agent bekerja", value: working.length, icon: "sparkles", tone: "acc", hint: "2 runner dan 1 sesi interaktif", go: "#/office" },
        { label: "Runner online", value: `${online}<span class="faint" style="font-size:15px;font-weight:500">/${AG.RUNNERS.length}</span>`, icon: "device-laptop", tone: "ok", hint: "ThinkPad Adi offline 3 jam", go: "#/settings/runner" },
      ])}
      <div class="m1-duo">
        ${card({ title: "Perlu tindakan Anda", desc: "Review gate, keputusan setelah gagal, dan izin agent.", action: AG.seeAll("#/inbox"), pad: false, body: `<div class="box m1-act">${perm}${need.map(needRow).join("")}</div>` })}
        ${card({ title: "Runner tim", desc: `${online} dari ${AG.RUNNERS.length} online`, action: AG.seeAll("#/settings/runner", "Kelola"), pad: false, body: `<div class="box">${AG.RUNNERS.map((r) => `<a class="lrow" href="#/settings/runner"><span class="live ${r.st === "busy" ? "busy" : r.st === "off" ? "off" : ""}"></span><div class="grow" style="min-width:0"><div class="b5 trunc">${r.n}</div><div class="sm mut trunc">${r.client} · ${r.st === "busy" ? "mengerjakan " + r.job : r.st === "off" ? "offline " + r.seen : "idle"}</div></div>${av(r.who, 20)}</a>`).join("")}</div>` })}
      </div>
      <div class="m1-two">
        ${card({ title: `Agent sedang bekerja<span class="m1-cnt">${working.length}</span>`, desc: "Job agent yang berjalan di runner tim.", action: AG.seeAll("#/office", "AI Office"), pad: false, body: agents })}
        ${card({ title: `Project<span class="m1-cnt">${AG.PROJECTS.length}</span>`, desc: "Project yang bisa Anda buka.", pad: false, body: projects })}
      </div>
      ${card({ title: "Aktivitas terbaru", desc: "Perubahan di semua project PT Contoh.", action: AG.seeAll("#/settings/audit", "Audit log"), pad: false, body: activity })}`;
    return shell({ crumbs: ["PT Contoh", "Beranda"], body, width: "mid" });
  } });
AG.action("m1-perm", (el, a) => { S.permDone = true; AG.rerender(); AG.toast(a === "yes" ? "Izin diberikan sekali untuk KSR-26" : "Permintaan ditolak. Agent diberi tahu."); });

/* ====================== Ringkasan project ====================== */
AG.screen({ m: "M1", g: "Project", n: "Ringkasan project", route: "/p/:k", path: "/p/KSR",
  render: ({ k }) => {
    const p = AG.project(k), ts = ticketsOf(p.k).filter((t) => t.type !== "subtask"), total = ts.length;
    const cnt = (s) => ts.filter((t) => t.stage === s).length;
    const done = cnt("done"), max = Math.max(1, ...AG.STAGES.map((s) => cnt(s.k)));
    const nst = (st) => ts.filter((t) => t.state === st).length;
    const progress = total ? `<div class="box">
        <div class="m1-pad col g8" style="border-bottom:1px solid var(--line)"><div class="row"><span class="b5 grow">${done} dari ${total} tiket selesai</span><span class="mut sm">Sprint 4 · berakhir 22 Nov</span></div>
          <div class="m1-stack" role="img" aria-label="Sebaran tiket per stage">${AG.STAGES.filter((s) => cnt(s.k)).map((s) => `<i style="width:${(cnt(s.k) / total) * 100}%;background:${s.c}" title="${s.n}: ${cnt(s.k)}"></i>`).join("")}</div></div>
        ${AG.STAGES.map((s) => `<a class="m1-stg" href="#/p/${p.k}/board" style="--m1c:${s.c}">${AG.stageIcon(s.k)}<span class="row trunc"><span class="trunc">${s.n}</span>${s.persona ? `<span class="faint sm hide-sm">· ${AG.PERSONAS[s.persona].n}</span>` : ""}</span><span class="tab-num mut" style="text-align:right">${cnt(s.k)}</span><span class="bar"><i style="width:${(cnt(s.k) / max) * 100}%"></i></span></a>`).join("")}</div>`
      : empty("layout-kanban", "Belum ada tiket", "Buat tiket pertama atau impor dari Jira untuk mulai mengisi board.", btn("Buka board", "", "", { "data-go": `#/p/${p.k}/board` }));
    const repos = S.repos[p.k] || [], ppl = projPeople(p.k);
    const about = `<div class="box"><div class="m1-pad col g8" style="border-bottom:1px solid var(--line)"><p>${p.k === "KSR" ? "Aplikasi kasir web untuk toko retail PT Contoh: transaksi penjualan, pembayaran tunai dan QRIS, cetak struk, serta validasi stok ke Aplikasi Gudang. Target rilis 1.4 akhir November dengan pembayaran digital." : p.d + "."}</p><p class="mut sm row g4">${ic("robot", "s14")}Agent membaca deskripsi ini sebagai konteks project.</p></div>
      <div class="m1-pad"><dl class="props"><dt>Status</dt><dd>${S.archived[p.k] ? pill("Diarsipkan", "gray") : pill("Aktif", "ok")}</dd><dt>Kunci</dt><dd>${pill(p.k, "mono")}</dd><dt>Lead</dt><dd class="row">${av("OO", 20)}Olivia Owner</dd><dt>Workflow</dt><dd><a class="link" href="#/settings/p/${p.k}/workflow">${p.wf}</a></dd><dt>Akses Anda</dt><dd>Admin <span class="mut sm">(Owner workspace)</span></dd><dt>Dibuat</dt><dd>${p.k === "KSR" ? "2 Okt 2026" : p.k === "GDG" ? "16 Okt 2026" : "3 Nov 2026"}</dd></dl></div></div>`;
    const activity = feed(p.k === "KSR" ? [["reviewer", "menyelesaikan review AI <b class=fg>KSR-24</b>", "5 mnt lalu"], ["OO", "memaksa <b class=fg>KSR-30</b> kembali ke Development", "40 mnt lalu"], ["AA", "menambahkan repo <span class=mono>kasir-api</span>", "1 jam lalu"], ["OO", "memberi akses <b class=fg>Engineering</b> sebagai Member", "2 jam lalu"], ["FN", "mendapat izin <code>pnpm test:e2e</code> di <b class=fg>KSR-27</b>", "kemarin"], ["OO", "membuat project <b class=fg>Kasir Online</b>", "2 Okt"]] : [["AA", "menambahkan repo", "kemarin"], ["OO", `membuat project <b class=fg>${p.n}</b>`, "kemarin"]]);
    const repoBox = repos.length ? `<div class="box">${repos.map((r) => `<a class="lrow" href="#/settings/p/${p.k}/repos">${ic(PROV[r.prov][1], "s18 mut")}<div class="grow" style="min-width:0"><div class="mono b5 trunc">${r.n}</div><div class="mut sm trunc">${r.path === "clone" ? "Clone dari provider" : "Upload lewat CLI"} · ${r.sync}</div></div>${pill(r.b, "mono gray", "", "git-branch")}</a>`).join("")}</div>` : empty("git-branch", "Belum ada repo", "Tambahkan repo agar agent bisa bekerja.", btn("Tambah repo", "", "plus", { "data-go": `#/settings/p/${p.k}/repos` }));
    const peopleBox = `<div class="box">${ppl.slice(0, 5).map((m) => `<div class="lrow" style="min-height:48px">${mwho(m, MEM[m].job || MEM[m].role)}<span class="grow"></span><span class="mut sm hide-sm">${teamOf(m)?.n || "Tanpa tim"}</span></div>`).join("")}${ppl.length > 5 ? `<div class="lrow mut sm" style="min-height:40px">dan ${ppl.length - 5} orang lainnya</div>` : ""}</div>`;
    const body = `<div class="page-head">${projLogo(p.k)}<div class="grow"><h1>${p.n}</h1><p>${p.d}</p></div><div class="acts">${btn("", "icon", "settings", { "data-go": `#/settings/p/${p.k}/general`, "aria-label": "Pengaturan project" })}${btn("Buka board", "pri", "layout-kanban", { "data-go": `#/p/${p.k}/board` })}</div></div>
      ${AG.stats([
        { label: "Tiket terbuka", value: total - done, icon: "ticket", hint: `${total} tiket di project ini`, go: `#/p/${p.k}/list` },
        { label: "Selesai", value: `${done}<span class="faint" style="font-size:15px;font-weight:500">/${total}</span>`, icon: "circle-check", tone: "ok", hint: "Sprint 4 · berakhir 22 Nov", go: `#/p/${p.k}/reports` },
        { label: "Menunggu review", value: nst("review"), icon: "eye", tone: "warn", hint: "PRD, desain, dan PR", go: `#/p/${p.k}/board` },
        { label: "Butuh keputusan", value: nst("keputusan"), icon: "alert-triangle", tone: "dng", hint: nst("keputusan") ? "KSR-30 gagal 3 kali" : "Tidak ada", go: nst("keputusan") ? "#/t/KSR-30" : `#/p/${p.k}/board` },
      ])}
      <div class="m1-duo">
        ${card({ title: "Progres per stage", desc: `Workflow ${p.wf}`, pad: !total, body: progress })}
        ${card({ title: "Tentang project", desc: "Deskripsi dan properti", action: AG.seeAll(`#/settings/p/${p.k}/general`, "Ubah"), pad: false, body: about })}
      </div>
      <div class="m1-duo">
        ${card({ title: "Aktivitas terbaru", desc: "Perubahan terakhir di project ini.", action: AG.seeAll("#/settings/audit", "Audit log"), pad: false, body: activity })}
        ${card({ title: `Anggota<span class="m1-cnt">${ppl.length}</span>`, desc: "Lewat tim dan akses langsung.", action: AG.seeAll(`#/settings/p/${p.k}/access`, "Kelola"), pad: false, body: peopleBox })}
      </div>
      ${card({ title: `Repo<span class="m1-cnt">${repos.length}</span>`, desc: "Kode yang boleh dibaca dan diubah agent.", action: AG.seeAll(`#/settings/p/${p.k}/repos`, "Kelola"), pad: !repos.length, body: repoBox })}`;
    return shell({ project: p.k, crumbs: AG.projectCrumb(p.k, "Ringkasan"), ...AG.projectTabs(p.k), actions: `<span class="stack hide-sm">${ppl.slice(0, 3).map((x) => mav(x, 24)).join("")}</span>`, body });
  } });

/* ====================== Pengaturan workspace ====================== */
AG.screen({ m: "M1", g: "Pengaturan workspace", n: "Umum", route: "/settings/general",
  render: () => {
    const no2fa = memKeys().filter((k) => !MEM[k].tfa).length;
    const tg = (label, toast, on = true) => sw(on, label);
    return set(["Workspace", "Umum"], `${scheduledBanner()}${sub("ws", "profile", [
      { k: "profile", label: "Profil workspace", icon: "building", desc: "Logo, nama, alamat URL" },
      { k: "signin", label: "Metode masuk", icon: "key", desc: "Email, Google, GitHub" },
      { k: "security", label: "Keamanan", icon: "shield-check", desc: "2FA wajib", count: no2fa, alert: true },
      { k: "danger", label: "Zona berbahaya", icon: "alert-triangle", desc: "Keluar atau hapus" },
    ], [
      ["profile", card({ title: "Profil workspace", desc: "Logo, nama, dan alamat yang dilihat anggota.", pad: false, foot: AG.savebar("Simpan perubahan", { toast: "Profil workspace disimpan" }), body: `<div class="box"><div class="m1-pad">${uploader({ key: "ws", name: "PT Contoh", color: "var(--accent)", owner: "workspace" })}</div>${srow("Nama", "Tampil di sidebar, email, dan undangan.", inp("PT Contoh"))}${srow("Alamat URL", "Mengganti alamat membuat tautan lama tidak berlaku.", inp("pt-contoh", { pre: "agentgate.id/w/" }))}${srow("Zona waktu", "Dipakai untuk tenggat, sprint, dan audit log.", tzCombo("Asia/Jakarta (WIB)"))}${srow("Bahasa", "Bahasa antarmuka dan email bawaan.", AG.combo({ options: ["Bahasa Indonesia", "English"], value: "Bahasa Indonesia" }))}</div>` })],
      ["signin", card({ title: "Metode masuk", desc: "Berlaku untuk semua anggota workspace.", pad: false, foot: AG.savebar("Simpan perubahan", { toast: "Metode masuk disimpan" }), body: `<div class="box">${srow("Email dan password", "Selalu aktif sebagai cadangan.", pill("Selalu aktif", "gray"))}${srow(`<span class="row">${ic("brand-google")}Google</span>`, "Anggota bisa masuk dengan akun Google.", tg("Masuk dengan Google", "Pengaturan masuk dengan Google disimpan"))}${srow(`<span class="row">${ic("brand-github")}GitHub</span>`, "Anggota bisa masuk dengan akun GitHub.", tg("Masuk dengan GitHub", "Pengaturan masuk dengan GitHub disimpan"))}${srow("Batasi domain email", "Hanya email @agentgate.test dan @tokomaju.id yang bisa diundang.", tg("Batasi domain email", "Pembatasan domain disimpan", false))}</div>` })],
      ["security", card({ title: "Keamanan", desc: "Aturan masuk untuk semua anggota workspace.", pad: false, foot: AG.savebar("Simpan perubahan", { toast: "Pengaturan keamanan disimpan" }), body: `<div class="box">${srow("Wajibkan autentikasi dua langkah", "Anggota tanpa 2FA diarahkan ke halaman setup sebelum bisa membuka workspace.", tg("Wajibkan 2FA", "Pengaturan 2FA workspace disimpan"))}${srow("Status anggota", `${no2fa} dari ${memKeys().length} anggota belum mengaktifkan 2FA.`, btn("Lihat anggota", "sm", "users", { "data-act": "m1-filter", "data-arg": "tfa:Belum" }))}${srow("Masa sesi", "Anggota diminta masuk ulang setelah tidak aktif.", AG.combo({ options: ["8 jam", "24 jam", "7 hari", "30 hari"], value: "7 hari" }))}</div>` })],
      ["danger", dzCard(`${dangerRow("Keluar dari workspace", "Akses Anda ke semua project PT Contoh berakhir.", btn("Keluar", "dng", "door-exit", { "data-dialog": "leave-workspace" }))}${S.wsDel ? dangerRow("Hapus workspace", "Dijadwalkan hapus permanen pada 12 Desember 2026.", btn("Batalkan penghapusan", "", "arrow-back-up", { "data-act": "m1-ws-undel" })) : dangerRow("Hapus workspace", "Semua project, repo, dan audit log dihapus permanen 30 hari setelah dijadwalkan. Selama masa itu masih bisa dibatalkan.", btn("Hapus workspace", "dng", "trash", { "data-dialog": "delete-workspace" }))}`, "Tindakan di sini berdampak ke seluruh workspace.")],
    ], { title: "Umum", desc: "Profil, metode masuk, dan keamanan workspace PT Contoh." })}`, "mid");
  } });
AG.action("m1-ws-del", () => { S.wsDel = true; AG.closeDialog(); AG.rerender(); AG.toast("Penghapusan dijadwalkan 12 Desember 2026"); });
AG.action("m1-ws-undel", () => { S.wsDel = false; AG.rerender(); AG.toast("Penghapusan workspace dibatalkan"); });

/* ---------- Anggota ---------- */
const MDT = "m1-members";
const memRows = () => memKeys();
AG.screen({ m: "M1", g: "Pengaturan workspace", n: "Anggota", route: "/settings/members",
  render: () => {
    const keys = memRows(); S.memKeys = keys;
    const pre = S.preTfa; S.preTfa = false;
    const table = AG.datatable({
      id: MDT, select: true, pageSize: 10, search: "Cari nama, email, atau tim",
      filters: [
        { k: "role", label: "Role", icon: "key", options: ROLE_ORDER },
        { k: "team", label: "Tim", icon: "users-group", options: [...TEAMS.map((t) => t.n), "Tanpa tim"] },
        { k: "job", label: "Jabatan", icon: "id-badge-2", options: [...JOBS, "Tanpa jabatan"] },
        { k: "tfa", label: "2FA", icon: "shield-check", options: ["Aktif", "Belum"], value: pre ? ["Belum"] : [] },
      ],
      columns: [{ label: "Nama" }, { label: "Jabatan", hide: "sm" }, { label: "Tim", hide: "sm" }, { label: "Role" }, { label: "2FA", hide: "sm" }, { label: "Bergabung", hide: "sm" }, { label: "", sort: false, w: "48px", align: "right" }],
      rows: keys.map((k) => {
        const m = MEM[k], t = teamOf(k), me = k === AG.ME;
        return {
          cells: [mwho(k, me ? m.email + " · Anda" : m.email), m.job || `<span class="faint">—</span>`, t ? `<a class="m1-tm" href="#/settings/teams/${t.id}">${ic("users-group", "s14 faint")}${t.n}</a>` : `<span class="faint">Tanpa tim</span>`,
            pill(m.role, m.role === "Owner" ? "acc" : m.role === "Admin" ? "info" : ""), m.tfa ? pill("Aktif", "ok") : pill("Belum", "warn"), `<span class="mut nowrap">${joinedOf(k)}</span>`,
            me ? "" : btn("", "ghost icon sm", "dots", { "data-menu": "member-actions", "data-arg": k, "aria-label": "Aksi untuk " + m.name })],
          sort: [m.name, m.job || "~", t?.n || "~", ROLE_ORDER.indexOf(m.role), m.tfa ? 1 : 0, dateKey(joinedOf(k)), ""],
          text: [m.name, m.email, m.job, t?.n, m.role].join(" "),
          filter: { role: m.role, team: t?.n || "Tanpa tim", job: m.job || "Tanpa jabatan", tfa: m.tfa ? "Aktif" : "Belum" },
        };
      }),
      bulk: `${btn("Ubah role", "sm", "key", { "data-menu": "m1-bulk-role" })}${btn("Masukkan ke tim", "sm", "users-group", { "data-menu": "m1-bulk-team" })}${btn("Ingatkan 2FA", "sm", "shield-check", { "data-act": "m1-bulk-2fa" })}${btn("Keluarkan", "sm dng", "logout", { "data-dialog": "m1-bulk-remove" })}`,
      empty: AG.empty("users", "Tidak ada anggota yang cocok", "Ubah kata kunci atau hapus filter."),
    });
    const inv = S.invites.length ? `<div class="box">${S.invites.map((v, i) => `<div class="lrow">${tile("mail")}<div class="grow" style="min-width:0"><div class="mono trunc">${v.e}</div><div class="mut sm">${v.r}${v.j ? " · " + v.j : ""} · diundang ${MEM[v.by].name} · berlaku sampai ${v.w}</div></div>${btn("Kirim ulang", "sm", "", { "data-toast": "Undangan dikirim ulang ke " + v.e })}${btn("", "ghost icon sm", "x", { "data-act": "m1-inv-cancel", "data-arg": i, "aria-label": "Batalkan undangan " + v.e })}</div>`).join("")}</div>` : empty("mail", "Tidak ada undangan tertunda", "Undangan yang belum diterima tampil di sini.", btn("Undang anggota", "", "users-plus", { "data-dialog": "invite-member" }));
    return set(["Workspace", "Anggota"], `
      ${head("Anggota", `${keys.length} anggota di PT Contoh. Role menentukan apa yang boleh dilakukan; tim dipakai untuk memberi akses project sekaligus.`, btn("Undang anggota", "pri", "users-plus", { "data-dialog": "invite-member" }))}
      ${tabs("members", [["list", "Anggota", keys.length, table], ["inv", "Undangan tertunda", S.invites.length, card({ title: "Undangan tertunda", desc: "Undangan berlaku 7 hari. Undangan lama untuk email yang sama dibatalkan otomatis.", pad: !S.invites.length, body: inv })]], "list")}`, "mid");
  } });
function dateKey(d) { const [dd, mm] = d.split(" "); return `${MON[mm] || 0}-${String(+dd).padStart(2, "0")}`; }
const selected = () => [...(AG._dts[MDT]?.sel || [])].map((i) => S.memKeys[i]).filter(Boolean);
AG.menu("m1-bulk-role", () => `<div class="mlab">Ubah role ${selected().length} anggota</div>${ROLE_ORDER.filter((r) => r !== "Owner").map((r) => `<button class="mi" data-act="m1-bulk-set" data-arg="role|${r}">${ic("key")}${r}</button>`).join("")}`);
AG.menu("m1-bulk-team", () => `<div class="mlab">Masukkan ke tim</div>${TEAMS.map((t) => `<button class="mi" data-act="m1-bulk-set" data-arg="team|${t.id}">${ic("users-group")}${t.n}<span class="tail">${t.m.length}</span></button>`).join("")}`);
AG.action("m1-bulk-set", (el, a) => {
  const [kind, v] = a.split("|"); const ks = selected().filter((k) => k !== AG.ME);
  if (!ks.length) return AG.toast("Anda tidak bisa mengubah role sendiri", "dng");
  if (kind === "role") ks.forEach((k) => (MEM[k].role = v));
  else { TEAMS.forEach((t) => (t.m = t.m.filter((x) => !ks.includes(x)))); team(v).m.push(...ks); }
  AG._dts[MDT].sel.clear(); AG.rerender();
  AG.toast(kind === "role" ? `Role ${ks.length} anggota diubah ke ${v}` : `${ks.length} anggota dimasukkan ke tim ${team(v).n}`);
});
AG.action("m1-bulk-2fa", () => { const ks = selected().filter((k) => !MEM[k].tfa); AG.toast(ks.length ? `Pengingat 2FA dikirim ke ${ks.length} anggota` : "Semua anggota terpilih sudah mengaktifkan 2FA"); });
AG.dialog("m1-bulk-remove", () => {
  const ks = selected().filter((k) => k !== AG.ME);
  return { title: `Keluarkan ${ks.length} anggota?`, icon: "logout", tone: "dng", desc: "Mereka kehilangan akses ke PT Contoh dan semua project. Tiket yang ditugaskan ke mereka tetap ada tanpa penerima.",
    body: `<div class="box">${ks.slice(0, 6).map((k) => `<div class="lrow" style="min-height:44px">${mwho(k)}</div>`).join("")}${ks.length > 6 ? `<div class="lrow mut sm">dan ${ks.length - 6} lainnya</div>` : ""}</div>${selected().includes(AG.ME) ? `<p class="hint">Anda sendiri tidak ikut dikeluarkan.</p>` : ""}`,
    foot: `${btn("Batal", "", "", { "data-close": true })}${btn(`Keluarkan ${ks.length} anggota`, "dngs", "", { "data-act": "m1-bulk-rm" })}` };
});
AG.action("m1-bulk-rm", () => { const ks = selected().filter((k) => k !== AG.ME); S.removed.push(...ks); AG._dts[MDT].sel.clear(); AG.closeDialog(); AG.rerender(); AG.toast(`${ks.length} anggota dikeluarkan`); });
AG.action("m1-filter", (el, a) => {
  const [, v] = a.split(":"); const f = AG._filters[MDT + "-f"];
  if (f) { f.filters.forEach((x) => (x.value = x.k === "tfa" ? [v] : [])); f.q = ""; } else S.preTfa = true;
  S.tab["/settings/members|members"] = "list"; AG._dts[MDT]?.sel.clear();
  if (AG.route.path !== "/settings/members") go("#/settings/members"); else AG.rerender();
});
AG.action("m1-inv-cancel", (el, i) => { const v = S.invites.splice(+i, 1)[0]; AG.rerender(); AG.toast(`Undangan ${v.e} dibatalkan`); });

/* Menu aksi anggota dengan submenu role dan jabatan */
AG.menu("member-actions", (k) => { const m = MEM[k], t = teamOf(k); return `<div class="mlab">${m.name}</div>
  <button class="m1-mi" data-act="m1-sub" data-arg="role:${k}">${ic("key")}Ubah role<span class="tail">${ic("chevron-right", "s14")}</span></button>
  <button class="m1-mi" data-act="m1-sub" data-arg="job:${k}">${ic("id-badge-2")}Ubah jabatan<span class="tail">${ic("chevron-right", "s14")}</span></button>
  <button class="m1-mi" data-act="m1-sub" data-arg="team:${k}">${ic("users-group")}Pindah tim<span class="tail">${ic("chevron-right", "s14")}</span></button>
  ${t ? `<a class="mi" href="#/settings/teams/${t.id}">${ic("users")}Lihat tim ${t.n}</a>` : ""}
  ${m.tfa ? "" : `<button class="mi" data-toast="Pengingat aktifkan 2FA dikirim ke ${m.email}">${ic("shield-check")}Ingatkan aktifkan 2FA</button>`}
  <div class="msep"></div>
  <button class="mi dng" data-dialog="remove-member" data-arg="${k}">${ic("logout")}Keluarkan dari workspace</button>`; });
function openSub(trigger) {
  const [kind, k] = trigger.dataset.arg.split(":");
  const parent = trigger.closest(".menu");
  if (!parent || (document.querySelector(".m1-sub")?.dataset.for === trigger.dataset.arg)) return;
  document.querySelectorAll(".m1-sub").forEach((m) => m.remove());
  parent.querySelectorAll(".m1-mi").forEach((x) => x.classList.toggle("hov", x === trigger));
  const opts = kind === "role" ? ROLE_ORDER.map((r) => [r, r]) : kind === "job" ? [...JOBS.map((j) => [j, j]), ["", "Tanpa jabatan"]] : [...TEAMS.map((t) => [t.id, t.n]), ["", "Tanpa tim"]];
  const cur = kind === "role" ? MEM[k].role : kind === "job" ? MEM[k].job : teamOf(k)?.id || "";
  const m = document.createElement("div"); m.className = "menu m1-sub"; m.dataset.for = trigger.dataset.arg; m.setAttribute("role", "menu");
  m.innerHTML = `<div class="mlab">${{ role: "Role", job: "Jabatan", team: "Tim" }[kind]}</div>${opts.map(([v, l]) => `<button class="mi" data-act="m1-set" data-arg="${kind}|${k}|${v}">${v === cur ? ic("check") : `<span style="width:16px"></span>`}${l}${kind === "role" && !ROLES[v.toLowerCase()] ? `<span class="tail">custom</span>` : ""}</button>`).join("")}`;
  document.body.appendChild(m);
  const pr = parent.getBoundingClientRect(), ir = trigger.getBoundingClientRect(), w = m.offsetWidth, h = m.offsetHeight;
  let x = pr.left - w - 4; if (x < 8) x = pr.right + 4; if (x + w > innerWidth - 8) { x = Math.max(8, innerWidth - w - 8); }
  let y = x >= pr.left - 4 && x <= pr.right ? ir.bottom + 4 : ir.top - 4; if (y + h > innerHeight - 8) y = Math.max(8, innerHeight - h - 8);
  m.style.left = x + "px"; m.style.top = y + "px";
}
AG.action("m1-sub", (el) => openSub(el));
document.addEventListener("mouseover", (e) => { const t = e.target.closest?.(".m1-mi[data-act='m1-sub']"); if (t) openSub(t); });
AG.action("m1-set", (el, a) => {
  const [kind, k, v] = a.split("|"); const m = MEM[k];
  if (kind === "role") m.role = v; else if (kind === "job") m.job = v;
  else { TEAMS.forEach((t) => (t.m = t.m.filter((x) => x !== k))); if (v) team(v).m.push(k); }
  AG.rerender(); AG.toast(kind === "role" ? `Role ${m.name} diubah ke ${v}` : kind === "job" ? `Jabatan ${m.name} diubah ke ${v || "tanpa jabatan"}` : v ? `${m.name} dipindah ke tim ${team(v).n}` : `${m.name} dikeluarkan dari tim`);
});

/* ---------- Tim ---------- */
const projNames = (t) => (t.p.length ? t.p.map(([k, r]) => `<span class="row g4 nowrap" style="display:inline-flex;margin-right:10px">${AG.projDot(k, 8)}${AG.project(k).n}<span class="faint">· ${r}</span></span>`).join("") : `<span class="faint">Belum punya akses project</span>`);
AG.screen({ m: "M1", g: "Pengaturan workspace", n: "Tim", route: "/settings/teams",
  render: () => set(["Workspace", "Tim"], `
    ${head("Tim", "Kelompokkan anggota untuk memberi akses project sekaligus. Satu orang bisa masuk satu tim utama.", btn("Tim baru", "pri", "plus", { "data-dialog": "new-team" }))}
    ${AG.datatable({ id: "m1-teams", search: "Cari tim atau anggota", pageSize: 10,
      filters: [{ k: "acc", label: "Akses project", icon: "folder", options: ["Punya akses", "Belum punya akses"] }],
      columns: [{ label: "Tim" }, { label: "Lead", hide: "sm" }, { label: "Anggota" }, { label: "Akses project", hide: "sm" }, { label: "", sort: false, w: "40px" }],
      rows: TEAMS.map((t) => { const ms = t.m.filter((k) => !S.removed.includes(k)); return {
        cells: [`<div class="row">${tile("users-group")}<div style="min-width:0"><div class="b5">${t.n}</div><div class="mut sm trunc" style="max-width:280px">${t.d}</div></div></div>`, t.lead ? `<span class="row">${mav(t.lead, 20)}<span class="trunc">${MEM[t.lead].name}</span></span>` : `<span class="faint">—</span>`,
          ms.length ? `<span class="row"><span class="stack">${ms.slice(0, 4).map((k) => mav(k, 22)).join("")}</span><span class="mut sm tab-num">${ms.length}</span></span>` : pill("Kosong", "gray"), projNames(t), ic("chevron-right", "faint")],
        sort: [t.n, t.lead ? MEM[t.lead].name : "~", ms.length, t.p.length, ""], text: [t.n, t.d, ...ms.map((k) => MEM[k].name)].join(" "), filter: { acc: t.p.length ? "Punya akses" : "Belum punya akses" } }; }),
      rowGo: (i) => `#/settings/teams/${TEAMS[i].id}` })}`, "mid") });

AG.screen({ m: "M1", g: "Pengaturan workspace", n: "Detail tim", route: "/settings/teams/:id", path: "/settings/teams/engineering",
  render: ({ id }) => {
    const t = team(id), ms = t.m.filter((k) => !S.removed.includes(k)), others = memKeys().filter((k) => !t.m.includes(k));
    const members = `<div class="lrow" style="gap:8px;flex-wrap:wrap;padding:0;border:0"><span class="grow" style="min-width:220px" data-m1="addmem">${AG.combo({ options: peopleOpts(others).map((o) => ({ ...o, sub: [MEM[o.v].job, teamOf(o.v) ? "Tim " + teamOf(o.v).n : "Tanpa tim"].filter(Boolean).join(" · ") })), multi: true, ph: "Pilih anggota workspace…", w: 340 })}</span>${btn("Tambahkan", "pri", "plus", { "data-act": "m1-team-add", "data-arg": t.id })}</div>
      ${AG.datatable({ id: "m1-tm-" + t.id, select: true, search: "Cari anggota tim", pageSize: 10,
        filters: [{ k: "job", label: "Jabatan", icon: "id-badge-2", options: [...JOBS, "Tanpa jabatan"] }, { k: "role", label: "Role", icon: "key", options: ROLE_ORDER }],
        columns: [{ label: "Nama" }, { label: "Jabatan", hide: "sm" }, { label: "Role", hide: "sm" }, { label: "", sort: false, w: "100px", align: "right" }],
        rows: ms.map((k) => ({ cells: [mwho(k, k === t.lead ? "Lead tim · " + MEM[k].email : MEM[k].email), MEM[k].job || `<span class="faint">—</span>`, pill(MEM[k].role), k === t.lead ? pill("Lead", "acc") : btn("Keluarkan", "ghost sm", "", { "data-act": "m1-team-rm", "data-arg": t.id + ":" + k })], sort: [MEM[k].name, MEM[k].job, MEM[k].role, ""], text: MEM[k].name + " " + MEM[k].email + " " + MEM[k].job, filter: { job: MEM[k].job || "Tanpa jabatan", role: MEM[k].role } })),
        bulk: btn("Keluarkan dari tim", "sm dng", "logout", { "data-act": "m1-team-rmsel", "data-arg": t.id }),
        empty: AG.empty("users-group", "Belum ada anggota", "Pilih anggota workspace di atas untuk mengisi tim ini.") })}`;
    const access = t.p.length ? `<div>${t.p.map(([k, r]) => `<a class="lrow" href="#/settings/p/${k}/access">${projLogo(k, 28, 7)}<div class="grow"><div class="b5">${AG.project(k).n}</div><div class="mut sm">${ms.length} anggota tim mendapat role ${r}</div></div>${pill(r)}${ic("chevron-right", "faint")}</a>`).join("")}</div>` : empty("folder", "Belum ada akses project", "Beri akses dari halaman Akses di pengaturan project.", btn("Atur akses Kasir Online", "", "", { "data-go": "#/settings/p/KSR/access" }));
    return set(["Tim", t.n], `
      ${head(t.n, `${ms.length} anggota · ${t.p.length ? "akses ke " + t.p.map(([k]) => AG.project(k).n).join(" dan ") : "belum punya akses project"}.`)}
      ${sub("team", "members", [
        { k: "profile", label: "Profil tim", icon: "settings", desc: "Nama, deskripsi, lead" },
        { k: "members", label: "Anggota", icon: "users", count: ms.length, desc: "Tambah dan keluarkan" },
        { k: "access", label: "Akses project", icon: "folder", count: t.p.length, desc: "Project yang bisa dibuka" },
        { k: "danger", label: "Zona berbahaya", icon: "alert-triangle", desc: "Hapus tim" },
      ], [
        ["profile", card({ title: "Profil tim", desc: "Nama, deskripsi, dan lead tim.", pad: false, foot: AG.savebar("Simpan tim", { toast: "Tim disimpan" }), body: `${srow("Nama tim", "", inp(t.n))}${srow("Deskripsi", "Opsional. Tampil di daftar tim.", inp(t.d))}${srow("Lead tim", "Dihubungi agent saat butuh keputusan tim.", AG.combo({ options: peopleOpts(ms.length ? ms : memKeys()), value: t.lead, ph: "Pilih lead" }))}` })],
        ["members", card({ title: "Anggota", desc: `Anggota yang sudah di tim lain akan dipindah ke ${t.n}.`, body: members })],
        ["access", card({ title: "Akses project", desc: "Diatur dari pengaturan akses tiap project.", pad: !t.p.length, body: access })],
        ["danger", dzCard(`${dangerRow("Hapus tim", "Akses project yang diberikan lewat tim ini ikut dicabut. Anggota tetap ada di workspace.", btn("Hapus tim", "dng", "trash", { "data-toast": `Tim ${t.n} dihapus`, "data-go": "#/settings/teams" }))}`)],
      ])}`, "mid");
  } });
AG.action("m1-team-add", (el, id) => {
  const v = comboOf('[data-m1="addmem"]') || []; if (!v.length) return AG.toast("Pilih anggota dulu", "dng");
  TEAMS.forEach((t) => (t.m = t.m.filter((x) => !v.includes(x)))); team(id).m.push(...v);
  AG.rerender(); AG.toast(v.length === 1 ? `${MEM[v[0]].name} ditambahkan ke tim` : `${v.length} anggota ditambahkan ke tim`);
});
AG.action("m1-team-rm", (el, a) => { const [id, k] = a.split(":"); const t = team(id); t.m = t.m.filter((x) => x !== k); AG.rerender(); AG.toast(`${MEM[k].name} dikeluarkan dari ${t.n}`); });
AG.action("m1-team-rmsel", (el, id) => {
  const t = team(id), d = AG._dts["m1-tm-" + id], ms = t.m.filter((k) => !S.removed.includes(k));
  const ks = [...d.sel].map((i) => ms[i]).filter((k) => k && k !== t.lead);
  t.m = t.m.filter((x) => !ks.includes(x)); d.sel.clear(); AG.rerender(); AG.toast(ks.length ? `${ks.length} anggota dikeluarkan dari ${t.n}` : "Lead tim tidak bisa dikeluarkan. Ganti lead dulu.", ks.length ? "" : "dng");
});

/* ---------- Jabatan ---------- */
AG.screen({ m: "M1", g: "Pengaturan workspace", n: "Jabatan", route: "/settings/jobs",
  render: () => set(["Workspace", "Jabatan"], `
    ${head("Jabatan", "Label peran kerja anggota, terpisah dari role akses. Dipakai untuk penugasan default dan agar agent tahu siapa yang diajak review. Pasang ke anggota lewat menu ⋯ di halaman <a class='link' href='#/settings/members'>Anggota</a>.", btn("Jabatan baru", "pri", "plus", { "data-dialog": "m1-job", "data-arg": "new" }))}
    ${AG.datatable({ id: "m1-jobs", search: "Cari jabatan", pageSize: 10,
      filters: [{ k: "use", label: "Pemakaian", icon: "users", options: ["Dipakai", "Belum dipakai"] }],
      columns: [{ label: "Jabatan" }, { label: "Anggota" }, { label: "Dipakai di tim", hide: "sm" }, { label: "", sort: false, w: "84px", align: "right" }],
      rows: JOBS.map((n, i) => { const ks = memKeys().filter((k) => MEM[k].job === n), ts = [...new Set(ks.map((k) => teamOf(k)?.n).filter(Boolean))]; return {
        cells: [`<span class="row">${ic("id-badge-2", "faint")}<b class="b5">${n}</b></span>`, ks.length ? `<span class="row"><span class="stack">${ks.slice(0, 4).map((k) => mav(k, 20)).join("")}</span><span class="mut sm tab-num">${ks.length}</span></span>` : `<span class="faint">Belum dipakai</span>`, ts.length ? `<span class="mut">${ts.join(", ")}</span>` : `<span class="faint">—</span>`,
          `<span class="row g4" style="justify-content:flex-end">${btn("", "ghost icon sm", "pencil", { "data-dialog": "m1-job", "data-arg": i, "aria-label": "Ubah " + n })}${btn("", "ghost icon sm", "trash", { "data-act": "m1-job-del", "data-arg": i, "aria-label": "Hapus " + n })}</span>`],
        sort: [n, ks.length, ts.join(), ""], text: n, filter: { use: ks.length ? "Dipakai" : "Belum dipakai" } }; }) })}`, "mid") });
AG.dialog("m1-job", (i) => {
  const isNew = i === "new", n = isNew ? "" : JOBS[+i];
  return { title: isNew ? "Jabatan baru" : `Ubah ${n}`, icon: "id-badge-2", tone: "acc",
    body: `${fld("Nama jabatan", inp(n, { ph: "Misalnya Mobile Developer", a: { "data-m1": "jobname" } }), isNew ? "" : `Dipakai ${memKeys().filter((k) => MEM[k].job === n).length} anggota; semuanya ikut berganti.`)}`,
    foot: `${btn("Batal", "", "", { "data-close": true })}${btn(isNew ? "Tambah jabatan" : "Simpan", "pri", "", { "data-act": "m1-job-save", "data-arg": i })}`,
    after: (el) => { const x = el.querySelector('[data-m1="jobname"]'); x.focus(); x.addEventListener("keydown", (e) => e.key === "Enter" && el.querySelector('[data-act="m1-job-save"]').click()); } };
});
AG.action("m1-job-save", (el, i) => {
  const v = document.querySelector('[data-m1="jobname"]').value.trim(); if (!v) return AG.toast("Nama jabatan tidak boleh kosong", "dng");
  if (JOBS.includes(v) && JOBS[+i] !== v) return AG.toast(`${esc(v)} sudah ada`, "dng");
  if (i === "new") { JOBS.push(v); AG.toast(`Jabatan ${esc(v)} ditambahkan`); }
  else { const old = JOBS[+i]; JOBS[+i] = v; Object.values(MEM).forEach((p) => p.job === old && (p.job = v)); AG.toast("Jabatan diubah"); }
  AG.closeDialog(); AG.rerender();
});
AG.action("m1-job-del", (el, i) => { const n = JOBS[+i]; if (memKeys().some((k) => MEM[k].job === n)) return AG.toast(`${n} masih dipakai anggota. Ganti jabatan mereka dulu.`, "dng"); JOBS.splice(+i, 1); AG.rerender(); AG.toast(`Jabatan ${n} dihapus`); });

/* ---------- Role ---------- */
AG.screen({ m: "M1", g: "Pengaturan workspace", n: "Role", route: "/settings/roles",
  render: () => { const ids = Object.keys(ROLES); return set(["Workspace", "Role"], `
    ${head("Role", "Kumpulan izin untuk anggota. Role bisa berbeda per project; Owner dan Admin workspace otomatis menjadi Admin di semua project.", btn("Role baru", "pri", "plus", { "data-go": "#/settings/roles/new" }))}
    ${AG.datatable({ id: "m1-roles", search: "Cari role", pageSize: 10,
      filters: [{ k: "type", label: "Jenis", icon: "category-2", options: ["Bawaan", "Custom"] }],
      columns: [{ label: "Role" }, { label: "Izin", hide: "sm" }, { label: "Anggota", align: "right" }, { label: "", sort: false, w: "40px" }],
      rows: ids.map((id) => { const r = ROLES[id]; const names = PERMS.flatMap(([, , , x]) => x).filter((p) => r.p.includes(p[0])).map((p) => p[1]); return {
        cells: [`<div><div class="row"><b class="b5">${r.n}</b>${r.b ? pill("Bawaan", "gray") : pill("Custom", "acc")}</div><div class="mut sm">${r.d}</div></div>`, `<span class="mut">${r.p.length === ALLP.length ? "Semua izin" : r.p.length ? `${r.p.length} izin · ${names.slice(0, 2).join(", ")}${names.length > 2 ? ", …" : ""}` : "Hanya melihat"}</span>`, `<span class="tab-num">${roleCount(r.n)}</span>`, ic("chevron-right", "faint")],
        sort: [ROLE_ORDER.indexOf(r.n), r.p.length, roleCount(r.n), ""], text: r.n + " " + r.d, filter: { type: r.b ? "Bawaan" : "Custom" } }; }),
      rowGo: (i) => `#/settings/roles/${ids[i]}` })}`, "mid"); } });

AG.screen({ m: "M1", g: "Pengaturan workspace", n: "Form role", route: "/settings/roles/:id", path: "/settings/roles/release-manager",
  render: ({ id }) => {
    const isNew = id === "new", r = isNew ? { n: "", b: false, d: "", p: [] } : ROLES[id] || ROLES["release-manager"];
    const lock = r.b, holders = memKeys().filter((k) => MEM[k].role === r.n);
    const permBox = (items) => `<div class="box m1-perms">${items.map(([pk, t, d, only]) => { const on = r.p.includes(pk), dis = lock || (only === "owner" && id !== "owner"); return `<div class="lrow">${dis ? `<span class="cb ${on ? "on" : ""}" style="margin-top:2px;opacity:.6" aria-disabled="true">${ic("check")}</span>` : `<span style="margin-top:2px">${cb(on, t)}</span>`}<div class="grow"><div class="b5">${t}</div><div class="mut">${d}${only === "owner" && id !== "owner" ? ` <span class="row g4 text-warn" style="display:inline-flex">${ic("lock", "s14")}Hanya untuk Owner</span>` : ""}</div></div></div>`; }).join("")}</div>`;
    const detail = lock ? `<div class="alert gray">${ic("lock")}<span>Role bawaan tidak bisa diubah. Duplikat untuk membuat versi Anda sendiri.</span></div><div class="box">${srow("Nama role", "", `<b class="b5">${r.n}</b>`)}${srow("Deskripsi", "", `<span class="mut">${r.d}</span>`)}</div>`
      : `<div class="box">${srow("Nama role", "Tampil di daftar anggota dan akses project.", inp(r.n, { ph: "Misalnya Release Manager" }))}${srow("Deskripsi", "Opsional.", inp(r.d, { ph: "Untuk apa role ini?" }))}</div>`;
    const saveRole = lock ? "" : AG.savebar(isNew ? "Buat role" : "Simpan role", { toast: isNew ? "Role dibuat" : "Role disimpan", msg: isNew ? "Role baru belum disimpan" : "" });
    const people = holders.length ? `<div class="box">${holders.map((k) => `<div class="lrow" style="min-height:48px">${mwho(k, MEM[k].job || MEM[k].email)}<span class="grow"></span><span class="mut sm hide-sm">${teamOf(k)?.n || "Tanpa tim"}</span></div>`).join("")}</div>` : empty("users", "Belum ada anggota", "Pasang role ini lewat menu ⋯ di halaman Anggota.", btn("Buka anggota", "", "", { "data-go": "#/settings/members" }));
    return set(["Role", isNew ? "Role baru" : r.n], `
      ${head(isNew ? "Role baru" : r.n, isNew ? "Pilih izin yang diberikan ke role ini." : `${r.b ? "Role bawaan" : "Role custom"} · ${r.p.length} dari ${ALLP.length} izin · ${holders.length} anggota`, r.b ? btn("Duplikat jadi role custom", "", "copy", { "data-go": "#/settings/roles/new", "data-toast": `Salinan ${r.n} dibuat` }) : "")}
      ${sub("role", "detail", [
        { k: "detail", label: "Detail", icon: "key", desc: "Nama dan deskripsi" },
        ...PERMS.map(([k, g, i, items]) => ({ k, label: g, icon: i, count: `${items.filter((p) => r.p.includes(p[0])).length}/${items.length}`, desc: items.length + " izin", group: "Izin" })),
        ...(isNew ? [] : [{ k: "people", label: "Anggota", icon: "users", count: holders.length, desc: "Pemegang role ini", group: "Lainnya" }]),
      ], [
        ["detail", `${card({ title: "Detail", desc: lock ? "Nama dan deskripsi role bawaan." : "Nama dan deskripsi role.", pad: lock, foot: saveRole, body: detail })}${!lock && !isNew ? dzCard(dangerRow("Hapus role", holders.length ? `${holders.length} anggota dipindah ke Member.` : "Belum dipakai anggota mana pun.", btn("Hapus role", "dng", "trash", { "data-toast": "Role dihapus", "data-go": "#/settings/roles" }))) : ""}`],
        ...PERMS.map(([k, g, , items]) => [k, card({ title: `Izin ${g.toLowerCase()}`, desc: lock ? "Hanya bisa dilihat. Duplikat role untuk mengubah izin." : "Centang izin yang diberikan ke role ini.", pad: false, foot: saveRole, body: permBox(items) })]),
        ["people", card({ title: "Anggota dengan role ini", desc: "Ubah role anggota lewat menu ⋯ di halaman Anggota.", action: holders.length ? AG.seeAll("#/settings/members", "Kelola") : "", pad: !holders.length, body: people })],
      ])}`, "mid");
  } });

/* ---------- Audit log ---------- */
const auditDetail = (e, close = true) => `${close ? `<div class="row"><b class="grow" style="font-size:15px">${e.a}</b>${btn("", "ghost icon sm", "x", { "data-act": "m1-audit", "data-arg": "-1", "aria-label": "Tutup detail" })}</div>` : ""}
  <span class="mono mut">${e.code}</span>
  <dl class="props"><dt>Waktu</dt><dd class="mono">${e.d} 2026, ${e.t} WIB</dd><dt>Pelaku</dt><dd class="row">${mav(e.who, 20)}<span class="trunc">${MEM[e.who].name}</span></dd>${e.proj ? `<dt>Project</dt><dd class="row">${AG.projDot(e.proj)}${AG.project(e.proj).n}</dd>` : ""}<dt>Objek</dt><dd class="trunc">${/^KSR-\d+$/.test(e.o) ? AG.tref(e.o) : esc(e.o)}</dd><dt>Kategori</dt><dd>${AUDIT_CAT[e.cat]}</dd><dt>Alamat IP</dt><dd class="mono">${e.ip}</dd></dl>
  <div class="col"><b class="b6">Perubahan</b><div class="box soft" style="padding:12px;display:grid;gap:6px">${e.ch.map(([k, v]) => `<div><span class="mut">${k}:</span> ${v}</div>`).join("")}</div></div>
  <div class="row wrap">${btn("Salin sebagai JSON", "", "copy", { "data-toast": "Kejadian disalin sebagai JSON" })}${e.proj ? btn("Buka project", "ghost", "external-link", { "data-go": `#/p/${e.proj}` }) : ""}</div>`;
AG.screen({ m: "M1", g: "Pengaturan workspace", n: "Audit log", route: "/settings/audit",
  render: () => {
    const cur = S.auditSel != null ? AUDIT[S.auditSel] : null, wideView = innerWidth >= 1100;
    const whoOpts = [...new Set(AUDIT.map((e) => MEM[e.who].name))].sort();
    return set(["Workspace", "Audit log"], `
      ${head("Audit log", "Riwayat perubahan di workspace ini, terbaru di atas. Waktu dalam WIB. Disimpan tanpa batas waktu.", btn("Ekspor CSV", "", "download", { "data-toast": "Ekspor audit log disiapkan, tautan dikirim ke email" }))}
      ${AG.datatable({ id: "m1-audit", pageSize: 25, search: "Cari kejadian, kode, atau objek",
        filters: [
          { k: "cat", label: "Kejadian", icon: "category-2", options: Object.values(AUDIT_CAT) },
          { k: "who", label: "Pelaku", icon: "user", options: whoOpts },
          { k: "proj", label: "Objek / project", icon: "folder", options: [...AG.PROJECTS.map((p) => p.n), "Workspace"] },
          { k: "range", label: "Rentang waktu", icon: "calendar", options: ["Hari ini", "Kemarin", "7 hari terakhir", "Lebih lama"] },
        ],
        columns: [{ label: "Waktu", w: "130px" }, { label: "Pelaku" }, { label: "Kejadian" }, { label: "Objek", hide: "sm" }, { label: "Project", hide: "sm" }],
        rows: AUDIT.map((e) => ({
          cells: [`<span class="mono mut nowrap ${e.i === S.auditSel ? "m1-cur" : ""}" data-ai="${e.i}">${e.d}, ${e.t}</span>`, `<span class="row">${mav(e.who, 20)}<span class="trunc hide-sm">${MEM[e.who].name}</span></span>`, `<div><div class="b5">${e.a}</div><div class="mono faint xs">${e.code}</div></div>`, `<span class="mut trunc" style="display:block;max-width:180px">${esc(e.o)}</span>`, e.proj ? `<span class="row g4" style="max-width:150px">${AG.projDot(e.proj, 8)}<span class="trunc">${AG.project(e.proj).n}</span></span>` : `<span class="faint">Workspace</span>`],
          sort: [e.key, MEM[e.who].name, e.a, e.o, e.proj || "~"], text: [e.a, e.code, e.o, MEM[e.who].name].join(" "),
          filter: { cat: AUDIT_CAT[e.cat], who: MEM[e.who].name, proj: e.proj ? AG.project(e.proj).n : "Workspace", range: e.range } })),
        empty: AG.empty("list-details", "Tidak ada kejadian", "Tidak ada kejadian untuk filter ini. Coba perluas rentang waktu.") })}`, "wide",
      { drawer: cur && wideView ? `<aside class="drawer" aria-label="Detail kejadian">${auditDetail(cur)}</aside>` : "" });
  },
  after: (root) => {
    const t = root.querySelector('[data-dt="m1-audit"] .dt-t');
    t?.addEventListener("click", (e) => { const tr = e.target.closest("tbody tr"); const m = tr?.querySelector("[data-ai]"); if (m) AG.ACTIONS["m1-audit"](tr, m.dataset.ai); });
    t?.addEventListener("mouseover", (e) => { const tr = e.target.closest("tbody tr"); if (tr?.querySelector("[data-ai]")) tr.style.cursor = "pointer"; });
  } });
AG.action("m1-audit", (el, i) => {
  if (+i >= 0 && innerWidth < 1100) { AG.openDialog("m1-audit-detail", i); return; }
  S.auditSel = +i < 0 ? null : +i; AG.rerender();
});
AG.dialog("m1-audit-detail", (i) => ({ title: AUDIT[+i].a, icon: "list-details", body: auditDetail(AUDIT[+i], false) }));

/* ====================== Akun ====================== */
AG.screen({ m: "M1", g: "Akun", n: "Profil", route: "/settings/profile",
  render: () => set(["Akun", "Profil"], sub("profile", "me", [
    { k: "me", label: "Profil", icon: "user", desc: "Foto, nama, zona waktu" },
    { k: "linked", label: "Akun terhubung", icon: "plug-connected", desc: "Google, GitHub", count: 1 },
    { k: "security", label: "Keamanan", icon: "shield-check", desc: "Password dan 2FA" },
    { k: "sessions", label: "Sesi aktif", icon: "device-laptop", desc: "Perangkat yang masuk", count: 2 },
  ], [
    ["me", card({ title: "Profil", desc: "Terlihat oleh anggota lain dan agent.", pad: false, foot: AG.savebar("Simpan perubahan", { toast: "Profil disimpan" }), body: `<div class="box"><div class="m1-pad">${uploader({ key: "me", name: P.OO.name, color: P.OO.c, round: true, label: "foto", owner: "profil", hint: "JPG atau PNG, persegi, maksimal 1 MB." })}</div>${srow("Nama lengkap", "", inp(P.OO.name))}${srow("Nama panggilan", "Dipakai agent saat menyebut Anda.", inp("Olivia", { pre: "@" }))}${srow("Email", "Untuk mengganti email, hubungi Owner workspace.", `<label class="inp ro">${ic("lock", "s14 faint")}<span class="trunc">${P.OO.email}</span></label>`)}${srow("Jabatan", "Diatur Admin di halaman Anggota.", `<label class="inp ro">${ic("id-badge-2", "s14 faint")}<span class="trunc">${P.OO.job}</span></label>`)}${srow("Zona waktu", "", tzCombo("Asia/Jakarta (WIB)"))}</div>` })],
    ["linked", card({ title: "Akun terhubung", desc: "Masuk tanpa password lewat penyedia ini.", pad: false, body: `<div class="box">${srow(`<span class="row">${ic("brand-google")}Google</span>`, "Terhubung sebagai owner@agentgate.test", btn("Putuskan", "", "", { "data-toast": "Akun Google diputuskan" }))}${srow(`<span class="row">${ic("brand-github")}GitHub</span>`, "Belum terhubung.", btn("Hubungkan", "", "", { "data-toast": "Mengalihkan ke GitHub…" }))}</div>` })],
    ["security", card({ title: "Keamanan", desc: "Password dan autentikasi dua langkah.", pad: false, body: `<div class="box">${srow("Password", "Terakhir diganti 11 Nov 2026.", btn("Ganti password", "", "", { "data-go": "#/settings/password" }))}${srow("Autentikasi dua langkah", "Kode dari aplikasi authenticator diminta setiap kali masuk.", `${pill("Aktif", "ok")}${btn("Kelola", "", "", { "data-go": "#/settings/2fa/active" })}`)}</div>` })],
    ["sessions", `${card({ title: "Sesi aktif", desc: "Perangkat yang sedang masuk ke akun Anda.", pad: false, body: `<div class="box">${[["device-laptop", "Chrome di Linux", "Jakarta · 10.0.4.7 · sesi ini", ""], ["device-mobile", "Safari di iPhone", "Jakarta · aktif 2 hari lalu", "1"]].map(([i, n, d, x]) => `<div class="lrow">${tile(i)}<div class="grow"><div class="b5">${n}</div><div class="mut sm">${d}</div></div>${x ? btn("Keluarkan", "sm", "", { "data-toast": "Sesi Safari di iPhone dikeluarkan" }) : pill("Sesi ini", "ok")}</div>`).join("")}</div>` })}
      ${dzCard(dangerRow("Keluar dari semua perangkat lain", "Semua sesi selain sesi ini harus masuk ulang.", btn("Keluarkan semua", "dng", "logout", { "data-toast": "Semua sesi lain dikeluarkan" })))}`],
  ], { title: "Profil", desc: "Data diri, akun terhubung, keamanan, dan sesi aktif Anda." }), "mid") });

AG.screen({ m: "M1", g: "Akun", n: "Ganti password", route: "/settings/password",
  render: () => set(["Akun", "Password"], `
    ${head("Password", "Password untuk masuk dengan email. Akun Google dan GitHub diatur di Profil.")}
    ${card({ title: "Ganti password", desc: "Setelah diganti, sesi di perangkat lain otomatis keluar.", pad: false, foot: AG.savebar("Ganti password", { act: "m1-pw" }), body: `<div class="box">${srow("Password saat ini", `<a class="link" href="#/forgot">Lupa password?</a>`, pw("rahasia-toko-123"))}${srowT("Password baru", "Minimal 12 karakter.", `<div class="col g4" style="width:100%">${pw("toko123", { err: true, a: { "data-m1": "pwnew" } })}<span class="errt" data-m1="pwerr">Password baru minimal 12 karakter.</span></div>`)}${srow("Ulangi password baru", "", pw("", { a: { "data-m1": "pwrep" } }))}</div>` })}`),
  after: (root) => { const n = root.querySelector('[data-m1="pwnew"]'), er = root.querySelector('[data-m1="pwerr"]'); n.addEventListener("input", () => { const ok = n.value.length >= 12; n.parentElement.classList.toggle("err", !ok); er.style.display = ok ? "none" : ""; }); } });
AG.action("m1-pw", () => {
  const n = document.querySelector('[data-m1="pwnew"]').value, r = document.querySelector('[data-m1="pwrep"]').value;
  if (n.length < 12) return AG.toast("Password baru minimal 12 karakter", "dng");
  if (n !== r) return AG.toast("Ulangi password belum sama", "dng");
  AG.toast("Password diganti. Sesi lain sudah dikeluarkan.");
});
AG.action("m1-eye", (el) => { const i = el.parentElement.querySelector("input"); i.type = i.type === "password" ? "text" : "password"; el.innerHTML = ic(i.type === "password" ? "eye" : "eye-off", "s14"); el.setAttribute("aria-label", i.type === "password" ? "Tampilkan password" : "Sembunyikan password"); });

AG.screen({ m: "M1", g: "Akun", n: "Aktifkan 2FA", route: "/settings/2fa",
  render: () => set(["Akun", "Autentikasi dua langkah"], `
    ${head("Autentikasi dua langkah", "Lindungi akun dengan kode dari aplikasi authenticator.")}
    <div class="alert">${ic("info-circle")}<span>PT Contoh mewajibkan 2FA. Aktifkan dulu untuk membuka workspace.</span></div>
    ${card({ title: "Hubungkan aplikasi authenticator", desc: "Tiga langkah, kurang dari satu menit.", pad: false, foot: footBar(`${btn("Batal", "ghost", "", { "data-go": "#/settings/profile" })}${btn("Aktifkan 2FA", "pri", "shield-check", { "data-act": "m1-2fa-on" })}`), body: `<div>
      <div class="lrow" style="align-items:flex-start;gap:14px;padding:16px"><span class="step">1</span><div class="grow col g12"><div><div class="b5">Pindai kode QR</div><div class="mut">Pakai Google Authenticator, 1Password, atau aplikasi sejenis.</div></div>
        <div class="row wrap g16">${qr()}<div class="col"><span class="mut">Tidak bisa memindai? Masukkan kode ini:</span><button class="inp mono" style="gap:10px;width:auto" data-toast="Kode rahasia disalin">JBSW Y3DP EHPK 3PXP${ic("copy", "s14 faint")}</button></div></div></div></div>
      <div class="lrow" style="align-items:flex-start;gap:14px;padding:16px"><span class="step">2</span><div class="grow col g12"><div><div class="b5">Masukkan kode 6 digit</div><div class="mut">Kode berganti setiap 30 detik.</div></div>${otp()}</div></div>
      <div class="lrow" style="align-items:flex-start;gap:14px;padding:16px"><span class="step">3</span><div class="grow"><div class="b5">Simpan kode pemulihan</div><div class="mut">Ditampilkan setelah 2FA aktif, untuk masuk saat HP tidak di tangan.</div></div></div></div>` })}`),
  after: (root) => wireOtp(root) });
AG.action("m1-2fa-on", () => {
  const code = [...document.querySelectorAll(".m1-otp input")].map((x) => x.value).join("");
  if (code.length < 6) { AG.toast("Masukkan 6 digit kode dari aplikasi", "dng"); document.querySelectorAll(".m1-otp input")[code.length]?.focus(); return; }
  S.copied = false; AG.toast("2FA aktif"); go("#/settings/2fa/recovery");
});

AG.screen({ m: "M1", g: "Akun", n: "2FA aktif", route: "/settings/2fa/active",
  render: () => set(["Akun", "Autentikasi dua langkah"], `
    ${head("Autentikasi dua langkah", "Kode dari aplikasi authenticator diminta setiap kali masuk.", pill("Aktif sejak 11 Nov 2026", "ok", "", "shield-check"))}
    ${card({ title: "Metode", desc: "Perangkat dan kode cadangan untuk masuk.", pad: false, body: `<div class="box">${srow("Aplikasi authenticator", "Ditambahkan 11 Nov 2026 dari Chrome di Linux.", btn("Ganti perangkat", "", "", { "data-go": "#/settings/2fa" }))}${srow("Kode pemulihan", "8 dari 10 kode belum dipakai. Pakai saat HP tidak di tangan.", btn("Buat kode baru", "", "refresh", { "data-act": "m1-recodes" }))}${srow("Perangkat tepercaya", "1 perangkat tidak diminta kode selama 30 hari.", btn("Lupakan semua", "", "", { "data-toast": "Perangkat tepercaya dilupakan" }))}</div>` })}
    ${dzCard(`<div class="m1-pad" style="padding-bottom:0"><div class="alert warn">${ic("alert-triangle")}<span>PT Contoh mewajibkan 2FA. Bila dinonaktifkan, Anda harus mengaktifkannya lagi sebelum bisa membuka workspace.</span></div></div>
      <div class="srow m1-top" style="border-top:0"><div><div class="t">Nonaktifkan 2FA</div><div class="d">Masukkan password dan kode dari aplikasi untuk konfirmasi.</div></div><div class="ctl col g12" style="display:grid">${fld("Password", pw(""))}${fld("Kode", inp("", { ph: "123 456", cls: "mono", a: { inputmode: "numeric" } }))}${btn("Nonaktifkan", "dng", "", { "data-toast": "Kode salah. 2FA tetap aktif.", "data-tone": "dng" })}</div></div>`)}`) });
AG.action("m1-recodes", () => { S.copied = false; go("#/settings/2fa/recovery"); });

const CODES = ["k3x9q-2mfp7", "8vtr4-hn2ws", "p6dlz-9qj3c", "w2bmn-7xk4r", "c5hgt-3pv8e", "r9jfq-6ws2m", "a4nzk-8tc5y", "m7qpe-2hd9v", "t3wxa-5jr6b", "e8kcv-4mn7p"];
AG.screen({ m: "M1", g: "Akun", n: "Kode pemulihan", route: "/settings/2fa/recovery",
  render: () => set(["Akun", "Kode pemulihan"], `
    ${head("Simpan kode pemulihan", "Setiap kode hanya bisa dipakai sekali. Halaman ini tidak bisa dibuka lagi; kode lama sudah tidak berlaku.")}
    <div class="alert warn">${ic("alert-triangle")}<span>Simpan di pengelola password atau cetak. Tanpa kode ini dan tanpa HP, akun tidak bisa dipulihkan.</span></div>
    ${card({ title: "10 kode pemulihan", desc: "Dibuat 12 Nov 2026 untuk owner@agentgate.test.", action: `${btn(S.copied ? "Tersalin" : "Salin semua", "sm", S.copied ? "check" : "copy", { "data-act": "m1-copy" })}${btn("Unduh .txt", "sm", "download", { "data-act": "m1-copy", "data-arg": "dl" })}`, pad: false,
      body: `<div class="m1-codes">${CODES.map((c) => `<span>${c}</span>`).join("")}</div>`, foot: footBar(btn("Sudah saya simpan", S.copied ? "pri" : "", "check", { "data-go": "#/settings/2fa/active", "data-toast": "Kode pemulihan disimpan" })) })}`) });
AG.action("m1-copy", (el, a) => { try { navigator.clipboard?.writeText(CODES.join("\n")).catch(() => {}); } catch {} S.copied = true; AG.rerender(); AG.toast(a === "dl" ? "agentgate-kode-pemulihan.txt diunduh" : "10 kode disalin"); });

/* ====================== Pengaturan project ====================== */
const pset = (k, last, body, width = "narrow") => shell({ nav: "settings", project: k, crumbs: ["Pengaturan", AG.project(k).n, last], body, width });
const PDESC = { KSR: "Aplikasi kasir web untuk toko retail PT Contoh: transaksi penjualan, pembayaran **tunai dan QRIS**, cetak struk, serta validasi stok ke Aplikasi Gudang.\n\n- Target rilis 1.4 akhir November dengan pembayaran digital\n- Bahasa kode: TypeScript (kasir-web), Go (kasir-api)" };
AG.screen({ m: "M1", g: "Pengaturan project", n: "Project · Umum", route: "/settings/p/:k/general", path: "/settings/p/KSR/general",
  render: ({ k }) => {
    const p = AG.project(k), c = projColor(k), img = S.logo[k];
    const prev = (s, r) => `<span class="m1-mini" style="width:${s}px;height:${s}px;border-radius:${r}px;background:${img ? "transparent" : c};font-size:${Math.round(s * 0.38)}px" data-m1-fb="${k}">${img ? `<img src="${img}" alt="">` : initials(p.n)}</span>`;
    const look = card({ title: "Logo dan warna", desc: "Tampil di sidebar, board, dan email notifikasi.", pad: false, foot: AG.savebar("Simpan tampilan", { toast: "Tampilan project disimpan" }), body: `
      <div class="m1-blk"><div class="m1-sublab">Logo</div>${uploader({ key: k, name: p.n, color: c })}</div>
      <div class="m1-blk" data-m1="pcolor"><div><div class="m1-sublab">Warna</div><div class="mut sm">${img ? "Dipakai untuk penanda project di sidebar, label, dan latar logo cadangan." : "Dipakai sebagai latar logo inisial dan penanda project."}</div></div>${AG.colorPicker(c)}<span class="hint">Pilih dari palet atau masukkan kode hex sendiri.</span></div>
      <div class="m1-blk"><div class="m1-sublab">Pratinjau</div><div class="m1-sizes"><div><span class="m1-side">${prev(20, 5)}<span class="trunc">${p.n}</span></span>Sidebar</div><div>${prev(32, 8)}Kartu project</div><div>${prev(48, 12)}Halaman project</div><div><span class="row g4" style="color:var(--fg)"><span class="dot" style="background:${c}" data-m1-dot></span><span class="mono">${p.k}-24</span></span>Kode tiket</div></div></div>` });
    const wfStages = AG.STAGES.map((s) => `<span class="pill gray">${AG.stageIcon(s.k)}${s.n}</span>`).join(`<span class="arr">${ic("chevron-right", "s12")}</span>`);
    return pset(p.k, "Umum", sub("pgen", "detail", [
      { k: "detail", label: "Detail", icon: "file-text", desc: "Nama, kunci, lead, deskripsi" },
      { k: "look", label: "Logo dan warna", icon: "palette", desc: "Tampilan di sidebar dan board" },
      { k: "wf", label: "Workflow", icon: "route", desc: p.wf },
      { k: "danger", label: "Zona berbahaya", icon: "alert-triangle", desc: "Arsipkan atau hapus" },
    ], [
      ["detail", card({ title: "Detail project", desc: "Nama, kunci, lead, dan deskripsi untuk agent.", pad: false, foot: AG.savebar("Simpan perubahan", { toast: "Project disimpan" }), body: `<div class="box">${srow("Nama project", "Terlihat oleh semua orang yang punya akses.", inp(p.n))}${srow("Kunci", "Awalan nomor tiket. Tidak bisa diubah.", pill(p.k, "mono"))}${srow("Lead", "Penerima default tiket tanpa penanggung jawab.", AG.combo({ options: peopleOpts(projPeople(p.k)), value: "OO", ph: "Pilih lead", w: 320 }))}${srow("Status", "", S.archived[p.k] ? pill("Diarsipkan", "gray") : pill("Aktif", "ok"))}
        <div class="m1-form" style="border-top:1px solid var(--line)">${fld("Deskripsi", AG.editor({ html: AG.mdToHtml(PDESC[p.k] || p.d + "."), compact: true, h: "120px" }), "Agent membaca deskripsi ini sebagai konteks project. Markdown didukung.")}</div></div>` })],
      ["look", look],
      ["wf", card({ title: "Workflow", desc: "Stage, gate, dan executor project ini.", action: AG.seeAll(`#/settings/p/${p.k}/workflow`, "Kelola"), pad: false, body: `<div class="box"><div class="lrow" style="min-height:60px">${tile("route", "acc")}<div class="grow"><div class="b5">${p.wf}</div><div class="mut sm">${AG.STAGES.length} stage · ${AG.STAGES.filter((s) => s.gate).length} gate · di-clone dari template</div></div></div><div class="m1-pad m1-wf" style="border-top:1px solid var(--line)">${wfStages}</div></div>` })],
      ["danger", dzCard(`${S.archived[p.k] ? dangerRow("Pulihkan project", "Project bisa diubah lagi dan job agent bisa berjalan.", btn("Pulihkan", "", "arrow-back-up", { "data-act": "m1-archive", "data-arg": p.k })) : dangerRow("Arsipkan project", "Project tetap bisa dibuka, tetapi ditandai diarsipkan dan tidak bisa diubah. Job agent yang berjalan dihentikan.", btn("Arsipkan", "dng", "archive", { "data-act": "m1-archive", "data-arg": p.k }))}${dangerRow("Hapus project", "Semua tiket, repo, dan knowledge project ini dihapus permanen.", btn("Hapus project", "dng", "trash", { "data-dialog": "m1-del-project", "data-arg": p.k }))}`)],
    ], { title: "Umum", desc: `Detail, tampilan, dan workflow project ${p.n}.`, head: btn("Buka project", "", "external-link", { "data-go": `#/p/${p.k}` }) }), "mid");
  },
  after: (root, { k }) => {
    const cp = root.querySelector('[data-m1="pcolor"]');
    cp?.addEventListener("color-change", (e) => { S.color[k] = e.detail; root.querySelectorAll(`[data-m1-fb="${k}"]`).forEach((x) => { if (!x.querySelector("img")) x.style.background = e.detail; }); root.querySelectorAll("[data-m1-dot]").forEach((x) => (x.style.background = e.detail)); });
    root.querySelector('[data-panel="look"] .savebar .btn.pri')?.addEventListener("click", () => { AG.project(k).c = projColor(k); delete S.color[k]; setTimeout(AG.rerender, 0); });
    root.querySelector('[data-panel="look"] .savebar .btn.ghost')?.addEventListener("click", () => { delete S.color[k]; setTimeout(AG.rerender, 0); });
  } });
AG.action("m1-archive", (el, k) => { S.archived[k] = !S.archived[k]; AG.rerender(); AG.toast(S.archived[k] ? `${AG.project(k).n} diarsipkan` : `${AG.project(k).n} dipulihkan`); });
AG.dialog("m1-del-project", (k) => { const p = AG.project(k); return { title: `Hapus ${p.n}?`, icon: "trash", tone: "dng", desc: "Tindakan ini tidak bisa dibatalkan.",
  body: `<div class="alert dng">${ic("alert-triangle")}<span>${ticketsOf(k).length} tiket, ${(S.repos[k] || []).length} repo, dan seluruh knowledge project ikut terhapus. Job agent yang berjalan dihentikan.</span></div>${fld(`Ketik <code>${p.k}</code> untuk konfirmasi`, inp("", { ph: p.k, cls: "mono", a: { "data-m1": "pdel", "aria-label": "Konfirmasi kunci project" } }))}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Hapus project", "dngs dis", "", { "data-m1": "pdelbtn", "data-toast": "Prototype: project tidak benar-benar dihapus", "data-close": true })}`,
  after: (el) => { const i = el.querySelector('[data-m1="pdel"]'), b = el.querySelector('[data-m1="pdelbtn"]'); i.addEventListener("input", () => b.classList.toggle("dis", i.value.trim() !== p.k)); } }; });

const PROLES = ["Admin", "Member", "Viewer", "Release Manager"];
AG.screen({ m: "M1", g: "Pengaturan project", n: "Project · Akses", route: "/settings/p/:k/access", path: "/settings/p/KSR/access",
  render: ({ k }) => {
    const p = AG.project(k);
    const has = (id) => S.access.some((a) => a.id === id);
    const opts = [...TEAMS.filter((t) => !has(t.id)).map((t) => ({ v: "t:" + t.id, l: "Tim " + t.n, sub: `${t.m.length} anggota`, lead: teamLead, group: "Tim" })), ...peopleOpts(memKeys().filter((x) => !has(x))).map((o) => ({ ...o, v: "u:" + o.v, group: "Orang" }))];
    const roleOpts = PROLES.map((r) => ({ v: r, l: r, sub: { Admin: "Kelola akses, repo, workflow", Member: "Buat tiket, setujui gate", Viewer: "Hanya melihat", "Release Manager": "Setujui dan paksa transisi" }[r] }));
    const rows = S.access.map((a, i) => {
      const lead = a.t === "team" ? tile("users-group") : mav(a.id, 28);
      const n = a.t === "team" ? "Tim " + team(a.id).n : MEM[a.id].name;
      const d = a.t === "team" ? `${team(a.id).m.length} anggota` : a.lock ? `${a.lock} · otomatis Admin` : MEM[a.id].email;
      const ctl = a.lock ? `<span class="row g4 mut sm nowrap m1-rc" style="justify-content:flex-end">${ic("lock", "s14")}Admin</span>` : `<span class="m1-rc" data-m1-role="${i}">${AG.combo({ options: roleOpts, value: a.r, w: 260 })}</span>`;
      return `<div class="lrow">${lead}<div class="grow" style="min-width:0"><div class="b5 trunc">${n}</div><div class="mut sm trunc">${d}</div></div>${ctl}${a.lock ? `<span style="width:32px;flex-shrink:0"></span>` : btn("", "ghost icon", "x", { "data-act": "m1-acc-rm", "data-arg": i, "aria-label": "Cabut akses " + n })}</div>`;
    }).join("");
    const MAT = [["Lihat tiket dan board", [1, 1, 1, 1]], ["Buat dan ubah tiket", [1, 1, 0, 0]], ["Setujui gate dan review", [1, 1, 0, 1]], ["Paksa transisi stage", [1, 0, 0, 1]], ["Kelola akses, repo, dan workflow", [1, 0, 0, 0]]];
    return pset(p.k, "Akses", `
      ${head("Akses", `Siapa saja yang bisa membuka ${p.n} dan apa yang boleh mereka lakukan di project ini.`)}
      <div class="alert gray">${ic("info-circle")}<span>Owner dan Admin workspace otomatis menjadi Admin di semua project. Role anggota lain bisa berbeda per project.</span></div>
      ${card({ title: "Beri akses", desc: "Pilih tim atau orang, lalu tentukan role-nya di project ini.", body: `<div class="lrow" style="gap:8px;flex-wrap:wrap;padding:0;border:0"><span class="grow" style="min-width:220px" data-m1="accwho">${AG.combo({ options: opts, ph: "Cari tim atau orang…", w: 360, multi: true })}</span><span class="m1-rc" data-m1="accrole">${AG.combo({ options: roleOpts, value: "Member", w: 260 })}</span>${btn("Beri akses", "pri", "", { "data-act": "m1-acc-add" })}</div>` })}
      ${card({ title: `Punya akses<span class="m1-cnt">${S.access.length}</span>`, desc: `Total ${projPeople(p.k).length} orang lewat tim dan undangan langsung.`, pad: false, body: `<div class="box">${rows}</div>` })}
      ${card({ title: "Role di project ini", desc: "Ringkasan izin tiap role. Ubah izinnya di pengaturan Role workspace.", action: AG.seeAll("#/settings/roles", "Kelola"), pad: false, body: `<div class="box tblwrap"><table class="tbl m1-perm"><thead><tr><th>Izin</th>${PROLES.map((r) => `<th>${r}</th>`).join("")}</tr></thead><tbody>${MAT.map(([n, v]) => `<tr><td>${n}</td>${v.map((x) => `<td>${x ? `<span class="text-ok">${ic("check", "s14")}</span>` : `<span class="faint">—</span>`}</td>`).join("")}</tr>`).join("")}</tbody></table></div>` })}`);
  },
  after: (root) => root.querySelectorAll("[data-m1-role]").forEach((s) => s.addEventListener("combo-change", (e) => { S.access[+s.dataset.m1Role].r = e.detail; AG.toast(`Role diubah ke ${e.detail}`); })) });
AG.action("m1-acc-rm", (el, i) => { const a = S.access.splice(+i, 1)[0]; AG.rerender(); AG.toast(`Akses ${a.t === "team" ? "tim " + team(a.id).n : MEM[a.id].name} dicabut`); });
AG.action("m1-acc-add", () => {
  const v = comboOf('[data-m1="accwho"]') || [], r = comboOf('[data-m1="accrole"]') || "Member";
  if (!v.length) return AG.toast("Pilih tim atau orang dulu", "dng");
  v.forEach((x) => { const [t, id] = x.split(":"); S.access.push({ t: t === "t" ? "team" : "user", id, r }); });
  AG.rerender(); AG.toast(`Akses ${r} diberikan ke ${v.length === 1 ? (v[0][0] === "t" ? "tim " + team(v[0].slice(2)).n : MEM[v[0].slice(2)].name) : v.length + " penerima"}`);
});

AG.screen({ m: "M1", g: "Pengaturan project", n: "Project · Repo", route: "/settings/p/:k/repos", path: "/settings/p/KSR/repos",
  render: ({ k }) => {
    const p = AG.project(k), repos = S.repos[p.k] || (S.repos[p.k] = []);
    const form = S.repoForm ? card({ title: "Repo baru", desc: "Hubungkan repositori dari GitHub, GitLab, atau Bitbucket.", cls: "m1-acc", action: btn("", "ghost icon sm", "x", { "data-act": "m1-repo-form", "aria-label": "Tutup form" }), pad: false,
      foot: footBar(`${btn("Batal", "ghost", "", { "data-act": "m1-repo-form" })}${btn("Tambah repo", "pri", "check", { "data-act": "m1-repo-add", "data-arg": p.k })}`), body: `<div class="m1-form">
        ${fld("Provider", `<div class="m1-prov" role="radiogroup" aria-label="Provider">${Object.entries(PROV).map(([id, [n, i]]) => `<button class="m1-pathcard ${S.prov === id ? "on" : ""}" data-radio="m1-prov" data-p="${id}" role="radio"><span class="radio ${S.prov === id ? "on" : ""}"></span>${ic(i, "s18")}<span class="b5">${n}</span></button>`).join("")}</div>`)}
        <label class="row" style="align-items:flex-start">${cb(false, "Versi perusahaan")}<span class="mut">Server sendiri (GitHub Enterprise Server, GitLab Self-Managed, Bitbucket Data Center)</span></label>
        <div class="m1-g2">${fld("URL repo", inp("", { ph: "https://" + PROV[S.prov][2] + "kasir-mobile", a: { "data-m1": "rurl" } }))}${fld("Branch utama", inp("main", { cls: "mono" }))}</div>
        ${fld("Nama", inp("", { ph: "kasir-mobile", cls: "mono", a: { "data-m1": "rname" } }), "Diisi otomatis dari URL.")}
        <div class="fld"><label>Jalur kode ke server</label><div class="grid2">
          <button class="m1-pathcard on" data-radio="m1-path"><span class="radio on"></span><span class="col g4"><b class="b5">Clone dari provider</b><span class="mut sm">Server meng-clone langsung. Untuk provider cloud yang terjangkau server.</span></span></button>
          <button class="m1-pathcard" data-radio="m1-path"><span class="radio"></span><span class="col g4"><b class="b5">Upload lewat CLI</b><span class="mut sm">CLI meng-upload dari laptop. Untuk semua provider, termasuk di balik firewall.</span></span></button></div>
          <span class="hint">Upload CLI tidak pernah mengirim file yang di-ignore <code>.gitignore</code> maupun file rahasia (<code>.env</code>, private key). Secret dipindai sebelum upload.</span></div></div>` }) : "";
    const list = repos.length ? `<div>${repos.map((r, i) => `<div class="lrow">${ic(PROV[r.prov][1], "s18 mut")}<div class="grow" style="min-width:0"><div class="row"><span class="mono b5">${r.n}</span>${r.ent ? pill("Self-Managed", "gray") : ""}</div><div class="mut sm trunc">${r.u}</div><div class="faint xs row g4" style="margin-top:2px">${ic(r.path === "clone" ? "cloud" : "upload", "s12")}${r.path === "clone" ? "Clone dari provider" : "Upload lewat CLI"} · ${r.sync}</div></div><span class="hide-sm">${pill(r.b, "mono gray", "", "git-branch")}</span>${btn("", "ghost icon", "dots", { "data-menu": "m1-repo-actions", "data-arg": p.k + ":" + i, "aria-label": "Aksi repo " + r.n })}</div>`).join("")}</div>`
      : empty("git-branch", "Belum ada repo", "Tambahkan repo agar agent bisa membaca dan mengubah kode.", btn("Tambah repo", "pri", "plus", { "data-act": "m1-repo-form" }));
    return pset(p.k, "Repo", `
      ${head("Repo", `Repositori yang boleh dibaca dan diubah agent untuk ${p.n}. Satu project bisa punya beberapa repo.`, S.repoForm ? "" : btn("Tambah repo", "pri", "plus", { "data-act": "m1-repo-form" }))}
      ${form}
      ${card({ title: `Repo terhubung<span class="m1-cnt">${repos.length}</span>`, desc: `Token akses provider diatur di <a class="link" href="#/settings/git">Git provider</a>. Repo di balik firewall dicek lewat runner yang online.`, pad: !repos.length, body: list })}`);
  },
  after: (root) => {
    const url = root.querySelector('[data-m1="rurl"]'); if (!url) return;
    const name = root.querySelector('[data-m1="rname"]');
    root.querySelectorAll("[data-p]").forEach((b) => b.addEventListener("click", () => { S.prov = b.dataset.p; url.placeholder = "https://" + PROV[b.dataset.p][2] + "kasir-mobile"; }));
    url.addEventListener("input", () => { const m = url.value.replace(/\.git$/, "").split("/").filter(Boolean).pop(); if (m && !name.dataset.touched) name.value = m; });
    name.addEventListener("input", () => (name.dataset.touched = "1"));
    url.focus();
  } });
AG.action("m1-repo-form", () => { S.repoForm = !S.repoForm; AG.rerender(); });
AG.action("m1-repo-add", (el, k) => {
  const u = document.querySelector('[data-m1="rurl"]').value.trim(), n = document.querySelector('[data-m1="rname"]').value.trim();
  if (!u || !n) { AG.toast("Isi URL repo dulu", "dng"); document.querySelector('[data-m1="rurl"]').focus(); return; }
  const up = document.querySelectorAll('[data-radio="m1-path"]')[1].classList.contains("on");
  S.repos[k].push({ n, prov: S.prov, u: u.replace(/^https?:\/\//, ""), b: "main", path: up ? "upload" : "clone", sync: up ? "menunggu upload pertama dari CLI" : "clone pertama berjalan" });
  S.repoForm = false; AG.rerender(); AG.toast(up ? `Repo ${esc(n)} ditambahkan. Jalankan ag repo push di laptop.` : `Repo ${esc(n)} ditambahkan, clone dimulai`);
});
AG.menu("m1-repo-actions", (a) => { const [k, i] = a.split(":"); const r = S.repos[k][+i]; return `<div class="mlab">${r.n}</div>
  <button class="mi" data-toast="${r.path === "clone" ? "Sinkronisasi " + r.n + " dimulai" : "Jalankan ag repo push " + r.n + " di laptop"}">${ic("refresh")}${r.path === "clone" ? "Sinkronkan sekarang" : "Cara upload ulang"}</button>
  <button class="mi" data-act="m1-repo-path" data-arg="${a}">${ic("arrows-exchange")}Ganti ke ${r.path === "clone" ? "upload lewat CLI" : "clone dari provider"}</button>
  <a class="mi" href="#/p/${k}/graph">${ic("topology-star-3")}Lihat code graph</a>
  <div class="msep"></div><button class="mi dng" data-act="m1-repo-rm" data-arg="${a}">${ic("trash")}Lepas repo</button>`; });
AG.action("m1-repo-path", (el, a) => { const [k, i] = a.split(":"); const r = S.repos[k][+i]; r.path = r.path === "clone" ? "upload" : "clone"; r.sync = "jalur baru menunggu sinkron pertama"; AG.rerender(); AG.toast(`Jalur kode ${r.n} diganti`); });
AG.action("m1-repo-rm", (el, a) => { const [k, i] = a.split(":"); const r = S.repos[k].splice(+i, 1)[0]; AG.rerender(); AG.toast(`Repo ${r.n} dilepas dari project`); });

/* ====================== 404 ====================== */
AG.screen({ m: "M1", g: "Lainnya", n: "Halaman tidak ditemukan", route: "/404",
  render: () => solo(`<div class="col g12" style="justify-items:center;text-align:center">${logo()}<span class="mono mut" style="margin-top:20px">404</span><h1 style="font-size:22px">Halaman tidak ditemukan</h1><p class="mut" style="max-width:40ch">Tautannya mungkin sudah tidak berlaku, atau Anda tidak punya akses ke halaman ini di workspace PT Contoh.</p><div class="row wrap" style="justify-content:center">${btn("Kembali ke beranda", "pri", "", { "data-go": "#/home" })}${btn("Ganti workspace", "", "", { "data-go": "#/workspaces" })}</div></div>`, true) });

/* ====================== Dialog ====================== */
const WF = [["Superpowers-style", "Brainstorm → PRD → desain → dev → review → test"], ["Scrum", "Sprint, backlog, review"], ["Kanban", "Alur kontinu dengan batas WIP"], ["BMAD", "Analyst, PM, Architect, Dev, QA"], ["Spec-driven", "Spesifikasi dulu, lalu implementasi"], ["Shape Up", "Siklus 6 minggu dan cooldown"], ["Bug Triage", "Laporan, reproduksi, perbaikan"], ["Waterfall", "Tahap berurutan dengan gate"], ["Hotfix/Incident", "Jalur cepat untuk insiden produksi"]];
AG.dialog("new-project", () => ({
  title: "Project baru", icon: "folder", tone: "acc", w: "w560",
  body: `${fld("Nama", inp("", { ph: "Misalnya Aplikasi Gudang", a: { "data-m1": "pname" } }))}
    <div class="m1-g2">${fld("Workflow", AG.combo({ options: WF.map(([v, d]) => ({ v, l: v, sub: d })), value: "Superpowers-style", w: 320 }), "Template di-clone dan bisa diubah.")}${fld("Kunci", inp("", { ph: "GDG", cls: "mono", a: { "data-m1": "pkey", maxlength: "6" } }))}</div>
    <p class="hint" data-m1="phint">Kunci menjadi awalan nomor tiket, misalnya GDG-12. Tidak bisa diubah setelah dibuat.</p>
    ${fld(`Deskripsi <span class="opt">(opsional)</span>`, AG.editor({ compact: true, h: "80px", ph: "Apa yang dibangun di project ini?" }))}
    ${fld("Akses awal", AG.combo({ options: [...TEAMS.filter((t) => t.m.length).map((t) => ({ v: t.id, l: `Tim ${t.n} sebagai Member`, sub: `${t.m.length} anggota`, lead: teamLead, group: "Tim" })), { v: "me", l: "Hanya saya", sub: "Tambahkan orang lain nanti", lead: av("OO", 20), group: "Lainnya" }], value: "engineering", multi: true }))}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Buat project", "pri", "", { "data-act": "m1-proj-create" })}`,
  after: (el) => {
    const n = el.querySelector('[data-m1="pname"]'), k = el.querySelector('[data-m1="pkey"]'), h = el.querySelector('[data-m1="phint"]');
    const upd = () => (h.textContent = `Kunci menjadi awalan nomor tiket, misalnya ${k.value || "GDG"}-12. Tidak bisa diubah setelah dibuat.`);
    n.addEventListener("input", () => { if (!k.dataset.touched) { const w = n.value.trim().split(/\s+/).filter(Boolean); k.value = (w.length > 1 ? w.map((x) => x[0]).join("") : (w[0] || "").replace(/[aiueo]/gi, "").slice(0, 3) || (w[0] || "")).toUpperCase().slice(0, 4); upd(); } });
    k.addEventListener("input", () => { k.dataset.touched = "1"; k.value = k.value.toUpperCase().replace(/[^A-Z0-9]/g, ""); upd(); });
    n.focus();
  },
}));
AG.action("m1-proj-create", () => {
  const n = document.querySelector('[data-m1="pname"]').value.trim(), k = document.querySelector('[data-m1="pkey"]').value.trim();
  if (!n) { AG.toast("Isi nama project dulu", "dng"); document.querySelector('[data-m1="pname"]').focus(); return; }
  if (AG.PROJECTS.some((p) => p.k === k)) { AG.toast(`Kunci ${k} sudah dipakai project lain`, "dng"); return; }
  AG.closeDialog(); AG.toast(`Project ${esc(n)} (${esc(k)}) dibuat`);
});

AG.dialog("leave-workspace", () => {
  const owners = memKeys().filter((k) => MEM[k].role === "Owner");
  const last = MEM[AG.ME].role === "Owner" && owners.length <= 1;
  return { title: "Keluar dari PT Contoh?", icon: "door-exit", tone: "dng",
    desc: "Akses Anda ke semua project di workspace ini berakhir. Untuk bergabung lagi, Anda butuh undangan baru.",
    body: last ? `<div class="alert warn">${ic("alert-triangle")}<span>Anda Owner terakhir. Jadikan anggota lain Owner dulu lewat halaman <a class="link" href="#/settings/members" data-close>Anggota</a> sebelum keluar.</span></div>` : `<p class="mut">Tiket yang ditugaskan ke Anda tetap ada tanpa penerima.</p>`,
    foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Keluar dari workspace", "dngs" + (last ? " dis" : ""), "", last ? { disabled: true } : { "data-go": "#/workspaces", "data-toast": "Anda keluar dari PT Contoh" })}` };
});

AG.dialog("delete-workspace", () => ({
  title: "Hapus PT Contoh?", icon: "trash", tone: "dng",
  desc: `Workspace dijadwalkan hapus dan hilang permanen pada <b class="fg b5">12 Desember 2026</b>. Sampai tanggal itu hanya Owner yang bisa membukanya dan membatalkan penghapusan.`,
  body: `<div class="alert dng">${ic("alert-triangle")}<span>3 project, 5 repo, 16 tiket, ${memKeys().length} anggota, dan seluruh audit log ikut terhapus. Runner anggota berhenti menerima job.</span></div>
    ${fld(`Ketik <code>pt-contoh</code> untuk konfirmasi`, inp("", { ph: "pt-contoh", cls: "mono", a: { "data-m1": "delconf", "aria-label": "Konfirmasi alamat workspace" } }))}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Jadwalkan penghapusan", "dngs dis", "", { "data-act": "m1-ws-del", "data-m1": "delbtn" })}`,
  after: (el) => { const i = el.querySelector('[data-m1="delconf"]'), b = el.querySelector('[data-m1="delbtn"]'); i.addEventListener("input", () => b.classList.toggle("dis", i.value.trim() !== "pt-contoh")); },
}));

AG.dialog("invite-member", () => ({
  title: "Undang anggota", icon: "mail", tone: "acc", w: "w560", desc: "Undangan berlaku 7 hari. Undangan lama untuk email yang sama dibatalkan.",
  body: `${fld("Email", `<label class="inp m1-chips" data-m1="chips">${["budi.h@tokomaju.id"].map(chip).join("")}<input placeholder="Tambah email…" aria-label="Email" data-m1="chipin"></label>`, "Tekan Enter atau koma setelah tiap email. Bisa beberapa sekaligus.")}
    <div class="grid2">${fld("Role", `<span data-m1="invrole">${AG.combo({ options: PROLES.map((r) => ({ v: r, l: r })), value: "Member" })}</span>`)}${fld("Jabatan", `<span data-m1="invjob">${AG.combo({ options: [{ v: "", l: "Tanpa jabatan" }, ...JOBS.map((j) => ({ v: j, l: j }))], value: "Developer" })}</span>`)}</div>
    ${fld(`Tim <span class="opt">(opsional)</span>`, `<span data-m1="invteam">${AG.combo({ options: [{ v: "", l: "Tidak masuk tim" }, ...TEAMS.map((t) => ({ v: t.id, l: t.n, sub: `${t.m.length} anggota · ${t.p.length ? t.p.map(([k]) => AG.project(k).n).join(", ") : "belum punya akses project"}`, lead: teamLead }))], value: "engineering" })}</span>`, "Anggota langsung mendapat akses project milik tim.")}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Kirim 1 undangan", "pri", "send", { "data-act": "m1-invite", "data-m1": "invbtn" })}`,
  after: (el) => {
    const box = el.querySelector('[data-m1="chips"]'), input = el.querySelector('[data-m1="chipin"]'), b = el.querySelector('[data-m1="invbtn"]');
    const count = () => { const n = box.querySelectorAll(".m1-chip").length; b.lastChild.textContent = n ? `Kirim ${n} undangan` : "Kirim undangan"; b.classList.toggle("dis", !n); };
    const add = () => { const vals = input.value.split(/[,\s]+/).filter(Boolean); vals.forEach((v) => input.insertAdjacentHTML("beforebegin", chip(v))); input.value = ""; count(); };
    input.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === ",") { e.preventDefault(); add(); } if (e.key === "Backspace" && !input.value) { box.querySelector(".m1-chip:last-of-type")?.remove(); count(); } });
    input.addEventListener("blur", add);
    box.addEventListener("click", (e) => { const x = e.target.closest(".m1-chip button"); if (x) { x.parentElement.remove(); count(); } });
    input.focus(); count();
  },
}));
function chip(e) { const ok = /^[^@\s]+@[^@\s]+\.[a-z]{2,}$/i.test(e); return `<span class="pill m1-chip ${ok ? "" : "dng"}" data-e="${esc(e)}" title="${ok ? "" : "Format email belum benar"}">${esc(e)}<button type="button" aria-label="Hapus ${esc(e)}">${ic("x")}</button></span>`; }
AG.action("m1-invite", () => {
  const chips = [...document.querySelectorAll(".m1-chip")];
  if (!chips.length) return AG.toast("Tambahkan minimal satu email", "dng");
  if (chips.some((c) => c.classList.contains("dng"))) return AG.toast("Ada email dengan format salah", "dng");
  const r = comboOf('[data-m1="invrole"]') || "Member", j = comboOf('[data-m1="invjob"]') || "";
  chips.forEach((c) => S.invites.push({ e: c.dataset.e, r, j, w: "19 Nov", by: AG.ME }));
  AG.closeDialog(); AG.toast(`${chips.length} undangan dikirim`);
  if (AG.route.path === "/settings/members") AG.rerender();
});

AG.dialog("remove-member", (k) => {
  const p = MEM[k] || MEM.DD;
  const t = teamOf(k);
  return { title: `Keluarkan ${p.name}?`, icon: "logout", tone: "dng",
    body: `<p class="mut">${p.name} (${p.email}) kehilangan akses ke PT Contoh dan semua project yang dia ikuti.${t ? ` Keanggotaan di tim ${t.n} juga dicabut.` : ""} Tiket yang ditugaskan ke ${p.name.split(" ")[0]} tetap ada tanpa penerima.</p>
      ${AG.RUNNERS.some((r) => r.who === k) ? `<div class="alert warn">${ic("device-laptop")}<span>Runner ${AG.RUNNERS.find((r) => r.who === k).n} ikut diputus dan job yang berjalan dihentikan.</span></div>` : ""}`,
    foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Keluarkan", "dngs", "", { "data-act": "m1-remove", "data-arg": k })}` };
});
AG.action("m1-remove", (el, k) => { S.removed.push(k); AG.closeDialog(); AG.rerender(); AG.toast(`${MEM[k].name} dikeluarkan dari workspace`); });

AG.dialog("new-team", () => ({
  title: "Tim baru", icon: "users-group", tone: "acc", w: "w560",
  body: `${fld("Nama tim", inp("", { ph: "Misalnya Tim Backend", a: { "data-m1": "tname" } }))}
    ${fld(`Deskripsi <span class="opt">(opsional)</span>`, inp("", { ph: "Apa yang dikerjakan tim ini?" }))}
    ${fld(`Anggota <span class="opt">(opsional)</span>`, `<span data-m1="tmem">${AG.combo({ options: peopleOpts(memKeys()).map((o) => ({ ...o, sub: [MEM[o.v].job, teamOf(o.v) ? "Tim " + teamOf(o.v).n : "Tanpa tim"].filter(Boolean).join(" · ") })), multi: true, ph: "Cari dan pilih anggota…", w: 360 })}</span>`, "Anggota yang sudah di tim lain akan dipindah ke tim baru.")}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Buat tim", "pri", "", { "data-act": "m1-team-new" })}`,
  after: (el) => el.querySelector('[data-m1="tname"]').focus(),
}));
AG.action("m1-team-new", () => {
  const n = document.querySelector('[data-m1="tname"]').value.trim();
  if (!n) { AG.toast("Isi nama tim dulu", "dng"); document.querySelector('[data-m1="tname"]').focus(); return; }
  const ks = comboOf('[data-m1="tmem"]') || [];
  const id = n.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "tim-" + TEAMS.length;
  TEAMS.forEach((t) => (t.m = t.m.filter((x) => !ks.includes(x))));
  TEAMS.push({ id, n, d: "", lead: ks[0] || null, m: ks, p: [] });
  AG.closeDialog(); AG.toast(`Tim ${esc(n)} dibuat`); go(`#/settings/teams/${id}`);
});
})();
