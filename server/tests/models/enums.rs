//! Setiap enum database: semua varian `EnumIter` tersimpan, literal tak dikenal ditolak CHECK
//! constraint lewat raw SQL (rules §4).

use agentgate_server::{
    app::App,
    models::{
        audit_events::AuditAction, identities::IdentityProvider, roles::RoleKey,
        user_tokens::TokenPurpose,
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveEnum, ConnectionTrait, DbBackend, Iterable, Statement, Value};
use uuid::Uuid;

use crate::support::insert_user;

async fn insert_token(db: &sea_orm::DatabaseConnection, user_id: Uuid, purpose: &str) -> bool {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO user_tokens (user_id, purpose, token_digest, expires_at) \
         VALUES ($1, $2, $3, now())",
        [
            Value::from(user_id),
            Value::from(purpose),
            Value::from(Uuid::now_v7().to_string()),
        ],
    ))
    .await
    .is_ok()
}

async fn insert_identity(db: &sea_orm::DatabaseConnection, user_id: Uuid, provider: &str) -> bool {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO identities (user_id, provider, provider_uid) VALUES ($1, $2, $3)",
        [
            Value::from(user_id),
            Value::from(provider),
            Value::from(Uuid::now_v7().to_string()),
        ],
    ))
    .await
    .is_ok()
}

#[tokio::test]
async fn token_purpose_check_matches_enum() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "enum-token@example.com").await;
    for purpose in TokenPurpose::iter() {
        assert!(
            insert_token(db, user.id, &purpose.to_value()).await,
            "{purpose:?}"
        );
    }
    assert!(!insert_token(db, user.id, "bogus").await);
}

#[tokio::test]
async fn identity_provider_check_matches_enum() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "enum-identity@example.com").await;
    for provider in IdentityProvider::iter() {
        assert!(
            insert_identity(db, user.id, &provider.to_value()).await,
            "{provider:?}"
        );
    }
    assert!(!insert_identity(db, user.id, "bogus").await);
}

async fn insert_role_key(db: &sea_orm::DatabaseConnection, workspace_id: Uuid, key: &str) -> bool {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO roles (workspace_id, key, name) VALUES ($1, $2, $3)",
        [
            Value::from(workspace_id),
            Value::from(key),
            Value::from(Uuid::now_v7().to_string()),
        ],
    ))
    .await
    .is_ok()
}

async fn insert_audit(db: &sea_orm::DatabaseConnection, workspace_id: Uuid, action: &str) -> bool {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO audit_events (workspace_id, action, subject_type, subject_id) \
         VALUES ($1, $2, 'workspace', $1)",
        [Value::from(workspace_id), Value::from(action)],
    ))
    .await
    .is_ok()
}

/// Workspace lewat raw SQL, tanpa role bawaan, supaya setiap key bisa dipakai sekali.
async fn bare_workspace(db: &sea_orm::DatabaseConnection) -> Uuid {
    let id = Uuid::now_v7();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO workspaces (id, name, slug) VALUES ($1, 'Enum', $2)",
        [
            Value::from(id),
            Value::from(format!("enum-{}", id.simple())),
        ],
    ))
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn role_key_check_matches_enum() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let workspace_id = bare_workspace(db).await;
    for key in RoleKey::iter() {
        assert!(
            insert_role_key(db, workspace_id, &key.to_value()).await,
            "{key:?}"
        );
    }
    assert!(!insert_role_key(db, workspace_id, "bogus").await);
}

#[tokio::test]
async fn audit_action_check_matches_enum() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let workspace_id = bare_workspace(db).await;
    for action in AuditAction::iter() {
        assert!(
            insert_audit(db, workspace_id, &action.to_value()).await,
            "{action:?}"
        );
    }
    assert!(!insert_audit(db, workspace_id, "workspace.bogus").await);
}
