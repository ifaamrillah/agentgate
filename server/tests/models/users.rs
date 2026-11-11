use agentgate_server::{
    app::App,
    common::settings::{AuthSettings, Settings},
    models::users::{self, NewUser, UserError},
    services::auth::password::{self, PASSWORD_MAX_LEN, PASSWORD_MIN_LEN, Verify},
};
use loco_rs::{Error, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use secrecy::SecretString;

use crate::support::{TEST_PASSWORD, insert_user};

fn auth_settings(ctx: &loco_rs::app::AppContext) -> AuthSettings {
    Settings::from_ctx(ctx).unwrap().auth
}

fn secret(s: &str) -> SecretString {
    SecretString::from(s.to_string())
}

/// Kode error validasi pertama untuk `field`, atau panic bila error-nya bukan validasi.
fn validation_code(err: &Error, field: &str) -> String {
    match err {
        Error::Validation(e) => e.errors[field][0].code.clone(),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn email_is_normalized_and_unique_case_insensitive() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = insert_user(db, "  Ana@Example.COM ").await;
    assert_eq!(user.email, "ana@example.com");

    let found = users::Model::find_by_email(db, " ANA@example.com")
        .await
        .unwrap();
    assert_eq!(found.map(|u| u.id), Some(user.id));
    assert!(
        users::Model::find_by_email(db, "bob@example.com")
            .await
            .unwrap()
            .is_none()
    );

    let dup = users::ActiveModel::create(
        db,
        NewUser {
            email: "ana@EXAMPLE.com".to_string(),
            name: "Ana Lain".to_string(),
            password_hash: None,
        },
    )
    .await;
    assert!(matches!(dup, Err(UserError::EmailTaken)), "{dup:?}");
}

#[tokio::test]
async fn invalid_email_and_name_are_rejected_with_codes() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let res = users::ActiveModel::create(
        db,
        NewUser {
            email: "not-an-email".to_string(),
            name: "  \u{200b} ".to_string(),
            password_hash: None,
        },
    )
    .await;
    let Err(UserError::Invalid(errors)) = res else {
        panic!("expected invalid, got {res:?}")
    };
    assert_eq!(errors.errors["email"][0].code, "email-invalid");
    assert_eq!(errors.errors["name"][0].code, "name-invisible-chars");
}

#[tokio::test]
async fn user_id_is_uuid_v7_and_name_is_trimmed() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = users::ActiveModel::create(
        db,
        NewUser {
            email: "v7@example.com".to_string(),
            name: "  Budi  ".to_string(),
            password_hash: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(user.id.get_version_num(), 7);
    assert_eq!(user.name, "Budi");
}

#[tokio::test]
async fn disabled_user_is_not_active() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = insert_user(db, "off@example.com").await;
    assert!(user.is_active());

    let mut active: users::ActiveModel = user.into();
    active.disabled_at = Set(Some(chrono::Utc::now().into()));
    let user = active.update(db).await.unwrap();
    assert!(!user.is_active());
}

#[tokio::test]
async fn password_shorter_than_12_is_rejected() {
    let boot = boot_test::<App>().await.unwrap();
    let auth = auth_settings(&boot.app_context);

    let short = "a".repeat(PASSWORD_MIN_LEN - 1);
    let err = password::hash(&auth, secret(&short)).await.unwrap_err();
    assert_eq!(validation_code(&err, "password"), "password-too-short");

    let long = "a".repeat(PASSWORD_MAX_LEN + 1);
    let err = password::hash(&auth, secret(&long)).await.unwrap_err();
    assert_eq!(validation_code(&err, "password"), "password-too-long");

    // Batas dihitung dalam karakter, bukan byte: 12 karakter multibyte lolos.
    assert!(
        password::hash(&auth, secret(&"é".repeat(PASSWORD_MIN_LEN)))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn rehash_is_reported_when_params_increase() {
    let boot = boot_test::<App>().await.unwrap();
    let auth = auth_settings(&boot.app_context);

    let hash = password::hash(&auth, secret(TEST_PASSWORD)).await.unwrap();
    assert!(hash.starts_with("$argon2id$"), "{hash}");
    assert_eq!(
        password::verify(&auth, &secret(TEST_PASSWORD), &hash)
            .await
            .unwrap(),
        Verify::Ok
    );
    assert_eq!(
        password::verify(&auth, &secret("wrong password!!"), &hash)
            .await
            .unwrap(),
        Verify::Mismatch
    );

    let mut stronger = auth.clone();
    stronger.argon2_iterations += 1;
    stronger.argon2_memory_kib *= 2;
    assert_eq!(
        password::verify(&stronger, &secret(TEST_PASSWORD), &hash)
            .await
            .unwrap(),
        Verify::OkNeedsRehash
    );
    // Password salah tidak pernah melaporkan rehash.
    assert_eq!(
        password::verify(&stronger, &secret("wrong password!!"), &hash)
            .await
            .unwrap(),
        Verify::Mismatch
    );
    // Hash rusak dianggap tidak cocok, bukan error 500.
    assert_eq!(
        password::verify(&auth, &secret(TEST_PASSWORD), "not-a-phc-string")
            .await
            .unwrap(),
        Verify::Mismatch
    );

    password::verify_dummy(&auth, &secret(TEST_PASSWORD))
        .await
        .unwrap();
}
