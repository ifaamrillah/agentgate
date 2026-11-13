// Log job (components/log_view.html): tetap di baris terakhir saat isi diganti polling HTMX,
// kecuali pengguna sedang menggulir ke atas.
(() => {
  const pinned = new WeakMap();
  const atEnd = (el) => el.scrollHeight - el.scrollTop - el.clientHeight < 40;
  const stick = () => {
    document.querySelectorAll('[data-log]').forEach((el) => {
      if (pinned.get(el) !== false) el.scrollTop = el.scrollHeight;
    });
  };
  document.addEventListener('scroll', (e) => {
    const el = e.target;
    if (el instanceof Element && el.matches('[data-log]')) pinned.set(el, atEnd(el));
  }, true);
  document.addEventListener('htmx:after:swap', stick);
  document.addEventListener('htmx:afterSwap', stick);
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', stick);
  else stick();
})();
