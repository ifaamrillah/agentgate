use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

#[tokio::test]
async fn health_and_ping_respond_ok() {
    request::<App, _, _>(|request, _ctx| async move {
        assert_eq!(request.get("/_health").await.status_code(), 200);
        assert_eq!(request.get("/_ping").await.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn readiness_is_public_and_checks_dependencies() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/_readiness").await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.json::<serde_json::Value>()["ok"], true);
    })
    .await;
}

/// `/_metrics` 404 tanpa token yang benar; dengan token membalas teks Prometheus berisi request
/// per route (pola route, bukan URL asli), antrean, pool, dan login gagal.
#[tokio::test]
async fn metrics_require_the_bearer_token_and_report_routes() {
    request::<App, _, _>(|request, _ctx| async move {
        // test.yaml: settings.system.metrics_token = "metrik-tes".
        assert_eq!(request.get("/_metrics").await.status_code(), 404);
        let res = request
            .get("/_metrics")
            .add_header("authorization", "Bearer salah")
            .await;
        assert_eq!(res.status_code(), 404);
        request.get("/login").await;
        let res = request
            .post("/login")
            .form(&[("email", "tidak@ada.test"), ("password", "salah-sekali")])
            .await;
        assert!(res.status_code().is_client_error() || res.status_code() == 200);
        let res = request
            .get("/_metrics")
            .add_header("authorization", "Bearer metrik-tes")
            .await;
        assert_eq!(res.status_code(), 200);
        let body = res.text();
        assert!(
            body.contains(
                r#"agentgate_http_requests_total{method="GET",route="/login",status="200"}"#
            ),
            "{body}"
        );
        assert!(body.contains("agentgate_http_request_duration_seconds_bucket"));
        assert!(body.contains("agentgate_queue_oldest_queued_seconds"));
        assert!(body.contains("agentgate_db_pool_max"));
        assert!(body.contains("agentgate_login_failures_total"));
        assert!(!body.contains("tidak@ada.test"));
    })
    .await;
}
