//! Setiap enum database: semua varian `EnumIter` tersimpan, literal tak dikenal ditolak CHECK
//! constraint lewat raw SQL (rules §4).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        identities::IdentityProvider,
        repos::RepoProvider,
        roles::RoleKey,
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
        "INSERT INTO roles (workspace_id, key, name, builtin) VALUES ($1, $2, $3, true)",
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
    insert_audit_row(db, workspace_id, action, "workspace").await
}

async fn insert_audit_row(
    db: &sea_orm::DatabaseConnection,
    workspace_id: Uuid,
    action: &str,
    subject_type: &str,
) -> bool {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO audit_events (workspace_id, action, subject_type, subject_id) \
         VALUES ($1, $2, $3, $1)",
        [
            Value::from(workspace_id),
            Value::from(action),
            Value::from(subject_type),
        ],
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

#[tokio::test]
async fn audit_subject_check_matches_enum() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let workspace_id = bare_workspace(db).await;
    for subject in AuditSubject::iter() {
        assert!(
            insert_audit_row(db, workspace_id, "workspace.updated", &subject.to_value()).await,
            "{subject:?}"
        );
    }
    assert!(!insert_audit_row(db, workspace_id, "workspace.updated", "bogus").await);
}

#[tokio::test]
async fn builtin_flag_must_match_role_key() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let workspace_id = bare_workspace(db).await;
    let insert = |key: Option<&'static str>, builtin: bool| async move {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO roles (workspace_id, key, name, builtin) VALUES ($1, $2, $3, $4)",
            [
                Value::from(workspace_id),
                Value::from(key),
                Value::from(Uuid::now_v7().to_string()),
                Value::from(builtin),
            ],
        ))
        .await
        .is_ok()
    };
    assert!(!insert(Some("owner"), false).await, "key tanpa builtin");
    assert!(!insert(None, true).await, "builtin tanpa key");
    assert!(insert(None, false).await);
}

#[tokio::test]
async fn repo_provider_check_matches_enum() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let workspace_id = bare_workspace(db).await;
    let project_id = Uuid::now_v7();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO projects (id, workspace_id, key, name) VALUES ($1, $2, 'ENUM', 'Enum')",
        [Value::from(project_id), Value::from(workspace_id)],
    ))
    .await
    .unwrap();
    let insert = |provider: String| async move {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO repos (workspace_id, project_id, name, provider, url, default_branch) \
             VALUES ($1, $2, $3, $4, 'https://example.com/a', 'main')",
            [
                Value::from(workspace_id),
                Value::from(project_id),
                Value::from(Uuid::now_v7().to_string()),
                Value::from(provider),
            ],
        ))
        .await
        .is_ok()
    };
    for provider in RepoProvider::iter() {
        assert!(insert(provider.to_value()).await, "{provider:?}");
    }
    assert!(!insert("bogus".to_string()).await);
}

/// `UPDATE <table> SET <column> = value WHERE id = row` berhasil (CHECK menerima nilai).
async fn update_ok(
    db: &sea_orm::DatabaseConnection,
    table: &str,
    column: &str,
    id: Uuid,
    value: &str,
) -> bool {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!("UPDATE {table} SET {column} = $1 WHERE id = $2"),
        [Value::from(value), Value::from(id)],
    ))
    .await
    .is_ok()
}

/// Kolom enum tabel M2 (tiket, relasi, riwayat, notifikasi, field custom, skop penghitung):
/// setiap varian diterima CHECK, literal lain ditolak.
#[tokio::test]
async fn ticket_enum_checks_match_enums() {
    use agentgate_server::{
        models::{
            custom_fields::{self, FieldInput, FieldKind},
            notifications::{self, NewNotification, NotificationKind},
            ticket_code::CounterScope,
            ticket_events::{self, EventKind},
            ticket_links::{self, LinkKind},
            tickets::{TicketInput, TicketPriority, TicketStatus},
        },
        services::{tickets::create, workspaces::Acting},
    };

    use crate::support::{a_project, a_user, a_workspace, builtin_role};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.workspace_id();
    let project = a_project(ctx, &workspace, "ENM").await;
    let role = builtin_role(ctx, &workspace, RoleKey::Owner).await;
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let mut ids = Vec::new();
    for title in ["Satu", "Dua"] {
        let form = create::TicketForm {
            input: TicketInput {
                title: title.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        ids.push(
            create::run(db, &acting, project.id, &form)
                .await
                .unwrap()
                .id,
        );
    }
    let link =
        ticket_links::ActiveModel::link(db, wid, ids[0], ids[1], LinkKind::Relates, owner.id)
            .await
            .unwrap()
            .unwrap();
    let event = ticket_events::Model::of_ticket(db, wid, ids[0])
        .await
        .unwrap()
        .remove(0);
    let notification = notifications::ActiveModel::notify(
        db,
        wid,
        NewNotification {
            user_id: owner.id,
            actor_id: None,
            kind: NotificationKind::Assigned,
            ticket_id: Some(ids[0]),
            comment_id: None,
        },
    )
    .await
    .unwrap();
    let field = custom_fields::ActiveModel::create(
        db,
        wid,
        &FieldInput {
            name: "Nilai".into(),
            kind: FieldKind::Text,
            options: Vec::new(),
            required: false,
        },
    )
    .await
    .unwrap();

    let cases: Vec<(&str, &str, Uuid, Vec<String>)> = vec![
        (
            "tickets",
            "status",
            ids[0],
            TicketStatus::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "tickets",
            "priority",
            ids[0],
            TicketPriority::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "ticket_links",
            "kind",
            link.id,
            LinkKind::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "ticket_events",
            "kind",
            event.id,
            EventKind::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "notifications",
            "kind",
            notification.id,
            NotificationKind::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "custom_fields",
            "kind",
            field.id,
            FieldKind::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "workspaces",
            "ticket_counter_scope",
            workspace.id,
            CounterScope::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "projects",
            "ticket_counter_scope",
            project.id,
            CounterScope::iter().map(|v| v.to_value()).collect(),
        ),
    ];
    for (table, column, id, values) in cases {
        for value in &values {
            assert!(
                update_ok(db, table, column, id, value).await,
                "{table}.{column} = {value}"
            );
        }
        assert!(
            !update_ok(db, table, column, id, "bogus").await,
            "{table}.{column}"
        );
    }
}

/// Kolom enum tabel M3 (keadaan stage tiket, status versi artifact, peran penugasan stage):
/// setiap varian diterima CHECK, literal lain ditolak.
#[tokio::test]
async fn workflow_enum_checks_match_enums() {
    use agentgate_server::models::{
        artifact_versions::{self, VersionStatus},
        ticket_artifacts,
        ticket_stage_people::{self, StagePeople, StageRole},
        tickets::{StageState, TicketInput},
    };
    use agentgate_server::services::{tickets::create, workspaces::Acting};

    use crate::support::{a_project, a_user, a_workspace, builtin_role};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.workspace_id();
    let project = a_project(ctx, &workspace, "WFE").await;
    let role = builtin_role(ctx, &workspace, RoleKey::Owner).await;
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let form = create::TicketForm {
        input: TicketInput {
            title: "Satu".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let ticket = create::run(db, &acting, project.id, &form).await.unwrap();
    let artifact =
        ticket_artifacts::ActiveModel::create(db, wid, ticket.id, "todo", "prd.md", owner.id)
            .await
            .unwrap();
    let version = artifact_versions::ActiveModel::append(
        db,
        wid,
        artifact.id,
        "# PRD",
        VersionStatus::Draft,
        owner.id,
    )
    .await
    .unwrap();
    ticket_stage_people::ActiveModel::replace(
        db,
        wid,
        ticket.id,
        "todo",
        &StagePeople {
            executors: vec![owner.id],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let person: Uuid = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id FROM ticket_stage_people WHERE ticket_id = $1",
            [Value::from(ticket.id)],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    let cases: Vec<(&str, &str, Uuid, Vec<String>)> = vec![
        (
            "tickets",
            "stage_state",
            ticket.id,
            StageState::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "artifact_versions",
            "status",
            version.id,
            VersionStatus::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "ticket_stage_people",
            "role",
            person,
            StageRole::iter().map(|v| v.to_value()).collect(),
        ),
    ];
    for (table, column, id, values) in cases {
        for value in &values {
            assert!(
                update_ok(db, table, column, id, value).await,
                "{table}.{column} = {value}"
            );
        }
        assert!(
            !update_ok(db, table, column, id, "bogus").await,
            "{table}.{column}"
        );
    }
}
