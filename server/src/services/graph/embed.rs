//! Embedding lokal per simbol untuk pencarian semantik code graph (R2).
//!
//! - Mesin: fastembed (ONNX Runtime di CPU, feature Cargo `semantic`) atau `fake` (vektor hash
//!   deterministik; tes dan build tanpa feature). Model diunduh ke `graph.embedding.cache_dir`
//!   saat proses mulai ([`warm`]) atau oleh job index, tidak pernah di jalur request: selama
//!   model belum siap, pencarian jatuh ke leksikal.
//! - Teks simbol = jenis + nama lengkap + signature + docstring + path + potongan kode (maks.
//!   30 baris). Hash teks disimpan di `graph_symbols.embed_hash`; `graph_embeddings` dikunci
//!   (repo, model, hash), jadi index ulang hanya meng-embed simbol yang teksnya berubah.
//! - Vektor di pgvector; jarak kosinus (`<=>`). Skala project (puluhan ribu simbol) cukup
//!   dengan scan berurutan; index HNSW menyusul bila perlu (dimensi kolom tidak dikunci).
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock, Mutex},
    time::Instant,
};

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement, Value,
};
use uuid::Uuid;

use crate::{common::settings::EmbeddingSettings, models::workspaces::WorkspaceId};

/// Dimensi vektor `fake`.
pub const FAKE_DIM: usize = 64;
/// Baris kode simbol yang ikut di-embed.
pub const CODE_LINES: i32 = 30;
/// Batas karakter teks simbol.
pub const TEXT_MAX: usize = 1600;

/// Model yang didukung (kunci config → model fastembed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSpec {
    pub key: &'static str,
    pub dim: usize,
    /// Awalan teks query/dokumen (keluarga E5 dilatih dengan awalan ini).
    pub query_prefix: &'static str,
    pub passage_prefix: &'static str,
}

/// Daftar model yang bisa dipilih di `graph.embedding.model`.
pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        key: "multilingual-e5-small",
        dim: 384,
        query_prefix: "query: ",
        passage_prefix: "passage: ",
    },
    ModelSpec {
        key: "multilingual-minilm-q",
        dim: 384,
        query_prefix: "",
        passage_prefix: "",
    },
    ModelSpec {
        key: "bge-small-en-q",
        dim: 384,
        query_prefix: "",
        passage_prefix: "",
    },
];

/// Spesifikasi model config.
#[must_use]
pub fn spec(model: &str) -> Option<ModelSpec> {
    MODELS.iter().copied().find(|m| m.key == model.trim())
}

enum Engine {
    Fake,
    #[cfg(feature = "semantic")]
    Fast(Mutex<fastembed::TextEmbedding>),
}

/// Mesin embedding yang sudah dimuat.
pub struct Embedder {
    /// Kunci model yang disimpan di `graph_embeddings.model` (`fake:<dim>` untuk `fake`).
    pub model: String,
    spec: ModelSpec,
    engine: Engine,
}

impl std::fmt::Debug for Embedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Embedder")
            .field("model", &self.model)
            .finish()
    }
}

impl Embedder {
    fn fake() -> Self {
        Self {
            model: format!("fake:{FAKE_DIM}"),
            spec: ModelSpec {
                key: "fake",
                dim: FAKE_DIM,
                query_prefix: "",
                passage_prefix: "",
            },
            engine: Engine::Fake,
        }
    }

    fn run(&self, texts: Vec<String>, batch: usize) -> Result<Vec<Vec<f32>>, String> {
        let _ = batch;
        match &self.engine {
            Engine::Fake => Ok(texts.iter().map(|t| fake_vector(t)).collect()),
            #[cfg(feature = "semantic")]
            Engine::Fast(m) => m
                .lock()
                .map_err(|_| "model lock poisoned".to_string())?
                .embed(texts, Some(batch.max(1)))
                .map_err(|e| e.to_string()),
        }
    }

    /// Embed teks dokumen (simbol).
    ///
    /// # Errors
    ///
    /// Galat mesin.
    pub fn passages(&self, texts: &[String], batch: usize) -> Result<Vec<Vec<f32>>, String> {
        self.run(
            texts
                .iter()
                .map(|t| format!("{}{t}", self.spec.passage_prefix))
                .collect(),
            batch,
        )
    }

    /// Embed query pencarian.
    ///
    /// # Errors
    ///
    /// Galat mesin.
    pub fn query(&self, q: &str) -> Result<Vec<f32>, String> {
        self.run(vec![format!("{}{q}", self.spec.query_prefix)], 1)?
            .pop()
            .ok_or_else(|| "empty".into())
    }
}

/// Vektor `fake`: kata (pecahan camelCase/snake_case) di-hash ke [`FAKE_DIM`] ember lalu
/// dinormalisasi. Deterministik; teks dengan kata yang sama → kosinus tinggi.
#[must_use]
pub fn fake_vector(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; FAKE_DIM];
    for w in agentgate_graph::protocol::words(&[text]).split(' ') {
        if w.len() < 2 {
            continue;
        }
        let d = ring::digest::digest(&ring::digest::SHA256, w.as_bytes());
        let b = d.as_ref();
        v[usize::from(b[0]) % FAKE_DIM] += 1.0;
        v[usize::from(b[1]) % FAKE_DIM] += 0.5;
    }
    normalize(&mut v);
    v
}

fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in v {
            *x /= n;
        }
    }
}

#[derive(Clone)]
enum Slot {
    Loading,
    Ready(Arc<Embedder>),
    Failed(String),
}

static SLOTS: LazyLock<Mutex<HashMap<String, Slot>>> = LazyLock::new(Mutex::default);

fn slot_key(s: &EmbeddingSettings) -> String {
    format!(
        "{}|{}|{}",
        s.provider.trim(),
        s.model.trim(),
        s.dir().display()
    )
}

fn slots() -> std::sync::MutexGuard<'static, HashMap<String, Slot>> {
    SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Muat mesin (blocking: bisa mengunduh model). Dipakai job dan [`warm`].
///
/// # Errors
///
/// Embedding dimatikan, provider/model tidak dikenal, feature `semantic` tidak ada, atau
/// model gagal diunduh/dimuat.
pub fn load_blocking(s: &EmbeddingSettings) -> Result<Arc<Embedder>, String> {
    if !s.enabled {
        return Err("embedding dimatikan di konfigurasi".into());
    }
    let key = slot_key(s);
    if let Some(Slot::Ready(e)) = slots().get(&key) {
        return Ok(e.clone());
    }
    let result = build(s).map(Arc::new);
    slots().insert(
        key,
        match &result {
            Ok(e) => Slot::Ready(e.clone()),
            Err(m) => Slot::Failed(m.clone()),
        },
    );
    result
}

fn build(s: &EmbeddingSettings) -> Result<Embedder, String> {
    match s.provider.trim() {
        "fake" => Ok(Embedder::fake()),
        "fastembed" => {
            let spec = spec(&s.model)
                .ok_or_else(|| format!("model embedding tidak dikenal: {}", s.model))?;
            fastembed_engine(s, spec)
        }
        other => Err(format!("provider embedding tidak dikenal: {other}")),
    }
}

#[cfg(feature = "semantic")]
fn fastembed_engine(s: &EmbeddingSettings, spec: ModelSpec) -> Result<Embedder, String> {
    use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
    let model = match spec.key {
        "multilingual-e5-small" => EmbeddingModel::MultilingualE5Small,
        "multilingual-minilm-q" => EmbeddingModel::ParaphraseMLMiniLML12V2Q,
        _ => EmbeddingModel::BGESmallENV15Q,
    };
    std::fs::create_dir_all(s.dir()).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let engine = TextEmbedding::try_new(
        TextInitOptions::new(model)
            .with_cache_dir(s.dir())
            .with_show_download_progress(false),
    )
    .map_err(|e| format!("model {} gagal dimuat: {e}", spec.key))?;
    tracing::info!(
        model = spec.key,
        ms = started.elapsed().as_millis() as u64,
        "embedding model loaded"
    );
    Ok(Embedder {
        model: spec.key.to_string(),
        spec,
        engine: Engine::Fast(Mutex::new(engine)),
    })
}

#[cfg(not(feature = "semantic"))]
fn fastembed_engine(_: &EmbeddingSettings, _: ModelSpec) -> Result<Embedder, String> {
    Err("server dibangun tanpa feature `semantic` (fastembed)".into())
}

/// Mesin yang sudah siap tanpa menunggu (jalur request). Belum dimuat → mulai muat di
/// latar belakang dan `None` (pencarian memakai leksikal).
#[must_use]
pub fn ready(s: &EmbeddingSettings) -> Option<Arc<Embedder>> {
    if !s.enabled {
        return None;
    }
    let key = slot_key(s);
    match slots().get(&key).cloned() {
        Some(Slot::Ready(e)) => return Some(e),
        Some(Slot::Loading | Slot::Failed(_)) => return None,
        None => {}
    }
    if s.provider.trim() == "fake" {
        return load_blocking(s).ok();
    }
    warm(s.clone());
    None
}

/// Status mesin untuk UI/ops: `None` = belum dimuat.
#[must_use]
pub fn state(s: &EmbeddingSettings) -> Option<Result<String, String>> {
    match slots().get(&slot_key(s))? {
        Slot::Ready(e) => Some(Ok(e.model.clone())),
        Slot::Loading => None,
        Slot::Failed(m) => Some(Err(m.clone())),
    }
}

/// Muat model di latar belakang (start proses). Idempoten.
pub fn warm(s: EmbeddingSettings) {
    if !s.enabled {
        return;
    }
    {
        let mut map = slots();
        let key = slot_key(&s);
        if map.contains_key(&key) {
            return;
        }
        map.insert(key, Slot::Loading);
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    handle.spawn_blocking(move || {
        // Hapus tanda Loading supaya load_blocking benar-benar memuat.
        slots().remove(&slot_key(&s));
        if let Err(e) = load_blocking(&s) {
            tracing::warn!(error = %e, "embedding model unavailable; search stays lexical");
        }
    });
}

static CONFIG: LazyLock<Mutex<Option<EmbeddingSettings>>> = LazyLock::new(Mutex::default);

/// Simpan config embedding proses (saat start) untuk pemanggil tanpa `AppContext` (MCP).
pub fn configure(s: EmbeddingSettings) {
    *CONFIG
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(s);
}

/// Config embedding proses ([`configure`]); default bila belum diset.
#[must_use]
pub fn configured() -> EmbeddingSettings {
    CONFIG
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .unwrap_or_default()
}

/// Literal vektor pgvector (`[0.1,0.2,...]`).
#[must_use]
pub fn literal(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 10 + 2);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{x:.6}"));
    }
    s.push(']');
    s
}

#[derive(Debug, FromQueryResult)]
struct SymText {
    id: Uuid,
    kind: String,
    qualified: String,
    signature: String,
    doc: String,
    path: String,
    code: String,
    embed_hash: String,
}

/// Teks simbol yang di-embed.
#[must_use]
pub fn symbol_text(
    kind: &str,
    qualified: &str,
    signature: &str,
    doc: &str,
    path: &str,
    code: &str,
) -> String {
    let mut t = format!("{kind} {qualified}\n");
    for part in [signature, doc, path, code] {
        if !part.trim().is_empty() {
            t.push_str(part.trim());
            t.push('\n');
        }
    }
    t.chars().take(TEXT_MAX).collect()
}

fn hash(model: &str, text: &str) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, format!("{model}\n{text}").as_bytes());
    hex::encode(&d.as_ref()[..16])
}

/// Hasil embed satu repo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmbedReport {
    /// Simbol yang punya embedding.
    pub symbols: usize,
    /// Teks baru yang di-embed pada run ini.
    pub embedded: usize,
    pub ms: u64,
    pub error: String,
}

fn stmt(sql: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}

/// Embed simbol repo yang teksnya berubah, lalu catat statistik di `graph_repos`.
///
/// # Errors
///
/// Error database (galat mesin dicatat di `graph_repos.embed_error`).
pub async fn index_repo(
    db: &DatabaseConnection,
    s: &EmbeddingSettings,
    wid: WorkspaceId,
    repo: Uuid,
) -> Result<EmbedReport, DbErr> {
    if !s.enabled {
        return Ok(EmbedReport::default());
    }
    let started = Instant::now();
    let settings = s.clone();
    let embedder = match tokio::task::spawn_blocking(move || load_blocking(&settings)).await {
        Ok(Ok(e)) => e,
        Ok(Err(e)) => {
            return record(
                db,
                repo,
                EmbedReport {
                    error: e,
                    ..Default::default()
                },
            )
            .await;
        }
        Err(e) => {
            let report = EmbedReport {
                error: e.to_string(),
                ..Default::default()
            };
            return record(db, repo, report).await;
        }
    };
    let rows = SymText::find_by_statement(stmt(
        "SELECT s.id, s.kind, s.qualified, s.signature, s.doc, f.path, s.embed_hash,
                left(array_to_string((string_to_array(f.content, E'\\n'))
                     [s.start_line:least(s.end_line, s.start_line + $3)], E'\\n'), 1200) AS code
           FROM graph_symbols s JOIN graph_files f ON f.id = s.file_id
          WHERE s.workspace_id = $1 AND s.repo_id = $2 AND s.kind <> 'file' AND NOT s.test
          ORDER BY s.callers DESC, s.id
          LIMIT $4",
        vec![
            wid.get().into(),
            repo.into(),
            (CODE_LINES - 1).into(),
            i64::try_from(s.max_symbols).unwrap_or(50_000).into(),
        ],
    ))
    .all(db)
    .await?;
    let mut ids = Vec::new();
    let mut hashes = Vec::new();
    let mut texts: HashMap<String, String> = HashMap::new();
    for r in &rows {
        let text = symbol_text(
            &r.kind,
            &r.qualified,
            &r.signature,
            &r.doc,
            &r.path,
            &r.code,
        );
        let h = hash(&embedder.model, &text);
        if r.embed_hash != h {
            ids.push(r.id);
            hashes.push(h.clone());
        }
        texts.entry(h).or_insert(text);
    }
    if !ids.is_empty() {
        db.execute_raw(stmt(
            "UPDATE graph_symbols s SET embed_hash = v.h
               FROM unnest($1::uuid[], $2::text[]) AS v(id, h) WHERE s.id = v.id",
            vec![ids.into(), hashes.into()],
        ))
        .await?;
    }
    let have: HashSet<String> = db
        .query_all_raw(stmt(
            "SELECT hash FROM graph_embeddings WHERE repo_id = $1 AND model = $2",
            vec![repo.into(), embedder.model.clone().into()],
        ))
        .await?
        .iter()
        .filter_map(|r| r.try_get::<String>("", "hash").ok())
        .collect();
    let missing: Vec<(String, String)> = texts
        .into_iter()
        .filter(|(h, _)| !have.contains(h))
        .collect();
    let mut report = EmbedReport::default();
    for chunk in missing.chunks(s.batch_size.max(1) * 4) {
        let (hs, ts): (Vec<String>, Vec<String>) = chunk.iter().cloned().unzip();
        let e = embedder.clone();
        let batch = s.batch_size;
        let vectors = match tokio::task::spawn_blocking(move || e.passages(&ts, batch)).await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                report.error = e;
                break;
            }
            Err(e) => {
                report.error = e.to_string();
                break;
            }
        };
        let lits: Vec<String> = vectors.iter().map(|v| literal(v)).collect();
        report.embedded += lits.len();
        db.execute_raw(stmt(
            "INSERT INTO graph_embeddings (workspace_id, repo_id, model, hash, embedding)
             SELECT $1, $2, $3, v.h, v.e::vector FROM unnest($4::text[], $5::text[]) AS v(h, e)
             ON CONFLICT DO NOTHING",
            vec![
                wid.get().into(),
                repo.into(),
                embedder.model.clone().into(),
                hs.into(),
                lits.into(),
            ],
        ))
        .await?;
    }
    db.execute_raw(stmt(
        "DELETE FROM graph_embeddings e WHERE e.repo_id = $1
           AND (e.model <> $2 OR NOT EXISTS (SELECT 1 FROM graph_symbols s
                 WHERE s.repo_id = e.repo_id AND s.embed_hash = e.hash))",
        vec![repo.into(), embedder.model.clone().into()],
    ))
    .await?;
    let count = db
        .query_one_raw(stmt(
            "SELECT count(*) AS n FROM graph_symbols s JOIN graph_embeddings e
               ON e.repo_id = s.repo_id AND e.model = $2 AND e.hash = s.embed_hash
              WHERE s.repo_id = $1",
            vec![repo.into(), embedder.model.clone().into()],
        ))
        .await?
        .and_then(|r| r.try_get::<i64>("", "n").ok())
        .unwrap_or(0);
    report.symbols = usize::try_from(count).unwrap_or(0);
    report.ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    record(db, repo, report).await
}

async fn record(
    db: &DatabaseConnection,
    repo: Uuid,
    report: EmbedReport,
) -> Result<EmbedReport, DbErr> {
    db.execute_raw(stmt(
        "UPDATE graph_repos SET embed_symbols = $2, embed_ms = $3, embed_error = $4,
                embedded_at = CASE WHEN $4 = '' THEN now() ELSE embedded_at END
          WHERE repo_id = $1",
        vec![
            repo.into(),
            i32::try_from(report.symbols).unwrap_or(i32::MAX).into(),
            i32::try_from(report.ms).unwrap_or(i32::MAX).into(),
            report.error.chars().take(300).collect::<String>().into(),
        ],
    ))
    .await?;
    if !report.error.is_empty() {
        tracing::warn!(error = %report.error, %repo, "graph embedding failed");
    }
    Ok(report)
}

/// Kirim index embedding ke antrean (setelah index tree-sitter selesai).
///
/// # Errors
///
/// Error antrean.
pub async fn enqueue(
    ctx: &loco_rs::app::AppContext,
    workspace: Uuid,
    repo: Uuid,
) -> loco_rs::Result<()> {
    use loco_rs::bgworker::BackgroundWorker;
    crate::workers::graph_embed::GraphEmbedWorker::perform_later(
        ctx,
        crate::workers::graph_embed::GraphEmbedArgs {
            workspace_id: workspace,
            repo_id: repo,
        },
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_vectors_are_normalized_and_word_based() {
        let a = fake_vector("refundOrder supervisor");
        let b = fake_vector("refund_order Supervisor");
        let c = fake_vector("reserveStock warehouse");
        let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
        assert!((dot(&a, &a) - 1.0).abs() < 1e-4);
        assert!(dot(&a, &b) > 0.5);
        assert!(dot(&a, &c) < dot(&a, &b));
        assert_eq!(literal(&[0.5, -1.0]), "[0.500000,-1.000000]");
        assert!(spec("multilingual-e5-small").is_some());
        assert!(spec("x").is_none());
    }
}
