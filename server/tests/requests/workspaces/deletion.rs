//! `POST /w/{slug}/settings/delete` dan `/delete/cancel`: hapus workspace dengan masa tenggang
//! 30 hari (hanya Owner bawaan). Penghapusan permanen dilakukan job pembersihan
//! (`tests/workers/cleanup.rs`).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        permission::Permission,
        roles::RoleKey,
        workspace_members, workspaces,
    },
    services::workspaces::{
        Acting,
        deletion::{self, DeletionError},
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::ConnectionTrait;

use crate::support::{a_user, a_workspace, add_member, builtin_role, custom_role, sign_in_as};

const DELETE: &str = "/w/acme/settings/delete";
const CANCEL: &str = "/w/acme/settings/delete/cancel";

async fn reload(ctx: &AppContext) -> workspaces::Model {
    workspaces::Model::find_by_slug(&ctx.db, "acme")
        .await
        .unwrap()
        .unwrap()
}

async fn actions(ctx: &AppContext, workspace: &workspaces::Model) -> Vec<AuditAction> {
    crate::support::newest_audit_events(&ctx.db, workspace.workspace_id(), 20)
        .await
        .unwrap()
        .iter()
        .map(|e| e.action().unwrap())
        .collect()
}

#[tokio::test]
async fn owner_schedules_deletion_after_typing_slug() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &owner).await;

        let html = request
            .get("/w/acme/settings/danger")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "[data-settings-danger] form[method=\"post\"][action=\"/w/acme/settings/delete\"] input[name=\"confirm_slug\"]",
        );
        assert_css_not_exists(&html, "[data-deletion-banner]");

        // Harus persis slug-nya (spasi di tepi diabaikan); huruf besar tidak dianggap sama.
        for typed in ["", "acm", "lain", "ACME"] {
            let res = request
                .post(DELETE)
                .add_cookie(cookie.clone())
                .form(&[("confirm_slug", typed)])
                .await;
            assert_eq!(res.status_code(), 422, "{typed:?}");
            assert_css_exists(&res.text(), "#delete-confirm-error");
            assert!(reload(&ctx).await.deletion_scheduled_at.is_none());
        }

        let res = request
            .post(DELETE)
            .add_cookie(cookie.clone())
            .form(&[("confirm_slug", " acme ")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme");
        let scheduled = reload(&ctx).await.deletion_scheduled_at.unwrap();
        let age = chrono::Utc::now() - scheduled.with_timezone(&chrono::Utc);
        assert!(age.num_seconds().abs() < 60);

        // Banner merah untuk Owner: tanggal dihapus permanen dan tombol batalkan.
        let home = request.get("/w/acme").add_cookie(cookie.clone()).await;
        assert_eq!(home.status_code(), 200);
        let html = home.text();
        assert_css_exists(&html, "[data-deletion-banner][data-variant=\"destructive\"]");
        assert_css_exists(
            &html,
            "[data-deletion-banner] form[method=\"post\"][action=\"/w/acme/settings/delete/cancel\"]",
        );
        // Job pembersihan berjalan 03:00 UTC (10:00 WIB): hapus permanen mulai run pertama
        // setelah tenggat 30 hari, ditampilkan dengan jam WIB.
        let deadline = (scheduled + chrono::Duration::days(30)).with_timezone(&chrono::Utc);
        let mut run = deadline.date_naive().and_hms_opt(3, 0, 0).unwrap().and_utc();
        if run <= deadline {
            run += chrono::Duration::days(1);
        }
        let expected = format!(
            "akan dihapus permanen mulai {} WIB",
            run.with_timezone(&chrono::FixedOffset::east_opt(7 * 3600).unwrap())
                .format("%d/%m/%Y 10:00")
        );
        assert!(html.contains(&expected), "{expected}: {html}");
        // Penjadwalan ulang tidak memperpanjang tenggat dan tidak menulis audit lagi.
        request
            .post(DELETE)
            .add_cookie(cookie.clone())
            .form(&[("confirm_slug", "acme")])
            .await;
        assert_eq!(reload(&ctx).await.deletion_scheduled_at, Some(scheduled));

        // Member tidak lagi melihat workspace.
        let cookie = sign_in_as(&request, &member).await;
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(cookie)
                .await
                .status_code(),
            404
        );

        let acts = actions(&ctx, &workspace).await;
        assert_eq!(
            acts.iter()
                .filter(|a| **a == AuditAction::WorkspaceDeletionScheduled)
                .count(),
            1,
            "jadwal ulang tidak diaudit: {acts:?}"
        );
        let event = &crate::support::newest_audit_events(&ctx.db, workspace.workspace_id(), 1)
            .await
            .unwrap()[0];
        assert_eq!(event.action().unwrap(), AuditAction::WorkspaceDeletionScheduled);
        assert_eq!(event.subject().unwrap(), AuditSubject::Workspace);
        assert_eq!(event.actor_id, Some(owner.id));
    })
    .await;
}

#[tokio::test]
async fn owner_can_cancel_scheduled_deletion() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post(DELETE)
            .add_cookie(cookie.clone())
            .form(&[("confirm_slug", "acme")])
            .await;
        assert_eq!(res.status_code(), 303);

        let res = request.post(CANCEL).add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme");
        assert!(reload(&ctx).await.deletion_scheduled_at.is_none());
        let html = request
            .get("/w/acme")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_not_exists(&html, "[data-deletion-banner]");
        // Membatalkan yang tidak terjadwal tidak menulis apa pun.
        let res = request.post(CANCEL).add_cookie(cookie).await;
        assert_eq!(res.status_code(), 303);

        let acts = actions(&ctx, &workspace).await;
        assert_eq!(
            acts,
            [
                AuditAction::WorkspaceDeletionCancelled,
                AuditAction::WorkspaceDeletionScheduled,
                AuditAction::WorkspaceCreated
            ]
        );

        let cookie = sign_in_as(&request, &member).await;
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(cookie)
                .await
                .status_code(),
            200
        );
    })
    .await;
}

#[tokio::test]
async fn only_builtin_owner_can_schedule_or_cancel_deletion() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        let manager = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        // Role custom yang memegang ManageWorkspace (dibuat Owner) tetap bukan Owner.
        let role = custom_role(
            &ctx,
            &workspace,
            "Pengelola",
            &[Permission::ManageWorkspace],
        )
        .await;
        workspace_members::ActiveModel::add(&ctx.db, workspace.workspace_id(), manager.id, role.id)
            .await
            .unwrap();

        for user in [&admin, &manager] {
            let cookie = sign_in_as(&request, user).await;
            let html = request
                .get("/w/acme/settings")
                .add_cookie(cookie.clone())
                .await
                .text();
            assert_css_not_exists(&html, "form[action=\"/w/acme/settings/delete\"]");
            // Wajibkan 2FA juga milik Owner (spec "bisa diwajibkan Owner"), bukan ManageWorkspace.
            assert_css_not_exists(&html, "form[action=\"/w/acme/settings/require-2fa\"]");
            // Sub-halaman Keamanan dan Zona berbahaya hanya untuk Owner.
            for path in ["/w/acme/settings/security", "/w/acme/settings/danger"] {
                let res = request.get(path).add_cookie(cookie.clone()).await;
                assert_eq!(res.status_code(), 404, "{path}");
            }
            let res = request
                .post(DELETE)
                .add_cookie(cookie.clone())
                .form(&[("confirm_slug", "acme")])
                .await;
            assert_eq!(res.status_code(), 404);
            assert!(reload(&ctx).await.deletion_scheduled_at.is_none());
            for enabled in ["true", "false"] {
                let res = request
                    .post("/w/acme/settings/require-2fa")
                    .add_cookie(cookie.clone())
                    .form(&[("enabled", enabled)])
                    .await;
                assert_eq!(res.status_code(), 404, "{enabled}");
            }
            assert!(reload(&ctx).await.require_2fa_at.is_none());
        }

        let cookie = sign_in_as(&request, &manager).await;

        let owner_cookie = sign_in_as(&request, &owner).await;
        request
            .post(DELETE)
            .add_cookie(owner_cookie)
            .form(&[("confirm_slug", "acme")])
            .await;
        // Non-Owner tidak bisa membatalkan: workspace sudah tersembunyi baginya (404).
        let res = request.post(CANCEL).add_cookie(cookie).await;
        assert_eq!(res.status_code(), 404);
        assert!(reload(&ctx).await.deletion_scheduled_at.is_some());
    })
    .await;
}

/// Workspace dihapus permanen oleh job pembersihan setelah `Tenant` memuatnya: 404, bukan 500.
#[tokio::test]
async fn schedule_or_cancel_after_purge_is_not_found() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).slug("acme").create(ctx).await;
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
    let res = deletion::schedule(&ctx.db, &acting, "acme").await;
    assert!(matches!(res, Err(DeletionError::NotFound)), "{res:?}");
    let res = deletion::cancel(&ctx.db, &acting).await;
    assert!(matches!(res, Err(DeletionError::NotFound)), "{res:?}");
}
