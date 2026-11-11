use agentgate_server::{
    app::App,
    models::{job_titles, roles::RoleKey, users, workspace_members, workspaces},
};
use loco_rs::{app::Hooks, testing::prelude::*};
use sea_orm::{EntityTrait, PaginatorTrait};

const EMAILS: [&str; 5] = [
    "owner@agentgate.test",
    "admin@agentgate.test",
    "dev@agentgate.test",
    "qa@agentgate.test",
    "viewer@agentgate.test",
];

#[tokio::test]
async fn seed_creates_verified_users_without_passwords_and_is_repeatable() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let base = std::path::Path::new("src/fixtures");
    App::seed(ctx, base).await.unwrap();
    // Dijalankan lagi tanpa reset: tidak error dan tidak menggandakan data.
    App::seed(ctx, base).await.unwrap();
    for email in EMAILS {
        let user = users::Model::find_by_email(&ctx.db, email)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("{email} tidak ada"));
        assert!(user.email_verified_at.is_some(), "{email}");
        assert!(user.password_hash.is_none(), "{email}");
        assert!(user.is_active(), "{email}");
    }
}

#[tokio::test]
async fn seed_creates_sample_workspace_with_roles_and_job_titles() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let base = std::path::Path::new("src/fixtures");
    App::seed(ctx, base).await.unwrap();
    App::seed(ctx, base).await.unwrap();

    let workspace = workspaces::Model::find_by_slug(&ctx.db, "pt-contoh")
        .await
        .unwrap()
        .expect("workspace seed");
    assert_eq!(workspace.name, "PT Contoh");
    assert_eq!(workspaces::Entity::find().count(&ctx.db).await.unwrap(), 1);
    let wid = workspace.workspace_id();

    let titles = job_titles::Model::in_workspace(&ctx.db, wid).await.unwrap();
    let mut names: Vec<_> = titles.iter().map(|t| t.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Dev", "PM", "QA"]);
    let title_name = |id: Option<uuid::Uuid>| {
        id.and_then(|id| titles.iter().find(|t| t.id == id))
            .map(|t| t.name.clone())
    };

    let expected = [
        ("owner@agentgate.test", RoleKey::Owner, Some("PM")),
        ("admin@agentgate.test", RoleKey::Admin, None),
        ("dev@agentgate.test", RoleKey::Member, Some("Dev")),
        ("qa@agentgate.test", RoleKey::Member, Some("QA")),
        ("viewer@agentgate.test", RoleKey::Viewer, None),
    ];
    for (email, key, title) in expected {
        let user = users::Model::find_by_email(&ctx.db, email)
            .await
            .unwrap()
            .unwrap();
        let (member, role) = workspace_members::Model::find_with_role(&ctx.db, wid, user.id)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("{email} bukan anggota"));
        assert_eq!(role.role_key().unwrap(), Some(key), "{email}");
        assert_eq!(title_name(member.job_title_id).as_deref(), title, "{email}");
    }
    assert_eq!(
        workspace_members::Entity::find()
            .count(&ctx.db)
            .await
            .unwrap(),
        5
    );
}
