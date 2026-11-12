/* M9 Planning & laporan · M11 Multi-client & integrasi · M12 Rilis */
(() => {
const { ic, av, btn, inp, sel, fld, pill, sw, cb, srow, section, head, empty, stageIcon, prioIcon, typeIcon, labelPill, esc } = AG;
const K = "KSR";

/* ====================== CSS modul (awalan pz-) ====================== */
AG.css(`
.pz-gr260 { grid-template-columns: minmax(0, 1fr) 260px; } .pz-gl300 { grid-template-columns: 300px minmax(0, 1fr); } .pz-gr340 { grid-template-columns: minmax(0, 1fr) 340px; }
@media (max-width: 1099px) { .pz-gr260, .pz-gl300, .pz-gr340 { grid-template-columns: 1fr; } }
.pz-hw .page-head > .grow { flex-basis: 280px; }
.pz-tip { position: fixed; z-index: 95; pointer-events: none; background: var(--fg); color: var(--bg); font-size: 12px; padding: 6px 9px; border-radius: 6px; box-shadow: var(--shadow-md); max-width: 280px; line-height: 1.45; white-space: pre-line; }
.pz-sp { border: 1px solid var(--line); border-radius: 10px; background: var(--raised); min-width: 0; }
.pz-sp.drop { outline: 2px dashed var(--accent); background: var(--accent-soft); }
.pz-sph { display: flex; align-items: center; gap: 12px; padding: 12px 16px; flex-wrap: wrap; }
.pz-sph h2 { font-size: 14px; }
.pz-cap { display: grid; gap: 4px; min-width: 150px; }
.pz-cap .bar { width: 150px; }
.pz-stk { display: flex; height: 6px; border-radius: 3px; overflow: hidden; background: var(--hover); gap: 1px; }
.pz-stk i { display: block; height: 100%; }
.pz-tr { display: grid; grid-template-columns: 14px 16px 16px 62px minmax(0, 1fr) auto 108px 92px 34px 22px 26px; gap: 10px; align-items: center; padding: 0 12px 0 10px; height: 42px; border-top: 1px solid var(--line); min-width: 0; }
.pz-tr:hover { background: var(--muted-bg); }
.pz-tr .grip { color: var(--input); cursor: grab; display: inline-flex; }
.pz-tr.dragging { opacity: 0.4; }
.pz-ai { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; color: var(--accent-fg); white-space: nowrap; }
.pz-ai.diff s { color: var(--faint); }
.pz-pts { font-family: var(--mono); font-size: 12px; text-align: center; border-radius: 5px; background: var(--hover); height: 22px; line-height: 22px; color: var(--fg2); }
.pz-pts.none { background: none; border: 1px dashed var(--input); color: var(--faint); }
.pz-add { display: flex; align-items: center; gap: 8px; height: 38px; padding: 0 16px; color: var(--muted); border-top: 1px solid var(--line); width: 100%; }
.pz-add:hover { color: var(--fg); background: var(--muted-bg); }
@media (max-width: 899px) { .pz-tr { grid-template-columns: 14px 16px 16px 58px minmax(0, 1fr) 30px 22px 30px; } .pz-tr > .hide-sm { display: none; } .pz-cap .bar { width: 110px; } }
.pz-tl { overflow: auto; border: 1px solid var(--line); border-radius: 10px; background: var(--raised); position: relative; max-height: calc(100vh - 230px); }
.pz-tlg { position: relative; }
.pz-tlh { position: sticky; top: 0; z-index: 4; display: flex; background: var(--muted-bg); border-bottom: 1px solid var(--line); }
.pz-lab { position: sticky; left: 0; z-index: 3; width: 280px; flex-shrink: 0; background: var(--raised); border-right: 1px solid var(--line); display: flex; align-items: center; gap: 8px; padding: 0 12px; min-width: 0; }
.pz-tlh .pz-lab { background: var(--muted-bg); z-index: 5; color: var(--muted); font-size: 12px; font-weight: 500; }
.pz-trk { position: relative; flex-shrink: 0; }
.pz-tlr { display: flex; height: 36px; border-bottom: 1px solid var(--line); }
.pz-tlr:hover .pz-lab, .pz-tlr:hover { background: var(--muted-bg); }
.pz-tlr.grp { height: 32px; background: var(--bg-sub); }
.pz-tlr.grp .pz-lab { background: var(--bg-sub); font-weight: 500; color: var(--fg2); }
.pz-bar { position: absolute; top: 8px; height: 20px; border-radius: 5px; display: flex; align-items: center; padding: 0 7px; font-size: 11.5px; font-weight: 500; overflow: hidden; white-space: nowrap; cursor: pointer; border: 1px solid transparent; color: var(--fg); }
.pz-bar > b { position: absolute; left: 0; top: 0; bottom: 0; opacity: 0.85; }
.pz-bar > span { position: relative; }
.pz-bar.est { border-style: dashed; background: transparent !important; color: var(--muted); }
.pz-bar.epic { top: 11px; height: 14px; border-radius: 3px; }
.pz-mk { position: absolute; top: 0; bottom: 0; width: 0; border-left: 1px dashed var(--line2); z-index: 1; pointer-events: none; }
.pz-mk.today { border-left: 2px solid var(--danger); }
.pz-mk.rel { border-left: 1px dashed var(--success); }
.pz-mkl { position: absolute; top: 3px; line-height: 16px; font-size: 11px; padding: 0 5px; border-radius: 4px; white-space: nowrap; transform: translateX(-50%); z-index: 6; }
.pz-mkl.today { background: var(--danger); color: #fff; }
.pz-mkl.rel { background: var(--success-soft); color: var(--success); border: 1px solid var(--success); cursor: pointer; }
.pz-hd1, .pz-hd2 { position: relative; height: 22px; }
.pz-hd1 span, .pz-hd2 span { position: absolute; top: 3px; font-size: 11.5px; color: var(--muted); white-space: nowrap; padding-left: 4px; border-left: 1px solid var(--line); line-height: 16px; }
.pz-hd1 span { color: var(--fg2); font-weight: 500; }
.pz-hd2 span.wk { color: var(--faint); }
.pz-hd2 span.td { color: var(--danger); font-weight: 600; }
.pz-spb { position: absolute; height: 16px; top: 3px; border-radius: 4px; font-size: 11px; padding: 0 6px; background: var(--accent-soft); color: var(--accent-fg); white-space: nowrap; overflow: hidden; line-height: 16px; }
.pz-spb.old { background: var(--hover); color: var(--muted); }
.pz-cal { border: 1px solid var(--line); border-radius: 10px; overflow: hidden; background: var(--raised); }
.pz-calh, .pz-calw { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); }
.pz-calh div { padding: 8px 10px; font-size: 12px; color: var(--muted); background: var(--muted-bg); border-bottom: 1px solid var(--line); font-weight: 500; }
.pz-calw { border-top: 1px solid var(--line); position: relative; }
.pz-calw:nth-child(2) { border-top: 0; }
.pz-cd { min-height: 116px; padding: 44px 6px 6px; border-left: 1px solid var(--line); display: grid; gap: 3px; align-content: start; position: relative; min-width: 0; }
.pz-cd:first-child { border-left: 0; }
.pz-cd.out { background: var(--bg-sub); }
.pz-cd.out .num { color: var(--faint); }
.pz-cd.we { background: var(--bg-sub); }
.pz-cd .num { position: absolute; top: 5px; left: 8px; font-size: 12px; color: var(--muted); font-variant-numeric: tabular-nums; }
.pz-cd.today .num { background: var(--danger); color: #fff; border-radius: 10px; padding: 0 6px; font-weight: 600; }
.pz-band { position: absolute; top: 26px; height: 14px; font-size: 10.5px; line-height: 14px; padding: 0 6px; border-radius: 3px; background: var(--accent-soft); color: var(--accent-fg); white-space: nowrap; overflow: hidden; z-index: 1; }
.pz-band.nx { background: var(--hover); color: var(--muted); }
.pz-ev { display: flex; align-items: center; gap: 4px; font-size: 11.5px; padding: 2px 5px; border-radius: 4px; background: var(--hover); color: var(--fg2); min-width: 0; cursor: pointer; border: 1px solid transparent; }
.pz-ev:hover { border-color: var(--input); }
.pz-ev.est { background: none; border: 1px dashed var(--input); color: var(--muted); }
.pz-ev.late { background: var(--danger-soft); color: var(--danger); }
.pz-ev.rel { background: var(--success-soft); color: var(--success); }
.pz-ev .ic { width: 12px; height: 12px; }
.pz-ev .mono { white-space: nowrap; flex-shrink: 0; font-size: 11px; }
.pz-agenda { display: grid; }
.pz-ag { display: grid; grid-template-columns: 48px minmax(0, 1fr); gap: 12px; padding: 10px 0; border-top: 1px solid var(--line); }
.pz-ag:first-child { border-top: 0; }
.pz-ag .d { text-align: center; line-height: 1.1; } .pz-ag .d b { font-size: 18px; display: block; } .pz-ag .d span { font-size: 11px; color: var(--muted); }
.pz-cw { overflow-x: auto; }
@media (max-width: 899px) { .pz-cw .pz-chart { min-width: 600px; } }
.pz-chart { width: 100%; height: auto; display: block; color: var(--muted); }
.pz-chart text { font-size: 11px; fill: currentColor; font-family: var(--font); }
.pz-chart .gl { stroke: var(--line); stroke-width: 1; }
.pz-chart .hit { fill: transparent; }
.pz-chart .hit:hover { fill: var(--fg); fill-opacity: 0.04; }
.pz-leg { display: flex; gap: 14px; flex-wrap: wrap; font-size: 12px; color: var(--muted); }
.pz-leg span { display: inline-flex; align-items: center; gap: 6px; }
.pz-leg i { width: 10px; height: 10px; border-radius: 2px; display: inline-block; }
.pz-leg i.ln { height: 2px; width: 14px; }
.pz-leg i.dash { height: 0; width: 14px; border-top: 2px dashed; background: none !important; }
.pz-hb { display: grid; grid-template-columns: 120px minmax(0, 1fr) 92px; gap: 12px; align-items: center; padding: 7px 0; }
.pz-hb .tr { display: flex; height: 16px; border-radius: 4px; overflow: hidden; background: var(--hover); }
.pz-hb .tr i { display: block; height: 100%; }
@media (max-width: 899px) { .pz-hb { grid-template-columns: 92px minmax(0, 1fr) 70px; gap: 8px; } }
.pz-srch { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 24px; align-items: start; }
@media (max-width: 899px) { .pz-srch { grid-template-columns: 1fr; } .pz-sf-wrap { order: 2; } }
.pz-sf .mlab { padding-left: 8px; margin-top: 8px; }
.pz-sfi { display: flex; align-items: center; gap: 8px; height: 30px; padding: 0 8px; border-radius: 6px; color: var(--fg2); width: 100%; min-width: 0; }
.pz-sfi:hover { background: var(--hover); }
.pz-sfi.on { background: var(--active); color: var(--fg); font-weight: 500; }
.pz-sfi .ic { color: var(--faint); }
.pz-sfi .n { margin-left: auto; font-size: 11.5px; color: var(--faint); }
.pz-qbox { position: relative; }
.pz-qrow { display: flex; gap: 8px; }
.pz-q { font-family: var(--mono) !important; font-size: 12.5px !important; height: 36px; }
.pz-sug { position: absolute; left: 0; top: 40px; z-index: 30; width: min(420px, 100%); background: var(--raised); border: 1px solid var(--line2); border-radius: 8px; box-shadow: var(--shadow-md); padding: 4px; display: none; }
.pz-sug.on { display: block; }
.pz-sug .mi .tail { font-family: var(--font); }
.pz-sug .mi code { background: none; border: 0; padding: 0; color: var(--fg); }
.pz-qerr { color: var(--danger); font-size: 12px; display: flex; gap: 6px; align-items: center; }
.pz-bulk { display: flex; align-items: center; gap: 8px; padding: 8px 10px 8px 14px; border-radius: 8px; background: var(--fg); color: var(--bg); flex-wrap: wrap; }
.pz-bulk .btn { background: transparent; color: var(--bg); border-color: color-mix(in srgb, var(--bg) 25%, transparent); box-shadow: none; }
.pz-bulk .btn:hover { background: color-mix(in srgb, var(--bg) 12%, transparent); color: var(--bg); }
.pz-steps { list-style: none; margin: 0; padding: 0; display: flex; gap: 4px; overflow-x: auto; scrollbar-width: none; }
.pz-steps li { display: flex; align-items: center; gap: 4px; flex-shrink: 0; }
.pz-steps li + li::before { content: ""; width: 18px; height: 1px; background: var(--line2); }
.pz-st { display: inline-flex; align-items: center; gap: 8px; height: 32px; padding: 0 10px 0 6px; border-radius: 7px; color: var(--muted); font-weight: 500; white-space: nowrap; }
.pz-st:hover { background: var(--hover); color: var(--fg); }
.pz-st.now { background: var(--accent-soft); color: var(--accent-fg); }
.pz-st .step { background: var(--hover); color: var(--muted); }
.pz-st.now .step { background: var(--accent); color: #fff; }
.pz-st .step.done { background: var(--success); color: #fff; }
.pz-st .step .ic { width: 12px; height: 12px; stroke-width: 2.6; }
.pz-opt { display: flex; gap: 12px; align-items: flex-start; padding: 14px 16px; border: 1px solid var(--line2); border-radius: 10px; cursor: pointer; background: var(--raised); }
.pz-opt:hover { border-color: var(--input); }
.pz-opt.on { border-color: var(--accent); box-shadow: var(--ring); }
.pz-opt .radio { margin-top: 2px; }
.pz-map { display: grid; grid-template-columns: minmax(0, 1fr) 24px minmax(0, 1fr); gap: 10px 8px; align-items: center; padding: 14px 16px; }
.pz-map .arw { color: var(--faint); display: inline-flex; justify-content: center; }
.pz-map .jh { font-size: 12px; color: var(--muted); font-weight: 500; }
.pz-jst { display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: 5px; background: var(--hover); font-size: 12px; font-weight: 600; letter-spacing: 0.01em; color: var(--fg2); text-transform: uppercase; }
.pz-drop { border: 1.5px dashed var(--input); border-radius: 10px; padding: 28px; display: grid; justify-items: center; gap: 8px; text-align: center; color: var(--muted); background: var(--muted-bg); cursor: pointer; }
.pz-drop:hover { border-color: var(--accent); color: var(--fg); }
.pz-log { font-family: var(--mono); font-size: 12px; }
.pz-log .boxfoot { font-family: var(--font); font-size: 13px; }
.pz-log .lrow { min-height: 38px; padding: 6px 16px; gap: 10px; }
.pz-tok { display: flex; align-items: center; gap: 8px; font-family: var(--mono); font-size: 13px; background: var(--code-bg); border: 1px solid var(--line); border-radius: 8px; padding: 10px 12px; word-break: break-all; }
.pz-up { display: flex; gap: 2px; height: 20px; align-items: stretch; }
.pz-up i { flex: 1; border-radius: 2px; background: var(--success); opacity: 0.55; min-width: 2px; }
.pz-up i:hover { opacity: 1; }
.pz-up i.w { background: var(--warn); } .pz-up i.d { background: var(--danger); }
.pz-sys { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px 16px; padding: 14px 16px; border-top: 1px solid var(--line); align-items: center; }
.pz-sys:first-child { border-top: 0; }
.pz-sys .meta { display: flex; gap: 14px; flex-wrap: wrap; color: var(--muted); font-size: 12px; grid-column: 1 / -1; }
.pz-sys .pz-up { grid-column: 1 / -1; }
.pz-hero { display: flex; gap: 14px; align-items: center; padding: 16px; border-radius: 10px; background: var(--success-soft); color: var(--success); }
.pz-hero.warn { background: var(--warn-soft); color: var(--warn); }
.pz-hero b { font-size: 15px; color: var(--fg); }
.pz-inst { min-height: 100vh; min-height: 100dvh; background: var(--page); }
.pz-ihd { display: flex; align-items: center; gap: 16px; padding: 14px 24px; border-bottom: 1px solid var(--line); background: var(--raised); position: sticky; top: 0; z-index: 10; }
.pz-iin { max-width: 760px; margin: 0 auto; padding: 48px 16px 80px; display: grid; gap: 28px; }
.pz-iin h1 { font-size: 28px; letter-spacing: -0.02em; line-height: 1.2; }
.pz-cmd { display: flex; align-items: center; gap: 10px; background: #111116; color: #e5e5ee; border-radius: 10px; padding: 6px 6px 6px 16px; font-family: var(--mono); font-size: 13px; min-width: 0; }
.pz-cmd code { background: none; border: 0; padding: 0; color: inherit; flex: 1; min-width: 0; overflow-x: auto; white-space: nowrap; scrollbar-width: none; }
.pz-cmd .p { color: #7c7c8a; user-select: none; }
.pz-cmd .btn { background: #26262e; border-color: #33333d; color: #e5e5ee; box-shadow: none; }
.pz-cmd .btn:hover { background: #33333d; color: #fff; }
.pz-sha { font-family: var(--mono); font-size: 11.5px; color: var(--muted); word-break: break-all; }
.pz-nb { display: grid; grid-template-columns: 22px minmax(0, 1fr); gap: 12px; align-items: start; }
.pz-w220 { width: 220px; max-width: 100%; display: inline-flex; } .pz-w220 > .combo { width: 100%; }
.pz-w180 { width: 180px; max-width: 100%; display: inline-flex; } .pz-w180 > .combo { width: 100%; }
.pz-mon { display: inline-flex; align-items: center; gap: 4px; }
.pz-mon b { min-width: 112px; text-align: center; font-weight: 600; }
.pz-tbar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; min-width: 0; }
.pz-cell { display: grid; gap: 2px; min-width: 0; }
.pz-cell small { color: var(--muted); font-size: 12px; }
.pz-sp-wrap .sp-body > .sp-panel { gap: 20px; }
.pz-env { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
@media (max-width: 599px) { .pz-env { grid-template-columns: 1fr; } }
.pz-ib { display: grid; gap: 12px; }
.pz-kv { display: grid; grid-template-columns: 150px minmax(0, 1fr); gap: 8px 16px; padding: 14px 16px; }
.pz-kv dt { color: var(--muted); } .pz-kv dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
@container (max-width: 480px) { .pz-kv { grid-template-columns: 1fr; gap: 2px; } .pz-kv dd { margin-bottom: 8px; } }
.pz-src .lrow { min-height: 40px; }
.pz-ed { min-width: 0; } .pz-ed .ed .foot { flex-wrap: wrap; row-gap: 4px; }
.pz-prose blockquote { margin: 0 0 8px; padding: 2px 12px; border-left: 3px solid var(--line2); color: var(--muted); }
/* v3: sub-bagian di dalam satu kartu panel */
.pz-sub { display: grid; gap: 10px; min-width: 0; align-content: start; }
.cx-b > .pz-sub + .pz-sub { border-top: 1px solid var(--line); padding-top: 16px; margin-top: 4px; }
.pz-subh { display: flex; align-items: flex-start; gap: 12px; }
.pz-subh h3 { font-size: 13px; font-weight: 600; }
.pz-subh p { color: var(--muted); font-size: 13px; margin-top: 2px; }
.pz-sub > .box { box-shadow: none; }
.cardx .pz-tl { border: 0; border-radius: 0 0 10px 10px; }
.cardx .pz-cal { border: 0; border-radius: 0 0 10px 10px; }
.pz-sf-wrap { position: sticky; top: 112px; }
.pz-sf .cx-b { padding: 6px; gap: 2px; }
@media (max-width: 899px) { .pz-sf-wrap { position: static; } }
.cx-a .pz-leg { justify-content: flex-end; }
.pz-foot { padding: 12px 16px; }
.cx-b > * + .pz-sub:not(.pz-sub + .pz-sub) { border-top: 1px solid var(--line); padding-top: 16px; margin-top: 4px; }
.cx-b > .alert + .pz-sub { border-top: 0 !important; padding-top: 0; margin-top: 0; }
.cx-b > .alert + .box { border-top: 1px solid var(--line); margin-top: 4px; }
.pz-sp-wrap .cx-b > * { min-width: 0; }
.pz-stepw { margin: -6px -16px 0; padding: 0 10px 10px; border-bottom: 1px solid var(--line); }
@container (max-width: 620px) { .pz-rcard .cx-h { flex-wrap: wrap; } .pz-rcard .cx-a { flex-basis: 100%; flex-shrink: 1; } .pz-rcard .cx-a .pz-leg { justify-content: flex-start; } }
`);

/* ====================== Tooltip sederhana untuk [data-tip] ====================== */
let tipEl = null;
document.addEventListener("mouseover", (e) => {
  const t = e.target.closest?.("[data-tip]");
  if (!t) { tipEl?.remove(); tipEl = null; return; }
  if (!tipEl) { tipEl = document.createElement("div"); tipEl.className = "pz-tip"; document.body.appendChild(tipEl); }
  tipEl.textContent = t.dataset.tip;
});
document.addEventListener("mousemove", (e) => {
  if (!tipEl) return;
  const w = tipEl.offsetWidth, h = tipEl.offsetHeight;
  let x = e.clientX + 12, y = e.clientY + 14;
  if (x + w > innerWidth - 8) x = e.clientX - w - 12;
  if (y + h > innerHeight - 8) y = e.clientY - h - 12;
  tipEl.style.left = x + "px"; tipEl.style.top = y + "px";
});
window.addEventListener("hashchange", () => { tipEl?.remove(); tipEl = null; });

/* ====================== Data modul ====================== */
const PZ = (AG.PZ = {
  sp: {}, order: [],
  zoom: "month",
  q: 'project = KSR AND sprint = "Sprint 4" AND stage != Selesai ORDER BY priority',
  filter: 0, sel: new Set(),
  step: { jira: 2, csv: 1 },
  jiraEnv: "cloud", jiraMode: "two",
  sub: {}, jdirty: false,
  filters: [
    { n: "Sprint aktif", q: 'project = KSR AND sprint = "Sprint 4" AND stage != Selesai ORDER BY priority', i: "target", own: "Anda" },
    { n: "Review saya", q: 'project = KSR AND stage = "Code Review" AND assignee = currentUser()', i: "eye", own: "Anda" },
    { n: "Butuh keputusan", q: 'status = "Butuh keputusan"', i: "alert-triangle", own: "Tim Kasir" },
    { n: "Bug terbuka", q: "type = Bug AND stage != Selesai", i: "bug", own: "Tim Kasir" },
    { n: "Pembayaran", q: "label = Pembayaran", i: "tag", own: "Workspace" },
    { n: "Belum diestimasi", q: "project = KSR AND points is EMPTY", i: "sparkles", own: "Anda" },
  ],
});
const CAP = 24;
const SPRINTS = {
  s4: { n: "Sprint 4", from: "4 Nov", to: "17 Nov", goal: "Pembayaran QRIS dan struk siap dipakai toko percontohan", active: true },
  s5: { n: "Sprint 5", from: "18 Nov", to: "1 Des", goal: "Mode offline dan laporan harian" },
};
const EST = {
  "KSR-24": { ai: 8, d: [14, 6] }, "KSR-26": { ai: 3, d: [8, 3], by: "Dewi Dev" }, "KSR-27": { ai: 3, d: [4, 1] },
  "KSR-30": { ai: 2, d: [2, 2] }, "KSR-29": { ai: 3, d: [5, 2] }, "KSR-28": { ai: 5, d: [6, 4] },
  "KSR-33": { ai: 8, d: [30, 10], by: "Olivia Owner" }, "KSR-35": { ai: 2, d: [2, 1] }, "KSR-31": { ai: null },
  "KSR-36": { ai: 5, d: [6, 2] }, "KSR-37": { ai: 2, d: [3, 1] }, "KSR-38": { ai: 8, d: [16, 6] },
};
AG.TICKETS.forEach((t) => {
  if (t.type === "epic" || t.type === "subtask" || t.stage === "done") return;
  const s = t.sprint === "Sprint 4" ? "s4" : ["KSR-33", "KSR-35", "KSR-31"].includes(t.k) ? "s5" : "bl";
  PZ.sp[t.k] = s; PZ.order.push(t.k);
});
const T = (k) => AG.ticket(k);
const pts = (keys) => keys.reduce((a, k) => a + (T(k).pts || 0), 0);
const inSp = (s) => PZ.order.filter((k) => PZ.sp[k] === s);
const fmt = (n) => String(n).replace(".", ",");

/* Komponen kecil */
const stkBar = (keys) => {
  const by = {}; keys.forEach((k) => { const t = T(k); by[t.stage] = (by[t.stage] || 0) + (t.pts || 1); });
  const tot = Object.values(by).reduce((a, b) => a + b, 0) || 1;
  return `<div class="pz-stk" style="width:100%">${AG.STAGES.filter((s) => by[s.k]).map((s) => `<i style="width:${(by[s.k] / tot) * 100}%;background:${s.c}" data-tip="${s.n}: ${by[s.k]} poin"></i>`).join("")}</div>`;
};
const stepper = (key, steps) => {
  const cur = PZ.step[key];
  return `<ol class="pz-steps" aria-label="Langkah">${steps.map((s, i) => `<li><button type="button" class="pz-st ${i === cur ? "now" : ""}" data-act="pz-step" data-arg="${key}|${i}" ${i === cur ? 'aria-current="step"' : ""}><span class="step ${i < cur ? "done" : ""}">${i < cur ? ic("check") : i + 1}</span>${s}</button></li>`).join("")}</ol>`;
};
const stepNav = (key, n, last) => {
  const cur = PZ.step[key];
  return `<div class="row between">${cur > 0 ? btn("Kembali", "", "chevron-left", { "data-act": "pz-step", "data-arg": `${key}|${cur - 1}` }) : "<span></span>"}${cur < n - 1 ? btn("Lanjut", "pri", "", { "data-act": "pz-step", "data-arg": `${key}|${cur + 1}` }) : last}</div>`;
};
AG.action("pz-step", (el, arg) => { const [k, i] = arg.split("|"); PZ.step[k] = +i; AG.rerender(); });
AG.action("pz-copy", (el, arg) => { try { navigator.clipboard?.writeText(arg).catch(() => {}); } catch {} AG.toast("Disalin ke clipboard"); });

/* ====================== v2: helper bersama modul ====================== */
/* v3: sub-bagian (judul kecil) di dalam satu kartu panel, dan kartu panel subpage */
const sub = (t, d, body, act = "") => `<div class="pz-sub">${t || act ? `<div class="pz-subh"><div class="grow"><h3>${t}</h3>${d ? `<p>${d}</p>` : ""}</div>${act}</div>` : ""}${body}</div>`;
const card = (title, desc, body, foot = "", action = "", pad = true) => AG.card({ title, desc, body, foot, action, pad });
/* Sub-halaman dengan menu kiri; panel aktif diingat per halaman supaya tidak kembali ke awal saat render ulang. */
const subpage = (key, def, o) => {
  const active = o.nav.some((n) => n.k === PZ.sub[key]) ? PZ.sub[key] : def;
  return `<div class="pz-sp-wrap" data-pz-sp="${key}">${AG.subpage({ title: o.title, desc: o.desc, head: o.head || "", nav: o.nav, active, body: o.panels.map(([k, h]) => AG.panel(k, h, k === active)).join("") })}</div>`;
};
document.addEventListener("click", (e) => {
  const a = e.target.closest?.("[data-pz-sp] [data-tab]"); if (!a) return;
  if (a.matches('a[href="#"]')) e.preventDefault();
  PZ.sub[a.closest("[data-pz-sp]").dataset.pzSp] = a.dataset.tab;
  scrollTo({ top: 0 });
}, true);
/* Pencarian bar filter di layar non-tabel: render ulang setelah berhenti mengetik, fokus dikembalikan. */
const LIVE = new Set(["pz-bl", "pz-rm", "pz-cal"]);
let liveT = 0;
document.addEventListener("input", (e) => {
  const s = e.target.closest?.("[data-fsearch]"); if (!s || !LIVE.has(s.dataset.fsearch)) return;
  const id = s.dataset.fsearch; clearTimeout(liveT);
  liveT = setTimeout(() => { AG.rerender(); const n = document.querySelector(`[data-fsearch="${id}"]`); if (n) { n.focus(); n.setSelectionRange(n.value.length, n.value.length); } }, 250);
});
/* Combo yang membawa aksi: bungkus dengan data-pz-cc="nama". */
const CC = {};
document.addEventListener("combo-change", (e) => { const w = e.target.closest?.("[data-pz-cc]"); if (w && CC[w.dataset.pzCc]) CC[w.dataset.pzCc](e.detail, w); });
const ccWrap = (name, html, cls = "pz-w220") => `<span class="${cls}" data-pz-cc="${name}">${html}</span>`;
/* Pilihan orang untuk AG.combo. */
const peopleOpts = (extra = []) => [...extra, ...Object.entries(AG.PEOPLE).map(([k, p]) => ({ v: k, l: p.name, sub: p.job || p.role, lead: av(k, 18) }))];
const stageOpts = (extra = []) => [...extra, ...AG.STAGES.map((s) => ({ v: s.k, l: s.n, lead: stageIcon(s.k) }))];
/* Filter tiket bersama (backlog, roadmap, kalender). */
const TF = () => [
  { k: "type", label: "Tipe", icon: "category-2", options: ["Story", "Task", "Bug", "Epic"] },
  { k: "who", label: "Penerima", icon: "user", options: [...Object.values(AG.PEOPLE).map((p) => p.name), "Tanpa penerima"] },
  { k: "label", label: "Label", icon: "tag", options: Object.keys(AG.LABELS) },
  { k: "prio", label: "Prioritas", icon: "antenna-bars-5", options: Object.values(AG.PRIORITY).map((p) => p.n) },
  { k: "stage", label: "Stage", icon: "route", options: AG.STAGES.map((s) => s.n) },
];
const tPass = (id, t) => {
  const q = AG.filterQuery(id).trim(); if (q && !`${t.k} ${t.t}`.toLowerCase().includes(q)) return false;
  const v = (k) => AG.filterValue(id, k);
  if (v("type").length && !v("type").includes(AG.TYPES[t.type].n)) return false;
  if (v("who").length && !v("who").includes(t.who ? AG.PEOPLE[t.who].name : "Tanpa penerima")) return false;
  if (v("label").length && !t.lbl.some((l) => v("label").includes(l))) return false;
  if (v("prio").length && !v("prio").includes(AG.PRIORITY[t.p].n)) return false;
  if (v("stage").length && !v("stage").includes(AG.stage(t.stage).n)) return false;
  return true;
};
const anyFilter = (id) => !!AG.filterQuery(id).trim() || (AG._filters[id]?.filters || []).some((f) => f.value.length);
const dtSel = (id) => AG._dts[id]?.sel || new Set();
const dtReset = (id) => { const d = AG._dts[id]; if (d) { d.sel.clear(); d.page = 1; } };

/* ====================== Grafik SVG ====================== */
const H = 250, PL = 36, PR = 14, PT = 14, PB = 28;
function frame(n, yMax, yStep, labels, W = 680) {
  const unit = "";
  const x = (i) => PL + (n === 1 ? 0 : (i * (W - PL - PR)) / (n - 1));
  const y = (v) => PT + (H - PT - PB) * (1 - v / yMax);
  let g = "";
  for (let v = 0; v <= yMax; v += yStep) g += `<line class="gl" x1="${PL}" x2="${W - PR}" y1="${y(v)}" y2="${y(v)}"/><text x="${PL - 8}" y="${y(v) + 4}" text-anchor="end">${v}${unit}</text>`;
  labels.forEach((l, i) => { if (l !== "") g += `<text x="${x(i)}" y="${H - 8}" text-anchor="middle">${l}</text>`; });
  return { x, y, g, W };
}
const path = (pts) => pts.map(([a, b], i) => `${i ? "L" : "M"}${a.toFixed(1)},${b.toFixed(1)}`).join("");
/* series: { n, c, v: [angka|null], dash, area, w } ; tips: teks per indeks x */
function lineChart({ labels, series, yMax, yStep, tips, marks = [], w = 680 }) {
  const f = frame(labels.length, yMax, yStep, labels, w), W = w;
  const colW = (W - PL - PR) / (labels.length - 1);
  let s = "";
  marks.forEach((m) => { s += `<line x1="${f.x(m.i)}" x2="${f.x(m.i)}" y1="${PT}" y2="${H - PB}" style="stroke:${m.c};stroke-width:1.5" stroke-dasharray="${m.dash ? "4 3" : ""}"/><text x="${f.x(m.i) + 4}" y="${PT + 10}" style="fill:${m.c}">${m.l}</text>`; });
  series.forEach((se) => {
    const p = se.v.map((v, i) => (v == null ? null : [f.x(i), f.y(v)])).filter(Boolean);
    if (se.area) s += `<path d="${path(p)}L${p[p.length - 1][0]},${f.y(0)}L${p[0][0]},${f.y(0)}Z" style="fill:${se.c};fill-opacity:0.1"/>`;
    s += `<path d="${path(p)}" style="fill:none;stroke:${se.c};stroke-width:${se.w || 2}" stroke-dasharray="${se.dash ? "5 4" : ""}" stroke-linejoin="round"/>`;
    if (!se.dash) p.forEach(([a, b]) => (s += `<circle cx="${a}" cy="${b}" r="2.6" style="fill:var(--raised);stroke:${se.c};stroke-width:1.6"/>`));
  });
  labels.forEach((l, i) => { s += `<rect class="hit" x="${f.x(i) - colW / 2}" y="${PT}" width="${colW}" height="${H - PT - PB}" data-tip="${esc(tips[i])}"/>`; });
  return `<div class="pz-cw"><svg class="pz-chart" viewBox="0 0 ${W} ${H}" role="img">${f.g}${s}</svg></div>`;
}
const legend = (items) => `<div class="pz-leg">${items.map(([n, c, k]) => `<span><i class="${k || ""}" style="background:${c};border-color:${c}"></i>${n}</span>`).join("")}</div>`;


/* ====================== M9 · Backlog ====================== */
function trow(k) {
  const t = T(k), e = EST[k] || {}, st = AG.stage(t.stage);
  const aiCell = e.ai == null ? `<span class="pz-ai" data-tip="Brainstorming belum selesai, AI belum bisa mengusulkan">${ic("sparkles", "s14")}—</span>`
    : e.ai === t.pts ? `<span class="pz-ai" data-tip="Usulan AI ${e.ai} poin diterima\nDurasi: ${e.d[0]} jam AI + ${e.d[1]} jam manusia">${ic("sparkles", "s14")}AI ${e.ai}</span>`
    : `<span class="pz-ai diff" data-tip="AI mengusulkan ${e.ai} poin, diubah ${e.by} menjadi ${t.pts}\nDurasi: ${e.d[0]} jam AI + ${e.d[1]} jam manusia">${ic("sparkles", "s14")}<s>AI ${e.ai}</s>${ic("user", "s14")}</span>`;
  return `<div class="pz-tr" draggable="true" data-pz-drag="${k}" data-go="#/t/${k}">
    <span class="grip" aria-hidden="true">${ic("grip-vertical", "s14")}</span>${prioIcon(t.p)}${typeIcon(t.type)}<span class="mono mut">${k}</span>
    <span class="trunc">${t.t}</span>
    <span class="row g4 hide-sm">${t.lbl.map(labelPill).join("")}</span>
    <span class="row g4 hide-sm sm mut trunc">${stageIcon(t.stage)}<span class="trunc">${st.n}</span></span>
    <span class="hide-sm">${aiCell}</span>
    ${t.pts ? `<span class="pz-pts" data-tip="Story point">${t.pts}</span>` : `<button class="pz-pts none" data-toast="AI mengestimasi KSR-31 setelah brainstorming selesai" aria-label="Minta estimasi">?</button>`}
    ${av(t.who, 22)}
    ${btn("", "ghost icon sm", "dots", { "data-menu": "pz-move", "data-arg": k, "aria-label": `Pindahkan ${k}` })}
  </div>`;
}
const blShow = (keys) => keys.filter((k) => tPass("pz-bl", T(k)));
const noMatch = `<div style="padding:0 16px 14px">${empty("filter", "Tidak ada tiket yang cocok", "Ubah kata kunci atau hapus filter.")}</div>`;
function sprintBlock(s) {
  const sp = SPRINTS[s], keys = inSp(s), shown = blShow(keys), p = pts(keys), over = p > CAP;
  const unest = keys.filter((k) => !T(k).pts).length;
  const ctl = sp.active
    ? btn("Selesaikan sprint", "", "flag", { "data-dialog": "complete-sprint" })
    : btn("Mulai sprint", "pri", "player-play", { "data-dialog": "start-sprint" });
  return `<section class="pz-sp" data-pz-drop="${s}" aria-label="${sp.n}">
    <div class="pz-sph">
      <div class="grow col g4" style="min-width:220px">
        <div class="row wrap"><h2>${sp.n}</h2>${sp.active ? pill("Aktif", "acc", "", "player-play") : pill("Berikutnya", "gray")}<span class="mut">${sp.from} – ${sp.to}</span>${sp.active ? `<span class="mut">· sisa 5 hari (3 hari kerja)</span>` : ""}</div>
        <span class="mut sm row g4">${ic("target", "s14")}${sp.goal}</span>
      </div>
      <div class="pz-cap">
        <div class="row between sm"><span class="mut">${keys.length} tiket</span><span class="tab-num ${over ? "text-warn b6" : "b5"}" data-tip="Kapasitas dihitung dari velocity 3 sprint terakhir (rata-rata 18,3) ditambah throughput agent">${p} / ${CAP} poin</span></div>
        ${sp.active ? stkBar(keys) : `<div class="bar ${over ? "warn" : ""}"><i style="width:${Math.min(100, (p / CAP) * 100)}%"></i></div>`}
      </div>
      ${ctl}${btn("", "ghost icon", "dots", { "data-menu": "pz-sprint", "data-arg": s, "aria-label": `Opsi ${sp.n}` })}
    </div>
    ${over ? `<div class="alert warn" style="margin:0 16px 12px">${ic("alert-triangle")}<span>Melebihi kapasitas ${p - CAP} poin. AI menyarankan memindahkan <b>KSR-27</b> (3 poin) ke Sprint 5.</span></div>` : ""}
    ${unest ? `<div class="alert gray" style="margin:0 16px 12px">${ic("sparkles")}<span>${unest} tiket belum punya estimasi. AI akan mengusulkan setelah brainstorming selesai.</span></div>` : ""}
    ${shown.map(trow).join("") || (keys.length ? noMatch : `<div style="padding:0 16px 14px">${empty("stack-2", "Sprint masih kosong", "Seret tiket dari backlog ke sini.")}</div>`)}
    <button class="pz-add" data-dialog="new-ticket">${ic("plus", "s14")}Buat tiket</button>
  </section>`;
}
AG.screen({ m: "M9", g: "Planning", n: "Backlog & sprint", route: "/p/:k/backlog", path: "/p/KSR/backlog",
  render: () => {
    const bl = inSp("bl"), blS = blShow(bl), all = PZ.order.length, shown = blShow(PZ.order).length;
    return AG.shell({ project: K, crumbs: AG.projectCrumb(K, "Backlog"), ...AG.projectTabs(K),
      actions: btn("Sprint baru", "", "plus", { "data-toast": "Sprint 6 dibuat (2 Des – 15 Des)" }),
      body: `${head("Backlog", "Rencanakan sprint. Seret tiket antar sprint atau pakai menu di ujung baris. Angka AI adalah usulan story point, angka di kotak adalah yang disepakati.")}
      ${AG.stats([
        { label: "Sprint 4 selesai", value: `0 <span class="mut" style="font-size:13px;font-weight:500">/ 26 poin</span>`, icon: "flag", tone: "acc", hint: "13 poin di Review & Testing" },
        { label: "Velocity rata-rata", value: "18,3", icon: "chart-line", hint: "3 sprint terakhir" },
        { label: "Akurasi estimasi AI", value: "84%", icon: "sparkles", tone: "ok", hint: "Lihat kalibrasi", go: "#/p/KSR/reports" },
        { label: "Perkiraan akhir Sprint 4", value: "3 poin sisa", icon: "alert-triangle", tone: "warn", hint: "KSR-27 berisiko tertunda" },
      ])}
      ${AG.filterBar({ id: "pz-bl", search: "Cari tiket di sprint dan backlog…", filters: TF(), count: anyFilter("pz-bl") ? `${shown} dari ${all} tiket` : `${all} tiket` })}
      ${sprintBlock("s4")}${sprintBlock("s5")}
      <section class="pz-sp" data-pz-drop="bl" aria-label="Backlog">
        <div class="pz-sph"><div class="grow row"><h2>Backlog</h2><span class="mut">${bl.length} tiket · ${pts(bl)} poin</span></div>${btn("Urutkan dengan AI", "ghost sm", "sparkles", { "data-toast": "AI mengurutkan backlog berdasarkan prioritas dan dependensi" })}</div>
        ${blS.map(trow).join("") || (bl.length ? noMatch : `<div style="padding:0 16px 14px">${empty("stack-2", "Backlog kosong", "Semua tiket sudah masuk sprint.")}</div>`)}
        <button class="pz-add" data-dialog="new-ticket">${ic("plus", "s14")}Buat tiket</button>
      </section>` });
  },
  after: (root) => {
    let drag = null;
    root.querySelectorAll("[data-pz-drag]").forEach((r) => {
      r.addEventListener("dragstart", (e) => { drag = r.dataset.pzDrag; r.classList.add("dragging"); e.dataTransfer.setData("text/plain", drag); });
      r.addEventListener("dragend", () => { r.classList.remove("dragging"); root.querySelectorAll(".pz-sp.drop").forEach((x) => x.classList.remove("drop")); });
    });
    root.querySelectorAll("[data-pz-drop]").forEach((z) => {
      z.addEventListener("dragover", (e) => { e.preventDefault(); z.classList.add("drop"); });
      z.addEventListener("dragleave", (e) => { if (!z.contains(e.relatedTarget)) z.classList.remove("drop"); });
      z.addEventListener("drop", (e) => { e.preventDefault(); if (drag) moveTicket(drag, z.dataset.pzDrop); });
    });
  } });
const spName = (s) => (s === "bl" ? "Backlog" : SPRINTS[s].n);
function moveTicket(k, to) {
  if (PZ.sp[k] === to) return AG.rerender();
  PZ.sp[k] = to; AG.rerender(); AG.toast(`${k} dipindah ke ${spName(to)}`);
}
AG.action("pz-move", (el, arg) => { const [k, to] = arg.split("|"); if (to === "top" || to === "bottom") { PZ.order = PZ.order.filter((x) => x !== k); to === "top" ? PZ.order.unshift(k) : PZ.order.push(k); AG.rerender(); AG.toast(`${k} dipindah ke ${to === "top" ? "atas" : "bawah"}`); } else moveTicket(k, to); });
AG.menu("pz-move", (k) => `<div class="mlab">Pindahkan ${k} ke</div>
  ${["s4", "s5", "bl"].map((s) => `<button class="mi" data-act="pz-move" data-arg="${k}|${s}">${ic(s === "bl" ? "stack-2" : s === "s4" ? "player-play" : "calendar")}${spName(s)}${PZ.sp[k] === s ? `<span class="tail">${ic("check", "s14")}</span>` : ""}</button>`).join("")}
  <div class="msep"></div>
  <button class="mi" data-act="pz-move" data-arg="${k}|top">${ic("arrow-forward-up")}Ke paling atas</button>
  <button class="mi" data-act="pz-move" data-arg="${k}|bottom">${ic("arrow-back-up")}Ke paling bawah</button>
  <div class="msep"></div>
  <a class="mi" href="#/t/${k}">${ic("external-link")}Buka tiket</a>
  <button class="mi" data-toast="AI mengestimasi ulang ${k}">${ic("sparkles")}Estimasi ulang dengan AI</button>`);
AG.menu("pz-sprint", (s) => `<button class="mi" data-toast="Mode edit ${SPRINTS[s].n}">${ic("pencil")}Edit nama, tujuan, tanggal</button>
  <button class="mi" data-toast="Kapasitas ${SPRINTS[s].n} diatur per anggota">${ic("users")}Atur kapasitas</button>
  <a class="mi" href="#/p/KSR/reports">${ic("chart-line")}Laporan sprint</a>
  <div class="msep"></div><button class="mi dng" data-toast="${SPRINTS[s].n} dihapus, tiket kembali ke backlog" data-tone="dng">${ic("trash")}Hapus sprint</button>`);

AG.dialog("start-sprint", () => {
  const keys = inSp("s5"), p = pts(keys);
  return { title: "Mulai Sprint 5", desc: `${keys.length} tiket · ${p} dari ${CAP} poin kapasitas`, icon: "player-play", tone: "acc", w: "w560",
    body: `<div class="alert warn">${ic("alert-triangle")}<span>Sprint 4 masih aktif sampai 17 Nov. Sprint 5 akan berjalan paralel, atau selesaikan Sprint 4 dulu.</span></div>
      ${fld("Nama sprint", inp("Sprint 5"))}
      ${fld("Tujuan sprint", inp("Mode offline dan laporan harian", { area: true }))}
      <div class="grid3">${fld("Durasi", sel(["1 minggu", "2 minggu", "3 minggu", "Custom"], "2 minggu"))}${fld("Mulai", inp("18 Nov 2026"))}${fld("Selesai", inp("1 Des 2026"))}</div>
      <div class="box">${keys.map((k) => `<div class="lrow" style="min-height:40px">${prioIcon(T(k).p)}${typeIcon(T(k).type)}<span class="mono mut">${k}</span><span class="grow trunc">${T(k).t}</span><span class="pz-pts" style="width:28px">${T(k).pts ?? "?"}</span></div>`).join("")}</div>
      ${keys.some((k) => !T(k).pts) ? `<div class="alert gray">${ic("sparkles")}<span>KSR-31 belum diestimasi. Tiket tetap bisa masuk sprint, estimasi AI menyusul setelah brainstorming selesai.</span></div>` : ""}`,
    foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Mulai sprint", "pri", "player-play", { "data-close": true, "data-toast": "Sprint 5 dimulai" })}` };
});
AG.dialog("complete-sprint", () => {
  const left = inSp("s4");
  return { title: "Selesaikan Sprint 4", desc: "Dijadwalkan selesai 17 Nov, 5 hari lagi.", icon: "flag", tone: "acc", w: "w560",
    body: `${AG.stats([{ label: "Selesai", value: "0", hint: "0 poin", tone: "ok" }, { label: "Belum selesai", value: left.length, hint: `${pts(left)} poin`, tone: "warn" }, { label: "Ditambah tengah sprint", value: "1", hint: "KSR-27" }])}
      ${fld("Pindahkan tiket yang belum selesai ke", `<div class="col">
        ${[["s5", "Sprint 5", "18 Nov – 1 Des · 15 poin saat ini"], ["new", "Sprint baru", "Dibuat otomatis dengan durasi 2 minggu"], ["bl", "Backlog", "Dikeluarkan dari semua sprint"]].map(([v, n, d], i) => `<div class="pz-opt ${i === 0 ? "on" : ""}" data-radio="cs-to"><span class="radio ${i === 0 ? "on" : ""}"></span><span class="grow"><b class="b5">${n}</b><br><span class="mut sm">${d}</span></span></div>`).join("")}</div>`)}
      <div class="box">${left.map((k) => `<div class="lrow" style="min-height:40px">${typeIcon(T(k).type)}<span class="mono mut">${k}</span><span class="grow trunc">${T(k).t}</span><span class="hide-sm">${AG.statePill(T(k).state)}</span></div>`).join("")}</div>
      <div class="alert gray">${ic("info-circle")}<span>Job agent yang sedang berjalan (KSR-26, KSR-29) tidak dihentikan. Hanya sprint-nya yang berpindah. Laporan Sprint 4 tersimpan di Laporan.</span></div>`,
    foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Selesaikan sprint", "pri", "flag", { "data-close": true, "data-toast": "Sprint 4 selesai, 6 tiket dipindah ke Sprint 5" })}` };
});

/* ====================== M9 · Roadmap (Gantt) ====================== */
const D = (m, d) => (m === 10 ? d - 1 : m === 11 ? 30 + d : 60 + d); // hari sejak 1 Okt 2026
const TODAY = D(11, 12), DAYS = 92;
const ZOOM = { week: 30, month: 12, quarter: 6 };
const RM = [
  { g: "Epic" },
  { k: "KSR-40", a: D(11, 4), b: D(11, 30), p: 40, epic: true },
  { k: "KSR-24", a: D(11, 4), b: D(11, 18), p: 75, ind: true },
  { k: "KSR-28", a: D(11, 9), b: D(11, 20), p: 50, ind: true },
  { g: "Sprint 4 · tanpa epic" },
  { k: "KSR-26", a: D(11, 6), b: D(11, 17), p: 60 },
  { k: "KSR-30", a: D(11, 5), b: D(11, 14), p: 70 },
  { k: "KSR-29", a: D(11, 10), b: D(11, 19), p: 65 },
  { k: "KSR-27", a: D(11, 13), b: D(11, 21), p: 0, est: true },
  { g: "Berikutnya" },
  { k: "KSR-35", a: D(11, 18), b: D(11, 20), p: 0, est: true },
  { k: "KSR-33", a: D(11, 18), b: D(12, 8), p: 0, est: true },
  { k: "KSR-36", a: D(12, 2), b: D(12, 9), p: 0, est: true },
  { k: "KSR-38", a: D(12, 7), b: D(12, 18), p: 0, est: true },
];
const DEPS = [["KSR-30", "KSR-29"], ["KSR-26", "KSR-33"], ["KSR-24", "KSR-28"]];
const RELS = [{ n: "v1.3", d: D(11, 3), id: "v1-3" }, { n: "v1.4", d: D(11, 20), id: "v1-4" }, { n: "v1.5", d: D(12, 4), id: "v1-5" }];
const MON = ["Okt", "Nov", "Des"];
const dLabel = (i) => (i < 31 ? `${i + 1} Okt` : i < 61 ? `${i - 30} Nov` : `${i - 60} Des`);
AG.screen({ m: "M9", g: "Planning", n: "Roadmap", route: "/p/:k/roadmap", path: "/p/KSR/roadmap",
  render: () => {
    const LW = innerWidth < 900 ? 150 : 280, px = ZOOM[PZ.zoom], tw = DAYS * px, RH = 36, GH = 32, HH = 88;
    const X = (d) => d * px;
    // filter: grup hanya tampil bila punya baris yang lolos
    const vis = []; let grp = null;
    RM.forEach((r) => { if (r.g) grp = r; else if (tPass("pz-rm", T(r.k))) { if (grp) { vis.push(grp); grp = null; } vis.push(r); } });
    const nT = vis.filter((r) => !r.g).length, allT = RM.filter((r) => !r.g).length;
    // header
    const hd1 = MON.map((m, i) => `<span style="left:${X([0, 31, 61][i])}px">${m} 2026</span>`).join("");
    let hd2 = "";
    for (let d = 0; d < DAYS; d++) {
      const dow = (d + 4) % 7; // 1 Okt 2026 = Kamis(4)
      const lbl = d < 31 ? d + 1 : d < 61 ? d - 30 : d - 60;
      if (PZ.zoom === "week") hd2 += `<span class="${d === TODAY ? "td" : dow === 0 || dow === 6 ? "wk" : ""}" style="left:${X(d)}px;width:${px}px;border-left-color:${dow === 1 ? "var(--line2)" : "transparent"}">${lbl}</span>`;
      else if (PZ.zoom === "month" && dow === 1) hd2 += `<span class="${Math.abs(d - TODAY) < 4 ? "td" : ""}" style="left:${X(d)}px">${lbl}</span>`;
    }
    const sprints = [["Sprint 3", D(10, 21), D(11, 3), "old"], ["Sprint 4", D(11, 4), D(11, 17), ""], ["Sprint 5", D(11, 18), D(12, 1), ""], ["Sprint 6", D(12, 2), D(12, 15), "old"]];
    const spb = PZ.zoom === "quarter" ? "" : sprints.map(([n, a, b, c]) => `<span class="pz-spb ${c}" style="left:${X(a) + 1}px;width:${X(b + 1) - X(a) - 2}px" data-tip="${n} · ${dLabel(a)} – ${dLabel(b)}">${n}</span>`).join("");
    // rows
    let y = HH; const pos = {};
    const rows = vis.map((r) => {
      if (r.g) { y += GH; return `<div class="pz-tlr grp"><div class="pz-lab" style="width:${LW}px">${ic("chevron-down", "s14 faint")}<span class="trunc">${r.g}</span></div><div class="pz-trk" style="width:${tw}px"></div></div>`; }
      const t = T(r.k), st = AG.stage(t.stage), e = EST[r.k];
      pos[r.k] = { y: y + RH / 2, a: X(r.a), b: X(r.b + 1) };
      y += RH;
      const col = r.epic ? AG.TYPES.epic.c : st.c.startsWith("var") ? "#8d8d99" : st.c;
      const tip = `${r.k} · ${t.t}\n${dLabel(r.a)} – ${dLabel(r.b)}${e && e.d ? `\nEstimasi: ${e.d[0]} jam AI + ${e.d[1]} jam manusia` : ""}\n${r.est ? "Belum mulai · tanggal dari estimasi AI" : `Progres ${r.p}% · ${st.n}`}`;
      const bar = `<div class="pz-bar ${r.est ? "est" : ""} ${r.epic ? "epic" : ""}" data-go="#/t/${r.k}" data-tip="${esc(tip)}" style="left:${X(r.a)}px;width:${X(r.b + 1) - X(r.a)}px;background:color-mix(in srgb, ${col} 18%, var(--raised));border-color:${r.est ? col : "transparent"}">${r.p ? `<b style="width:${r.p}%;background:${col}"></b>` : ""}${r.epic ? "" : `<span style="${r.p > 45 ? "color:#fff" : ""}">${r.k}</span>`}</div>`;
      return `<div class="pz-tlr"><div class="pz-lab" style="width:${LW}px;${r.ind ? "padding-left:30px" : ""}">${typeIcon(t.type)}<a class="mono mut" href="#/t/${r.k}">${r.k}</a><span class="trunc grow hide-sm">${t.t}</span><span class="grow only-sm"></span>${prioIcon(t.p)}${av(t.who, 20)}</div><div class="pz-trk" style="width:${tw}px">${bar}</div></div>`;
    }).join("");
    // marker + dependensi
    const marks = `<div class="pz-mk today" style="left:${LW + X(TODAY) + px / 2}px"></div>${RELS.map((r) => `<div class="pz-mk rel" style="left:${LW + X(r.d + 1)}px"></div>`).join("")}`;
    const labels = `<span class="pz-mkl today" style="left:${X(TODAY) + px / 2}px">Hari ini</span>${RELS.map((r) => `<span class="pz-mkl rel" data-go="#/p/KSR/releases/${r.id}" style="left:${X(r.d + 1)}px" data-tip="Release ${r.n} · ${dLabel(r.d)}">${r.n}</span>`).join("")}`;
    const arrows = DEPS.map(([f, t]) => {
      const a = pos[f], b = pos[t]; if (!a || !b) return "";
      const x1 = LW + a.b, y1 = a.y, x2 = LW + b.a, y2 = b.y, mx = Math.max(x1 + 8, x2 - 8);
      const d = x2 > x1 + 16 ? `M${x1},${y1} H${mx} V${y2} H${x2 - 2}` : `M${x1},${y1} h8 V${(y1 + y2) / 2} H${x2 - 10} V${y2} H${x2 - 2}`;
      return `<path d="${d}" style="fill:none;stroke:var(--muted);stroke-width:1.3" marker-end="url(#pz-arr)"><title>${f} blocks ${t}</title></path>`;
    }).join("");
    const svg = `<svg width="${LW + tw}" height="${y}" style="position:absolute;left:0;top:0;pointer-events:none;z-index:2" aria-hidden="true"><defs><marker id="pz-arr" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L8,4 L0,8 Z" style="fill:var(--muted)"/></marker></defs>${arrows}</svg>`;
    const right = `<span class="seg" role="group" aria-label="Zoom">${[["week", "Minggu"], ["month", "Bulan"], ["quarter", "Kuartal"]].map(([z, n]) => `<button class="${PZ.zoom === z ? "on" : ""}" data-act="pz-zoom" data-arg="${z}">${n}</button>`).join("")}</span>${btn("Hari ini", "", "focus-2", { "data-act": "pz-today" })}${btn("", "ghost icon", "download", { "data-toast": "Roadmap diekspor ke PNG", "aria-label": "Ekspor roadmap" })}`;
    return AG.shell({ project: K, crumbs: AG.projectCrumb(K, "Roadmap"), ...AG.projectTabs(K),
      body: `${head("Roadmap", "Bar dihitung dari estimasi AI dan diperbarui otomatis saat tiket bergerak. Garis putus-putus berarti belum mulai.")}
      ${AG.filterBar({ id: "pz-rm", search: "Cari tiket…", filters: TF(), right, count: anyFilter("pz-rm") ? `${nT} dari ${allT} tiket` : "" })}
      ${card("Linimasa tiket", legend([["Selesai sebagian (progres)", "var(--accent)"], ["Estimasi, belum mulai", "transparent", ""], ["Blocks", "var(--muted)", "ln"], ["Release", "var(--success)", "ln"]]).replace('background:transparent;border-color:transparent', "border:1px dashed var(--muted)"),
        nT ? `<div class="pz-tl" id="pz-tl"><div class="pz-tlg" style="width:${LW + tw}px">
        <div class="pz-tlh" style="height:${HH}px"><div class="pz-lab" style="width:${LW}px">Tiket</div><div class="pz-trk" style="width:${tw}px"><div class="pz-hd1">${hd1}</div><div class="pz-hd2">${hd2}</div><div style="position:relative;height:22px">${spb}</div><div style="position:relative;height:22px">${labels}</div></div></div>
        ${rows}${marks}${svg}
      </div></div>` : empty("filter", "Tidak ada tiket yang cocok", "Ubah kata kunci atau hapus filter."), "", `<span class="mut sm row g4 hide-sm">${ic("alert-triangle", "s14 text-warn")}KSR-29 menunggu KSR-30 (blocks)</span>`, !nT)}` });
  },
  after: (root) => { const s = root.querySelector("#pz-tl"); if (s) s.scrollLeft = Math.max(0, TODAY * ZOOM[PZ.zoom] - (innerWidth < 900 ? 60 : 220)); } });
AG.action("pz-zoom", (el, z) => { PZ.zoom = z; AG.rerender(); });
AG.action("pz-today", () => { const s = document.getElementById("pz-tl"); s?.scrollTo({ left: Math.max(0, TODAY * ZOOM[PZ.zoom] - (innerWidth < 900 ? 60 : 220)), behavior: "smooth" }); });

/* ====================== M9 · Kalender ====================== */
const CAL = {
  14: [{ k: "KSR-30", late: true }],
  16: [{ k: "KSR-26" }],
  17: [{ k: "KSR-24", est: true, tip: "Perkiraan selesai AI: 17 Nov (1 hari sebelum due)" }, { k: "KSR-26", est: true, late: true, tip: "Perkiraan selesai AI: 17 Nov, terlambat 1 hari dari due 16 Nov" }],
  18: [{ k: "KSR-24" }],
  19: [{ k: "KSR-29", est: true, tip: "Perkiraan selesai AI: 19 Nov" }],
  20: [{ k: "KSR-28" }, { rel: "v1.4" }, { k: "KSR-35", est: true, tip: "Perkiraan selesai AI: 20 Nov" }],
  21: [{ k: "KSR-27" }, { k: "KSR-27", est: true, tip: "Perkiraan selesai AI: 21 Nov" }],
  30: [{ k: "KSR-40" }],
};
const evKind = (e) => (e.rel ? "Release" : e.est ? "Perkiraan selesai AI" : "Due date");
const evPass = (e) => {
  const kinds = AG.filterValue("pz-cal", "kind");
  if (kinds.length && !kinds.includes(evKind(e))) return false;
  if (e.rel) return !AG.filterQuery("pz-cal").trim() && ["type", "who", "label"].every((k) => !AG.filterValue("pz-cal", k).length);
  return tPass("pz-cal", T(e.k));
};
const evChip = (e) => {
  if (e.rel) return `<a class="pz-ev rel" href="#/p/KSR/releases/v1-4" data-tip="Release ${e.rel} dijadwalkan">${ic("rocket")}<span class="trunc">Release ${e.rel}</span></a>`;
  const t = T(e.k);
  return `<a class="pz-ev ${e.est ? "est" : ""} ${e.late ? "late" : ""}" href="#/t/${e.k}" data-tip="${esc(e.tip || `${e.k} · ${t.t}\nDue date${e.late ? " · butuh keputusan, berisiko lewat" : ""}`)}">${e.est ? ic("sparkles") : typeIcon(t.type, "")}<span class="mono">${e.k}</span><span class="trunc">${t.t}</span></a>`;
};
AG.screen({ m: "M9", g: "Planning", n: "Kalender", route: "/p/:k/calendar", path: "/p/KSR/calendar",
  render: () => {
    const weeks = [];
    for (let w = 0; w < 6; w++) { const r = []; for (let i = 0; i < 7; i++) r.push(w * 7 + i - 5); weeks.push(r); } // 26 Okt = -5 → hari ke-n Nov
    const ev = (d) => (CAL[d] || []).filter(evPass);
    const nEv = Object.keys(CAL).reduce((a, d) => a + ev(d).length, 0), allEv = Object.values(CAL).flat().length;
    const bands = (wk) => {
      const out = []; const sp = [["Sprint 4", 4, 17, ""], ["Sprint 5", 18, 31, "nx"]];
      sp.forEach(([n, a, b, c]) => { const s = Math.max(a, wk[0]), e = Math.min(b, wk[6]); if (s <= e) out.push(`<span class="pz-band ${c}" style="left:calc(${((s - wk[0]) / 7) * 100}% + 4px);width:calc(${((e - s + 1) / 7) * 100}% - 8px)" data-tip="${n} · ${a} Nov – ${b > 30 ? "1 Des" : b + " Nov"}">${n}</span>`); });
      return out.join("");
    };
    const grid = weeks.map((wk) => `<div class="pz-calw">${bands(wk)}${wk.map((d, i) => {
      const out = d < 1 || d > 30, num = d < 1 ? 31 + d : d > 30 ? d - 30 : d;
      return `<div class="pz-cd ${out ? "out" : ""} ${i > 4 && !out ? "we" : ""} ${d === 12 ? "today" : ""}"><span class="num">${num}${d === 1 ? " Nov" : d === 31 ? " Des" : ""}</span>${out ? "" : ev(d).map(evChip).join("")}</div>`;
    }).join("")}</div>`).join("");
    const agenda = Object.keys(CAL).filter((d) => ev(d).length).map((d) => `<div class="pz-ag"><div class="d"><b class="${+d === 12 ? "text-dng" : ""}">${d}</b><span>Nov</span></div><div class="col g4">${ev(d).map(evChip).join("")}</div></div>`).join("");
    const right = `<span class="pz-mon">${btn("", "ghost icon", "chevron-left", { "data-toast": "Oktober 2026", "aria-label": "Bulan sebelumnya" })}<b>November 2026</b>${btn("", "ghost icon", "chevron-right", { "data-toast": "Desember 2026", "aria-label": "Bulan berikutnya" })}</span>${btn("Hari ini", "", "", { "data-toast": "Kembali ke 12 November" })}<span class="seg" role="group" aria-label="Tampilan"><button class="on">Bulan</button><button data-toast="Tampilan minggu">Minggu</button></span>`;
    return AG.shell({ project: K, crumbs: AG.projectCrumb(K, "Kalender"), ...AG.projectTabs(K),
      body: `${head("Kalender", "Due date tiket dan perkiraan selesai dari estimasi AI. Pita di atas tanggal menandai sprint.")}
      ${AG.filterBar({ id: "pz-cal", search: "Cari tiket…", filters: [{ k: "kind", label: "Jenis", icon: "calendar", options: ["Due date", "Perkiraan selesai AI", "Release"] }, ...TF().slice(0, 3)], right, count: anyFilter("pz-cal") ? `${nEv} dari ${allEv} acara` : "" })}
      ${card("Due date & perkiraan selesai", legend([["Due date", "var(--line2)"], ["Perkiraan selesai AI", "var(--muted)", "dash"], ["Lewat/berisiko", "var(--danger)"], ["Release", "var(--success)"]]),
        `<div class="pz-cal hide-sm"><div class="pz-calh">${["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"].map((d) => `<div>${d}</div>`).join("")}</div>${grid}</div>
        <div class="only-sm pz-agenda" style="padding:16px">${agenda || empty("filter", "Tidak ada acara yang cocok", "Ubah kata kunci atau hapus filter.")}</div>`, "", "", false)}
      ${(() => { const nd = AG.TICKETS.filter((t) => !t.due && t.stage !== "done" && t.type !== "epic" && t.type !== "subtask").slice(0, 4);
        return card("Tanpa tanggal", `${nd.length} tiket belum punya due date maupun estimasi, jadi tidak muncul di kalender.`, `<div class="box">${nd.map((t) => `<a class="lrow" href="#/t/${t.k}">${prioIcon(t.p)}${typeIcon(t.type)}<span class="mono mut">${t.k}</span><span class="grow trunc">${t.t}</span>${av(t.who, 20)}</a>`).join("")}</div>`, "", AG.seeAll("#/p/KSR/backlog", "Atur di backlog"), false); })()}` });
  } });

/* ====================== M9 · Rilis ====================== */
const NOTES_MD = `## v1.4 · Pembayaran QRIS dan perbaikan struk

Rilis ini membawa pembayaran QRIS langsung dari layar kasir dan beberapa perbaikan struk.

### Fitur baru
- **Bayar dengan QRIS** (KSR-24). Kasir menampilkan kode QR dinamis, status pembayaran masuk otomatis lewat callback.
- **Diskon per item** (KSR-28). Diskon bisa diberikan per barang di keranjang, dengan batas sesuai hak kasir.
- **Cetak ulang struk** (KSR-29) dari riwayat transaksi.

### Perbaikan
- Checkout ditolak bila stok tidak cukup (KSR-26).
- Struk kini memuat nama kasir (KSR-30).

### Catatan untuk toko
> Aktifkan QRIS di **Pengaturan toko → Pembayaran** setelah merchant ID dari bank disetujui.`;
const RELEASES = [
  { id: "v1-5", n: "v1.5", st: "Rencana", tone: "gray", date: "4 Des 2026", sortD: 20261204, desc: "Mode offline dan laporan", keys: ["KSR-33", "KSR-35", "KSR-31", "KSR-27"], md: "" },
  { id: "v1-4", n: "v1.4", st: "Belum dirilis", tone: "acc", date: "20 Nov 2026", sortD: 20261120, desc: "Pembayaran QRIS dan perbaikan struk", keys: ["KSR-24", "KSR-26", "KSR-28", "KSR-29", "KSR-30"], md: NOTES_MD },
  { id: "v1-3", n: "v1.3", st: "Dirilis", tone: "ok", date: "3 Nov 2026", sortD: 20261103, desc: "Login PIN dan pembulatan total", keys: ["KSR-22", "KSR-21"], done: true, md: "## v1.3 · Login PIN dan pembulatan total\n\n### Fitur baru\n- **Login kasir dengan PIN 6 digit** (KSR-22).\n\n### Perbaikan\n- Pembulatan total belanja tidak lagi selisih Rp1 (KSR-21)." },
  { id: "v1-2", n: "v1.2", st: "Dirilis", tone: "ok", date: "13 Okt 2026", sortD: 20261013, desc: "Riwayat transaksi per shift", keys: ["KSR-17", "KSR-18"], done: true, md: "## v1.2\n\n- Riwayat transaksi per shift." },
  { id: "v1-1", n: "v1.1", st: "Dirilis", tone: "ok", date: "22 Sep 2026", sortD: 20260922, desc: "Scan barcode dan logo toko di struk", keys: ["KSR-16", "KSR-19"], done: true, md: "## v1.1\n\n- Scan barcode." },
];
const rel = (id) => RELEASES.find((r) => r.id === id) || RELEASES[1];
const relDone = (r) => (r.done ? r.keys.length : r.keys.filter((k) => T(k).stage === "done").length);
AG.screen({ m: "M9", g: "Planning", n: "Rilis", route: "/p/:k/releases", path: "/p/KSR/releases",
  render: () => AG.shell({ project: K, crumbs: AG.projectCrumb(K, "Rilis"), ...AG.projectTabs(K), width: "mid",
    actions: btn("Versi baru", "pri", "plus", { "data-dialog": "pz-new-release" }),
    body: `${head("Rilis", "Kelompokkan tiket per versi. Release notes disusun agent Tech Writer dari tiket, PR, dan artifact yang masuk.")}
    <div class="alert gray">${ic("info-circle")}<span>Field <b>Fix version</b> di tiket menentukan versi. Saat sinkron Jira, field ini dipetakan ke <i>Fix Version/s</i>.</span></div>
    ${AG.datatable({ id: "pz-rels", search: "Cari versi…", pageSize: 10, rowGo: (i) => `#/p/KSR/releases/${RELEASES[i].id}`,
      filters: [{ k: "st", label: "Status", icon: "rocket", options: ["Rencana", "Belum dirilis", "Dirilis"] }],
      columns: [{ label: "Versi" }, { label: "Status", w: "130px" }, { label: "Progres", w: "180px" }, { label: "Tanggal rilis", hide: "sm", w: "130px" }, { label: "Release notes", hide: "sm", w: "150px" }],
      rows: RELEASES.map((r) => { const done = relDone(r); return {
        cells: [`<div class="row">${ic("rocket", "faint")}<span class="pz-cell"><b class="b6">${r.n}</b><small>${r.desc}</small></span></div>`, pill(r.st, r.tone),
          `<div class="col g4"><div class="bar ${r.done ? "ok" : ""}"><i style="width:${(done / r.keys.length) * 100}%"></i></div><span class="sm mut">${done} dari ${r.keys.length} tiket selesai</span></div>`, r.date,
          !r.md ? `<span class="mut">Belum ada</span>` : r.done ? pill("Diterbitkan", "ok", "", "check") : pill("Draf AI siap", "acc", "", "sparkles")],
        sort: [r.sortD, r.st, done / r.keys.length, r.sortD, r.md ? (r.done ? 2 : 1) : 0], text: `${r.n} ${r.desc}`, filter: { st: r.st } }; }) })}` }) });
const relCrumbs = (r, last) => [AG.projectCrumb(K, "")[0], `<a href="#/p/KSR/releases">Rilis</a>`, ...(last ? [`<a href="#/p/KSR/releases/${r.id}">${r.n}</a>`, last] : [r.n])];
AG.screen({ m: "M9", g: "Planning", n: "Detail rilis", route: "/p/:k/releases/:id", path: "/p/KSR/releases/v1-4",
  render: ({ id }) => {
    const r = rel(id), done = relDone(r), notesGo = `#/p/KSR/releases/${r.id}/notes`;
    const notes = r.md
      ? `<div class="box"><div class="prose pz-prose" style="padding:16px 18px">${AG.mdToHtml(r.md)}</div><div class="boxfoot" style="justify-content:space-between;flex-wrap:wrap"><span class="row g4 sm mut">${av("writer", 18)}Tech Writer · dari ${r.keys.length} tiket dan 4 PR</span>${btn("Salin Markdown", "sm ghost", "copy", { "data-act": "pz-copy", "data-arg": r.md })}</div></div>`
      : `<div class="box">${empty("writing", "Release notes belum ada", "Tech Writer menyusun draf setelah tiket pertama di versi ini selesai. Anda juga bisa menulis sendiri.", `<div class="row">${btn("Susun dengan AI", "", "sparkles", { "data-toast": "Tech Writer menyusun draf dari 4 tiket" })}${btn("Tulis manual", "pri", "pencil", { "data-go": notesGo })}</div>`)}</div>`;
    return AG.shell({ project: K, crumbs: relCrumbs(r), ...AG.projectTabs(K), width: "mid",
      actions: `${btn("", "ghost icon", "dots", { "data-menu": "pz-rel", "aria-label": "Opsi versi" })}${r.done ? "" : btn(`Rilis ${r.n}`, "pri", "rocket", { "data-dialog": "pz-release", "data-arg": r.id })}`,
      body: `${head(`${r.n} <span style="vertical-align:3px">${pill(r.st, r.tone)}</span>`, `${r.desc} · target ${r.date}`)}
      ${AG.stats([
        { label: "Tiket", value: r.keys.length, icon: "ticket", hint: "di versi ini" },
        { label: "Selesai", value: done, icon: "circle-check", tone: "ok", hint: `${Math.round((done / r.keys.length) * 100)}% dari versi` },
        { label: "Story point", value: pts(r.keys.filter((k) => AG.TICKETS.some((t) => t.k === k))), icon: "chart-bar", hint: "total estimasi" },
        { label: "Perkiraan siap", value: r.done ? "Dirilis" : "21 Nov", icon: r.done ? "rocket" : "alert-triangle", tone: r.done ? "ok" : "warn", hint: r.done ? r.date : "1 hari lewat target" },
      ])}
      <div class="grid2 pz-gr340">
        ${section("Release notes", r.md ? `Disusun agent Tech Writer · 12 Nov 10.14${r.done ? " · diterbitkan" : " · bisa diedit sebelum dirilis"}` : "Belum disusun",
          notes, r.md ? `<div class="row g4">${r.done ? "" : btn("Susun ulang", "sm", "sparkles", { "data-toast": `Tech Writer menyusun ulang release notes dari ${r.keys.length} tiket` })}${btn("Edit", "sm", "pencil", { "data-go": notesGo })}</div>` : "")}
        ${section("Tiket dalam versi ini", `${r.keys.length} tiket`, `${r.done || r.id !== "v1-4" ? "" : `<div class="alert warn">${ic("alert-triangle")}<span>KSR-30 butuh keputusan. Rilis tetap bisa jalan bila tiket ini dipindah ke v1.5.</span></div>`}<div class="box">${r.keys.map((k) => { const t = AG.TICKETS.find((x) => x.k === k); return t ? `<a class="lrow" href="#/t/${k}" style="min-height:44px">${prioIcon(t.p)}${typeIcon(t.type)}<span class="mono mut">${k}</span><span class="grow trunc">${t.t}</span>${stageIcon(t.stage)}</a>` : `<a class="lrow" href="#/search" style="min-height:44px">${ic("circle-check-filled", "text-ok")}<span class="mono mut">${k}</span><span class="grow trunc mut">Tiket lama (arsip)</span></a>`; }).join("")}<button class="pz-add" data-toast="Pilih tiket untuk ditambahkan ke ${r.n}">${ic("plus", "s14")}Tambah tiket</button></div>`)}
      </div>` });
  } });
AG.screen({ m: "M9", g: "Planning", n: "Editor release notes", route: "/p/:k/releases/:id/notes", path: "/p/KSR/releases/v1-4/notes",
  render: ({ id }) => {
    const r = rel(id), back = `#/p/KSR/releases/${r.id}`;
    const src = r.keys.map((k) => { const t = AG.TICKETS.find((x) => x.k === k); return t ? `<a class="lrow" href="#/t/${k}">${typeIcon(t.type)}<span class="mono mut">${k}</span><span class="grow trunc">${t.t}</span></a>` : ""; }).join("");
    return AG.shell({ project: K, crumbs: relCrumbs(r, "Release notes"), ...AG.projectTabs(K), width: "mid",
      actions: btn("Lihat hasil", "", "eye", { "data-go": back }),
      body: `${head(`Release notes ${r.n}`, "Draf disusun agent Tech Writer dari tiket, PR, dan artifact. Diterbitkan bersama rilis dan dikirim lewat webhook <code>release.published</code>.")}
      <div class="grid2 pz-gr260" style="align-items:start">
        <div class="pz-ed">${AG.editor({ full: true, html: r.md ? AG.mdToHtml(r.md) : "", ph: "Tulis release notes… Ketik / untuk menyisipkan blok, @ untuk menyebut tiket", save: "Simpan", footer: `${btn("Susun ulang", "ghost sm", "sparkles", { "data-toast": "Tech Writer menyusun ulang draf. Perubahan Anda disimpan sebagai versi terpisah" })}` })}</div>
        <aside class="col g16">
          ${section("Sumber", `${r.keys.length} tiket · 4 PR`, `<div class="box pz-src">${src}<a class="lrow" href="#/p/KSR/prs">${ic("git-pull-request", "faint")}<span class="grow">4 pull request merged</span>${ic("chevron-right", "s14 faint")}</a></div>`)}
          ${section("Riwayat versi", "", `<div class="box pz-src">${[["Olivia Owner", "OO", "Hari ini 10.32", "Edit bagian Catatan untuk toko"], ["Tech Writer", "writer", "12 Nov 10.14", "Draf dari 5 tiket"], ["Tech Writer", "writer", "11 Nov 16.02", "Draf awal"]].map(([n, a, w, d], i) => `<button class="lrow" style="width:100%;text-align:left" data-toast="${i ? `Membandingkan dengan versi ${w}` : "Ini versi terbaru"}">${av(a, 20)}<span class="grow pz-cell"><span class="trunc">${d}</span><small>${n} · ${w}</small></span></button>`).join("")}</div>`)}
        </aside>
      </div>` });
  } });
AG.menu("pz-rel", () => `<button class="mi" data-toast="Mode edit versi">${ic("pencil")}Edit nama dan tanggal</button><button class="mi" data-toast="Release notes diekspor ke CHANGELOG.md">${ic("markdown")}Ekspor ke CHANGELOG.md</button><div class="msep"></div><button class="mi dng" data-toast="Versi diarsipkan" data-tone="dng">${ic("archive")}Arsipkan versi</button>`);
AG.dialog("pz-release", (id) => {
  const r = rel(id), left = r.keys.filter((k) => T(k).stage !== "done");
  return { title: `Rilis ${r.n}`, desc: "Tandai versi sebagai dirilis dan terbitkan release notes.", icon: "rocket", tone: "acc", w: "w560",
    body: `<div class="alert warn">${ic("alert-triangle")}<span>${left.length} tiket belum selesai.</span></div>
      ${fld("Tiket yang belum selesai", sel([["v1-5", "Pindahkan ke v1.5"], ["none", "Lepas dari versi"], ["keep", "Biarkan di versi ini"]], "v1-5"))}
      ${srow("Terbitkan release notes", "Ditampilkan di halaman rilis dan dikirim lewat webhook <code>release.published</code>.", sw(true, "Terbitkan release notes"))}
      ${srow("Buat tag git", "Tag <code>v1.4.0</code> di kasir-api dan kasir-web.", sw(true, "Buat tag git"))}`,
    foot: `${btn("Batal", "", "", { "data-close": true })}${btn(`Rilis ${r.n}`, "pri", "rocket", { "data-close": true, "data-toast": `${r.n} dirilis` })}` };
});
AG.dialog("pz-new-release", () => ({ title: "Versi baru", icon: "rocket", tone: "acc",
  body: `${fld("Nama", inp("v1.6"))}${fld("Deskripsi", inp("", { ph: "Misalnya: Program poin pelanggan" }))}<div class="grid2">${fld("Mulai", inp("2 Des 2026"))}${fld("Target rilis", inp("18 Des 2026"))}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Buat versi", "pri", "", { "data-close": true, "data-toast": "Versi v1.6 dibuat" })}` }));

/* ====================== M9 · Laporan ====================== */
function rBurndown() {
  const L = ["4", "5", "6", "9", "10", "11", "12", "13", "16", "17"];
  const ideal = L.map((_, i) => +(26 - (26 / 9) * i).toFixed(1));
  const act = [23, 23, 26, 26, 26, 26, 26, null, null, null];
  const proj = [null, null, null, null, null, null, 26, 13, 8, 3];
  const tips = L.map((l, i) => `${l} Nov\nIdeal: ${fmt(ideal[i])} poin${act[i] != null ? `\nSisa: ${act[i]} poin` : ""}${proj[i] != null && i > 6 ? `\nProyeksi AI: ${proj[i]} poin` : ""}${i === 2 ? "\nKSR-27 ditambahkan (+3)" : ""}`);
  return `<div class="grid2 pz-gr260">
    ${AG.card({ cls: "pz-rcard", title: "Burndown · Sprint 4", desc: "Sisa story point per hari kerja", action: legend([["Ideal", "var(--faint)", "dash"], ["Sisa poin", "var(--accent)", "ln"], ["Proyeksi AI", "var(--warn)", "dash"]]), body: `
    ${lineChart({ labels: L, yMax: 30, yStep: 5, tips, marks: [{ i: 6, c: "var(--danger)", l: "Hari ini" }], series: [{ n: "Ideal", c: "var(--faint)", v: ideal, dash: true, w: 1.5 }, { n: "Sisa", c: "var(--accent)", v: act, area: true }, { n: "Proyeksi", c: "var(--warn)", v: proj, dash: true }] })}
    <span class="mut sm">Sumbu x: hari kerja November. Poin turun saat tiket masuk stage Selesai.</span>` })}
    ${card("Kondisi sprint", "Sprint 4 · sisa 5 hari", `
      <div class="alert warn">${ic("alert-triangle")}<span>Belum ada poin yang selesai, tetapi 13 poin sudah di Code Review dan Testing. AI memperkirakan sisa <b>3 poin</b> di akhir sprint.</span></div>
      <div class="box">${[["plus", "faint", "Scope bertambah", "+3"], ["eye", "faint", "Menunggu review manusia", "8"], ["alert-triangle", "text-dng", "Butuh keputusan", "2"], ["hourglass", "faint", "Antre runner", "3"]].map(([i, c, n, v]) => `<div class="lrow">${ic(i, c)}<span class="grow">${n}</span><b>${v}</b></div>`).join("")}</div>`)}</div>`;
}
function rBurnup() {
  const L = ["Sprint 1", "Sprint 2", "Sprint 3", "Sprint 4", "Sprint 5", "Sprint 6"];
  const scope = [60, 72, 84, 95, 95, 95], done = [14, 34, 55, 55, null, null], proj = [null, null, null, 55, 76, 95];
  return AG.card({ cls: "pz-rcard", title: "Burnup · Kasir Online", desc: "Scope dan poin selesai per sprint", action: legend([["Scope", "var(--faint)", "ln"], ["Selesai", "var(--success)", "ln"], ["Proyeksi AI", "var(--success)", "dash"]]), body: `
    ${lineChart({ w: 1000, labels: L, yMax: 100, yStep: 20, tips: L.map((l, i) => `${l}${i === 3 ? " (berjalan)" : i > 3 ? " (proyeksi)" : ""}\nScope: ${scope[i]} poin${done[i] != null ? `\nSelesai: ${done[i]} poin` : ""}${proj[i] != null && i > 3 ? `\nProyeksi: ${proj[i]} poin` : ""}`), marks: [{ i: 3, c: "var(--danger)", l: "Hari ini" }], series: [{ c: "var(--faint)", v: scope, w: 1.5 }, { c: "var(--success)", v: done, area: true }, { c: "var(--success)", v: proj, dash: true }] })}
    <span class="mut sm">Scope naik 35 poin sejak Sprint 1. Dengan velocity sekarang, scope saat ini selesai di akhir Sprint 6 (15 Des), setelah target v1.5.</span>` });
}
function rVelocity() {
  const S = [["Sprint 1", 18, 14], ["Sprint 2", 22, 20], ["Sprint 3", 24, 21], ["Sprint 4", 26, null]];
  const W = 1000, f = frame(S.length, 30, 5, S.map(() => ""), W); const bw = 44, gx = (W - PL - PR) / S.length;
  let s = f.g;
  S.forEach(([n, c, d], i) => {
    const cx = PL + gx * i + gx / 2;
    s += `<rect x="${cx - bw - 2}" y="${f.y(c)}" width="${bw}" height="${f.y(0) - f.y(c)}" rx="3" style="fill:var(--line2)" data-tip="${n}\nKomitmen: ${c} poin"/>`;
    s += d == null ? `<rect x="${cx + 2}" y="${f.y(0) - 2}" width="${bw}" height="2" style="fill:var(--accent)"/><text x="${cx + 2 + bw / 2}" y="${f.y(0) - 8}" text-anchor="middle">berjalan</text>`
      : `<rect x="${cx + 2}" y="${f.y(d)}" width="${bw}" height="${f.y(0) - f.y(d)}" rx="3" style="fill:var(--accent)" data-tip="${n}\nSelesai: ${d} poin (${Math.round((d / c) * 100)}%)"/>`;
    s += `<text x="${cx}" y="${H - 8}" text-anchor="middle">${n}</text>`;
  });
  s += `<line x1="${PL}" x2="${W - PR}" y1="${f.y(18.3)}" y2="${f.y(18.3)}" style="stroke:var(--warn);stroke-width:1.2" stroke-dasharray="4 3"/><text x="${W - PR}" y="${f.y(18.3) - 5}" text-anchor="end" style="fill:var(--warn)">rata-rata 18,3</text>`;
  return AG.card({ cls: "pz-rcard", title: "Velocity", desc: "Komitmen dan poin selesai per sprint", action: legend([["Komitmen", "var(--line2)"], ["Selesai", "var(--accent)"], ["Rata-rata 3 sprint", "var(--warn)", "dash"]]), body: `
    <div class="pz-cw"><svg class="pz-chart" viewBox="0 0 ${W} ${H}" role="img" aria-label="Grafik velocity">${s}</svg></div>
    <span class="mut sm">Komitmen naik tiap sprint, penyelesaian 88% rata-rata. Kapasitas Sprint 5 disarankan 24 poin.</span>` });
}
const SP3 = [["KSR-22", "Login kasir dengan PIN 6 digit", "story", 3, 6, 7.2], ["KSR-21", "Pembulatan total belanja selisih Rp1", "bug", 1, 2, 1.5], ["KSR-19", "Tambah barang dengan scan barcode", "story", 3, 4, 4.6], ["KSR-18", "Buka dan tutup shift kasir", "story", 6, 8, 9.1], ["KSR-17", "Riwayat transaksi per shift", "story", 3, 5, 4.4], ["KSR-16", "Logo toko di header struk", "task", 1, 1.5, 2.2], ["KSR-20", "Kunci layar otomatis setelah 5 menit", "task", 2, 3, 2.6], ["KSR-15", "Total tidak berubah setelah hapus item", "bug", 2, 3, 3.4]];
function rSprint() {
  return `<div class="col g16">
    <div class="pz-tbar">${ccWrap("pz-sprint-rep", AG.combo({ value: "s3", options: [{ v: "s4", l: "Sprint 4", sub: "Berjalan · laporan sementara di Burndown" }, { v: "s3", l: "Sprint 3", sub: "21 Okt – 3 Nov" }, { v: "s2", l: "Sprint 2", sub: "7 – 20 Okt" }, { v: "s1", l: "Sprint 1", sub: "23 Sep – 6 Okt" }] }))}<span class="mut">Tujuan: login kasir aman dan total belanja akurat</span></div>
    ${AG.stats([{ label: "Komitmen", value: "24", icon: "target", hint: "poin di awal sprint" }, { label: "Selesai", value: "21", icon: "circle-check", tone: "ok", hint: "88% dari komitmen" }, { label: "Dipindah ke Sprint 4", value: "2", icon: "arrow-right", tone: "warn", hint: "7 poin" }, { label: "Ditambah tengah sprint", value: "1", icon: "plus", hint: "KSR-21" }])}
    ${section("Selesai", "8 tiket · 21 poin", AG.datatable({ id: "pz-sp3", pageSize: 10, rowGo: (i) => (i < 2 ? `#/t/${SP3[i][0]}` : "#/search"),
      columns: [{ label: "Tiket", w: "110px" }, { label: "Judul" }, { label: "Poin", w: "70px", align: "right" }, { label: "Estimasi", hide: "sm", w: "100px", align: "right" }, { label: "Aktual", hide: "sm", w: "100px", align: "right" }],
      rows: SP3.map(([k, t, ty, p, e, a]) => ({ cells: [`<span class="row g4 nowrap">${typeIcon(ty)}<span class="mono mut">${k}</span></span>`, `<span class="trunc">${t}</span>`, `<span class="mono">${p}</span>`, `<span class="mut">${fmt(e)} jam</span>`, `<span class="${a > e * 1.15 ? "text-warn" : ""}">${fmt(a)} jam</span>`], sort: [k, t, p, e, a] })) }))}
    ${section("Tidak selesai, dipindah ke Sprint 4", "2 tiket · 7 poin", `<div class="box">${["KSR-28", "KSR-30"].map((k) => `<a class="lrow" href="#/t/${k}">${prioIcon(T(k).p)}${typeIcon(T(k).type)}<span class="mono mut">${k}</span><span class="grow trunc">${T(k).t}</span><span class="pz-pts" style="width:28px">${T(k).pts}</span></a>`).join("")}</div>`)}</div>`;
}
CC["pz-sprint-rep"] = (v) => AG.toast(v === "s4" ? "Sprint 4 masih berjalan, lihat tab Burndown" : `Laporan ${v.replace("s", "Sprint ")} ditampilkan`);
function rCfd() {
  const L = ["20 Okt", "23", "27", "30", "3 Nov", "6", "10", "12"];
  const data = { done: [0, 0, 1, 1, 2, 2, 2, 2], test: [0, 1, 1, 2, 1, 1, 2, 2], review: [0, 1, 1, 1, 1, 2, 1, 1], dev: [2, 2, 3, 2, 3, 3, 4, 4], design: [1, 1, 1, 2, 2, 1, 1, 1], prd: [2, 2, 2, 1, 1, 2, 2, 2], brainstorm: [1, 1, 1, 1, 1, 1, 1, 1], backlog: [5, 4, 3, 4, 4, 3, 3, 3] };
  const order = ["done", "test", "review", "dev", "design", "prd", "brainstorm", "backlog"];
  const W = 1000, f = frame(L.length, 18, 3, L, W);
  const base = L.map(() => 0); let s = "";
  order.forEach((k) => {
    const st = AG.stage(k), lo = [...base]; data[k].forEach((v, i) => (base[i] += v));
    const top = base.map((v, i) => [f.x(i), f.y(v)]), bot = lo.map((v, i) => [f.x(i), f.y(v)]).reverse();
    s += `<path d="${path(top)}L${bot.map((p) => p.join(",")).join("L")}Z" style="fill:${st.c};fill-opacity:0.78;stroke:var(--raised);stroke-width:1"><title>${st.n}</title></path>`;
  });
  const colW = (W - PL - PR) / (L.length - 1);
  L.forEach((l, i) => { s += `<rect class="hit" x="${f.x(i) - colW / 2}" y="${PT}" width="${colW}" height="${H - PT - PB}" data-tip="${esc(`${l}\n` + [...order].reverse().map((k) => `${AG.stage(k).n}: ${data[k][i]}`).join("\n"))}"/>`; });
  return AG.card({ cls: "pz-rcard", title: "Cumulative flow", desc: "Jumlah tiket per stage, 3 minggu terakhir", body: `
    <div class="pz-cw"><svg class="pz-chart" viewBox="0 0 ${W} ${H}" role="img" aria-label="Cumulative flow">${f.g}${s}</svg></div>
    ${legend([...order].reverse().map((k) => [AG.stage(k).n, AG.stage(k).c]))}
    <span class="mut sm">Pita Development melebar sejak 3 Nov: WIP 4 dari batas 4. Pertimbangkan menahan tiket baru masuk Development.</span>` });
}
function rCalib() {
  const P = [[2, 1.5, "KSR-21"], [6, 7.2, "KSR-22"], [3, 3.4], [4, 5.1], [8, 9.6], [5, 4.6], [10, 13.5], [12, 12.4], [1.5, 2.2], [7, 8.9], [14, 15.1], [3, 2.6], [9, 11.8], [16, 19.5]];
  const S = 230, M = 34, sc = (v) => M + (v / 20) * (S - M - 10);
  let s = "";
  for (let v = 0; v <= 20; v += 5) s += `<line class="gl" x1="${sc(0)}" x2="${sc(20)}" y1="${S - sc(v) + M - 10}" y2="${S - sc(v) + M - 10}"/><text x="${M - 6}" y="${S - sc(v) + M - 6}" text-anchor="end">${v}</text><text x="${sc(v)}" y="${S + 14}" text-anchor="middle">${v}</text>`;
  const Y = (v) => S - sc(v) + M - 10;
  s += `<line x1="${sc(0)}" y1="${Y(0)}" x2="${sc(20)}" y2="${Y(20)}" style="stroke:var(--faint);stroke-width:1.2" stroke-dasharray="4 3"/>`;
  P.forEach(([e, a, k]) => { s += `<circle cx="${sc(e)}" cy="${Y(a)}" r="${k ? 5 : 4}" style="fill:${a > e * 1.15 ? "var(--warn)" : "var(--accent)"};fill-opacity:0.85;stroke:var(--raised);stroke-width:1.5" data-tip="${k ? k + "\n" : ""}Estimasi ${fmt(e)} jam\nAktual ${fmt(a)} jam"/>`; });
  const rows = [["Brainstorming", "0,5 + 3,0", "0,6 + 3,4", "1,12"], ["PRD", "1,2 + 4,0", "1,1 + 4,5", "1,05"], ["Desain UI", "2,0 + 4,5", "2,1 + 6,1", "1,30"], ["Development", "5,0 + 2,0", "5,6 + 2,2", "1,12"], ["Code Review", "0,8 + 2,0", "0,6 + 1,9", "0,90"], ["Testing", "1,5 + 4,0", "1,8 + 6,0", "1,42"]];
  return `<div class="grid2 pz-gl300">
    ${card("Estimasi vs aktual", "Jam per tiket", `<svg class="pz-chart" viewBox="0 0 ${S + 10} ${S + 34}" role="img" aria-label="Scatter estimasi vs aktual">${s}<text x="${(S + 10) / 2 + 12}" y="${S + 32}" text-anchor="middle">estimasi (jam)</text><text x="10" y="${M - 18}" >aktual</text></svg>
    ${legend([["Sesuai (±15%)", "var(--accent)"], ["Lebih lama", "var(--warn)"], ["Garis tepat", "var(--faint)", "dash"]])}`)}
    ${card("Faktor koreksi per stage", "Dari 14 tiket Selesai di 3 sprint terakhir. Angka setelah “+” adalah waktu kerja manusia.", `<div class="alert">${ic("sparkles")}<span>Estimasi AI rata-rata <b>16% terlalu optimis</b>, terutama di Testing dan Desain UI karena menunggu review manusia. Faktor koreksi dipakai otomatis untuk estimasi berikutnya di Kasir Online.</span></div>
    ${AG.datatable({ id: "pz-calib", pageSize: 10, columns: [{ label: "Stage" }, { label: "Estimasi AI + manusia" }, { label: "Aktual median" }, { label: "Faktor", w: "90px" }],
      rows: rows.map(([n, e, a, k]) => ({ cells: [n, `<span class="mono mut">${e} j</span>`, `<span class="mono">${a} j</span>`, `<span class="pill ${+k.replace(",", ".") > 1.2 ? "warn" : "gray"} mono">×${k}</span>`], sort: [n, e, a, +k.replace(",", ".")] })) })}
    `)}</div>`;
}
function rCycle() {
  const R = [["Brainstorming", 0.6, 3.4], ["PRD", 1.1, 4.5], ["Desain UI", 2.1, 6.1], ["Development", 5.6, 2.2], ["Code Review", 0.6, 7.6], ["Testing", 1.8, 9.2]];
  const max = 12;
  return AG.card({ cls: "pz-rcard", title: "Cycle time per stage", desc: "Median jam, 3 sprint terakhir", action: legend([["Waktu AI", "var(--accent)"], ["Waktu manusia (kerja + menunggu)", "#c2410c"]]), body: `
    <div class="alert warn">${ic("hourglass")}<span>78% cycle time habis menunggu manusia. Code Review rata-rata menunggu 7,6 jam: aktifkan notifikasi review atau tambah reviewer.</span></div>
    <div>${R.map(([n, a, h]) => `<div class="pz-hb"><span class="trunc">${n}</span><div class="tr" style="width:${((a + h) / max) * 100}%;min-width:40px"><i style="width:${(a / (a + h)) * 100}%;background:var(--accent)" data-tip="${n}\nAI: ${fmt(a)} jam"></i><i style="width:${(h / (a + h)) * 100}%;background:#c2410c;opacity:0.75" data-tip="${n}\nManusia: ${fmt(h)} jam"></i></div><span class="mono sm mut" style="text-align:right">${fmt(+(a + h).toFixed(1))} j</span></div>`).join("")}</div>` });
}
AG.screen({ m: "M9", g: "Laporan", n: "Laporan", route: "/p/:k/reports", path: "/p/KSR/reports",
  render: () => {
    const tabs = [["burndown", "Burndown", rBurndown], ["burnup", "Burnup", rBurnup], ["velocity", "Velocity", rVelocity], ["sprint", "Sprint report", rSprint], ["cfd", "Cumulative flow", rCfd], ["calib", "Kalibrasi estimasi", rCalib], ["cycle", "Cycle time", rCycle]];
    const cur = tabs.some(([k]) => k === PZ.sub.rep) ? PZ.sub.rep : "burndown";
    return AG.shell({ project: K, crumbs: AG.projectCrumb(K, "Laporan"), ...AG.projectTabs(K),
      actions: btn("Ekspor PDF", "", "download", { "data-toast": "Laporan diekspor ke PDF" }),
      body: `${head("Laporan", "Data dicatat dari pergerakan tiket dan durasi job agent, tanpa time tracking manual. Diperbarui terakhir 10.45 WIB.")}
      <div data-tabs-root data-pz-sp="rep" class="col g16"><nav class="tabs" role="tablist">${tabs.map(([k, n]) => `<button role="tab" data-tab="${k}" class="${k === cur ? "on" : ""}">${n}</button>`).join("")}</nav>
      ${tabs.map(([k, , fn]) => `<div data-panel="${k}" class="${k === cur ? "on" : ""}">${fn()}</div>`).join("")}</div>` });
  } });

/* ====================== M9 · Pencarian lanjutan ====================== */
const FIELDS = { project: "Kode project", stage: "Stage workflow", status: "Status di stage", assignee: "Penerima", type: "Tipe tiket", sprint: "Sprint", label: "Label", priority: "Prioritas", points: "Story point", text: "Judul dan deskripsi" };
const VALS = {
  project: AG.PROJECTS.map((p) => p.k), stage: AG.STAGES.map((s) => (s.n.includes(" ") ? `"${s.n}"` : s.n)),
  status: Object.values(AG.STATES).map((s) => `"${s.n}"`), assignee: ["currentUser()", ...Object.values(AG.PEOPLE).map((p) => `"${p.name}"`), "EMPTY"],
  type: Object.values(AG.TYPES).map((t) => t.n), sprint: ['"Sprint 4"', '"Sprint 3"', "openSprints()"], label: Object.keys(AG.LABELS), priority: Object.values(AG.PRIORITY).map((p) => p.n), points: ["EMPTY", "1", "2", "3", "5", "8", "13"], text: ['"struk"'],
};
function fieldVal(t, f) {
  switch (f) {
    case "project": return [t.k.split("-")[0]];
    case "stage": return [AG.stage(t.stage).n, t.stage];
    case "status": return [AG.STATES[t.state].n];
    case "assignee": return t.who ? [t.who, AG.PEOPLE[t.who].name] : [];
    case "type": return [AG.TYPES[t.type].n];
    case "sprint": return t.sprint ? [t.sprint] : [];
    case "label": return t.lbl;
    case "priority": return [AG.PRIORITY[t.p].n];
    case "points": return t.pts ? [String(t.pts)] : [];
    case "text": return [t.t];
  }
  return null;
}
function runQuery(q) {
  const body = q.replace(/\s+ORDER\s+BY\s+.*$/i, "").trim();
  const order = /ORDER\s+BY\s+priority/i.test(q);
  const clauses = body ? body.split(/\s+AND\s+/i) : [];
  const tests = [];
  for (const c of clauses) {
    const m = c.match(/^(\w+)\s*(!=|=|~|not\s+in|in|is\s+not|is)\s*(.+)$/i);
    if (!m) return { err: `Tidak bisa membaca “${c}”. Format: field operator nilai` };
    const f = m[1].toLowerCase(), op = m[2].toLowerCase().replace(/\s+/g, " ");
    if (!FIELDS[f]) return { err: `Field “${m[1]}” tidak dikenal. Coba: ${Object.keys(FIELDS).slice(0, 5).join(", ")}` };
    const raw = m[3].trim();
    const vals = (/in$/.test(op) ? raw.replace(/^\(|\)$/g, "").split(",") : [raw]).map((v) => v.trim().replace(/^"|"$/g, "").toLowerCase()).map((v) => (v === "currentuser()" ? AG.ME.toLowerCase() : v));
    tests.push((t) => {
      const fv = fieldVal(t, f).map((x) => String(x).toLowerCase());
      if (op === "is" || op === "is not") { const emp = fv.length === 0; return op === "is" ? emp : !emp; }
      if (op === "~") return fv.some((x) => x.includes(vals[0]));
      if (vals[0] === "opensprints()") return fv.includes("sprint 4") === (op === "=");
      const hit = vals.some((v) => fv.includes(v));
      return op === "=" || op === "in" ? hit : !hit;
    });
  }
  let res = AG.TICKETS.filter((t) => tests.every((fn) => fn(t)));
  if (order) res = [...res].sort((a, b) => a.p - b.p);
  return { res };
}
function suggestions(q) {
  const m = q.match(/(\w+)\s*(!=|=|~|in|is)\s*("[^"]*|[^\s"]*)$/i);
  if (m && FIELDS[m[1].toLowerCase()]) { const part = m[3].replace(/^"/, "").toLowerCase(); return { kind: `Nilai ${m[1]}`, items: VALS[m[1].toLowerCase()].filter((v) => v.toLowerCase().replace(/"/g, "").startsWith(part)).map((v) => ({ v, rep: m[3], t: "" })) }; }
  const w = q.match(/(\w*)$/)[1].toLowerCase();
  const afterVal = /("|\)|\w)\s+$/.test(q) && !/\b(AND|OR|BY)\s+$/i.test(q) && q.trim() !== "";
  if (afterVal && !w) return { kind: "Kata kunci", items: [{ v: "AND ", rep: "", t: "gabung syarat" }, { v: "ORDER BY priority", rep: "", t: "urutkan" }] };
  return { kind: "Field", items: Object.entries(FIELDS).filter(([f]) => f.startsWith(w)).map(([f, t]) => ({ v: f + " = ", rep: w, t })) };
}
const sugHtml = (q) => { const s = suggestions(q); return s.items.length ? `<div class="mlab">${s.kind}</div>${s.items.slice(0, 8).map((x) => `<button class="mi" data-pz-sug="${esc(x.v)}" data-rep="${esc(x.rep)}"><code>${esc(x.v)}</code><span class="tail">${x.t}</span></button>`).join("")}` : `<div class="mlab">Tidak ada saran</div>`; };
const DUE_SORT = (d) => (d ? +d.split(" ")[0] + (d.includes("Des") ? 100 : 0) : 999);
function resultTable(res) {
  const bulk = `${btn("Stage", "sm", "route", { "data-menu": "pz-bulk", "data-arg": "stage" })}${btn("Penerima", "sm", "user", { "data-menu": "pz-bulk", "data-arg": "who" })}${btn("Sprint", "sm", "player-play", { "data-menu": "pz-bulk", "data-arg": "sprint" })}${btn("Edit massal", "sm", "pencil", { "data-dialog": "pz-bulk-edit" })}${btn("", "sm ghost icon", "trash", { "data-act": "pz-bulkset", "data-arg": "dipindah ke sampah (30 hari)", "aria-label": "Hapus tiket terpilih" })}`;
  return AG.datatable({ id: "pz-search", select: true, bulk, pageSize: 10, search: "Saring hasil…",
    filters: [{ k: "stage", label: "Stage", icon: "route", options: AG.STAGES.map((s) => s.n) }, { k: "status", label: "Status", icon: "circle-dashed", options: Object.values(AG.STATES).map((s) => s.n) }, { k: "who", label: "Penerima", icon: "user", options: [...Object.values(AG.PEOPLE).map((p) => p.name), "Tanpa penerima"] }, { k: "type", label: "Tipe", icon: "category-2", options: ["Epic", "Story", "Task", "Bug", "Subtask"] }],
    right: btn("Ekspor CSV", "ghost", "download", { "data-toast": `${res.length} tiket diekspor ke CSV` }),
    rowGo: (i) => `#/t/${res[i].k}`,
    empty: empty("search", "Tidak ada tiket yang cocok", "Ubah query atau saringan, misalnya hapus syarat assignee atau stage.", btn("Reset query", "", "refresh", { "data-act": "pz-filter", "data-arg": "0" })),
    columns: [{ label: "Tiket", w: "100px" }, { label: "Judul" }, { label: "Stage", w: "150px" }, { label: "Status", hide: "sm", w: "170px" }, { label: "Penerima", w: "90px" }, { label: "Poin", hide: "sm", w: "64px", align: "right" }, { label: "Due", hide: "sm", w: "80px" }],
    rows: res.map((t) => ({
      cells: [`<span class="nowrap">${AG.tref(t.k)}</span>`, `<span class="row" style="min-width:180px">${prioIcon(t.p)}<span class="trunc">${t.t}</span></span>`, `<span class="row g4 nowrap">${stageIcon(t.stage)}${AG.stage(t.stage).n}</span>`, AG.statePill(t.state), av(t.who, 22), `<span class="mono">${t.pts ?? "—"}</span>`, `<span class="mut nowrap">${t.due || "—"}</span>`],
      sort: [+t.k.split("-")[1], t.t, AG.STAGES.findIndex((s) => s.k === t.stage), AG.STATES[t.state].n, t.who ? AG.PEOPLE[t.who].name : "~", t.pts ?? -1, DUE_SORT(t.due)],
      text: `${t.k} ${t.t}`, filter: { stage: AG.stage(t.stage).n, status: AG.STATES[t.state].n, who: t.who ? AG.PEOPLE[t.who].name : "Tanpa penerima", type: AG.TYPES[t.type].n } })) });
}
AG.screen({ m: "M9", g: "Pencarian", n: "Pencarian lanjutan", route: "/search",
  render: () => {
    const r = runQuery(PZ.q);
    return AG.shell({ crumbs: ["Pencarian"],
      actions: btn("Simpan filter", "", "star", { "data-dialog": "save-filter" }),
      body: `${head("Pencarian lanjutan", "Tulis query ala JQL. Gabungkan syarat dengan <code>AND</code>, pakai <code>=</code>, <code>!=</code>, <code>~</code>, <code>in (…)</code>, <code>is EMPTY</code>.")}
      <div class="pz-srch">
        <aside class="pz-sf-wrap" aria-label="Saved filter">${AG.card({ cls: "pz-sf", title: "Saved filter", desc: `${PZ.filters.length} filter · Anda, Tim Kasir, workspace`, body:
          PZ.filters.map((f, i) => `<button class="pz-sfi ${PZ.filter === i ? "on" : ""}" data-act="pz-filter" data-arg="${i}">${ic(f.i, "s14")}<span class="trunc">${f.n}</span><span class="n">${(runQuery(f.q).res || []).length}</span></button>`).join("") })}
        </aside>
        <div class="col g16" style="min-width:0">
          <div class="box pad col g8"><div class="pz-qbox"><div class="pz-qrow">${inp(PZ.q, { cls: "pz-q", a: { id: "pz-q", "aria-label": "Query", spellcheck: "false", autocomplete: "off" } })}${btn("Jalankan", "pri", "search", { "data-act": "pz-run" })}</div>
            <div class="pz-sug" id="pz-sug" role="listbox"></div></div>
            ${r.err ? `<div class="pz-qerr">${ic("alert-circle", "s14")}${r.err}</div>` : `<span class="mut sm">${r.res.length} tiket cocok dengan query · tekan Enter untuk menjalankan, klik kotak untuk saran field</span>`}</div>
          ${r.err ? `<div class="box">${empty("alert-circle", "Query belum bisa dijalankan", "Perbaiki query di atas, lalu tekan Jalankan.")}</div>` : resultTable(r.res)}
        </div></div>` });
  },
  after: (root) => {
    const q = root.querySelector("#pz-q"), box = root.querySelector("#pz-sug");
    if (!q) return;
    const show = () => { box.innerHTML = sugHtml(q.value.slice(0, q.selectionStart ?? q.value.length)); box.classList.add("on"); };
    q.addEventListener("focus", show); q.addEventListener("input", show); q.addEventListener("click", show);
    q.addEventListener("keydown", (e) => { if (e.key === "Enter") { PZ.q = q.value; dtReset("pz-search"); AG.rerender(); } if (e.key === "Escape") box.classList.remove("on"); });
    q.addEventListener("blur", () => setTimeout(() => box.classList.remove("on"), 150));
    box.addEventListener("mousedown", (e) => {
      const b = e.target.closest("[data-pz-sug]"); if (!b) return; e.preventDefault();
      const cur = q.value.slice(0, q.selectionStart ?? q.value.length), rest = q.value.slice(cur.length);
      const head = b.dataset.rep ? cur.slice(0, cur.length - b.dataset.rep.length) : cur.replace(/\s*$/, " ");
      q.value = head + b.dataset.pzSug + rest; const p = (head + b.dataset.pzSug).length; q.setSelectionRange(p, p); show();
    });
  } });
AG.action("pz-run", () => { PZ.q = document.getElementById("pz-q").value; dtReset("pz-search"); AG.rerender(); });
AG.action("pz-filter", (el, i) => { PZ.filter = +i; PZ.q = PZ.filters[+i].q; dtReset("pz-search"); AG.rerender(); });
AG.action("pz-bulkset", (el, v) => { const n = dtSel("pz-search").size; dtSel("pz-search").clear(); AG.rerender(); AG.toast(`${n} tiket ${/^(dipindah|diubah)/.test(v) ? v : "diubah: " + v}`); });
AG.menu("pz-bulk", (what) => {
  const opts = what === "stage" ? AG.STAGES.map((s) => [s.n, s.i]) : what === "who" ? Object.entries(AG.PEOPLE).map(([k, p]) => [p.name, "user"]) : [["Sprint 4", "player-play"], ["Sprint 5", "calendar"], ["Backlog", "stack-2"]];
  return `<div class="mlab">${what === "stage" ? "Pindahkan ke stage" : what === "who" ? "Ubah penerima" : "Pindahkan ke sprint"}</div>${opts.map(([n, i]) => `<button class="mi" data-act="pz-bulkset" data-arg="${esc(n)}">${ic(i)}${n}</button>`).join("")}${what === "stage" ? `<div class="msep"></div><div class="mlab" style="white-space:normal">Gate tetap ditegakkan per tiket</div>` : ""}`;
});
AG.dialog("pz-bulk-edit", () => ({ title: `Edit ${dtSel("pz-search").size} tiket`, desc: "Hanya field yang diisi yang berubah.", icon: "pencil", tone: "acc", w: "w560",
  body: `<div class="grid2">${fld("Prioritas", AG.combo({ value: "", options: [{ v: "", l: "(tidak diubah)" }, ...[1, 2, 3, 4].map((p) => ({ v: p, l: AG.PRIORITY[p].n, lead: prioIcon(p) }))] }))}${fld("Sprint", AG.combo({ value: "", options: [{ v: "", l: "(tidak diubah)" }, { v: "s4", l: "Sprint 4", sub: "Aktif" }, { v: "s5", l: "Sprint 5" }, { v: "bl", l: "Backlog" }] }))}${fld("Penerima", AG.combo({ value: "", options: peopleOpts([{ v: "", l: "(tidak diubah)" }]) }))}${fld("Fix version", AG.combo({ value: "", options: [{ v: "", l: "(tidak diubah)" }, ...RELEASES.filter((r) => !r.done).map((r) => ({ v: r.id, l: r.n, sub: r.desc }))] }))}</div>
    ${fld("Tambah label", AG.combo({ multi: true, ph: "Pilih label…", options: Object.keys(AG.LABELS).map((l) => ({ v: l, l, lead: `<span class="dot" style="background:${AG.LABELS[l]}"></span>` })) }))}
    <div class="alert gray">${ic("info-circle")}<span>Perpindahan stage tetap divalidasi gate. Tiket yang belum memenuhi syarat akan dilewati dan dilaporkan.</span></div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Terapkan", "pri", "check", { "data-close": true, "data-act": "pz-bulkset", "data-arg": "diubah lewat edit massal" })}` }));
AG.dialog("save-filter", () => ({ title: "Simpan filter", icon: "star", tone: "acc",
  body: `${fld("Nama", inp("Sprint aktif belum selesai", { a: { id: "pz-fname" } }))}${fld("Query", `<div class="code" style="white-space:pre-wrap">${esc(PZ.q)}</div>`)}
    ${fld("Bisa dilihat oleh", AG.combo({ value: "me", options: [{ v: "me", l: "Saya saja", lead: ic("lock", "s14") }, { v: "team", l: "Tim Kasir", sub: "5 anggota", lead: ic("users", "s14") }, { v: "ws", l: "Seluruh workspace", lead: ic("world", "s14") }] }))}
    ${srow("Tampilkan di sidebar", "Muncul sebagai pintasan di navigasi.", sw(false, "Tampilkan di sidebar"))}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Simpan", "pri", "check", { "data-act": "pz-savefilter" })}` }));
AG.action("pz-savefilter", () => { const n = document.getElementById("pz-fname")?.value || "Filter baru"; PZ.filters.push({ n, q: PZ.q, i: "star", own: "Anda" }); PZ.filter = PZ.filters.length - 1; AG.closeDialog?.(); AG.rerender(); AG.toast(`Filter “${n}” disimpan`); });

/* ====================== M9 · Components ====================== */
const COMPS = [
  { n: "Pembayaran", mod: "kasir-api/src/payment", who: "DD", t: 4, d: "QRIS, callback, rekonsiliasi" },
  { n: "Keranjang & checkout", mod: "kasir-web/src/cart", who: "DD", t: 3, d: "Keranjang, diskon, validasi stok" },
  { n: "Struk & cetak", mod: "kasir-web/src/receipt", who: "QQ", t: 3, d: "Template struk, printer Bluetooth" },
  { n: "Stok", mod: "kasir-api/src/inventory", who: "AA", t: 2, d: "Sinkron stok ke Aplikasi Gudang" },
  { n: "Auth kasir", mod: "kasir-api/src/auth", who: "AA", t: 1, d: "PIN, sesi shift" },
  { n: "Laporan", mod: null, who: "OO", t: 2, d: "Laporan harian dan ekspor" },
];
const MODS = ["kasir-api/src/payment", "kasir-api/src/inventory", "kasir-api/src/auth", "kasir-api/src/report", "kasir-api/src/customer", "kasir-web/src/cart", "kasir-web/src/receipt", "kasir-web/src/settings"];
AG.screen({ m: "M9", g: "Pengaturan project", n: "Components", route: "/settings/p/:k/components", path: "/settings/p/KSR/components",
  render: () => AG.shell({ nav: "settings", project: K, crumbs: ["Pengaturan", AG.project(K).n, "Components"], width: "mid",
    body: subpage("comp", "list", { title: "Components", desc: "Bagian produk Kasir Online. Tiap component terhubung ke modul di code graph, jadi tiket baru otomatis diberi component dan pemiliknya dari file yang terdampak.",
      nav: [{ k: "list", label: "Daftar component", icon: "components", count: COMPS.length }, { k: "rules", label: "Aturan otomatis", icon: "sparkles" }],
      panels: [
        ["list", card("Daftar component", "Pemilik component otomatis jadi reviewer tiket di bagiannya.", `${COMPS[5].mod ? "" : `<div class="alert">${ic("sparkles")}<span>AI menemukan modul <code>kasir-api/src/report</code> yang belum punya component. <a class="link" href="#" data-act="pz-linkcomp">Hubungkan ke Laporan</a></span></div>`}
          ${AG.datatable({ id: "pz-comps", search: "Cari component atau modul…", pageSize: 10,
            filters: [{ k: "who", label: "Pemilik", icon: "user", options: Object.values(AG.PEOPLE).map((p) => p.name) }, { k: "st", label: "Code graph", icon: "topology-star-3", options: ["Terhubung", "Belum terhubung"] }],
            columns: [{ label: "Component" }, { label: "Modul code graph", hide: "sm" }, { label: "Pemilik", w: "150px" }, { label: "Tiket", w: "70px", align: "right" }, { label: "", w: "48px", sort: false }],
            rows: COMPS.map((c, i) => ({ cells: [`<span class="pz-cell"><b class="b5">${c.n}</b><small>${c.d}</small></span>`, c.mod ? `<a class="row g4 mono sm" href="#/p/KSR/graph">${ic("topology-star-3", "s14 faint")}${c.mod}</a>` : `<span class="pill warn">Belum terhubung</span>`, `<span class="row g8 nowrap">${av(c.who, 20)}<span class="trunc">${AG.PEOPLE[c.who].name}</span></span>`, `<a class="link mono" href="#/search" data-act="pz-compsearch" data-arg="${i}">${c.t}</a>`, `<div style="text-align:right">${btn("", "ghost icon sm", "dots", { "data-menu": "pz-comp", "data-arg": c.n, "aria-label": `Opsi ${c.n}` })}</div>`],
              sort: [c.n, c.mod || "~", AG.PEOPLE[c.who].name, c.t, 0], text: `${c.n} ${c.d} ${c.mod || ""}`, filter: { who: AG.PEOPLE[c.who].name, st: c.mod ? "Terhubung" : "Belum terhubung" } })) })}`, "", btn("Component baru", "pri sm", "plus", { "data-dialog": "pz-new-comp" }))],
        ["rules", card("Aturan otomatis", "Berlaku untuk tiket baru dan draft dari AI.", `<div class="box">
            ${srow("Isi component otomatis", "Saat draft tiket dibuat, AI memilih component dari file yang terdampak di code graph.", sw(true, "Isi component otomatis"))}
            ${srow("Pemilik component jadi reviewer", "Pemilik ditambahkan sebagai reviewer manusia di stage Code Review.", sw(true, "Pemilik jadi reviewer"))}
            ${srow("Penerima default", "Bila tiket belum punya penerima.", `<span class="pz-w220">${AG.combo({ value: "owner", options: [{ v: "owner", l: "Pemilik component" }, { v: "none", l: "Tanpa penerima" }, { v: "creator", l: "Pembuat tiket" }] })}</span>`)}
            ${srow("Reviewer cadangan", "Dipakai bila pemilik component sedang cuti atau tidak aktif.", `<span class="pz-w220">${AG.combo({ value: "AA", options: peopleOpts() })}</span>`)}
          </div>`, AG.savebar("Simpan aturan", { toast: "Aturan component disimpan" }))],
      ] }) }) });
AG.action("pz-linkcomp", () => { COMPS[5].mod = "kasir-api/src/report"; AG.rerender(); AG.toast("Laporan terhubung ke kasir-api/src/report"); });
AG.action("pz-compsearch", (el, i) => { PZ.q = `label = ${COMPS[+i].n === "Pembayaran" ? "Pembayaran" : "Kasir"}`; dtReset("pz-search"); location.hash = "#/search"; });
AG.menu("pz-comp", (n) => `<button class="mi" data-toast="Edit component ${n}">${ic("pencil")}Edit</button><button class="mi" data-toast="Pemilik ${n} diubah">${ic("user")}Ganti pemilik</button><a class="mi" href="#/p/KSR/graph">${ic("topology-star-3")}Lihat di code graph</a><div class="msep"></div><button class="mi dng" data-toast="Component ${n} dihapus" data-tone="dng">${ic("trash")}Hapus</button>`);
AG.dialog("pz-new-comp", () => ({ title: "Component baru", icon: "components", tone: "acc",
  body: `${fld("Nama", inp("", { ph: "Misalnya: Pelanggan & poin" }))}${fld("Deskripsi", inp("", { ph: "Cakupan component ini" }))}${fld("Modul code graph", AG.combo({ multi: true, value: ["kasir-api/src/customer"], ph: "Pilih modul…", options: MODS.map((m) => ({ v: m, l: m, group: m.split("/")[0], lead: ic("topology-star-3", "s14 faint") })) }), "Bisa lebih dari satu modul, termasuk lintas repo.")}${fld("Pemilik", AG.combo({ value: "OO", options: peopleOpts() }))}`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Buat", "pri", "", { "data-close": true, "data-toast": "Component dibuat" })}` }));

/* ====================== M11 · Jira ====================== */
const JMAP = [["To Do", "backlog"], ["Analysis", "brainstorm"], ["Spec", "prd"], ["In Design", "design"], ["In Progress", "dev"], ["In Review", "review"], ["QA", "test"], ["Done", "done"], ["Blocked", null]];
const JLOG = [
  ["10.42", "in", "ok", "SR-311 → KSR-24", "Komentar dari Jira disalin"],
  ["10.40", "out", "ok", "KSR-26 → SR-305", "Due date diperbarui di Jira"],
  ["10.31", "in", "gate", "SR-298 ↛ KSR-30", "Status “Done” dari Jira ditolak: gate Testing belum lulus. Dikembalikan ke QA"],
  ["10.12", "out", "ok", "KSR-28 → SR-302", "Status In Design"],
  ["09.58", "in", "fail", "SR-280", "Lampiran 32 MB melebihi batas 25 MB, dilewati"],
  ["09.41", "out", "ok", "KSR-29 → SR-303", "Story point 3 disalin"],
  ["09.20", "in", "gate", "SR-301 ↛ KSR-27", "Status “In Review” ditolak: belum ada PR terbuka"],
  ["09.02", "in", "ok", "SR-312 → KSR-42", "Tiket baru dari Jira dibuat di Backlog"],
  ["08.47", "out", "ok", "KSR-24 → SR-311", "Label Pembayaran ditambahkan"],
  ["08.30", "out", "ok", "KSR-30 → SR-298", "Prioritas Urgent"],
  ["08.12", "in", "ok", "SR-290 → KSR-21", "Komentar dari Jira disalin"],
  ["07.55", "in", "fail", "SR-277", "User budi@sevima-retail.id belum dipetakan, penerima dikosongkan"],
];
const JRES = { ok: ["Berhasil", "circle-check", "text-ok"], gate: ["Ditolak gate", "hand-stop", "text-warn"], fail: ["Gagal", "alert-circle", "text-dng"] };
const jiraSave = (label = "Simpan & sinkron") => AG.savebar(label, { toast: "Konfigurasi Jira disimpan, sinkron berjalan", msg: "Perubahan berlaku pada sinkron berikutnya" });
AG.screen({ m: "M11", g: "Integrasi", n: "Connector Jira", route: "/settings/jira",
  render: () => {
    const env = PZ.jiraEnv, mode = PZ.jiraMode;
    const conn = `<div class="row"><span class="live"></span><span class="grow pz-cell"><b class="b5">Terhubung ke sevima-retail.atlassian.net</b><small>SR ↔ KSR · ${mode === "two" ? "sinkron dua arah" : mode === "mirror" ? "mirror satu arah" : "impor sekali"} · terakhir 3 menit lalu</small></span></div>`;
    const overview = card("Ringkasan", "Status koneksi dan sinkron 24 jam terakhir.", `<div class="alert warn">${ic("alert-triangle")}<span>Status Jira <b>Blocked</b> belum dipetakan. <button type="button" class="link" data-tab="status">Atur pemetaan status</button></span></div>
      ${conn}
      ${AG.stats([{ label: "Tiket tersinkron", value: "142", icon: "arrows-exchange", tone: "acc", hint: "SR ↔ KSR" }, { label: "Perubahan 24 jam", value: "37", icon: "activity", hint: "21 masuk · 16 keluar" }, { label: "Ditolak gate", value: "2", icon: "hand-stop", tone: "warn", hint: "Lihat di log sinkron" }])}
      ${sub("Konfigurasi saat ini", "", `<div class="box"><dl class="pz-kv">
        <dt>Koneksi</dt><dd>Jira Cloud · sevima-retail.atlassian.net</dd><dt>Project</dt><dd>SR ↔ ${AG.projDot(K)} Kasir Online</dd><dt>Mode</dt><dd>${mode === "two" ? "Sinkron dua arah" : mode === "mirror" ? "Mirror satu arah" : "Impor sekali"}</dd><dt>Status</dt><dd>8 dari 9 dipetakan</dd><dt>Field</dt><dd>8 field</dd><dt>User</dt><dd>3 dari 4 dipetakan</dd><dt>Data awal</dt><dd>142 tiket, 610 komentar, 88 lampiran, 4 sprint</dd></dl></div>`)}`, "", `${btn("Sinkron sekarang", "sm", "refresh", { "data-toast": "Sinkron dimulai" })}${btn("", "ghost icon sm", "dots", { "data-menu": "pz-jira", "aria-label": "Opsi koneksi" })}`);
    const connPanel = card("Koneksi", "Jenis Jira, kredensial, dan project yang disinkronkan.", `${sub("Jenis Jira", "", `<div class="pz-env">${[["cloud", "Jira Cloud", "atlassian.net, login dengan email dan API token", "cloud"], ["dc", "Jira Data Center", "Server sendiri, versi 8.14 ke atas", "server"]].map(([k, n, d, i]) => `<div class="pz-opt ${env === k ? "on" : ""}" data-act="pz-jenv" data-arg="${k}" role="radio" aria-checked="${env === k}"><span class="radio ${env === k ? "on" : ""}"></span><span class="grow pz-cell"><b class="b5 row g4">${ic(i, "s14")}${n}</b><small>${d}</small></span></div>`).join("")}</div>`)}
      ${sub("Kredensial", "", `<div class="col g16">${env === "cloud"
        ? `${fld("URL site", inp("https://sevima-retail.atlassian.net"))}<div class="grid2">${fld("Email akun", inp("olivia@sevima-retail.id"))}${fld("API token", inp("••••••••••••••••••••", { type: "password" }), `Buat di <button type="button" class="link" data-toast="Membuka id.atlassian.com/manage-profile/security/api-tokens">id.atlassian.com</button>`)}</div>`
        : `${fld("URL server", inp("https://jira.sevima-retail.id"))}${fld("Personal Access Token", inp("", { type: "password", ph: "PAT dari profil Jira Anda" }), "Server di jaringan internal bisa dijangkau lewat runner.")}`}
        <div class="row wrap">${btn("Uji koneksi", "", "plug", { "data-toast": "Terhubung sebagai Olivia Owner · 3 project terlihat" })}<span class="row g4 sm text-ok">${ic("circle-check", "s14")}Terhubung · ${env === "cloud" ? "Jira Cloud" : "Data Center"} · 3 project</span></div></div>`)}
      ${sub("Project", "", `<div>${fld("Project Jira", AG.combo({ value: "SR", options: [{ v: "SR", l: "SR · Sevima Retail POS", sub: "142 issue" }, { v: "GD", l: "GD · Gudang", sub: "58 issue" }, { v: "WEB", l: "WEB · Website", sub: "31 issue" }] }), "Dipetakan ke project Agent Gate <b>Kasir Online (KSR)</b>.")}</div>`)}`, jiraSave());
    const modePanel = card("Mode sinkron", "Bisa diganti kapan saja. Mengganti ke impor sekali akan menghentikan sinkron.", `${sub("", "", `<div class="col">${[["once", "Impor sekali", "Tiket, komentar, lampiran, user, sprint, dan pemetaan status dipindahkan ke Agent Gate. Setelah itu koneksi selesai.", "file-import"],
      ["mirror", "Mirror satu arah", "Agent Gate → Jira. Jira menjadi cermin yang hanya untuk dilihat; perubahan di Jira ditimpa.", "arrow-right"],
      ["two", "Sinkron dua arah", "Perubahan di kedua sisi disinkronkan.", "arrows-exchange"]].map(([k, n, d, i]) => `<div class="pz-opt ${mode === k ? "on" : ""}" data-act="pz-jmode" data-arg="${k}" role="radio" aria-checked="${mode === k}"><span class="radio ${mode === k ? "on" : ""}"></span><span class="tile">${ic(i)}</span><span class="grow pz-cell"><b class="b5">${n}</b><small>${d}</small></span></div>`).join("")}</div>`)}
      ${mode === "two" ? sub("Aturan bentrok", "Berlaku saat sisi Jira dan Agent Gate mengubah tiket yang sama.", `<div class="col g12">
        <div class="pz-nb"><span class="tile acc" style="width:22px;height:22px">${ic("route", "s14")}</span><span><b class="b5">Status dan stage selalu mengikuti Agent Gate.</b> <span class="mut">Gate ditegakkan di sini, jadi perpindahan status dari Jira yang belum memenuhi gate ditolak dan dikembalikan, lalu dicatat di log.</span></span></div>
        <div class="pz-nb"><span class="tile" style="width:22px;height:22px">${ic("clock", "s14")}</span><span><b class="b5">Field lain mengikuti perubahan terakhir.</b> <span class="mut">Judul, deskripsi, label, prioritas, due date, dan komentar memakai yang paling baru diubah.</span></span></div></div>`) : ""}`, jiraSave());
    const stOpts = stageOpts([{ v: "", l: "Tidak dipetakan" }]);
    const statusPanel = card("Status Jira ↔ stage", "Status Jira project SR dipetakan ke stage workflow Kasir Online.", `<div class="alert warn">${ic("alert-triangle")}<span>Status <b>Blocked</b> belum dipetakan. Saran AI: jadikan status <b>Butuh keputusan</b> di stage yang sama, bukan stage terpisah.</span></div>
      <div class="box"><div class="pz-map"><span class="jh">Status Jira (SR)</span><span></span><span class="jh">Stage Agent Gate (KSR)</span>
      ${JMAP.map(([j, st]) => `<span><span class="pz-jst">${j}</span></span><span class="arw">${ic("arrows-exchange", "s14")}</span>${AG.combo({ value: st || "", options: stOpts })}`).join("")}</div></div>`, jiraSave());
    const fieldsPanel = card("Field & user", "Field Jira dipetakan ke field Agent Gate. User dicocokkan otomatis lewat email.", `${sub("Field", "", `<div class="box"><div class="pz-map"><span class="jh">Field Jira</span><span></span><span class="jh">Field Agent Gate</span>
      ${[["Summary", "Judul"], ["Description", "Deskripsi"], ["Story Points", "Story point"], ["Sprint", "Sprint"], ["Fix Version/s", "Rilis"], ["Components", "Components"], ["Due date", "Due date"], ["Toko (customfield_10031)", "Toko (field custom)"]].map(([j, a]) => `<span class="mono sm">${j}</span><span class="arw">${ic("arrows-exchange", "s14")}</span>${AG.combo({ value: a, options: [...["Judul", "Deskripsi", "Story point", "Sprint", "Rilis", "Components", "Due date", "Toko (field custom)", "Label", "Prioritas"].map((x) => ({ v: x, l: x })), { v: "-", l: "Tidak disinkronkan" }] })}`).join("")}</div></div>`)}
      ${sub("User", "", `<div class="alert gray">${ic("info-circle")}<span>Budi Santoso belum jadi anggota. Tiketnya diimpor tanpa penerima, atau <a class="link" href="#/settings/members">undang dulu</a>.</span></div><div class="box">${[["Olivia O.", "olivia@sevima-retail.id", "OO"], ["Dewi Pratiwi", "dewi@sevima-retail.id", "DD"], ["Qori A.", "qori@sevima-retail.id", "QQ"], ["Budi Santoso", "budi@sevima-retail.id", null]].map(([n, e, k]) => `<div class="lrow"><span class="grow pz-cell"><b class="b5">${n}</b><small>${e}</small></span>${ic("arrows-exchange", "s14 faint")}<span class="pz-w180">${AG.combo({ value: k || "", ph: "Belum dipetakan", options: peopleOpts([{ v: "", l: "Belum dipetakan" }]) })}</span></div>`).join("")}</div>
`)}`, jiraSave());
    const logPanel = card("Log sinkron", "24 jam terakhir. Perubahan yang ditolak gate dikembalikan ke status sebelumnya di Jira.", AG.datatable({ id: "pz-jlog", search: "Cari tiket atau pesan…", pageSize: 10,
      filters: [{ k: "res", label: "Hasil", icon: "circle-check", options: Object.values(JRES).map((x) => x[0]) }, { k: "dir", label: "Arah", icon: "arrows-exchange", options: ["Jira → Agent Gate", "Agent Gate → Jira"] }],
      right: btn("Unduh log", "ghost", "download", { "data-toast": "jira-sync-log.jsonl diunduh" }),
      columns: [{ label: "Waktu", w: "80px" }, { label: "Hasil", w: "150px" }, { label: "Tiket", w: "170px" }, { label: "Pesan", hide: "sm" }],
      rows: JLOG.map(([w, d, r, k, m]) => ({ cells: [`<span class="mono mut">${w}</span>`, `<span class="row g4 nowrap ${JRES[r][2]}">${ic(JRES[r][1], "s14")}${JRES[r][0]}</span>`, `<span class="mono sm nowrap">${k}</span>`, `<span class="mut">${m}</span>`], sort: [w, r, k, m], text: `${k} ${m}`, filter: { res: JRES[r][0], dir: d === "in" ? "Jira → Agent Gate" : "Agent Gate → Jira" } })) }));
    return AG.shell({ nav: "settings", crumbs: ["Pengaturan", "Jira"], width: "mid",
      body: subpage("jira", "overview", { title: "Jira", desc: "Connector Jira untuk impor, mirror, atau sinkron dua arah dengan Kasir Online.",
        nav: [{ k: "overview", label: "Ringkasan", icon: "layout-dashboard" }, { k: "conn", label: "Koneksi", icon: "plug" }, { k: "mode", label: "Mode sinkron", icon: "arrows-exchange" }, { k: "status", label: "Status ↔ stage", icon: "route", count: 1, alert: true }, { k: "fields", label: "Field & user", icon: "list-details" }, { k: "log", label: "Log sinkron", icon: "history", count: JLOG.length }],
        panels: [["overview", overview], ["conn", connPanel], ["mode", modePanel], ["status", statusPanel], ["fields", fieldsPanel], ["log", logPanel]] }) });
  } });
AG.action("pz-jmode", (el, k) => { PZ.jiraMode = k; AG.rerender(); });
AG.action("pz-jenv", (el, k) => { PZ.jiraEnv = k; AG.rerender(); });
AG.menu("pz-jira", () => `<button class="mi" data-toast="Sinkron dijeda">${ic("player-pause")}Jeda sinkron</button><button class="mi" data-toast="Sinkron penuh dijadwalkan">${ic("refresh")}Sinkron ulang semua</button><div class="msep"></div><button class="mi dng" data-toast="Koneksi Jira diputus. Data yang sudah diimpor tetap ada" data-tone="dng">${ic("plug")}Putuskan koneksi</button>`);

/* ====================== M11 · Impor CSV ====================== */
const CSTEPS = ["Unggah", "Pemetaan kolom", "Pratinjau", "Impor"];
const CSVCOL = [["Issue key", "Kode lama (alias)", "SR-311"], ["Summary", "Judul", "Bayar pakai QRIS"], ["Issue Type", "Tipe", "Story"], ["Status", "Stage", "In Review"], ["Assignee", "Penerima", "dewi@sevima-retail.id"], ["Priority", "Prioritas", "High"], ["Story Points", "Story point", "8"], ["Labels", "Label", "pembayaran"], ["Created", "Tanggal dibuat", "2026-10-02"], ["Toko", "Lewati kolom", "Cabang Dago"]];
const AGF = ["Kode lama (alias)", "Judul", "Deskripsi", "Tipe", "Stage", "Penerima", "Prioritas", "Story point", "Label", "Sprint", "Due date", "Tanggal dibuat"];
const PREV = [["2", "Bayar pakai QRIS", "story", "review", ""], ["3", "Validasi stok saat checkout", "story", "dev", ""], ["4", "Diskon per item", "story", "design", ""], ["9", "Ekspor transaksi ke CSV", "task", "dev", ""], ["17", "Struk tanpa nama kasir", "bug", "test", "warn|Penerima “budi@…” bukan anggota, dikosongkan"], ["22", "Scanner membaca dua kali", "bug", "backlog", ""], ["58", "", "task", "backlog", "dng|Judul kosong, dilewati"], ["63", "Pajak PPN per toko", "task", "backlog", ""], ["91", "Laporan harian via email", "story", "brainstorm", "warn|Status “Analysis” dipetakan ke Brainstorming"], ["120", "Mode offline", "story", "prd", ""], ["133", "Program poin pelanggan", "story", "backlog", "warn|Label “loyalty” dibuat sebagai label baru"], ["201", "Tes cetak", "task", "backlog", "dng|Baris duplikat dari baris 200, dilewati"]];
function csvStep() {
  const s = PZ.step.csv;
  if (s === 0) return `<div class="pz-drop" data-act="pz-step" data-arg="csv|1" role="button" tabindex="0">${ic("cloud-upload", "s24")}<b class="b5" style="color:var(--fg)">Seret berkas CSV ke sini, atau klik untuk memilih</b><span class="sm">UTF-8, pemisah koma atau titik koma, maksimal 20 MB</span></div>
    <div class="row wrap sm mut">${ic("download", "s14")}<button type="button" class="link" data-toast="Template CSV diunduh">Unduh template CSV</button><span>· Ekspor Jira, Trello, dan Linear bisa langsung dipakai.</span></div>`;
  if (s === 1) return `<div class="box"><div class="lrow">${ic("file-text", "faint")}<span class="grow pz-cell"><b class="b5">tiket-kasir-2026.csv</b><small>214 baris · 10 kolom · pemisah koma</small></span>${btn("Ganti", "sm", "", { "data-act": "pz-step", "data-arg": "csv|0" })}</div>
      <div class="lrow" style="min-height:56px"><span class="grow pz-cell"><b class="b5">Project tujuan</b><small>Tiket masuk ke Backlog project ini</small></span><span class="pz-w220">${AG.combo({ value: "KSR", options: AG.PROJECTS.map((p) => ({ v: p.k, l: p.n, sub: p.k, lead: AG.projDot(p.k) })) })}</span></div></div>
    <div class="alert">${ic("sparkles")}<span>9 kolom dipetakan otomatis. Kolom <b>Toko</b> bisa dibuat sebagai field custom baru.</span></div>
    ${AG.datatable({ id: "pz-csvmap", pageSize: 10, columns: [{ label: "Kolom CSV", w: "150px" }, { label: "Contoh", hide: "sm" }, { label: "Field Agent Gate", w: "240px", sort: false }],
      rows: CSVCOL.map(([c, f, ex]) => ({ cells: [`<span class="mono">${c}</span>`, `<span class="mut trunc">${ex}</span>`, AG.combo({ value: f, options: [...AGF.map((x) => ({ v: x, l: x })), { v: "Lewati kolom", l: "Lewati kolom", group: "Lainnya" }, { v: "new", l: "Field custom baru…", group: "Lainnya" }] })], sort: [c, ex, f] })) })}`;
  if (s === 2) return `${AG.stats([{ label: "Siap diimpor", value: "209", icon: "circle-check", tone: "ok", hint: "dari 214 baris" }, { label: "Peringatan", value: "3", icon: "alert-triangle", tone: "warn", hint: "tetap diimpor" }, { label: "Dilewati", value: "2", icon: "circle-x", tone: "dng", hint: "tidak diimpor" }])}
    ${AG.datatable({ id: "pz-csvprev", search: "Cari judul…", pageSize: 10, filters: [{ k: "res", label: "Hasil", icon: "filter", options: ["Siap", "Peringatan", "Dilewati"] }],
      columns: [{ label: "Baris", w: "70px" }, { label: "Judul" }, { label: "Tipe", w: "60px" }, { label: "Stage", hide: "sm", w: "150px" }, { label: "Catatan" }],
      rows: PREV.map(([r, t, ty, st, n]) => { const [tone, msg] = n.split("|"); return { cells: [`<span class="mono mut">${r}</span>`, t || `<span class="faint">—</span>`, typeIcon(ty), `<span class="row g4 nowrap">${stageIcon(st)}${AG.stage(st).n}</span>`, msg ? `<span class="text-${tone} sm">${msg}</span>` : `<span class="text-ok row g4 sm">${ic("check", "s14")}Siap</span>`], sort: [+r, t, ty, st, tone || "a"], text: `${t} ${msg || ""}`, filter: { res: !msg ? "Siap" : tone === "warn" ? "Peringatan" : "Dilewati" } }; }) })}`;
  return `<div class="box pad col g12"><div class="row"><span class="tile ok">${ic("circle-check")}</span><span class="grow pz-cell"><b class="b5">Impor selesai</b><small>209 tiket masuk ke Kasir Online · kode lama tersimpan sebagai alias</small></span></div><div class="bar ok"><i style="width:100%"></i></div>
    <div class="row wrap">${btn("Buka board", "pri", "layout-kanban", { "data-go": "#/p/KSR/board" })}${btn("Unduh laporan baris dilewati", "", "download", { "data-toast": "laporan-impor.csv diunduh" })}${btn("Impor berkas lain", "ghost", "plus", { "data-act": "pz-step", "data-arg": "csv|0" })}</div></div>`;
}
const CSVHIST = [["tiket-kasir-2026.csv", "OO", "Hari ini 10.20", "KSR", 209, 2, "Selesai"], ["trello-gudang.csv", "AA", "8 Nov 2026", "GDG", 58, 0, "Selesai"], ["linear-export.csv", "AA", "2 Nov 2026", "PPL", 0, 31, "Dibatalkan"]];
AG.screen({ m: "M11", g: "Integrasi", n: "Impor CSV", route: "/settings/import-csv",
  render: () => AG.shell({ nav: "settings", crumbs: ["Pengaturan", "Impor CSV"], width: "mid",
    body: subpage("csv", "new", { title: "Impor CSV", desc: "Pindahkan tiket dari spreadsheet atau alat lain. Kode tiket lama tetap bisa dicari sebagai alias.",
      nav: [{ k: "new", label: "Impor baru", icon: "file-import" }, { k: "hist", label: "Riwayat impor", icon: "history", count: CSVHIST.length }],
      panels: [
        ["new", card("Impor baru", `Langkah ${PZ.step.csv + 1} dari ${CSTEPS.length} · ${CSTEPS[PZ.step.csv]}`, `<div class="pz-stepw">${stepper("csv", CSTEPS)}</div><div class="col g12">${csvStep()}</div>`,
          PZ.step.csv < 3 ? `<div class="pz-foot">${stepNav("csv", CSTEPS.length, "").replace(/>Lanjut</, PZ.step.csv === 2 ? ">Impor 209 tiket<" : ">Lanjut<")}</div>` : "")],
        ["hist", card("Riwayat impor", "Laporan baris yang dilewati disimpan 30 hari.", AG.datatable({ id: "pz-csvhist", search: "Cari berkas…", pageSize: 10,
          columns: [{ label: "Berkas" }, { label: "Project", w: "130px", hide: "sm" }, { label: "Hasil", w: "160px" }, { label: "", w: "48px", sort: false }],
          rows: CSVHIST.map(([f, who, w, p, ok, skip, st]) => ({ cells: [`<span class="row g8">${ic("file-text", "faint")}<span class="pz-cell"><b class="b5 mono sm">${f}</b><small>${AG.PEOPLE[who].name} · ${w}</small></span></span>`, `<span class="row g4 nowrap">${AG.projDot(p)}${AG.project(p).n}</span>`, st === "Selesai" ? `<span class="pz-cell"><span class="text-ok">${ok} tiket masuk</span><small>${skip} baris dilewati</small></span>` : pill(st, "gray"), `<div style="text-align:right">${btn("", "ghost icon sm", "download", { "data-toast": `Laporan ${f} diunduh`, "aria-label": `Unduh laporan ${f}` })}</div>`], sort: [f, p, ok, 0], text: f })) }))],
      ] }) }) });

/* ====================== M11 · API & webhook ====================== */
const TOKENS = [
  { n: "Dashboard BI toko", pre: "ag_pat_7Kx2", sc: ["tiket:baca", "laporan:baca"], who: "OO", used: "12 mnt lalu", su: 1, exp: "12 Feb 2027", se: 20270212 },
  { n: "Script migrasi stok", pre: "ag_pat_Qm9d", sc: ["tiket:tulis", "tiket:baca"], who: "AA", used: "3 hari lalu", su: 3, exp: "30 Nov 2026", se: 20261130, warn: true },
  { n: "Bot WhatsApp internal", pre: "ag_pat_c41L", sc: ["tiket:baca", "komentar:tulis"], who: "DD", used: "Belum pernah", su: 99, exp: "Tanpa batas", se: 99999999 },
  { n: "Sinkron katalog produk", pre: "ag_pat_Hs20", sc: ["tiket:tulis"], who: "AA", used: "1 jam lalu", su: 2, exp: "1 Mar 2027", se: 20270301 },
  { n: "Laporan mingguan Sheets", pre: "ag_pat_W8pe", sc: ["laporan:baca"], who: "OO", used: "6 hari lalu", su: 4, exp: "15 Jan 2027", se: 20270115 },
];
const HOOKS = [
  { url: "https://bi.sevima-retail.id/hooks/agentgate", ev: ["ticket.updated", "sprint.completed"], st: "ok", last: "200 · 2 mnt lalu", on: true },
  { url: "https://hooks.slack-relay.internal/ksr", ev: ["release.published"], st: "dng", last: "500 · 1 jam lalu · gagal 3×", on: true },
  { url: "https://n8n.sevima-retail.id/webhook/stok", ev: ["ticket.created"], st: "off", last: "Dinonaktifkan", on: false },
];
const DELIV = [
  ["10.44", "ticket.updated", 0, 200, 182], ["10.31", "ticket.updated", 0, 200, 164], ["10.12", "sprint.completed", 0, 200, 240], ["09.40", "release.published", 1, 500, 3021],
  ["09.35", "release.published", 1, 500, 3007], ["09.30", "release.published", 1, 500, 3012], ["09.02", "ticket.updated", 0, 200, 158], ["08.47", "ticket.updated", 0, 200, 171],
  ["08.12", "ticket.created", 2, 410, 96], ["07.58", "ticket.updated", 0, 200, 149], ["07.30", "ticket.updated", 0, 200, 203], ["07.02", "sprint.completed", 0, 200, 188],
  ["06.40", "ticket.updated", 0, 200, 166], ["06.15", "ticket.updated", 0, 429, 51],
];
const EVENTS = ["ticket.created", "ticket.updated", "ticket.stage_changed", "ticket.deleted", "comment.created", "sprint.started", "sprint.completed", "release.published", "gate.rejected", "job.failed"];
const hostOf = (u) => u.replace(/^https:\/\//, "").split("/")[0];
AG.screen({ m: "M11", g: "Integrasi", n: "API & webhook", route: "/settings/api",
  render: () => {
    const tokens = section("Token API", "Cakupan membatasi apa yang bisa dilakukan token. Token ikut role pembuatnya dan disimpan sebagai digest.",
      AG.datatable({ id: "pz-tokens", search: "Cari token…", pageSize: 10, filters: [{ k: "who", label: "Pembuat", icon: "user", options: Object.values(AG.PEOPLE).map((p) => p.name) }],
        columns: [{ label: "Nama" }, { label: "Cakupan", hide: "sm" }, { label: "Terakhir dipakai", hide: "sm", w: "140px" }, { label: "Kedaluwarsa", w: "130px" }, { label: "", w: "80px", sort: false }],
        rows: TOKENS.map((t, i) => ({ cells: [`<div class="row">${av(t.who, 20)}<span class="pz-cell"><b class="b5">${t.n}</b><small class="mono">${t.pre}••••</small></span></div>`, `<div class="row wrap g4">${t.sc.map((s) => `<span class="pill gray mono">${s}</span>`).join("")}</div>`, `<span class="mut">${t.used}</span>`, `<span class="${t.warn ? "text-warn" : "mut"} nowrap">${t.exp}</span>`, `<div style="text-align:right">${btn("Cabut", "sm dng", "", { "data-dialog": "pz-revoke", "data-arg": i })}</div>`],
          sort: [t.n, t.sc.join(), t.su, t.se, 0], text: `${t.n} ${t.pre}`, filter: { who: AG.PEOPLE[t.who].name } })) }),
      btn("Token baru", "pri", "plus", { "data-dialog": "new-api-token" }));
    const hooks = section("Webhook", "Payload ditandatangani HMAC-SHA256 di header <code>X-AgentGate-Signature</code>. Pengiriman gagal diulang hingga 5 kali.",
      AG.datatable({ id: "pz-hooks", pageSize: 10, columns: [{ label: "Tujuan" }, { label: "Pengiriman terakhir", hide: "sm", w: "210px" }, { label: "Aktif", w: "70px", sort: false }, { label: "", w: "96px", sort: false }],
        rows: HOOKS.map((h, i) => ({ cells: [`<div class="col g4" style="min-width:0"><span class="mono sm trunc">${h.url}</span><span class="row wrap g4">${h.ev.map((e) => `<span class="pill gray mono">${e}</span>`).join("")}</span></div>`, `<span class="sm row g4 ${h.st === "dng" ? "text-dng" : h.st === "ok" ? "text-ok" : "mut"}">${ic(h.st === "dng" ? "circle-x" : h.st === "ok" ? "circle-check" : "player-pause", "s14")}${h.last}</span>`, sw(h.on, `Aktifkan webhook ${hostOf(h.url)}`), `<div class="row g4" style="justify-content:flex-end">${h.st === "dng" ? btn("", "sm icon", "refresh", { "data-toast": "Payload release.published dikirim ulang", "aria-label": "Kirim ulang" }) : ""}${btn("", "ghost icon sm", "dots", { "data-menu": "pz-hook", "aria-label": "Opsi webhook" })}</div>`], sort: [h.url, h.st, 0, 0] })) }),
      btn("Webhook baru", "pri", "plus", { "data-dialog": "new-webhook" }));
    const log = section("Log pengiriman", "30 hari terakhir. Buka baris untuk melihat payload dan respons.",
      AG.datatable({ id: "pz-deliv", search: "Cari event…", pageSize: 10,
        filters: [{ k: "ev", label: "Event", icon: "webhook", options: [...new Set(DELIV.map((d) => d[1]))] }, { k: "res", label: "Hasil", icon: "circle-check", options: ["Berhasil", "Gagal"] }, { k: "hook", label: "Tujuan", icon: "world", options: HOOKS.map((h) => hostOf(h.url)) }],
        columns: [{ label: "Waktu", w: "80px" }, { label: "Event" }, { label: "Tujuan", hide: "sm" }, { label: "Status", w: "90px" }, { label: "Durasi", hide: "sm", w: "90px", align: "right" }, { label: "", w: "64px", sort: false }],
        rows: DELIV.map(([w, ev, h, code, ms], i) => ({ cells: [`<span class="mono mut">${w}</span>`, `<span class="mono sm">${ev}</span>`, `<span class="mut trunc">${hostOf(HOOKS[h].url)}</span>`, `<span class="pill ${code < 300 ? "ok" : "dng"} mono">${code}</span>`, `<span class="mono mut">${ms} ms</span>`, `<div style="text-align:right">${btn("Lihat", "sm ghost", "", { "data-dialog": "pz-deliv", "data-arg": i })}</div>`],
          sort: [w, ev, h, code, ms, 0], text: `${ev} ${HOOKS[h].url}`, filter: { ev, res: code < 300 ? "Berhasil" : "Gagal", hook: hostOf(HOOKS[h].url) } })) }));
    const docs = card("Dokumentasi", "REST API JSON. Spesifikasi lengkap tersedia di OpenAPI.", `${sub("Endpoint", "", `<div class="box"><div class="lrow"><span class="tile">${ic("api")}</span><span class="grow pz-cell"><b class="b5">Base URL</b><small class="mono">https://app.agentgate.id/api/v1</small></span>${btn("", "ghost icon sm", "copy", { "data-act": "pz-copy", "data-arg": "https://app.agentgate.id/api/v1", "aria-label": "Salin base URL" })}</div>
        <div class="lrow"><span class="tile">${ic("key")}</span><span class="grow pz-cell"><b class="b5">Autentikasi</b><small>Header <code>Authorization: Bearer &lt;token&gt;</code></small></span></div>
        <div class="lrow"><span class="tile">${ic("gauge")}</span><span class="grow pz-cell"><b class="b5">Batas pemakaian</b><small>600 request per menit per token · header <code>X-RateLimit-Remaining</code></small></span></div>
        <div class="lrow"><span class="tile">${ic("plug-connected")}</span><span class="grow pz-cell"><b class="b5">Client AI dan runner</b><small>Codex, Gemini CLI, Cursor, OpenCode, Copilot CLI memakai MCP dan runner, bukan token API.</small></span><a class="link sm nowrap" href="#/settings/connect">Hubungkan client</a></div></div>`)}
      ${sub("Contoh request", "", `<div class="code">curl -H "Authorization: Bearer ag_pat_…" \\\n  "https://app.agentgate.id/api/v1/tickets?project=KSR&stage=review"</div>`)}`, "",
      `${btn("", "ghost icon sm", "download", { "data-toast": "openapi-agentgate-v1.yaml diunduh", "aria-label": "Unduh OpenAPI" })}${btn("Dokumentasi REST", "sm", "external-link", { "data-toast": "Membuka docs.agentgate.id/api" })}`);
    return AG.shell({ nav: "settings", crumbs: ["Pengaturan", "API & webhook"], width: "mid",
      body: subpage("api", "tokens", { title: "API & webhook", desc: "REST API JSON untuk integrasi pihak ketiga, dan webhook untuk menerima event dari Agent Gate.",
        nav: [{ k: "tokens", label: "Token API", icon: "key", count: TOKENS.length }, { k: "hooks", label: "Webhook", icon: "webhook", count: 1, alert: true }, { k: "log", label: "Log pengiriman", icon: "history" }, { k: "docs", label: "Dokumentasi", icon: "book" }],
        panels: [["tokens", tokens], ["hooks", hooks], ["log", log], ["docs", docs]] }) });
  } });
AG.dialog("pz-deliv", (i) => {
  const [w, ev, h, code, ms] = DELIV[+i] || DELIV[3], ok = code < 300;
  return { title: `${ev} · ${code}`, desc: `${HOOKS[h].url} · hari ini ${w} · ${ms} ms`, icon: ok ? "circle-check" : "circle-x", tone: ok ? "ok" : "dng", w: "w640",
    body: `${fld("Payload", `<div class="code" style="white-space:pre-wrap">{\n  "event": "${ev}",\n  "project": "KSR",\n  "ticket": { "key": "KSR-24", "stage": "review" },\n  "at": "2026-11-12T${w.replace(".", ":")}:08+07:00"\n}</div>`)}
      ${fld("Respons", `<div class="code" style="white-space:pre-wrap">HTTP ${code}${ok ? " OK\n{\"received\": true}" : code === 429 ? " Too Many Requests" : code === 410 ? " Gone" : " Internal Server Error\nupstream relay timeout"}</div>`)}
      ${ok ? "" : `<div class="alert warn">${ic("alert-triangle")}<span>Pengiriman diulang otomatis dengan jeda bertahap. Setelah 5 kali gagal, webhook dinonaktifkan dan Owner diberi tahu.</span></div>`}`,
    foot: `${btn("Tutup", "", "", { "data-close": true })}${ok ? "" : btn("Kirim ulang", "pri", "refresh", { "data-close": true, "data-toast": `${ev} dikirim ulang` })}` };
});
AG.menu("pz-hook", () => `<button class="mi" data-toast="Menampilkan riwayat pengiriman webhook ini">${ic("history")}Riwayat pengiriman</button><button class="mi" data-toast="Ping terkirim · 200 OK">${ic("send")}Kirim event uji</button><button class="mi" data-toast="Secret baru dibuat">${ic("key")}Rotasi secret</button><div class="msep"></div><button class="mi dng" data-toast="Webhook dihapus" data-tone="dng">${ic("trash")}Hapus</button>`);
AG.dialog("pz-revoke", (i) => ({ title: `Cabut token “${TOKENS[+i].n}”?`, desc: "Integrasi yang memakai token ini langsung berhenti. Tindakan ini tidak bisa dibatalkan.", icon: "key-off", tone: "dng",
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Cabut token", "dngs", "", { "data-close": true, "data-toast": "Token dicabut" })}` }));
const SCOPES = [["tiket:baca", "Membaca tiket, komentar, riwayat"], ["tiket:tulis", "Membuat dan mengubah tiket (tetap lewat gate)"], ["komentar:tulis", "Menulis komentar"], ["laporan:baca", "Membaca laporan dan sprint"], ["knowledge:baca", "Membaca knowledge base"], ["webhook:kelola", "Mengatur webhook"]];
const projOpts = (all = true) => [...(all ? [{ v: "all", l: "Semua project" }] : []), ...AG.PROJECTS.map((p) => ({ v: p.k, l: p.n, sub: p.k, lead: AG.projDot(p.k) }))];
AG.dialog("new-api-token", () => ({ title: "Token API baru", icon: "key", tone: "acc", w: "w560",
  body: `${fld("Nama", inp("", { ph: "Misalnya: Dashboard BI toko" }))}
    ${fld("Cakupan", `<div class="box">${SCOPES.map(([s, d], i) => `<div class="lrow" style="min-height:42px">${cb(i === 0, s)}<span class="grow"><span class="mono">${s}</span> <span class="mut sm">· ${d}</span></span></div>`).join("")}</div>`)}
    <div class="grid2">${fld("Kedaluwarsa", sel(["30 hari", "90 hari", "1 tahun", "Tanpa batas"], "90 hari"))}${fld("Project", AG.combo({ value: "all", options: projOpts() }))}</div>`,
  foot: `${btn("Batal", "", "", { "data-close": true })}${btn("Buat token", "pri", "", { "data-act": "pz-token" })}` }));
AG.action("pz-token", (el) => {
  const d = el.closest(".dialog"), tok = "ag_pat_Vn4R8sYw2LqT6mKc9HzE1bXa7DfJ3pUo";
  d.querySelector(".dh").innerHTML = `<span class="tile lg ok">${ic("circle-check", "s18")}</span><div class="grow"><h2 id="dlg-t">Token dibuat</h2><p>Salin sekarang. Token ini tidak akan ditampilkan lagi.</p></div>`;
  d.querySelector(".db").innerHTML = `<div class="pz-tok"><span class="grow">${tok}</span>${btn("Salin", "sm", "copy", { "data-act": "pz-copy", "data-arg": tok })}</div>
    <div class="alert warn">${ic("alert-triangle")}<span>Simpan di secret manager. Kalau hilang, cabut token ini dan buat yang baru.</span></div>
    <div class="code">curl -H "Authorization: Bearer ${tok.slice(0, 14)}…" \\\n  https://app.agentgate.id/api/v1/tickets?project=KSR</div>`;
  d.querySelector(".df").innerHTML = btn("Sudah saya simpan", "pri", "", { "data-close": true });
});
AG.dialog("new-webhook", () => ({ title: "Webhook baru", icon: "webhook", tone: "acc", w: "w560",
  body: `${fld("URL tujuan", inp("", { ph: "https://contoh.id/hooks/agentgate" }), "Harus HTTPS.")}
    ${fld("Secret", `<div class="row">${inp("whsec_3fa9c1e07b", { cls: "mono" })}${btn("", "icon", "refresh", { "data-toast": "Secret baru dibuat", "aria-label": "Buat secret baru" })}</div>`)}
    ${fld("Event", AG.combo({ multi: true, value: ["ticket.stage_changed"], ph: "Pilih event…", options: EVENTS.map((e) => ({ v: e, l: e, group: e.split(".")[0] })) }))}
    ${fld("Project", AG.combo({ multi: true, value: ["all"], options: projOpts() }))}`,
  foot: `${btn("Kirim event uji", "ghost", "send", { "data-toast": "Ping terkirim · 200 OK dalam 182 ms" })}<span class="grow"></span>${btn("Batal", "", "", { "data-close": true })}${btn("Buat webhook", "pri", "", { "data-close": true, "data-toast": "Webhook dibuat" })}` }));

/* ====================== M11 · Ekspor ====================== */
const EXPORTS = [["Hari ini, 09.12", 20261112, "OO", "proc", 62, "—"], ["5 Nov 2026", 20261105, "AA", "ready", null, "171 MB"], ["29 Okt 2026", 20261029, "OO", "exp", null, "168 MB"], ["12 Okt 2026", 20261012, "OO", "exp", null, "152 MB"], ["1 Okt 2026", 20261001, "AA", "fail", null, "—"]];
const EXST = { proc: ["Sedang diproses", "acc"], ready: ["Siap diunduh", "ok"], exp: ["Kedaluwarsa", "gray"], fail: ["Gagal", "dng"] };
AG.screen({ m: "M11", g: "Integrasi", n: "Ekspor data", route: "/settings/export",
  render: () => {
    const fresh = card("Ekspor baru", "Data terstruktur dalam JSON, dokumen dalam Markdown, dikemas jadi satu berkas ZIP.", `<div class="alert gray">${ic("shield-check")}<span>Hanya Owner dan Admin yang bisa mengekspor. Setiap ekspor dicatat di <a class="link" href="#/settings/audit">audit log</a>.</span></div>
      ${sub("Isi ekspor", "", `<div class="box">${[["Tiket, komentar, riwayat", "JSON per project, relasi dan field custom ikut", true], ["Artifact", "PRD, tech spec, mockup HTML sebagai Markdown/HTML beserta versinya", true], ["Knowledge base", "Folder .agentgate/knowledge/ per project", true], ["Lampiran dan evidence", "Screenshot, log test. Bisa besar (±2,4 GB)", false], ["Workflow, persona, skill pack", "YAML yang bisa diimpor ulang", true], ["Audit log", "JSON Lines 12 bulan terakhir", false]].map(([t, d, on]) => `<div class="lrow">${cb(on, t)}<span class="grow pz-cell"><b class="b5">${t}</b><small>${d}</small></span></div>`).join("")}</div>`)}
      ${sub("Cakupan", "", `<div class="box">${srow("Project", "Pilih satu atau beberapa project.", `<span class="pz-w220">${AG.combo({ multi: true, value: ["all"], options: projOpts() })}</span>`)}${srow("Kirim link ke", "Link unduhan berlaku 7 hari.", `<span class="pz-w220">${AG.combo({ value: "OO", options: peopleOpts().filter((o) => ["OO", "AA"].includes(o.v)) })}</span>`)}</div>`)}
`, AG.savebar("Mulai ekspor", { msg: "Perkiraan 186 MB · diproses di worker, Anda dapat notifikasi saat siap", cancel: false, toast: "Ekspor dimulai. Anda dapat notifikasi saat siap" }));
    const hist = section("Riwayat ekspor", "Berkas disimpan 7 hari setelah selesai.", AG.datatable({ id: "pz-exports", pageSize: 10, filters: [{ k: "st", label: "Status", icon: "filter", options: Object.values(EXST).map((x) => x[0]) }], search: "Cari…",
      columns: [{ label: "Dibuat" }, { label: "Oleh", hide: "sm", w: "150px" }, { label: "Status", w: "200px" }, { label: "Ukuran", hide: "sm", w: "90px", align: "right" }, { label: "", w: "90px", sort: false }],
      rows: EXPORTS.map(([w, sd, who, st, p, size]) => ({ cells: [`<b class="b5">${w}</b>`, `<span class="row g8 nowrap">${av(who, 20)}${AG.PEOPLE[who].name}</span>`, p != null ? `<span class="col g4"><span class="bar" style="max-width:160px"><i style="width:${p}%"></i></span><span class="sm mut">${p}% · knowledge base</span></span>` : pill(EXST[st][0], EXST[st][1]), `<span class="mut">${size}</span>`, `<div style="text-align:right">${st === "ready" ? btn("Unduh", "sm", "download", { "data-toast": "agentgate-ptcontoh-2026-11-05.zip diunduh" }) : st === "fail" ? btn("Ulangi", "sm ghost", "refresh", { "data-toast": "Ekspor diulang" }) : ""}</div>`],
        sort: [sd, AG.PEOPLE[who].name, st, parseInt(size) || 0, 0], text: `${w} ${AG.PEOPLE[who].name}`, filter: { st: EXST[st][0] } })) }));
    return AG.shell({ nav: "settings", crumbs: ["Pengaturan", "Ekspor data"], width: "mid",
      body: subpage("export", "new", { title: "Ekspor data", desc: "Unduh seluruh data workspace PT Contoh untuk cadangan atau pindah server.",
        nav: [{ k: "new", label: "Ekspor baru", icon: "file-export" }, { k: "hist", label: "Riwayat ekspor", icon: "history", count: EXPORTS.length }],
        panels: [["new", fresh], ["hist", hist]] }) });
  } });

/* ====================== M12 · Halaman instal CLI ====================== */
const SHA = [["agentgate-aarch64-apple-darwin.tar.xz", "9f2c4e1a7b03d58e6c41f0a2b7d9e3c5a18f6b20d4e7c9a1b3f5d8e2c0a6b4f1"], ["agentgate-x86_64-apple-darwin.tar.xz", "1b7d03e5c9a2f4d68e0b1c3a5f7d9e2b4c6a8f0d1e3b5c7a9f2d4e6b8c0a1e3d"], ["agentgate-x86_64-unknown-linux-gnu.tar.xz", "c4a81f0e3b6d29a5f7c1e8d04b2a6c9f3e5d7b1a8c0f2e4d6b9a3c5e7f1d0b2a"], ["agentgate-aarch64-unknown-linux-gnu.tar.xz", "e0d6b2f8a4c1937e5b0d2f6a8c4e1b3d5f7a9c2e4b6d8f0a1c3e5b7d9f2a4c6e"], ["agentgate-x86_64-pc-windows-msvc.zip", "5e3c9a17f2d40b8e6a1c5f3d7b9e2a4c6f8d0b1e3a5c7f9d2b4e6a8c0f1d3b5a"]];
const cmd = (c, p = "$") => `<div class="pz-cmd"><span class="p">${p}</span><code>${esc(c)}</code>${btn("Salin", "sm", "copy", { "data-act": "pz-copy", "data-arg": c })}</div>`;
AG.screen({ m: "M12", g: "Rilis", n: "Instal CLI (publik)", route: "/install",
  render: () => `<div class="pz-inst">
    <header class="pz-ihd"><a class="logo" href="#/install"><span class="mk">${ic("route", "s14")}</span>Agent Gate</a><span class="grow"></span><button type="button" class="link hide-sm" data-toast="Membuka docs.agentgate.id">Dokumentasi</button><button type="button" class="link hide-sm" data-toast="Membuka github.com/agentgate/agentgate/releases">GitHub</button>${btn("Masuk", "", "", { "data-go": "#/login" })}</header>
    <main class="pz-iin">
      <div class="col g12"><span class="row g4">${pill("v0.4.2 · 12 Nov 2026", "acc", "", "rocket")}<button type="button" class="link sm" data-toast="Membuka catatan rilis v0.4.2">Catatan rilis</button></span>
        <h1>Instal CLI agentgate</h1><p class="mut md" style="max-width:60ch">Satu binary berisi installer aturan, indexer code graph, dan runner yang menjalankan client AI di laptop Anda. Windows, macOS, dan Linux.</p></div>
      <section class="cardx" data-tabs-root><header class="cx-h"><div class="grow"><h2>Pasang</h2><p>Pilih sistem operasi, lalu jalankan perintahnya di terminal.</p></div></header>
        <nav class="tabs" role="tablist" style="padding:0 16px;margin-top:0">${[["unix", "macOS / Linux", "terminal-2"], ["win", "Windows", "device-desktop"], ["brew", "Homebrew", "package"]].map(([k, n, i], j) => `<button role="tab" data-tab="${k}" class="${j ? "" : "on"}">${ic(i, "s14")}${n}</button>`).join("")}</nav>
        <div style="padding:16px">
        <div data-panel="unix" class="on col">${cmd("curl -fsSL https://agentgate.id/install.sh | sh")}<span class="mut sm">Memasang ke <code>~/.agentgate/bin</code> dan menambahkannya ke PATH. Tidak butuh sudo.</span></div>
        <div data-panel="win" class="col">${cmd("irm https://agentgate.id/install.ps1 | iex", "PS>")}<span class="mut sm">Jalankan di PowerShell biasa (bukan administrator). Scoop dan winget menyusul.</span></div>
        <div data-panel="brew" class="col">${cmd("brew install agentgate/tap/agentgate")}<span class="mut sm">Tap dibuat otomatis oleh pipeline rilis. Perbarui dengan <code>brew upgrade agentgate</code>.</span></div>
        </div>
      </section>
      ${section("Langkah berikutnya", "", `<div class="box pad col g16">${[["Masuk dengan akun Agent Gate", "Membuka browser untuk konfirmasi kode perangkat.", "agentgate login"], ["Pasang aturan di repo", "Membuat backup aturan lama lalu memasang aturan workspace.", "agentgate init"], ["Nyalakan runner", "Menerima job dari board. Runner tidak pernah menyala otomatis.", "agentgate runner start"]].map(([t, d, c], i) => `<div class="pz-nb"><span class="step">${i + 1}</span><div class="col"><span><b class="b5">${t}</b> <span class="mut">· ${d}</span></span>${cmd(c)}</div></div>`).join("")}</div>`)}
      ${section("Verifikasi unduhan", "Installer mengecek checksum SHA-256 otomatis. Setiap rilis juga disertai artifact attestation GitHub.", `<div class="alert gray">${ic("info-circle")}<span>Binary belum ditandatangani Apple Developer ID atau sertifikat Windows. macOS mungkin meminta konfirmasi saat pertama dijalankan dari unduhan manual; jalur script di atas tidak terpengaruh.</span></div><div class="box">${SHA.map(([f, h]) => `<div class="lrow" style="align-items:flex-start;padding-top:10px;padding-bottom:10px"><span class="grow col g4"><span class="mono sm b5">${f}</span><span class="pz-sha">${h}</span></span>${btn("", "ghost icon sm", "copy", { "data-act": "pz-copy", "data-arg": h, "aria-label": `Salin checksum ${f}` })}</div>`).join("")}</div>
        <div class="box pad col"><span class="b5">Cek attestation</span>${cmd("gh attestation verify agentgate-x86_64-unknown-linux-gnu.tar.xz --repo agentgate/agentgate")}</div>`)}
      ${section("Pembaruan", "", `<div class="box pad col">${cmd("agentgate self-update")}<span class="mut sm">Runner memberi tahu di web app saat versi baru tersedia. Versi minimum yang didukung server saat ini: 0.4.0.</span></div>`)}
    </main></div>` });

/* ====================== M12 · Status sistem ====================== */
const up = (seed, warnAt = [], dngAt = []) => `<div class="pz-up" aria-label="Riwayat 60 hari">${Array.from({ length: 60 }, (_, i) => { const d = 60 - i; const cls = dngAt.includes(i) ? "d" : warnAt.includes(i) ? "w" : ""; return `<i class="${cls}" data-tip="${d === 1 ? "Kemarin" : d + " hari lalu"} · ${cls === "d" ? "Gangguan 14 menit" : cls === "w" ? "Lambat" : "Normal"}"></i>`; }).join("")}</div>`;
const sysRow = (i, n, st, meta, bar = "") => `<div class="pz-sys"><div class="row">${ic(i, "faint")}<b class="b5">${n}</b></div><span class="row g4 sm ${st[0] === "ok" ? "text-ok" : st[0] === "warn" ? "text-warn" : "text-dng"}">${ic(st[0] === "ok" ? "circle-check" : "alert-triangle", "s14")}${st[1]}</span>${meta ? `<div class="meta">${meta.map((m) => `<span>${m}</span>`).join("")}</div>` : ""}${bar}</div>`;
const ERRS = [["10.31", "worker", "error", "knowledge_scan job 8812: timeout runner Laptop Dewi setelah 300 dtk"], ["09.58", "web", "error", "POST /api/v1/tickets 413: lampiran melebihi 25 MB"], ["09.40", "worker", "warning", "webhook release.published ke hooks.slack-relay.internal gagal (500), dijadwalkan ulang"], ["08.12", "web", "warning", "Login gagal 5 kali dari 103.21.44.9, akun dikunci 15 menit"], ["07.55", "worker", "warning", "jira_sync: user budi@sevima-retail.id belum dipetakan"], ["06.02", "runner", "warning", "Runner ThinkPad Adi terputus tanpa sinyal berhenti"], ["02.00", "scheduler", "error", "backup_postgres: S3 AccessDenied (bucket ag-backup-offsite)"], ["01.30", "worker", "warning", "graph_index kasir-web: 2 file dilewati (lebih dari 2 MB)"], ["00.12", "scheduler", "warning", "Scan drift aturan: 1 repo berbeda dari workspace"]];
const BACKUPS = [["12 Nov 02.00", 20261112, "fail", "—", "S3 AccessDenied"], ["11 Nov 02.00", 20261111, "fail", "—", "S3 AccessDenied"], ["10 Nov 02.00", 20261110, "ok", "1,1 GB", "4 mnt 12 dtk"], ["9 Nov 02.00", 20261109, "ok", "1,1 GB", "4 mnt 3 dtk"], ["8 Nov 02.00", 20261108, "ok", "1,0 GB", "3 mnt 58 dtk"], ["7 Nov 02.00", 20261107, "ok", "1,0 GB", "3 mnt 51 dtk"], ["6 Nov 02.00", 20261106, "ok", "1,0 GB", "3 mnt 49 dtk"]];
AG.screen({ m: "M12", g: "Rilis", n: "Status sistem", route: "/settings/system",
  render: () => {
    const sum = card("Ringkasan", "Diperiksa 12 Nov 2026, 10.45 WIB.", `<div class="pz-hero warn"><span class="tile lg warn">${ic("alert-triangle", "s18")}</span><span class="grow"><b>Berjalan normal, 1 peringatan</b><br><span class="sm">Backup terakhir lebih dari 24 jam lalu.</span></span></div>
      ${AG.stats([{ label: "Uptime", value: "6 hari", icon: "activity", tone: "ok", hint: "deploy 6 Nov 06.12" }, { label: "Runner aktif", value: "3 / 4", icon: "device-laptop", hint: "ThinkPad Adi offline", go: "#/settings/runner" }, { label: "Job menunggu", value: "14", icon: "hourglass", hint: "tertua 2 menit" }, { label: "Job gagal 24 jam", value: "2", icon: "alert-circle", tone: "dng", hint: "Lihat log error" }])}
      ${sub("Versi", "", `<div class="box"><dl class="pz-kv"><dt>Versi server</dt><dd class="row wrap"><span class="mono">agentgate-server 0.9.0</span>${pill("terbaru", "ok")}</dd><dt>Image</dt><dd class="mono sm mut">ghcr.io/agentgate/server@sha256:4be1…c09a</dd><dt>Commit</dt><dd class="mono">release · 8c41f2e</dd><dt>CLI minimum</dt><dd>0.4.0 · <a class="link" href="#/install">halaman instal</a></dd></dl></div>`)}
      ${sub("Perlu perhatian", "", `<div class="box">
        <button class="lrow" style="width:100%;text-align:left" data-tab="backup">${ic("alert-triangle", "text-warn")}<span class="grow pz-cell"><b class="b5">Backup Postgres gagal 2 kali</b><small>Kredensial bucket luar VPS kedaluwarsa</small></span>${ic("chevron-right", "s14 faint")}</button>
        <button class="lrow" style="width:100%;text-align:left" data-tab="err">${ic("alert-circle", "text-dng")}<span class="grow pz-cell"><b class="b5">3 error dalam 24 jam</b><small>worker, web, scheduler</small></span>${ic("chevron-right", "s14 faint")}</button></div>`)}`, "", btn("Periksa ulang", "sm", "refresh", { "data-toast": "Status diperbarui" }));
    const svc = card("Layanan", "Riwayat 60 hari. Arahkan kursor ke batang untuk detail.", `<div class="box">
        ${sysRow("world", "Web", ["ok", "Normal"], ["2 container", "p95 142 ms", "18 req/s"], up(1, [21]))}
        ${sysRow("cpu", "Worker", ["ok", "Normal"], ["4 thread", "indexing graph, embedding, email"], up(2))}
        ${sysRow("clock", "Scheduler", ["ok", "Normal"], ["7 jadwal", "berikutnya: scan drift aturan 11.00"], up(3))}
        ${sysRow("activity", "Realtime LISTEN/NOTIFY", ["ok", "Normal"], ["p50 18 ms", "p95 64 ms", "412 listener (browser 396, runner 16)"], `<div style="grid-column:1/-1">${lineSpark()}</div>`)}
      </div>
      ${sub("Antrean job", "Menunggu 14 · berjalan 3 · selesai 24 jam 1.284", AG.datatable({ id: "pz-queues", pageSize: 10, columns: [{ label: "Antrean" }, { label: "Menunggu", w: "100px", align: "right" }, { label: "Berjalan", w: "100px", align: "right" }, { label: "Gagal 24 jam", w: "120px", align: "right", hide: "sm" }, { label: "Rata-rata durasi", hide: "sm", w: "140px", align: "right" }],
        rows: [["graph_index", 6, 1, 0, "48 dtk", 48], ["embedding", 5, 1, 0, "3,1 dtk", 3.1], ["mailer", 0, 0, 0, "0,8 dtk", 0.8], ["knowledge_scan", 2, 1, 1, "4 mnt", 240], ["jira_sync", 1, 0, 0, "12 dtk", 12], ["webhook", 0, 0, 1, "0,4 dtk", 0.4]].map(([q, w, r, f, d, ds]) => ({ cells: [`<span class="mono">${q}</span>`, w, r, f ? `<span class="text-dng">${f}</span>` : `<span class="mut">0</span>`, `<span class="mut">${d}</span>`], sort: [q, w, r, f, ds] })) }))}
      ${sub("Infrastruktur", "", `<div class="box">
        ${srow("Domain preview mockup", "<span class=\"mono\">*.preview.agentgate-mock.id</span> · TLS berlaku hingga 4 Feb 2027 · terisolasi dari domain aplikasi", pill("Aktif", "ok", "", "check"))}
        ${srow("SMTP", "smtp.mailgun.org:587 · pengirim no-reply@agentgate.id · 212 email 24 jam, 0 gagal", btn("Kirim email uji", "", "mail", { "data-toast": "Email uji dikirim ke owner@agentgate.test" }))}
      </div>`)}`);
    const db = card("Database & storage", "Postgres untuk data aplikasi, SeaweedFS (S3) untuk berkas.", `${sub("Postgres", "", `<div class="box">${sysRow("database", "Postgres 18.1 + pgvector 0.8", ["ok", "Normal"], ["38 dari 100 koneksi", "ukuran 4,2 GB", "cache hit 99,2%"], up(4, [], [44]))}</div>
        <div class="box"><dl class="pz-kv"><dt>Migrasi terakhir</dt><dd class="mono sm">m20261106_runner_quota</dd><dt>Ekstensi</dt><dd>pgvector 0.8, pg_trgm, pgcrypto</dd><dt>Tabel terbesar</dt><dd>job_events 1,3 GB · embeddings 1,1 GB · tickets_history 0,6 GB</dd></dl></div>`)}
      ${sub("Storage S3 · SeaweedFS", "212 GB dari 500 GB terpakai", `<div class="box pad col g12"><div class="bar"><i style="width:42%"></i></div>
        ${[["Evidence (screenshot, log test)", 61, "var(--accent)"], ["Lampiran tiket", 24, "#0ea5e9"], ["Artifact & mockup", 11, "#db2777"], ["Lainnya", 4, "var(--faint)"]].map(([n, p, c]) => `<div class="pz-hb" style="grid-template-columns:minmax(0,1fr) 120px 48px"><span class="trunc">${n}</span><div class="tr"><i style="width:${p}%;background:${c}"></i></div><span class="mono sm mut" style="text-align:right">${p}%</span></div>`).join("")}
        <span class="mut sm">Evidence lebih dari 90 hari dipindah ke kelas penyimpanan murah. Sekitar 18 GB bisa dibersihkan.</span></div>`)}`);
    const backup = `${card("Backup", "Snapshot Postgres ke bucket di luar VPS aplikasi.", `<div class="alert warn">${ic("alert-triangle")}<span>Backup terakhir berhasil 10 Nov 02.00 (31 jam lalu). Backup 11 dan 12 Nov gagal: kredensial bucket luar VPS kedaluwarsa.</span></div>
      ${sub("Backup Postgres", "", `<div class="box">${srow("Backup manual", "Membuat snapshot sekarang ke bucket ag-backup-offsite.", btn("Backup sekarang", "", "database", { "data-toast": "Backup dimulai" }))}${srow("Pulihkan", "Pemulihan dilakukan dari CLI server agar tidak terpicu tak sengaja.", btn("Panduan pemulihan", "ghost", "book", { "data-toast": "Membuka docs.agentgate.id/restore" }))}</div>`)}
      ${sub("Jadwal & tujuan", "", `<div class="box">
        ${srow("Jadwal", "Waktu server WIB.", `<span class="pz-w220">${AG.combo({ value: "daily", options: [{ v: "6h", l: "Setiap 6 jam" }, { v: "daily", l: "Setiap hari 02.00" }, { v: "weekly", l: "Setiap Minggu 02.00" }] })}</span>`)}
        ${srow("Simpan selama", "Backup yang lebih lama dihapus otomatis.", `<span class="pz-w220">${AG.combo({ value: "30", options: [["7", "7 hari"], ["30", "30 hari"], ["90", "90 hari"], ["365", "1 tahun"]].map(([v, l]) => ({ v, l })) })}</span>`)}
        ${srow("Bucket tujuan", "Disarankan di luar VPS aplikasi.", inp("s3://ag-backup-offsite/postgres", { cls: "mono" }))}
        ${srow("Access key", "Kedaluwarsa 10 Nov 2026. Ganti dengan key baru.", inp("", { type: "password", ph: "Access key baru" }))}
        ${srow("Enkripsi", "AES-256 dengan kunci di secret server.", sw(true, "Enkripsi backup"))}
      </div>`)}`, AG.savebar("Simpan pengaturan backup", { toast: "Pengaturan backup disimpan", msg: "Access key belum diisi" }))}
      ${section("Riwayat backup", "", AG.datatable({ id: "pz-backups", pageSize: 10, filters: [{ k: "st", label: "Hasil", icon: "filter", options: ["Berhasil", "Gagal"] }], search: "Cari…",
        columns: [{ label: "Waktu" }, { label: "Hasil", w: "120px" }, { label: "Ukuran", w: "100px", align: "right", hide: "sm" }, { label: "Keterangan", hide: "sm" }],
        rows: BACKUPS.map(([w, sd, st, size, note]) => ({ cells: [`<b class="b5">${w}</b>`, st === "ok" ? pill("Berhasil", "ok", "", "check") : pill("Gagal", "dng", "", "x"), `<span class="mut">${size}</span>`, `<span class="${st === "ok" ? "mut" : "text-dng"}">${note}</span>`], sort: [sd, st, parseFloat(size.replace(",", ".")) || 0, note], text: `${w} ${note}`, filter: { st: st === "ok" ? "Berhasil" : "Gagal" } })) }))}`;
    const err = section("Log error", "24 jam terakhir dari web, worker, scheduler, dan runner.", AG.datatable({ id: "pz-errs", search: "Cari pesan…", pageSize: 10,
      filters: [{ k: "src", label: "Sumber", icon: "server", options: ["web", "worker", "scheduler", "runner"] }, { k: "lvl", label: "Level", icon: "alert-circle", options: ["error", "warning"] }],
      right: btn("Unduh log 24 jam", "ghost", "download", { "data-toast": "agentgate-logs.jsonl diunduh" }),
      columns: [{ label: "Waktu", w: "80px" }, { label: "Level", w: "100px" }, { label: "Sumber", w: "110px" }, { label: "Pesan" }],
      rows: ERRS.map(([w, s, l, m]) => ({ cells: [`<span class="mono mut">${w}</span>`, `<span class="row g4 ${l === "error" ? "text-dng" : "text-warn"}">${ic(l === "error" ? "alert-circle" : "alert-triangle", "s14")}${l}</span>`, `<span class="pill gray mono">${s}</span>`, `<span class="sm" style="overflow-wrap:anywhere">${m}</span>`], sort: [w, l, s, m], text: m, filter: { src: s, lvl: l } })) }));
    return AG.shell({ nav: "settings", crumbs: ["Pengaturan", "Status sistem"], width: "mid",
      body: subpage("sys", "sum", { title: "Status sistem", desc: "Kesehatan server Agent Gate. Hanya terlihat oleh Owner.",
        nav: [{ k: "sum", label: "Ringkasan", icon: "heart-rate-monitor" }, { k: "svc", label: "Layanan", icon: "server" }, { k: "db", label: "Database & storage", icon: "database" }, { k: "backup", label: "Backup", icon: "cloud", count: 1, alert: true }, { k: "err", label: "Log error", icon: "alert-circle", count: ERRS.filter((e) => e[2] === "error").length }],
        panels: [["sum", sum], ["svc", svc], ["db", db], ["backup", backup], ["err", err]] }) });
  } });
function lineSpark() {
  const v = [16, 18, 17, 21, 19, 18, 24, 31, 22, 18, 17, 19, 20, 18, 26, 19, 18, 17, 18, 20, 22, 19, 18, 18];
  const w = 600, h = 44, m = 34;
  const p = v.map((x, i) => [(i / (v.length - 1)) * w, h - 4 - (x / m) * (h - 8)]);
  return `<svg class="pz-chart" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" style="height:44px" aria-label="Latensi 24 jam">${`<path d="${path(p)}L${w},${h}L0,${h}Z" style="fill:var(--accent);fill-opacity:0.08"/><path d="${path(p)}" style="fill:none;stroke:var(--accent);stroke-width:1.5" vector-effect="non-scaling-stroke"/>`}${p.map(([a], i) => `<rect class="hit" x="${a - w / v.length / 2}" y="0" width="${w / v.length}" height="${h}" data-tip="${String(i).padStart(2, "0")}.00 · p50 ${v[i]} ms"/>`).join("")}</svg>`;
}
})();
