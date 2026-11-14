//! Migrasi R4: kunci client `gemini` (Gemini CLI) dipetakan ke `antigravity` (Antigravity CLI).

use agentgate_server::app::App;
use loco_rs::testing::prelude::*;
use migration::m20261129_000001_r4_antigravity::{down_sql, up_sql};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value};
use uuid::Uuid;

use crate::support::{a_user, a_workspace};

async fn run(db: &DatabaseConnection, sql: &[String]) {
    for s in sql {
        db.execute_unprepared(s).await.unwrap();
    }
}

async fn text(db: &DatabaseConnection, sql: &str, id: Uuid) -> String {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [Value::from(id)],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get::<String>("", "v")
    .unwrap()
}

async fn persona(db: &DatabaseConnection, wid: Uuid, key: &str, models: &str) -> Uuid {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO personas (workspace_id, key, name, color, models) \
         VALUES ($1, $2, $2, 'blue', $3::jsonb) RETURNING id",
        [Value::from(wid), Value::from(key), Value::from(models)],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "id")
    .unwrap()
}

#[tokio::test]
async fn gemini_client_keys_become_antigravity() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.id;
    let runner: Uuid = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO runners (workspace_id, user_id, name, client, public_key) \
             VALUES ($1, $2, 'laptop', 'gemini', 'k') RETURNING id",
            [Value::from(wid), Value::from(owner.id)],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO runner_prefs (workspace_id, user_id, default_client) \
         VALUES ($1, $2, 'gemini') \
         ON CONFLICT (workspace_id, user_id) DO UPDATE SET default_client = 'gemini'",
        [Value::from(wid), Value::from(owner.id)],
    ))
    .await
    .unwrap();
    let old = persona(
        db,
        wid,
        "r4-lama",
        r#"{"gemini": {"model": "gemini-2.5-pro"}, "claude": {"model": "claude-x"}}"#,
    )
    .await;
    let typed = persona(
        db,
        wid,
        "r4-ketik",
        r#"{"gemini": {"model": "gemini-3.1-pro-high"}}"#,
    )
    .await;
    let both = persona(
        db,
        wid,
        "r4-dua",
        r#"{"gemini": {"model": "g"}, "antigravity": {"effort": "high"}}"#,
    )
    .await;

    // Dua kali: pernyataan idempoten.
    run(db, &up_sql()).await;
    run(db, &up_sql()).await;
    let models = "SELECT models::text AS v FROM personas WHERE id = $1";
    assert_eq!(
        text(db, "SELECT client AS v FROM runners WHERE id = $1", runner).await,
        "antigravity"
    );
    assert_eq!(
        text(
            db,
            "SELECT default_client AS v FROM runner_prefs WHERE workspace_id = $1",
            wid
        )
        .await,
        "antigravity"
    );
    // Model katalog Gemini CLI lama dibuang (entri kosong ikut dibuang); client lain utuh.
    assert_eq!(
        text(db, models, old).await,
        r#"{"claude": {"model": "claude-x"}}"#
    );
    assert_eq!(
        text(db, models, typed).await,
        r#"{"antigravity": {"model": "gemini-3.1-pro-high"}}"#
    );
    assert_eq!(
        text(db, models, both).await,
        r#"{"antigravity": {"effort": "high"}}"#
    );

    run(db, &down_sql()).await;
    assert_eq!(
        text(db, "SELECT client AS v FROM runners WHERE id = $1", runner).await,
        "gemini"
    );
    assert_eq!(
        text(db, models, typed).await,
        r#"{"gemini": {"model": "gemini-3.1-pro-high"}}"#
    );
    // Effort tidak ada di Gemini CLI: entri yang hanya berisi effort hilang.
    assert_eq!(text(db, models, both).await, "{}");
}
