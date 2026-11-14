//! R2 lewat provider git (GitHub palsu, koneksi PAT): git tag saat "Rilis sekarang" dan PR
//! knowledge setelah usulan di-approve.

use agentgate_server::models::releases;
use loco_rs::{TestServer, app::AppContext};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};

use super::tickets::{Fixture, fixture};
use crate::support::{
    fake_github::{self, FakeGithub, TOKEN},
    request_with_settings,
};

const REPO: &str = "pt-contoh/kasir-api";

fn settings_for(base: String) -> impl FnOnce(&mut Value) {
    move |s| {
        s["git"] =
            json!({ "github_api_base": base, "github_web_base": base, "timeout_seconds": 5 });
    }
}

pub(super) async fn add_repo(request: &TestServer, f: &Fixture) {
    let res = request
        .post("/w/acme/p/KSR/settings/repos")
        .add_cookie(f.cookie.clone())
        .form(&[
            ("name", "kasir-api"),
            ("provider", "github"),
            ("url", "https://github.com/pt-contoh/kasir-api"),
            ("default_branch", "main"),
        ])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

pub(super) async fn connect(request: &TestServer, f: &Fixture, gh: &FakeGithub) {
    gh.add_repo(REPO);
    let res = request
        .post("/w/acme/settings/git/connect")
        .add_cookie(f.cookie.clone())
        .form(&[("kind", "github"), ("token", TOKEN)])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

async fn new_release(
    request: &TestServer,
    ctx: &AppContext,
    f: &Fixture,
    name: &str,
) -> releases::Model {
    let res = request
        .post("/w/acme/p/KSR/releases")
        .add_cookie(f.cookie.clone())
        .form(&[("name", name)])
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    releases::Entity::find()
        .filter(releases::Column::WorkspaceId.eq(f.workspace.id))
        .filter(releases::Column::Name.eq(name))
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("rilis ada")
}

async fn reload(ctx: &AppContext, r: &releases::Model) -> releases::Model {
    releases::Entity::find_by_id(r.id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn release_now_tags_head_of_each_linked_repo() {
    let gh = fake_github::start().await;
    request_with_settings(
        settings_for(gh.base_url.clone()),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            add_repo(&request, &f).await;

            // Provider belum tertaut: rilis tetap jalan, tag tidak dibuat (peringatan).
            let r = new_release(&request, &ctx, &f, "v1.3").await;
            let show = format!("/w/acme/p/KSR/releases/{}", r.id);
            let page = request.get(&show).add_cookie(f.cookie.clone()).await.text();
            assert!(page.contains("data-tag-unlinked"), "{page}");
            let res = request
                .post(&format!("{show}/release"))
                .add_cookie(f.cookie.clone())
                .form(&[("make_tag", "1"), ("tag", "v1.3"), ("tag_target", "head")])
                .await;
            assert_eq!(res.status_code(), 303);
            let r = reload(&ctx, &r).await;
            assert_eq!(r.status, "released");
            assert_eq!(r.git_tags[0]["error"], "git-not-connected");
            let page = request.get(&show).add_cookie(f.cookie.clone()).await.text();
            assert!(page.contains("data-flash=\"warn\""), "{page}");
            assert!(page.contains("data-tag-error=\"git-not-connected\""));

            // Tertaut: tag di HEAD branch utama.
            connect(&request, &f, &gh).await;
            let r = new_release(&request, &ctx, &f, "Rilis Lebaran").await;
            let show = format!("/w/acme/p/KSR/releases/{}", r.id);
            let page = request.get(&show).add_cookie(f.cookie.clone()).await.text();
            assert!(
                page.contains("value=\"Rilis-Lebaran\""),
                "nama tag default dari nama rilis"
            );
            // SHA commit tidak valid → ditolak sebelum rilis.
            let res = request
                .post(&format!("{show}/release"))
                .add_cookie(f.cookie.clone())
                .form(&[
                    ("make_tag", "1"),
                    ("tag", "v1.4"),
                    ("tag_target", "commit"),
                    ("tag_sha", "xyz"),
                ])
                .await;
            assert_eq!(res.status_code(), 303);
            assert_ne!(reload(&ctx, &r).await.status, "released");
            let res = request
                .post(&format!("{show}/release"))
                .add_cookie(f.cookie.clone())
                .form(&[("make_tag", "1"), ("tag", "v1.4"), ("tag_target", "head")])
                .await;
            assert_eq!(res.status_code(), 303);
            let refs = gh.refs();
            assert_eq!(refs.len(), 1, "{refs:?}");
            assert_eq!(refs[0].0, REPO);
            assert_eq!(refs[0].1, "refs/tags/v1.4");
            assert_eq!(refs[0].2, "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678");
            let r = reload(&ctx, &r).await;
            assert_eq!(r.git_tags[0]["error"], "");
            let page = request.get(&show).add_cookie(f.cookie.clone()).await.text();
            assert!(page.contains("data-release-tags"));
            assert!(page.contains("/pt-contoh/kasir-api/releases/tag/v1.4"));

            // Commit tertentu; tag yang sudah ada dilaporkan per repo.
            let r = new_release(&request, &ctx, &f, "v1.5").await;
            let show = format!("/w/acme/p/KSR/releases/{}", r.id);
            request
                .post(&format!("{show}/release"))
                .add_cookie(f.cookie.clone())
                .form(&[
                    ("make_tag", "1"),
                    ("tag", "v1.4"),
                    ("tag_target", "commit"),
                    ("tag_sha", "abcdef1"),
                ])
                .await;
            let r = reload(&ctx, &r).await;
            assert_eq!(r.status, "released");
            assert_eq!(r.git_tags[0]["error"], "git-tag-exists");
            assert_eq!(r.git_tags[0]["sha"], "abcdef1");
        },
    )
    .await;
}
