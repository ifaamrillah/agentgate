# Spike: Membuka terminal interaktif per OS

Tanggal: 2026-11-11 · Milestone: M0 (spike), dipakai di M5 · Spec: `docs/spec.md` bagian "Runner & eksekusi job"

## Pertanyaan

1. Bagaimana runner `agentgate` (Rust, single binary) membuka **jendela terminal baru** di desktop user yang menjalankan sesi client (misalnya `claude "<prompt awal>"`) di working directory tertentu (git worktree), di Linux, macOS, dan Windows?
2. Bagaimana deteksi otomatis program terminal, dan bagaimana user mengganti program terminal per OS?
3. Bagaimana menyusun argumen tanpa risiko shell injection (prompt berisi teks bebas dari tiket)?
4. Bagaimana runner tahu jendela berhasil terbuka, dan apa fallback-nya kalau gagal?
5. Crate Rust apa yang relevan, atau cukup `std::process::Command`?

## Rekomendasi

1. **Pakai `std::process::Command` + crate `which`.** Tidak ada crate yang matang untuk "buka terminal X dengan perintah Y". Crate `open`/`opener` hanya membuka file atau URL dengan aplikasi default. Di Linux, `.sh` bisa terbuka di text editor, jadi crate ini tidak cocok.
2. **Terminal tidak pernah menerima prompt atau perintah client secara inline.** Runner menulis **launcher script trampolin** yang isinya teks tetap: path absolut binary `agentgate` dan job ID (UUID). Terminal menjalankan launcher tersebut, lalu launcher memanggil `agentgate job attach <job-id>`. Binary `agentgate` kemudian mengambil konteks job dan menjalankan `claude` dengan argv array lewat `Command`, tanpa shell, dengan stdio mewarisi TTY terminal. Prompt dari tiket tidak pernah lewat parser shell, `cmd.exe`, AppleScript, maupun parser `wt.exe`.
3. **Simpan launcher di direktori state runner, bukan di worktree.** Contoh: `~/.local/state/agentgate/launch/<job-id>.sh`, `~/Library/Application Support/agentgate/launch/<job-id>.command`, `%LOCALAPPDATA%\agentgate\launch\<job-id>.cmd`, dengan mode `0700` di Unix. Alasannya: (a) AI yang bekerja di worktree tidak bisa mengubah launcher, (b) launcher tidak ikut ter-commit, (c) `.agentgate/tmp/` saat ini belum ada di `.gitignore` repo. Kalau tetap ingin di worktree (`.agentgate/tmp/launch.*`), runner wajib menambahkan entri di `.git/info/exclude` worktree.
4. **Sukses diukur dengan handshake, bukan exit code.** Banyak terminal langsung detach dengan exit code 0 (terbukti untuk `gnome-terminal` di host ini). Runner menganggap pembukaan **berhasil** hanya kalau `agentgate job attach <id>` melapor balik ke runner lewat IPC lokal dalam batas waktu (usulan 20 detik). Kalau spawn gagal, proses exit non-zero sebelum handshake, atau handshake timeout, runner mencoba kandidat terminal berikutnya (hanya untuk kegagalan cepat), lalu mengirim **notifikasi fallback** berisi perintah manual.
5. **Default per OS:**
   - Linux: auto-detect berurutan, lihat tabel di bawah.
   - macOS: `open -a Terminal <launcher>.command`. Cara ini tidak butuh izin Automation/TCC.
   - Windows: `wt.exe -w new new-tab -d <dir> --title agentgate cmd.exe /k <launcher>.cmd` kalau `wt.exe` ada. Fallback-nya `cmd.exe /k <launcher>.cmd` dengan `CREATE_NEW_CONSOLE`, tanpa `start` dan tanpa PowerShell.
6. **Konfigurasi user** berupa template argv per OS. Placeholder `{cwd}`, `{launcher}`, dan `{title}` hanya boleh mengisi **satu elemen argv utuh** dan tidak pernah digabung ke dalam string shell. Contoh:

   ```toml
   [terminal.linux]
   program = "kitty"
   args = ["--directory", "{cwd}", "--title", "{title}", "{launcher}"]

   [terminal.macos]
   preset = "iterm2"            # atau program/args custom

   [terminal.windows]
   preset = "auto"              # auto | wt | conhost | custom
   ```

## Tabel per OS

Notasi: `{cwd}` = path worktree, `{L}` = path absolut launcher script, `{T}` = judul jendela. Kalau terminal tidak punya flag cwd, runner memakai `Command::current_dir({cwd})`, dan launcher juga melakukan `cd` sendiri.

### Linux

Urutan deteksi yang direkomendasikan:

1. Setting user.
2. `$TERMINAL`.
3. `xdg-terminal-exec`.
4. Terminal bawaan desktop sesuai `$XDG_CURRENT_DESKTOP`.
5. `x-terminal-emulator`.
6. Daftar umum: ghostty, wezterm, kitty, alacritty, foot (hanya Wayland), tilix, xterm.

| Program | Deteksi | argv exact | Catatan |
|---|---|---|---|
| (setting user) | `which(program)` | sesuai template | Selalu menang. |
| `$TERMINAL` | env non-kosong + `which` | `[$TERMINAL, "-e", {L}]` | Konvensi de facto (i3/sway, dll.), bukan standar. Mengasumsikan dukungan `-e`. Isi env kadang berisi argumen, jadi perlakukan sebagai nama program saja. belum terverifikasi lintas terminal |
| `xdg-terminal-exec` | `which` | `["xdg-terminal-exec", "--dir=" + {cwd}, {L}]` | Proposal spec freedesktop, sudah dipakai di beberapa distro baru. Flag `--dir` belum terverifikasi. Tanpa flag, andalkan `current_dir`. |
| gnome-terminal | `which`; GNOME/Ubuntu | `["gnome-terminal", "--window", "--title=" + {T}, "--working-directory=" + {cwd}, "--", {L}]` | **Gunakan `--`.** `-e` sudah deprecated dan menerima satu string yang di-parse ulang. Client bicara ke `gnome-terminal-server` via D-Bus lalu langsung exit 0 (detach). Ada `--wait` agar client menunggu child selesai. **Terverifikasi di host** (lihat hasil uji), kecuali `--wait`. |
| ptyxis (GNOME 47+, Ubuntu 25.04+ / Fedora) | `which` | `["ptyxis", "--new-window", "--working-directory=" + {cwd}, "--", {L}]` | belum terverifikasi. Bisa terpasang sebagai Flatpak (`flatpak run app.devsuite.Ptyxis`), dan sandbox Flatpak bisa menghalangi eksekusi launcher di host. |
| kgx (GNOME Console) | `which` | `["kgx", "--working-directory=" + {cwd}, "-e", {L}]` | belum terverifikasi |
| konsole | `which`; KDE | `["konsole", "--workdir", {cwd}, "-p", "tabtitle=" + {T}, "-e", {L}]` | `-e` mengambil sisa argv. Proses mungkin detach. belum terverifikasi |
| xfce4-terminal | `which`; XFCE | `["xfce4-terminal", "--working-directory=" + {cwd}, "--title=" + {T}, "-x", {L}]` | `-x` = sisa argv adalah command. `-e` = satu string, jangan dipakai. belum terverifikasi |
| kitty | `which` | `["kitty", "--directory", {cwd}, "--title", {T}, {L}]` | Proses tetap hidup sampai jendela ditutup, sehingga exit status bisa dipantau. belum terverifikasi |
| wezterm | `which` | `["wezterm", "start", "--always-new-process", "--cwd", {cwd}, "--", {L}]` | Tanpa `--always-new-process`, perintah diserahkan ke GUI yang sudah jalan dan klien langsung exit. Sumber: dokumentasi resmi. Belum diuji. |
| alacritty | `which` | `["alacritty", "--working-directory", {cwd}, "--title", {T}, "-e", {L}]` | `-e` mengambil sisa argv. Proses tetap hidup. belum terverifikasi |
| foot | `which` + Wayland | `["foot", "--working-directory=" + {cwd}, "--title=" + {T}, {L}]` | Hanya Wayland. belum terverifikasi |
| ghostty | `which` | `["ghostty", "--working-directory=" + {cwd}, "--title=" + {T}, "-e", {L}]` | Menurut dokumentasi, `-e` memaksa `gtk-single-instance=false` dan menjalankan command tanpa shell expansion, jadi proses baru tetap hidup. Terpasang di host (1.3.1, target `x-terminal-emulator`), tetapi belum dibuka (jatah uji hanya satu jendela). |
| tilix | `which` | `["tilix", "--working-directory=" + {cwd}, "-e", {L}]` | `-e` menerima **satu string** yang di-parse tilix. Aman hanya karena `{L}` adalah path tanpa spasi (pastikan path state dir bebas spasi, atau quote). belum terverifikasi |
| x-terminal-emulator | `which` (Debian/Ubuntu) | `["x-terminal-emulator", "-T", {T}, "-e", {L}]` | Debian Policy mewajibkan dukungan `-e cmd args` dan `-T`. Tidak ada flag cwd standar, jadi pakai `current_dir`. Di host ini menunjuk ke `/usr/bin/ghostty`. |
| xterm | `which` | `["xterm", "-T", {T}, "-e", {L}]` | Pilihan terakhir. Pakai `current_dir`. XWayland diperlukan di Wayland. |

**Deteksi sesi grafis (Linux):**

- Ada sesi grafis kalau `WAYLAND_DISPLAY` non-kosong **dan** socket `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY` ada, **atau** `DISPLAY` non-kosong. `XDG_SESSION_TYPE` (`wayland`/`x11`/`tty`) hanya dipakai sebagai petunjuk.
- Kalau tidak ada sesi grafis (SSH, TTY, container), **jangan mencoba membuka terminal**. Langsung kirim notifikasi fallback.
- Wayland: terminal GTK/Qt modern berjalan native. xterm butuh XWayland, yang biasanya `DISPLAY=:0` tetap tersedia. Focus-stealing prevention membuat jendela baru mungkin tidak langsung di-raise; GNOME menampilkan notifikasi "… is ready". Runner tidak punya token `XDG_ACTIVATION_TOKEN`, jadi ini diterima apa adanya.
- **Runner sebagai service:** spec menyatakan runner dinyalakan manual (`agentgate runner start`), biasanya dari terminal dalam sesi desktop, sehingga env sudah lengkap. Kalau nanti dijalankan sebagai `systemd --user` service, env grafis tersedia setelah desktop memanggil `dbus-update-activation-environment`/`systemctl --user import-environment`. Di host ini `systemctl --user show-environment` sudah berisi `DISPLAY` dan `WAYLAND_DISPLAY`. System service (root/daemon lain) **tidak** didukung untuk mode interaktif.
- `gnome-terminal` butuh session D-Bus (`DBUS_SESSION_BUS_ADDRESS`). Ini juga ikut di-import oleh systemd user.

### macOS

| Program | Deteksi | argv exact | Catatan |
|---|---|---|---|
| Terminal.app (default) | selalu ada | `["open", "-a", "Terminal", {L}]` dengan `{L}` berakhiran `.command` dan executable | Lewat LaunchServices, **tidak butuh izin Automation (TCC)**. Membuka jendela baru yang menjalankan script. `open` exit ≠ 0 kalau aplikasi tidak ditemukan. Setelah itu `open` detach. belum terverifikasi di mesin macOS (perlu job GitHub Actions `macos-latest`, tetapi runner CI tidak punya sesi GUI interaktif yang bisa dilihat) |
| Terminal.app via AppleScript | — | `["osascript", "-e", "tell application \"Terminal\" to do script \"<quoted {L}>\"", "-e", "tell application \"Terminal\" to activate"]` | **Tidak direkomendasikan sebagai default.** Memicu prompt TCC Automation ("… wants access to control Terminal"). Izin diatribusikan ke *responsible app* (misalnya iTerm/Terminal tempat runner dijalankan), bukan ke `agentgate`. Kalau ditolak: error `-1743` (errAEEventNotPermitted) dan tidak ditanya ulang sampai user mengubah System Settings → Privacy & Security → Automation atau `tccutil reset AppleEvents`. String di dalam `do script` adalah shell, sehingga quoting berlapis (AppleScript + sh). |
| iTerm2 | `/Applications/iTerm.app` atau `mdfind kMDItemCFBundleIdentifier == com.googlecode.iterm2` | `["open", "-a", "iTerm", {L}]` | belum terverifikasi apakah iTerm menjalankan `.command` lewat `open -a`. Alternatifnya AppleScript `tell application "iTerm" to create window with default profile command "<{L}>"`, dengan isu TCC yang sama seperti di atas. |
| Ghostty | `/Applications/Ghostty.app` | `["open", "-na", "Ghostty", "--args", "--working-directory=" + {cwd}, "-e", {L}]` | `ghostty --help` (1.3.1): di macOS peluncuran CLI langsung tidak didukung dan harus lewat `open -na Ghostty.app --args …`. belum terverifikasi |
| WezTerm | `which wezterm` atau app bundle | `["wezterm", "start", "--always-new-process", "--cwd", {cwd}, "--", {L}]` | belum terverifikasi di macOS |
| Kitty | `which kitty` / app bundle | `["open", "-na", "kitty", "--args", "--directory", {cwd}, {L}]` | belum terverifikasi |
| Alacritty | app bundle | `["open", "-na", "Alacritty", "--args", "--working-directory", {cwd}, "-e", {L}]` | belum terverifikasi |

Catatan macOS:

- Launcher `.command` dibuat lokal oleh runner, jadi tidak membawa xattr `com.apple.quarantine`. Gatekeeper tidak memblokir script tanpa quarantine. belum terverifikasi
- Terminal.app membiarkan jendela tetap terbuka setelah script selesai, sesuai setting profil "When the shell exits".
- Deteksi sesi GUI: proses runner harus berada di sesi Aqua user. `launchctl managername` mengembalikan `Aqua` (belum terverifikasi). Runner yang dijalankan via SSH tidak bisa membuka jendela, sehingga langsung masuk fallback.

### Windows

| Program | Deteksi | argv exact | Catatan |
|---|---|---|---|
| Windows Terminal (default kalau ada) | `which("wt.exe")`. Biasanya app execution alias di `%LOCALAPPDATA%\Microsoft\WindowsApps\wt.exe`, bawaan Windows 11. Alias bisa dimatikan user di Settings → App execution aliases. | `["wt.exe", "-w", "new", "new-tab", "-d", {cwd}, "--title", {T}, "cmd.exe", "/k", {L}]` | `-w new` (atau `-1`) memaksa jendela baru. Tanpa itu, perilaku mengikuti setting `windowingBehavior` (bisa jadi tab di jendela yang ada). **`;` adalah pemisah subcommand `wt` dan harus di-escape `\;`**: path yang mengandung `;` wajib di-escape atau ditolak. `wt.exe` langsung kembali (detach). Jangan panggil `wt` dari PowerShell, karena PowerShell menunggu aplikasi Store dan `;` bentrok. Sumber: Microsoft Learn. Belum diuji. |
| Console host baru (fallback) | selalu ada | `Command::new("cmd.exe").args(["/k", {L}]).current_dir({cwd}).creation_flags(CREATE_NEW_CONSOLE)` | **Tanpa `start`.** `cmd /c start "" cmd /k …` menambah satu lapis parsing cmd (`^ & | % "`), sedangkan `CREATE_NEW_CONSOLE` (0x10) langsung membuat jendela konsol baru dengan handle proses nyata yang bisa ditunggu. Di Windows 11 yang default terminal-nya Windows Terminal (defterm), konsol ini otomatis tampil di WT. belum terverifikasi |
| `cmd /c start` | — | `["cmd.exe", "/c", "start", "\"agentgate\"", "/D", {cwd}, "cmd.exe", "/k", {L}]` | Tidak direkomendasikan. Argumen judul pertama yang ber-quote wajib ada. Quoting Rust MSVC-style tidak sama dengan aturan cmd. |
| PowerShell `Start-Process` | — | `["powershell.exe", "-NoProfile", "-Command", "Start-Process cmd.exe -ArgumentList '/k','<L>' -WorkingDirectory '<cwd>'"]` | Tidak direkomendasikan. Ada tiga lapis quoting (Rust → PowerShell → CreateProcess), lambat start, dan bisa terhalang execution policy/AMSI. |

Catatan quoting Windows:

- `std::process::Command` meng-quote argumen dengan aturan MSVC (`CommandLineToArgvW`). Ini benar untuk `wt.exe`, tetapi **tidak** untuk `cmd.exe`, yang punya parser sendiri.
- Sejak Rust 1.77.2 (CVE-2024-24576, "BatBadBut"), menjalankan `.bat`/`.cmd` langsung dengan argumen yang tidak bisa di-escape aman akan menghasilkan error. Ini alasan lain agar launcher `.cmd` **tidak menerima argumen**: semua datanya sudah tertanam sebagai teks tetap yang aman (path + UUID).
- `cmd /k "<path>"`: cmd memakai aturan "strip quote pertama dan terakhir". Path launcher dijamin tidak mengandung `"`, `%`, `^`, `&`, atau `!` karena runner memilih direktori state sendiri, dan runner memvalidasi ini sebelum spawn.
- `{cwd}` dengan spasi aman untuk `-d` di `wt` (satu elemen argv) dan untuk `current_dir`.

## Desain launcher script

Prinsip: launcher adalah **trampolin bodoh**. Semua logika (ambil konteks tiket, menyusun prompt, memilih client dan flag, laporan ke runner) ada di binary `agentgate`, yang memanggil client dengan argv array tanpa shell.

Alur:

1. Runner menerima job interaktif, menyiapkan worktree, lalu menulis aturan.
2. Runner menulis `<state>/launch/<job-id>.{sh|command|cmd}` dengan mode `0700`, dan membuat nonce sekali pakai di file terpisah `<job-id>.nonce` (`0600`).
3. Runner men-spawn terminal sesuai tabel dengan `stdin/stdout/stderr = null`, dan di Unix `process_group(0)`. Dengan begitu Ctrl+C di runner tidak ikut membunuh jendela, dan exit-nya di-reap di thread terpisah supaya tidak jadi zombie.
4. Di dalam terminal, launcher menjalankan `agentgate job attach <job-id>`. Binary ini membaca nonce (lalu menghapusnya), terhubung ke runner lewat IPC lokal (Unix socket di state dir / named pipe di Windows), dan mengirim `attached{job, pid}`. Runner menandai "terminal terbuka", lalu `attach` menjalankan `claude <prompt>` dengan `current_dir(worktree)` dan stdio mewarisi TTY.
5. Saat client selesai, `attach` melapor `exited{code}`, launcher menampilkan "Tekan Enter untuk menutup", dan runner menghapus file launcher.

Isi launcher:

`launch.sh` / `.command` (Linux/macOS):

```sh
#!/bin/sh
# generated by agentgate runner — do not edit
cd '/home/u/proj/.worktrees/T-123' || exit 1
exec '/home/u/.local/bin/agentgate' job attach 0b6f3c1e-...-9a2d
```

- Nilai path di-quote dengan single quote. Single quote di dalam path ditulis sebagai `'\''`. Ini quoting POSIX yang lengkap dan aman, bisa memakai crate `shlex::try_quote` atau fungsi 5 baris sendiri.
- Job ID divalidasi dengan regex UUID sebelum ditulis.
- `exec` dipakai agar sinyal dan exit status diteruskan.
- Pesan "Tekan Enter" ditangani oleh `agentgate job attach` sendiri, sehingga jendela tidak langsung hilang kalau terjadi error.

`launch.cmd` (Windows):

```bat
@echo off
cd /d "C:\Users\u\proj\.worktrees\T-123" || exit /b 1
"C:\Users\u\AppData\Local\agentgate\bin\agentgate.exe" job attach 0b6f3c1e-...-9a2d
```

- Path yang mengandung `" % ^ & ! ;` ditolak sebelum menulis. Ini sangat jarang di Windows, dan kalau terjadi runner langsung masuk fallback manual.
- Encoding: tulis sebagai ASCII/UTF-8 tanpa BOM. Path non-ASCII memerlukan `chcp 65001` di baris kedua (belum terverifikasi).

**Mengapa tidak inline (`gnome-terminal -- claude "<prompt>"`)?**

- Prompt berasal dari teks tiket, yang merupakan data tidak tepercaya.
- Inline masih aman di Linux kalau argv langsung (`--`) dipakai. Namun `-e` (konsole/tilix/x-terminal-emulator varian lama), AppleScript `do script`, `wt.exe` (`;`), dan `cmd.exe` semuanya mem-parse ulang string. Mempertahankan quoting yang benar di sekitar 15 terminal × 3 OS berisiko tinggi.
- Dengan trampolin, satu-satunya data yang masuk ke parser asing adalah path milik runner dan UUID.

**Bonus:** launcher yang sama bisa ditampilkan di notifikasi fallback ("jalankan: `agentgate job attach <id>`"), dan user bisa menyalinnya ke terminal mana pun.

## Desain error & fallback

| Tahap | Sinyal gagal | Tindakan |
|---|---|---|
| Pra-cek | Tidak ada sesi grafis (Linux: `DISPLAY`/`WAYLAND_DISPLAY` kosong; macOS: bukan sesi Aqua; Windows: session 0 / service) | Lewati semua kandidat, langsung fallback. |
| Deteksi | Tidak ada kandidat yang `which` berhasil | Fallback. |
| Spawn | `Command::spawn` error (ENOENT, EACCES) | Coba kandidat berikutnya. |
| Exit cepat | Proses exit **non-zero** dalam ≤ 3 detik. Contoh di host: `gnome-terminal` tanpa display exit 1 dengan "Cannot open display". | Coba kandidat berikutnya. |
| Exit 0 cepat | Normal untuk terminal yang detach (gnome-terminal, wt, `open`, konsole?) | Tunggu handshake. |
| Handshake | Tidak ada `attached` dalam 20 detik (bisa dikonfigurasi) | **Jangan** coba kandidat lain, karena jendela mungkin sudah terbuka dan dua sesi bisa paralel. Kirim fallback, tandai job `waiting_manual_attach`. Kalau `attach` datang terlambat, tetap diterima. |
| macOS TCC | Hanya kalau preset AppleScript dipakai: stderr `osascript` mengandung `-1743` | Fallback + pesan cara mengaktifkan izin Automation, dan sarankan preset `open -a`. |
| Client exit | `exited{code≠0}` | Laporkan ke server sebagai log job, bukan sebagai kegagalan membuka terminal. |

Isi notifikasi fallback (ke web + CLI runner):

- Alasan singkat, misalnya "Terminal tidak bisa dibuka: tidak ada sesi grafis".
- Perintah yang bisa disalin: `cd <worktree> && agentgate job attach <job-id>`.
- Perintah slash yang setara di client (`/ag-...`) sesuai spec.
- Nonce tidak wajib untuk attach manual. User sudah terautentikasi lewat login runner, dan IPC lokal hanya bisa diakses user yang sama.

Pencegahan dobel: runner hanya menerima satu `attached` per job. `attach` kedua ditolak dengan pesan "sesi sudah aktif di jendela lain".

## Hasil uji di host

Host: Ubuntu 24.04.5 LTS, kernel 7.0.0-42, GNOME (`XDG_CURRENT_DESKTOP=ubuntu:GNOME`), `XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY=wayland-0`, `DISPLAY=:0` (XWayland), `$TERMINAL` kosong.

Terminal terpasang (`which`):

| Program | Ada? | Catatan |
|---|---|---|
| x-terminal-emulator | ya | → `/usr/bin/ghostty` (alternatives auto, priority 50; gnome-terminal.wrapper priority 40) |
| gnome-terminal | ya | GNOME Terminal 3.52.0, VTE 0.76.0 |
| ghostty | ya | 1.3.1 stable |
| kgx, ptyxis, konsole, xfce4-terminal, kitty, wezterm, alacritty, foot, tilix, xterm, terminator | tidak | — |

Uji yang dijalankan:

1. **Membuka satu jendela gnome-terminal** dengan worktree tiruan di scratchpad yang path-nya sengaja berisi spasi, `'`, `"`, dan `$x` (`…/scratchpad/wt dir 'q" $x`). Launcher `…/.agentgate/tmp/launch.sh` melakukan `cd "$(dirname "$0")/../.."`, menulis file penanda, lalu `echo agentgate-spike; sleep 3`.
   - argv: `gnome-terminal --window --title=agentgate-spike --working-directory=<dir> -- <launcher>`
   - Hasil: exit code **0** setelah **0.51 detik**, jadi client detach jauh sebelum `sleep 3` selesai. Penanda `launch.started` berisi cwd yang benar (path aneh utuh, tanpa injeksi), dan `launch.done` berisi `exit=0` setelah sekitar 3 detik.
   - Kesimpulan: argv `--` aman dan benar. Exit code 0 **tidak** membuktikan apa pun tentang command di dalamnya, sehingga handshake/penanda diperlukan.
2. **Tanpa sesi grafis** (`env -i PATH=/usr/bin:/bin HOME=$HOME gnome-terminal --window -- /bin/true`): exit **1**, stderr `Failed to parse arguments: Cannot open display:`, tidak ada jendela. Kegagalan cepat seperti ini bisa dideteksi dari exit code.
3. **Program tidak ada**: shell exit 127. Di Rust, ini muncul sebagai `io::ErrorKind::NotFound` dari `spawn()`.
4. **systemd user env**: `systemctl --user show-environment` berisi `DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-0`, `XDG_SESSION_TYPE=wayland`, sehingga runner sebagai `systemd --user` service di host ini akan bisa membuka terminal (belum diuji langsung).

Tidak diuji: ghostty, terminal lain, `gnome-terminal --wait`, macOS, dan Windows. Semua yang ditandai "belum terverifikasi" perlu diuji di M5 lewat GitHub Actions (`windows-latest`, `macos-latest`, dengan catatan CI tidak punya desktop interaktif, jadi yang bisa diuji di sana hanya spawn/exit code) dan uji manual di mesin dev macOS/Windows.

## Sumber

- Windows Terminal command line arguments: https://learn.microsoft.com/en-us/windows/terminal/command-line-arguments
- Windows Terminal `windowingBehavior`: https://learn.microsoft.com/en-us/windows/terminal/customize-settings/startup#new-instance-behavior
- WezTerm `wezterm start`: https://wezterm.org/cli/start.html
- Ghostty config reference (`working-directory`, `-e`, `gtk-single-instance`): https://ghostty.org/docs/config/reference
- GNOME Terminal: `gnome-terminal --help-all` di host (3.52.0)
- Ghostty: `ghostty +help` di host (1.3.1), termasuk catatan macOS `open -na Ghostty.app --args …`
- Debian Policy, virtual package `x-terminal-emulator` (`-e`, `-T`): https://www.debian.org/doc/debian-policy/ch-customized-programs.html#packages-providing-a-terminal-emulator
- xdg-terminal-exec (proposal): https://gitlab.freedesktop.org/terminal-wg/specifications/-/merge_requests/3 (belum terverifikasi status terbarunya)
- Rust CVE-2024-24576 (BatBadBut): https://blog.rust-lang.org/2024/04/09/cve-2024-24576.html
- Rust `std::os::unix::process::CommandExt::process_group`: https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html
- Rust `std::os::windows::process::CommandExt::creation_flags`: https://doc.rust-lang.org/std/os/windows/process/trait.CommandExt.html
- Crate `which`: https://crates.io/crates/which; crate `shlex`: https://crates.io/crates/shlex
- Apple TCC Automation / `-1743`: https://developer.apple.com/documentation/bundleresources/information_property_list/nsappleeventsusagedescription (belum terverifikasi detail atribusi responsible app)

Hanya halaman Microsoft Learn, WezTerm, dan Ghostty yang dibaca selama spike ini. Sumber lain adalah referensi umum yang belum dibuka ulang.

## Risiko

- **Matriks terminal besar dan berubah.** Flag bisa berubah antarversi; contohnya `-e` gnome-terminal sudah deprecated, dan GNOME pindah ke ptyxis. Mitigasinya: preset hanya untuk terminal populer, sisanya lewat template custom, plus perintah diagnostik `agentgate doctor terminal` yang membuka jendela uji.
- **Terminal Flatpak/Snap** mungkin tidak bisa mengeksekusi binary host atau mengakses state dir.
- **Detach + handshake timeout** bisa salah positif (jendela terbuka tetapi lambat, misalnya cold start Windows Terminal). Mitigasinya: attach terlambat tetap diterima, dan notifikasi fallback bisa ditarik kembali.
- **Focus/raise di Wayland**: jendela bisa terbuka di belakang. User perlu notifikasi desktop tambahan (`notify-rust`, opsional).
- **macOS tanpa TCC** bergantung pada `open -a … .command`. Perilaku iTerm2 dan Gatekeeper belum terverifikasi.
- **Windows**: app execution alias `wt.exe` bisa mati, path dengan `;` atau karakter spesial cmd, mesin korporat dengan kebijakan AppLocker/WDAC yang memblokir `.cmd` dari `%LOCALAPPDATA%`, serta SmartScreen.
- **Keamanan IPC lokal**: socket/named pipe harus dibatasi ke user yang sama (`0600` di dir `0700`; ACL named pipe default sudah per user, belum terverifikasi). Nonce mencegah proses lain mengklaim job.
- **Env bocor**: terminal mewarisi env runner, termasuk token. Runner harus `env_remove` variabel sensitif sebelum spawn, dan `attach` mengambil kredensial dari keyring/state sendiri.
- **CI tidak punya desktop**, sehingga uji end-to-end "jendela benar-benar terbuka" untuk macOS/Windows tetap manual.

## Dampak ke M5

- Tambah subcommand **`agentgate job attach <job-id>`**, yang menjadi pintu masuk tunggal sesi interaktif, baik dari terminal otomatis maupun manual.
- Modul `cli::terminal`:
  - `detect() -> Vec<Candidate>` per OS (`cfg(target_os)`).
  - `Candidate { program, args_template }`.
  - `render(template, cwd, launcher, title) -> Vec<OsString>` dengan substitusi utuh per elemen.
  - `launch() -> Result<LaunchOutcome>`, yang memuat spawn + pengamatan exit cepat ≤ 3 detik.
- Modul `cli::launcher`: menulis script per OS (POSIX single-quote, validasi karakter `.cmd`), permission `0700`, dan cleanup.
- IPC lokal runner ↔ attach: Unix socket / named pipe, pesan `attached`/`exited`, nonce, dan timeout handshake yang bisa dikonfigurasi.
- Setting runner per user per OS: `terminal.{linux,macos,windows}` = `preset` atau `program`+`args`, plus `handshake_timeout`.
- Notifikasi fallback ke server (event job `terminal_open_failed` dengan alasan + perintah manual), status job `waiting_manual_attach`.
- `agentgate doctor terminal` untuk diagnosis.
- Dependensi: `which`. `shlex` opsional (fungsi quoting sendiri sudah cukup). Tidak perlu `open`/`opener`.
- Uji:
  - Unit test render argv dan quoting launcher, termasuk path dengan spasi, `'`, `"`, `$`, `;`, dan unicode.
  - Integration test spawn dengan terminal palsu (script yang mencatat argv).
  - Uji manual di tiga OS sebelum M5 dianggap selesai.
- Tutup open question di spec: "Cara membuka terminal interaktif dengan sesi client di Windows, macOS, dan Linux".
