// Tombol `data-sidebar-toggle="<id sidebar>"` membuka/menutup sidebar Basecoat (drawer di
// mobile). Basecoat 1.1.0 memasang open()/close()/toggle() di elemen `.sidebar`, tetapi tidak
// menyediakan tombol pemicunya. Dipasang lewat htmx.onLoad (juga untuk konten hasil swap);
// tombol yang sudah dipasang ditandai `data-sidebar-toggle-ready` sehingga aman dipanggil ulang.
(() => {
  const init = (button) => {
    if (button.dataset.sidebarToggleReady) return;
    button.dataset.sidebarToggleReady = "true";
    button.addEventListener("click", () => {
      const sidebar = document.getElementById(button.dataset.sidebarToggle);
      if (sidebar && typeof sidebar.toggle === "function") sidebar.toggle();
    });
  };

  htmx.onLoad((root) => {
    if (!(root instanceof Element)) return;
    if (root.matches("[data-sidebar-toggle]")) init(root);
    root.querySelectorAll("[data-sidebar-toggle]").forEach(init);
  });
})();
