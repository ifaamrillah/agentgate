//! `/w/{slug}/p/{key}/settings/repos`: repo project (`AdministerProject`). URL hanya https atau
//! ssh `git@`; nama unik per project tanpa membedakan huruf besar.

use agentgate_server::{
    app::App,
    models::{
        audit_events::{self, AuditAction, AuditSubject},
        repos::{self, RepoProvider},
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::EntityTrait;

use crate::support::{a_project, a_user, a_workspace, sign_in_as};

fn repo_form<'a>(name: &'a str, provider: &'a str, url: &'a str) -> [(&'static str, &'a str); 4] {
    [
        ("name", name),
        ("provider", provider),
        ("url", url),
        ("default_branch", "main"),
    ]
}

#[tokio::test]
async fn repo_url_must_be_https_or_ssh() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let project = a_project(&ctx, &workspace, "KSR").await;
        let cookie = sign_in_as(&request, &owner).await;

        let page = request
            .get("/w/acme/p/KSR/settings/repos")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(
            &page.text(),
            "form[method=\"post\"][action=\"/w/acme/p/KSR/settings/repos\"] input#repo-url",
        );

        for bad in [
            "http://github.com/acme/kasir",
            "ftp://github.com/acme/kasir",
            "https://token@github.com/acme/kasir",
            "github.com/acme/kasir",
        ] {
            let res = request
                .post("/w/acme/p/KSR/settings/repos")
                .add_cookie(cookie.clone())
                .form(&repo_form("kasir-api", "github", bad))
                .await;
            assert_eq!(res.status_code(), 422, "{bad}");
            let html = res.text();
            assert_css_exists(&html, "#repo-url-error");
            assert_css_exists(&html, "input#repo-name[value=\"kasir-api\"]");
        }
        let long = format!("https://github.com/{}", "a".repeat(500));
        let res = request
            .post("/w/acme/p/KSR/settings/repos")
            .add_cookie(cookie.clone())
            .form(&repo_form("kasir-api", "github", &long))
            .await;
        assert_eq!(res.status_code(), 422);
        // Penyedia di luar daftar → 422.
        let res = request
            .post("/w/acme/p/KSR/settings/repos")
            .add_cookie(cookie.clone())
            .form(&repo_form(
                "kasir-api",
                "sourceforge",
                "git@github.com:acme/kasir.git",
            ))
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#repo-provider-error");

        let res = request
            .post("/w/acme/p/KSR/settings/repos")
            .add_cookie(cookie.clone())
            .form(&repo_form(
                "kasir-api",
                "github",
                "https://github.com/acme/kasir-api.git",
            ))
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/p/KSR/settings/repos");
        let res = request
            .post("/w/acme/p/KSR/settings/repos")
            .add_cookie(cookie.clone())
            .form(&repo_form(
                "kasir-web",
                "gitlab",
                "git@gitlab.com:acme/kasir-web.git",
            ))
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        // Nama sama tanpa membedakan huruf besar → 422.
        let res = request
            .post("/w/acme/p/KSR/settings/repos")
            .add_cookie(cookie.clone())
            .form(&repo_form(
                "KASIR-API",
                "github",
                "https://github.com/acme/x",
            ))
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "#repo-name-error");

        let list = repos::Model::of_project(&ctx.db, wid, project.id)
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
        let api = &list[0];
        assert_eq!(api.name, "kasir-api");
        assert_eq!(api.provider().unwrap(), RepoProvider::Github);

        // Ubah, lalu hapus.
        let res = request
            .post(&format!("/w/acme/p/KSR/settings/repos/{}", api.id))
            .add_cookie(cookie.clone())
            .form(&[
                ("name", "kasir-api"),
                ("provider", "github"),
                ("url", "git@github.com:acme/kasir-api.git"),
                ("default_branch", "develop"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let updated = repos::Entity::find_by_id(api.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.default_branch, "develop");
        assert_eq!(updated.url, "git@github.com:acme/kasir-api.git");
        let res = request
            .post(&format!("/w/acme/p/KSR/settings/repos/{}", api.id))
            .add_cookie(cookie.clone())
            .form(&repo_form("kasir-api", "github", "http://x/y"))
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), &format!("#repo-{}-url-error", api.id));

        // Project home menampilkan repo.
        let home = request
            .get("/w/acme/p/KSR")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(&home, "[data-repo]");

        let res = request
            .post(&format!("/w/acme/p/KSR/settings/repos/{}/delete", api.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            repos::Entity::find_by_id(api.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
        // Repo yang sudah dihapus atau milik project lain → 404.
        let res = request
            .post(&format!("/w/acme/p/KSR/settings/repos/{}/delete", api.id))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);

        let events = audit_events::Model::newest_first(&ctx.db, wid, 10)
            .await
            .unwrap();
        let actions: Vec<_> = events
            .iter()
            .take(4)
            .map(|e| (e.action().unwrap(), e.subject().unwrap()))
            .collect();
        assert_eq!(
            actions,
            vec![
                (AuditAction::RepoRemoved, AuditSubject::Repo),
                (AuditAction::RepoUpdated, AuditSubject::Repo),
                (AuditAction::RepoAdded, AuditSubject::Repo),
                (AuditAction::RepoAdded, AuditSubject::Repo),
            ]
        );
        assert_eq!(
            events[1].changes["default_branch"],
            serde_json::json!({ "from": "main", "to": "develop" })
        );
    })
    .await;
}
