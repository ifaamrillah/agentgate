use axum::http::StatusCode;
use loco_rs::{
    Result,
    controller::views::{ViewRenderer, engines::TeraView},
};
use serde_json::json;

/// Dipakai hanya bila template error sendiri gagal dirender.
pub const FALLBACK_HTML: &str = "<!doctype html><html lang=\"id\"><head><meta charset=\"utf-8\"><title>Terjadi kesalahan</title></head><body><p>Terjadi kesalahan. Silakan coba lagi.</p></body></html>";

/// Semua kunci yang bisa dikembalikan `message_key`; masing-masing punya `-title` dan `-body`
/// di Fluent (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    "error-403",
    "error-404",
    "error-422",
    "error-429",
    "error-500",
    "error-503",
    "error-4xx",
];

/// Kunci Fluent untuk satu status. Status tanpa teks khusus memakai teks umum 4xx/5xx.
fn message_key(status: StatusCode) -> &'static str {
    match status.as_u16() {
        403 => "error-403",
        404 => "error-404",
        422 => "error-422",
        429 => "error-429",
        503 => "error-503",
        s if s >= 500 => "error-500",
        _ => "error-4xx",
    }
}

pub fn page(view: &TeraView, status: StatusCode, request_id: &str) -> Result<String> {
    view.render(
        "errors/error.html",
        json!({
            "status": status.as_u16(),
            "key": message_key(status),
            "request_id": request_id,
        }),
    )
}
