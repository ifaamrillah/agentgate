//! `POST /w/{slug}/settings/require-2fa`: workspace mewajibkan 2FA (`ManageWorkspace`). Redirect
//! anggota tanpa 2FA ke setup dilakukan extractor `Tenant` (lihat juga `tenant_isolation`).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{self, AuditAction, AuditSubject},
        roles::RoleKey,
        users, workspaces,
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};

use crate::support::{a_user, a_workspace, add_member, sign_in_as};

const TOGGLE: &str = "/w/acme/settings/require-2fa";

/// Aktifkan 2FA `user` langsung di database, setelah login (sesi tetap berlaku).
async fn enable_totp(ctx: &AppContext, user: &users::Model) {
    let mut active = user.clone().into_active_model();
    active.totp_enabled_at = Set(Some(chrono::Utc::now().into()));
    active.update(&ctx.db).await.unwrap();
}

async fn reload(ctx: &AppContext) -> workspaces::Model {
    workspaces::Model::find_by_slug(&ctx.db, "acme")
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn owner_toggles_require_2fa_only_with_own_2fa() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let cookie = sign_in_as(&request, &owner).await;

        let html = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "[data-settings-danger] form[method=\"post\"][action=\"/w/acme/settings/require-2fa\"] input[name=\"enabled\"][value=\"true\"]",
        );

        // Pelaku belum punya 2FA: mewajibkan 2FA akan mengunci dirinya sendiri.
        let res = request
            .post(TOGGLE)
            .add_cookie(cookie.clone())
            .form(&[("enabled", "true")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "[data-flash=\"error\"]");
        assert!(res.text().contains("aktifkan 2FA"), "{}", res.text());
        assert!(reload(&ctx).await.require_2fa_at.is_none());

        enable_totp(&ctx, &owner).await;
        let res = request
            .post(TOGGLE)
            .add_cookie(cookie.clone())
            .form(&[("enabled", "true")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/settings");
        assert!(reload(&ctx).await.require_2fa_at.is_some());
        let html = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "form[action=\"/w/acme/settings/require-2fa\"] input[name=\"enabled\"][value=\"false\"]",
        );

        // Mengirim nilai yang sama lagi tidak menulis audit baru.
        request
            .post(TOGGLE)
            .add_cookie(cookie.clone())
            .form(&[("enabled", "true")])
            .await;
        let res = request
            .post(TOGGLE)
            .add_cookie(cookie.clone())
            .form(&[("enabled", "false")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(reload(&ctx).await.require_2fa_at.is_none());

        let events = audit_events::Model::newest_first(&ctx.db, workspace.workspace_id(), 10)
            .await
            .unwrap();
        let toggles: Vec<_> = events
            .iter()
            .filter(|e| e.action().unwrap() == AuditAction::WorkspaceRequire2faChanged)
            .collect();
        assert_eq!(toggles.len(), 2);
        assert_eq!(toggles[0].subject().unwrap(), AuditSubject::Workspace);
        assert_eq!(toggles[0].actor_id, Some(owner.id));
        assert_eq!(
            toggles[0].changes,
            serde_json::json!({ "enabled": { "from": true, "to": false } })
        );
        assert_eq!(
            toggles[1].changes,
            serde_json::json!({ "enabled": { "from": false, "to": true } })
        );
    })
    .await;
}

#[tokio::test]
async fn admin_cannot_toggle_require_2fa_gets_404() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &admin).await;
        enable_totp(&ctx, &admin).await;
        let html = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_not_exists(&html, "[data-settings-danger]");
        let res = request
            .post(TOGGLE)
            .add_cookie(cookie)
            .form(&[("enabled", "true")])
            .await;
        assert_eq!(res.status_code(), 404);
        assert!(reload(&ctx).await.require_2fa_at.is_none());
    })
    .await;
}

#[tokio::test]
async fn require_2fa_redirects_member_without_totp_to_setup() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let owner_cookie = sign_in_as(&request, &owner).await;
        let member_cookie = sign_in_as(&request, &member).await;
        enable_totp(&ctx, &owner).await;
        let res = request
            .post(TOGGLE)
            .add_cookie(owner_cookie.clone())
            .form(&[("enabled", "true")])
            .await;
        assert_eq!(res.status_code(), 303);

        // Berlaku di setiap halaman workspace, termasuk POST.
        for path in ["/w/acme", "/w/acme/leave"] {
            let res = request.get(path).add_cookie(member_cookie.clone()).await;
            assert_eq!(res.status_code(), 303, "{path}");
            assert_eq!(res.header("location"), "/account/2fa?required=acme");
        }
        let res = request
            .post("/w/acme/leave")
            .add_cookie(member_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), "/account/2fa?required=acme");
        // Owner yang sudah ber-2FA tidak terpengaruh.
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(owner_cookie)
                .await
                .status_code(),
            200
        );
    })
    .await;
}

#[tokio::test]
async fn member_with_totp_unaffected_by_require_2fa() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let mut active = workspace.into_active_model();
        active.require_2fa_at = Set(Some(chrono::Utc::now().into()));
        active.update(&ctx.db).await.unwrap();

        let cookie = sign_in_as(&request, &member).await;
        enable_totp(&ctx, &member).await;
        let res = request.get("/w/acme").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
    })
    .await;
}
