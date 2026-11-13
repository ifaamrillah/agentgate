// Race init skrip halaman vs htmx (utang M10): skrip `defer` bisa jalan setelah htmx memproses
// <body>, sehingga `htmx:load` awal terlewat bila skrip hanya mendaftar `htmx.onLoad`. Setiap skrip
// yang meng-upgrade elemen wajib juga memindai dokumen sendiri saat siap.
//
// Dijalankan `bin/tests/js_test.sh` (node --test), tanpa browser: DOM tiruan minimal.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const JS = fileURLToPath(new URL("../../assets/static/js/", import.meta.url));

// Elemen tiruan: cukup untuk `matches`/`querySelectorAll`/`dataset`.
class FakeElement {
  constructor(selector) {
    this.selector = selector;
    this.dataset = {};
    this.open = false;
    this.opened = 0;
    this.textContent = "";
  }
  matches(sel) { return sel.includes(this.selector); }
  querySelectorAll() { return []; }
  querySelector() { return null; }
  showModal() { this.open = true; this.opened += 1; }
  addEventListener() {}
  setAttribute() {}
  after() {}
  closest() { return null; }
}

// Dokumen yang sudah selesai di-parse dan sudah diproses htmx sebelum skrip dimuat:
// `htmx.onLoad` hanya menyimpan callback, tidak memanggil ulang untuk <body>.
function world(selector) {
  const el = new FakeElement(selector);
  const loaders = [];
  const document = {
    readyState: "interactive",
    documentElement: { classList: { contains: () => false } },
    body: {},
    querySelectorAll: (sel) => (sel.includes(selector) ? [el] : []),
    querySelector: () => null,
    getElementById: () => null,
    addEventListener() {},
    createElement: () => new FakeElement("div"),
  };
  const window = { htmx: { onLoad: (fn) => loaders.push(fn) } };
  const ctx = {
    window,
    document,
    htmx: window.htmx,
    Element: FakeElement,
    Document: Object,
    HTMLDialogElement: FakeElement,
    getComputedStyle: () => ({ fontFamily: "x" }),
    console,
    setTimeout,
    queueMicrotask,
  };
  ctx.Document = function Document() {};
  Object.setPrototypeOf(document, ctx.Document.prototype);
  return { el, loaders, ctx };
}

const run = (file, ctx) => vm.runInNewContext(readFileSync(JS + file, "utf8"), ctx, { filename: file });

for (const [file, selector, upgraded] of [
  ["markdown.js", "pre[data-mermaid]", (el) => el.dataset.mermaidState !== undefined],
  ["md-editor.js", "[data-md-editor]", (el) => el.dataset.mdReady !== undefined],
  ["combobox.js", "[data-combo]", (el) => el.dataset.ready === "true"],
  ["dialog.js", "dialog[data-open-on-load]", (el) => el.opened === 1],
]) {
  test(`${file} meng-upgrade elemen walau htmx sudah memproses body`, () => {
    const { el, loaders, ctx } = world(selector);
    try {
      run(file, ctx);
    } catch (err) {
      // Elemen tiruan tidak lengkap untuk kerja upgrade sesungguhnya; yang diuji hanya pemicunya.
      if (!upgraded(el)) throw err;
    }
    assert.ok(upgraded(el), `${file} tidak memindai dokumen saat dimuat`);
    assert.ok(loaders.length >= 1, `${file} tidak mendaftar htmx.onLoad untuk swap berikutnya`);
  });

  test(`${file} idempoten saat htmx:load menyusul untuk elemen yang sama`, () => {
    const { el, loaders, ctx } = world(selector);
    try {
      run(file, ctx);
      loaders.forEach((fn) => fn(el));
    } catch (err) {
      if (!upgraded(el)) throw err;
    }
    if (file === "dialog.js") assert.equal(el.opened, 1);
    assert.ok(upgraded(el));
  });
}
