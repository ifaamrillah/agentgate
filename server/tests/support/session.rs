//! Helper login untuk tes request (rules §7 Testing auth): login lewat `POST /login` dengan
//! `TEST_PASSWORD`, lalu cookie sesi dikirim eksplisit di request berikutnya. `TestServer`
//! tidak menyimpan cookie (`save_cookies: false`), jadi setiap tes memegang cookie-nya sendiri.

use std::{future::Future, net::SocketAddr};

use agentgate_server::{app::App, models::users};
use loco_rs::{
    TestServer,
    app::{AppContext, Hooks},
    auth::cookies::Cookie,
    boot::StartMode,
    environment::Environment,
};

use super::TEST_PASSWORD;

/// Login sebagai `user` dan kembalikan cookie sesinya. Panic bila login tidak berakhir
/// dengan sesi (mis. user belum terverifikasi atau punya 2FA).
pub async fn sign_in_as(request: &TestServer, user: &users::Model) -> Cookie<'static> {
    let res = request
        .post("/login")
        .form(&[("email", user.email.as_str()), ("password", TEST_PASSWORD)])
        .await;
    assert_eq!(res.status_code(), 303, "login gagal: {}", res.text());
    assert_eq!(res.header("location"), "/");
    session_cookie(&res).expect("login tidak memasang cookie sesi")
}

/// Cookie sesi yang dipasang respons, bila ada (nama cookie memakai suffix worktree).
pub fn session_cookie(res: &axum_test::TestResponse) -> Option<Cookie<'static>> {
    res.cookies()
        .iter()
        .find(|c| c.name().starts_with("agentgate_session"))
        .cloned()
}

/// Seperti `request::<App, _, _>`, tetapi `settings:` config diubah dulu oleh `edit` sebelum
/// boot (mis. `absolute_lifetime_hours: 0`).
pub async fn request_with_settings<E, F, Fut>(edit: E, callback: F)
where
    E: FnOnce(&mut serde_json::Value),
    F: FnOnce(TestServer, AppContext) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut config = App::load_config(&Environment::Test).await.unwrap();
    edit(config.settings.as_mut().unwrap());
    let boot = App::boot(StartMode::ServerOnly, &Environment::Test, config)
        .await
        .unwrap();
    let server = TestServer::new(
        boot.router
            .unwrap()
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .unwrap();
    callback(server, boot.app_context).await;
}
