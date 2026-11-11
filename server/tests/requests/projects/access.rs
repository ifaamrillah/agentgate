//! Akses project: extractor `ProjectAccess` (tanpa akses → 404), pengaturan akses
//! (`/p/{key}/settings/access`, `AdministerProject`), dan sidebar yang hanya berisi project yang
//! bisa dibuka.

use agentgate_server::{
    app::App,
    models::{
        audit_events::{self, AuditAction, AuditSubject},
        permission::Permission,
        project_grants::{self, GrantSubject},
        roles::{self, RoleKey},
        team_members, workspace_members,
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{EntityTrait, PaginatorTrait};

use crate::support::{
    a_project, a_team, a_user, a_workspace, add_member, builtin_role, custom_role, grant,
    sign_in_as,
};

#[tokio::test]
async fn member_without_grant_gets_404_for_project() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        a_project(&ctx, &workspace, "KSR").await;
        let cookie = sign_in_as(&request, &member).await;
        for path in [
            "/w/acme/p/KSR",
            "/w/acme/p/KSR/settings",
            "/w/acme/p/KSR/settings/access",
            "/w/acme/p/KSR/settings/repos",
            "/w/acme/p/NOPE",
        ] {
            let res = request.get(path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        // Owner tanpa grant tetap Admin project.
        let cookie = sign_in_as(&request, &owner).await;
        let res = request.get("/w/acme/p/KSR").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 200);
    })
    .await;
}

#[tokio::test]
async fn team_grant_gives_access_to_team_members() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let outsider = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        add_member(&ctx, &workspace, &outsider, RoleKey::Member).await;
        let project = a_project(&ctx, &workspace, "KSR").await;
        let team = a_team(&ctx, &workspace, "Backend", &[&dev]).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;

        let owner_cookie = sign_in_as(&request, &owner).await;
        let page = request
            .get("/w/acme/p/KSR/settings/access")
            .add_cookie(owner_cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(
            &page.text(),
            &format!(
                "select[name=\"subject\"] option[value=\"team:{}\"]",
                team.id
            ),
        );
        let res = request
            .post("/w/acme/p/KSR/settings/access")
            .add_cookie(owner_cookie.clone())
            .form(&[
                ("subject", format!("team:{}", team.id)),
                ("role_id", member_role.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/p/KSR/settings/access");
        let grants = project_grants::Model::of_project(&ctx.db, wid, project.id)
            .await
            .unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].subject().unwrap(), GrantSubject::Team(team.id));
        let events = audit_events::Model::newest_first(&ctx.db, wid, 1)
            .await
            .unwrap();
        assert_eq!(
            events[0].action().unwrap(),
            AuditAction::ProjectAccessGranted
        );
        assert_eq!(events[0].subject().unwrap(), AuditSubject::ProjectGrant);
        assert_eq!(events[0].subject_id, grants[0].id);

        let cookie = sign_in_as(&request, &dev).await;
        let res = request
            .get("/w/acme/p/KSR")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        // Member project tidak mengatur project.
        assert_css_not_exists(&res.text(), "a[href=\"/w/acme/p/KSR/settings\"]");
        let res = request
            .get("/w/acme/p/KSR/settings/access")
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);

        let cookie = sign_in_as(&request, &outsider).await;
        let res = request.get("/w/acme/p/KSR").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn viewer_grant_cannot_edit_project_settings_gets_404() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let viewer = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &viewer, RoleKey::Member).await;
        let project = a_project(&ctx, &workspace, "KSR").await;
        let viewer_role = builtin_role(&ctx, &workspace, RoleKey::Viewer).await;
        let grant = grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::User(viewer.id),
            &viewer_role,
        )
        .await;
        let cookie = sign_in_as(&request, &viewer).await;

        let res = request
            .get("/w/acme/p/KSR")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        for path in [
            "/w/acme/p/KSR/settings",
            "/w/acme/p/KSR/settings/access",
            "/w/acme/p/KSR/settings/repos",
        ] {
            let res = request.get(path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "GET {path}");
        }
        // POST ditolak sebelum body dibaca (body kosong pun 404, bukan 400/422).
        for path in [
            "/w/acme/p/KSR/settings".to_string(),
            "/w/acme/p/KSR/settings/archive".to_string(),
            "/w/acme/p/KSR/settings/access".to_string(),
            format!("/w/acme/p/KSR/settings/access/{}/role", grant.id),
            format!("/w/acme/p/KSR/settings/access/{}/revoke", grant.id),
            "/w/acme/p/KSR/settings/repos".to_string(),
        ] {
            let res = request.post(&path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "POST {path}");
        }
        assert!(
            project_grants::Entity::find_by_id(grant.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_some()
        );
    })
    .await;
}

#[tokio::test]
async fn removing_team_grant_revokes_access() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        let project = a_project(&ctx, &workspace, "KSR").await;
        let team = a_team(&ctx, &workspace, "Backend", &[&dev]).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let viewer_role = builtin_role(&ctx, &workspace, RoleKey::Viewer).await;
        let grant = grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::Team(team.id),
            &member_role,
        )
        .await;
        let dev_cookie = sign_in_as(&request, &dev).await;
        let res = request
            .get("/w/acme/p/KSR")
            .add_cookie(dev_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);

        let owner_cookie = sign_in_as(&request, &owner).await;
        // Ganti role grant.
        let res = request
            .post(&format!("/w/acme/p/KSR/settings/access/{}/role", grant.id))
            .add_cookie(owner_cookie.clone())
            .form(&[("role_id", viewer_role.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let events = audit_events::Model::newest_first(&ctx.db, wid, 1)
            .await
            .unwrap();
        assert_eq!(
            events[0].action().unwrap(),
            AuditAction::ProjectAccessChanged
        );
        assert_eq!(
            events[0].changes["role"],
            serde_json::json!({ "from": "Member", "to": "Viewer" })
        );

        // Grant milik project lain atau id tidak valid → 404.
        let res = request
            .post("/w/acme/p/KSR/settings/access/bukan-uuid/revoke")
            .add_cookie(owner_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post(&format!(
                "/w/acme/p/KSR/settings/access/{}/revoke",
                uuid::Uuid::now_v7()
            ))
            .add_cookie(owner_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);

        let res = request
            .post(&format!(
                "/w/acme/p/KSR/settings/access/{}/revoke",
                grant.id
            ))
            .add_cookie(owner_cookie)
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let events = audit_events::Model::newest_first(&ctx.db, wid, 1)
            .await
            .unwrap();
        assert_eq!(
            events[0].action().unwrap(),
            AuditAction::ProjectAccessRevoked
        );
        assert_eq!(events[0].subject_id, grant.id);

        let res = request
            .get("/w/acme/p/KSR")
            .add_cookie(dev_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let home = request.get("/w/acme").add_cookie(dev_cookie).await.text();
        assert_css_not_exists(&home, "a[href=\"/w/acme/p/KSR\"]");
    })
    .await;
}

#[tokio::test]
async fn project_of_other_workspace_gets_404_even_with_same_key() {
    request::<App, _, _>(|request, ctx| async move {
        let alice = a_user().create(&ctx).await;
        let bob = a_user().create(&ctx).await;
        let acme = a_workspace(&alice).slug("acme").create(&ctx).await;
        let globex = a_workspace(&bob).slug("globex").create(&ctx).await;
        a_project(&ctx, &acme, "KSR").await;
        let theirs = a_project(&ctx, &globex, "KSR").await;
        let cookie = sign_in_as(&request, &alice).await;
        // Bukan anggota globex: 404, walau key sama dengan project miliknya sendiri.
        let res = request
            .get("/w/globex/p/KSR")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .get("/w/acme/p/KSR")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);

        // Grant dengan id milik workspace lain lewat URL acme → 404.
        let role = builtin_role(&ctx, &globex, RoleKey::Viewer).await;
        let foreign = grant(&ctx, &globex, &theirs, GrantSubject::User(bob.id), &role).await;
        let res = request
            .post(&format!(
                "/w/acme/p/KSR/settings/access/{}/revoke",
                foreign.id
            ))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        // Role workspace lain sebagai role grant → 404.
        let res = request
            .post("/w/acme/p/KSR/settings/access")
            .add_cookie(cookie)
            .form(&[
                ("subject", format!("user:{}", alice.id)),
                ("role_id", role.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn sidebar_lists_only_accessible_projects() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        let ksr = a_project(&ctx, &workspace, "KSR").await;
        let gdg = a_project(&ctx, &workspace, "GDG").await;
        a_project(&ctx, &workspace, "HRD").await;
        let viewer = builtin_role(&ctx, &workspace, RoleKey::Viewer).await;
        let team = a_team(&ctx, &workspace, "QA", &[&dev]).await;
        grant(&ctx, &workspace, &ksr, GrantSubject::User(dev.id), &viewer).await;
        grant(&ctx, &workspace, &gdg, GrantSubject::Team(team.id), &viewer).await;

        let cookie = sign_in_as(&request, &dev).await;
        let html = request.get("/w/acme").add_cookie(cookie).await.text();
        assert_css_exists(&html, "[data-project-list] a[href=\"/w/acme/p/KSR\"]");
        assert_css_exists(&html, "[data-project-list] a[href=\"/w/acme/p/GDG\"]");
        assert_css_not_exists(&html, "a[href=\"/w/acme/p/HRD\"]");
        // Beranda workspace mendaftar project yang sama.
        assert_css_exists(&html, "[data-project=\"KSR\"]");
        assert_css_not_exists(&html, "[data-project=\"HRD\"]");

        let cookie = sign_in_as(&request, &owner).await;
        let html = request.get("/w/acme").add_cookie(cookie).await.text();
        for key in ["KSR", "GDG", "HRD"] {
            assert_css_exists(
                &html,
                &format!("[data-project-list] a[href=\"/w/acme/p/{key}\"]"),
            );
        }
    })
    .await;
}

#[tokio::test]
async fn grant_role_cannot_exceed_actor_access() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let lead = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &lead, RoleKey::Member).await;
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        let lead_cookie = sign_in_as(&request, &lead).await;
        // Lead (Member workspace) membuat project → Admin project lewat grant.
        let res = request
            .post("/w/acme/projects/new")
            .add_cookie(lead_cookie.clone())
            .form(&[("key", "KSR"), ("name", "Kasir"), ("description", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        let owner_role = builtin_role(&ctx, &workspace, RoleKey::Owner).await;
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;

        let page = request
            .get("/w/acme/p/KSR/settings/access")
            .add_cookie(lead_cookie.clone())
            .await
            .text();
        // Pilihan role hanya yang boleh ia berikan.
        assert_css_exists(
            &page,
            &format!(
                "select[name=\"role_id\"] option[value=\"{}\"]",
                member_role.id
            ),
        );
        assert_css_not_exists(&page, &format!("option[value=\"{}\"]", owner_role.id));
        assert_css_not_exists(&page, &format!("option[value=\"{}\"]", admin_role.id));

        for role in [&owner_role, &admin_role] {
            let res = request
                .post("/w/acme/p/KSR/settings/access")
                .add_cookie(lead_cookie.clone())
                .form(&[
                    ("subject", format!("user:{}", dev.id)),
                    ("role_id", role.id.to_string()),
                ])
                .await;
            assert_eq!(res.status_code(), 422, "{}", role.name);
            assert!(
                res.text().contains("tidak bisa memberikan"),
                "{}",
                role.name
            );
        }
        let res = request
            .post("/w/acme/p/KSR/settings/access")
            .add_cookie(lead_cookie.clone())
            .form(&[
                ("subject", format!("user:{}", dev.id)),
                ("role_id", member_role.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        // Subjek yang sama dua kali → 422.
        let res = request
            .post("/w/acme/p/KSR/settings/access")
            .add_cookie(lead_cookie.clone())
            .form(&[
                ("subject", format!("user:{}", dev.id)),
                ("role_id", member_role.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 422);

        // Lead tidak bisa mencabut grant Admin (role yang tidak boleh ia berikan).
        let project = agentgate_server::models::projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        let grants = project_grants::Model::of_project(&ctx.db, wid, project.id)
            .await
            .unwrap();
        let lead_grant = grants.iter().find(|g| g.user_id == Some(lead.id)).unwrap();
        let res = request
            .post(&format!(
                "/w/acme/p/KSR/settings/access/{}/revoke",
                lead_grant.id
            ))
            .add_cookie(lead_cookie)
            .await;
        assert_eq!(res.status_code(), 422);

        // Owner workspace boleh memberi role Owner.
        let owner_cookie = sign_in_as(&request, &owner).await;
        let other = a_team(&ctx, &workspace, "Ops", &[]).await;
        let res = request
            .post("/w/acme/p/KSR/settings/access")
            .add_cookie(owner_cookie)
            .form(&[
                ("subject", format!("team:{}", other.id)),
                ("role_id", owner_role.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
    })
    .await;
}

#[tokio::test]
async fn role_used_by_a_grant_cannot_be_deleted() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let project = a_project(&ctx, &workspace, "KSR").await;
        let team = a_team(&ctx, &workspace, "QA", &[]).await;
        let role = custom_role(&ctx, &workspace, "Penguji", &[Permission::ViewProject]).await;
        grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::Team(team.id),
            &role,
        )
        .await;
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
    })
    .await;
}

#[tokio::test]
async fn removed_or_leaving_member_loses_grants_and_team_memberships() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let qa = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let dev_member = add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        add_member(&ctx, &workspace, &qa, RoleKey::Member).await;
        let project = a_project(&ctx, &workspace, "KSR").await;
        let viewer = builtin_role(&ctx, &workspace, RoleKey::Viewer).await;
        let team = a_team(&ctx, &workspace, "Tim", &[&dev, &qa]).await;
        grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::Team(team.id),
            &viewer,
        )
        .await;
        let dev_grant = grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::User(dev.id),
            &viewer,
        )
        .await;
        let qa_grant = grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::User(qa.id),
            &viewer,
        )
        .await;

        // Dikeluarkan Owner.
        let cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post(&format!(
                "/w/acme/settings/members/{}/remove",
                dev_member.id
            ))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(
            project_grants::Entity::find_by_id(dev_grant.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            team_members::Model::find(&ctx.db, wid, team.id, dev.id)
                .await
                .unwrap()
                .is_none()
        );

        // Keluar sendiri.
        let cookie = sign_in_as(&request, &qa).await;
        let res = request.post("/w/acme/leave").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(
            project_grants::Entity::find_by_id(qa_grant.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            team_members::Entity::find().count(&ctx.db).await.unwrap(),
            0
        );
        // Grant tim tetap ada.
        assert_eq!(
            project_grants::Model::of_project(&ctx.db, wid, project.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            workspace_members::Model::find_with_role(&ctx.db, wid, qa.id)
                .await
                .unwrap()
                .is_none()
        );
    })
    .await;
}

/// Role tanpa izin project tidak memberi akses, jadi tidak bisa dipakai untuk grant (form tidak
/// menawarkannya; POST langsung ditolak service), baik saat memberi maupun mengganti role.
#[tokio::test]
async fn grant_role_without_project_permissions_is_rejected() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        let project = a_project(&ctx, &workspace, "KSR").await;
        let teams_only =
            custom_role(&ctx, &workspace, "Pengatur tim", &[Permission::ManageTeams]).await;
        let cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post("/w/acme/p/KSR/settings/access")
            .add_cookie(cookie.clone())
            .form(&[
                ("subject", format!("user:{}", dev.id)),
                ("role_id", teams_only.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(
            res.text().contains("tidak memberi akses project"),
            "{}",
            res.text()
        );
        assert!(
            project_grants::Model::of_project(&ctx.db, wid, project.id)
                .await
                .unwrap()
                .is_empty()
        );

        let viewer = builtin_role(&ctx, &workspace, RoleKey::Viewer).await;
        let dev_grant = grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::User(dev.id),
            &viewer,
        )
        .await;
        let res = request
            .post(&format!(
                "/w/acme/p/KSR/settings/access/{}/role",
                dev_grant.id
            ))
            .add_cookie(cookie)
            .form(&[("role_id", teams_only.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 422);
        let still = project_grants::Entity::find_by_id(dev_grant.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(still.role_id, viewer.id);
    })
    .await;
}
