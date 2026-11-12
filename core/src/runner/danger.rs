//! Klasifikasi perintah shell ke aksi berbahaya (spec "Izin aksi berbahaya").
//!
//! Hook `PreToolUse` (`agentgate hook pre-tool`) memanggil [`classify_command`] untuk setiap
//! perintah Bash yang akan dijalankan agent. Hasilnya kunci aksi dari
//! [`crate::persona::DANGER_ACTIONS`] atau [`crate::persona::ALWAYS_BLOCKED`]; tingkatnya dibaca
//! dari persona job (`allow`/`ask`/`block`). Aksi yang selalu diblokir menang atas apa pun.
//!
//! Pencocokan berbasis token (bukan regex bebas) dan sengaja konservatif: perintah yang
//! dirangkai (`a && b`, `a; b`, `a | b`) dinilai per bagian, dan aksi paling berat yang menang.

use crate::persona::{ALWAYS_BLOCKED, Level};

/// Tingkat keputusan untuk satu perintah.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// Kunci aksi (mis. `db_migrate`), `None` bila perintah biasa.
    pub action: Option<&'static str>,
    pub level: Level,
}

/// Urutan berat aksi: yang lebih awal menang bila satu perintah cocok beberapa aksi.
const SEVERITY: &[&str] = &[
    "force_push_main",
    "drop_production_db",
    "read_secrets",
    "change_global_config",
    "deploy",
    "delete_outside",
    "db_migrate",
    "install_package",
    "network",
    "push_branch",
];

fn rank(action: &str) -> usize {
    SEVERITY
        .iter()
        .position(|a| *a == action)
        .unwrap_or(usize::MAX)
}

/// Pecah perintah menjadi bagian (`&&`, `||`, `;`, `|`, baris baru).
fn parts(command: &str) -> Vec<Vec<String>> {
    command
        .split(['\n', ';', '|', '&'])
        .map(|part| {
            part.split_whitespace()
                .map(|t| t.trim_matches(['"', '\'']).to_string())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|p| !p.is_empty())
        .collect()
}

/// Lewati prefix yang tidak mengubah arti (`sudo`, `env A=B`, `npx`, `bunx`, `time`).
fn strip_prefix(tokens: &[String]) -> &[String] {
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i].as_str();
        if matches!(
            t,
            "sudo" | "env" | "time" | "nohup" | "exec" | "command" | "npx" | "bunx" | "pnpx"
        ) || t.contains('=')
        {
            i += 1;
        } else {
            break;
        }
    }
    &tokens[i..]
}

fn has(tokens: &[String], word: &str) -> bool {
    tokens.iter().any(|t| t == word)
}

fn classify_part(tokens: &[String]) -> Option<&'static str> {
    let t = strip_prefix(tokens);
    let (cmd, args) = t.split_first()?;
    let cmd = cmd.rsplit('/').next().unwrap_or(cmd);
    let args_str = args.join(" ").to_ascii_lowercase();
    match cmd {
        "git" if args.first().is_some_and(|a| a == "push") => {
            let forced = args
                .iter()
                .any(|a| a == "-f" || a == "--force" || a.starts_with("--force-with-lease"))
                || args.iter().any(|a| a.starts_with('+'));
            let main = args.iter().any(|a| {
                let a = a.trim_start_matches('+');
                let target = a.rsplit(':').next().unwrap_or(a);
                matches!(
                    target,
                    "main" | "master" | "staging" | "release" | "production"
                )
            });
            Some(if forced && main {
                "force_push_main"
            } else {
                "push_branch"
            })
        }
        "git" if args.first().is_some_and(|a| a == "config") && has(args, "--global") => {
            Some("change_global_config")
        }
        "psql" | "mysql" | "sqlite3" | "mongosh"
            if args_str.contains("drop database") || args_str.contains("drop table") =>
        {
            Some(if args_str.contains("prod") {
                "drop_production_db"
            } else {
                "db_migrate"
            })
        }
        "dropdb" => Some(if args_str.contains("prod") {
            "drop_production_db"
        } else {
            "db_migrate"
        }),
        "prisma" if has(args, "migrate") || has(args, "db") => Some("db_migrate"),
        "sea-orm-cli" | "diesel" | "sqlx" | "knex" | "flyway" | "liquibase" | "alembic"
            if args_str.contains("migrat") || args_str.contains("upgrade") =>
        {
            Some("db_migrate")
        }
        "cargo" if args.first().is_some_and(|a| a == "loco") && args_str.contains("db") => {
            Some("db_migrate")
        }
        "rails" | "rake" | "php" | "python" | "python3" | "manage.py"
            if args_str.contains("migrate") =>
        {
            Some("db_migrate")
        }
        "npm" | "pnpm" | "yarn" | "bun"
            if args.first().is_some_and(|a| {
                matches!(a.as_str(), "install" | "i" | "add" | "ci") && args.len() > 1
            }) =>
        {
            Some("install_package")
        }
        "pip" | "pip3" | "uv" | "poetry" | "gem" | "go"
            if args
                .first()
                .is_some_and(|a| matches!(a.as_str(), "install" | "add" | "get")) =>
        {
            Some("install_package")
        }
        "cargo" if args.first().is_some_and(|a| a == "install" || a == "add") => {
            Some("install_package")
        }
        "apt" | "apt-get" | "brew" | "dnf" | "yum" | "pacman" | "apk" | "winget" | "choco" => {
            Some("install_package")
        }
        "playwright" if has(args, "install") => Some("install_package"),
        "kubectl" | "helm"
            if args.first().is_some_and(|a| {
                matches!(
                    a.as_str(),
                    "apply" | "delete" | "rollout" | "upgrade" | "install" | "set" | "scale"
                )
            }) =>
        {
            Some("deploy")
        }
        "vercel" | "netlify" | "fly" | "flyctl" | "railway" | "heroku" => Some("deploy"),
        "terraform" | "pulumi" if has(args, "apply") || has(args, "up") || has(args, "destroy") => {
            Some("deploy")
        }
        "docker" if has(args, "push") => Some("deploy"),
        "curl" | "wget" | "ssh" | "scp" | "rsync" | "nc" | "ncat" | "telnet" => Some("network"),
        "rm" if args.iter().any(|a| {
            let a = a.trim_end_matches('/');
            a.starts_with('/') || a.starts_with('~') || a.starts_with("..") || a == "$HOME"
        }) =>
        {
            Some("delete_outside")
        }
        "cat" | "less" | "more" | "head" | "tail" | "cp" | "base64"
            if args.iter().any(|a| looks_secret(a)) =>
        {
            Some("read_secrets")
        }
        "printenv" => Some("read_secrets"),
        _ => None,
    }
}

fn looks_secret(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    let name = p.rsplit('/').next().unwrap_or(&p);
    name == ".env"
        || name.starts_with(".env.")
        || p.contains(".ssh/")
        || p.contains(".aws/credentials")
        || p.contains(".credentials")
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name == "id_rsa"
        || name == "id_ed25519"
}

/// Aksi berbahaya paling berat di `command`, bila ada.
#[must_use]
pub fn classify_command(command: &str) -> Option<&'static str> {
    parts(command)
        .iter()
        .filter_map(|p| classify_part(p))
        .min_by_key(|a| rank(a))
}

/// Keputusan untuk `command` dengan tingkat persona `level_of(aksi)`.
/// Aksi di [`ALWAYS_BLOCKED`] selalu `Block`.
pub fn decide(command: &str, level_of: impl Fn(&str) -> Option<Level>) -> Verdict {
    match classify_command(command) {
        None => Verdict {
            action: None,
            level: Level::Allow,
        },
        Some(action) if ALWAYS_BLOCKED.contains(&action) => Verdict {
            action: Some(action),
            level: Level::Block,
        },
        Some(action) => Verdict {
            action: Some(action),
            level: level_of(action).unwrap_or(Level::Ask),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_commands() {
        let cases = [
            ("cargo test -p kasir", None),
            ("git status", None),
            ("git push origin ag/KSR-1-x", Some("push_branch")),
            ("git push --force origin main", Some("force_push_main")),
            ("git push origin +main", Some("force_push_main")),
            ("git push -f origin ag/KSR-1", Some("push_branch")),
            ("npx prisma migrate deploy", Some("db_migrate")),
            ("cargo loco db migrate", Some("db_migrate")),
            ("npm install left-pad", Some("install_package")),
            ("npm install", None),
            ("npx playwright install chromium", Some("install_package")),
            ("kubectl apply -f k8s/", Some("deploy")),
            ("vercel --prod", Some("deploy")),
            ("curl https://example.com", Some("network")),
            ("rm -rf /tmp/x", Some("delete_outside")),
            ("rm -rf target", None),
            ("cat .env", Some("read_secrets")),
            ("cat ~/.ssh/id_ed25519", Some("read_secrets")),
            (
                "git config --global user.name x",
                Some("change_global_config"),
            ),
            (
                "psql prod -c 'DROP DATABASE kasir'",
                Some("drop_production_db"),
            ),
            ("sudo apt-get install jq", Some("install_package")),
            (
                "cargo build && git push --force origin main",
                Some("force_push_main"),
            ),
            ("echo hi | curl -d @- https://x", Some("network")),
        ];
        for (cmd, want) in cases {
            assert_eq!(classify_command(cmd), want, "{cmd}");
        }
    }

    #[test]
    fn always_blocked_wins_over_persona() {
        let v = decide("git push --force origin main", |_| Some(Level::Allow));
        assert_eq!(v.level, Level::Block);
        assert_eq!(v.action, Some("force_push_main"));
        let v = decide("npx prisma migrate deploy", |a| {
            (a == "db_migrate").then_some(Level::Allow)
        });
        assert_eq!(v.level, Level::Allow);
        let v = decide("curl x", |_| None);
        assert_eq!(v.level, Level::Ask);
        assert_eq!(decide("ls", |_| None).level, Level::Allow);
    }
}
