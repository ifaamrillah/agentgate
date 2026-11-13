//! Binary `agentgate`: dijalankan user di laptopnya untuk login, memasang aturan Agent Gate ke
//! repo, mengelola antrean job, dan menjalankan runner job.

mod api;
mod auth;
mod config;
mod doctor;
mod error;
mod git;
mod graph;
mod hook;
mod install;
mod jobs;
mod knowledge;
mod runner;
mod ui;

#[cfg(test)]
mod testutil;

use std::{io, process::ExitCode};

use clap::{Parser, Subcommand};

use crate::{
    config::{Credentials, Paths},
    error::{Error, Result},
};

#[derive(Parser)]
#[command(name = "agentgate", version = agentgate_core::VERSION, about = "Agent Gate CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Login ke server Agent Gate (kode perangkat lewat browser).
    Login {
        /// URL server, mis. https://agentgate.example.com
        #[arg(long, env = "AGENTGATE_SERVER")]
        server: Option<String>,
    },
    /// Hapus login di perangkat ini.
    Logout,
    /// Akun, runner, dan status aturan repo saat ini.
    Status,
    /// Pasang aturan Agent Gate di repo ini (aturan lama di-backup) lalu commit.
    Init {
        /// Kode project (mis. KSR).
        #[arg(long)]
        project: Option<String>,
        /// Jangan bertanya konfirmasi.
        #[arg(long)]
        yes: bool,
    },
    /// Pulihkan aturan lama dari backup terbaru lalu commit.
    Uninstall {
        #[arg(long)]
        yes: bool,
    },
    /// Cek atau sinkronkan file aturan dengan server.
    Rules {
        #[command(subcommand)]
        action: RulesCmd,
    },
    /// Antrean job yang bisa Anda lihat.
    Jobs,
    /// Ambil, lepas, batalkan, atau buka sesi interaktif satu job.
    Job {
        #[command(subcommand)]
        action: JobCmd,
    },
    /// Runner job di perangkat ini.
    Runner {
        #[command(subcommand)]
        action: RunnerCmd,
    },
    /// Hook client AI (dipanggil dari .claude/settings.json).
    Hook {
        #[command(subcommand)]
        event: HookCmd,
    },
    /// Code graph repo ini: index, SCIP, status.
    Graph {
        #[command(subcommand)]
        action: GraphCmd,
    },
    /// Dokumen knowledge project di .agentgate/knowledge/: tarik, status, sinkron.
    Knowledge {
        #[command(subcommand)]
        action: KnowledgeCmd,
    },
    /// Periksa kesiapan perangkat ini.
    Doctor,
}

#[derive(Subcommand)]
enum KnowledgeCmd {
    /// Tulis dokumen server ke .agentgate/knowledge/ (file yang diubah di repo dilewati).
    Pull {
        #[arg(long)]
        project: Option<String>,
        /// Timpa juga file yang diubah di repo.
        #[arg(long)]
        force: bool,
    },
    /// Bandingkan file lokal dengan server.
    Status {
        #[arg(long)]
        project: Option<String>,
    },
    /// Kirim perubahan lokal (versi baru atau usulan), tarik versi server, lalu commit.
    Sync {
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum GraphCmd {
    /// Index repo ini (hanya file yang berubah sejak index terakhir).
    Index {
        /// Kirim ulang semua file.
        #[arg(long)]
        full: bool,
        /// Kode project (bawaan: .agentgate/project.toml).
        #[arg(long)]
        project: Option<String>,
        /// Nama repo di project (bawaan: nama folder repo).
        #[arg(long)]
        repo: Option<String>,
    },
    /// Kirim index SCIP (mis. hasil `scip-typescript index`) agar relasi lebih presisi.
    Scip {
        /// Berkas SCIP (bawaan: index.scip di root repo).
        file: Option<std::path::PathBuf>,
        /// Nama indexer yang ditampilkan (bawaan: dari metadata SCIP).
        #[arg(long, default_value = "")]
        indexer: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        repo: Option<String>,
    },
    /// Status index semua repo project.
    Status {
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum RulesCmd {
    /// Bandingkan checksum file aturan dengan versi server.
    Check,
    /// Tulis aturan versi server di branch agentgate/sync-rules lalu commit.
    Sync,
}

#[derive(Subcommand)]
enum JobCmd {
    /// Ambil job (Anda menjadi pemegang lock).
    Claim { id: String },
    /// Lepas job kembali ke antrean.
    Release { id: String },
    /// Batalkan job.
    Cancel { id: String },
    /// Buka sesi client interaktif di worktree job.
    Attach { id: String },
}

#[derive(Subcommand)]
enum RunnerCmd {
    /// Nyalakan runner dan jalankan job yang siap.
    Start {
        /// Jangan bertanya konfirmasi.
        #[arg(long)]
        yes: bool,
        /// Berhenti setelah satu job selesai.
        #[arg(long)]
        once: bool,
    },
}

#[derive(Subcommand)]
enum HookCmd {
    /// PreToolUse: izin aksi berbahaya dan RTK.
    PreTool,
    /// SessionStart: konteks job/project.
    SessionStart,
}

async fn run(cmd: Cmd, paths: &Paths) -> Result<ExitCode> {
    let mut out = io::stdout();
    match cmd {
        Cmd::Login { server } => {
            let server = server
                .or_else(|| Credentials::load(paths).ok().flatten().map(|c| c.server))
                .ok_or(Error::ServerRequired)?;
            let creds = auth::login(paths, &server, &mut out).await?;
            println!(
                "\nLogin berhasil sebagai {} di workspace {}.",
                creds.email, creds.workspace
            );
        }
        Cmd::Logout => {
            if auth::logout(paths).await? {
                println!("Anda sudah logout dari perangkat ini.");
            } else {
                println!("Anda belum login.");
            }
        }
        Cmd::Status => doctor::status(paths, &mut out).await?,
        Cmd::Init { project, yes } => {
            let (api, _) = auth::session(paths).await?;
            let res = install::init(paths, &api, project.as_deref(), yes, &mut out).await?;
            println!(
                "\nAgent Gate terpasang: {} file aturan lama di-backup, {} file aturan ditulis{}.",
                res.backed_up.len(),
                res.written.len(),
                if res.committed {
                    ", perubahan di-commit"
                } else {
                    " (tidak ada perubahan untuk di-commit)"
                }
            );
            println!(
                "Isi `[test] command` di .agentgate/project.toml supaya runner bisa membuat \
                 evidence tes."
            );
        }
        Cmd::Uninstall { yes } => {
            let manifest = install::uninstall(paths, yes, &mut out)?;
            println!(
                "Aturan lama dipulihkan dari {} dan di-commit.",
                manifest.archive
            );
        }
        Cmd::Rules { action } => {
            let (api, _) = auth::session(paths).await?;
            match action {
                RulesCmd::Check => install::check(&api, &mut out).await?,
                RulesCmd::Sync => install::sync(&api, &mut out).await?,
            }
        }
        Cmd::Jobs => {
            let (api, _) = auth::session(paths).await?;
            print!("{}", jobs::table(&jobs::list(&api).await?));
        }
        Cmd::Job { action } => {
            let (api, _) = auth::session(paths).await?;
            match action {
                JobCmd::Claim { id } => {
                    jobs::act(paths, &api, &id, jobs::Action::Claim, &mut out).await?;
                }
                JobCmd::Release { id } => {
                    jobs::act(paths, &api, &id, jobs::Action::Release, &mut out).await?;
                }
                JobCmd::Cancel { id } => {
                    jobs::act(paths, &api, &id, jobs::Action::Cancel, &mut out).await?;
                }
                JobCmd::Attach { id } => {
                    let ctx = runner::registered(paths, &api)?;
                    let id = jobs::resolve(&api, &id).await?;
                    runner::job::attach(&ctx, id, &mut out).await?;
                }
            }
        }
        Cmd::Runner {
            action: RunnerCmd::Start { yes, once },
        } => runner::start(paths, yes, once, &mut out).await?,
        Cmd::Hook { event } => match event {
            HookCmd::PreTool => hook::pre_tool(paths).await,
            HookCmd::SessionStart => hook::session_start(paths),
        },
        Cmd::Knowledge { action } => {
            let (api, _) = auth::session(paths).await?;
            match action {
                KnowledgeCmd::Pull { project, force } => {
                    knowledge::pull(&api, project.as_deref(), force, &mut out).await?;
                }
                KnowledgeCmd::Status { project } => {
                    knowledge::status(&api, project.as_deref(), &mut out).await?;
                }
                KnowledgeCmd::Sync { project } => {
                    knowledge::sync(&api, project.as_deref(), &mut out).await?;
                }
            }
        }
        Cmd::Graph { action } => {
            let (api, _) = auth::session(paths).await?;
            match action {
                GraphCmd::Index {
                    full,
                    project,
                    repo,
                } => {
                    graph::index(&api, project.as_deref(), repo.as_deref(), full, &mut out).await?;
                }
                GraphCmd::Scip {
                    file,
                    indexer,
                    project,
                    repo,
                } => {
                    graph::scip(
                        &api,
                        project.as_deref(),
                        repo.as_deref(),
                        file.as_deref(),
                        &indexer,
                        &mut out,
                    )
                    .await?;
                }
                GraphCmd::Status { project } => {
                    graph::status(&api, project.as_deref(), &mut out).await?;
                }
            }
        }
        Cmd::Doctor => {
            if !doctor::doctor(paths, &mut out).await? {
                return Ok(ExitCode::FAILURE);
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(cmd) = cli.command else {
        use clap::CommandFactory;
        let _ = Cli::command().print_help();
        return ExitCode::SUCCESS;
    };
    // reqwest dan WebSocket sama-sama memakai rustls dengan provider ring.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Runtime tidak bisa dibuat: {e}");
            return ExitCode::FAILURE;
        }
    };
    let paths = Paths::from_env();
    match runtime.block_on(run(cmd, &paths)) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{}", e.user_message());
            ExitCode::FAILURE
        }
    }
}
