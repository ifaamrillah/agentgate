use agentgate_server::{
    app::App,
    services::auth::dev_login,
    tasks::dev_login::{DevLogin, issue_link},
};
use loco_rs::{
    prelude::*,
    task::{Task, Vars},
};

use crate::support::a_user;

#[tokio::test]
async fn dev_login_task_refuses_outside_development() {
    let boot = boot_test::<App>().await.unwrap();
    let user = a_user().create(&boot.app_context).await;
    let vars = Vars::from_cli_args(vec![("email".into(), user.email.clone())]);
    let err = DevLogin.run(&boot.app_context, &vars).await.unwrap_err();
    assert!(err.to_string().contains("development"), "{err}");
}

#[tokio::test]
async fn issued_link_carries_a_verifiable_token_for_the_user() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let user = a_user().create(ctx).await;
    let link = issue_link(
        &ctx.db,
        "http://localhost:5150",
        "s3cret",
        &user.email,
        5_000,
    )
    .await
    .unwrap();
    let token = link
        .strip_prefix("http://localhost:5150/_dev/login?token=")
        .unwrap();
    assert_eq!(dev_login::verify("s3cret", token, 5_000), Some(user.id));
}

#[tokio::test]
async fn issued_link_refuses_unknown_and_disabled_users() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let disabled = a_user().disabled().create(ctx).await;
    for email in ["nobody@example.com", disabled.email.as_str()] {
        assert!(
            issue_link(&ctx.db, "http://localhost:5150", "s3cret", email, 0)
                .await
                .is_err(),
            "{email}"
        );
    }
}
