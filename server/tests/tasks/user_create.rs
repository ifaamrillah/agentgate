use agentgate_server::{app::App, models::users, tasks::user_create::UserCreate};
use loco_rs::{
    task::{Task, Vars},
    testing::prelude::*,
};

fn vars(email: &str, name: &str) -> Vars {
    Vars::from_cli_args(vec![
        ("email".into(), email.into()),
        ("name".into(), name.into()),
    ])
}

#[tokio::test]
async fn user_create_task_creates_verified_user() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    UserCreate
        .run(ctx, &vars("  New.Dev@Example.com ", "Dev Baru"))
        .await
        .unwrap();
    let user = users::Model::find_by_email(&ctx.db, "new.dev@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.name, "Dev Baru");
    assert!(user.email_verified_at.is_some());
    assert!(user.password_hash.is_none());
}

#[tokio::test]
async fn user_create_task_fails_for_taken_or_invalid_input() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    UserCreate
        .run(ctx, &vars("a@example.com", "Ana"))
        .await
        .unwrap();
    assert!(
        UserCreate
            .run(ctx, &vars("A@example.com", "Ana"))
            .await
            .is_err()
    );
    assert!(
        UserCreate
            .run(ctx, &vars("not-an-email", "Ana"))
            .await
            .is_err()
    );
    assert!(
        UserCreate
            .run(ctx, &Vars::from_cli_args(vec![]))
            .await
            .is_err()
    );
}
