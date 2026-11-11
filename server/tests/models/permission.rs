//! `roles.permissions` (`text[]`): setiap varian `Permission` tersimpan dan terbaca kembali,
//! dan elemen tak dikenal ditolak CHECK `permissions <@ ARRAY[...]` lewat raw SQL (rules §4).

use agentgate_server::{
    app::App,
    models::{permission::Permission, roles},
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveEnum, ConnectionTrait, DbBackend, Iterable, Statement, Value};

use crate::support::{a_user, a_workspace};

async fn insert_role(
    db: &sea_orm::DatabaseConnection,
    workspace_id: uuid::Uuid,
    name: &str,
    permissions: Vec<String>,
) -> bool {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO roles (workspace_id, name, permissions) VALUES ($1, $2, $3)",
        [
            Value::from(workspace_id),
            Value::from(name),
            Value::from(permissions),
        ],
    ))
    .await
    .is_ok()
}

#[tokio::test]
async fn permission_strings_roundtrip_and_unknown_rejected() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.workspace_id();

    // Lewat raw SQL: setiap literal varian lolos CHECK, satu per satu dan sekaligus.
    for p in Permission::iter() {
        assert!(
            insert_role(
                &ctx.db,
                workspace.id,
                &format!("raw {p:?}"),
                vec![p.to_value()]
            )
            .await,
            "{p:?}"
        );
    }
    let all: Vec<String> = Permission::iter().map(|p| p.to_value()).collect();
    assert!(insert_role(&ctx.db, workspace.id, "raw semua", all).await);
    assert!(
        !insert_role(
            &ctx.db,
            workspace.id,
            "raw bogus",
            vec!["bogus".to_string()]
        )
        .await
    );
    assert!(
        !insert_role(
            &ctx.db,
            workspace.id,
            "raw campur",
            vec!["view_project".to_string(), "bogus".to_string()]
        )
        .await
    );

    // Lewat model: Vec<Permission> tersimpan dan terbaca kembali sama.
    let saved =
        roles::ActiveModel::create_custom(&ctx.db, wid, "Semua Izin", Permission::iter().collect())
            .await
            .unwrap();
    let loaded = roles::Model::find_in(&ctx.db, wid, saved.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded.permissions().unwrap(),
        Permission::iter().collect::<Vec<_>>()
    );
    assert!(!loaded.builtin);
    assert_eq!(loaded.role_key().unwrap(), None);
}
