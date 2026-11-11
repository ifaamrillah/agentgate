use agentgate_server::{app::App, services::auth::dev_login};
use loco_rs::testing::prelude::*;

use crate::support::a_user;

#[tokio::test]
async fn dev_login_route_is_404_in_test_env() {
    request::<App, _, _>(|request, ctx| async move {
        let user = a_user().create(&ctx).await;
        let secret = ctx
            .config
            .auth
            .as_ref()
            .and_then(|a| a.session.as_ref())
            .and_then(|s| s.secret.clone())
            .unwrap();
        let token = dev_login::sign(&secret, user.id, chrono::Utc::now().timestamp());
        let res = request.get(&format!("/_dev/login?token={token}")).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
