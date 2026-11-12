//! GitHub codeload palsu untuk tes impor skill pack: server Axum kecil di `127.0.0.1:0` yang
//! melayani `/{owner}/{repo}/tar.gz/{ref}` dengan tarball buatan tes (folder teratas
//! `repo-ref/`, header pax global berisi sha commit, seperti `git archive`).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};

pub const SHA: &str = "a41e7d0c9b1f2e3d4c5b6a7980716253443526f1";

type Repos = Arc<Mutex<HashMap<String, Vec<u8>>>>;

pub struct FakeCodeload {
    pub base_url: String,
    repos: Repos,
}

impl FakeCodeload {
    /// Daftarkan repo `owner/repo` (semua ref) dengan berkas `files` (path relatif akar repo).
    pub fn add_repo(&self, owner_repo: &str, files: &[(&str, &[u8])]) {
        let top = owner_repo.rsplit('/').next().unwrap();
        self.repos.lock().unwrap().insert(
            owner_repo.to_string(),
            tarball(&format!("{top}-main"), files),
        );
    }
}

/// `.tar.gz` dengan header pax global `comment=<SHA>` lalu berkas di bawah `top/`.
pub fn tarball(top: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
    let enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut b = tar::Builder::new(enc);
    let pax = format!("52 comment={SHA}\n");
    let mut h = tar::Header::new_ustar();
    h.set_path("pax_global_header").unwrap();
    h.set_entry_type(tar::EntryType::XGlobalHeader);
    h.set_size(pax.len() as u64);
    h.set_mode(0o644);
    h.set_cksum();
    b.append(&h, pax.as_bytes()).unwrap();
    for (path, body) in files {
        let mut h = tar::Header::new_gnu();
        h.set_size(body.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, format!("{top}/{path}"), *body)
            .unwrap();
    }
    b.into_inner().unwrap().finish().unwrap()
}

async fn serve(
    State(repos): State<Repos>,
    Path((owner, repo, _r)): Path<(String, String, String)>,
) -> impl IntoResponse {
    match repos.lock().unwrap().get(&format!("{owner}/{repo}")) {
        Some(bytes) => (StatusCode::OK, bytes.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn start() -> FakeCodeload {
    let repos: Repos = Arc::default();
    let app = Router::new()
        .route("/{owner}/{repo}/tar.gz/{*git_ref}", get(serve))
        .with_state(repos.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FakeCodeload {
        base_url: format!("http://{addr}"),
        repos,
    }
}
