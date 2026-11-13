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
            project_id: None,
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

/// Kolom enum skill pack (M4): sumber, status, risiko.
#[tokio::test]
async fn skill_pack_enum_checks_match_enums() {
    use agentgate_server::models::skill_packs::{SkillRisk, SkillSource, SkillStatus};
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let workspace_id = bare_workspace(db).await;
    let id: Uuid = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO skill_packs (workspace_id, key, name, source) \
             VALUES ($1, 'enum', 'Enum', 'custom') RETURNING id",
            [Value::from(workspace_id)],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    let cases: Vec<(&str, Vec<String>)> = vec![
        (
            "source",
            SkillSource::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "status",
            SkillStatus::iter().map(|v| v.to_value()).collect(),
        ),
        ("risk", SkillRisk::iter().map(|v| v.to_value()).collect()),
    ];
    for (column, values) in cases {
        for value in &values {
            assert!(
                update_ok(db, "skill_packs", column, id, value).await,
                "{column} = {value}"
            );
        }
        assert!(
            !update_ok(db, "skill_packs", column, id, "bogus").await,
            "{column}"
        );
    }
}

/// Kolom enum tabel runner & job (M5): status/jenis/mode job, aliran log, jenis evidence,
/// status izin, kebijakan kuota; setiap varian diterima CHECK, literal lain ditolak.
#[tokio::test]
async fn runner_job_enum_checks_match_enums() {
    use agentgate_core::runner::protocol::{
        EvidenceKind, JobKind, JobMode, JobStatus, LogStream, PermissionStatus,
    };
    use agentgate_server::models::{runner_prefs::QuotaPolicy, tickets::TicketInput};
    use agentgate_server::services::{tickets::create, workspaces::Acting};

    use crate::support::{a_project, a_user, a_workspace, builtin_role};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let project = a_project(ctx, &workspace, "RJE").await;
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
    let insert = |sql: &str, values: Vec<Value>| {
        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);
        async move {
            db.query_one_raw(stmt)
                .await
                .unwrap()
                .unwrap()
                .try_get::<Uuid>("", "id")
                .unwrap()
        }
    };
    let w = Value::from(workspace.id);
    let job = insert(
        "INSERT INTO jobs (workspace_id, project_id, ticket_id, kind, mode, stage_key, persona, \
         status) VALUES ($1, $2, $3, 'stage', 'auto', 'todo', 'dev', 'succeeded') RETURNING id",
        vec![w.clone(), project.id.into(), ticket.id.into()],
    )
    .await;
    let log = insert(
        "INSERT INTO job_logs (workspace_id, job_id, stream, body) VALUES ($1, $2, 'info', 'x') \
         RETURNING id",
        vec![w.clone(), job.into()],
    )
    .await;
    let evidence = insert(
        "INSERT INTO job_evidence (workspace_id, job_id, ticket_id, stage_key, kind, name, \
         content_type, size_bytes, sha256, storage_key, signature) VALUES ($1, $2, $3, 'todo', \
         'log', 'a.txt', 'text/plain', 1, 'x', 'k', 's') RETURNING id",
        vec![w.clone(), job.into(), ticket.id.into()],
    )
    .await;
    let permission = insert(
        "INSERT INTO job_permissions (workspace_id, job_id, ticket_id, action, command) \
         VALUES ($1, $2, $3, 'db_migrate', 'x') RETURNING id",
        vec![w.clone(), job.into(), ticket.id.into()],
    )
    .await;
    let prefs = insert(
        "INSERT INTO runner_prefs (workspace_id, user_id) VALUES ($1, $2) RETURNING id",
        vec![w, owner.id.into()],
    )
    .await;
    let strs = |all: &[&str]| all.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
    let cases: Vec<(&str, &str, Uuid, Vec<String>)> = vec![
        (
            "jobs",
            "kind",
            job,
            // `knowledge` wajib tanpa tiket (jobs_ticket_kind_check), diuji terpisah di bawah.
            strs(
                &JobKind::ALL
                    .iter()
                    .filter(|v| **v != JobKind::Knowledge)
                    .map(|v| v.as_str())
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "jobs",
            "mode",
            job,
            strs(&JobMode::ALL.iter().map(|v| v.as_str()).collect::<Vec<_>>()),
        ),
        (
            "jobs",
            "status",
            job,
            strs(
                &JobStatus::ALL
                    .iter()
                    .map(|v| v.as_str())
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "job_logs",
            "stream",
            log,
            strs(
                &LogStream::ALL
                    .iter()
                    .map(|v| v.as_str())
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "job_evidence",
            "kind",
            evidence,
            strs(
                &EvidenceKind::ALL
                    .iter()
                    .map(|v| v.as_str())
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "job_permissions",
            "status",
            permission,
            strs(
                &PermissionStatus::ALL
                    .iter()
                    .map(|v| v.as_str())
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "runner_prefs",
            "quota_policy",
            prefs,
            QuotaPolicy::iter().map(|v| v.to_value()).collect(),
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
    // Job knowledge = tanpa tiket; job tiket tidak boleh berjenis knowledge.
    assert!(!update_ok(db, "jobs", "kind", job, "knowledge").await);
    let unlinked = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE jobs SET kind = 'knowledge', ticket_id = NULL WHERE id = $1",
            [Value::from(job)],
        ))
        .await;
    assert!(unlinked.is_ok(), "{unlinked:?}");
    let stage_without_ticket = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE jobs SET kind = 'stage' WHERE id = $1",
            [Value::from(job)],
        ))
        .await;
    assert!(stage_without_ticket.is_err());
}

#[tokio::test]
async fn git_enum_checks_match_enums() {
    use agentgate_core::git::{CiState, PrState};
    use agentgate_server::models::git_connections::GitProviderKind;

    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let workspace_id = bare_workspace(db).await;
    let insert_conn = |provider: String| async move {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO git_connections (workspace_id, provider, host, api_base, web_base, \
             token_encrypted, webhook_secret_encrypted) VALUES ($1, $2, $3, 'a', 'w', '\\x00', \
             '\\x00')",
            [
                Value::from(workspace_id),
                Value::from(provider),
                Value::from(Uuid::now_v7().to_string()),
            ],
        ))
        .await
        .is_ok()
    };
    for kind in GitProviderKind::iter() {
        assert!(insert_conn(kind.to_value()).await, "{kind:?}");
    }
    assert!(!insert_conn("gitlab".to_string()).await);

    let project_id = Uuid::now_v7();
    let repo_id = Uuid::now_v7();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO projects (id, workspace_id, key, name) VALUES ($1, $2, 'GIT', 'Git')",
        [Value::from(project_id), Value::from(workspace_id)],
    ))
    .await
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO repos (id, workspace_id, project_id, name, provider, url, default_branch) \
         VALUES ($1, $2, $3, 'api', 'github', 'https://github.com/a/b', 'main')",
        [
            Value::from(repo_id),
            Value::from(workspace_id),
            Value::from(project_id),
        ],
    ))
    .await
    .unwrap();
    let pr_id = Uuid::now_v7();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO pull_requests (id, workspace_id, project_id, repo_id, number, title, url, \
         head_branch, base_branch, state) VALUES ($1, $2, $3, $4, 1, 't', 'u', 'h', 'main', \
         'open')",
        [
            Value::from(pr_id),
            Value::from(workspace_id),
            Value::from(project_id),
            Value::from(repo_id),
        ],
    ))
    .await
    .unwrap();
    for s in PrState::ALL {
        assert!(update_ok(db, "pull_requests", "state", pr_id, s.as_str()).await);
    }
    assert!(!update_ok(db, "pull_requests", "state", pr_id, "bogus").await);
    for s in CiState::ALL {
        assert!(update_ok(db, "pull_requests", "ci_state", pr_id, s.as_str()).await);
    }
    assert!(!update_ok(db, "pull_requests", "ci_state", pr_id, "bogus").await);
}

#[tokio::test]
async fn code_graph_checks_match_enums() {
    use agentgate_graph::{
        SymbolKind,
        build::{Confidence, EdgeKind},
        cross::{LinkConfidence, LinkKind},
        walk::SkipReason,
    };
    use agentgate_server::models::graph_repos::{IndexMode, IndexSource, IndexStatus};

    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let workspace_id = bare_workspace(db).await;
    let project_id = Uuid::now_v7();
    let repo_id = Uuid::now_v7();
    let exec = |sql: &'static str, vals: Vec<Value>| async move {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vals,
        ))
        .await
        .unwrap();
    };
    exec(
        "INSERT INTO projects (id, workspace_id, key, name) VALUES ($1, $2, 'GRF', 'Graph')",
        vec![Value::from(project_id), Value::from(workspace_id)],
    )
    .await;
    exec(
        "INSERT INTO repos (id, workspace_id, project_id, name, provider, url, default_branch) \
         VALUES ($1, $2, $3, 'web', 'github', 'https://github.com/a/w', 'main')",
        vec![
            Value::from(repo_id),
            Value::from(workspace_id),
            Value::from(project_id),
        ],
    )
    .await;
    exec(
        "INSERT INTO graph_repos (repo_id, workspace_id, project_id) VALUES ($1, $2, $3)",
        vec![
            Value::from(repo_id),
            Value::from(workspace_id),
            Value::from(project_id),
        ],
    )
    .await;
    let update_repo = |col: &'static str, v: String| async move {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("UPDATE graph_repos SET {col} = $1 WHERE repo_id = $2"),
            [Value::from(v), Value::from(repo_id)],
        ))
        .await
        .is_ok()
    };
    for s in IndexStatus::iter() {
        assert!(update_repo("status", s.to_value()).await, "{s:?}");
    }
    for s in IndexSource::iter() {
        assert!(update_repo("source", s.to_value()).await, "{s:?}");
    }
    for s in IndexMode::iter() {
        assert!(update_repo("mode", s.to_value()).await, "{s:?}");
    }
    for col in ["status", "source", "mode"] {
        assert!(!update_repo(col, "bogus".into()).await, "{col}");
    }
    let file_id = Uuid::now_v7();
    exec(
        "INSERT INTO graph_files (id, workspace_id, repo_id, path, lang) VALUES ($1, $2, $3, 'a.ts', 'typescript')",
        vec![Value::from(file_id), Value::from(workspace_id), Value::from(repo_id)],
    )
    .await;
    for r in SkipReason::ALL {
        assert!(
            update_ok(db, "graph_files", "skip_reason", file_id, r.as_str()).await,
            "{r:?}"
        );
    }
    assert!(!update_ok(db, "graph_files", "skip_reason", file_id, "bogus").await);
    let sym = |id: Uuid| {
        exec(
            "INSERT INTO graph_symbols (id, workspace_id, repo_id, file_id, name, qualified, kind, start_line, end_line) \
             VALUES ($1, $2, $3, $4, 'a', 'a', 'function', 1, 1)",
            vec![
                Value::from(id),
                Value::from(workspace_id),
                Value::from(repo_id),
                Value::from(file_id),
            ],
        )
    };
    let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
    sym(a).await;
    sym(b).await;
    for k in SymbolKind::ALL {
        assert!(
            update_ok(db, "graph_symbols", "kind", a, k.as_str()).await,
            "{k:?}"
        );
    }
    assert!(!update_ok(db, "graph_symbols", "kind", a, "bogus").await);
    for c in [Confidence::Certain, Confidence::Inferred] {
        assert!(update_ok(db, "graph_symbols", "dead", a, c.as_str()).await);
    }
    let edge = Uuid::now_v7();
    exec(
        "INSERT INTO graph_edges (id, workspace_id, repo_id, from_id, to_id, kind, confidence, source) \
         VALUES ($1, $2, $3, $4, $5, 'calls', 'certain', 'tree_sitter')",
        vec![Value::from(edge), Value::from(workspace_id), Value::from(repo_id), Value::from(a), Value::from(b)],
    )
    .await;
    for k in EdgeKind::ALL {
        assert!(
            update_ok(db, "graph_edges", "kind", edge, k.as_str()).await,
            "{k:?}"
        );
    }
    assert!(update_ok(db, "graph_edges", "source", edge, "scip").await);
    assert!(!update_ok(db, "graph_edges", "kind", edge, "bogus").await);
    let link = Uuid::now_v7();
    exec(
        "INSERT INTO graph_links (id, workspace_id, project_id, kind, confidence, from_repo_id) \
         VALUES ($1, $2, $3, 'http', 'certain', $4)",
        vec![
            Value::from(link),
            Value::from(workspace_id),
            Value::from(project_id),
            Value::from(repo_id),
        ],
    )
    .await;
    for k in LinkKind::ALL {
        assert!(
            update_ok(db, "graph_links", "kind", link, k.as_str()).await,
            "{k:?}"
        );
    }
    for c in LinkConfidence::ALL {
        assert!(
            update_ok(db, "graph_links", "confidence", link, c.as_str()).await,
            "{c:?}"
        );
    }
    assert!(!update_ok(db, "graph_links", "confidence", link, "bogus").await);
}

/// Kolom enum tabel M9 (sprint, log scope, rilis, versi release notes, estimasi, saved filter):
/// setiap varian diterima CHECK, literal lain ditolak.
#[tokio::test]
async fn planning_enum_checks_match_enums() {
    use agentgate_server::models::{
        release_note_versions::VersionSource,
        releases::{NotesSource, ReleaseStatus},
        saved_filters::Visibility,
        sprint_scope_events::ScopeKind,
        sprints::SprintState,
        ticket_estimates::EstimateSource,
        tickets::TicketInput,
    };
    use agentgate_server::services::{tickets::create, workspaces::Acting};

    use crate::support::{a_project, a_user, a_workspace, builtin_role};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let project = a_project(ctx, &workspace, "PLN").await;
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
    let insert = |sql: &str, values: Vec<Value>| {
        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);
        async move {
            db.query_one_raw(stmt)
                .await
                .unwrap()
                .unwrap()
                .try_get::<Uuid>("", "id")
                .unwrap()
        }
    };
    let w = Value::from(workspace.id);
    let sprint = insert(
        "INSERT INTO sprints (workspace_id, project_id, name, starts_on, ends_on) \
         VALUES ($1, $2, 'Sprint 1', '2026-11-02', '2026-11-13') RETURNING id",
        vec![w.clone(), project.id.into()],
    )
    .await;
    let scope = insert(
        "INSERT INTO sprint_scope_events (workspace_id, sprint_id, ticket_id, kind) \
         VALUES ($1, $2, $3, 'added') RETURNING id",
        vec![w.clone(), sprint.into(), ticket.id.into()],
    )
    .await;
    let release = insert(
        "INSERT INTO releases (workspace_id, project_id, name) VALUES ($1, $2, 'v1.0') \
         RETURNING id",
        vec![w.clone(), project.id.into()],
    )
    .await;
    let version = insert(
        "INSERT INTO release_note_versions (workspace_id, release_id, number, body, source) \
         VALUES ($1, $2, 1, 'x', 'manual') RETURNING id",
        vec![w.clone(), release.into()],
    )
    .await;
    let estimate = insert(
        "INSERT INTO ticket_estimates (workspace_id, ticket_id, source) VALUES ($1, $2, 'ai') \
         RETURNING id",
        vec![w.clone(), ticket.id.into()],
    )
    .await;
    let filter = insert(
        "INSERT INTO saved_filters (workspace_id, owner_id, name, query) \
         VALUES ($1, $2, 'Saya', 'assignee = currentUser()') RETURNING id",
        vec![w, owner.id.into()],
    )
    .await;
    let cases: Vec<(&str, &str, Uuid, Vec<String>)> = vec![
        (
            "sprints",
            "state",
            sprint,
            SprintState::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "sprint_scope_events",
            "kind",
            scope,
            ScopeKind::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "releases",
            "status",
            release,
            ReleaseStatus::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "releases",
            "notes_source",
            release,
            NotesSource::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "release_note_versions",
            "source",
            version,
            VersionSource::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "ticket_estimates",
            "source",
            estimate,
            EstimateSource::iter().map(|v| v.to_value()).collect(),
        ),
        (
            "saved_filters",
            "visibility",
            filter,
            // `team` wajib dengan team_id (CHECK terpisah).
            Visibility::iter()
                .filter(|v| *v != Visibility::Team)
                .map(|v| v.to_value())
                .collect(),
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
    assert!(!update_ok(db, "saved_filters", "visibility", filter, "team").await);
}

/// Kolom enum tabel M10 (penghematan token, tampilan karakter AI Office).
#[tokio::test]
async fn visual_enum_checks_match_enums() {
    use agentgate_server::models::{office_looks::LookSubject, token_savings::SavingFeature};

    use crate::support::{a_project, a_user, a_workspace};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let project = a_project(ctx, &workspace, "VIS").await;
    let row = |sql: &str, values: Vec<Value>| {
        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);
        async move {
            db.query_one_raw(stmt)
                .await
                .unwrap()
                .unwrap()
                .try_get::<Uuid>("", "id")
                .unwrap()
        }
    };
    let saving = row(
        "INSERT INTO token_savings (workspace_id, project_id, feature, tokens) \
         VALUES ($1, $2, 'code_graph', 10) RETURNING id",
        vec![workspace.id.into(), project.id.into()],
    )
    .await;
    let look = row(
        "INSERT INTO office_looks (workspace_id, subject, subject_key) \
         VALUES ($1, 'persona', 'dev') RETURNING id",
        vec![workspace.id.into()],
    )
    .await;
    for v in SavingFeature::iter() {
        assert!(
            update_ok(db, "token_savings", "feature", saving, &v.to_value()).await,
            "{v:?}"
        );
    }
    assert!(!update_ok(db, "token_savings", "feature", saving, "rtk").await);
    for v in LookSubject::iter() {
        assert!(
            update_ok(db, "office_looks", "subject", look, &v.to_value()).await,
            "{v:?}"
        );
    }
    assert!(!update_ok(db, "office_looks", "subject", look, "bot").await);
}

#[tokio::test]
async fn api_webhook_enum_checks_match_enums() {
    use agentgate_server::models::{
        api_tokens::ApiScope, outbox_events::WebhookEvent, webhook_deliveries::DeliveryStatus,
    };

    use crate::support::{a_project, a_user, a_workspace};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let project = a_project(ctx, &workspace, "API").await;
    let run = |sql: &str, values: Vec<Value>| {
        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);
        async move { db.execute_raw(stmt).await.is_ok() }
    };
    let token = |scopes: Vec<String>| {
        run(
            "INSERT INTO api_tokens (workspace_id, user_id, name, prefix, digest, scopes) \
             VALUES ($1, $2, 'T', 'ag_pat_x', $3, $4)",
            vec![
                workspace.id.into(),
                owner.id.into(),
                Uuid::now_v7().to_string().into(),
                scopes.into(),
            ],
        )
    };
    for s in ApiScope::iter() {
        assert!(token(vec![s.as_str()]).await, "{s:?}");
    }
    assert!(token(ApiScope::iter().map(ApiScope::as_str).collect()).await);
    assert!(!token(vec!["tiket:hapus".to_string()]).await);
    assert!(!token(Vec::new()).await);

    let hook = |events: Vec<String>| {
        run(
            "INSERT INTO webhooks (workspace_id, url, secret_encrypted, events) \
             VALUES ($1, 'https://contoh.id/h', '\\x00', $2)",
            vec![workspace.id.into(), events.into()],
        )
    };
    for e in WebhookEvent::subscribable() {
        assert!(hook(vec![e.as_str()]).await, "{e:?}");
    }
    assert!(!hook(vec![WebhookEvent::Ping.as_str()]).await);
    assert!(!hook(vec!["ticket.exploded".to_string()]).await);

    let outbox = |event: String| {
        run(
            "INSERT INTO outbox_events (workspace_id, project_id, event, subject_id) \
             VALUES ($1, $2, $3, $4)",
            vec![
                workspace.id.into(),
                project.id.into(),
                event.into(),
                Uuid::now_v7().into(),
            ],
        )
    };
    for e in WebhookEvent::subscribable() {
        assert!(outbox(e.as_str()).await, "{e:?}");
    }
    assert!(!outbox("ping".to_string()).await);

    let hook_id = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO webhooks (workspace_id, url, secret_encrypted, events) \
             VALUES ($1, 'https://contoh.id/x', '\\x00', ARRAY['ticket.created']) RETURNING id",
            [workspace.id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Uuid>("", "id")
        .unwrap();
    let delivery = |event: String, status: String| {
        run(
            "INSERT INTO webhook_deliveries (workspace_id, webhook_id, event, body, status) \
             VALUES ($1, $2, $3, '{}', $4)",
            vec![
                workspace.id.into(),
                hook_id.into(),
                event.into(),
                status.into(),
            ],
        )
    };
    for e in WebhookEvent::iter() {
        assert!(delivery(e.as_str(), "pending".into()).await, "{e:?}");
    }
    for s in DeliveryStatus::iter() {
        assert!(delivery("ping".into(), s.to_value()).await, "{s:?}");
    }
    assert!(!delivery("ping".into(), "sent".into()).await);
    assert!(!delivery("job.started".into(), "pending".into()).await);
}

/// Kolom enum tabel M11 (ekspor data, impor CSV).
#[tokio::test]
async fn transfer_enum_checks_match_enums() {
    use agentgate_server::models::{
        data_exports::{ExportPart, ExportStatus},
        ticket_imports::{Delimiter, ImportStatus},
    };

    use crate::support::{a_user, a_workspace};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let row = |sql: &str| {
        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, [workspace.id.into()]);
        async move {
            db.query_one_raw(stmt)
                .await
                .unwrap()
                .unwrap()
                .try_get::<Uuid>("", "id")
                .unwrap()
        }
    };
    let export = row(
        "INSERT INTO data_exports (workspace_id, parts) VALUES ($1, ARRAY['tickets']) RETURNING id",
    )
    .await;
    let import = row(
        "INSERT INTO ticket_imports (workspace_id, filename, size_bytes, delimiter) \
         VALUES ($1, 'a.csv', 10, 'comma') RETURNING id",
    )
    .await;
    for v in ExportStatus::iter() {
        assert!(
            update_ok(db, "data_exports", "status", export, &v.to_value()).await,
            "{v:?}"
        );
    }
    assert!(!update_ok(db, "data_exports", "status", export, "done").await);
    let parts_ok = |value: String| {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE data_exports SET parts = ARRAY[$1] WHERE id = $2",
            [Value::from(value), Value::from(export)],
        );
        async move { db.execute_raw(stmt).await.is_ok() }
    };
    for v in ExportPart::iter() {
        assert!(parts_ok(v.to_value()).await, "{v:?}");
        assert!(
            update_ok(db, "data_exports", "phase", export, &v.to_value()).await,
            "{v:?}"
        );
    }
    assert!(!parts_ok("secrets".into()).await);
    assert!(!update_ok(db, "data_exports", "phase", export, "secrets").await);
    for v in ImportStatus::iter() {
        assert!(
            update_ok(db, "ticket_imports", "status", import, &v.to_value()).await,
            "{v:?}"
        );
    }
    assert!(!update_ok(db, "ticket_imports", "status", import, "ready").await);
    for v in Delimiter::iter() {
        assert!(
            update_ok(db, "ticket_imports", "delimiter", import, &v.to_value()).await,
            "{v:?}"
        );
    }
    assert!(!update_ok(db, "ticket_imports", "delimiter", import, "tab").await);
}

#[tokio::test]
async fn jira_enum_checks_match_enums() {
    use agentgate_server::models::{
        jira_comment_links::CommentOrigin,
        jira_connections::{JiraEnv, SyncMode},
        jira_sync_logs::{Direction, SyncResult},
    };

    use crate::support::{a_user, a_workspace};

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    let owner = a_user().create(ctx).await;
    let run = |sql: &'static str, values: Vec<Value>| {
        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);
        async move { db.execute_raw(stmt).await.is_ok() }
    };
    // Satu koneksi per workspace: setiap percobaan memakai workspace baru.
    let (run, owner) = (&run, &owner);
    let conn = |env: String, mode: String| async move {
        let ws = a_workspace(owner).create(ctx).await;
        run(
            "INSERT INTO jira_connections (workspace_id, env, base_url, token_encrypted, mode) \
             VALUES ($1, $2, 'https://x.atlassian.net', '\\x00', $3)",
            vec![ws.id.into(), env.into(), mode.into()],
        )
        .await
    };
    for e in JiraEnv::iter() {
        assert!(conn(e.to_value(), "mirror".into()).await, "{e:?}");
    }
    for m in SyncMode::iter() {
        assert!(conn("cloud".into(), m.to_value()).await, "{m:?}");
    }
    assert!(!conn("server".into(), "mirror".into()).await);
    assert!(!conn("cloud".into(), "sideways".into()).await);

    let ws = a_workspace(owner).create(ctx).await;
    let conn_id = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO jira_connections (workspace_id, env, base_url, token_encrypted) \
             VALUES ($1, 'cloud', 'https://y.atlassian.net', '\\x00') RETURNING id",
            [ws.id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Uuid>("", "id")
        .unwrap();
    let log = |direction: String, result: String| {
        run(
            "INSERT INTO jira_sync_logs (workspace_id, connection_id, direction, result, message) \
             VALUES ($1, $2, $3, $4, 'jira-log-issue-created')",
            vec![ws.id.into(), conn_id.into(), direction.into(), result.into()],
        )
    };
    for d in Direction::iter() {
        for r in SyncResult::iter() {
            assert!(log(d.to_value(), r.to_value()).await, "{d:?} {r:?}");
        }
    }
    assert!(!log("both".into(), "ok".into()).await);
    assert!(!log("in".into(), "maybe".into()).await);

    // Asal komentar: varian yang tidak dikenal ditolak sebelum FK diperiksa tidak bisa diuji
    // tanpa komentar nyata; cukup pastikan nilai CHECK sama dengan enum.
    let check = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT pg_get_constraintdef(oid) AS def FROM pg_constraint \
             WHERE conrelid = 'jira_comment_links'::regclass AND contype = 'c'"
                .to_string(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "def")
        .unwrap();
    for o in CommentOrigin::iter() {
        assert!(check.contains(&format!("'{}'", o.to_value())), "{check}");
    }
    assert_eq!(check.matches("::text").count(), CommentOrigin::iter().count(), "{check}");
}
