//! `agentgate status` dan `agentgate doctor`.

use std::{io::Write, process::Command};

use chrono::Utc;

use crate::{
    auth,
    config::{Credentials, Paths, RunnerIdentity},
    error::Result,
    git, hook, install,
    runner::clients::ClientKind,
};

/// `agentgate status`: akun, runner, dan aturan repo saat ini.
///
/// # Errors
///
/// Belum login, server gagal.
pub async fn status(paths: &Paths, out: &mut impl Write) -> Result<()> {
    let (api, _) = auth::session(paths).await?;
    let me = auth::me(&api).await?;
    let _ = writeln!(
        out,
        "Server     {}\nAkun       {} <{}>\nWorkspace  {} ({})",
        api.base(),
        me.user,
        me.email,
        me.workspace,
        me.slug
    );
    match RunnerIdentity::load(paths)? {
        Some(r) if r.runner_id.is_some() && r.server == api.base() => {
            let _ = writeln!(
                out,
                "Runner     {} ({})",
                r.name,
                r.runner_id.map(|id| id.to_string()).unwrap_or_default()
            );
        }
        _ => {
            let _ = writeln!(out, "Runner     belum terdaftar (`agentgate runner start`)");
        }
    }
    let root = std::env::current_dir().ok().and_then(|d| git::toplevel(&d));
    match root {
        None => {
            let _ = writeln!(out, "Repo       (bukan repo git)");
        }
        Some(root) => match install::repo_status(&api, &root).await {
            Ok(None) => {
                let _ = writeln!(out, "Repo       {} belum di-init", root.display());
            }
            Ok(Some((project, drift))) => {
                let state = if drift.changed.is_empty() {
                    "sesuai server".to_string()
                } else {
                    format!("berbeda: {}", drift.changed.join(", "))
                };
                let _ = writeln!(
                    out,
                    "Repo       {} · project {} · aturan {state}",
                    root.display(),
                    project.project.key
                );
            }
            Err(e) => {
                let _ = writeln!(out, "Repo       {} · {}", root.display(), e.user_message());
            }
        },
    }
    Ok(())
}

fn version_of(program: &str) -> Option<String> {
    let out = Command::new(program).arg("--version").output().ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    })
}

/// `agentgate doctor`. `Ok(false)` bila ada pemeriksaan wajib yang gagal.
///
/// # Errors
///
/// Error IO saat membaca konfigurasi.
pub async fn doctor(paths: &Paths, out: &mut impl Write) -> Result<bool> {
    let mut ok = true;
    let mut check = |pass: bool, required: bool, label: &str, detail: String| {
        let mark = if pass {
            "✓"
        } else if required {
            "✗"
        } else {
            "!"
        };
        if !pass && required {
            ok = false;
        }
        let _ = writeln!(out, "{mark} {label:<12} {detail}");
    };
    let creds = Credentials::load(paths)?;
    match &creds {
        Some(c) if c.expiring(Utc::now()) && c.refresh_token.is_none() => check(
            false,
            true,
            "Login",
            "token kedaluwarsa; jalankan `agentgate login`".into(),
        ),
        Some(c) => check(true, true, "Login", format!("{} di {}", c.email, c.server)),
        None => check(
            false,
            true,
            "Login",
            "belum login (`agentgate login`)".into(),
        ),
    }
    let session = if creds.is_some() {
        auth::session(paths).await.ok()
    } else {
        None
    };
    match &session {
        Some((api, _)) => match auth::me(api).await {
            Ok(me) => check(
                true,
                true,
                "Server",
                format!("{} (versi {})", api.base(), me.version),
            ),
            Err(e) => check(
                false,
                true,
                "Server",
                crate::error::Error::from(e).user_message(),
            ),
        },
        None => check(false, true, "Server", "tidak dicek (belum login)".into()),
    }
    match version_of("git") {
        Some(v) => check(true, true, "git", v),
        None => check(false, true, "git", "tidak ditemukan di PATH".into()),
    }
    // Client default runner (`AGENTGATE_CLIENT`, bawaan Claude Code) wajib ada; client lain
    // hanya dicatat karena job memakai client pilihan server per runner.
    let default = std::env::var("AGENTGATE_CLIENT")
        .ok()
        .and_then(|c| ClientKind::parse(&c))
        .unwrap_or(ClientKind::Claude);
    for client in ClientKind::all() {
        let program = client.program();
        let required = *client == default;
        match version_of(&program) {
            Some(v) => check(true, required, client.name(), format!("{v} ({program})")),
            None => check(
                false,
                required,
                client.name(),
                format!(
                    "`{program}` tidak ditemukan{}",
                    if required {
                        "; pasang client ini untuk menjalankan job"
                    } else {
                        " (opsional)"
                    }
                ),
            ),
        }
    }
    match hook::which("rtk") {
        Some(p) => check(true, false, "RTK", p.display().to_string()),
        None => check(
            false,
            false,
            "RTK",
            "tidak terpasang; output perintah agent tidak dikompresi".into(),
        ),
    }
    let root = std::env::current_dir().ok().and_then(|d| git::toplevel(&d));
    match (root, &session) {
        (Some(root), Some((api, _))) => match install::repo_status(api, &root).await {
            Ok(Some((_, d))) if d.changed.is_empty() => {
                check(true, true, "Aturan", "sesuai versi server".into());
            }
            Ok(Some((_, d))) => check(
                false,
                true,
                "Aturan",
                format!("berbeda: {} (`agentgate rules sync`)", d.changed.join(", ")),
            ),
            Ok(None) => check(true, false, "Aturan", "repo ini belum di-init".into()),
            Err(e) => check(false, true, "Aturan", e.user_message()),
        },
        _ => check(true, false, "Aturan", "tidak dicek".into()),
    }
    Ok(ok)
}
