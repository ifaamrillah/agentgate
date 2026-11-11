//! Setiap enum database: semua varian `EnumIter` tersimpan, literal tak dikenal ditolak CHECK
//! constraint lewat raw SQL (rules §4).

use agentgate_server::{
    app::App,
    models::{identities::IdentityProvider, user_tokens::TokenPurpose},
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
