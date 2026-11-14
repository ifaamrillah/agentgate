//! Audit log append-only: siapa mengubah apa di workspace, kapan. Baris hanya ditambah lewat
//! [`ActiveModel::append`], dipanggil `services::audit::record` di transaksi yang sama dengan
//! perubahannya; model ini sengaja tidak punya jalur update atau delete (rules "Audit log").
//! Halaman audit membaca lewat [`Model::page`]: keyset `(created_at, id)` menurun (rules §5),
//! memakai index `(workspace_id, created_at DESC, id DESC)`, dengan filter [`AuditGroup`].
//!
//! Baris audit ikut terhapus saat workspace-nya dihapus permanen (FK `ON DELETE CASCADE`, job
//! pembersihan): audit log adalah milik workspace, bukan arsip di luar workspace.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use loco_rs::model::{TenantEntity, TenantQueryExt};
use sea_orm::{
    ActiveValue::Set, Iterable, QueryOrder, QuerySelect, entity::prelude::*, sea_query::ExprTrait,
};

pub use super::_entities::audit_events::{ActiveModel, Column, Entity, Model};
use super::workspaces::WorkspaceId;

/// Jenis kejadian. Disimpan sebagai string `<subjek>.<aksi>` dengan CHECK constraint (rules §4,
/// dijaga `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum AuditAction {
    #[sea_orm(string_value = "workspace.created")]
    WorkspaceCreated,
    #[sea_orm(string_value = "workspace.updated")]
    WorkspaceUpdated,
    #[sea_orm(string_value = "workspace.require_2fa_changed")]
    WorkspaceRequire2faChanged,
    #[sea_orm(string_value = "workspace.deletion_scheduled")]
    WorkspaceDeletionScheduled,
    #[sea_orm(string_value = "workspace.deletion_cancelled")]
    WorkspaceDeletionCancelled,
    #[sea_orm(string_value = "member.invited")]
    MemberInvited,
    #[sea_orm(string_value = "member.joined")]
    MemberJoined,
    #[sea_orm(string_value = "member.role_changed")]
    MemberRoleChanged,
    #[sea_orm(string_value = "member.removed")]
    MemberRemoved,
    #[sea_orm(string_value = "member.left")]
    MemberLeft,
    #[sea_orm(string_value = "member.job_title_changed")]
    MemberJobTitleChanged,
    /// Undangan dibatalkan sebelum diterima (Pengaturan → Anggota).
    #[sea_orm(string_value = "member.invitation_revoked")]
    MemberInvitationRevoked,
    #[sea_orm(string_value = "role.created")]
    RoleCreated,
    #[sea_orm(string_value = "role.updated")]
    RoleUpdated,
    #[sea_orm(string_value = "role.deleted")]
    RoleDeleted,
    #[sea_orm(string_value = "job_title.created")]
    JobTitleCreated,
    #[sea_orm(string_value = "job_title.updated")]
    JobTitleUpdated,
    #[sea_orm(string_value = "job_title.deleted")]
    JobTitleDeleted,
    #[sea_orm(string_value = "team.created")]
    TeamCreated,
    #[sea_orm(string_value = "team.updated")]
    TeamUpdated,
    #[sea_orm(string_value = "team.deleted")]
    TeamDeleted,
    #[sea_orm(string_value = "team.member_added")]
    TeamMemberAdded,
    #[sea_orm(string_value = "team.member_removed")]
    TeamMemberRemoved,
    #[sea_orm(string_value = "project.created")]
    ProjectCreated,
    #[sea_orm(string_value = "project.updated")]
    ProjectUpdated,
    #[sea_orm(string_value = "project.archived")]
    ProjectArchived,
    #[sea_orm(string_value = "project.access_granted")]
    ProjectAccessGranted,
    #[sea_orm(string_value = "project.access_changed")]
    ProjectAccessChanged,
    #[sea_orm(string_value = "project.access_revoked")]
    ProjectAccessRevoked,
    #[sea_orm(string_value = "repo.added")]
    RepoAdded,
    #[sea_orm(string_value = "repo.updated")]
    RepoUpdated,
    #[sea_orm(string_value = "repo.removed")]
    RepoRemoved,
    /// Tiket dipindah ke sampah.
    #[sea_orm(string_value = "ticket.trashed")]
    TicketTrashed,
    #[sea_orm(string_value = "ticket.restored")]
    TicketRestored,
    /// Tiket dihapus permanen dari sampah (oleh orang; purge 30 hari tidak diaudit per tiket).
    #[sea_orm(string_value = "ticket.purged")]
    TicketPurged,
    #[sea_orm(string_value = "ticket_type.created")]
    TicketTypeCreated,
    #[sea_orm(string_value = "ticket_type.updated")]
    TicketTypeUpdated,
    #[sea_orm(string_value = "ticket_type.deleted")]
    TicketTypeDeleted,
    #[sea_orm(string_value = "custom_field.created")]
    CustomFieldCreated,
    #[sea_orm(string_value = "custom_field.updated")]
    CustomFieldUpdated,
    #[sea_orm(string_value = "custom_field.deleted")]
    CustomFieldDeleted,
    /// Template kode tiket workspace atau project diubah.
    #[sea_orm(string_value = "ticket_code.updated")]
    TicketCodeUpdated,
    #[sea_orm(string_value = "workflow.created")]
    WorkflowCreated,
    /// Versi baru workflow disimpan dari builder atau YAML.
    #[sea_orm(string_value = "workflow.updated")]
    WorkflowUpdated,
    #[sea_orm(string_value = "workflow.deleted")]
    WorkflowDeleted,
    /// Tiket dipindah ke stage lain karena stage-nya dihapus atau workflow tipenya diganti.
    #[sea_orm(string_value = "workflow.migrated")]
    WorkflowMigrated,
    /// Workflow per tipe tiket atau aturan transisi project diubah.
    #[sea_orm(string_value = "project.workflows_changed")]
    ProjectWorkflowsChanged,
    /// Admin melompati stage (alasan di `changes`).
    #[sea_orm(string_value = "ticket.stage_jumped")]
    TicketStageJumped,
    /// Admin memaksa transisi walau gate belum terpenuhi (alasan di `changes`).
    #[sea_orm(string_value = "ticket.transition_forced")]
    TicketTransitionForced,
    #[sea_orm(string_value = "persona.created")]
    PersonaCreated,
    #[sea_orm(string_value = "persona.updated")]
    PersonaUpdated,
    /// Persona dihapus; pengganti di `changes`.
    #[sea_orm(string_value = "persona.deleted")]
    PersonaDeleted,
    #[sea_orm(string_value = "skill_pack.created")]
    SkillPackCreated,
    #[sea_orm(string_value = "skill_pack.updated")]
    SkillPackUpdated,
    /// Skill pack dikirim ke review (hasil pindai di `changes`).
    #[sea_orm(string_value = "skill_pack.submitted")]
    SkillPackSubmitted,
    #[sea_orm(string_value = "skill_pack.approved")]
    SkillPackApproved,
    #[sea_orm(string_value = "skill_pack.rejected")]
    SkillPackRejected,
    #[sea_orm(string_value = "skill_pack.deleted")]
    SkillPackDeleted,
    /// Client AI disetujui mengakses workspace lewat MCP (consent OAuth).
    #[sea_orm(string_value = "mcp.connected")]
    McpConnected,
    /// Akses client AI dicabut.
    #[sea_orm(string_value = "mcp.revoked")]
    McpRevoked,
    /// Runner perangkat didaftarkan (M5).
    #[sea_orm(string_value = "runner.registered")]
    RunnerRegistered,
    #[sea_orm(string_value = "runner.revoked")]
    RunnerRevoked,
    /// Runner ditandai/dilepas sebagai runner tim, atau auto-ambil/batas paralelnya diubah (R1).
    #[sea_orm(string_value = "runner.updated")]
    RunnerUpdated,
    /// Job dibatalkan executor/Admin project.
    #[sea_orm(string_value = "job.cancelled")]
    JobCancelled,
    /// Permintaan izin aksi berbahaya dijawab.
    #[sea_orm(string_value = "job.permission_decided")]
    JobPermissionDecided,
    /// Provider git dihubungkan ke workspace (M6; token tidak pernah dicatat).
    #[sea_orm(string_value = "git.connected")]
    GitConnected,
    /// Token provider git diganti.
    #[sea_orm(string_value = "git.updated")]
    GitUpdated,
    #[sea_orm(string_value = "git.disconnected")]
    GitDisconnected,
    /// Repo ditautkan ke koneksi provider (+ webhook).
    #[sea_orm(string_value = "repo.linked")]
    RepoLinked,
    /// Konvensi git project diubah.
    #[sea_orm(string_value = "project.git_updated")]
    ProjectGitUpdated,
    /// PR di-merge Agent Gate (mode merge otomatis).
    #[sea_orm(string_value = "pull_request.merged")]
    PullRequestMerged,
    /// Toggle penghemat token project diubah (M10).
    #[sea_orm(string_value = "project.token_saver_updated")]
    ProjectTokenSaverUpdated,
    /// Tampilan karakter AI Office diubah (M10).
    #[sea_orm(string_value = "office.look_updated")]
    OfficeLookUpdated,
    /// Token API dibuat (M11; token mentah tidak pernah dicatat).
    #[sea_orm(string_value = "api_token.created")]
    ApiTokenCreated,
    #[sea_orm(string_value = "api_token.revoked")]
    ApiTokenRevoked,
    /// Rate limit per token diubah Owner/Admin (R1).
    #[sea_orm(string_value = "api_token.updated")]
    ApiTokenUpdated,
    /// Webhook keluar dibuat (M11).
    #[sea_orm(string_value = "webhook.created")]
    WebhookCreated,
    /// Webhook diubah (URL, event, project, aktif/nonaktif).
    #[sea_orm(string_value = "webhook.updated")]
    WebhookUpdated,
    #[sea_orm(string_value = "webhook.deleted")]
    WebhookDeleted,
    /// Secret webhook dibuat ulang (secret tidak pernah dicatat).
    #[sea_orm(string_value = "webhook.secret_rotated")]
    WebhookSecretRotated,
    /// Ekspor data workspace diminta (M11).
    #[sea_orm(string_value = "export.requested")]
    ExportRequested,
    /// Berkas ekspor diunduh.
    #[sea_orm(string_value = "export.downloaded")]
    ExportDownloaded,
    /// Impor CSV tiket selesai.
    #[sea_orm(string_value = "import.completed")]
    ImportCompleted,
    /// Bukti gate manual diunggah anggota (R1).
    #[sea_orm(string_value = "evidence.submitted")]
    EvidenceSubmitted,
    /// Bukti gate manual disetujui approver stage (memenuhi gate).
    #[sea_orm(string_value = "evidence.approved")]
    EvidenceApproved,
    #[sea_orm(string_value = "evidence.rejected")]
    EvidenceRejected,
    /// Koneksi Jira dibuat (M11; token tidak pernah dicatat).
    #[sea_orm(string_value = "jira.connected")]
    JiraConnected,
    /// Koneksi Jira diubah (kredensial, project, mode, pemetaan, jeda).
    #[sea_orm(string_value = "jira.updated")]
    JiraUpdated,
    /// Koneksi Jira diputus (data yang sudah diimpor tetap ada).
    #[sea_orm(string_value = "jira.disconnected")]
    JiraDisconnected,
}

/// Kelompok aksi untuk filter halaman audit: awalan `<subjek>` dari aksi, kecuali jabatan
/// yang ikut kelompok anggota (jabatan adalah atribut anggota, dikelola `ManageMembers`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum AuditGroup {
    Workspace,
    Member,
    Role,
    Team,
    Project,
    Repo,
    Ticket,
    /// Persona, skill pack, dan client MCP.
    Agent,
}

impl AuditGroup {
    /// Nilai di query string (`?group=member`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Member => "member",
            Self::Role => "role",
            Self::Team => "team",
            Self::Project => "project",
            Self::Repo => "repo",
            Self::Ticket => "ticket",
            Self::Agent => "agent",
        }
    }

    /// Kelompok dari nilai query string; nilai tak dikenal → `None` (filter diabaikan).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::iter().find(|g| g.as_str() == value)
    }

    /// Semua aksi kelompok ini.
    #[must_use]
    pub fn actions(self) -> Vec<AuditAction> {
        AuditAction::iter().filter(|a| a.group() == self).collect()
    }
}

impl AuditAction {
    /// Kelompok filter aksi ini. `match` lengkap: aksi baru wajib diberi kelompok.
    #[must_use]
    pub fn group(self) -> AuditGroup {
        match self {
            Self::WorkspaceCreated
            | Self::WorkspaceUpdated
            | Self::WorkspaceRequire2faChanged
            | Self::WorkspaceDeletionScheduled
            | Self::WorkspaceDeletionCancelled => AuditGroup::Workspace,
            Self::MemberInvited
            | Self::MemberJoined
            | Self::MemberRoleChanged
            | Self::MemberRemoved
            | Self::MemberLeft
            | Self::MemberJobTitleChanged
            | Self::MemberInvitationRevoked
            | Self::JobTitleCreated
            | Self::JobTitleUpdated
            | Self::JobTitleDeleted => AuditGroup::Member,
            Self::RoleCreated | Self::RoleUpdated | Self::RoleDeleted => AuditGroup::Role,
            Self::TeamCreated
            | Self::TeamUpdated
            | Self::TeamDeleted
            | Self::TeamMemberAdded
            | Self::TeamMemberRemoved => AuditGroup::Team,
            Self::ProjectCreated
            | Self::ProjectUpdated
            | Self::ProjectArchived
            | Self::ProjectAccessGranted
            | Self::ProjectAccessChanged
            | Self::ProjectAccessRevoked
            | Self::ProjectWorkflowsChanged => AuditGroup::Project,
            Self::RepoAdded
            | Self::RepoUpdated
            | Self::RepoRemoved
            | Self::RepoLinked
            | Self::GitConnected
            | Self::GitUpdated
            | Self::GitDisconnected
            | Self::PullRequestMerged => AuditGroup::Repo,
            Self::TicketTrashed
            | Self::TicketRestored
            | Self::TicketPurged
            | Self::TicketTypeCreated
            | Self::TicketTypeUpdated
            | Self::TicketTypeDeleted
            | Self::CustomFieldCreated
            | Self::CustomFieldUpdated
            | Self::CustomFieldDeleted
            | Self::TicketCodeUpdated
            | Self::WorkflowCreated
            | Self::WorkflowUpdated
            | Self::WorkflowDeleted
            | Self::WorkflowMigrated
            | Self::TicketStageJumped
            | Self::TicketTransitionForced
            | Self::EvidenceSubmitted
            | Self::EvidenceApproved
            | Self::EvidenceRejected => AuditGroup::Ticket,
            Self::PersonaCreated
            | Self::PersonaUpdated
            | Self::PersonaDeleted
            | Self::SkillPackCreated
            | Self::SkillPackUpdated
            | Self::SkillPackSubmitted
            | Self::SkillPackApproved
            | Self::SkillPackRejected
            | Self::SkillPackDeleted
            | Self::McpConnected
            | Self::McpRevoked
            | Self::RunnerRegistered
            | Self::RunnerRevoked
            | Self::RunnerUpdated
            | Self::JobCancelled
            | Self::JobPermissionDecided => AuditGroup::Agent,
            Self::ProjectGitUpdated | Self::ProjectTokenSaverUpdated => AuditGroup::Project,
            Self::OfficeLookUpdated => AuditGroup::Agent,
            Self::ApiTokenCreated
            | Self::ApiTokenRevoked
            | Self::ApiTokenUpdated
            | Self::WebhookCreated
            | Self::WebhookUpdated
            | Self::WebhookDeleted
            | Self::WebhookSecretRotated
            | Self::JiraConnected
            | Self::JiraUpdated
            | Self::JiraDisconnected => AuditGroup::Workspace,
            Self::ExportRequested | Self::ExportDownloaded | Self::ImportCompleted => {
                AuditGroup::Workspace
            }
        }
    }
}

/// Posisi keyset: baris terakhir yang sudah tampil. Halaman berikutnya berisi baris yang
/// `(created_at, id)`-nya lebih kecil. Di URL berupa base64url tanpa padding dari
/// `<mikrodetik unix>_<uuid>` (presisi `timestamptz` Postgres adalah mikrodetik, jadi nilainya
/// kembali persis). Opaque supaya tidak dianggap API yang bisa dirakit sendiri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditCursor {
    pub created_at: DateTimeWithTimeZone,
    pub id: Uuid,
}

impl AuditCursor {
    #[must_use]
    pub fn after(event: &Model) -> Self {
        Self {
            created_at: event.created_at,
            id: event.id,
        }
    }

    /// Nilai `?before=`.
    #[must_use]
    pub fn to_param(&self) -> String {
        URL_SAFE_NO_PAD.encode(format!(
            "{}_{}",
            self.created_at.timestamp_micros(),
            self.id.simple()
        ))
    }

    /// Kursor dari `?before=`; bentuk yang tidak valid → `None` (kembali ke halaman pertama).
    /// Waktu di luar `[epoch, sekarang + 1 hari]` juga `None`: tidak ada baris audit di luar
    /// rentang itu, dan nilai buatan di luar rentang `timestamptz` Postgres akan membuat query
    /// gagal (500) alih-alih diabaikan.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let raw = URL_SAFE_NO_PAD.decode(value).ok()?;
        let (micros, id) = std::str::from_utf8(&raw).ok()?.split_once('_')?;
        let micros: i64 = micros.parse().ok()?;
        let latest = (chrono::Utc::now() + chrono::Duration::days(1)).timestamp_micros();
        if !(0..=latest).contains(&micros) {
            return None;
        }
        let created_at = chrono::DateTime::from_timestamp_micros(micros)?;
        Some(Self {
            created_at: created_at.into(),
            id: Uuid::try_parse(id).ok()?,
        })
    }
}

/// Satu halaman audit: baris (terbaru dulu) dan kursor halaman berikutnya bila masih ada.
#[derive(Debug)]
pub struct AuditPage {
    pub events: Vec<Model>,
    pub next: Option<AuditCursor>,
}

/// Jenis subjek yang diubah (`subject_type`). Disimpan sebagai string dengan CHECK constraint
/// (rules §4, dijaga `tests/models/enums.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum AuditSubject {
    #[sea_orm(string_value = "workspace")]
    Workspace,
    #[sea_orm(string_value = "member")]
    Member,
    #[sea_orm(string_value = "role")]
    Role,
    #[sea_orm(string_value = "job_title")]
    JobTitle,
    #[sea_orm(string_value = "team")]
    Team,
    #[sea_orm(string_value = "project")]
    Project,
    #[sea_orm(string_value = "project_grant")]
    ProjectGrant,
    #[sea_orm(string_value = "repo")]
    Repo,
    #[sea_orm(string_value = "invitation")]
    Invitation,
    #[sea_orm(string_value = "ticket")]
    Ticket,
    #[sea_orm(string_value = "ticket_type")]
    TicketType,
    #[sea_orm(string_value = "custom_field")]
    CustomField,
    #[sea_orm(string_value = "workflow")]
    Workflow,
    #[sea_orm(string_value = "persona")]
    Persona,
    #[sea_orm(string_value = "skill_pack")]
    SkillPack,
    /// Grant OAuth client MCP (`oauth_grants`).
    #[sea_orm(string_value = "mcp_client")]
    McpClient,
    #[sea_orm(string_value = "runner")]
    Runner,
    #[sea_orm(string_value = "job")]
    Job,
    #[sea_orm(string_value = "git_connection")]
    GitConnection,
    #[sea_orm(string_value = "pull_request")]
    PullRequest,
    #[sea_orm(string_value = "api_token")]
    ApiToken,
    #[sea_orm(string_value = "webhook")]
    Webhook,
    #[sea_orm(string_value = "data_export")]
    DataExport,
    #[sea_orm(string_value = "ticket_import")]
    TicketImport,
    #[sea_orm(string_value = "jira_connection")]
    JiraConnection,
    /// Bukti gate manual (`manual_evidence`, R1).
    #[sea_orm(string_value = "evidence")]
    Evidence,
}

/// Data satu baris audit untuk [`ActiveModel::append`].
#[derive(Debug)]
pub struct NewAuditEvent {
    pub workspace_id: WorkspaceId,
    pub actor_id: Option<Uuid>,
    pub action: AuditAction,
    pub subject_type: AuditSubject,
    pub subject_id: Uuid,
    pub changes: serde_json::Value,
    pub request_id: Option<String>,
}

impl TenantEntity for Entity {
    type TenantId = WorkspaceId;

    fn tenant_column() -> Column {
        Column::WorkspaceId
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert {
            // Append-only: tidak ada jalur aplikasi yang boleh mengubah baris audit.
            return Err(DbErr::Custom("audit_events is append-only".to_string()));
        }
        if self.id.is_not_set() {
            self.id = Set(Uuid::now_v7());
        }
        Ok(self)
    }
}

impl Model {
    /// Aksi bertipe dari kolom string. Gagal hanya bila CHECK constraint dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`AuditAction`].
    pub fn action(&self) -> Result<AuditAction, DbErr> {
        AuditAction::try_from_value(&self.action)
    }

    /// Jenis subjek bertipe dari kolom string. Gagal hanya bila CHECK constraint dilewati.
    ///
    /// # Errors
    ///
    /// Nilai kolom bukan varian [`AuditSubject`].
    pub fn subject(&self) -> Result<AuditSubject, DbErr> {
        AuditSubject::try_from_value(&self.subject_type)
    }

    /// Satu halaman (`limit` baris) kejadian di workspace ini, terbaru dulu, sebelum `before`
    /// (keyset `(created_at, id) < (…)`), hanya kelompok `group` bila diisi. Mengambil satu
    /// baris lebih untuk tahu apakah ada halaman berikutnya.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn page<C: ConnectionTrait>(
        db: &C,
        workspace_id: WorkspaceId,
        group: Option<AuditGroup>,
        before: Option<AuditCursor>,
        limit: u64,
    ) -> Result<AuditPage, DbErr> {
        let mut query = Entity::find().in_tenant(workspace_id);
        if let Some(group) = group {
            query = query
                .filter(Column::Action.is_in(group.actions().into_iter().map(|a| a.to_value())));
        }
        if let Some(cursor) = before {
            query = query.filter(
                Expr::tuple([Expr::col(Column::CreatedAt), Expr::col(Column::Id)]).lt(Expr::tuple(
                    [Expr::val(cursor.created_at), Expr::val(cursor.id)],
                )),
            );
        }
        let mut events = query
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(limit + 1)
            .all(db)
            .await?;
        let more = events.len() as u64 > limit;
        events.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        let next = if more {
            events.last().map(AuditCursor::after)
        } else {
            None
        };
        Ok(AuditPage { events, next })
    }
}

impl ActiveModel {
    /// Tambahkan satu baris audit.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn append<C: ConnectionTrait>(db: &C, new: NewAuditEvent) -> Result<Model, DbErr> {
        Self {
            workspace_id: Set(new.workspace_id.get()),
            actor_id: Set(new.actor_id),
            action: Set(new.action.to_value()),
            subject_type: Set(new.subject_type.to_value()),
            subject_id: Set(new.subject_id),
            changes: Set(new.changes),
            request_id: Set(new.request_id),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use sea_orm::prelude::Uuid;

    use super::AuditCursor;

    fn cursor(micros: i64) -> AuditCursor {
        AuditCursor {
            created_at: chrono::DateTime::from_timestamp_micros(micros)
                .unwrap()
                .into(),
            id: Uuid::now_v7(),
        }
    }

    #[test]
    fn cursor_param_is_opaque_and_round_trips() {
        let c = cursor(chrono::Utc::now().timestamp_micros());
        let param = c.to_param();
        assert!(
            param
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        );
        assert!(!param.contains('='));
        let raw = String::from_utf8(URL_SAFE_NO_PAD.decode(&param).unwrap()).unwrap();
        assert_eq!(
            raw,
            format!("{}_{}", c.created_at.timestamp_micros(), c.id.simple())
        );
        assert_eq!(AuditCursor::parse(&param), Some(c));
        // Batas bawah (epoch) masih sah.
        let epoch = cursor(0);
        assert_eq!(AuditCursor::parse(&epoch.to_param()), Some(epoch));
    }

    #[test]
    fn cursor_outside_range_or_not_encoded_is_none() {
        let id = Uuid::now_v7().simple();
        let enc = |s: String| URL_SAFE_NO_PAD.encode(s);
        let tomorrow_plus = (chrono::Utc::now() + chrono::Duration::days(2)).timestamp_micros();
        for value in [
            enc(format!("-1_{id}")),
            enc(format!("{}_{id}", i64::MIN)),
            enc(format!("{tomorrow_plus}_{id}")),
            enc(format!("{}_{id}", i64::MAX)),
            format!("{}_{id}", chrono::Utc::now().timestamp_micros()),
            "%%%".to_string(),
            String::new(),
            enc("bukan".to_string()),
        ] {
            assert_eq!(AuditCursor::parse(&value), None, "{value}");
        }
        assert!(AuditCursor::parse(&enc(format!("0_{id}"))).is_some());
    }
}
