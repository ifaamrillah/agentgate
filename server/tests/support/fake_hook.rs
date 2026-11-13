//! Penerima webhook palsu untuk tes M11: server Axum di `127.0.0.1:0` yang mencatat setiap
//! `POST /hook` (header + body) dan membalas status dari antrean [`FakeHook::respond`]
//! (kosong = 200). Tanpa jaringan nyata.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};

#[derive(Debug, Clone)]
pub struct Received {
    pub headers: HeaderMap,
    pub body: String,
}

impl Received {
    pub fn header(&self, name: &str) -> String {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string()
    }
}

#[derive(Default)]
struct Inner {
    received: Vec<Received>,
    statuses: VecDeque<u16>,
}

type Shared = Arc<Mutex<Inner>>;

pub struct FakeHook {
    /// `http://127.0.0.1:<port>/hook`.
    pub url: String,
    state: Shared,
}

impl FakeHook {
    /// Permintaan yang diterima, urut waktu.
    pub fn received(&self) -> Vec<Received> {
        self.state.lock().unwrap().received.clone()
    }

    /// Buang antrean status: permintaan berikutnya dibalas 200.
    pub fn clear(&self) {
        self.state.lock().unwrap().statuses.clear();
    }

    /// Balas `statuses` untuk permintaan berikutnya (berurutan), lalu kembali 200.
    pub fn respond(&self, statuses: &[u16]) {
        self.state.lock().unwrap().statuses.extend(statuses);
    }
}

async fn hook(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    let mut s = state.lock().unwrap();
    s.received.push(Received {
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    });
    let code = s.statuses.pop_front().unwrap_or(200);
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::OK);
    (status, format!("{{\"received\":{}}}", status.is_success()))
}

pub async fn start() -> FakeHook {
    let state: Shared = Arc::default();
    let app = Router::new()
        .route("/hook", post(hook))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FakeHook {
        url: format!("http://{addr}/hook"),
        state,
    }
}
