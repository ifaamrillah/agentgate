/* Modul M7 Code graph · M8 Knowledge base · M10 Visual.
   Bagian A: data code graph, M7 status index, M10 graph viewer. */
(() => {
const { ic, av, btn, pill, sw, cb, section, head, tref, esc } = AG;
const K = "KSR";

AG.css(`
.gi-lang { display: flex; height: 6px; border-radius: 3px; overflow: hidden; width: 120px; background: var(--hover); }
.gi-lang i { display: block; height: 100%; }
.gi-legend { display: flex; gap: 10px; flex-wrap: wrap; font-size: 12px; color: var(--muted); }
.gi-legend span { display: inline-flex; align-items: center; gap: 5px; }
.gi-legend i { width: 8px; height: 8px; border-radius: 2px; display: inline-block; }
.gi-ep { display: grid; gap: 2px; min-width: 0; max-width: 340px; }
.gi-ep b { font-weight: 500; font-family: var(--mono); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gi-ep span { color: var(--muted); font-size: 12px; }
/* Kartu dengan judul di dalam: judul sejajar di atas, kartu sama tinggi. */
.gi-card { display: flex; flex-direction: column; min-width: 0; }
.gi-ch { display: grid; gap: 2px; padding: 14px 16px 12px; border-bottom: 1px solid var(--line); }
.gi-ch b { font-weight: 600; font-size: 14px; }
.gi-ch span { color: var(--muted); font-size: 12.5px; }
.gi-cb { flex: 1; }
.gi-card > .boxfoot { align-items: center; }
.gi-r { display: grid; gap: 6px; padding: 10px 16px; border-top: 1px solid var(--line); min-height: 60px; align-content: center; }
.gi-r:first-child { border-top: 0; }
.gi-g2 { align-items: stretch; }
.gi-al { flex-wrap: wrap; } .gi-al > .grow { min-width: 200px; flex: 1; }
.gi-cols { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); }
.gi-col { min-width: 0; display: flex; flex-direction: column; } .gi-col + .gi-col { border-left: 1px solid var(--line); }
.gi-col .gi-ch { background: var(--muted-bg); } .gi-col .gi-ch b { font-size: 13px; }
@container (max-width: 760px) { .gi-cols { grid-template-columns: 1fr; } .gi-col + .gi-col { border-left: 0; border-top: 1px solid var(--line); } }

.gv { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 16px; align-items: stretch; }
.gv-side { display: flex; flex-direction: column; min-width: 0; } .gv-side > .cardx { flex: 1; }
.gv-main { display: flex; flex-direction: column; min-width: 0; }
.gv-lv { display: flex; align-items: center; gap: 8px; padding: 6px 16px; border-bottom: 1px solid var(--line); flex-wrap: wrap; min-height: 44px; }
.gv-bc { display: flex; align-items: center; gap: 2px; min-width: 0; flex-wrap: wrap; }
.gv-bc button { height: 26px; padding: 0 8px; border-radius: 6px; color: var(--muted); display: inline-flex; align-items: center; gap: 6px; font-weight: 500; }
.gv-bc button:hover { background: var(--hover); color: var(--fg); }
.gv-bc button.on { color: var(--fg); background: var(--active); }
.gv-bc button small { font-size: 11px; color: var(--faint); font-weight: 400; }
.gv-bc .sep { color: var(--input); }
.gv-canvas { position: relative; flex: 1; overflow: auto; background-color: var(--bg-sub); background-image: radial-gradient(var(--line2) 1px, transparent 1.2px); background-size: 22px 22px; min-height: 560px; border-radius: 0 0 10px 10px; }
.gv-svg { display: block; width: 100%; min-width: 780px; height: auto; padding: 56px 16px 64px; box-sizing: border-box; }
.gv-n { cursor: pointer; }
.gv-n .bg { fill: var(--raised); stroke: var(--line2); stroke-width: 1; }
.gv-n:hover .bg { stroke: var(--input); }
.gv-n .t { fill: var(--fg); font-size: 12.5px; font-weight: 500; }
.gv-n .s { fill: var(--muted); font-size: 10.5px; font-family: var(--mono); }
.gv-n.tk .bg { fill: var(--accent-soft); stroke: var(--accent); }
.gv-n.imp .bg { fill: var(--warn-soft); stroke: var(--warn); }
.gv-n.root .bg { stroke: var(--warn); stroke-width: 2; }
.gv-n.sel .bg { stroke: var(--accent); stroke-width: 2; }
.gv-n.dead .bg { stroke-dasharray: 4 3; stroke: var(--danger); } .gv-n.dead .s { fill: var(--danger); }
.gv-n.dim { opacity: 0.28; }
.gv-n.out .bg { fill: var(--muted-bg); }
.gv-n .drill { fill: var(--faint); }
.gv-e { fill: none; stroke: var(--input); stroke-width: 1.4; }
.gv-e.dg { stroke-dasharray: 5 4; }
.gv-e.hl { stroke: var(--accent); stroke-width: 1.8; }
.gv-e.im { stroke: var(--warn); stroke-width: 1.8; }
.gv-e.dim { opacity: 0.25; }
.gv-el { fill: var(--muted); font-size: 10.5px; paint-order: stroke; stroke: var(--bg-sub); stroke-width: 4px; stroke-linejoin: round; }
.gv-mk { fill: var(--faint); } .gv-mk.acc { fill: var(--accent); } .gv-mk.warn { fill: var(--warn); }
.gv-ov { position: absolute; display: flex; gap: 6px; align-items: center; z-index: 2; }
.gv-ov.tl { top: 12px; left: 16px; right: 140px; flex-wrap: wrap; }
.gv-ov.tr { top: 12px; right: 16px; }
.gv-ov.bl { bottom: 12px; left: 16px; right: 16px; flex-wrap: wrap; }
.gv-zoom { display: inline-flex; flex-direction: column; background: var(--raised); border: 1px solid var(--line2); border-radius: 8px; box-shadow: var(--shadow-sm); overflow: hidden; }
.gv-zoom button { width: 32px; height: 32px; display: inline-flex; align-items: center; justify-content: center; color: var(--fg2); }
.gv-zoom button + button { border-top: 1px solid var(--line); }
.gv-zoom button:hover { background: var(--hover); }
.gv-zoom button[disabled] { color: var(--input); pointer-events: none; }
.gv-flt { height: 26px; padding: 0 9px; border-radius: 13px; background: var(--raised); border: 1px solid var(--line2); display: inline-flex; align-items: center; gap: 6px; font-size: 12px; color: var(--fg2); box-shadow: var(--shadow-sm); }
.gv-flt i { width: 8px; height: 8px; border-radius: 2px; }
.gv-flt.off { color: var(--faint); text-decoration: line-through; box-shadow: none; background: var(--muted-bg); }
.gv-flt.off i { opacity: 0.35; }
.gv-flt-lab { font-size: 11.5px; color: var(--muted); margin: 0 2px 0 6px; }
.gv-flt-lab:first-child { margin-left: 0; }
.gv-banner { display: inline-flex; align-items: center; gap: 8px; height: 30px; padding: 0 6px 0 10px; border-radius: 8px; background: var(--raised); border: 1px solid var(--accent); color: var(--accent-fg); box-shadow: var(--shadow-sm); font-size: 12.5px; }
.gv-banner.warn { border-color: var(--warn); color: var(--warn); }
.gv-panel .cx-b { gap: 18px; }
.gv-panel .cx-h h2 { font-family: var(--mono); font-weight: 600; overflow-wrap: anywhere; }
.gv-panel h3 { font-size: 12px; color: var(--muted); font-weight: 500; margin-bottom: 6px; }
.gv-rel { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 6px; min-width: 0; }
.gv-rel:hover { background: var(--hover); }
.gv-rel .mono { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gv-srch { position: relative; width: 340px; max-width: 100%; }
.gv-res { position: absolute; top: calc(100% + 4px); left: 0; right: 0; z-index: 30; background: var(--raised); border: 1px solid var(--line2); border-radius: 8px; box-shadow: var(--shadow-md); padding: 4px; display: none; max-height: 380px; overflow-y: auto; }
.gv-res.on { display: block; }
.gv-res .mi { height: auto; min-height: 34px; padding: 6px 8px; white-space: normal; }
.gv-res .sc { margin-left: auto; font-family: var(--mono); font-size: 11px; color: var(--success); }
.gv-tool { display: flex; align-items: center; gap: 8px; padding: 10px 16px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
@media (max-width: 1099px) { .gv { grid-template-columns: 1fr; } }
@media (max-width: 899px) { .gv-ov.bl { position: sticky; left: 0; bottom: auto; padding: 0 12px 12px; } .gv-canvas { min-height: 0; } .gv-svg { padding-bottom: 12px; } .gv-tool, .gv-lv { padding: 8px 12px; } .gv-srch { width: 100%; } .gv-canvas { min-height: 440px; } .gv-ov.tl { right: 60px; } }
`);

/* ---------- Data code graph (dipakai M7, M8, M10) ---------- */
const LAYERS = { ui: ["UI", "#db2777"], logic: ["Logika", "#5b5bd6"], api: ["API", "#0891b2"], data: ["Data", "#16a34a"], test: ["Test", "#c2410c"], ext: ["Eksternal", "#64748b"], doc: ["Dokumen", "#a16207"] };
const LANGS = { TypeScript: "#3178c6", Go: "#00add8", SQL: "#e38c00", Markdown: "#8d8d99" };
const LV = ["Project", "Repo", "Modul", "File", "Fungsi"];
const PATH = ["Kasir Online", "kasir-web", "cart", "promo.ts", "calcPromo()"];
const DRILL = ["web", "cart", "promo", "calcPromo"];
const TK_NOTE = { "KSR-21": "selesai · PR #131 merged", "KSR-24": "PR #142 · Code Review", "KSR-26": "PR #145 · Development", "KSR-28": "perkiraan dari tech spec" };

/* n: id, label, sub, x, y, layer, lang, kind, path, tk (tiket), docs, flags */
const L = [
  { nodes: [
      { id: "web", n: "kasir-web", s: "repo · 438 file", x: 140, y: 230, layer: "ui", lang: "TypeScript", kind: "Repo", path: "github.com/ptcontoh/kasir-web", tk: ["KSR-21", "KSR-24", "KSR-26", "KSR-28"], docs: ["Halaman & Routing", "Komponen UI & Design System"] },
      { id: "api", n: "kasir-api", s: "repo · 846 file", x: 460, y: 230, layer: "api", lang: "Go", kind: "Repo", path: "github.com/ptcontoh/kasir-api", tk: ["KSR-24", "KSR-26"], docs: ["API", "Arsitektur Sistem"] },
      { id: "db", n: "PostgreSQL", s: "34 tabel", x: 770, y: 120, layer: "data", lang: "SQL", kind: "Database", path: "kasir-api/migrations", tk: ["KSR-26"], docs: ["Model Data & Database"] },
      { id: "xnd", n: "Xendit QRIS", s: "layanan eksternal", x: 770, y: 340, layer: "ext", lang: "", kind: "Eksternal", path: "api.xendit.co", tk: ["KSR-24"], docs: ["Integrasi Pihak Ketiga"] },
      { id: "doc", n: "Arsitektur Sistem", s: "knowledge", x: 460, y: 60, layer: "doc", lang: "Markdown", kind: "Dokumen", path: ".agentgate/knowledge/arsitektur-sistem.md", tk: [], docs: [] },
      { id: "prd", n: "PRD KSR-28", s: "artifact", x: 140, y: 60, layer: "doc", lang: "Markdown", kind: "Artifact", path: "KSR-28/prd.md", tk: ["KSR-28"], docs: [] },
      { id: "prn", n: "printer-bridge", s: "shared package", x: 460, y: 400, layer: "logic", lang: "TypeScript", kind: "Shared package", path: "@ptcontoh/printer-bridge", tk: [], docs: ["Shared Library"] },
    ],
    edges: [["web", "api", 1, "HTTP · 42 call"], ["api", "db", 1, "SQL · 118 query"], ["api", "xnd", 1, "HTTP"], ["xnd", "api", 0, "webhook"], ["doc", "api", 1, "dokumentasi"], ["doc", "web", 1, ""], ["prd", "web", 0, "dugaan dampak"], ["web", "prn", 1, "import"], ["api", "prn", 0, "event struk.cetak"]] },
  { nodes: [
      { id: "ui", n: "shared/ui", s: "modul · 31 file", x: 330, y: 60, layer: "ui", lang: "TypeScript", kind: "Modul", path: "kasir-web/src/shared/ui", tk: [], docs: ["Komponen UI & Design System"] },
      { id: "catalog", n: "product-catalog", s: "modul · 22 file", x: 100, y: 160, layer: "ui", lang: "TypeScript", kind: "Modul", path: "kasir-web/src/features/catalog", tk: [], docs: [] },
      { id: "cart", n: "cart", s: "modul · 14 file", x: 330, y: 230, layer: "logic", lang: "TypeScript", kind: "Modul", path: "kasir-web/src/features/cart", tk: ["KSR-21", "KSR-26", "KSR-28"], docs: ["Aturan Bisnis", "State Management"] },
      { id: "checkout", n: "checkout", s: "node pusat · 19 file", x: 560, y: 230, layer: "logic", lang: "TypeScript", kind: "Modul (node pusat)", path: "kasir-web/src/features/checkout", tk: ["KSR-26"], docs: ["Alur Bisnis / User Journey"] },
      { id: "payment", n: "payment", s: "modul · 17 file", x: 780, y: 130, layer: "logic", lang: "TypeScript", kind: "Modul", path: "kasir-web/src/features/payment", tk: ["KSR-24"], docs: ["Integrasi Pihak Ketiga"] },
      { id: "receipt", n: "receipt", s: "modul · 12 file", x: 780, y: 330, layer: "logic", lang: "TypeScript", kind: "Modul", path: "kasir-web/src/features/receipt", tk: [], docs: [] },
      { id: "auth", n: "auth", s: "modul · 9 file", x: 100, y: 400, layer: "logic", lang: "TypeScript", kind: "Modul", path: "kasir-web/src/features/auth", tk: [], docs: ["Role & Permission aplikasi"] },
      { id: "apic", n: "api-client", s: "modul · 11 file", x: 560, y: 410, layer: "api", lang: "TypeScript", kind: "Modul", path: "kasir-web/src/api", tk: ["KSR-24", "KSR-26"], docs: ["API"] },
    ],
    edges: [["catalog", "cart", 1, ""], ["cart", "checkout", 1, "12"], ["checkout", "payment", 1, "8"], ["checkout", "receipt", 1, "5"], ["payment", "apic", 1, ""], ["checkout", "apic", 1, ""], ["receipt", "apic", 1, ""], ["cart", "ui", 1, ""], ["catalog", "ui", 1, ""], ["auth", "apic", 1, ""], ["cart", "apic", 0, "promoApi"]] },
  { nodes: [
      { id: "page", n: "CartPage.tsx", s: "komponen · 210 baris", x: 110, y: 140, layer: "ui", lang: "TypeScript", kind: "File", path: "src/features/cart/CartPage.tsx", tk: [], docs: ["Halaman & Routing"] },
      { id: "row", n: "CartItemRow.tsx", s: "komponen · 96 baris", x: 110, y: 330, layer: "ui", lang: "TypeScript", kind: "File", path: "src/features/cart/CartItemRow.tsx", tk: ["KSR-28"], docs: ["Komponen UI & Design System"] },
      { id: "disc", n: "DiscountInput.tsx", s: "komponen · 58 baris", x: 340, y: 420, layer: "ui", lang: "TypeScript", kind: "File", path: "src/features/cart/DiscountInput.tsx", tk: ["KSR-28"], docs: [] },
      { id: "store", n: "cartStore.ts", s: "state · 184 baris", x: 340, y: 140, layer: "logic", lang: "TypeScript", kind: "File", path: "src/features/cart/cartStore.ts", tk: ["KSR-26"], docs: ["State Management"] },
      { id: "totals", n: "useCartTotals.ts", s: "hook · 72 baris", x: 570, y: 140, layer: "logic", lang: "TypeScript", kind: "File", path: "src/features/cart/useCartTotals.ts", tk: ["KSR-21"], docs: [] },
      { id: "promo", n: "promo.ts", s: "logika · 148 baris", x: 570, y: 330, layer: "logic", lang: "TypeScript", kind: "File", path: "src/features/cart/promo.ts", tk: ["KSR-21", "KSR-28"], docs: ["Aturan Bisnis"] },
      { id: "ptest", n: "promo.test.ts", s: "test · 14 kasus", x: 800, y: 420, layer: "test", lang: "TypeScript", kind: "File", path: "src/features/cart/promo.test.ts", tk: ["KSR-21"], docs: ["Strategi Testing"] },
      { id: "papi", n: "promoApi.ts", s: "luar modul · api-client", x: 800, y: 230, layer: "api", lang: "TypeScript", kind: "File", path: "src/api/promoApi.ts", tk: [], docs: ["API"], out: 1 },
    ],
    edges: [["page", "row", 1, ""], ["page", "store", 1, ""], ["row", "disc", 1, ""], ["disc", "store", 1, ""], ["page", "totals", 1, ""], ["totals", "store", 1, ""], ["totals", "promo", 1, ""], ["ptest", "promo", 1, ""], ["promo", "papi", 0, "dynamic import"]] },
  { nodes: [
      { id: "ext1", n: "useCartTotals", s: "luar file", x: 110, y: 150, layer: "logic", lang: "TypeScript", kind: "Fungsi", path: "src/features/cart/useCartTotals.ts:18", tk: ["KSR-21"], docs: [], out: 1 },
      { id: "ext2", n: "promo.test.ts", s: "luar file · 14 kasus", x: 110, y: 300, layer: "test", lang: "TypeScript", kind: "Test", path: "src/features/cart/promo.test.ts", tk: [], docs: [], out: 1 },
      { id: "calcPromo", n: "calcPromo()", s: "export · baris 12", x: 350, y: 220, layer: "logic", lang: "TypeScript", kind: "Fungsi", path: "src/features/cart/promo.ts:12", tk: ["KSR-28"], docs: ["Aturan Bisnis"] },
      { id: "apply", n: "applyItemDiscount()", s: "baris 41", x: 600, y: 100, layer: "logic", lang: "TypeScript", kind: "Fungsi", path: "src/features/cart/promo.ts:41", tk: ["KSR-28"], docs: [] },
      { id: "cap", n: "capDiscount()", s: "baris 66", x: 600, y: 220, layer: "logic", lang: "TypeScript", kind: "Fungsi", path: "src/features/cart/promo.ts:66", tk: [], docs: ["Aturan Bisnis"] },
      { id: "round", n: "roundRupiah()", s: "baris 88", x: 600, y: 340, layer: "logic", lang: "TypeScript", kind: "Fungsi", path: "src/features/cart/promo.ts:88", tk: ["KSR-21"], docs: [] },
      { id: "active", n: "isPromoActive()", s: "baris 101", x: 350, y: 400, layer: "logic", lang: "TypeScript", kind: "Fungsi", path: "src/features/cart/promo.ts:101", tk: [], docs: [] },
      { id: "legacy", n: "legacyBundlePromo()", s: "dead code · 0 pemanggil", x: 110, y: 430, layer: "logic", lang: "TypeScript", kind: "Fungsi", path: "src/features/cart/promo.ts:120", tk: [], docs: [], dead: 1 },
      { id: "pval", n: "promoApi.validate()", s: "luar file", x: 820, y: 220, layer: "api", lang: "TypeScript", kind: "Fungsi", path: "src/api/promoApi.ts:9", tk: [], docs: ["API"], out: 1 },
    ],
    edges: [["ext1", "calcPromo", 1, ""], ["ext2", "calcPromo", 1, ""], ["calcPromo", "apply", 1, ""], ["calcPromo", "cap", 1, ""], ["calcPromo", "round", 1, ""], ["calcPromo", "active", 1, ""], ["apply", "round", 1, ""], ["cap", "pval", 0, "dugaan"], ["legacy", "round", 1, ""]] },
  { nodes: [
      { id: "c1", n: "useCartTotals()", s: "cart/useCartTotals.ts", x: 130, y: 90, layer: "logic", lang: "TypeScript", kind: "Pemanggil", path: "src/features/cart/useCartTotals.ts:18", tk: ["KSR-21"], docs: [], out: 1 },
      { id: "c2", n: "CheckoutSummary", s: "checkout/Summary.tsx", x: 130, y: 230, layer: "ui", lang: "TypeScript", kind: "Pemanggil", path: "src/features/checkout/Summary.tsx:44", tk: ["KSR-26"], docs: [], out: 1 },
      { id: "c3", n: "promo.test.ts", s: "7 kasus calcPromo", x: 130, y: 370, layer: "test", lang: "TypeScript", kind: "Test", path: "src/features/cart/promo.test.ts", tk: [], docs: [], out: 1 },
      { id: "calcPromo", n: "calcPromo()", s: "promo.ts:12 · 38 baris", x: 440, y: 230, layer: "logic", lang: "TypeScript", kind: "Fungsi", path: "src/features/cart/promo.ts:12", tk: ["KSR-28"], docs: ["Aturan Bisnis"] },
      { id: "d1", n: "applyItemDiscount()", s: "promo.ts:41", x: 760, y: 50, layer: "logic", lang: "TypeScript", kind: "Dipanggil", path: "src/features/cart/promo.ts:41", tk: ["KSR-28"], docs: [] },
      { id: "d2", n: "capDiscount()", s: "promo.ts:66", x: 760, y: 150, layer: "logic", lang: "TypeScript", kind: "Dipanggil", path: "src/features/cart/promo.ts:66", tk: [], docs: [] },
      { id: "d3", n: "isPromoActive()", s: "promo.ts:101", x: 760, y: 250, layer: "logic", lang: "TypeScript", kind: "Dipanggil", path: "src/features/cart/promo.ts:101", tk: [], docs: [] },
      { id: "d4", n: "roundRupiah()", s: "promo.ts:88", x: 760, y: 350, layer: "logic", lang: "TypeScript", kind: "Dipanggil", path: "src/features/cart/promo.ts:88", tk: ["KSR-21"], docs: [] },
      { id: "doc", n: "Aturan Bisnis", s: "knowledge · bagian Diskon", x: 440, y: 70, layer: "doc", lang: "Markdown", kind: "Dokumen", path: ".agentgate/knowledge/aturan-bisnis.md#diskon", tk: [], docs: [] },
      { id: "pv", n: "promoApi.validate()", s: "api/promoApi.ts:9", x: 440, y: 420, layer: "api", lang: "TypeScript", kind: "Dipanggil", path: "src/api/promoApi.ts:9", tk: [], docs: [] },
      { id: "rt", n: "POST /v1/promo/validate", s: "kasir-api · lintas repo", x: 760, y: 460, layer: "api", lang: "Go", kind: "Route", path: "kasir-api/internal/promo/handler.go:27", tk: [], docs: ["API"], out: 1 },
    ],
    edges: [["c1", "calcPromo", 1, ""], ["c2", "calcPromo", 0, "via props"], ["c3", "calcPromo", 1, ""], ["calcPromo", "d1", 1, ""], ["calcPromo", "d2", 1, ""], ["calcPromo", "d3", 1, ""], ["calcPromo", "d4", 1, ""], ["doc", "calcPromo", 1, ""], ["calcPromo", "pv", 0, "dugaan"], ["pv", "rt", 0, "HTTP"]] },
];
const SYMS = [
  { n: "calcPromo", k: "fungsi", f: "kasir-web/src/features/cart/promo.ts:12", lv: 4, id: "calcPromo", sem: 0.87 },
  { n: "applyItemDiscount", k: "fungsi", f: "kasir-web/src/features/cart/promo.ts:41", lv: 3, id: "apply", sem: 0.81 },
  { n: "capDiscount", k: "fungsi", f: "kasir-web/src/features/cart/promo.ts:66", lv: 3, id: "cap", sem: 0.74 },
  { n: "DiscountInput", k: "komponen", f: "kasir-web/src/features/cart/DiscountInput.tsx", lv: 2, id: "disc", sem: 0.66 },
  { n: "promo.ts", k: "file", f: "kasir-web/src/features/cart/promo.ts", lv: 2, id: "promo", sem: 0.63 },
  { n: "roundRupiah", k: "fungsi", f: "kasir-web/src/features/cart/promo.ts:88", lv: 3, id: "round", sem: 0.41 },
  { n: "isPromoActive", k: "fungsi", f: "kasir-web/src/features/cart/promo.ts:101", lv: 3, id: "active", sem: 0.38 },
  { n: "legacyBundlePromo", k: "fungsi", f: "kasir-web/src/features/cart/promo.ts:120", lv: 3, id: "legacy", sem: 0.36 },
  { n: "cartStore", k: "file", f: "kasir-web/src/features/cart/cartStore.ts", lv: 2, id: "store" },
  { n: "useCartTotals", k: "hook", f: "kasir-web/src/features/cart/useCartTotals.ts:18", lv: 2, id: "totals" },
  { n: "CartPage", k: "komponen", f: "kasir-web/src/features/cart/CartPage.tsx", lv: 2, id: "page" },
  { n: "checkout", k: "modul", f: "kasir-web/src/features/checkout", lv: 1, id: "checkout" },
  { n: "payment", k: "modul", f: "kasir-web/src/features/payment", lv: 1, id: "payment" },
  { n: "kasir-api", k: "repo", f: "github.com/ptcontoh/kasir-api", lv: 0, id: "api" },
];
const SNIP = `<span class="faint">// src/features/cart/promo.ts</span>
<span class="text-acc">export function</span> calcPromo(items: CartItem[], promo?: Promo): PromoResult {
  <span class="text-acc">if</span> (!promo || !isPromoActive(promo, <span class="text-acc">new</span> Date())) <span class="text-acc">return</span> { diskon: <span class="text-ok">0</span>, rincian: [] };
  <span class="text-acc">const</span> rincian = items.map((it) => applyItemDiscount(it, promo));
  <span class="text-acc">const</span> total = rincian.reduce((s, r) => s + r.potongan, <span class="text-ok">0</span>);
  <span class="text-acc">return</span> { diskon: roundRupiah(capDiscount(total, promo.maks)), rincian };
}`;

const GV = { lv: 0, sel: "web", tk: null, imp: false, hide: new Set() };
AG.GRAPH = { L, LV, PATH, GV, LAYERS };
const nodeOf = (lv, id) => L[lv].nodes.find((n) => n.id === id);

/* ---------- Dialog & aksi graph ---------- */
AG.action("gv-open", (el, arg) => { const [lv, id] = arg.split(":"); GV.lv = +lv; GV.sel = id; GV.imp = false; if (location.hash !== `#/p/${K}/graph`) location.hash = `#/p/${K}/graph`; else AG.rerender(); });
AG.action("gv-sel", (el, arg) => { GV.sel = arg; AG.rerender(); });
AG.action("gv-lv", (el, arg) => { GV.lv = +arg; GV.sel = GV.lv === 4 ? "calcPromo" : DRILL[GV.lv]; GV.imp = false; AG.rerender(); });
AG.action("gv-in", () => {
  const n = nodeOf(GV.lv, GV.sel);
  if (GV.lv >= 4) return AG.toast("Sudah di level fungsi");
  if (n && n.id === DRILL[GV.lv]) { GV.lv++; GV.sel = GV.lv === 4 ? "calcPromo" : DRILL[GV.lv]; GV.imp = false; return AG.rerender(); }
  AG.toast(`Prototype: zoom contoh lewat ${PATH.slice(1).join(" › ")}`);
});
AG.action("gv-out", () => { if (GV.lv === 0) return; GV.lv--; GV.sel = DRILL[GV.lv]; GV.imp = false; AG.rerender(); });
AG.action("gv-fit", () => { GV.imp = false; GV.tk = null; GV.hide.clear(); AG.rerender(); AG.toast("Tampilan direset"); });
AG.action("gv-imp", () => { GV.imp = !GV.imp; AG.rerender(); });
AG.action("gv-tk", (el, arg) => { GV.tk = arg || null; AG.rerender(); });
AG.action("gv-flt", (el, arg) => { GV.hide.has(arg) ? GV.hide.delete(arg) : GV.hide.add(arg); AG.rerender(); });
AG.action("gv-q", (el, arg) => { const i = document.getElementById("gv-q"); if (i) { i.value = arg; i.dispatchEvent(new Event("input")); i.focus(); } });
AG.menu("gv-ticket", () => `<div class="mlab">Sorot kode yang tersentuh tiket</div>
  <button class="mi ${!GV.tk ? "hov" : ""}" data-act="gv-tk" data-arg="">${ic("circle-x")}Tanpa sorotan</button><div class="msep"></div>
  ${Object.keys(TK_NOTE).map((k) => { const t = AG.ticket(k); return `<button class="mi ${GV.tk === k ? "hov" : ""}" data-act="gv-tk" data-arg="${k}">${AG.typeIcon(t.type)}<span class="mono">${k}</span><span class="trunc" style="max-width:150px">${t.t}</span><span class="tail">${TK_NOTE[k].split(" · ")[0]}</span></button>`; }).join("")}`);
AG.dialog("gv-snippet", (arg) => { const n = nodeOf(GV.lv, arg) || nodeOf(4, "calcPromo"); return { w: "w760", icon: "code", title: n.n, desc: `<span class="mono">${n.path}</span> · diambil lewat <code>ag_snippet</code>`,
  body: `<div class="code">${SNIP}</div><div class="row wrap sm mut">${ic("info-circle", "s14")}Relasi ditandai <b class="fg">pasti</b> bila hasil parsing tree-sitter/SCIP, <b class="fg">dugaan</b> bila hasil inferensi (misalnya URL dinamis).</div>`,
  foot: `${btn("Salin", "", "copy", { "data-toast": "Kode disalin" })}${btn("Buka di GitHub", "", "external-link", { "data-toast": "Membuka promo.ts di GitHub" })}${btn("Tutup", "pri", "", { "data-close": true })}` }; });
AG.dialog("gi-reindex", () => ({ w: "w560", icon: "refresh", tone: "acc", title: "Index ulang code graph", desc: "Tree-sitter berjalan di server tanpa token AI. SCIP diminta dari runner untuk resolusi simbol yang presisi.",
  body: `${AG.fld("Cakupan", `<div class="col g4">${[["all", "Semua repo project", "kasir-api, kasir-web · 1.284 file"], ["api", "kasir-api", "846 file · Go"], ["web", "kasir-web", "438 file · TypeScript"]].map(([v, t, d], i) => `<button class="lrow" style="border:1px solid var(--line);border-radius:8px;min-height:44px" data-radio="gi-sc"><span class="radio ${i === 0 ? "on" : ""}"></span><span class="grow"><b class="b5">${t}</b> <span class="mut sm">${d}</span></span></button>`).join("")}</div>`)}
  ${AG.fld("Jenis", `<div class="seg"><button class="on">Inkremental (file berubah)</button><button>Penuh</button></div>`, "Inkremental hanya memproses 37 file yang berubah sejak index terakhir.")}
  <div class="box">${AG.srow("Minta SCIP dari runner", "scip-go dan scip-typescript dijalankan di laptop pemilik runner", AG.sel([["r2", "Laptop Dewi · online"], ["r1", "MacBook Olivia · online"], ["r4", "ThinkPad Adi · offline"]], "r2"))}${AG.srow("Perbarui embedding", "fastembed-rs untuk semantic search", sw(true, "Perbarui embedding"))}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Mulai index ulang", "pri", "refresh", { "data-close": true, "data-toast": "Index ulang dimulai · estimasi 3 menit" })}` }));

const gTabs = (cur) => `<nav class="tabs" aria-label="Code graph"><a href="#/p/${K}/graph" class="${cur === "v" ? "on" : ""}">${ic("topology-star-3", "s14")}Viewer</a><a href="#/p/${K}/graph/index" class="${cur === "i" ? "on" : ""}">${ic("database", "s14")}Status index</a></nav>`;
const gHead = (act = "") => head("Code graph", "Peta kode Kasir Online dari tree-sitter (server) dan SCIP (CLI). Dipakai agent lewat MCP dan oleh tim untuk analisis dampak.", act);
const conf = (p) => (p ? pill("pasti", "ok") : pill("dugaan", "warn"));
const langBar = (parts) => `<div class="gi-lang" title="${parts.map(([n, v]) => `${n} ${v}%`).join(", ")}">${parts.map(([n, v]) => `<i style="width:${v}%;background:${LANGS[n] || "#a3a3ad"}"></i>`).join("")}</div>`;
/* Kartu dengan judul di dalam: dipakai grid supaya judul sejajar dan kartu sama tinggi. */
const gCard = (t, d, body, action = "") => AG.card({ title: t, desc: d, action, body, pad: false });
/* Tautan teks header kartu yang memicu aksi/toast (bukan pindah rute). */
const seeBtn = (label, attrs) => `<button type="button" class="seeall" ${Object.entries(attrs).map(([k, v]) => `${k}="${esc(v)}"`).join(" ")}>${label}${ic("arrow-right", "s14")}</button>`;

/* Sub-menu lokal: ingat panel aktif supaya tetap sama saat layar dirender ulang. */
const SUBS = { gi: { sec: "repo" }, tk: { sec: "ring" }, ws: { sec: "docs" } };
AG.SUBS_M7 = SUBS;
document.addEventListener("click", (e) => {
  const a = e.target.closest(".sp-i[data-tab]"); if (!a) return;
  const w = a.closest("[data-sp]"); if (!w || !SUBS[w.dataset.sp]) return;
  e.preventDefault(); SUBS[w.dataset.sp].sec = a.dataset.tab;
}, true);
AG.spWrap = (key, o) => `<div data-sp="${key}">${AG.subpage({ ...o, active: SUBS[key].sec })}</div>`;

/* ====================== M7 · Status index ====================== */
const REPOS = [
  { n: "kasir-api", lang: [["Go", 92], ["SQL", 6], ["YAML", 2]], ts: `<span class="row g4 nowrap">${ic("circle-check", "s14 text-ok")}Terbaru · <span class="mono">8e1f0c2</span></span><span class="mut sm nowrap">14 mnt lalu</span>`, scip: `<span class="row g4 nowrap">${ic("circle-check", "s14 text-ok")}scip-go</span><span class="mut sm nowrap">Laptop Dewi · 2 jam lalu</span>`, sym: 7912, file: 846, rel: 27400, cov: 98.1, up: "14 mnt lalu", st: "Terbaru" },
  { n: "kasir-web", lang: [["TypeScript", 95], ["CSS", 5]], ts: `<span class="col g4" style="min-width:140px"><span class="row g4 text-acc nowrap">${ic("loader-2", "s14")}Inkremental · 3 file</span><span class="bar"><i style="width:64%"></i></span></span>`, scip: `<span class="row g4 text-warn nowrap">${ic("alert-triangle", "s14")}scip-typescript</span><span class="text-warn sm nowrap">basi 5 hari</span>`, sym: 10508, file: 438, rel: 36910, cov: 95.2, up: "sedang berjalan", st: "Berjalan" },
];
const FLOWS = [
  ["kasir-web", "payment/qrisApi.ts · createQris()", "kasir-api", "POST /v1/payments/qris", 1, "HTTP"],
  ["kasir-web", "checkout/stockApi.ts · checkStock()", "kasir-api", "GET /v1/stock/:sku", 1, "HTTP"],
  ["kasir-web", "api/promoApi.ts · validate()", "kasir-api", "POST /v1/promo/validate", 0, "HTTP"],
  ["Xendit", "callback pembayaran", "kasir-api", "POST /v1/payments/qris/callback", 1, "Webhook"],
  ["kasir-api", "payment.Settle() publish", "kasir-api worker", "payment.settled → receipt.Print", 1, "Queue"],
  ["kasir-web", "report/reportApi.ts · exportCsv()", "kasir-api", "Route tidak ditemukan", 2, "HTTP"],
  ["kasir-web", "receipt/printer.ts · print()", "printer-bridge", "@ptcontoh/printer-bridge · cetakStruk()", 1, "Shared package"],
  ["kasir-api", "receipt.Print() publish", "printer-bridge", "event struk.cetak", 0, "Queue"],
];
const HOT = [["kasir-api/internal/payment/qris.go", 41, 92, "DD"], ["kasir-web/src/features/cart/promo.ts", 33, 74, "AA"], ["kasir-web/src/features/checkout/Summary.tsx", 27, 61, "DD"], ["kasir-api/internal/receipt/render.go", 22, 50, "QQ"], ["kasir-api/migrations/0031_promo.sql", 14, 31, "AA"]];
const COUP = [["cart/promo.ts", "cart/useCartTotals.ts", 86], ["payment/qris.go", "payment/callback.go", 79], ["receipt/render.go", "receipt/template.html", 71], ["checkout/Summary.tsx", "cart/cartStore.ts", 58], ["auth/pin.go", "auth/session.go", 52]];
const OWN = [["payment", "DD", 64, 212], ["cart", "AA", 52, 148], ["receipt", "QQ", 47, 96], ["checkout", "DD", 58, 131], ["auth", "DD", 81, 74]];
const DEAD = [["legacyBundlePromo()", "kasir-web", "cart/promo.ts:120", 1, "3:legacy", "4 bulan"], ["formatStrukLama()", "kasir-api", "receipt/legacy.go:14", 1, "3:legacy", "7 bulan"], ["GetStockV1()", "kasir-api", "stock/handler.go:88", 0, "1:checkout", "2 bulan"], ["useOldPrinter()", "kasir-web", "receipt/useOldPrinter.ts", 1, "1:receipt", "5 bulan"], ["parseVoucherCode()", "kasir-web", "cart/voucher.ts:31", 0, "2:promo", "3 minggu"]];
const BENCH = [["Agent Gate (tree-sitter + SCIP)", 97, 95, "3 mnt 10 dtk", "182 MB", 1], ["codebase-memory", 94, 91, "4 mnt 42 dtk", "240 MB", 0], ["Graphify", 89, 86, "6 mnt 05 dtk", "311 MB", 0], ["Tree-sitter saja (tanpa SCIP)", 84, 88, "2 mnt 20 dtk", "120 MB", 0]];

function giRepo() {
  const tbl = AG.datatable({ id: "gi-repos", search: "", pageSize: 10,
    columns: [{ label: "Repo" }, { label: "Bahasa", sort: false, hide: "sm" }, { label: "Tree-sitter (server)", sort: false }, { label: "SCIP (CLI)", sort: false, hide: "sm" }, { label: "Simbol", hide: "sm", align: "right" }, { label: "Cakupan" }, { label: "", sort: false, w: "44px" }],
    rows: REPOS.map((r) => ({ sort: [r.n, "", "", "", r.sym, r.cov], text: r.n,
      cells: [`<span class="row nowrap">${ic("git-branch", "s14 faint")}<b class="b5">${r.n}</b></span><span class="mut sm nowrap">${r.file} file · main</span>`, `${langBar(r.lang)}<span class="xs mut">${r.lang.map(([n, v]) => `${n} ${v}%`).slice(0, 1).join(" · ")}</span>`, r.ts, r.scip, `<span class="tab-num">${r.sym.toLocaleString("id-ID")}</span>`,
        `<span class="col g4" style="min-width:70px"><span class="tab-num">${String(r.cov).replace(".", ",")}%</span><span class="bar ${r.cov > 97 ? "ok" : ""}"><i style="width:${r.cov}%"></i></span></span>`, btn("", "ghost icon sm", "dots", { "data-menu": "gi-repo", "data-arg": r.n, "aria-label": `Aksi ${r.n}` })] })) });
  const alert = `<div class="alert gi-al warn">${ic("alert-triangle", "s14")}<span class="grow">SCIP kasir-web basi 5 hari. Relasi lintas file TypeScript memakai hasil tree-sitter sampai runner mengirim index baru. Jalankan <code>agentgate graph scip</code> di laptop Anda.</span>${btn("Minta ke runner", "sm", "send", { "data-toast": "Permintaan SCIP dikirim ke Laptop Dewi" })}</div>`;
  return `${AG.card({ title: "Repo & index", desc: "Status tiap repo. SCIP hanya dikirim dari laptop user; server cukup tree-sitter.", body: `${alert}${AG.stats([
      { label: "Simbol", value: "18.420", icon: "code", hint: "fungsi, kelas, tipe, route" },
      { label: "Relasi", value: "64.310", icon: "topology-star-3", hint: "91% pasti · 9% dugaan" },
      { label: "File terindeks", value: `1.238<span class="mut" style="font-size:14px;font-weight:500">/ 1.284</span>`, icon: "file-code", hint: "46 dilewati" },
      { label: "Cakupan index", value: "96,4%", icon: "gauge", tone: "ok", delta: "+1,2%", hint: "naik minggu ini" },
    ])}${tbl}` })}
    <div class="grid2 gi-g2">
      ${gCard("Cakupan index", "File yang tidak masuk graph beserta alasannya. File rahasia tidak pernah di-upload.", [["file-code", "File hasil generate", "src/gen/*.pb.go", 21], ["eye-off", "Diabaikan .gitignore", "dist/, coverage/", 20], ["archive", "Terlalu besar (&gt; 1 MB)", "vendor/qrcode.min.js", 3], ["alert-circle", "Gagal parsing", "receipt/legacy-old.js", 2]].map(([i, t, d, n]) => `<div class="lrow"><span class="tile">${ic(i, "s14")}</span><span class="grow" style="min-width:0"><b class="b5">${t}</b><br><span class="mono mut sm">${d}</span></span><span class="tab-num b5">${n}</span></div>`).join(""),
        seeBtn("Lihat 46 file", { "data-toast": "Daftar 46 file dibuka" }))}
      ${gCard("Mesin index", "Komponen yang membangun dan melayani graph.", [["binary-tree", "Tree-sitter", "Server · 8 bahasa prioritas · tanpa token AI"], ["terminal-2", "SCIP", "Dari CLI · scip-go, scip-typescript"], ["sparkles", "Embedding", "fastembed-rs · 18.420 vektor"], ["plug-connected", "Tool MCP", "ag_search 1.204 · ag_trace 388 · ag_snippet 2.010 minggu ini"]].map(([i, t, d]) => `<div class="lrow"><span class="tile acc">${ic(i, "s14")}</span><span class="grow" style="min-width:0"><b class="b5">${t}</b><br><span class="mut sm">${d}</span></span></div>`).join(""),
        seeBtn("Benchmark", { "data-act": "gi-sec", "data-arg": "bench" }))}
    </div>`;
}
function giCross() {
  const tbl = AG.datatable({ id: "gi-flows", search: "Cari endpoint atau file…", pageSize: 10,
    filters: [{ k: "jenis", label: "Jenis", icon: "plug-connected", options: ["HTTP", "Webhook", "Queue", "Shared package"] }, { k: "conf", label: "Keyakinan", icon: "circle-check", options: ["pasti", "dugaan", "tidak cocok"] }],
    columns: [{ label: "Asal" }, { label: "", sort: false, w: "28px", hide: "sm" }, { label: "Tujuan" }, { label: "Jenis", hide: "sm" }, { label: "Keyakinan" }],
    rows: FLOWS.map(([a, af, b, bf, p, kind]) => ({ sort: [a + af, "", b + bf, kind, p], filter: { jenis: kind, conf: p === 2 ? "tidak cocok" : p ? "pasti" : "dugaan" }, text: `${a} ${af} ${b} ${bf} ${kind}`,
      cells: [`<span class="gi-ep"><b>${af}</b><span>${a}</span></span>`, `<span class="faint">${ic("arrow-right", "s14")}</span>`, `<span class="gi-ep"><b class="${p === 2 ? "text-dng" : ""}">${bf}</b><span>${b}</span></span>`, `<span class="mut">${kind}</span>`, `<span class="row g4 nowrap">${p === 2 ? pill("tidak cocok", "dng") + tref("KSR-27") : conf(p)}</span>`] })) });
  return AG.card({ title: "Lintas repo", desc: "HTTP call ↔ route, event/queue, dan shared package yang dicocokkan antar repo.", action: `<div class="gi-legend hide-sm"><span><i style="background:var(--success)"></i>pasti = hasil parsing</span><span><i style="background:var(--warn)"></i>dugaan = hasil inferensi</span></div>`, body: `${AG.stats([
      { label: "Relasi lintas repo", value: FLOWS.length, icon: "arrows-exchange", hint: "antar 3 repo" },
      { label: "Pasti", value: FLOWS.filter((f) => f[4] === 1).length, icon: "circle-check", tone: "ok", hint: "hasil parsing" },
      { label: "Dugaan", value: FLOWS.filter((f) => f[4] === 0).length, icon: "help-circle", tone: "warn", hint: "hasil inferensi" },
      { label: "Tidak cocok", value: FLOWS.filter((f) => f[4] === 2).length, icon: "alert-triangle", tone: "dng", hint: "route tidak ditemukan" },
    ])}${tbl}` });
}
function giGit() {
  const rowsHot = HOT.map(([f, c, w, o]) => `<div class="gi-r"><span class="row"><span class="mono sm trunc grow" title="${f}">${f.split("/").slice(-2).join("/")}</span>${av(o, 18)}</span><span class="row"><span class="bar warn grow"><i style="width:${w}%"></i></span><span class="tab-num sm mut nowrap" style="width:66px;text-align:right">${c} commit</span></span></div>`).join("");
  const rowsCoup = COUP.map(([a, b, p]) => `<div class="gi-r"><span class="row"><span class="mono sm trunc grow">${a}</span><b class="tab-num sm">${p}%</b></span><span class="mono sm trunc mut">↔ ${b}</span></div>`).join("");
  const rowsOwn = OWN.map(([m, o, p, c]) => `<div class="gi-r"><span class="row">${av(o, 18)}<span class="mono sm trunc grow">${m}</span><span class="mut sm nowrap">${AG.PEOPLE[o].name.split(" ")[0]}</span></span><span class="row"><span class="bar grow"><i style="width:${p}%;background:${AG.PEOPLE[o].c}"></i></span><span class="tab-num sm mut nowrap" style="width:66px;text-align:right">${p}% · ${c}</span></span></div>`).join("");
  const col = (t, d, act, rows) => `<div class="gi-col"><div class="gi-ch"><span class="row"><b class="grow">${t}</b>${act}</span><span>${d}</span></div>${rows}</div>`;
  return AG.card({ title: "Riwayat git", desc: "Dihitung dari riwayat git 90 hari terakhir pada branch main · diperbarui setiap merge.", pad: false,
    body: `<div class="gi-cols">
      ${col("Hotspot", "File yang paling sering berubah.", seeBtn("Graph", { "data-act": "gv-open", "data-arg": "2:promo" }), rowsHot)}
      ${col("Coupling", "File yang selalu berubah bersamaan.", seeBtn("Tiket refactor", { "data-toast": "Tiket KSR-43 “Pisahkan promo dan useCartTotals” dibuat di Backlog" }), rowsCoup)}
      ${col("Pemilik kode", "Dari git blame per modul.", `<a class="seeall" href="#/settings/p/${K}/repos">Kelola${ic("arrow-right", "s14")}</a>`, rowsOwn)}
    </div>` });
}
function giQuality() {
  const tbl = AG.datatable({ id: "gi-dead", search: "Cari simbol…", pageSize: 10,
    filters: [{ k: "repo", label: "Repo", icon: "git-branch", options: ["kasir-api", "kasir-web"] }, { k: "conf", label: "Keyakinan", icon: "circle-check", options: ["pasti", "dugaan"] }],
    columns: [{ label: "Simbol" }, { label: "Repo", hide: "sm" }, { label: "Tidak dipanggil sejak", hide: "sm" }, { label: "Keyakinan" }, { label: "", sort: false, align: "right" }],
    rows: DEAD.map(([n, r, f, p, go, since]) => ({ sort: [n, r, since, p], filter: { repo: r, conf: p ? "pasti" : "dugaan" }, text: `${n} ${r} ${f}`,
      cells: [`<b class="mono">${n}</b><br><span class="mut sm mono">${f}</span>`, r, `<span class="mut">${since}</span>`, conf(p), `<span class="row g4 nowrap" style="justify-content:flex-end">${btn("", "sm ghost icon", "topology-star-3", { "data-act": "gv-open", "data-arg": go, "aria-label": `Lihat ${n} di graph`, title: "Lihat di graph" })}${btn("Buat tiket", "sm", "plus", { "data-toast": `Tiket KSR-42 “Hapus ${n}” dibuat di Backlog` })}</span>`] })) });
  return AG.card({ title: "Kualitas kode", desc: "Simbol tanpa pemanggil. Tinjau sebelum dihapus; relasi dugaan bisa saja dipanggil secara dinamis.", body: `${AG.stats([
      { label: "Dead code", value: DEAD.length, icon: "trash", tone: "warn", hint: "simbol tanpa pemanggil" },
      { label: "TODO di kode", value: 12, icon: "list-check", hint: "4 baru minggu ini" },
      { label: "Fungsi kompleks", value: 3, icon: "alert-triangle", tone: "dng", hint: "cyclomatic &gt; 15" },
    ])}${tbl}` });
}
function giBench() {
  const tbl = AG.datatable({ id: "gi-bench", search: "", pageSize: 10,
    columns: [{ label: "Alat" }, { label: "Recall call graph", align: "right" }, { label: "Presisi", align: "right", hide: "sm" }, { label: "Waktu index", hide: "sm" }, { label: "Ukuran index", hide: "sm" }],
    rows: BENCH.map(([n, r, p, t, s, me]) => ({ sort: [n, r, p, t, s], cells: [`<span class="row">${me ? ic("star", "s14 text-acc") : ic("circle", "s14 faint")}<b class="${me ? "b6" : "b5"}">${n}</b></span>`, `<span class="row g8 nowrap" style="justify-content:flex-end"><span class="bar ${me ? "ok" : ""}" style="width:90px"><i style="width:${r}%"></i></span><span class="tab-num b5" style="width:32px;text-align:right">${r}%</span></span>`, `<span class="tab-num">${p}%</span>`, `<span class="mut">${t}</span>`, `<span class="mut tab-num">${s}</span>`] })) });
  return AG.card({ title: "Benchmark", desc: "Recall dan presisi menjawab pertanyaan “siapa memanggil X” dan “X memanggil apa”.", action: btn("Jalankan ulang", "sm", "player-play", { "data-toast": "Benchmark dijalankan di server · estimasi 12 menit" }),
    body: `<div class="alert ok">${ic("circle-check", "s14")}<span><b>Benchmark lulus · 8 Nov.</b> Repo yang sama (kasir-api + kasir-web), 412 pertanyaan call graph yang dikurasi tim. Syarat lulus: recall minimal 95%.</span></div>${tbl}` });
}

AG.screen({ m: "M7", g: "Code graph", n: "Status index code graph", route: "/p/:k/graph/index", path: "/p/KSR/graph/index",
  render: () => {
    const nav = [
      { k: "repo", label: "Repo & index", icon: "database", count: REPOS.length },
      { k: "cross", label: "Lintas repo", icon: "arrows-exchange", count: FLOWS.length },
      { k: "git", label: "Riwayat git", icon: "history" },
      { k: "quality", label: "Kualitas kode", icon: "trash", count: DEAD.length },
      { k: "bench", label: "Benchmark", icon: "gauge" },
    ];
    const s = SUBS.gi.sec;
    const body = `${gHead(btn("Index ulang", "pri", "refresh", { "data-dialog": "gi-reindex" }))}
      ${gTabs("i")}
      ${AG.spWrap("gi", { nav, body: AG.panel("repo", giRepo(), s === "repo") + AG.panel("cross", giCross(), s === "cross") + AG.panel("git", giGit(), s === "git") + AG.panel("quality", giQuality(), s === "quality") + AG.panel("bench", giBench(), s === "bench") })}`;
    return AG.shell({ project: K, crumbs: AG.projectCrumb(K, "Code graph"), body, width: "wide" });
  } });
AG.action("gi-sec", (el, arg) => { SUBS.gi.sec = arg; AG.rerender(); scrollTo(0, 0); });
AG.menu("gi-repo", (arg) => `<div class="mlab">${arg}</div>
  <button class="mi" data-dialog="gi-reindex">${ic("refresh")}Index ulang repo ini</button>
  <button class="mi" data-toast="Permintaan SCIP dikirim ke Laptop Dewi">${ic("terminal-2")}Minta SCIP dari runner</button>
  <a class="mi" href="#/settings/p/${K}/repos">${ic("settings")}Pengaturan repo</a>
  <button class="mi" data-toast="Log index terakhir dibuka">${ic("logs")}Log index</button>`);

/* ====================== M10 · Graph viewer ====================== */
function edgePath(a, b, W, H) {
  const ax = a.x, ay = a.y, bx = b.x, by = b.y;
  const dx = bx - ax, dy = by - ay, len = Math.hypot(dx, dy) || 1;
  const nx = -dy / len, ny = dx / len, bend = 14;
  const mx = (ax + bx) / 2 + nx * bend, my = (ay + by) / 2 + ny * bend;
  const clip = (cx, cy, tx, ty, pad) => { const vx = tx - cx, vy = ty - cy; const t = Math.min((W / 2 + pad) / Math.abs(vx || 1e-6), (H / 2 + pad) / Math.abs(vy || 1e-6)); return [cx + vx * t, cy + vy * t]; };
  const [sx, sy] = clip(ax, ay, mx, my, 2), [ex, ey] = clip(bx, by, mx, my, 5);
  return { d: `M${sx.toFixed(1)},${sy.toFixed(1)} Q${mx.toFixed(1)},${my.toFixed(1)} ${ex.toFixed(1)},${ey.toFixed(1)}`, lx: (sx + 2 * mx + ex) / 4, ly: (sy + 2 * my + ey) / 4 };
}
function impactSet(lv, id) {
  /* Analisis dampak: semua node yang bergantung pada node terpilih (pemanggil, berulang). */
  const out = new Set(), q = [id];
  while (q.length) { const c = q.shift(); L[lv].edges.forEach(([f, t]) => { if (t === c && !out.has(f) && f !== id) { out.add(f); q.push(f); } }); }
  return out;
}
function graphSvg() {
  const lvl = L[GV.lv], W = 164, H = 50;
  const vis = (n) => !GV.hide.has(n.layer) && !(n.lang && GV.hide.has(n.lang));
  const imp = GV.imp ? impactSet(GV.lv, GV.sel) : new Set();
  const byId = Object.fromEntries(lvl.nodes.map((n) => [n.id, n]));
  const edges = lvl.edges.filter(([f, t]) => vis(byId[f]) && vis(byId[t])).map(([f, t, p, lab]) => {
    const a = byId[f], b = byId[t], { d, lx, ly } = edgePath(a, b, W, H);
    const hl = GV.tk && a.tk.includes(GV.tk) && b.tk.includes(GV.tk);
    const im = GV.imp && (imp.has(f) || f === GV.sel) && (imp.has(t) || t === GV.sel);
    const dim = GV.imp ? !im : GV.tk && !hl;
    return `<path class="gv-e ${p ? "" : "dg"} ${hl ? "hl" : ""} ${im ? "im" : ""} ${dim ? "dim" : ""}" d="${d}" marker-end="url(#gv-a${hl ? "c" : im ? "w" : ""})"/>${lab && !dim ? `<text class="gv-el" x="${lx.toFixed(0)}" y="${(ly - 4).toFixed(0)}" text-anchor="middle">${esc(lab)}</text>` : ""}`;
  }).join("");
  const nodes = lvl.nodes.filter(vis).map((n) => {
    const tk = GV.tk && n.tk.includes(GV.tk), im = imp.has(n.id), root = GV.imp && n.id === GV.sel;
    const dim = GV.imp ? !im && !root : GV.tk && !tk && n.id !== GV.sel;
    const drill = GV.lv < 4 && n.id === DRILL[GV.lv];
    const col = LAYERS[n.layer][1];
    return `<g class="gv-n ${n.id === GV.sel ? "sel" : ""} ${tk ? "tk" : ""} ${im ? "imp" : ""} ${root ? "root" : ""} ${dim ? "dim" : ""} ${n.dead ? "dead" : ""} ${n.out ? "out" : ""}" data-act="gv-sel" data-arg="${n.id}" ${drill ? `data-dbl="1"` : ""} transform="translate(${n.x - W / 2},${n.y - H / 2})" role="button" tabindex="0" aria-label="${esc(n.kind + " " + n.n)}">
      <rect class="bg" width="${W}" height="${H}" rx="9"/><rect x="0" y="9" width="3" height="${H - 18}" rx="1.5" fill="${col}"/>
      <text class="t" x="14" y="21">${esc(n.n.length > 21 ? n.n.slice(0, 20) + "…" : n.n)}</text><text class="s" x="14" y="37">${esc(n.s.length > 24 ? n.s.slice(0, 23) + "…" : n.s)}</text>
      ${n.lang ? `<circle cx="${W - 12}" cy="13" r="3.5" fill="${LANGS[n.lang]}"><title>${n.lang}</title></circle>` : ""}
      ${drill ? `<g transform="translate(${W - 20},${H - 20})"><rect width="14" height="14" rx="4" fill="var(--accent-soft)"/><path d="M4 7h6M7 4v6" stroke="var(--accent-fg)" stroke-width="1.6" stroke-linecap="round"/></g>` : ""}
    </g>`;
  }).join("");
  return `<svg class="gv-svg" viewBox="0 0 920 ${GV.lv === 4 ? 510 : 470}" role="img" aria-label="Graph level ${LV[GV.lv]}">
    <defs>${[["", "gv-mk"], ["c", "gv-mk acc"], ["w", "gv-mk warn"]].map(([s, c]) => `<marker id="gv-a${s}" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path class="${c}" d="M0,1 L9,5 L0,9 z"/></marker>`).join("")}</defs>
    ${edges}${nodes}</svg>`;
}
function relList(list, dir) {
  if (!list.length) return `<div class="mut sm" style="padding:4px 8px">Tidak ada</div>`;
  return list.map(([n, p]) => `<button class="gv-rel" style="width:100%" data-act="gv-sel" data-arg="${n.id}">${ic(dir === "in" ? "arrow-back-up" : "arrow-forward-up", "s14 faint")}<span class="mono grow" style="text-align:left">${esc(n.n)}</span>${conf(p)}</button>`).join("");
}
function gvPanel() {
  const n = nodeOf(GV.lv, GV.sel) || L[GV.lv].nodes[0];
  const lvl = L[GV.lv], by = Object.fromEntries(lvl.nodes.map((x) => [x.id, x]));
  const ins = lvl.edges.filter(([, t]) => t === n.id).map(([f, , p]) => [by[f], p]);
  const outs = lvl.edges.filter(([f]) => f === n.id).map(([, t, p]) => [by[t], p]);
  const imp = impactSet(GV.lv, n.id);
  const drill = GV.lv < 4 && n.id === DRILL[GV.lv];
  const body = `<div class="mono mut sm" style="word-break:break-all">${n.path}</div>
    <div class="row wrap">${drill ? btn("Zoom masuk", "sm pri", "zoom-in", { "data-act": "gv-in" }) : ""}${btn("Lihat kode", "sm", "code", { "data-dialog": "gv-snippet", "data-arg": n.id })}${btn(GV.imp ? "Tutup dampak" : "Analisis dampak", "sm", "target", { "data-act": "gv-imp" })}</div>
    ${GV.imp ? `<div class="alert warn">${ic("target", "s14")}<span><b>${imp.size} node terdampak</b> bila ${esc(n.n)} berubah${imp.size ? ": " + [...imp].map((i) => esc(by[i].n)).join(", ") : ""}.</span></div>` : ""}
    <div><h3>Pemanggil · ${ins.length}</h3>${relList(ins, "in")}</div>
    <div><h3>Dipanggil · ${outs.length}</h3>${relList(outs, "out")}</div>
    <div><h3>Tiket</h3>${n.tk.length ? `<div class="col g4">${n.tk.map((k) => `<div class="row">${tref(k)}<span class="trunc grow sm">${AG.ticket(k).t}</span></div>`).join("")}</div>` : `<div class="mut sm">Belum ada tiket yang menyentuh node ini</div>`}</div>
    <div><h3>Dokumen knowledge</h3>${n.docs.length ? `<div class="col g4">${n.docs.map((d) => `<a class="row link" href="#/p/${K}/knowledge/${d === "Arsitektur Sistem" ? "arsitektur" : "aturan-bisnis"}">${ic("file-text", "s14")}${d}</a>`).join("")}</div>` : `<div class="mut sm">Tidak ada</div>`}</div>
    <div class="mut xs row g4">${ic("info-circle", "s14")}Klik ganda node bertanda + untuk zoom masuk.</div>`;
  return AG.card({ cls: "gv-panel", title: esc(n.n), desc: `${n.kind} · ${LAYERS[n.layer][0]}${n.lang ? " · " + n.lang : ""}`,
    action: `<span class="tile" style="color:${LAYERS[n.layer][1]}">${ic(n.layer === "doc" ? "file-text" : GV.lv === 0 ? "git-branch" : GV.lv === 1 ? "components" : GV.lv === 2 ? "file-code" : "code", "s14")}</span>`, body });
}

AG.screen({ m: "M10", g: "Code graph", n: "Graph viewer", route: "/p/:k/graph", path: "/p/KSR/graph",
  render: () => {
    const tk = GV.tk ? AG.ticket(GV.tk) : null;
    const tkCount = GV.tk ? L[GV.lv].nodes.filter((n) => n.tk.includes(GV.tk)).length : 0;
    const flt = (k, lab, c) => `<button class="gv-flt ${GV.hide.has(k) ? "off" : ""}" data-act="gv-flt" data-arg="${k}" aria-pressed="${!GV.hide.has(k)}"><i style="background:${c}"></i>${lab}</button>`;
    const body = `${gHead()}
      ${gTabs("v")}
      <div class="gv">
      <div class="box gv-main">
        <div class="gv-tool">
          <div class="gv-srch"><label class="inp">${ic("search", "s14 faint")}<input id="gv-q" style="border:0;outline:0;background:none;flex:1;min-width:0" placeholder="Cari simbol atau makna…" aria-label="Cari simbol" autocomplete="off"><kbd>/</kbd></label><div class="gv-res" id="gv-res" role="listbox"></div></div>
          <span class="grow"></span>
          ${btn(tk ? `<span class="mono">${GV.tk}</span>` : "Sorot tiket", "", "ticket", { "data-menu": "gv-ticket" })}
          ${btn("Analisis dampak", GV.imp ? "pri" : "", "target", { "data-act": "gv-imp" })}
        </div>
        <div class="gv-lv">
          <nav class="gv-bc" aria-label="Level zoom">${PATH.slice(0, GV.lv + 1).map((p, i) => `${i ? `<span class="sep">${ic("chevron-right", "s14")}</span>` : ""}<button class="${i === GV.lv ? "on" : ""}" data-act="gv-lv" data-arg="${i}"><small>${LV[i]}</small>${i === 0 ? AG.projDot(K, 8) : ""}<span class="${i ? "mono" : ""}">${p}</span></button>`).join("")}</nav>
          <span class="grow"></span>
          <span class="mut sm hide-sm">${L[GV.lv].nodes.length} node · ${L[GV.lv].edges.length} relasi</span>
        </div>
        <div class="gv-canvas" id="gv-canvas">
          <div class="gv-ov tl">${tk ? `<span class="gv-banner">${ic("ticket", "s14")}<span><b class="mono">${GV.tk}</b> · ${tkCount} node tersentuh · ${TK_NOTE[GV.tk]}</span>${btn("", "ghost icon sm", "x", { "data-act": "gv-tk", "data-arg": "", "aria-label": "Hapus sorotan" })}</span>` : ""}
            ${GV.imp ? `<span class="gv-banner warn">${ic("target", "s14")}<span>Dampak dari <b class="mono">${esc(nodeOf(GV.lv, GV.sel).n)}</b></span>${btn("", "ghost icon sm", "x", { "data-act": "gv-imp", "aria-label": "Tutup analisis dampak" })}</span>` : ""}</div>
          <div class="gv-ov tr"><div class="gv-zoom">
            <button data-act="gv-in" aria-label="Zoom masuk" title="Zoom masuk" ${GV.lv === 4 ? "disabled" : ""}>${ic("zoom-in")}</button>
            <button data-act="gv-out" aria-label="Zoom keluar" title="Zoom keluar" ${GV.lv === 0 ? "disabled" : ""}>${ic("zoom-out")}</button>
            <button data-act="gv-fit" aria-label="Reset tampilan" title="Reset tampilan">${ic("focus-2")}</button></div></div>
          ${graphSvg()}
          <div class="gv-ov bl"><span class="gv-flt-lab">Layer</span>${Object.entries(LAYERS).map(([k, [n, c]]) => flt(k, n, c)).join("")}<span class="gv-flt-lab">Bahasa</span>${["TypeScript", "Go", "SQL"].map((k) => flt(k, k, LANGS[k])).join("")}
            <span class="gv-flt-lab hide-sm">Relasi</span><span class="gi-legend hide-sm"><span><svg width="22" height="6"><path d="M0 3h22" stroke="var(--faint)" stroke-width="1.5"/></svg>pasti</span><span><svg width="22" height="6"><path d="M0 3h22" stroke="var(--faint)" stroke-width="1.5" stroke-dasharray="4 3"/></svg>dugaan</span></span></div>
        </div>
      </div>
      <aside class="gv-side" aria-label="Detail node">${gvPanel()}</aside>
    </div>`;
    return AG.shell({ project: K, crumbs: AG.projectCrumb(K, "Code graph"), body, width: "wide",
      actions: btn("", "ghost icon", "link", { "data-toast": "Tautan graph disalin", "aria-label": "Salin tautan" }) });
  },
  after: (root) => {
    const q = root.querySelector("#gv-q"), res = root.querySelector("#gv-res");
    const draw = () => {
      const v = q.value.trim().toLowerCase();
      if (!v) { res.innerHTML = `<div class="mlab">Coba pencarian makna</div>${["logika perhitungan diskon", "verifikasi callback pembayaran", "cetak ulang struk"].map((s) => `<button class="mi" data-act="gv-q" data-arg="${s}">${ic("sparkles")}${s}</button>`).join("")}<div class="mlab">Contoh nama</div><button class="mi" data-act="gv-q" data-arg="calcPromo">${ic("code")}calcPromo</button>`; return; }
      const byName = SYMS.filter((s) => s.n.toLowerCase().includes(v));
      const sem = /diskon|potong|promo|harga|total/.test(v) ? SYMS.filter((s) => s.sem).sort((a, b) => b.sem - a.sem) : [];
      const row = (s, score) => `<button class="mi" data-act="gv-open" data-arg="${s.lv}:${s.id}">${ic(s.k === "fungsi" ? "code" : s.k === "file" || s.k === "komponen" || s.k === "hook" ? "file-code" : "components")}<span class="grow" style="min-width:0"><span class="mono">${s.n}</span> <span class="faint xs">${s.k}</span><br><span class="mut xs">${s.f}</span></span>${score ? `<span class="sc">${score.toFixed(2)}</span>` : ""}</button>`;
      res.innerHTML = (byName.length ? `<div class="mlab">Nama simbol</div>${byName.slice(0, 5).map((s) => row(s)).join("")}` : "")
        + (sem.length ? `<div class="mlab row">${ic("sparkles", "s14")}Makna (semantic search) · “${esc(q.value)}”</div>${sem.slice(0, 5).map((s) => row(s, s.sem)).join("")}` : "")
        || `<div class="mut" style="padding:12px">Tidak ada simbol yang cocok.</div>`;
    };
    q.addEventListener("focus", () => { draw(); res.classList.add("on"); });
    q.addEventListener("input", () => { draw(); res.classList.add("on"); });
    document.addEventListener("click", function h(e) { if (!root.contains(q)) return document.removeEventListener("click", h); if (!e.target.closest(".gv-srch")) res.classList.remove("on"); });
    root.querySelectorAll(".gv-n[data-dbl]").forEach((g) => g.addEventListener("dblclick", () => { GV.sel = g.dataset.arg; AG.ACTIONS["gv-in"](); }));
    root.querySelectorAll(".gv-n").forEach((g) => g.addEventListener("keydown", (e) => { if (e.key === "Enter") g.dispatchEvent(new MouseEvent("click", { bubbles: true })); }));
    const key = (e) => { if (!root.contains(q)) return document.removeEventListener("keydown", key); if (e.key === "/" && document.activeElement !== q && !/INPUT|TEXTAREA/.test(document.activeElement?.tagName)) { e.preventDefault(); q.focus(); } };
    document.addEventListener("keydown", key);
  } });
})();

/* Bagian B: M8 Knowledge base. */
(() => {
const { ic, av, btn, pill, sw, cb, section, head, tref, esc, srow, fld, sel, inp } = AG;
const K = "KSR";
const SUBS = AG.SUBS_M7;

AG.css(`
/* Beranda knowledge (opsi B) */
.kb-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; display: inline-block; }
.kb-pm { font-family: var(--mono); font-size: 12px; white-space: nowrap; } .kb-pm b { font-weight: 500; }
.kb-top { display: grid; grid-template-columns: minmax(0, 1.75fr) minmax(0, 1fr); gap: 16px; align-items: stretch; }
.kb-bh { display: flex; align-items: center; gap: 8px; padding: 12px 16px; border-bottom: 1px solid var(--line); min-height: 50px; } .kb-bh h3 { font-size: 14px; }
.kb-ar { display: flex; align-items: center; gap: 12px; padding: 11px 16px; border-top: 1px solid var(--line); min-width: 0; } .kb-bh + .kb-ar { border-top: 0; }
.kb-ar > .grow { display: grid; gap: 2px; min-width: 0; } .kb-ar .acts { display: flex; gap: 6px; flex-shrink: 0; }
.kb-health { padding: 16px; display: grid; gap: 16px; align-content: start; }
.kb-donut { display: flex; align-items: center; gap: 18px; } .kb-donut svg { width: 92px; height: 92px; flex-shrink: 0; }
.kb-leg { display: grid; gap: 6px; font-size: 12.5px; flex: 1; } .kb-leg > div { display: flex; align-items: center; gap: 8px; } .kb-leg b { margin-left: auto; font-weight: 500; font-variant-numeric: tabular-nums; }
.kb-hrow { display: flex; align-items: center; gap: 8px; padding-top: 12px; border-top: 1px solid var(--line); font-size: 13px; }
.kb-grp > .cx-h { flex-wrap: wrap; } .kb-grp > .cx-h > .grow { min-width: 220px; } .kb-grp .cx-a { flex: 0 1 280px; } .kb-grp .cx-a .fbar { width: 100%; }
.kb-top > .cardx .kb-ar:first-child { border-top: 0; }
.kb-cards { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
.kb-card { display: flex; flex-direction: column; gap: 12px; padding: 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--raised); min-width: 0; color: inherit; }
.kb-card:hover { border-color: var(--input); box-shadow: var(--shadow-sm); }
.kb-seg { display: flex; height: 6px; border-radius: 3px; overflow: hidden; gap: 2px; background: var(--hover); } .kb-seg i { display: block; height: 100%; }
.kb-cdl { display: grid; gap: 6px; font-size: 12.5px; color: var(--fg2); flex: 1; align-content: start; }
.kb-cf { display: flex; align-items: center; gap: 8px; justify-content: space-between; border-top: 1px solid var(--line); padding-top: 10px; }
.kb-card.add { border-style: dashed; align-items: center; justify-content: center; color: var(--muted); text-align: center; background: transparent; cursor: pointer; min-height: 160px; }
.kb-nores { display: none; } .kb-nores.on { display: block; }
.kb-back { display: inline-flex; align-items: center; gap: 6px; color: var(--muted); font-weight: 500; font-size: 12.5px; width: fit-content; } .kb-back:hover { color: var(--fg); }
.kb-dn { display: grid; gap: 2px; min-width: 0; } .kb-dn b { font-weight: 500; } .kb-dn span { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
/* Tinjau usulan (mode fokus) */
.kb-rvbar { display: flex; align-items: center; gap: 10px; padding: 10px 14px; flex-wrap: wrap; }
.kb-steps { display: flex; gap: 4px; } .kb-steps i { width: 20px; height: 4px; border-radius: 2px; background: var(--line2); } .kb-steps i.on { background: var(--accent); } .kb-steps i.done { background: var(--success); }
.kb-kbd { display: flex; gap: 12px; font-size: 12px; color: var(--muted); flex-wrap: wrap; } .kb-kbd > span { display: inline-flex; gap: 4px; align-items: center; }
.kb-rv { display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: 16px; align-items: start; }
.kb-rvmain { display: grid; gap: 12px; min-width: 0; }
.kb-qh { padding: 10px 14px; font-size: 12px; color: var(--muted); font-weight: 500; display: flex; gap: 6px; align-items: center; border-bottom: 1px solid var(--line); background: var(--muted-bg); }
.kb-qh:not(:first-child) { border-top: 1px solid var(--line); }
.kb-qi { display: grid; grid-template-columns: 8px minmax(0, 1fr) auto; gap: 10px; align-items: start; width: 100%; text-align: left; padding: 10px 14px; border-top: 1px solid var(--line); }
.kb-qh + .kb-qi { border-top: 0; }
.kb-qi:hover { background: var(--muted-bg); } .kb-qi.on { background: var(--accent-soft); box-shadow: inset 2px 0 0 var(--accent); }
.kb-qi .kb-dot { margin-top: 6px; } .kb-qi b { font-weight: 500; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .kb-qi .s { font-size: 12px; color: var(--muted); display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
.kb-qd { display: flex; align-items: center; gap: 8px; padding: 9px 14px; border-top: 1px solid var(--line); font-size: 12.5px; min-width: 0; } .kb-qh + .kb-qd { border-top: 0; }
.kb-meta { padding: 16px; display: grid; gap: 12px; }
.kb-meta h2 { font-size: 17px; letter-spacing: -0.01em; }
.kb-meta h2 a:hover { text-decoration: underline; }
.kb-mf { display: flex; flex-wrap: wrap; gap: 6px 20px; font-size: 12.5px; color: var(--muted); } .kb-mf > span { display: inline-flex; gap: 6px; align-items: center; } .kb-mf b { color: var(--fg); font-weight: 500; }
.kb-sbs { overflow: hidden; border-radius: 0 0 0 0; }
.kb-sbs-h, .kb-sbs-r { display: grid; grid-template-columns: 1fr 1fr; }
.kb-sbs-h > div { padding: 9px 14px; font-size: 12px; color: var(--muted); font-weight: 500; background: var(--muted-bg); border-bottom: 1px solid var(--line); display: flex; gap: 6px; align-items: center; }
.kb-sbs-h > div + div, .kb-sbs-r > div + div { border-left: 1px solid var(--line); }
.kb-sbs-r > div { padding: 5px 14px; font-size: 13px; line-height: 1.6; min-height: 30px; min-width: 0; overflow-wrap: anywhere; }
.kb-sbs-r > .del { background: var(--danger-soft); color: var(--danger); } .kb-sbs-r > .add { background: var(--success-soft); color: var(--fg); }
.kb-sbs-r > .emp { background: repeating-linear-gradient(135deg, transparent 0 6px, var(--hover) 6px 7px); }
.kb-sbs-hd { padding: 6px 14px; font-size: 12px; color: var(--muted); background: var(--muted-bg); border-bottom: 1px solid var(--line); }
.kb-sbs-r + .kb-sbs-hd { border-top: 1px solid var(--line); }
.kb-trow { display: grid; grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.4fr) minmax(0, 2.4fr); gap: 10px; font-size: 12.5px; }
.kb-trow > span { overflow-wrap: anywhere; } .kb-trow > span:first-child { font-family: var(--mono); font-size: 12px; }
.kb-dh { font-weight: 600; color: var(--fg); }
.kb-rvfoot { display: flex; align-items: center; gap: 8px; padding: 12px 16px; flex-wrap: wrap; }
.kb-rvcard .kb-meta { border-bottom: 1px solid var(--line); }
.kb-rvcard .cx-h p { margin-top: 6px; }
.kb-grpl { display: inline-flex; align-items: center; gap: 6px; color: var(--muted); font-weight: 500; font-size: 12.5px; width: fit-content; } .kb-grpl:hover { color: var(--fg); }
.kb-own { display: flex; align-items: center; gap: 10px; padding: 10px 14px; font-size: 12.5px; color: var(--muted); } .kb-own b { color: var(--fg); }
.kb-queue .kb-qh:first-child { border-top: 0; }
.kb-rvfoot .msg { margin-right: auto; color: var(--muted); display: flex; gap: 8px; align-items: center; font-size: 12.5px; }
@container (max-width: 1000px) { .kb-cards { grid-template-columns: repeat(2, minmax(0, 1fr)); } .kb-top { grid-template-columns: 1fr; } .kb-rv { grid-template-columns: 1fr; } .kb-rv > .kb-queue { order: 2; } }
@container (max-width: 560px) { .kb-cards { grid-template-columns: 1fr; } .kb-sbs-h, .kb-sbs-r { grid-template-columns: 1fr; } .kb-sbs-r > .emp, .kb-sbs-h > div + div { display: none; } .kb-sbs-r > div + div { border-left: 0; } .kb-sbs-r > div:not([class]) + div:not([class]) { display: none; }
  .kb-ar { flex-wrap: wrap; } .kb-ar > .grow { flex-basis: calc(100% - 48px); } .kb-ar .acts { width: 100%; justify-content: flex-end; } .kb-ar > .kb-pm { display: none; }
  .kb-grp .cx-a { flex-basis: 100%; } .kb-rvfoot { position: static; box-shadow: none; } .kb-rvfoot .msg { display: none; } .kb-rvfoot > .btn { flex: 1; justify-content: center; } .kb-rvfoot > .btn.pri { flex-basis: 100%; } .kb-kbd { display: none; } }
.kb-depth { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
.kb-depth > button { border: 1px solid var(--line2); border-radius: 9px; padding: 12px; display: grid; gap: 4px; align-content: start; background: var(--raised); }
.kb-depth > button.on { border-color: var(--accent); box-shadow: var(--ring); }
.kb-depth b { font-weight: 600; }
.kb-depth .n { font-family: var(--mono); font-size: 12px; color: var(--fg2); }
@media (max-width: 899px) { .kb-depth { grid-template-columns: 1fr; } }
.kb-ed2 { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 24px; align-items: start; }
.kb-main { display: grid; gap: 14px; min-width: 0; }
.kb-aside { display: grid; gap: 12px; align-content: start; min-width: 0; position: sticky; top: 64px; }
.kb-sc .cx-h { padding: 12px 16px; } .kb-sc .cx-h h2 { font-size: 13px; }
.kb-sc .row > a { flex-shrink: 0; white-space: nowrap; }
.kb-sc .cx-b { gap: 14px; }
.kb-ed2 .ed .foot { flex-wrap: wrap; row-gap: 6px; }
.kb-title { font-size: 26px; font-weight: 600; letter-spacing: -0.02em; outline: none; border-radius: 6px; }
.kb-title:empty::before { content: attr(data-ph); color: var(--faint); }
.kb-title:focus { background: var(--raised); box-shadow: 0 0 0 6px var(--raised); }
.kb-ol { display: block; width: 100%; text-align: left; padding: 5px 8px; border-radius: 6px; color: var(--fg2); font-size: 12.5px; border-left: 2px solid var(--line2); border-radius: 0 6px 6px 0; }
.kb-ol:hover { background: var(--hover); } .kb-ol.on { border-left-color: var(--accent); color: var(--fg); font-weight: 500; }
.kb-tw { overflow-x: auto; margin: 0 0 8px; }
.kb-tw table { margin: 0; min-width: 520px; }
.ed .area .kb-blk, .ed .area .kb-drop { margin: 0 0 12px; font-size: 13px; line-height: 1.5; }
.ed .area .kb-blk .code { white-space: pre; font-size: 11.5px; padding: 12px; overflow-x: auto; }
.ed .area a { color: var(--accent-fg); }
@media (max-width: 1099px) { .kb-ed2 { grid-template-columns: 1fr; } .kb-aside { position: static; } }
@media (max-width: 899px) { .kb-title { font-size: 22px; } }
.kb-men { color: var(--accent-fg); background: var(--accent-soft); border-radius: 4px; padding: 0 4px; font-weight: 500; }
.kb-dl { color: var(--accent-fg); border-bottom: 1px dashed var(--accent); }
.kb-cm { background: color-mix(in srgb, var(--warn) 22%, transparent); border-bottom: 2px solid var(--warn); border-radius: 2px; cursor: pointer; padding: 0 1px; color: inherit; }
.kb-cm.on { background: color-mix(in srgb, var(--warn) 38%, transparent); }
.kb-blk { border: 1px solid var(--line); border-radius: 10px; overflow: hidden; background: var(--raised); }
.kb-blkh { white-space: nowrap; flex-wrap: wrap; display: flex; align-items: center; gap: 8px; padding: 6px 8px 6px 12px; border-bottom: 1px solid var(--line); background: var(--muted-bg); font-size: 12px; color: var(--muted); }
.kb-blk .code { border: 0; border-radius: 0; margin: 0; }
.kb-mm { display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 3fr); }
.kb-mm .code { border-right: 1px solid var(--line); font-size: 11.5px; }
.kb-mm svg { width: 100%; height: auto; display: block; padding: 10px; box-sizing: border-box; }
.kb-mm.prev .code { display: none; } .kb-mm.prev { grid-template-columns: 1fr; }
.kb-mm.src svg { display: none; } .kb-mm.src { grid-template-columns: 1fr; } .kb-mm.src .code { border-right: 0; }
.kb-mm .nd rect { fill: var(--raised); stroke: var(--line2); }
.kb-mm .nd.a rect { fill: var(--accent-soft); stroke: var(--accent); }
.kb-mm .nd text { fill: var(--fg); font-size: 13px; font-weight: 500; }
.kb-mm .nd .s { fill: var(--muted); font-size: 10.5px; font-weight: 400; }
.kb-mm .ln { stroke: var(--faint); fill: none; stroke-width: 1.3; }
.kb-mm .ln.d { stroke-dasharray: 4 3; }
.kb-mm .lb { fill: var(--muted); font-size: 10px; }
@media (max-width: 899px) { .kb-mm { grid-template-columns: 1fr; } .kb-mm .code { border-right: 0; border-bottom: 1px solid var(--line); } }
.kb-drop { border: 1px dashed var(--input); border-radius: 10px; padding: 18px; display: flex; align-items: center; gap: 12px; color: var(--muted); }
.kb-thr { border: 1px solid var(--line2); background: var(--raised); border-radius: 9px; padding: 10px 12px; display: grid; gap: 8px; font-size: 12.5px; }
.kb-thr.on { border-color: var(--warn); box-shadow: 0 0 0 3px var(--warn-soft); }
.kb-thr q { quotes: none; color: var(--muted); border-left: 2px solid var(--warn); padding-left: 8px; font-size: 12px; }
.kb-thr .msg { display: grid; grid-template-columns: 20px 1fr; gap: 8px; }
.kb-ver { display: grid; grid-template-columns: 8px minmax(0, 1fr); gap: 10px; padding: 6px 4px; border-radius: 6px; }
.kb-ver:hover { background: var(--hover); }
.kb-ver i { width: 8px; height: 8px; border-radius: 50%; background: var(--input); margin-top: 5px; }
.kb-ver.cur i { background: var(--accent); }
.kb-hl-k { color: var(--accent-fg); } .kb-hl-s { color: var(--success); } .kb-hl-c { color: var(--faint); } .kb-hl-f { color: var(--info); }
`);

/* ---------- Data katalog ---------- */
const DOCS = [
  ["Gambaran umum", "layout-dashboard", [["overview", "Overview Project", "ok", "AA", "10 Nov"], ["peta-repo", "Peta Repo", "ok", "AA", "10 Nov"], ["tech-stack", "Tech Stack & Dependencies", "stale", "AA", "21 Okt", "xendit-go naik ke v4 sejak 3 Nov"], ["glosarium", "Glosarium", "ok", "OO", "2 Nov"]]],
  ["Arsitektur", "hierarchy-2", [["arsitektur", "Arsitektur Sistem", "ok", "AA", "10 Nov"], ["alur-data", "Alur Data", "ok", "AA", "8 Nov"], ["integrasi", "Integrasi Pihak Ketiga", "prop", "DD", "6 Nov"], ["adr", "ADR · 6 keputusan", "ok", "AA", "4 Nov"]]],
  ["Domain & bisnis", "building-store", [["alur-bisnis", "Alur Bisnis / User Journey", "ok", "OO", "9 Nov"], ["aturan-bisnis", "Aturan Bisnis", "ok", "OO", "11 Nov"], ["siklus", "Siklus Status Transaksi", "ok", "OO", "5 Nov"], ["role", "Role & Permission aplikasi", "ok", "AA", "1 Nov"]]],
  ["Backend & kode", "server", [["modul-payment", "Modul: payment", "ok", "DD", "10 Nov"], ["modul-receipt", "Modul: receipt", "stale", "QQ", "24 Okt", "render.go berubah 6 kali sejak dokumen ditulis"], ["model-data", "Model Data & Database", "ok", "AA", "7 Nov"], ["migration", "Riwayat Migration", "ok", "AA", "7 Nov"], ["api", "API", "prop", "DD", "6 Nov"], ["job", "Event, Queue & Cron", "ok", "DD", "3 Nov"], ["error", "Error Handling & Logging", "ok", "DD", "30 Okt"]]],
  ["Frontend", "layout-sidebar", [["routing", "Halaman & Routing", "ok", "DD", "9 Nov"], ["komponen-ui", "Komponen UI & Design System", "ok", "OO", "8 Nov"], ["state", "State Management", "ok", "DD", "8 Nov"]]],
  ["Kualitas", "shield-check", [["testing", "Strategi Testing", "ok", "QQ", "6 Nov"], ["konvensi", "Konvensi & Coding Style", "ok", "AA", "1 Nov"], ["security", "Security", "ok", "AA", "2 Nov"], ["debt", "Technical Debt", "stale", "AA", "15 Okt", "4 TODO baru dan 2 dead code terdeteksi"]]],
  ["Operasional", "server", [["setup", "Setup & Cara Menjalankan", "ok", "DD", "28 Okt"], ["env", "Konfigurasi & Env Var", "prop", "AA", "28 Okt"], ["cicd", "Build & CI/CD", "ok", "AA", "5 Nov"], ["deploy", "Deployment & Infrastruktur", "ok", "AA", "5 Nov"], ["runbook", "Runbook", "ok", "AA", "30 Okt"]]],
  ["Riwayat", "history", [["changelog", "Changelog per rilis", "ok", "OO", "11 Nov"], ["peta-fitur", "Peta Fitur", "ok", "OO", "11 Nov"]]],
];
const ALLDOCS = DOCS.flatMap(([g, , list]) => list.map((d) => ({ g, id: d[0], n: d[1], st: d[2], own: d[3], up: d[4], why: d[5] })));
const docOf = (id) => ALLDOCS.find((d) => d.id === id) || ALLDOCS.find((d) => d.id === "arsitektur");
const ST = { ok: ["Terbaru", "ok", "circle-check"], stale: ["Basi", "warn", "clock"], prop: ["Ada usulan", "acc", "git-pull-request"] };
const KB = { owner: "AA", sel: null };
/* Kelompok dokumen: slug untuk rute /p/:k/knowledge/g/:group */
const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
const GROUPS = DOCS.map(([g, i, list]) => ({ g, i, s: slug(g), ids: list.map((d) => d[0]) }));
const grpOf = (x) => GROUPS.find((G) => G.s === x || G.g === x) || GROUPS[0];
const GDESC = { "Gambaran umum": "Ringkasan untuk anggota baru dan agent AI.", Arsitektur: "Keputusan dan struktur sistem yang jarang berubah.", "Domain & bisnis": "Aturan toko yang harus dipatuhi kode.", "Backend & kode": "Struktur kode di kasir-api.", Frontend: "Struktur aplikasi kasir di tablet.", Kualitas: "Standar kualitas yang dicek di setiap PR.", Operasional: "Cara menjalankan dan merawat sistem.", Riwayat: "Catatan yang dihasilkan dari tiket Done." };
const kbHome = `#/p/${K}/knowledge`, kbGroup = (g) => `#/p/${K}/knowledge/g/${grpOf(g).s}`, kbDoc = (id) => `#/p/${K}/knowledge/${id}`, kbReview = `#/p/${K}/knowledge/proposals`;

/* Usulan perubahan dari AI */
const PROPS = [
  { id: "p1", doc: "api", trig: "done", tl: "KSR-22 selesai", tk: "KSR-22", w: "2 jam lalu", tok: "18rb token", st: "pending",
    why: "Endpoint login PIN dari KSR-22 belum tercatat. Tabel endpoint autentikasi diperbarui dari diff graph.",
    diff: [["hd", "@@ api.md · Autentikasi @@"], ["", "| Method | Path | Keterangan |"], ["", "| POST | /v1/auth/login | Login email + password admin toko |"], ["del", "| POST | /v1/auth/kasir | Login kasir dengan nama pengguna |"], ["add", "| POST | /v1/auth/pin | Login kasir dengan PIN 6 digit (KSR-22) |"], ["add", "| POST | /v1/auth/pin/reset | Reset PIN oleh supervisor |"], ["", ""], ["add", "PIN di-hash dengan argon2id. Lima kali salah mengunci akun kasir 15 menit."]] },
  { id: "p2", doc: "integrasi", trig: "merge", tl: "Merge PR #139 ke main", w: "5 jam lalu", tok: "26rb token", st: "pending",
    why: "PR #139 mengganti verifikasi callback Xendit ke HMAC-SHA256 dan menambah retry. Bagian QRIS diselaraskan.",
    diff: [["hd", "@@ integrasi-pihak-ketiga.md · Xendit QRIS @@"], ["", "## Callback pembayaran"], ["del", "Callback dicek dengan token statis `X-CALLBACK-TOKEN`."], ["add", "Callback diverifikasi dengan signature HMAC-SHA256 (header `X-Signature`)"], ["add", "memakai `QRIS_CALLBACK_SECRET`. Callback ganda diabaikan lewat idempotency key."], ["", ""], ["add", "Bila Xendit tidak merespons, pembuatan QR diulang 3 kali (backoff 1s, 2s, 4s)."]] },
  { id: "p3", doc: "env", trig: "audit", tl: "Audit mingguan · 10 Nov", w: "kemarin", tok: "41rb token", st: "pending",
    why: "Audit menemukan env var baru yang dipakai kode tetapi belum terdokumentasi. Dokumen ditandai basi sampai usulan disetujui.",
    diff: [["hd", "@@ konfigurasi-env.md · Pembayaran @@"], ["", "| Nama | Wajib | Keterangan |"], ["", "| XENDIT_API_KEY | ya | Kunci API Xendit (simpan di secret manager) |"], ["add", "| QRIS_CALLBACK_SECRET | ya | Secret verifikasi signature callback |"], ["add", "| QRIS_EXPIRY_SECONDS | tidak | Masa berlaku QR, default 300 |"], ["del", "| XENDIT_CALLBACK_TOKEN | ya | Token callback lama |"]] },
  { id: "p4", doc: "aturan-bisnis", trig: "done", tl: "KSR-21 selesai", tk: "KSR-21", w: "11 Nov", tok: "9rb token", st: "approved", by: "OO",
    why: "Aturan pembulatan total belanja diperjelas setelah perbaikan selisih Rp1.",
    diff: [["hd", "@@ aturan-bisnis.md · Pembulatan @@"], ["del", "Total dibulatkan ke ratusan terdekat."], ["add", "Total dibulatkan ke bawah ke kelipatan Rp100 setelah diskon dan pajak, sekali saja di akhir."]] },
];
const TRIG = { done: ["Tiket Done", "circle-check"], merge: ["Merge", "git-merge"], audit: ["Audit mingguan", "calendar-stats"], manual: ["Manual", "hand-stop"] };
AG.KNOWLEDGE = { DOCS, PROPS };

/* ---------- Aksi, menu, dialog ---------- */
const pending = () => PROPS.filter((p) => p.st === "pending");
const pendOf = (id) => PROPS.filter((p) => p.doc === id && p.st === "pending");
/* Usulan yang sedang ditinjau: KB.sel bila masih menunggu, kalau tidak yang pertama. */
const curProp = () => { const l = pending(); return l.find((p) => p.id === KB.sel) || l[0] || null; };
function settle(p, st) {
  const list = pending(), i = list.indexOf(p);
  p.st = st; p.by = AG.ME;
  const d = docOf(p.doc);
  if (!pendOf(d.id).length) d.st = st === "approved" ? "ok" : p.trig === "audit" || d.why ? "stale" : "ok";
  if (st === "approved") d.up = "baru saja";
  const next = list[i + 1] || list[i - 1]; KB.sel = next && next !== p ? next.id : null;
}
const STALE_DIFF = {
  "tech-stack": [["hd", "@@ tech-stack.md · Dependencies @@"], ["", "| Paket | Versi | Dipakai untuk |"], ["del", "| xendit-go | v3.2 | Pembayaran QRIS |"], ["add", "| xendit-go | v4.1 | Pembayaran QRIS (signature HMAC) |"]],
  "modul-receipt": [["hd", "@@ modul-receipt.md · Render struk @@"], ["del", "Struk dirender sebagai teks 32 kolom."], ["add", "Struk dirender 32 kolom (printer 58 mm) atau 48 kolom (80 mm) sesuai printer toko."], ["add", "Nama kasir diambil dari sesi login PIN."]],
  debt: [["hd", "@@ technical-debt.md · Daftar @@"], ["", "- Scanner barcode membaca dua kali (KSR-37)"], ["add", "- 4 TODO baru di internal/receipt dan internal/stock"], ["add", "- 2 fungsi tidak terpakai: legacyRound(), oldPrinterInit()"]],
};
AG.action("kb-review", (el, id) => { if (id) KB.sel = id; if (location.hash === kbReview) AG.rerender(); else location.hash = kbReview; });
AG.action("kb-sel", (el, id) => { KB.sel = id; AG.rerender(); });
AG.action("kb-nav", (el, d) => { const l = pending(), c = curProp(); if (!c) return; const n = l[Math.max(0, Math.min(l.length - 1, l.indexOf(c) + +d))]; KB.sel = n.id; AG.rerender(); });
AG.action("kb-approve", (el, arg) => { const p = PROPS.find((x) => x.id === arg); if (!p || p.st !== "pending") return; settle(p, "approved"); AG.rerender(); AG.toast(`Usulan untuk “${docOf(p.doc).n}” disetujui dan di-commit ke .agentgate/knowledge/`); });
AG.action("kb-reject-ok", (el, arg) => { const p = PROPS.find((x) => x.id === arg); settle(p, "rejected"); AG.closeDialog(); AG.rerender(); AG.toast("Usulan ditolak. Alasan dikirim ke Agent Tech Writer."); });
AG.action("kb-undo", (el, arg) => { const p = PROPS.find((x) => x.id === arg); p.st = "pending"; docOf(p.doc).st = "prop"; KB.sel = p.id; AG.rerender(); AG.toast("Usulan dikembalikan ke antrean"); });
AG.action("kb-stale-ok", (el, id) => { const d = docOf(id); d.st = "ok"; d.up = "baru saja"; AG.rerender(); AG.toast(`“${d.n}” ditandai masih benar · dicek lagi di audit 17 Nov`); });
AG.action("kb-stale-ai", (el, id) => {
  const d = docOf(id), pid = "s-" + id;
  if (!PROPS.find((x) => x.id === pid && x.st === "pending")) {
    const i = PROPS.findIndex((x) => x.id === pid); if (i >= 0) PROPS.splice(i, 1);
    PROPS.unshift({ id: pid, doc: id, trig: "manual", tl: "Diminta manual", w: "baru saja", tok: "14rb token", st: "pending", why: `${d.why}. Diperbarui atas permintaan ${AG.PEOPLE[AG.ME].name}.`, diff: STALE_DIFF[id] || [["hd", `@@ ${id}.md @@`], ["add", "Bagian diperbarui mengikuti kode terbaru."]] });
    d.st = "prop";
  }
  AG.rerender(); AG.toast("Agent Tech Writer menyusun usulan · masuk ke antrean tinjau");
});
AG.action("kb-owner", (el, arg) => { KB.owner = arg; AG.rerender(); AG.toast(`${AG.PEOPLE[arg].name} sekarang pemilik knowledge Kasir Online`); });
AG.action("kb-cm", (el, arg) => { document.querySelectorAll(".kb-cm,.kb-thr").forEach((x) => x.classList.toggle("on", x.dataset.arg === arg || x.id === "thr-" + arg)); document.getElementById("thr-" + arg)?.scrollIntoView({ block: "nearest", behavior: "smooth" }); });
AG.action("kb-mm", (el, arg) => { const w = el.closest(".kb-blk").querySelector(".kb-mm"); w.classList.remove("prev", "src"); if (arg) w.classList.add(arg); });
AG.menu("kb-owner", () => `<div class="mlab">Pemilik knowledge menyetujui usulan AI</div>${["AA", "OO", "DD", "QQ"].map((k) => `<button class="mi ${KB.owner === k ? "hov" : ""}" data-act="kb-owner" data-arg="${k}">${av(k, 18)}${AG.PEOPLE[k].name}${KB.owner === k ? `<span class="tail">${ic("check", "s14")}</span>` : ""}</button>`).join("")}`);
AG.menu("kb-doc", (arg) => `<div class="mlab">${esc(docOf(arg).n)}</div>
  <a class="mi" href="#/p/${K}/knowledge/${arg}">${ic("pencil")}Buka editor</a>
  <button class="mi" data-dialog="kb-scan" data-arg="${arg}">${ic("refresh")}Scan ulang dokumen ini</button>
  <button class="mi" data-dialog="kb-history">${ic("history")}Riwayat versi</button>
  <button class="mi" data-act="gv-open" data-arg="0:doc">${ic("topology-star-3")}Lihat di graph</button>
  <div class="msep"></div><button class="mi dng" data-toast="Dokumen diarsipkan">${ic("archive")}Arsipkan</button>`);
AG.menu("kb-more", () => `<button class="mi" data-dialog="kb-history">${ic("history")}Riwayat versi</button>
  <button class="mi" data-dialog="kb-scan" data-arg="arsitektur">${ic("refresh")}Minta AI memperbarui</button>
  <button class="mi" data-toast="Markdown diunduh">${ic("download")}Unduh .md</button>
  <button class="mi" data-toast="Tautan disalin">${ic("link")}Salin tautan</button>`);

const DEPTH = { cepat: ["Cepat", "Struktur, tech stack, dan peta repo", 120, 9, 4], standar: ["Standar", "Semua dokumen relevan, detail sedang", 410, 31, 12], mendalam: ["Mendalam", "Detail per modul, contoh kode, diagram", 1300, 98, 35] };
AG.dialog("kb-scan", (arg) => {
  const one = arg && arg !== "all" ? docOf(arg) : null;
  return { w: "w640", icon: "sparkles", tone: "acc", title: one ? `Scan ulang “${one.n}”` : "Scan knowledge", desc: "AI membaca code graph dan kode, lalu menulis dokumen. Hasil scan masuk sebagai usulan yang harus disetujui pemilik knowledge.",
    body: `${fld("Kedalaman", `<div class="kb-depth">${Object.entries(DEPTH).map(([k, [n, d, tok, rp, mnt]]) => `<button data-radio="kb-depth" data-k="${k}" class="${k === "standar" ? "on" : ""}"><b>${n}</b><span class="mut sm">${d}</span><span class="n">~${tok >= 1000 ? (tok / 1000).toFixed(1).replace(".", ",") + " jt" : tok + " rb"} token</span><span class="mut xs">± Rp${rp}.000 · ${mnt} menit</span></button>`).join("")}</div>`)}
      ${fld("Cakupan", `<div class="row wrap"><div class="seg"><button class="${one ? "" : "on"}">Project</button><button>Repo</button><button class="${one ? "on" : ""}">Folder</button></div><span class="grow" style="min-width:180px">${inp(one ? "kasir-api/internal/receipt" : "", { ph: "Semua repo: kasir-api, kasir-web", cls: "mono" })}</span></div>`, "Scan ulang bisa dipakai untuk menaikkan kedalaman satu repo atau folder saja.")}
      ${fld("Runner yang dipakai", sel([["r1", "MacBook Olivia · Claude Code · online"], ["r2", "Laptop Dewi · Claude Code · sibuk (1 job)"], ["r3", "PC Qori · Codex CLI · sibuk"], ["r4", "ThinkPad Adi · Gemini CLI · offline"]], "r1"), "Biaya scan memakai token AI milik orang yang menjalankannya.")}
      <div class="box soft pad row g12" id="kb-est"><span class="tile acc">${ic("coins", "s14")}</span><span class="grow"><b class="b5">Estimasi: ~410 rb token · ± Rp31.000</b><br><span class="mut sm">33 dokumen relevan · 1.284 file · memakai code graph (hemat ±62% dibanding membaca file utuh)</span></span></div>`,
    foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Mulai scan", "pri", "player-play", { "data-close": true, "data-toast": "Scan dijalankan di MacBook Olivia · hasil masuk ke Usulan" })}`,
    after: (el) => el.querySelectorAll("[data-k]").forEach((b) => b.addEventListener("click", () => { const [, , tok, rp] = DEPTH[b.dataset.k]; el.querySelector("#kb-est b").textContent = `Estimasi: ~${tok >= 1000 ? (tok / 1000).toFixed(1).replace(".", ",") + " jt" : tok + " rb"} token · ± Rp${rp}.000`; })) };
});
const VERS = [["9f3c2a1", "AA", "Setujui usulan: diagram komponen + printer-bridge", "10 Nov 16.02", "Agent Tech Writer · dari merge PR #139"], ["4b81d0e", "DD", "Tambah alur callback QRIS", "6 Nov 10.15", ""], ["c27e9f4", "AA", "Scan standar: versi awal", "3 Nov 09.40", "Scan awal · Agent Tech Writer"], ["1a0d3b7", "AA", "Buat dokumen", "3 Nov 09.12", ""]];
AG.dialog("kb-history", () => ({ w: "w640", icon: "history", title: "Riwayat versi", desc: "Setiap simpan menjadi commit git di <code>.agentgate/knowledge/</code>.",
  body: `<div class="box">${VERS.map(([h, w, m, d, s], i) => `<div class="lrow">${av(w, 24)}<span class="grow" style="min-width:0"><b class="b5">${m}</b><br><span class="mut sm"><span class="mono">${h}</span> · ${AG.PEOPLE[w].name} · ${d}${s ? " · " + s : ""}</span></span>${i === 0 ? pill("Saat ini", "acc") : btn("Lihat diff", "sm", "", { "data-dialog": "kb-ver", "data-arg": h })}</div>`).join("")}</div>`,
  foot: btn("Tutup", "pri", "", { "data-close": true }) }));
AG.dialog("kb-ver", (arg) => ({ w: "w760", icon: "git-commit", title: `Versi ${arg}`, desc: "Perbandingan dengan versi saat ini · arsitektur-sistem.md",
  body: `<div class="diff"><div class="hd">@@ Komponen utama @@</div><div>flowchart LR</div><div>  web[kasir-web] --&gt;|HTTP| api[kasir-api]</div><div class="add">  web --&gt; prn[printer-bridge]</div><div class="add">  api -. event struk.cetak .-&gt; prn</div><div>  api --&gt; db[(PostgreSQL)]</div><div class="del">  xnd[Xendit] --&gt; api</div><div class="add">  xnd[Xendit QRIS] -. callback HMAC .-&gt; api</div></div>`,
  foot: `${btn("Pulihkan versi ini", "", "arrow-back-up", { "data-close": true, "data-toast": `Versi ${arg} dipulihkan sebagai commit baru` })}${btn("Tutup", "pri", "", { "data-close": true })}` }));
AG.dialog("kb-reject", (arg) => { const p = PROPS.find((x) => x.id === arg); return { icon: "x", tone: "dng", title: "Tolak usulan", desc: `Usulan untuk “${docOf(p.doc).n}” tidak akan menimpa dokumen.`,
  body: fld("Alasan untuk AI", inp("", { area: true, ph: "Mis. endpoint /pin/reset belum dirilis, jangan dicatat dulu" }), "Alasan dipakai Agent Tech Writer agar usulan berikutnya lebih tepat."),
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Tolak usulan", "dngs", "", { "data-act": "kb-reject-ok", "data-arg": arg })}` }; });
AG.dialog("kb-new", () => ({ w: "w560", icon: "file-text", title: "Dokumen baru", desc: "Buat manual atau minta AI menyusun dari code graph.",
  body: `${fld("Jenis dokumen", sel([["c", "Prosedur tutup kasir (custom)"], ["runbook", "Runbook"], ["adr", "ADR"], ["modul", "Modul/Service"], ["kosong", "Dokumen kosong"]], "c"))}${fld("Judul", inp("SOP tutup kasir harian"))}
    <div class="box">${srow("Minta AI menyusun draf", "Draf masuk sebagai usulan · ±18 rb token dari runner Anda", sw(true, "Minta AI menyusun draf"))}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Buat dokumen", "pri", "", { "data-close": true, "data-toast": "Dokumen dibuat · AI menyusun draf" })}` }));

/* Tab garis bawah knowledge: Beranda · Tinjau usulan */
const tabsKb = (cur) => { const n = pending().length; return `<nav class="tabs" aria-label="Knowledge"><a href="${kbHome}" class="${cur === "h" ? "on" : ""}">${ic("home", "s14")}Beranda</a><a href="${kbReview}" class="${cur === "p" ? "on" : ""}">${ic("git-pull-request", "s14")}Tinjau usulan${n ? `<span class="n">${n}</span>` : ""}</a></nav>`; };
const kbCrumbs = (...rest) => [AG.projectCrumb(K)[0], ...(rest.length ? [`<a href="${kbHome}">Knowledge</a>`, ...rest] : ["Knowledge"])];
const stPill = (st) => pill(ST[st][0], ST[st][1], "", ST[st][2]);
const STC = { ok: "var(--success)", stale: "var(--warn)", prop: "var(--accent)" };
const stDot = (st) => `<span class="kb-dot" style="background:${STC[st]}" title="${ST[st][0]}"></span>`;
const pmOf = (p) => `<span class="kb-pm"><b class="text-ok">+${p.diff.filter((x) => x[0] === "add").length}</b> <b class="text-dng">−${p.diff.filter((x) => x[0] === "del").length}</b></span>`;
const trigTxt = (p) => `${TRIG[p.trig][0]}${p.tk ? ` (${p.tk})` : p.trig === "merge" ? " · PR #139" : ""}`;
const whyOf = (d) => d.why || "Ditandai basi oleh audit mingguan";
const donut = (parts, total, center) => { const r = 36, C = 2 * Math.PI * r; let off = 0;
  return `<svg viewBox="0 0 92 92" role="img" aria-label="Kesehatan dokumen ${center} terbaru"><circle cx="46" cy="46" r="${r}" fill="none" stroke="var(--hover)" stroke-width="10"/>${parts.map(([v, c]) => { const len = (v / total) * C; const s = v ? `<circle cx="46" cy="46" r="${r}" fill="none" stroke="${c}" stroke-width="10" stroke-dasharray="${Math.max(0, len - 2)} ${C}" stroke-dashoffset="${-off}" transform="rotate(-90 46 46)"/>` : ""; off += len; return s; }).join("")}<text x="46" y="51" text-anchor="middle" style="font-size:17px;font-weight:600;fill:var(--fg)">${center}</text></svg>`; };

/* ====================== Beranda knowledge ====================== */
AG.screen({ m: "M8", g: "Knowledge", n: "Beranda knowledge", route: "/p/:k/knowledge", path: "/p/KSR/knowledge",
  render: () => {
    const pend = pending(), stale = ALLDOCS.filter((d) => d.st === "stale"), n = (st) => ALLDOCS.filter((d) => d.st === st).length;
    const rows = [
      ...pend.map((p) => { const d = docOf(p.doc); return `<div class="kb-ar"><span class="tile acc">${ic("git-pull-request", "s14")}</span><div class="grow"><div class="row wrap g8"><a class="b5" href="${kbDoc(d.id)}">${d.n}</a><span class="mut sm">usulan · ${trigTxt(p)}</span></div><div class="mut sm trunc">${p.why}</div></div>${pmOf(p)}<span class="acts">${btn("Tinjau", "sm", "", { "data-act": "kb-review", "data-arg": p.id })}</span></div>`; }),
      ...stale.map((d) => `<div class="kb-ar"><span class="tile warn">${ic("clock", "s14")}</span><div class="grow"><div class="row wrap g8"><a class="b5" href="${kbDoc(d.id)}">${d.n}</a><span class="mut sm">basi sejak ${d.up}</span></div><div class="mut sm trunc">${whyOf(d)}</div></div><span class="acts">${btn("Masih benar", "sm ghost", "", { "data-act": "kb-stale-ok", "data-arg": d.id })}${btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "kb-stale-ai", "data-arg": d.id })}</span></div>`),
    ];
    const cards = GROUPS.map((G) => { const ds = G.ids.map(docOf), c = (st) => ds.filter((d) => d.st === st).length, own = [...new Set(ds.map((d) => d.own))];
      return `<a class="kb-card" href="${kbGroup(G.s)}" data-kbn="${esc((G.g + " " + ds.map((d) => d.n).join(" ")).toLowerCase())}"><span class="row"><span class="tile">${ic(G.i, "s14")}</span><b class="grow b6 trunc">${G.g}</b><span class="mut sm">${ds.length}</span></span>
        <span class="kb-seg" title="${c("ok")} terbaru, ${c("prop")} ada usulan, ${c("stale")} basi">${["ok", "prop", "stale"].map((st) => (c(st) ? `<i style="flex:${c(st)};background:${STC[st]}"></i>` : "")).join("")}</span>
        <span class="kb-cdl">${ds.slice(0, 4).map((d) => `<span class="row g8">${stDot(d.st)}<span class="trunc">${d.n}</span></span>`).join("")}${ds.length > 4 ? `<span class="mut sm">+${ds.length - 4} dokumen lainnya</span>` : ""}</span>
        <span class="kb-cf"><span class="sm ${c("stale") ? "text-warn" : c("prop") ? "text-acc" : "mut"}">${[c("stale") ? `${c("stale")} basi` : "", c("prop") ? `${c("prop")} usulan` : ""].filter(Boolean).join(" · ") || "Semua terbaru"}</span><span class="stack">${own.slice(0, 3).map((k) => av(k, 20)).join("")}</span></span></a>`; }).join("");
    const body = `
      ${head("Knowledge", `Living document Kasir Online hasil scan project. Sumber kebenaran di <code>.agentgate/knowledge/</code> (repo kasir-api), dengan mirror di server untuk pencarian.`,
        `${btn("Dokumen baru", "", "plus", { "data-dialog": "kb-new" })}${btn("Scan ulang", "pri", "refresh", { "data-dialog": "kb-scan" })}`)}
      ${tabsKb("h")}
      ${AG.stats([
        { label: "Dokumen", value: ALLDOCS.length, icon: "file-text", hint: "8 dari 10 kelompok relevan" },
        { label: "Terbaru", value: n("ok"), icon: "circle-check", tone: "ok", hint: "sinkron dengan kode" },
        { label: "Usulan", value: pend.length, icon: "git-pull-request", tone: "acc", hint: "menunggu persetujuan", go: kbReview },
        { label: "Basi", value: stale.length, icon: "clock", tone: "warn", hint: "dari audit mingguan" },
      ])}
      <div class="kb-top">
        ${AG.card({ title: `Perlu tindakan <span class="pill gray" style="margin-left:4px">${rows.length}</span>`, desc: "Usulan AI yang menunggu persetujuan dan dokumen basi.", pad: false,
          action: pend.length ? btn(`Tinjau ${pend.length} usulan`, "sm pri", "player-play", { "data-act": "kb-review", "data-arg": pend[0].id }) : "",
          body: rows.join("") || `<div style="padding:16px">${AG.empty("circle-check", "Tidak ada yang perlu ditindak", "Semua usulan sudah ditinjau dan tidak ada dokumen basi.")}</div>` })}
        ${AG.card({ title: "Kesehatan dokumen", desc: "Status semua dokumen project.", pad: false, body: `<div class="kb-health">
          <div class="kb-donut">${donut([[n("ok"), STC.ok], [n("prop"), STC.prop], [n("stale"), STC.stale]], ALLDOCS.length, Math.round((n("ok") / ALLDOCS.length) * 100) + "%")}
            <div class="kb-leg">${[["ok", "Terbaru"], ["prop", "Ada usulan"], ["stale", "Basi"]].map(([k, l]) => `<div>${stDot(k)}${l}<b>${n(k)}</b></div>`).join("")}</div></div>
          <div class="col g4"><span class="row g4 sm">${ic("loader-2", "s14 text-acc")}<span class="grow">Scan ulang <b class="b5">Mendalam</b> · Laptop Dewi</span><span class="mut tab-num">64%</span></span><span class="bar"><i style="width:64%"></i></span><span class="mono xs mut">kasir-api/internal/receipt</span></div>
          <div class="kb-hrow">${av(KB.owner, 20)}<span class="grow" style="min-width:0">Pemilik knowledge<br><b class="b5">${AG.PEOPLE[KB.owner].name}</b></span>${btn("Ganti", "sm", "", { "data-menu": "kb-owner" })}</div>
        </div>` })}
      </div>
      ${AG.card({ title: "Kelompok dokumen", desc: "Buka kartu untuk melihat semua dokumen di kelompok itu.", cls: "kb-grp", action: AG.filterBar({ id: "kb-home", search: "Cari dokumen…" }),
        body: `<div class="alert gray">${ic("info-circle", "s14")}<span>Kelompok Mobile dan Data tidak dibuat karena tidak relevan untuk Kasir Online. Jenis dokumen custom diatur di <a class="link" href="#/settings/knowledge">Knowledge workspace</a>. Scan terakhir: Standar · 3 Nov · Adi · 398 rb token.</span></div>
          <div class="kb-cards">${cards}<button class="kb-card add" data-dialog="kb-new">${ic("plus", "s18")}<b class="b5">Dokumen atau jenis custom</b><span class="sm">Mis. SOP tutup kasir harian</span></button></div>
          <div class="kb-nores">${AG.empty("search", "Tidak ada dokumen yang cocok", "Ubah kata kunci pencarian.")}</div>` })}`;
    return AG.shell({ project: K, crumbs: kbCrumbs(), ...AG.projectTabs(K), body, width: "mid" });
  },
  after: (root) => {
    const apply = (q) => {
      let any = false;
      root.querySelectorAll(".kb-card[data-kbn]").forEach((c) => { const ok = !q || c.dataset.kbn.includes(q); c.style.display = ok ? "" : "none"; any ||= ok; });
      root.querySelector(".kb-card.add").style.display = q ? "none" : "";
      root.querySelector(".kb-nores").classList.toggle("on", !any);
    };
    apply(AG.filterQuery("kb-home").trim());
    root.addEventListener("filter-search", (e) => { if (e.detail.id === "kb-home") apply(e.detail.q.toLowerCase().trim()); });
  } });

/* ====================== Tinjau usulan (mode fokus) ====================== */
const inl = (t) => esc(t).replace(/`([^`]+)`/g, "<code>$1</code>").replace(/\*\*(.+?)\*\*/g, "<b>$1</b>");
const dl = (t) => (t.startsWith("|") ? `<span class="kb-trow">${t.split("|").slice(1, -1).map((c) => `<span>${inl(c.trim())}</span>`).join("")}</span>` : t.startsWith("## ") ? `<b class="kb-dh">${inl(t.slice(3))}</b>` : inl(t) || "&nbsp;");
function sbsRows(diff) {
  const rows = []; let i = 0;
  while (i < diff.length) {
    const [c, t] = diff[i];
    if (c === "hd") { rows.push(`<div class="kb-sbs-hd">${esc(t.replace(/@@/g, "").trim())}</div>`); i++; continue; }
    if (c === "") { rows.push(`<div class="kb-sbs-r"><div>${dl(t)}</div><div>${dl(t)}</div></div>`); i++; continue; }
    const dels = [], adds = [];
    while (i < diff.length && diff[i][0] === "del") dels.push(diff[i++][1]);
    while (i < diff.length && diff[i][0] === "add") adds.push(diff[i++][1]);
    for (let j = 0; j < Math.max(dels.length, adds.length); j++) rows.push(`<div class="kb-sbs-r">${dels[j] != null ? `<div class="del">${dl(dels[j])}</div>` : `<div class="emp"></div>`}${adds[j] != null ? `<div class="add">${dl(adds[j])}</div>` : `<div class="emp"></div>`}</div>`);
  }
  return rows.join("");
}
AG.screen({ m: "M8", g: "Knowledge", n: "Tinjau usulan knowledge", route: "/p/:k/knowledge/proposals", path: "/p/KSR/knowledge/proposals",
  render: () => {
    const list = pending(), p = curProp(), done = PROPS.filter((x) => x.st !== "pending"), i = p ? list.indexOf(p) : -1;
    const queue = `<aside class="kb-queue" aria-label="Antrean usulan">${AG.card({ title: "Antrean", desc: "Usulan dari AI, urut dari yang terbaru.", pad: false,
      foot: `<div class="kb-own">${av(KB.owner, 18)}<span class="grow">Pemilik knowledge<br><b class="b5">${AG.PEOPLE[KB.owner].name}</b></span>${btn("Ganti", "sm", "", { "data-menu": "kb-owner" })}</div>`, body: `
      <div class="kb-qh">${ic("git-pull-request", "s14")}Menunggu persetujuan<span class="grow"></span><span>${list.length}</span></div>
      ${list.map((x) => { const d = docOf(x.doc); return `<button class="kb-qi ${x === p ? "on" : ""}" data-act="kb-sel" data-arg="${x.id}" aria-current="${x === p}">${stDot("prop")}<span style="min-width:0"><b>${d.n}</b><span class="s">${ic(TRIG[x.trig][1], "s12")}${TRIG[x.trig][0]} · ${x.w}</span></span>${pmOf(x)}</button>`; }).join("") || `<div class="kb-qd mut">${ic("circle-check", "s14 text-ok")}Antrean kosong</div>`}
      ${done.length ? `<div class="kb-qh">${ic("history", "s14")}Sudah ditinjau<span class="grow"></span><span>${done.length}</span></div>
      ${done.map((x) => `<div class="kb-qd"><a class="grow trunc" href="${kbDoc(x.doc)}">${docOf(x.doc).n}</a>${x.st === "approved" ? pill("Disetujui", "ok", "", "check") : pill("Ditolak", "dng", "", "x")}${x.id !== "p4" ? btn("", "ghost icon sm", "arrow-back-up", { "data-act": "kb-undo", "data-arg": x.id, "aria-label": `Batalkan keputusan ${docOf(x.doc).n}` }) : ""}</div>`).join("")}` : ""}` })}
    </aside>`;
    let main;
    if (!p) main = AG.card({ title: "Tinjau usulan", body: `${AG.empty("circle-check", "Semua usulan sudah ditinjau", "Dokumen yang disetujui sudah di-commit ke .agentgate/knowledge/. Usulan baru muncul setelah tiket Done, merge ke main, atau audit mingguan.", `<a class="btn pri" href="${kbHome}">${ic("home", "s14")}Ke beranda knowledge</a>`)}` });
    else {
      const d = docOf(p.doc), G = grpOf(d.g);
      main = AG.card({ cls: "kb-rvcard", pad: false,
        title: `<span class="row g8">Usulan ${i + 1} dari ${list.length}<span class="kb-steps" aria-hidden="true">${list.map((x, j) => `<i class="${j === i ? "on" : ""}"></i>`).join("")}</span></span>`,
        desc: `<span class="kb-kbd"><span><kbd>A</kbd>Approve</span><span><kbd>R</kbd>Tolak</span><span><kbd>J</kbd><kbd>K</kbd>Pindah</span></span>`,
        action: `${btn("", "sm icon", "chevron-up", { "data-act": "kb-nav", "data-arg": "-1", "aria-label": "Usulan sebelumnya (K)", ...(i > 0 ? {} : { disabled: true }) })}${btn("", "sm icon", "chevron-down", { "data-act": "kb-nav", "data-arg": "1", "aria-label": "Usulan berikutnya (J)", ...(i < list.length - 1 ? {} : { disabled: true }) })}`,
        foot: `<div class="kb-rvfoot"><span class="msg">${ic("git-commit", "s14")}Approve menulis commit ke .agentgate/knowledge/</span>${btn("Tolak", "", "x", { "data-dialog": "kb-reject", "data-arg": p.id })}${btn("Edit dulu", "", "pencil", { "data-go": kbDoc(d.id) })}${btn(list.length > 1 ? "Approve dan lanjut" : "Approve", "pri", "check", { "data-act": "kb-approve", "data-arg": p.id })}</div>`,
        body: `
        <div class="kb-meta">
          <div class="col g4"><a class="kb-grpl" href="${kbGroup(G.s)}">${ic(G.i, "s14")}${G.g}</a><h2><a href="${kbDoc(d.id)}">${d.n}</a></h2><span class="mono xs mut">.agentgate/knowledge/${d.id}.md</span></div>
          <p>${p.why}</p>
          <div class="kb-mf"><span>${ic(TRIG[p.trig][1], "s14")}Dipicu <b>${TRIG[p.trig][0]}</b></span>${p.tk ? `<span>Tiket ${tref(p.tk)}</span>` : ""}<span>${av("writer", 16)}<b>Agent Tech Writer</b></span><span>${ic("clock", "s14")}${p.w}</span><span>${ic("coins", "s14")}${p.tok}</span><span>${pmOf(p)}</span></div>
        </div>
        <div class="kb-sbs"><div class="kb-sbs-h"><div>${ic("file-text", "s14")}Sebelum</div><div>${ic("sparkles", "s14")}Sesudah (usulan AI)</div></div>${sbsRows(p.diff)}</div>` });
    }
    const body = `
      ${head("Tinjau usulan", "Perubahan dari AI masuk sebagai usulan dan baru menimpa dokumen setelah disetujui pemilik knowledge. Tinjau satu per satu: sebelum dan sesudah.", btn("Sinkronkan sekarang", "", "refresh", { "data-dialog": "kb-scan" }))}
      ${tabsKb("p")}
      <div class="kb-rv">${queue}<div class="kb-rvmain">${main}</div></div>`;
    return AG.shell({ project: K, crumbs: kbCrumbs("Tinjau usulan"), ...AG.projectTabs(K), body, width: "wide" });
  } });
document.addEventListener("keydown", (e) => {
  if (!/^\/p\/[^/]+\/knowledge\/proposals$/.test(AG.route.path) || e.ctrlKey || e.metaKey || e.altKey) return;
  if (/INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName) || document.activeElement?.isContentEditable || document.getElementById("overlay")?.children.length) return;
  const p = curProp(); if (!p) return;
  const k = e.key.toLowerCase();
  if (k === "a") AG.ACTIONS["kb-approve"](null, p.id);
  else if (k === "r") AG.openDialog("kb-reject", p.id);
  else if (k === "j" || k === "k") AG.ACTIONS["kb-nav"](null, k === "j" ? "1" : "-1");
});

/* ====================== Daftar per kelompok ====================== */
const dnum = (s) => { const m = /(\d+) (Okt|Nov)/.exec(s); return m ? (m[2] === "Nov" ? 1100 : 1000) + +m[1] : 9999; };
AG.screen({ m: "M8", g: "Knowledge", n: "Dokumen per kelompok", route: "/p/:k/knowledge/g/:group", path: "/p/KSR/knowledge/g/arsitektur",
  render: ({ group }) => {
    const G = grpOf(group), ds = G.ids.map(docOf), c = (st) => ds.filter((d) => d.st === st).length;
    const rows = ds.map((d) => { const p = pendOf(d.id)[0]; const sub = d.st === "stale" ? `<span class="text-warn">${whyOf(d)}</span>` : p ? `<span class="mut">${p.why}</span>` : `<span class="mut mono">.agentgate/knowledge/${d.id}.md</span>`;
      return { cells: [`<span class="row g12" style="min-width:0"><span class="tile hide-sm">${ic("file-text", "s14")}</span><span class="kb-dn"><b>${d.n}</b>${sub}</span></span>`, stPill(d.st), `<span class="row g4">${av(d.own, 18)}${AG.PEOPLE[d.own].name.split(" ")[0]}</span>`, `<span class="mut">${d.up}</span>`,
          `<span class="row g4" style="justify-content:flex-end">${p ? btn("Tinjau usulan", "sm", "", { "data-act": "kb-review", "data-arg": p.id }) : d.st === "stale" ? btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "kb-stale-ai", "data-arg": d.id }) : ""}${btn("", "ghost icon sm", "dots", { "data-menu": "kb-doc", "data-arg": d.id, "aria-label": `Aksi ${d.n}` })}</span>`],
        sort: [d.n, d.st, d.own, dnum(d.up)], text: `${d.n} ${d.id} ${d.why || ""}`, filter: { st: ST[d.st][0], own: AG.PEOPLE[d.own].name } }; });
    const body = `
      ${head(G.g, `${ds.length} dokumen · ${GDESC[G.g] || ""}`, `${btn("Dokumen baru", "", "plus", { "data-dialog": "kb-new" })}${btn("Scan ulang kelompok", "", "refresh", { "data-dialog": "kb-scan" })}`, `<span class="tile lg">${ic(G.i, "s18")}</span>`)}
      <nav class="tabs" aria-label="Kelompok dokumen">${GROUPS.map((x) => `<a href="${kbGroup(x.s)}" class="${x === G ? "on" : ""}">${x.g} <span class="n">${x.ids.length}</span></a>`).join("")}</nav>
      ${AG.stats([
        { label: "Terbaru", value: c("ok"), icon: "circle-check", tone: "ok", hint: "sinkron dengan kode" },
        { label: "Ada usulan", value: c("prop"), icon: "git-pull-request", tone: "acc", hint: c("prop") ? "menunggu approve" : "tidak ada", ...(c("prop") ? { go: kbReview } : {}) },
        { label: "Basi", value: c("stale"), icon: "clock", tone: "warn", hint: c("stale") ? "perlu dicek" : "tidak ada" },
      ])}
      ${AG.card({ title: "Dokumen", desc: "Klik baris untuk membuka editor dokumen.", body: AG.datatable({ id: "kb-g-" + G.s, columns: [{ label: "Dokumen" }, { label: "Status", w: "130px" }, { label: "Pemilik", w: "110px", hide: "sm" }, { label: "Diperbarui", w: "110px", hide: "sm" }, { label: "", w: "190px", sort: false, align: "right", hide: "sm" }],
        rows, search: "Cari di kelompok ini…", filters: [{ k: "st", label: "Status", icon: "circle-dashed", options: ["Terbaru", "Ada usulan", "Basi"] }, { k: "own", label: "Pemilik", icon: "user", options: [...new Set(ds.map((d) => AG.PEOPLE[d.own].name))] }],
        rowGo: (i) => kbDoc(ds[i].id), pageSize: 10, empty: AG.empty("file-text", "Tidak ada dokumen", "Tidak ada dokumen yang cocok dengan filter.") }) })}`;
    return AG.shell({ project: K, crumbs: kbCrumbs(G.g), ...AG.projectTabs(K), body, width: "mid" });
  } });

/* ====================== Editor dokumen ====================== */
const MERMAID = `flowchart LR
  web[kasir-web<br/>tablet kasir] -->|HTTP| api[kasir-api]
  web --> prn[printer-bridge]
  api --> db[(PostgreSQL)]
  api -->|buat QR| xnd[Xendit QRIS]
  xnd -. callback HMAC .-> api
  api -. event struk.cetak .-> prn`;
const mmSvg = () => {
  const nd = (x, y, w, t, s, a) => `<g class="nd ${a ? "a" : ""}" transform="translate(${x},${y})"><rect width="${w}" height="44" rx="8"/><text x="${w / 2}" y="${s ? 19 : 27}" text-anchor="middle">${t}</text>${s ? `<text class="s" x="${w / 2}" y="33" text-anchor="middle">${s}</text>` : ""}</g>`;
  return `<svg viewBox="10 30 490 210" role="img" aria-label="Pratinjau diagram Mermaid arsitektur">
    <defs><marker id="kb-ar" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M0,1 L9,5 L0,9 z" fill="var(--faint)"/></marker></defs>
    <path class="ln" d="M120 62 H196" marker-end="url(#kb-ar)"/><text class="lb" x="158" y="55" text-anchor="middle">HTTP</text>
    <path class="ln" d="M70 84 V160 H196" marker-end="url(#kb-ar)"/>
    <path class="ln" d="M296 62 H372" marker-end="url(#kb-ar)"/>
    <path class="ln" d="M246 84 V120 H400 V150" marker-end="url(#kb-ar)"/><text class="lb" x="330" y="114" text-anchor="middle">buat QR</text>
    <path class="ln d" d="M420 194 V214 H262 V196" marker-end="url(#kb-ar)"/><text class="lb" x="344" y="230" text-anchor="middle">callback HMAC</text>
    <path class="ln d" d="M226 84 V140" marker-end="url(#kb-ar)"/><text class="lb" x="208" y="118" text-anchor="end">struk.cetak</text>
    ${nd(20, 40, 100, "kasir-web", "tablet kasir")}${nd(196, 40, 100, "kasir-api", "", 1)}${nd(372, 40, 116, "PostgreSQL", "34 tabel")}${nd(176, 150, 120, "printer-bridge", "shared package")}${nd(360, 150, 120, "Xendit QRIS", "eksternal")}
  </svg>`;
};
const GO = `<span class="kb-hl-k">func</span> <span class="kb-hl-f">VerifyCallback</span>(r *http.Request, secret []<span class="kb-hl-k">byte</span>) <span class="kb-hl-k">error</span> {
  sig := r.Header.Get(<span class="kb-hl-s">"X-Signature"</span>)
  body, _ := io.ReadAll(r.Body)
  mac := hmac.New(sha256.New, secret)
  mac.Write(body)
  <span class="kb-hl-k">if</span> !hmac.Equal([]<span class="kb-hl-k">byte</span>(sig), []<span class="kb-hl-k">byte</span>(hex.EncodeToString(mac.Sum(<span class="kb-hl-k">nil</span>)))) {
    <span class="kb-hl-k">return</span> ErrInvalidSignature <span class="kb-hl-c">// tolak, jangan ubah status transaksi</span>
  }
  <span class="kb-hl-k">return</span> <span class="kb-hl-k">nil</span>
}`;
const OUTLINE = ["Komponen utama", "Layanan", "Alur pembayaran QRIS", "Kesiapan mode offline (KSR-33)"];
AG.action("kb-outline", (el, arg) => {
  const h = [...document.querySelectorAll(".kb-ed2 .ed .area h2")][+arg];
  if (h) { h.scrollIntoView({ behavior: "smooth", block: "center" }); document.querySelectorAll(".kb-ol").forEach((x, i) => x.classList.toggle("on", i === +arg)); }
});

AG.screen({ m: "M8", g: "Knowledge", n: "Editor dokumen knowledge", route: "/p/:k/knowledge/:doc", path: "/p/KSR/knowledge/arsitektur",
  render: ({ doc }) => {
    const d = docOf(doc);
    const full = d.id === "arsitektur";
    const task = (on, t) => `<li class="task"><button type="button" class="cb ${on ? "on" : ""}" data-toggle contenteditable="false">${ic("check")}</button><span>${t}</span></li>`;
    const html = full ? `
      <p>Kasir Online terdiri dari dua repo: <strong>kasir-web</strong> (React, dipakai di tablet kasir) dan <strong>kasir-api</strong> (Go). Pembayaran QRIS memakai Xendit, lihat <a class="kb-dl" href="#/p/${K}/knowledge/integrasi">Integrasi Pihak Ketiga</a>. Pemilik dokumen ini <span class="mention">@Adi Admin</span>.</p>
      <h2>${OUTLINE[0]}</h2>
      <div class="kb-blk" contenteditable="false"><div class="kb-blkh">${ic("sitemap", "s14")}Mermaid<span class="grow"></span><div class="seg"><button class="on" data-act="kb-mm" data-arg="">Berdampingan</button><button data-act="kb-mm" data-arg="prev">Pratinjau</button><button data-act="kb-mm" data-arg="src">Kode</button></div></div>
        <div class="kb-mm"><div class="code">${esc(MERMAID)}</div>${mmSvg()}</div></div>
      <h2>${OUTLINE[1]}</h2>
      <div class="kb-tw"><table><tr><th>Komponen</th><th>Repo</th><th>Bahasa</th><th>Pemilik</th><th>Node graph</th></tr>
        <tr><td>Aplikasi kasir</td><td><code>kasir-web</code></td><td>TypeScript</td><td><span class="mention">@Dewi Dev</span></td><td><a data-act="gv-open" data-arg="0:web" href="#/p/${K}/graph">graph: kasir-web</a></td></tr>
        <tr><td>API &amp; worker</td><td><code>kasir-api</code></td><td>Go</td><td><span class="mention">@Adi Admin</span></td><td><a data-act="gv-open" data-arg="0:api" href="#/p/${K}/graph">graph: kasir-api</a></td></tr>
        <tr><td>Cetak struk</td><td><code>printer-bridge</code></td><td>TypeScript</td><td><span class="mention">@Qori QA</span></td><td><a data-act="gv-open" data-arg="0:prn" href="#/p/${K}/graph">graph: printer-bridge</a></td></tr></table></div>
      <h2>${OUTLINE[2]}</h2>
      <p>Kasir membuat tagihan QRIS (<span class="mention">KSR-24</span>). <mark class="kb-cm" data-act="kb-cm" data-arg="c1">Callback dari Xendit diverifikasi dengan signature HMAC sebelum status transaksi diubah.</mark> Endpoint callback dibuat di <span class="mention">KSR-41</span>.</p>
      <pre data-lang="go"><code>${GO}</code></pre>
      <h2>${OUTLINE[3]}</h2>
      <ul>${task(1, "Transaksi tunai tersimpan di IndexedDB saat koneksi putus")}${task(1, "Antrian sinkron dikirim ulang saat online")}${task(0, "QRIS dinonaktifkan otomatis saat offline")}${task(0, `<mark class="kb-cm" data-act="kb-cm" data-arg="c2">Nomor struk offline tidak bentrok antar tablet</mark>`)}</ul>
      <div class="kb-drop" contenteditable="false">${ic("photo", "s20")}<span class="grow"><b class="fg b5">Tempel atau seret gambar ke sini</b><br><span class="sm">PNG, JPG, SVG · disimpan di <span class="mono">.agentgate/knowledge/img/</span></span></span>${btn("Unggah", "sm", "upload", { "data-toast": "jaringan-toko.png diunggah" })}</div>
      <p></p>`
      : `<p>Dokumen “${esc(d.n)}” dibuat dari scan standar project Kasir Online. Isi lengkap ada di prototype untuk dokumen <a class="kb-dl" href="#/p/${K}/knowledge/arsitektur">Arsitektur Sistem</a>.</p><h2>Ringkasan</h2><p>Tulis ringkasan di sini, atau ketik / untuk menyisipkan tabel, diagram Mermaid, atau tautan ke node graph.</p>`;
    const side = (icon, t, inner, action = "") => AG.card({ cls: "kb-sc", title: `<span class="row g8">${ic(icon, "s14 faint")}${t}</span>`, action, body: inner });
    const aside = `
      ${full ? side("list", "Outline", `<div class="col g4">${OUTLINE.map((h, i) => `<button class="kb-ol" data-act="kb-outline" data-arg="${i}">${esc(h)}</button>`).join("")}</div>`) : ""}
      ${full ? side("message-circle", `Komentar <span class="pill gray">2</span>`, `<div class="col g12">
        <div class="kb-thr" id="thr-c1" data-arg="c1"><q>Callback dari Xendit diverifikasi dengan signature HMAC…</q>
          <div class="msg">${av("DD", 20)}<div><b class="b5">Dewi Dev</b> <span class="faint xs">kemarin</span><br>Callback ganda sudah idempoten? <a class="kb-men" href="#/settings/members">@Adi Admin</a> perlu dicatat di sini juga.</div></div>
          <div class="msg">${av("AA", 20)}<div><b class="b5">Adi Admin</b> <span class="faint xs">5 jam lalu</span><br>Sudah, lihat usulan dari PR #139.</div></div>
          <div class="row">${btn("Balas", "sm ghost", "", { "data-toast": "Balasan terkirim" })}${btn("Selesai", "sm ghost", "check", { "data-toast": "Komentar ditandai selesai" })}</div></div>
        <div class="kb-thr" id="thr-c2" data-arg="c2"><q>Nomor struk offline tidak bentrok…</q>
          <div class="msg">${av("QQ", 20)}<div><b class="b5">Qori QA</b> <span class="faint xs">2 jam lalu</span><br>Usul format prefix ID tablet, mis. <code>T03-000123</code>. Terkait ${tref("KSR-30")}.</div></div>
          <div class="row">${btn("Balas", "sm ghost", "", { "data-toast": "Balasan terkirim" })}${btn("Selesai", "sm ghost", "check", { "data-toast": "Komentar ditandai selesai" })}</div></div></div>`) : ""}
      ${side("topology-star-3", "Terhubung ke graph", `<div class="row wrap">${[["0:api", "kasir-api"], ["0:web", "kasir-web"], ["1:payment", "payment"], ["0:prn", "printer-bridge"], ["0:xnd", "Xendit QRIS"]].map(([a, n]) => `<button class="chip mono" data-act="gv-open" data-arg="${a}">${n}</button>`).join("")}</div><div class="mut xs" style="margin-top:8px">Dokumen ditandai basi bila node ini berubah.</div>`)}
      ${side("ticket", "Tiket terkait", `<div class="col g4">${["KSR-24", "KSR-41", "KSR-33"].map((k) => `<div class="row" style="min-width:0">${tref(k)}<span class="trunc sm">${AG.ticket(k).t}</span></div>`).join("")}</div>`)}
      ${side("git-commit", "Riwayat versi", `<div>${VERS.map(([h, w, m, dt], i) => `<button class="kb-ver ${i === 0 ? "cur" : ""}" style="width:100%" ${i ? `data-dialog="kb-ver" data-arg="${h}"` : `data-dialog="kb-history"`}><i></i><span style="min-width:0;text-align:left"><span class="sm trunc" style="display:block">${m}</span><span class="xs mut"><span class="mono">${h}</span> · ${AG.PEOPLE[w].name.split(" ")[0]} · ${dt}</span></span></button>`).join("")}</div>`, `<button type="button" class="seeall" data-dialog="kb-history">Semua${ic("arrow-right", "s14")}</button>`)}`;
    const body = `<div class="kb-ed2">
      <div class="kb-main">
        <div class="row wrap sm mut">${stPill(d.st)}<a class="pill gray" href="${kbGroup(d.g)}">${ic(grpOf(d.g).i)}${d.g}</a><span class="row g4">${av(d.own, 18)}${AG.PEOPLE[d.own].name}</span><span>· diperbarui ${d.up} · commit <span class="mono">9f3c2a1</span></span><span class="mono xs hide-sm">.agentgate/knowledge/${d.id}.md</span></div>
        <h1 class="kb-title" contenteditable="true" aria-label="Judul dokumen">${d.n}</h1>
        ${d.st === "stale" ? `<div class="alert warn">${ic("clock", "s14")}<span class="grow"><b>Dokumen basi.</b> ${whyOf(d)}.</span>${btn("Masih benar", "sm ghost", "", { "data-act": "kb-stale-ok", "data-arg": d.id })}${btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "kb-stale-ai", "data-arg": d.id })}</div>` : ""}
        ${(() => { const p = pendOf(d.id)[0]; return p ? `<div class="alert">${ic("git-pull-request", "s14")}<span class="grow"><b>Ada usulan perubahan dari AI</b> · ${trigTxt(p)} · ${pmOf(p)}</span>${btn("Tinjau", "sm pri", "", { "data-act": "kb-review", "data-arg": p.id })}</div>` : ""; })()}
        ${AG.editor({ full: true, html, ph: "Mulai menulis… ketik / untuk menyisipkan blok", save: "Simpan & commit", footer: btn("Riwayat", "ghost sm", "history", { "data-dialog": "kb-history" }) })
          .replace('data-toast="Perubahan disimpan"', 'data-toast="Tersimpan sebagai commit 7d2e91b ke .agentgate/knowledge/"')}
      </div>
      <aside class="kb-aside" aria-label="Komentar dan info dokumen">${aside}</aside></div>`;
    return AG.shell({ project: K, crumbs: kbCrumbs(`<a href="${kbGroup(d.g)}">${d.g}</a>`, d.n), ...AG.projectTabs(K), body, width: "wide",
      actions: btn("", "ghost icon", "dots", { "data-menu": "kb-more", "aria-label": "Lainnya" }) });
  } });

/* ====================== Pengaturan knowledge workspace ====================== */
const WS = [
  { id: "standar-coding", n: "Standar coding perusahaan", own: "AA", p: ["KSR", "GDG", "PPL"], up: "4 Nov", i: "code", stages: ["Development", "Code Review"],
    md: `## Penamaan\n\n- Fungsi dan variabel memakai camelCase, file memakai kebab-case\n- Nama tabel jamak dengan huruf kecil: \`transactions\`, \`receipts\`\n- Hindari singkatan kecuali yang ada di Glosarium bisnis retail\n\n## Wajib di setiap PR\n\n- [x] Setiap perubahan logika disertai test\n- [x] Tidak ada secret di kode; pakai env var dan secret manager\n- [ ] Endpoint baru tercatat di dokumen API\n\n## Error handling\n\n> Jangan menelan error. Kembalikan error yang bisa dibaca kasir, catat detail teknis di log.\n\n\`\`\`go\nif err != nil {\n  return fmt.Errorf("simpan transaksi %s: %w", trx.ID, err)\n}\n\`\`\`\n\n## Batas ukuran\n\n| Hal | Batas |\n| --- | --- |\n| Panjang fungsi | 60 baris |\n| Ukuran PR | 400 baris berubah |\n| Kompleksitas | cyclomatic 15 |\n` },
  { id: "kebijakan-keamanan", n: "Kebijakan keamanan", own: "AA", p: ["KSR", "GDG", "PPL"], up: "1 Nov", i: "shield-lock", stages: ["Development", "Code Review", "QA"], md: `## Data pelanggan\n\n- Nomor telepon dan email dienkripsi saat disimpan\n- Log tidak boleh memuat PIN, token, atau nomor kartu\n` },
  { id: "konvensi-commit", n: "Konvensi commit & PR", own: "DD", p: ["KSR", "GDG", "PPL"], up: "28 Okt", i: "git-commit", stages: ["Development"], md: `## Format commit\n\n- \`feat(kasir): diskon per item\`\n- Satu tiket satu branch: \`ksr-28-diskon-item\`\n` },
  { id: "glosarium-retail", n: "Glosarium bisnis retail", own: "OO", p: ["KSR", "PPL"], up: "20 Okt", i: "book", stages: ["PRD", "Design UI"], md: `## Istilah\n\n- **Shift**: periode kerja satu kasir dari buka sampai tutup laci\n- **Void**: pembatalan item sebelum transaksi dibayar\n` },
];
const TYPES = [
  { id: "prosedur-tutup-kasir", n: "Prosedur tutup kasir", g: "Operasional", p: ["KSR"], c: 2, i: "cash-register",
    tpl: `## Tujuan\n\nJelaskan kapan prosedur ini dipakai dan siapa yang menjalankannya.\n\n## Langkah\n\n- [ ] Hitung uang di laci per pecahan\n- [ ] Cocokkan dengan laporan shift di aplikasi\n- [ ] Cetak struk tutup shift dan tanda tangan supervisor\n\n## Bila ada selisih\n\n| Selisih | Tindakan |\n| --- | --- |\n| Sampai Rp5.000 | Catat di laporan shift |\n| Lebih dari Rp5.000 | Lapor supervisor, buat tiket |\n`,
    ai: `Tulis berdasarkan modul **shift** dan **report** di kasir-api.\n\n- Sertakan batas selisih yang diizinkan dari dokumen Aturan Bisnis\n- Rujuk endpoint \`POST /v1/shifts/:id/close\`\n- Jangan menyalin nilai konfigurasi rahasia\n` },
  { id: "integrasi-printer", n: "Integrasi printer struk", g: "Arsitektur", p: ["KSR", "GDG"], c: 3, i: "printer", tpl: `## Perangkat yang didukung\n\n## Alur cetak\n`, ai: `Tulis dari package printer-bridge.\n` },
  { id: "matriks-harga", n: "Matriks harga & promo", g: "Domain & bisnis", p: ["KSR", "PPL"], c: 1, i: "tag", tpl: `## Aturan harga\n\n## Promo aktif\n`, ai: `Ambil dari tabel promo dan aturan bisnis.\n` },
];
const projOpts = () => AG.PROJECTS.map((p) => ({ v: p.k, l: p.n, lead: AG.projDot(p.k, 8), sub: p.d }));
const peopleOpts = () => Object.entries(AG.PEOPLE).filter(([k]) => k !== "VV").map(([k, p]) => ({ v: k, l: p.name, sub: p.job, lead: av(k, 18) }));
const pn = (ks) => (ks.length === AG.PROJECTS.length ? "semua project" : ks.map((k) => AG.project(k).n).join(", "));

AG.screen({ m: "M8", g: "Knowledge", n: "Knowledge workspace", route: "/settings/knowledge",
  render: () => {
    const s = SUBS.ws.sec;
    const docs = AG.card({ title: "Dokumen workspace", desc: "Dibaca agent di semua project, misalnya standar coding dan kebijakan keamanan.", pad: false,
      action: `<a class="btn sm" href="#/settings/knowledge/doc/baru">${ic("plus", "s14")}Dokumen baru</a>`, body: WS.map((w) => `<a class="lrow" href="#/settings/knowledge/doc/${w.id}"><span class="tile">${ic(w.i, "s14")}</span><span class="grow" style="min-width:0"><b class="b5">${w.n}</b><br><span class="mut sm trunc" style="display:block">Berlaku untuk ${pn(w.p)} · diperbarui ${w.up}</span></span><span class="hide-sm">${av(w.own, 20)}</span>${ic("chevron-right", "s14 faint")}</a>`).join("") });
    const types = AG.card({ title: "Jenis dokumen custom", desc: "Tambahan dari katalog bawaan, lengkap dengan template dan instruksi untuk AI.", pad: false,
      action: `<a class="btn sm" href="#/settings/knowledge/type/baru">${ic("plus", "s14")}Jenis baru</a>`, body: TYPES.map((t) => `<a class="lrow" href="#/settings/knowledge/type/${t.id}"><span class="tile acc">${ic("category-2", "s14")}</span><span class="grow" style="min-width:0"><span class="row g8 wrap"><b class="b5">${t.n}</b><span class="pill gray">${t.g}</span></span><span class="mut sm trunc" style="display:block">${pn(t.p)} · ${t.c} dokumen dibuat · template + instruksi AI</span></span>${ic("chevron-right", "s14 faint")}</a>`).join("") });
    const auto = AG.card({ title: "Update otomatis", desc: "Perubahan dari AI selalu masuk sebagai usulan diff.",
      foot: AG.savebar("Simpan perubahan", { toast: "Pengaturan update otomatis disimpan" }),
      body: `<div class="alert gray">${ic("info-circle", "s14")}<span>Slash command <code>/ag-knowledge-sync</code> menjalankan sinkron manual dari client AI. Biaya scan memakai token AI milik orang yang menjalankannya.</span></div>
      <div class="box">
        ${srow("Setiap tiket Done", "Update inkremental pada dokumen yang tersentuh, dihitung dari diff graph", sw(true, "Tiket Done"))}
        ${srow("Setiap merge ke branch utama", "Re-index graph lalu perbarui dokumen yang terdampak", sw(true, "Merge"))}
        ${srow("Audit mingguan", "Audit penuh untuk mendeteksi dokumen basi", `<span style="width:150px;display:block">${sel(["Senin 07.00", "Jumat 17.00", "Minggu 22.00"], "Senin 07.00")}</span>${sw(true, "Audit mingguan")}`)}
        ${srow("Kedalaman scan bawaan", "Dipakai saat project baru dibuat", sel(["Cepat", "Standar", "Mendalam"], "Standar"))}
        ${srow("Sinkron ke repo", "Dokumen disimpan di <code>.agentgate/knowledge/</code> dan di-commit lewat PR", sw(true, "Sinkron ke repo"))}
      </div>` });
    const body = `${head("Knowledge workspace", "Standar yang berlaku di semua project, jenis dokumen custom, dan aturan update otomatis.")}
      ${AG.spWrap("ws", { nav: [
        { k: "docs", label: "Dokumen workspace", icon: "file-text", count: WS.length, desc: "Standar lintas project" },
        { k: "types", label: "Jenis dokumen", icon: "category-2", count: TYPES.length, desc: "Template + instruksi AI" },
        { k: "auto", label: "Update otomatis", icon: "refresh", desc: "Pemicu dan audit" },
      ], body: AG.panel("docs", docs, s === "docs") + AG.panel("types", types, s === "types") + AG.panel("auto", auto, s === "auto") })}`;
    return AG.shell({ nav: "settings", crumbs: ["Pengaturan", "Knowledge workspace"], body, width: "mid" });
  } });

const wsCrumbs = (n) => ["Pengaturan", `<a href="#/settings/knowledge">Knowledge workspace</a>`, n];
const metaBox = (t, inner, action = "") => AG.card({ cls: "kb-sc", title: t, action, body: inner });

AG.screen({ m: "M8", g: "Knowledge", n: "Dokumen workspace", route: "/settings/knowledge/doc/:id", path: "/settings/knowledge/doc/standar-coding",
  render: ({ id }) => {
    const w = WS.find((x) => x.id === id);
    const isNew = !w;
    const d = w || { n: "", own: AG.ME, p: AG.PROJECTS.map((p) => p.k), up: "", i: "file-text", stages: [], md: "" };
    const STG = ["Brainstorming", "PRD", "Design UI", "Development", "Code Review", "QA", "Release"];
    const body = `${head(isNew ? "Dokumen workspace baru" : esc(d.n), "Berlaku untuk semua project terpilih dan dibaca agent lewat MCP di stage yang relevan.")}
      <div class="kb-ed2">
      <div class="kb-main">
        ${AG.card({ title: "Isi dokumen", desc: "Standar yang harus diikuti agent dan tim. Tabel, checklist, blok kode, dan Mermaid didukung.",
          foot: AG.savebar(isNew ? "Buat dokumen" : "Simpan perubahan", { msg: isNew ? "Dokumen baru belum disimpan" : `Terakhir diperbarui ${d.up} oleh ${AG.PEOPLE[d.own].name}`, toast: isNew ? "Dokumen workspace dibuat" : "Dokumen workspace disimpan · agent memakai versi baru mulai job berikutnya" }),
          body: `${AG.fld("Judul", AG.inp(d.n, { ph: "Mis. Standar coding perusahaan" }))}
          ${AG.editor({ full: true, html: AG.mdToHtml(d.md), ph: "Tulis standar yang harus diikuti agent dan tim… ketik / untuk menyisipkan blok" })}` })}
      </div>
      <aside class="kb-aside" aria-label="Pengaturan dokumen">
        ${metaBox("Pengaturan", `${AG.fld("Berlaku untuk", AG.combo({ options: projOpts(), value: d.p, multi: true, ph: "Pilih project" }), "Project baru otomatis ikut bila semua project dipilih.")}
          ${AG.fld("Pemilik", AG.combo({ options: peopleOpts(), value: d.own }))}
          ${AG.fld("Dibaca di stage", `<div class="row wrap">${STG.map((x) => `<button class="chip ${d.stages.includes(x) ? "on" : ""}" data-toggle>${x}</button>`).join("")}</div>`)}`)}
        ${isNew ? "" : metaBox("Info", `<dl class="props"><dt>Berkas</dt><dd class="mono xs" style="word-break:break-all">workspace/${d.id}.md</dd><dt>Dibaca agent</dt><dd>214 kali minggu ini</dd><dt>Versi</dt><dd><button class="link" data-dialog="kb-history">4 versi</button></dd></dl>`)}
        ${isNew ? "" : metaBox("Zona berbahaya", `<p class="mut sm">Dokumen yang diarsipkan tidak lagi dibaca agent.</p><button class="btn dng sm" data-toast="Dokumen diarsipkan" style="justify-self:start">${ic("archive", "s14")}Arsipkan dokumen</button>`)}
      </aside></div>`;
    return AG.shell({ nav: "settings", crumbs: wsCrumbs(isNew ? "Dokumen baru" : d.n), body, width: "wide" });
  } });

AG.screen({ m: "M8", g: "Knowledge", n: "Jenis dokumen custom", route: "/settings/knowledge/type/:id", path: "/settings/knowledge/type/prosedur-tutup-kasir",
  render: ({ id }) => {
    const t = TYPES.find((x) => x.id === id);
    const isNew = !t;
    const d = t || { n: "", g: "Operasional", p: ["KSR"], c: 0, tpl: "", ai: "" };
    const GROUPS = ["Gambaran umum", "Arsitektur", "Domain & bisnis", "Backend & kode", "Frontend", "Kualitas", "Operasional", "Riwayat"];
    const body = `${head(isNew ? "Jenis dokumen baru" : esc(d.n), "Jenis custom muncul di katalog setiap project yang dipilih. AI memakai template dan instruksi ini saat menyusun dokumen.")}
      <div class="kb-ed2">
      <div class="kb-main">
        ${AG.card({ title: "Jenis dokumen", desc: "Nama, kerangka dokumen, dan instruksi untuk Agent Tech Writer.",
          foot: AG.savebar(isNew ? "Buat jenis" : "Simpan jenis", { msg: isNew ? "Jenis baru belum disimpan" : `${d.c} dokumen memakai jenis ini`, toast: isNew ? "Jenis dokumen dibuat" : "Jenis dokumen disimpan" }),
          body: `${AG.fld("Nama jenis", AG.inp(d.n, { ph: "Mis. Prosedur tutup kasir" }))}
          ${AG.fld("Template dokumen", AG.editor({ full: true, html: AG.mdToHtml(d.tpl), ph: "Tulis kerangka dokumen…" }), "Kerangka yang diisi saat dokumen jenis ini dibuat. Boleh memakai tabel, checklist, dan diagram Mermaid.")}
          ${AG.fld("Instruksi untuk AI", AG.editor({ html: AG.mdToHtml(d.ai), h: "160px", ph: "Mis. sumber kode yang dibaca, batasan, gaya penulisan…" }), "Dibaca Agent Tech Writer saat scan dan saat memperbarui dokumen jenis ini.")}` })}
      </div>
      <aside class="kb-aside" aria-label="Pengaturan jenis">
        ${metaBox("Pengaturan", `${AG.fld("Kelompok", AG.combo({ options: GROUPS.map((g) => ({ v: g, l: g })), value: d.g }))}
          ${AG.fld("Berlaku untuk project", AG.combo({ options: projOpts(), value: d.p, multi: true, ph: "Pilih project" }), "Hanya muncul di katalog project terpilih.")}
          ${AG.fld("Pembuatan", `<div class="col g8">${[["Dibuat AI saat scan", true], ["Diperbarui saat tiket Done", true], ["Wajib disetujui pemilik knowledge", true]].map(([l, on]) => `<div class="row g8 sm" style="justify-content:space-between">${l}${sw(on, l)}</div>`).join("")}</div>`)}`)}
        ${isNew ? "" : metaBox(`Dokumen jenis ini <span class="pill gray">${d.c}</span>`, `<div class="col g4">${["SOP tutup kasir harian", "Tutup kasir saat offline"].slice(0, Math.min(2, d.c)).map((x) => `<a class="row g4 sm link" href="#/p/KSR/knowledge/runbook">${ic("file-text", "s14")}${x}</a>`).join("")}</div>`)}
        ${isNew ? "" : metaBox("Zona berbahaya", `<p class="mut sm">Dokumen yang sudah dibuat tetap ada, tetapi jenis ini hilang dari katalog.</p><button class="btn dng sm" data-toast="Jenis dokumen dihapus dari katalog" style="justify-self:start">${ic("trash", "s14")}Hapus jenis</button>`)}
      </aside></div>`;
    return AG.shell({ nav: "settings", crumbs: wsCrumbs(isNew ? "Jenis baru" : d.n), body, width: "wide" });
  } });
})();

/* Bagian C: AI Office dipindah ke screens-office.js (M10). Aturan bersama untuk panel lain tetap di sini. */
AG.css(`.mk-side dd > a, .gv-panel .row > a { flex-shrink: 0; white-space: nowrap; }`);

/* Bagian D: M10 Penghemat token dan pratinjau mockup. */
(() => {
const { ic, av, btn, pill, sw, section, head, tref, esc } = AG;
const K = "KSR";

AG.css(`
.tk-chart { width: 100%; height: auto; display: block; }
.tk-chart .ax { fill: var(--muted); font-size: 11px; }
.tk-chart .gl { stroke: var(--line); }
.tk-chart .u { fill: var(--accent); }
.tk-chart .s { fill: var(--accent); opacity: 0.28; }
.tk-chart .c { fill: var(--success); opacity: 0.55; }
.tk-chart .o { fill: var(--warn); opacity: 0.8; }
.tk-chart .lb { fill: var(--fg2); font-size: 11.5px; }
.tk-chart .v { fill: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }
.tk-leg { display: flex; gap: 14px; flex-wrap: wrap; font-size: 12px; color: var(--muted); }
.tk-leg span { display: inline-flex; align-items: center; gap: 6px; }
.tk-leg i { width: 10px; height: 10px; border-radius: 3px; display: inline-block; }
.tk-feat { display: grid; grid-template-columns: minmax(0, 1fr) 88px 110px 44px; gap: 14px; align-items: center; padding: 12px 16px; border-top: 1px solid var(--line); }
.tk-feat:first-child { border-top: 0; }
.tk-note { display: flex; gap: 8px; align-items: center; padding: 10px 16px; font-size: 12.5px; color: var(--muted); background: var(--muted-bg); }
.tk-ch { display: grid; gap: 12px; padding-top: 4px; }
.tk-feat .d { color: var(--muted); font-size: 12.5px; margin-top: 2px; }
@media (max-width: 899px) { .tk-feat { grid-template-columns: minmax(0, 1fr) 44px; } .tk-feat .c2, .tk-feat .c3 { display: none; } }

.mk { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 16px; align-items: start; }
.mk-main { display: flex; flex-direction: column; min-width: 0; }
.mk-tool { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
.mk-stage { flex: 1; background: var(--bg-sub); border-radius: 0 0 10px 10px; padding: 24px; display: grid; gap: 20px; align-content: start; justify-items: center; overflow: auto; }
.mk-frame { width: 100%; max-width: 1100px; border: 1px solid var(--line2); border-radius: 12px; overflow: hidden; background: var(--raised); box-shadow: var(--shadow-md); }
.mk-frame.mob { width: 390px; max-width: 100%; border-radius: 30px; border: 8px solid var(--of-bezel, #1f1f26); }
.mk-bar { height: 38px; display: flex; align-items: center; gap: 8px; padding: 0 12px; border-bottom: 1px solid var(--line); background: var(--muted-bg); }
.mk-dots { display: inline-flex; gap: 5px; } .mk-dots i { width: 9px; height: 9px; border-radius: 50%; background: var(--input); display: block; }
.mk-url { flex: 1; min-width: 0; height: 24px; border-radius: 6px; background: var(--raised); border: 1px solid var(--line); display: flex; align-items: center; gap: 6px; padding: 0 8px; font-family: var(--mono); font-size: 11.5px; color: var(--fg2); }
.mk-frame iframe { display: block; width: 100%; height: 600px; border: 0; background: #fff; }
.mk-frame.mob iframe { height: 700px; }
.mk-frame.mob .mk-bar { border-radius: 0; }
.mk-cmp { display: grid; grid-template-columns: repeat(2, minmax(0, 390px)); gap: 24px; justify-content: center; width: 100%; }
.mk-cmp .mk-frame { width: 100%; }
.mk-cap { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--muted); margin-bottom: 8px; }
.mk-thumbs { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; }
.mk-th { border: 1px solid var(--line2); border-radius: 8px; overflow: hidden; background: var(--raised); text-align: left; }
.mk-th.on { border-color: var(--accent); box-shadow: var(--ring); }
.mk-th svg { display: block; width: 100%; height: 74px; background: var(--muted-bg); }
.mk-th span { display: block; padding: 6px 8px; font-size: 11.5px; color: var(--muted); }
.mk-side { min-width: 0; position: sticky; top: 112px; }
.mk-card .cx-b { gap: 18px; }
.mk-foot { display: flex; gap: 8px; flex-wrap: wrap; justify-content: flex-end; padding: 12px 16px; }
.mk-side h3 { font-size: 12px; color: var(--muted); font-weight: 500; margin-bottom: 6px; }
@media (max-width: 1099px) { .mk { grid-template-columns: 1fr; } .mk-side { position: static; } }
@media (max-width: 899px) { .mk-stage { padding: 12px; } .mk-cmp { grid-template-columns: 1fr; } .mk-frame iframe { height: 520px; } }
`);

/* ====================== Penghemat token ====================== */
const FEATS = [
  ["Code graph built-in", "AI menjelajah lewat MCP graph, bukan grep atau membaca file utuh", 1, 1.9],
  ["RTK", "Output perintah (test log, git status, build log) dikompresi lewat hook. Bila hook client tidak mendukung, AI memakai rtk lewat aturan MCP", 1, 1.4],
  ["Aturan lewat MCP", "Instruksi diambil per stage, bukan semua aturan sekaligus", 1, 0.2],
  ["Serah terima ringkas", "Agent Dev menerima ringkasan PRD + AC + mockup, bukan transkrip brainstorming", 1, 0.15],
  ["Respons MCP berlapis", "Ringkasan dulu, detail diambil hanya saat diminta", 1, 0.6],
  ["Prefix konteks stabil", "Supaya prompt cache client sering kena", 1, 0.5],
  ["Gaya ringkas", "Per persona. Hanya untuk chat dan log; artifact tetap ditulis lengkap", 0, 0.05, true],
  ["Model per persona", "Model murah untuk tugas ringan (opsional)", 0, 0, false],
];
const WEEKS = [["22 Sep", 0.6, 0.3], ["29 Sep", 0.7, 0.4], ["6 Okt", 0.8, 0.5], ["13 Okt", 0.9, 0.55], ["20 Okt", 1.0, 0.6], ["27 Okt", 1.1, 0.7], ["3 Nov", 1.3, 0.8], ["10 Nov", 1.5, 0.95]];
const TKT = [["KSR-24", 520, 140, 610, "Rp142.000"], ["KSR-26", 410, 120, 480, "Rp118.000"], ["KSR-29", 300, 60, 390, "Rp84.000"], ["KSR-30", 280, 50, 300, "Rp76.000"], ["KSR-28", 190, 80, 210, "Rp61.000"], ["KSR-33", 160, 40, 150, "Rp44.000"]];
const JOBS = [["#2291", "KSR-26", "dev", "Claude Code · claude-sonnet", "412 rb", "38 rb", "1,1 jt", "Rp41.200", "jalan"], ["#2288", "KSR-29", "qa", "Codex CLI · gpt-5-codex", "236 rb", "21 rb", "520 rb", "Rp27.900", "jalan"], ["#2285", "KSR-24", "reviewer", "Claude Code · claude-opus", "188 rb", "14 rb", "640 rb", "Rp58.300", "selesai"], ["#2281", "KSR-30", "qa", "Codex CLI · gpt-5-codex", "142 rb", "9 rb", "310 rb", "Rp19.400", "gagal"], ["#2277", "KSR-28", "uiux", "Claude Code · claude-sonnet", "120 rb", "46 rb", "190 rb", "Rp22.600", "selesai"], ["#2270", "KSR-35", "pm", "Claude Code · claude-haiku", "64 rb", "12 rb", "88 rb", "Rp3.100", "selesai"]];

function weekChart() {
  const W = 680, H = 220, L0 = 44, B = 190, T = 14, max = 2.5, bw = 44, gap = (W - L0 - 10) / WEEKS.length;
  const y = (v) => B - (v / max) * (B - T);
  return `<svg class="tk-chart" viewBox="0 0 ${W} ${H}" role="img" aria-label="Token per minggu: terpakai dan dihemat">
    ${[0, 0.5, 1, 1.5, 2, 2.5].map((v) => `<line class="gl" x1="${L0}" x2="${W}" y1="${y(v)}" y2="${y(v)}"/><text class="ax" x="${L0 - 8}" y="${y(v) + 4}" text-anchor="end">${v ? String(v).replace(".", ",") + " jt" : "0"}</text>`).join("")}
    ${WEEKS.map(([l, u, s], i) => { const x = L0 + gap * i + (gap - bw) / 2; return `<g><title>${l}: terpakai ${String(u).replace(".", ",")} jt, dihemat ${String(s).replace(".", ",")} jt</title><rect class="u" x="${x}" y="${y(u)}" width="${bw}" height="${B - y(u)}" rx="3"/><rect class="s" x="${x}" y="${y(u + s)}" width="${bw}" height="${y(u) - y(u + s) - 2}" rx="3"/><text class="ax" x="${x + bw / 2}" y="${B + 18}" text-anchor="middle">${l}</text></g>`; }).join("")}
  </svg>`;
}
function ticketChart() {
  const W = 680, rowH = 30, L0 = 230, max = 1300, H = TKT.length * rowH + 10;
  const sx = (v) => (v / max) * (W - L0 - 80);
  return `<svg class="tk-chart" viewBox="0 0 ${W} ${H}" role="img" aria-label="Token per tiket">
    ${TKT.map(([k, i, o, c, rp], r) => { const t = AG.ticket(k), yy = r * rowH + 6; return `<g><title>${k}: input ${i} rb, output ${o} rb, cache ${c} rb · ${rp}</title>
      <text class="lb" x="0" y="${yy + 15}" style="font-family:var(--mono);font-size:11px">${k}</text><text class="lb" x="56" y="${yy + 15}">${esc(t.t.length > 24 ? t.t.slice(0, 23) + "…" : t.t)}</text>
      <rect class="u" x="${L0}" y="${yy + 4}" width="${sx(i)}" height="16" rx="2"/><rect class="o" x="${L0 + sx(i) + 1}" y="${yy + 4}" width="${sx(o)}" height="16"/><rect class="c" x="${L0 + sx(i) + sx(o) + 2}" y="${yy + 4}" width="${sx(c)}" height="16" rx="2"/>
      <text class="v" x="${L0 + sx(i + o + c) + 8}" y="${yy + 16}">${rp}</text></g>`; }).join("")}
  </svg>`;
}
AG.screen({ m: "M10", g: "Penghemat token", n: "Penghemat token", route: "/settings/p/:k/tokens", path: "/settings/p/KSR/tokens",
  render: () => {
    const SUBS = AG.SUBS_M7, s = SUBS.tk.sec;
    const ring = AG.card({ title: "Ringkasan", desc: "Penghematan token 30 hari terakhir dan tren 8 minggu.", body: `<div class="alert gi-al ok">${ic("topology-star-3", "s14")}<span class="grow"><b>Code graph menyumbang penghematan terbesar.</b> 1,9 jt token dari 4,8 jt · agent memanggil ag_search dan ag_snippet, bukan membaca file utuh.</span>${btn("Lihat fitur", "sm", "", { "data-act": "tk-sec", "data-arg": "feat" })}</div>
      ${AG.stats([
        { label: "Token dihemat", value: "4,8 jt", icon: "leaf", tone: "ok", hint: "38% dari tanpa penghemat" },
        { label: "Biaya dihemat", value: "Rp1,2 jt", icon: "leaf", tone: "ok", hint: "30 hari terakhir" },
        { label: "Token terpakai", value: "7,9 jt", icon: "chart-bar", hint: "input + output + cache" },
        { label: "Perkiraan biaya", value: "Rp2,1 jt", icon: "coins", hint: "142 job · 4 client" },
      ])}
      <div class="tk-ch"><div class="row wrap"><b class="b5 grow">Token per minggu</b><div class="tk-leg"><span><i style="background:var(--accent)"></i>Terpakai</span><span><i style="background:var(--accent);opacity:.28"></i>Dihemat (perkiraan tanpa penghemat)</span></div></div>${weekChart()}</div>` });
    const feat = AG.card({ title: "Fitur penghemat", desc: "Fitur wajib selalu aktif. Fitur opsional bisa dimatikan per project. Gaya ringkas dan model diatur per persona.", pad: false,
      action: AG.seeAll("#/settings/personas", "Persona"),
      foot: AG.savebar("Simpan perubahan", { toast: "Pengaturan penghemat token disimpan" }),
      body: `<div class="tk-note">${ic("info-circle", "s14")}Tidak dipakai: proxy kompresi level API dan anggaran token per job.</div>${FEATS.map(([n, d, wajib, saved, on]) => `<div class="tk-feat"><div><div class="b5">${n}</div><div class="d">${d}</div></div>
        <span class="c2">${wajib ? pill("Wajib", "gray", "", "lock") : pill("Opsional", "")}</span>
        <span class="c3 sm ${saved ? "" : "mut"}">${saved ? `hemat <b class="tab-num">${String(saved).replace(".", ",")} jt</b>` : "belum aktif"}</span>
        <span>${wajib ? `<button type="button" class="sw on" role="switch" aria-checked="true" aria-label="${n} wajib aktif" aria-disabled="true" style="opacity:.55;cursor:not-allowed" data-toast="${n} wajib aktif dan tidak bisa dimatikan"></button>` : sw(on, n)}</span></div>`).join("")}` });
    const tick = AG.card({ title: "Pemakaian per tiket", desc: "Tiket dengan pemakaian terbesar bulan ini.", body: `<div class="tk-leg"><span><i style="background:var(--accent)"></i>Input</span><span><i style="background:var(--warn);opacity:.8"></i>Output</span><span><i style="background:var(--success);opacity:.55"></i>Cache read</span></div><div class="tblwrap">${ticketChart()}</div>` });
    const num = (x) => parseFloat(x.replace(",", ".")) * (x.includes("jt") ? 1000 : 1);
    const job = AG.card({ title: "Pemakaian per job", desc: "Dilaporkan client: Claude Code, Codex CLI, Gemini CLI.", body: AG.datatable({ id: "tk-jobs", search: "Cari job atau tiket…", pageSize: 10,
        filters: [{ k: "st", label: "Status", icon: "circle-check", options: ["jalan", "selesai", "gagal"] }, { k: "cl", label: "Client", icon: "terminal-2", options: ["Claude Code", "Codex CLI"] }],
        right: btn("Ekspor CSV", "", "download", { "data-toast": "pemakaian-token-KSR.csv diunduh" }),
        columns: [{ label: "Job" }, { label: "Tiket" }, { label: "Persona · client", hide: "sm" }, { label: "Input", align: "right" }, { label: "Output", align: "right", hide: "sm" }, { label: "Cache", align: "right", hide: "sm" }, { label: "Biaya", align: "right" }],
        rows: JOBS.map(([j, k, p, cl, i, o, c, rp, st]) => ({ sort: [j, k, cl, num(i), num(o), num(c), +rp.replace(/\D/g, "")], filter: { st, cl: cl.split(" · ")[0] }, text: `${j} ${k} ${cl}`,
          cells: [`<span class="row g4 nowrap"><span class="mono">${j}</span>${st === "jalan" ? `<span class="live busy"></span>` : st === "gagal" ? pill("gagal", "dng") : ""}</span>`, tref(k), `<span class="row" style="min-width:0">${av(p, 18)}<span class="sm mut trunc">${cl}</span></span>`, `<span class="tab-num">${i}</span>`, `<span class="tab-num">${o}</span>`, `<span class="tab-num">${c}</span>`, `<span class="tab-num b5">${rp}</span>`] })),
        rowGo: (i) => `#/t/${JOBS[i][1]}` }) });
    const body = `${head("Penghemat token", "Semua penghematan token untuk Kasir Online. Pemakaian dan perkiraan biaya berasal dari laporan client user lewat runner.")}
      ${AG.spWrap("tk", { nav: [
        { k: "ring", label: "Ringkasan", icon: "chart-bar" },
        { k: "feat", label: "Fitur penghemat", icon: "adjustments-horizontal", count: FEATS.length },
        { k: "tick", label: "Per tiket", icon: "ticket" },
        { k: "job", label: "Per job", icon: "terminal-2", count: JOBS.length },
      ], body: AG.panel("ring", ring, s === "ring") + AG.panel("feat", feat, s === "feat") + AG.panel("tick", tick, s === "tick") + AG.panel("job", job, s === "job") })}`;
    return AG.shell({ nav: "settings", project: K, crumbs: ["Pengaturan", "Kasir Online", "Penghemat token"], body, width: "mid" });
  } });
AG.action("tk-sec", (el, arg) => { AG.SUBS_M7.tk.sec = arg; AG.rerender(); });

/* ====================== Pratinjau mockup ====================== */
const MK = { dev: "web", ver: "v2", cmp: false };
const mockHtml = (ver, qa) => {
  const v2 = ver === "v2";
  const items = [["Kopi Susu Gula Aren 1L", 2, 38000, v2 ? 10 : 0], ["Roti Tawar Gandum", 1, 18500, 0], ["Susu UHT Cokelat 250ml", 4, 6500, v2 ? 5 : 0]];
  const sub = items.reduce((s, [, q, h]) => s + q * h, 0), disc = items.reduce((s, [, q, h, d]) => s + Math.round((q * h * d) / 100), 0);
  const rp = (n) => "Rp" + n.toLocaleString("id-ID");
  const prods = [["Kopi Susu Gula Aren 1L", 38000, "#c08457"], ["Roti Tawar Gandum", 18500, "#e3b778"], ["Susu UHT Cokelat", 6500, "#8b5e3c"], ["Air Mineral 600ml", 4000, "#7cc4e8"], ["Mi Goreng Instan", 3500, "#f2c14e"], ["Teh Botol 450ml", 6000, "#c0392b"], ["Gula Pasir 1kg", 17000, "#ececec"], ["Minyak Goreng 2L", 36000, "#f4d35e"]];
  return `<!doctype html><html lang="id"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>
*{box-sizing:border-box}body{margin:0;font-family:system-ui,-apple-system,Segoe UI,sans-serif;font-size:13px;color:#1f2430;background:#f3f4f7}
.top{height:52px;background:#fff;border-bottom:1px solid #e6e7ec;display:flex;align-items:center;gap:12px;padding:0 16px}.logo{width:28px;height:28px;border-radius:8px;background:#7c3aed;color:#fff;display:grid;place-items:center;font-weight:700}
.srch{flex:1;max-width:380px;height:34px;border:1px solid #dcdde3;border-radius:8px;display:flex;align-items:center;padding:0 10px;color:#8a8d99}
.who{margin-left:auto;font-size:12px;color:#5b5f6b}.wrap{display:grid;grid-template-columns:1fr 360px;gap:16px;padding:16px;height:calc(100vh - 52px)}
.cats{display:flex;gap:6px;margin-bottom:12px}.cat{height:28px;padding:0 12px;border-radius:14px;background:#fff;border:1px solid #e1e2e8;display:inline-flex;align-items:center}.cat.on{background:#1f2430;color:#fff;border-color:#1f2430}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:10px}.p{background:#fff;border:1px solid #e6e7ec;border-radius:10px;padding:10px;cursor:pointer}.p:hover{border-color:#7c3aed}
.img{height:70px;border-radius:7px;margin-bottom:8px;opacity:.85}.pn{font-weight:500;line-height:1.3;min-height:34px}.pp{color:#5b5f6b;margin-top:4px}
.cart{background:#fff;border:1px solid #e6e7ec;border-radius:12px;display:flex;flex-direction:column;min-height:0}.ch{padding:14px 16px;border-bottom:1px solid #eef0f3;font-weight:600;display:flex;justify-content:space-between}
.items{flex:1;overflow:auto}.it{padding:12px 16px;border-bottom:1px solid #f1f2f5}.ir{display:flex;justify-content:space-between;gap:8px}.iq{color:#6b6f7b;font-size:12px;margin-top:2px}
.dc{margin-top:8px;display:flex;align-items:center;gap:6px}.badge{background:#ffe4e8;color:#be123c;border-radius:6px;padding:2px 7px;font-size:11.5px;font-weight:600${qa ? ";white-space:nowrap;overflow:hidden;max-width:58px;outline:2px solid #ef4444;outline-offset:2px" : ""}}
.dbtn{border:1px dashed #c9cbd3;border-radius:6px;padding:2px 8px;font-size:11.5px;color:#5b5f6b;background:none}
.sum{padding:12px 16px;border-top:1px solid #eef0f3;display:grid;gap:6px}.sr{display:flex;justify-content:space-between;color:#5b5f6b}.sr.t{color:#1f2430;font-weight:700;font-size:16px}.sr.d{color:#be123c}
.pay{display:grid;grid-template-columns:1fr 1fr;gap:8px;padding:0 16px 16px}.b{height:44px;border-radius:9px;border:1px solid #d9dae2;background:#fff;font-weight:600;font-size:14px}.b.q{background:${qa ? "#4f46e5" : "#7c3aed"};border-color:transparent;color:#fff${qa ? ";outline:2px solid #ef4444;outline-offset:2px" : ""}}
.qr{display:none;position:fixed;inset:0;background:rgba(15,15,25,.45);place-items:center}.qr.on{display:grid}.qb{background:#fff;border-radius:14px;padding:20px;text-align:center;width:260px}.qc{width:160px;height:160px;margin:12px auto;background:repeating-linear-gradient(90deg,#111 0 6px,#fff 6px 12px),#fff;background-blend-mode:difference;border:8px solid #fff;outline:1px solid #ddd}
.wrap>*{min-height:0}.wrap>div:first-child{overflow:auto}@media(max-width:640px){.wrap{grid-template-columns:1fr;height:auto}.cart{order:-1}.srch{display:none}.grid{grid-template-columns:repeat(2,1fr)}.cats{overflow:auto}.img{height:52px}}
</style></head><body>
<div class="top"><div class="logo">K</div><b>Kasir</b><div class="srch">Cari produk atau scan barcode</div><div class="who">Toko Sumber Rejeki · Kasir: Dewi</div></div>
<div class="wrap"><div><div class="cats"><span class="cat on">Semua</span><span class="cat">Minuman</span><span class="cat">Roti</span><span class="cat">Sembako</span></div>
<div class="grid">${prods.map(([n, h, c]) => `<div class="p"><div class="img" style="background:${c}"></div><div class="pn">${n}</div><div class="pp">${rp(h)}</div></div>`).join("")}</div></div>
<div class="cart"><div class="ch"><span>Keranjang</span><span style="color:#6b6f7b;font-weight:400">${items.length} item</span></div><div class="items">
${items.map(([n, q, h, d]) => `<div class="it"><div class="ir"><b style="font-weight:500">${n}</b><span>${rp(q * h - Math.round((q * h * d) / 100))}</span></div><div class="iq">${q} × ${rp(h)}${d ? ` · <s>${rp(q * h)}</s>` : ""}</div>${v2 ? `<div class="dc">${d ? `<span class="badge">Diskon ${d}%</span>` : ""}<button class="dbtn">${d ? "Ubah" : "+ Diskon item"}</button></div>` : ""}</div>`).join("")}
</div><div class="sum"><div class="sr"><span>Subtotal</span><span>${rp(sub)}</span></div>${v2 ? `<div class="sr d"><span>Diskon item</span><span>−${rp(disc)}</span></div>` : `<div class="sr"><span>Diskon transaksi</span><span>Rp0</span></div>`}<div class="sr t"><span>Total</span><span>${rp(Math.floor((sub - disc) / 100) * 100)}</span></div></div>
<div class="pay"><button class="b">Tunai</button><button class="b q" onclick="document.getElementById('qr').classList.add('on')">Bayar QRIS</button></div></div></div>
<div class="qr" id="qr" onclick="this.classList.remove('on')"><div class="qb"><b>Scan untuk membayar</b><div class="qc"></div><div style="color:#5b5f6b">${rp(Math.floor((sub - disc) / 100) * 100)} · berlaku 5 menit</div></div></div>
</body></html>`;
};
const frame = (dev, ver, qa = false) => `<div class="mk-frame ${dev === "mob" ? "mob" : ""}">
  <div class="mk-bar"><span class="mk-dots"><i></i><i></i><i></i></span><span class="mk-url">${ic("lock", "s12")}<span class="trunc">ksr-28.agentgate-preview.app${qa ? "/qa/build-146" : "/" + ver}</span></span>${dev === "web" ? pill("sandbox", "gray", "", "shield-lock") : ""}</div>
  <iframe title="${qa ? "Screenshot QA" : "Mockup " + ver}" sandbox="allow-scripts" referrerpolicy="no-referrer" loading="lazy" srcdoc="${esc(mockHtml(ver, qa))}"></iframe></div>`;
const thumb = (dev, ver) => `<svg viewBox="0 0 160 74" aria-hidden="true">${dev === "web"
  ? `<rect x="8" y="8" width="144" height="8" rx="2" fill="var(--line2)"/>${[0, 1, 2, 3].map((i) => `<rect x="${8 + i * 26}" y="22" width="22" height="20" rx="3" fill="var(--input)"/><rect x="${8 + i * 26}" y="46" width="22" height="20" rx="3" fill="var(--input)"/>`).join("")}<rect x="112" y="22" width="40" height="44" rx="3" fill="var(--raised)" stroke="var(--line2)"/>${ver === "v2" ? `<rect x="116" y="32" width="14" height="4" rx="2" fill="#e11d48" opacity=".6"/><rect x="116" y="44" width="14" height="4" rx="2" fill="#e11d48" opacity=".6"/>` : ""}<rect x="116" y="58" width="32" height="5" rx="2" fill="#7c3aed"/>`
  : `<rect x="60" y="4" width="40" height="66" rx="6" fill="var(--raised)" stroke="var(--line2)"/><rect x="64" y="10" width="32" height="5" rx="2" fill="var(--line2)"/>${[0, 1].map((i) => `<rect x="${64 + i * 17}" y="19" width="15" height="14" rx="2" fill="var(--input)"/>`).join("")}<rect x="64" y="37" width="32" height="20" rx="2" fill="var(--hover)" stroke="var(--line2)"/>${ver === "v2" ? `<rect x="67" y="41" width="10" height="3" rx="1.5" fill="#e11d48" opacity=".6"/>` : ""}<rect x="64" y="60" width="32" height="5" rx="2" fill="#7c3aed"/>`}</svg>`;

AG.action("mk-set", (el, arg) => { arg.split(",").forEach((a) => { const [k, v] = a.split(":"); MK[k] = k === "cmp" ? v === "1" : v; }); AG.rerender(); });
AG.dialog("mk-revise", () => ({ icon: "pencil", title: "Minta revisi mockup", desc: "Agent UI/UX membuat v3 dengan catatan Anda.",
  body: AG.fld("Catatan", AG.inp("Label diskon jangan terpotong di layar mobile. Tampilkan batas diskon maksimal dari KSR-35.", { area: true })),
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Kirim ke agent", "pri", "send", { "data-close": true, "data-toast": "Revisi dikirim · Agent UI/UX mulai membuat v3" })}` }));

AG.screen({ m: "M10", g: "Mockup", n: "Pratinjau mockup HTML", route: "/p/:k/mockup/:id", path: "/p/KSR/mockup/KSR-28",
  render: ({ id }) => {
    const t = AG.ticket(id);
    const stage = MK.cmp
      ? `<div class="alert gray" style="max-width:804px;width:100%">${ic("info-circle", "s14")}<span>Agent QA membandingkan screenshot build dengan mockup ini secara otomatis. Pratinjau memakai build preview PR #146. <b>2 perbedaan</b> ditandai merah.</span></div>
         <div class="mk-cmp"><div><div class="mk-cap">${ic("layout-sidebar", "s14")}Mockup ${MK.ver} · acuan</div>${frame("mob", MK.ver)}</div><div><div class="mk-cap">${ic("test-pipe", "s14")}Screenshot QA · build PR #146 · 12 Nov 14.20</div>${frame("mob", MK.ver, true)}</div></div>
         <div class="box" style="max-width:804px;width:100%">${[["Label “Diskon 10%” terpotong di lebar 390px", "dng"], ["Warna tombol Bayar QRIS #4f46e5, mockup memakai #7c3aed", "warn"]].map(([x, tone]) => `<div class="lrow">${ic("alert-triangle", `s14 text-${tone}`)}<span class="grow">${x}</span>${btn("Buat catatan", "sm", "", { "data-toast": "Catatan ditambahkan ke KSR-28" })}</div>`).join("")}</div>`
      : `${frame(MK.dev === "mob" ? "mob" : "web", MK.ver)}
         <div style="width:100%;max-width:1100px"><div class="mk-cap">${ic("photo", "s14")}Screenshot otomatis · dibuat runner saat mockup diunggah</div>
         <div class="mk-thumbs">${[["v2", "web", "v2 · Web · 1440×900"], ["v2", "mob", "v2 · Mobile · 390×844"], ["v1", "web", "v1 · Web · 1440×900"], ["v1", "mob", "v1 · Mobile · 390×844"]].map(([v, d, l]) => `<button class="mk-th ${MK.ver === v && MK.dev === d ? "on" : ""}" data-act="mk-set" data-arg="ver:${v},dev:${d}">${thumb(d, v)}<span>${l}</span></button>`).join("")}</div></div>`;
    const body = `${head(`Mockup ${t.k}`, `${esc(t.t)} · dibuat Agent UI/UX di stage Desain UI. Tinjau lalu approve atau minta revisi.`)}
      <nav class="tabs" aria-label="Mode pratinjau"><button class="${MK.cmp ? "" : "on"}" data-act="mk-set" data-arg="cmp:0">${ic("eye", "s14")}Pratinjau</button><button class="${MK.cmp ? "on" : ""}" data-act="mk-set" data-arg="cmp:1">${ic("arrows-exchange", "s14")}Bandingkan dengan QA <span class="n">2</span></button></nav>
      <div class="mk">
      <div class="box mk-main">
        <div class="mk-tool">
          ${MK.cmp ? `<span class="mut sm row g4">${ic("device-mobile", "s14")}Dibandingkan di lebar mobile 390px</span>` : `<div class="seg" aria-label="Perangkat"><button class="${MK.dev === "web" ? "on" : ""}" data-act="mk-set" data-arg="dev:web">${ic("device-desktop", "s14")}Web</button><button class="${MK.dev === "mob" ? "on" : ""}" data-act="mk-set" data-arg="dev:mob">${ic("device-mobile", "s14")}Mobile</button></div>`}
          <div class="seg" aria-label="Versi"><button class="${MK.ver === "v1" ? "on" : ""}" data-act="mk-set" data-arg="ver:v1">v1</button><button class="${MK.ver === "v2" ? "on" : ""}" data-act="mk-set" data-arg="ver:v2">v2 · terbaru</button></div>
          <span class="grow"></span>
          ${btn("", "ghost icon", "external-link", { "aria-label": "Buka di tab baru", title: "Buka di tab baru", "data-toast": "Membuka ksr-28.agentgate-preview.app di tab baru" })}
        </div>
        <div class="mk-stage">${stage}</div></div>
      <aside class="mk-side" aria-label="Info mockup">${AG.card({ cls: "mk-card", title: `<span class="row g8">${av("uiux", 22)}Mockup ${MK.ver}</span>`, desc: `Agent UI/UX · ${MK.ver === "v2" ? "12 Nov 09.12" : "11 Nov 16.40"}`, action: AG.statePill(t.state),
        foot: `<div class="mk-foot">${btn("Minta revisi", "", "pencil", { "data-dialog": "mk-revise" })}${btn("Approve mockup", "pri", "check", { "data-toast": "Mockup v2 disetujui · KSR-28 lanjut ke Development" })}</div>`,
        body: `<div class="alert gray">${ic("shield-lock", "s14")}<span class="sm">Mockup berjalan di iframe sandbox pada domain terpisah <span class="mono">*.agentgate-preview.app</span>, tanpa akses ke sesi atau data Agent Gate.</span></div>
        <dl class="props"><dt>Tiket</dt><dd class="row" style="min-width:0">${tref(t.k)}<span class="trunc sm">${t.t}</span></dd><dt>Stage</dt><dd class="row g4">${AG.stageIcon(t.stage)}${AG.stage(t.stage).n}</dd><dt>Jenis project</dt><dd>Web + mobile (tablet kasir)</dd><dt>Acuan</dt><dd><a class="link" href="#/p/${K}/knowledge/komponen-ui">Komponen UI</a></dd></dl>
        <div><h3>Gate stage</h3><div class="col g4">${AG.stage("design").gate.map((g) => `<span class="row sm">${ic("circle-check", "s14 text-ok")}${g}</span>`).join("")}</div></div>
        <div><h3>Perubahan v2</h3><ul class="sm" style="margin:0;padding-left:18px;display:grid;gap:4px;color:var(--fg2)"><li>Diskon per item dengan badge persen</li><li>Ringkasan “Diskon item” di total</li><li>Tombol “+ Diskon item” di tiap baris</li></ul></div>` })}
      </aside></div>`;
    return AG.shell({ project: K, crumbs: [AG.projectCrumb(K)[0], tref(id), "Mockup"], body, width: "wide" });
  } });
})();
