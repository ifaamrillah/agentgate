use agentgate_server::{
    app::App,
    common::crypto::sha256_hex,
    models::{
        _entities::user_tokens as entity,
        user_tokens::{self, TokenPurpose},
    },
};
use chrono::Duration;
use loco_rs::testing::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::support::insert_user;

const TTL: Duration = Duration::minutes(30);

#[tokio::test]
async fn reset_token_cannot_be_used_twice() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "twice@example.com").await;

    let raw = user_tokens::issue(db, user.id, TokenPurpose::PasswordReset, TTL)
        .await
        .unwrap();
    assert_eq!(
        user_tokens::consume(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap(),
        Some(user.id)
    );
    assert_eq!(
        user_tokens::consume(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap(),
        None
    );
    assert!(
        !user_tokens::peek(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn expired_token_is_rejected() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "expired@example.com").await;

    let raw = user_tokens::issue(
        db,
        user.id,
        TokenPurpose::PasswordReset,
        Duration::seconds(-1),
    )
    .await
    .unwrap();
    assert!(
        !user_tokens::peek(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap()
    );
    assert_eq!(
        user_tokens::consume(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn new_reset_token_invalidates_previous() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "again@example.com").await;

    let first = user_tokens::issue(db, user.id, TokenPurpose::PasswordReset, TTL)
        .await
        .unwrap();
    let verify = user_tokens::issue(db, user.id, TokenPurpose::EmailVerification, TTL)
        .await
        .unwrap();
    let second = user_tokens::issue(db, user.id, TokenPurpose::PasswordReset, TTL)
        .await
        .unwrap();
    assert_ne!(first, second);

    assert_eq!(
        user_tokens::consume(db, &first, TokenPurpose::PasswordReset)
            .await
            .unwrap(),
        None
    );
    // Token purpose lain milik user yang sama tidak ikut dibatalkan.
    assert!(
        user_tokens::peek(db, &verify, TokenPurpose::EmailVerification)
            .await
            .unwrap()
    );
    assert_eq!(
        user_tokens::consume(db, &second, TokenPurpose::PasswordReset)
            .await
            .unwrap(),
        Some(user.id)
    );
}

#[tokio::test]
async fn peek_does_not_consume_token() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "peek@example.com").await;

    let raw = user_tokens::issue(db, user.id, TokenPurpose::PasswordReset, TTL)
        .await
        .unwrap();
    assert!(
        user_tokens::peek(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap()
    );
    assert!(
        user_tokens::peek(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap()
    );
    assert_eq!(
        user_tokens::consume(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap(),
        Some(user.id)
    );
}

#[tokio::test]
async fn token_is_bound_to_its_purpose_and_stored_as_digest() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "purpose@example.com").await;

    let raw = user_tokens::issue(db, user.id, TokenPurpose::EmailVerification, TTL)
        .await
        .unwrap();
    assert!(
        !user_tokens::peek(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap()
    );
    assert_eq!(
        user_tokens::consume(db, &raw, TokenPurpose::PasswordReset)
            .await
            .unwrap(),
        None
    );

    let rows = entity::Entity::find()
        .filter(entity::Column::UserId.eq(user.id))
        .all(db)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].token_digest, sha256_hex(&raw));
    assert_ne!(rows[0].token_digest, raw);
    assert_eq!(rows[0].id.get_version_num(), 7);

    assert!(
        !user_tokens::peek(db, "garbage", TokenPurpose::EmailVerification)
            .await
            .unwrap()
    );
}
