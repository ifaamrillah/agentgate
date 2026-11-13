//! Protokol runner: bentuk JSON API `/api/runner/…` dan pesan WebSocket `/api/runner/ws`.
//!
//! Kontrol (realtime) lewat WebSocket: runner mengirim [`ClientMsg`], server mendorong
//! [`ServerMsg`]. Data (log, langkah, evidence, izin, hasil) lewat HTTP JSON supaya bisa
//! diulang dan dibatasi ukurannya. Semua waktu RFC 3339 UTC.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::persona::Level;

macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $v:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($(#[$vm])* $v),+ }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$v),+];

            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$v => $s),+ }
            }

            #[must_use]
            pub fn parse(value: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|x| x.as_str() == value.trim())
            }
        }
    };
}

str_enum!(
    /// Status job. `queued` → `claimed` (lock executor) → `running` → `paused` (menunggu izin) /
    /// `quota` (kuota AI habis) → `succeeded` | `failed` | `cancelled`.
    JobStatus {
        Queued => "queued",
        Claimed => "claimed",
        Running => "running",
        Paused => "paused",
        Quota => "quota",
        Succeeded => "succeeded",
        Failed => "failed",
        Cancelled => "cancelled",
    }
);

impl JobStatus {
    /// Job belum selesai (masih di antrean atau dipegang runner).
    #[must_use]
    pub fn is_open(self) -> bool {
        !matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }

    /// Job sedang dipegang runner (proses client hidup atau dijeda).
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }
}

str_enum!(
    /// Asal job: stage tiket, `@agent-…` di komentar, atau scan/update knowledge (tanpa tiket,
    /// `JobSpec::ticket` kosong).
    JobKind {
        Stage => "stage",
        Comment => "comment",
        Knowledge => "knowledge",
    }
);

str_enum!(
    /// Mode eksekusi: headless (otomatis) atau sesi terminal dengan manusia.
    JobMode {
        Auto => "auto",
        Interactive => "interactive",
    }
);

str_enum!(
    /// Jenis evidence yang dihasilkan runner.
    EvidenceKind {
        TestReport => "test_report",
        Screenshot => "screenshot",
        Log => "log",
    }
);

str_enum!(
    /// Keadaan satu langkah job.
    StepState {
        Wait => "wait",
        Now => "now",
        Ok => "ok",
        Fail => "fail",
    }
);

str_enum!(
    /// Jenis baris log (warna di web).
    LogStream {
        Info => "info",
        Out => "out",
        Err => "err",
        Tool => "tool",
        Ok => "ok",
        Warn => "warn",
        Fail => "fail",
    }
);

str_enum!(
    /// Jawaban permintaan izin aksi berbahaya.
    PermissionStatus {
        Pending => "pending",
        Allowed => "allowed",
        /// Diizinkan untuk semua job tiket ini (aksi yang sama).
        AllowedTicket => "allowed_ticket",
        Denied => "denied",
    }
);

str_enum!(
    /// Hasil akhir yang dilaporkan runner.
    Outcome {
        Succeeded => "succeeded",
        Failed => "failed",
        /// Kuota AI user habis; job menunggu keputusan (tunggu reset / lepas).
        Quota => "quota",
        Cancelled => "cancelled",
    }
);

/// `POST /api/runner/register`: daftarkan (atau perbarui) runner perangkat ini.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRunner {
    /// Id runner yang sudah tersimpan di perangkat (daftar ulang), atau `None` untuk baru.
    #[serde(default)]
    pub runner_id: Option<Uuid>,
    pub name: String,
    pub os: String,
    /// Client default runner (kunci katalog). Kosong = "Client default" di pengaturan runner web.
    #[serde(default)]
    pub client: String,
    pub version: String,
    /// Kunci publik Ed25519 (base64, 32 byte).
    pub public_key: String,
}

/// Runner terdaftar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerInfo {
    pub id: Uuid,
    pub name: String,
    pub workspace: String,
    pub user_email: String,
    pub auto_claim: bool,
    #[serde(default)]
    pub max_parallel: Option<u32>,
    /// Client default runner yang dipakai server untuk job runner ini.
    #[serde(default)]
    pub client: String,
}

/// Pesan runner → server (WebSocket).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Pesan pertama setelah tersambung.
    Hello { runner_id: Uuid, version: String },
    /// Tiap ≤ 30 dtk; `running` = job yang sedang dikerjakan runner.
    Heartbeat { running: Vec<Uuid> },
}

/// Pesan server → runner (WebSocket).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome {
        runner_id: Uuid,
    },
    /// Job yang terkunci untuk pemilik runner dan siap dijalankan di runner ini.
    JobReady {
        job: JobSummary,
    },
    /// Job dibatalkan dari web; runner menghentikan proses client.
    JobCancelled {
        job_id: Uuid,
    },
    /// Permintaan izin dijawab.
    PermissionDecided {
        job_id: Uuid,
        permission_id: Uuid,
        status: PermissionStatus,
    },
    Error {
        message: String,
    },
}

/// Ringkasan job di antrean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobSummary {
    pub id: Uuid,
    pub kind: JobKind,
    pub mode: JobMode,
    pub status: JobStatus,
    pub ticket: String,
    pub title: String,
    pub stage: String,
    pub stage_name: String,
    pub persona: String,
    pub priority: i16,
    /// Job ini terkunci untuk user token ini.
    pub mine: bool,
    /// User token ini executor stage (boleh Ambil).
    pub can_claim: bool,
    pub created_at: String,
}

/// File aturan Agent Gate yang ditulis ke repo/worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleFile {
    pub path: String,
    pub body: String,
}

/// Izin satu aksi berbahaya di persona job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DangerRule {
    pub action: String,
    pub level: Level,
}

/// `POST /api/runner/jobs/{id}/start`: semua yang dibutuhkan runner untuk menjalankan job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSpec {
    pub id: Uuid,
    pub kind: JobKind,
    pub mode: JobMode,
    pub ticket: String,
    pub title: String,
    pub url: String,
    pub project: String,
    /// Nama repo project (cocokkan dengan repo lokal yang di-`init`).
    pub repos: Vec<String>,
    pub stage: String,
    pub stage_name: String,
    pub persona: String,
    pub persona_name: String,
    /// Model persona untuk client job (kosong = default client).
    pub model: String,
    pub effort: String,
    pub client: String,
    /// Slash command stage (`/ag-<stage>`) untuk fallback interaktif.
    pub command: String,
    /// Prompt lengkap (persona, stage, tiket). Tidak pernah lewat shell.
    pub prompt: String,
    pub branch: String,
    /// Branch dasar worktree dan target PR (branch default repo). Kosong = HEAD repo lokal.
    #[serde(default)]
    pub base_branch: String,
    /// Konvensi git project (hasil kode, gaya dan bahasa commit/PR).
    #[serde(default)]
    pub git: crate::git::GitSpec,
    pub mcp_url: String,
    pub rules: Vec<RuleFile>,
    pub danger: Vec<DangerRule>,
    /// Aksi yang sudah diizinkan untuk tiket ini ("Selalu untuk tiket ini").
    pub allowed_actions: Vec<String>,
}

/// Satu baris log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub stream: LogStream,
    pub text: String,
}

/// `POST /api/runner/jobs/{id}/logs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogBatch {
    pub lines: Vec<LogLine>,
}

/// Satu langkah job (rencana dari agent atau fase runner).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub state: StepState,
    #[serde(default)]
    pub seconds: Option<u32>,
}

/// `POST /api/runner/jobs/{id}/steps`: daftar langkah lengkap (menggantikan yang lama).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepsUpdate {
    pub steps: Vec<Step>,
}

/// Ringkasan evidence (ikut ditandatangani). Urutan field = urutan serialisasi kanonik.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSummary {
    /// Hasil test: lulus/gagal. `None` untuk screenshot/log.
    #[serde(default)]
    pub passed: Option<bool>,
    #[serde(default)]
    pub total: Option<u32>,
    #[serde(default)]
    pub failed: Option<u32>,
    /// Perintah yang dijalankan atau keterangan singkat.
    #[serde(default)]
    pub note: String,
}

/// `POST /api/runner/jobs/{id}/evidence`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceUpload {
    pub kind: EvidenceKind,
    pub name: String,
    pub content_type: String,
    /// Isi berkas (base64 standar).
    pub content: String,
    pub sha256: String,
    pub summary: EvidenceSummary,
    /// Tanda tangan Ed25519 (base64) atas [`crate::runner::signing::evidence_payload`].
    pub signature: String,
}

/// `POST /api/runner/jobs/{id}/permissions`: agent ingin menjalankan aksi berbahaya.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionAsk {
    pub command: String,
    pub action: String,
    #[serde(default)]
    pub reason: String,
}

/// Permintaan izin dan statusnya.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionInfo {
    pub id: Uuid,
    pub status: PermissionStatus,
}

/// `POST /api/runner/jobs/{id}/finish`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FinishReport {
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tokens: Option<u64>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    /// Kuota habis: waktu reset (RFC 3339), bila client menyebutkannya.
    #[serde(default)]
    pub quota_reset_at: Option<String>,
    /// Rincian pemakaian token dari client (M10, halaman Penghemat token).
    #[serde(default)]
    pub usage: Option<TokenUsage>,
    /// File yang diubah job per repo (git diff base…HEAD), untuk sorot tiket di graph viewer.
    #[serde(default)]
    pub changed_files: Vec<ChangedFile>,
}

/// Pemakaian token satu job seperti dilaporkan client (Claude Code `result.usage`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub cache_read: u64,
    #[serde(default)]
    pub cache_write: u64,
}

impl TokenUsage {
    /// Total semua jenis token.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }
}

/// Satu file yang diubah job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    /// Nama repo project (sama dengan [`JobSpec::repos`]).
    pub repo: String,
    pub path: String,
}

/// Batas jumlah file yang dilaporkan per job (sisanya dibuang runner).
pub const CHANGED_FILES_MAX: usize = 2000;

/// `POST /api/runner/jobs/{id}/pull-request`: branch tiket sudah di-push; server membuka PR
/// lewat provider git (atau mengembalikan PR terbuka untuk branch itu).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestOpen {
    /// Nama repo project (sama dengan [`JobSpec::repos`]).
    pub repo: String,
    pub branch: String,
    #[serde(default)]
    pub base: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
}

/// PR yang dibuka/ditemukan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestInfo {
    pub number: i64,
    pub url: String,
    pub state: crate::git::PrState,
    /// `false` = PR untuk branch ini sudah ada sebelumnya.
    pub created: bool,
}

/// `POST /api/cli/rules/pull-request`: branch `agentgate/sync-rules` sudah di-push.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesPullRequest {
    pub project: String,
    pub repo: String,
}

/// `POST /api/runner/drift`: hasil cek checksum file aturan satu repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub repo: String,
    pub checksum: String,
    pub changed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_round_trip() {
        for s in JobStatus::ALL {
            assert_eq!(JobStatus::parse(s.as_str()), Some(*s));
            assert_eq!(
                serde_json::to_value(s).unwrap(),
                serde_json::Value::from(s.as_str())
            );
        }
        assert!(JobStatus::Paused.is_open() && !JobStatus::Failed.is_open());
        assert_eq!(
            PermissionStatus::parse("allowed_ticket"),
            Some(PermissionStatus::AllowedTicket)
        );
    }

    #[test]
    fn ws_messages_are_tagged() {
        let m = ClientMsg::Heartbeat { running: vec![] };
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            r#"{"type":"heartbeat","running":[]}"#
        );
        let s: ServerMsg = serde_json::from_str(
            r#"{"type":"job_cancelled","job_id":"00000000-0000-0000-0000-000000000000"}"#,
        )
        .unwrap();
        assert_eq!(
            s,
            ServerMsg::JobCancelled {
                job_id: Uuid::nil()
            }
        );
    }
}
