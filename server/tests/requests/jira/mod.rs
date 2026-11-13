//! M11 Connector Jira terhadap Jira palsu lokal (`support::fake_jira`, 127.0.0.1:0, tanpa
//! jaringan nyata): halaman Pengaturan → Jira (`ui`), mirror, dua arah, dan impor sekali
//! (`sync`). Worker berjalan inline di tes, jadi sinkron selesai sebelum respons kembali.

mod sync;
mod ui;

use agentgate_server::models::{jira_connections, jira_sync_logs};
use loco_rs::{TestServer, app::AppContext, auth::cookies::Cookie};
use sea_orm::{EntityTrait, QueryOrder};

use crate::{
    requests::tickets::Fixture,
    support::{
        a_project, a_user, a_workspace,
        fake_jira::{self, FakeJira},
        sign_in_as,
    },
};

/// Seperti `tickets::fixture`, tetapi email Owner sama dengan user Jira palsu sehingga user
/// terpetakan otomatis lewat email.
pub async fn fixture(request: &TestServer, ctx: &AppContext) -> Fixture {
    let owner = a_user()
        .name("Olivia Owner")
        .email(fake_jira::EMAIL)
        .create(ctx)
        .await;
    let workspace = a_workspace(&owner).slug("acme").create(ctx).await;
    a_project(ctx, &workspace, "KSR").await;
    let cookie = sign_in_as(request, &owner).await;
    Fixture {
        owner,
        workspace,
        cookie,
    }
}

/// Simpan koneksi Cloud ke Jira palsu: project SR ↔ KSR.
pub async fn connect(request: &TestServer, cookie: &Cookie<'static>, fake: &FakeJira) {
    let res = request
        .post("/w/acme/settings/jira/conn")
        .add_cookie(cookie.clone())
        .form(&[
            ("env", "cloud"),
            ("base_url", fake.url.as_str()),
            ("email", fake_jira::EMAIL),
            ("token", fake_jira::TOKEN),
            ("jira_project", "SR"),
            ("project", "KSR"),
        ])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

/// Simpan mode (memulai sinkron inline).
pub async fn set_mode(request: &TestServer, cookie: &Cookie<'static>, mode: &str) {
    let res = request
        .post("/w/acme/settings/jira/mode")
        .add_cookie(cookie.clone())
        .form(&[("mode", mode)])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

/// Petakan user Jira (akun Olivia di Jira palsu) ke `user_id`.
pub async fn map_user(request: &TestServer, cookie: &Cookie<'static>, user_id: uuid::Uuid) {
    let res = request
        .post("/w/acme/settings/jira/fields")
        .add_cookie(cookie.clone())
        .form(&[(format!("u:{}", fake_jira::ACCOUNT), user_id.to_string())])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

/// Sinkron sekarang.
pub async fn sync_now(request: &TestServer, cookie: &Cookie<'static>) {
    let res = request
        .post("/w/acme/settings/jira/sync")
        .add_cookie(cookie.clone())
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

pub async fn connection(ctx: &AppContext) -> jira_connections::Model {
    jira_connections::Entity::find()
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("koneksi Jira ada")
}

/// Log sinkron, terlama dulu: (arah, hasil, pesan).
pub async fn logs(ctx: &AppContext) -> Vec<jira_sync_logs::Model> {
    jira_sync_logs::Entity::find()
        .order_by_asc(jira_sync_logs::Column::CreatedAt)
        .order_by_asc(jira_sync_logs::Column::Id)
        .all(&ctx.db)
        .await
        .unwrap()
}

pub async fn messages(ctx: &AppContext) -> Vec<String> {
    logs(ctx).await.into_iter().map(|l| l.message).collect()
}
