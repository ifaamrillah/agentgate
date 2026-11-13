//! Job pembersihan harian (`workers::cleanup`) dan task-nya (`bin/loco task cleanup`).

use agentgate_server::{
    app::App,
    models::{
        invitations::{self, NewInvitation},
        login_throttles,
        permission::Permission,
        project_grants::GrantSubject,
        repos::{self, RepoInput},
        roles::RoleKey,
        user_tokens, workspaces,
    },
    services::{
        audit::{self, AuditAction, AuditEntry, AuditSubject},
        cleanup::CleanupStep,
    },
    tasks::cleanup::Cleanup,
    workers::cleanup,
};
use chrono::{Duration, Utc};
use loco_rs::{
    app::AppContext,
    task::{Task, Vars},
    testing::prelude::*,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DbBackend, EntityTrait, IntoActiveModel,
    PaginatorTrait, Statement,
};

use crate::support::{
    a_project, a_team, a_user, a_workspace, add_member, builtin_role, custom_role, grant,
};

/// Tabel milik workspace yang harus kosong setelah workspace dihapus permanen.
const TENANT_TABLES: &[&str] = &[
    "roles",
    "job_titles",
    "workspace_members",
    "invitations",
    "audit_events",
    "teams",
    "team_members",
    "projects",
    "project_grants",
    "repos",
    "ticket_types",
];

/// Tandai `workspace` terjadwal hapus `days` hari lalu.
async fn scheduled_days_ago(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    days: i64,
) -> workspaces::Model {
    let mut active = workspace.clone().into_active_model();
    active.deletion_scheduled_at = Set(Some((Utc::now() - Duration::days(days)).into()));
    active.update(&ctx.db).await.unwrap()
}

async fn exists(ctx: &AppContext, workspace: &workspaces::Model) -> bool {
    workspaces::Model::find_by_id(&ctx.db, workspace.id)
        .await
        .unwrap()
        .is_some()
}

async fn rows_of(ctx: &AppContext, table: &str, workspace: &workspaces::Model) -> i64 {
    let row = ctx
        .db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("SELECT count(*) AS n FROM {table} WHERE workspace_id = $1"),
            [workspace.id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<i64>("", "n").unwrap()
}

/// Workspace dengan semua jenis data tenant: role custom, anggota berjabatan, undangan
/// (tertunda dan diterima), tim, project, grant tim/orang dengan role custom, repo, dan audit.
async fn fully_populated(ctx: &AppContext, slug: &str) -> workspaces::Model {
    let owner = a_user().create(ctx).await;
    let dev = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).slug(slug).create(ctx).await;
    let wid = workspace.workspace_id();
    let member = add_member(ctx, &workspace, &dev, RoleKey::Member).await;
    let title = agentgate_server::models::job_titles::ActiveModel::create(&ctx.db, wid, "Dev")
        .await
        .unwrap();
    member.set_job_title(&ctx.db, Some(title.id)).await.unwrap();
    let reviewer = custom_role(ctx, &workspace, "Reviewer", &[Permission::ViewProject]).await;
    for (email, accepted) in [("tamu@example.test", false), ("lama@example.test", true)] {
        let (invitation, _) = invitations::ActiveModel::issue(
            &ctx.db,
            wid,
            NewInvitation {
                email: email.to_string(),
                role_id: reviewer.id,
                job_title_id: Some(title.id),
                invited_by: owner.id,
                ttl: Duration::days(7),
            },
        )
        .await
        .unwrap();
        if accepted {
            let mut active = invitation.into_active_model();
            active.accepted_at = Set(Some(Utc::now().into()));
            active.update(&ctx.db).await.unwrap();
        }
    }
    let team = a_team(ctx, &workspace, "Backend", &[&dev]).await;
    let project = a_project(ctx, &workspace, "KSR").await;
    grant(
        ctx,
        &workspace,
        &project,
        GrantSubject::Team(team.id),
        &reviewer,
    )
    .await;
    let member_role = builtin_role(ctx, &workspace, RoleKey::Member).await;
    grant(
        ctx,
        &workspace,
        &project,
        GrantSubject::User(dev.id),
        &member_role,
    )
    .await;
    repos::ActiveModel::add(
        &ctx.db,
        wid,
        project.id,
        &RepoInput {
            name: "kasir".to_string(),
            provider: "github".to_string(),
            url: "https://github.com/contoh/kasir".to_string(),
            default_branch: "main".to_string(),
        },
    )
    .await
    .unwrap();
    audit::record(
        &ctx.db,
        AuditEntry {
            workspace_id: wid,
            actor_id: Some(owner.id),
            action: AuditAction::ProjectCreated,
            subject: (AuditSubject::Project, project.id),
            changes: serde_json::json!({ "key": "KSR" }),
            request_id: None,
        },
    )
    .await
    .unwrap();
    for table in TENANT_TABLES {
        assert!(rows_of(ctx, table, &workspace).await > 0, "{table} kosong");
    }
    workspace
}

#[tokio::test]
async fn cleanup_deletes_workspace_after_30_days_only() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let due = fully_populated(ctx, "lama").await;
    let due = scheduled_days_ago(ctx, &due, 31).await;
    let grace = a_workspace(&owner).slug("tenggang").create(ctx).await;
    let grace = scheduled_days_ago(ctx, &grace, 29).await;
    let active = a_workspace(&owner).slug("aktif").create(ctx).await;

    let report = cleanup::run(ctx).await.unwrap();
    assert_eq!(report.count(CleanupStep::Workspaces), 1);
    assert_eq!(report.failed(), 0);

    assert!(!exists(ctx, &due).await);
    for table in TENANT_TABLES {
        assert_eq!(rows_of(ctx, table, &due).await, 0, "{table}");
    }
    assert!(exists(ctx, &grace).await);
    assert!(exists(ctx, &active).await);
    // User tetap ada: hanya data workspace yang dihapus.
    assert!(
        agentgate_server::models::users::Model::find_by_id(&ctx.db, owner.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn deleting_fully_populated_workspace_cascades_in_one_statement() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let doomed = fully_populated(ctx, "hapus").await;
    let kept = fully_populated(ctx, "simpan").await;
    // FK `project_grants.role_id`, `invitations.role_id`, dan `workspace_members.role_id` tanpa
    // ON DELETE: tetap berhasil karena baris yang merujuk ikut terhapus di statement yang sama.
    ctx.db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM workspaces WHERE id = $1",
            [doomed.id.into()],
        ))
        .await
        .unwrap();
    for table in TENANT_TABLES {
        assert_eq!(rows_of(ctx, table, &doomed).await, 0, "{table}");
        assert!(rows_of(ctx, table, &kept).await > 0, "{table}");
    }
}

#[tokio::test]
async fn cleanup_removes_stale_auth_rows_and_old_invitations() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let user = a_user().create(ctx).await;
    let now = Utc::now();
    let ago = |d: Duration| -> sea_orm::prelude::DateTimeWithTimeZone { (now - d).into() };

    // login_throttles: jendela lewat & tidak terkunci → hapus; jendela berjalan, atau masih
    // terkunci → simpan.
    for (key, window_started, locked_until) in [
        ("email:basi@example.test", ago(Duration::hours(2)), None),
        (
            "ip:10.0.0.1",
            ago(Duration::hours(2)),
            Some(ago(Duration::hours(1))),
        ),
        ("email:baru@example.test", ago(Duration::minutes(1)), None),
        (
            "ip:10.0.0.2",
            ago(Duration::hours(2)),
            Some((now + Duration::minutes(10)).into()),
        ),
    ] {
        login_throttles::ActiveModel {
            key: Set(key.to_string()),
            failures: Set(3),
            window_started_at: Set(window_started),
            locked_until: Set(locked_until),
            created_at: Set(window_started),
            updated_at: Set(window_started),
        }
        .insert(&ctx.db)
        .await
        .unwrap();
    }

    // user_tokens: kedaluwarsa/dipakai > 7 hari → hapus; lainnya simpan.
    for (digest, expires_at, used_at) in [
        ("a", ago(Duration::days(8)), None),
        (
            "b",
            (now + Duration::days(1)).into(),
            Some(ago(Duration::days(8))),
        ),
        ("c", ago(Duration::days(6)), None),
        (
            "d",
            (now + Duration::days(1)).into(),
            Some(ago(Duration::days(1))),
        ),
        ("e", (now + Duration::days(1)).into(), None),
    ] {
        user_tokens::ActiveModel {
            id: Set(uuid::Uuid::now_v7()),
            user_id: Set(user.id),
            purpose: Set("password_reset".to_string()),
            token_digest: Set(digest.to_string()),
            expires_at: Set(expires_at),
            used_at: Set(used_at),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
    }

    // invitations: selesai (diterima/dibatalkan/kedaluwarsa) > 30 hari → hapus.
    let workspace = a_workspace(&user).slug("acme").create(ctx).await;
    let role = builtin_role(ctx, &workspace, RoleKey::Member).await;
    let mut kept = Vec::new();
    let mut gone = Vec::new();
    for (email, ttl_days, accepted, revoked) in [
        ("kedaluwarsa-lama@example.test", -31, None, None),
        ("diterima-lama@example.test", 7, Some(31), None),
        ("dibatalkan-lama@example.test", 7, None, Some(31)),
        ("kedaluwarsa-baru@example.test", -29, None, None),
        ("diterima-baru@example.test", 7, Some(1), None),
        ("tertunda@example.test", 7, None, None),
    ] {
        let (invitation, _) = invitations::ActiveModel::issue(
            &ctx.db,
            workspace.workspace_id(),
            NewInvitation {
                email: email.to_string(),
                role_id: role.id,
                job_title_id: None,
                invited_by: user.id,
                ttl: Duration::days(ttl_days),
            },
        )
        .await
        .unwrap();
        let id = invitation.id;
        let mut active = invitation.into_active_model();
        active.accepted_at = Set(accepted.map(|d| ago(Duration::days(d))));
        active.revoked_at = Set(revoked.map(|d| ago(Duration::days(d))));
        active.update(&ctx.db).await.unwrap();
        if email.ends_with("-lama@example.test") {
            gone.push(id);
        } else {
            kept.push(id);
        }
    }

    let report = cleanup::run(ctx).await.unwrap();
    assert_eq!(report.failed(), 0);
    assert_eq!(report.count(CleanupStep::LoginThrottles), 2);
    assert_eq!(report.count(CleanupStep::UserTokens), 2);
    assert_eq!(report.count(CleanupStep::Invitations), 3);

    let throttles: Vec<String> = login_throttles::Entity::find()
        .all(&ctx.db)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.key)
        .collect();
    assert_eq!(throttles.len(), 2);
    assert!(throttles.contains(&"email:baru@example.test".to_string()));
    assert!(throttles.contains(&"ip:10.0.0.2".to_string()));

    let mut digests: Vec<String> = user_tokens::Entity::find()
        .all(&ctx.db)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.token_digest)
        .collect();
    digests.sort();
    assert_eq!(digests, ["c", "d", "e"]);

    for id in gone {
        assert!(
            invitations::Entity::find_by_id(id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_none()
        );
    }
    for id in kept {
        assert!(
            invitations::Entity::find_by_id(id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_some()
        );
    }
}

#[tokio::test]
async fn cleanup_purges_expired_sessions() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    // Tabel sesi dibuat store Loco saat pertama dipakai.
    loco_rs::auth::session_store::of(ctx).await.unwrap();
    let past = Utc::now().timestamp() - 60;
    let future = Utc::now().timestamp() + 3600;
    for (id, expires_at) in [("basi", past), ("aktif", future)] {
        ctx.db
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO loco_sessions (id_hash, user_key, data, expires_at) \
                 VALUES ($1, NULL, '{}', $2)",
                [id.into(), expires_at.into()],
            ))
            .await
            .unwrap();
    }
    let report = cleanup::run(ctx).await.unwrap();
    assert_eq!(report.count(CleanupStep::Sessions), 1);
    let left = ctx
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT count(*) AS n FROM loco_sessions",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap();
    assert_eq!(left, 1);
}

#[tokio::test]
async fn cleanup_is_idempotent() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let due = fully_populated(ctx, "lama").await;
    scheduled_days_ago(ctx, &due, 40).await;
    let first = cleanup::run(ctx).await.unwrap();
    assert_eq!(first.count(CleanupStep::Workspaces), 1);
    let second = cleanup::run(ctx).await.unwrap();
    assert_eq!(second.failed(), 0);
    for step in [
        CleanupStep::Sessions,
        CleanupStep::LoginThrottles,
        CleanupStep::UserTokens,
        CleanupStep::Invitations,
        CleanupStep::Workspaces,
        CleanupStep::Tickets,
        CleanupStep::Webhooks,
    ] {
        assert_eq!(second.count(step), 0, "{step:?}");
    }
}

/// Jumlah baris `loco_sessions` dengan `id_hash` ini.
async fn sessions_with(ctx: &AppContext, id_hash: &str) -> i64 {
    ctx.db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT count(*) AS n FROM loco_sessions WHERE id_hash = $1",
            [id_hash.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap()
}

/// Baris basi untuk setiap langkah pembersihan masih ada (`true`) atau sudah hilang semua.
async fn stale_rows_left(
    ctx: &AppContext,
    token: uuid::Uuid,
    invitation: uuid::Uuid,
    workspace: &workspaces::Model,
) -> [bool; 5] {
    [
        sessions_with(ctx, "basi").await == 1,
        login_throttles::Entity::find_by_id("email:basi@example.test")
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some(),
        user_tokens::Entity::find_by_id(token)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some(),
        invitations::Entity::find_by_id(invitation)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some(),
        exists(ctx, workspace).await,
    ]
}

#[tokio::test]
async fn cleanup_task_is_dry_run_without_confirm() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let now = Utc::now();
    let ago = |d: Duration| -> sea_orm::prelude::DateTimeWithTimeZone { (now - d).into() };
    let owner = a_user().create(ctx).await;
    let due = a_workspace(&owner).slug("lama").create(ctx).await;
    let due = scheduled_days_ago(ctx, &due, 31).await;

    // Satu baris basi untuk setiap langkah: throttle, token, undangan selesai, workspace.
    login_throttles::ActiveModel {
        key: Set("email:basi@example.test".to_string()),
        failures: Set(3),
        window_started_at: Set(ago(Duration::hours(2))),
        locked_until: Set(None),
        created_at: Set(ago(Duration::hours(2))),
        updated_at: Set(ago(Duration::hours(2))),
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    let token = user_tokens::ActiveModel {
        id: Set(uuid::Uuid::now_v7()),
        user_id: Set(owner.id),
        purpose: Set("password_reset".to_string()),
        token_digest: Set("basi".to_string()),
        expires_at: Set(ago(Duration::days(8))),
        used_at: Set(None),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
    .id;
    let role = builtin_role(ctx, &due, RoleKey::Member).await;
    let (invitation, _) = invitations::ActiveModel::issue(
        &ctx.db,
        due.workspace_id(),
        NewInvitation {
            email: "kedaluwarsa-lama@example.test".to_string(),
            role_id: role.id,
            job_title_id: None,
            invited_by: owner.id,
            ttl: Duration::days(-31),
        },
    )
    .await
    .unwrap();
    let invitation = invitation.id;

    // Database baru: tabel sesi belum dibuat store Loco; dry run tetap berhasil.
    ctx.db
        .execute_unprepared("DROP TABLE IF EXISTS loco_sessions")
        .await
        .unwrap();
    Cleanup
        .run(ctx, &Vars::from_cli_args(vec![]))
        .await
        .unwrap();

    // Sesi kedaluwarsa (tabel dibuat store Loco saat pertama dipakai).
    loco_rs::auth::session_store::of(ctx).await.unwrap();
    ctx.db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO loco_sessions (id_hash, user_key, data, expires_at) \
             VALUES ('basi', NULL, '{}', $1)",
            [(now.timestamp() - 60).into()],
        ))
        .await
        .unwrap();

    Cleanup
        .run(ctx, &Vars::from_cli_args(vec![]))
        .await
        .unwrap();
    assert_eq!(
        stale_rows_left(ctx, token, invitation, &due).await,
        [true; 5],
        "dry run tidak menghapus (sesi, throttle, token, undangan, workspace)"
    );
    Cleanup
        .run(
            ctx,
            &Vars::from_cli_args(vec![("confirm".into(), "no".into())]),
        )
        .await
        .unwrap();
    assert_eq!(
        stale_rows_left(ctx, token, invitation, &due).await,
        [true; 5],
        "hanya confirm:yes yang memutasi (sesi, throttle, token, undangan, workspace)"
    );

    Cleanup
        .run(
            ctx,
            &Vars::from_cli_args(vec![("confirm".into(), "yes".into())]),
        )
        .await
        .unwrap();
    assert_eq!(
        stale_rows_left(ctx, token, invitation, &due).await,
        [false; 5]
    );
    assert_eq!(workspaces::Entity::find().count(&ctx.db).await.unwrap(), 0);
}

/// Tiket di sampah lebih dari 30 hari dihapus permanen beserta file lampirannya; yang lebih baru
/// dan yang tidak di sampah tetap.
#[tokio::test]
async fn cleanup_purges_tickets_trashed_over_30_days() {
    use agentgate_server::{
        models::{ticket_attachments, tickets},
        services::{
            tickets::{attachments, create, trash},
            workspaces::Acting,
        },
    };

    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let wid = workspace.workspace_id();
    let project = a_project(ctx, &workspace, "KSR").await;
    let role = builtin_role(ctx, &workspace, RoleKey::Owner).await;
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let mut ids = Vec::new();
    for title in ["Lama", "Baru", "Aktif"] {
        let form = create::TicketForm {
            input: tickets::TicketInput {
                title: title.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        ids.push(
            create::run(&ctx.db, &acting, project.id, &form)
                .await
                .unwrap()
                .id,
        );
    }
    let file = attachments::upload(
        &ctx.db,
        &ctx.storage,
        &acting,
        ids[0],
        "a.txt",
        Some("text/plain"),
        axum::body::Bytes::from_static(b"isi"),
    )
    .await
    .unwrap();
    for (id, days) in [(ids[0], 31), (ids[1], 29)] {
        trash::trash(&ctx.db, &acting, id).await.unwrap();
        let mut active = tickets::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .into_active_model();
        active.deleted_at = Set(Some((Utc::now() - Duration::days(days)).into()));
        active.update(&ctx.db).await.unwrap();
    }

    let report = cleanup::run(ctx).await.unwrap();
    assert_eq!(report.count(CleanupStep::Tickets), 1);
    assert_eq!(report.failed(), 0);
    assert!(
        tickets::Model::find_in(&ctx.db, wid, ids[0])
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        tickets::Model::find_in(&ctx.db, wid, ids[1])
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        tickets::Model::find_in(&ctx.db, wid, ids[2])
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        ticket_attachments::Entity::find_by_id(file.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        ctx.storage
            .download::<Vec<u8>>(std::path::Path::new(&file.storage_key))
            .await
            .is_err()
    );
}
