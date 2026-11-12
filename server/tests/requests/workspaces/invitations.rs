//! Undangan email: undang dari Pengaturan → Anggota (`ManageMembers`), halaman publik
//! `/invitations/{token}` (GET tidak mengonsumsi token), terima (login, email harus sama), dan
//! daftar lewat undangan (email terkunci, langsung terverifikasi).

use agentgate_server::{
    app::App,
    common::{crypto, settings::Settings},
    models::{
        audit_events::{AuditAction, AuditSubject},
        invitations, job_titles,
        roles::RoleKey,
        users, workspace_members,
    },
};
use loco_rs::{TestServer, app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel};

use crate::support::{
    TEST_PASSWORD, a_user, a_workspace, add_member, builtin_role, invitation_token_in, mail_to,
    request_with_settings, sent_mail, session_cookie, sign_in_as,
};

const MEMBERS: &str = "/w/acme/settings/members";
const INVITE: &str = "/w/acme/settings/members/invitations";

/// Kirim undangan lewat form sebagai pemilik `cookie`, lalu kembalikan token dari email.
async fn invite(
    request: &TestServer,
    ctx: &AppContext,
    cookie: &loco_rs::auth::cookies::Cookie<'static>,
    email: &str,
    role_id: uuid::Uuid,
) -> String {
    let res = request
        .post(INVITE)
        .add_cookie(cookie.clone())
        .form(&[
            ("email", email),
            ("role_id", &role_id.to_string()),
            ("job_title_id", ""),
        ])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let mail = mail_to(ctx, &users::normalize_email(email));
    invitation_token_in(mail.last().expect("email undangan tidak terkirim")).unwrap()
}

async fn invitations_for(ctx: &AppContext, email: &str) -> Vec<invitations::Model> {
    invitations::Entity::find()
        .all(&ctx.db)
        .await
        .unwrap()
        .into_iter()
        .filter(|i| i.email == email)
        .collect()
}

#[tokio::test]
async fn owner_can_invite_and_email_is_sent() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().name("Olivia Pemilik").create(&ctx).await;
        let workspace = a_workspace(&owner)
            .name("Acme Raya")
            .slug("acme")
            .create(&ctx)
            .await;
        let wid = workspace.workspace_id();
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let title = job_titles::ActiveModel::create(&ctx.db, wid, "QA")
            .await
            .unwrap();
        let cookie = sign_in_as(&request, &owner).await;

        // Indeks Pengaturan menautkan halaman Anggota; halaman itu berisi form undang.
        let html = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            &format!(".app-side a[href=\"{MEMBERS}\"]"),
        );
        let page = request.get(MEMBERS).add_cookie(cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert_css_exists(
            &html,
            &format!("form[method=\"post\"][action=\"{INVITE}\"] input#invite-email"),
        );
        assert_css_exists(
            &html,
            &format!("select#invite-role option[value=\"{}\"]", member_role.id),
        );
        assert_css_exists(
            &html,
            &format!("select#invite-job-title option[value=\"{}\"]", title.id),
        );

        let res = request
            .post(INVITE)
            .add_cookie(cookie.clone())
            .form(&[
                ("email", " Budi@Example.TEST "),
                ("role_id", &member_role.id.to_string()),
                ("job_title_id", &title.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), MEMBERS);

        let found = invitations_for(&ctx, "budi@example.test").await;
        let [invitation] = found.as_slice() else {
            panic!("harus tepat satu undangan");
        };
        assert_eq!(invitation.workspace_id, wid.get());
        assert_eq!(invitation.role_id, member_role.id);
        assert_eq!(invitation.job_title_id, Some(title.id));
        assert_eq!(invitation.invited_by, Some(owner.id));
        assert!(invitation.accepted_at.is_none() && invitation.revoked_at.is_none());
        let days = Settings::from_ctx(&ctx).unwrap().auth.invitation_days;
        let ttl = invitation.expires_at.to_utc() - chrono::Utc::now();
        assert!(ttl > chrono::Duration::days(days) - chrono::Duration::minutes(5));
        assert!(ttl <= chrono::Duration::days(days));

        // Email: link publik dari public_base_url, nama workspace, pengundang, dan role.
        let mail = mail_to(&ctx, "budi@example.test");
        assert_eq!(mail.len(), 1);
        let message = &mail[0];
        let token = invitation_token_in(message).unwrap();
        assert_eq!(invitation.token_digest, crypto::sha256_hex(&token));
        let base = Settings::from_ctx(&ctx).unwrap().public_base_url;
        let link = format!("{base}/invitations/{token}");
        assert!(message.matches(&link).count() >= 2, "{message}");
        assert!(message.contains("text/plain") && message.contains("text/html"));
        for text in ["Acme Raya", "Olivia Pemilik", "Member"] {
            assert!(message.contains(text), "{text}\n{message}");
        }

        // Audit tanpa token.
        let event = &crate::support::newest_audit_events(&ctx.db, wid, 1)
            .await
            .unwrap()[0];
        assert_eq!(event.action().unwrap(), AuditAction::MemberInvited);
        assert_eq!(event.subject().unwrap(), AuditSubject::Invitation);
        assert_eq!(event.subject_id, invitation.id);
        assert_eq!(event.actor_id, Some(owner.id));
        assert_eq!(event.changes["email"], "budi@example.test");
        assert!(!event.changes.to_string().contains(&token));

        // Undangan tertunda tampil dengan tombol kirim ulang dan batalkan.
        let html = request.get(MEMBERS).add_cookie(cookie.clone()).await.text();
        assert_css_exists(&html, "[data-invitation]");
        assert!(html.contains("budi@example.test"));
        assert_css_exists(
            &html,
            &format!(
                "form[action=\"{INVITE}/{}/resend\"] button[type=\"submit\"]",
                invitation.id
            ),
        );
        assert_css_exists(
            &html,
            &format!(
                "form[action=\"{INVITE}/{}/revoke\"] button[type=\"submit\"]",
                invitation.id
            ),
        );
    })
    .await;
}

#[tokio::test]
async fn invite_existing_member_gets_422() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let existing = a_user().email("ada@example.test").create(&ctx).await;
        add_member(&ctx, &workspace, &existing, RoleKey::Viewer).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &owner).await;

        for email in ["ADA@example.test", "not-an-email"] {
            let res = request
                .post(INVITE)
                .add_cookie(cookie.clone())
                .form(&[
                    ("email", email),
                    ("role_id", &member_role.id.to_string()),
                    ("job_title_id", ""),
                ])
                .await;
            assert_eq!(res.status_code(), 422, "{email}");
            let html = res.text();
            assert_css_exists(&html, "#invite-email-error");
            assert_css_exists(&html, &format!("input#invite-email[value=\"{email}\"]"));
        }
        let html = request
            .post(INVITE)
            .add_cookie(cookie.clone())
            .form(&[
                ("email", "ada@example.test"),
                ("role_id", &member_role.id.to_string()),
                ("job_title_id", ""),
            ])
            .await
            .text();
        assert!(html.contains("sudah menjadi anggota"), "{html}");
        assert!(
            invitations::Entity::find()
                .all(&ctx.db)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(sent_mail(&ctx).is_empty());
    })
    .await;
}

#[tokio::test]
async fn reinvite_revokes_previous_token() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &owner).await;

        let first = invite(&request, &ctx, &cookie, "cici@example.test", member_role.id).await;
        let second = invite(&request, &ctx, &cookie, "Cici@example.test", member_role.id).await;
        assert_ne!(first, second);
        let all = invitations_for(&ctx, "cici@example.test").await;
        assert_eq!(all.len(), 2);
        assert_eq!(all.iter().filter(|i| i.revoked_at.is_none()).count(), 1);
        assert_eq!(
            request
                .get(&format!("/invitations/{first}"))
                .await
                .status_code(),
            404
        );
        assert_eq!(
            request
                .get(&format!("/invitations/{second}"))
                .await
                .status_code(),
            200
        );

        // Kirim ulang: token baru, token lama dibatalkan.
        let active = all.iter().find(|i| i.revoked_at.is_none()).unwrap();
        let res = request
            .post(&format!("{INVITE}/{}/resend", active.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), MEMBERS);
        let third =
            invitation_token_in(mail_to(&ctx, "cici@example.test").last().unwrap()).unwrap();
        assert_ne!(third, second);
        assert_eq!(
            request
                .get(&format!("/invitations/{second}"))
                .await
                .status_code(),
            404
        );
        assert_eq!(
            request
                .get(&format!("/invitations/{third}"))
                .await
                .status_code(),
            200
        );

        // Batalkan: tidak ada undangan aktif tersisa.
        let active = invitations_for(&ctx, "cici@example.test")
            .await
            .into_iter()
            .find(|i| i.revoked_at.is_none())
            .unwrap();
        let res = request
            .post(&format!("{INVITE}/{}/revoke", active.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            request
                .get(&format!("/invitations/{third}"))
                .await
                .status_code(),
            404
        );
        assert!(
            invitations_for(&ctx, "cici@example.test")
                .await
                .iter()
                .all(|i| i.revoked_at.is_some())
        );
        // Undangan yang sudah dibatalkan tidak bisa dikirim ulang atau dibatalkan lagi.
        let res = request
            .post(&format!("{INVITE}/{}/resend", active.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn invite_link_get_does_not_accept() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().name("Olivia Pemilik").create(&ctx).await;
        let workspace = a_workspace(&owner)
            .name("Acme Raya")
            .slug("acme")
            .create(&ctx)
            .await;
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &owner).await;
        let token = invite(&request, &ctx, &cookie, "dodi@example.test", admin_role.id).await;
        let path = format!("/invitations/{token}");

        // Belum login: nama workspace, pengundang, role, tombol Daftar dan Masuk.
        let res = request.get(&path).await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.header("referrer-policy"), "no-referrer");
        let html = res.text();
        assert_css_exists(&html, "[data-invitation-page]");
        for text in ["Acme Raya", "Olivia Pemilik", "Admin"] {
            assert!(html.contains(text), "{text}");
        }
        assert_css_exists(&html, &format!("a[href=\"{path}/register\"]"));
        assert_css_exists(
            &html,
            &format!("a[href=\"/login?return_to=%2Finvitations%2F{token}\"]"),
        );
        assert_css_not_exists(&html, &format!("form[action=\"{path}/accept\"]"));

        // Sudah login (email sama): tombol terima, tetapi GET tetap tidak menerima.
        let dodi = a_user().email("dodi@example.test").create(&ctx).await;
        let dodi_cookie = sign_in_as(&request, &dodi).await;
        let html = request
            .get(&path)
            .add_cookie(dodi_cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            &format!("form[method=\"post\"][action=\"{path}/accept\"] button[type=\"submit\"]"),
        );
        let found = invitations_for(&ctx, "dodi@example.test").await;
        let [invitation] = found.as_slice() else {
            panic!("harus tepat satu undangan");
        };
        assert!(invitation.accepted_at.is_none());
        assert!(
            workspace_members::Model::find_with_role(&ctx.db, workspace.workspace_id(), dodi.id)
                .await
                .unwrap()
                .is_none()
        );

        // Halaman login dengan return_to membawa token di URL: no-referrer, juga saat 422.
        let login = format!("/login?return_to=%2Finvitations%2F{token}");
        let res = request.get(&login).await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.header("referrer-policy"), "no-referrer");
        let res = request
            .post("/login")
            .form(&[
                ("email", "dodi@example.test"),
                ("password", "password yang salah sekali"),
                ("return_to", path.as_str()),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_eq!(res.header("referrer-policy"), "no-referrer");

        // Masuk lewat link undangan: login kembali ke halaman undangan.
        let res = request
            .post("/login")
            .form(&[
                ("email", "dodi@example.test"),
                ("password", TEST_PASSWORD),
                ("return_to", path.as_str()),
            ])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), path);
    })
    .await;
}

#[tokio::test]
async fn invite_accept_requires_matching_email() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let title = job_titles::ActiveModel::create(&ctx.db, wid, "Dev")
            .await
            .unwrap();
        let owner_cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post(INVITE)
            .add_cookie(owner_cookie.clone())
            .form(&[
                ("email", "Eka@Example.test"),
                ("role_id", &member_role.id.to_string()),
                ("job_title_id", &title.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303);
        let token = invitation_token_in(&mail_to(&ctx, "eka@example.test")[0]).unwrap();
        let accept = format!("/invitations/{token}/accept");

        // User lain → 422, tidak bergabung, token tidak terpakai.
        let other = a_user().email("lain@example.test").create(&ctx).await;
        let other_cookie = sign_in_as(&request, &other).await;
        let res = request.post(&accept).add_cookie(other_cookie.clone()).await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("bukan untuk akun"), "{}", res.text());
        assert!(
            workspace_members::Model::find_with_role(&ctx.db, wid, other.id)
                .await
                .unwrap()
                .is_none()
        );

        // Belum login → ke login.
        let res = request.post(&accept).await;
        assert_eq!(res.status_code(), 303);
        assert!(
            res.header("location")
                .to_str()
                .unwrap()
                .starts_with("/login")
        );

        // Email sama (tanpa membedakan huruf besar) → bergabung dengan role dan jabatan undangan.
        let eka = a_user().email("EKA@example.test").create(&ctx).await;
        let eka_cookie = sign_in_as(&request, &eka).await;
        let res = request.post(&accept).add_cookie(eka_cookie.clone()).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme");
        let (member, role) = workspace_members::Model::find_with_role(&ctx.db, wid, eka.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(role.id, member_role.id);
        assert_eq!(member.job_title_id, Some(title.id));
        let invitation = &invitations_for(&ctx, "eka@example.test").await[0];
        assert!(invitation.accepted_at.is_some());
        let event = &crate::support::newest_audit_events(&ctx.db, wid, 1)
            .await
            .unwrap()[0];
        assert_eq!(event.action().unwrap(), AuditAction::MemberJoined);
        assert_eq!(event.subject().unwrap(), AuditSubject::Member);
        assert_eq!(event.subject_id, member.id);
        assert_eq!(event.actor_id, Some(eka.id));
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(eka_cookie.clone())
                .await
                .status_code(),
            200
        );

        // Token sekali pakai.
        let res = request.post(&accept).add_cookie(eka_cookie.clone()).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn accepting_when_already_member_just_redirects() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let owner_cookie = sign_in_as(&request, &owner).await;
        let token = invite(
            &request,
            &ctx,
            &owner_cookie,
            "fajar@example.test",
            member_role.id,
        )
        .await;

        // Bergabung lewat jalan lain setelah diundang (mis. ditambahkan langsung).
        let fajar = a_user().email("fajar@example.test").create(&ctx).await;
        add_member(&ctx, &workspace, &fajar, RoleKey::Viewer).await;
        let cookie = sign_in_as(&request, &fajar).await;
        let res = request
            .post(&format!("/invitations/{token}/accept"))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme");
        // Role yang ada tidak berubah; undangan selesai.
        let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, fajar.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(role.role_key().unwrap(), Some(RoleKey::Viewer));
        assert!(
            invitations_for(&ctx, "fajar@example.test").await[0]
                .accepted_at
                .is_some()
        );
    })
    .await;
}

#[tokio::test]
async fn register_via_invite_marks_email_verified_and_joins() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let owner_cookie = sign_in_as(&request, &owner).await;
        let token = invite(
            &request,
            &ctx,
            &owner_cookie,
            "gita@example.test",
            member_role.id,
        )
        .await;
        let register = format!("/invitations/{token}/register");

        // Form: email terkunci (hanya tampilan), nama dan password.
        let res = request.get(&register).await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.header("referrer-policy"), "no-referrer");
        let html = res.text();
        assert!(html.contains("gita@example.test"));
        assert_css_not_exists(&html, "input[name=\"email\"]");
        assert_css_exists(
            &html,
            &format!("form[method=\"post\"][action=\"{register}\"] input[name=\"name\"]"),
        );

        // Password terlalu pendek → 422, nama terisi kembali, belum ada user.
        let res = request
            .post(&register)
            .form(&[("name", "Gita"), ("password", "pendek")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "input[name=\"name\"][value=\"Gita\"]");
        assert!(
            users::Model::find_by_email(&ctx.db, "gita@example.test")
                .await
                .unwrap()
                .is_none()
        );

        // Email di body diabaikan: server memakai email undangan.
        let res = request
            .post(&register)
            .form(&[
                ("name", "Gita Undangan"),
                ("email", "lain@example.test"),
                ("password", TEST_PASSWORD),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme");
        let cookie = session_cookie(&res).expect("tidak login setelah daftar");
        let gita = users::Model::find_by_email(&ctx.db, "gita@example.test")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(gita.name, "Gita Undangan");
        assert!(gita.email_verified_at.is_some());
        assert!(
            users::Model::find_by_email(&ctx.db, "lain@example.test")
                .await
                .unwrap()
                .is_none()
        );
        let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, gita.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(role.id, member_role.id);
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(cookie)
                .await
                .status_code(),
            200
        );
        // Token sudah dipakai.
        assert_eq!(request.get(&register).await.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn register_via_invite_for_existing_account_offers_login() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let hana = a_user().email("hana@example.test").create(&ctx).await;
        let owner_cookie = sign_in_as(&request, &owner).await;
        let token = invite(
            &request,
            &ctx,
            &owner_cookie,
            "hana@example.test",
            member_role.id,
        )
        .await;
        let register = format!("/invitations/{token}/register");

        let html = request.get(&register).await.text();
        assert_css_not_exists(&html, "input[name=\"password\"]");
        assert_css_exists(
            &html,
            &format!("a[href=\"/login?return_to=%2Finvitations%2F{token}\"]"),
        );
        // POST langsung tidak membuat akun atau mengubah password.
        let res = request
            .post(&register)
            .form(&[("name", "Penyusup"), ("password", "password lain sekali")])
            .await;
        assert_eq!(res.status_code(), 422);
        let unchanged = users::Model::find_by_id(&ctx.db, hana.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.password_hash, hana.password_hash);
        assert_eq!(unchanged.name, hana.name);
    })
    .await;
}

#[tokio::test]
async fn expired_invite_shows_invalid_page() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let owner_cookie = sign_in_as(&request, &owner).await;
        let token = invite(
            &request,
            &ctx,
            &owner_cookie,
            "indah@example.test",
            member_role.id,
        )
        .await;
        let invitation = invitations_for(&ctx, "indah@example.test").await.remove(0);
        let mut active = invitation.into_active_model();
        active.expires_at = Set((chrono::Utc::now() - chrono::Duration::minutes(1)).into());
        active.update(&ctx.db).await.unwrap();
        let indah = a_user().email("indah@example.test").create(&ctx).await;
        let cookie = sign_in_as(&request, &indah).await;

        let invalid_page = |res: &axum_test::TestResponse| {
            assert_eq!(res.status_code(), 404);
            assert_eq!(res.header("referrer-policy"), "no-referrer");
            assert_css_exists(&res.text(), "[data-page=\"invitation-invalid\"]");
        };
        invalid_page(&request.get(&format!("/invitations/{token}")).await);
        invalid_page(&request.get(&format!("/invitations/{token}/register")).await);
        invalid_page(
            &request
                .post(&format!("/invitations/{token}/accept"))
                .add_cookie(cookie.clone())
                .await,
        );
        invalid_page(
            &request
                .post(&format!("/invitations/{token}/register"))
                .form(&[("name", "Indah"), ("password", TEST_PASSWORD)])
                .await,
        );
        // Token yang tidak pernah ada: halaman yang sama.
        invalid_page(&request.get("/invitations/tidak-ada").await);
        assert!(
            workspace_members::Model::find_with_role(&ctx.db, workspace.workspace_id(), indah.id)
                .await
                .unwrap()
                .is_none()
        );
    })
    .await;
}

#[tokio::test]
async fn member_cannot_invite_gets_404() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let owner_cookie = sign_in_as(&request, &owner).await;
        invite(
            &request,
            &ctx,
            &owner_cookie,
            "joko@example.test",
            member_role.id,
        )
        .await;
        let pending = invitations_for(&ctx, "joko@example.test").await.remove(0);

        for key in [RoleKey::Member, RoleKey::Viewer] {
            let user = a_user().create(&ctx).await;
            add_member(&ctx, &workspace, &user, key).await;
            let cookie = sign_in_as(&request, &user).await;
            assert_eq!(
                request
                    .get(MEMBERS)
                    .add_cookie(cookie.clone())
                    .await
                    .status_code(),
                404
            );
            let res = request
                .post(INVITE)
                .add_cookie(cookie.clone())
                .form(&[
                    ("email", "baru@example.test"),
                    ("role_id", &member_role.id.to_string()),
                    ("job_title_id", ""),
                ])
                .await;
            assert_eq!(res.status_code(), 404, "{key:?}");
            // Body tidak valid pun 404: izin dicek sebelum body dibaca.
            let res = request
                .post(INVITE)
                .add_cookie(cookie.clone())
                .form(&[("x", "y")])
                .await;
            assert_eq!(res.status_code(), 404);
            for action in ["resend", "revoke"] {
                let res = request
                    .post(&format!("{INVITE}/{}/{action}", pending.id))
                    .add_cookie(cookie.clone())
                    .await;
                assert_eq!(res.status_code(), 404, "{action}");
            }
        }
        assert!(invitations_for(&ctx, "baru@example.test").await.is_empty());
        let pending = invitations::Entity::find_by_id(pending.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert!(pending.revoked_at.is_none());
    })
    .await;
}

#[tokio::test]
async fn admin_cannot_invite_with_a_role_above_their_own() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        let owner_role = builtin_role(&ctx, &workspace, RoleKey::Owner).await;
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &admin).await;

        // Pilihan role Owner tidak tersedia untuk Admin.
        let html = request.get(MEMBERS).add_cookie(cookie.clone()).await.text();
        assert_css_not_exists(
            &html,
            &format!("select#invite-role option[value=\"{}\"]", owner_role.id),
        );
        let res = request
            .post(INVITE)
            .add_cookie(cookie.clone())
            .form(&[
                ("email", "kiki@example.test"),
                ("role_id", &owner_role.id.to_string()),
                ("job_title_id", ""),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#invite-role-error");
        assert!(invitations_for(&ctx, "kiki@example.test").await.is_empty());

        // Role yang izinnya dipegang Admin sendiri boleh.
        invite(&request, &ctx, &cookie, "kiki@example.test", admin_role.id).await;

        // Role id dari workspace lain → 404.
        let other = a_workspace(&owner).create(&ctx).await;
        let foreign = builtin_role(&ctx, &other, RoleKey::Member).await;
        let res = request
            .post(INVITE)
            .add_cookie(cookie.clone())
            .form(&[
                ("email", "lia@example.test"),
                ("role_id", &foreign.id.to_string()),
                ("job_title_id", ""),
            ])
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn admin_cannot_touch_owner_invitation() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        let owner_role = builtin_role(&ctx, &workspace, RoleKey::Owner).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let owner_cookie = sign_in_as(&request, &owner).await;
        let token = invite(
            &request,
            &ctx,
            &owner_cookie,
            "oki@example.test",
            owner_role.id,
        )
        .await;
        let pending = invitations_for(&ctx, "oki@example.test").await.remove(0);
        let cookie = sign_in_as(&request, &admin).await;

        // Baris undangan Owner tanpa tombol kirim ulang/batalkan untuk Admin.
        let html = request.get(MEMBERS).add_cookie(cookie.clone()).await.text();
        assert!(html.contains("oki@example.test"));
        assert_css_not_exists(
            &html,
            &format!("form[action=\"{INVITE}/{}/resend\"]", pending.id),
        );
        assert_css_not_exists(
            &html,
            &format!("form[action=\"{INVITE}/{}/revoke\"]", pending.id),
        );

        for action in ["resend", "revoke"] {
            let res = request
                .post(&format!("{INVITE}/{}/{action}", pending.id))
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 422, "{action}");
            assert!(res.text().contains("tidak bisa memberikan"), "{action}");
        }
        // Undang ulang email yang sama sebagai Member tidak membatalkan undangan Owner.
        let res = request
            .post(INVITE)
            .add_cookie(cookie.clone())
            .form(&[
                ("email", "oki@example.test"),
                ("role_id", &member_role.id.to_string()),
                ("job_title_id", ""),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#invite-email-error");
        let all = invitations_for(&ctx, "oki@example.test").await;
        assert_eq!(all.len(), 1);
        assert!(all[0].revoked_at.is_none());
        assert_eq!(
            request
                .get(&format!("/invitations/{token}"))
                .await
                .status_code(),
            200
        );
        assert_eq!(mail_to(&ctx, "oki@example.test").len(), 1);
    })
    .await;
}

/// Undangan adalah jalan masuk yang disengaja: tetap bisa daftar walau pendaftaran publik
/// ditutup.
#[tokio::test]
async fn register_via_invite_works_when_registration_is_closed() {
    request_with_settings(
        |s| s["auth"]["registration_open"] = serde_json::Value::Bool(false),
        |request, ctx| async move {
            assert_eq!(request.get("/register").await.status_code(), 404);
            let owner = a_user().create(&ctx).await;
            let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
            let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
            let owner_cookie = sign_in_as(&request, &owner).await;
            let token = invite(
                &request,
                &ctx,
                &owner_cookie,
                "tutup@example.test",
                member_role.id,
            )
            .await;
            let register = format!("/invitations/{token}/register");
            assert_eq!(request.get(&register).await.status_code(), 200);
            let res = request
                .post(&register)
                .form(&[("name", "Tutup"), ("password", TEST_PASSWORD)])
                .await;
            assert_eq!(res.status_code(), 303, "{}", res.text());
            assert_eq!(res.header("location"), "/w/acme");
            let user = users::Model::find_by_email(&ctx.db, "tutup@example.test")
                .await
                .unwrap()
                .unwrap();
            assert!(
                workspace_members::Model::find_with_role(
                    &ctx.db,
                    workspace.workspace_id(),
                    user.id
                )
                .await
                .unwrap()
                .is_some()
            );
        },
    )
    .await;
}

/// Role custom dengan semua izin Admin tidak bisa mengundang sebagai role bawaan Admin (Admin
/// di semua project hanya diberikan Owner atau Admin).
#[tokio::test]
async fn custom_role_with_admin_permissions_cannot_invite_as_admin() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let admin_permissions = agentgate_server::models::permission::BUILTIN_ROLES
            .iter()
            .find(|r| r.key == RoleKey::Admin)
            .unwrap()
            .permissions
            .to_vec();
        let co_admin = agentgate_server::models::roles::ActiveModel::create_custom(
            &ctx.db,
            wid,
            "Wakil",
            admin_permissions,
        )
        .await
        .unwrap();
        let wakil = a_user().create(&ctx).await;
        workspace_members::ActiveModel::add(&ctx.db, wid, wakil.id, co_admin.id)
            .await
            .unwrap();
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &wakil).await;

        let html = request.get(MEMBERS).add_cookie(cookie.clone()).await.text();
        assert_css_not_exists(
            &html,
            &format!("select#invite-role option[value=\"{}\"]", admin_role.id),
        );
        let res = request
            .post(INVITE)
            .add_cookie(cookie.clone())
            .form(&[
                ("email", "kiki@example.test"),
                ("role_id", &admin_role.id.to_string()),
                ("job_title_id", ""),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#invite-role-error");
        assert!(invitations_for(&ctx, "kiki@example.test").await.is_empty());

        // Role custom-nya sendiri tetap boleh diberikan.
        invite(&request, &ctx, &cookie, "kiki@example.test", co_admin.id).await;
    })
    .await;
}
