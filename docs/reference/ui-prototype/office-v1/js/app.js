/* Halaman prototype: tab variasi (hash #v1/#v2/#v3/#now), tema (?theme=dark|light atau tombol). */
(() => {
  const root = document.documentElement;
  const DESC = {
    v1: ["V1 · Isometrik Studio", "A1 yang dipoles: diorama di atas alas, partisi rendah (tanpa kaca yang menutupi isi), karakter chibi dua kali lebih besar, furnitur lebih kaya, cahaya jendela siang dan lampu hangat malam."],
    v2: ["V2 · Denah", "Floor-plan modern dari atas, setipis Linear/Figma. Karakter jadi token avatar dengan cincin status. Paling mudah dibaca dan paling murah dirawat, tapi kurang terasa sebagai 'kantor'."],
    v3: ["V3 · Diorama hangat", "Potongan gedung 2.5D (dollhouse): ruangan berjajar per lantai, karakter menghadap depan dan lebih besar, ilustrasi hangat dengan garis tinta. Project baru = gedung bertambah tinggi."],
    now: ["Sekarang", "Implementasi saat ini (Canvas 2D, A1) dengan data contoh yang sama."],
  };
  let theme = new URLSearchParams(location.search).get("theme");
  if (!theme) { try { theme = localStorage.getItem("of-theme"); } catch { /* */ } }
  const sysDark = () => matchMedia("(prefers-color-scheme: dark)").matches;
  const applyTheme = () => {
    const dark = theme ? theme === "dark" : sysDark();
    root.dataset.theme = dark ? "dark" : "light";
    document.getElementById("theme").innerHTML = `${window.ic(dark ? "circle-half-2" : "moon")}<span>${dark ? "Terang" : "Gelap"}</span>`;
    if (cur === "now") showNow();
  };
  document.getElementById("theme").onclick = () => {
    theme = root.dataset.theme === "dark" ? "light" : "dark";
    try { localStorage.setItem("of-theme", theme); } catch { /* */ }
    applyTheme();
  };

  const zb = { out: "minus", in: "plus", fit: "arrows-maximize" };
  document.querySelectorAll("[data-zoom]").forEach((b) => { if (zb[b.dataset.zoom]) b.innerHTML = window.ic(zb[b.dataset.zoom]) + (b.dataset.zoom === "fit" ? "<span>Pas layar</span>" : ""); });

  let cur = null, inst = null;
  const live = document.getElementById("live"), now = document.getElementById("now");
  function showNow() {
    const d = root.dataset.theme === "dark";
    document.getElementById("now-a").src = d ? "shots/now-dark.png" : "shots/now-light.png";
    document.getElementById("now-b").src = d ? "shots/now-fit-dark.png" : "shots/now-fit.png";
  }
  function show(v) {
    if (!DESC[v]) v = "v1";
    cur = v;
    document.querySelectorAll("#vtabs [data-v]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.v === v)));
    document.getElementById("vtitle").textContent = DESC[v][0];
    document.getElementById("vdesc").textContent = DESC[v][1];
    inst?.destroy(); inst = null;
    live.hidden = v === "now"; now.hidden = v !== "now";
    if (v === "now") { showNow(); return; }
    inst = window.OfficeCore.mount(live, window.VARIANTS[v], window.OFFICE_DATA);
    window.__office = inst;
  }
  document.getElementById("vtabs").onclick = (e) => { const b = e.target.closest("[data-v]"); if (b) location.hash = b.dataset.v; };
  window.addEventListener("hashchange", () => show(location.hash.slice(1)));
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => { if (!theme) applyTheme(); });
  applyTheme();
  show(location.hash.slice(1) || "v1");
})();
