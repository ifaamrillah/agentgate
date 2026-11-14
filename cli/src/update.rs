//! `agentgate self-update`: jalankan ulang installer rilis lewat server yang sedang dipakai
//! (`{server}/install.sh` atau `{server}/install.ps1`, M12). Server meneruskan ke rilis CLI yang
//! dikonfigurasi admin, jadi CLI tidak perlu tahu alamat rilisnya sendiri.

use std::process::Command;

use crate::error::{Error, Result};

/// Program dan argumen yang menjalankan installer untuk OS `os` (`std::env::consts::OS`).
#[must_use]
pub fn installer_command(server: &str, os: &str) -> (String, Vec<String>) {
    let server = server.trim_end_matches('/');
    if os == "windows" {
        (
            "powershell".into(),
            vec![
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-Command".into(),
                format!("irm {server}/install.ps1 | iex"),
            ],
        )
    } else {
        (
            "sh".into(),
            vec!["-c".into(), format!("curl -fsSL {server}/install.sh | sh")],
        )
    }
}

/// Jalankan installer; output installer langsung ke terminal user.
///
/// # Errors
///
/// Program tidak bisa dijalankan atau installer keluar dengan status gagal.
pub fn run(server: &str) -> Result<()> {
    let (program, args) = installer_command(server, std::env::consts::OS);
    println!("Mengunduh installer dari {}…", server.trim_end_matches('/'));
    let status = Command::new(&program)
        .args(&args)
        .status()
        .map_err(|source| Error::Spawn {
            program: program.clone(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::UpdateFailed(status.code().unwrap_or(-1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_goes_through_the_server() {
        let (p, a) = installer_command("https://ag.example.com/", "linux");
        assert_eq!(p, "sh");
        assert_eq!(a[1], "curl -fsSL https://ag.example.com/install.sh | sh");
        let (p, a) = installer_command("https://ag.example.com", "windows");
        assert_eq!(p, "powershell");
        assert_eq!(
            a.last().unwrap(),
            "irm https://ag.example.com/install.ps1 | iex"
        );
    }
}
