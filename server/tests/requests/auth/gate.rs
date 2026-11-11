use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

#[tokio::test]
async fn protected_page_redirects_to_login_with_return_to() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/?tab=a%20b").await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            res.header("location"),
            "/login?return_to=%2F%3Ftab%3Da%2520b"
        );

        // Non-GET tanpa sesi: ke /login tanpa return_to (tidak ada halaman untuk kembali).
        let post = request.post("/").await;
        assert_eq!(post.status_code(), 303);
        assert_eq!(post.header("location"), "/login");
    })
    .await;
}

#[tokio::test]
async fn public_routes_are_reachable_without_a_session() {
    request::<App, _, _>(|request, _ctx| async move {
        for path in ["/login", "/_dev/ui", "/_ping", "/_health"] {
            let res = request.get(path).await;
            assert_eq!(res.status_code(), 200, "{path}");
        }
    })
    .await;
}
