//! `/w/{slug}/settings/teams`: tim (`ManageTeams`). Anggota tim harus anggota workspace; hapus
//! tim mencabut akses project yang diberikan lewat tim itu.

use agentgate_server::{
    app::App,
    models::{
        audit_events::{self, AuditAction, AuditSubject},
        project_grants::{self, GrantSubject},
        roles::RoleKey,
        team_members, teams,
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::EntityTrait;

use crate::support::{a_project, a_user, a_workspace, add_member, builtin_role, grant, sign_in_as};

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
            "[data-settings-index] a[href=\"/w/acme/settings/teams\"]",
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

        let events = audit_events::Model::newest_first(&ctx.db, wid, 10)
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
        let events = audit_events::Model::newest_first(&ctx.db, wid, 1)
            .await
            .unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::TeamDeleted);

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
