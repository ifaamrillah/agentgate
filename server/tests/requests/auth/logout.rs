use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

use crate::support::{a_user, sign_in_as};

#[tokio::test]
async fn logout_ends_the_session() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.post("/logout").add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login");
        // Cookie lama tidak lagi membuka halaman.
        let home = request.get("/account").add_cookie(cookie).await;
        assert_eq!(home.status_code(), 303);
    })
    .await;
}

#[tokio::test]
async fn get_logout_does_not_sign_out() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let res = request.get("/logout").add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 405);
        let home = request.get("/account").add_cookie(cookie).await;
        assert_eq!(home.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn logout_with_stale_cookie_still_redirects() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let cookie = sign_in_as(&request, &user).await;
        let first = request.post("/logout").add_cookie(cookie.clone()).await;
        assert_eq!(first.status_code(), 303);
        // Sesi sudah dihapus di server; cookie yang sama dikirim lagi.
        let again = request.post("/logout").add_cookie(cookie).await;
        assert_eq!(again.status_code(), 303);
        assert_eq!(again.header("location"), "/login");
        // Tanpa cookie sama sekali juga lolos gate.
        let none = request.post("/logout").await;
        assert_eq!(none.status_code(), 303);
        assert_eq!(none.header("location"), "/login");
    })
    .await;
}
