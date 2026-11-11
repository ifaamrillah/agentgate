//! Workspace, role bawaan, dan isolasi query per workspace.

use agentgate_server::{
    app::App,
    models::{
        permission::{BUILTIN_ROLES, Permission},
        roles::{self, RoleKey},
        workspace_members,
        workspaces::{self, CreateWorkspace, WorkspaceError},
    },
};
use loco_rs::{model::TenantQueryExt, testing::prelude::*};
use sea_orm::{EntityTrait, Iterable};

use crate::support::{a_user, a_workspace, insert_user};

#[tokio::test]
async fn builtin_roles_created_with_workspace() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.workspace_id();

    let roles = roles::Model::in_workspace(&ctx.db, wid).await.unwrap();
    assert_eq!(roles.len(), 4);
    for builtin in BUILTIN_ROLES {
        let role = roles
            .iter()
            .find(|r| r.role_key().unwrap() == Some(builtin.key))
            .unwrap_or_else(|| panic!("{:?} tidak ada", builtin.key));
        assert!(role.builtin);
        assert_eq!(role.name, builtin.key.default_name());
        let mut got = role.permissions().unwrap();
        got.sort_by_key(|p| p.to_string());
        let mut want = builtin.permissions.to_vec();
        want.sort_by_key(|p| p.to_string());
        assert_eq!(got, want, "{:?}", builtin.key);
    }

    // Owner = semua; Admin = semua kecuali ManageWorkspace; Viewer = ViewProject.
    let by_key = |key| {
        roles
            .iter()
            .find(|r| r.role_key().unwrap() == Some(key))
            .unwrap()
    };
    for p in Permission::iter() {
        assert!(by_key(RoleKey::Owner).has(p), "{p:?}");
        assert_eq!(
            by_key(RoleKey::Admin).has(p),
            p != Permission::ManageWorkspace,
            "{p:?}"
        );
        assert_eq!(
            by_key(RoleKey::Viewer).has(p),
            p == Permission::ViewProject,
            "{p:?}"
        );
    }
    let member = by_key(RoleKey::Member);
    for p in [
        Permission::ViewProject,
        Permission::EditProject,
        Permission::CreateProjects,
    ] {
        assert!(member.has(p));
    }
    assert!(!member.has(Permission::ManageMembers));

    let (membership, role) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(membership.workspace_id, workspace.id);
    assert_eq!(role.role_key().unwrap(), Some(RoleKey::Owner));
}

#[tokio::test]
async fn slug_unique_case_insensitive_at_model_level() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "slug-model@example.com").await;
    let new = |slug: &str| CreateWorkspace {
        name: "Acme".to_string(),
        slug: slug.to_string(),
        created_by: Some(user.id),
    };
    let first = workspaces::ActiveModel::create(db, new("  Acme "))
        .await
        .unwrap();
    assert_eq!(first.slug, "acme", "slug dinormalisasi huruf kecil");
    let err = workspaces::ActiveModel::create(db, new("ACME"))
        .await
        .unwrap_err();
    let WorkspaceError::Invalid(errors) = err else {
        panic!("{err:?}");
    };
    assert_eq!(errors.errors["slug"][0].code, "slug-taken");
    assert_eq!(
        workspaces::Model::find_by_slug(db, "AcMe")
            .await
            .unwrap()
            .map(|w| w.id),
        Some(first.id)
    );
}

#[tokio::test]
async fn tenant_queries_never_cross_workspaces() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let a = a_workspace(&owner).create(ctx).await;
    let b = a_workspace(&owner).create(ctx).await;
    let role_of_b = roles::Model::find_builtin(&ctx.db, b.workspace_id(), RoleKey::Owner)
        .await
        .unwrap()
        .unwrap();
    // Id role milik B tidak ditemukan bila dicari di A, walau lewat primary key.
    assert!(
        roles::Model::find_in(&ctx.db, a.workspace_id(), role_of_b.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        roles::Entity::find_by_id(role_of_b.id)
            .in_tenant(a.workspace_id())
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none()
    );
    // Member tidak bisa diberi role dari workspace lain.
    let other = a_user().create(ctx).await;
    assert!(
        workspace_members::ActiveModel::add(&ctx.db, a.workspace_id(), other.id, role_of_b.id)
            .await
            .is_err()
    );
}
