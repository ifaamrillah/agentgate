// Reporter error browser ke Sentry (R1). Dimuat layout hanya bila SENTRY_DSN diisi (meta
// `ag-sentry-dsn`); tanpa SDK pihak ketiga supaya CSP `script-src 'self'` tetap.
//
// Yang dikirim: jenis + pesan error, stack trace, URL halaman TANPA query/hash, user agent,
// environment, release. Yang tidak pernah dikirim: cookie, isi form/body, query string, header,
// data user. Maksimal 10 event per halaman (cegah banjir dari loop error).
(function () {
  "use strict";
  var meta = function (name) {
    var el = document.querySelector('meta[name="' + name + '"]');
    return el ? el.getAttribute("content") || "" : "";
  };
  var dsn = meta("ag-sentry-dsn");
  // https://<key>@<host>/<project>
  var m = /^(https?):\/\/([^@\/]+)@([^\/]+)\/(?:.*\/)?(\d+)$/.exec(dsn);
  if (!m) return;
  var endpoint = m[1] + "://" + m[3] + "/api/" + m[4] + "/envelope/?sentry_version=7&sentry_key=" + encodeURIComponent(m[2]);
  var environment = meta("ag-sentry-environment");
  var release = meta("ag-sentry-release");
  var sent = 0;
  var MAX = 10;

  function uuid() {
    var b = new Uint8Array(16);
    crypto.getRandomValues(b);
    return Array.prototype.map.call(b, function (x) { return ("0" + x.toString(16)).slice(-2); }).join("");
  }

  // Buang query string dan hash dari URL (bisa memuat token).
  function clean(url) {
    return String(url || "").split(/[?#]/)[0];
  }

  function frames(stack) {
    if (!stack) return undefined;
    var out = [];
    String(stack).split("\n").slice(0, 30).forEach(function (line) {
      var f = /(?:at\s+(.*?)\s+\(|at\s+|@)(\S+?):(\d+):(\d+)\)?\s*$/.exec(line.trim());
      if (f) out.push({ function: f[1] || "?", filename: clean(f[2]), lineno: +f[3], colno: +f[4], in_app: true });
    });
    return out.length ? { frames: out.reverse() } : undefined;
  }

  function send(type, value, stack) {
    if (sent >= MAX || !window.fetch) return;
    sent += 1;
    var id = uuid();
    var event = {
      event_id: id,
      timestamp: Date.now() / 1000,
      platform: "javascript",
      level: "error",
      environment: environment || undefined,
      release: release || undefined,
      request: { url: clean(location.href), headers: { "User-Agent": navigator.userAgent } },
      exception: { values: [{ type: type || "Error", value: String(value || "").slice(0, 500), stacktrace: frames(stack) }] }
    };
    var body = JSON.stringify({ event_id: id, sent_at: new Date().toISOString() }) + "\n" +
      JSON.stringify({ type: "event" }) + "\n" + JSON.stringify(event);
    try {
      fetch(endpoint, { method: "POST", body: body, keepalive: true, credentials: "omit", mode: "cors" }).catch(function () {});
    } catch (_) { /* laporan error tidak boleh membuat error baru */ }
  }

  window.addEventListener("error", function (e) {
    var err = e.error;
    send(err && err.name, (err && err.message) || e.message, err && err.stack);
  });
  window.addEventListener("unhandledrejection", function (e) {
    var r = e.reason;
    send((r && r.name) || "UnhandledRejection", (r && r.message) || r, r && r.stack);
  });
  window.agSentry = { capture: function (err) { send(err && err.name, err && err.message, err && err.stack); } };
})();
