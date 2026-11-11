//! `/w/{slug}/settings/roles`: role custom (`ManageRoles`). Role bawaan tidak bisa diubah atau
//! dihapus (`role-builtin`), role yang masih dipakai tidak bisa dihapus (`role-in-use`).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        invitations,
        permission::Permission,
        project_grants::GrantSubject,
        roles::{self, RoleKey},
        workspace_members, workspaces,
    },
};
use loco_rs::testing::prelude::*;

use crate::support::{a_project, a_user, a_workspace, add_member, grant, sign_in_as};

async fn custom_role(
    ctx: &loco_rs::app::AppContext,
    workspace: &workspaces::Model,
    name: &str,
) -> roles::Model {
    roles::ActiveModel::create_custom(
        &ctx.db,
        workspace.workspace_id(),
        name,
        vec![Permission::ViewProject],
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn custom_role_with_permissions_can_be_created() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let cookie = sign_in_as(&request, &owner).await;

        // Indeks Pengaturan menautkan halaman role dan jabatan.
        let html = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "[data-settings-index] a[href=\"/w/acme/settings/roles\"]",
        );
        assert_css_exists(
            &html,
            "[data-settings-index] a[href=\"/w/acme/settings/job-titles\"]",
        );

        let form = request
            .get("/w/acme/settings/roles/new")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(form.status_code(), 200);
        let html = form.text();
        assert_css_exists(
            &html,
            "form[method=\"post\"][action=\"/w/acme/settings/roles/new\"]",
        );
        assert_css_exists(
            &html,
            "input[type=\"checkbox\"][value=\"manage_workspace\"]",
        );
        assert_css_exists(&html, "input[type=\"checkbox\"][value=\"view_project\"]");
        assert!(html.contains("Lihat project"), "label izin dari Fluent");

        let res = request
            .post("/w/acme/settings/roles/new")
            .add_cookie(cookie.clone())
            .form(&[
                ("name", "  QA Lead "),
                ("permissions", "edit_project"),
                ("permissions", "view_project"),
                ("permissions", "view_project"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/settings/roles");

        let role = roles::Model::in_workspace(&ctx.db, wid)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.name == "QA Lead")
            .unwrap();
        assert!(!role.builtin);
        assert_eq!(
            role.permissions().unwrap(),
            vec![Permission::EditProject, Permission::ViewProject]
        );
        let events = crate::support::newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::RoleCreated);
        assert_eq!(events[0].subject().unwrap(), AuditSubject::Role);
        assert_eq!(events[0].subject_id, role.id);
        assert_eq!(events[0].actor_id, Some(owner.id));

        let html = request
            .get("/w/acme/settings/roles")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert!(html.contains("Role dibuat"));
        assert!(html.contains("QA Lead"));
        let edit = format!("/w/acme/settings/roles/{}/edit", role.id);
        assert_css_exists(&html, &format!("a[href=\"{edit}\"]"));

        // Ubah: nama dan izin, audit hanya field yang berubah.
        let html = request.get(&edit).add_cookie(cookie.clone()).await.text();
        assert_css_exists(&html, "input#role-name[value=\"QA Lead\"]");
        assert_css_exists(&html, "input[value=\"edit_project\"][checked]");
        assert_css_not_exists(&html, "input[value=\"manage_roles\"][checked]");
        let res = request
            .post(&edit)
            .add_cookie(cookie.clone())
            .form(&[("name", "QA"), ("permissions", "view_project")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let updated = roles::Model::find_in(&ctx.db, wid, role.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "QA");
        assert_eq!(
            updated.permissions().unwrap(),
            vec![Permission::ViewProject]
        );
        let events = crate::support::newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::RoleUpdated);
        assert_eq!(
            events[0].changes,
            serde_json::json!({
                "name": { "from": "QA Lead", "to": "QA" },
                "permissions": {
                    "from": ["edit_project", "view_project"],
                    "to": ["view_project"],
                },
            })
        );

        // Izin tak dikenal atau kosong: 422, form terisi kembali.
        for body in [
            vec![("name", "Aneh"), ("permissions", "bogus")],
            vec![("name", "Kosong")],
        ] {
            let res = request
                .post("/w/acme/settings/roles/new")
                .add_cookie(cookie.clone())
                .form(&body)
                .await;
            assert_eq!(res.status_code(), 422, "{body:?}");
            let html = res.text();
            assert_css_exists(&html, "#role-permissions-error");
            assert_css_exists(&html, &format!("input#role-name[value=\"{}\"]", body[0].1));
        }

        // Hapus role yang tidak dipakai.
        let res = request
            .post(&format!("/w/acme/settings/roles/{}/delete", role.id))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(
            roles::Model::find_in(&ctx.db, wid, role.id)
                .await
                .unwrap()
                .is_none()
        );
        let events = crate::support::newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::RoleDeleted);
        assert_eq!(events[0].subject_id, role.id);
    })
    .await;
}

#[tokio::test]
async fn builtin_role_cannot_be_edited_or_deleted() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let cookie = sign_in_as(&request, &owner).await;
        let audit_before = crate::support::newest_audit_events(&ctx.db, wid, 50)
            .await
            .unwrap()
            .len();

        let html = request
            .get("/w/acme/settings/roles")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(&html, "[data-builtin-role]");
        assert_css_not_exists(&html, "[data-builtin-role] a[href$=\"/edit\"]");
        assert_css_not_exists(&html, "[data-builtin-role] form");

        for key in [RoleKey::Owner, RoleKey::Viewer] {
            let role = roles::Model::find_builtin(&ctx.db, wid, key)
                .await
                .unwrap()
                .unwrap();
            let edit = format!("/w/acme/settings/roles/{}/edit", role.id);
            let res = request.get(&edit).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 422, "{key:?}");
            assert!(res.text().contains("Role bawaan tidak bisa diubah"));
            let res = request
                .post(&edit)
                .add_cookie(cookie.clone())
                .form(&[("name", "Bos"), ("permissions", "view_project")])
                .await;
            assert_eq!(res.status_code(), 422, "{key:?}");
            assert!(res.text().contains("Role bawaan tidak bisa diubah"));
            let res = request
                .post(&format!("/w/acme/settings/roles/{}/delete", role.id))
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 422, "{key:?}");

            let unchanged = roles::Model::find_in(&ctx.db, wid, role.id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(unchanged, role);
        }
        // Model menolak juga, bukan hanya controller.
        let owner_role = roles::Model::find_builtin(&ctx.db, wid, RoleKey::Owner)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            roles::ensure_role_editable(&owner_role),
            Err(roles::RoleError::Builtin)
        ));
        assert!(matches!(
            roles::ActiveModel::update_custom(&ctx.db, owner_role.clone(), "X", vec![]).await,
            Err(roles::RoleError::Builtin)
        ));
        assert!(matches!(
            roles::ActiveModel::delete_custom(&ctx.db, owner_role).await,
            Err(roles::RoleError::Builtin)
        ));
        assert_eq!(
            crate::support::newest_audit_events(&ctx.db, wid, 50)
                .await
                .unwrap()
                .len(),
            audit_before
        );
    })
    .await;
}

#[tokio::test]
async fn role_in_use_cannot_be_deleted() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let role = custom_role(&ctx, &workspace, "Tamu").await;
        let guest = a_user().create(&ctx).await;
        workspace_members::ActiveModel::add(&ctx.db, wid, guest.id, role.id)
            .await
            .unwrap();
        let cookie = sign_in_as(&request, &owner).await;

        let res = request
            .post(&format!("/w/acme/settings/roles/{}/delete", role.id))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("masih dipakai"));
        assert!(
            roles::Model::find_in(&ctx.db, wid, role.id)
                .await
                .unwrap()
                .is_some()
        );
        let events = crate::support::newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap();
        assert_ne!(events[0].action().unwrap(), AuditAction::RoleDeleted);
    })
    .await;
}

#[tokio::test]
async fn member_without_manage_roles_gets_404() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let role = custom_role(&ctx, &workspace, "Tamu").await;
        let edit = format!("/w/acme/settings/roles/{}/edit", role.id);
        let delete = format!("/w/acme/settings/roles/{}/delete", role.id);

        for key in [RoleKey::Member, RoleKey::Viewer] {
            let user = a_user().create(&ctx).await;
            add_member(&ctx, &workspace, &user, key).await;
            let cookie = sign_in_as(&request, &user).await;
            for path in [
                "/w/acme/settings/roles",
                "/w/acme/settings/roles/new",
                &edit,
            ] {
                let res = request.get(path).add_cookie(cookie.clone()).await;
                assert_eq!(res.status_code(), 404, "{key:?} GET {path}");
            }
            // Body tidak valid tetap 404: guard sebelum body.
            for path in ["/w/acme/settings/roles/new", &edit] {
                let res = request
                    .post(path)
                    .add_cookie(cookie.clone())
                    .form(&[("name", ""), ("permissions", "bogus")])
                    .await;
                assert_eq!(res.status_code(), 404, "{key:?} POST {path}");
            }
            let res = request.post(&delete).add_cookie(cookie).await;
            assert_eq!(res.status_code(), 404, "{key:?} delete");
        }

        // Role milik workspace lain → 404 untuk Owner workspace ini; id bukan UUID juga 404.
        let other_owner = a_user().create(&ctx).await;
        a_workspace(&other_owner).slug("lain").create(&ctx).await;
        let cookie = sign_in_as(&request, &other_owner).await;
        let foreign_edit = format!("/w/lain/settings/roles/{}/edit", role.id);
        let res = request.get(&foreign_edit).add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post(&foreign_edit)
            .add_cookie(cookie.clone())
            .form(&[("name", "Dibajak"), ("permissions", "view_project")])
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post(&format!("/w/lain/settings/roles/{}/delete", role.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .get("/w/lain/settings/roles/bukan-uuid/edit")
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);
        let unchanged = roles::Model::find_in(&ctx.db, workspace.workspace_id(), role.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.name, "Tamu");
    })
    .await;
}

#[tokio::test]
async fn duplicate_role_name_gets_422() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        custom_role(&ctx, &workspace, "QA").await;
        let other = custom_role(&ctx, &workspace, "Dev").await;
        let cookie = sign_in_as(&request, &owner).await;

        // Sama dengan role custom atau bawaan, tanpa membedakan huruf besar.
        for name in ["qa", "OWNER"] {
            let res = request
                .post("/w/acme/settings/roles/new")
                .add_cookie(cookie.clone())
                .form(&[("name", name), ("permissions", "view_project")])
                .await;
            assert_eq!(res.status_code(), 422, "{name}");
            let html = res.text();
            assert_css_exists(&html, "#role-name-error");
            assert!(html.contains("sudah dipakai"));
            assert_css_exists(&html, "input[value=\"view_project\"][checked]");
        }

        let res = request
            .post(&format!("/w/acme/settings/roles/{}/edit", other.id))
            .add_cookie(cookie)
            .form(&[("name", "Qa"), ("permissions", "view_project")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#role-name-error");
    })
    .await;
}

async fn role_with(
    ctx: &loco_rs::app::AppContext,
    workspace: &workspaces::Model,
    name: &str,
    permissions: Vec<Permission>,
) -> roles::Model {
    roles::ActiveModel::create_custom(&ctx.db, workspace.workspace_id(), name, permissions)
        .await
        .unwrap()
}

#[tokio::test]
async fn non_owner_cannot_grant_or_revoke_permissions_they_lack() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let manager_role = role_with(
            &ctx,
            &workspace,
            "Pengelola",
            vec![Permission::ManageRoles, Permission::ViewProject],
        )
        .await;
        let auditor = role_with(
            &ctx,
            &workspace,
            "Auditor",
            vec![Permission::ViewAuditLog, Permission::ViewProject],
        )
        .await;

        // Admin (tanpa ManageWorkspace) tidak bisa membuat role dengan ManageWorkspace.
        let admin = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &admin).await;
        let html = request
            .get("/w/acme/settings/roles/new")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "input[type=\"checkbox\"][value=\"manage_workspace\"][disabled]",
        );
        assert_css_not_exists(
            &html,
            "input[type=\"checkbox\"][value=\"manage_roles\"][disabled]",
        );
        let res = request
            .post("/w/acme/settings/roles/new")
            .add_cookie(cookie)
            .form(&[("name", "Bos"), ("permissions", "manage_workspace")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#role-permissions-error");

        // Pemegang role custom dengan ManageRoles: tidak bisa menambah izin ke role-nya sendiri.
        let manager = a_user().create(&ctx).await;
        workspace_members::ActiveModel::add(&ctx.db, wid, manager.id, manager_role.id)
            .await
            .unwrap();
        let cookie = sign_in_as(&request, &manager).await;
        let own = format!("/w/acme/settings/roles/{}/edit", manager_role.id);
        let res = request
            .post(&own)
            .add_cookie(cookie.clone())
            .form(&[
                ("name", "Pengelola"),
                ("permissions", "manage_roles"),
                ("permissions", "view_project"),
                ("permissions", "manage_workspace"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#role-permissions-error");

        // Mencabut izin yang tidak dipegang (ViewAuditLog) juga ditolak.
        let edit = format!("/w/acme/settings/roles/{}/edit", auditor.id);
        let html = request.get(&edit).add_cookie(cookie.clone()).await.text();
        assert_css_exists(
            &html,
            "input[type=\"checkbox\"][value=\"view_audit_log\"][checked][disabled]",
        );
        assert_css_exists(
            &html,
            "input[type=\"hidden\"][name=\"permissions\"][value=\"view_audit_log\"]",
        );
        let res = request
            .post(&edit)
            .add_cookie(cookie.clone())
            .form(&[("name", "Auditor"), ("permissions", "view_project")])
            .await;
        assert_eq!(res.status_code(), 422);
        // Izin yang tidak dipegang tetap terbawa (input hidden): ganti nama lolos.
        let res = request
            .post(&edit)
            .add_cookie(cookie)
            .form(&[
                ("name", "Pemeriksa"),
                ("permissions", "view_audit_log"),
                ("permissions", "view_project"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let unchanged = roles::Model::find_in(&ctx.db, wid, manager_role.id)
            .await
            .unwrap()
            .unwrap();
        assert!(!unchanged.has(Permission::ManageWorkspace));

        // Owner memegang semua izin: semuanya boleh.
        let cookie = sign_in_as(&request, &owner).await;
        let html = request
            .get("/w/acme/settings/roles/new")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_not_exists(&html, "input[type=\"checkbox\"][disabled]");
        let res = request
            .post("/w/acme/settings/roles/new")
            .add_cookie(cookie.clone())
            .form(&[("name", "Bos"), ("permissions", "manage_workspace")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = request
            .post(&edit)
            .add_cookie(cookie.clone())
            .form(&[("name", "Pemeriksa"), ("permissions", "view_project")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = request
            .post(&own)
            .add_cookie(cookie)
            .form(&[
                ("name", "Pengelola"),
                ("permissions", "manage_roles"),
                ("permissions", "manage_workspace"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
    })
    .await;
}

async fn invite(
    ctx: &loco_rs::app::AppContext,
    workspace: &workspaces::Model,
    role: &roles::Model,
    state: &str,
) {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    let now = chrono::Utc::now();
    let mut invitation = invitations::ActiveModel {
        workspace_id: Set(workspace.id),
        email: Set(format!(
            "{state}-{}@example.test",
            uuid::Uuid::now_v7().simple()
        )),
        role_id: Set(role.id),
        token_digest: Set(uuid::Uuid::now_v7().simple().to_string()),
        expires_at: Set((now + chrono::Duration::days(7)).into()),
        ..Default::default()
    };
    match state {
        "accepted" => invitation.accepted_at = Set(Some(now.into())),
        "revoked" => invitation.revoked_at = Set(Some(now.into())),
        "expired" => invitation.expires_at = Set((now - chrono::Duration::hours(1)).into()),
        _ => {}
    }
    invitation.insert(&ctx.db).await.unwrap();
}

#[tokio::test]
async fn only_active_invitations_block_role_deletion() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let cookie = sign_in_as(&request, &owner).await;

        for state in ["accepted", "revoked", "expired"] {
            let role = custom_role(&ctx, &workspace, state).await;
            invite(&ctx, &workspace, &role, state).await;
            let res = request
                .post(&format!("/w/acme/settings/roles/{}/delete", role.id))
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 303, "{state}: {}", res.text());
            assert!(
                roles::Model::find_in(&ctx.db, wid, role.id)
                    .await
                    .unwrap()
                    .is_none()
            );
        }

        let role = custom_role(&ctx, &workspace, "aktif").await;
        invite(&ctx, &workspace, &role, "accepted").await;
        invite(&ctx, &workspace, &role, "active").await;
        let res = request
            .post(&format!("/w/acme/settings/roles/{}/delete", role.id))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("undangan aktif"));
        // Transaksi dibatalkan: undangan yang sudah selesai pun tetap ada.
        use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
        let left = invitations::Entity::find()
            .filter(invitations::Column::RoleId.eq(role.id))
            .count(&ctx.db)
            .await
            .unwrap();
        assert_eq!(left, 2);
    })
    .await;
}

/// Izin role yang dipakai grant project hanya diubah Owner atau Admin bawaan: tanpa itu,
/// pemegang `ManageRoles` menaikkan akses project semua pemegang grant tanpa lewat aturan
/// pemberian grant (`can_grant`).
#[tokio::test]
async fn only_admin_everywhere_changes_permissions_of_a_granted_role() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let manager_role = role_with(
            &ctx,
            &workspace,
            "Pengelola",
            vec![
                Permission::ManageRoles,
                Permission::AdministerProject,
                Permission::EditProject,
                Permission::ViewProject,
            ],
        )
        .await;
        let manager = a_user().create(&ctx).await;
        workspace_members::ActiveModel::add(&ctx.db, wid, manager.id, manager_role.id)
            .await
            .unwrap();
        let tamu = custom_role(&ctx, &workspace, "Tamu").await;
        let budi = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &budi, RoleKey::Viewer).await;
        let project = a_project(&ctx, &workspace, "KSR").await;
        grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::User(budi.id),
            &tamu,
        )
        .await;
        let edit = format!("/w/acme/settings/roles/{}/edit", tamu.id);

        let cookie = sign_in_as(&request, &manager).await;
        let res = request
            .post(&edit)
            .add_cookie(cookie.clone())
            .form(&[
                ("name", "Tamu"),
                ("permissions", "view_project"),
                ("permissions", "administer_project"),
            ])
            .await;
        assert_eq!(res.status_code(), 422, "{}", res.text());
        let html = res.text();
        assert_css_exists(&html, "#role-permissions-error");
        assert!(html.contains("dipakai untuk akses project"), "{html}");
        let unchanged = roles::Model::find_in(&ctx.db, wid, tamu.id)
            .await
            .unwrap()
            .unwrap();
        assert!(!unchanged.has(Permission::AdministerProject));
        // Ganti nama saja (izin sama) tetap boleh.
        let res = request
            .post(&edit)
            .add_cookie(cookie)
            .form(&[("name", "Tamu Project"), ("permissions", "view_project")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        // Admin bawaan boleh.
        let admin = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &admin).await;
        let res = request
            .post(&edit)
            .add_cookie(cookie)
            .form(&[
                ("name", "Tamu Project"),
                ("permissions", "view_project"),
                ("permissions", "edit_project"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
    })
    .await;
}

/// Service membaca ulang role pelaku setelah kunci: pelaku yang kehilangan `ManageRoles` di
/// tengah jalan, dan role yang sudah dihapus, mendapat `NotFound` (404), bukan 500.
#[tokio::test]
async fn role_service_rechecks_actor_and_target_under_lock() {
    use agentgate_server::services::workspaces::{Acting, roles as service};
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.workspace_id();
    let admin = a_user().create(ctx).await;
    let admin_member = add_member(ctx, &workspace, &admin, RoleKey::Admin).await;
    let admin_role = roles::Model::find_builtin(&ctx.db, wid, RoleKey::Admin)
        .await
        .unwrap()
        .unwrap();
    let viewer_role = roles::Model::find_builtin(&ctx.db, wid, RoleKey::Viewer)
        .await
        .unwrap()
        .unwrap();
    let target = custom_role(ctx, &workspace, "Tamu").await;
    let acting = Acting {
        workspace: &workspace,
        user: &admin,
        role: &admin_role,
        request_id: None,
    };

    // Role tujuan sudah tidak ada.
    let gone = uuid::Uuid::now_v7();
    let res = service::update(&ctx.db, &acting, gone, "X", vec![Permission::ViewProject]).await;
    assert!(matches!(res, Err(roles::RoleError::NotFound)), "{res:?}");
    let res = service::delete(&ctx.db, &acting, gone).await;
    assert!(matches!(res, Err(roles::RoleError::NotFound)), "{res:?}");

    // Role di DB sudah Viewer, tetapi Acting masih membawa Admin dari awal request.
    admin_member
        .set_role(&ctx.db, viewer_role.id)
        .await
        .unwrap();
    let res = service::create(&ctx.db, &acting, "Baru", vec![Permission::ViewProject]).await;
    assert!(matches!(res, Err(roles::RoleError::NotFound)), "{res:?}");
    let res = service::update(
        &ctx.db,
        &acting,
        target.id,
        "Ubah",
        vec![Permission::ViewProject],
    )
    .await;
    assert!(matches!(res, Err(roles::RoleError::NotFound)), "{res:?}");
    let res = service::delete(&ctx.db, &acting, target.id).await;
    assert!(matches!(res, Err(roles::RoleError::NotFound)), "{res:?}");
    assert!(
        roles::Model::find_in(&ctx.db, wid, target.id)
            .await
            .unwrap()
            .is_some()
    );
}
