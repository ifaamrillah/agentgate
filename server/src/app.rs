use std::path::Path;

use async_trait::async_trait;
use axum::Router as AxumRouter;
use loco_rs::{
    Result,
    app::{AppContext, Hooks, Initializer},
    bgworker::{BackgroundWorker, Queue},
    boot::{BootResult, StartMode, create_app},
    config::Config,
    controller::{
        AppRoutes,
        middleware::{self, MiddlewareLayer, MiddlewareStackExt},
    },
    db,
    environment::Environment,
    task::Tasks,
};
use migration::Migrator;
use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait, Value};

use crate::{
    common::settings::Settings,
    controllers, initializers,
    models::{
        job_titles,
        project_grants::{self, GrantSubject},
        projects::{self, NewProject},
        roles::{self, RoleKey},
        team_members, teams, users, workspace_members, workspaces,
    },
    services::workspaces::create::{self, NewWorkspace},
};

pub struct App;

/// User pertama di fixture; bila sudah ada, seed user dianggap sudah dijalankan.
const SEED_MARKER_EMAIL: &str = "owner@agentgate.test";
/// Workspace contoh seed; bila slug ini sudah ada, seed workspace dilewati.
const SEED_WORKSPACE_SLUG: &str = "pt-contoh";
/// Anggota workspace contoh: email user seed, role bawaan, dan jabatan (bila ada). Pemilik
/// (`SEED_MARKER_EMAIL`) dibuat sebagai Owner oleh `create::run`.
const SEED_MEMBERS: &[(&str, RoleKey, Option<&str>)] = &[
    (SEED_MARKER_EMAIL, RoleKey::Owner, Some("PM")),
    ("admin@agentgate.test", RoleKey::Admin, None),
    ("dev@agentgate.test", RoleKey::Member, Some("Dev")),
    ("qa@agentgate.test", RoleKey::Member, Some("QA")),
    ("viewer@agentgate.test", RoleKey::Viewer, None),
];
const SEED_JOB_TITLES: &[&str] = &["PM", "Dev", "QA"];
/// Tim contoh dan anggotanya (email user seed).
const SEED_TEAMS: &[(&str, &[&str])] = &[
    ("Backend", &["dev@agentgate.test"]),
    ("QA", &["qa@agentgate.test"]),
];
/// Project contoh: key dan nama. Pembuatnya Owner seed.
const SEED_PROJECT: (&str, &str) = ("KSR", "Kasir Online");

/// Semua tabel yang dikosongkan hook `truncate`; tabel baru didaftarkan di sini.
const TRUNCATE_TABLES: &[&str] = &[
    "users",
    "user_tokens",
    "login_throttles",
    "identities",
    "recovery_codes",
    "workspaces",
    "roles",
    "job_titles",
    "workspace_members",
    "invitations",
    "audit_events",
    "loco_sessions",
    "loco_remember_tokens",
];

#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        "agentgate"
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            agentgate_core::VERSION,
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    /// Tes memakai storage in-memory (rules §8): cepat dan tidak meninggalkan file.
    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        // Hub realtime LISTEN/NOTIFY (M12): satu per proses; listener baru dibuka saat
        // pendengar pertama mendaftar.
        ctx.shared_store
            .insert(std::sync::Arc::new(crate::services::realtime::Hub::new(
                ctx.db.get_postgres_connection_pool().clone(),
            )));
        if ctx.environment == Environment::Test {
            let storage = loco_rs::storage::Storage::single(loco_rs::storage::drivers::mem::new());
            return Ok(ctx.into_builder().storage(storage.into()).build());
        }
        Ok(ctx)
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        // Settings divalidasi sebelum apa pun jalan, supaya config rusak gagal saat boot.
        Settings::from_json(config.settings.as_ref())?;
        create_app::<Self, Migrator>(mode, environment, config).await
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![Box::new(
            initializers::view_engine::ViewEngineInitializer,
        )])
    }

    fn routes(ctx: &AppContext) -> AppRoutes {
        controllers::routes(ctx)
    }

    fn middlewares(ctx: &AppContext) -> Vec<Box<dyn MiddlewareLayer>> {
        let mut stack = middleware::default_middleware_stack(ctx);
        // Logger Loco mencatat URI utuh; pengganti ini menyamarkan query string route yang
        // membawa token email (rules §7).
        stack.replace(
            "logger",
            Box::new(controllers::middleware::request_log::RequestLog::new(ctx)),
        );
        // Gate harus di DALAM `auth_session` (yang memasang state sesi per request), jadi
        // disisipkan sebelum `auth_session` di Vec: yang lebih awal di Vec lebih dalam.
        stack.insert_before(
            "auth_session",
            Box::new(controllers::middleware::gate::Gate::new(ctx)),
        );
        stack
    }

    async fn before_routes(_ctx: &AppContext) -> Result<AxumRouter<AppContext>> {
        // Fallback dipasang di sini (sebelum middleware Loco diterapkan) supaya respons 404
        // ikut membawa CSP, request id, dan header keamanan lain.
        Ok(AxumRouter::new().fallback(controllers::middleware::html_errors::not_found))
    }

    async fn after_routes(router: AxumRouter, ctx: &AppContext) -> Result<AxumRouter> {
        // CSRF membungkus semua route (di luar stack Loco, sehingga berjalan sebelum gate login
        // di `middlewares`); penolakannya dirender sebagai halaman HTML oleh layer html_errors
        // yang dipasang view engine di luar layer ini. Pemisah domain pratinjau mockup (M10)
        // paling luar: host pratinjau tidak pernah mencapai route aplikasi lain.
        let preview_host =
            std::sync::Arc::new(crate::common::settings::Settings::from_ctx(ctx)?.preview_host());
        Ok(router
            // Webhook keluar (M11): delivery yang ditulis selama request di-enqueue setelah
            // handler selesai (setelah commit).
            .layer(axum::middleware::from_fn_with_state(
                ctx.clone(),
                controllers::middleware::webhook_dispatch::layer,
            ))
            .layer(axum::middleware::from_fn(
                controllers::middleware::csrf::layer,
            ))
            .layer(axum::middleware::from_fn_with_state(
                preview_host,
                controllers::middleware::preview_host::layer,
            )))
    }

    async fn connect_workers(ctx: &AppContext, queue: &Queue) -> Result<()> {
        // Pembersihan harian; jadwalnya lewat task `cleanup` di bagian `scheduler` config.
        queue
            .register(crate::workers::cleanup::CleanupWorker::build(ctx))
            .await?;
        // Index code graph oleh server (M7).
        queue
            .register(crate::workers::graph_index::GraphIndexWorker::build(ctx))
            .await?;
        // Pengiriman webhook keluar (M11).
        queue
            .register(crate::workers::webhook_delivery::WebhookDeliveryWorker::build(ctx))
            .await?;
        // Ekspor data workspace dan impor CSV tiket (M11).
        queue
            .register(crate::workers::data_export::DataExportWorker::build(ctx))
            .await?;
        queue
            .register(crate::workers::ticket_import::TicketImportWorker::build(
                ctx,
            ))
            .await?;
        // Sinkron Jira (M11).
        queue
            .register(crate::workers::jira_sync::JiraSyncWorker::build(ctx))
            .await?;
        Ok(())
    }

    fn register_tasks(tasks: &mut Tasks) {
        #[cfg(debug_assertions)]
        tasks.register(crate::tasks::dev_login::DevLogin);
        tasks.register(crate::tasks::cleanup::Cleanup);
        tasks.register(crate::tasks::export_sweep::ExportSweep);
        tasks.register(crate::tasks::jira_sync::JiraSync);
        tasks.register(crate::tasks::knowledge_audit::KnowledgeAudit);
        tasks.register(crate::tasks::webhooks::Webhooks);
        tasks.register(crate::tasks::user_create::UserCreate);
        tasks.register(crate::tasks::user_set_password::UserSetPassword);
    }

    /// Kosongkan tabel sebelum tes (`dangerously_truncate`). Satu `TRUNCATE` untuk semua tabel
    /// supaya urutan FK tidak penting. Tabel sesi Loco dibuat malas saat sesi pertama dipakai,
    /// jadi hanya tabel yang sudah ada yang ikut (`to_regclass`).
    async fn truncate(ctx: &AppContext) -> Result<()> {
        let existing = ctx
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT t FROM unnest($1::text[]) AS t WHERE to_regclass(t) IS NOT NULL",
                [Value::from(
                    TRUNCATE_TABLES
                        .iter()
                        .map(|t| (*t).to_string())
                        .collect::<Vec<_>>(),
                )],
            ))
            .await?
            .iter()
            .map(|row| row.try_get_by_index::<String>(0))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if !existing.is_empty() {
            // Nama tabel berasal dari konstanta TRUNCATE_TABLES, bukan input user.
            ctx.db
                .execute_unprepared(&format!("TRUNCATE {} CASCADE", existing.join(", ")))
                .await?;
        }
        Ok(())
    }

    /// Seed dev: user (fixture), lalu workspace "PT Contoh" dengan role bawaan, jabatan, semua
    /// user seed sebagai anggota, tim Backend dan QA, dan project KSR dengan aksesnya.
    /// Dijalankan ulang tanpa `db reset` tidak error: bagian yang sudah ada dilewati.
    async fn seed(ctx: &AppContext, base: &Path) -> Result<()> {
        if users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL)
            .await?
            .is_some()
        {
            tracing::info!("seed user dilewati: user seed sudah ada");
        } else {
            db::seed::<users::ActiveModel>(&ctx.db, &base.join("users.yaml").to_string_lossy())
                .await?;
        }
        seed_workspace(ctx).await?;
        seed_tickets(ctx).await?;
        seed_workflows(ctx).await?;
        seed_agents(ctx).await?;
        seed_runner(ctx).await?;
        seed_git(ctx).await?;
        seed_graph(ctx, base).await?;
        seed_knowledge(ctx).await?;
        seed_planning(ctx).await?;
        seed_visual(ctx).await?;
        seed_jira(ctx).await?;
        seed_integrations(ctx).await
    }
}

/// Connector Jira contoh (M11) di PT Contoh: Jira Cloud `sevima-retail.atlassian.net` (token
/// palsu, terenkripsi), project SR ↔ KSR, mode dua arah, peta status dengan satu status
/// (`Blocked`) sengaja belum dipetakan, peta field dan user (satu user Jira belum dipetakan),
/// beberapa tiket tertaut, dan ±12 baris log sinkron (berhasil/ditolak gate/gagal, masuk/keluar).
/// Koneksi dibuat **dijeda** supaya dev tidak memanggil Jira sungguhan dengan token palsu
/// (dorongan setelah request dan scheduler melewati koneksi yang dijeda). Dilewati bila
/// workspace sudah punya koneksi Jira.
#[allow(clippy::too_many_lines)]
async fn seed_jira(ctx: &AppContext) -> Result<()> {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    use crate::{
        common::crypto,
        models::{
            jira_connections::{
                self, Catalog, CatalogProject, FieldEntry, FieldMap, FieldTarget, StatusCategory,
                StatusEntry, StatusMap, SyncMode, SyncStats, UserEntry, UserMap,
            },
            jira_links::{self, NewLink},
            jira_sync_logs::{self, Direction, LogArgs, SyncResult},
            tickets,
        },
        services::jira::{api::JiraStatus, issues, mapping},
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    if jira_connections::Model::of_workspace(&ctx.db, wid)
        .await?
        .is_some()
    {
        tracing::info!("seed jira dilewati: koneksi Jira sudah ada");
        return Ok(());
    }
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed tidak ada"))?;
    let settings = Settings::from_ctx(ctx)?;
    let key = settings
        .auth
        .encryption_key_bytes()
        .map_err(|e| loco_rs::Error::string(&e.to_string()))?;
    let token = crypto::encrypt(&key, b"seed-jira-api-token-palsu")
        .map_err(|e| loco_rs::Error::string(&e.to_string()))?;

    // Status Jira project SR (prototype M11) → stage yang disarankan; Blocked dibiarkan kosong.
    let stages = mapping::project_stages(&ctx.db, wid, &project).await?;
    let raw = [
        ("10000", "To Do", StatusCategory::New),
        ("10001", "Analysis", StatusCategory::Indeterminate),
        ("10002", "Spec", StatusCategory::Indeterminate),
        ("10003", "In Design", StatusCategory::Indeterminate),
        ("10004", "In Progress", StatusCategory::Indeterminate),
        ("10005", "In Review", StatusCategory::Indeterminate),
        ("10006", "QA", StatusCategory::Indeterminate),
        ("10007", "Done", StatusCategory::Done),
        ("10008", "Blocked", StatusCategory::Indeterminate),
    ];
    let statuses = StatusMap {
        statuses: raw
            .iter()
            .map(|(id, name, category)| {
                let status = JiraStatus {
                    id: (*id).to_string(),
                    name: (*name).to_string(),
                    category: *category,
                };
                StatusEntry {
                    id: status.id.clone(),
                    name: status.name.clone(),
                    category: *category,
                    stage: (*name != "Blocked")
                        .then(|| mapping::suggest_stage(&status, &stages))
                        .flatten(),
                }
            })
            .collect(),
    };
    let fields = FieldMap {
        fields: [
            ("summary", "Summary", FieldTarget::Title),
            ("description", "Description", FieldTarget::Description),
            (
                "customfield_10016",
                "Story Points",
                FieldTarget::StoryPoints,
            ),
            ("customfield_10020", "Sprint", FieldTarget::Sprint),
            ("fixVersions", "Fix Version/s", FieldTarget::Release),
            ("components", "Components", FieldTarget::Component),
            ("duedate", "Due date", FieldTarget::DueOn),
            ("labels", "Labels", FieldTarget::Labels),
            ("priority", "Priority", FieldTarget::Priority),
            ("customfield_10031", "Toko", FieldTarget::Skip),
        ]
        .into_iter()
        .map(|(id, name, target)| FieldEntry {
            id: id.into(),
            name: name.into(),
            target,
        })
        .collect(),
    };
    let mut users_map = Vec::new();
    for (account, name, email) in [
        ("acc-owner", "Owner Contoh", SEED_MARKER_EMAIL),
        ("acc-dev", "Dewi Dev", "dev@agentgate.test"),
        ("acc-qa", "Qori QA", "qa@agentgate.test"),
        ("acc-budi", "Budi Santoso", "budi@sevima-retail.id"),
    ] {
        let user_id = users::Model::find_by_email(&ctx.db, email)
            .await?
            .map(|u| u.id);
        users_map.push(UserEntry {
            account: account.into(),
            name: name.into(),
            email: email.into(),
            user_id,
        });
    }
    let now = chrono::Utc::now();
    let conn = jira_connections::ActiveModel {
        workspace_id: Set(wid.get()),
        env: Set("cloud".into()),
        base_url: Set("https://sevima-retail.atlassian.net".into()),
        email: Set("olivia@sevima-retail.id".into()),
        token_encrypted: Set(token),
        account_name: Set("Olivia O.".into()),
        jira_project_key: Set("SR".into()),
        jira_project_name: Set("Sevima Retail POS".into()),
        project_id: Set(Some(project.id)),
        mode: Set(SyncMode::TwoWay.as_str()),
        status_map: Set(jira_connections::ActiveModel::json(&statuses)),
        field_map: Set(jira_connections::ActiveModel::json(&fields)),
        user_map: Set(jira_connections::ActiveModel::json(&UserMap {
            users: users_map,
        })),
        catalog: Set(jira_connections::ActiveModel::json(&Catalog {
            projects: vec![
                CatalogProject {
                    id: "10100".into(),
                    key: "SR".into(),
                    name: "Sevima Retail POS".into(),
                },
                CatalogProject {
                    id: "10101".into(),
                    key: "GD".into(),
                    name: "Gudang".into(),
                },
                CatalogProject {
                    id: "10102".into(),
                    key: "WEB".into(),
                    name: "Website".into(),
                },
            ],
        })),
        owner_id: Set(Some(owner.id)),
        last_sync_at: Set(Some((now - chrono::Duration::minutes(3)).into())),
        pulled_at: Set(Some((now - chrono::Duration::minutes(3)).into())),
        paused_at: Set(Some(now.into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    // Tiket tertaut: KSR-1..KSR-8 ↔ SR-301..; status Jira = status yang dipetakan ke stagenya.
    let mut linked = Vec::new();
    for n in 1..=8 {
        let code = format!("{}-{n}", SEED_PROJECT.0);
        let Some((t, _)) = tickets::Model::find_by_code(&ctx.db, wid, &code).await? else {
            continue;
        };
        let issue_key = format!("SR-{}", 300 + n);
        let status = statuses
            .status_for(&t.stage_key, "")
            .map(|s| s.id.clone())
            .unwrap_or_default();
        jira_links::ActiveModel::link(
            &ctx.db,
            NewLink {
                workspace_id: wid,
                connection_id: conn.id,
                ticket_id: t.id,
                issue_id: format!("{}", 20_000 + n),
                issue_key: issue_key.clone(),
                jira_status_id: status,
                snapshot: issues::of_ticket(&t),
            },
        )
        .await?;
        linked.push((t, issue_key));
    }
    let mut row: jira_connections::ActiveModel = conn.clone().into();
    row.stats = Set(jira_connections::ActiveModel::json(&SyncStats {
        tickets: linked.len() as u64,
        comments: 61,
        attachments: 8,
        sprints: 2,
    }));
    row.update(&ctx.db).await?;

    // Log sinkron contoh (terbaru dulu, menit ke belakang).
    let pick = |i: usize| linked.get(i % linked.len().max(1));
    type SeedLog = (
        i64,
        Direction,
        SyncResult,
        &'static str,
        Option<usize>,
        LogArgs,
    );
    let entries: [SeedLog; 12] = [
        (
            8,
            Direction::In,
            SyncResult::Ok,
            "jira-log-comment-pulled",
            Some(0),
            LogArgs::default(),
        ),
        (
            10,
            Direction::Out,
            SyncResult::Ok,
            "jira-log-issue-updated",
            Some(1),
            LogArgs {
                detail: "due_on".into(),
                ..LogArgs::default()
            },
        ),
        (
            19,
            Direction::In,
            SyncResult::Gate,
            "jira-log-gate-rejected",
            Some(2),
            LogArgs {
                status: "Done".into(),
                reason: "ticket-gates-unmet".into(),
                back: "QA".into(),
                ..LogArgs::default()
            },
        ),
        (
            38,
            Direction::Out,
            SyncResult::Ok,
            "jira-log-status-pushed",
            Some(3),
            LogArgs {
                status: "In Design".into(),
                ..LogArgs::default()
            },
        ),
        (
            52,
            Direction::In,
            SyncResult::Fail,
            "jira-log-attachment-skipped",
            None,
            LogArgs {
                detail: "rekaman-layar.mp4".into(),
                size: "32".into(),
                ..LogArgs::default()
            },
        ),
        (
            69,
            Direction::Out,
            SyncResult::Ok,
            "jira-log-issue-updated",
            Some(4),
            LogArgs {
                detail: "story_points".into(),
                ..LogArgs::default()
            },
        ),
        (
            90,
            Direction::In,
            SyncResult::Gate,
            "jira-log-gate-rejected",
            Some(5),
            LogArgs {
                status: "In Review".into(),
                reason: "ticket-gates-unmet".into(),
                back: "In Progress".into(),
                ..LogArgs::default()
            },
        ),
        (
            108,
            Direction::In,
            SyncResult::Ok,
            "jira-log-ticket-created",
            Some(6),
            LogArgs::default(),
        ),
        (
            123,
            Direction::Out,
            SyncResult::Ok,
            "jira-log-issue-updated",
            Some(0),
            LogArgs {
                detail: "labels".into(),
                ..LogArgs::default()
            },
        ),
        (
            140,
            Direction::Out,
            SyncResult::Ok,
            "jira-log-issue-updated",
            Some(2),
            LogArgs {
                detail: "priority".into(),
                ..LogArgs::default()
            },
        ),
        (
            158,
            Direction::In,
            SyncResult::Ok,
            "jira-log-comment-pulled",
            Some(7),
            LogArgs::default(),
        ),
        (
            175,
            Direction::In,
            SyncResult::Fail,
            "jira-log-user-unmapped",
            None,
            LogArgs {
                email: "budi@sevima-retail.id".into(),
                ..LogArgs::default()
            },
        ),
    ];
    for (minutes, direction, result, message, ticket, args) in entries {
        let at: sea_orm::prelude::DateTimeWithTimeZone =
            (now - chrono::Duration::minutes(minutes)).into();
        let (ticket_id, ticket_code, issue_key) = match ticket.and_then(pick) {
            Some((t, key)) => (Some(t.id), t.code.clone(), key.clone()),
            None => (None, String::new(), "SR-277".to_string()),
        };
        jira_sync_logs::ActiveModel {
            workspace_id: Set(wid.get()),
            connection_id: Set(conn.id),
            direction: Set(sea_orm::ActiveEnum::to_value(&direction)),
            result: Set(sea_orm::ActiveEnum::to_value(&result)),
            ticket_id: Set(ticket_id),
            ticket_code: Set(ticket_code),
            issue_key: Set(issue_key),
            message: Set(message.to_string()),
            args: Set(serde_json::to_value(&args).unwrap_or_default()),
            created_at: Set(at),
            updated_at: Set(at),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?;
    }
    tracing::info!(
        links = linked.len(),
        "seed jira: koneksi contoh dibuat (dijeda)"
    );
    Ok(())
}

/// Data integrasi (M11): dua token API, dua webhook dengan riwayat pengiriman (satu gagal
/// beruntun), dan satu ekspor data siap diunduh. Dilewati bila workspace sudah punya token API.
async fn seed_integrations(ctx: &AppContext) -> Result<()> {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

    use crate::{
        models::{api_tokens, webhook_deliveries},
        services::{api_tokens as tokens_svc, exports, webhooks, workspaces::Acting},
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    if api_tokens::Entity::find()
        .filter(api_tokens::Column::WorkspaceId.eq(wid.get()))
        .one(&ctx.db)
        .await?
        .is_some()
    {
        tracing::info!("seed integrasi dilewati: token API sudah ada");
        return Ok(());
    }
    let Some(owner) = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL).await? else {
        return Ok(());
    };
    let Some((_, role)) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id).await?
    else {
        return Ok(());
    };
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let fail = |e: &dyn std::fmt::Debug| loco_rs::Error::string(&format!("seed integrasi: {e:?}"));
    for (name, scopes, expiry, project) in [
        (
            "Dashboard BI toko",
            vec!["tiket:baca", "laporan:baca"],
            "365",
            "",
        ),
        (
            "Script migrasi stok",
            vec!["tiket:baca", "tiket:tulis"],
            "30",
            SEED_PROJECT.0,
        ),
    ] {
        let input = tokens_svc::TokenInput {
            name: name.into(),
            scopes: scopes.into_iter().map(String::from).collect(),
            expiry: expiry.into(),
            project: project.into(),
        };
        tokens_svc::create(&ctx.db, &acting, &input)
            .await
            .map_err(|e| fail(&e))?;
    }
    let settings = Settings::from_ctx(ctx)?;
    let now = chrono::Utc::now();
    // (url, event, [(menit lalu, status HTTP)])
    #[allow(clippy::type_complexity)]
    let hooks: [(&str, &str, &[(i64, i32)]); 2] = [
        (
            "https://bi.sevima-retail.id/hooks/agentgate",
            "ticket.updated",
            &[(2, 200), (14, 200), (33, 200), (61, 200), (95, 200)],
        ),
        (
            "https://hooks.slack-relay.internal/ksr",
            "release.published",
            &[(40, 500), (45, 500), (50, 500)],
        ),
    ];
    for (url, event, sends) in hooks {
        let input = webhooks::WebhookInput {
            url: url.into(),
            events: vec![event.into()],
            projects: Vec::new(),
        };
        let (hook, _) = webhooks::create(&ctx.db, &acting, &settings, &input)
            .await
            .map_err(|e| fail(&e))?;
        let last = sends
            .first()
            .map(|(m, c)| (now - chrono::Duration::minutes(*m), *c));
        let failures = i32::try_from(sends.iter().filter(|(_, c)| *c >= 300).count()).unwrap_or(0);
        let mut summary: crate::models::webhooks::ActiveModel = hook.clone().into();
        summary.last_delivery_at = Set(last.map(|(at, _)| at.into()));
        summary.last_status = Set(last.map(|(_, c)| c));
        summary.consecutive_failures = Set(failures);
        summary.update(&ctx.db).await?;
        for (minutes, code) in sends {
            let at = now - chrono::Duration::minutes(*minutes);
            let ok = (200..300).contains(code);
            let body = serde_json::json!({
                "event": event,
                "project": SEED_PROJECT.0,
                "ticket": { "key": "KSR-2", "stage": "review" },
                "at": at.to_rfc3339(),
            });
            webhook_deliveries::ActiveModel {
                workspace_id: Set(wid.get()),
                webhook_id: Set(hook.id),
                event: Set(event.into()),
                body: Set(body.to_string()),
                status: Set(if ok { "succeeded" } else { "failed" }.into()),
                attempts: Set(if ok { 1 } else { 5 }),
                response_status: Set(Some(*code)),
                response_body: Set(if ok {
                    "{\"received\": true}".into()
                } else {
                    "upstream relay timeout".into()
                }),
                duration_ms: Set(Some(if ok { 170 } else { 3010 })),
                delivered_at: Set(ok.then(|| at.into())),
                created_at: Set(at.into()),
                updated_at: Set(at.into()),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await?;
        }
    }
    let form = exports::ExportForm {
        parts: ["tickets", "artifacts", "knowledge", "config"]
            .into_iter()
            .map(String::from)
            .collect(),
        projects: Vec::new(),
        recipient: owner.id.to_string(),
    };
    let export = exports::request(ctx, &acting, &form)
        .await
        .map_err(|e| fail(&e))?;
    exports::run(ctx, export.id).await?;
    Ok(())
}

/// Data visual (M10) di KSR: riwayat job selesai 4 minggu terakhir dengan rincian token, biaya,
/// dan file berubah (halaman Penghemat token, sorot tiket di graph viewer), baris penghematan
/// per fitur, job terakhir tiket menunggu review (KSR-11) dan butuh keputusan (KSR-12) agar
/// tampil di AI Office, dan mockup HTML `checkout-qris.html` di KSR-11. Dilewati bila workspace
/// sudah punya data penghematan.
async fn seed_visual(ctx: &AppContext) -> Result<()> {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

    use crate::{
        models::{jobs as job_model, tickets, token_savings},
        services::{tickets::artifacts, workspaces::Acting},
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    if token_savings::Entity::find()
        .filter(token_savings::Column::WorkspaceId.eq(wid.get()))
        .one(&ctx.db)
        .await?
        .is_some()
    {
        tracing::info!("seed visual dilewati: data penghematan sudah ada");
        return Ok(());
    }
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed tidak ada"))?;
    let runner = crate::services::jobs::runners_of(&ctx.db, wid, None)
        .await?
        .into_iter()
        .find(|r| r.user_id == owner.id)
        .map(|r| r.id);
    let now = chrono::Utc::now();
    // (tiket, persona, status, hari lalu, input, output, cache read, cache write, file berubah)
    #[allow(clippy::type_complexity)]
    let history: &[(&str, &str, &str, i64, i64, i64, i64, i64, &[(&str, &str)])] = &[
        (
            "KSR-3",
            "dev",
            "succeeded",
            24,
            182_000,
            21_400,
            96_000,
            12_000,
            &[
                ("kasir-api", "src/orders/handlers.ts"),
                ("kasir-api", "src/orders/repo.ts"),
            ],
        ),
        (
            "KSR-3",
            "qa",
            "succeeded",
            23,
            64_000,
            8_200,
            41_000,
            6_000,
            &[("kasir-api", "tests/orders.test.ts")],
        ),
        (
            "KSR-10",
            "dev",
            "succeeded",
            16,
            240_500,
            30_100,
            120_000,
            18_000,
            &[
                ("kasir-api", "src/payments/qris.ts"),
                ("kasir-web", "src/pages/checkout.tsx"),
                ("kasir-web", "src/api/client.ts"),
            ],
        ),
        (
            "KSR-10",
            "reviewer",
            "succeeded",
            15,
            51_000,
            6_400,
            22_000,
            3_000,
            &[],
        ),
        (
            "KSR-6",
            "dev",
            "failed",
            9,
            98_000,
            12_300,
            40_000,
            8_000,
            &[("kasir-api", "src/stock/guard.ts")],
        ),
        (
            "KSR-6",
            "dev",
            "succeeded",
            8,
            132_000,
            17_800,
            70_000,
            9_500,
            &[
                ("kasir-api", "src/stock/guard.ts"),
                ("kasir-web", "src/pages/refund.tsx"),
            ],
        ),
        (
            "KSR-1",
            "uiux",
            "succeeded",
            3,
            44_000,
            9_900,
            15_000,
            4_000,
            &[],
        ),
        (
            "KSR-11",
            "analyst",
            "succeeded",
            1,
            38_000,
            7_600,
            12_000,
            2_500,
            &[],
        ),
        (
            "KSR-12",
            "architect",
            "failed",
            0,
            71_000,
            9_100,
            30_000,
            5_000,
            &[],
        ),
    ];
    let features = [
        token_savings::SavingFeature::CodeGraph,
        token_savings::SavingFeature::Layered,
        token_savings::SavingFeature::McpRules,
        token_savings::SavingFeature::Handoff,
    ];
    for (i, (code, persona, status, days, input, output, cache_read, cache_write, files)) in
        history.iter().enumerate()
    {
        let Some((ticket, _)) = tickets::Model::find_by_code(&ctx.db, wid, code).await? else {
            continue;
        };
        let at = now - chrono::Duration::days(*days) - chrono::Duration::minutes(40);
        let total = input + output + cache_read + cache_write;
        // Tarif kira-kira Sonnet: $3/juta input, $15/juta output, cache baca 10%, tulis 125%.
        let cost = input * 3 + output * 15 + cache_read * 3 / 10 + cache_write * 15 / 4;
        let job = job_model::ActiveModel {
            workspace_id: Set(wid.get()),
            project_id: Set(project.id),
            ticket_id: Set(Some(ticket.id)),
            kind: Set("stage".into()),
            mode: Set("auto".into()),
            stage_key: Set(ticket.stage_key.clone()),
            persona: Set((*persona).into()),
            status: Set((*status).into()),
            requested_by: Set(Some(owner.id)),
            claimed_by: Set(Some(owner.id)),
            runner_id: Set(runner),
            client: Set("claude".into()),
            model: Set("claude-sonnet-4-5".into()),
            summary: Set(if *status == "failed" {
                "Gate tests_passed belum terpenuhi: 2 tes gagal di modul stok".into()
            } else {
                format!("{code} selesai: perubahan siap direview")
            }),
            tokens: Set(Some(total)),
            tokens_input: Set(Some(*input)),
            tokens_output: Set(Some(*output)),
            tokens_cache_read: Set(Some(*cache_read)),
            tokens_cache_write: Set(Some(*cache_write)),
            cost_usd_micros: Set(Some(cost)),
            changed_files: Set(serde_json::json!(
                files
                    .iter()
                    .map(|(repo, path)| serde_json::json!({ "repo": repo, "path": path }))
                    .collect::<Vec<_>>()
            )),
            claimed_at: Set(Some((at - chrono::Duration::minutes(25)).into())),
            started_at: Set(Some((at - chrono::Duration::minutes(24)).into())),
            finished_at: Set(Some(at.into())),
            created_at: Set((at - chrono::Duration::minutes(30)).into()),
            updated_at: Set(at.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?;
        for (k, feature) in features.iter().enumerate() {
            #[allow(clippy::cast_possible_wrap)]
            let tokens = total / (6 + (i as i64 + k as i64) % 5) / (k as i64 + 1);
            token_savings::ActiveModel {
                workspace_id: Set(wid.get()),
                project_id: Set(project.id),
                job_id: Set(Some(job.id)),
                ticket_id: Set(Some(ticket.id)),
                feature: Set(sea_orm::ActiveEnum::to_value(feature)),
                tool: Set(["ag_snippet", "ag_context", "ag_rules", "ag_handoff"][k].into()),
                tokens: Set(tokens),
                created_at: Set(at.into()),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await?;
        }
    }

    // Mockup HTML di KSR-11 (stage menunggu review).
    if let Some((ticket, _)) = tickets::Model::find_by_code(&ctx.db, wid, "KSR-11").await? {
        let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
            .await?
            .ok_or_else(|| loco_rs::Error::string("owner seed bukan anggota"))?;
        let acting = Acting {
            workspace: &workspace,
            user: &owner,
            role: &role,
            request_id: None,
        };
        let fail = |e: &dyn std::fmt::Debug| loco_rs::Error::string(&format!("seed visual: {e:?}"));
        let art = artifacts::create(&ctx.db, &acting, ticket.id, "checkout-qris.html")
            .await
            .map_err(|e| fail(&e))?;
        artifacts::save(&ctx.db, &acting, ticket.id, art.id, SEED_MOCKUP)
            .await
            .map_err(|e| fail(&e))?;
    }
    Ok(())
}

const SEED_MOCKUP: &str = r#"<!doctype html>
<html lang="id"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Checkout QRIS</title>
<style>
body{margin:0;font:15px/1.5 system-ui,sans-serif;background:#f6f6f8;color:#17171c}
.wrap{max-width:420px;margin:0 auto;padding:24px 16px}
.card{background:#fff;border:1px solid #e1e1e8;border-radius:14px;padding:20px;box-shadow:0 8px 24px rgb(16 16 24/.08)}
h1{font-size:20px;margin:0 0 4px}.muted{color:#63636f;font-size:13px}
.row{display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #ebebef}
.total{font-weight:700;font-size:18px;border:0}
.qr{margin:18px auto;width:180px;height:180px;border-radius:12px;background:repeating-conic-gradient(#17171c 0 25%,#fff 0 50%) 0 0/20px 20px;border:10px solid #fff;outline:1px solid #e1e1e8}
button{width:100%;height:44px;border:0;border-radius:10px;background:#5b5bd6;color:#fff;font-weight:600;font-size:15px}
</style></head>
<body><div class="wrap"><div class="card">
<h1>Bayar dengan QRIS</h1><p class="muted">Pindai kode di bawah dengan aplikasi bank atau e-wallet Anda.</p>
<div class="row"><span>Kopi susu gula aren × 2</span><span>Rp 44.000</span></div>
<div class="row"><span>Diskon member</span><span>−Rp 4.400</span></div>
<div class="row total"><span>Total</span><span>Rp 39.600</span></div>
<div class="qr" role="img" aria-label="Kode QRIS"></div>
<p class="muted" style="text-align:center">Berlaku 04:59</p>
<button type="button">Saya sudah bayar</button>
</div></div></body></html>
"#;

/// Knowledge contoh (M8) di KSR: dokumen terbaru, satu basi, usulan AI (perubahan + dokumen baru),
/// pemilik knowledge, dan satu dokumen workspace. Dilewati bila project sudah punya dokumen.
async fn seed_knowledge(ctx: &AppContext) -> Result<()> {
    use crate::{
        models::{knowledge_docs, knowledge_proposals::ProposalTrigger, workspace_members},
        services::{
            knowledge::{docs, proposals},
            workspaces::Acting,
        },
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    if !knowledge_docs::Model::of_project(&ctx.db, wid, project.id)
        .await?
        .is_empty()
    {
        tracing::info!("seed knowledge dilewati: dokumen sudah ada");
        return Ok(());
    }
    let Some(owner) = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL).await? else {
        return Ok(());
    };
    let Some((_, role)) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id).await?
    else {
        return Ok(());
    };
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let fail = |e: crate::services::knowledge::Failure| {
        loco_rs::Error::string(&format!("seed knowledge: {e:?}"))
    };
    let samples: [(&str, &str, &str); 6] = [
        (
            "overview",
            "Overview Kasir Online",
            "# Kasir Online\n\nAplikasi kasir untuk toko retail: transaksi, shift, refund, dan laporan harian.\n\n## Pengguna\n\n- Kasir\n- Supervisor toko\n- Admin pusat\n",
        ),
        (
            "peta-repo",
            "Peta Repo",
            "## Repo\n\n| Repo | Isi |\n| --- | --- |\n| kasir-api | API Go: transaksi, stok, refund |\n| kasir-web | Web kasir (TypeScript) |\n",
        ),
        (
            "arsitektur",
            "Arsitektur Sistem",
            "## Komponen\n\n```mermaid\nflowchart LR\n  Web[kasir-web] --> API[kasir-api] --> DB[(Postgres)]\n```\n",
        ),
        (
            "aturan-bisnis",
            "Aturan Bisnis Refund",
            "## Refund\n\nKasir boleh refund sampai Rp500.000. Di atas itu wajib persetujuan supervisor.\n",
        ),
        (
            "api",
            "Dokumentasi API",
            "## Endpoint\n\n- `POST /v1/orders/:id` checkout\n- `POST /v1/refunds` refund\n",
        ),
        (
            "testing",
            "Strategi Testing",
            "## Tes\n\n- Unit test di setiap package\n- E2E Playwright untuk alur kasir\n",
        ),
    ];
    let mut created = Vec::new();
    for (type_key, title, body) in samples {
        let doc = docs::create(
            &ctx.db,
            &acting,
            &docs::NewDoc {
                project_id: Some(project.id),
                title: title.into(),
                type_key: type_key.into(),
                body: body.into(),
                slug: String::new(),
            },
        )
        .await
        .map_err(fail)?;
        created.push(doc);
    }
    docs::set_owner(&ctx.db, &acting, project.id, Some(owner.id))
        .await
        .map_err(fail)?;
    if let Some(api) = created.iter().find(|d| d.type_key == "api") {
        docs::mark_stale(
            &ctx.db,
            wid,
            &[api.id],
            "Endpoint refund berubah di kasir-api sejak dokumen diperbarui",
        )
        .await?;
    }
    let mut change = proposals::NewProposal::new(project.id, ProposalTrigger::Done);
    change.slug = "aturan-bisnis-refund".into();
    change.body = "## Refund\n\nKasir boleh refund sampai Rp500.000. Di atas itu wajib persetujuan supervisor.\n\n## Refund parsial\n\nRefund parsial dicatat per item; total refund tidak boleh melebihi nilai transaksi.\n".into();
    change.reason = "Tiket KSR-1 menambah refund parsial".into();
    change.trigger_ref = "Tiket KSR-1 selesai".into();
    change.persona = "writer".into();
    change.tokens = Some(18_400);
    proposals::propose(&ctx.db, wid, change)
        .await
        .map_err(fail)?;
    let mut new_doc = proposals::NewProposal::new(project.id, ProposalTrigger::Scan);
    new_doc.title = "Glosarium Retail".into();
    new_doc.type_key = "glosarium".into();
    new_doc.body = "## Istilah\n\n- **Shift**: periode kerja satu kasir\n- **Void**: pembatalan item sebelum dibayar\n".into();
    new_doc.reason = "Istilah domain yang sering muncul di kode".into();
    new_doc.persona = "writer".into();
    new_doc.tokens = Some(9_200);
    proposals::propose(&ctx.db, wid, new_doc)
        .await
        .map_err(fail)?;
    docs::create(
        &ctx.db,
        &acting,
        &docs::NewDoc {
            project_id: None,
            title: "Standar coding perusahaan".into(),
            type_key: "konvensi".into(),
            body: "## Penamaan\n\n- Fungsi camelCase, file kebab-case\n\n## Wajib di setiap PR\n\n- [x] Test untuk setiap perubahan logika\n- [x] Tanpa secret di kode\n".into(),
            slug: String::new(),
        },
    )
    .await
    .map_err(fail)?;
    tracing::info!("seed knowledge: 6 dokumen, 2 usulan, 1 dokumen workspace");
    Ok(())
}

/// Code graph contoh (M7): index `fixtures/graph/{kasir-api,kasir-web}` lewat jalur index server
/// yang sama, plus statistik git sintetis (arsip tidak membawa riwayat). Dilewati bila sudah ada.
async fn seed_graph(ctx: &AppContext, base: &Path) -> Result<()> {
    use agentgate_graph::history::{Coupling, GitStats, Hotspot, Owner};

    use crate::{
        models::{graph_repos, repos},
        services::graph::ingest::{self, DirIndex},
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL).await?;
    for repo in repos::Model::of_project(&ctx.db, wid, project.id).await? {
        let root = base.join("graph").join(&repo.name);
        if !root.is_dir() {
            continue;
        }
        let done = graph_repos::Model::of_repo(&ctx.db, wid, repo.id)
            .await?
            .is_some_and(|r| r.indexed_at.is_some());
        if done {
            tracing::info!("seed graph dilewati: {} sudah ter-index", repo.name);
            continue;
        }
        let git = (repo.name == "kasir-api").then(|| GitStats {
            window_days: 90,
            commits: 64,
            hotspots: vec![
                Hotspot {
                    path: "src/payments/qris.ts".into(),
                    commits: 23,
                    churn: 812,
                    top_author: "Dewi Lestari".into(),
                },
                Hotspot {
                    path: "src/orders/handlers.ts".into(),
                    commits: 17,
                    churn: 430,
                    top_author: "Olivia Putri".into(),
                },
                Hotspot {
                    path: "src/stock/guard.ts".into(),
                    commits: 9,
                    churn: 156,
                    top_author: "Olivia Putri".into(),
                },
            ],
            coupling: vec![
                Coupling {
                    a: "src/orders/handlers.ts".into(),
                    b: "src/stock/guard.ts".into(),
                    together: 8,
                    percent: 89,
                },
                Coupling {
                    a: "src/payments/qris.ts".into(),
                    b: "src/server.ts".into(),
                    together: 5,
                    percent: 42,
                },
            ],
            owners: vec![
                Owner {
                    module: "src/payments".into(),
                    author: "Dewi Lestari".into(),
                    email: "dewi@pt-contoh.id".into(),
                    percent: 71,
                    commits: 25,
                },
                Owner {
                    module: "src/orders".into(),
                    author: "Olivia Putri".into(),
                    email: "olivia@pt-contoh.id".into(),
                    percent: 64,
                    commits: 18,
                },
            ],
        });
        ingest::index_dir(
            &ctx.db,
            wid,
            &repo,
            DirIndex {
                root: &root,
                full: true,
                commit: "9f2c4e1a7b3d5f6081a2b3c4d5e6f708192a3b4c",
                git,
                by: owner.as_ref().map(|o| o.id),
            },
        )
        .await?;
    }
    Ok(())
}

/// Tiket contoh di KSR (sekali: dilewati bila KSR sudah punya tiket): epic dengan story dan
/// subtask, bug, relasi `blocks`, komentar dengan @mention, lewat service produksi.
/// Satu tiket contoh; `parent` = indeks tiket sebelumnya di `SEED_TICKETS`.
struct SeedTicket {
    title: &'static str,
    kind: &'static str,
    status: &'static str,
    priority: &'static str,
    labels: &'static str,
    points: &'static str,
    assignee: &'static str,
    parent: Option<usize>,
}

const SEED_TICKETS: &[SeedTicket] = &[
    SeedTicket {
        title: "Rilis kasir offline v1",
        kind: "epic",
        status: "in_progress",
        priority: "high",
        labels: "rilis",
        points: "",
        assignee: SEED_MARKER_EMAIL,
        parent: None,
    },
    SeedTicket {
        title: "Sinkronisasi transaksi saat koneksi kembali",
        kind: "story",
        status: "in_progress",
        priority: "urgent",
        labels: "offline, sync",
        points: "8",
        assignee: "dev@agentgate.test",
        parent: Some(0),
    },
    SeedTicket {
        title: "Simpan transaksi ke IndexedDB",
        kind: "subtask",
        status: "in_progress",
        priority: "medium",
        labels: "offline",
        points: "3",
        assignee: "dev@agentgate.test",
        parent: Some(1),
    },
    SeedTicket {
        title: "Antrean retry dengan backoff",
        kind: "subtask",
        status: "todo",
        priority: "medium",
        labels: "sync",
        points: "2",
        assignee: "dev@agentgate.test",
        parent: Some(1),
    },
    SeedTicket {
        title: "Cetak struk thermal 58 mm",
        kind: "story",
        status: "todo",
        priority: "high",
        labels: "printer",
        points: "5",
        assignee: "",
        parent: Some(0),
    },
    SeedTicket {
        title: "Total diskon salah saat item dihapus",
        kind: "bug",
        status: "in_progress",
        priority: "urgent",
        labels: "kasir, diskon",
        points: "2",
        assignee: "qa@agentgate.test",
        parent: None,
    },
    SeedTicket {
        title: "Driver printer Bluetooth Android",
        kind: "task",
        status: "backlog",
        priority: "medium",
        labels: "printer",
        points: "3",
        assignee: "",
        parent: None,
    },
    SeedTicket {
        title: "Laporan penjualan harian per kasir",
        kind: "story",
        status: "backlog",
        priority: "low",
        labels: "laporan",
        points: "5",
        assignee: "",
        parent: None,
    },
    SeedTicket {
        title: "Uji regresi alur pembayaran QRIS",
        kind: "task",
        status: "todo",
        priority: "medium",
        labels: "qa",
        points: "",
        assignee: "qa@agentgate.test",
        parent: None,
    },
    SeedTicket {
        title: "Ekspor laporan ke Excel",
        kind: "task",
        status: "done",
        priority: "low",
        labels: "laporan",
        points: "1",
        assignee: SEED_MARKER_EMAIL,
        parent: None,
    },
];

async fn seed_tickets(ctx: &AppContext) -> Result<()> {
    use crate::{
        models::{ticket_types, tickets::TicketInput},
        services::{
            tickets::{self as ticket_service, create::TicketForm},
            workspaces::Acting,
        },
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    if tickets_exist(ctx, wid, project.id).await? {
        tracing::info!("seed tiket dilewati: KSR sudah punya tiket");
        return Ok(());
    }
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed tidak ada"))?;
    let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed bukan anggota"))?;
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let types = ticket_types::Model::all_in(&ctx.db, wid).await?;
    let fail =
        |e: ticket_service::TicketFailure| loco_rs::Error::string(&format!("seed tiket: {e:?}"));
    let mut codes: Vec<String> = Vec::new();
    for SeedTicket {
        title,
        kind,
        status,
        priority,
        labels,
        points,
        assignee,
        parent,
    } in SEED_TICKETS
    {
        let assignee = if assignee.is_empty() {
            String::new()
        } else {
            users::Model::find_by_email(&ctx.db, assignee)
                .await?
                .map(|u| u.id.to_string())
                .unwrap_or_default()
        };
        let form = TicketForm {
            input: TicketInput {
                title: (*title).to_string(),
                description: format!(
                    "{title}.\n\nKriteria selesai:\n- Diuji di perangkat kasir\n- Tercatat di changelog"
                ),
                status: (*status).to_string(),
                priority: (*priority).to_string(),
                labels: (*labels).to_string(),
                due_on: String::new(),
                story_points: (*points).to_string(),
            },
            type_id: types
                .iter()
                .find(|t| t.key == *kind)
                .map(|t| t.id.to_string())
                .unwrap_or_default(),
            assignee,
            parent: parent.map(|i| codes[i].clone()).unwrap_or_default(),
            custom: std::collections::BTreeMap::new(),
            stage: String::new(),
            sprint: None,
            release: None,
            component: None,
        };
        let ticket = ticket_service::create::run(&ctx.db, &acting, project.id, &form)
            .await
            .map_err(fail)?;
        codes.push(ticket.code);
    }
    // Driver printer memblokir cetak struk; komentar dengan @mention ke dev.
    let struk = tickets_by_code(ctx, wid, &codes[4]).await?;
    ticket_service::links::add(&ctx.db, &acting, struk, "blocked_by", &codes[6])
        .await
        .map_err(fail)?;
    let sync = tickets_by_code(ctx, wid, &codes[1]).await?;
    ticket_service::comments::post(
        &ctx.db,
        &acting,
        sync,
        "@dev tolong cek konflik bila transaksi yang sama terkirim dua kali.",
    )
    .await
    .map_err(fail)?;
    Ok(())
}

/// Workflow contoh (M3): template Superpowers dipakai tipe Story di KSR, lalu tiga story di
/// berbagai keadaan stage (menunggu review dengan artifact dua versi, butuh keputusan setelah
/// tiga kali reject, dan eksekusi dengan gate evidence). Lewat service produksi; dilewati bila
/// workflow Superpowers sudah ada.
async fn seed_workflows(ctx: &AppContext) -> Result<()> {
    use crate::{
        models::{ticket_types, tickets::TicketInput, workflows as workflow_model},
        services::{
            tickets::{self as ticket_service, artifacts, create::TicketForm, stages},
            workflows::{self, ProjectWorkflows, Source},
            workspaces::Acting,
        },
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    let all = workflow_model::Model::all_in(&ctx.db, wid).await?;
    if all
        .iter()
        .any(|w| w.template_key.as_deref() == Some("superpowers"))
    {
        tracing::info!("seed workflow dilewati: Superpowers sudah ada");
        return Ok(());
    }
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed tidak ada"))?;
    let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed bukan anggota"))?;
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let fail =
        |e: ticket_service::TicketFailure| loco_rs::Error::string(&format!("seed workflow: {e:?}"));
    let superpowers = workflows::create(
        &ctx.db,
        &acting,
        "Superpowers-style",
        "Brainstorm, rencana, eksekusi bertahap dengan TDD, lalu review.",
        Some(Source::Template("superpowers".to_string())),
    )
    .await
    .map_err(fail)?;
    let types = ticket_types::Model::all_in(&ctx.db, wid).await?;
    let Some(story) = types.iter().find(|t| t.key == "story") else {
        return Ok(());
    };
    workflows::set_project_workflows(
        &ctx.db,
        &acting,
        project.id,
        &ProjectWorkflows {
            overrides: vec![(story.id, Some(superpowers.id))],
            allow_force_transition: false,
        },
    )
    .await
    .map_err(fail)?;

    let new_story = |title: &'static str| TicketForm {
        input: TicketInput {
            title: title.to_string(),
            description: format!("{title}.\n\nKriteria selesai:\n- Diuji di perangkat kasir"),
            status: "in_progress".to_string(),
            priority: "high".to_string(),
            labels: "pembayaran".to_string(),
            due_on: String::new(),
            story_points: "5".to_string(),
        },
        type_id: story.id.to_string(),
        assignee: String::new(),
        parent: String::new(),
        custom: std::collections::BTreeMap::new(),
        stage: "brainstorm".to_string(),
        sprint: None,
        release: None,
        component: None,
    };
    // Artifact output stage sekarang lalu minta review.
    let to_review = |ticket_id: uuid::Uuid, body: &'static str| {
        let acting = &acting;
        async move {
            let a = artifacts::create(&ctx.db, acting, ticket_id, "")
                .await
                .map_err(fail)?;
            artifacts::save(&ctx.db, acting, ticket_id, a.id, body)
                .await
                .map_err(fail)?;
            stages::complete(&ctx.db, acting, ticket_id)
                .await
                .map_err(fail)?;
            Ok::<_, loco_rs::Error>(())
        }
    };
    const BRAINSTORM: &str = "# Brainstorm\n\n## Masalah\nKasir belum menerima QRIS.\n\n## Alternatif\n- QRIS statis\n- QRIS dinamis per transaksi\n\n## Keputusan\nQRIS dinamis lewat payment gateway.";
    const PLAN: &str = "# Rencana implementasi\n\n## Tugas\n- [ ] Endpoint buat tagihan\n- [ ] Polling status bayar\n- [ ] Cetak struk setelah lunas";

    // 1. Menunggu review di Brainstorming (artifact v1 template, v2 isi).
    let qris = ticket_service::create::run(
        &ctx.db,
        &acting,
        project.id,
        &new_story("Checkout QRIS di kasir"),
    )
    .await
    .map_err(fail)?;
    to_review(qris.id, BRAINSTORM).await?;

    // 2. Butuh keputusan di Rencana: tiga kali reject.
    let refund = ticket_service::create::run(
        &ctx.db,
        &acting,
        project.id,
        &new_story("Refund parsial dari riwayat transaksi"),
    )
    .await
    .map_err(fail)?;
    to_review(refund.id, BRAINSTORM).await?;
    stages::approve(&ctx.db, &acting, refund.id, "")
        .await
        .map_err(fail)?;
    to_review(refund.id, PLAN).await?;
    for note in [
        "Belum ada langkah rollback bila gateway gagal.",
        "Tugas refund ke gateway belum dipecah.",
        "Masih belum menjawab refund sebagian item.",
    ] {
        let t = stages::reject(&ctx.db, &acting, refund.id, note)
            .await
            .map_err(fail)?;
        if t.stage_state == "active" {
            stages::complete(&ctx.db, &acting, refund.id)
                .await
                .map_err(fail)?;
        }
    }

    // 3. Eksekusi: gate evidence (tes, PR) belum terpenuhi.
    let member = ticket_service::create::run(
        &ctx.db,
        &acting,
        project.id,
        &new_story("Diskon member bertingkat"),
    )
    .await
    .map_err(fail)?;
    to_review(member.id, BRAINSTORM).await?;
    stages::approve(&ctx.db, &acting, member.id, "")
        .await
        .map_err(fail)?;
    to_review(member.id, PLAN).await?;
    stages::approve(&ctx.db, &acting, member.id, "Rencana oke, lanjut.")
        .await
        .map_err(fail)?;
    Ok(())
}

/// Planning contoh (M9) di KSR: sprint aktif berisi tiket berpoin, sprint berikutnya (rencana),
/// rilis v1.4 dengan Fix version, dan dua component. Lewat service produksi (izin, log scope);
/// dilewati bila KSR sudah punya sprint.
async fn seed_planning(ctx: &AppContext) -> Result<()> {
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
        QueryOrder,
    };

    use crate::{
        models::{sprints, tickets},
        services::{
            planning::{
                self,
                components::ComponentInput,
                releases::ReleaseInput,
                sprints::{Place, SprintInput},
            },
            workspaces::Acting,
        },
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    if !sprints::Model::of_project(&ctx.db, wid, project.id)
        .await?
        .is_empty()
    {
        tracing::info!("seed planning dilewati: KSR sudah punya sprint");
        return Ok(());
    }
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed tidak ada"))?;
    let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed bukan anggota"))?;
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let fail = |e: planning::Failure| loco_rs::Error::string(&format!("seed planning: {e:?}"));
    let today = planning::today();
    let day = |n: i64| (today + chrono::Duration::days(n)).to_string();
    let active = planning::sprints::create(
        &ctx.db,
        &acting,
        project.id,
        &SprintInput {
            name: "Sprint 4".into(),
            goal: "Kasir tetap jalan saat offline".into(),
            starts_on: day(-4),
            ends_on: day(9),
            capacity: "30".into(),
        },
    )
    .await
    .map_err(fail)?;
    planning::sprints::create(
        &ctx.db,
        &acting,
        project.id,
        &SprintInput {
            name: "Sprint 5".into(),
            goal: "Struk digital dan laporan harian".into(),
            starts_on: day(10),
            ends_on: day(23),
            capacity: "30".into(),
        },
    )
    .await
    .map_err(fail)?;
    let release = planning::releases::create(
        &ctx.db,
        &acting,
        project.id,
        &ReleaseInput {
            name: "v1.4".into(),
            description: "Mode offline kasir".into(),
            starts_on: day(-4),
            target_on: day(20),
        },
    )
    .await
    .map_err(fail)?;
    let payments = planning::components::create(
        &ctx.db,
        &acting,
        project.id,
        &ComponentInput {
            name: "Pembayaran".into(),
            description: "QRIS, tunai, refund".into(),
            owner: owner.id.to_string(),
            modules: Vec::new(),
        },
    )
    .await
    .map_err(fail)?;
    planning::components::create(
        &ctx.db,
        &acting,
        project.id,
        &ComponentInput {
            name: "Struk & printer".into(),
            description: "Cetak dan kirim struk".into(),
            owner: String::new(),
            modules: Vec::new(),
        },
    )
    .await
    .map_err(fail)?;
    let list = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(project.id))
        .filter(tickets::Column::DeletedAt.is_null())
        .order_by_asc(tickets::Column::CreatedAt)
        .all(&ctx.db)
        .await?;
    // Lima tiket pertama ke Sprint 4 (Fix version v1.4); yang tanpa poin diberi 3.
    for t in list.iter().take(5) {
        let t =
            planning::sprints::move_ticket(&ctx.db, &acting, t.id, Some(active.id), Place::Keep)
                .await
                .map_err(fail)?;
        let mut m = t.clone().into_active_model();
        m.release_id = Set(Some(release.id));
        m.component_id = Set(Some(payments.id));
        if t.story_points.is_none() {
            m.story_points = Set(Some(3));
        }
        m.update(&ctx.db).await?;
    }
    planning::sprints::start(&ctx.db, &acting, active.id, None)
        .await
        .map_err(fail)?;
    Ok(())
}

async fn tickets_exist(
    ctx: &AppContext,
    wid: workspaces::WorkspaceId,
    project_id: uuid::Uuid,
) -> Result<bool> {
    let counts = crate::models::tickets::Model::count_by_status(&ctx.db, wid, project_id).await?;
    Ok(counts.iter().any(|(_, n)| *n > 0))
}

async fn tickets_by_code(
    ctx: &AppContext,
    wid: workspaces::WorkspaceId,
    code: &str,
) -> Result<uuid::Uuid> {
    crate::models::tickets::Model::find_by_code(&ctx.db, wid, code)
        .await?
        .map(|(t, _)| t.id)
        .ok_or_else(|| loco_rs::Error::string("tiket seed tidak ada"))
}

/// Workspace contoh lewat jalur produksi (`create::run_in`), sehingga isinya sama dengan
/// workspace yang dibuat user. Anggota lain dan jabatan ditambahkan lewat model. Semuanya satu
/// transaksi: bila satu langkah gagal tidak ada yang tersimpan, jadi seed berikutnya (yang
/// melewati workspace bila slug-nya sudah ada) tidak meninggalkan PT Contoh setengah jadi.
async fn seed_workspace(ctx: &AppContext) -> Result<()> {
    if workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG)
        .await?
        .is_some()
    {
        tracing::info!("seed workspace dilewati: {SEED_WORKSPACE_SLUG} sudah ada");
        return Ok(());
    }
    let seed_user = |email: &'static str| async move {
        users::Model::find_by_email(&ctx.db, email)
            .await?
            .ok_or_else(|| loco_rs::Error::string(&format!("user seed {email} tidak ada")))
    };
    let owner = seed_user(SEED_MARKER_EMAIL).await?;
    let txn = ctx.db.begin().await?;
    let workspace = create::run_in(
        &txn,
        &owner,
        NewWorkspace {
            name: "PT Contoh".to_string(),
            slug: SEED_WORKSPACE_SLUG.to_string(),
        },
        None,
    )
    .await?;
    let wid = workspace.workspace_id();
    let mut titles = Vec::with_capacity(SEED_JOB_TITLES.len());
    for name in SEED_JOB_TITLES {
        titles.push(job_titles::ActiveModel::create(&txn, wid, name).await?);
    }
    for (email, key, title) in SEED_MEMBERS {
        let user = seed_user(email).await?;
        let member = if *key == RoleKey::Owner {
            workspace_members::Model::find_with_role(&txn, wid, user.id)
                .await?
                .map(|(member, _)| member)
                .ok_or_else(|| loco_rs::Error::string("owner seed bukan anggota"))?
        } else {
            let role = roles::Model::find_builtin(&txn, wid, *key)
                .await?
                .ok_or_else(|| loco_rs::Error::string("role bawaan tidak ada"))?;
            workspace_members::ActiveModel::add(&txn, wid, user.id, role.id).await?
        };
        let title_id = title.and_then(|t| titles.iter().find(|j| j.name == t).map(|j| j.id));
        if title_id.is_some() {
            member.set_job_title(&txn, title_id).await?;
        }
    }
    seed_project(&txn, wid, &owner, &seed_user).await?;
    txn.commit().await?;
    Ok(())
}

/// Tim, project KSR, dan akses contoh di transaksi seed workspace: tim Backend (dev@) dan QA
/// (qa@) ber-role Member di KSR, viewer@ ber-role Viewer langsung. Owner dan Admin seed sudah
/// Admin di semua project lewat role workspace-nya.
async fn seed_project<F, Fut>(
    txn: &sea_orm::DatabaseTransaction,
    wid: workspaces::WorkspaceId,
    owner: &users::Model,
    seed_user: &F,
) -> Result<()>
where
    F: Fn(&'static str) -> Fut,
    Fut: std::future::Future<Output = Result<users::Model>>,
{
    let role = |key: RoleKey| async move {
        roles::Model::find_builtin(txn, wid, key)
            .await?
            .ok_or_else(|| loco_rs::Error::string("role bawaan tidak ada"))
    };
    let member_role = role(RoleKey::Member).await?;
    let viewer_role = role(RoleKey::Viewer).await?;
    let (key, name) = SEED_PROJECT;
    let project = projects::ActiveModel::create(
        txn,
        wid,
        NewProject {
            key: key.to_string(),
            name: name.to_string(),
            description: "Aplikasi kasir untuk toko retail.".to_string(),
            color: "#0d9488".to_string(),
            created_by: Some(owner.id),
        },
    )
    .await?;
    for (team_name, emails) in SEED_TEAMS {
        let team = teams::ActiveModel::create(txn, wid, team_name).await?;
        for email in *emails {
            let user = seed_user(email).await?;
            team_members::ActiveModel::add(txn, wid, team.id, user.id).await?;
        }
        project_grants::ActiveModel::grant(
            txn,
            wid,
            project.id,
            GrantSubject::Team(team.id),
            member_role.id,
        )
        .await?;
    }
    let viewer = seed_user("viewer@agentgate.test").await?;
    project_grants::ActiveModel::grant(
        txn,
        wid,
        project.id,
        GrantSubject::User(viewer.id),
        viewer_role.id,
    )
    .await?;
    Ok(())
}

/// Skill pack contoh (sudah di-approve, dipasang di persona Developer) dan draft tiket hasil
/// brainstorming di project KSR (M4). Dilewati bila skill pack contoh sudah ada.
async fn seed_agents(ctx: &AppContext) -> Result<()> {
    use crate::{
        models::{personas, skill_packs},
        services::{
            drafts::{self, DraftInput},
            personas::{self as persona_service, Edit},
            skills,
            workspaces::Acting,
        },
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    if skill_packs::Model::all_in(&ctx.db, wid)
        .await?
        .iter()
        .any(|p| p.name == "Konvensi kasir")
    {
        tracing::info!("seed agent dilewati: skill pack contoh sudah ada");
        return Ok(());
    }
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed tidak ada"))?;
    let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed bukan anggota"))?;
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let fail = |e: &dyn std::fmt::Debug| loco_rs::Error::string(&format!("seed agent: {e:?}"));
    let pack = skills::create_custom(
        &ctx.db,
        &acting,
        "Konvensi kasir",
        "Aturan kode dan istilah domain aplikasi kasir.",
    )
    .await
    .map_err(|e| fail(&e))?;
    skills::save_file(
        &ctx.db,
        &acting,
        pack.id,
        "SKILL.md",
        "---\nname: konvensi-kasir\ndescription: Aturan kode dan istilah domain kasir\nallowed-tools: Read\n---\n\n# Konvensi kasir\n\n- Nominal uang selalu integer rupiah, bukan float.\n- Istilah: transaksi, struk, shift, kasir.\n- Setiap endpoint baru wajib punya tes integrasi.\n",
    )
    .await
    .map_err(|e| fail(&e))?;
    skills::submit(&ctx.db, &acting, pack.id, "")
        .await
        .map_err(|e| fail(&e))?;
    skills::approve(&ctx.db, &acting, pack.id, "")
        .await
        .map_err(|e| fail(&e))?;
    if let Some(dev) = personas::Model::find_active(&ctx.db, wid, "dev").await? {
        persona_service::update(&ctx.db, &acting, dev.id, Edit::Skills(vec![pack.id]))
            .await
            .map_err(|e| fail(&e))?;
    }
    let draft =
        |r: &str, parent: Option<&str>, ty: &str, title: &str, points: Option<i32>| DraftInput {
            reference: r.to_string(),
            parent: parent.map(str::to_string),
            type_key: ty.to_string(),
            title: title.to_string(),
            description: format!("{title}. Disusun dari sesi brainstorming laporan harian."),
            story_points: points,
            estimate: points.map(|p| {
                let h = f64::from(p);
                crate::services::drafts::DraftEstimate {
                    stages: [
                        ("prd", 0.5, 1.0),
                        ("development", 1.5, 0.5),
                        ("code_review", 0.2, 0.8),
                    ]
                    .iter()
                    .map(|(k, a, m)| crate::models::ticket_estimates::StageHours {
                        stage: (*k).to_string(),
                        ai_hours: a * h,
                        human_hours: m * h,
                    })
                    .collect(),
                    basis: "Mirip fitur laporan sebelumnya; diskalakan dari story point.".into(),
                    persona: "analyst".into(),
                }
            }),
        };
    drafts::save_from_agent(
        &ctx.db,
        &acting,
        project.id,
        None,
        &[
            draft(
                "e1",
                None,
                "epic",
                "Laporan harian otomatis lewat email",
                None,
            ),
            draft(
                "s1",
                Some("e1"),
                "story",
                "Pengaturan penerima dan jam kirim",
                Some(3),
            ),
            draft(
                "t1",
                Some("s1"),
                "subtask",
                "Form jam kirim per toko",
                Some(1),
            ),
            draft(
                "s2",
                Some("e1"),
                "story",
                "Isi laporan: omzet, transaksi, produk terlaris",
                Some(5),
            ),
            draft("s3", Some("e1"), "story", "Lampiran PDF laporan", Some(3)),
            draft(
                "b1",
                None,
                "task",
                "Berhenti berlangganan dari email laporan",
                Some(2),
            ),
        ],
    )
    .await
    .map_err(|e| fail(&e))?;
    Ok(())
}

/// Runner & job contoh (M5): runner owner online + runner Dev offline; story di Eksekusi dengan
/// job berjalan (log, langkah, evidence bertanda tangan); story baru yang job-nya antre; komentar
/// `@agent-qa` yang job-nya menunggu izin aksi. Dilewati bila workspace sudah punya runner.
/// Git & PR contoh (M6, sekali): koneksi GitHub (token palsu, tidak dipakai memanggil API), repo
/// KSR `kasir-api` dan `kasir-web` tertaut + webhook, dan tiga PR tiket (CI lulus, CI gagal,
/// merged). Merge otomatis dan pembukaan PR baru butuh token asli.
async fn seed_git(ctx: &AppContext) -> Result<()> {
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    };

    use crate::{
        common::{crypto, settings::Settings},
        models::{git_connections, pull_requests, repos, tickets},
    };
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    if !git_connections::Model::of_workspace(&ctx.db, wid)
        .await?
        .is_empty()
    {
        tracing::info!("seed git dilewati: koneksi sudah ada");
        return Ok(());
    }
    let settings = Settings::from_ctx(ctx)?;
    let key = settings
        .auth
        .encryption_key_bytes()
        .map_err(|e| loco_rs::Error::string(&format!("seed git: {e:?}")))?;
    let enc = |v: &str| {
        crypto::encrypt(&key, v.as_bytes())
            .map_err(|e| loco_rs::Error::string(&format!("seed git: {e:?}")))
    };
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL).await?;
    let now = chrono::Utc::now();
    let ago = |m: i64| -> chrono::DateTime<chrono::FixedOffset> {
        (now - chrono::Duration::minutes(m)).into()
    };
    let conn = git_connections::ActiveModel {
        workspace_id: Set(wid.get()),
        provider: Set("github".into()),
        host: Set("github.com".into()),
        api_base: Set(settings.git.github_api_base.clone()),
        web_base: Set(settings.git.github_web_base.clone()),
        account: Set("pt-contoh-bot".into()),
        token_encrypted: Set(enc("ghp_seed_token_tidak_berlaku")?),
        webhook_secret_encrypted: Set(enc(&crypto::random_token())?),
        created_by: Set(owner.map(|o| o.id)),
        last_event_at: Set(Some(ago(2))),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    let mut linked = Vec::new();
    for (i, name) in ["kasir-api", "kasir-web"].into_iter().enumerate() {
        let existing = repos::Model::of_project(&ctx.db, wid, project.id)
            .await?
            .into_iter()
            .find(|r| r.name == name);
        let repo = match existing {
            Some(r) => r,
            None => {
                repos::ActiveModel {
                    workspace_id: Set(wid.get()),
                    project_id: Set(project.id),
                    name: Set(name.into()),
                    provider: Set("github".into()),
                    url: Set(format!("https://github.com/pt-contoh/{name}")),
                    default_branch: Set("main".into()),
                    ..Default::default()
                }
                .insert(&ctx.db)
                .await?
            }
        };
        let mut m: repos::ActiveModel = repo.into();
        m.connection_id = Set(Some(conn.id));
        m.full_name = Set(format!("pt-contoh/{name}"));
        m.webhook_id = Set(Some(9001 + i64::try_from(i).unwrap_or(0)));
        m.last_event_at = Set(Some(ago(2 + 2 * i64::try_from(i).unwrap_or(0))));
        linked.push(m.update(&ctx.db).await?);
    }
    let ticket_rows = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(project.id))
        .filter(tickets::Column::DeletedAt.is_null())
        .order_by_asc(tickets::Column::Number)
        .all(&ctx.db)
        .await?;
    let checks = |states: &[(&str, &str)]| {
        serde_json::Value::from(
            states
                .iter()
                .map(|(n, s)| serde_json::json!({ "name": n, "state": s, "url": "" }))
                .collect::<Vec<_>>(),
        )
    };
    let samples = [
        (
            0usize,
            142,
            "feat(pembayaran): integrasi QRIS",
            "open",
            "success",
            checks(&[
                ("build", "success"),
                ("cargo test", "success"),
                ("clippy", "success"),
            ]),
            302,
            41,
            40,
        ),
        (
            1,
            87,
            "feat(bayar): panel QRIS di layar kasir",
            "open",
            "failure",
            checks(&[("build", "success"), ("e2e playwright", "failure")]),
            120,
            18,
            25,
        ),
        (
            0,
            131,
            "feat(auth): login kasir dengan PIN 6 digit",
            "merged",
            "success",
            checks(&[("build", "success"), ("cargo test", "success")]),
            210,
            33,
            4300,
        ),
    ];
    for (n, (repo_i, number, title, state, ci, checks, add, del, mins)) in
        samples.into_iter().enumerate()
    {
        let Some(ticket) = ticket_rows.get(n + 1) else {
            break;
        };
        let repo = &linked[repo_i];
        let branch = agentgate_core::git::render_branch(
            agentgate_core::git::DEFAULT_BRANCH_PATTERN,
            &ticket.code,
            &ticket.title,
            "feature",
        );
        pull_requests::ActiveModel {
            workspace_id: Set(wid.get()),
            project_id: Set(project.id),
            repo_id: Set(repo.id),
            ticket_id: Set(Some(ticket.id)),
            number: Set(number),
            title: Set(title.into()),
            url: Set(format!(
                "https://github.com/{}/pull/{number}",
                repo.full_name
            )),
            head_branch: Set(branch),
            base_branch: Set("main".into()),
            head_sha: Set(format!("{:07x}{number}", number * 7919)),
            state: Set(state.into()),
            ci_state: Set(ci.into()),
            checks: Set(checks),
            additions: Set(add),
            deletions: Set(del),
            author: Set("pt-contoh-bot".into()),
            merged_at: Set((state == "merged").then(|| ago(mins))),
            closed_at: Set((state == "merged").then(|| ago(mins))),
            created_at: Set(ago(mins + 60)),
            updated_at: Set(ago(mins)),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?;
    }
    Ok(())
}

async fn seed_runner(ctx: &AppContext) -> Result<()> {
    use agentgate_core::runner::{
        protocol::{
            EvidenceKind, EvidenceSummary, JobStatus, LogStream, PermissionAsk, Step, StepState,
        },
        sha256_hex,
        signing::{evidence_payload, public_key_b64, sign},
    };
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
    };

    use crate::{
        models::{job_evidence, jobs as job_model, runners, tickets},
        services::{
            jobs,
            tickets::{self as ticket_service, stages::Flow},
            workspaces::Acting,
        },
    };
    const SECRET: [u8; 32] = [11; 32];
    let Some(workspace) = workspaces::Model::find_by_slug(&ctx.db, SEED_WORKSPACE_SLUG).await?
    else {
        return Ok(());
    };
    let wid = workspace.workspace_id();
    let Some(project) = projects::Model::find_by_key(&ctx.db, wid, SEED_PROJECT.0).await? else {
        return Ok(());
    };
    if !jobs::runners_of(&ctx.db, wid, None).await?.is_empty() {
        tracing::info!("seed runner dilewati: runner sudah ada");
        return Ok(());
    }
    let owner = users::Model::find_by_email(&ctx.db, SEED_MARKER_EMAIL)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed tidak ada"))?;
    let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
        .await?
        .ok_or_else(|| loco_rs::Error::string("owner seed bukan anggota"))?;
    let acting = Acting {
        workspace: &workspace,
        user: &owner,
        role: &role,
        request_id: None,
    };
    let fail = |e: &dyn std::fmt::Debug| loco_rs::Error::string(&format!("seed runner: {e:?}"));
    let now = chrono::Utc::now();
    let runner = runners::ActiveModel {
        workspace_id: Set(wid.get()),
        user_id: Set(owner.id),
        name: Set("MacBook Olivia".into()),
        os: Set("macOS 15".into()),
        client: Set("claude".into()),
        version: Set(agentgate_core::VERSION.into()),
        public_key: Set(public_key_b64(&SECRET)),
        connected_at: Set(Some(now.into())),
        last_seen_at: Set(Some(now.into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    if let Some(dev) = users::Model::find_by_email(&ctx.db, "dev@agentgate.test").await? {
        runners::ActiveModel {
            workspace_id: Set(wid.get()),
            user_id: Set(dev.id),
            name: Set("ThinkPad Dewi".into()),
            os: Set("Ubuntu 24.04".into()),
            client: Set("claude".into()),
            version: Set(agentgate_core::VERSION.into()),
            public_key: Set(public_key_b64(&[12; 32])),
            last_seen_at: Set(Some((now - chrono::Duration::hours(3)).into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?;
    }

    // Story di Eksekusi (seed M3, sebelum ada job): job berjalan di runner owner.
    let in_execute = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ProjectId.eq(project.id))
        .filter(tickets::Column::StageKey.eq("execute"))
        .one(&ctx.db)
        .await?;
    if let Some(ticket) = in_execute {
        let flow = Flow::load(&ctx.db, wid, &ticket).await?;
        if let Some(stage) = flow.rule("execute").cloned()
            && let Some(job) = jobs::on_stage_entered(&ctx.db, wid, &ticket, &stage).await?
        {
            jobs::claim(&ctx.db, &acting, job.id, Some(runner.id))
                .await
                .map_err(|e| fail(&e))?;
            let steps = [
                ("Baca konteks dan rencana", StepState::Ok, Some(88)),
                (
                    "Tes gagal untuk diskon bertingkat",
                    StepState::Ok,
                    Some(101),
                ),
                ("Implementasi aturan diskon", StepState::Ok, Some(456)),
                ("Menulis tes kasus tepi", StepState::Now, None),
                ("Commit di branch", StepState::Wait, None),
                ("Ajukan review", StepState::Wait, None),
            ]
            .map(|(name, state, seconds)| Step {
                name: name.into(),
                state,
                seconds,
            });
            let branch = agentgate_core::runner::branch_name(&ticket.code, &ticket.title);
            let mut m = job_model::Entity::find_by_id(job.id)
                .one(&ctx.db)
                .await?
                .ok_or_else(|| loco_rs::Error::string("job seed hilang"))?
                .into_active_model();
            m.status = Set(JobStatus::Running.as_str().into());
            m.client = Set("claude".into());
            m.model = Set("claude-sonnet-4-5".into());
            m.branch = Set(branch.clone());
            m.started_at = Set(Some((now - chrono::Duration::minutes(19)).into()));
            m.tokens = Set(Some(48_200));
            m.steps = Set(serde_json::to_value(steps).unwrap_or_default());
            m.update(&ctx.db).await?;
            for (stream, text) in [
                (LogStream::Info, format!("Job {} · stage Eksekusi + TDD · runner MacBook Olivia", ticket.code)),
                (LogStream::Info, format!("worktree ~/.local/state/agentgate/wt/kasir-api/{branch}")),
                (LogStream::Ok, "✓ aturan ditulis dari server".to_string()),
                (LogStream::Out, "claude -p --setting-sources project --strict-mcp-config --model claude-sonnet-4-5".to_string()),
                (LogStream::Tool, format!("→ ag_context {} · rencana v1 · 3 kriteria", ticket.code)),
                (LogStream::Ok, "✓ langkah 1 Baca konteks dan rencana".to_string()),
                (LogStream::Ok, "✓ langkah 2 Tes gagal untuk diskon bertingkat".to_string()),
                (LogStream::Ok, "✓ langkah 3 Implementasi aturan diskon".to_string()),
                (LogStream::Out, "$ cargo test -p kasir-api diskon::".to_string()),
                (LogStream::Ok, "  test diskon::member_gold_10_persen ... ok".to_string()),
                (LogStream::Fail, "  test diskon::member_bertumpuk_promo ... FAILED".to_string()),
                (LogStream::Tool, "→ ag_search \"diskon bertumpuk promo\" · 2 hasil".to_string()),
                (LogStream::Out, "  edit src/diskon/member.rs +14 −3".to_string()),
                (LogStream::Ok, "  test result: ok. 9 passed; 0 failed".to_string()),
            ] {
                jobs::log(&ctx.db, wid, job.id, stream, &text).await?;
            }
            let files: [(EvidenceKind, &str, &str, Vec<u8>, EvidenceSummary); 2] = [
                (
                    EvidenceKind::TestReport,
                    "test-diskon.xml",
                    "application/xml",
                    b"<testsuite name=\"diskon\" tests=\"9\" failures=\"0\"/>".to_vec(),
                    EvidenceSummary {
                        passed: Some(true),
                        total: Some(9),
                        failed: Some(0),
                        note: "cargo test -p kasir-api diskon::".into(),
                    },
                ),
                (
                    EvidenceKind::Screenshot,
                    "checkout-diskon.png",
                    "image/png",
                    include_bytes!("fixtures/evidence-checkout.png").to_vec(),
                    EvidenceSummary::default(),
                ),
            ];
            for (kind, name, content_type, data, summary) in files {
                let id = uuid::Uuid::now_v7();
                let key = format!("evidence/{}/{id}", job.id);
                ctx.storage
                    .upload(Path::new(&key), &axum::body::Bytes::from(data.clone()))
                    .await
                    .map_err(|e| fail(&e))?;
                let sha = sha256_hex(&data);
                let signature = sign(
                    &SECRET,
                    &evidence_payload(job.id, kind, name, &sha, &summary),
                );
                job_evidence::ActiveModel {
                    id: Set(id),
                    workspace_id: Set(wid.get()),
                    job_id: Set(job.id),
                    ticket_id: Set(ticket.id),
                    stage_key: Set(ticket.stage_key.clone()),
                    runner_id: Set(Some(runner.id)),
                    kind: Set(kind.as_str().into()),
                    name: Set(name.into()),
                    content_type: Set(content_type.into()),
                    size_bytes: Set(i32::try_from(data.len()).unwrap_or(i32::MAX)),
                    sha256: Set(sha),
                    storage_key: Set(key),
                    passed: Set(summary.passed),
                    total: Set(summary.total.and_then(|n| i32::try_from(n).ok())),
                    failed: Set(summary.failed.and_then(|n| i32::try_from(n).ok())),
                    note: Set(summary.note.clone()),
                    signature: Set(signature),
                    ..Default::default()
                }
                .insert(&ctx.db)
                .await?;
            }

            // @agent-qa di komentar tiket ini: job komentar menunggu izin aksi.
            ticket_service::comments::post(
                &ctx.db,
                &acting,
                ticket.id,
                "@agent-qa tolong jalankan tes end-to-end checkout dengan diskon member.",
            )
            .await
            .map_err(|e| fail(&e))?;
            if let Some(cjob) = jobs::of_comments(&ctx.db, wid, ticket.id).await?.pop() {
                let mut m = cjob.clone().into_active_model();
                m.status = Set(JobStatus::Running.as_str().into());
                m.runner_id = Set(Some(runner.id));
                m.client = Set("claude".into());
                let cjob = m.update(&ctx.db).await?;
                jobs::log(
                    &ctx.db,
                    wid,
                    cjob.id,
                    LogStream::Info,
                    "Job komentar · Agent QA · runner MacBook Olivia",
                )
                .await?;
                let session_less = PermissionAsk {
                    command: "npx playwright install chromium".into(),
                    action: "install_package".into(),
                    reason: "Browser Playwright belum terpasang; dibutuhkan untuk tes end-to-end checkout.".into(),
                };
                jobs::runner::ask_permission(&ctx.db, wid, &cjob, &session_less)
                    .await
                    .map_err(|e| fail(&e))?;
            }
        }
    }

    // Story baru melewati Brainstorming dan Rencana: job Eksekusi antre untuk diambil.
    let types = crate::models::ticket_types::Model::all_in(&ctx.db, wid).await?;
    if let Some(story) = types.iter().find(|t| t.key == "story") {
        let form = ticket_service::create::TicketForm {
            input: crate::models::tickets::TicketInput {
                title: "Struk digital via WhatsApp".into(),
                description: "Kirim struk ke nomor WhatsApp pelanggan setelah transaksi.".into(),
                status: "in_progress".into(),
                priority: "urgent".into(),
                labels: "struk".into(),
                due_on: String::new(),
                story_points: "3".into(),
            },
            type_id: story.id.to_string(),
            assignee: String::new(),
            parent: String::new(),
            custom: std::collections::BTreeMap::new(),
            stage: "brainstorm".into(),
            sprint: None,
            release: None,
            component: None,
        };
        let t = ticket_service::create::run(&ctx.db, &acting, project.id, &form)
            .await
            .map_err(|e| fail(&e))?;
        for body in [
            "# Brainstorm\n\nStruk dikirim lewat WhatsApp Business API.",
            "# Rencana implementasi\n\n- [ ] Template pesan\n- [ ] Kirim setelah lunas",
        ] {
            let a = ticket_service::artifacts::create(&ctx.db, &acting, t.id, "")
                .await
                .map_err(|e| fail(&e))?;
            ticket_service::artifacts::save(&ctx.db, &acting, t.id, a.id, body)
                .await
                .map_err(|e| fail(&e))?;
            ticket_service::stages::complete(&ctx.db, &acting, t.id)
                .await
                .map_err(|e| fail(&e))?;
            ticket_service::stages::approve(&ctx.db, &acting, t.id, "")
                .await
                .map_err(|e| fail(&e))?;
        }
    }
    Ok(())
}
