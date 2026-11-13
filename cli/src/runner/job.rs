//! Eksekusi satu job di runner: `start` → worktree → aturan → client AI → tes → evidence →
//! commit lokal → `finish`. Juga `agentgate job attach` (sesi interaktif di terminal).

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
};

use agentgate_core::{
    git::CodeOutput,
    runner::protocol::{
        ChangedFile, FinishReport, JobKind, JobMode, JobSpec, LogBatch, LogLine, LogStream,
        Outcome, PullRequestInfo, PullRequestOpen, Step, StepState, StepsUpdate, TokenUsage,
    },
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::watch,
};
use uuid::Uuid;

use super::{
    clients::{self, ClientKind, Finished, Parsed},
    evidence, terminal, worktree,
};
use crate::{
    api::Api,
    config::{self, JobState, Paths, ProjectFile, Repos},
    error::{Error, Result},
    ui,
};

/// Batas baris per kiriman log (server: 500).
const LOG_BATCH: usize = 200;
/// Batas waktu `[test] command` (detik).
const TEST_TIMEOUT: u64 = 30 * 60;

/// Semua yang dibutuhkan untuk menjalankan job di perangkat ini.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub paths: Paths,
    /// Client API dengan header runner.
    pub api: Api,
    pub runner_id: Uuid,
    pub secret: [u8; 32],
    /// Client default runner, dipakai bila `JobSpec.client` tidak dikenal.
    pub client: ClientKind,
}

impl Ctx {
    /// Client job: `JobSpec.client` (dipilih server), atau client default runner.
    #[must_use]
    pub fn client_for(&self, spec: &JobSpec) -> ClientKind {
        ClientKind::parse(&spec.client).unwrap_or(self.client)
    }

    fn job_path(&self, id: Uuid, tail: &str) -> String {
        format!("/api/runner/jobs/{id}/{tail}")
    }
}

/// Log job: dicetak lokal dan dikirim ke server per batch.
struct Logger {
    api: Api,
    path: String,
    prefix: String,
    buf: Vec<LogLine>,
}

impl Logger {
    fn new(ctx: &Ctx, id: Uuid, ticket: &str) -> Self {
        Self {
            api: ctx.api.clone(),
            path: ctx.job_path(id, "logs"),
            prefix: if ticket.is_empty() {
                id.to_string()[..8].to_string()
            } else {
                ticket.to_string()
            },
            buf: Vec::new(),
        }
    }

    fn push(&mut self, stream: LogStream, text: impl Into<String>) {
        let text = text.into();
        println!(
            "[{}] {:<4} {}",
            self.prefix,
            stream.as_str(),
            ui::clip(&text, 200)
        );
        self.buf.push(LogLine { stream, text });
    }

    async fn flush(&mut self) {
        while !self.buf.is_empty() {
            let n = self.buf.len().min(LOG_BATCH);
            let lines: Vec<LogLine> = self.buf.drain(..n).collect();
            if let Err(e) = self.api.post_unit(&self.path, &LogBatch { lines }).await {
                eprintln!("[{}] log tidak terkirim: {e}", self.prefix);
            }
        }
    }
}

/// Langkah job: fase runner + rencana agent (`TodoWrite`).
struct Steps {
    prepare: StepState,
    agent: StepState,
    plan: Vec<Step>,
    tests: StepState,
    evidence: StepState,
}

impl Steps {
    fn new() -> Self {
        Self {
            prepare: StepState::Now,
            agent: StepState::Wait,
            plan: Vec::new(),
            tests: StepState::Wait,
            evidence: StepState::Wait,
        }
    }

    fn render(&self) -> Vec<Step> {
        let step = |name: &str, state| Step {
            name: name.into(),
            state,
            seconds: None,
        };
        let mut out = vec![step("Siapkan worktree", self.prepare)];
        if self.plan.is_empty() {
            out.push(step("Jalankan agent", self.agent));
        } else {
            out.extend(self.plan.iter().take(35).cloned());
        }
        out.push(step("Jalankan tes", self.tests));
        out.push(step("Kirim evidence", self.evidence));
        out
    }

    async fn post(&self, ctx: &Ctx, id: Uuid) {
        let _ = ctx
            .api
            .post_unit(
                &ctx.job_path(id, "steps"),
                &StepsUpdate {
                    steps: self.render(),
                },
            )
            .await;
    }
}

/// Hasil menjalankan client.
#[derive(Debug)]
enum Agent {
    Done {
        ok: bool,
        summary: String,
        tokens: Option<u64>,
        usage: Option<TokenUsage>,
        cost_usd: Option<f64>,
    },
    Quota {
        text: String,
    },
    Cancelled,
}

/// `POST start` → `JobSpec`.
///
/// # Errors
///
/// Lihat [`crate::api::ApiError`].
pub async fn start(ctx: &Ctx, id: Uuid) -> Result<JobSpec> {
    Ok(ctx
        .api
        .post(&ctx.job_path(id, "start"), &serde_json::json!({}))
        .await?)
}

/// Repo lokal + worktree + aturan + state hook untuk `spec`.
///
/// # Errors
///
/// Repo tidak ditemukan, git/IO gagal.
pub fn prepare(ctx: &Ctx, spec: &JobSpec) -> Result<PathBuf> {
    let repos = Repos::load(&ctx.paths)?;
    let repo = repos
        .find_for(&spec.project, &spec.repos)
        .ok_or_else(|| Error::RepoNotFound(spec.project.clone()))?;
    let wt = worktree::path_for(&ctx.paths.state, &repo.name, &spec.branch);
    worktree::prepare(&repo.path, &wt, &spec.branch, &spec.base_branch)?;
    worktree::write_rules(&wt, &spec.rules)?;
    JobState::from_spec(spec, ctx.runner_id).save(&ctx.paths)?;
    Ok(wt)
}

/// Perintah tes dari `.agentgate/project.toml` worktree.
fn test_command(wt: &Path) -> String {
    ProjectFile::load(wt)
        .ok()
        .flatten()
        .map(|p| p.test.command.trim().to_string())
        .unwrap_or_default()
}

/// Jalankan `[test] command` (konfigurasi repo, bukan data tiket) lewat shell OS.
async fn run_tests(wt: &Path, command: &str) -> (bool, String) {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };
    cmd.current_dir(wt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let run = tokio::time::timeout(config::seconds(TEST_TIMEOUT), cmd.output()).await;
    match run {
        Ok(Ok(out)) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            let err = String::from_utf8_lossy(&out.stderr);
            if !err.trim().is_empty() {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&err);
            }
            (out.status.success(), text)
        }
        Ok(Err(e)) => (false, format!("tes tidak bisa dijalankan: {e}")),
        Err(_) => (false, "tes melewati batas waktu".into()),
    }
}

/// Env job untuk proses client: hook `agentgate hook` membaca job dan client dari sini.
fn job_env(cmd: &mut Command, ctx: &Ctx, spec: &JobSpec, client: ClientKind) {
    cmd.env("AGENTGATE_JOB_ID", spec.id.to_string())
        .env(crate::hook::JOB_CLIENT_ENV, client.key())
        .env("AGENTGATE_CONFIG_DIR", &ctx.paths.config)
        .env("AGENTGATE_STATE_DIR", &ctx.paths.state);
}

/// State aliran output client selama job.
#[derive(Default)]
struct Stream {
    finished: Option<Finished>,
    quota: Option<String>,
}

impl Stream {
    /// Catat satu hasil parser: log, langkah, hasil akhir, deteksi kuota.
    async fn absorb(
        &mut self,
        parsed: Parsed,
        ctx: &Ctx,
        spec: &JobSpec,
        log: &mut Logger,
        steps: &mut Steps,
    ) {
        for entry in parsed.logs {
            if self.quota.is_none() && clients::is_quota(&entry.text) {
                self.quota = Some(entry.text.clone());
            }
            log.push(entry.stream, entry.text);
        }
        if let Some(plan) = parsed.steps {
            steps.plan = plan;
            steps.post(ctx, spec.id).await;
        }
        if let Some(f) = parsed.finished {
            if self.quota.is_none() && f.is_error && clients::is_quota(&f.text) {
                self.quota = Some(f.text.clone());
            }
            self.finished = Some(f);
        }
    }
}

/// Jalankan client headless dan alirkan output-nya ke log.
async fn run_agent(
    ctx: &Ctx,
    spec: &JobSpec,
    wt: &Path,
    log: &mut Logger,
    steps: &mut Steps,
    cancel: &mut watch::Receiver<bool>,
) -> Result<Agent> {
    let client = ctx.client_for(spec);
    let program = client.program();
    let invocation = client.auto(spec, wt);
    log.push(
        LogStream::Info,
        format!("Client {} ({program})", client.name()),
    );
    let mut cmd = Command::new(&program);
    cmd.args(&invocation.args).current_dir(wt);
    job_env(&mut cmd, ctx, spec, client);
    for (key, value) in &invocation.env {
        cmd.env(key, value);
    }
    let mut child = cmd
        .stdin(if invocation.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| Error::Spawn {
            program: program.clone(),
            source,
        })?;
    if let (Some(prompt), Some(mut stdin)) = (invocation.stdin, child.stdin.take()) {
        // Ditulis di task sendiri: client bisa mulai menulis stdout sebelum stdin habis dibaca.
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(prompt.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });
    }
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Ok(Agent::Done {
            ok: false,
            summary: "output client tidak bisa dibaca".into(),
            tokens: None,
            usage: None,
            cost_usd: None,
        });
    };
    let mut out = BufReader::new(stdout).lines();
    let mut err = BufReader::new(stderr).lines();
    let (mut out_open, mut err_open, mut cancel_open) = (true, true, true);
    let mut parser = client.parser();
    let mut stream = Stream::default();
    let mut tick = tokio::time::interval(config::seconds(2));
    while out_open || err_open {
        tokio::select! {
            line = out.next_line(), if out_open => match line {
                Ok(Some(l)) => stream.absorb(parser.line(&l), ctx, spec, log, steps).await,
                _ => {
                    out_open = false;
                    stream.absorb(parser.eof(), ctx, spec, log, steps).await;
                }
            },
            line = err.next_line(), if err_open => match line {
                Ok(Some(l)) if !l.trim().is_empty() => {
                    if stream.quota.is_none() && clients::is_quota(&l) {
                        stream.quota = Some(l.clone());
                    }
                    log.push(LogStream::Err, ui::clip(l.trim(), 2_000));
                }
                Ok(Some(_)) => {}
                _ => err_open = false,
            },
            _ = tick.tick() => log.flush().await,
            changed = cancel.changed(), if cancel_open => match changed {
                Ok(()) if *cancel.borrow() => {
                    let _ = child.kill().await;
                    return Ok(Agent::Cancelled);
                }
                Ok(()) => {}
                Err(_) => cancel_open = false,
            },
        }
    }
    let status = child.wait().await.map_err(|source| Error::Spawn {
        program: program.clone(),
        source,
    })?;
    if let Some(text) = stream.quota {
        return Ok(Agent::Quota { text });
    }
    let (ok, summary, tokens, usage, cost_usd) = match stream.finished {
        Some(f) => (
            status.success() && !f.is_error,
            f.text,
            f.tokens,
            f.usage,
            f.cost_usd,
        ),
        None => (
            false,
            format!("client berhenti tanpa hasil (status {status})"),
            None,
            None,
            None,
        ),
    };
    Ok(Agent::Done {
        ok,
        summary,
        tokens,
        usage,
        cost_usd,
    })
}

/// Tes + evidence + commit lokal setelah agent selesai.
async fn wrap_up(ctx: &Ctx, spec: &JobSpec, wt: &Path, log: &mut Logger, steps: &mut Steps) {
    let command = test_command(wt);
    steps.tests = StepState::Now;
    steps.post(ctx, spec.id).await;
    if command.is_empty() {
        log.push(
            LogStream::Warn,
            "Belum ada [test] command di .agentgate/project.toml; evidence tes dilewati.",
        );
        steps.tests = StepState::Ok;
    } else {
        log.push(LogStream::Info, format!("$ {command}"));
        log.flush().await;
        let (passed, output) = run_tests(wt, &command).await;
        let report = evidence::test_report(&ctx.secret, spec.id, &command, passed, &output);
        let summary = &report.summary;
        let counts = match (summary.total, summary.failed) {
            (Some(t), Some(f)) => format!(" ({t} tes, {f} gagal)"),
            _ => String::new(),
        };
        if passed {
            log.push(LogStream::Ok, format!("Tes lulus{counts}"));
        } else {
            log.push(LogStream::Fail, format!("Tes gagal{counts}"));
        }
        steps.tests = if passed {
            StepState::Ok
        } else {
            StepState::Fail
        };
        upload(ctx, spec.id, &report, log).await;
    }
    steps.evidence = StepState::Now;
    steps.post(ctx, spec.id).await;
    let (shots, skipped) = evidence::screenshots(&ctx.secret, spec.id, wt);
    for name in skipped {
        log.push(
            LogStream::Warn,
            format!("Screenshot {name} dilewati (kosong atau > 10 MB)"),
        );
    }
    for shot in &shots {
        upload(ctx, spec.id, shot, log).await;
    }
    steps.evidence = StepState::Ok;
    if spec.kind == JobKind::Knowledge {
        // Hasil job knowledge = usulan di server (lewat MCP); tidak ada commit/PR.
        return;
    }
    let message = format!("feat({}): {}", spec.ticket, spec.title);
    match worktree::commit_work(wt, &spec.rules, &message) {
        Ok(true) => log.push(
            LogStream::Info,
            format!("Perubahan di-commit di branch {}", spec.branch),
        ),
        Ok(false) => log.push(LogStream::Info, "Tidak ada perubahan kode untuk di-commit"),
        Err(e) => log.push(
            LogStream::Warn,
            format!("Commit gagal: {}", e.user_message()),
        ),
    }
    if spec.git.code_output == CodeOutput::Pr {
        open_pull_request(ctx, spec, wt, &message, log).await;
    }
}

/// Hasil kode "PR otomatis": push branch tiket lalu minta server membuka PR. Gagal tidak
/// menggagalkan job (gate `pr_open` yang menahan stage); pesannya masuk log job.
async fn open_pull_request(ctx: &Ctx, spec: &JobSpec, wt: &Path, title: &str, log: &mut Logger) {
    if worktree::ahead_of(wt, &spec.base_branch) == 0 {
        log.push(
            LogStream::Info,
            "Tidak ada commit baru di branch tiket; PR tidak dibuka",
        );
        return;
    }
    if let Err(e) = worktree::push(wt, &spec.branch) {
        log.push(
            LogStream::Warn,
            format!(
                "Push branch {} gagal: {}. Push manual lalu buka PR.",
                spec.branch,
                e.user_message()
            ),
        );
        return;
    }
    log.push(LogStream::Ok, format!("Branch {} di-push", spec.branch));
    let repo = Repos::load(&ctx.paths)
        .ok()
        .and_then(|r| {
            r.find_for(&spec.project, &spec.repos)
                .map(|e| e.name.clone())
        })
        .unwrap_or_default();
    // Nama repo menurut server (spec.repos) yang cocok dengan repo lokal.
    let repo = spec
        .repos
        .iter()
        .find(|n| config::repo_name_matches(n, &repo))
        .cloned()
        .unwrap_or(repo);
    let req = PullRequestOpen {
        repo,
        branch: spec.branch.clone(),
        base: spec.base_branch.clone(),
        title: title.to_string(),
        body: format!("Tiket {}: {}\n\n{}", spec.ticket, spec.title, spec.url),
    };
    match ctx
        .api
        .post::<_, PullRequestInfo>(&ctx.job_path(spec.id, "pull-request"), &req)
        .await
    {
        Ok(pr) => log.push(
            LogStream::Ok,
            format!(
                "PR #{} {}: {}",
                pr.number,
                if pr.created { "dibuka" } else { "sudah ada" },
                pr.url
            ),
        ),
        Err(e) => log.push(
            LogStream::Warn,
            format!("PR gagal dibuka: {}", Error::from(e).user_message()),
        ),
    }
}

async fn upload(
    ctx: &Ctx,
    id: Uuid,
    up: &agentgate_core::runner::protocol::EvidenceUpload,
    log: &mut Logger,
) {
    match ctx.api.post_unit(&ctx.job_path(id, "evidence"), up).await {
        Ok(()) => log.push(
            LogStream::Ok,
            format!("Evidence {} terkirim: {}", up.kind.as_str(), up.name),
        ),
        Err(e) => log.push(
            LogStream::Fail,
            format!(
                "Evidence {} ditolak: {}",
                up.name,
                Error::from(e).user_message()
            ),
        ),
    }
}

/// File yang diubah job di repo pertamanya (satu repo per job runner).
fn changed_files(spec: &JobSpec, wt: &Path) -> Vec<ChangedFile> {
    let Some(repo) = spec.repos.first() else {
        return Vec::new();
    };
    worktree::changed_files(wt, &spec.base_branch)
        .into_iter()
        .map(|path| ChangedFile {
            repo: repo.clone(),
            path,
        })
        .collect()
}

async fn finish(ctx: &Ctx, id: Uuid, report: &FinishReport) -> Result<()> {
    ctx.api
        .post::<_, serde_json::Value>(&ctx.job_path(id, "finish"), report)
        .await?;
    Ok(())
}

fn cleanup(ctx: &Ctx, id: Uuid) {
    let _ = config::remove(&ctx.paths.job_state(id));
}

/// Jalankan satu job sampai selesai (atau dibatalkan).
///
/// # Errors
///
/// `start`/`finish` gagal di server.
pub async fn run(ctx: &Ctx, id: Uuid, mut cancel: watch::Receiver<bool>) -> Result<()> {
    let spec = start(ctx, id).await?;
    let mut log = Logger::new(ctx, id, &spec.ticket);
    let mut steps = Steps::new();
    let wt = match prepare(ctx, &spec) {
        Ok(wt) => wt,
        Err(e) => {
            let msg = e.user_message();
            log.push(LogStream::Fail, msg.clone());
            log.flush().await;
            return finish(
                ctx,
                id,
                &FinishReport {
                    outcome: Some(Outcome::Failed),
                    summary: msg,
                    ..FinishReport::default()
                },
            )
            .await;
        }
    };
    log.push(
        LogStream::Info,
        format!("Worktree {} · branch {}", wt.display(), spec.branch),
    );
    steps.prepare = StepState::Ok;
    if spec.mode == JobMode::Interactive {
        return open_terminal(ctx, &spec, &mut log, &mut steps).await;
    }
    steps.agent = StepState::Now;
    steps.post(ctx, id).await;
    let agent = match run_agent(ctx, &spec, &wt, &mut log, &mut steps, &mut cancel).await {
        Ok(a) => a,
        Err(e) => {
            let msg = e.user_message();
            log.push(LogStream::Fail, msg.clone());
            Agent::Done {
                ok: false,
                summary: msg,
                tokens: None,
                usage: None,
                cost_usd: None,
            }
        }
    };
    let report = match agent {
        Agent::Cancelled => {
            log.push(LogStream::Warn, "Job dibatalkan; proses client dihentikan.");
            log.flush().await;
            cleanup(ctx, id);
            // Job sudah ditutup server; finish akan ditolak.
            return Ok(());
        }
        Agent::Quota { text } => {
            log.push(LogStream::Warn, "Kuota AI habis");
            FinishReport {
                outcome: Some(Outcome::Quota),
                quota_reset_at: clients::quota_reset_at(&text),
                summary: text,
                ..FinishReport::default()
            }
        }
        Agent::Done {
            ok,
            summary,
            tokens,
            usage,
            cost_usd,
        } => {
            steps.agent = if ok { StepState::Ok } else { StepState::Fail };
            if ok {
                wrap_up(ctx, &spec, &wt, &mut log, &mut steps).await;
            }
            steps.post(ctx, id).await;
            FinishReport {
                outcome: Some(if ok {
                    Outcome::Succeeded
                } else {
                    Outcome::Failed
                }),
                summary,
                tokens,
                cost_usd,
                quota_reset_at: None,
                usage,
                changed_files: changed_files(&spec, &wt),
            }
        }
    };
    log.flush().await;
    let res = finish(ctx, id, &report).await;
    cleanup(ctx, id);
    res
}

/// Job interaktif: launcher → terminal. Gagal → server memberi tahu executor.
async fn open_terminal(
    ctx: &Ctx,
    spec: &JobSpec,
    log: &mut Logger,
    steps: &mut Steps,
) -> Result<()> {
    steps.agent = StepState::Now;
    steps.post(ctx, spec.id).await;
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("agentgate"));
    let opened = terminal::write_launcher(&ctx.paths.state, &exe, spec.id)
        .map(|l| terminal::open(&l))
        .unwrap_or(false);
    if opened {
        log.push(LogStream::Info, "Terminal sesi interaktif dibuka");
    } else {
        log.push(
            LogStream::Warn,
            format!(
                "Terminal tidak bisa dibuka. Jalankan `agentgate job attach {}` atau {} di client \
                 Anda.",
                spec.id, spec.command
            ),
        );
        let _ = ctx
            .api
            .post_empty(&ctx.job_path(spec.id, "terminal-failed"))
            .await;
    }
    log.flush().await;
    Ok(())
}

/// `agentgate job attach <id>`: sesi client interaktif di worktree job, lalu (bila disetujui)
/// tes, evidence, commit, dan selesai.
///
/// # Errors
///
/// Runner belum terdaftar, server/git gagal, client tidak bisa dijalankan.
pub async fn attach(ctx: &Ctx, id: Uuid, out: &mut impl Write) -> Result<()> {
    let spec = start(ctx, id).await?;
    let wt = prepare(ctx, &spec)?;
    let _ = writeln!(
        out,
        "Job {} · {} · stage {} · persona {}\nWorktree: {}\n",
        spec.ticket,
        spec.title,
        spec.stage_name,
        spec.persona_name,
        wt.display()
    );
    let _ = out.flush();
    let client = ctx.client_for(&spec);
    let program = client.program();
    let mut cmd = Command::new(&program);
    cmd.args(client.interactive(&spec, &wt)).current_dir(&wt);
    job_env(&mut cmd, ctx, &spec, client);
    // Trust/MCP workspace yang sama dengan job headless (bukan flag: sesi interaktif).
    for (key, value) in client.auto(&spec, &wt).env {
        cmd.env(key, value);
    }
    let status = cmd.status().await.map_err(|source| Error::Spawn {
        program: program.clone(),
        source,
    })?;
    if !ui::confirm("Jalankan tes, kirim evidence, commit, dan tandai job selesai?") {
        let _ = writeln!(
            out,
            "Job tetap berjalan. Jalankan `agentgate job attach {id}` lagi untuk melanjutkan."
        );
        return Ok(());
    }
    let mut log = Logger::new(ctx, id, &spec.ticket);
    let mut steps = Steps::new();
    steps.prepare = StepState::Ok;
    steps.agent = if status.success() {
        StepState::Ok
    } else {
        StepState::Fail
    };
    wrap_up(ctx, &spec, &wt, &mut log, &mut steps).await;
    steps.post(ctx, id).await;
    log.flush().await;
    finish(
        ctx,
        id,
        &FinishReport {
            outcome: Some(if status.success() {
                Outcome::Succeeded
            } else {
                Outcome::Failed
            }),
            summary: "Sesi interaktif selesai".into(),
            changed_files: changed_files(&spec, &wt),
            ..FinishReport::default()
        },
    )
    .await?;
    cleanup(ctx, id);
    Ok(())
}
