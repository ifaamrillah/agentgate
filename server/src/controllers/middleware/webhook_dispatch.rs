//! Kirim webhook setelah commit tanpa menyentuh setiap controller: request dijalankan di dalam
//! `outbox_events::collect_pending`, yang mengumpulkan id delivery yang ditulis
//! `outbox_events::publish` selama request. Setelah handler selesai (transaksinya sudah commit
//! atau rollback), id itu di-enqueue ke `WebhookDeliveryWorker`. Delivery dari transaksi yang
//! di-rollback tidak ada di database, jadi worker melewatinya. Enqueue best-effort: sapuan
//! scheduler menyelamatkan yang gagal.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use loco_rs::app::AppContext;

use crate::{models::outbox_events, workers::webhook_delivery};

pub async fn layer(State(ctx): State<AppContext>, req: Request, next: Next) -> Response {
    if matches!(req.method().as_str(), "GET" | "HEAD" | "OPTIONS") {
        return next.run(req).await;
    }
    let (res, ids) = outbox_events::collect_pending(next.run(req)).await;
    webhook_delivery::enqueue(&ctx, ids).await;
    res
}
