/* M10 AI Office: kantor isometrik dengan tiga gaya seni.
   Satu mesin denah, data, dan interaksi; gaya hanya mengganti palet dan aset.
   A1 Vektor lembut · A2 Pixel art · A3 Malam hangat. Pilihan disimpan di AG.UI.office.
   Prefix kelas: ofx- (layar), ofg- (layar opsi gaya). */
(() => {
const { ic, av, btn, pill, esc, tref } = AG;
if (!AG.UI.office) AG.UI.office = "A1";

/* ====================================================================== */
/* CSS                                                                     */
/* ====================================================================== */
AG.css(`
.ofx { display: flex; flex-direction: column; min-height: 0; flex: 1; min-width: 0; }
@media (min-width: 1200px) { .ofx.page { flex: 0 0 auto; height: calc(100dvh - 48px); } html[data-shell-v="A"] .ofx.page { height: calc(100dvh - 66px); }
  .main:has(.pbar) .ofx.page { height: calc(100dvh - 95px); } html[data-shell-v="A"] .main:has(.pbar) .ofx.page { height: calc(100dvh - 113px); } }
.ofx-top { padding: 22px 24px 0; display: grid; gap: 14px; }
.ofx-top .tabs { margin-top: 0; }
.ofx-top .tabs .n { margin-left: 2px; }
.ofx-body { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr) 344px; grid-template-rows: minmax(0, 1fr); }
.ofx-stage { position: relative; overflow: hidden; min-width: 0; min-height: 0; touch-action: none; cursor: grab; outline: none; user-select: none; -webkit-user-select: none; background: var(--page); }
.ofx-stage.drag { cursor: grabbing; } .ofx-stage.hot { cursor: pointer; }
.ofx-stage:focus-visible { box-shadow: inset 0 0 0 2px var(--accent); }
.ofx-cv { position: absolute; inset: 0; width: 100%; height: 100%; display: block; }
.ofx-labels { position: absolute; inset: 0; pointer-events: none; overflow: hidden; }
.ofx-lb { position: absolute; left: 0; top: 0; pointer-events: auto; will-change: transform; padding: 0; border: 0; background: none; }
.ofx-lb > span { display: inline-flex; align-items: center; gap: 5px; height: 22px; padding: 0 8px 0 7px; border-radius: 11px; background: var(--raised); border: 1px solid var(--line2); box-shadow: 0 1px 3px rgb(16 16 24 / 0.1); font-size: 11.5px; font-weight: 500; color: var(--fg); white-space: nowrap; line-height: 1; }
.ofx-lb .d { width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0; }
.ofx-lb .k { display: inline-flex; align-items: center; gap: 3px; margin: 0 -4px 0 3px; padding: 0 7px 0 6px; height: 16px; border-radius: 8px; font-family: var(--mono); font-size: 10.5px; font-weight: 500; }
.ofx-lb .k .ic { width: 11px; height: 11px; stroke-width: 2.3; }
.ofx-lb.warn .k { background: var(--warn-soft); color: var(--warn); }
.ofx-lb.dng .k { background: var(--danger); color: #fff; }
.ofx-lb.ok .k { background: var(--success-soft); color: var(--success); }
.ofx-lb.gray .k { background: var(--hover); color: var(--muted); }
.ofx-lb.dng > span { border-color: color-mix(in srgb, var(--danger) 55%, var(--line2)); }
.ofx-lb.warn > span { border-color: color-mix(in srgb, var(--warn) 40%, var(--line2)); }
.ofx-lb:hover > span { border-color: var(--input); box-shadow: 0 2px 8px rgb(16 16 24 / 0.14); }
.ofx-lb.sel > span { border-color: var(--accent); box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent); }
.ofx-lb:focus-visible { outline: none; } .ofx-lb:focus-visible > span { box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 40%, transparent); }
.ofx-lb.cmp .nm { display: none; } .ofx-lb.cmp > span { padding: 0 6px; gap: 0; } .ofx-lb.cmp .k { margin: 0 0 0 4px; }
.ofx-lb.cmp.plain { display: none; }
.ofx.nonames .ofx-lb.plain:not(.sel) { display: none; }
.ofx-rl { position: absolute; left: 0; top: 0; pointer-events: auto; display: grid; gap: 1px; padding: 6px 10px 6px 9px; border-radius: 9px; background: var(--raised); border: 1px solid var(--line2); box-shadow: 0 1px 3px rgb(16 16 24 / 0.08); text-align: left; will-change: transform; max-width: 240px; }
.ofx-rl b { font-size: 12.5px; font-weight: 600; display: flex; align-items: center; gap: 6px; white-space: nowrap; color: var(--fg); }
.ofx-rl b i { width: 8px; height: 8px; border-radius: 3px; flex-shrink: 0; }
.ofx-rl small { font-size: 11px; color: var(--muted); white-space: nowrap; padding-left: 14px; }
.ofx-rl:hover { border-color: var(--input); } .ofx-rl.on { border-color: var(--accent); }
.ofx-rl.cmp small { display: none; }
@media (prefers-reduced-motion: no-preference) {
  .ofx-lb.warn > span, .ofx-lb.dng > span { animation: ofx-float 2.4s ease-in-out infinite; }
  .ofx-lb.dng > span::after { content: ""; position: absolute; inset: -1px; border-radius: 12px; border: 2px solid var(--danger); animation: ofx-ping 1.6s ease-out infinite; pointer-events: none; }
}
.ofx-lb > span { position: relative; }
@keyframes ofx-float { 50% { transform: translateY(-3px); } }
@keyframes ofx-ping { from { opacity: .7; transform: scale(1); } to { opacity: 0; transform: scale(1.18, 1.5); } }
.ofx-ov { position: absolute; z-index: 3; display: flex; gap: 8px; align-items: center; pointer-events: none; }
.ofx-ov > * { pointer-events: auto; }
.ofx-ov.tl { left: 14px; top: 14px; right: 200px; flex-wrap: wrap; }
.ofx-ov.tr { right: 14px; top: 14px; }
.ofx-ov.bl { left: 14px; bottom: 14px; align-items: flex-end; }
.ofx-ov.br { right: 14px; bottom: 14px; }
.ofx-card { background: var(--raised); border: 1px solid var(--line2); border-radius: 9px; box-shadow: 0 2px 8px rgb(16 16 24 / 0.08); }
.ofx-sum { display: inline-flex; gap: 14px; align-items: center; padding: 6px 12px; font-size: 12px; color: var(--muted); flex-wrap: wrap; }
.ofx-sum b { color: var(--fg); font-weight: 600; font-variant-numeric: tabular-nums; margin-right: 3px; }
.ofx-sum > span { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
.ofx-sum i { width: 7px; height: 7px; border-radius: 50%; display: inline-block; }
.ofx-zoom { display: inline-flex; align-items: stretch; overflow: hidden; height: 32px; }
.ofx-zoom button { min-width: 32px; display: inline-flex; align-items: center; justify-content: center; gap: 6px; color: var(--fg2); font-size: 12px; padding: 0 8px; font-variant-numeric: tabular-nums; }
.ofx-zoom button:hover { background: var(--hover); color: var(--fg); }
.ofx-zoom button + button { border-left: 1px solid var(--line); }
.ofx-zoom .pct { min-width: 50px; font-weight: 500; }
.ofx-tbtn { height: 32px; display: inline-flex; align-items: center; gap: 6px; padding: 0 10px; font-size: 12.5px; color: var(--fg2); font-weight: 500; }
.ofx-tbtn:hover { color: var(--fg); border-color: var(--input); }
.ofx-tbtn.on { color: var(--accent-fg); }
.ofx-tbtn .sw-dot { width: 14px; height: 14px; border-radius: 4px; flex-shrink: 0; }
.ofx-mm { display: block; width: 172px; height: 108px; cursor: pointer; border-radius: 9px; }
.ofx-hint { position: absolute; left: 50%; bottom: 16px; transform: translateX(-50%); z-index: 2; font-size: 12px; color: var(--muted); padding: 6px 11px; display: inline-flex; gap: 8px; align-items: center; white-space: nowrap; transition: opacity .4s; pointer-events: none; }
.ofx-hint kbd { font-family: var(--mono); font-size: 10.5px; border: 1px solid var(--line2); border-bottom-width: 2px; border-radius: 4px; padding: 0 4px; color: var(--fg2); background: var(--muted-bg); }
.ofx-hint.gone { opacity: 0; }
.ofx-side { border-left: 1px solid var(--line); background: var(--page); overflow-y: auto; min-width: 0; min-height: 0; }
.ofx-sp { padding: 14px 14px 20px; display: grid; gap: 12px; align-content: start; }
.ofx-cd .cx-h { padding: 12px 14px; } .ofx-cd .cx-h h2 { display: flex; align-items: center; gap: 8px; } .ofx-cd .cx-h p { font-size: 12.5px; }
.ofx-cd .cx-b { padding: 12px 14px; gap: 14px; }
.ofx-cd .ofx-todo:last-child { margin-bottom: 0; }
.ofx-cd .ofx-run { margin: 0 -6px; width: calc(100% + 12px); }
.ofx-cd .ofx-feed .fe:first-child { border-top: 0; padding-top: 0; } .ofx-cd .ofx-feed .fe:last-child { padding-bottom: 0; }
.ofx-cd .cx-f .ofx-acts { padding: 12px 14px; }
.ofx-dcard .cx-h h2 { font-size: 15px; } .ofx-dcard .cx-b { gap: 16px; }
.ofx-h { font-size: 12px; color: var(--muted); font-weight: 500; display: flex; align-items: center; gap: 6px; margin-bottom: 8px; }
.ofx-h .n { margin-left: auto; font-variant-numeric: tabular-nums; }
.ofx-todo { display: flex; align-items: center; gap: 10px; padding: 9px 10px; border: 1px solid var(--line); border-radius: 9px; background: var(--raised); width: 100%; text-align: left; margin-bottom: 6px; min-width: 0; }
.ofx-todo:hover { border-color: var(--input); box-shadow: var(--shadow-sm); }
.ofx-todo .tt { display: block; font-size: 12.5px; color: var(--fg2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 2px; }
.ofx-run { display: flex; align-items: center; gap: 10px; padding: 8px 8px; border-radius: 8px; width: 100%; text-align: left; min-width: 0; }
.ofx-run:hover { background: var(--hover); }
.ofx-run .s { font-size: 11.5px; color: var(--muted); display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ofx-run.off { opacity: .7; }
.ofx-det { display: grid; gap: 16px; align-content: start; }
.ofx-dh { display: flex; gap: 12px; align-items: center; }
.ofx-pv { width: 52px; height: 58px; border-radius: 12px; background: var(--muted-bg); border: 1px solid var(--line); flex-shrink: 0; display: block; }
.ofx-props { grid-template-columns: 74px minmax(0, 1fr); gap: 9px 10px; font-size: 12.5px; }
.ofx-log { max-height: 236px; overflow-y: auto; font-size: 11.5px; padding: 10px 12px; }
.ofx-log .new { animation: fade .3s ease; }
.ofx-acts { display: flex; gap: 8px; flex-wrap: wrap; }
.ofx-jobs { display: flex; gap: 6px; flex-wrap: wrap; } .ofx-jobs > a { padding: 2px 8px; border: 1px solid var(--line2); border-radius: 6px; }
.ofx-back { display: inline-flex; align-items: center; gap: 4px; color: var(--muted); font-size: 12.5px; font-weight: 500; } .ofx-back:hover { color: var(--fg); }
.ofx-feed .fe { align-items: flex-start; }
/* Gaya pixel: label kotak tajam */
.ofx[data-style="A2"] .ofx-lb > span, .ofx[data-style="A2"] .ofx-rl { border-radius: 0; border: 2px solid #2a2238; box-shadow: 3px 3px 0 rgb(42 34 56 / 0.28); font-family: var(--mono); font-size: 11px; }
.ofx[data-style="A2"] .ofx-lb > span { height: 22px; }
.ofx[data-style="A2"] .ofx-lb .k { border-radius: 0; height: 14px; }
.ofx[data-style="A2"] .ofx-lb .d { border-radius: 0; }
.ofx[data-style="A2"] .ofx-lb.sel > span { border-color: var(--accent); box-shadow: 3px 3px 0 color-mix(in srgb, var(--accent) 45%, transparent); }
.ofx[data-style="A2"] .ofx-rl b { font-family: var(--mono); font-size: 12px; }
.ofx[data-style="A2"] .ofx-lb.dng > span::after { border-radius: 0; }
/* Gaya malam: label kaca gelap, selalu gelap karena kanvasnya gelap */
.ofx[data-style="A3"] .ofx-stage { background: #0b0f22; }
.ofx[data-style="A3"] .ofx-lb > span, .ofx[data-style="A3"] .ofx-rl { background: rgb(22 26 48 / 0.86); border-color: rgb(255 255 255 / 0.13); color: #eef0ff; box-shadow: 0 2px 10px rgb(0 0 0 / 0.4); backdrop-filter: blur(4px); }
.ofx[data-style="A3"] .ofx-rl small { color: #a9aed0; }
.ofx[data-style="A3"] .ofx-rl b { color: #eef0ff; }
.ofx[data-style="A3"] .ofx-lb.warn .k { background: rgb(240 162 75 / 0.2); color: #ffc477; }
.ofx[data-style="A3"] .ofx-lb.ok .k { background: rgb(76 199 130 / 0.2); color: #7be0a6; }
.ofx[data-style="A3"] .ofx-lb.gray .k { background: rgb(255 255 255 / 0.08); color: #b8bcd8; }
.ofx[data-style="A3"] .ofx-lb.sel > span { border-color: #9a9aff; }
.ofx[data-style="A3"] .ofx-hint { background: rgb(22 26 48 / 0.86); border-color: rgb(255 255 255 / 0.13); color: #b8bcd8; }
.ofx[data-style="A3"] .ofx-hint kbd { background: rgb(255 255 255 / 0.06); border-color: rgb(255 255 255 / 0.18); color: #dfe2ff; }
@media (max-width: 1199px) { .ofx-body { grid-template-columns: 1fr; } .ofx-side { border-left: 0; border-top: 1px solid var(--line); overflow: visible; } .ofx-stage { height: 560px; } .ofx.page { height: auto; } }
@media (max-width: 899px) {
  .ofx-top { padding: 16px 16px 0; } .ofx-stage { height: 460px; }
  .ofx-ov.tl { right: 14px; } .ofx-sum { gap: 10px; } .ofx-sum .hide-sm { display: none; }
  .ofx-ov.tr { top: auto; bottom: 56px; } .ofx-ov.bl { display: none; } .ofx-hint { display: none; }
  .ofx-sp { padding: 16px; }
}
.ofx.embed { border: 1px solid var(--line2); border-radius: 12px; overflow: hidden; background: var(--bg); box-shadow: var(--shadow-md); }
.ofx.embed .ofx-body { height: 640px; }
@media (max-width: 1199px) { .ofx.embed .ofx-body { height: auto; } }
.ofx.embed .ofx-stage { border-radius: 0; }

/* ---------- Layar opsi gaya ---------- */
.ofg-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; align-items: stretch; }
@media (max-width: 1099px) { .ofg-grid { grid-template-columns: 1fr; } }
.ofg-card { text-align: left; display: grid; grid-template-rows: auto auto 1fr auto; gap: 12px; padding: 12px 12px 14px; border-radius: 12px; background: var(--raised); border: 1px solid var(--line); min-width: 0; cursor: pointer; transition: border-color .15s, box-shadow .15s; }
.ofg-card:hover { border-color: var(--input); box-shadow: var(--shadow-sm); }
.ofg-card.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent), 0 6px 18px rgb(16 16 24 / 0.08); }
.ofg-thumb { display: block; width: 100%; aspect-ratio: 16 / 9; border-radius: 8px; border: 1px solid var(--line); background: var(--page); }
.ofg-h { display: flex; align-items: center; gap: 8px; padding: 0 2px; }
.ofg-l { width: 26px; height: 22px; border-radius: 6px; background: var(--hover); display: inline-flex; align-items: center; justify-content: center; font-weight: 600; font-size: 11.5px; color: var(--fg2); flex-shrink: 0; }
.ofg-card.on .ofg-l { background: var(--accent); color: var(--on-accent); }
.ofg-card p { color: var(--muted); font-size: 13px; margin: 0; padding: 0 2px; }
.ofg-pc { display: grid; gap: 5px; font-size: 12.5px; padding: 0 2px; align-content: start; }
.ofg-pc .row { align-items: flex-start; } .ofg-pc .ic { margin-top: 2px; flex-shrink: 0; }
.ofg-foot { display: flex; align-items: center; gap: 8px; padding: 0 2px; }
.ofg-live .cx-b > .ofx.embed { border: 0; border-radius: 0 0 10px 10px; box-shadow: none; }
`);

/* ====================================================================== */
/* Warna                                                                   */
/* ====================================================================== */
const h2r = (h) => { h = h.replace("#", ""); if (h.length === 3) h = [...h].map((c) => c + c).join(""); const n = parseInt(h, 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; };
const r2h = (a) => "#" + a.map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0")).join("");
const mix = (a, b, t) => { const A = h2r(a), B = h2r(b); return r2h(A.map((v, i) => v + (B[i] - v) * t)); };
const shade = (c, f) => (f < 0 ? mix(c, "#000000", -f) : mix(c, "#ffffff", f));
const rgba = (h, a) => { const [r, g, b] = h2r(h); return `rgba(${r},${g},${b},${a})`; };

/* Palet dasar (A1 siang). Nilai = warna permukaan atas; sisi kiri dan kanan diturunkan. */
const PAL1 = {
  bgA: "#f1f4fb", bgB: "#e2e8f4", bgDA: "#191b24", bgDB: "#121319",
  slab: "#d6dbe8", wall: "#f6f2ec", wallL: "#efe9e1", wallCap: "#ddd5ca", base: "#d2c8bb",
  winFrame: "#ffffff", sky1: "#9fd2ff", sky2: "#e3f3ff", cloud: "#ffffff",
  glass: "#bfe4f5", glassFrame: "#b7c0cf", half: "#ebe5dd",
  fKSR: "#ece9f6", fKSR2: "#e4e0f1", fMEET: "#cfe6ea", fMEET2: "#b9d9de", fCOR: "#e9d2b2", fCOR2: "#e0c6a2",
  fGDG: "#e0e6ee", fGDG2: "#d5dde8", fPPL: "#f4e9d6", fPPL2: "#ede0c8", fPAN: "#f6efe2", fPAN2: "#cfe3dc",
  grout: "#000000",
  desk: "#fbfaf7", deskLeg: "#a7afbf", mon: "#3b404d", screen: "#1f2742", screenHi: "#8fc0ff", screenOff: "#454b59", kb: "#e4e7ee",
  chair: "#56627f", chairLeg: "#3d4558", wood: "#d6a676", woodD: "#b88454",
  shelf: "#cfa173", board: "#ffffff", boardFrame: "#c5cbd6",
  sofa: "#7aa5cc", cushion: "#f4c98d", metal: "#c9ced6", fridge: "#f0f3f7", counter: "#fbfaf7", counterTop: "#d7c0a2",
  pot: "#ec9f7a", potB: "#f2f0ea", leaf1: "#6cc286", leaf2: "#4ea56c", leaf3: "#9adbab",
  rug: "#efd9cc", rug2: "#e5c6b3", rugB: "#dfe6f7", rugB2: "#cfd8f0",
  mug: "#ffffff", cooler: "#eaf0f6", water: "#9ad4f6", beanbag: "#f3a35f", beanbag2: "#8fb7e8", cardboard: "#d9aa6f", pallet: "#c39466",
  tv: "#2b2f3a", tvScreen: "#24304f", door: "#c99a70", doorFrame: "#ffffff", lamp: "#ffd98a", lampShade: "#f3f0e8", printer: "#e9ecf1",
  pants: "#33395a", shoes: "#3a3030", shell: "#f5f6fa", shellD: "#dfe3ec", eye: "#ffffff",
};
/* A3: malam. Turunan A1 yang digelapkan dan dibirukan, ditambah sumber cahaya. */
const NIGHT = "#141a35";
const nightify = (c) => mix(shade(c, -0.3), "#1c2652", 0.55);
const PAL3 = Object.fromEntries(Object.entries(PAL1).map(([k, v]) => [k, nightify(v)]));
Object.assign(PAL3, {
  bgA: "#151b3a", bgB: "#0a0d1e", bgDA: "#151b3a", bgDB: "#0a0d1e",
  sky1: "#0b1030", sky2: "#25305e", cloud: "#2c3768", screen: "#14203f", screenHi: "#9fd0ff", lamp: "#ffd27a",
  water: "#3d6f9a", eye: "#e8fbff", shell: mix("#f5f6fa", NIGHT, 0.45), shellD: mix("#dfe3ec", NIGHT, 0.5), tvScreen: "#1d2d5c",
});
/* A2: pixel art. Palet terbatas, lebih jenuh, garis luar gelap. */
const PAL2 = {
  bgA: "#cfdbe8", bgB: "#c3d1e0", bgDA: "#1a1f33", bgDB: "#141829",
  slab: "#7d86a8", wall: "#e9eef5", wallL: "#dfe6ef", wallCap: "#9aa6bf", base: "#7e8aa6",
  winFrame: "#ffffff", sky1: "#6fc3f7", sky2: "#b9e6ff", cloud: "#ffffff",
  glass: "#a9dcf2", glassFrame: "#8793ad", half: "#dcd4c6",
  fKSR: "#d4d9ea", fKSR2: "#c7cde2", fMEET: "#5fa8b8", fMEET2: "#4f93a3", fCOR: "#c98e5a", fCOR2: "#b97d4b",
  fGDG: "#bfc8d4", fGDG2: "#b1bbca", fPPL: "#ecd29b", fPPL2: "#e2c486", fPAN: "#f3ebdb", fPAN2: "#6cb7a6",
  grout: "#000000",
  desk: "#f4f1ea", deskLeg: "#6d7690", mon: "#2f3445", screen: "#203058", screenHi: "#7fd0ff", screenOff: "#3d4357", kb: "#d9dce6",
  chair: "#3e4a74", chairLeg: "#2b3150", wood: "#c07a43", woodD: "#9c5f30",
  shelf: "#b9783f", board: "#ffffff", boardFrame: "#8793ad",
  sofa: "#d9534f", cushion: "#ffd166", metal: "#b4bccb", fridge: "#e9edf3", counter: "#f4f1ea", counterTop: "#c9a27a",
  pot: "#d9663b", potB: "#efe9dd", leaf1: "#5cb85c", leaf2: "#3f8f45", leaf3: "#8fd88a",
  rug: "#e7b9a6", rug2: "#d99f8a", rugB: "#b9c7f0", rugB2: "#a3b4e6",
  mug: "#ffffff", cooler: "#dfe8f2", water: "#5bb8f0", beanbag: "#f08c3a", beanbag2: "#5d8fd6", cardboard: "#d39a55", pallet: "#a87447",
  tv: "#242836", tvScreen: "#22305a", door: "#b97d4b", doorFrame: "#ffffff", lamp: "#ffd166", lampShade: "#f4f1ea", printer: "#dfe3ea",
  pants: "#2f3758", shoes: "#2a2238", shell: "#f2f4f8", shellD: "#c9cfdb", eye: "#ffffff", outline: "#2a2238",
};

const STYLES = {
  A1: { k: "A1", n: "Vektor lembut", sub: "Pastel, flat dengan bayangan halus, siang hari", pal: PAL1, edge: 0.55, shade: 1,
    d: "Ilustrasi vektor pastel dengan cahaya matahari dari jendela, bayangan lembut, dan sudut membulat. Paling tenang dan paling dekat dengan nuansa Linear.",
    plus: ["Paling rapi dan profesional, cocok untuk mode terang", "Detail tetap tajam di semua tingkat zoom"], minus: ["Di mode gelap kantornya tetap terang", "Kesan “game” paling sedikit"] },
  A2: { k: "A2", n: "Pixel art", sub: "Piksel tajam ala Habbo dan Gather", pal: PAL2, pixel: true, edge: 0.35, shade: 1.1,
    d: "Seluruh kantor dirender ke piksel besar dengan palet terbatas dan garis luar gelap, seperti Habbo atau Gather. Label ikut bergaya kotak.",
    plus: ["Paling berkarakter dan menyenangkan", "Palet terbatas membuat status mudah dibedakan"], minus: ["Detail kecil (teks di papan) hilang saat jauh", "Bisa terasa kurang serius untuk sebagian tim"] },
  A3: { k: "A3", n: "Malam hangat", sub: "Lampu meja dan layar memancarkan cahaya", pal: PAL3, glow: true, edge: 0.18, shade: 1.15,
    d: "Kantor di malam hari: layar dan lampu meja memancarkan cahaya hangat, jendela menampilkan langit malam. Cocok untuk mode gelap dan layar pemantau di dinding.",
    plus: ["Paling atmosferik; karakter yang bekerja langsung terlihat dari cahaya layar", "Nyaman di mode gelap"], minus: ["Kontras perabot lebih rendah", "Kurang cocok bila dicetak atau dipakai di mode terang"] },
};
const TONE_CACHE = {};
function tonesOf(S, c) {
  const key = S.k + c; let t = TONE_CACHE[key];
  if (!t) { t = TONE_CACHE[key] = { t: c, l: shade(c, -0.09 * S.shade), r: shade(c, -0.2 * S.shade) }; }
  return t;
}
/* Warna data (baju, kulit) mengikuti suasana gaya. */
const SC = {};
function scol(S, c) { if (!S.glow) return c; const k = c; return SC[k] || (SC[k] = mix(c, NIGHT, 0.42)); }
const mat = (S, k) => tonesOf(S, S.pal[k] || k);

/* Status: warna hex untuk kanvas dan nada untuk komponen. */
const OST = {
  jalan: ["Bekerja", "acc", "#5b5bd6"], review: ["Butuh approval", "warn", "#e3962b"], keputusan: ["Butuh keputusan", "dng", "#e5484d"],
  rapat: ["Brainstorming", "info", "#3b82f6"], idle: ["Menganggur", "gray", "#a1a7b5"], pemilik: ["Online", "ok", "#30a46c"],
  ok: ["Disetujui", "ok", "#30a46c"], jeda: ["Dijeda", "gray", "#a1a7b5"], jalan2: ["Berjalan", "acc", "#5b5bd6"],
};

/* ====================================================================== */
/* Denah                                                                   */
/* ====================================================================== */
const T = 28, W = 27, D = 22, WH = 3;
const P = (x, y, z = 0) => [(x - y) * T, ((x + y) * T) / 2 - z * T];
const ROOMS = [
  { k: "KSR", n: "Kasir Online", c: "#7c3aed", r: [0, 0, 17, 11], top: true, fl: "tile", f: ["fKSR", "fKSR2"] },
  { k: "MEET", n: "Ruang rapat", c: "#0891b2", r: [17, 0, 27, 11], top: true, fl: "carpet", f: ["fMEET", "fMEET2"] },
  { k: "GDG", n: "Aplikasi Gudang", c: "#0ea5e9", r: [0, 13, 8.5, 22], fl: "big", f: ["fGDG", "fGDG2"] },
  { k: "PPL", n: "Portal Pelanggan", c: "#f59e0b", r: [8.5, 13, 17, 22], fl: "tile", f: ["fPPL", "fPPL2"] },
  { k: "PANTRY", n: "Pantry", c: "#a16207", r: [17, 13, 27, 22], fl: "checker", f: ["fPAN", "fPAN2"] },
];
const COR = [0, 11, 27, 13];
const room = (k) => ROOMS.find((r) => r.k === k);
const TABS = ["KSR", "GDG", "PPL", "MEET", "PANTRY"];
/* Celah pintu di partisi setengah tinggi (y = 11 dan y = 13). */
const DOORS = { KSR: [7.6, 9.2, 11], MEET: [20.6, 22.2, 11], GDG: [3.6, 5.2, 13], PPL: [12.2, 13.8, 13], PANTRY: [18.0, 19.6, 13] };
const doorX = (k) => (DOORS[k][0] + DOORS[k][1]) / 2;

/* Kursi dan titik berdiri. f = arah hadap. out = jalur dari kursi ke pintu ruangan. */
const SEATS = {
  k1: { x: 3.2, y: 3.35, f: "+y", room: "KSR", sit: 1, out: [[3.2, 2.35], [8.4, 2.35], [8.4, 11]] },
  k2: { x: 5.6, y: 3.35, f: "+y", room: "KSR", sit: 1, out: [[5.6, 2.35], [8.4, 2.35], [8.4, 11]] },
  k3: { x: 3.2, y: 6.65, f: "-y", room: "KSR", sit: 1, out: [[3.2, 8.5], [8.4, 8.5], [8.4, 11]] },
  k4: { x: 5.6, y: 6.65, f: "-y", room: "KSR", sit: 1, out: [[5.6, 8.5], [8.4, 8.5], [8.4, 11]] },
  k5: { x: 11.2, y: 3.35, f: "+y", room: "KSR", sit: 1, out: [[11.2, 2.35], [8.4, 2.35], [8.4, 11]] },
  k6: { x: 13.6, y: 3.35, f: "+y", room: "KSR", sit: 1, out: [[13.6, 2.35], [8.4, 2.35], [8.4, 11]] },
  k7: { x: 11.2, y: 6.65, f: "-y", room: "KSR", sit: 1, out: [[11.2, 8.5], [8.4, 8.5], [8.4, 11]] },
  k8: { x: 13.6, y: 6.65, f: "-y", room: "KSR", sit: 1, out: [[13.6, 8.5], [8.4, 8.5], [8.4, 11]] },
  k9: { x: 15.6, y: 6.6, f: "-x", room: "KSR", sit: 0, out: [[15.6, 8.5], [8.4, 8.5], [8.4, 11]], act: "tablet" },
  m1: { x: 21.0, y: 3.8, f: "+y", room: "MEET", sit: 1, out: [[21.0, 3.0], [19.0, 3.0], [19.0, 9.3], [21.4, 9.3], [21.4, 11]] },
  m2: { x: 22.6, y: 3.8, f: "+y", room: "MEET", sit: 1, out: [[22.6, 3.0], [19.0, 3.0], [19.0, 9.3], [21.4, 9.3], [21.4, 11]] },
  m3: { x: 24.2, y: 3.8, f: "+y", room: "MEET", sit: 1, out: [[24.2, 3.0], [26.0, 3.0], [26.0, 9.3], [21.4, 9.3], [21.4, 11]] },
  mp: { x: 19.0, y: 2.5, f: "+y", room: "MEET", sit: 0, out: [[19.0, 9.3], [21.4, 9.3], [21.4, 11]], act: "point" },
  p1: { x: 17.95, y: 17.0, f: "+x", room: "PANTRY", sit: 1, sofa: 1, out: [[18.8, 17.0], [18.8, 13]], act: "mug" },
  p2: { x: 21.0, y: 14.75, f: "+y", room: "PANTRY", sit: 0, out: [[18.8, 15.0], [18.8, 13]], act: "mug" },
  p3: { x: 23.75, y: 17.7, f: "+x", room: "PANTRY", sit: 0, out: [[22.6, 15.4], [18.8, 15.0], [18.8, 13]], act: "tablet" },
  p4: { x: 22.6, y: 16.0, f: "+y", room: "PANTRY", sit: 0, out: [[18.8, 15.0], [18.8, 13]] },
  p5: { x: 21.0, y: 20.0, f: "+y", room: "PANTRY", sit: 0, out: [[18.8, 19.2], [18.8, 13]] },
  p6: { x: 25.4, y: 19.4, f: "+y", room: "PANTRY", sit: 0, out: [[22.6, 15.4], [18.8, 15.0], [18.8, 13]] },
};
const KSR_FREE = ["k9", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8"];
const PANTRY_FREE = ["p4", "p5", "p6", "p2", "p3", "p1"];

/* ====================================================================== */
/* Data karakter                                                           */
/* ====================================================================== */
const LOOK = {
  dev: { acc: "headset" }, qa: { acc: "glasses" }, reviewer: { acc: "glasses" }, uiux: { acc: "beret" }, pm: { acc: "none" },
  analyst: { acc: "none" }, architect: { acc: "none" }, devops: { acc: "cap" }, writer: { acc: "beanie" },
};
const HUMAN = {
  DD: { skin: "#c88d62", hair: "#231a16", style: "bob", acc: "none" },
  QQ: { skin: "#e3b089", hair: "#d8899b", style: "hijab", acc: "glasses" },
  OO: { skin: "#f2c9a4", hair: "#6b3f28", style: "long", acc: "none" },
};
const CH0 = [
  { id: "rev", p: "reviewer", seat: "k1", tk: "KSR-24", st: "review", runner: "Laptop Dewi", since: "10.05", step: 4, steps: 4, doing: "Review AI selesai · 3 catatan" },
  { id: "ux", p: "uiux", seat: "k2", tk: "KSR-28", st: "review", runner: "Laptop Dewi", since: "kemarin", step: 5, steps: 5, doing: "Mockup v2 siap direview" },
  { id: "dev", p: "dev", seat: "k3", tk: "KSR-26", st: "jalan", runner: "Laptop Dewi", since: "08.42", step: 4, steps: 7, doing: "Menulis tes stok minus" },
  { id: "dewi", who: "DD", seat: "k4", st: "pemilik", runner: "Laptop Dewi", jobs: ["KSR-26", "KSR-24", "KSR-28"], doing: "Mengawasi 3 job di runner-nya" },
  { id: "pm", p: "pm", seat: "k5", tk: "KSR-35", st: "review", runner: "MacBook Olivia", since: "07.30", step: 3, steps: 3, doing: "PRD v1 siap direview" },
  { id: "qa2", p: "qa", seat: "k6", tk: "KSR-30", st: "keputusan", runner: "PC Qori", since: "09.55", step: 3, steps: 3, doing: "Gagal 3 kali · total struk selisih" },
  { id: "qa", p: "qa", seat: "k7", tk: "KSR-29", st: "jalan", runner: "PC Qori", since: "09.10", step: 14, steps: 18, doing: "Playwright skenario 14 dari 18" },
  { id: "qori", who: "QQ", seat: "k8", st: "pemilik", runner: "PC Qori", jobs: ["KSR-29", "KSR-30"], doing: "Mengawasi 2 job di runner-nya" },
  { id: "an", p: "analyst", seat: "mp", tk: "KSR-31", st: "rapat", runner: "MacBook Olivia", since: "10.20", doing: "Brainstorming laporan harian" },
  { id: "oli", who: "OO", seat: "m2", st: "rapat", runner: "MacBook Olivia", jobs: ["KSR-31", "KSR-35"], doing: "Brainstorming bersama Agent Analyst" },
  { id: "arch", p: "architect", seat: "p1", st: "idle", doing: "Menunggu tiket" },
  { id: "ops", p: "devops", seat: "p2", st: "idle", doing: "Menunggu tiket", wander: true },
  { id: "tw", p: "writer", seat: "p3", st: "idle", doing: "Menunggu tiket" },
];
const LOGS = {
  dev: ["Membaca ringkasan PRD + AC KSR-26", "ag_search \"validasi stok checkout\" → 6 simbol", "ag_trace checkStock → 3 pemanggil", "Menulis tes: stok_kurang_menolak_checkout", "rtk: go test ./internal/stock/... (1,2 rb → 140 token)", "✓ 14 tes lulus · 1 gagal: stok minus", "Memperbaiki guard qty <= 0", "✓ 15 tes lulus", "Commit: feat(stock): tolak checkout bila stok kurang", "Push ke branch KSR-26-validasi-stok"],
  qa: ["Membuka preview build PR #146", "Playwright: skenario 13/18 · cetak ulang dari riwayat", "Screenshot: riwayat-struk-mobile.png", "Membandingkan dengan mockup v1", "✓ skenario 14/18 lulus", "Playwright: skenario 15/18 · printer offline", "✓ skenario 15/18 lulus", "Playwright: skenario 16/18 · struk ganda"],
  qa2: ["✗ Percobaan 3: total struk selisih Rp100", "Struk: nama kasir kosong pada transaksi QRIS", "Gate “Hasil test lulus” tidak terpenuhi", "Menunggu keputusan manusia"],
  rev: ["Review PR #142 selesai", "3 catatan: 1 penting, 2 saran", "CI hijau · 212 tes", "Menunggu approval Code Review"],
  ux: ["Mockup v2 diunggah ke preview sandbox", "Screenshot web + mobile dibuat otomatis", "Menunggu review mockup"],
  pm: ["PRD v1 KSR-35 selesai · 7 acceptance criteria", "Menunggu review PRD"],
  an: ["Sesi brainstorming bersama Olivia", "Catatan: laporan dikirim jam 21.00", "Pertanyaan: format PDF atau ringkasan email?", "Olivia: ringkasan email, PDF terlampir", "Menyimpan catatan brainstorm KSR-31"],
  gen: ["Membaca tiket dan acceptance criteria", "ag_search konteks terkait → 4 simbol", "Menyusun rencana kerja 6 langkah", "Langkah 1: membaca kode terkait", "Langkah 2: menulis draf", "Langkah 3: menjalankan tes"],
};
const logKey = (c) => ({ dewi: "dev", qori: "qa", oli: "an" })[c.id] || (LOGS[c.id] && c.st !== "jalan2" ? c.id : c.logk || "gen");
const pname = (k) => AG.PEOPLE[k].name;
const oname = (c) => (c.who ? pname(c.who) : "Agent " + AG.PERSONAS[c.p].n);
const SHORT = { analyst: "Analyst", pm: "PM", uiux: "UI/UX", architect: "Architect", dev: "Developer", reviewer: "Reviewer", qa: "QA", devops: "DevOps", writer: "Writer" };
const oshort = (c) => c.alias || (c.who ? pname(c.who).split(" ")[0] : SHORT[c.p]);

let CH = [];
function resetChars() {
  CH = JSON.parse(JSON.stringify(CH0)).map((c, i) => {
    const s = SEATS[c.seat];
    const look = c.who ? { ...HUMAN[c.who], shirt: AG.PEOPLE[c.who].c } : { shirt: AG.PERSONAS[c.p].c, acc: LOOK[c.p].acc };
    return Object.assign(c, { x: s.x, y: s.y, look, ph: (i * 0.37) % 1, path: null, room: s.room });
  });
}
resetChars();
const chr = (id) => CH.find((c) => c.id === id);
const todo = () => CH.filter((c) => c.st === "review" || c.st === "keputusan");
const count = (st) => CH.filter((c) => c.st === st && c.p).length;
const roomOf = (c) => (c.path ? "COR" : SEATS[c.seat]?.room || c.room);

/* Status ruangan untuk label dan tab. */
function roomSub(k) {
  const here = CH.filter((c) => !c.path && SEATS[c.seat].room === k);
  if (k === "PANTRY") return here.length ? `${here.length} agent menganggur` : "Kosong";
  if (k === "MEET") return here.some((c) => c.st === "rapat") ? "KSR-31 · brainstorming" : "Kosong";
  if (!here.length) return "Tidak ada job aktif";
  const a = here.filter((c) => c.p).length, n = here.filter((c) => c.st === "review" || c.st === "keputusan").length;
  return `${a} agent${n ? ` · ${n} perlu tindakan` : ""}`;
}

/* ====================================================================== */
/* Mesin gambar                                                            */
/* ====================================================================== */
/* G: konteks gambar aktif { c, S, sc, ox, oy, t, dark } */
let G = null;
const path = (pts) => { const c = G.c; c.beginPath(); c.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < pts.length; i++) c.lineTo(pts[i][0], pts[i][1]); c.closePath(); };
const fillP = (pts, f) => { path(pts); G.c.fillStyle = f; G.c.fill(); };
const lw = (n = 1) => n / G.sc;
const quad = (x0, y0, x1, y1, z = 0) => [P(x0, y0, z), P(x1, y0, z), P(x1, y1, z), P(x0, y1, z)];
function line(a, b, col, w = 1) { const c = G.c; c.beginPath(); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); c.strokeStyle = col; c.lineWidth = lw(w); c.stroke(); }

/* Kotak isometrik dengan tiga nada: atas, kiri (+y), kanan (+x). */
function box(x, y, z, w, d, h, m, o = {}) {
  const t = typeof m === "string" ? mat(G.S, m) : m;
  const L = [P(x, y + d, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x, y + d, z + h)];
  const R = [P(x + w, y, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x + w, y, z + h)];
  const Tp = [P(x, y, z + h), P(x + w, y, z + h), P(x + w, y + d, z + h), P(x, y + d, z + h)];
  if (!o.noL) fillP(L, t.l);
  if (!o.noR) fillP(R, t.r);
  if (!o.noT) fillP(Tp, t.t);
  const S = G.S;
  if (S.edge && !o.noEdge && h > 0.015) {
    const c = G.c; c.beginPath(); c.moveTo(...Tp[3]); c.lineTo(...Tp[2]); c.lineTo(...Tp[1]);
    c.strokeStyle = `rgba(255,255,255,${S.edge * (o.edge ?? 1)})`; c.lineWidth = lw(S.pixel ? 1 : 1); c.stroke();
  }
  if (S.pixel && !o.noOut) {
    const c = G.c; path([P(x, y, z + h), P(x + w, y, z + h), P(x + w, y, z), P(x + w, y + d, z), P(x, y + d, z), P(x, y + d, z + h)]);
    c.strokeStyle = S.pal.outline; c.lineWidth = lw(1); c.stroke();
  }
}
/* Elips di lantai (lingkaran isometrik). r dalam satuan ubin. */
function ell(cx, cy, z, r, fill, k = 1) { const [X, Y] = P(cx, cy, z); const c = G.c; c.beginPath(); c.ellipse(X, Y, r * T * 1.414 * k, r * T * 0.707 * k, 0, 0, Math.PI * 2); c.fillStyle = fill; c.fill(); }
/* Silinder tegak (pot, gelas, kaki meja bundar). */
function cyl(cx, cy, z, r, h, m, o = {}) {
  const t = typeof m === "string" ? mat(G.S, m) : m;
  const [X, Y0] = P(cx, cy, z), Y1 = Y0 - h * T, rx = r * T * 1.414, ry = r * T * 0.707, c = G.c;
  const g = c.createLinearGradient(X - rx, 0, X + rx, 0); g.addColorStop(0, t.l); g.addColorStop(0.55, t.l); g.addColorStop(1, t.r);
  c.beginPath(); c.ellipse(X, Y0, rx, ry, 0, 0, Math.PI); c.lineTo(X - rx, Y1); c.ellipse(X, Y1, rx, ry, 0, Math.PI, 0, true); c.closePath();
  c.fillStyle = G.S.pixel ? t.l : g; c.fill();
  c.beginPath(); c.ellipse(X, Y1, rx, ry, 0, 0, Math.PI * 2); c.fillStyle = o.top || t.t; c.fill();
  if (G.S.pixel && !o.noOut) { c.beginPath(); c.ellipse(X, Y0, rx, ry, 0, 0, Math.PI); c.lineTo(X - rx, Y1); c.ellipse(X, Y1, rx, ry, 0, Math.PI, 0, true); c.closePath(); c.ellipse(X, Y1, rx, ry, 0, 0, Math.PI * 2); c.strokeStyle = G.S.pal.outline; c.lineWidth = lw(1); c.stroke(); }
}
/* Bayangan lembut di lantai. */
function softShadow(pts, a = 1, spread = 0.18) {
  const S = G.S;
  if (S.pixel) { fillP(pts, rgba("#1b1530", 0.16 * a)); return; }
  const base = (S.glow ? 0.34 : 0.11) * a;
  // blur murah: beberapa poligon yang diperbesar dari titik tengah dengan alfa kecil
  const cx = pts.reduce((s, p) => s + p[0], 0) / pts.length, cy = pts.reduce((s, p) => s + p[1], 0) / pts.length;
  const e = spread * T;
  for (let i = 3; i >= 0; i--) {
    const k = i * e / 3;
    fillP(pts.map(([x, y]) => { const dx = x - cx, dy = y - cy, l = Math.hypot(dx, dy) || 1; return [x + (dx / l) * k, y + (dy / l) * k * 0.6]; }), `rgba(20,24,50,${base / 3})`);
  }
}
function blob(X, Y, rx, ry, col) { const c = G.c; c.beginPath(); c.ellipse(X, Y, rx, ry, 0, 0, Math.PI * 2); c.fillStyle = col; c.fill(); }
/* Bayangan elips di bawah karakter / benda bundar. */
function footShadow(x, y, r, a = 1) {
  const [X, Y] = P(x, y), c = G.c, S = G.S, rx = r * T * 1.414, ry = r * T * 0.707;
  if (S.pixel) { blob(X, Y, rx, ry, rgba("#1b1530", 0.2 * a)); return; }
  const g = c.createRadialGradient(X, Y, 0, X, Y, rx);
  const k = S.glow ? 0.45 : 0.2;
  g.addColorStop(0, `rgba(20,24,50,${k * a})`); g.addColorStop(0.6, `rgba(20,24,50,${k * 0.6 * a})`); g.addColorStop(1, "rgba(20,24,50,0)");
  c.save(); c.translate(X, Y); c.scale(1, ry / rx); c.translate(-X, -Y); c.beginPath(); c.arc(X, Y, rx, 0, Math.PI * 2); c.fillStyle = g; c.fill(); c.restore();
}

/* Gambar pada bidang dinding: belakang (y = 0) dan kiri (x = 0). Satuan lokal = piksel dunia, v ke bawah. */
function onBack(x0, ztop, fn) { const [e, f] = P(x0, 0, ztop); const c = G.c; c.save(); c.transform(1, 0.5, 0, 1, e, f); fn(c); c.restore(); }
function onLeft(yEnd, ztop, fn) { const [e, f] = P(0, yEnd, ztop); const c = G.c; c.save(); c.transform(1, -0.5, 0, 1, e, f); fn(c); c.restore(); }
function rr(c, x, y, w, h, r) { c.beginPath(); c.roundRect ? c.roundRect(x, y, w, h, r) : c.rect(x, y, w, h); }

/* ---------- Ikon persona sebagai Path2D ---------- */
const ICP = {};
function iconPaths(n) {
  if (ICP[n]) return ICP[n];
  const s = (window.ICONS || {})[n] || ""; const out = [];
  s.replace(/d="([^"]+)"/g, (_, d) => { try { out.push(new Path2D(d)); } catch {} return ""; });
  return (ICP[n] = out);
}
function drawIcon(c, n, x, y, size, col, w = 2.4) {
  c.save(); c.translate(x, y); c.scale(size / 24, size / 24); c.strokeStyle = col; c.lineWidth = w; c.lineCap = "round"; c.lineJoin = "round"; c.fillStyle = "none";
  iconPaths(n).forEach((p) => c.stroke(p)); c.restore();
}

/* ====================================================================== */
/* Lantai, dinding, dan dekor dinding                                      */
/* ====================================================================== */
const hash = (a, b = 0) => { const s = Math.sin(a * 127.1 + b * 311.7) * 43758.5453; return s - Math.floor(s); };
function floorArea(x0, y0, x1, y1, kind, ka, kb) {
  const S = G.S, A = S.pal[ka], B = S.pal[kb], c = G.c;
  fillP(quad(x0, y0, x1, y1), A);
  const groutA = S.pixel ? 0.12 : S.glow ? 0.2 : 0.07;
  if (kind === "tile" || kind === "checker") {
    for (let i = Math.floor(x0); i < x1; i++) for (let j = Math.floor(y0); j < y1; j++) {
      if ((i + j) % 2 === 0) continue;
      fillP(quad(Math.max(i, x0), Math.max(j, y0), Math.min(i + 1, x1), Math.min(j + 1, y1)), B);
    }
  }
  if (kind === "tile" || kind === "checker" || kind === "big") {
    const s = kind === "big" ? 2 : 1;
    c.beginPath();
    for (let x = Math.ceil(x0 / s) * s; x < x1; x += s) { if (x <= x0) continue; const a = P(x, y0), b = P(x, y1); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); }
    for (let y = Math.ceil(y0 / s) * s; y < y1; y += s) { if (y <= y0) continue; const a = P(x0, y), b = P(x1, y); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); }
    c.strokeStyle = `rgba(0,0,0,${groutA})`; c.lineWidth = lw(1); c.stroke();
  } else if (kind === "wood") {
    const st = 0.5; let r = 0;
    for (let y = y0; y < y1 - 0.001; y += st, r++) {
      if (r % 2) fillP(quad(x0, y, x1, Math.min(y + st, y1)), B);
      c.beginPath();
      for (let x = x0 + 0.4 + hash(r) * 2.2; x < x1; x += 2.6) { const a = P(x, y), b = P(x, Math.min(y + st, y1)); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); }
      const a = P(x0, y), b = P(x1, y); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]);
      c.strokeStyle = `rgba(80,40,10,${S.pixel ? 0.22 : 0.1})`; c.lineWidth = lw(1); c.stroke();
    }
  } else if (kind === "carpet") {
    fillP(quad(x0 + 0.5, y0 + 0.5, x1 - 0.5, y1 - 0.5), B);
    fillP(quad(x0 + 0.75, y0 + 0.75, x1 - 0.75, y1 - 0.75), A);
  }
}
function rug(x0, y0, x1, y1, ka, kb) {
  const S = G.S;
  softShadow(quad(x0 + 0.05, y0 + 0.08, x1 + 0.05, y1 + 0.08), 0.35, 0.05);
  fillP(quad(x0, y0, x1, y1), S.pal[kb]);
  fillP(quad(x0 + 0.22, y0 + 0.22, x1 - 0.22, y1 - 0.22), S.pal[ka]);
  if (!S.pixel) { const c = G.c; path(quad(x0 + 0.38, y0 + 0.38, x1 - 0.38, y1 - 0.38)); c.strokeStyle = rgba(S.pal[kb], 0.9); c.lineWidth = lw(1.5); c.setLineDash([lw(4), lw(4)]); c.stroke(); c.setLineDash([]); }
}
/* Jendela di dinding belakang (along x) atau kiri (along y). */
function windowOn(side, a, b, z0 = 1.05, z1 = 2.55) {
  const S = G.S, c = G.c, Pw = (u, z) => (side === "back" ? P(u, 0, z) : P(0, u, z));
  const fr = S.pal.winFrame;
  // bingkai luar dan kaca
  fillP([Pw(a - 0.1, z0 - 0.1), Pw(b + 0.1, z0 - 0.1), Pw(b + 0.1, z1 + 0.1), Pw(a - 0.1, z1 + 0.1)], side === "back" ? shade(fr, -0.04) : shade(fr, -0.12));
  const glass = [Pw(a, z0), Pw(b, z0), Pw(b, z1), Pw(a, z1)];
  const top = Pw((a + b) / 2, z1), bot = Pw((a + b) / 2, z0);
  const g = c.createLinearGradient(top[0], top[1], bot[0], bot[1]); g.addColorStop(0, S.pal.sky1); g.addColorStop(1, S.pal.sky2);
  fillP(glass, S.pixel ? S.pal.sky1 : g);
  c.save(); path(glass); c.clip();
  if (S.glow) {
    // langit malam: bintang, bulan, siluet kota dengan jendela menyala
    for (let i = 0; i < 9; i++) { const u = a + hash(a + i, 3) * (b - a), z = z0 + 0.6 + hash(i, a) * (z1 - z0 - 0.6); const [X, Y] = Pw(u, z); blob(X, Y, 0.9, 0.9, "rgba(255,255,255,.75)"); }
    if (side === "back" && a > 20) { const [X, Y] = Pw(a + 0.7, z1 - 0.4); blob(X, Y, 6, 6, "#fff3c4"); blob(X + 2.5, Y - 1.5, 5.2, 5.2, S.pal.sky1); }
    for (let i = 0; i < 6; i++) { const u0 = a + i * (b - a) / 6, hgt = 0.3 + hash(i, b) * 0.55; fillP([Pw(u0, z0), Pw(u0 + (b - a) / 6 - 0.02, z0), Pw(u0 + (b - a) / 6 - 0.02, z0 + hgt), Pw(u0, z0 + hgt)], "#1a2147");
      for (let k = 0; k < 3; k++) if (hash(i, k + a) > 0.45) { const [X, Y] = Pw(u0 + 0.08 + k * 0.12, z0 + 0.12 + hash(k, i) * (hgt - 0.2)); c.fillStyle = "#ffcf6e"; c.fillRect(X - 1, Y - 1.4, 2, 2.4); } }
  } else {
    // awan dan bukit kota jauh
    const cl = S.pal.cloud;
    [[0.3, 0.72, 1], [0.7, 0.5, 0.8]].forEach(([fu, fz, s]) => { const [X, Y] = Pw(a + (b - a) * fu, z0 + (z1 - z0) * fz); blob(X, Y, 9 * s, 4 * s, cl); blob(X + 6 * s, Y - 2.5 * s, 7 * s, 4.5 * s, cl); blob(X - 6 * s, Y + 0.5, 6 * s, 3 * s, cl); });
    for (let i = 0; i < 5; i++) { const u0 = a + i * (b - a) / 5, hgt = 0.18 + hash(i, b) * 0.32; fillP([Pw(u0, z0), Pw(u0 + (b - a) / 5 - 0.03, z0), Pw(u0 + (b - a) / 5 - 0.03, z0 + hgt), Pw(u0, z0 + hgt)], S.pixel ? "#9cc4e4" : "rgba(120,160,200,.35)"); }
    if (!S.pixel) { const p1 = Pw(a + (b - a) * 0.15, z1), p2 = Pw(a + (b - a) * 0.45, z1), p3 = Pw(a + (b - a) * 0.05, z0), p4 = Pw(a - (b - a) * 0.25, z0); fillP([p1, p2, [p3[0] + (p2[0] - p1[0]), p3[1] + (p2[1] - p1[1])], p3], "rgba(255,255,255,.28)"); }
  }
  c.restore();
  // palang tengah dan ambang
  const mx = (a + b) / 2, mz = z0 + (z1 - z0) * 0.62;
  line(Pw(mx, z0), Pw(mx, z1), fr, 2.2); line(Pw(a, mz), Pw(b, mz), fr, 1.6);
  if (side === "back") box(a - 0.15, 0, z0 - 0.12, b - a + 0.3, 0.14, 0.06, "winFrame", { edge: 0.6 });
  else box(0, a - 0.15, z0 - 0.12, 0.14, b - a + 0.3, 0.06, "winFrame", { edge: 0.6 });
}
/* Papan kanban project KSR di dinding belakang. */
function kanban(x0, x1, z0, z1) {
  const S = G.S, w = (x1 - x0) * T, h = (z1 - z0) * T;
  onBack(x0, z1, (c) => {
    c.fillStyle = shade(S.pal.boardFrame, -0.1); rr(c, -3, -3, w + 6, h + 6, 3); c.fill();
    c.fillStyle = S.pal.board; rr(c, 0, 0, w, h, 2); c.fill();
    const cols = AG.STAGES.filter((s) => s.k !== "backlog"), cw = (w - 8) / cols.length;
    if (!S.pixel) { c.fillStyle = S.glow ? "#aeb3d6" : "#3b404d"; c.font = "600 5.6px Geist, system-ui, sans-serif"; c.fillText("Sprint 4 · Kasir Online", 5, 8); c.fillStyle = S.glow ? "#7d83a8" : "#8d93a3"; c.font = "500 4.4px Geist, system-ui, sans-serif"; c.fillText("13 tiket · 4 perlu review", w - 52, 8); }
    else { c.fillStyle = "#2a2238"; c.fillRect(5, 4, 34, 3); }
    cols.forEach((st, i) => {
      const cx = 4 + i * cw, col = st.c.startsWith("var") ? "#30a46c" : st.c;
      c.fillStyle = S.glow ? mix(col, NIGHT, 0.35) : col; c.fillRect(cx + 1, 11, cw - 3, 2.4);
      if (i) { c.fillStyle = S.glow ? "rgba(255,255,255,.06)" : "rgba(0,0,0,.07)"; c.fillRect(cx - 1, 11, 0.8, h - 14); }
      const list = AG.TICKETS.filter((t) => t.stage === st.k).slice(0, 4);
      list.forEach((t, j) => {
        const tone = { jalan: "#cfd0ff", review: "#ffe2b0", keputusan: "#ffc6c6", manusia: "#cfe3ff", siap: "#d5f2df", antre: "#eceef3" }[t.state] || "#eee";
        const yy = 16.5 + j * 6.6; c.fillStyle = S.glow ? mix(tone, NIGHT, 0.45) : tone; c.fillRect(cx + 1, yy, cw - 3, 6);
        c.fillStyle = S.glow ? "rgba(255,255,255,.18)" : "rgba(0,0,0,.18)"; c.fillRect(cx + 2.5, yy + 2, (cw - 8) * (0.5 + hash(i, j) * 0.4), 1);
      });
    });
    c.fillStyle = "rgba(0,0,0,.25)"; c.fillRect(w * 0.08, h + 3, 2, 3); c.fillRect(w * 0.92, h + 3, 2, 3);
  });
  // rak spidol
  box(x0 + 0.6, 0, z0 - 0.16, 1.4, 0.12, 0.05, "boardFrame");
}
/* Layar TV ruang rapat dengan catatan brainstorming. */
function tvWall(x0, x1, z0, z1) {
  const S = G.S, w = (x1 - x0) * T, h = (z1 - z0) * T, on = CH.some((c) => c.st === "rapat");
  onBack(x0, z1, (c) => {
    c.fillStyle = S.pal.tv; rr(c, -2, -2, w + 4, h + 4, 3); c.fill();
    const g = c.createLinearGradient(0, 0, w, h); g.addColorStop(0, on ? (S.glow ? "#2b4a8f" : "#3a5ea8") : S.pal.screenOff); g.addColorStop(1, on ? (S.glow ? "#1a2a5a" : "#24376a") : S.pal.screenOff);
    c.fillStyle = S.pixel ? (on ? "#33508f" : S.pal.screenOff) : g; c.fillRect(0, 0, w, h);
    if (!on) return;
    if (!S.pixel && G.sc > 0.5) { c.fillStyle = "#ffffff"; c.font = "600 6.4px Geist, system-ui, sans-serif"; c.fillText("Brainstorm · KSR-31", 5, 10); c.fillStyle = "rgba(255,255,255,.6)"; c.font = "500 4.6px Geist, system-ui, sans-serif"; c.fillText("Laporan penjualan harian", 5, 17); }
    [["#ffe28a", 0], ["#b9f0c9", 1], ["#ffc0d0", 2], ["#bfe0ff", 3]].forEach(([col, i]) => { const x = 6 + i * (w - 12) / 4, y = S.pixel ? 8 : 22; c.fillStyle = col; c.fillRect(x, y, (w - 12) / 4 - 4, h - y - 5); c.fillStyle = "rgba(0,0,0,.2)"; for (let k = 0; k < 2; k++) c.fillRect(x + 2, y + 3 + k * 4, (w - 12) / 4 - 9 - k * 3, 1); });
  });
}
function wallClock(x, z) {
  const S = G.S, t = G.t;
  onBack(x, z, (c) => {
    c.fillStyle = S.pal.boardFrame; c.beginPath(); c.arc(0, 0, 7.5, 0, 7); c.fill();
    c.fillStyle = S.pal.board; c.beginPath(); c.arc(0, 0, 6.2, 0, 7); c.fill();
    c.strokeStyle = S.glow ? "#cfd3ff" : "#3b404d"; c.lineWidth = 1; c.lineCap = "round";
    const now = new Date(), m = now.getMinutes() / 60, hh = ((now.getHours() % 12) + m) / 12;
    c.beginPath(); c.moveTo(0, 0); c.lineTo(Math.sin(hh * 6.283) * 3.3, -Math.cos(hh * 6.283) * 3.3); c.moveTo(0, 0); c.lineTo(Math.sin(m * 6.283) * 4.8, -Math.cos(m * 6.283) * 4.8); c.stroke();
  });
}
function doorLeft(y0, y1) {
  const S = G.S, fr = S.pal.doorFrame;
  fillP([P(0, y1 + 0.08, 0), P(0, y0 - 0.08, 0), P(0, y0 - 0.08, 2.25), P(0, y1 + 0.08, 2.25)], tonesOf(S, fr).r);
  fillP([P(0, y1, 0), P(0, y0, 0), P(0, y0, 2.15), P(0, y1, 2.15)], tonesOf(S, S.pal.door).r);
  const m = (y0 + y1) / 2; line(P(0, m, 0), P(0, m, 2.15), shade(S.pal.door, -0.35), 1);
  [[y0 + 0.12, m - 0.08], [m + 0.08, y1 - 0.12]].forEach(([a, b]) => { const c = G.c; path([P(0, b, 0.2), P(0, a, 0.2), P(0, a, 1.95), P(0, b, 1.95)]); c.strokeStyle = rgba("#000000", 0.12); c.lineWidth = lw(1); c.stroke(); });
  const [X, Y] = P(0, m - 0.16, 1.0); blob(X, Y, 1.6, 1.6, "#e9dcc6");
  const [X2, Y2] = P(0, m + 0.16, 1.0); blob(X2, Y2, 1.6, 1.6, "#e9dcc6");
  // papan keluar
  const ex = [P(0, m + 0.35, 2.45), P(0, m - 0.35, 2.45), P(0, m - 0.35, 2.7), P(0, m + 0.35, 2.7)];
  fillP(ex, S.glow ? "#2bd17e" : "#22a565");
}

/* ====================================================================== */
/* Perabot                                                                 */
/* ====================================================================== */
let OBJ = [];
const add = (x0, y0, x1, y1, rm, draw, k) => OBJ.push({ k: k ?? (x0 + x1 + y0 + y1) / 2, rm, draw, fp: [x0, y0, x1, y1] });
const seatOcc = (sid) => CH.find((c) => c.seat === sid && !c.path);
/* Meja kerja dengan monitor. mon: "s" layar menghadap +y (terlihat), "n" layar menghadap -y (punggung monitor terlihat). */
function deskObj(x, y, w, d, mon, sid, rm, prop = 0) {
  add(x, y, x + w, y + d, rm, () => {
    const S = G.S, occ = sid ? seatOcc(sid) : null, on = !!occ;
    box(x + 0.08, y + 0.1, 0, 0.08, d - 0.2, 0.68, "deskLeg");
    box(x + w - 0.16, y + 0.1, 0, 0.08, d - 0.2, 0.68, "deskLeg");
    box(x + 0.16, mon === "s" ? y + 0.12 : y + d - 0.2, 0.25, w - 0.32, 0.06, 0.4, "deskLeg", { noEdge: 1 });
    box(x, y, 0.68, w, d, 0.06, "desk", { edge: 1.2 });
    if (!mon) return;
    // keyboard dan mouse
    const ky = mon === "s" ? y + d - 0.42 : y + 0.18;
    box(x + w / 2 - 0.4, ky, 0.74, 0.8, 0.22, 0.025, "kb", { noOut: 1 });
    box(x + w / 2 + 0.55, ky + 0.04, 0.74, 0.12, 0.16, 0.03, "kb", { noOut: 1 });
    // properti meja
    const px = x + (prop % 2 ? 0.25 : w - 0.5), py = mon === "s" ? y + 0.35 : y + d - 0.55;
    if (prop % 3 === 0) { cyl(px + 0.1, py + 0.1, 0.74, 0.07, 0.14, "mug", { top: shade(S.pal.mug, -0.25) }); }
    else if (prop % 3 === 1) { cyl(px + 0.1, py + 0.12, 0.74, 0.09, 0.1, "pot"); blob(...P(px + 0.1, py + 0.12, 0.98), 4.2, 3.6, S.pal.leaf1); blob(...P(px + 0.06, py + 0.1, 1.04), 2.6, 2.4, S.pal.leaf3); }
    else { box(px - 0.1, py, 0.74, 0.36, 0.26, 0.05, "board", { noOut: 1 }); box(px - 0.08, py + 0.02, 0.79, 0.34, 0.24, 0.02, "board", { noOut: 1 }); }
    if (mon === "n" && rm === "KSR" && prop % 2 === 0) deskLampDraw(x + 0.25, y + 0.25);
    // monitor
    const mx = x + w / 2 - 0.58, my = mon === "s" ? y + 0.1 : y + d - 0.24;
    box(mx + 0.5, my + 0.03, 0.74, 0.16, 0.08, 0.16, "mon", { noOut: 1 });
    box(mx + 0.38, my - 0.06 + (mon === "s" ? 0.1 : -0.05), 0.74, 0.4, 0.22, 0.02, "mon", { noOut: 1 });
    box(mx, my, 0.86, 1.16, 0.14, 0.66, "mon", { edge: 0.4 });
    if (mon === "s") {
      const yy = my + 0.14, sc = [P(mx + 0.06, yy, 0.92), P(mx + 1.1, yy, 0.92), P(mx + 1.1, yy, 1.46), P(mx + 0.06, yy, 1.46)];
      if (!on) { fillP(sc, S.pal.screenOff); return; }
      fillP(sc, S.pal.screen);
      const working = occ.st === "jalan" || occ.st === "pemilik";
      const off = working ? Math.floor(G.t * 1.6 + occ.ph * 10) : 3;
      const cols = S.glow ? ["#7aa7ff", "#ffd27a", "#8ff0c0", "#ff9cc6", "#cfd6ff"] : ["#7aa7ff", "#ffcf6b", "#7ee0b0", "#ff99c2", "#c9d1ff"];
      for (let k = 0; k < 5; k++) {
        const n = (k + off) % 9, ind = (hash(n, sid.length) * 3 | 0) * 0.08, len = 0.22 + hash(n, 2) * 0.55;
        const z = 1.39 - k * 0.1;
        line(P(mx + 0.14 + ind, yy, z), P(mx + 0.14 + ind + len, yy, z), cols[n % 5], S.pixel ? 1 : 1.6);
      }
      if (occ.st === "review" || occ.st === "keputusan") { const col = occ.st === "review" ? "#ffb347" : "#ff6b6b"; fillP([P(mx + 0.6, yy, 0.98), P(mx + 1.04, yy, 0.98), P(mx + 1.04, yy, 1.14), P(mx + 0.6, yy, 1.14)], col); }
    } else {
      // punggung monitor dengan logo; cahaya layar bocor ke tepi di gaya malam
      const [X, Y] = P(mx + 0.58, my + 0.14, 1.2); blob(X, Y, 2.2, 2.2, shade(S.pal.mon, 0.25));
      if (on && S.glow) { const c = G.c; const [a1, b1] = P(mx, my, 1.52), [a2, b2] = P(mx + 1.16, my, 1.52); c.strokeStyle = "rgba(140,190,255,.55)"; c.lineWidth = lw(2); c.beginPath(); c.moveTo(a1, b1); c.lineTo(a2, b2); c.stroke(); }
    }
  });
}
/* Kursi kantor. back: "n" sandaran di -y (orang menghadap +y), "s" sandaran di +y, "w" sandaran di -x. */
function chairObj(x, y, back, rm) {
  const seat = () => {
    footShadow(x, y, 0.32, 0.8);
    box(x - 0.04, y - 0.04, 0.06, 0.08, 0.08, 0.32, "chairLeg", { noOut: 1 });
    [[-0.26, 0], [0.26, 0], [0, -0.26], [0, 0.26]].forEach(([a, b]) => box(x + a - 0.04, y + b - 0.04, 0, 0.08, 0.08, 0.06, "chairLeg", { noOut: 1 }));
    box(x - 0.3, y - 0.3, 0.38, 0.6, 0.6, 0.1, "chair");
  };
  const backrest = () => {
    if (back === "n") { box(x - 0.04, y - 0.34, 0.44, 0.08, 0.06, 0.12, "chairLeg", { noOut: 1 }); box(x - 0.28, y - 0.4, 0.52, 0.56, 0.1, 0.48, "chair"); }
    if (back === "s") { box(x - 0.04, y + 0.28, 0.44, 0.08, 0.06, 0.1, "chairLeg", { noOut: 1 }); box(x - 0.28, y + 0.3, 0.5, 0.56, 0.1, 0.4, "chair"); }
    if (back === "w") { box(x - 0.4, y - 0.28, 0.52, 0.1, 0.56, 0.48, "chair"); }
  };
  if (back === "s") { add(x - 0.3, y - 0.3, x + 0.3, y + 0.3, rm, seat, x + y - 0.08); add(x - 0.3, y + 0.3, x + 0.3, y + 0.4, rm, backrest, x + y + 0.32); }
  else add(x - 0.4, y - 0.4, x + 0.3, y + 0.3, rm, () => { backrest(); seat(); }, x + y - 0.2);
}
function plantObj(x, y, s = 1, rm, kind = 0) {
  const r = 0.24 * s;
  add(x - r, y - r, x + r, y + r, rm, () => {
    const S = G.S; footShadow(x + 0.05, y + 0.06, r * 1.25, 0.9);
    cyl(x, y, 0, r, 0.42 * s, kind ? "potB" : "pot");
    const [X, Y] = P(x, y, 0.42 * s); const k = s * 1.05;
    if (kind === 2) { // pohon tinggi
      line([X, Y], [X, Y - 26 * k], shade(S.pal.woodD, -0.2), 2);
      [[-7, -30, 11, 9, "leaf2"], [7, -34, 10, 9, "leaf2"], [0, -40, 12, 10, "leaf1"], [-5, -46, 8, 7, "leaf1"], [5, -47, 7, 6, "leaf3"], [-1, -36, 6, 5, "leaf3"]].forEach(([dx, dy, rx, ry, m]) => blob(X + dx * k, Y + dy * k, rx * k, ry * k, S.pal[m]));
      return;
    }
    [[-6, -8, 9, 8, "leaf2"], [6, -9, 8, 7.5, "leaf2"], [0, -15, 10, 9, "leaf1"], [-4, -20, 6.5, 6, "leaf1"], [4, -19, 5.5, 5, "leaf3"], [-1, -12, 4, 3.5, "leaf3"]].forEach(([dx, dy, rx, ry, m]) => blob(X + dx * k, Y + dy * k, rx * k, ry * k, S.pal[m]));
  });
}
/* Lampu meja gooseneck; menyala hangat di gaya malam. */
function deskLampDraw(x, y, z = 0.74) {
  const S = G.S, c = G.c;
  cyl(x, y, z, 0.09, 0.03, "mon", { noOut: 1 });
  const a = P(x, y, z + 0.03), b = P(x, y, z + 0.55), d = P(x + 0.22, y + 0.12, z + 0.62);
  c.beginPath(); c.moveTo(...a); c.lineTo(...b); c.lineTo(...d); c.strokeStyle = S.pal.mon; c.lineWidth = lw(S.pixel ? 1 : 1.6); c.lineCap = "round"; c.stroke();
  const [X, Y] = d; c.beginPath(); c.moveTo(X - 5, Y + 3); c.lineTo(X - 2, Y - 3); c.lineTo(X + 4, Y - 2); c.lineTo(X + 6, Y + 4); c.closePath(); c.fillStyle = S.glow ? "#3a3f5c" : S.pal.mon; c.fill();
  blob(X + 0.6, Y + 3.6, 3.6, 1.4, S.glow ? "#fff1c4" : "#ffe7a3");
}
function floorLampDraw(x, y) {
  const S = G.S; footShadow(x, y, 0.25);
  cyl(x, y, 0, 0.18, 0.05, "deskLeg"); box(x - 0.03, y - 0.03, 0, 0.06, 0.06, 1.75, "deskLeg", { noOut: 1 });
  cyl(x, y, 1.65, 0.24, 0.32, tonesOf(S, S.glow ? "#ffe2a8" : S.pal.lampShade));
}
function shelfObj() {
  // rak buku di dinding kiri KSR
  add(0, 6.6, 0.6, 9.4, "KSR", () => {
    const S = G.S, books = ["#e07a5f", "#3d5a80", "#81b29a", "#f2cc8f", "#9b7ede", "#ef8fa8", "#5fb0c9"];
    box(0, 6.6, 0, 0.55, 2.8, 1.95, "shelf");
    [0.08, 0.68, 1.28].forEach((z, li) => {
      fillP([P(0.55, 6.7, z), P(0.55, 9.3, z), P(0.55, 9.3, z + 0.55), P(0.55, 6.7, z + 0.55)], shade(S.pal.shelf, -0.38));
      let yy = 6.72;
      for (let i = 0; yy < 9.2; i++) {
        const bw = 0.12 + hash(i, li) * 0.08, bh = 0.34 + hash(li, i) * 0.16, col = scol(S, books[(i + li * 3) % books.length]);
        if (hash(i + 7, li) > 0.86) { yy += 0.2; continue; }
        box(0.12, yy, z, 0.36, Math.min(bw, 9.28 - yy), bh, tonesOf(S, col), { noOut: !S.pixel, noEdge: 1 }); yy += bw + 0.01;
      }
    });
  });
}
function rackObj(y0, y1) {
  add(0, y0, 0.75, y1, "GDG", () => {
    const S = G.S;
    [y0 + 0.02, y1 - 0.1].forEach((yy) => box(0.06, yy, 0, 0.08, 0.08, 2.1, "deskLeg"));
    [y0 + 0.02, y1 - 0.1].forEach((yy) => box(0.62, yy, 0, 0.08, 0.08, 2.1, "deskLeg"));
    [0.08, 0.72, 1.36, 2.0].forEach((z, li) => {
      box(0.06, y0, z, 0.64, y1 - y0, 0.05, "deskLeg");
      if (li === 3) return;
      let yy = y0 + 0.15;
      for (let i = 0; yy < y1 - 0.5; i++) { const bw = 0.48 + hash(i, li) * 0.3, bh = 0.32 + hash(li, i) * 0.22; box(0.14, yy, z + 0.05, 0.5, bw, bh, "cardboard"); line(P(0.64, yy + bw / 2, z + 0.05 + bh), P(0.64, yy + bw / 2, z + 0.05 + bh * 0.6), shade(S.pal.cardboard, -0.3), 1); yy += bw + 0.12; }
    });
  });
}

function buildScene() {
  OBJ = [];
  /* ---- Kasir Online ---- */
  [[2, "k1", "k3", 0], [4.4, "k2", "k4", 1], [10, "k5", "k7", 2], [12.4, "k6", "k8", 3]].forEach(([x, sb, sf, i]) => {
    deskObj(x, 4, 2.4, 1, "n", sb, "KSR", i);
    deskObj(x, 5, 2.4, 1, "s", sf, "KSR", i + 1);
    chairObj(SEATS[sb].x, SEATS[sb].y, "n", "KSR");
    chairObj(SEATS[sf].x, SEATS[sf].y, "s", "KSR");
  });
  shelfObj();
  plantObj(16.3, 0.55, 1.15, "KSR", 2); plantObj(0.5, 10.3, 1.1, "KSR"); plantObj(13.3, 0.5, 0.9, "KSR", 1); plantObj(0.5, 4.6, 0.85, "KSR", 1);
  // meja berdiri + printer
  add(15.3, 7.3, 16.6, 8.2, "KSR", () => { box(15.3, 7.3, 0, 1.3, 0.9, 0.95, "printer"); box(15.45, 7.4, 0.95, 0.9, 0.7, 0.32, "printer", { edge: 1.2 }); box(15.6, 7.85, 1.27, 0.6, 0.12, 0.02, "board"); });
  add(15.4, 9.4, 16.6, 10.4, "KSR", () => { box(15.4, 9.4, 0, 1.2, 1.0, 0.8, "shelf"); fillP([P(16.6, 9.5, 0.1), P(16.6, 10.3, 0.1), P(16.6, 10.3, 0.7), P(16.6, 9.5, 0.7)], shade(mat(G.S, "shelf").r, -0.15)); line(P(16.6, 9.9, 0.1), P(16.6, 9.9, 0.7), shade(mat(G.S, "shelf").r, -0.35), 1); });
  /* ---- Ruang rapat ---- */
  add(19.8, 4.6, 25.0, 6.3, "MEET", () => {
    const S = G.S;
    softShadow(quad(19.9, 4.75, 25.15, 6.5), 1);
    box(20.5, 5.2, 0, 0.3, 0.5, 0.66, "woodD"); box(24.0, 5.2, 0, 0.3, 0.5, 0.66, "woodD");
    box(19.8, 4.6, 0.66, 5.2, 1.7, 0.08, "wood", { edge: 1.3 });
    // laptop Olivia + kertas + teko
    box(22.25, 4.75, 0.74, 0.7, 0.45, 0.025, "mon", { noOut: 1 });
    fillP([P(22.25, 4.75, 0.77), P(22.95, 4.75, 0.77), P(22.95, 4.75, 1.18), P(22.25, 4.75, 1.18)], S.pal.mon);
    box(21.1, 5.0, 0.74, 0.42, 0.3, 0.02, "board", { noOut: 1 }); box(23.7, 5.4, 0.74, 0.42, 0.3, 0.02, "board", { noOut: 1 });
    cyl(24.4, 4.95, 0.74, 0.1, 0.26, "glass", { top: S.pal.water }); cyl(20.6, 5.7, 0.74, 0.07, 0.14, "mug");
  });
  ["m1", "m2", "m3"].forEach((s) => chairObj(SEATS[s].x, SEATS[s].y, "n", "MEET"));
  [21.0, 22.6, 24.2].forEach((x) => chairObj(x, 7.15, "s", "MEET"));
  add(23.9, 0.05, 26.6, 0.65, "MEET", () => { box(23.9, 0.05, 0, 2.7, 0.6, 0.78, "woodD"); [24.55, 25.9].forEach((x) => line(P(x, 0.65, 0.08), P(x, 0.65, 0.7), rgba("#000000", 0.18), 1)); });
  plantObj(26.3, 0.4, 0.75, "MEET", 1); plantObj(17.6, 10.3, 1.15, "MEET", 2); plantObj(26.4, 10.4, 1, "MEET");
  /* ---- Lorong ---- */
  add(25.95, 11.2, 26.6, 11.85, "COR", () => { const S = G.S; footShadow(26.3, 11.55, 0.4); box(25.98, 11.22, 0, 0.6, 0.6, 1.0, "cooler"); cyl(26.28, 11.52, 1.0, 0.22, 0.42, tonesOf(S, S.pal.water), { top: shade(S.pal.water, 0.2) }); box(26.1, 11.82, 0.6, 0.15, 0.03, 0.08, "deskLeg", { noOut: 1 }); });
  plantObj(16.6, 11.45, 0.8, "COR", 1); plantObj(0.45, 12.9, 0.7, "COR");
  /* ---- Aplikasi Gudang ---- */
  rackObj(13.9, 17.5);
  deskObj(3.4, 15.0, 2.4, 1, "n", "g1", "GDG", 2); chairObj(4.6, 14.4, "n", "GDG");
  deskObj(3.4, 18.6, 2.4, 1, "n", "g2", "GDG", 0); chairObj(4.6, 18.0, "n", "GDG");
  add(6.4, 19.5, 7.8, 20.9, "GDG", () => { softShadow(quad(6.45, 19.6, 7.95, 21.05)); box(6.4, 19.5, 0, 1.4, 1.4, 0.14, "pallet"); box(6.5, 19.6, 0.14, 0.62, 0.62, 0.5, "cardboard"); box(7.12, 19.6, 0.14, 0.6, 0.62, 0.5, "cardboard"); box(6.5, 20.22, 0.14, 0.62, 0.6, 0.5, "cardboard"); box(7.12, 20.22, 0.14, 0.6, 0.6, 0.42, "cardboard"); box(6.75, 19.85, 0.64, 0.7, 0.62, 0.42, "cardboard"); });
  plantObj(7.9, 13.6, 1, "GDG", 1);
  /* ---- Portal Pelanggan ---- */
  deskObj(10.0, 15.0, 2.4, 1, "s", "q1", "PPL", 1); chairObj(11.2, 16.6, "s", "PPL");
  deskObj(12.4, 15.0, 2.4, 1, "s", "q2", "PPL", 2); chairObj(13.6, 16.6, "s", "PPL");
  add(14.3, 19.4, 15.5, 20.6, "PPL", () => { const S = G.S; footShadow(14.9, 20.0, 0.62); const [X, Y] = P(14.9, 20.0, 0); const t = tonesOf(S, S.pal.beanbag2); blob(X, Y - 9, 22, 13, t.l); blob(X - 3, Y - 13, 16, 9, t.t); blob(X + 9, Y - 8, 10, 8, t.r); });
  add(16.2, 13.4, 16.7, 13.9, "PPL", () => floorLampDraw(16.45, 13.65));
  add(17.2, 15.1, 17.7, 15.6, "PANTRY", () => floorLampDraw(17.45, 15.35));
  add(26.2, 7.4, 26.7, 7.9, "MEET", () => floorLampDraw(26.45, 7.65));
  plantObj(9.1, 21.3, 1.1, "PPL", 2); plantObj(16.4, 21.4, 0.85, "PPL");
  /* ---- Pantry ---- */
  add(20.2, 13.15, 25.0, 14.05, "PANTRY", () => {
    const S = G.S;
    box(20.2, 13.15, 0, 4.8, 0.88, 0.88, "counter");
    [21.4, 22.6, 23.8].forEach((x) => { line(P(x, 14.03, 0.08), P(x, 14.03, 0.8), rgba("#000000", 0.12), 1); blob(...P(x - 0.12, 14.03, 0.7), 0.8, 1.6, rgba("#000000", 0.35)); });
    box(20.15, 13.1, 0.88, 4.9, 0.98, 0.06, "counterTop", { edge: 1 });
    // mesin kopi
    box(20.45, 13.25, 0.94, 0.6, 0.5, 0.62, "mon"); box(20.5, 13.75, 1.1, 0.5, 0.04, 0.3, "metal", { noOut: 1 }); cyl(20.75, 13.62, 0.94, 0.07, 0.1, "mug");
    const [lx, ly] = P(20.55, 13.75, 1.48); blob(lx, ly, 1.2, 1.2, S.glow ? "#6bff9e" : "#30a46c");
    // wastafel, buah, microwave
    const [sx, sy] = P(22.0, 13.6, 0.95); blob(sx, sy, 11, 5, shade(S.pal.metal, -0.15)); blob(sx, sy + 0.8, 9, 4, shade(S.pal.metal, -0.32));
    box(21.9, 13.2, 0.94, 0.08, 0.08, 0.4, "metal", { noOut: 1 });
    const [fx, fy] = P(23.0, 13.6, 0.97); blob(fx, fy, 7, 3.5, scol(S, "#f2e4c9")); blob(fx - 3, fy - 2.5, 2.6, 2.6, scol(S, "#ff8a5c")); blob(fx + 2, fy - 2.8, 2.6, 2.6, scol(S, "#ffd166")); blob(fx, fy - 4, 2.4, 2.4, scol(S, "#7cc576"));
    box(23.8, 13.25, 0.94, 0.85, 0.55, 0.45, "printer"); fillP([P(23.88, 13.8, 1.0), P(24.4, 13.8, 1.0), P(24.4, 13.8, 1.32), P(23.88, 13.8, 1.32)], S.glow ? "#2a3150" : "#3b404d");
    // kabinet atas di dinding? tidak ada dinding; rak gantung kecil
  });
  // uap kopi digambar dinamis
  add(20.45, 13.25, 21.05, 13.75, "PANTRY", () => steam(20.75, 13.62, 1.06), 20.75 + 13.62 + 0.9);
  add(25.3, 13.15, 26.4, 14.1, "PANTRY", () => { const S = G.S; box(25.3, 13.15, 0, 1.1, 0.95, 2.05, "fridge", { edge: 1.2 }); line(P(25.3, 14.1, 1.3), P(26.4, 14.1, 1.3), rgba("#000000", 0.14), 1.2); box(25.45, 14.1, 1.4, 0.05, 0.03, 0.45, "metal", { noOut: 1 }); box(25.45, 14.1, 0.75, 0.05, 0.03, 0.4, "metal", { noOut: 1 }); fillP([P(25.9, 14.1, 1.6), P(26.2, 14.1, 1.6), P(26.2, 14.1, 1.85), P(25.9, 14.1, 1.85)], scol(S, "#ffd166")); });
  // sofa di partisi kiri (sandaran di -x)
  add(17.15, 15.6, 18.35, 19.5, "PANTRY", () => {
    const S = G.S;
    softShadow(quad(17.2, 15.7, 18.6, 19.6), 1);
    box(17.2, 15.75, 0, 1.1, 3.6, 0.42, "sofa");
    box(17.2, 15.75, 0.42, 0.32, 3.6, 0.55, "sofa", { edge: 0.8 });
    box(17.2, 15.75, 0.42, 1.1, 0.3, 0.28, "sofa");
    box(17.52, 16.1, 0.42, 0.75, 1.42, 0.1, "sofa", { edge: 0.8 }); box(17.52, 17.56, 0.42, 0.75, 1.42, 0.1, "sofa", { edge: 0.8 });
    box(17.55, 18.4, 0.52, 0.18, 0.5, 0.36, "cushion");
  }, 17.95 + 17.0 - 0.3);
  add(17.2, 19.05, 18.3, 19.35, "PANTRY", () => box(17.2, 19.05, 0.42, 1.1, 0.3, 0.28, "sofa"));
  add(19.4, 16.4, 20.4, 18.2, "PANTRY", () => { const S = G.S; footShadow(19.9, 17.3, 0.75, 0.9); box(19.5, 16.5, 0, 0.1, 0.1, 0.34, "woodD"); box(20.2, 18.0, 0, 0.1, 0.1, 0.34, "woodD"); box(20.2, 16.5, 0, 0.1, 0.1, 0.34, "woodD"); box(19.5, 18.0, 0, 0.1, 0.1, 0.34, "woodD"); box(19.4, 16.4, 0.34, 1.0, 1.8, 0.06, "wood", { edge: 1.2 }); cyl(19.75, 16.9, 0.4, 0.07, 0.13, "mug"); box(19.6, 17.4, 0.4, 0.5, 0.35, 0.02, tonesOf(S, scol(S, "#9b7ede")), { noOut: 1 }); });
  add(24.15, 17.15, 25.05, 18.05, "PANTRY", () => { const S = G.S; footShadow(24.6, 17.6, 0.45); cyl(24.6, 17.6, 0, 0.26, 0.04, "deskLeg"); cyl(24.6, 17.6, 0.04, 0.05, 0.98, "deskLeg"); cyl(24.6, 17.6, 1.02, 0.44, 0.06, "wood"); cyl(24.45, 17.5, 1.08, 0.06, 0.12, "mug"); });
  add(21.4, 20.0, 22.6, 21.0, "PANTRY", () => { const S = G.S; footShadow(22.0, 20.5, 0.62); const [X, Y] = P(22.0, 20.5, 0); const t = tonesOf(S, S.pal.beanbag); blob(X, Y - 9, 22, 13, t.l); blob(X - 3, Y - 13, 16, 9, t.t); blob(X + 9, Y - 8, 10, 8, t.r); });
  plantObj(26.4, 21.4, 1.2, "PANTRY", 2); plantObj(17.5, 21.4, 0.9, "PANTRY");
  /* ---- Partisi kaca ---- */
  for (let j = 0; j < 11; j++) glassSeg("y", 17, j, j + 1, "KSR");
  for (let j = 13; j < 22; j++) { glassSeg("y", 8.5, j, j + 1, "GDG"); glassSeg("y", 17, j, j + 1, "PPL"); }
  [[11, "KSR"], [13, "GDG"]].forEach(([yy, rm]) => {
    for (let i = 0; i < W; i++) {
      const dk = Object.entries(DOORS).find(([, d]) => d[2] === yy && i + 1 > d[0] && i < d[1]);
      let a = i, b = i + 1;
      if (dk) { const d = dk[1]; if (a < d[0]) b = d[0]; else if (b > d[1]) a = d[1]; else continue; if (b - a < 0.02) continue; }
      halfSeg(yy, a, b, rm);
    }
  });
}
function steam(x, y, z) {
  const S = G.S, c = G.c; if (S.pixel || G.still) return;
  const [X, Y] = P(x, y, z);
  for (let i = 0; i < 2; i++) {
    const t = (G.t * 0.5 + i * 0.5) % 1;
    c.beginPath(); c.moveTo(X - 1.5 + i * 3, Y - 2 - t * 10); c.bezierCurveTo(X - 4 + i * 3, Y - 6 - t * 10, X + 2 + i * 3, Y - 9 - t * 10, X - 1 + i * 3, Y - 13 - t * 10);
    c.strokeStyle = `rgba(255,255,255,${0.75 * (1 - t) * (S.glow ? 0.6 : 1)})`; c.lineWidth = lw(1.6); c.lineCap = "round"; c.stroke();
  }
}
/* Segmen partisi kaca penuh di sepanjang sumbu y pada x tetap. */
function glassSeg(ax, x, y0, y1, rm) {
  add(x - 0.05, y0, x + 0.05, y1, rm, () => {
    const S = G.S, c = G.c, H = 2.75;
    box(x - 0.05, y0, 0, 0.1, y1 - y0, 0.14, "glassFrame", { noOut: 1 });
    const pane = [P(x, y1, 0.14), P(x, y0, 0.14), P(x, y0, H), P(x, y1, H)];
    fillP(pane, rgba(S.pal.glass, S.pixel ? 0.35 : S.glow ? 0.12 : 0.24));
    if (!S.pixel) { const a = P(x, y0 + 0.25, 0.6), b = P(x, y0 + 0.55, 2.2); c.beginPath(); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); c.strokeStyle = `rgba(255,255,255,${S.glow ? 0.12 : 0.55})`; c.lineWidth = lw(3); c.stroke(); }
    box(x - 0.04, y0, H, 0.08, y1 - y0, 0.07, "glassFrame", { noOut: 1 });
    box(x - 0.035, y0, 0, 0.07, 0.06, H, "glassFrame", { noOut: 1 });
    if (S.pixel) { path(pane); c.strokeStyle = S.pal.outline; c.lineWidth = lw(1); c.stroke(); }
  });
}
/* Segmen partisi setengah tinggi: dinding rendah + kaca, sepanjang x pada y tetap. */
function halfSeg(y, x0, x1, rm) {
  add(x0, y - 0.08, x1, y + 0.08, rm, () => {
    const S = G.S, c = G.c;
    box(x0, y - 0.08, 0, x1 - x0, 0.16, 0.5, "half", { noEdge: 1, noOut: 1 });
    box(x0, y - 0.09, 0.5, x1 - x0, 0.18, 0.05, "glassFrame", { noOut: 1 });
    const pane = [P(x0, y, 0.55), P(x1, y, 0.55), P(x1, y, 1.3), P(x0, y, 1.3)];
    fillP(pane, rgba(S.pal.glass, S.pixel ? 0.35 : S.glow ? 0.12 : 0.24));
    box(x0, y - 0.035, 1.3, x1 - x0, 0.07, 0.05, "glassFrame", { noOut: 1 });
    if (Math.abs(x0 - Math.round(x0)) < 0.01) box(x0, y - 0.035, 0.55, 0.06, 0.07, 0.75, "glassFrame", { noOut: 1 });
    if (S.pixel) { path([P(x0, y + 0.08, 0), P(x1, y + 0.08, 0), P(x1, y, 1.35), P(x0, y, 1.35)]); c.strokeStyle = S.pal.outline; c.lineWidth = lw(1); c.stroke(); }
  });
}

/* ====================================================================== */
/* Karakter (vektor)                                                       */
/* ====================================================================== */
const FACE = { "+y": ["f", false], "+x": ["f", true], "-x": ["b", false], "-y": ["b", true] };
function poseOf(c) {
  const s = SEATS[c.seat] || {};
  if (c.path) return { pose: "walk", arms: "walk", mood: "ok" };
  const sit = !!s.sit;
  const st = c.st;
  if (st === "review") return { pose: sit ? "sit" : "stand", arms: "raise", mood: "ok" };
  if (st === "keputusan") return { pose: sit ? "sit" : "stand", arms: "head", mood: "sad" };
  if (st === "jalan" || st === "pemilik") return { pose: sit ? "sit" : "stand", arms: s.act === "tablet" ? "tablet" : "type", mood: "ok" };
  if (st === "ok") return { pose: sit ? "sit" : "stand", arms: "idle", mood: "happy" };
  if (st === "jeda") return { pose: sit ? "sit" : "stand", arms: "idle", mood: "sleepy" };
  if (st === "rapat") return { pose: sit ? "sit" : "stand", arms: s.act === "point" ? "point" : "idle", mood: "happy" };
  if (st === "idle") return { pose: sit ? "sit" : "stand", arms: s.act || "idle", mood: s.sofa ? "sleepy" : "ok" };
  return { pose: sit ? "sit" : "stand", arms: "idle", mood: "ok" };
}
/* Gambar satu karakter chibi di titik kaki (X, Y) dalam piksel dunia. */
function drawCharV(ch, X, Y, face, flip, po, t, sel) {
  const S = G.S, c = G.c, look = ch.look, bot = !ch.who;
  const shirt = scol(S, look.shirt), shirtT = tonesOf(S, shirt), skin = scol(S, look.skin || "#e8b996"), hair = scol(S, look.hair || "#2a2a2a");
  const pants = S.pal.pants, shoes = S.pal.shoes, shell = S.pal.shell, shellD = S.pal.shellD;
  const ph = ch.ph * 6.283, still = G.still;
  const sit = po.pose === "sit", walk = po.pose === "walk";
  const bob = still ? 0 : walk ? Math.abs(Math.sin(t * 9 + ph)) * -1.6 : po.arms === "type" ? Math.sin(t * 3 + ph) * 0.35 : Math.sin(t * 2 + ph) * 0.6;
  const dy = (sit ? 2 : 0) + bob;
  // cincin pilihan dan bayangan kaki
  if (sel) {
    c.save(); c.translate(X, Y); c.scale(1, 0.5);
    const pul = still ? 0 : (Math.sin(t * 4) + 1) / 2;
    c.beginPath(); c.arc(0, 0, 21 + pul * 2, 0, 7); c.fillStyle = "rgba(91,91,214,.16)"; c.fill();
    c.lineWidth = lw(2) * 2; c.strokeStyle = S.glow ? "#a9a9ff" : "#5b5bd6"; c.stroke(); c.restore();
  }
  if (ch.st === "keputusan" && !po.pose !== "walk") { c.save(); c.translate(X, Y); c.scale(1, 0.5); const pul = still ? 0.5 : (t * 0.8) % 1; c.beginPath(); c.arc(0, 0, 14 + pul * 14, 0, 7); c.strokeStyle = `rgba(229,72,77,${0.7 * (1 - pul)})`; c.lineWidth = lw(2) * 2; c.stroke(); c.restore(); }
  if (!sit) footShadow2(X, Y, 11);
  c.save(); c.translate(X, Y); if (flip) c.scale(-1, 1);
  const front = face === "f";
  // ---- kaki ----
  if (sit) {
    if (front) { rr(c, -8, -12 + dy, 16, 6, 3); c.fillStyle = pants; c.fill(); rr(c, -7, -7 + dy, 5, 7, 2); c.fill(); rr(c, 2, -7 + dy, 5, 7, 2); c.fill(); c.fillStyle = shoes; rr(c, -8, -1.5 + dy, 6.5, 3, 1.5); c.fill(); rr(c, 1.5, -1.5 + dy, 6.5, 3, 1.5); c.fill(); }
  } else {
    const sw = walk && !still ? Math.sin(t * 9 + ph) * 2.6 : 0;
    c.fillStyle = bot ? shellD : pants;
    rr(c, -6.5, -12 + dy, 5.5, 12 - dy + sw * 0.3 - 1, 2.4); c.fill();
    rr(c, 1, -12 + dy, 5.5, 12 - dy - sw * 0.3 - 1, 2.4); c.fill();
    c.fillStyle = bot ? shell : shoes;
    rr(c, -7.5 + (front ? -0.5 : 0.5), -2.4 + sw * 0.3, 7, 3.4, 1.7); c.fill();
    rr(c, 0.5 + (front ? -0.5 : 0.5), -2.4 - sw * 0.3, 7, 3.4, 1.7); c.fill();
  }
  const by = -10 + dy; // dasar badan
  const sh = (a) => [a > 0 ? 8.5 : -8.5, by - 13];
  const arm = (from, to, col) => { c.beginPath(); c.moveTo(from[0], from[1]); c.lineTo(to[0], to[1]); c.lineCap = "round"; c.strokeStyle = col; c.lineWidth = 5.4; c.stroke(); };
  const hand = (p, col = bot ? shell : skin) => { c.beginPath(); c.arc(p[0], p[1], 2.9, 0, 7); c.fillStyle = col; c.fill(); };
  const armCol = shirtT.l, armColB = shirtT.r;
  const wave = still ? 0 : Math.sin(t * 7 + ph) * 3.2, tap = still ? 0 : Math.sin(t * 16 + ph * 3), tap2 = still ? 0 : Math.sin(t * 16 + ph * 3 + 2.2);
  const walkSw = walk && !still ? Math.sin(t * 9 + ph) * 3 : 0;
  // lengan di belakang badan
  if (!front) { arm(sh(-1), [-10.5, by - 3 + walkSw], armColB); arm(sh(1), [10.5, by - 3 - walkSw], armColB); }
  else if (po.arms === "idle" || po.arms === "walk") { arm(sh(1), [10.5, by - 2 - walkSw], armColB); }
  // ---- badan ----
  c.beginPath(); c.moveTo(-9.5, by); c.bezierCurveTo(-10.5, by - 9, -9, by - 15, 0, by - 15); c.bezierCurveTo(9, by - 15, 10.5, by - 9, 9.5, by); c.closePath();
  if (S.pixel) c.fillStyle = shirt; else { const g = c.createLinearGradient(-10, 0, 10, 0); g.addColorStop(0, shade(shirt, 0.06)); g.addColorStop(0.6, shirt); g.addColorStop(1, shirtT.r); c.fillStyle = g; }
  c.fill();
  if (front) {
    if (bot) { c.beginPath(); c.arc(-1.2, by - 7, 4.6, 0, 7); c.fillStyle = shade(shirt, 0.82); c.fill(); drawIcon(c, AG.PERSONAS[ch.p].i, -5.2, by - 11, 8, shirt, 2.8); }
    else { c.beginPath(); c.moveTo(-4, by - 14.6); c.lineTo(-1, by - 10.5); c.lineTo(2, by - 14.6); c.fillStyle = skin; c.fill(); }
  } else if (bot) { rr(c, -4, by - 12, 8, 6, 2); c.fillStyle = shade(shirt, -0.2); c.fill(); }
  // ---- lengan depan sesuai pose (lengan terangkat digambar setelah kepala) ----
  const late = [];
  if (front) {
    const a = po.arms;
    if (a === "type") { arm(sh(-1), [-6.5, by - 3 + tap * 0.9], armCol); hand([-6.5, by - 3 + tap * 0.9]); arm(sh(1), [4, by - 3 + tap2 * 0.9], armColB); hand([4, by - 3 + tap2 * 0.9]); }
    else if (a === "raise") { arm(sh(-1), [-10.5, by - 2], armCol); hand([-10.5, by - 2]); const hx = 16 + wave * 0.5, hyy = by - 31; late.push(() => { arm(sh(1), [hx - 0.5, hyy + 3], armColB); c.beginPath(); c.arc(hx, hyy, 3.6, 0, 7); c.fillStyle = bot ? shell : skin; c.fill(); if (!S.pixel) { c.strokeStyle = rgba("#000000", 0.12); c.lineWidth = 0.8; c.stroke(); } }); }
    else if (a === "head") { late.push(() => { arm(sh(-1), [-13.5, by - 21], armCol); hand([-13.5, by - 21.5]); arm(sh(1), [13.5, by - 21], armColB); hand([13.5, by - 21.5]); }); }
    else if (a === "mug") { arm(sh(-1), [-4.5, by - 6], armCol); rr(c, -7.5, by - 11.5, 6, 7, 1.5); c.fillStyle = scol(S, "#ffffff"); c.fill(); c.fillStyle = shade(shirt, -0.1); c.fillRect(-7.5, by - 9, 6, 1.6); hand([-4.5, by - 6]); if (!still && !S.pixel) { const tt = (t * 0.6 + ch.ph) % 1; c.beginPath(); c.moveTo(-4.5, by - 13 - tt * 7); c.quadraticCurveTo(-7, by - 16 - tt * 7, -4.5, by - 19 - tt * 7); c.strokeStyle = `rgba(255,255,255,${0.7 * (1 - tt)})`; c.lineWidth = 1.2; c.stroke(); } }
    else if (a === "tablet") { arm(sh(-1), [-5, by - 5], armCol); arm(sh(1), [5, by - 5], armColB); c.save(); c.transform(1, -0.25, 0, 1, 0, 0); rr(c, -6.5, by - 9, 13, 8, 1.5); c.fillStyle = scol(S, "#2c3140"); c.fill(); c.fillStyle = S.glow ? "#9fd0ff" : "#8fc0ff"; c.fillRect(-5.3, by - 7.8, 10.6, 5.6); c.restore(); hand([-5.5, by - 5]); hand([5.5, by - 5]); }
    else if (a === "point") { arm(sh(-1), [-10, by - 2], armCol); hand([-10, by - 2]); late.push(() => { arm(sh(1), [17, by - 21 + wave * 0.2], armColB); hand([17.5, by - 21.5 + wave * 0.2]); }); }
    else { arm(sh(-1), [-10.5, by - 2 + walkSw], armCol); hand([-10.5, by - 2 + walkSw]); hand([10.5, by - 2 - walkSw]); }
  } else { hand([-10.5, by - 3 + walkSw]); hand([10.5, by - 3 - walkSw]); }
  // ---- kepala ----
  const hy = by - 26; // pusat kepala
  if (bot) botHead(ch, hy, front, po, t, shell, shellD, shirt);
  else humanHead(ch, hy, front, po, t, skin, hair, look);
  late.forEach((f) => f());
  c.restore();
  // ---- gelembung kecil di kanvas ----
  bubble(ch, X, Y, hy, po, t, flip);
}
function footShadow2(X, Y, r) { const c = G.c, S = G.S; if (S.pixel) { blob(X, Y, r, r / 2, "rgba(27,21,48,.22)"); return; } const g = c.createRadialGradient(X, Y, 0, X, Y, r); const k = S.glow ? 0.5 : 0.24; g.addColorStop(0, `rgba(20,24,50,${k})`); g.addColorStop(1, "rgba(20,24,50,0)"); c.save(); c.translate(X, Y); c.scale(1, 0.5); c.translate(-X, -Y); c.beginPath(); c.arc(X, Y, r, 0, 7); c.fillStyle = g; c.fill(); c.restore(); }
function eyesFor(mood, front, y, col, blink, ox = -2) {
  const c = G.c; if (!front) return;
  const L = -4.6 + ox, R = 3.4 + ox;
  c.strokeStyle = col; c.fillStyle = col; c.lineCap = "round";
  if (mood === "happy") { c.lineWidth = 1.5; c.beginPath(); c.moveTo(L - 2, y + 0.6); c.quadraticCurveTo(L, y - 2.2, L + 2, y + 0.6); c.moveTo(R - 2, y + 0.6); c.quadraticCurveTo(R, y - 2.2, R + 2, y + 0.6); c.stroke(); return; }
  if (mood === "sleepy" || blink) { c.lineWidth = 1.4; c.beginPath(); c.moveTo(L - 1.8, y); c.lineTo(L + 1.8, y); c.moveTo(R - 1.8, y); c.lineTo(R + 1.8, y); c.stroke(); return; }
  c.beginPath(); c.ellipse(L, y, 1.7, 2.1, 0, 0, 7); c.ellipse(R, y, 1.7, 2.1, 0, 0, 7); c.fill();
  if (mood === "sad") { c.lineWidth = 1.2; c.beginPath(); c.moveTo(L - 2.2, y - 4.4); c.lineTo(L + 1.6, y - 3.2); c.moveTo(R + 2.2, y - 4.4); c.lineTo(R - 1.6, y - 3.2); c.stroke(); }
}
function humanHead(ch, hy, front, po, t, skin, hair, look) {
  const c = G.c, S = G.S, r = 12.5, still = G.still, ox = -1.6;
  const blink = !still && ((t + ch.ph * 5) % 4.2) < 0.12;
  if (look.style === "hijab") {
    const hj = hair;
    c.beginPath(); c.moveTo(-14, hy + 15); c.bezierCurveTo(-16, hy + 2, -15, hy - 14, 0, hy - 14.5); c.bezierCurveTo(15, hy - 14, 16, hy + 2, 14, hy + 15); c.closePath(); c.fillStyle = hj; c.fill();
    if (!S.pixel) { c.beginPath(); c.moveTo(6, hy - 12); c.bezierCurveTo(13, hy - 6, 13, hy + 6, 9, hy + 14); c.strokeStyle = shade(hj, -0.15); c.lineWidth = 2.4; c.stroke(); }
    if (front) {
      c.beginPath(); c.ellipse(ox - 0.6, hy + 1, 9, 9.8, 0, 0, 7); c.fillStyle = skin; c.fill();
      eyesFor(po.mood, true, hy + 1.5, "#2a1d17", blink, ox);
      blushMouth(hy, po.mood, ox);
      accessory(look.acc, hy, true, ox, r);
    }
    return;
  }
  // kepala
  c.beginPath(); c.arc(0, hy, r, 0, 7); c.fillStyle = front ? skin : hair; c.fill();
  if (front) {
    // telinga
    c.beginPath(); c.arc(r - 1.5, hy + 1.5, 2.6, 0, 7); c.fillStyle = shade(skin, -0.06); c.fill();
    // rambut
    c.fillStyle = hair; c.beginPath();
    if (look.style === "long") {
      c.moveTo(-r - 1.5, hy + 13); c.bezierCurveTo(-r - 3, hy - 6, -r + 1, hy - r - 2, 0, hy - r - 1.5); c.bezierCurveTo(r + 1, hy - r - 2, r + 3, hy - 4, r + 1.5, hy + 13);
      c.lineTo(r - 3, hy + 13); c.bezierCurveTo(r - 2, hy + 2, r - 3, hy - 4, r - 5, hy - 6); c.bezierCurveTo(1, hy - 4, -5, hy - 4, -9, hy - 2); c.bezierCurveTo(-10, hy + 3, -10, hy + 8, -9, hy + 13); c.closePath();
    } else if (look.style === "bob") {
      c.moveTo(-r - 1.2, hy + 5); c.bezierCurveTo(-r - 2.5, hy - 8, -r + 2, hy - r - 1.5, 1, hy - r - 1.2); c.bezierCurveTo(r + 1.5, hy - r - 1, r + 2.2, hy - 4, r + 1, hy + 5);
      c.lineTo(r - 2.5, hy + 5); c.bezierCurveTo(r - 2, hy - 3, r - 4, hy - 5, r - 7, hy - 6); c.bezierCurveTo(1, hy - 4.5, -6, hy - 3, -9.5, hy - 1); c.lineTo(-10, hy + 5); c.closePath();
    } else {
      c.moveTo(-r - 0.5, hy + 1); c.bezierCurveTo(-r - 1.5, hy - 9, -r + 2, hy - r - 1.5, 1, hy - r - 1); c.bezierCurveTo(r + 1, hy - r - 1, r + 1.5, hy - 5, r, hy); c.bezierCurveTo(r - 4, hy - 6, -4, hy - 7, -r - 0.5, hy + 1); c.closePath();
    }
    c.fill();
    if (!S.pixel) { c.beginPath(); c.ellipse(-3, hy - r + 3, 4.5, 1.5, -0.25, 0, 7); c.fillStyle = "rgba(255,255,255,.22)"; c.fill(); }
    eyesFor(po.mood, true, hy + 1.8, "#2a1d17", blink, ox);
    blushMouth(hy, po.mood, ox);
  } else {
    if (look.style === "long") { c.beginPath(); c.moveTo(-r - 1.5, hy); c.lineTo(-r, hy + 13); c.lineTo(r, hy + 13); c.lineTo(r + 1.5, hy); c.closePath(); c.fillStyle = hair; c.fill(); }
    if (!S.pixel) { c.beginPath(); c.ellipse(-3, hy - 6, 5, 2.4, -0.4, 0, 7); c.fillStyle = "rgba(255,255,255,.12)"; c.fill(); }
    c.beginPath(); c.arc(-r + 0.5, hy + 2, 2.6, 0, 7); c.fillStyle = shade(skin, -0.06); c.fill();
  }
  accessory(look.acc, hy, front, ox, r);
}
function blushMouth(hy, mood, ox) {
  const c = G.c;
  c.fillStyle = "rgba(255,120,130,.32)"; c.beginPath(); c.ellipse(-8 + ox, hy + 5.5, 2.4, 1.4, 0, 0, 7); c.ellipse(5.6 + ox, hy + 5.5, 2.4, 1.4, 0, 0, 7); c.fill();
  c.strokeStyle = "#8a3b2c"; c.lineWidth = 1.3; c.lineCap = "round"; c.beginPath();
  if (mood === "sad") { c.moveTo(-2.5 + ox, hy + 8.2); c.quadraticCurveTo(-0.5 + ox, hy + 6.6, 1.5 + ox, hy + 8.2); }
  else if (mood === "happy") { c.moveTo(-2.8 + ox, hy + 6.4); c.quadraticCurveTo(-0.5 + ox, hy + 9.4, 1.8 + ox, hy + 6.4); }
  else { c.moveTo(-2 + ox, hy + 7); c.quadraticCurveTo(-0.5 + ox, hy + 8.2, 1 + ox, hy + 7); }
  c.stroke();
}
function botHead(ch, hy, front, po, t, shell, shellD, col) {
  const c = G.c, S = G.S, still = G.still, ox = -1.6;
  const sc = OST[ch.path ? "jalan2" : ch.st][2];
  // antena
  c.strokeStyle = shellD; c.lineWidth = 1.8; c.beginPath(); c.moveTo(0, hy - 11); c.lineTo(0, hy - 17); c.stroke();
  if (S.glow || ch.st === "keputusan" || ch.st === "review") { const pul = still ? 0.5 : (Math.sin(t * 5 + ch.ph * 6) + 1) / 2; c.beginPath(); c.arc(0, hy - 18.5, 5 + pul * 2, 0, 7); c.fillStyle = rgba(sc, 0.18 + pul * 0.12); c.fill(); }
  c.beginPath(); c.arc(0, hy - 18.5, 3, 0, 7); c.fillStyle = sc; c.fill();
  c.beginPath(); c.arc(-0.9, hy - 19.4, 0.9, 0, 7); c.fillStyle = "rgba(255,255,255,.7)"; c.fill();
  // telinga
  c.fillStyle = col; rr(c, -15.5, hy - 4, 4, 9, 2); c.fill(); rr(c, 11.5, hy - 4, 4, 9, 2); c.fill();
  // cangkang kepala
  rr(c, -13.5, hy - 12, 27, 23.5, 9);
  if (S.pixel) c.fillStyle = shell; else { const g = c.createLinearGradient(-13, 0, 13, 0); g.addColorStop(0, shade(shell, 0.04)); g.addColorStop(0.65, shell); g.addColorStop(1, shellD); c.fillStyle = g; }
  c.fill();
  if (front) {
    rr(c, -10.5 + ox * 0.6, hy - 7.5, 19.5, 14, 6); c.fillStyle = S.glow ? shade(col, -0.62) : shade(col, -0.5); c.fill();
    const blink = !still && ((t + ch.ph * 5) % 3.6) < 0.12;
    const ecol = S.glow ? "#e8fbff" : "#ffffff";
    if (S.glow) { c.beginPath(); c.ellipse(-1.5 + ox, hy - 0.5, 9, 5, 0, 0, 7); c.fillStyle = rgba(shade(col, 0.5), 0.28); c.fill(); }
    eyesFor(po.mood, true, hy - 0.8, ecol, blink, ox + 0.4);
    if (po.mood === "happy" || po.mood === "ok") { c.strokeStyle = rgba("#ffffff", 0.75); c.lineWidth = 1.1; c.beginPath(); c.moveTo(-3 + ox, hy + 4); c.quadraticCurveTo(-0.6 + ox, hy + (po.mood === "happy" ? 6 : 5), 1.8 + ox, hy + 4); c.stroke(); }
    if (!S.pixel) { c.beginPath(); c.ellipse(-6, hy - 10, 4, 1.2, -0.15, 0, 7); c.fillStyle = "rgba(255,255,255,.5)"; c.fill(); }
  } else {
    c.fillStyle = shellD; [-3, 0, 3].forEach((d) => { rr(c, -6, hy - 2 + d, 12, 1.4, 0.7); c.fill(); });
    c.fillStyle = col; rr(c, -13.5, hy + 6, 27, 3, 1.5); c.fill();
  }
  accessory(ch.look.acc, hy, front, ox, 13, true);
}
function accessory(a, hy, front, ox, r, bot) {
  const c = G.c, S = G.S;
  if (!a || a === "none") return;
  if (a === "headset") {
    c.strokeStyle = scol(S, "#2b2f3a"); c.lineWidth = 2.4; c.beginPath(); c.arc(0, hy, r + 1.2, Math.PI * 1.08, Math.PI * 1.92); c.stroke();
    c.fillStyle = scol(S, "#2b2f3a"); rr(c, -r - 3, hy - 2, 4.5, 8, 2); c.fill(); rr(c, r - 1.5, hy - 2, 4.5, 8, 2); c.fill();
    if (front) { c.lineWidth = 1.4; c.beginPath(); c.moveTo(-r - 0.8, hy + 5); c.quadraticCurveTo(-r + 1, hy + 10, -5 + ox, hy + 9); c.stroke(); c.beginPath(); c.arc(-5 + ox, hy + 9, 1.4, 0, 7); c.fillStyle = "#e5484d"; c.fill(); }
  } else if (a === "glasses" && front) {
    c.strokeStyle = scol(S, "#2b2f3a"); c.lineWidth = 1.2;
    const y = bot ? hy - 0.8 : hy + 1.8;
    c.beginPath(); c.arc(-4.6 + ox, y, 3.4, 0, 7); c.moveTo(3.4 + ox + 3.4, y); c.arc(3.4 + ox, y, 3.4, 0, 7); c.moveTo(-1.2 + ox, y); c.lineTo(0 + ox, y); c.stroke();
    if (!S.pixel) { c.fillStyle = "rgba(255,255,255,.18)"; c.beginPath(); c.arc(-4.6 + ox, y, 3.2, 0, 7); c.arc(3.4 + ox, y, 3.2, 0, 7); c.fill(); }
  } else if (a === "cap") {
    const col = scol(S, "#e5484d");
    c.fillStyle = col; c.beginPath(); c.arc(0, hy - 3, r + 0.6, Math.PI, 0); c.closePath(); c.fill();
    c.fillStyle = shade(col, -0.2); rr(c, front ? -r - 7 : -2, hy - 4.5, front ? 13 : 0.1, 3.6, 1.8); c.fill();
    c.fillStyle = shade(col, 0.4); c.beginPath(); c.arc(0, hy - r - 2, 1.6, 0, 7); c.fill();
  } else if (a === "beanie") {
    const col = scol(S, "#f2a541");
    c.fillStyle = col; c.beginPath(); c.arc(0, hy - 2, r + 1, Math.PI, 0); c.closePath(); c.fill();
    c.fillStyle = shade(col, -0.18); rr(c, -r - 1.5, hy - 4.5, 2 * r + 3, 4.5, 2); c.fill();
    c.fillStyle = shade(col, 0.35); c.beginPath(); c.arc(0, hy - r - 3, 3, 0, 7); c.fill();
  } else if (a === "beret") {
    const col = scol(S, "#2b2f3a");
    c.fillStyle = col; c.beginPath(); c.ellipse(1, hy - r + 2, r + 2, 5.5, -0.12, 0, 7); c.fill();
    c.beginPath(); c.arc(2, hy - r - 3, 1.4, 0, 7); c.fill();
  } else if (a === "scarf") {
    const col = scol(S, "#e5484d"); c.fillStyle = col; rr(c, -9, hy + r - 2, 18, 4.5, 2.2); c.fill(); rr(c, 3, hy + r + 1, 4, 8, 2); c.fill();
  }
}
/* Gelembung status kecil yang digambar di kanvas: mengetik, ide, kopi. */
function bubble(ch, X, Y, hy, po, t, flip) {
  const c = G.c, S = G.S, still = G.still, top = Y + hy - 14;
  const bx = X + 15, byy = top - 4;
  const bubbleBg = S.glow ? "rgba(30,36,70,.92)" : S.pixel ? "#ffffff" : "rgba(255,255,255,.96)";
  const bubbleLine = S.glow ? "rgba(255,255,255,.18)" : "rgba(30,30,60,.14)";
  const pill = (w, h) => { rr(c, bx - 2, byy - h / 2, w, h, S.pixel ? 0 : h / 2); c.fillStyle = bubbleBg; c.fill(); c.strokeStyle = S.pixel ? S.pal.outline : bubbleLine; c.lineWidth = S.pixel ? lw(1) : 1; c.stroke(); };
  if ((ch.st === "jalan" || ch.st === "pemilik") && !ch.path && po.arms === "type") {
    if (ch.who) return;
    pill(20, 11);
    for (let i = 0; i < 3; i++) { const a = still ? 0.6 : 0.3 + 0.7 * Math.max(0, Math.sin(t * 6 - i * 0.9)); c.beginPath(); c.arc(bx + 3 + i * 5, byy, 1.6, 0, 7); c.fillStyle = S.glow ? `rgba(169,169,255,${a})` : `rgba(91,91,214,${a})`; c.fill(); }
  } else if (ch.st === "rapat" && !ch.who) {
    pill(15, 15); drawIcon(c, "bulb", bx - 0.5, byy - 5.5, 11, S.glow ? "#ffd27a" : "#d38a00", 2.4);
  } else if (ch.st === "idle" && po.mood === "sleepy" && !ch.path && !S.pixel) {
    c.fillStyle = S.glow ? "rgba(220,225,255,.8)" : "rgba(90,95,130,.7)"; c.font = "600 8px Geist, system-ui, sans-serif";
    for (let i = 0; i < 2; i++) { const tt = still ? 0.4 + i * 0.3 : (t * 0.35 + i * 0.5) % 1; c.globalAlpha = Math.sin(tt * Math.PI); c.fillText("z", X + 9 + tt * 6 + i * 3, top + 2 - tt * 12); } c.globalAlpha = 1;
  } else if (ch.st === "keputusan" && !S.pixel) {
    const tt = still ? 0.3 : (t * 0.7) % 1; c.beginPath(); const sx = X + 14, sy = top + 12 + tt * 6; c.moveTo(sx, sy - 4); c.quadraticCurveTo(sx + 3, sy, sx, sy + 2); c.quadraticCurveTo(sx - 3, sy, sx, sy - 4); c.fillStyle = `rgba(120,190,255,${0.9 - tt * 0.6})`; c.fill();
  } else if (ch.st === "ok") {
    const k = still ? 1 : 0.85 + Math.sin(t * 4) * 0.15; c.beginPath(); c.arc(bx + 4, byy, 6.5 * k, 0, 7); c.fillStyle = "#30a46c"; c.fill(); c.strokeStyle = "#fff"; c.lineWidth = 1.6; c.beginPath(); c.moveTo(bx + 1, byy); c.lineTo(bx + 3.4, byy + 2.4); c.lineTo(bx + 7, byy - 2.2); c.stroke();
  }
}

/* ====================================================================== */
/* Karakter pixel (A2): sprite piksel prosedural dengan garis luar          */
/* ====================================================================== */
const SPR = {};
function pixSprite(ch, face, flip, po, frame) {
  const look = ch.look, bot = !ch.who, S = STYLES.A2;
  const key = [ch.id, look.shirt, look.acc, face, flip, po.pose, po.arms, po.mood, frame, ch.st].join("|");
  if (SPR[key]) return SPR[key];
  const Wd = 24, Ht = 38, cv = document.createElement("canvas"); cv.width = Wd; cv.height = Ht;
  const c = cv.getContext("2d");
  const R = (x, y, w, h, col) => { c.fillStyle = col; c.fillRect(x, y, w, h); };
  const shirt = look.shirt, shirtD = shade(shirt, -0.25), shirtL = shade(shirt, 0.18), skin = look.skin || "#e8b996", skinD = shade(skin, -0.15), hair = look.hair || "#2a2a2a";
  const pants = S.pal.pants, shoes = S.pal.shoes, shell = S.pal.shell, shellD = S.pal.shellD, sc = OST[ch.path ? "jalan2" : ch.st][2];
  const front = face === "f", sit = po.pose === "sit", walk = po.pose === "walk";
  const bx = 12; // garis tengah
  const B = sit ? 1 : 0; // turun saat duduk
  // kaki
  if (sit) { if (front) { R(bx - 5, 28 + B, 10, 3, pants); R(bx - 5, 31, 3, 4, pants); R(bx + 2, 31, 3, 4, pants); R(bx - 6, 35, 4, 2, shoes); R(bx + 2, 35, 4, 2, shoes); } }
  else {
    const s = walk ? (frame % 2 ? 1 : -1) : 0;
    R(bx - 4, 29, 3, 6 + s, bot ? shellD : pants); R(bx + 1, 29, 3, 6 - s, bot ? shellD : pants);
    R(bx - 5, 35 + s, 4, 2, bot ? shell : shoes); R(bx + 1, 35 - s, 4, 2, bot ? shell : shoes);
  }
  // badan
  const ty = 20 + B;
  R(bx - 5, ty, 10, 9, shirt); R(bx - 6, ty + 2, 12, 7, shirt); R(bx + 3, ty + 1, 3, 8, shirtD); R(bx - 6, ty + 2, 1, 7, shirtL);
  if (front) { if (bot) { R(bx - 2, ty + 3, 4, 3, shade(shirt, 0.7)); R(bx - 1, ty + 4, 2, 1, shirt); } else { R(bx - 1, ty, 2, 2, skin); } }
  // lengan
  const a = po.arms, f2 = frame % 2;
  const handC = bot ? shell : skin;
  if (!front || a === "idle" || a === "walk") { const s = walk ? (f2 ? 1 : -1) : 0; R(bx - 8, ty + 2, 2, 6 + s, shirtD); R(bx - 8, ty + 8 + s, 2, 2, handC); R(bx + 6, ty + 2, 2, 6 - s, shirtD); R(bx + 6, ty + 8 - s, 2, 2, handC); }
  else if (a === "type") { R(bx - 7, ty + 2, 2, 4, shirtD); R(bx - 6, ty + 6 - f2, 3, 2, handC); R(bx + 5, ty + 2, 2, 4, shirtD); R(bx + 3, ty + 6 - (1 - f2), 3, 2, handC); }
  else if (a === "raise") { R(bx - 8, ty + 2, 2, 6, shirtD); R(bx - 8, ty + 8, 2, 2, handC); }
  else if (a === "mug") { R(bx + 6, ty + 2, 2, 6, shirtD); R(bx + 6, ty + 8, 2, 2, handC); R(bx - 7, ty + 2, 2, 4, shirtD); R(bx - 6, ty + 3, 3, 4, "#ffffff"); R(bx - 4, ty + 6, 2, 2, handC); }
  else if (a === "tablet") { R(bx - 7, ty + 2, 2, 4, shirtD); R(bx + 5, ty + 2, 2, 4, shirtD); R(bx - 5, ty + 3, 10, 5, "#2c3140"); R(bx - 4, ty + 4, 8, 3, "#8fc0ff"); R(bx - 6, ty + 6, 2, 2, handC); R(bx + 4, ty + 6, 2, 2, handC); }
  else if (a === "point") { R(bx - 8, ty + 2, 2, 6, shirtD); R(bx - 8, ty + 8, 2, 2, handC); }
  // kepala
  const hy = 5 + B;
  if (bot) {
    R(bx, hy - 4, 1, 3, shellD); R(bx - 1, hy - 6, 3, 2, sc);
    R(bx - 7, hy, 14, 13, shell); R(bx - 6, hy - 1, 12, 1, shell); R(bx - 6, hy + 13, 12, 1, shell); R(bx + 4, hy, 3, 13, shellD); R(bx - 8, hy + 4, 1, 5, shirt); R(bx + 7, hy + 4, 1, 5, shirt);
    if (front) {
      R(bx - 6, hy + 3, 11, 7, shade(shirt, -0.5));
      const e = po.mood === "sleepy" ? 1 : 2, ey = hy + 5 + (po.mood === "happy" ? 0 : 0);
      if (po.mood === "happy") { R(bx - 5, ey, 3, 1, "#fff"); R(bx + 1, ey, 3, 1, "#fff"); R(bx - 5, ey + 1, 1, 1, "#fff"); R(bx - 3, ey + 1, 1, 1, "#fff"); R(bx + 1, ey + 1, 1, 1, "#fff"); R(bx + 3, ey + 1, 1, 1, "#fff"); }
      else { R(bx - 4, ey, 2, e, "#fff"); R(bx + 1, ey, 2, e, "#fff"); }
      if (po.mood === "sad") { R(bx - 5, ey - 2, 2, 1, "#fff"); R(bx + 2, ey - 2, 2, 1, "#fff"); }
    } else { R(bx - 4, hy + 4, 8, 1, shellD); R(bx - 4, hy + 6, 8, 1, shellD); R(bx - 7, hy + 10, 14, 2, shirt); }
  } else if (look.style === "hijab") {
    const hd = shade(hair, -0.18), hl = shade(hair, 0.2);
    R(bx - 5, hy - 2, 10, 1, hair); R(bx - 7, hy - 1, 14, 1, hair); R(bx - 8, hy, 16, 13, hair); R(bx - 7, hy + 13, 14, 2, hair); R(bx - 8, hy + 15, 16, 3, hair);
    R(bx + 5, hy, 3, 13, hd); R(bx + 5, hy + 15, 3, 3, hd); R(bx - 6, hy - 1, 4, 1, hl); R(bx - 7, hy, 1, 4, hl);
    if (front) { R(bx - 5, hy + 3, 9, 9, skin); R(bx - 4, hy + 2, 7, 1, skin); R(bx - 4, hy + 12, 7, 1, skin); R(bx - 5, hy + 3, 1, 1, hair); R(bx + 3, hy + 3, 1, 1, hair); R(bx + 3, hy + 11, 1, 1, hair); R(bx - 5, hy + 11, 1, 1, hair); R(bx + 2, hy + 4, 2, 7, skinD); }
    else { R(bx - 1, hy + 4, 1, 9, hd); }
  } else {
    R(bx - 7, hy, 14, 13, front ? skin : hair); R(bx - 6, hy - 1, 12, 1, front ? skin : hair); R(bx - 6, hy + 13, 12, 1, front ? skin : hair);
    if (front) {
      R(bx + 4, hy + 2, 3, 10, skinD);
      R(bx - 7, hy - 1, 14, 4, hair); R(bx - 8, hy + 1, 2, 6, hair); R(bx + 5, hy + 1, 3, 5, hair); R(bx - 5, hy + 3, 4, 1, hair); R(bx - 5, hy - 1, 4, 1, shade(hair, 0.3));
      if (look.style === "long") { R(bx - 9, hy + 3, 3, 12, hair); R(bx + 6, hy + 3, 3, 12, hair); }
      if (look.style === "bob") { R(bx - 9, hy + 3, 3, 7, hair); R(bx + 6, hy + 3, 3, 7, hair); }
    } else {
      R(bx - 7, hy - 1, 14, 2, hair); R(bx - 4, hy + 1, 5, 1, shade(hair, 0.3)); R(bx - 5, hy + 2, 2, 1, shade(hair, 0.3)); R(bx + 4, hy + 1, 3, 12, shade(hair, -0.2));
      R(bx - 8, hy + 6, 1, 3, skinD); R(bx + 7, hy + 6, 1, 3, skinD);
      if (look.style === "long") { R(bx - 8, hy + 10, 16, 6, hair); R(bx + 4, hy + 10, 4, 6, shade(hair, -0.2)); }
      else { R(bx - 4, hy + 13, 8, 1, skin); }
    }
  }
  if (front && !bot) {
    const ey = hy + 7;
    if (po.mood === "happy") { R(bx - 5, ey, 1, 1, "#2a1d17"); R(bx - 4, ey - 1, 1, 1, "#2a1d17"); R(bx - 3, ey, 1, 1, "#2a1d17"); R(bx, ey, 1, 1, "#2a1d17"); R(bx + 1, ey - 1, 1, 1, "#2a1d17"); R(bx + 2, ey, 1, 1, "#2a1d17"); }
    else if (po.mood === "sleepy") { R(bx - 5, ey, 3, 1, "#2a1d17"); R(bx, ey, 3, 1, "#2a1d17"); }
    else { R(bx - 4, ey - 1, 2, 2, "#2a1d17"); R(bx + 1, ey - 1, 2, 2, "#2a1d17"); }
    if (po.mood === "sad") { R(bx - 5, ey - 3, 2, 1, "#2a1d17"); R(bx + 2, ey - 3, 2, 1, "#2a1d17"); }
    R(bx - 6, ey + 2, 2, 1, "#f19aa1"); R(bx + 3, ey + 2, 2, 1, "#f19aa1");
    R(bx - 2, ey + 3, 2, 1, "#8a3b2c");
  }
  // lengan yang terangkat digambar di atas kepala
  if (front && a === "raise") { R(bx + 6, ty, 2, 3, shirtD); R(bx + 8 - f2, ty - 7, 2, 8, shirtD); R(bx + 8 - f2, ty - 10, 3, 3, handC); }
  if (front && a === "head") { R(bx - 8, ty, 2, 2, shirtD); R(bx - 10, ty - 6, 2, 7, shirtD); R(bx - 11, ty - 9, 3, 3, handC); R(bx + 6, ty, 2, 2, shirtD); R(bx + 8, ty - 6, 2, 7, shirtD); R(bx + 8, ty - 9, 3, 3, handC); }
  if (front && a === "point") { R(bx + 6, ty, 2, 3, shirtD); R(bx + 8, ty - 2, 2, 2, shirtD); R(bx + 10, ty - 4, 2, 2, shirtD); R(bx + 11, ty - 6, 2, 2, handC); }
  // aksesori
  const acc = look.acc;
  if (acc === "headset") { R(bx - 8, hy - 1, 16, 1, "#2b2f3a"); R(bx - 9, hy + 4, 2, 4, "#2b2f3a"); R(bx + 7, hy + 4, 2, 4, "#2b2f3a"); if (front) { R(bx - 7, hy + 9, 1, 2, "#2b2f3a"); R(bx - 6, hy + 11, 2, 1, "#e5484d"); } }
  if (acc === "glasses" && front) { const ey = bot ? hy + 5 : hy + 6; R(bx - 6, ey - 1, 5, 1, "#2b2f3a"); R(bx, ey - 1, 5, 1, "#2b2f3a"); R(bx - 6, ey + 2, 5, 1, "#2b2f3a"); R(bx, ey + 2, 5, 1, "#2b2f3a"); R(bx - 6, ey, 1, 2, "#2b2f3a"); R(bx + 4, ey, 1, 2, "#2b2f3a"); R(bx - 1, ey, 1, 1, "#2b2f3a"); }
  if (acc === "cap") { R(bx - 7, hy - 2, 14, 4, "#e5484d"); R(bx - 8, hy, 16, 2, "#e5484d"); if (front) R(bx - 11, hy + 1, 6, 2, "#b3363a"); }
  if (acc === "beanie") { R(bx - 7, hy - 3, 14, 5, "#f2a541"); R(bx - 8, hy + 1, 16, 2, "#c98a2c"); R(bx - 1, hy - 5, 3, 2, "#ffd38a"); }
  if (acc === "beret") { R(bx - 7, hy - 3, 15, 3, "#2b2f3a"); R(bx, hy - 4, 2, 1, "#2b2f3a"); }
  if (acc === "scarf") { R(bx - 5, ty, 10, 2, "#e5484d"); R(bx + 2, ty + 2, 2, 4, "#e5484d"); }
  // garis luar
  const img = c.getImageData(0, 0, Wd, Ht), d = img.data, out = new Uint8ClampedArray(d), oc = h2r(S.pal.outline);
  for (let y = 0; y < Ht; y++) for (let x = 0; x < Wd; x++) {
    const i = (y * Wd + x) * 4; if (d[i + 3]) continue;
    const n = (xx, yy) => xx >= 0 && yy >= 0 && xx < Wd && yy < Ht && d[(yy * Wd + xx) * 4 + 3] > 0;
    if (n(x - 1, y) || n(x + 1, y) || n(x, y - 1) || n(x, y + 1)) { out[i] = oc[0]; out[i + 1] = oc[1]; out[i + 2] = oc[2]; out[i + 3] = 255; }
  }
  c.putImageData(new ImageData(out, Wd, Ht), 0, 0);
  let res = cv;
  if (flip) { const f = document.createElement("canvas"); f.width = Wd; f.height = Ht; const fc = f.getContext("2d"); fc.translate(Wd, 0); fc.scale(-1, 1); fc.drawImage(cv, 0, 0); res = f; }
  return (SPR[key] = { cv: res, ax: 12, ay: 37 });
}
function drawCharP(ch, X, Y, face, flip, po, t, sel) {
  const c = G.c, S = G.S, still = G.still;
  const frame = still ? 0 : po.pose === "walk" ? Math.floor(t * 6) : po.arms === "type" ? Math.floor(t * 4 + ch.ph * 4) : po.arms === "raise" ? Math.floor(t * 2.5) : 0;
  const sp = pixSprite(ch, face, flip, po, frame);
  if (sel) { c.save(); c.translate(X, Y); c.scale(1, 0.5); c.beginPath(); c.arc(0, 0, 22, 0, 7); c.fillStyle = "rgba(91,91,214,.25)"; c.fill(); c.lineWidth = lw(1) * 2; c.strokeStyle = "#5b5bd6"; c.stroke(); c.restore(); }
  if (po.pose !== "sit") blob(X, Y, 11, 5, "rgba(27,21,48,.22)");
  // gambar sprite pada grid piksel buffer
  // sprite dirancang untuk skala buffer 0,55; skala lain memakai nearest-neighbor
  let k = G.sc / 0.7; if (k > 0.74 && k < 1.36) k = 1;
  const fx = Math.round(G.ox + X * G.sc), fy = Math.round(G.oy + Y * G.sc) + (po.pose === "sit" ? 1 : 0);
  const w = Math.round(sp.cv.width * k), h = Math.round(sp.cv.height * k), sx = fx - Math.round(sp.ax * k), sy = fy - Math.round(sp.ay * k);
  c.save(); c.setTransform(1, 0, 0, 1, 0, 0); c.imageSmoothingEnabled = false; c.drawImage(sp.cv, sx, sy, w, h); c.restore();
  if (k >= 0.8) bubbleP(ch, sx + w - 3, sy + 2, po, t);
}
function bubbleP(ch, bx, by, po, t) {
  const c = G.c; c.save(); c.setTransform(1, 0, 0, 1, 0, 0);
  const R = (x, y, w, h, col) => { c.fillStyle = col; c.fillRect(x, y, w, h); };
  if ((ch.st === "jalan") && !ch.path && po.arms === "type") {
    R(bx - 1, by - 1, 13, 7, "#2a2238"); R(bx, by, 11, 5, "#ffffff");
    const k = G.still ? 3 : Math.floor(t * 3) % 4; for (let i = 0; i < 3; i++) R(bx + 2 + i * 3, by + 2, 1, 1, i < k ? "#5b5bd6" : "#c9cfdb");
  } else if (ch.st === "rapat" && !ch.who) { R(bx - 1, by - 1, 9, 9, "#2a2238"); R(bx, by, 7, 7, "#ffffff"); R(bx + 2, by + 1, 3, 3, "#ffd166"); R(bx + 3, by + 4, 1, 2, "#8c6d1f"); }
  else if (ch.st === "ok") { R(bx - 1, by - 1, 9, 9, "#2a2238"); R(bx, by, 7, 7, "#30a46c"); R(bx + 1, by + 3, 1, 1, "#fff"); R(bx + 2, by + 4, 1, 1, "#fff"); R(bx + 3, by + 3, 1, 1, "#fff"); R(bx + 4, by + 2, 1, 1, "#fff"); R(bx + 5, by + 1, 1, 1, "#fff"); }
  else if (ch.st === "idle" && po.mood === "sleepy" && !G.still) { const k = Math.floor(t * 2) % 3; R(bx + k, by - k * 2, 3, 1, "#7d86a8"); R(bx + 2 + k, by + 1 - k * 2, 1, 1, "#7d86a8"); R(bx + k, by + 2 - k * 2, 3, 1, "#7d86a8"); }
  c.restore();
}

/* ====================================================================== */
/* Render adegan                                                           */
/* ====================================================================== */
const WB = (() => { const xs = [P(0, D)[0] - 12, P(W, 0)[0] + 12]; return { x0: xs[0], x1: xs[1], y0: P(0, 0, WH)[1] - 60, y1: P(W, D, -0.6)[1] + 54 }; })();
const isDark = () => { const r = document.documentElement; return r.dataset.theme ? r.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches; };
const RM_BOX = (r) => r.top ? [r.r[0], r.r[1], r.r[2], r.r[3], WH] : [r.r[0], r.r[1], r.r[2], r.r[3], 1.4];

function charPos(ch) { return [ch.x, ch.y]; }
function charFacing(ch) {
  if (ch.path && ch.dir) return FACE[ch.dir];
  const s = SEATS[ch.seat]; return FACE[(s && s.f) || "+y"];
}
/* Gambar seluruh kantor ke context c. V = { sc, ox, oy } transform dunia → piksel kanvas. */
function drawScene(c, S, V, t, opt = {}) {
  G = { c, S, sc: V.sc, ox: V.ox, oy: V.oy, t, dark: opt.dark, still: !!opt.still };
  const cw = c.canvas.width, chh = c.canvas.height;
  c.setTransform(1, 0, 0, 1, 0, 0);
  // latar
  const dark = opt.dark && !S.glow;
  const g = c.createRadialGradient(cw * 0.5, chh * 0.42, 0, cw * 0.5, chh * 0.5, Math.max(cw, chh) * 0.75);
  g.addColorStop(0, dark ? S.pal.bgDA : S.pal.bgA); g.addColorStop(1, dark ? S.pal.bgDB : S.pal.bgB);
  c.fillStyle = S.pixel ? (dark ? S.pal.bgDB : S.pal.bgB) : g; c.fillRect(0, 0, cw, chh);
  if (S.pixel) { c.fillStyle = dark ? "#1f2540" : "#b9c8d9"; for (let x = (Math.round(V.ox) % 8 + 8) % 8; x < cw; x += 8) for (let y = (Math.round(V.oy) % 8 + 8) % 8; y < chh; y += 8) c.fillRect(x, y, 1, 1); }
  if (S.glow) { for (let i = 0; i < 70; i++) { const x = hash(i, 1) * cw, y = hash(i, 2) * chh, a = 0.25 + hash(i, 3) * 0.5; c.fillStyle = `rgba(220,226,255,${a})`; c.fillRect(x, y, 1.2, 1.2); } }
  c.setTransform(V.sc, 0, 0, V.sc, V.ox, V.oy);
  // alas bangunan + bayangan besar
  const slabPts = [P(-0.25, -0.25, 0), P(W, -0.25, 0), P(W, -0.25, -0.6), P(W, D, -0.6), P(-0.25, D, -0.6), P(-0.25, D, 0)];
  c.save();
  if (!S.pixel) { c.shadowColor = S.glow ? "rgba(0,0,0,.6)" : dark ? "rgba(0,0,0,.5)" : "rgba(60,70,120,.22)"; c.shadowBlur = 46 * V.sc; c.shadowOffsetY = 26 * V.sc; }
  fillP(slabPts, mat(S, "slab").r); c.restore();
  box(-0.25, -0.25, -0.6, W + 0.25, D + 0.25, 0.6, "slab", { edge: 0.4 });
  // dinding belakang
  box(-0.25, -0.25, 0, 0.25, D + 0.25, WH, "wallL", { edge: 0.6 });
  box(0, -0.25, 0, W, 0.25, WH, "wall", { edge: 0.6 });
  wallPaint();
  // lantai
  ROOMS.forEach((r) => floorArea(...r.r, r.fl, ...r.f));
  floorArea(...COR, "wood", "fCOR", "fCOR2");
  lightPatches();
  rug(1.4, 2.3, 7.4, 7.7, "rugB", "rugB2"); rug(9.4, 2.3, 15.4, 7.7, "rugB", "rugB2");
  rug(18.7, 15.5, 21.5, 19.3, "rug", "rug2");
  // bayangan statis perabot
  furnitureShadows();
  // jendela dan dekor dinding
  windowOn("back", 1.0, 3.6); windowOn("back", 13.9, 16.0); windowOn("back", 24.0, 26.4, 1.15, 2.55);
  windowOn("left", 1.2, 3.8); windowOn("left", 18.6, 21.0);
  kanban(4.6, 12.3, 0.9, 2.5); tvWall(19.5, 23.2, 1.05, 2.4); wallClock(13.15, 2.0);
  doorLeft(11.4, 12.6);
  // objek terurut
  const list = OBJ.slice();
  CH.forEach((ch) => { const [x, y] = charPos(ch); list.push({ k: x + y + 0.01, rm: roomOf(ch), ch }); });
  list.sort((a, b) => a.k - b.k);
  for (const o of list) {
    if (o.ch) {
      const ch = o.ch, [x, y] = charPos(ch), [face, flip] = charFacing(ch), po = poseOf(ch);
      const [X, Y] = P(x, y, 0);
      (S.pixel ? drawCharP : drawCharV)(ch, X, Y, face, flip, po, t, opt.sel === ch.id);
    } else o.draw();
  }
  if (S.glow) lights(c, S, V, t);
  // fokus ruangan: tabir di luar ruangan terpilih
  if (opt.room && opt.room !== "all") {
    const r = room(opt.room); const [x0, y0, x1, y1, h] = RM_BOX(r);
    c.save(); c.setTransform(1, 0, 0, 1, 0, 0); c.beginPath(); c.rect(0, 0, cw, chh);
    c.setTransform(V.sc, 0, 0, V.sc, V.ox, V.oy);
    const hx = [P(x0, y0, h), P(x1, y0, h), P(x1, y0, 0), P(x1, y1, 0), P(x0, y1, 0), P(x0, y1, h)];
    c.moveTo(...hx[0]); hx.slice(1).forEach((p) => c.lineTo(...p)); c.closePath();
    c.fillStyle = S.glow ? "rgba(8,10,24,.62)" : opt.dark ? "rgba(17,17,21,.6)" : S.pixel ? "rgba(207,219,232,.7)" : "rgba(241,244,251,.66)"; c.fill("evenodd"); c.restore();
  }
  c.setTransform(1, 0, 0, 1, 0, 0);
}
function wallPaint() {
  const S = G.S;
  // lis bawah, wainscot warna ruangan, lis atas
  const seg = (side, a, b, col) => {
    const Pw = (u, z) => (side === "back" ? P(u, 0, z) : P(0, u, z));
    const t = tonesOf(S, scol(S, col));
    fillP([Pw(a, 0), Pw(b, 0), Pw(b, 0.95), Pw(a, 0.95)], side === "back" ? t.l : t.r);
    fillP([Pw(a, 0.95), Pw(b, 0.95), Pw(b, 1.0), Pw(a, 1.0)], side === "back" ? shade(t.l, 0.25) : shade(t.r, 0.2));
    fillP([Pw(a, 0), Pw(b, 0), Pw(b, 0.1), Pw(a, 0.1)], side === "back" ? tonesOf(S, S.pal.base).l : tonesOf(S, S.pal.base).r);
  };
  const tint = (c) => (S.pixel ? mix(c, "#ffffff", 0.62) : mix(c, "#ffffff", 0.8));
  seg("back", 0, 17, tint("#7c3aed")); seg("back", 17, 27, tint("#0891b2"));
  seg("left", 0, 11, tint("#7c3aed")); seg("left", 11, 13, mix(S === STYLES.A3 ? PAL1.fCOR : S.pal.fCOR, "#ffffff", 0.2)); seg("left", 13, 22, tint("#0ea5e9"));
  // garis sambungan dinding di sudut
  line(P(0, 0, 0), P(0, 0, WH), rgba("#000000", 0.08), 1);
}
/* Cahaya masuk dari jendela (A1) atau cahaya bulan (A3). */
function lightPatches() {
  const S = G.S, c = G.c; if (S.pixel) return;
  const col = S.glow ? "rgba(150,180,255,0.07)" : "rgba(255,248,215,0.55)";
  c.save(); if (!S.glow) c.globalCompositeOperation = "soft-light";
  [[1.0, 3.6], [13.9, 16.0], [24.0, 26.4]].forEach(([a, b]) => { const pts = [P(a + 0.3, 0), P(b + 0.3, 0), P(b + 1.6, 3.4), P(a + 1.6, 3.4)]; const g = c.createLinearGradient(...P((a + b) / 2, 0), ...P((a + b) / 2 + 1, 3.4)); g.addColorStop(0, col); g.addColorStop(1, "rgba(255,248,215,0)"); fillP(pts, g); });
  [[1.2, 3.8], [18.6, 21.0]].forEach(([a, b]) => { const pts = [P(0, a + 0.3), P(0, b + 0.3), P(3.4, b + 1.6), P(3.4, a + 1.6)]; const g = c.createLinearGradient(...P(0, (a + b) / 2), ...P(3.4, (a + b) / 2 + 1)); g.addColorStop(0, col); g.addColorStop(1, "rgba(255,248,215,0)"); fillP(pts, g); });
  c.restore();
}
function furnitureShadows() {
  const S = G.S, c = G.c;
  const sh = (x0, y0, x1, y1, a = 1) => softShadow([P(x0 + 0.12, y0 + 0.12), P(x1 + 0.35, y0 + 0.12), P(x1 + 0.35, y1 + 0.3), P(x0 + 0.12, y1 + 0.3)], a, 0.12);
  [[2, 4, 6.8, 6], [10, 4, 14.8, 6], [3.4, 15, 5.8, 16], [3.4, 18.6, 5.8, 19.6], [10, 15, 14.8, 16]].forEach((r) => sh(...r, 0.9));
  sh(0, 6.6, 0.55, 9.4); sh(0, 13.9, 0.7, 17.5); sh(15.3, 7.3, 16.6, 8.2); sh(15.4, 9.4, 16.6, 10.4);
  sh(20.2, 13.15, 25.0, 14.05); sh(25.3, 13.15, 26.4, 14.1); sh(23.9, 0.05, 26.6, 0.65);
}
/* Cahaya malam: layar, lampu meja, TV, lampu lantai. Digabung secara aditif. */
function lights(c, S, V, t) {
  c.save(); c.globalCompositeOperation = "lighter";
  const glow = (x, y, z, r, col, a) => { const [X, Y] = P(x, y, z); const g = c.createRadialGradient(X, Y, 0, X, Y, r); g.addColorStop(0, rgba(col, a)); g.addColorStop(0.4, rgba(col, a * 0.45)); g.addColorStop(1, rgba(col, 0)); c.fillStyle = g; c.beginPath(); c.arc(X, Y, r, 0, 7); c.fill(); };
  const pool = (x, y, rx, col, a) => { const [X, Y] = P(x, y, 0); c.save(); c.translate(X, Y); c.scale(1, 0.5); const g = c.createRadialGradient(0, 0, 0, 0, 0, rx); g.addColorStop(0, rgba(col, a)); g.addColorStop(1, rgba(col, 0)); c.fillStyle = g; c.beginPath(); c.arc(0, 0, rx, 0, 7); c.fill(); c.restore(); };
  const fl = (k) => (G.still ? 1 : 0.92 + Math.sin(t * 3 + k) * 0.05 + Math.sin(t * 17 + k * 3) * 0.03);
  // layar komputer di KSR (meja dengan penghuni)
  [["k1", 3.2, 4.85, "n"], ["k2", 5.6, 4.85, "n"], ["k3", 3.2, 5.2, "s"], ["k4", 5.6, 5.2, "s"], ["k5", 11.2, 4.85, "n"], ["k6", 13.6, 4.85, "n"], ["k7", 11.2, 5.2, "s"], ["k8", 13.6, 5.2, "s"]].forEach(([sid, x, y, m], i) => {
    const occ = seatOcc(sid); if (!occ) return;
    const col = occ.st === "keputusan" ? "#ff6b6b" : occ.st === "review" ? "#ffb04d" : "#6aa8ff";
    glow(x, m === "s" ? y + 0.9 : y - 0.9, 0.9, 50, col, 0.3 * fl(i));
    pool(x, m === "s" ? y + 1.2 : y - 1.3, 64, col, 0.26);
  });
  // lampu meja hangat di tiap pod
  [[2.47, 4.37], [10.47, 4.37]].forEach(([x, y], i) => { glow(x, y, 1.3, 70, "#ffbf66", 0.55 * fl(i + 9)); pool(x + 0.3, y + 0.3, 120, "#ffa94d", 0.3); });
  // lampu meja rapat dan TV
  pool(22.4, 5.4, 190, "#ffb05c", 0.2);
  if (CH.some((ch) => ch.st === "rapat")) { glow(21.35, 0.2, 1.7, 90, "#7aa7ff", 0.4); pool(21.35, 1.8, 110, "#6f9bff", 0.2); }
  // lampu lantai
  [[16.45, 13.65], [17.45, 15.35], [26.45, 7.65]].forEach(([x, y], i) => { glow(x, y, 1.82, 60, "#ffd27a", 0.65); pool(x, y, 150, "#ffa94d", 0.32); });
  glow(20.55, 13.75, 1.48, 14, "#6bff9e", 0.5);
  pool(21.2, 17.4, 220, "#ffae55", 0.12);
  // kulkas dan dispenser
  glow(26.05, 14.1, 1.7, 18, "#ffd166", 0.3); glow(26.28, 11.6, 1.2, 26, "#7fd0ff", 0.25);
  // jendela memantulkan cahaya bulan
  c.restore();
  // vinyet
  c.save(); c.setTransform(1, 0, 0, 1, 0, 0); const cw = c.canvas.width, chh = c.canvas.height; const v = c.createRadialGradient(cw / 2, chh / 2, Math.min(cw, chh) * 0.35, cw / 2, chh / 2, Math.max(cw, chh) * 0.8); v.addColorStop(0, "rgba(0,0,0,0)"); v.addColorStop(1, "rgba(0,0,10,.45)"); c.fillStyle = v; c.fillRect(0, 0, cw, chh); c.restore();
  c.setTransform(V.sc, 0, 0, V.sc, V.ox, V.oy);
}

/* ---------- Kuantisasi warna untuk gaya pixel ---------- */
let QPAL = null; const QC = new Map();
function pixelPalette() {
  if (QPAL) return QPAL;
  const S = STYLES.A2, set = new Set();
  const addc = (h) => { if (typeof h === "string" && h[0] === "#") set.add(h.toLowerCase()); };
  Object.values(S.pal).forEach((v) => { addc(v); if (typeof v === "string" && v[0] === "#") { const t = tonesOf(S, v); addc(t.l); addc(t.r); addc(shade(v, 0.3)); } });
  ["fKSR", "fKSR2", "fMEET", "fMEET2", "fCOR", "fCOR2", "fGDG", "fGDG2", "fPPL", "fPPL2", "fPAN", "fPAN2", "rug", "rug2", "rugB", "rugB2"].forEach((k) => { addc(shade(S.pal[k], -0.14)); addc(mix(S.pal[k], S.pal.glass, 0.35)); addc(mix(S.pal[k], "#000000", 0.08)); });
  ["wall", "wallL", "half"].forEach((k) => { const t = tonesOf(S, S.pal[k]); addc(mix(t.l, S.pal.glass, 0.35)); addc(mix(t.r, S.pal.glass, 0.35)); });
  Object.values(AG.PERSONAS).forEach((p) => { [p.c, shade(p.c, -0.25), shade(p.c, 0.18), shade(p.c, -0.5), shade(p.c, 0.7), tonesOf(S, p.c).l, tonesOf(S, p.c).r].forEach(addc); });
  Object.values(AG.PEOPLE).forEach((p) => { [p.c, shade(p.c, -0.25), shade(p.c, 0.18)].forEach(addc); });
  Object.values(HUMAN).forEach((h) => [h.skin, h.hair, shade(h.skin, -0.15), shade(h.hair, -0.15)].forEach(addc));
  Object.values(OST).forEach((s) => addc(s[2]));
  ["#ffffff", "#2a2238", "#2b2f3a", "#e5484d", "#b3363a", "#f2a541", "#c98a2c", "#ffd38a", "#ffd166", "#8c6d1f", "#30a46c", "#5b5bd6", "#c9cfdb", "#7d86a8", "#8fc0ff", "#2c3140", "#f19aa1", "#8a3b2c", "#2a1d17", "#7aa7ff", "#ffcf6b", "#7ee0b0", "#ff99c2", "#c9d1ff", "#ffb347", "#ff6b6b", "#22a565", "#e9dcc6", "#9cc4e4", "#33508f", "#ffe28a", "#b9f0c9", "#ffc0d0", "#bfe0ff", "#cfd0ff", "#ffe2b0", "#ffc6c6", "#cfe3ff", "#d5f2df", "#eceef3", "#1f2540", "#b9c8d9", "#9b7ede", "#ff8a5c", "#7cc576", "#f2e4c9", "#e07a5f", "#3d5a80", "#81b29a", "#f2cc8f", "#ef8fa8", "#5fb0c9", "#6b7080", "#3b404d", "#454b59", "#dcdff5"].forEach(addc);
  return (QPAL = [...set].map(h2r));
}
function quantize(ctx, w, h) {
  const pal = pixelPalette(), img = ctx.getImageData(0, 0, w, h), d = img.data;
  for (let i = 0; i < d.length; i += 4) {
    const key = (d[i] << 16) | (d[i + 1] << 8) | d[i + 2];
    let v = QC.get(key);
    if (v === undefined) {
      let best = 0, bd = 1e9; const r = d[i], g = d[i + 1], b = d[i + 2];
      for (let j = 0; j < pal.length; j++) { const p = pal[j], dr = r - p[0], dg = g - p[1], db = b - p[2], dd = dr * dr * 2 + dg * dg * 4 + db * db * 3; if (dd < bd) { bd = dd; best = j; } }
      const p = pal[best]; v = (p[0] << 16) | (p[1] << 8) | p[2]; QC.set(key, v);
    }
    d[i] = (v >> 16) & 255; d[i + 1] = (v >> 8) & 255; d[i + 2] = v & 255; d[i + 3] = 255;
  }
  ctx.putImageData(img, 0, 0);
}
/* Render ke kanvas tampilan. cam = { z, cx, cy } (cx, cy = koordinat dunia di tengah). */
const BUF = document.createElement("canvas");
const BCTX = BUF.getContext("2d", { willReadFrequently: true });
function paint(canvas, styleKey, cam, t, opt = {}) {
  const S = STYLES[styleKey] || STYLES.A1, ctx = canvas.getContext("2d"), dpr = opt.dpr || 1;
  const cw = canvas.width, chh = canvas.height;
  if (S.pixel) {
    const PX = Math.max(2, Math.round((cam.z * dpr) / 0.7));
    const bw = Math.ceil(cw / PX), bh = Math.ceil(chh / PX);
    if (BUF.width !== bw || BUF.height !== bh) { BUF.width = bw; BUF.height = bh; }
    const sc = (cam.z * dpr) / PX;
    drawScene(BCTX, S, { sc, ox: Math.round(bw / 2 - cam.cx * sc), oy: Math.round(bh / 2 - cam.cy * sc) }, t, opt);
    quantize(BCTX, bw, bh);
    ctx.imageSmoothingEnabled = false; ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.drawImage(BUF, 0, 0, bw * PX, bh * PX);
    return { sc: cam.z * dpr, ox: Math.round(bw / 2 - cam.cx * sc) * PX, oy: Math.round(bh / 2 - cam.cy * sc) * PX };
  }
  const sc = cam.z * dpr, V = { sc, ox: cw / 2 - cam.cx * sc, oy: chh / 2 - cam.cy * sc };
  drawScene(ctx, S, V, t, opt);
  return V;
}

/* ====================================================================== */
/* Pergerakan karakter                                                     */
/* ====================================================================== */
const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
function routeBetween(fromSeat, toSeat, fromXY) {
  const A = SEATS[fromSeat], B = SEATS[toSeat];
  const pts = [];
  (fromXY ? [] : A.out).forEach((p) => pts.push(p));
  const ra = A.room, rb = B.room;
  const da = [doorX(ra), DOORS[ra][2]], db = [doorX(rb), DOORS[rb][2]];
  if (ra !== rb) { pts.push([da[0], 12]); pts.push([db[0], 12]); B.out.slice().reverse().forEach((p) => pts.push(p)); }
  else { B.out.slice().reverse().forEach((p) => pts.push(p)); }
  pts.push([B.x, B.y]);
  return pts;
}
function startWalk(c, toSeat, onArrive) {
  const pts = routeBetween(c.seat, toSeat);
  c.seatTo = toSeat; c.onArrive = onArrive;
  if (reduced()) { arrive(c); return; }
  c.path = pts; c.pi = 0;
}
function arrive(c) {
  const s = SEATS[c.seatTo]; c.seat = c.seatTo; c.x = s.x; c.y = s.y; c.path = null; c.room = s.room; c.dir = null;
  const fn = c.onArrive; c.onArrive = null; c.seatTo = null; fn && fn(c);
}
function stepWalkers(dt) {
  let moving = false;
  CH.forEach((c) => {
    if (!c.path) return; moving = true;
    let rem = dt * 2.4;
    while (rem > 0 && c.path) {
      const [tx, ty] = c.path[c.pi], dx = tx - c.x, dy = ty - c.y, dist = Math.hypot(dx, dy);
      if (dist > 0.001) c.dir = Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? "+x" : "-x") : dy > 0 ? "+y" : "-y";
      if (dist <= rem) { c.x = tx; c.y = ty; rem -= dist; c.pi++; if (c.pi >= c.path.length) { if (c.wanderBack) { const w = c.wanderBack; c.wanderBack = null; c.path = null; c.pause = 3.2; c.wanderNext = w; c.dir = "+x"; } else arrive(c); } }
      else { c.x += (dx / dist) * rem; c.y += (dy / dist) * rem; rem = 0; }
    }
  });
  return moving;
}
/* Agent DevOps sesekali berjalan ke dispenser di lorong lalu kembali ke pantry. */
let wanderT = 7;
function stepWander(dt) {
  const c = chr("ops"); if (!c || reduced() || c.st !== "idle" || c.seat !== "p2") return;
  if (c.pause > 0) { c.pause -= dt; if (c.pause <= 0 && c.wanderNext) { c.path = c.wanderNext; c.pi = 0; c.wanderNext = null; c.seatTo = "p2"; c.onArrive = null; } return; }
  if (c.path) return;
  wanderT -= dt;
  if (wanderT > 0) return;
  wanderT = 26;
  c.path = [[18.8, 15.0], [18.8, 13], [18.8, 12.1], [25.6, 12.1]]; c.pi = 0;
  c.wanderBack = [[18.8, 12.1], [18.8, 13], [18.8, 15.0], [SEATS.p2.x, SEATS.p2.y]];
}

/* ====================================================================== */
/* Instans kantor: kanvas, kamera, interaksi, label                         */
/* ====================================================================== */
const OF = { sel: null, room: "all", names: true, cam: null, hint: true };
let ENG = null;
function mount(root) {
  ENG?.destroy();
  const stage = root.querySelector(".ofx-stage"); if (!stage) return;
  const wrap = root.querySelector(".ofx"), cv = stage.querySelector(".ofx-cv"), lab = stage.querySelector(".ofx-labels"), mm = stage.querySelector(".ofx-mm");
  const styleKey = () => wrap.dataset.style || AG.UI.office || "A1";
  let W0 = 0, H0 = 0, dpr = 1, alive = true, last = performance.now(), t0 = last, lastDraw = 0, dirty = true, V = null, anim = null;
  const cam = OF.cam || (OF.cam = { z: 1, cx: (WB.x0 + WB.x1) / 2, cy: (WB.y0 + WB.y1) / 2, fit: true });
  const clampZ = (z) => Math.max(0.35, Math.min(2.6, z));
  const fitBox = (b, pad = 0.94) => { const z = clampZ(Math.min(W0 / (b.x1 - b.x0), H0 / (b.y1 - b.y0)) * pad); return { z, cx: (b.x0 + b.x1) / 2, cy: (b.y0 + b.y1) / 2 }; };
  const roomBox = (k) => { const r = room(k); const [x0, y0, x1, y1, h] = RM_BOX(r); const pts = [P(x0, y0, h), P(x1, y0, h), P(x1, y1, 0), P(x0, y1, 0), P(x1, y0, 0), P(x0, y1, h)]; return { x0: Math.min(...pts.map((p) => p[0])) - 30, x1: Math.max(...pts.map((p) => p[0])) + 30, y0: Math.min(...pts.map((p) => p[1])) - 70, y1: Math.max(...pts.map((p) => p[1])) + 50 }; };
  const resize = () => {
    const r = stage.getBoundingClientRect(); dpr = Math.min(2, window.devicePixelRatio || 1);
    W0 = Math.max(1, r.width); H0 = Math.max(1, r.height);
    cv.width = Math.round(W0 * dpr); cv.height = Math.round(H0 * dpr);
    if (cam.fit) Object.assign(cam, homeView(), { fit: true });
    dirty = true;
  };
  const ro = new ResizeObserver(resize); ro.observe(stage); resize();
  /* Tampilan awal: seluruh kantor bila muat dengan nyaman; bila tidak, zoom sedang ke area paling ramai. */
  function homeView() {
    const f = fitBox(WB), want = Math.max(f.z, styleKey() === "A2" ? Math.min(1.3, W0 / 640) : Math.min(1, W0 / 1180));
    if (want <= f.z + 0.02) return f;
    const [hx, hy] = P(12.5, 6.2, 0.9), k = Math.min(1, (want - f.z) / 0.25);
    return { z: want, cx: f.cx + (hx - f.cx) * k, cy: f.cy + (hy - f.cy) * k };
  }
  const toScreen = (wx, wy) => [W0 / 2 + (wx - cam.cx) * cam.z, H0 / 2 + (wy - cam.cy) * cam.z];
  const toWorld = (sx, sy) => [cam.cx + (sx - W0 / 2) / cam.z, cam.cy + (sy - H0 / 2) / cam.z];
  const fly = (to, ms = 480) => {
    if (reduced()) { Object.assign(cam, to, { fit: false }); dirty = true; return; }
    anim = { from: { z: cam.z, cx: cam.cx, cy: cam.cy }, to, t: performance.now(), ms }; cam.fit = false;
  };
  const zoomAt = (sx, sy, f) => { const [wx, wy] = toWorld(sx, sy); const z = clampZ(cam.z * f); cam.cx = wx - (sx - W0 / 2) / z; cam.cy = wy - (sy - H0 / 2) / z; cam.z = z; cam.fit = false; anim = null; dirty = true; hideHint(); };
  const pan = (dx, dy) => { cam.cx -= dx / cam.z; cam.cy -= dy / cam.z; cam.fit = false; anim = null; dirty = true; hideHint(); };
  const hideHint = () => { if (OF.hint) { OF.hint = false; stage.querySelector(".ofx-hint")?.classList.add("gone"); } };
  /* Hit test karakter di layar. */
  const charAt = (sx, sy) => {
    let best = null, bk = -1;
    CH.forEach((ch) => { const [X, Y] = toScreen(...P(ch.x, ch.y)); const z = cam.z; if (sx > X - 16 * z && sx < X + 16 * z && sy > Y - 66 * z && sy < Y + 6 * z) { const k = ch.x + ch.y; if (k > bk) { bk = k; best = ch; } } });
    return best;
  };
  /* ---- pointer: geser, klik, pinch ---- */
  const ptrs = new Map(); let down = null, pinch = null;
  stage.addEventListener("pointerdown", (e) => {
    if (e.target.closest(".ofx-ov, .ofx-lb, .ofx-rl")) return;
    stage.setPointerCapture(e.pointerId); ptrs.set(e.pointerId, [e.clientX, e.clientY]);
    if (ptrs.size === 2) { const [a, b] = [...ptrs.values()]; pinch = { d: Math.hypot(a[0] - b[0], a[1] - b[1]), z: cam.z }; down = null; return; }
    down = { x: e.clientX, y: e.clientY, moved: false };
  });
  stage.addEventListener("pointermove", (e) => {
    const r = stage.getBoundingClientRect();
    if (ptrs.has(e.pointerId)) {
      const prev = ptrs.get(e.pointerId); ptrs.set(e.pointerId, [e.clientX, e.clientY]);
      if (pinch && ptrs.size === 2) { const [a, b] = [...ptrs.values()]; const d = Math.hypot(a[0] - b[0], a[1] - b[1]); const f = (pinch.z * (d / pinch.d)) / cam.z; zoomAt((a[0] + b[0]) / 2 - r.left, (a[1] + b[1]) / 2 - r.top, f); return; }
      if (down) { if (!down.moved && Math.hypot(e.clientX - down.x, e.clientY - down.y) > 4) { down.moved = true; stage.classList.add("drag"); } if (down.moved) pan(e.clientX - prev[0], e.clientY - prev[1]); }
      return;
    }
    stage.classList.toggle("hot", !!charAt(e.clientX - r.left, e.clientY - r.top));
  });
  const up = (e) => {
    const r = stage.getBoundingClientRect();
    if (down && !down.moved && ptrs.has(e.pointerId)) { const ch = charAt(e.clientX - r.left, e.clientY - r.top); selectChar(ch ? ch.id : null, false); }
    ptrs.delete(e.pointerId); if (ptrs.size < 2) pinch = null; down = null; stage.classList.remove("drag");
  };
  stage.addEventListener("pointerup", up); stage.addEventListener("pointercancel", up);
  stage.addEventListener("wheel", (e) => {
    if (e.target.closest(".ofx-side")) return; e.preventDefault();
    const r = stage.getBoundingClientRect();
    if (e.ctrlKey || e.metaKey) zoomAt(e.clientX - r.left, e.clientY - r.top, Math.exp(-e.deltaY * 0.0022));
    else pan(-e.deltaX, -e.deltaY);
  }, { passive: false });
  stage.addEventListener("keydown", (e) => {
    if (e.target !== stage) return;
    const k = e.key, s = 60;
    if (k === "ArrowLeft") pan(s, 0); else if (k === "ArrowRight") pan(-s, 0); else if (k === "ArrowUp") pan(0, s); else if (k === "ArrowDown") pan(0, -s);
    else if (k === "+" || k === "=") zoomAt(W0 / 2, H0 / 2, 1.2); else if (k === "-") zoomAt(W0 / 2, H0 / 2, 1 / 1.2); else if (k === "0") api.fit(); else return;
    e.preventDefault();
  });
  /* ---- mini-map ---- */
  const mmc = mm?.getContext("2d");
  const mmGeom = () => { const w = mm.clientWidth, h = mm.clientHeight, s = Math.min((w - 12) / (WB.x1 - WB.x0), (h - 12) / (WB.y1 - WB.y0)); return { w, h, s, ox: w / 2 - ((WB.x0 + WB.x1) / 2) * s, oy: h / 2 - ((WB.y0 + WB.y1) / 2) * s }; };
  const drawMini = () => {
    if (!mmc) return; const g = mmGeom(), d = Math.min(2, window.devicePixelRatio || 1);
    if (mm.width !== Math.round(g.w * d)) { mm.width = Math.round(g.w * d); mm.height = Math.round(g.h * d); }
    const c = mmc, S = STYLES[styleKey()], dark = S.glow || isDark();
    c.setTransform(d, 0, 0, d, 0, 0); c.clearRect(0, 0, g.w, g.h);
    c.fillStyle = S.glow ? "#121735" : dark ? "#1c1c22" : "#ffffff"; c.fillRect(0, 0, g.w, g.h);
    const M = (x, y, z = 0) => { const [a, b] = P(x, y, z); return [g.ox + a * g.s, g.oy + b * g.s]; };
    const q = (x0, y0, x1, y1, col) => { c.beginPath(); [M(x0, y0), M(x1, y0), M(x1, y1), M(x0, y1)].forEach((p, i) => (i ? c.lineTo(...p) : c.moveTo(...p))); c.closePath(); c.fillStyle = col; c.fill(); c.strokeStyle = dark ? "rgba(255,255,255,.18)" : "rgba(0,0,0,.14)"; c.lineWidth = 1; c.stroke(); };
    ROOMS.forEach((r) => q(...r.r, rgba(r.c, OF.room === r.k ? 0.55 : dark ? 0.3 : 0.2)));
    q(...COR, dark ? "rgba(255,255,255,.06)" : "rgba(0,0,0,.05)");
    CH.forEach((ch) => { const [x, y] = M(ch.x, ch.y); c.beginPath(); c.arc(x, y, ch.id === OF.sel ? 3.4 : 2.4, 0, 7); c.fillStyle = OST[ch.st][2]; c.fill(); });
    const [a0, b0] = toWorld(0, 0), [a1, b1] = toWorld(W0, H0);
    c.strokeStyle = S.glow ? "#a9a9ff" : "#5b5bd6"; c.lineWidth = 1.5; c.strokeRect(g.ox + a0 * g.s, g.oy + b0 * g.s, (a1 - a0) * g.s, (b1 - b0) * g.s);
    c.fillStyle = "rgba(91,91,214,.08)"; c.fillRect(g.ox + a0 * g.s, g.oy + b0 * g.s, (a1 - a0) * g.s, (b1 - b0) * g.s);
  };
  if (mm) {
    let mmDown = false;
    const mmGo = (e) => { const r = mm.getBoundingClientRect(), g = mmGeom(); cam.cx = (e.clientX - r.left - g.ox) / g.s; cam.cy = (e.clientY - r.top - g.oy) / g.s; cam.fit = false; anim = null; dirty = true; };
    mm.addEventListener("pointerdown", (e) => { e.stopPropagation(); mmDown = true; mm.setPointerCapture(e.pointerId); mmGo(e); });
    mm.addEventListener("pointermove", (e) => { if (mmDown) mmGo(e); });
    mm.addEventListener("pointerup", () => (mmDown = false));
  }
  /* ---- label HTML ---- */
  const LB = {};
  const labelHtml = (ch) => {
    const s = OST[ch.path ? "jalan2" : ch.st];
    const chip = ch.st === "review" ? ["warn", "hand-stop", ch.tk] : ch.st === "keputusan" ? ["dng", "alert-triangle", ch.tk] : ch.st === "ok" ? ["ok", "check", "Disetujui"] : ch.st === "jeda" ? ["gray", "player-pause", "Dijeda"] : null;
    return { cls: chip ? chip[0] : "plain", html: `<span><i class="d" style="background:${s[2]}"></i><span class="nm">${esc(oshort(ch))}</span>${chip ? `<span class="k">${ic(chip[1])}${esc(chip[2])}</span>` : ""}</span>` };
  };
  const syncLabels = () => {
    CH.forEach((ch) => {
      let el = LB[ch.id];
      const { cls, html } = labelHtml(ch), sig = cls + html + (OF.sel === ch.id);
      if (!el) { el = LB[ch.id] = document.createElement("button"); el.type = "button"; el.dataset.act = "of-sel"; el.dataset.arg = ch.id; el.dataset.fly = "0"; lab.appendChild(el); }
      if (el._sig !== sig) { el._sig = sig; el.className = `ofx-lb ${cls} ${OF.sel === ch.id ? "sel" : ""}`; el.innerHTML = html; el.setAttribute("aria-label", `${oname(ch)}, ${OST[ch.st][0]}${ch.tk ? ", " + ch.tk : ""}`); el._w = 0; }
    });
    ROOMS.forEach((r) => {
      const id = "rm-" + r.k; let el = LB[id];
      const sig = roomSub(r.k) + OF.room;
      if (!el) { el = LB[id] = document.createElement("button"); el.type = "button"; el.dataset.act = "of-room"; el.dataset.arg = r.k; lab.appendChild(el); }
      if (el._sig !== sig) { el._sig = sig; el.className = `ofx-rl ${OF.room === r.k ? "on" : ""}`; el.innerHTML = `<b><i style="background:${r.c}"></i>${esc(r.n)}</b><small>${esc(roomSub(r.k))}</small>`; el.setAttribute("aria-label", `Fokus ke ${r.n}`); el._w = 0; }
    });
  };
  let hudC = null;
  const placeLabels = () => {
    const z = cam.z, cmp = z < 0.62;
    const placed = [];
    const hitIn = (x, y, w, h, gap = 3) => placed.find((p) => x < p.x + p.w + gap && x + w + gap > p.x && y < p.y + p.h + gap && y + h + gap > p.y);
    // label karakter lebih dulu: paling dekat ke depan diletakkan lebih dulu, yang bertabrakan naik
    const items = CH.map((ch) => {
      const el = LB[ch.id]; if (!el._w) { el._w = el.offsetWidth; el._h = el.offsetHeight || 22; }
      const po = poseOf(ch), headTop = (po.pose === "sit" ? 52 : 56) + (ch.who ? 0 : 8);
      const [X, Y] = toScreen(...P(ch.x, ch.y));
      return { el, ch, w: el._w, h: 22, x: X - el._w / 2, y: Y - headTop * z - 26, ax: X };
    }).sort((a, b) => b.y - a.y);
    items.forEach((it) => {
      const plain = it.el.classList.contains("plain");
      it.el.classList.toggle("cmp", cmp);
      if (cmp && plain) return;
      const w = cmp ? Math.min(it.w, 110) : it.w, x = cmp ? it.ax - w / 2 : it.x;
      let y = it.y, p, guard = 0;
      while ((p = hitIn(x, y, w, it.h)) && guard++ < 8) y = p.y - it.h - 4;
      placed.push({ x, y, w, h: it.h });
      it.el.style.transform = `translate(${Math.round(cmp ? it.ax - 24 : x)}px,${Math.round(y)}px)`;
    });
    // label ruangan: prioritas lebih rendah; bergeser atau disembunyikan bila menabrak (juga HUD & tepi panggung)
    const now = performance.now();
    if (!hudC || now - hudC.t > 400) {
      const o = lab.getBoundingClientRect();
      hudC = { t: now, r: [...stage.querySelectorAll(".ofx-ov > *, .ofx-hint:not(.gone)")].map((e) => e.getBoundingClientRect()).filter((q) => q.width > 0).map((q) => ({ x: q.left - o.left, y: q.top - o.top, w: q.width, h: q.height })) };
    }
    const hud = hudC.r, hitHud = (x, y, w, h) => hud.find((p) => x < p.x + p.w + 6 && x + w + 6 > p.x && y < p.y + p.h + 6 && y + h + 6 > p.y);
    ROOMS.forEach((r) => {
      const el = LB["rm-" + r.k]; if (!el) return;
      el.classList.toggle("cmp", z < 0.55);
      if (!el._w || el._cmp !== z < 0.55) { el._cmp = z < 0.55; el._w = el.offsetWidth; el._h = el.offsetHeight; }
      const h = el._h, w = el._w;
      let X, Y;
      if (r.top) { [X, Y] = toScreen(...P((r.r[0] + r.r[2]) / 2, 0, WH)); X -= w / 2; Y -= h + 10 * z + 8; }
      else { [X, Y] = toScreen(...P((r.r[0] + r.r[2]) / 2, D, -0.6)); X -= w / 2; Y += 10; }
      let p, guard = 0, ok = true;
      while ((p = hitIn(X, Y, w, h, 6)) && guard++ < 4) { if (r.top) Y = p.y - h - 8; else Y = p.y + p.h + 8; }
      if (hitIn(X, Y, w, h, 6) || hitHud(X, Y, w, h) || X < 4 || Y < 4 || X + w > W0 - 4 || Y + h > H0 - 4) ok = false;
      el.style.visibility = ok ? "" : "hidden";
      if (ok) placed.push({ x: X, y: Y, w, h });
      el.style.transform = `translate(${Math.round(X)}px,${Math.round(Y)}px)`;
    });
  };
  /* ---- loop ---- */
  let lastSig = "";
  const frame = (now) => {
    if (!alive) return;
    if (!cv.isConnected) { api.destroy(); return; }
    const dt = Math.min(0.1, (now - last) / 1000); last = now;
    if (anim) { const k = Math.min(1, (now - anim.t) / anim.ms), e = 1 - Math.pow(1 - k, 3); cam.z = anim.from.z + (anim.to.z - anim.from.z) * e; cam.cx = anim.from.cx + (anim.to.cx - anim.from.cx) * e; cam.cy = anim.from.cy + (anim.to.cy - anim.from.cy) * e; dirty = true; if (k >= 1) anim = null; }
    const rm = reduced();
    const walking = stepWalkers(dt); stepWander(dt);
    const S = STYLES[styleKey()], fps = S.pixel ? 10 : 30;
    const animate = !rm || walking;
    if (dirty || (animate && now - lastDraw > 1000 / fps)) {
      lastDraw = now;
      const t = (now - t0) / 1000;
      V = paint(cv, styleKey(), cam, t, { dpr, sel: OF.sel, room: OF.room, dark: isDark(), still: rm });
      syncLabels(); placeLabels(); drawMini();
      const pct = stage.querySelector("[data-pct]"); if (pct) pct.textContent = Math.round(cam.z * 100) + "%";
      dirty = false;
      const sig = CH.map((c) => c.st + c.seat + !!c.path).join();
      if (sig !== lastSig) { if (lastSig) refreshUI(); lastSig = sig; }
    }
    requestAnimationFrame(frame);
  };
  const api = {
    destroy() { alive = false; ro.disconnect(); if (ENG === api) ENG = null; },
    dirty() { dirty = true; },
    fit() { fly(fitBox(WB)); cam.fitNext = true; },
    zoom(f) { const to = { z: clampZ(cam.z * f), cx: cam.cx, cy: cam.cy }; fly(to, 220); },
    zoomTo(z) { fly({ z: clampZ(z), cx: cam.cx, cy: cam.cy }, 220); },
    room(k) { fly(k === "all" ? fitBox(WB) : fitBox(roomBox(k), 0.92)); },
    focusChar(id) { const ch = chr(id); if (!ch) return; const [x, y] = P(ch.x, ch.y, 1); const z = Math.max(cam.z, 1.05); const [sx, sy] = toScreen(x, y); if (sx > 80 && sx < W0 - 80 && sy > 90 && sy < H0 - 60 && cam.z >= 0.8) return; fly({ z: clampZ(z), cx: x, cy: y }); },
    styleKey,
  };
  ENG = api;
  requestAnimationFrame(frame);
  return api;
}

/* ====================================================================== */
/* Panel samping                                                           */
/* ====================================================================== */
const fmtLog = (l, i = 0) => `<span class="d">10.${String(31 + i * 2).padStart(2, "0")}</span> ${esc(l).replace(/^✓/, '<span class="g">✓</span>').replace(/^✗/, '<span class="r">✗</span>').replace(/(ag_\w+|rtk)/, '<span class="b">$1</span>')}`;
const charAv = (c, s = 26) => av(c.who || c.p, s);
function todoList() {
  const l = todo();
  if (!l.length) return `<div class="mut sm row g4">${ic("circle-check", "s14 text-ok")}Tidak ada yang menunggu Anda.</div>`;
  return l.map((x) => `<button class="ofx-todo" data-act="of-sel" data-arg="${x.id}" data-fly="1">${charAv(x)}<span class="grow" style="min-width:0"><span class="row g4"><span class="mono sm">${x.tk}</span>${pill(OST[x.st][0], OST[x.st][1])}</span><span class="tt">${esc(AG.ticket(x.tk).t)}</span></span>${ic("chevron-right", "s14 faint")}</button>`).join("");
}
function runnerList() {
  return AG.RUNNERS.map((r) => {
    const owner = CH.find((c) => c.who === r.who), jobs = CH.filter((c) => c.runner === r.n && c.p && c.tk);
    const st = r.st === "off" ? "off" : jobs.some((j) => j.st === "jalan") ? "busy" : "";
    const sub = r.st === "off" ? `Offline · ${r.seen}` : jobs.length ? `${jobs.length} job · ${jobs.map((j) => j.tk).slice(0, 3).join(", ")}` : "Idle, menunggu job";
    return `<button class="ofx-run ${r.st === "off" ? "off" : ""}" ${owner ? `data-act="of-sel" data-arg="${owner.id}" data-fly="1"` : `data-go="#/settings/runner"`}>${av(r.who, 26)}<span class="grow" style="min-width:0"><span class="row g6"><span class="live ${st === "off" ? "off" : st}"></span><b class="b5 sm trunc">${esc(r.n)}</b><span class="faint xs">${esc(r.client)}</span></span><span class="s">${esc(sub)}</span></span>${ic("chevron-right", "s14 faint")}</button>`;
  }).join("");
}
function overview() {
  const feed = [["dev", "Agent Developer menulis tes stok minus", "KSR-26", "1 mnt"], ["qa", "Agent QA menjalankan skenario 14/18", "KSR-29", "3 mnt"], ["qa2", "Agent QA gagal 3 kali, butuh keputusan", "KSR-30", "18 mnt"], ["an", "Brainstorming dimulai oleh Olivia", "KSR-31", "25 mnt"], ["rev", "Review AI selesai, 3 catatan", "KSR-24", "40 mnt"]];
  const n = todo().length;
  return `<div class="ofx-sp">
    ${AG.card({ cls: "ofx-cd", title: `Perlu tindakan Anda${n ? `<span class="pill warn">${n}</span>` : ""}`, desc: "Approval gate dan keputusan setelah gagal.", action: AG.seeAll("#/inbox"), body: todoList() })}
    ${AG.card({ cls: "ofx-cd", title: "Runner", desc: `${AG.RUNNERS.filter((r) => r.st !== "off").length} dari ${AG.RUNNERS.length} online`, action: AG.seeAll("#/settings/runner", "Kelola"), body: runnerList() })}
    ${AG.card({ cls: "ofx-cd", title: "Aktivitas terbaru", desc: "Dari log runner, diperbarui langsung.", body: `<div class="feed ofx-feed">${feed.map(([id, t, k, w]) => `<div class="fe">${charAv(chr(id), 20)}<span class="grow sm">${t} <a class="mono mut" href="#/t/${k}">${k}</a></span><span class="faint xs">${w}</span></div>`).join("")}</div>` })}
  </div>`;
}
function detail(c) {
  const t = c.tk ? AG.ticket(c.tk) : null, s = OST[c.path ? "jalan2" : c.st], human = !!c.who;
  const lk = logKey(c), lines = LOGS[lk] || [], live = c.st === "jalan" || c.st === "pemilik" || c.st === "rapat";
  let acts = "";
  if (c.st === "review") acts = `${btn("Approve", "pri", "check", { "data-act": "of-approve", "data-arg": c.id })}${btn("Reject", "", "x", { "data-dialog": "of-reject", "data-arg": c.id })}`;
  else if (c.st === "keputusan") acts = `${btn("Coba lagi dengan catatan", "pri", "refresh", { "data-dialog": "of-retry", "data-arg": c.id })}${btn("Reject", "", "x", { "data-dialog": "of-reject", "data-arg": c.id })}`;
  else if (c.st === "jalan") acts = `${btn("Jeda", "", "player-pause", { "data-act": "of-pause", "data-arg": c.id })}${btn("Buka terminal", "", "terminal-2", { "data-toast": `Terminal ${c.runner} dibuka (hanya baca)` })}`;
  else if (c.st === "jeda") acts = btn("Lanjutkan", "pri", "player-play", { "data-act": "of-pause", "data-arg": c.id });
  else if (c.st === "idle" && !c.path) acts = btn("Tugaskan tiket", "pri", "plus", { "data-menu": "of-assign", "data-arg": c.id });
  else if (c.st === "rapat") acts = btn("Gabung sesi", "pri", "message-circle", { "data-toast": "Lanjutkan dengan /ag-brainstorming KSR-31 di client Anda" });
  else if (c.st === "pemilik") acts = btn("Lihat runner", "", "device-laptop", { "data-go": "#/settings/runner" });
  const body = `
    <div class="ofx-dh"><canvas class="ofx-pv" data-pv="${c.id}" width="104" height="116" aria-hidden="true"></canvas><div class="grow" style="min-width:0">${pill(s[0], s[1])}<div class="mut sm" style="margin-top:6px">${c.path ? "Sedang berjalan" : room(SEATS[c.seat].room).n}${c.runner ? ` · ${esc(c.runner)}` : ""}</div></div></div>
    ${c.st === "keputusan" ? `<div class="alert dng">${ic("alert-triangle", "s14")}<span><b>Butuh keputusan.</b> ${esc(c.doing)}. Gate “Hasil test lulus” belum terpenuhi.</span></div>` : c.st === "review" ? `<div class="alert warn">${ic("hand-stop", "s14")}<span>Menunggu approval Anda di stage <b>${AG.stage(t.stage).n}</b>.</span></div>` : c.st === "ok" ? `<div class="alert ok">${ic("circle-check", "s14")}<span>Disetujui. Agent kembali ke pantry dan tiket lanjut ke stage berikutnya.</span></div>` : ""}
    <dl class="props ofx-props">
      ${t ? `<dt>Tiket</dt><dd class="row" style="min-width:0">${tref(t.k)}<span class="trunc sm">${esc(t.t)}</span></dd><dt>Stage</dt><dd class="row g4">${AG.stageIcon(t.stage)}${AG.stage(t.stage).n}</dd>` : human ? `<dt>Job</dt><dd class="ofx-jobs">${c.jobs.map(tref).join("")}</dd>` : `<dt>Tiket</dt><dd class="mut">Tidak ada job</dd>`}
      ${c.runner ? `<dt>Runner</dt><dd class="row g4"><span class="live ${c.st === "jalan" ? "busy" : ""}"></span>${c.runner}</dd>` : ""}
      ${c.since ? `<dt>Mulai</dt><dd>${c.since}</dd>` : ""}
      ${c.steps ? `<dt>Progres</dt><dd class="row"><span class="bar grow"><i style="width:${Math.round((c.step / c.steps) * 100)}%"></i></span><span class="mut sm tab-num">${c.step}/${c.steps}</span></dd>` : ""}</dl>
    ${c.st !== "idle" && c.st !== "ok" ? `<div><div class="ofx-h">${ic("terminal-2", "s14")}Log kerja ${live ? pill("live", "acc") : ""}</div><div class="term ofx-log" id="ofx-log" data-k="${lk}" data-live="${live ? 1 : 0}">${lines.slice(0, live ? 4 : lines.length).map((l, i) => `<div>${fmtLog(l, i)}</div>`).join("")}</div></div>` : `<div class="empty" style="padding:18px">${ic("coffee", "s18")}<span>${c.path ? "Sedang berjalan ke pantry." : "Agent menganggur di pantry. Tugaskan tiket supaya mulai bekerja."}</span></div>`}
    ${c.p ? `<div class="row g12 wrap"><button class="link sm row g4" data-dialog="of-custom" data-arg="${c.id}">${ic("palette", "s14")}Kustom karakter</button><a class="link sm row g4" href="#/settings/personas">${ic("robot", "s14")}Persona ${esc(AG.PERSONAS[c.p].n)}</a></div>` : ""}`;
  return `<div class="ofx-sp">${AG.card({ cls: "ofx-cd ofx-dcard", title: esc(oname(c)), desc: human ? AG.PEOPLE[c.who].job + " · pemilik runner" : AG.PERSONAS[c.p].t,
    action: btn("", "ghost icon sm", "x", { "data-act": "of-sel", "data-arg": "", "aria-label": "Tutup detail, kembali ke semua di kantor" }), body,
    foot: `<div class="ofx-acts">${acts}${t ? btn("Buka tiket", "ghost", "external-link", { "data-go": `#/t/${t.k}` }) : ""}</div>` })}</div>`;
}
function panelHtml() { const c = OF.sel ? chr(OF.sel) : null; return c ? detail(c) : overview(); }
/* Pratinjau karakter kecil (di panel dan dialog kustom). */
function paintPreview(cv, ch, styleKey, zoom = 1.25) {
  if (!cv || !ch) return;
  const S = STYLES[styleKey], ctx = cv.getContext("2d"), w = cv.width, h = cv.height;
  const po = { ...poseOf({ ...ch, path: null, seat: "p4" }), pose: "stand" };
  if (po.arms === "type") po.arms = "idle";
  const draw = (c, sc, ox, oy) => {
    G = { c, S, sc, ox, oy, t: 0.8, still: true };
    c.setTransform(1, 0, 0, 1, 0, 0); c.fillStyle = S.glow ? "#161c3a" : S.pixel ? "#dfe6ef" : "#f1f3f9"; c.fillRect(0, 0, c.canvas.width, c.canvas.height);
    c.setTransform(sc, 0, 0, sc, ox, oy);
    fillP([P(-0.9, -0.9), P(0.9, -0.9), P(0.9, 0.9), P(-0.9, 0.9)], S.pixel ? S.pal.fKSR : S.pal.fKSR);
    (S.pixel ? drawCharP : drawCharV)({ ...ch, path: null, st: ch.st === "jalan" ? "pemilik" : ch.st }, 0, 0, "f", false, po, 0.8, false);
  };
  if (S.pixel) {
    const PX = Math.max(2, Math.round(2 * zoom)), bw = Math.ceil(w / PX), bh = Math.ceil(h / PX), sc = zoom / PX;
    const b = document.createElement("canvas"); b.width = bw; b.height = bh; const bc = b.getContext("2d", { willReadFrequently: true });
    draw(bc, sc, Math.round(bw / 2), Math.round(bh * 0.84)); quantize(bc, bw, bh);
    ctx.imageSmoothingEnabled = false; ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.drawImage(b, 0, 0, bw * PX, bh * PX);
  } else draw(ctx, zoom, w / 2, h * 0.84);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}
let logTimer = null;
function startLog() {
  clearInterval(logTimer);
  logTimer = setInterval(() => {
    const el = document.getElementById("ofx-log"); if (!el) { if (!document.querySelector(".ofx")) clearInterval(logTimer); return; }
    if (el.dataset.live !== "1") return;
    const pool = LOGS[el.dataset.k] || [], n = el.children.length; if (n >= pool.length) return;
    const d = document.createElement("div"); d.className = "new"; d.innerHTML = fmtLog(pool[n], n); el.appendChild(d); el.scrollTop = el.scrollHeight;
  }, 2200);
}
function summaryHtml() {
  const it = [["#5b5bd6", count("jalan"), "bekerja", ""], ["#e3962b", count("review"), "butuh approval", ""], ["#e5484d", count("keputusan"), "butuh keputusan", ""], ["#a1a7b5", count("idle"), "di pantry", "hide-sm"]];
  return it.map(([c, n, l, cls]) => `<span class="${cls}"><i style="background:${c}"></i><b>${n}</b>${l}</span>`).join("");
}
function tabsHtml() {
  const n = (k) => CH.filter((c) => !c.path && SEATS[c.seat].room === k).length;
  return `<nav class="tabs ofx-tabs" aria-label="Filter ruangan">${[["all", "Semua ruangan", CH.length], ...TABS.map((k) => [k, room(k).n, n(k)])].map(([k, l, c]) => `<button class="${OF.room === k ? "on" : ""}" data-act="of-room" data-arg="${k}" aria-pressed="${OF.room === k}">${k !== "all" ? `<span class="dot" style="background:${room(k).c}"></span>` : ""}${l}<span class="n">${c}</span></button>`).join("")}</nav>`;
}
/* Perbarui bagian UI tanpa merender ulang kanvas. */
function refreshUI(keepScroll = true) {
  const side = document.getElementById("ofx-side");
  if (side) { const y = side.scrollTop; side.innerHTML = panelHtml(); if (keepScroll) side.scrollTop = y; startLog(); side.querySelectorAll("[data-pv]").forEach((cv) => paintPreview(cv, chr(cv.dataset.pv), ENG?.styleKey() || AG.UI.office, 1.05)); }
  const sum = document.getElementById("ofx-sum"); if (sum) sum.innerHTML = summaryHtml();
  const tabs = document.querySelector(".ofx-tabs"); if (tabs) tabs.outerHTML = tabsHtml();
  ENG?.dirty();
}
function selectChar(id, fly) {
  OF.sel = id || null;
  refreshUI(false);
  const side = document.getElementById("ofx-side"); if (side) side.scrollTop = 0;
  if (OF.sel && fly) ENG?.focusChar(OF.sel);
  if (OF.sel && innerWidth < 1200 && side) side.scrollIntoView({ behavior: reduced() ? "auto" : "smooth", block: "start" });
}

/* ====================================================================== */
/* Aksi, menu, dialog                                                      */
/* ====================================================================== */
AG.action("of-sel", (el, id) => selectChar(id, el?.dataset.fly === "1"));
AG.action("of-room", (el, k) => { OF.room = OF.room === k && k !== "all" ? "all" : k; refreshUI(); ENG?.room(OF.room); });
AG.action("of-zoom", (el, d) => ENG?.zoom(d === "in" ? 1.25 : 0.8));
AG.action("of-100", () => ENG?.zoomTo(1));
AG.action("of-fit", () => { OF.room = "all"; refreshUI(); ENG?.fit(); });
AG.action("of-names", (el) => { OF.names = !OF.names; el.classList.toggle("on", OF.names); el.setAttribute("aria-pressed", OF.names); document.querySelector(".ofx")?.classList.toggle("nonames", !OF.names); });
AG.action("of-style", (el, k) => { AG.setUI("office", k); QC.clear(); AG.rerender(); AG.toast(`Gaya AI Office: ${k} ${STYLES[k].n}`); });
AG.action("of-approve", (el, id) => {
  const c = chr(id); const st = AG.stage(AG.ticket(c.tk).stage);
  c.st = "ok"; c.doing = "Disetujui · lanjut ke stage berikutnya";
  refreshUI(); AG.toast(`${c.tk} disetujui di stage ${st.n} · lanjut ke stage berikutnya`);
  setTimeout(() => {
    if (c.st !== "ok") return;
    const free = PANTRY_FREE.find((s) => !CH.some((o) => o.seat === s || o.seatTo === s));
    if (!free) return;
    c.st = "idle"; c.doneTk = c.tk;
    startWalk(c, free, (cc) => { cc.tk = null; cc.doing = "Menunggu tiket"; cc.since = null; cc.steps = null; refreshUI(); });
    refreshUI();
  }, reduced() ? 900 : 2600);
});
AG.action("of-pause", (el, id) => { const c = chr(id); c.st = c.st === "jeda" ? "jalan" : "jeda"; refreshUI(); AG.toast(c.st === "jeda" ? `Job ${c.tk} dijeda di ${c.runner}` : `Job ${c.tk} dilanjutkan`); });
AG.action("of-redo", (el, id) => { const c = chr(id); c.st = "jalan"; c.doing = "Mengulang dengan catatan Anda"; c.step = 1; c.logk = "gen"; AG.closeDialog(); refreshUI(); AG.toast(`${c.tk} dikembalikan ke ${oname(c)} dengan catatan`); });
AG.dialog("of-reject", (id) => { const c = chr(id); return { icon: "x", tone: "dng", title: `Reject ${c.tk}`, desc: `${esc(AG.ticket(c.tk).t)}. ${esc(oname(c))} mengulang pekerjaan dengan catatan Anda.`,
  body: AG.fld("Catatan untuk agent", AG.inp("", { area: true, ph: "Mis. tombol QRIS harus tetap terlihat saat keranjang panjang" }), "Catatan masuk ke log tiket dan dibaca agent sebelum mengulang."),
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Reject dan kirim catatan", "dngs", "x", { "data-act": "of-redo", "data-arg": id })}` }; });
AG.dialog("of-retry", (id) => { const c = chr(id); return { icon: "refresh", tone: "acc", title: `Coba lagi ${c.tk}`, desc: `${esc(oname(c))} menjalankan ulang pekerjaan dengan catatan tambahan.`,
  body: AG.fld("Catatan", AG.inp("Nama kasir diambil dari sesi login PIN, bukan dari profil toko. Cek juga pembulatan Rp100.", { area: true })),
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Jalankan ulang", "pri", "player-play", { "data-act": "of-redo", "data-arg": id })}` }; });
AG.menu("of-assign", (id) => { const c = chr(id); return `<div class="mlab">Tugaskan ke ${esc(oname(c))}</div>${AG.TICKETS.filter((t) => t.stage === "backlog" || t.state === "antre").slice(0, 5).map((t) => `<button class="mi" data-act="of-assign-ok" data-arg="${id}:${t.k}">${AG.typeIcon(t.type)}<span class="mono mut">${t.k}</span><span class="trunc">${esc(t.t)}</span></button>`).join("")}`; });
AG.action("of-assign-ok", (el, arg) => {
  const [id, k] = arg.split(":"); const c = chr(id);
  const free = KSR_FREE.find((s) => !CH.some((o) => o.seat === s || o.seatTo === s));
  if (!free) { AG.toast("Semua meja di Kasir Online terisi", "dng"); return; }
  Object.assign(c, { tk: k, doing: "Berjalan ke meja di Kasir Online", step: 1, steps: 6, since: "baru saja", runner: "MacBook Olivia", logk: "gen" });
  c.st = "jalan";
  startWalk(c, free, (cc) => { cc.doing = "Membaca tiket dan acceptance criteria"; refreshUI(); });
  refreshUI(); AG.toast(`${k} ditugaskan ke ${oname(c)} · berjalan ke Kasir Online`);
});
AG.action("of-reset", () => { resetChars(); OF.sel = null; refreshUI(); AG.toast("Data contoh dikembalikan"); });
AG.menu("of-style", () => `<div class="mlab">Gaya seni kantor</div>${Object.values(STYLES).map((s) => `<button class="mi" data-act="of-style" data-arg="${s.k}"><span class="ofg-l" style="width:24px;height:20px">${s.k}</span><span class="grow"><span>${s.n}</span><br><span class="faint xs">${s.sub}</span></span>${AG.UI.office === s.k ? `<span class="tail">${ic("check", "s14")}</span>` : ""}</button>`).join("")}<div class="msep"></div><a class="mi" href="#/opsi/office-gaya">${ic("layout-grid")}Bandingkan ketiga gaya</a>`);

/* Dialog kustom karakter dengan pratinjau langsung. */
const ACCS = [["none", "Tidak ada"], ["headset", "Headset"], ["glasses", "Kacamata"], ["cap", "Topi"], ["beanie", "Kupluk"], ["beret", "Baret"], ["scarf", "Syal"]];
let DRAFT = null;
AG.dialog("of-custom", (id) => {
  const c = chr(id) || chr(OF.sel) || CH.find((x) => x.p);
  DRAFT = { id: c.id, shirt: c.look.shirt, acc: c.look.acc || "none", alias: c.alias || "" };
  const opts = CH.map((x) => ({ v: x.id, l: oname(x), sub: x.who ? "Manusia · pemilik runner" : AG.PERSONAS[x.p].t, lead: av(x.who || x.p, 18), group: x.who ? "Manusia" : "Agent" }));
  return { w: "w640", icon: "palette", tone: "acc", title: "Kustom karakter", desc: "Tampilan berlaku untuk semua orang di workspace. Gaya seni kantor diatur terpisah.",
    body: `<div class="grid2" style="grid-template-columns:200px minmax(0,1fr);gap:20px;align-items:start">
      <div class="col g8"><canvas id="ofx-cpv" width="400" height="440" style="width:200px;height:220px;border-radius:12px;border:1px solid var(--line);display:block"></canvas><span class="faint xs" style="text-align:center">Pratinjau gaya ${STYLES[AG.UI.office].n}</span></div>
      <div class="col g16">
        ${AG.fld("Karakter", AG.combo({ options: opts, value: c.id }))}
        ${AG.fld("Nama tampilan", AG.inp(DRAFT.alias, { ph: oshort({ ...c, alias: "" }), a: { id: "ofx-alias" } }), "Muncul di label atas kepala.")}
        ${AG.fld("Warna baju", AG.colorPicker(c.look.shirt))}
        ${AG.fld("Aksesori", `<div class="row wrap g6" role="radiogroup" aria-label="Aksesori">${ACCS.map(([k, l]) => `<button type="button" class="chip ${DRAFT.acc === k ? "on" : ""}" data-radio="ofacc" data-acc="${k}" role="radio" aria-checked="${DRAFT.acc === k}">${l}</button>`).join("")}</div>`)}
      </div></div>`,
    foot: `${c.p ? `<a class="link sm" href="#/settings/personas" style="margin-right:auto">Atur persona ${esc(AG.PERSONAS[c.p].n)}</a>` : ""}${btn("Batal", "", "", { "data-close": true })}${btn("Simpan karakter", "pri", "check", { "data-act": "of-custom-ok" })}`,
    after(el) {
      const cv = el.querySelector("#ofx-cpv");
      const redraw = () => { const ch = chr(DRAFT.id); paintPreview(cv, { ...ch, look: { ...ch.look, shirt: DRAFT.shirt, acc: DRAFT.acc }, alias: DRAFT.alias }, AG.UI.office, 3.2); };
      redraw();
      el.addEventListener("combo-change", (e) => { AG.closeDialog(); AG.openDialog("of-custom", e.detail); });
      el.addEventListener("color-change", (e) => { DRAFT.shirt = e.detail; redraw(); });
      el.addEventListener("click", (e) => { const b = e.target.closest("[data-acc]"); if (b) { DRAFT.acc = b.dataset.acc; el.querySelectorAll("[data-acc]").forEach((x) => { x.classList.toggle("on", x === b); x.setAttribute("aria-checked", x === b); }); redraw(); } });
      el.querySelector("#ofx-alias")?.addEventListener("input", (e) => (DRAFT.alias = e.target.value.trim()));
    } };
});
AG.action("of-custom-ok", () => {
  const c = chr(DRAFT.id); c.look = { ...c.look, shirt: DRAFT.shirt, acc: DRAFT.acc }; c.alias = DRAFT.alias || null;
  Object.keys(SPR).forEach((k) => k.startsWith(c.id + "|") && delete SPR[k]);
  AG.closeDialog(); refreshUI(); document.querySelectorAll(".ofx-lb").forEach((l) => (l._sig = "")); AG.toast(`Karakter ${oname(c)} diperbarui`);
});

/* ====================================================================== */
/* Tata letak layar                                                        */
/* ====================================================================== */
function officeLayout({ embed = false } = {}) {
  const k = AG.UI.office, S = STYLES[k];
  return `<div class="ofx ${embed ? "embed" : "page"} ${OF.names ? "" : "nonames"}" data-style="${k}">
    ${embed ? "" : `<div class="ofx-top">${AG.head("AI Office", "Kantor PT Contoh, live dari runner. Seret untuk menjelajah, klik karakter untuk melihat log dan memberi approval.", `${btn("Kustom karakter", "", "palette", { "data-dialog": "of-custom", "data-arg": OF.sel || "" })}${btn("Persona agent", "ghost hide-sm", "robot", { "data-go": "#/settings/personas" })}`)}${tabsHtml()}</div>`}
    ${embed ? `<div style="padding:0 16px;border-bottom:1px solid var(--line)">${tabsHtml()}</div>` : ""}
    <div class="ofx-body">
      <div class="ofx-stage" tabindex="0" role="application" aria-roledescription="kantor isometrik" aria-label="Kantor isometrik. Seret atau panah untuk menggeser, plus dan minus untuk zoom, 0 untuk pas layar. Karakter bisa dipilih lewat label namanya.">
        <canvas class="ofx-cv" aria-hidden="true"></canvas>
        <div class="ofx-labels"></div>
        <div class="ofx-ov tl"><div class="ofx-card ofx-sum" id="ofx-sum" aria-live="polite">${summaryHtml()}</div></div>
        <div class="ofx-ov tr">
          <button class="ofx-card ofx-tbtn ${OF.names ? "on" : ""}" data-act="of-names" aria-pressed="${OF.names}" title="Tampilkan label nama">${ic("tag", "s14")}<span class="hide-sm">Label</span></button>
          <button class="ofx-card ofx-tbtn" data-menu="of-style" aria-label="Gaya seni: ${S.n}">${ic("palette", "s14")}<span class="hide-sm">${k} · ${S.n}</span><span class="show-sm-only" style="display:none">${k}</span>${ic("chevron-down", "s14 faint")}</button>
        </div>
        <div class="ofx-ov bl"><canvas class="ofx-mm ofx-card" aria-label="Mini-map: klik untuk berpindah"></canvas></div>
        <div class="ofx-ov br"><div class="ofx-card ofx-zoom">
          <button data-act="of-zoom" data-arg="out" aria-label="Perkecil" title="Perkecil">${ic("minus", "s14")}</button>
          <button class="pct" data-act="of-100" data-pct title="Kembali ke 100%">100%</button>
          <button data-act="of-zoom" data-arg="in" aria-label="Perbesar" title="Perbesar">${ic("plus", "s14")}</button>
          <button data-act="of-fit" title="Pas layar">${ic("arrows-maximize", "s14")}<span class="hide-sm">Pas layar</span></button>
        </div></div>
        <div class="ofx-hint ofx-card ${OF.hint ? "" : "gone"}">${ic("hand-stop", "s14")}Seret untuk menggeser<span class="faint">·</span><kbd>Ctrl</kbd> + scroll untuk zoom</div>
      </div>
      <aside class="ofx-side" id="ofx-side" aria-label="Detail kantor" aria-live="polite">${panelHtml()}</aside>
    </div></div>`;
}
function afterOffice(root) {
  buildScene();
  mount(root);
  startLog();
  root.querySelectorAll("[data-pv]").forEach((cv) => paintPreview(cv, chr(cv.dataset.pv), AG.UI.office, 1.05));
}

AG.screen({ m: "M10", g: "AI Office", n: "AI Office", route: "/office",
  render: () => AG.shell({ crumbs: ["AI Office"], flush: true, body: officeLayout(), actions: `${btn("Bandingkan gaya", "ghost sm hide-sm", "palette", { "data-go": "#/opsi/office-gaya" })}` }),
  after: afterOffice });

/* ====================================================================== */
/* Layar opsi: gaya AI Office                                              */
/* ====================================================================== */
function thumb(cv, k) {
  const r = cv.getBoundingClientRect(), d = Math.min(2, window.devicePixelRatio || 1);
  cv.width = Math.round(r.width * d); cv.height = Math.round(r.height * d);
  const z = (r.width / (k === "A2" ? 460 : 520)), [cx, cy] = P(8.4, 5.2, 0.7);
  paint(cv, k, { z, cx, cy }, 1.3, { dpr: d, dark: isDark(), still: true });
}
AG.screen({ m: "Opsi", g: "Opsi desain", n: "Gaya AI Office", route: "/opsi/office-gaya",
  render: () => {
    const cur = AG.UI.office;
    const cards = Object.values(STYLES).map((s) => `<div class="ofg-card ${cur === s.k ? "on" : ""}" role="button" tabindex="0" data-act="of-style" data-arg="${s.k}" aria-pressed="${cur === s.k}">
        <canvas class="ofg-thumb" data-thumb="${s.k}" aria-hidden="true"></canvas>
        <div class="ofg-h"><span class="ofg-l">${s.k}</span><b class="md b6 grow">${s.n}</b>${s.k === "A1" ? `<span title="Dipilih pemilik produk">${pill("Bawaan", "acc")}</span>` : ""}${cur === s.k ? pill("Dipakai", "ok", "", "check") : ""}</div>
        <div class="col g8"><p>${s.d}</p>
          <div class="ofg-pc">${s.plus.map((x) => `<span class="row g8">${ic("check", "s14 text-ok")}<span>${x}</span></span>`).join("")}${s.minus.map((x) => `<span class="row g8">${ic("minus", "s14 mut")}<span class="mut">${x}</span></span>`).join("")}</div></div>
        <div class="ofg-foot">${btn(cur === s.k ? "Gaya ini dipakai" : "Pakai gaya ini", cur === s.k ? "" : "pri", "check", { "data-act": "of-style", "data-arg": s.k })}</div>
      </div>`).join("");
    const body = `${AG.head("Gaya AI Office", "Pemilik produk memilih <b>A1 Vektor lembut</b> sebagai gaya bawaan. Ketiga gaya memakai denah, data runner, dan interaksi yang sama; yang berbeda hanya palet dan aset.", `${btn("Buka AI Office", "pri", "external-link", { "data-go": "#/office" })}`)}
      <div class="col g20" style="margin-top:20px">
      ${cur !== "A1" ? `<div class="alert warn">${ic("alert-triangle", "s14")}<span class="grow">Perangkat ini sedang memakai ${cur} · ${STYLES[cur].n}. Gaya bawaan untuk semua orang tetap A1 Vektor lembut.</span>${btn("Kembali ke A1", "sm", "", { "data-act": "of-style", "data-arg": "A1" })}</div>` : ""}
      <div class="ofg-grid">${cards}</div>
      ${AG.card({ cls: "ofg-live", title: `<span class="row g8">Pratinjau langsung${pill(`${cur} · ${STYLES[cur].n}`, "acc")}</span>`, desc: "Seret, zoom, dan klik karakter seperti di AI Office. Animasi berhenti bila sistem meminta gerak dikurangi. Pilihan tersimpan di perangkat ini.",
        action: btn("Kembalikan data contoh", "sm ghost", "refresh", { "data-act": "of-reset" }), pad: false, body: officeLayout({ embed: true }) })}
      </div>`;
    return AG.shell({ crumbs: [`<a href="#/opsi/office">Opsi desain</a>`, "Gaya AI Office"], body });
  },
  after: (root) => {
    afterOffice(root);
    root.querySelectorAll("[data-thumb]").forEach((cv) => thumb(cv, cv.dataset.thumb));
    root.querySelectorAll(".ofg-card").forEach((el) => el.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); AG.ACTIONS["of-style"](el, el.dataset.arg); } }));
  } });

AG.OFFICE = { STYLES, paint, CH: () => CH, OF, ENG: () => ENG, P };
})();
