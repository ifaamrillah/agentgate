use agentgate_server::{
    app::App,
    models::{
        job_titles,
        project_access::{self, Accessor},
        project_grants::{self, GrantSubject},
        projects,
        roles::RoleKey,
        team_members, teams, users, workspace_members, workspaces,
    },
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

#[tokio::test]
async fn seed_creates_sample_project_with_teams_and_grants() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let base = std::path::Path::new("src/fixtures");
    App::seed(ctx, base).await.unwrap();
    App::seed(ctx, base).await.unwrap();

    let workspace = workspaces::Model::find_by_slug(&ctx.db, "pt-contoh")
        .await
        .unwrap()
        .unwrap();
    let wid = workspace.workspace_id();
    let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
        .await
        .unwrap()
        .expect("project seed");
    assert_eq!(project.name, "Kasir Online");
    assert_eq!(projects::Entity::find().count(&ctx.db).await.unwrap(), 1);

    let user = |email: &'static str| async move {
        users::Model::find_by_email(&ctx.db, email)
            .await
            .unwrap()
            .unwrap()
    };
    let all_teams = teams::Model::in_workspace(&ctx.db, wid).await.unwrap();
    let names: Vec<_> = all_teams.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Backend", "QA"]);
    let dev = user("dev@agentgate.test").await;
    let qa = user("qa@agentgate.test").await;
    let viewer = user("viewer@agentgate.test").await;
    for (team, member) in all_teams.iter().zip([&dev, &qa]) {
        let members = team_members::Model::of_team(&ctx.db, wid, team.id)
            .await
            .unwrap();
        assert_eq!(members.len(), 1, "{}", team.name);
        assert_eq!(members[0].1.id, member.id, "{}", team.name);
    }

    let grants = project_grants::Model::of_project(&ctx.db, wid, project.id)
        .await
        .unwrap();
    let subjects: Vec<_> = grants.iter().map(|g| g.subject().unwrap()).collect();
    assert_eq!(subjects.len(), 3);
    for team in &all_teams {
        assert!(
            subjects.contains(&GrantSubject::Team(team.id)),
            "{}",
            team.name
        );
    }
    assert!(subjects.contains(&GrantSubject::User(viewer.id)));

    // Akses efektif: dev dan qa lewat tim (Member), viewer langsung (Viewer).
    for (email, can_edit) in [
        ("dev@agentgate.test", true),
        ("qa@agentgate.test", true),
        ("viewer@agentgate.test", false),
    ] {
        let u = user(email).await;
        let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, u.id)
            .await
            .unwrap()
            .unwrap();
        let who = Accessor {
            workspace_id: wid,
            user_id: u.id,
            role: &role,
        };
        let effective = project_access::effective_role(&ctx.db, &who, project.id)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("{email} tanpa akses"));
        assert_eq!(
            effective.has(agentgate_server::models::permission::Permission::EditProject),
            can_edit,
            "{email}"
        );
    }
}

#[tokio::test]
async fn seed_creates_sample_planning() {
    use agentgate_server::models::{components, releases, sprints, tickets};
    use sea_orm::{ColumnTrait, QueryFilter};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let base = std::path::Path::new("src/fixtures");
    App::seed(ctx, base).await.unwrap();
    App::seed(ctx, base).await.unwrap();
    let workspace = workspaces::Model::find_by_slug(&ctx.db, "pt-contoh")
        .await
        .unwrap()
        .unwrap();
    let wid = workspace.workspace_id();
    let project = projects::Model::find_by_key(&ctx.db, wid, "KSR")
        .await
        .unwrap()
        .unwrap();
    let all = sprints::Model::of_project(&ctx.db, wid, project.id)
        .await
        .unwrap();
    assert_eq!(all.len(), 2, "seed kedua tidak menggandakan sprint");
    let active = all
        .iter()
        .find(|s| s.state() == sprints::SprintState::Active)
        .expect("sprint aktif");
    let in_sprint = tickets::Entity::find()
        .filter(tickets::Column::SprintId.eq(active.id))
        .count(&ctx.db)
        .await
        .unwrap();
    assert_eq!(in_sprint, 5);
    assert_eq!(releases::Entity::find().count(&ctx.db).await.unwrap(), 1);
    assert_eq!(components::Entity::find().count(&ctx.db).await.unwrap(), 2);
}
