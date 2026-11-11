//! `project_access`: Owner/Admin workspace = Admin di semua project; selain itu gabungan izin
//! project dari grant langsung dan grant tim. `projects::accessible_for` memakai aturan yang
//! sama untuk banyak project sekaligus.

use agentgate_server::{
    app::App,
    models::{
        permission::Permission::{self, AdministerProject, EditProject, ManageTeams, ViewProject},
        project_access::{self, Accessor, EffectiveRole},
        project_grants::GrantSubject,
        projects,
        roles::{self, RoleKey},
        users, workspaces,
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ConnectionTrait, DbBackend, Statement, Value};

use crate::support::{
    a_project, a_team, a_user, a_workspace, add_member, builtin_role, custom_role, grant,
};

async fn role_of(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    user: &users::Model,
) -> roles::Model {
    agentgate_server::models::workspace_members::Model::find_with_role(
        &ctx.db,
        workspace.workspace_id(),
        user.id,
    )
    .await
    .unwrap()
    .unwrap()
    .1
}

async fn effective(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    user: &users::Model,
    project: &projects::Model,
) -> Option<EffectiveRole> {
    let role = role_of(ctx, workspace, user).await;
    let who = Accessor {
        workspace_id: workspace.workspace_id(),
        user_id: user.id,
        role: &role,
    };
    project_access::effective_role(&ctx.db, &who, project.id)
        .await
        .unwrap()
}

async fn accessible_keys(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    user: &users::Model,
) -> Vec<String> {
    let role = role_of(ctx, workspace, user).await;
    let who = Accessor {
        workspace_id: workspace.workspace_id(),
        user_id: user.id,
        role: &role,
    };
    projects::Model::accessible_for(&ctx.db, &who)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.key)
        .collect()
}

fn perms(role: &EffectiveRole) -> Vec<Permission> {
    role.permissions().to_vec()
}

#[tokio::test]
async fn workspace_admin_is_admin_on_all_projects() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let admin = a_user().create(ctx).await;
    let member = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    add_member(ctx, &workspace, &admin, RoleKey::Admin).await;
    add_member(ctx, &workspace, &member, RoleKey::Member).await;
    let beta = a_project(ctx, &workspace, "BETA").await;
    let alfa = a_project(ctx, &workspace, "ALFA").await;
    // Project workspace lain tidak pernah ikut.
    let other_ws = a_workspace(&a_user().create(ctx).await).create(ctx).await;
    a_project(ctx, &other_ws, "LAIN").await;

    for user in [&owner, &admin] {
        for project in [&alfa, &beta] {
            let role = effective(ctx, &workspace, user, project).await.unwrap();
            assert_eq!(
                perms(&role),
                vec![ViewProject, EditProject, AdministerProject]
            );
            assert_eq!(role.label(), "Admin");
        }
        assert_eq!(
            accessible_keys(ctx, &workspace, user).await,
            ["ALFA", "BETA"]
        );
    }
    // Member workspace (izin workspace ViewProject/EditProject) tanpa grant: tidak ada akses.
    assert!(effective(ctx, &workspace, &member, &alfa).await.is_none());
    assert!(accessible_keys(ctx, &workspace, &member).await.is_empty());
}

#[tokio::test]
async fn custom_role_with_administer_project_is_not_admin_everywhere() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let lead = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.workspace_id();
    let lead_role = custom_role(ctx, &workspace, "Lead", &[AdministerProject, ManageTeams]).await;
    agentgate_server::models::workspace_members::ActiveModel::add(
        &ctx.db,
        wid,
        lead.id,
        lead_role.id,
    )
    .await
    .unwrap();
    let project = a_project(ctx, &workspace, "KSR").await;
    assert!(effective(ctx, &workspace, &lead, &project).await.is_none());

    // Grant dengan role tanpa izin project tidak memberi akses.
    let teams_only = custom_role(ctx, &workspace, "Pengatur tim", &[ManageTeams]).await;
    grant(
        ctx,
        &workspace,
        &project,
        GrantSubject::User(lead.id),
        &teams_only,
    )
    .await;
    assert!(effective(ctx, &workspace, &lead, &project).await.is_none());
    assert!(accessible_keys(ctx, &workspace, &lead).await.is_empty());
}

#[tokio::test]
async fn user_in_two_teams_gets_union_of_permissions() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let dev = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    add_member(ctx, &workspace, &dev, RoleKey::Viewer).await;
    let project = a_project(ctx, &workspace, "KSR").await;
    let other = a_project(ctx, &workspace, "LAIN").await;

    let viewer = builtin_role(ctx, &workspace, RoleKey::Viewer).await;
    let editor = custom_role(ctx, &workspace, "Editor", &[EditProject]).await;
    let qa = a_team(ctx, &workspace, "QA", &[&dev]).await;
    let backend = a_team(ctx, &workspace, "Backend", &[&dev]).await;
    grant(
        ctx,
        &workspace,
        &project,
        GrantSubject::Team(qa.id),
        &viewer,
    )
    .await;
    let role = effective(ctx, &workspace, &dev, &project).await.unwrap();
    assert_eq!(perms(&role), vec![ViewProject]);

    grant(
        ctx,
        &workspace,
        &project,
        GrantSubject::Team(backend.id),
        &editor,
    )
    .await;
    let role = effective(ctx, &workspace, &dev, &project).await.unwrap();
    // EditProject mencakup ViewProject; gabungan dari dua tim.
    assert_eq!(perms(&role), vec![ViewProject, EditProject]);
    assert!(role.has(EditProject));
    assert!(!role.has(AdministerProject));
    assert_eq!(role.label(), "Editor, Viewer");

    // Grant langsung menambah izin di atas grant tim.
    let admin_role = builtin_role(ctx, &workspace, RoleKey::Admin).await;
    grant(
        ctx,
        &workspace,
        &project,
        GrantSubject::User(dev.id),
        &admin_role,
    )
    .await;
    let role = effective(ctx, &workspace, &dev, &project).await.unwrap();
    assert!(role.has(AdministerProject));

    // Project tanpa grant untuknya tetap tertutup, dan hanya KSR yang terdaftar.
    assert!(effective(ctx, &workspace, &dev, &other).await.is_none());
    assert_eq!(accessible_keys(ctx, &workspace, &dev).await, ["KSR"]);
}

#[tokio::test]
async fn grant_has_exactly_one_subject() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let project = a_project(ctx, &workspace, "KSR").await;
    let team = a_team(ctx, &workspace, "QA", &[&owner]).await;
    let role = builtin_role(ctx, &workspace, RoleKey::Viewer).await;
    let insert = |team_id: Option<uuid::Uuid>, user_id: Option<uuid::Uuid>| {
        let db = &ctx.db;
        let (wid, pid, rid) = (workspace.id, project.id, role.id);
        async move {
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO project_grants (workspace_id, project_id, team_id, user_id, role_id) \
                 VALUES ($1, $2, $3, $4, $5)",
                [
                    Value::from(wid),
                    Value::from(pid),
                    Value::from(team_id),
                    Value::from(user_id),
                    Value::from(rid),
                ],
            ))
            .await
            .is_ok()
        }
    };
    assert!(!insert(None, None).await, "tanpa subjek");
    assert!(!insert(Some(team.id), Some(owner.id)).await, "dua subjek");
    // User yang bukan anggota workspace ditolak FK ke workspace_members.
    let stranger = a_user().create(ctx).await;
    assert!(!insert(None, Some(stranger.id)).await, "bukan anggota");
    assert!(insert(Some(team.id), None).await);
    assert!(!insert(Some(team.id), None).await, "grant tim ganda");
    assert!(insert(None, Some(owner.id)).await);
    assert!(!insert(None, Some(owner.id)).await, "grant orang ganda");
}
