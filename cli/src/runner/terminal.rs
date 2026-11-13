//! Terminal untuk job interaktif: launcher di state dir yang menjalankan
//! `agentgate job attach <id>`, dibuka di jendela terminal sesuai OS. Program terminal bisa
//! diganti lewat `AGENTGATE_TERMINAL` (program + argumen; path launcher ditambahkan di akhir).

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use uuid::Uuid;

use crate::error::{Result, io_at};

/// Tulis launcher job `job` (dapat dieksekusi) di `<state>/launch/`.
///
/// # Errors
///
/// IO.
pub fn write_launcher(state: &Path, exe: &Path, job: Uuid) -> Result<PathBuf> {
    let dir = state.join("launch");
    std::fs::create_dir_all(&dir).map_err(io_at(&dir))?;
    let exe = exe.to_string_lossy();
    if cfg!(windows) {
        let path = dir.join(format!("{job}.cmd"));
        let body = format!("@echo off\r\n\"{exe}\" job attach {job}\r\npause\r\n");
        std::fs::write(&path, body).map_err(io_at(&path))?;
        Ok(path)
    } else {
        // `.command` dibuka Terminal.app di macOS; di Linux dijalankan langsung.
        let path = dir.join(format!("{job}.command"));
        let body = format!(
            "#!/bin/sh\n{} job attach {job}\nprintf '\\nTekan Enter untuk menutup.'\nread _\n",
            sh_quote(&exe)
        );
        std::fs::write(&path, body).map_err(io_at(&path))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                .map_err(io_at(&path))?;
        }
        Ok(path)
    }
}

/// Kutip satu argumen untuk `sh` (hanya path executable; data tiket tidak pernah lewat sini).
#[must_use]
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Kandidat perintah pembuka terminal untuk `os` (`std::env::consts::OS`).
#[must_use]
pub fn candidates(os: &str, launcher: &Path, custom: Option<&str>) -> Vec<Vec<String>> {
    let l = launcher.to_string_lossy().into_owned();
    let mut out = Vec::new();
    if let Some(custom) = custom.filter(|c| !c.trim().is_empty()) {
        let mut parts: Vec<String> = custom.split_whitespace().map(str::to_string).collect();
        parts.push(l.clone());
        out.push(parts);
    }
    match os {
        "macos" => out.push(vec!["open".into(), "-a".into(), "Terminal".into(), l]),
        "windows" => out.push(vec![
            "cmd".into(),
            "/c".into(),
            "start".into(),
            String::new(),
            l,
        ]),
        _ => {
            out.push(vec!["x-terminal-emulator".into(), "-e".into(), l.clone()]);
            out.push(vec!["gnome-terminal".into(), "--".into(), l.clone()]);
            out.push(vec!["konsole".into(), "-e".into(), l.clone()]);
            out.push(vec!["xterm".into(), "-e".into(), l]);
        }
    }
    out
}

/// Buka launcher di terminal; `false` bila tidak ada terminal yang bisa dijalankan.
#[must_use]
pub fn open(launcher: &Path) -> bool {
    let custom = std::env::var("AGENTGATE_TERMINAL").ok();
    let os = std::env::consts::OS;
    let graphical =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    // Linux tanpa sesi grafis: emulator terminal pasti gagal, kecuali program kustom.
    if os == "linux" && !graphical && custom.as_deref().is_none_or(|c| c.trim().is_empty()) {
        return false;
    }
    for cmd in candidates(os, launcher, custom.as_deref()) {
        let Some((program, args)) = cmd.split_first() else {
            continue;
        };
        let spawned = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if spawned.is_ok() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    #[test]
    fn launcher_runs_job_attach() {
        let state = temp_dir("launch");
        let job = Uuid::new_v4();
        let path = write_launcher(&state, Path::new("/opt/ag it's/agentgate"), job).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        if cfg!(unix) {
            assert!(body.contains(&format!("'/opt/ag it'\\''s/agentgate' job attach {job}")));
        }
    }

    #[test]
    fn terminal_candidates_per_os() {
        let l = Path::new("/s/x.command");
        assert_eq!(
            candidates("macos", l, None),
            [["open", "-a", "Terminal", "/s/x.command"]]
        );
        let linux = candidates("linux", l, Some("kitty -e"));
        assert_eq!(linux[0], ["kitty", "-e", "/s/x.command"]);
        assert_eq!(linux[1][0], "x-terminal-emulator");
        assert_eq!(
            candidates("windows", l, None)[0][..3],
            ["cmd", "/c", "start"]
        );
    }
}
