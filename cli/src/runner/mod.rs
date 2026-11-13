//! `agentgate runner start`: daftarkan runner perangkat ini, dengarkan event server, dan
//! jalankan job yang siap.
//!
//! Kontrol lewat WebSocket `/api/runner/ws` (bearer) dengan heartbeat; bila WebSocket tidak
//! tersedia (proxy, firewall), runner beralih ke polling `GET /api/runner/events` yang memberi
//! event yang sama. `job_ready` dikirim ulang oleh polling selama job belum dimulai, jadi
//! runner menyimpan job yang sudah ditangani.

pub mod clients;
pub mod evidence;
pub mod job;
pub mod slots;
pub mod terminal;
pub mod worktree;

use std::{
    collections::{HashMap, HashSet},
    io::Write,
};

use agentgate_core::runner::{
    protocol::{ClientMsg, JobSummary, RegisterRunner, RunnerInfo, ServerMsg},
    signing::public_key_b64,
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::{net::TcpStream, sync::watch, task::JoinSet};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest, http::HeaderValue},
};
use uuid::Uuid;

use self::clients::ClientKind;
use crate::{
    api::{Api, ApiError},
    auth,
    config::{self, Paths, Repos, RunnerIdentity},
    error::{Error, Result},
    ui,
};

/// Interval heartbeat WebSocket (server: online = heartbeat < 60 dtk).
const HEARTBEAT: u64 = 25;
/// Interval polling event tanpa WebSocket.
const POLL: u64 = 5;

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Daftarkan (atau perbarui) runner perangkat ini; id disimpan di `runner.json`.
///
/// `client` = client default runner pilihan `--client`; `None` → server memakai "Client
/// default" di pengaturan runner web.
///
/// # Errors
///
/// Server gagal, IO.
pub async fn register(
    paths: &Paths,
    api: &Api,
    client: Option<ClientKind>,
) -> Result<(job::Ctx, RunnerInfo)> {
    let mut identity = RunnerIdentity::load_or_create(paths)?;
    let secret = identity.secret_bytes(paths)?;
    let known = identity.runner_id.filter(|_| identity.server == api.base());
    let request = |runner_id| RegisterRunner {
        runner_id,
        name: identity.name.clone(),
        os: std::env::consts::OS.into(),
        client: client.map(ClientKind::key).unwrap_or_default().into(),
        version: agentgate_core::VERSION.into(),
        public_key: public_key_b64(&secret),
    };
    let info: RunnerInfo = match api.post("/api/runner/register", &request(known)).await {
        Ok(info) => info,
        // Runner lama dicabut/dihapus: daftar sebagai runner baru.
        Err(ApiError::NotFound(_) | ApiError::Rejected(_)) if known.is_some() => {
            api.post("/api/runner/register", &request(None)).await?
        }
        Err(e) => return Err(e.into()),
    };
    identity.runner_id = Some(info.id);
    identity.server = api.base().to_string();
    identity.save(paths)?;
    let client = ClientKind::parse(&info.client)
        .or(client)
        .unwrap_or(ClientKind::Claude);
    Ok((
        job::Ctx {
            paths: paths.clone(),
            api: api.with_runner(info.id),
            runner_id: info.id,
            secret,
            client,
        },
        info,
    ))
}

/// Konteks runner yang sudah terdaftar (untuk `job attach`), tanpa mendaftar ulang. Client job
/// diambil dari `JobSpec.client`; `client` hanya cadangan.
///
/// # Errors
///
/// Runner belum terdaftar di server ini.
pub fn registered(paths: &Paths, api: &Api, client: Option<ClientKind>) -> Result<job::Ctx> {
    let identity = RunnerIdentity::load(paths)?.ok_or(Error::NoRunner)?;
    let runner_id = identity
        .runner_id
        .filter(|_| identity.server == api.base())
        .ok_or(Error::NoRunner)?;
    Ok(job::Ctx {
        paths: paths.clone(),
        api: api.with_runner(runner_id),
        runner_id,
        secret: identity.secret_bytes(paths)?,
        client: client.unwrap_or(ClientKind::Claude),
    })
}

/// Sumber event: WebSocket atau polling.
enum Source {
    Ws(SplitStream<Ws>),
    Poll { first: bool },
}

enum Event {
    Msgs(Vec<ServerMsg>),
    /// WebSocket putus: beralih ke polling.
    Disconnected,
}

async fn connect_ws(api: &Api, runner: Uuid) -> Option<Ws> {
    let mut req = api
        .ws_url(&format!("/api/runner/ws?runner={runner}"))
        .into_client_request()
        .ok()?;
    let bearer = HeaderValue::from_str(&format!("Bearer {}", api.token())).ok()?;
    req.headers_mut().insert("authorization", bearer);
    let connect = tokio_tungstenite::connect_async(req);
    let (mut ws, _) = tokio::time::timeout(config::seconds(10), connect)
        .await
        .ok()?
        .ok()?;
    let hello = ClientMsg::Hello {
        runner_id: runner,
        version: agentgate_core::VERSION.into(),
    };
    let text = serde_json::to_string(&hello).ok()?;
    ws.send(Message::text(text)).await.ok()?;
    Some(ws)
}

impl Source {
    async fn next(&mut self, api: &Api, runner: Uuid) -> Result<Event> {
        match self {
            Self::Ws(stream) => loop {
                match stream.next().await {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(msg) = serde_json::from_str::<ServerMsg>(&text) {
                            return Ok(Event::Msgs(vec![msg]));
                        }
                    }
                    Some(Ok(Message::Close(_)) | Err(_)) | None => return Ok(Event::Disconnected),
                    Some(Ok(_)) => {}
                }
            },
            Self::Poll { first } => {
                if !*first {
                    tokio::time::sleep(config::seconds(POLL)).await;
                }
                *first = false;
                match api
                    .get::<Vec<ServerMsg>>(&format!("/api/runner/events?runner={runner}"))
                    .await
                {
                    Ok(msgs) => Ok(Event::Msgs(msgs)),
                    Err(ApiError::Unauthorized) => Err(ApiError::Unauthorized.into()),
                    Err(ApiError::NotFound(c)) => Err(ApiError::NotFound(c).into()),
                    Err(e) => {
                        eprintln!("Event tidak bisa diambil: {e}. Mencoba lagi…");
                        Ok(Event::Msgs(Vec::new()))
                    }
                }
            }
        }
    }
}

fn print_plan(
    out: &mut impl Write,
    paths: &Paths,
    api: &Api,
    email: &str,
    repos: &Repos,
    client: Option<ClientKind>,
) {
    let client = client.map_or_else(
        || "client AI job (bawaan: Client default di pengaturan runner web)".to_string(),
        |c| format!("{} ({})", c.name(), c.program()),
    );
    let _ = writeln!(
        out,
        "Runner Agent Gate di perangkat ini akan:\n\
         \x20 • terhubung ke {} sebagai {email} dan menerima job yang Anda ambil (atau auto-ambil);\n\
         \x20 • menjalankan {client} tanpa diawasi di worktree terpisah {}/wt/<repo>/<branch>;\n\
         \x20 • menulis aturan terbaru server ke worktree, menjalankan [test] command, lalu mengirim\n\
         \x20   log dan evidence bertanda tangan ke server;\n\
         \x20 • meng-commit hasil kerja di branch tiket (lokal, tanpa push);\n\
         \x20 • membuka terminal untuk job interaktif.\n\
         Aksi berbahaya dibatasi izin persona lewat hook `agentgate hook pre-tool`.",
        api.base(),
        paths.state.display()
    );
    if repos.repos.is_empty() {
        let _ = writeln!(
            out,
            "\nPeringatan: belum ada repo yang di-init. Jalankan `agentgate init` di repo project."
        );
    } else {
        let _ = writeln!(out, "\nRepo lokal:");
        for r in &repos.repos {
            let _ = writeln!(out, "  {} ({}) {}", r.name, r.project, r.path.display());
        }
    }
    let _ = out.flush();
}

/// `agentgate runner start [--yes] [--once] [--client C]`.
///
/// # Errors
///
/// Belum login, pendaftaran gagal, akses dicabut.
pub async fn start(
    paths: &Paths,
    yes: bool,
    once: bool,
    client: Option<ClientKind>,
    out: &mut impl Write,
) -> Result<()> {
    let (api, creds) = auth::session(paths).await?;
    let repos = Repos::load(paths)?;
    print_plan(out, paths, &api, &creds.email, &repos, client);
    if !yes && !ui::confirm("\nNyalakan runner?") {
        return Err(Error::Aborted);
    }
    let (ctx, info) = register(paths, &api, client).await?;
    let mut source = match connect_ws(&api, info.id).await {
        Some(ws) => {
            let (tx, rx) = ws.split();
            let _ = writeln!(out, "\nRunner {} aktif (WebSocket).", info.name);
            (Source::Ws(rx), Some(tx))
        }
        None => {
            let _ = writeln!(
                out,
                "\nRunner {} aktif (WebSocket tidak tersedia, memakai polling).",
                info.name
            );
            (Source::Poll { first: true }, None)
        }
    };
    let _ = writeln!(
        out,
        "Workspace {} · {} · client default {}. Menunggu job… (Ctrl+C untuk berhenti)",
        info.workspace,
        info.user_email,
        ctx.client.name()
    );
    let _ = out.flush();
    let limit = match info.max_parallel {
        Some(n) => format!("maksimal {n} job paralel"),
        None => "tanpa batas job paralel".into(),
    };
    let _ = writeln!(out, "Batas: {limit}.");
    let _ = out.flush();
    run_loop(&ctx, &api, &mut source, once, info.max_parallel).await
}

/// Kode server: runner sudah menjalankan `max_parallel` job.
pub const RUNNER_BUSY: &str = "job-runner-busy";

fn spawn_job(
    tasks: &mut JoinSet<(JobSummary, Result<()>)>,
    cancels: &mut HashMap<Uuid, watch::Sender<bool>>,
    ctx: &job::Ctx,
    job: JobSummary,
) {
    println!(
        "Job {} · {} · {} dimulai",
        job.ticket, job.stage_name, job.title
    );
    let (tx, rx) = watch::channel(false);
    cancels.insert(job.id, tx);
    let ctx = ctx.clone();
    tasks.spawn(async move {
        let res = job::run(&ctx, job.id, rx).await;
        (job, res)
    });
}

type Sink = SplitSink<Ws, Message>;

async fn run_loop(
    ctx: &job::Ctx,
    api: &Api,
    (source, sink): &mut (Source, Option<Sink>),
    once: bool,
    max_parallel: Option<u32>,
) -> Result<()> {
    let mut tasks: JoinSet<(JobSummary, Result<()>)> = JoinSet::new();
    let mut slots = slots::Slots::new(max_parallel);
    let mut cancels: HashMap<Uuid, watch::Sender<bool>> = HashMap::new();
    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut heartbeat = tokio::time::interval(config::seconds(HEARTBEAT));
    let mut started = 0usize;
    let mut stop_error: Option<Error> = None;
    loop {
        tokio::select! {
            event = source.next(api, ctx.runner_id) => match event {
                Ok(Event::Msgs(msgs)) => {
                    for msg in msgs {
                        match msg {
                            ServerMsg::JobReady { job } => {
                                if (once && started > 0) || !seen.insert(job.id) {
                                    continue;
                                }
                                started += 1;
                                let label = format!("{} · {}", job.ticket, job.stage_name);
                                match slots.offer(job) {
                                    Some(job) => spawn_job(&mut tasks, &mut cancels, ctx, job),
                                    None => println!(
                                        "Job {label} menunggu slot ({} berjalan).",
                                        slots.running()
                                    ),
                                }
                            }
                            ServerMsg::JobCancelled { job_id } => {
                                if let Some(tx) = cancels.get(&job_id) {
                                    println!("Job {job_id} dibatalkan dari web.");
                                    let _ = tx.send(true);
                                } else if slots.drop_pending(job_id) {
                                    println!("Job {job_id} dibatalkan sebelum dimulai.");
                                }
                            }
                            ServerMsg::PermissionDecided { job_id, status, .. } => {
                                println!("Izin job {job_id} dijawab: {}", status.as_str());
                            }
                            ServerMsg::Welcome { .. } => {}
                            ServerMsg::Error { message } => {
                                stop_error = Some(Error::ServerClosed(message));
                                break;
                            }
                        }
                    }
                    if stop_error.is_some() {
                        break;
                    }
                }
                Ok(Event::Disconnected) => {
                    eprintln!("WebSocket terputus; beralih ke polling.");
                    *source = Source::Poll { first: true };
                    *sink = None;
                }
                Err(e) => {
                    stop_error = Some(e);
                    break;
                }
            },
            Some(done) = tasks.join_next(), if !tasks.is_empty() => {
                if let Ok((job, res)) = done {
                    let id = job.id;
                    cancels.remove(&id);
                    match res {
                        Ok(()) => println!("Job {id} selesai."),
                        Err(Error::Api(ref e)) if e.code() == Some(RUNNER_BUSY) => {
                            println!("Server: runner penuh; job {id} kembali ke antrean lokal.");
                            slots.requeue(job);
                            continue;
                        }
                        Err(e) => eprintln!("Job {id} gagal: {}", e.user_message()),
                    }
                    for next in slots.finish(id) {
                        spawn_job(&mut tasks, &mut cancels, ctx, next);
                    }
                }
                if once && tasks.is_empty() && slots.pending() == 0 {
                    break;
                }
            },
            _ = heartbeat.tick() => {
                // Job yang ditolak server karena runner penuh dicoba lagi.
                for next in slots.finish(Uuid::nil()) {
                    spawn_job(&mut tasks, &mut cancels, ctx, next);
                }
                if let Some(tx) = sink.as_mut() {
                    let msg = ClientMsg::Heartbeat { running: cancels.keys().copied().collect() };
                    if let Ok(text) = serde_json::to_string(&msg)
                        && tx.send(Message::text(text)).await.is_err()
                    {
                        *sink = None;
                    }
                }
            },
            _ = tokio::signal::ctrl_c() => {
                println!("Menghentikan runner…");
                break;
            }
        }
    }
    for tx in cancels.values() {
        let _ = tx.send(true);
    }
    while tasks.join_next().await.is_some() {}
    match stop_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
