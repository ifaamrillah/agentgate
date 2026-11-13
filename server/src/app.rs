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
        // Index code graph oleh server (M7).
        queue
            .register(crate::workers::graph_index::GraphIndexWorker::build(ctx))
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
        seed_workspace(ctx).await?;
        seed_tickets(ctx).await?;
        seed_workflows(ctx).await?;
        seed_agents(ctx).await?;
        seed_runner(ctx).await?;
        seed_git(ctx).await
    }
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
