//! R2 lewat provider git (GitHub palsu, koneksi PAT): git tag saat "Rilis sekarang" dan PR
//! knowledge setelah usulan di-approve.

use agentgate_server::models::{knowledge_prs, releases};
use loco_rs::{TestServer, app::AppContext};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Value, json};

use super::tickets::{Fixture, fixture};
use crate::support::{
    fake_github::{self, FakeGithub, TOKEN},
    oauth_flow, request_with_settings,
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

async fn propose(request: &TestServer, mcp: &str, body: &str, slug: Option<&str>) -> String {
    let mut args = json!({
        "action": "propose", "project": "KSR", "title": "Alur Refund",
        "type": "overview", "body": body, "reason": "Dari tiket refund",
    });
    if let Some(s) = slug {
        args["slug"] = json!(s);
    }
    let (err, value) = super::mcp::call(request, mcp, "ag_knowledge", args).await;
    assert!(!err, "{value}");
    value["proposal"].as_str().unwrap().to_string()
}

async fn approve(request: &TestServer, f: &Fixture, id: &str) {
    let res = request
        .post(&format!("/w/acme/p/KSR/knowledge/proposals/{id}/approve"))
        .add_cookie(f.cookie.clone())
        .form(&Vec::<(&str, &str)>::new())
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
}

async fn kb_prs(ctx: &AppContext, f: &Fixture) -> Vec<knowledge_prs::Model> {
    knowledge_prs::Entity::find()
        .filter(knowledge_prs::Column::WorkspaceId.eq(f.workspace.id))
        .order_by_asc(knowledge_prs::Column::Version)
        .all(&ctx.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn approved_knowledge_opens_a_pr_on_an_agentgate_branch() {
    let gh = fake_github::start().await;
    request_with_settings(
        settings_for(gh.base_url.clone()),
        |request, ctx| async move {
            let f = fixture(&request, &ctx).await;
            let (_, mcp, _) =
                oauth_flow::connect(&request, &f.cookie, &f.workspace.id.to_string()).await;

            // Tanpa repo tertaut: dicatat `skipped`, tidak ada panggilan provider.
            let id = propose(&request, &mcp, "# Refund\n\nSupervisor menyetujui.", None).await;
            approve(&request, &f, &id).await;
            let rows = kb_prs(&ctx, &f).await;
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].status, "skipped");
            assert!(gh.files().is_empty());

            add_repo(&request, &f).await;
            connect(&request, &f, &gh).await;

            // v2 → branch baru dari HEAD main, file ditulis di branch itu, PR dibuka.
            let id = propose(
                &request,
                &mcp,
                "# Refund\n\nManajer menyetujui.",
                Some("alur-refund"),
            )
            .await;
            approve(&request, &f, &id).await;
            let rows = kb_prs(&ctx, &f).await;
            assert_eq!(rows[1].status, "open", "{:?}", rows[1]);
            let branch = "agentgate/knowledge-alur-refund-v2";
            assert_eq!(rows[1].branch, branch);
            let refs = gh.refs();
            assert_eq!(refs.len(), 1);
            assert_eq!(refs[0].1, format!("refs/heads/{branch}"));
            let files = gh.files();
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].1, branch);
            assert_eq!(files[0].2, ".agentgate/knowledge/alur-refund.md");
            assert!(files[0].3.contains("Manajer menyetujui."));
            let pulls = gh.pulls();
            assert_eq!(pulls.len(), 1);
            assert_eq!(pulls[0].head, branch);
            assert_eq!(pulls[0].base, "main");

            // v3 saat PR masih terbuka → commit baru di branch yang sama, tanpa PR baru.
            let id = propose(
                &request,
                &mcp,
                "# Refund\n\nManajer toko menyetujui.",
                Some("alur-refund"),
            )
            .await;
            approve(&request, &f, &id).await;
            let rows = kb_prs(&ctx, &f).await;
            assert_eq!(rows[2].status, "open", "{:?}", rows[2]);
            assert_eq!(rows[2].branch, branch);
            assert_eq!(rows[2].pull_request_id, rows[1].pull_request_id);
            assert_eq!(gh.refs().len(), 1);
            assert_eq!(gh.pulls().len(), 1);
            let files = gh.files();
            assert_eq!(files.len(), 2);
            assert!(
                files.iter().all(|f| f.1 != "main"),
                "tidak pernah menulis branch utama"
            );

            // Halaman dokumen menampilkan kartu PR.
            let page = request
                .get("/w/acme/p/KSR/knowledge/alur-refund")
                .add_cookie(f.cookie.clone())
                .await
                .text();
            assert!(page.contains("data-kb-repo-prs"), "{page}");
            assert!(page.contains("data-kb-pr=\"open\""));
            assert!(page.contains("data-kb-pr=\"skipped\""));
        },
    )
    .await;
}
