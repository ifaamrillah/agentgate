//! `/w/{slug}/settings/job-titles`: jabatan tim (`ManageMembers`). Hapus jabatan membuat member
//! yang memakainya tanpa jabatan (FK `ON DELETE SET NULL`).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{self, AuditAction, AuditSubject},
        job_titles,
        roles::RoleKey,
        workspace_members,
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::EntityTrait;

use crate::support::{a_user, a_workspace, add_member, sign_in_as};

#[tokio::test]
async fn job_title_crud_and_delete_unassigns_members() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let admin = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        // Admin memegang ManageMembers: boleh mengelola jabatan.
        let cookie = sign_in_as(&request, &admin).await;

        let page = request
            .get("/w/acme/settings/job-titles")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(
            &page.text(),
            "form[method=\"post\"][action=\"/w/acme/settings/job-titles\"] input#job-title-name",
        );

        let res = request
            .post("/w/acme/settings/job-titles")
            .add_cookie(cookie.clone())
            .form(&[("name", " PM ")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/settings/job-titles");
        let title = job_titles::Model::in_workspace(&ctx.db, wid)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.name == "PM")
            .unwrap();

        // Nama sama tanpa membedakan huruf besar → 422, input terisi kembali.
        let res = request
            .post("/w/acme/settings/job-titles")
            .add_cookie(cookie.clone())
            .form(&[("name", "pm")])
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert_css_exists(&html, "#job-title-name-error");
        assert_css_exists(&html, "input#job-title-name[value=\"pm\"]");
        assert!(html.contains("sudah dipakai"));

        // Ganti nama.
        let res = request
            .post(&format!("/w/acme/settings/job-titles/{}", title.id))
            .add_cookie(cookie.clone())
            .form(&[("name", "Product Manager")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let renamed = job_titles::Entity::find_by_id(title.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(renamed.name, "Product Manager");
        // Nama kosong saat ganti nama → 422 di baris itu.
        let res = request
            .post(&format!("/w/acme/settings/job-titles/{}", title.id))
            .add_cookie(cookie.clone())
            .form(&[("name", " ")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), &format!("#job-title-{}-error", title.id));

        // Member memakai jabatan itu, lalu jabatan dihapus.
        let member = workspace_members::Model::find_with_role(&ctx.db, wid, admin.id)
            .await
            .unwrap()
            .unwrap()
            .0
            .set_job_title(&ctx.db, Some(title.id))
            .await
            .unwrap();
        assert_eq!(member.job_title_id, Some(title.id));
        let res = request
            .post(&format!("/w/acme/settings/job-titles/{}/delete", title.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(
            job_titles::Entity::find_by_id(title.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
        let member = workspace_members::Entity::find_by_id(member.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(member.job_title_id, None);
        assert_eq!(member.workspace_id, wid.get());

        let events = audit_events::Model::newest_first(&ctx.db, wid, 10)
            .await
            .unwrap();
        let actions: Vec<_> = events
            .iter()
            .take(3)
            .map(|e| (e.action().unwrap(), e.subject().unwrap(), e.subject_id))
            .collect();
        assert_eq!(
            actions,
            vec![
                (
                    AuditAction::JobTitleDeleted,
                    AuditSubject::JobTitle,
                    title.id
                ),
                (
                    AuditAction::JobTitleUpdated,
                    AuditSubject::JobTitle,
                    title.id
                ),
                (
                    AuditAction::JobTitleCreated,
                    AuditSubject::JobTitle,
                    title.id
                ),
            ]
        );
        assert_eq!(
            events[1].changes,
            serde_json::json!({ "name": { "from": "PM", "to": "Product Manager" } })
        );
        assert!(events.iter().take(3).all(|e| e.actor_id == Some(admin.id)));
    })
    .await;
}

#[tokio::test]
async fn job_titles_need_manage_members_and_own_workspace() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let title = job_titles::ActiveModel::create(&ctx.db, workspace.workspace_id(), "QA")
            .await
            .unwrap();
        let rename = format!("/w/acme/settings/job-titles/{}", title.id);
        let delete = format!("{rename}/delete");

        let member = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &member).await;
        let res = request
            .get("/w/acme/settings/job-titles")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        for path in ["/w/acme/settings/job-titles", rename.as_str()] {
            let res = request
                .post(path)
                .add_cookie(cookie.clone())
                .form(&[("name", "")])
                .await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request.post(&delete).add_cookie(cookie).await;
        assert_eq!(res.status_code(), 404);

        // Jabatan workspace lain → 404.
        let other = a_user().create(&ctx).await;
        a_workspace(&other).slug("lain").create(&ctx).await;
        let cookie = sign_in_as(&request, &other).await;
        let res = request
            .post(&format!("/w/lain/settings/job-titles/{}", title.id))
            .add_cookie(cookie.clone())
            .form(&[("name", "Dibajak")])
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post(&format!("/w/lain/settings/job-titles/{}/delete", title.id))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);
        let unchanged = job_titles::Entity::find_by_id(title.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.name, "QA");
    })
    .await;
}
