# GitHub App

Sejak R2, cara utama menautkan GitHub adalah **GitHub App** milik instance Agent Gate. Personal
access token (PAT) dan GitHub Enterprise Server tetap jalan sebagai cadangan.

Kelebihan App dibanding PAT:

- Izin per repo yang dipilih saat memasang, bukan seluruh akses akun.
- Token instalasi berumur 1 jam dibuat server dari kunci privat App (JWT RS256) dan di-cache;
  tidak ada token yang disimpan di database.
- Satu endpoint webhook untuk semua instalasi; tidak perlu membuat webhook per repo.
- Tidak ikut mati saat karyawan pemilik PAT keluar.

## 1. Buat App (sekali per instance)

Di GitHub: **Settings → Developer settings → GitHub Apps → New GitHub App** (untuk organisasi:
**Organization settings → Developer settings → GitHub Apps**).

| Isian | Nilai |
|---|---|
| GitHub App name | mis. `Agent Gate PT Contoh` (menjadi slug, mis. `agent-gate-pt-contoh`) |
| Homepage URL | `{APP_URL}` |
| Callback URL | `{APP_URL}/git/github-app/callback` |
| Request user authorization (OAuth) during installation | **centang** (dipakai untuk memastikan pemasang benar-benar punya akses ke instalasi) |
| Setup URL | kosong (callback di atas yang dipakai) |
| Webhook → Active | centang |
| Webhook URL | `{APP_URL}/webhooks/github-app` |
| Webhook secret | `openssl rand -hex 32` |
| Where can this GitHub App be installed? | *Only on this account* untuk satu organisasi; *Any account* bila beberapa organisasi memakai instance yang sama |

### Izin minimum (Repository permissions)

| Izin | Akses | Untuk |
|---|---|---|
| Contents | Read and write | clone/arsip untuk code graph, git tag rilis, commit file knowledge |
| Pull requests | Read and write | membuat dan menggabungkan PR tiket, PR knowledge |
| Checks | Read-only | status CI di tiket |
| Commit statuses | Read-only | status CI (status lama) |
| Metadata | Read-only | wajib dari GitHub |

Tidak ada izin organisasi atau akun yang dibutuhkan.

### Event webhook (Subscribe to events)

`Pull request`, `Check run`, `Check suite`, `Status`, `Push`. Event `installation` dan
`installation_repositories` selalu dikirim GitHub untuk App dan ditangani otomatis (instalasi
dihapus/ditangguhkan, repo ditambah/dikurangi).

## 2. Ambil kredensial

Dari halaman App:

1. **App ID** dan **slug** (bagian akhir URL `https://github.com/apps/<slug>`).
2. **Client ID** lalu **Generate a new client secret**.
3. **Private keys → Generate a private key** (file `.pem`).

## 3. Isi env server

```sh
GITHUB_APP_ID=123456
GITHUB_APP_SLUG=agent-gate-pt-contoh
GITHUB_APP_CLIENT_ID=Iv23li...
GITHUB_APP_CLIENT_SECRET=...
GITHUB_APP_WEBHOOK_SECRET=...          # sama dengan isian Webhook secret
# PEM dalam satu baris, baris baru diganti \n:
GITHUB_APP_PRIVATE_KEY="$(awk 'NF {sub(/\r/, ""); printf "%s\\n",$0;}' agent-gate.private-key.pem)"
```

Keenamnya harus terisi; kalau ada yang kosong, pilihan GitHub App di Pengaturan → Git dinonaktifkan
dan hanya PAT yang tersedia. Restart `web` dan `worker` setelah mengubah env.

## 4. Pasang di workspace

Pemilik/admin workspace: **Pengaturan → Git → Hubungkan GitHub → GitHub App (disarankan) →
Pasang di GitHub**. Pilih organisasi dan repo yang boleh diakses, lalu GitHub mengembalikan ke
Agent Gate. Repo yang dipilih langsung muncul; tautkan repo project seperti biasa di
Pengaturan project → Repo.

Mengubah daftar repo atau izin dilakukan di GitHub (**Kelola di GitHub** di baris koneksi);
perubahan masuk lewat webhook, atau tekan **Segarkan**.

## Pemecahan masalah

| Gejala | Penyebab umum |
|---|---|
| Tombol App nonaktif | salah satu env `GITHUB_APP_*` kosong |
| Kembali dari GitHub dengan pesan "Instalasi GitHub App tidak bisa diverifikasi" | Request user authorization tidak dicentang, atau pemasang bukan anggota akun instalasi |
| Repo bertanda "belum diberi akses ke GitHub App" | repo tidak dipilih saat memasang App; tambahkan di GitHub |
| Webhook 401 di Recent Deliveries GitHub | `GITHUB_APP_WEBHOOK_SECRET` beda dengan isian di GitHub |
| Koneksi bertanda ditangguhkan | instalasi di-suspend di GitHub; aktifkan lagi di sana |
