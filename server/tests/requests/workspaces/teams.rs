//! `/w/{slug}/settings/teams`: tim (`ManageTeams`). Anggota tim harus anggota workspace; hapus
//! tim mencabut akses project yang diberikan lewat tim itu.

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        permission::Permission,
        project_grants::{self, GrantSubject},
        roles::RoleKey,
        team_members, teams, workspace_members,
    },
    services::workspaces::{
        Actor,
        teams::{self as service, TeamFailure},
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::EntityTrait;

use crate::support::{
    a_project, a_user, a_workspace, add_member, builtin_role, custom_role, grant, sign_in_as,
};

#[tokio::test]
async fn team_crud_and_members() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        let dev = a_user().name("Dewi Dev").create(&ctx).await;
        let stranger = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        // Admin memegang ManageTeams.
        let cookie = sign_in_as(&request, &admin).await;

        let index = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await;
        assert_css_exists(
            &index.text(),
            ".app-side a[href=\"/w/acme/settings/teams\"]",
        );
        let page = request
            .get("/w/acme/settings/teams")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(
            &page.text(),
            "form[method=\"post\"][action=\"/w/acme/settings/teams\"] input#team-name",
        );

        let res = request
            .post("/w/acme/settings/teams")
            .add_cookie(cookie.clone())
            .form(&[("name", " Backend ")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let team = teams::Model::in_workspace(&ctx.db, wid)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.name == "Backend")
            .unwrap();
        assert_eq!(
            res.header("location"),
            format!("/w/acme/settings/teams/{}", team.id)
        );
        let res = request
            .post("/w/acme/settings/teams")
            .add_cookie(cookie.clone())
            .form(&[("name", "backend")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#team-name-error");

        let base = format!("/w/acme/settings/teams/{}", team.id);
        let page = request.get(&base).add_cookie(cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        // Pilihan anggota: anggota workspace saja.
        let html = page.text();
        assert_css_exists(
            &html,
            &format!("select[name=\"user_id\"] option[value=\"{}\"]", dev.id),
        );
        assert_css_not_exists(&html, &format!("option[value=\"{}\"]", stranger.id));

        let res = request
            .post(&format!("{base}/members"))
            .add_cookie(cookie.clone())
            .form(&[("user_id", dev.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), base);
        assert!(
            team_members::Model::find(&ctx.db, wid, team.id, dev.id)
                .await
                .unwrap()
                .is_some()
        );
        // Orang yang bukan anggota workspace → 404.
        let res = request
            .post(&format!("{base}/members"))
            .add_cookie(cookie.clone())
            .form(&[("user_id", stranger.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 404);
        let html = request.get(&base).add_cookie(cookie.clone()).await.text();
        assert_css_exists(&html, "[data-team-member]");
        assert!(html.contains("Dewi Dev"));

        // Ganti nama.
        let res = request
            .post(&base)
            .add_cookie(cookie.clone())
            .form(&[("name", "Tim Backend")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        let res = request
            .post(&format!("{base}/members/remove"))
            .add_cookie(cookie.clone())
            .form(&[("user_id", dev.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(
            team_members::Model::find(&ctx.db, wid, team.id, dev.id)
                .await
                .unwrap()
                .is_none()
        );

        let events = crate::support::newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap();
        let actions: Vec<_> = events
            .iter()
            .take(4)
            .map(|e| (e.action().unwrap(), e.subject().unwrap(), e.subject_id))
            .collect();
        assert_eq!(
            actions,
            vec![
                (AuditAction::TeamMemberRemoved, AuditSubject::Team, team.id),
                (AuditAction::TeamUpdated, AuditSubject::Team, team.id),
                (AuditAction::TeamMemberAdded, AuditSubject::Team, team.id),
                (AuditAction::TeamCreated, AuditSubject::Team, team.id),
            ]
        );
        assert_eq!(events[2].changes["user_id"], dev.id.to_string());
        assert_eq!(
            events[1].changes["name"],
            serde_json::json!({ "from": "Backend", "to": "Tim Backend" })
        );
    })
    .await;
}

#[tokio::test]
async fn deleting_team_removes_its_project_grants() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        let project = a_project(&ctx, &workspace, "KSR").await;
        let team = crate::support::a_team(&ctx, &workspace, "QA", &[&dev]).await;
        let role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let team_grant = grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::Team(team.id),
            &role,
        )
        .await;

        let dev_cookie = sign_in_as(&request, &dev).await;
        let res = request
            .get("/w/acme/p/KSR")
            .add_cookie(dev_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);

        let cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post(&format!("/w/acme/settings/teams/{}/delete", team.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/settings/teams");
        assert!(
            teams::Model::find_in(&ctx.db, wid, team.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            project_grants::Entity::find_by_id(team_grant.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
        let res = request.get("/w/acme/p/KSR").add_cookie(dev_cookie).await;
        assert_eq!(res.status_code(), 404);
        let events = crate::support::newest_audit_events(&ctx.db, wid, 1)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::TeamDeleted);
        assert_eq!(
            events[0].changes["grants"],
            serde_json::json!([{ "id": team_grant.id, "project_id": project.id }])
        );

        // Tim yang sudah dihapus atau id tidak valid → 404.
        let res = request
            .get(&format!("/w/acme/settings/teams/{}", team.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .get("/w/acme/settings/teams/bukan-uuid")
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn member_cannot_manage_teams_gets_404() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let team = crate::support::a_team(&ctx, &workspace, "QA", &[]).await;
        let cookie = sign_in_as(&request, &member).await;
        for path in [
            "/w/acme/settings/teams".to_string(),
            format!("/w/acme/settings/teams/{}", team.id),
        ] {
            let res = request.get(&path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "GET {path}");
        }
        for path in [
            "/w/acme/settings/teams".to_string(),
            format!("/w/acme/settings/teams/{}", team.id),
            format!("/w/acme/settings/teams/{}/delete", team.id),
            format!("/w/acme/settings/teams/{}/members", team.id),
            format!("/w/acme/settings/teams/{}/members/remove", team.id),
        ] {
            let res = request.post(&path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "POST {path}");
        }
    })
    .await;
}

/// "Lead": role custom dengan `ManageTeams` + `AdministerProject`, Admin project hanya di X
/// (lewat grant ber-role Lead). Ia tidak bisa masuk (atau memasukkan orang) ke tim yang punya
/// akses di atas aksesnya sendiri; Admin workspace bisa.
#[tokio::test]
async fn team_member_add_cannot_exceed_actor_project_access() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        let lead = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        let lead_role = custom_role(
            &ctx,
            &workspace,
            "Lead",
            &[Permission::ManageTeams, Permission::AdministerProject],
        )
        .await;
        workspace_members::ActiveModel::add(&ctx.db, wid, lead.id, lead_role.id)
            .await
            .unwrap();
        let x = a_project(&ctx, &workspace, "XXX").await;
        let y = a_project(&ctx, &workspace, "YYY").await;
        grant(
            &ctx,
            &workspace,
            &x,
            GrantSubject::User(lead.id),
            &lead_role,
        )
        .await;
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let on_x = crate::support::a_team(&ctx, &workspace, "Tim X", &[]).await;
        grant(
            &ctx,
            &workspace,
            &x,
            GrantSubject::Team(on_x.id),
            &lead_role,
        )
        .await;
        let on_y = crate::support::a_team(&ctx, &workspace, "Tim Y", &[]).await;
        grant(
            &ctx,
            &workspace,
            &y,
            GrantSubject::Team(on_y.id),
            &admin_role,
        )
        .await;
        let lead_on_y = crate::support::a_team(&ctx, &workspace, "Tim Y Lead", &[]).await;
        grant(
            &ctx,
            &workspace,
            &y,
            GrantSubject::Team(lead_on_y.id),
            &lead_role,
        )
        .await;

        let cookie = sign_in_as(&request, &lead).await;
        for team in [&on_y, &lead_on_y] {
            let res = request
                .post(&format!("/w/acme/settings/teams/{}/members", team.id))
                .add_cookie(cookie.clone())
                .form(&[("user_id", lead.id.to_string())])
                .await;
            assert_eq!(res.status_code(), 422, "{}", team.name);
            assert!(res.text().contains("akses lebih tinggi"), "{}", res.text());
            assert!(
                team_members::Model::find(&ctx.db, wid, team.id, lead.id)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        // Tim yang aksesnya tercakup akses Lead: boleh.
        let res = request
            .post(&format!("/w/acme/settings/teams/{}/members", on_x.id))
            .add_cookie(cookie.clone())
            .form(&[("user_id", lead.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        // Admin workspace (Admin di semua project) boleh.
        let admin_cookie = sign_in_as(&request, &admin).await;
        let res = request
            .post(&format!("/w/acme/settings/teams/{}/members", on_y.id))
            .add_cookie(admin_cookie)
            .form(&[("user_id", lead.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(
            team_members::Model::find(&ctx.db, wid, on_y.id, lead.id)
                .await
                .unwrap()
                .is_some()
        );
    })
    .await;
}

/// Izin `ManageTeams` dibaca ulang di dalam transaksi: pelaku yang diturunkan setelah lolos
/// guard controller mendapat `NotFound` untuk setiap perubahan tim.
#[tokio::test]
async fn demoted_actor_cannot_change_teams() {
    request::<App, _, _>(|_request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).create(&ctx).await;
        let wid = workspace.workspace_id();
        let admin_member = add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        let team = crate::support::a_team(&ctx, &workspace, "QA", &[&dev]).await;
        // Diturunkan menjadi Member (tanpa ManageTeams).
        let member_role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        admin_member
            .set_role(&ctx.db, member_role.id)
            .await
            .unwrap();
        let actor = Actor {
            workspace_id: wid,
            user_id: admin.id,
            request_id: None,
        };
        let not_found = |r: Result<(), TeamFailure>| matches!(r, Err(TeamFailure::NotFound));
        assert!(matches!(
            service::create(&ctx.db, &actor, "Baru").await,
            Err(TeamFailure::NotFound)
        ));
        assert!(matches!(
            service::rename(&ctx.db, &actor, team.id, "Lain").await,
            Err(TeamFailure::NotFound)
        ));
        assert!(not_found(
            service::add_member(&ctx.db, &actor, team.id, admin.id).await
        ));
        assert!(not_found(
            service::remove_member(&ctx.db, &actor, team.id, dev.id).await
        ));
        assert!(not_found(service::delete(&ctx.db, &actor, team.id).await));
        assert_eq!(
            teams::Model::in_workspace(&ctx.db, wid)
                .await
                .unwrap()
                .len(),
            1
        );
    })
    .await;
}

/// Dikeluarkan dari tim → request berikutnya ke project yang diakses lewat tim itu 404; audit
/// mencatat grant tim yang tidak lagi menjangkaunya.
#[tokio::test]
async fn removed_team_member_loses_project_access() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let dev = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        add_member(&ctx, &workspace, &dev, RoleKey::Member).await;
        let project = a_project(&ctx, &workspace, "KSR").await;
        let team = crate::support::a_team(&ctx, &workspace, "QA", &[&dev]).await;
        let role = builtin_role(&ctx, &workspace, RoleKey::Member).await;
        let team_grant = grant(
            &ctx,
            &workspace,
            &project,
            GrantSubject::Team(team.id),
            &role,
        )
        .await;

        let dev_cookie = sign_in_as(&request, &dev).await;
        let res = request
            .get("/w/acme/p/KSR")
            .add_cookie(dev_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);

        let cookie = sign_in_as(&request, &owner).await;
        let res = request
            .post(&format!(
                "/w/acme/settings/teams/{}/members/remove",
                team.id
            ))
            .add_cookie(cookie)
            .form(&[("user_id", dev.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = request.get("/w/acme/p/KSR").add_cookie(dev_cookie).await;
        assert_eq!(res.status_code(), 404);

        let events = crate::support::newest_audit_events(&ctx.db, wid, 1)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::TeamMemberRemoved);
        assert_eq!(
            events[0].changes["grants"],
            serde_json::json!([{ "id": team_grant.id, "project_id": project.id }])
        );
    })
    .await;
}
