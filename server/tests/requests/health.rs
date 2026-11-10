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
