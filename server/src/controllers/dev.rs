//! Route khusus development/test: katalog komponen `/_dev/ui`. Hanya dikompilasi di build
//! debug dan hanya didaftarkan di environment development/test. Tidak menyentuh database.
use loco_rs::prelude::*;

use crate::views;

#[debug_handler]
async fn ui(
    ViewEngine(v): ViewEngine<TeraView>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    views::dev::ui(&v, &query)
}

/// Dipakai tes CSRF: POST/DELETE yang lolos membalas 200.
async fn echo() -> Result<Response> {
    format::text("ok")
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/_dev")
        .add("/ui", get(ui))
        .add("/echo", post(echo).delete(echo))
}
