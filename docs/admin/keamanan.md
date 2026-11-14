# Keamanan (ringkasan untuk admin)

- **Sesi** server-side di database, cookie `__Host-` Secure; logout/ganti password/nonaktifkan user
  mencabut sesi seketika. Password argon2id; login dikunci setelah 5 gagal per email / 20 per IP
  dalam 15 menit. 2FA TOTP opsional, bisa diwajibkan Owner.
- **CSRF** lewat `Sec-Fetch-Site`/`Origin` untuk semua POST berbasis cookie. Pengecualian hanya
  endpoint bertoken bearer/PKCE (`/mcp`, `/oauth/token`, `/oauth/register`, `/oauth/revoke`, `/oauth/device_authorization`, `/api/v1/`).
- **Header**: CSP `script-src 'self'` tanpa inline, HSTS, `X-Frame-Options`, `nosniff`,
  `Referrer-Policy: same-origin`, `Permissions-Policy` mematikan kamera/mikrofon/lokasi/pembayaran/USB.
- **Isolasi tenant**: setiap route `/w/{slug}` diuji otomatis menjawab 404 untuk anggota workspace
  lain (tes `authz_sweep`).
- **SSRF**: URL webhook, Jira, dan GitHub Enterprise ditolak bila ke alamat privat/loopback/link-local
  (diperiksa juga setelah DNS); redirect unduhan tarball dibatasi.
- **OAuth MCP**: pendaftaran client dinamis dibatasi 20 per IP per jam; kode otorisasi hangus setelah
  satu percobaan gagal; PKCE wajib.
- **Pratinjau mockup** di domain terpisah, iframe sandbox, tanpa cookie aplikasi.
- **Secret** integrasi dienkripsi XChaCha20-Poly1305 dengan `ENCRYPTION_KEY`; token API/MCP disimpan
  sebagai hash.
- **Metrik** `/_metrics` butuh token dan diblok Caddy dari internet.

Lapor celah: hubungi admin instance; jangan buka issue publik.
