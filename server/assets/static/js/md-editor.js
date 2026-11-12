// Editor Markdown lengkap (komponen `md_editor`): meng-upgrade <textarea> di dalam
// `[data-md-editor]` menjadi Milkdown Crepe. Bundle Crepe (±2,8 MB) dan CSS-nya hanya dimuat
// saat halaman benar-benar punya editor (juga setelah swap HTMX). Textarea tetap menjadi sumber
// nilai form: setiap perubahan di editor disalin ke textarea, jadi submit biasa maupun HTMX
// mengirim Markdown terbaru. Tanpa JS (atau bila bundle gagal dimuat) textarea biasa tetap dipakai.
// Tombol `[data-md-cancel]` mengembalikan isi awal; bila berupa tautan, navigasinya tetap jalan.
// Label UI datang dari `[data-md-labels]` (Fluent) supaya tidak ada teks di JS.
(() => {
  const BASE = "/static/vendor/milkdown-crepe-7.23.0/";
  let crepeModule = null;

  const loadCss = () => {
    if (document.querySelector("link[data-md-editor-css]")) return;
    // Disisipkan SEBELUM app.css: override di md-editor.css (selector lebih spesifik) tetap menang.
    const anchor = document.querySelector('link[rel="stylesheet"]');
    for (const name of ["crepe-common.css", "crepe-frame.css"]) {
      const link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = BASE + name;
      link.dataset.mdEditorCss = "";
      anchor ? anchor.before(link) : document.head.append(link);
    }
  };

  const loadCrepe = () => {
    crepeModule ??= import(BASE + "crepe.bundle.mjs");
    return crepeModule;
  };

  const config = (Crepe, root, labels, textarea) => {
    const L = (k, fallback) => labels[k] || fallback;
    return {
      root,
      defaultValue: textarea.value,
      features: {
        [Crepe.Feature.Latex]: false,
        [Crepe.Feature.TopBar]: false,
        ...(Crepe.Feature.AI ? { [Crepe.Feature.AI]: false } : {}),
        // Unggah gambar butuh endpoint lampiran (milestone berikutnya): tanpa onUpload, blok
        // gambar hanya menerima tautan.
      },
      featureConfigs: {
        [Crepe.Feature.Placeholder]: { text: textarea.placeholder || L("placeholder", "") },
        [Crepe.Feature.Toolbar]: {
          boldLabel: L("bold"), italicLabel: L("italic"), strikethroughLabel: L("strike"),
          codeLabel: L("code"), linkLabel: L("link"),
        },
        [Crepe.Feature.CodeMirror]: {
          theme: [],
          previewLabel: L("preview"), previewLoading: L("previewLoading"),
          searchPlaceholder: L("language"), noResultText: L("noResult"), copyText: L("copy"),
          previewToggleText: (previewOnly) => (previewOnly ? L("edit") : L("hide")),
        },
        [Crepe.Feature.ImageBlock]: {
          inlineUploadPlaceholderText: L("imageLink"), blockUploadPlaceholderText: L("imageLink"),
          blockCaptionPlaceholderText: L("caption"), blockConfirmButton: L("confirm"),
          inlineConfirmButton: L("confirm"),
        },
        [Crepe.Feature.BlockEdit]: {
          textGroup: {
            label: L("groupText"), text: { label: L("paragraph") }, h1: { label: L("h1") },
            h2: { label: L("h2") }, h3: { label: L("h3") }, h4: { label: L("h4") },
            h5: { label: L("h5") }, h6: { label: L("h6") }, quote: { label: L("quote") },
            divider: { label: L("divider") },
          },
          listGroup: {
            label: L("groupList"), bulletList: { label: L("bullet") },
            orderedList: { label: L("ordered") }, taskList: { label: L("task") },
          },
          advancedGroup: {
            label: L("groupAdvanced"), image: { label: L("image") },
            codeBlock: { label: L("codeBlock") }, table: { label: L("table") }, math: null,
          },
        },
      },
    };
  };

  async function upgrade(box) {
    if (box.dataset.mdReady !== undefined) return;
    box.dataset.mdReady = "loading";
    const textarea = box.querySelector("textarea");
    const surface = box.querySelector("[data-md-surface]");
    if (!textarea || !surface) return;
    const labels = { ...(box.querySelector("[data-md-labels]")?.dataset || {}) };
    const initial = textarea.value;
    loadCss();
    let Crepe;
    try {
      ({ Crepe } = await loadCrepe());
    } catch (err) {
      // Bundle gagal dimuat (jaringan, CSP): biarkan textarea biasa.
      box.dataset.mdReady = "failed";
      console.error("md-editor:", err);
      return;
    }
    const crepe = new Crepe(config(Crepe, surface, labels, textarea));
    crepe.on((api) =>
      api.markdownUpdated((_ctx, markdown) => {
        textarea.value = markdown;
        box.dataset.mdDirty = "";
      }),
    );
    // Gaya serialisasi sama dengan Markdown yang ditulis manusia/AI: bullet "-", rule "---".
    crepe.editor.config((ctx) =>
      ctx.update("remarkStringifyOptions", (prev) => ({ ...prev, bullet: "-", rule: "-" })),
    );
    await crepe.create();
    textarea.hidden = true;
    surface.hidden = false;
    box.dataset.mdReady = "";
    box._mdCrepe = crepe;
    box._mdInitial = initial;
  }

  document.addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-md-cancel]");
    if (!btn) return;
    const box = btn.closest("[data-md-editor]") || btn.closest("form")?.querySelector("[data-md-editor]");
    if (!box) return;
    const isLink = btn.tagName === "A";
    if (!isLink) e.preventDefault();
    const textarea = box.querySelector("textarea");
    const initial = box._mdInitial ?? textarea.defaultValue;
    textarea.value = initial;
    delete box.dataset.mdDirty;
    const crepe = box._mdCrepe;
    if (crepe && !isLink) {
      // Buat ulang editor dengan isi awal (Crepe tidak punya setter isi publik).
      const surface = box.querySelector("[data-md-surface]");
      await crepe.destroy();
      surface.replaceChildren();
      delete box.dataset.mdReady;
      box._mdCrepe = null;
      textarea.hidden = false;
      surface.hidden = true;
      upgrade(box);
    }
  });

  // markdownUpdated di-debounce 200 ms oleh Milkdown: sinkronkan sekali lagi tepat sebelum submit
  // (fase capture, sebelum HTMX membaca isi form).
  document.addEventListener(
    "submit",
    (e) => {
      e.target.querySelectorAll?.("[data-md-editor]").forEach((box) => {
        if (box._mdCrepe) box.querySelector("textarea").value = box._mdCrepe.getMarkdown();
      });
    },
    true,
  );

  const scan = (root) => {
    const scope = root instanceof Element || root instanceof Document ? root : document;
    const boxes = scope.matches?.("[data-md-editor]") ? [scope] : scope.querySelectorAll("[data-md-editor]");
    boxes.forEach((box) => upgrade(box));
  };
  if (window.htmx?.onLoad) htmx.onLoad(scan);
  else document.addEventListener("DOMContentLoaded", () => scan(document));
})();
