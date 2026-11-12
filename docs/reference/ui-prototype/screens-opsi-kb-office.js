/* Opsi desain: Knowledge (katalog + usulan) dan AI Office. Tiga pratinjau interaktif per area.
   Prefix kelas: ok- (bersama/knowledge), oka/okb/okc (knowledge A/B/C), oo/ooa/oob/ooc (office). */
(() => {
const { ic, av, btn, pill, esc, tref } = AG;
const K = "KSR";

AG.css(`
/* ---------- bersama ---------- */
.ok-info { display: flex; gap: 20px; padding: 16px 18px; align-items: flex-start; }
.ok-info p { margin-top: 4px; max-width: 86ch; }
.ok-let { width: 22px; height: 22px; border-radius: 6px; background: var(--fg); color: var(--bg); display: inline-flex; align-items: center; justify-content: center; font-size: 12px; font-weight: 600; flex-shrink: 0; }
.tabs .ok-let { width: 18px; height: 18px; font-size: 11px; border-radius: 5px; background: var(--hover); color: var(--muted); }
.tabs .on .ok-let { background: var(--fg); color: var(--bg); }
.ok-pc { display: grid; grid-template-columns: 1fr 1fr; gap: 6px 24px; margin-top: 10px; }
.ok-pc > div { display: grid; gap: 4px; align-content: start; }
.ok-pc .row { align-items: flex-start; } .ok-pc .ic { margin-top: 2px; }
.ok-info-act { display: grid; gap: 8px; justify-items: end; flex-shrink: 0; max-width: 230px; text-align: right; }
@media (max-width: 899px) { .ok-info { flex-direction: column; } .ok-pc { grid-template-columns: 1fr; } .ok-info-act { justify-items: start; text-align: left; max-width: none; } }
.ok-frame { border: 1px solid var(--line2); border-radius: 12px; background: var(--raised); box-shadow: var(--shadow-md); overflow: hidden; min-width: 0; }
.ok-chrome { height: 34px; display: flex; align-items: center; gap: 12px; padding: 0 12px; background: var(--muted-bg); border-bottom: 1px solid var(--line); }
.ok-dots { display: flex; gap: 6px; } .ok-dots i { width: 10px; height: 10px; border-radius: 50%; background: var(--line2); }
.ok-url { flex: 1; max-width: 420px; margin: 0 auto; height: 22px; border-radius: 6px; background: var(--raised); border: 1px solid var(--line); color: var(--muted); font-size: 11.5px; display: flex; align-items: center; justify-content: center; gap: 6px; font-family: var(--mono); overflow: hidden; white-space: nowrap; }
.ok-scr { height: 780px; position: relative; display: flex; flex-direction: column; background: var(--page); container-type: inline-size; overflow: hidden; }
.ok-top { height: 42px; flex-shrink: 0; display: flex; align-items: center; gap: 8px; padding: 0 16px; background: var(--bg); border-bottom: 1px solid var(--line); }
.ok-crumbs { display: flex; align-items: center; gap: 6px; color: var(--muted); min-width: 0; white-space: nowrap; overflow: hidden; }
.ok-crumbs b { color: var(--fg); font-weight: 500; overflow: hidden; text-overflow: ellipsis; }
.ok-back { display: inline-flex; align-items: center; gap: 6px; color: var(--muted); font-weight: 500; } .ok-back:hover { color: var(--fg); }
@media (max-width: 899px) { .ok-scr { height: auto; overflow: visible; } }

/* ---------- knowledge: isi dokumen ---------- */
.ok-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; display: inline-block; }
.ok-pm { font-family: var(--mono); font-size: 12px; white-space: nowrap; } .ok-pm b { font-weight: 500; }
.ok-h1 { font-size: 26px; font-weight: 600; letter-spacing: -0.02em; line-height: 1.2; }
.ok-prose { display: grid; gap: 12px; font-size: 14px; line-height: 1.7; color: var(--fg2); min-width: 0; }
.ok-prose h2 { font-size: 17px; color: var(--fg); margin-top: 10px; letter-spacing: -0.01em; scroll-margin-top: 60px; }
.ok-prose ul { margin: 0; padding-left: 20px; display: grid; gap: 4px; }
.ok-prose.dim { opacity: 0.5; }
.ok-prose.sm { font-size: 13px; gap: 10px; line-height: 1.6; } .ok-prose.sm h2 { font-size: 14.5px; margin-top: 6px; }
.ok-tw { overflow-x: auto; border: 1px solid var(--line); border-radius: 8px; }
.ok-tbl { width: 100%; border-collapse: collapse; font-size: 13px; }
.ok-tbl th, .ok-tbl td { padding: 7px 12px; border-bottom: 1px solid var(--line); text-align: left; vertical-align: top; }
.ok-tbl th { background: var(--muted-bg); font-weight: 500; color: var(--muted); font-size: 12px; } .ok-tbl tr:last-child td { border-bottom: 0; }
.ok-code { font-family: var(--mono); font-size: 12px; background: var(--code-bg); border: 1px solid var(--line); border-radius: 8px; padding: 10px 12px; margin: 0; white-space: pre-wrap; line-height: 1.6; color: var(--fg2); }
.ok-dl { color: var(--accent-fg); border-bottom: 1px dashed var(--accent); cursor: pointer; }
.ok-fig { border: 1px solid var(--line); border-radius: 10px; background: var(--muted-bg); padding: 12px; }
.ok-fig svg { width: 100%; height: auto; display: block; max-height: 150px; }
.ok-fig rect { fill: var(--raised); stroke: var(--line2); } .ok-fig rect.a { fill: var(--accent-soft); stroke: var(--accent); }
.ok-fig text { fill: var(--fg); font-size: 12px; font-weight: 500; } .ok-fig text.s { fill: var(--muted); font-size: 10px; font-weight: 400; }
.ok-fig path { stroke: var(--faint); fill: none; stroke-width: 1.3; } .ok-fig path.d { stroke-dasharray: 4 3; }
/* saran inline */
.ok-sug { border: 1px solid var(--line); border-radius: 10px; overflow: hidden; background: var(--raised); font-size: 13.5px; }
.ok-sug-hd { display: flex; gap: 6px; align-items: center; padding: 6px 12px; background: var(--muted-bg); border-bottom: 1px solid var(--line); color: var(--muted); font-size: 12px; }
.ok-sug-l { display: grid; grid-template-columns: 22px minmax(0, 1fr); padding: 3px 12px 3px 4px; line-height: 1.6; }
.ok-sug-l.add { background: var(--success-soft); } .ok-sug-l.add .ok-sug-t { color: var(--fg); }
.ok-sug-l.del { background: var(--danger-soft); } .ok-sug-l.del .ok-sug-t { color: var(--danger); text-decoration: line-through; text-decoration-color: color-mix(in srgb, var(--danger) 45%, transparent); }
.ok-sig { font-family: var(--mono); color: var(--faint); text-align: center; } .add > .ok-sig { color: var(--success); } .del > .ok-sig { color: var(--danger); }
.ok-trow { display: grid; grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.4fr) minmax(0, 2.4fr); gap: 10px; font-size: 12.5px; }
.ok-trow > span { overflow-wrap: anywhere; } .ok-trow > span:first-child { font-family: var(--mono); font-size: 12px; }
.ok-dh { font-weight: 600; color: var(--fg); }

/* ---------- knowledge A: tiga panel ---------- */
.oka { flex: 1; min-height: 0; display: grid; grid-template-columns: 248px minmax(0, 1fr) 264px; }
.oka-tree { border-right: 1px solid var(--line); background: var(--bg-sub); overflow-y: auto; padding: 10px 8px; display: flex; flex-direction: column; gap: 1px; min-width: 0; }
.oka-q { display: flex; align-items: center; gap: 8px; height: 30px; padding: 0 8px; border: 1px solid var(--line2); border-radius: 7px; background: var(--raised); color: var(--faint); margin: 0 2px 8px; flex-shrink: 0; }
.oka-q:focus-within { border-color: var(--accent); box-shadow: var(--ring); }
.oka-q input { border: 0; outline: 0; background: none; flex: 1; min-width: 0; font-size: 13px; color: var(--fg); }
.oka-ni, .oka-d, .oka-gh { display: flex; align-items: center; gap: 8px; width: 100%; height: 30px; padding: 0 8px; border-radius: 6px; color: var(--fg2); text-align: left; min-width: 0; flex-shrink: 0; }
.oka-ni:hover, .oka-d:hover, .oka-gh:hover { background: var(--hover); }
.oka-ni.on, .oka-d.on { background: var(--raised); color: var(--fg); font-weight: 500; box-shadow: var(--shadow-sm); outline: 1px solid var(--line); }
.oka-ni .ic, .oka-gh .ic { color: var(--faint); }
.oka-c { font-size: 11px; font-weight: 600; border-radius: 9px; padding: 0 6px; line-height: 17px; }
.oka-c.acc { background: var(--accent); color: var(--on-accent); } .oka-c.warn { background: var(--warn-soft); color: var(--warn); }
.oka-sec { display: flex; justify-content: space-between; font-size: 12px; color: var(--muted); font-weight: 500; padding: 14px 8px 4px; flex-shrink: 0; }
.oka-gh { font-weight: 500; color: var(--fg); height: 28px; }
.oka-d { padding-left: 30px; height: 28px; }
.oka-b { font-size: 11px; color: var(--accent-fg); background: var(--accent-soft); border-radius: 8px; padding: 0 6px; line-height: 16px; font-weight: 600; }
.oka-tfoot { margin-top: auto; padding-top: 12px; display: flex; gap: 6px; flex-wrap: wrap; }
.oka-main { overflow-y: auto; background: var(--raised); position: relative; display: flex; flex-direction: column; min-width: 0; }
.oka-bar { position: sticky; top: 0; z-index: 2; display: flex; align-items: center; gap: 8px; padding: 7px 12px 7px 20px; background: color-mix(in srgb, var(--raised) 92%, transparent); backdrop-filter: blur(6px); border-bottom: 1px solid var(--line); min-height: 44px; }
.oka-art { padding: 28px 40px 40px; display: grid; gap: 16px; max-width: 780px; width: 100%; margin: 0 auto; align-content: start; flex: 1; }
.oka-ml { display: flex; align-items: center; gap: 6px; color: var(--muted); font-size: 12.5px; flex-wrap: wrap; margin-top: -6px; }
.oka-ban { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: 8px; background: var(--accent-soft); color: var(--accent-fg); text-align: left; width: 100%; flex-wrap: wrap; }
.oka-ban:hover { outline: 1px solid var(--accent); }
.oka-why { display: flex; gap: 10px; padding: 12px; border: 1px solid var(--line); border-radius: 10px; background: var(--muted-bg); } .oka-why p { margin-top: 6px; }
.oka-rest { display: flex; align-items: center; gap: 10px; } .oka-rest::before, .oka-rest::after { content: ""; flex: 1; height: 1px; background: var(--line); }
.oka-review { position: sticky; bottom: 0; display: flex; align-items: center; gap: 8px; padding: 10px 16px; border-top: 1px solid var(--line2); background: var(--raised); box-shadow: 0 -6px 16px rgb(0 0 0 / 0.05); flex-wrap: wrap; }
.oka-side { border-left: 1px solid var(--line); background: var(--bg); overflow-y: auto; padding: 18px 16px; display: grid; gap: 22px; align-content: start; min-width: 0; }
.oka-s h4 { font-size: 12px; color: var(--muted); font-weight: 500; margin: 0 0 8px; }
.oka-toc { display: block; width: 100%; text-align: left; padding: 3px 10px; border-left: 2px solid var(--line); color: var(--fg2); font-size: 12.5px; }
.oka-toc:hover { border-left-color: var(--accent); color: var(--fg); }
.oka-props { display: grid; grid-template-columns: 78px minmax(0, 1fr); gap: 9px 8px; margin: 0; font-size: 12.5px; align-items: center; }
.oka-props dt { color: var(--muted); } .oka-props dd { margin: 0; min-width: 0; }
.oka-pr { display: flex; align-items: center; gap: 8px; width: 100%; padding: 8px; border: 1px solid var(--line); border-radius: 8px; text-align: left; margin-bottom: 6px; }
.oka-pr:hover:not([disabled]) { border-color: var(--accent); } .oka-pr[disabled] { opacity: 0.6; cursor: default; }
.oka-lk { display: flex; align-items: center; gap: 6px; font-size: 12.5px; padding: 3px 0; min-width: 0; color: var(--fg2); } a.oka-lk:hover { color: var(--accent-fg); }
.oka-qi { display: flex; align-items: flex-start; gap: 12px; padding: 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--raised); text-align: left; width: 100%; cursor: pointer; }
.oka-qi:hover { border-color: var(--input); box-shadow: var(--shadow-sm); } .oka-qi > .grow { display: grid; gap: 4px; }
.oka-qi.done { opacity: 0.65; }
.oka-list { display: grid; gap: 8px; }
@container (max-width: 1000px) { .oka { grid-template-columns: 220px minmax(0, 1fr); } .oka-side { display: none; } }
@container (max-width: 640px) { .oka { grid-template-columns: 1fr; } .oka-tree { max-height: 260px; border-right: 0; border-bottom: 1px solid var(--line); } .oka-art { padding: 20px 16px 28px; } .ok-h1 { font-size: 21px; } .oka-main { overflow: visible; } }

/* ---------- knowledge B: beranda + kartu ---------- */
.okb { flex: 1; overflow-y: auto; padding: 24px 28px 32px; display: grid; gap: 20px; align-content: start; }
.okb-head { display: flex; gap: 16px; align-items: flex-start; flex-wrap: wrap; } .okb-head h1 { font-size: 20px; letter-spacing: -0.015em; } .okb-head p { margin-top: 4px; }
.okb-top { display: grid; grid-template-columns: minmax(0, 1.75fr) minmax(0, 1fr); gap: 16px; align-items: stretch; }
.okb-bh { display: flex; align-items: center; gap: 8px; padding: 12px 16px; border-bottom: 1px solid var(--line); min-height: 50px; } .okb-bh h3 { font-size: 14px; }
.okb-ar { display: flex; align-items: center; gap: 12px; padding: 11px 16px; border-top: 1px solid var(--line); } .okb-ar:first-child { border-top: 0; }
.okb-ar > .grow { display: grid; gap: 2px; } .okb-ar .acts { display: flex; gap: 6px; flex-shrink: 0; }
.okb-health { padding: 16px; display: grid; gap: 16px; align-content: start; }
.okb-donut { display: flex; align-items: center; gap: 18px; }
.okb-donut svg { width: 92px; height: 92px; flex-shrink: 0; }
.okb-leg { display: grid; gap: 6px; font-size: 12.5px; flex: 1; } .okb-leg > div { display: flex; align-items: center; gap: 8px; } .okb-leg b { margin-left: auto; font-weight: 500; font-variant-numeric: tabular-nums; }
.okb-sh { display: flex; align-items: flex-end; gap: 12px; } .okb-sh h2 { font-size: 14px; }
.okb-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
.okb-card { display: flex; flex-direction: column; gap: 12px; padding: 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--raised); text-align: left; min-width: 0; cursor: pointer; }
.okb-card:hover { border-color: var(--input); box-shadow: var(--shadow-sm); }
.okb-seg { display: flex; height: 6px; border-radius: 3px; overflow: hidden; gap: 2px; background: var(--hover); } .okb-seg i { display: block; height: 100%; }
.okb-dl { display: grid; gap: 6px; font-size: 12.5px; color: var(--fg2); flex: 1; align-content: start; }
.okb-cf { display: flex; align-items: center; gap: 8px; justify-content: space-between; border-top: 1px solid var(--line); padding-top: 10px; }
.okb-card.add { border-style: dashed; align-items: center; justify-content: center; color: var(--muted); text-align: center; background: transparent; }
.okb-doc { display: grid; grid-template-columns: minmax(0, 1fr) 280px; gap: 16px; align-items: start; }
.okb-rv { flex: 1; min-height: 0; display: flex; flex-direction: column; }
.okb-rvbar { display: flex; align-items: center; gap: 10px; padding: 9px 16px; background: var(--bg); border-bottom: 1px solid var(--line); flex-wrap: wrap; }
.okb-steps { display: flex; gap: 4px; } .okb-steps i { width: 20px; height: 4px; border-radius: 2px; background: var(--line2); } .okb-steps i.on { background: var(--accent); }
.okb-rvgrid { flex: 1; overflow-y: auto; display: grid; grid-template-columns: 300px minmax(0, 1fr); gap: 16px; padding: 20px; align-items: start; }
.okb-sbs { overflow: hidden; }
.okb-sbs-h, .okb-sbs-r { display: grid; grid-template-columns: 1fr 1fr; }
.okb-sbs-h > div { padding: 9px 14px; font-size: 12px; color: var(--muted); font-weight: 500; background: var(--muted-bg); border-bottom: 1px solid var(--line); display: flex; gap: 6px; align-items: center; }
.okb-sbs-h > div + div, .okb-sbs-r > div + div { border-left: 1px solid var(--line); }
.okb-sbs-r > div { padding: 5px 14px; font-size: 13px; line-height: 1.6; min-height: 30px; min-width: 0; }
.okb-sbs-r > .del { background: var(--danger-soft); color: var(--danger); } .okb-sbs-r > .add { background: var(--success-soft); color: var(--fg); }
.okb-sbs-r > .emp { background: repeating-linear-gradient(135deg, transparent 0 6px, var(--hover) 6px 7px); }
.okb-sbs-hd { padding: 6px 14px; font-size: 12px; color: var(--muted); background: var(--muted-bg); border-block: 1px solid var(--line); }
.okb-sbs-hd:first-child { border-top: 0; }
.okb-rvfoot { display: flex; align-items: center; gap: 8px; padding: 12px 16px; background: var(--raised); border-top: 1px solid var(--line2); flex-wrap: wrap; }
.okb-rvfoot .msg { margin-right: auto; color: var(--muted); display: flex; gap: 8px; align-items: center; }
.okb-kbd { display: grid; gap: 6px; font-size: 12px; color: var(--muted); } .okb-kbd > span { display: flex; gap: 8px; align-items: center; }
@container (max-width: 1000px) { .okb-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } .okb-top, .okb-rvgrid, .okb-doc { grid-template-columns: 1fr; } }
@container (max-width: 560px) { .okb-grid { grid-template-columns: 1fr; } .okb { padding: 16px; } .okb-sbs-h, .okb-sbs-r { grid-template-columns: 1fr; } .okb-sbs-r > .emp, .okb-sbs-h > div + div { display: none; } .okb-sbs-r > div + div { border-left: 0; } .okb-ar { flex-wrap: wrap; } .okb-ar > .grow { flex-basis: calc(100% - 48px); } .okb-ar .acts { width: 100%; justify-content: flex-end; } .okb-head > .grow { flex-basis: 100%; } .okb-rvgrid { padding: 12px; } }

/* ---------- knowledge C: daftar + panel ---------- */
.okc { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr) 400px; }
.okc-main { overflow-y: auto; padding: 20px 24px 28px; display: grid; gap: 16px; align-content: start; min-width: 0; }
.okc-panel { border-left: 1px solid var(--line); background: var(--raised); display: flex; flex-direction: column; min-height: 0; min-width: 0; }
.okc-ph { padding: 14px 16px 0; display: grid; gap: 8px; border-bottom: 1px solid var(--line); } .okc-ph .tabs { border-bottom: 0; margin: 0; }
.okc-pb { flex: 1; overflow-y: auto; padding: 16px; display: grid; gap: 14px; align-content: start; }
.okc-pf { display: flex; gap: 8px; justify-content: flex-end; padding: 10px 16px; border-top: 1px solid var(--line); background: var(--muted-bg); flex-wrap: wrap; align-items: center; }
.okc-n { display: flex; align-items: center; gap: 10px; min-width: 0; } .okc-n > span { display: grid; min-width: 0; }
.okc-p { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.okc-wrap .dt-t tbody tr { cursor: pointer; }
.okc-panel > .empty { margin: 16px; }
.okc-prop { border: 1px solid var(--line); border-radius: 10px; overflow: hidden; }
.okc-prop .h { padding: 10px 12px; display: grid; gap: 4px; background: var(--muted-bg); border-bottom: 1px solid var(--line); }
.okc-prop .diff { border: 0; border-radius: 0; }
.okc-prop .diff .ln { color: var(--fg2); }
@container (max-width: 860px) { .okc { grid-template-columns: 1fr; } .okc-panel { border-left: 0; border-top: 1px solid var(--line); } .okc-main { overflow: visible; padding: 16px; } }
`);

/* ====================== Bersama ====================== */
function rr(reset = []) {
  const keep = {};
  document.querySelectorAll("[data-ks]").forEach((e) => { if (!reset.includes(e.dataset.ks)) keep[e.dataset.ks] = e.scrollTop; });
  AG.rerender();
  document.querySelectorAll("[data-ks]").forEach((e) => { if (keep[e.dataset.ks] != null) e.scrollTop = keep[e.dataset.ks]; });
}
const tw = (s, px = 11.5) => String(s).length * px * 0.56;
const svgIc = (n, x, y, s, cls = "oo-i") => `<svg class="${cls}" x="${x}" y="${y}" width="${s}" height="${s}" viewBox="0 0 24 24">${(window.ICONS || {})[n] || ""}</svg>`;
const optTabs = (cur, act, opts) => `<nav class="tabs" aria-label="Pilihan opsi">${Object.entries(opts).map(([k, o]) => `<button class="${cur === k ? "on" : ""}" data-act="${act}" data-arg="${k}" aria-pressed="${cur === k}"><span class="ok-let">${k}</span>${o.n}</button>`).join("")}</nav>`;
const optInfo = (o, k, chosen, act) => `<div class="box ok-info"><div class="grow">
    <div class="row wrap"><span class="ok-let">${k}</span><b class="md b6">${o.n}</b>${chosen === k ? pill("Dipilih", "ok", "", "check") : ""}</div>
    <p class="mut">${o.d}</p>
    <div class="ok-pc"><div>${o.plus.map((x) => `<span class="row g8">${ic("check", "s14 text-ok")}<span>${x}</span></span>`).join("")}</div><div>${o.minus.map((x) => `<span class="row g8">${ic("minus", "s14 mut")}<span class="mut">${x}</span></span>`).join("")}</div></div></div>
  <div class="ok-info-act">${btn(chosen === k ? "Opsi ini dipilih" : "Pilih opsi ini", chosen === k ? "" : "pri", "check", { "data-act": act, "data-arg": k })}<span class="mut xs">${o.try}</span></div></div>`;
const frame = (url, inner) => `<div class="ok-frame"><div class="ok-chrome" aria-hidden="true"><span class="ok-dots"><i></i><i></i><i></i></span><span class="ok-url">${ic("lock", "s12")}${url}</span><span style="width:42px"></span></div><div class="ok-scr">${inner}</div></div>`;

/* ====================== Data knowledge ====================== */
const SRC = AG.KNOWLEDGE || { DOCS: [], PROPS: [] };
const GROUPS = SRC.DOCS.map(([g, i, list]) => ({ g, i, ids: list.map((d) => d[0]) }));
const DOCS = SRC.DOCS.flatMap(([g, , list]) => list.map((d) => ({ g, id: d[0], n: d[1], st: d[2], own: d[3], up: d[4], why: d[5] })));
const DOCS0 = JSON.stringify(DOCS);
const PR0 = JSON.stringify(SRC.PROPS);
let PR = JSON.parse(PR0);
const doc = (id) => DOCS.find((d) => d.id === id) || DOCS[0];
const grp = (g) => GROUPS.find((x) => x.g === g) || GROUPS[0];
const ST = { ok: ["Terbaru", "ok", "circle-check", "var(--success)"], stale: ["Basi", "warn", "clock", "var(--warn)"], prop: ["Ada usulan", "acc", "git-pull-request", "var(--accent)"] };
const TRIG = { done: ["Tiket Done", "circle-check"], merge: ["Merge ke main", "git-merge"], audit: ["Audit mingguan", "calendar-stats"], manual: ["Diminta manual", "hand-stop"] };
const OWNER = "AA";
const pending = () => PR.filter((p) => p.st === "pending");
const pendOf = (id) => PR.filter((p) => p.doc === id && p.st === "pending");
const stPill = (st) => pill(ST[st][0], ST[st][1], "", ST[st][2]);
const stDot = (st) => `<span class="ok-dot" style="background:${ST[st][3]}" title="${ST[st][0]}"></span>`;
const pm = (p) => `<span class="ok-pm"><b class="text-ok">+${p.diff.filter((x) => x[0] === "add").length}</b> <b class="text-dng">−${p.diff.filter((x) => x[0] === "del").length}</b></span>`;
const trigTxt = (p) => `${TRIG[p.trig][0]}${p.tk ? ` (${p.tk})` : p.trig === "merge" ? " · PR #139" : ""}`;
const pname = (k) => AG.PEOPLE[k].name;

const inl = (t) => esc(t).replace(/`([^`]+)`/g, "<code>$1</code>").replace(/\*\*(.+?)\*\*/g, "<b>$1</b>");
const cellsOf = (t) => t.split("|").slice(1, -1).map((c) => c.trim());
const dl = (t) => (t.startsWith("|") ? `<span class="ok-trow">${cellsOf(t).map((c) => `<span>${inl(c)}</span>`).join("")}</span>` : t.startsWith("## ") ? `<b class="ok-dh">${inl(t.slice(3))}</b>` : inl(t) || "&nbsp;");
const hdTxt = (t) => esc(t.replace(/@@/g, "").trim());
/* Tampilan saran (A): baris diff dibaca seperti dokumen. */
const suggest = (p) => `<div class="ok-sug">${p.diff.map(([c, t]) => (c === "hd" ? `<div class="ok-sug-hd">${ic("file-text", "s12")}${hdTxt(t)}</div>` : `<div class="ok-sug-l ${c}"><span class="ok-sig">${c === "add" ? "+" : c === "del" ? "−" : ""}</span><span class="ok-sug-t">${dl(t)}</span></div>`)).join("")}</div>`;
/* Berdampingan (B). */
function sbsRows(diff) {
  const rows = []; let i = 0;
  while (i < diff.length) {
    const [c, t] = diff[i];
    if (c === "hd") { rows.push(`<div class="okb-sbs-hd">${hdTxt(t)}</div>`); i++; continue; }
    if (c === "") { rows.push(`<div class="okb-sbs-r"><div>${dl(t)}</div><div>${dl(t)}</div></div>`); i++; continue; }
    const dels = [], adds = [];
    while (i < diff.length && diff[i][0] === "del") dels.push(diff[i++][1]);
    while (i < diff.length && diff[i][0] === "add") adds.push(diff[i++][1]);
    for (let j = 0; j < Math.max(dels.length, adds.length); j++) rows.push(`<div class="okb-sbs-r">${dels[j] != null ? `<div class="del">${dl(dels[j])}</div>` : `<div class="emp"></div>`}${adds[j] != null ? `<div class="add">${dl(adds[j])}</div>` : `<div class="emp"></div>`}</div>`);
  }
  return rows.join("");
}
/* Unified (C). */
const unified = (p) => `<div class="diff">${p.diff.map(([c, t]) => `<div class="${c || "ln"}">${c === "add" ? "+ " : c === "del" ? "- " : c === "hd" ? "" : "  "}${esc(t)}</div>`).join("")}</div>`;

const FIG = `<div class="ok-fig"><svg viewBox="0 0 520 132" role="img" aria-label="Diagram komponen Kasir Online">
  <path d="M120 66 H186"/><path d="M296 52 L362 30"/><path class="d" d="M362 100 L296 80"/><path d="M296 66 H340 V66"/>
  <rect x="10" y="44" width="110" height="44" rx="8"/><text x="65" y="63" text-anchor="middle">kasir-web</text><text class="s" x="65" y="78" text-anchor="middle">tablet kasir</text>
  <rect class="a" x="186" y="44" width="110" height="44" rx="8"/><text x="241" y="71" text-anchor="middle">kasir-api</text>
  <rect x="362" y="8" width="148" height="40" rx="8"/><text x="436" y="26" text-anchor="middle">PostgreSQL</text><text class="s" x="436" y="40" text-anchor="middle">34 tabel</text>
  <rect x="362" y="80" width="148" height="40" rx="8"/><text x="436" y="98" text-anchor="middle">Xendit QRIS</text><text class="s" x="436" y="112" text-anchor="middle">callback HMAC</text></svg></div>`;
const lk = (id, t) => `<a class="ok-dl" data-act="ok-link" data-arg="${id}">${t}</a>`;
const BODY = {
  overview: [["p", "Kasir Online adalah aplikasi kasir untuk toko retail: transaksi tunai dan QRIS, cetak struk, dan laporan penjualan harian."], ["h", "Untuk siapa"], ["ul", ["Kasir toko: membuat transaksi di tablet", "Supervisor: membatalkan transaksi dan mereset PIN", "Pemilik toko: membaca laporan penjualan"]], ["h", "Repo"], ["ul", ["<b>kasir-api</b> (Go): API, worker, integrasi pembayaran", "<b>kasir-web</b> (React + TypeScript): aplikasi kasir di tablet"]], ["h", "Mulai dari sini"], ["p", `Baca ${lk("arsitektur", "Arsitektur Sistem")} lalu ${lk("setup", "Setup & Cara Menjalankan")}.`]],
  arsitektur: [["p", `Kasir Online terdiri dari dua repo: <b>kasir-web</b> dan <b>kasir-api</b>. Pembayaran QRIS memakai Xendit, lihat ${lk("integrasi", "Integrasi Pihak Ketiga")}.`], ["h", "Komponen utama"], ["fig", FIG], ["h", "Layanan"], ["tbl", [["Komponen", "Repo", "Bahasa", "Pemilik"], ["Aplikasi kasir", "kasir-web", "TypeScript", "Dewi"], ["API & worker", "kasir-api", "Go", "Adi"], ["Cetak struk", "printer-bridge", "TypeScript", "Qori"]]], ["h", "Alur pembayaran QRIS"], ["p", "Kasir membuat tagihan QRIS. Callback dari Xendit diverifikasi sebelum status transaksi diubah menjadi Lunas."]],
  integrasi: [["p", `Kasir Online memakai tiga layanan pihak ketiga. Semua kredensial disimpan di secret manager, lihat ${lk("env", "Konfigurasi & Env Var")}.`], ["h", "Xendit QRIS"], ["p", "Kasir membuat tagihan lewat <code>POST /qr_codes</code>. QR berlaku 5 menit dan ditampilkan di layar pelanggan."], ["h", "Callback pembayaran"], ["p", "Callback dicek dengan token statis <code>X-CALLBACK-TOKEN</code>."], ["p", "Status transaksi baru berubah menjadi <b>Lunas</b> setelah callback valid diterima."], ["h", "Printer struk"], ["p", "printer-bridge mengirim perintah ESC/POS ke printer thermal 58 mm di jaringan lokal toko."], ["h", "Email laporan"], ["ul", ["Laporan harian dikirim lewat Mailgun", "Template di <code>kasir-api/templates/email</code>", "Batas 300 email per hari pada paket saat ini"]]],
  api: [["p", "Semua endpoint memakai prefix <code>/v1</code>, JSON, dan token Bearer kecuali login."], ["h", "Autentikasi"], ["tbl", [["Method", "Path", "Keterangan"], ["POST", "/v1/auth/login", "Login email + password admin toko"], ["POST", "/v1/auth/kasir", "Login kasir dengan nama pengguna"]]], ["h", "Transaksi"], ["tbl", [["Method", "Path", "Keterangan"], ["POST", "/v1/transactions", "Buat transaksi dari keranjang"], ["POST", "/v1/transactions/:id/qris", "Buat tagihan QRIS"], ["POST", "/v1/transactions/:id/void", "Batalkan (supervisor)"]]], ["h", "Konvensi"], ["ul", ["Error memakai format <code>{ code, message }</code>", "Pagination dengan <code>cursor</code>, bukan nomor halaman"]]],
  env: [["note", "Nilai secret tidak pernah ditulis di dokumen ini. Hanya nama dan kegunaannya."], ["h", "Pembayaran"], ["tbl", [["Nama", "Wajib", "Keterangan"], ["XENDIT_API_KEY", "ya", "Kunci API Xendit"], ["XENDIT_CALLBACK_TOKEN", "ya", "Token callback lama"]]], ["h", "Database"], ["tbl", [["Nama", "Wajib", "Keterangan"], ["DATABASE_URL", "ya", "Koneksi PostgreSQL"], ["DB_MAX_CONNS", "tidak", "Default 20"]]]],
  "aturan-bisnis": [["p", "Aturan toko yang wajib dipatuhi kode dan dicek agent QA."], ["h", "Pembulatan"], ["p", "Total dibulatkan ke bawah ke kelipatan Rp100 setelah diskon dan pajak, sekali saja di akhir."], ["h", "Diskon"], ["ul", ["Diskon per item maksimal 20% untuk kasir biasa", "Diskon di atas batas butuh PIN supervisor"]], ["h", "Pembatalan"], ["p", "Transaksi lunas hanya bisa dibatalkan supervisor di hari yang sama."]],
  "modul-receipt": [["p", "Modul <code>internal/receipt</code> menyusun dan mencetak struk."], ["h", "Render struk"], ["p", "Struk dirender sebagai teks 32 kolom."], ["h", "Cetak ulang"], ["p", "Cetak ulang dari riwayat menandai struk dengan teks SALINAN."]],
  "tech-stack": [["h", "Dependencies"], ["tbl", [["Paket", "Versi", "Dipakai untuk"], ["xendit-go", "v3.2", "Pembayaran QRIS"], ["sqlc", "1.25", "Query database"], ["React", "18.3", "Aplikasi kasir"]]], ["h", "Runtime"], ["ul", ["Go 1.23", "Node 20 LTS", "PostgreSQL 16"]]],
  debt: [["h", "Daftar"], ["ul", ["Scanner barcode membaca dua kali (KSR-37)", "Modul promo belum punya test integrasi"]], ["h", "Prioritas"], ["p", "Dibahas di setiap sprint planning."]],
};
const GEN = {
  "Gambaran umum": ["Ringkasan untuk anggota baru dan agent AI.", "Dipakai 42 toko retail dengan 2 sampai 6 tablet kasir per toko", "Rilis setiap dua minggu mengikuti sprint", "Bahasa dokumen: Indonesia"],
  Arsitektur: ["Keputusan dan struktur sistem yang jarang berubah.", "kasir-web memanggil kasir-api lewat REST + JSON", "Event struk.cetak dikirim lewat Redis Streams ke printer-bridge", "Waktu disimpan dalam UTC, ditampilkan dalam WIB"],
  "Domain & bisnis": ["Aturan toko yang harus dipatuhi kode.", "Satu transaksi hanya punya satu metode pembayaran", "Transaksi lunas tidak bisa diubah, hanya dibatalkan supervisor", "Status transaksi: Draf, Menunggu bayar, Lunas, Batal"],
  "Backend & kode": ["Struktur kode di kasir-api.", "Setiap modul punya handler, service, dan repository", "Query database memakai sqlc, bukan ORM", "Error domain memakai kode, mis. ERR_STOCK_INSUFFICIENT"],
  Frontend: ["Struktur aplikasi kasir di tablet.", "Routing memakai React Router dengan lazy loading", "Keranjang disimpan di Zustand dan IndexedDB", "Komponen UI dari packages/ui"],
  Kualitas: ["Standar kualitas yang dicek di setiap PR.", "Unit test wajib untuk service; Playwright untuk alur kasir utama", "Lint: golangci-lint dan ESLint di CI", "Cakupan test minimal 70% untuk payment dan stock"],
  Operasional: ["Cara menjalankan dan merawat sistem.", "Lingkungan: local, staging, production", "Deploy lewat GitHub Actions", "Backup database setiap 6 jam, disimpan 14 hari"],
  Riwayat: ["Catatan yang dihasilkan dari tiket Done.", "v1.8 · Login kasir dengan PIN 6 digit (KSR-22)", "v1.7 · Perbaikan pembulatan total belanja (KSR-21)", "v1.6 · Cetak struk lewat printer-bridge"],
};
const bodyOf = (d) => BODY[d.id] || (() => { const g = GEN[d.g] || GEN.Arsitektur; return [["p", `${d.n} untuk Kasir Online, disusun Agent Tech Writer dari code graph repo kasir-api dan kasir-web.`], ["h", "Ringkasan"], ["p", g[0]], ["h", "Detail"], ["ul", g.slice(1)], ["h", "Terkait"], ["p", `Lihat juga ${lk("overview", "Overview Project")} dan ${lk("arsitektur", "Arsitektur Sistem")}.`]]; })();
function docBody(d, pre = "") {
  let hi = 0;
  return bodyOf(d).map(([k, v]) => {
    if (k === "h") return `<h2${pre ? ` id="${pre}-${hi++}"` : ""}>${v}</h2>`;
    if (k === "p") return `<p>${v}</p>`;
    if (k === "ul") return `<ul>${v.map((x) => `<li>${x}</li>`).join("")}</ul>`;
    if (k === "tbl") return `<div class="ok-tw"><table class="ok-tbl">${v.map((r, i) => `<tr>${r.map((c) => (i ? `<td>${c}</td>` : `<th>${c}</th>`)).join("")}</tr>`).join("")}</table></div>`;
    if (k === "note") return `<div class="alert gray">${ic("info-circle", "s14")}<span>${v}</span></div>`;
    return v;
  }).join("");
}
const outline = (d) => bodyOf(d).filter((x) => x[0] === "h").map((x) => x[1]);
const REL = {
  integrasi: [["KSR-24", "KSR-41"], ["internal/payment/xendit.go", "internal/payment/callback.go"]],
  api: [["KSR-22", "KSR-24"], ["internal/http/routes.go", "internal/auth/pin.go"]],
  env: [["KSR-24"], ["internal/config/config.go"]],
  "aturan-bisnis": [["KSR-21", "KSR-35"], ["internal/cart/total.go"]],
  "modul-receipt": [["KSR-30", "KSR-29"], ["internal/receipt/render.go"]],
  arsitektur: [["KSR-40"], ["cmd/api/main.go"]],
  "tech-stack": [[], ["go.mod", "kasir-web/package.json"]],
  debt: [["KSR-37"], ["internal/scanner/dedupe.go"]],
};
const relHtml = (d) => { const [tk, code] = REL[d.id] || [[], []]; return (tk.length || code.length) ? `${tk.map((k) => `<div class="oka-lk">${tref(k)}<span class="trunc">${esc(AG.ticket(k).t)}</span></div>`).join("")}${code.map((c) => `<a class="oka-lk" href="#/p/${K}/graph">${ic("file-code", "s14 faint")}<span class="mono trunc">${c}</span></a>`).join("")}` : `<span class="mut sm">Belum ada tiket atau kode terkait.</span>`; };
const STALE_DIFF = {
  "tech-stack": [["hd", "@@ tech-stack.md · Dependencies @@"], ["", "| Paket | Versi | Dipakai untuk |"], ["del", "| xendit-go | v3.2 | Pembayaran QRIS |"], ["add", "| xendit-go | v4.1 | Pembayaran QRIS (signature HMAC) |"]],
  "modul-receipt": [["hd", "@@ modul-receipt.md · Render struk @@"], ["del", "Struk dirender sebagai teks 32 kolom."], ["add", "Struk dirender 32 kolom (printer 58 mm) atau 48 kolom (80 mm) sesuai printer toko."], ["add", "Nama kasir diambil dari sesi login PIN."]],
  debt: [["hd", "@@ technical-debt.md · Daftar @@"], ["", "- Scanner barcode membaca dua kali (KSR-37)"], ["add", "- 4 TODO baru di internal/receipt dan internal/stock"], ["add", "- 2 fungsi tidak terpakai: legacyRound(), oldPrinterInit()"]],
};

/* ---------- Aksi knowledge ---------- */
const KS = { opt: "A", pick: null, a: { view: "doc", doc: "integrasi", mode: "doc", closed: new Set(["Gambaran umum", "Domain & bisnis", "Frontend", "Kualitas", "Operasional", "Riwayat"]) }, b: { view: "home", g: null, doc: null, ri: 0, from: "home" }, c: { v: "all", sel: "integrasi", tab: "diff" } };
function settle(p, st) {
  p.st = st; p.by = AG.ME;
  const d = doc(p.doc);
  if (!pendOf(d.id).length) d.st = st === "approved" ? "ok" : p.trig === "audit" || d.why ? "stale" : "ok";
  if (st === "approved") { d.up = "baru saja"; }
}
function staleProp(id) {
  const d = doc(id), pid = "s-" + id;
  if (PR.find((x) => x.id === pid && x.st === "pending")) return false;
  PR = PR.filter((x) => x.id !== pid);
  PR.unshift({ id: pid, doc: id, trig: "manual", w: "baru saja", tok: "14rb token", st: "pending", why: `${d.why}. Diperbarui atas permintaan ${pname(AG.ME)}.`, diff: STALE_DIFF[id] || [["hd", `@@ ${id}.md @@`], ["add", "Bagian diperbarui mengikuti kode terbaru."]] });
  d.st = "prop"; return true;
}
AG.action("ok-opt", (el, k) => { KS.opt = k; AG.rerender(); });
AG.action("ok-pick", (el, k) => { KS.pick = k; AG.rerender(); AG.toast(`Opsi ${k} dicatat sebagai pilihan tata letak knowledge`); });
AG.action("ok-reset", () => { PR = JSON.parse(PR0); JSON.parse(DOCS0).forEach((d, i) => Object.assign(DOCS[i], d)); KS.b.ri = 0; rr(); AG.toast("Data contoh dikembalikan"); });
AG.action("ok-approve", (el, id) => { const p = PR.find((x) => x.id === id); if (!p || p.st !== "pending") return; settle(p, "approved"); KS.a.mode = "doc"; rr(); AG.toast(`Usulan “${doc(p.doc).n}” disetujui · commit ke .agentgate/knowledge/`); });
AG.action("ok-reject-ok", (el, id) => { const p = PR.find((x) => x.id === id); settle(p, "rejected"); KS.a.mode = "doc"; AG.closeDialog(); rr(); AG.toast("Usulan ditolak · alasan dikirim ke Agent Tech Writer"); });
AG.dialog("ok-reject", (id) => { const p = PR.find((x) => x.id === id); return { icon: "x", tone: "dng", title: "Tolak usulan", desc: `Usulan untuk “${doc(p.doc).n}” tidak akan menimpa dokumen.`,
  body: AG.fld("Alasan untuk AI", AG.inp("", { area: true, ph: "Mis. endpoint /pin/reset belum dirilis, jangan dicatat dulu" }), "Alasan dipakai Agent Tech Writer agar usulan berikutnya lebih tepat."),
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Tolak usulan", "dngs", "", { "data-act": "ok-reject-ok", "data-arg": id })}` }; });
AG.action("ok-stale-ok", (el, id) => { const d = doc(id); d.st = "ok"; d.up = "baru saja"; rr(); AG.toast(`“${d.n}” ditandai masih benar · dicek lagi di audit 17 Nov`); });
AG.action("ok-stale-ai", (el, id) => { staleProp(id); rr(); AG.toast("Agent Tech Writer menyusun usulan · masuk ke antrean tinjau"); });
AG.action("ok-link", (el, id) => {
  if (KS.opt === "A") { Object.assign(KS.a, { view: "doc", doc: id, mode: "doc" }); KS.a.closed.delete(doc(id).g); rr(["oka-main"]); }
  else if (KS.opt === "B") { Object.assign(KS.b, { view: "doc", doc: id }); rr(["okb"]); }
  else { KS.c.sel = id; KS.c.tab = "pre"; rr(["okc-pb"]); }
});

/* ====================== Knowledge A: tiga panel ====================== */
AG.action("oka-doc", (el, id) => { Object.assign(KS.a, { view: "doc", doc: id, mode: "doc" }); rr(["oka-main", "oka-side"]); });
AG.action("oka-review", (el, id) => { Object.assign(KS.a, { view: "doc", doc: id, mode: "diff" }); KS.a.closed.delete(doc(id).g); rr(["oka-main", "oka-side"]); });
AG.action("oka-view", (el, v) => { KS.a.view = v; rr(["oka-main", "oka-side"]); });
AG.action("oka-mode", (el, m) => { KS.a.mode = m; rr(["oka-main"]); });
AG.action("oka-grp", (el, g) => { const c = KS.a.closed; c.has(g) ? c.delete(g) : c.add(g); rr(); });
AG.action("oka-toc", (el, id) => { document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" }); });

function kA() {
  const s = KS.a, pend = pending(), stale = DOCS.filter((d) => d.st === "stale");
  const qni = (k, i, l, n, tone) => `<button class="oka-ni ${s.view === k ? "on" : ""}" data-act="oka-view" data-arg="${k}">${ic(i, "s14")}<span class="grow trunc">${l}</span>${n ? `<span class="oka-c ${tone}">${n}</span>` : ""}</button>`;
  const tree = `<aside class="oka-tree" data-ks="oka-tree" aria-label="Pohon dokumen">
    <label class="oka-q">${ic("search", "s14")}<input placeholder="Cari dokumen" aria-label="Cari dokumen" data-oka-q></label>
    ${qni("queue", "git-pull-request", "Usulan menunggu", pend.length, "acc")}${qni("stale", "clock", "Dokumen basi", stale.length, "warn")}
    <div class="oka-sec">Dokumen<span>${DOCS.length}</span></div>
    ${GROUPS.map((G) => { const open = !s.closed.has(G.g) || !!s.q;
      const badge = G.ids.filter((id) => pendOf(id).length).length;
      return `<div class="oka-g" data-g="${esc(G.g)}"><button class="oka-gh" data-act="oka-grp" data-arg="${esc(G.g)}" aria-expanded="${open}">${ic(open ? "chevron-down" : "chevron-right", "s12")}${ic(G.i, "s14")}<span class="grow trunc">${G.g}</span>${!open && badge ? stDot("prop") : ""}<span class="faint sm">${G.ids.length}</span></button>
        ${open ? G.ids.map((id) => { const d = doc(id), n = pendOf(id).length; return `<button class="oka-d ${s.view === "doc" && s.doc === id ? "on" : ""}" data-act="oka-doc" data-arg="${id}" data-n="${esc(d.n.toLowerCase())}">${stDot(d.st)}<span class="trunc grow">${d.n}</span>${n ? `<span class="oka-b">${n}</span>` : ""}</button>`; }).join("") : ""}</div>`; }).join("")}
    <div class="oka-tfoot">${btn("Dokumen baru", "sm", "plus", { "data-dialog": "kb-new" })}${btn("Scan ulang", "sm", "refresh", { "data-dialog": "kb-scan" })}</div>
  </aside>`;
  let mid = "", side = "";
  if (s.view === "doc") {
    const d = doc(s.doc), p = pendOf(d.id)[0], diff = !!p && s.mode === "diff", out = outline(d), all = PR.filter((x) => x.doc === d.id);
    mid = `<main class="oka-main" data-ks="oka-main">
      <div class="oka-bar"><span class="ok-crumbs sm">${ic(grp(d.g).i, "s14")}${d.g}<span class="faint">/</span><b>${d.n}</b></span><span class="grow"></span>
        ${p ? `<div class="seg hide-sm" role="group" aria-label="Mode tampilan"><button class="${diff ? "" : "on"}" data-act="oka-mode" data-arg="doc">Dokumen</button><button class="${diff ? "on" : ""}" data-act="oka-mode" data-arg="diff">Perubahan diusulkan</button></div>` : ""}
        ${btn("", "ghost icon sm", "pencil", { "data-go": `#/p/${K}/knowledge/${d.id}`, "aria-label": "Edit dokumen", title: "Edit dokumen" })}${btn("", "ghost icon sm", "dots", { "data-menu": "kb-doc", "data-arg": d.id, "aria-label": "Aksi dokumen" })}</div>
      <article class="oka-art">
        <h1 class="ok-h1">${d.n}</h1>
        <div class="oka-ml">${av(d.own, 18)}<span>${pname(d.own)}</span><span class="faint">·</span><span>Diperbarui ${d.up}</span><span class="faint">·</span><span class="mono">${d.id}.md</span></div>
        ${d.st === "stale" ? `<div class="alert warn">${ic("clock", "s14")}<span class="grow"><b>Dokumen basi.</b> ${d.why}.</span></div><div class="row wrap">${btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "ok-stale-ai", "data-arg": d.id })}${btn("Tandai masih benar", "sm ghost", "check", { "data-act": "ok-stale-ok", "data-arg": d.id })}</div>` : ""}
        ${p && !diff ? `<button class="oka-ban" data-act="oka-mode" data-arg="diff">${ic("sparkles", "s14")}<span class="grow"><b>Agent Tech Writer mengusulkan perubahan</b> · ${trigTxt(p)} · ${p.w}</span>${pm(p)}<span class="row g4 b5">Tinjau ${ic("arrow-right", "s14")}</span></button>` : ""}
        ${diff ? `<div class="oka-why">${av("writer", 24)}<div class="grow"><div class="b5">Usulan Agent Tech Writer</div><div class="mut sm">Dipicu ${trigTxt(p)} · ${p.w} · ${p.tok}</div><p>${p.why}</p></div></div>${suggest(p)}<div class="oka-rest"><span class="mut sm">Bagian lain tidak berubah</span></div><div class="ok-prose dim">${docBody(d)}</div>`
          : `<div class="ok-prose">${docBody(d, "okh-" + d.id)}</div>`}
      </article>
      ${diff ? `<div class="oka-review">${ic("git-pull-request", "s14 text-acc")}<span class="grow"><b class="b5">Tinjau usulan</b> <span class="mut sm">· ${pm(p)} · menunggu ${pname(OWNER)}</span></span>${btn("Tolak", "sm", "x", { "data-dialog": "ok-reject", "data-arg": p.id })}${btn("Edit dulu", "sm", "pencil", { "data-go": `#/p/${K}/knowledge/${d.id}` })}${btn("Approve", "sm pri", "check", { "data-act": "ok-approve", "data-arg": p.id })}</div>` : ""}
    </main>`;
    side = `<aside class="oka-side" data-ks="oka-side" aria-label="Info dokumen">
      ${out.length && !diff ? `<div class="oka-s"><h4>Di halaman ini</h4>${out.map((h, i) => `<button class="oka-toc" data-act="oka-toc" data-arg="okh-${d.id}-${i}">${h}</button>`).join("")}</div>` : ""}
      <div class="oka-s"><h4>Properti</h4><dl class="oka-props"><dt>Status</dt><dd>${stPill(d.st)}</dd><dt>Pemilik</dt><dd class="row g4">${av(d.own, 18)}${pname(d.own).split(" ")[0]}</dd><dt>Diperbarui</dt><dd>${d.up}</dd><dt>Sumber</dt><dd class="mono trunc xs" title=".agentgate/knowledge/${d.id}.md">${d.id}.md</dd></dl></div>
      <div class="oka-s"><h4>Usulan perubahan</h4>${all.length ? all.map((x) => `<button class="oka-pr" data-act="oka-mode" data-arg="diff" ${x.st !== "pending" ? "disabled" : ""}>${ic(TRIG[x.trig][1], "s14 faint")}<span class="grow"><span class="sm b5">${TRIG[x.trig][0]}</span><br><span class="xs mut">${x.w} · ${x.st === "pending" ? "menunggu" : x.st === "approved" ? "disetujui" : "ditolak"}</span></span>${pm(x)}</button>`).join("") : `<span class="mut sm">Belum ada usulan.</span>`}</div>
      <div class="oka-s"><h4>Terhubung</h4>${relHtml(d)}</div>
      <div class="oka-s"><h4>Riwayat</h4><div class="col g4 sm"><span class="row g4"><span class="mono mut">9f3c2a1</span><span class="trunc">${d.up} · ${pname(d.own).split(" ")[0]}</span></span><span class="row g4"><span class="mono mut">c27e9f4</span><span class="trunc">3 Nov · scan awal</span></span></div>${btn("Semua versi", "sm ghost", "history", { "data-dialog": "kb-history" })}</div>
    </aside>`;
  } else {
    const queue = s.view === "queue";
    const items = queue ? pend : stale;
    const doneP = PR.filter((p) => p.st !== "pending");
    mid = `<main class="oka-main" data-ks="oka-main"><div class="oka-art">
      <h1 class="ok-h1">${queue ? "Usulan menunggu" : "Dokumen basi"}</h1>
      <p class="mut" style="margin-top:-8px">${queue ? "Perubahan dari AI baru menimpa dokumen setelah disetujui pemilik knowledge. Klik untuk meninjau di dalam dokumennya." : "Audit mingguan menemukan dokumen yang tidak lagi cocok dengan kode."}</p>
      <div class="oka-list">${items.map((x) => { if (queue) { const d = doc(x.doc); return `<div class="oka-qi" role="button" tabindex="0" data-act="oka-review" data-arg="${x.doc}"><span class="tile acc">${ic(TRIG[x.trig][1], "s14")}</span><span class="grow"><span class="row wrap g8"><b class="b5">${d.n}</b><span class="mut sm">${d.g}</span></span><span class="sm">${x.why}</span><span class="xs mut row g4">${av("writer", 14)}Agent Tech Writer · ${trigTxt(x)} · ${x.w}</span></span>${pm(x)}${ic("chevron-right", "s14 faint")}</div>`; }
        return `<div class="oka-qi" role="button" tabindex="0" data-act="oka-doc" data-arg="${x.id}"><span class="tile warn">${ic("clock", "s14")}</span><span class="grow"><span class="row wrap g8"><b class="b5">${x.n}</b><span class="mut sm">${x.g}</span></span><span class="sm">${x.why}</span><span class="row wrap g8" style="margin-top:4px">${btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "ok-stale-ai", "data-arg": x.id })}${btn("Masih benar", "sm ghost", "check", { "data-act": "ok-stale-ok", "data-arg": x.id })}</span></span></div>`; }).join("") || AG.empty("circle-check", queue ? "Semua usulan sudah ditinjau" : "Tidak ada dokumen basi", queue ? "Usulan baru muncul setelah tiket Done, merge ke main, atau audit mingguan." : "Semua dokumen cocok dengan kode terbaru.")}</div>
      ${queue && doneP.length ? `<div class="oka-s"><h4 style="font-size:12px;color:var(--muted);font-weight:500;margin:8px 0">Sudah ditinjau</h4><div class="oka-list">${doneP.map((x) => `<div class="oka-qi done" data-act="oka-doc" data-arg="${x.doc}">${x.st === "approved" ? `<span class="tile ok">${ic("check", "s14")}</span>` : `<span class="tile dng">${ic("x", "s14")}</span>`}<span class="grow"><b class="b5">${doc(x.doc).n}</b><span class="mut sm">${x.st === "approved" ? "Disetujui" : "Ditolak"} oleh ${pname(x.by).split(" ")[0]}</span></span>${pm(x)}</div>`).join("")}</div></div>` : ""}
    </div></main>`;
    side = `<aside class="oka-side" data-ks="oka-side"><div class="oka-s"><h4>Cara kerja usulan</h4><div class="col g8 sm">${[["circle-check", "Tiket Done, merge ke main, atau audit mingguan memicu Agent Tech Writer."], ["git-pull-request", "Perubahan masuk sebagai diff, dokumen belum berubah."], ["user-check", "Pemilik knowledge menyetujui, menolak, atau mengedit dulu."], ["git-commit", "Yang disetujui menjadi commit di .agentgate/knowledge/."]].map(([i, t]) => `<span class="row g8" style="align-items:flex-start">${ic(i, "s14 faint")}<span>${t}</span></span>`).join("")}</div></div>
      <div class="oka-s"><h4>Pemilik knowledge</h4><div class="row g8">${av(OWNER, 22)}<span class="b5">${pname(OWNER)}</span></div></div></aside>`;
  }
  return `<div class="ok-top"><span class="ok-crumbs">${AG.projDot(K)}Kasir Online<span class="faint">/</span><b>Knowledge</b></span></div><div class="oka">${tree}${mid}${side}</div>`;
}

/* ====================== Knowledge B: beranda + kartu ====================== */
AG.action("okb-go", (el, a) => {
  const s = KS.b; const [k, v] = [a.slice(0, a.indexOf(":") < 0 ? a.length : a.indexOf(":")), a.slice(a.indexOf(":") + 1)];
  if (k === "home") s.view = "home";
  if (k === "g") { s.view = "group"; s.g = v; }
  if (k === "d") { s.view = "doc"; s.doc = v; }
  if (k === "r") { s.from = s.view; s.view = "review"; s.ri = Math.max(0, pending().findIndex((p) => p.id === v)); }
  if (k === "n") s.ri = Math.max(0, Math.min(pending().length - 1, s.ri + +v));
  rr(["okb", "okb-rv"]);
});
const donut = (parts, total, center) => { const r = 36, C = 2 * Math.PI * r; let off = 0;
  return `<svg viewBox="0 0 92 92" role="img" aria-label="Kesehatan dokumen ${center}"><circle cx="46" cy="46" r="${r}" fill="none" stroke="var(--hover)" stroke-width="10"/>${parts.map(([v, c]) => { const len = (v / total) * C; const s = `<circle cx="46" cy="46" r="${r}" fill="none" stroke="${c}" stroke-width="10" stroke-dasharray="${Math.max(0, len - 2)} ${C}" stroke-dashoffset="${-off}" transform="rotate(-90 46 46)"/>`; off += len; return s; }).join("")}<text x="46" y="50" text-anchor="middle" style="font-size:17px;font-weight:600;fill:var(--fg)">${center}</text></svg>`; };
function kBHome() {
  const pend = pending(), stale = DOCS.filter((d) => d.st === "stale"), n = (st) => DOCS.filter((d) => d.st === st).length;
  const rows = [
    ...pend.map((p) => { const d = doc(p.doc); return `<div class="okb-ar"><span class="tile acc">${ic("git-pull-request", "s14")}</span><div class="grow"><div class="row wrap g8"><b class="b5">${d.n}</b><span class="mut sm">usulan · ${trigTxt(p)}</span></div><div class="mut sm trunc">${p.why}</div></div>${pm(p)}<span class="acts">${btn("Tinjau", "sm", "", { "data-act": "okb-go", "data-arg": "r:" + p.id })}</span></div>`; }),
    ...stale.map((d) => `<div class="okb-ar"><span class="tile warn">${ic("clock", "s14")}</span><div class="grow"><div class="row wrap g8"><b class="b5">${d.n}</b><span class="mut sm">basi sejak ${d.up}</span></div><div class="mut sm trunc">${d.why}</div></div><span class="acts">${btn("Masih benar", "sm ghost", "", { "data-act": "ok-stale-ok", "data-arg": d.id })}${btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "ok-stale-ai", "data-arg": d.id })}</span></div>`),
  ];
  return `<div class="okb" data-ks="okb">
    <div class="okb-head"><div class="grow"><h1>Knowledge</h1><p class="mut">Living document Kasir Online. Sumber kebenaran di <code>.agentgate/knowledge/</code>.</p></div><div class="row">${btn("Dokumen baru", "", "plus", { "data-dialog": "kb-new" })}${btn("Scan ulang", "pri", "refresh", { "data-dialog": "kb-scan" })}</div></div>
    ${AG.stats([{ label: "Dokumen", value: DOCS.length, icon: "file-text", hint: "8 dari 10 kelompok relevan" }, { label: "Terbaru", value: n("ok"), icon: "circle-check", tone: "ok", hint: "sinkron dengan kode" }, { label: "Usulan menunggu", value: pend.length, icon: "git-pull-request", tone: "acc", hint: "perlu persetujuan" }, { label: "Basi", value: stale.length, icon: "clock", tone: "warn", hint: "dari audit mingguan" }])}
    <div class="okb-top">
      <div class="box"><div class="okb-bh"><h3>Perlu tindakan</h3><span class="pill gray">${rows.length}</span><span class="grow"></span>${pend.length ? btn(`Tinjau ${pend.length} usulan`, "sm pri", "player-play", { "data-act": "okb-go", "data-arg": "r:" + pend[0].id }) : ""}</div>
        ${rows.join("") || `<div style="padding:16px">${AG.empty("circle-check", "Tidak ada yang perlu ditindak", "Semua usulan sudah ditinjau dan tidak ada dokumen basi.")}</div>`}</div>
      <div class="box"><div class="okb-bh"><h3>Kesehatan dokumen</h3></div><div class="okb-health">
        <div class="okb-donut">${donut([[n("ok"), "var(--success)"], [n("prop"), "var(--accent)"], [n("stale"), "var(--warn)"]], DOCS.length, Math.round((n("ok") / DOCS.length) * 100) + "%")}
          <div class="okb-leg">${[["ok", "Terbaru"], ["prop", "Ada usulan"], ["stale", "Basi"]].map(([k, l]) => `<div>${stDot(k)}${l}<b>${n(k)}</b></div>`).join("")}</div></div>
        <div class="col g4"><span class="row g4 sm">${ic("loader-2", "s14 text-acc")}<span class="grow">Scan ulang <b class="b5">Mendalam</b> · Laptop Dewi</span><span class="mut tab-num">64%</span></span><span class="bar"><i style="width:64%"></i></span><span class="mono xs mut">kasir-api/internal/receipt</span></div>
        <div class="row g8 sm">${av(OWNER, 20)}<span class="grow">Pemilik knowledge: <b class="b5">${pname(OWNER)}</b></span></div>
      </div></div>
    </div>
    <div class="okb-sh"><div class="grow"><h2>Kelompok dokumen</h2><p class="mut sm">Klik kartu untuk melihat semua dokumen di kelompok itu.</p></div></div>
    <div class="okb-grid">${GROUPS.map((G) => { const ds = G.ids.map(doc), c = (st) => ds.filter((d) => d.st === st).length; const own = [...new Set(ds.map((d) => d.own))];
      return `<div class="okb-card" role="button" tabindex="0" data-act="okb-go" data-arg="g:${esc(G.g)}"><div class="row"><span class="tile">${ic(G.i, "s14")}</span><b class="grow b6 trunc">${G.g}</b><span class="mut sm">${ds.length}</span></div>
        <span class="okb-seg" title="${c("ok")} terbaru, ${c("prop")} usulan, ${c("stale")} basi">${["ok", "prop", "stale"].map((st) => (c(st) ? `<i style="flex:${c(st)};background:${ST[st][3]}"></i>` : "")).join("")}</span>
        <span class="okb-dl">${ds.slice(0, 4).map((d) => `<span class="row g8">${stDot(d.st)}<span class="trunc">${d.n}</span></span>`).join("")}${ds.length > 4 ? `<span class="mut sm">+${ds.length - 4} dokumen lainnya</span>` : ""}</span>
        <span class="okb-cf"><span class="sm ${c("stale") ? "text-warn" : c("prop") ? "text-acc" : "mut"}">${c("stale") ? `${c("stale")} basi` : c("prop") ? `${c("prop")} usulan` : "Semua terbaru"}</span><span class="stack">${own.slice(0, 3).map((k) => av(k, 20)).join("")}</span></span></div>`; }).join("")}
      <div class="okb-card add" role="button" tabindex="0" data-dialog="kb-new">${ic("plus", "s18")}<b class="b5">Dokumen atau jenis custom</b><span class="sm">Mis. SOP tutup kasir harian</span></div></div>
    <div class="mut sm row g4">${ic("info-circle", "s14")}Kelompok Mobile dan Data tidak dibuat karena tidak relevan untuk Kasir Online.</div>
  </div>`;
}
function kBGroup() {
  const G = grp(KS.b.g), ds = G.ids.map(doc);
  return `<div class="okb" data-ks="okb">
    <button class="ok-back" data-act="okb-go" data-arg="home">${ic("arrow-left", "s14")}Knowledge</button>
    <div class="okb-head"><span class="tile lg">${ic(G.i, "s18")}</span><div class="grow"><h1>${G.g}</h1><p class="mut">${ds.length} dokumen · ${(GEN[G.g] || [""])[0]}</p></div>${btn("Scan ulang kelompok", "", "refresh", { "data-dialog": "kb-scan" })}</div>
    <div class="box">${ds.map((d) => { const p = pendOf(d.id)[0]; return `<div class="lrow" data-act="okb-go" data-arg="d:${d.id}" role="button" tabindex="0" style="cursor:pointer">${stDot(d.st)}<span class="grow" style="min-width:0"><b class="b5">${d.n}</b><br><span class="sm ${d.st === "stale" ? "text-warn" : "mut"}">${d.st === "stale" ? d.why : p ? p.why : `.agentgate/knowledge/${d.id}.md`}</span></span>${p ? btn("Tinjau usulan", "sm", "", { "data-act": "okb-go", "data-arg": "r:" + p.id }) : stPill(d.st)}<span class="row g4 mut sm hide-sm" style="width:90px">${av(d.own, 18)}${d.up}</span>${ic("chevron-right", "s14 faint")}</div>`; }).join("")}</div>
  </div>`;
}
function kBDoc() {
  const d = doc(KS.b.doc), p = pendOf(d.id)[0];
  return `<div class="okb" data-ks="okb">
    <span class="ok-crumbs"><button class="ok-back" data-act="okb-go" data-arg="home">Knowledge</button><span class="faint">/</span><button class="ok-back" data-act="okb-go" data-arg="g:${esc(d.g)}">${d.g}</button><span class="faint">/</span><b>${d.n}</b></span>
    <div class="okb-doc"><div class="box" style="padding:28px 32px"><div class="col g16"><h1 class="ok-h1">${d.n}</h1>
      ${p ? `<div class="alert">${ic("git-pull-request", "s14")}<span class="grow"><b>Ada usulan perubahan</b> · ${trigTxt(p)} · ${pm(p)}</span>${btn("Tinjau", "sm pri", "", { "data-act": "okb-go", "data-arg": "r:" + p.id })}</div>` : ""}
      ${d.st === "stale" ? `<div class="alert warn">${ic("clock", "s14")}<span class="grow"><b>Basi.</b> ${d.why}.</span>${btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "ok-stale-ai", "data-arg": d.id })}</div>` : ""}
      <div class="ok-prose">${docBody(d)}</div></div></div>
      <div class="col g12"><div class="box pad"><dl class="oka-props"><dt>Status</dt><dd>${stPill(d.st)}</dd><dt>Pemilik</dt><dd class="row g4">${av(d.own, 18)}${pname(d.own)}</dd><dt>Diperbarui</dt><dd>${d.up}</dd><dt>Sumber</dt><dd class="mono xs trunc">${d.id}.md</dd></dl></div>
        <div class="box pad col g8"><b class="sm b5">Terhubung</b>${relHtml(d)}</div>
        <div class="row">${btn("Edit dokumen", "", "pencil", { "data-go": `#/p/${K}/knowledge/${d.id}` })}${btn("", "icon", "dots", { "data-menu": "kb-doc", "data-arg": d.id, "aria-label": "Aksi dokumen" })}</div></div></div>
  </div>`;
}
function kBReview() {
  const list = pending(); const s = KS.b;
  if (!list.length) return `<div class="okb" data-ks="okb">${AG.empty("circle-check", "Semua usulan sudah ditinjau", "Dokumen yang disetujui sudah di-commit ke .agentgate/knowledge/.", btn("Kembali ke beranda", "pri", "arrow-left", { "data-act": "okb-go", "data-arg": "home" }))}</div>`;
  s.ri = Math.min(s.ri, list.length - 1);
  const p = list[s.ri], d = doc(p.doc);
  return `<div class="okb-rv">
    <div class="okb-rvbar">${btn("", "ghost icon sm", "x", { "data-act": "okb-go", "data-arg": "home", "aria-label": "Tutup mode tinjau" })}<b>Tinjau usulan</b><span class="mut">${s.ri + 1} dari ${list.length}</span><span class="okb-steps" aria-hidden="true">${list.map((x, i) => `<i class="${i === s.ri ? "on" : ""}"></i>`).join("")}</span><span class="grow"></span>
      ${btn("", "sm icon", "chevron-up", { "data-act": "okb-go", "data-arg": "n:-1", "aria-label": "Usulan sebelumnya (K)", ...(s.ri ? {} : { disabled: true }) })}${btn("", "sm icon", "chevron-down", { "data-act": "okb-go", "data-arg": "n:1", "aria-label": "Usulan berikutnya (J)", ...(s.ri < list.length - 1 ? {} : { disabled: true }) })}</div>
    <div class="okb-rvgrid" data-ks="okb-rv">
      <aside class="box pad col g16">
        <div class="col g4"><span class="mut sm">${ic(grp(d.g).i, "s14")} ${d.g}</span><a class="md b6 ok-dl" style="border:0;width:fit-content" data-act="okb-go" data-arg="d:${d.id}">${d.n}</a><span class="mono xs mut">.agentgate/knowledge/${d.id}.md</span></div>
        <div class="col g4"><span class="mut xs">Kenapa berubah</span><p>${p.why}</p></div>
        <dl class="oka-props"><dt>Dipicu</dt><dd class="row g4">${ic(TRIG[p.trig][1], "s14 faint")}${TRIG[p.trig][0]}</dd>${p.tk ? `<dt>Tiket</dt><dd>${tref(p.tk)}</dd>` : ""}<dt>Oleh</dt><dd class="row g4">${av("writer", 16)}Tech Writer</dd><dt>Waktu</dt><dd>${p.w}</dd><dt>Biaya</dt><dd>${p.tok}</dd><dt>Perubahan</dt><dd>${pm(p)}</dd></dl>
        <div class="okb-kbd"><span><kbd>A</kbd>Approve</span><span><kbd>R</kbd>Tolak</span><span><kbd>J</kbd><kbd>K</kbd>Pindah usulan</span></div>
      </aside>
      <div class="box okb-sbs"><div class="okb-sbs-h"><div>${ic("file-text", "s14")}Sebelum</div><div>${ic("sparkles", "s14")}Sesudah (usulan AI)</div></div>${sbsRows(p.diff)}</div>
    </div>
    <div class="okb-rvfoot"><span class="msg sm">${ic("git-commit", "s14")}Approve menulis commit ke .agentgate/knowledge/</span>${btn("Tolak", "", "x", { "data-dialog": "ok-reject", "data-arg": p.id })}${btn("Edit dulu", "", "pencil", { "data-go": `#/p/${K}/knowledge/${d.id}` })}${btn(list.length > 1 ? "Approve dan lanjut" : "Approve", "pri", "check", { "data-act": "ok-approve", "data-arg": p.id })}</div>
  </div>`;
}
function kB() {
  const v = KS.b.view;
  const inner = v === "review" ? kBReview() : v === "group" ? kBGroup() : v === "doc" ? kBDoc() : kBHome();
  return `<div class="ok-top"><span class="ok-crumbs">${AG.projDot(K)}Kasir Online<span class="faint">/</span><b>Knowledge</b></span></div>${inner}`;
}
document.addEventListener("keydown", (e) => {
  if (AG.route.path !== "/opsi/knowledge" || KS.opt !== "B" || KS.b.view !== "review" || e.ctrlKey || e.metaKey) return;
  if (/INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName) || document.getElementById("overlay")?.children.length) return;
  const list = pending(); const p = list[Math.min(KS.b.ri, list.length - 1)]; if (!p) return;
  const k = e.key.toLowerCase();
  if (k === "a") AG.ACTIONS["ok-approve"](null, p.id);
  else if (k === "r") AG.openDialog("ok-reject", p.id);
  else if (k === "j" || k === "k") AG.ACTIONS["okb-go"](null, `n:${k === "j" ? 1 : -1}`);
});

/* ====================== Knowledge C: daftar + panel ====================== */
AG.action("okc-v", (el, v) => { KS.c.v = v; rr(); });
AG.action("okc-tab", (el, t) => { KS.c.tab = t; rr(["okc-pb"]); });
AG.action("okc-sel", (el, id) => { KS.c.sel = id || null; rr(["okc-pb"]); });
AG.action("okc-bulk", (el, arg) => {
  const [m, id] = arg.split(":"); const d = AG._dts[id];
  const ids = [...d.sel].map((i) => (d.rows[i].cells[0].match(/data-okc="([^"]+)"/) || [])[1]).filter(Boolean); d.sel.clear();
  if (m === "ap") { let n = 0; ids.forEach((x) => pendOf(x).forEach((p) => { settle(p, "approved"); n++; })); AG.toast(n ? `${n} usulan disetujui` : "Dokumen terpilih tidak punya usulan", n ? "" : "dng"); }
  else { ids.forEach((x) => { if (doc(x).st === "stale") staleProp(x); }); AG.toast(`${ids.length} dokumen dikirim ke Agent Tech Writer untuk diperbarui`); }
  rr();
});
const dnum = (s) => { const m = /(\d+) (Okt|Nov)/.exec(s); return m ? (m[2] === "Nov" ? 1100 : 1000) + +m[1] : 9999; };
function kC() {
  const s = KS.c, stale = DOCS.filter((d) => d.st === "stale");
  const views = [["all", "Semua dokumen", DOCS.length], ["prop", "Ada usulan", DOCS.filter((d) => pendOf(d.id).length).length], ["stale", "Basi", stale.length]];
  const id = "okc-" + s.v;
  const rows = DOCS.filter((d) => s.v === "all" || (s.v === "prop" ? pendOf(d.id).length : d.st === s.v)).map((d) => ({
    cells: [`<span class="okc-n" data-okc="${d.id}">${ic("file-text", "s14 faint")}<span><b class="b5">${d.n}</b><span class="okc-p">${d.st === "stale" ? `<span class="text-warn">${d.why}</span>` : `${d.g} · ${d.id}.md`}</span></span></span>`,
      stPill(d.st), `<span class="row g4">${av(d.own, 18)}${pname(d.own).split(" ")[0]}</span>`, `<span class="mut">${d.up}</span>`,
      pendOf(d.id).length ? pill(String(pendOf(d.id).length), "acc", "", "git-pull-request") : `<span class="faint">—</span>`],
    sort: [d.n, d.st, d.own, dnum(d.up), pendOf(d.id).length], text: `${d.n} ${d.g} ${d.id}`, filter: { st: ST[d.st][0], g: d.g, own: pname(d.own) } }));
  const table = AG.datatable({ id, columns: [{ label: "Dokumen" }, { label: "Status", w: "120px" }, { label: "Pemilik", w: "100px", hide: "sm" }, { label: "Diperbarui", w: "100px", hide: "sm" }, { label: "Usulan", w: "80px" }], rows,
    search: "Cari dokumen…", pageSize: 10, select: true,
    filters: [{ k: "st", label: "Status", icon: "circle-half-2", options: ["Terbaru", "Basi", "Ada usulan"] }, { k: "g", label: "Kelompok", icon: "folders", options: GROUPS.map((g) => g.g) }, { k: "own", label: "Pemilik", icon: "user", options: ["AA", "OO", "DD", "QQ"].map(pname) }],
    bulk: `${btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "okc-bulk", "data-arg": "ai:" + id })}${btn("Approve usulan", "sm pri", "check", { "data-act": "okc-bulk", "data-arg": "ap:" + id })}` });
  const d = s.sel ? doc(s.sel) : null;
  let panel;
  if (!d) panel = `<aside class="okc-panel">${AG.empty("layout-sidebar", "Pilih dokumen", "Klik baris untuk melihat pratinjau dan usulan perubahan di sini.")}</aside>`;
  else {
    const ps = pendOf(d.id), past = PR.filter((x) => x.doc === d.id && x.st !== "pending");
    let body = "", foot = "";
    if (s.tab === "diff") {
      body = ps.length ? ps.map((p) => `<div class="okc-prop"><div class="h"><span class="row g8">${av("writer", 18)}<b class="b5 grow">Agent Tech Writer</b>${pm(p)}</span><span class="mut sm">${trigTxt(p)} · ${p.w} · ${p.tok}</span><span class="sm">${p.why}</span></div>${unified(p)}</div>`).join("")
        : `${AG.empty("git-pull-request", "Tidak ada usulan menunggu", "Usulan baru muncul setelah tiket Done, merge, atau audit mingguan.")}${past.map((p) => `<div class="row sm">${pill(p.st === "approved" ? "Disetujui" : "Ditolak", p.st === "approved" ? "ok" : "dng")}<span class="mut grow">${trigTxt(p)} · ${p.w}</span>${pm(p)}</div>`).join("")}`;
      if (ps.length) foot = `<div class="okc-pf"><span class="mut sm grow">Menunggu ${pname(OWNER).split(" ")[0]}</span>${btn("Tolak", "sm", "x", { "data-dialog": "ok-reject", "data-arg": ps[0].id })}${btn("Approve", "sm pri", "check", { "data-act": "ok-approve", "data-arg": ps[0].id })}</div>`;
    } else if (s.tab === "info") {
      body = `<dl class="oka-props"><dt>Status</dt><dd>${stPill(d.st)}</dd><dt>Kelompok</dt><dd>${d.g}</dd><dt>Pemilik</dt><dd class="row g4">${av(d.own, 18)}${pname(d.own)}</dd><dt>Diperbarui</dt><dd>${d.up}</dd><dt>Sumber</dt><dd class="mono xs trunc">.agentgate/knowledge/${d.id}.md</dd></dl><div class="col g4"><b class="sm b5">Terhubung</b>${relHtml(d)}</div>${btn("Riwayat versi", "sm", "history", { "data-dialog": "kb-history" })}`;
    } else {
      body = `${d.st === "stale" ? `<div class="alert warn">${ic("clock", "s14")}<span class="grow">${d.why}.</span></div>` : ""}<div class="ok-prose sm">${docBody(d)}</div>`;
      foot = `<div class="okc-pf">${d.st === "stale" ? btn("Minta AI perbarui", "sm", "sparkles", { "data-act": "ok-stale-ai", "data-arg": d.id }) : ""}${btn("Buka penuh", "sm", "arrows-maximize", { "data-go": `#/p/${K}/knowledge/${d.id}` })}</div>`;
    }
    panel = `<aside class="okc-panel" aria-label="Pratinjau dokumen"><div class="okc-ph"><div class="row top"><span class="tile">${ic("file-text", "s14")}</span><div class="grow" style="min-width:0"><b class="md b6 trunc" style="display:block">${d.n}</b><span class="mut sm">${d.g} · ${stDot(d.st)} ${ST[d.st][0]}</span></div>${btn("", "ghost icon sm", "pencil", { "data-go": `#/p/${K}/knowledge/${d.id}`, "aria-label": "Edit dokumen" })}${btn("", "ghost icon sm", "x", { "data-act": "okc-sel", "data-arg": "", "aria-label": "Tutup pratinjau" })}</div>
      <nav class="tabs" aria-label="Panel dokumen">${[["pre", "Pratinjau"], ["diff", `Usulan${ps.length ? ` <span class="n">${ps.length}</span>` : ""}`], ["info", "Info"]].map(([k, l]) => `<button class="${s.tab === k ? "on" : ""}" data-act="okc-tab" data-arg="${k}">${l}</button>`).join("")}</nav></div>
      <div class="okc-pb" data-ks="okc-pb">${body}</div>${foot}</aside>`;
  }
  return `<div class="ok-top"><span class="ok-crumbs">${AG.projDot(K)}Kasir Online<span class="faint">/</span><b>Knowledge</b></span></div>
    <style>.okc-wrap tr:has([data-okc="${esc(s.sel || "-")}"]) td { background: var(--accent-soft); }</style>
    <div class="okc"><div class="okc-main" data-ks="okc-main">
      <div class="okb-head"><div class="grow"><h1 style="font-size:20px">Knowledge</h1><p class="mut">Semua dokumen Kasir Online dalam satu daftar. Klik baris untuk pratinjau dan meninjau usulan.</p></div><div class="row">${btn("Dokumen baru", "", "plus", { "data-dialog": "kb-new" })}${btn("Scan ulang", "pri", "refresh", { "data-dialog": "kb-scan" })}</div></div>
      <nav class="tabs" aria-label="Tampilan daftar">${views.map(([k, l, n]) => `<button class="${s.v === k ? "on" : ""}" data-act="okc-v" data-arg="${k}">${l}<span class="n">${n}</span></button>`).join("")}</nav>
      <div class="okc-wrap">${table}</div>
    </div>${panel}</div>`;
}

/* ====================== Layar opsi knowledge ====================== */
const KOPT = {
  A: { n: "Dokumentasi tiga panel", d: "Seperti Notion atau Confluence: pohon dokumen per kelompok di kiri, isi dokumen di tengah, outline, status, pemilik, dan usulan di kanan. Usulan AI ditinjau sebagai mode saran langsung di dalam dokumen.", plus: ["Paling familiar untuk membaca dan menjelajah dokumen", "Usulan dibaca dalam konteks dokumen aslinya"], minus: ["Ringkasan kesehatan knowledge kurang menonjol", "Butuh layar lebar; di ponsel panel ditumpuk"], try: "Coba: klik banner ungu “Tinjau”, lalu Approve. Atau buka “Usulan menunggu”." },
  B: { n: "Beranda dan kartu kelompok", d: "Halaman depan menunjukkan kesehatan dokumen dan antrean tindakan (usulan dan dokumen basi), lalu grid kartu kelompok. Usulan ditinjau satu per satu dalam mode fokus berdampingan: sebelum dan sesudah.", plus: ["Pemilik knowledge langsung tahu apa yang harus dikerjakan", "Review cepat: approve dan lanjut, pintasan A, R, J, K"], minus: ["Butuh satu klik lagi untuk sampai ke dokumen", "Kurang cocok untuk membaca panjang"], try: "Coba: klik “Tinjau 3 usulan”, lalu tekan A di keyboard." },
  C: { n: "Daftar dan panel pratinjau", d: "Seperti Linear atau Jira: tabel dokumen dengan filter status, kelompok, dan pemilik, plus aksi massal. Klik baris membuka pratinjau dan diff usulan di panel kanan tanpa pindah halaman.", plus: ["Paling cepat untuk mencari, menyaring, dan aksi massal", "Konsisten dengan halaman daftar tiket"], minus: ["Terasa seperti tabel data, kurang nyaman untuk membaca", "Struktur kelompok kurang terlihat"], try: "Coba: klik baris lain, pakai filter Status, atau centang beberapa baris." },
};
AG.screen({ m: "Opsi", g: "Opsi desain", n: "Opsi knowledge", route: "/opsi/knowledge",
  render: () => {
    const o = KS.opt;
    const body = `${AG.head("Opsi tata letak knowledge", "Tiga usulan pengganti katalog dan halaman usulan perubahan knowledge yang sekarang. Setiap pratinjau interaktif dan memakai data dokumen Kasir Online yang sama, termasuk cara meninjau usulan (diff) dari AI.", btn("Lihat versi sekarang", "ghost", "external-link", { "data-go": `#/p/${K}/knowledge` }))}
      ${optTabs(o, "ok-opt", KOPT)}
      ${optInfo(KOPT[o], o, KS.pick, "ok-pick")}
      ${frame(`app.agentgate.id/p/KSR/knowledge${o === "B" && KS.b.view === "review" ? "/tinjau" : ""}`, o === "A" ? kA() : o === "B" ? kB() : kC())}
      <div class="row mut sm">${ic("info-circle", "s14")}<span class="grow">Approve dan tolak di pratinjau mengubah data contoh bersama untuk ketiga opsi.</span><button class="link" data-act="ok-reset">Kembalikan data contoh</button></div>`;
    return AG.shell({ crumbs: ["Opsi desain", "Knowledge"], body });
  },
  after: (root) => {
    const q = root.querySelector("[data-oka-q]");
    const apply = () => {
      const v = q.value.toLowerCase().trim();
      root.querySelectorAll(".oka-g").forEach((g) => {
        let any = false;
        g.querySelectorAll(".oka-d").forEach((d) => { const ok = !v || d.dataset.n.includes(v); d.style.display = ok ? "" : "none"; any ||= ok; });
        g.style.display = !v || any ? "" : "none";
      });
    };
    if (q) {
      if (KS.a.q) { q.value = KS.a.q; apply(); }
      q.addEventListener("input", () => {
        KS.a.q = q.value;
        if (q.value && KS.a.closed.size) { KS.a.closed.clear(); rr(); const n = document.querySelector("[data-oka-q]"); n?.focus(); n?.setSelectionRange(n.value.length, n.value.length); return; }
        apply();
      });
    }
    root.querySelector(".okc-wrap")?.addEventListener("click", (e) => {
      const tr = e.target.closest("tbody tr"); if (!tr || e.target.closest("button,a")) return;
      const m = tr.querySelector("[data-okc]"); if (!m) return;
      KS.c.sel = m.dataset.okc; KS.c.tab = pendOf(KS.c.sel).length ? "diff" : "pre"; rr(["okc-pb"]);
    });
    root.querySelectorAll("[role=button][data-act]").forEach((el) => el.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); el.click(); } }));
  } });

/* ====================== AI Office ====================== */
AG.css(`
.oo-i { fill: none; stroke: currentColor; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; }
.oo-arm { fill: none; stroke-width: 4.5; stroke-linecap: round; }
.oo-head { fill: var(--raised); }
.oo-badge { fill: none; stroke: #fff; stroke-width: 2.6; stroke-linecap: round; stroke-linejoin: round; }
.oo-ch { cursor: pointer; outline: none; }
.oo-ch .ring { fill: none; stroke: var(--accent); stroke-width: 2.5; opacity: 0; }
.oo-ch:hover .ring { opacity: 0.45; } .oo-ch.sel .ring, .oo-ch:focus-visible .ring { opacity: 1; }
.oo-tag { cursor: pointer; }
.oo-tag .nm { fill: var(--raised); stroke: var(--line2); }
.oo-tag.sel .nm { stroke: var(--accent); stroke-width: 2; }
.oo-tag text { fill: var(--fg); font-size: 11.5px; font-weight: 500; font-family: var(--font); }
.oo-chip text { font-size: 11px; font-weight: 600; font-family: var(--mono); }
.oo-chip.warn > rect { fill: var(--warn-soft); stroke: color-mix(in srgb, var(--warn) 40%, transparent); } .oo-chip.warn { color: var(--warn); } .oo-chip.warn text { fill: var(--warn); }
.oo-chip.dng > rect { fill: var(--danger); } .oo-chip.dng { color: #fff; } .oo-chip.dng text { fill: #fff; }
.oo-chip.ok > rect { fill: var(--success-soft); } .oo-chip.ok { color: var(--success); } .oo-chip.ok text { fill: var(--success); }
.oo-chip.acc > rect { fill: var(--accent-soft); } .oo-chip.acc { color: var(--accent-fg); } .oo-chip.acc circle { fill: var(--accent-fg); }
.oo-chip.info > rect { fill: var(--info-soft); } .oo-chip.info { color: var(--info); }
.oo-chip.gray > rect { fill: var(--hover); } .oo-chip.gray { color: var(--muted); }
.oo-sign rect.bg { fill: var(--raised); stroke: var(--line2); }
.oo-sign text { fill: var(--fg); font-size: 13px; font-weight: 600; font-family: var(--font); } .oo-sign text.s { fill: var(--muted); font-size: 11px; font-weight: 400; }
@keyframes oo-bob { 50% { transform: translateY(-1.6px); } } .oo-bob { animation: oo-bob 1.8s ease-in-out infinite; }
@keyframes oo-type { 50% { transform: translateY(1.8px); } } .oo-type { animation: oo-type 0.36s ease-in-out infinite; } .oo-type.b { animation-delay: 0.18s; }
@keyframes oo-blink { 0%, 93%, 100% { transform: scaleY(1); } 96% { transform: scaleY(0.1); } } .oo-eye { transform-box: fill-box; transform-origin: center; animation: oo-blink 4.6s infinite; }
@keyframes oo-pulse { 0% { transform: scale(1); opacity: 0.5; } 100% { transform: scale(1.35, 1.6); opacity: 0; } } .oo-pulse { transform-box: fill-box; transform-origin: center; animation: oo-pulse 1.6s ease-out infinite; }
@keyframes oo-float { 50% { transform: translateY(-3px); } } .oo-float { animation: oo-float 2.6s ease-in-out infinite; }
@keyframes oo-dots { 0%, 60%, 100% { opacity: 0.25; } 30% { opacity: 1; } } .oo-dt { animation: oo-dots 1.2s infinite; } .oo-dt.d2 { animation-delay: 0.2s; } .oo-dt.d3 { animation-delay: 0.4s; }
@keyframes oo-walk { 0%, 6% { transform: translateX(0); } 46%, 54% { transform: translateX(430px); } 94%, 100% { transform: translateX(0); } } .oob-walk { animation: oo-walk 30s ease-in-out infinite; }
@keyframes oo-wander { 0%, 100% { transform: translate(0, 0); } 30% { transform: translate(24px, 6px); } 62% { transform: translate(-12px, 12px); } } .oob-wander { animation: oo-wander 16s ease-in-out infinite; }
@keyframes oo-step { 50% { transform: translateY(-2px); } } .oob-step { animation: oo-step 0.5s ease-in-out infinite; }
@media (prefers-reduced-motion: reduce) { .oo-bob, .oo-type, .oo-eye, .oo-pulse, .oo-float, .oo-dt, .oob-walk, .oob-wander, .oob-step { animation: none !important; } }
.oo-sum { display: inline-flex; gap: 14px; align-items: center; background: var(--raised); border: 1px solid var(--line2); border-radius: 9px; padding: 6px 12px; box-shadow: var(--shadow-sm); font-size: 12px; color: var(--muted); flex-wrap: wrap; }
.oo-sum b { color: var(--fg); font-weight: 600; font-variant-numeric: tabular-nums; }
.oo-leg { display: flex; gap: 12px; font-size: 12px; color: var(--muted); flex-wrap: wrap; } .oo-leg span { display: inline-flex; align-items: center; gap: 5px; } .oo-leg i { width: 8px; height: 8px; border-radius: 50%; }
.oo-zoom { display: inline-flex; border: 1px solid var(--line2); border-radius: 7px; background: var(--raised); overflow: hidden; box-shadow: var(--shadow-sm); }
.oo-zoom button { width: 30px; height: 28px; display: inline-flex; align-items: center; justify-content: center; color: var(--fg2); } .oo-zoom button:hover { background: var(--hover); } .oo-zoom button + button { border-left: 1px solid var(--line); }
.oo-h { font-size: 12px; color: var(--muted); font-weight: 500; display: flex; align-items: center; gap: 6px; margin-bottom: 8px; }
.oo-todo { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--raised); width: 100%; text-align: left; margin-bottom: 6px; }
.oo-todo:hover { border-color: var(--input); box-shadow: var(--shadow-sm); }
.oo-room { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 6px; width: 100%; text-align: left; font-size: 12.5px; }
.oo-room:hover { background: var(--hover); } .oo-room.on { background: var(--accent-soft); color: var(--accent-fg); }
.oo-det { display: grid; gap: 14px; align-content: start; }
.oo-dh { display: flex; gap: 10px; align-items: flex-start; }
.oo-log { max-height: 210px; overflow-y: auto; font-size: 11.5px; padding: 10px 12px; } .oo-log .new { animation: fade 0.3s ease; }
.oo-acts { display: flex; gap: 8px; flex-wrap: wrap; }
.oo-props { grid-template-columns: 76px minmax(0, 1fr); gap: 8px 10px; font-size: 12.5px; }
.oo-jobs { display: flex; gap: 6px; flex-wrap: wrap; } .oo-jobs > a { padding: 2px 8px; border: 1px solid var(--line2); border-radius: 6px; }

/* A: isometrik */
.ooa { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr) 300px; }
.ooa-stage { position: relative; overflow: auto; display: flex; flex-direction: column; background: radial-gradient(ellipse at 50% 40%, var(--raised) 0%, var(--page) 72%); min-width: 0; }
.ooa-tb { display: flex; align-items: center; gap: 10px; padding: 12px 16px 0; flex-wrap: wrap; position: sticky; left: 0; }
.ooa-svg { display: block; width: 100%; height: auto; margin: auto; min-width: 680px; max-width: 1000px; padding: 4px 12px 12px; box-sizing: border-box; }
.ooa-stage.z2 .ooa-svg { max-width: none; width: 160%; min-width: 1100px; }
.ooa-floor .t { fill: color-mix(in srgb, var(--rc) 8%, var(--raised)); } .ooa-floor .l { fill: color-mix(in srgb, var(--rc) 22%, var(--line2)); } .ooa-floor .r { fill: color-mix(in srgb, var(--rc) 14%, var(--line)); }
.ooa-wall .t { fill: color-mix(in srgb, var(--rc) 45%, var(--raised)); } .ooa-wall .l { fill: color-mix(in srgb, var(--rc) 16%, var(--raised)); } .ooa-wall .r { fill: color-mix(in srgb, var(--rc) 22%, var(--raised)); }
.ooa-desk .t { fill: var(--raised); stroke: var(--line2); stroke-width: 0.6; } .ooa-desk .l { fill: var(--line2); } .ooa-desk .r { fill: var(--active); }
.ooa-dark .t { fill: #454552; } .ooa-dark .l { fill: #2c2c35; } .ooa-dark .r { fill: #383843; }
.ooa-wood .t { fill: color-mix(in srgb, #b7793a 35%, var(--raised)); } .ooa-wood .l { fill: color-mix(in srgb, #b7793a 55%, var(--line2)); } .ooa-wood .r { fill: color-mix(in srgb, #b7793a 45%, var(--line)); }
.ooa-sofa .t { fill: color-mix(in srgb, #0891b2 45%, var(--raised)); } .ooa-sofa .l { fill: color-mix(in srgb, #0891b2 60%, var(--line2)); } .ooa-sofa .r { fill: color-mix(in srgb, #0891b2 50%, var(--line)); }
.ooa-pot .t { fill: #c8a27a; } .ooa-pot .l { fill: #a77e57; } .ooa-pot .r { fill: #b88f66; }
.ooa-side { border-left: 1px solid var(--line); background: var(--bg); overflow-y: auto; padding: 16px; display: grid; gap: 18px; align-content: start; min-width: 0; }
@container (max-width: 900px) { .ooa { grid-template-columns: 1fr; } .ooa-side { border-left: 0; border-top: 1px solid var(--line); } .ooa-stage { min-height: 420px; } }

/* B: denah 2D */
.oob { flex: 1; min-height: 0; display: flex; flex-direction: column; position: relative; }
.oob-tb { display: flex; align-items: center; gap: 10px; padding: 10px 16px; background: var(--bg); border-bottom: 1px solid var(--line); flex-wrap: wrap; }
.oob-stage { flex: 1; position: relative; overflow: auto; background: var(--page); min-height: 0; }
.oob-svg { display: block; width: 100%; height: 100%; min-width: 820px; min-height: 520px; }
.oob-stage.z2 .oob-svg { width: 150%; height: 150%; min-width: 1240px; }
.oob-cor { fill: color-mix(in srgb, var(--fg) 4%, var(--raised)); }
.oob-wall { fill: none; stroke: color-mix(in srgb, var(--fg) 55%, var(--raised)); stroke-width: 6; }
.oob-door { fill: color-mix(in srgb, var(--fg) 4%, var(--raised)); }
.oob-desk { fill: var(--raised); stroke: color-mix(in srgb, var(--fg) 22%, var(--raised)); stroke-width: 1.2; }
.oob-chair { fill: color-mix(in srgb, var(--fg) 22%, var(--raised)); }
.oob-mon { fill: #34343f; } .oob-mon.on { fill: #3b3b9a; }
.oob-wood { fill: color-mix(in srgb, #b7793a 32%, var(--raised)); stroke: color-mix(in srgb, #b7793a 55%, var(--raised)); stroke-width: 1.5; }
.oob-counter { fill: color-mix(in srgb, var(--fg) 10%, var(--raised)); stroke: color-mix(in srgb, var(--fg) 22%, var(--raised)); }
.oob-sofa { fill: color-mix(in srgb, #0891b2 40%, var(--raised)); }
.oob-rug { fill: color-mix(in srgb, #a16207 14%, var(--raised)); }
.oob-leaf { fill: #3f9a5a; } .oob-leaf2 { fill: #2f7d47; }
.oob-label rect.bg { fill: var(--raised); stroke: var(--line2); } .oob-label text { fill: var(--fg); font-size: 13px; font-weight: 600; font-family: var(--font); } .oob-label text.s { fill: var(--muted); font-size: 11px; font-weight: 400; }
.oob-bub > rect, .oob-bub > path { fill: var(--raised); stroke: var(--line2); } .oob-bub text { fill: var(--fg2); font-size: 10.5px; font-weight: 500; font-family: var(--font); }
.oob-bub.warn > rect, .oob-bub.warn > path { fill: var(--warn-soft); stroke: color-mix(in srgb, var(--warn) 45%, transparent); } .oob-bub.warn text { fill: var(--warn); } .oob-bub.warn { color: var(--warn); }
.oob-bub.dng > rect, .oob-bub.dng > path { fill: var(--danger); stroke: var(--danger); } .oob-bub.dng text { fill: #fff; } .oob-bub.dng { color: #fff; }
.oob-bub.ok > rect, .oob-bub.ok > path { fill: var(--success-soft); stroke: transparent; } .oob-bub.ok text { fill: var(--success); } .oob-bub.ok { color: var(--success); }
.oob-bub.acc { color: var(--accent-fg); } .oob-bub.info { color: var(--info); } .oob-bub.gray { color: var(--muted); }
.oob-name > rect { fill: color-mix(in srgb, var(--fg) 82%, transparent); } .oob-name text { fill: var(--bg); font-size: 10.5px; font-weight: 500; font-family: var(--font); }
.oob-stage.nobub .oob-bub { display: none; }
.oob-pop { position: absolute; top: 64px; right: 12px; bottom: 12px; width: 350px; background: var(--raised); border: 1px solid var(--line2); border-radius: 12px; box-shadow: var(--shadow-lg); padding: 16px; overflow-y: auto; display: none; animation: pop 0.16s ease; z-index: 3; }
.oob-pop.open { display: block; }
.oob-hint { position: absolute; left: 12px; bottom: 12px; font-size: 12px; color: var(--muted); background: var(--raised); border: 1px solid var(--line2); border-radius: 8px; padding: 6px 10px; display: flex; gap: 6px; align-items: center; box-shadow: var(--shadow-sm); }
@container (max-width: 760px) { .oob-stage { flex: none; height: 460px; } .oob-pop { position: static; width: auto; margin: 12px; box-shadow: none; max-height: none; } .oob-hint { display: none; } }

/* C: papan tim */
.ooc { flex: 1; min-height: 0; position: relative; display: flex; }
.ooc-scroll { flex: 1; overflow-y: auto; padding: 20px 24px 28px; display: grid; gap: 16px; align-content: start; min-width: 0; }
.ooc-cols { display: grid; grid-template-columns: minmax(0, 1fr) 280px; gap: 20px; align-items: start; }
.ooc-main { display: grid; gap: 22px; min-width: 0; }
.ooc-grp { display: grid; gap: 10px; }
.ooc-gh { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; } .ooc-gh h3 { font-size: 14px; }
.ooc-cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(232px, 1fr)); gap: 10px; }
.ooc-card { display: grid; gap: 10px; padding: 12px 12px 12px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--raised); cursor: pointer; position: relative; align-content: start; min-width: 0; }
.ooc-card:hover { border-color: var(--input); box-shadow: var(--shadow-sm); } .ooc-card.sel { border-color: var(--accent); box-shadow: var(--ring); }
.ooc-card::before { content: ""; position: absolute; left: -1px; top: 12px; bottom: 12px; width: 3px; border-radius: 0 3px 3px 0; background: var(--sc); }
.ooc-tk { display: flex; gap: 6px; align-items: baseline; min-width: 0; font-size: 12.5px; } .ooc-tk .t { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.ooc-step { font-size: 12px; color: var(--muted); display: grid; gap: 6px; min-width: 0; }
.ooc-step .bar { height: 4px; } .ooc-step .bar.warn > i { background: var(--warn); } .ooc-step .bar.dng > i { background: var(--danger); } .ooc-step .bar.ok > i { background: var(--success); }
.ooc-empty { display: flex; align-items: center; gap: 10px; padding: 12px 14px; border: 1px dashed var(--input); border-radius: 10px; color: var(--muted); flex-wrap: wrap; }
.ooc-aside { display: grid; gap: 12px; position: sticky; top: 0; }
.ooc-map svg { width: 100%; height: auto; display: block; }
.ooc-map .rm { stroke: var(--line2); stroke-width: 4; } .ooc-map text { fill: var(--muted); font-size: 28px; font-weight: 500; font-family: var(--font); }
.ooc-map .dt { cursor: pointer; stroke: var(--raised); stroke-width: 5; } .ooc-map .dt.sel { stroke: var(--fg); stroke-width: 7; }
.ooc-over { position: absolute; top: 0; right: 0; bottom: 0; width: 380px; max-width: 100%; background: var(--raised); border-left: 1px solid var(--line2); box-shadow: var(--shadow-lg); padding: 16px; overflow-y: auto; transform: translateX(100%); visibility: hidden; transition: transform 0.2s ease, visibility 0.2s; z-index: 4; }
.ooc-over.open { transform: none; visibility: visible; }
@container (max-width: 980px) { .ooc-cols { grid-template-columns: 1fr; } .ooc-aside { position: static; } }
@container (max-width: 560px) { .ooc-scroll { padding: 16px; } }
`);

/* ---------- Data kantor ---------- */
const OCH0 = [
  { id: "dev", p: "dev", room: "KSR", tk: "KSR-26", st: "jalan", runner: "Laptop Dewi", since: "08.42", step: 4, steps: 7, doing: "Menulis tes stok minus" },
  { id: "dewi", who: "DD", room: "KSR", st: "pemilik", runner: "Laptop Dewi", jobs: ["KSR-26", "KSR-24", "KSR-28"], doing: "Mengawasi 3 job di runner-nya" },
  { id: "qa", p: "qa", room: "KSR", tk: "KSR-29", st: "jalan", runner: "PC Qori", since: "09.10", step: 14, steps: 18, doing: "Playwright skenario 14 dari 18" },
  { id: "qori", who: "QQ", room: "KSR", st: "pemilik", runner: "PC Qori", jobs: ["KSR-29", "KSR-30"], doing: "Mengawasi 2 job di runner-nya" },
  { id: "ux", p: "uiux", room: "KSR", tk: "KSR-28", st: "review", runner: "Laptop Dewi", since: "kemarin", step: 5, steps: 5, doing: "Mockup v2 siap direview" },
  { id: "rev", p: "reviewer", room: "KSR", tk: "KSR-24", st: "review", runner: "Laptop Dewi", since: "10.05", step: 4, steps: 4, doing: "Review AI selesai · 3 catatan" },
  { id: "pm", p: "pm", room: "KSR", tk: "KSR-35", st: "review", runner: "MacBook Olivia", since: "07.30", step: 3, steps: 3, doing: "PRD v1 siap direview" },
  { id: "qa2", p: "qa", room: "KSR", tk: "KSR-30", st: "keputusan", runner: "PC Qori", since: "09.55", step: 3, steps: 3, doing: "Gagal 3 kali · total struk selisih" },
  { id: "an", p: "analyst", room: "MEET", tk: "KSR-31", st: "rapat", runner: "MacBook Olivia", since: "10.20", doing: "Brainstorming laporan harian" },
  { id: "oli", who: "OO", room: "MEET", st: "rapat", runner: "MacBook Olivia", jobs: ["KSR-31", "KSR-35"], doing: "Brainstorming bersama Agent Analyst" },
  { id: "arch", p: "architect", room: "PANTRY", st: "idle", doing: "Menunggu tiket" },
  { id: "ops", p: "devops", room: "PANTRY", st: "idle", doing: "Menunggu tiket" },
  { id: "tw", p: "writer", room: "PANTRY", st: "idle", doing: "Menunggu tiket" },
];
let OCH = JSON.parse(JSON.stringify(OCH0));
const ochar = (id) => OCH.find((c) => c.id === id);
const OROOMS = [{ k: "KSR", n: "Kasir Online", c: "#7c3aed" }, { k: "MEET", n: "Ruang rapat", c: "#0891b2" }, { k: "GDG", n: "Aplikasi Gudang", c: "#0ea5e9" }, { k: "PPL", n: "Portal Pelanggan", c: "#f59e0b" }, { k: "PANTRY", n: "Pantry", c: "#a16207" }];
const OST = { jalan: ["Bekerja", "acc", "var(--accent)"], review: ["Butuh approval", "warn", "var(--warn)"], keputusan: ["Butuh keputusan", "dng", "var(--danger)"], rapat: ["Brainstorming", "info", "var(--info)"], idle: ["Menganggur", "gray", "var(--faint)"], pemilik: ["Online", "ok", "var(--success)"], ok: ["Disetujui", "ok", "var(--success)"], jeda: ["Dijeda", "gray", "var(--muted)"] };
const OLOGS = {
  dev: ["Membaca ringkasan PRD + AC KSR-26", "ag_search \"validasi stok checkout\" → 6 simbol", "ag_trace checkStock → 3 pemanggil", "Menulis tes: stok_kurang_menolak_checkout", "rtk: go test ./internal/stock/... (1,2 rb → 140 token)", "✓ 14 tes lulus · 1 gagal: stok minus", "Memperbaiki guard qty <= 0", "✓ 15 tes lulus", "Commit: feat(stock): tolak checkout bila stok kurang"],
  qa: ["Membuka preview build PR #146", "Playwright: skenario 13/18 · cetak ulang dari riwayat", "Screenshot: riwayat-struk-mobile.png", "Membandingkan dengan mockup v1", "✓ skenario 14/18 lulus", "Playwright: skenario 15/18 · printer offline", "✓ skenario 15/18 lulus"],
  qa2: ["✗ Percobaan 3: total struk selisih Rp100", "Struk: nama kasir kosong pada transaksi QRIS", "Gate “Hasil test lulus” tidak terpenuhi", "Menunggu keputusan manusia"],
  rev: ["Review PR #142 selesai", "3 catatan: 1 penting, 2 saran", "CI hijau · 212 tes", "Menunggu approval Code Review"],
  ux: ["Mockup v2 diunggah ke preview sandbox", "Screenshot web + mobile dibuat otomatis", "Menunggu review mockup"],
  pm: ["PRD v1 KSR-35 selesai · 7 acceptance criteria", "Menunggu review PRD"],
  an: ["Sesi brainstorming bersama Olivia", "Catatan: laporan dikirim jam 21.00", "Pertanyaan: format PDF atau ringkasan email?"],
  gen: ["Membaca tiket dan acceptance criteria", "ag_search konteks terkait → 4 simbol", "Menyusun rencana kerja 6 langkah", "Langkah 1: membaca kode terkait", "Langkah 2: menulis draf"],
};
const logKey = (c) => ({ dewi: "dev", qori: "qa", oli: "an" })[c.id] || (OLOGS[c.id] ? c.id : "gen");
const oname = (c) => (c.who ? pname(c.who) : "Agent " + AG.PERSONAS[c.p].n);
const oshort = (c) => (c.who ? pname(c.who).split(" ")[0] : { analyst: "Analyst", pm: "PM", uiux: "UI/UX", architect: "Architect", dev: "Developer", reviewer: "Reviewer", qa: "QA", devops: "DevOps", writer: "Writer" }[c.p]);
const ocol = (c) => (c.who ? AG.PEOPLE[c.who].c : AG.PERSONAS[c.p].c);
const OS = { opt: "A", pick: "A", sel: null, room: null, bub: true };
const ocount = (st) => OCH.filter((c) => c.st === st && c.p).length;
const todo = () => OCH.filter((c) => c.st === "review" || c.st === "keputusan");

/* ---------- Karakter (dipakai A dan B) ---------- */
function fig(c, S = 1) {
  const col = ocol(c), st = c.st, human = !!c.who, sc = OST[st][2];
  const skin = { DD: "#c68a5e", QQ: "#e0ac83", OO: "#f1c7a1" }[c.who] || "#e8b996";
  const hair = { DD: "#2a1d17", QQ: "#4a2f22", OO: "#5b3a29" }[c.who] || "#222";
  const mood = st === "keputusan" ? "sad" : st === "ok" || st === "rapat" ? "happy" : st === "idle" || st === "jeda" ? "sleepy" : "ok";
  const arm = (d) => `<path d="${d}" class="oo-arm" stroke="${col}"/>`;
  const arms = st === "review" ? `${arm("M-9 -20 Q-12 -12 -10 -6")}${arm("M9 -21 Q17 -28 15 -39")}<circle cx="15" cy="-41" r="3.4" fill="${human ? skin : col}"/>`
    : st === "jalan" ? `<g class="oo-type">${arm("M-9 -20 Q-13 -12 -6 -8")}</g><g class="oo-type b">${arm("M9 -20 Q13 -12 6 -8")}</g>`
    : mood === "sleepy" ? `${arm("M-9 -20 Q-10 -12 -3 -12")}${arm("M9 -20 Q10 -12 3 -12")}<rect x="-4.5" y="-17" width="9" height="8" rx="2" fill="var(--raised)" stroke="var(--muted)" stroke-width="1"/>`
    : st === "keputusan" ? `${arm("M-9 -21 Q-16 -28 -10 -36")}${arm("M9 -21 Q16 -28 10 -36")}`
    : `${arm("M-9 -20 Q-12 -12 -10 -6")}${arm("M9 -20 Q12 -12 10 -6")}`;
  const body = `<path d="M-11 -2 C-12 -16 -10 -27 0 -27 C10 -27 12 -16 11 -2 Z" fill="${col}"/>`;
  let head;
  if (human) {
    const eyes = mood === "happy" ? `<path d="M-5.5 -38 q2 -2.4 4 0 M1.5 -38 q2 -2.4 4 0" stroke="#2a1d17" stroke-width="1.4" fill="none" stroke-linecap="round"/>` : `<circle class="oo-eye" cx="-3.6" cy="-38" r="1.4" fill="#2a1d17"/><circle class="oo-eye" cx="3.6" cy="-38" r="1.4" fill="#2a1d17"/>`;
    head = `<circle cx="0" cy="-38" r="10.5" fill="${skin}"/><path d="M-10.6 -38.5 Q-10 -50 0 -49.5 Q10.6 -50 10.6 -38 Q6 -45 -1 -44 Q-7 -43.5 -10.6 -38.5 Z" fill="${hair}"/>${c.who === "OO" ? `<path d="M9.5 -42 Q13 -34 10 -27 M-9.5 -42 Q-13 -34 -10 -27" stroke="${hair}" stroke-width="3" fill="none" stroke-linecap="round"/>` : ""}${eyes}<path d="M-3 -33.5 q3 ${mood === "happy" ? 3 : 1.5} 6 0" stroke="#7a3b2a" stroke-width="1.3" fill="none" stroke-linecap="round"/>`;
  } else {
    const eyes = mood === "happy" ? `<path d="M-5.5 -38 q2 -2.6 4 0 M1.5 -38 q2 -2.6 4 0" stroke="#fff" stroke-width="1.6" fill="none" stroke-linecap="round"/>`
      : mood === "sleepy" ? `<path d="M-5.5 -38.5 h4 M1.5 -38.5 h4" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/>`
      : mood === "sad" ? `<path d="M-6 -42 l4 1.4 M6 -42 l-4 1.4" stroke="#fff" stroke-width="1.4" stroke-linecap="round"/><circle cx="-3.6" cy="-38" r="1.6" fill="#fff"/><circle cx="3.6" cy="-38" r="1.6" fill="#fff"/>`
      : `<ellipse class="oo-eye" cx="-3.6" cy="-38.5" rx="1.7" ry="2.1" fill="#fff"/><ellipse class="oo-eye" cx="3.6" cy="-38.5" rx="1.7" ry="2.1" fill="#fff"/>`;
    const mouth = mood === "happy" ? "M-3 -31.5 q3 2.6 6 0" : mood === "sad" ? "M-3 -30 q3 -2.4 6 0" : "M-2.5 -31 h5";
    head = `<line x1="0" y1="-49" x2="0" y2="-54" stroke="${col}" stroke-width="1.6"/><circle cx="0" cy="-55.5" r="2.6" fill="${sc}"/><rect class="oo-head" x="-12" y="-49" width="24" height="21" rx="7" stroke="${col}" stroke-width="2"/><rect x="-8.5" y="-44" width="17" height="10.5" rx="5" fill="${col}"/>${eyes}<path d="${mouth}" stroke="${col}" stroke-width="1.5" fill="none" stroke-linecap="round"/>`;
  }
  const badge = human ? "" : svgIc(AG.PERSONAS[c.p].i, -5, -20, 10, "oo-badge");
  return `<g transform="scale(${S})"><ellipse cx="0" cy="0" rx="15" ry="7" fill="${sc}" opacity=".2"/><ellipse class="ring" cx="0" cy="0" rx="19" ry="9"/><g class="${st === "jalan" || st === "rapat" ? "oo-bob" : ""}">${body}${arms}${badge}${head}</g></g>`;
}
/* Label nama + chip status, selalu di lapisan atas. */
function tag(c, X, Y) {
  const nm = oshort(c), nw = tw(nm) + 26, sc = OST[c.st][2];
  const defs = { review: ["warn", "hand-stop", c.tk], keputusan: ["dng", "alert-triangle", c.tk], ok: ["ok", "check", "Disetujui"], jalan: ["acc", "", ""], rapat: ["info", "bulb", ""], idle: ["gray", "coffee", ""], jeda: ["gray", "player-pause", ""] };
  const d = defs[c.st];
  let cw = 0, chip = "";
  if (d) {
    const [tone, i, txt] = d; cw = c.st === "jalan" ? 30 : (i ? 18 : 0) + (txt ? tw(txt, 11) + 8 : 0) + 8;
    chip = `<g class="oo-chip ${tone}" transform="translate(${(-cw / 2).toFixed(1)},-25)">${c.st === "keputusan" ? `<rect class="oo-pulse" x="0" y="-11" width="${cw}" height="22" rx="11" style="fill:var(--danger)"/>` : ""}<rect x="0" y="-11" width="${cw}" height="22" rx="11"/>${c.st === "jalan" ? `<circle class="oo-dt" cx="9" cy="0" r="2"/><circle class="oo-dt d2" cx="15" cy="0" r="2"/><circle class="oo-dt d3" cx="21" cy="0" r="2"/>` : `${i ? svgIc(i, 6, -6, 12) : ""}${txt ? `<text x="${i ? 21 : 7}" y="4">${esc(txt)}</text>` : ""}`}</g>`;
  }
  const x0 = -nw / 2;
  return `<g class="oo-tag ${OS.sel === c.id ? "sel" : ""}" data-och="${c.id}" data-act="oo-sel" data-arg="${c.id}" transform="translate(${X.toFixed(1)},${Y.toFixed(1)})"><g class="oo-float">${chip}<g transform="translate(${x0.toFixed(1)},0)"><rect class="nm" x="0" y="-11" width="${nw}" height="22" rx="11"/><circle cx="11" cy="0" r="3.5" fill="${sc}"/><text x="19" y="4">${esc(nm)}</text></g></g></g>`;
}

/* ---------- Panel detail (bersama) ---------- */
const fmtLog = (l, i = 0) => `<span class="d">10.${String(31 + i * 2).padStart(2, "0")}</span> ${esc(l).replace(/^✓/, '<span class="g">✓</span>').replace(/^✗/, '<span class="r">✗</span>').replace(/(ag_\w+|rtk)/, '<span class="b">$1</span>')}`;
function oDetail(c) {
  const t = c.tk ? AG.ticket(c.tk) : null, s = OST[c.st], human = !!c.who;
  const lk = logKey(c), lines = OLOGS[lk] || [], live = c.st === "jalan" || c.st === "pemilik";
  let acts = "";
  if (c.st === "review") acts = `${btn("Reject", "", "x", { "data-dialog": "oo-reject", "data-arg": c.id })}${btn("Approve", "pri", "check", { "data-act": "oo-approve", "data-arg": c.id })}`;
  else if (c.st === "keputusan") acts = `${btn("Reject", "", "x", { "data-dialog": "oo-reject", "data-arg": c.id })}${btn("Coba lagi dengan catatan", "pri", "refresh", { "data-dialog": "oo-retry", "data-arg": c.id })}`;
  else if (c.st === "jalan") acts = `${btn("Jeda", "", "player-pause", { "data-act": "oo-pause", "data-arg": c.id })}${btn("Buka terminal", "", "terminal-2", { "data-toast": `Terminal ${c.runner} dibuka (hanya baca)` })}`;
  else if (c.st === "jeda") acts = btn("Lanjutkan", "pri", "player-play", { "data-act": "oo-pause", "data-arg": c.id });
  else if (c.st === "idle") acts = btn("Tugaskan tiket", "pri", "plus", { "data-menu": "oo-assign", "data-arg": c.id });
  else if (c.st === "rapat") acts = btn("Gabung sesi", "pri", "message-circle", { "data-toast": "Lanjutkan dengan /ag-brainstorming KSR-31 di client Anda" });
  return `<div class="oo-det">
    <div class="oo-dh">${av(c.who || c.p, 40)}<div class="grow" style="min-width:0"><div class="md b6 trunc">${esc(oname(c))}</div><div class="mut sm trunc">${human ? AG.PEOPLE[c.who].job + " · pemilik runner" : AG.PERSONAS[c.p].t}</div></div>${btn("", "ghost icon sm", "x", { "data-act": "oo-sel", "data-arg": "", "aria-label": "Tutup panel" })}</div>
    ${c.st === "keputusan" ? `<div class="alert dng">${ic("alert-triangle", "s14")}<span><b>Butuh keputusan.</b> ${esc(c.doing)}. Gate “Hasil test lulus” belum terpenuhi.</span></div>` : c.st === "review" ? `<div class="alert warn">${ic("hand-stop", "s14")}<span>Menunggu approval Anda di stage <b>${AG.stage(t.stage).n}</b>.</span></div>` : ""}
    <dl class="props oo-props"><dt>Status</dt><dd>${pill(s[0], s[1])}</dd>
      ${t ? `<dt>Tiket</dt><dd class="row" style="min-width:0">${tref(t.k)}<span class="trunc sm">${esc(t.t)}</span></dd><dt>Stage</dt><dd class="row g4">${AG.stageIcon(t.stage)}${AG.stage(t.stage).n}</dd>` : human ? `<dt>Job</dt><dd class="oo-jobs">${c.jobs.map(tref).join("")}</dd>` : `<dt>Tiket</dt><dd class="mut">Tidak ada job</dd>`}
      ${c.runner ? `<dt>Runner</dt><dd class="row g4"><span class="live ${c.st === "jalan" ? "busy" : ""}"></span>${c.runner}</dd>` : ""}
      ${c.since ? `<dt>Mulai</dt><dd>${c.since}</dd>` : ""}
      ${c.steps ? `<dt>Progres</dt><dd class="row"><span class="bar grow"><i style="width:${Math.round((c.step / c.steps) * 100)}%"></i></span><span class="mut sm tab-num">${c.step}/${c.steps}</span></dd>` : ""}</dl>
    ${c.st !== "idle" ? `<div><div class="oo-h">${ic("terminal-2", "s14")}Log kerja ${live ? pill("live", "acc") : ""}</div><div class="term oo-log" id="oo-log" data-k="${lk}" data-live="${live ? 1 : 0}">${lines.slice(0, live ? 4 : lines.length).map((l, i) => `<div>${fmtLog(l, i)}</div>`).join("")}</div></div>` : `<div class="empty" style="padding:18px">${ic("coffee", "s18")}<span>Agent menganggur di pantry. Tugaskan tiket supaya mulai bekerja.</span></div>`}
    <div class="oo-acts">${acts}${t ? btn("Buka tiket", "ghost", "external-link", { "data-go": `#/t/${t.k}` }) : ""}</div>
    ${c.p ? `<a class="link sm row g4" href="#/settings/personas">${ic("robot", "s14")}Kustom karakter & persona</a>` : ""}
  </div>`;
}
const todoList = () => { const l = todo(); return l.length ? l.map((x) => `<button class="oo-todo" data-act="oo-sel" data-arg="${x.id}">${av(x.p, 26)}<span class="grow" style="min-width:0"><span class="row g4"><span class="mono sm">${x.tk}</span>${pill(OST[x.st][0], OST[x.st][1])}</span><span class="sm trunc" style="display:block">${esc(AG.ticket(x.tk).t)}</span></span>${ic("chevron-right", "s14 faint")}</button>`).join("") : `<div class="mut sm row g4">${ic("circle-check", "s14 text-ok")}Tidak ada yang menunggu Anda.</div>`; };
const summary = () => `<div class="oo-sum"><span class="row g4"><span class="live busy"></span><b>${ocount("jalan")}</b> bekerja</span><span><b>${ocount("review")}</b> butuh approval</span><span class="text-dng"><b class="text-dng">${ocount("keputusan")}</b> butuh keputusan</span><span><b>${ocount("idle")}</b> di pantry</span></div>`;
const legend = () => `<div class="oo-leg">${[["var(--accent)", "Bekerja"], ["var(--warn)", "Butuh approval"], ["var(--danger)", "Butuh keputusan"], ["var(--info)", "Brainstorming"], ["var(--faint)", "Menganggur"]].map(([c, l]) => `<span><i style="background:${c}"></i>${l}</span>`).join("")}</div>`;

/* ---------- Aksi office ---------- */
function panelHtml() {
  const c = OS.sel ? ochar(OS.sel) : null;
  if (OS.opt === "A") return c ? oDetail(c) : aOverview();
  return c ? oDetail(c) : "";
}
let tick = null;
function startTick() {
  clearInterval(tick); const ptr = {};
  tick = setInterval(() => {
    if (!document.querySelector(".oo-root")) return clearInterval(tick);
    const el = document.getElementById("oo-log");
    if (el && el.dataset.live === "1") { const pool = OLOGS[el.dataset.k] || []; const n = el.children.length; if (n < pool.length) { const d = document.createElement("div"); d.className = "new"; d.innerHTML = fmtLog(pool[n], n); el.appendChild(d); el.scrollTop = el.scrollHeight; } }
    document.querySelectorAll("[data-step]").forEach((s) => { const c = ochar(s.dataset.step); if (!c || c.st !== "jalan") return; const pool = OLOGS[logKey(c)]; ptr[c.id] = ((ptr[c.id] ?? 3) + 1) % pool.length; const t = s.querySelector(".trunc"); if (t) t.textContent = pool[ptr[c.id]].replace(/^[✓✗] /, ""); });
  }, 2400);
}
AG.action("oo-sel", (el, id) => {
  OS.sel = id || null;
  document.querySelectorAll("[data-och]").forEach((g) => g.classList.toggle("sel", g.dataset.och === OS.sel));
  const p = document.getElementById("oo-panel");
  if (p) { p.innerHTML = panelHtml(); p.classList.toggle("open", !!OS.sel); p.scrollTop = 0; startTick(); if (OS.sel && innerWidth < 900) p.scrollIntoView({ behavior: "smooth", block: "start" }); }
});
AG.action("oo-opt", (el, k) => { OS.opt = k; OS.sel = null; OS.room = null; AG.rerender(); });
AG.action("oo-pick", (el, k) => { OS.pick = k; AG.rerender(); AG.toast(`Opsi ${k} dicatat sebagai pilihan tampilan AI Office`); });
AG.action("oo-reset", () => { OCH = JSON.parse(JSON.stringify(OCH0)); OS.sel = null; rr(); AG.toast("Data contoh dikembalikan"); });
AG.action("oo-room", (el, k) => { OS.room = OS.room === k ? null : k; rr(); });
AG.action("oo-zoom", (el, d) => { document.querySelector(".ooa-stage,.oob-stage")?.classList.toggle("z2", d === "in"); });
AG.action("oo-bub", (el) => { OS.bub = !OS.bub; el.classList.toggle("on", OS.bub); el.setAttribute("aria-checked", OS.bub); document.querySelector(".oob-stage")?.classList.toggle("nobub", !OS.bub); });
AG.action("oo-approve", (el, id) => { const c = ochar(id); const st = AG.stage(AG.ticket(c.tk).stage); c.st = "ok"; c.doing = "Disetujui · lanjut ke stage berikutnya"; rr(); AG.toast(`${c.tk} disetujui di stage ${st.n} · lanjut ke stage berikutnya`); });
AG.action("oo-pause", (el, id) => { const c = ochar(id); c.st = c.st === "jeda" ? "jalan" : "jeda"; rr(); AG.toast(c.st === "jeda" ? `Job ${c.tk} dijeda di ${c.runner}` : `Job ${c.tk} dilanjutkan`); });
AG.action("oo-redo", (el, id) => { const c = ochar(id); c.st = "jalan"; c.doing = "Mengulang dengan catatan Anda"; c.step = 1; AG.closeDialog(); rr(); AG.toast(`${c.tk} dikembalikan ke ${oname(c)} dengan catatan`); });
AG.dialog("oo-reject", (id) => { const c = ochar(id); return { icon: "x", tone: "dng", title: `Reject ${c.tk}`, desc: `${esc(AG.ticket(c.tk).t)}. ${oname(c)} mengulang pekerjaan dengan catatan Anda.`,
  body: AG.fld("Catatan untuk agent", AG.inp("", { area: true, ph: "Mis. tombol QRIS harus tetap terlihat saat keranjang panjang" })),
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Reject", "dngs", "", { "data-act": "oo-redo", "data-arg": id })}` }; });
AG.dialog("oo-retry", (id) => { const c = ochar(id); return { icon: "refresh", tone: "acc", title: `Coba lagi ${c.tk}`, desc: "Agent QA menjalankan ulang test dengan catatan tambahan.",
  body: AG.fld("Catatan", AG.inp("Nama kasir diambil dari sesi login PIN, bukan dari profil toko. Cek juga pembulatan Rp100.", { area: true })),
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Jalankan ulang", "pri", "player-play", { "data-act": "oo-redo", "data-arg": id })}` }; });
AG.menu("oo-assign", (id) => { const c = ochar(id); return `<div class="mlab">Tugaskan ke ${esc(oname(c))}</div>${AG.TICKETS.filter((t) => t.stage === "backlog" || t.state === "antre").slice(0, 5).map((t) => `<button class="mi" data-act="oo-assign-ok" data-arg="${id}:${t.k}">${AG.typeIcon(t.type)}<span class="mono mut">${t.k}</span><span class="trunc">${esc(t.t)}</span></button>`).join("")}`; });
AG.action("oo-assign-ok", (el, arg) => { const [id, k] = arg.split(":"); const c = ochar(id); Object.assign(c, { st: "jalan", tk: k, doing: "Membaca tiket dan acceptance criteria", step: 1, steps: 6, since: "baru saja", runner: "MacBook Olivia", room: "KSR", moved: true }); OS.sel = id; rr(); AG.toast(`${k} ditugaskan ke ${oname(c)} · berjalan di MacBook Olivia`); });

/* ====================== Office A: isometrik dirapikan ====================== */
const U = 24;
const P = (x, y, z = 0) => [(x - y) * U * 0.866, (x + y) * U * 0.5 - z * U];
const pts = (...a) => a.map(([X, Y]) => `${X.toFixed(1)},${Y.toFixed(1)}`).join(" ");
function box(x, y, z, w, d, h, cls, extra = "") {
  return `<g class="${cls}" ${extra}><polygon class="l" points="${pts(P(x, y + d, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x, y + d, z + h))}"/><polygon class="r" points="${pts(P(x + w, y, z), P(x + w, y + d, z), P(x + w, y + d, z + h), P(x + w, y, z + h))}"/><polygon class="t" points="${pts(P(x, y, z + h), P(x + w, y, z + h), P(x + w, y + d, z + h), P(x, y + d, z + h))}"/></g>`;
}
const AR = { KSR: [0, 0, 13, 8], MEET: [15.5, 0, 7, 8], GDG: [0, 12.5, 6, 5.5], PPL: [7.5, 12.5, 5.5, 5.5], PANTRY: [15.5, 12.5, 7, 5.5] };
const ADESK = [[0.9, 2.0], [4.1, 2.0], [7.3, 2.0], [10.5, 2.0], [0.9, 5.8], [4.1, 5.8], [7.3, 5.8], [10.5, 5.8]];
const APOS = { dev: 0, dewi: 1, qa: 2, qori: 3, ux: 4, rev: 5, pm: 6, qa2: 7, an: [18.4, 2.1], oli: [20.2, 2.1], arch: [17.2, 15.2], ops: [18.9, 16.9], tw: [21.3, 13.9] };
function aPos(c, i) {
  if (c.moved) return [12.4, 3.6 + i * 0.4];
  const v = APOS[c.id]; if (typeof v === "number") { const [x, y] = ADESK[v]; return [x + 0.65, y - 0.45]; } return v;
}
function aDesk(x, y, on, tint) {
  const glow = on ? (() => { const [cx, cy] = P(x + 1.35, y + 0.4, 0.72); return `<ellipse cx="${cx.toFixed(1)}" cy="${cy.toFixed(1)}" rx="11" ry="5" fill="${tint}" opacity=".28"/>`; })() : "";
  return box(x, y, 0, 1.9, 0.9, 0.7, "ooa-desk") + glow + box(x + 1.05, y + 0.08, 0.7, 0.7, 0.1, 0.5, "ooa-dark") + (on ? `<polygon points="${pts(P(x + 1.1, y + 0.08, 0.76), P(x + 1.7, y + 0.08, 0.76), P(x + 1.7, y + 0.08, 1.14), P(x + 1.1, y + 0.08, 1.14))}" fill="${tint}" opacity=".0"/>` : "");
}
function aPlant(x, y) { const [cx, cy] = P(x + 0.25, y + 0.25, 0.45); return box(x, y, 0, 0.5, 0.5, 0.45, "ooa-pot") + `<ellipse cx="${cx.toFixed(1)}" cy="${(cy - 9).toFixed(1)}" rx="10" ry="12" fill="#3f9a5a"/><ellipse cx="${(cx + 5).toFixed(1)}" cy="${(cy - 4).toFixed(1)}" rx="7" ry="7" fill="#2f7d47"/>`; }
function officeA() {
  const base = [], items = [], top = [];
  const add = (k, room, svg) => items.push([k, `<g data-room="${room}">${svg}</g>`]);
  OROOMS.forEach((r) => {
    const [x, y, w, d] = AR[r.k], st = `style="--rc:${r.c}"`;
    base.push(`<g data-room="${r.k}">${box(x, y, -0.35, w, d, 0.35, "ooa-floor", st)}${box(x, y - 0.14, 0, w, 0.14, 1.0, "ooa-wall", st)}${box(x - 0.14, y - 0.14, 0, 0.14, d + 0.14, 1.0, "ooa-wall", st)}</g>`);
    const here = OCH.filter((c) => c.room === r.k && !c.moved || (c.moved && r.k === "KSR"));
    const sub = r.k === "PANTRY" ? `${here.length} agent menganggur` : r.k === "MEET" ? "KSR-31 · terkunci" : here.length ? `${here.filter((c) => c.p).length} agent · ${here.filter((c) => c.st === "review" || c.st === "keputusan").length} perlu tindakan` : "Tidak ada job aktif";
    const [sx, sy] = P(x + w, y + d, -0.35); const wd = Math.max(tw(r.n, 13), tw(sub, 11)) + 40;
    top.push(`<g class="oo-sign" data-room="${r.k}" data-act="oo-room" data-arg="${r.k}" style="cursor:pointer" transform="translate(${r.k === "KSR" ? (P(0, 8)[0] - wd - 14).toFixed(1) : (sx - wd / 2).toFixed(1)},${r.k === "KSR" ? 64 : (sy + 8).toFixed(1)})"><rect class="bg" width="${wd.toFixed(0)}" height="40" rx="9"/><rect x="12" y="10" width="8" height="8" rx="2.5" fill="${r.c}"/><text x="27" y="18">${r.n}</text><text class="s" x="12" y="32">${sub}</text></g>`);
  });
  ADESK.forEach(([x, y], i) => { const c = OCH.find((o) => APOS[o.id] === i); add(x + y + 1.4, "KSR", aDesk(x, y, c && c.st === "jalan", c ? ocol(c) : "#888")); });
  [[1.0, 14.6], [3.4, 14.6]].forEach(([x, y]) => add(x + y + 1.4, "GDG", aDesk(x, y, false)));
  [[8.4, 14.6], [10.8, 14.6]].forEach(([x, y]) => add(x + y + 1.4, "PPL", aDesk(x, y, false)));
  add(17.6 + 2.8 + 1.6, "MEET", box(17.6, 2.8, 0, 3.6, 2.2, 0.62, "ooa-wood"));
  add(16 + 13 + 1, "PANTRY", box(16, 13, 0, 3.2, 0.9, 0.85, "ooa-desk") + box(16.3, 13.15, 0.85, 0.5, 0.5, 0.55, "ooa-dark"));
  add(19.4 + 15.4 + 1, "PANTRY", box(19.4, 15.4, 0, 1.2, 1.2, 0.6, "ooa-wood"));
  add(20.6 + 13.2 + 0.6, "PANTRY", box(20.6, 13.2, 0, 1.7, 0.25, 0.85, "ooa-sofa"));
  add(20.6 + 13.45 + 1.2, "PANTRY", box(20.6, 13.45, 0, 1.7, 0.75, 0.4, "ooa-sofa"));
  add(12.3 + 0.5 + 0.5, "KSR", aPlant(12.3, 0.5)); add(21.7 + 0.6 + 0.5, "MEET", aPlant(21.7, 0.6)); add(0.4 + 17.3 + 0.5, "GDG", aPlant(0.4, 17.3)); add(15.9 + 17.3 + 0.5, "PANTRY", aPlant(15.9, 17.3));
  let mi = 0;
  OCH.forEach((c) => {
    const [x, y] = aPos(c, c.moved ? mi++ : 0), [X, Y] = P(x, y);
    const room = c.moved ? "KSR" : c.room;
    add(x + y, room, `<g class="oo-ch ${OS.sel === c.id ? "sel" : ""}" data-och="${c.id}" data-act="oo-sel" data-arg="${c.id}" tabindex="0" role="button" aria-label="${esc(oname(c))}, ${OST[c.st][0]}${c.tk ? ", " + c.tk : ""}" transform="translate(${X.toFixed(1)},${Y.toFixed(1)})"><title>${esc(oname(c))} · ${OST[c.st][0]}</title><rect x="-18" y="-70" width="36" height="74" fill="transparent"/>${fig(c, 1.2)}</g>`);
    top.push(`<g data-room="${room}">${tag(c, X, Y - 84)}</g>`);
  });
  items.sort((a, b) => a[0] - b[0]);
  const focus = OS.room ? `<style>.ooa-svg [data-room]:not([data-room="${OS.room}"]) { opacity: .28; }</style>` : "";
  return `${focus}<svg class="ooa-svg" viewBox="-395 -100 885 650" role="img" aria-label="Kantor isometrik AI Office">${base.join("")}${items.map((i) => i[1]).join("")}${top.join("")}</svg>`;
}
function aOverview() {
  return `<div><div class="md b6">Kantor PT Contoh</div><div class="mut sm">Live dari 3 runner. Klik karakter untuk melihat log kerja dan approve.</div></div>
    <div><div class="oo-h">${ic("hand-stop", "s14")}Perlu tindakan Anda · ${todo().length}</div>${todoList()}</div>
    <div><div class="oo-h">${ic("building", "s14")}Ruangan</div>${OROOMS.map((r) => { const n = OCH.filter((c) => c.room === r.k).length; return `<button class="oo-room ${OS.room === r.k ? "on" : ""}" data-act="oo-room" data-arg="${r.k}"><span class="dot" style="background:${r.c}"></span><span class="grow">${r.n}</span><span class="mut">${n ? n + " orang" : "kosong"}</span></button>`; }).join("")}</div>
    <div><div class="oo-h">${ic("device-laptop", "s14")}Runner</div>${AG.RUNNERS.map((r) => `<div class="row sm" style="padding:4px 0"><span class="live ${r.st === "off" ? "off" : r.st === "busy" ? "busy" : ""}"></span><span class="grow trunc">${r.n}</span><span class="mut xs">${r.st === "off" ? "offline" : r.st === "busy" ? "sibuk" : "idle"}</span></div>`).join("")}</div>`;
}
function oA() {
  return `<div class="ok-top"><span class="ok-crumbs"><b>AI Office</b></span><span class="grow"></span>${btn("Kustom karakter", "ghost sm hide-sm", "robot", { "data-go": "#/settings/personas" })}</div>
    <div class="ooa"><div class="ooa-stage">
      <div class="ooa-tb">${summary()}<span class="grow"></span><div class="oo-zoom"><button data-act="oo-zoom" data-arg="in" aria-label="Perbesar" title="Perbesar">${ic("zoom-in", "s14")}</button><button data-act="oo-zoom" data-arg="out" aria-label="Perkecil" title="Perkecil">${ic("zoom-out", "s14")}</button></div></div>
      ${officeA()}
      <div style="padding:0 16px 12px">${legend()}</div>
    </div><aside class="ooa-side" id="oo-panel" aria-live="polite" aria-label="Detail">${panelHtml()}</aside></div>`;
}

/* ====================== Office B: denah 2D ====================== */
const BR = { KSR: [40, 40, 560, 290], MEET: [620, 40, 340, 290], GDG: [40, 390, 270, 190], PPL: [330, 390, 270, 190], PANTRY: [620, 390, 340, 190] };
const BDESK = [[80, 74], [205, 74], [330, 74], [455, 74], [80, 196], [205, 196], [330, 196], [455, 196]];
const BPOS = { dev: 0, dewi: 1, qa: 2, qori: 3, ux: 4, rev: 5, pm: 6, qa2: 7, an: [745, 244], oli: [835, 244], arch: [790, 478], ops: [300, 382], tw: [880, 548] };
const bPos = (c, i) => { if (c.moved) return [560, 150 + i * 40]; const v = BPOS[c.id]; if (typeof v === "number") { const [x, y] = BDESK[v]; return [x + 50, y + 82]; } return v; };
function bDesk(x, y, on) {
  return `<rect class="oob-desk" x="${x}" y="${y}" width="100" height="42" rx="6"/><rect class="oob-mon ${on ? "on" : ""}" x="${x + 30}" y="${y + 6}" width="40" height="7" rx="2"/>${on ? `<rect x="${x + 33}" y="${y + 13}" width="34" height="3" rx="1.5" fill="var(--accent)" opacity=".5"/>` : ""}<rect x="${x + 34}" y="${y + 22}" width="32" height="10" rx="2" fill="color-mix(in srgb, var(--fg) 12%, var(--raised))"/><circle class="oob-chair" cx="${x + 50}" cy="${y + 64}" r="14"/>`;
}
const bPlant = (x, y, s = 1) => `<circle class="oob-leaf" cx="${x}" cy="${y}" r="${14 * s}"/><circle class="oob-leaf2" cx="${x + 5 * s}" cy="${y - 4 * s}" r="${8 * s}"/><circle class="oob-leaf2" cx="${x - 6 * s}" cy="${y + 5 * s}" r="${6 * s}"/>`;
function bBubble(c) {
  const map = { jalan: ["acc", "sparkles", c.doing.length > 22 ? c.doing.slice(0, 21) + "…" : c.doing], review: ["warn", "hand-stop", "Butuh approval"], keputusan: ["dng", "alert-triangle", "Butuh keputusan"], rapat: ["info", "bulb", c.p ? "Brainstorming" : ""], idle: ["gray", "coffee", "Istirahat"], ok: ["ok", "check", "Disetujui"], jeda: ["gray", "player-pause", "Dijeda"] };
  const d = map[c.st]; if (!d || !d[2]) return "";
  const [tone, i, t] = d, w = tw(t, 10.5) + 34, y = -64;
  return `<g class="oob-bub ${tone} oo-float">${c.st === "keputusan" ? `<rect class="oo-pulse" x="${-w / 2}" y="${y - 12}" width="${w}" height="22" rx="11" style="fill:var(--danger)"/>` : ""}<rect x="${(-w / 2).toFixed(1)}" y="${y - 12}" width="${w.toFixed(1)}" height="22" rx="11"/><path d="M-5 ${y + 9.5} L0 ${y + 15} L5 ${y + 9.5}"/>${svgIc(i, -w / 2 + 8, y - 7, 12)}<text x="${(-w / 2 + 24).toFixed(1)}" y="${y + 3}">${esc(t)}</text></g>`;
}
function officeB() {
  let s = `<defs><pattern id="oob-tiles" width="24" height="24" patternUnits="userSpaceOnUse"><path d="M24 0H0V24" fill="none" stroke="currentColor" stroke-opacity=".07"/></pattern></defs>`;
  s += `<rect class="oob-cor" x="24" y="24" width="952" height="572" rx="14"/><rect x="24" y="24" width="952" height="572" rx="14" fill="url(#oob-tiles)"/>`;
  OROOMS.forEach((r) => { const [x, y, w, h] = BR[r.k]; s += `<g data-room="${r.k}"><rect x="${x}" y="${y}" width="${w}" height="${h}" rx="8" fill="color-mix(in srgb, ${r.c} 9%, var(--raised))"/><rect x="${x}" y="${y}" width="${w}" height="${h}" rx="8" fill="url(#oob-tiles)"/><rect class="oob-wall" x="${x}" y="${y}" width="${w}" height="${h}" rx="8"/></g>`; });
  [[280, 327, 60, 8], [760, 327, 60, 8], [150, 386, 60, 8], [440, 386, 60, 8], [760, 386, 60, 8]].forEach(([x, y, w, h]) => (s += `<rect class="oob-door" x="${x}" y="${y}" width="${w}" height="${h}"/>`));
  s += `<rect class="oob-door" x="597" y="160" width="26" height="60"/>`;
  BDESK.forEach(([x, y], i) => { const c = OCH.find((o) => BPOS[o.id] === i); s += bDesk(x, y, c && c.st === "jalan"); });
  [[70, 430], [190, 430], [360, 430], [480, 430]].forEach(([x, y]) => (s += bDesk(x, y, false)));
  s += `<rect class="oob-wood" x="690" y="120" width="200" height="90" rx="40"/>${[735, 790, 845].map((x) => `<circle class="oob-chair" cx="${x}" cy="104" r="12"/><circle class="oob-chair" cx="${x}" cy="226" r="12"/>`).join("")}<rect x="810" y="46" width="120" height="10" rx="3" fill="#34343f"/><rect x="818" y="48" width="104" height="5" rx="2" fill="#0891b2" opacity=".6"/>`;
  s += `<ellipse class="oob-rug" cx="790" cy="512" rx="120" ry="52"/><rect class="oob-counter" x="640" y="402" width="220" height="34" rx="5"/><rect x="652" y="407" width="24" height="24" rx="4" fill="#34343f"/><rect x="722" y="408" width="44" height="22" rx="8" fill="color-mix(in srgb, #0ea5e9 25%, var(--raised))"/><rect class="oob-counter" x="902" y="402" width="44" height="56" rx="5"/>`;
  s += `<circle class="oob-wood" cx="720" cy="520" r="26"/><circle class="oob-wood" cx="830" cy="505" r="22"/><path class="oob-sofa" d="M860 566 h88 v-70 h-18 v52 h-70 z"/>`;
  s += bPlant(578, 306, 0.8) + bPlant(940, 312, 0.9) + bPlant(290, 562, 0.8) + bPlant(580, 562, 0.8) + bPlant(62, 312, 0.9);
  OROOMS.forEach((r) => { const [x, y, w] = BR[r.k]; const n = OCH.filter((c) => c.room === r.k).length; const sub = r.k === "PANTRY" ? `${n} menganggur` : r.k === "MEET" ? "KSR-31 · terkunci" : n ? `${n} orang · ${OCH.filter((c) => c.room === r.k && (c.st === "review" || c.st === "keputusan")).length} perlu tindakan` : "Tidak ada job aktif"; const wd = Math.max(tw(r.n, 13), tw(sub, 11)) + 36; const lx = x + 14, ly = y < 100 ? y - 19 : y + 136; s += `<g class="oob-label" data-room="${r.k}" transform="translate(${lx},${ly})"><rect class="bg" width="${wd}" height="38" rx="8"/><rect x="12" y="10" width="8" height="8" rx="2.5" fill="${r.c}"/><text x="27" y="18">${r.n}</text><text class="s" x="12" y="31">${sub}</text></g>`; });
  let mi = 0;
  const chars = OCH.map((c) => [c, bPos(c, c.moved ? mi++ : 0)]).sort((a, b) => a[1][1] - b[1][1]);
  s += chars.map(([c, [x, y]]) => {
    const walk = c.id === "ops" && c.st === "idle", wander = !walk && c.st === "idle";
    return `<g transform="translate(${x},${y})"><g class="${walk ? "oob-walk" : wander ? "oob-wander" : ""}"><g class="oo-ch ${OS.sel === c.id ? "sel" : ""}" data-och="${c.id}" data-act="oo-sel" data-arg="${c.id}" tabindex="0" role="button" aria-label="${esc(oname(c))}, ${OST[c.st][0]}"><title>${esc(oname(c))} · ${OST[c.st][0]}</title><rect x="-20" y="-60" width="40" height="76" fill="transparent"/><g class="${walk ? "oob-step" : ""}">${fig(c, 0.95)}</g>${walk ? "" : bBubble(c)}<g class="oob-name" transform="translate(0,14)"><rect x="${(-(tw(oshort(c), 10.5) + 14) / 2).toFixed(1)}" y="-8" width="${(tw(oshort(c), 10.5) + 14).toFixed(1)}" height="16" rx="8"/><text x="0" y="3.5" text-anchor="middle">${esc(oshort(c))}</text></g></g></g></g>`;
  }).join("");
  const focus = OS.room ? `<style>.oob-svg [data-room]:not([data-room="${OS.room}"]) { opacity: .35; }</style>` : "";
  return `${focus}<svg class="oob-svg" viewBox="20 20 960 580" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Denah kantor AI Office">${s}</svg>`;
}
function oB() {
  return `<div class="ok-top"><span class="ok-crumbs"><b>AI Office</b></span><span class="grow"></span>${btn("Kustom karakter", "ghost sm hide-sm", "robot", { "data-go": "#/settings/personas" })}</div>
    <div class="oob"><div class="oob-tb">${summary()}<span class="grow"></span><span class="row g8 sm mut">Gelembung status<button type="button" class="sw ${OS.bub ? "on" : ""}" role="switch" aria-checked="${OS.bub}" aria-label="Gelembung status" data-act="oo-bub"></button></span><div class="oo-zoom"><button data-act="oo-zoom" data-arg="in" aria-label="Perbesar" title="Perbesar">${ic("zoom-in", "s14")}</button><button data-act="oo-zoom" data-arg="out" aria-label="Perkecil" title="Perkecil">${ic("zoom-out", "s14")}</button></div></div>
      <div class="oob-stage ${OS.bub ? "" : "nobub"}">${officeB()}
        <div class="oob-hint">${ic("hand-stop", "s14")}Klik karakter untuk log kerja, approve, atau reject</div></div>
        <aside class="oob-pop ${OS.sel ? "open" : ""}" id="oo-panel" aria-live="polite" aria-label="Detail">${panelHtml()}</aside></div>`;
}

/* ====================== Office C: papan tim ====================== */
function cCard(c) {
  const s = OST[c.st], t = c.tk ? AG.ticket(c.tk) : null, human = !!c.who;
  const pct = c.steps ? Math.round((c.step / c.steps) * 100) : 0;
  const tone = { review: "warn", keputusan: "dng", ok: "ok" }[c.st] || "";
  const acts = c.st === "review" ? `${btn("Tolak", "sm", "", { "data-dialog": "oo-reject", "data-arg": c.id })}${btn("Approve", "sm pri", "check", { "data-act": "oo-approve", "data-arg": c.id })}`
    : c.st === "keputusan" ? btn("Putuskan", "sm dng", "alert-triangle", { "data-act": "oo-sel", "data-arg": c.id })
    : c.st === "idle" ? btn("Tugaskan", "sm", "plus", { "data-menu": "oo-assign", "data-arg": c.id })
    : c.st === "jalan" || human ? btn("Log", "sm ghost", "terminal-2", { "data-act": "oo-sel", "data-arg": c.id }) : "";
  return `<div class="ooc-card ${OS.sel === c.id ? "sel" : ""}" style="--sc:${s[2]}" data-och="${c.id}" data-act="oo-sel" data-arg="${c.id}" role="button" tabindex="0" aria-label="${esc(oname(c))}, ${s[0]}">
    <div class="row top">${av(c.who || c.p, 32)}<div class="grow col g4" style="min-width:0"><div class="b6 trunc">${esc(oname(c))}</div><div class="row g8" style="min-width:0">${pill(s[0], s[1], c.st === "jalan" ? "var(--accent)" : "")}<span class="mut xs trunc">${human ? AG.PEOPLE[c.who].job : c.runner || "Belum ada job"}</span></div></div></div>
    ${t ? `<div class="ooc-tk">${tref(t.k)}<span class="t">${esc(t.t)}</span></div>` : human ? `<div class="ooc-tk"><span class="mut">Job:</span><span class="oo-jobs">${c.jobs.map(tref).join("")}</span></div>` : ""}
    <div class="ooc-step" data-step="${c.id}"><span class="trunc">${esc(c.doing)}</span>${c.steps ? `<span class="row"><span class="bar grow ${tone}"><i style="width:${pct}%"></i></span><span class="xs tab-num">${c.step}/${c.steps}</span></span>` : ""}</div>
    <div class="row"><span class="mut xs grow">${c.since ? "Sejak " + c.since : human ? "Runner " + c.runner : "Di pantry"}</span>${acts}</div></div>`;
}
function miniMap() {
  let s = "";
  OROOMS.forEach((r) => { const [x, y, w, h] = BR[r.k]; s += `<rect class="rm" x="${x}" y="${y}" width="${w}" height="${h}" rx="10" fill="color-mix(in srgb, ${r.c} 12%, var(--raised))"/><text x="${x + 18}" y="${y + 44}">${r.n}</text>`; });
  let mi = 0;
  OCH.forEach((c) => { const [x, y] = bPos(c, c.moved ? mi++ : 0); s += `<circle class="dt ${OS.sel === c.id ? "sel" : ""}" data-och="${c.id}" data-act="oo-sel" data-arg="${c.id}" cx="${x}" cy="${y - 20}" r="${c.who ? 15 : 19}" fill="${c.who ? ocol(c) : OST[c.st][2]}"><title>${esc(oname(c))} · ${OST[c.st][0]}</title></circle>`; });
  return `<svg viewBox="30 30 940 560" role="img" aria-label="Mini-map kantor">${s}</svg>`;
}
function oC() {
  const fid = "ooc-f";
  const fb = AG.filterBar({ id: fid, search: "", filters: [{ k: "st", label: "Status", icon: "circle-half-2", options: ["Bekerja", "Butuh approval", "Butuh keputusan", "Brainstorming", "Menganggur"] }, { k: "pj", label: "Project", icon: "folders", options: AG.PROJECTS.map((p) => p.n) }] });
  const fst = AG.filterValue(fid, "st"), fpj = AG.filterValue(fid, "pj");
  const proj = { KSR: "Kasir Online", MEET: "Kasir Online", GDG: "Aplikasi Gudang", PPL: "Portal Pelanggan", PANTRY: "" };
  const okc = (c) => (!fst.length || fst.includes(OST[c.st][0])) && (!fpj.length || fpj.includes(proj[c.room]));
  const groups = ["KSR", "MEET", "PANTRY", "GDG", "PPL"].map((k) => [OROOMS.find((r) => r.k === k), OCH.filter((c) => c.room === k && okc(c))]);
  const main = groups.map(([r, list]) => {
    const all = OCH.filter((c) => c.room === r.k);
    if (!all.length) return fpj.length && !fpj.includes(proj[r.k]) ? "" : `<div class="ooc-grp"><div class="ooc-gh"><span class="dot" style="background:${r.c}"></span><h3>${r.n}</h3></div><div class="ooc-empty">${ic("armchair", "s18")}<span class="grow">Tidak ada job aktif · 2 meja kosong</span>${btn("Buka board", "sm ghost", "layout-kanban", { "data-go": `#/p/${r.k}/board` })}</div></div>`;
    if (!list.length) return "";
    const need = all.filter((c) => c.st === "review" || c.st === "keputusan").length;
    return `<div class="ooc-grp"><div class="ooc-gh"><span class="dot" style="background:${r.c}"></span><h3>${r.n}</h3><span class="mut sm">${all.length} ${r.k === "PANTRY" ? "agent menganggur" : "orang"}${need ? ` · <span class="text-warn">${need} perlu tindakan</span>` : ""}${r.k === "MEET" ? " · KSR-31 terkunci" : ""}</span></div><div class="ooc-cards">${list.map(cCard).join("")}</div></div>`;
  }).join("") || AG.empty("filter", "Tidak ada yang cocok", "Ubah atau hapus filter.");
  return `<div class="ok-top"><span class="ok-crumbs"><b>AI Office</b></span></div>
    <div class="ooc"><div class="ooc-scroll" data-ks="ooc">
      <div class="okb-head"><div class="grow"><h1 style="font-size:20px">AI Office</h1><p class="mut">Siapa sedang mengerjakan apa, live dari runner. Approve langsung dari kartu.</p></div>${btn("Lihat denah", "", "map-2", { "data-act": "oo-opt", "data-arg": "B" })}</div>
      ${AG.stats([{ label: "Bekerja", value: ocount("jalan"), icon: "sparkles", tone: "acc", hint: "agent sedang menjalankan job" }, { label: "Butuh approval", value: ocount("review"), icon: "hand-stop", tone: "warn", hint: "menunggu Anda" }, { label: "Butuh keputusan", value: ocount("keputusan"), icon: "alert-triangle", tone: "dng", hint: "gagal 3 kali" }, { label: "Menganggur", value: ocount("idle"), icon: "coffee", hint: "di pantry" }])}
      ${fb}
      <div class="ooc-cols"><div class="ooc-main">${main}</div>
        <aside class="ooc-aside"><div class="box ooc-map"><div class="row" style="padding:4px 4px 8px"><b class="sm b5 grow">Kantor</b><span class="mut xs">klik titik</span></div>${miniMap()}</div>
          <div class="box pad"><div class="oo-h">${ic("hand-stop", "s14")}Perlu tindakan · ${todo().length}</div>${todoList()}</div>
          <div class="box pad"><div class="oo-h">${ic("device-laptop", "s14")}Runner</div>${AG.RUNNERS.map((r) => `<div class="row sm" style="padding:4px 0"><span class="live ${r.st === "off" ? "off" : r.st === "busy" ? "busy" : ""}"></span><span class="grow trunc">${r.n}</span><span class="mut xs">${r.st === "off" ? "offline" : r.st === "busy" ? "sibuk" : "idle"}</span></div>`).join("")}</div></aside></div>
    </div><aside class="ooc-over ${OS.sel ? "open" : ""}" id="oo-panel" aria-live="polite" aria-label="Detail">${panelHtml()}</aside></div>`;
}

/* ====================== Layar opsi AI Office ====================== */
const OOPT = {
  A: { n: "Isometrik yang dirapikan", d: "Tetap kantor isometrik sesuai spec, tetapi ruangan dipisah dan lebih lega, karakter lebih besar dengan ekspresi sesuai status, papan nama ruangan dan label nama selalu terbaca, dekorasi dikurangi. Panel samping berisi daftar tindakan, ruangan, dan detail karakter.", plus: ["Paling dekat dengan spec dan paling berkesan", "Status terbaca dari wajah, pose, dan label tanpa hover"], minus: ["Paling mahal dibuat dan dirawat (PixiJS)", "Di layar kecil denah perlu digeser"], try: "Coba: klik karakter berlabel kuning, lalu Approve. Klik papan nama untuk fokus ruangan." },
  B: { n: "Denah 2D top-down", d: "Denah kantor dari atas seperti Gather: ruangan per project, ruang rapat, pantry, dan lorong. Karakter bergerak pelan, gelembung menunjukkan apa yang sedang dikerjakan. Klik karakter membuka kartu log di atas denah.", plus: ["Paling mudah dibaca: tidak ada sudut miring", "Terasa hidup tanpa kehilangan kejelasan"], minus: ["Kesan “kantor 3D” berkurang", "Banyak project berarti denah makin panjang"], try: "Coba: klik Agent QA dengan gelembung merah, atau matikan gelembung status." },
  C: { n: "Papan tim", d: "Kartu per agent dan manusia, dikelompokkan per ruangan atau project, dengan status live, tiket, progres, dan tombol approve langsung di kartu. Mini-map kantor tetap ada di samping untuk nuansa ruangan.", plus: ["Paling cepat untuk bertindak, approve dari kartu", "Ringan, mudah dibuat, rapi di ponsel"], minus: ["Suasana kantor yang jadi daya tarik hilang", "Mirip halaman daftar lain"], try: "Coba: Approve dari kartu, filter Status, atau klik titik di mini-map." },
};
AG.screen({ m: "Opsi", g: "Opsi desain", n: "Opsi AI Office", route: "/opsi/office",
  render: () => {
    const o = OS.opt;
    const body = `<div class="oo-root" hidden></div>${AG.head("Opsi tampilan AI Office", "Tiga arah pengganti kantor isometrik yang sekarang. Semua memakai data runner yang sama: klik karakter atau kartu untuk membuka log live, lalu Approve atau Reject.", btn("Buka AI Office", "ghost", "external-link", { "data-go": "#/office" }))}
      <div class="alert ok" style="align-items:center;flex-wrap:wrap;gap:10px 12px;padding:12px 14px">${ic("circle-check", "s18")}<span class="grow" style="min-width:220px"><b>Opsi A “Isometrik yang dirapikan” dipilih</b> (12 Nov 2026). AI Office sudah dibangun ulang dengan tiga gaya seni: vektor lembut, pixel art, dan malam hangat.</span>${btn("Pilih gaya seni", "pri", "palette", { "data-go": "#/opsi/office-gaya" })}${btn("Buka AI Office", "", "building-store", { "data-go": "#/office" })}</div>
      ${optTabs(o, "oo-opt", OOPT)}
      ${optInfo(OOPT[o], o, OS.pick, "oo-pick")}
      ${frame("app.agentgate.id/office", o === "A" ? oA() : o === "B" ? oB() : oC())}
      <div class="row mut sm">${ic("info-circle", "s14")}<span class="grow">Animasi berhenti bila sistem meminta gerak dikurangi (prefers-reduced-motion).</span><button class="link" data-act="oo-reset">Kembalikan data contoh</button></div>`;
    return AG.shell({ crumbs: ["Opsi desain", "AI Office"], body });
  },
  after: (root) => {
    startTick();
    root.querySelectorAll("[data-och][tabindex], [role=button][data-act]").forEach((el) => el.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); AG.ACTIONS[el.dataset.act]?.(el, el.dataset.arg, e); } }));
  } });
})();
