/* Data contoh AI Office (bentuk mengikuti GET /w/{slug}/office/state, disederhanakan).
   Dipakai bersama oleh ketiga variasi supaya perbandingannya adil. */
window.OFFICE_DATA = {
  workspace: "PT Contoh",
  rooms: [
    { k: "KSR", kind: "project", n: "Kasir Online", c: "#5b5bd6" },
    { k: "LMS", kind: "project", n: "Kampus Digital", c: "#d6409f" },
    { k: "MEET", kind: "meeting", n: "Ruang rapat", c: "#3b82f6" },
    { k: "HRM", kind: "project", n: "Portal HR", c: "#12a594" },
    { k: "ABS", kind: "project", n: "Absensi Mobile", c: "#8e4ec6" },
    { k: "PANTRY", kind: "pantry", n: "Pantry", c: "#a18072" },
  ],
  /* st: working | review | decision | meeting | idle | offline | online (manusia) */
  chars: [
    { id: "c1", bot: true, name: "Dev", icon: "code", color: "#5b5bd6", room: "KSR", st: "working", tk: "KSR-12", title: "Diskon member di kasir", doing: "Menulis tes unit untuk aturan diskon", step: 3, steps: 5, runner: "laptop-ifa" },
    { id: "c2", bot: true, name: "QA", icon: "bug", color: "#12a594", room: "KSR", st: "review", tk: "KSR-9", title: "Cetak struk thermal", doing: "Hasil uji siap: 14 lulus, 1 dilewati", step: 5, steps: 5, runner: "laptop-ifa" },
    { id: "c3", bot: false, name: "Ifa", full: "Muhammad Ifa", color: "#2f6f8f", skin: "#e8b48f", hair: "#2a1d16", style: "short", acc: "glasses", room: "KSR", st: "online", jobs: ["KSR-12", "KSR-9"], runner: "laptop-ifa" },
    { id: "c4", bot: true, name: "PM", icon: "list-check", color: "#d6409f", room: "HRM", st: "decision", tk: "HRM-4", title: "Aturan cuti setengah hari", doing: "Perlu keputusan: cuti setengah hari memotong kuota?", step: 1, steps: 4, runner: "pc-kantor" },
    { id: "c5", bot: true, name: "Dev", icon: "code", color: "#5b5bd6", room: "HRM", st: "offline", tk: "HRM-7", title: "Rekap absensi bulanan", doing: "Runner pc-kantor offline 12 menit", step: 2, steps: 5, runner: "pc-kantor" },
    { id: "c6", bot: true, name: "Arsitek", icon: "sitemap", color: "#0d74ce", room: "LMS", st: "working", tk: "LMS-3", title: "Skema nilai semester", doing: "Menyusun ADR tabel nilai", step: 2, steps: 4, runner: "laptop-ana" },
    { id: "c7", bot: false, name: "Ana", full: "Ana Pratiwi", color: "#c2410c", skin: "#c68a63", hair: "#1d1410", style: "hijab", hijab: "#5f8f7a", room: "LMS", st: "online", jobs: ["LMS-3"], runner: "laptop-ana" },
    { id: "c8", bot: true, name: "Desainer", icon: "palette", color: "#e54666", room: "MEET", st: "meeting", tk: "ABS-2", title: "Alur absen wajah", doing: "Brainstorming alur absen wajah", step: 1, steps: 3, runner: "laptop-budi" },
    { id: "c9", bot: false, name: "Budi", full: "Budi Santoso", color: "#3e63dd", skin: "#d9a07a", hair: "#3b2a1e", style: "short", room: "MEET", st: "meeting", tk: "ABS-2", jobs: ["ABS-2"], runner: "laptop-budi" },
    { id: "c10", bot: true, name: "QA", icon: "bug", color: "#12a594", room: "PANTRY", st: "idle" },
    { id: "c11", bot: true, name: "Penulis", icon: "writing", color: "#a18072", room: "PANTRY", st: "idle" },
  ],
};

window.ST = {
  working: { label: "Bekerja", icon: "terminal-2" },
  review: { label: "Menunggu review", icon: "eye", act: true },
  decision: { label: "Butuh keputusan", icon: "alert-triangle", act: true },
  meeting: { label: "Brainstorming", icon: "message-2" },
  idle: { label: "Menganggur", icon: "coffee" },
  offline: { label: "Runner offline", icon: "plug-x" },
  online: { label: "Online", icon: "user" },
};
