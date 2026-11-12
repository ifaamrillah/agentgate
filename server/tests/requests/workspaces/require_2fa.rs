//! `POST /w/{slug}/settings/require-2fa`: workspace mewajibkan 2FA (role bawaan Owner). Redirect
//! anggota tanpa 2FA ke setup dilakukan extractor `Tenant` (lihat juga `tenant_isolation`).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        permission::Permission,
        project_grants::GrantSubject,
        roles::RoleKey,
        users, workspace_members, workspaces,
    },
    services::workspaces::{
        Acting,
        require_two_factor::{self, RequireTwoFactorError},
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, IntoActiveModel};

use crate::support::{a_user, a_workspace, add_member, builtin_role, custom_role, sign_in_as};

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
            .get("/w/acme/settings/security")
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
        assert_eq!(res.header("location"), "/w/acme/settings/security");
        assert!(reload(&ctx).await.require_2fa_at.is_some());
        let html = request
            .get("/w/acme/settings/security")
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

        let events = crate::support::newest_audit_events(&ctx.db, workspace.workspace_id(), 10)
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

        // Berlaku di setiap halaman workspace, termasuk POST (kecuali keluar, lihat
        // `member_without_totp_can_still_leave`).
        for path in ["/w/acme", "/w/acme/settings/members"] {
            let res = request.get(path).add_cookie(member_cookie.clone()).await;
            assert_eq!(res.status_code(), 303, "{path}");
            assert_eq!(res.header("location"), "/account/2fa?required=acme");
        }
        let res = request
            .post("/w/acme/settings/roles/new")
            .add_cookie(member_cookie.clone())
            .form(&[("name", "X")])
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

/// Service memeriksa ulang role bawaan Owner di bawah kunci: pemegang `ManageWorkspace` lewat
/// role custom (yang lolos guard lama) tetap ditolak seperti tanpa izin.
#[tokio::test]
async fn service_rejects_non_owner_with_manage_workspace() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let manager = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let role = custom_role(ctx, &workspace, "Pengelola", &[Permission::ManageWorkspace]).await;
    workspace_members::ActiveModel::add(&ctx.db, workspace.workspace_id(), manager.id, role.id)
        .await
        .unwrap();
    enable_totp(ctx, &manager).await;
    let manager = users::Model::find_by_id(&ctx.db, manager.id)
        .await
        .unwrap()
        .unwrap();
    let acting = Acting {
        workspace: &workspace,
        user: &manager,
        role: &role,
        request_id: None,
    };
    let res = require_two_factor::set(&ctx.db, &acting, true).await;
    assert!(
        matches!(res, Err(RequireTwoFactorError::NotFound)),
        "{res:?}"
    );
}

/// Workspace dihapus permanen oleh job pembersihan setelah `Tenant` memuatnya: 404, bukan 500.
#[tokio::test]
async fn toggle_after_purge_is_not_found() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let owner_role = builtin_role(ctx, &workspace, RoleKey::Owner).await;
    ctx.db
        .execute_unprepared(&format!(
            "DELETE FROM workspaces WHERE id = '{}'",
            workspace.id
        ))
        .await
        .unwrap();
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &owner_role,
        request_id: None,
    };
    let res = require_two_factor::set(&ctx.db, &acting, false).await;
    assert!(
        matches!(res, Err(RequireTwoFactorError::NotFound)),
        "{res:?}"
    );
}

/// Anggota tanpa 2FA tidak dipaksa setup 2FA hanya untuk keluar dari workspace yang
/// mewajibkannya; bukan anggota tetap 404.
#[tokio::test]
async fn member_without_totp_can_still_leave() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let outsider = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let project = crate::support::a_project(&ctx, &workspace, "RHS").await;
        let viewer = builtin_role(&ctx, &workspace, RoleKey::Viewer).await;
        crate::support::grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::User(member.id),
            &viewer,
        )
        .await;
        let mut active = workspace.clone().into_active_model();
        active.require_2fa_at = Set(Some(chrono::Utc::now().into()));
        active.update(&ctx.db).await.unwrap();

        let outsider_cookie = sign_in_as(&request, &outsider).await;
        let res = request
            .get("/w/acme/leave")
            .add_cookie(outsider_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post("/w/acme/leave")
            .add_cookie(outsider_cookie)
            .await;
        assert_eq!(res.status_code(), 404);

        let cookie = sign_in_as(&request, &member).await;
        let page = request
            .get("/w/acme/leave")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200, "{}", page.text());
        assert_css_exists(
            &page.text(),
            "form[method=\"post\"][action=\"/w/acme/leave\"]",
        );
        // Shell tanpa konteks workspace: belum memenuhi 2FA = tidak melihat isi workspace.
        assert!(
            !page.text().contains("/w/acme/p/RHS"),
            "halaman keluar tidak boleh memuat navigasi workspace"
        );
        let res = request
            .post("/w/acme/leave")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/");
        assert!(
            workspace_members::Model::find_with_role(&ctx.db, workspace.workspace_id(), member.id)
                .await
                .unwrap()
                .is_none()
        );
    })
    .await;
}
