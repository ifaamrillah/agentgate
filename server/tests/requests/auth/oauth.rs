//! Login Google dan GitHub lewat provider palsu (`support::fake_oauth`).
use std::future::Future;

use agentgate_server::{
    app::App,
    models::{identities, users},
    services::auth::oauth::{self, OAuthOutcome},
};
use axum_test::{TestResponse, TestServer};
use loco_rs::{
    app::AppContext,
    auth::{cookies::Cookie, oauth::Identity},
    config::OAuthStrategy,
    testing::prelude::*,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::{Value, json};

use crate::support::{
    TEST_PASSWORD, a_user,
    fake_oauth::{self, FakeProvider},
    mail_to, request_with_config, session_cookie, token_in,
};

const NEW_PASSWORD: &str = "kuda baterai staples benar";

/// Boot app dengan `providers` (nama `google`/`github`, preset sesuai nama) menunjuk provider
/// palsu; `settings` boleh mengubah `settings:` config.
async fn with_fake<S, F, Fut>(providers: &[&str], settings: S, f: F)
where
    S: FnOnce(&mut Value),
    F: FnOnce(TestServer, AppContext, FakeProvider) -> Fut,
    Fut: Future<Output = ()>,
{
    let fake = fake_oauth::start().await;
    let configs: Vec<_> = providers
        .iter()
        .map(|name| {
            let strategy = match *name {
                "github" => OAuthStrategy::Github,
                _ => OAuthStrategy::Google,
            };
            ((*name).to_string(), fake.provider_config(strategy))
        })
        .collect();
    request_with_config(
        move |config| {
            let auth = config.auth.get_or_insert_with(Default::default);
            let oauth = auth.oauth.get_or_insert_with(Default::default);
            oauth.providers = configs.into_iter().collect();
            settings(config.settings.as_mut().unwrap());
        },
        |request, ctx| f(request, ctx, fake),
    )
    .await;
}

async fn with_google<F, Fut>(f: F)
where
    F: FnOnce(TestServer, AppContext, FakeProvider) -> Fut,
    Fut: Future<Output = ()>,
{
    with_fake(&["google"], |_| {}, f).await;
}

/// Nama cookie transaksi OAuth di test (dev/test: suffix worktree sama dengan cookie sesi).
fn state_cookie_name(provider: &str) -> String {
    let suffix = std::env::var("SESSION_COOKIE_SUFFIX").unwrap_or_default();
    format!("agentgate_oauth_{provider}{suffix}")
}

/// `POST /auth/{provider}`: cookie state dan URL authorize provider.
async fn start(
    request: &TestServer,
    provider: &str,
    return_to: Option<&str>,
) -> (Cookie<'static>, String) {
    let form: Vec<(&str, &str)> = return_to
        .map(|r| vec![("return_to", r)])
        .unwrap_or_default();
    let res = request.post(&format!("/auth/{provider}")).form(&form).await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let cookie = res
        .cookies()
        .iter()
        .find(|c| c.name() == state_cookie_name(provider))
        .cloned()
        .expect("start tidak memasang cookie state");
    (cookie, res.header("location").to_str().unwrap().to_string())
}

/// Ikuti URL authorize ke provider palsu; kembalikan path + query callback.
async fn authorize(location: &str) -> String {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let res = client.get(location).send().await.unwrap();
    assert!(res.status().is_redirection(), "{}", res.status());
    let target = res.headers()["location"].to_str().unwrap();
    let url = reqwest::Url::parse(target).unwrap();
    format!("{}?{}", url.path(), url.query().unwrap())
}

/// Alur lengkap: start, authorize, callback (dengan cookie sesi `session` bila ada).
async fn sign_in_with(
    request: &TestServer,
    provider: &str,
    return_to: Option<&str>,
    session: Option<Cookie<'static>>,
) -> TestResponse {
    let (state, location) = start(request, provider, return_to).await;
    let callback = authorize(&location).await;
    let mut req = request.get(&callback).add_cookie(state);
    if let Some(session) = session {
        req = req.add_cookie(session);
    }
    req.await
}

fn google_profile(sub: &str, email: &str, verified: bool) -> Value {
    json!({ "sub": sub, "email": email, "email_verified": verified, "name": "Gina Google" })
}

fn uid() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

fn new_email() -> String {
    format!("oauth-{}@example.test", uid())
}

async fn identities_of(ctx: &AppContext, user_id: uuid::Uuid) -> Vec<identities::Model> {
    identities::Entity::find()
        .filter(identities::Column::UserId.eq(user_id))
        .all(&ctx.db)
        .await
        .unwrap()
}

async fn reload(ctx: &AppContext, user: &users::Model) -> users::Model {
    users::Model::find_by_id(&ctx.db, user.id)
        .await
        .unwrap()
        .unwrap()
}

fn assert_state_cookie_cleared(res: &TestResponse, provider: &str) {
    let cookie = res
        .cookies()
        .iter()
        .find(|c| c.name() == state_cookie_name(provider))
        .cloned()
        .expect("callback tidak menghapus cookie state");
    assert_eq!(cookie.value(), "");
}

#[tokio::test]
async fn oauth_start_requires_post() {
    with_google(|request, _ctx, fake| async move {
        let get = request.get("/auth/google").await;
        assert!(
            [404, 405].contains(&get.status_code().as_u16()),
            "{}",
            get.status_code()
        );
        assert!(
            get.cookies()
                .iter()
                .all(|c| c.name() != state_cookie_name("google")),
            "GET tidak boleh memulai alur"
        );

        let (_, location) = start(&request, "google", None).await;
        assert!(
            location.starts_with(&format!("{}/authorize?", fake.base_url)),
            "{location}"
        );
        assert!(location.contains("state="), "{location}");
        assert!(location.contains("code_challenge="), "{location}");

        // Provider tak dikenal dan provider yang tidak dikonfigurasi: 404.
        for name in ["gitlab", "github"] {
            let res = request
                .post(&format!("/auth/{name}"))
                .form(&Vec::<(&str, &str)>::new())
                .await;
            assert_eq!(res.status_code(), 404, "{name}");
        }
    })
    .await;
}

#[tokio::test]
async fn flow_cookie_is_encrypted() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    with_google(|request, _ctx, _fake| async move {
        let (cookie, location) = start(&request, "google", Some("/account")).await;
        let url = reqwest::Url::parse(&location).unwrap();
        let state = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        let value = cookie.value();
        assert!(!value.contains(&state), "state terbaca di cookie");
        // Tidak ada bagian nilai cookie yang ter-decode menjadi JSON transaksi (verifier PKCE,
        // state, return_to): cookie bertanda tangan saja berisi `signature || base64(json)`.
        for start in 0..value.len() {
            if let Ok(bytes) = URL_SAFE_NO_PAD.decode(&value[start..]) {
                let text = String::from_utf8_lossy(&bytes);
                assert!(
                    !text.contains("verifier") && !text.contains(&state),
                    "transaksi OAuth terbaca: {text}"
                );
            }
        }
    })
    .await;
}

#[tokio::test]
async fn callback_with_wrong_state_is_rejected() {
    with_google(|request, ctx, fake| async move {
        let email = new_email();
        fake.set_identity(google_profile(&uid(), &email, true));

        // State di query tidak cocok dengan cookie.
        let (state, location) = start(&request, "google", None).await;
        authorize(&location).await;
        let res = request
            .get(&format!(
                "/auth/google/callback?code={}&state=forged",
                fake_oauth::CODE
            ))
            .add_cookie(state.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login?oauth_error=oauth-expired");
        assert_state_cookie_cleared(&res, "google");
        assert!(session_cookie(&res).is_none_or(|c| c.value().is_empty()));

        // Tanpa cookie state (tab lain, cookie diblokir, callback palsu).
        let callback = authorize(&location).await;
        let res = request.get(&callback).await;
        assert_eq!(res.header("location"), "/login?oauth_error=oauth-expired");

        assert!(
            users::Model::find_by_email(&ctx.db, &email)
                .await
                .unwrap()
                .is_none()
        );

        // Kontrol: state dan cookie yang cocok berhasil, jadi penolakan di atas karena state.
        let ok = request.get(&callback).add_cookie(state).await;
        assert_eq!(ok.status_code(), 303);
        assert_eq!(ok.header("location"), "/");
        assert_state_cookie_cleared(&ok, "google");
        assert!(
            users::Model::find_by_email(&ctx.db, &email)
                .await
                .unwrap()
                .is_some()
        );
    })
    .await;
}

#[tokio::test]
async fn provider_error_redirects_to_login_with_message() {
    with_google(|request, _ctx, _fake| async move {
        let (state, _) = start(&request, "google", None).await;
        let res = request
            .get("/auth/google/callback?error=access_denied&state=x")
            .add_cookie(state)
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/login?oauth_error=oauth-cancelled");
        assert_state_cookie_cleared(&res, "google");

        let page = request.get("/login?oauth_error=oauth-cancelled").await;
        assert_eq!(page.status_code(), 200);
        assert!(page.text().contains("Login dibatalkan."), "{}", page.text());

        // Kode tak dikenal tidak dirender.
        let page = request.get("/login?oauth_error=bogus").await;
        assert_eq!(page.status_code(), 200);
        assert!(!page.text().contains("bogus"));
    })
    .await;
}

#[tokio::test]
async fn new_verified_identity_creates_user() {
    with_google(|request, ctx, fake| async move {
        let sub = uid();
        let email = new_email();
        fake.set_identity(google_profile(&sub, &email, true));

        let res = sign_in_with(&request, "google", Some("/account"), None).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/account");
        assert_state_cookie_cleared(&res, "google");

        let user = users::Model::find_by_email(&ctx.db, &email)
            .await
            .unwrap()
            .expect("user dibuat");
        assert!(user.email_verified_at.is_some(), "email terverifikasi");
        assert!(user.password_hash.is_none(), "tanpa password");
        assert_eq!(user.name, "Gina Google");
        let linked = identities_of(&ctx, user.id).await;
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].provider, "google");
        assert_eq!(linked[0].provider_uid, sub);

        let cookie = session_cookie(&res).unwrap();
        assert_eq!(request.get("/").add_cookie(cookie).await.status_code(), 200);

        // Login berikutnya memakai identitas yang sama, tanpa user baru.
        let again = sign_in_with(&request, "google", None, None).await;
        assert_eq!(again.header("location"), "/");
        assert_eq!(
            users::Entity::find()
                .filter(users::Column::Email.eq(email.clone()))
                .count(&ctx.db)
                .await
                .unwrap(),
            1
        );
    })
    .await;
}

#[tokio::test]
async fn unsafe_return_to_is_dropped() {
    with_google(|request, _ctx, fake| async move {
        fake.set_identity(google_profile(&uid(), &new_email(), true));
        let res = sign_in_with(&request, "google", Some("//evil.test"), None).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/");
    })
    .await;
}

#[tokio::test]
async fn verified_email_links_existing_account() {
    with_google(|request, ctx, fake| async move {
        let user = a_user().create(&ctx).await;
        let sub = uid();
        // Email dari provider beda huruf besar-kecil: tetap cocok setelah normalisasi.
        fake.set_identity(google_profile(&sub, &user.email.to_uppercase(), true));

        let res = sign_in_with(&request, "google", None, None).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/");

        let linked = identities_of(&ctx, user.id).await;
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].provider_uid, sub);
        // Password lokal tetap ada untuk akun yang sudah terverifikasi.
        assert!(reload(&ctx, &user).await.password_hash.is_some());
    })
    .await;
}

#[tokio::test]
async fn verified_email_verifies_unverified_account_and_drops_its_password() {
    with_google(|request, ctx, fake| async move {
        // Akun belum terverifikasi: password-nya bisa dibuat orang lain yang mendaftarkan
        // email ini lebih dulu (pre-hijacking), jadi tidak boleh ikut berlaku.
        let user = a_user().unverified().create(&ctx).await;
        fake.set_identity(google_profile(&uid(), &user.email, true));

        let res = sign_in_with(&request, "google", None, None).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/");

        let user = reload(&ctx, &user).await;
        assert!(user.email_verified_at.is_some());
        assert!(user.password_hash.is_none());
        assert_eq!(identities_of(&ctx, user.id).await.len(), 1);
        let login = request
            .post("/login")
            .form(&[("email", user.email.as_str()), ("password", TEST_PASSWORD)])
            .await;
        assert_eq!(login.status_code(), 422);
    })
    .await;
}

#[tokio::test]
async fn unverified_email_does_not_link_existing_account() {
    with_google(|request, ctx, fake| async move {
        let user = a_user().create(&ctx).await;
        fake.set_identity(google_profile(&uid(), &user.email, false));

        let res = sign_in_with(&request, "google", None, None).await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            res.header("location"),
            "/login?oauth_error=oauth-email-unverified"
        );
        assert!(session_cookie(&res).is_none_or(|c| c.value().is_empty()));
        assert!(identities_of(&ctx, user.id).await.is_empty());

        let page = request
            .get("/login?oauth_error=oauth-email-unverified")
            .await;
        assert!(
            page.text()
                .contains("belum diverifikasi oleh Google atau GitHub"),
            "{}",
            page.text()
        );

        // Tanpa klaim email_verified sama sekali: juga ditolak, juga untuk email baru.
        fake.set_identity(json!({ "sub": uid(), "email": new_email() }));
        let res = sign_in_with(&request, "google", None, None).await;
        assert_eq!(
            res.header("location"),
            "/login?oauth_error=oauth-email-unverified"
        );
    })
    .await;
}

#[tokio::test]
async fn second_identity_of_same_provider_is_not_linked() {
    with_google(|request, ctx, fake| async move {
        let user = a_user().create(&ctx).await;
        fake.set_identity(google_profile(&uid(), &user.email, true));
        assert_eq!(
            sign_in_with(&request, "google", None, None)
                .await
                .header("location"),
            "/"
        );

        // Akun Google lain mengklaim email yang sama (mis. alamat dipindahtangankan di
        // provider): tidak ditautkan ke user yang sudah punya identitas Google.
        fake.set_identity(google_profile(&uid(), &user.email, true));
        let res = sign_in_with(&request, "google", None, None).await;
        assert_eq!(
            res.header("location"),
            "/login?oauth_error=oauth-identity-conflict"
        );
        assert!(session_cookie(&res).is_none_or(|c| c.value().is_empty()));
        assert_eq!(identities_of(&ctx, user.id).await.len(), 1);
        let page = request
            .get("/login?oauth_error=oauth-identity-conflict")
            .await;
        assert!(
            page.text().contains("sudah tertaut ke akun lain"),
            "{}",
            page.text()
        );
    })
    .await;
}

#[tokio::test]
async fn registration_closed_rejects_unknown_email() {
    with_fake(
        &["google"],
        |s| s["auth"]["registration_open"] = false.into(),
        |request, ctx, fake| async move {
            let email = new_email();
            fake.set_identity(google_profile(&uid(), &email, true));
            let res = sign_in_with(&request, "google", None, None).await;
            assert_eq!(
                res.header("location"),
                "/login?oauth_error=oauth-registration-closed"
            );
            assert!(
                users::Model::find_by_email(&ctx.db, &email)
                    .await
                    .unwrap()
                    .is_none()
            );

            // Akun yang sudah ada tetap bisa ditautkan saat pendaftaran ditutup.
            let user = a_user().create(&ctx).await;
            fake.set_identity(google_profile(&uid(), &user.email, true));
            let res = sign_in_with(&request, "google", None, None).await;
            assert_eq!(res.header("location"), "/");
        },
    )
    .await;
}

#[tokio::test]
async fn disabled_user_is_rejected() {
    with_google(|request, ctx, fake| async move {
        let user = a_user().disabled().create(&ctx).await;
        fake.set_identity(google_profile(&uid(), &user.email, true));
        let res = sign_in_with(&request, "google", None, None).await;
        assert_eq!(
            res.header("location"),
            "/login?oauth_error=oauth-user-disabled"
        );
        assert!(identities_of(&ctx, user.id).await.is_empty());
    })
    .await;
}

#[tokio::test]
async fn oauth_user_with_totp_still_needs_second_step() {
    with_google(|request, ctx, fake| async move {
        let user = a_user().with_totp().create(&ctx).await;
        fake.set_identity(google_profile(&uid(), &user.email, true));

        let res = sign_in_with(&request, "google", Some("/account"), None).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/login/2fa");
        // Sesi pending, bukan sesi login.
        let pending = session_cookie(&res).unwrap();
        assert_eq!(
            request.get("/").add_cookie(pending).await.status_code(),
            303
        );
    })
    .await;
}

#[tokio::test]
async fn oauth_sign_in_renews_session() {
    with_google(|request, ctx, fake| async move {
        // Sesi sebelum login yang punya id di server: sesi pending 2FA user lain.
        let other = a_user().with_totp().create(&ctx).await;
        let pending = request
            .post("/login")
            .form(&[("email", other.email.as_str()), ("password", TEST_PASSWORD)])
            .await;
        let before = session_cookie(&pending).unwrap();

        fake.set_identity(google_profile(&uid(), &new_email(), true));
        let res = sign_in_with(&request, "google", None, Some(before.clone())).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let after = session_cookie(&res).unwrap();
        assert_ne!(after.value(), before.value());
        assert_eq!(request.get("/").add_cookie(before).await.status_code(), 303);
        assert_eq!(request.get("/").add_cookie(after).await.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn github_uses_primary_verified_email() {
    with_fake(
        &["github"],
        |_| {},
        |request, ctx, fake| async move {
            let email = new_email();
            // Email profil GitHub privat (null); `/user/emails` memberi alamat primer terverifikasi.
            fake.set_identity(json!({ "id": 4242, "login": "octo", "name": null, "email": null }));
            fake.set_emails(json!([
                { "email": "lain@example.test", "primary": false, "verified": true },
                { "email": email, "primary": true, "verified": true },
            ]));

            let res = sign_in_with(&request, "github", None, None).await;
            assert_eq!(res.status_code(), 303, "{}", res.text());
            assert_eq!(res.header("location"), "/");
            let user = users::Model::find_by_email(&ctx.db, &email)
                .await
                .unwrap()
                .expect("user dibuat");
            assert_eq!(user.name, "octo");
            let linked = identities_of(&ctx, user.id).await;
            assert_eq!(linked[0].provider, "github");
            assert_eq!(linked[0].provider_uid, "4242");

            // Email primer belum terverifikasi: ditolak.
            fake.set_identity(json!({ "id": 4343, "login": "octo2", "email": null }));
            fake.set_emails(json!([{ "email": new_email(), "primary": true, "verified": false }]));
            let res = sign_in_with(&request, "github", None, None).await;
            assert_eq!(
                res.header("location"),
                "/login?oauth_error=oauth-email-unverified"
            );
        },
    )
    .await;
}

#[tokio::test]
async fn oauth_buttons_hidden_when_provider_not_configured() {
    request::<App, _, _>(|request, _ctx| async move {
        let html = request.get("/login").await.text();
        assert_css_not_exists(&html, r#"form[action^="/auth/"]"#);
    })
    .await;

    with_google(|request, _ctx, _fake| async move {
        let html = request.get("/login?return_to=/account").await.text();
        assert_css_exists(
            &html,
            r#"form[action="/auth/google"][method="post"][hx-boost="false"]"#,
        );
        assert_css_exists(
            &html,
            r#"form[action="/auth/google"] input[type="hidden"][name="return_to"][value="/account"]"#,
        );
        assert_css_not_exists(&html, r#"form[action="/auth/github"]"#);
        assert_css_not_exists(&html, r#"a[href^="/auth/"]"#);
    })
    .await;
}

#[tokio::test]
async fn oauth_only_user_can_set_password_via_reset() {
    with_google(|request, ctx, fake| async move {
        let email = new_email();
        fake.set_identity(google_profile(&uid(), &email, true));
        sign_in_with(&request, "google", None, None).await;

        request
            .post("/password/forgot")
            .form(&[("email", email.as_str())])
            .await;
        let raw = token_in(&mail_to(&ctx, &email)[0], "/password/reset").unwrap();
        let res = request
            .post("/password/reset")
            .form(&[("token", raw.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        let login = request
            .post("/login")
            .form(&[("email", email.as_str()), ("password", NEW_PASSWORD)])
            .await;
        assert_eq!(login.status_code(), 303, "{}", login.text());
        assert_eq!(login.header("location"), "/");
    })
    .await;
}

fn identity(provider: &str, uid: &str, email: &str) -> Identity {
    Identity {
        provider: provider.to_string(),
        uid: uid.to_string(),
        login: None,
        name: Some("Paralel".to_string()),
        email: Some(email.to_string()),
        email_verified: Some(true),
        avatar_url: None,
        access_token: "fake-access-token".to_string(),
        refresh_token: None,
        expires_at: None,
        scope: None,
        raw: json!({}),
    }
}

#[tokio::test]
async fn concurrent_callbacks_create_one_user_and_identity() {
    request::<App, _, _>(|_request, ctx| async move {
        let email = new_email();
        let id = identity("google", &uid(), &email);
        let results =
            futures::future::join_all((0..4).map(|_| oauth::resolve_user(&ctx, &id))).await;
        let mut ids = Vec::new();
        for result in results {
            match result.unwrap() {
                OAuthOutcome::SignedIn(user) => ids.push(user.id),
                OAuthOutcome::Rejected(code) => panic!("ditolak: {code}"),
            }
        }
        ids.dedup();
        assert_eq!(ids.len(), 1, "{ids:?}");
        assert_eq!(identities_of(&ctx, ids[0]).await.len(), 1);

        // Tiga akun GitHub berbeda dengan email yang sama, bersamaan: hanya satu yang tertaut,
        // sisanya konflik (user sudah punya identitas GitHub dengan uid lain).
        let others: Vec<_> = (0..3).map(|_| identity("github", &uid(), &email)).collect();
        let results =
            futures::future::join_all(others.iter().map(|i| oauth::resolve_user(&ctx, i))).await;
        let (mut linked, mut conflicts) = (0, 0);
        for result in results {
            match result.unwrap() {
                OAuthOutcome::SignedIn(u) if u.id == ids[0] => linked += 1,
                OAuthOutcome::Rejected(oauth::IDENTITY_CONFLICT) => conflicts += 1,
                other => panic!("{other:?}"),
            }
        }
        assert_eq!((linked, conflicts), (1, 2));
        assert_eq!(identities_of(&ctx, ids[0]).await.len(), 2);
    })
    .await;
}
