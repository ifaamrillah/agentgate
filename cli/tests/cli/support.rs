//! Server API palsu, direktori sementara, repo git, dan pemanggil binary.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Output,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde_json::Value;

/// Satu request yang diterima server palsu.
#[derive(Debug, Clone)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub at: Instant,
}

impl Req {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }

    pub fn form(&self) -> HashMap<String, String> {
        self.body
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.replace("%3A", ":").replace("%2F", "/")))
            .collect()
    }
}

/// Handler rute palsu: (server, request) → (status, body JSON).
pub type Handler = Arc<dyn Fn(&Fake, &Req) -> (u16, Value) + Send + Sync>;

pub struct Fake {
    pub base: String,
    pub reqs: Mutex<Vec<Req>>,
    pub handler: Handler,
}

impl Fake {
    pub fn requests(&self) -> Vec<Req> {
        self.reqs.lock().unwrap().clone()
    }

    pub fn find(&self, method: &str, path_suffix: &str) -> Vec<Req> {
        self.requests()
            .into_iter()
            .filter(|r| r.method == method && r.path.ends_with(path_suffix))
            .collect()
    }
}

async fn handle(
    State(fake): State<Arc<Fake>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let req = Req {
        method: method.to_string(),
        path: uri.path().to_string(),
        query: uri.query().unwrap_or_default().to_string(),
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
            .collect(),
        body: String::from_utf8_lossy(&body).into_owned(),
        at: Instant::now(),
    };
    fake.reqs.lock().unwrap().push(req.clone());
    let (status, body) = (fake.handler)(&fake, &req);
    let status = StatusCode::from_u16(status).unwrap();
    if body.is_null() {
        status.into_response()
    } else {
        (status, axum::Json(body)).into_response()
    }
}

/// Jalankan server palsu di port acak.
pub async fn serve(handler: Handler) -> Arc<Fake> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let fake = Arc::new(Fake {
        base,
        reqs: Mutex::new(Vec::new()),
        handler,
    });
    let app = Router::new().fallback(handle).with_state(fake.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    fake
}

pub fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("agentgate-it-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Repo git dengan satu commit awal dan identitas lokal.
pub fn git_repo(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.name", "Tes"]);
    git(dir, &["config", "user.email", "tes@example.com"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
    fs::write(dir.join("README.md"), "# repo\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", "awal"]);
}

/// Lingkungan terisolasi satu tes: HOME, config, state.
pub struct Env {
    pub root: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
    pub extra_path: Option<PathBuf>,
}

impl Env {
    pub fn new(tag: &str) -> Self {
        let root = temp_dir(tag);
        Self {
            config: root.join("config"),
            state: root.join("state"),
            root,
            extra_path: None,
        }
    }

    /// Tulis kredensial login untuk server palsu.
    pub fn login(&self, server: &str) {
        fs::create_dir_all(&self.config).unwrap();
        fs::write(
            self.config.join("credentials.json"),
            serde_json::json!({
                "server": server,
                "access_token": "at-test",
                "user": "Ani",
                "email": "ani@example.com",
                "workspace": "Kasir"
            })
            .to_string(),
        )
        .unwrap();
    }

    /// Jalankan `agentgate args…` di `cwd`.
    pub async fn run(&self, cwd: &Path, args: &[&str]) -> Output {
        self.run_with(cwd, args, &[], None).await
    }

    /// Seperti [`Env::run`], dengan env tambahan dan isi stdin.
    pub async fn run_with(
        &self,
        cwd: &Path,
        args: &[&str],
        vars: &[(&str, &str)],
        stdin: Option<&str>,
    ) -> Output {
        let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_agentgate"));
        cmd.args(args)
            .current_dir(cwd)
            .env("HOME", &self.root)
            .env("AGENTGATE_CONFIG_DIR", &self.config)
            .env("AGENTGATE_STATE_DIR", &self.state)
            .env("AGENTGATE_TIME_SCALE_MS", "5")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Tes")
            .env("GIT_AUTHOR_EMAIL", "tes@example.com")
            .env("GIT_COMMITTER_NAME", "Tes")
            .env("GIT_COMMITTER_EMAIL", "tes@example.com")
            .env_remove("AGENTGATE_SERVER")
            .env_remove("AGENTGATE_JOB_ID")
            .env_remove("AGENTGATE_CLAUDE_BIN")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .stdin(if stdin.is_some() {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in vars {
            cmd.env(k, v);
        }
        if let Some(extra) = &self.extra_path {
            let path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{path}", extra.display()));
        }
        let mut child = cmd.spawn().unwrap();
        if let Some(input) = stdin {
            use tokio::io::AsyncWriteExt;
            let mut pipe = child.stdin.take().unwrap();
            pipe.write_all(input.as_bytes()).await.unwrap();
            drop(pipe);
        }
        tokio::time::timeout(Duration::from_secs(90), child.wait_with_output())
            .await
            .expect("agentgate tidak selesai dalam 90 detik")
            .unwrap()
    }
}

pub fn text(out: &Output) -> String {
    format!(
        "status {:?}\n--- stdout\n{}\n--- stderr\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
