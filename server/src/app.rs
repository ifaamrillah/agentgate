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

    async fn after_routes(router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        // CSRF membungkus semua route (di luar stack Loco, sehingga berjalan sebelum gate login
        // di `middlewares`); penolakannya dirender sebagai halaman HTML oleh layer html_errors
        // yang dipasang view engine di luar layer ini.
        Ok(router.layer(axum::middleware::from_fn(
            controllers::middleware::csrf::layer,
        )))
    }

    async fn connect_workers(ctx: &AppContext, queue: &Queue) -> Result<()> {
        // Pembersihan harian; jadwalnya lewat task `cleanup` di bagian `scheduler` config.
        queue
            .register(crate::workers::cleanup::CleanupWorker::build(ctx))
            .await?;
        Ok(())
    }

    fn register_tasks(tasks: &mut Tasks) {
        #[cfg(debug_assertions)]
        tasks.register(crate::tasks::dev_login::DevLogin);
        tasks.register(crate::tasks::cleanup::Cleanup);
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
        seed_workspace(ctx).await
    }
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
