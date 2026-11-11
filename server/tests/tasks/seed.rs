use agentgate_server::{app::App, models::users};
use loco_rs::{app::Hooks, testing::prelude::*};

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
