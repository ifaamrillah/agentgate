//! M11 Ekspor data & impor CSV.
//!
//! - `data_exports`: satu permintaan ekspor workspace (Owner/Admin). `parts` = isi yang dipilih,
//!   `project_ids` kosong = semua project. Worker menyusun ZIP ke storage (`storage_key`), lalu
//!   `ready` dengan `expires_at` (7 hari). `queued`/`running` yang tertahan disapu task
//!   `export_sweep`; `ready` yang lewat `expires_at` menjadi `expired` dan berkasnya dihapus
//!   (task `cleanup`).
//! - `ticket_imports`: wizard impor CSV (unggah → pemetaan → pratinjau → impor). Berkas CSV di
//!   storage (`source_key`) sampai impor selesai/dibatalkan. `mapping` = kunci field Agent Gate
//!   per kolom (urutan sama dengan `headers`). Worker memproses baris mulai `processed`
//!   (dilanjutkan bila terhenti). `skipped_rows` = laporan baris dilewati, dikosongkan setelah
//!   `report_expires_at` (30 hari, task `cleanup`).
//! - Audit (`export.requested`, `export.downloaded`, `import.completed`) dan notifikasi in-app
//!   (`export_ready`, `export_failed`).

use sea_orm_migration::prelude::*;

const AUDIT_ACTIONS_M10: &str =
    "'workspace.created', 'workspace.updated', 'workspace.require_2fa_changed',
        'workspace.deletion_scheduled', 'workspace.deletion_cancelled',
        'member.invited', 'member.joined', 'member.role_changed', 'member.removed',
        'member.left', 'member.job_title_changed', 'member.invitation_revoked',
        'role.created', 'role.updated', 'role.deleted',
        'job_title.created', 'job_title.updated', 'job_title.deleted',
        'team.created', 'team.updated', 'team.deleted', 'team.member_added',
        'team.member_removed',
        'project.created', 'project.updated', 'project.archived',
        'project.access_granted', 'project.access_changed', 'project.access_revoked',
        'repo.added', 'repo.updated', 'repo.removed',
        'ticket.trashed', 'ticket.restored', 'ticket.purged',
        'ticket_type.created', 'ticket_type.updated', 'ticket_type.deleted',
        'custom_field.created', 'custom_field.updated', 'custom_field.deleted',
        'ticket_code.updated',
        'workflow.created', 'workflow.updated', 'workflow.deleted', 'workflow.migrated',
        'project.workflows_changed', 'ticket.stage_jumped', 'ticket.transition_forced',
        'persona.created', 'persona.updated', 'persona.deleted',
        'skill_pack.created', 'skill_pack.updated', 'skill_pack.submitted',
        'skill_pack.approved', 'skill_pack.rejected', 'skill_pack.deleted',
        'mcp.connected', 'mcp.revoked',
        'runner.registered', 'runner.revoked', 'job.cancelled', 'job.permission_decided',
        'git.connected', 'git.updated', 'git.disconnected', 'repo.linked',
        'project.git_updated', 'pull_request.merged',
        'project.token_saver_updated', 'office.look_updated'";
const AUDIT_ACTIONS_NEW: &str = "'export.requested', 'export.downloaded', 'import.completed'";

const AUDIT_SUBJECTS_M10: &str = "'workspace', 'member', 'role', 'job_title', 'team', 'project',
        'project_grant', 'repo', 'invitation', 'ticket', 'ticket_type', 'custom_field',
        'workflow', 'persona', 'skill_pack', 'mcp_client', 'runner', 'job', 'git_connection',
        'pull_request'";
const AUDIT_SUBJECTS_NEW: &str = "'data_export', 'ticket_import'";

const NOTIFICATION_KINDS_M10: &str = "'mentioned', 'assigned', 'commented',
        'review_requested', 'approved', 'rejected', 'decision_needed', 'agent_question',
        'job_queued', 'job_permission', 'job_quota', 'job_failed', 'job_terminal',
        'pr_opened', 'ci_failed', 'pr_merged', 'merge_failed',
        'knowledge_proposal', 'knowledge_job', 'knowledge_rejected'";
const NOTIFICATION_KINDS_NEW: &str = "'export_ready', 'export_failed'";

const EXPORT_PARTS: &str = "'tickets', 'artifacts', 'knowledge', 'attachments', 'config', 'audit'";

#[derive(DeriveMigrationName)]
pub struct Migration;

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check CHECK (action IN (
            {AUDIT_ACTIONS_M10}, {AUDIT_ACTIONS_NEW}))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M10}, {AUDIT_SUBJECTS_NEW}))"
        ),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check CHECK (kind IN (
            {NOTIFICATION_KINDS_M10}, {NOTIFICATION_KINDS_NEW}))"
        ),
        // requested_by/recipient_id SET NULL: riwayat ekspor tetap ada saat user dihapus.
        format!(
            "CREATE TABLE data_exports (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            requested_by uuid REFERENCES users (id) ON DELETE SET NULL,
            recipient_id uuid REFERENCES users (id) ON DELETE SET NULL,
            parts text[] NOT NULL CHECK (cardinality(parts) > 0
                AND parts <@ ARRAY[{EXPORT_PARTS}]::text[]),
            project_ids uuid[] NOT NULL DEFAULT '{{}}',
            status text NOT NULL DEFAULT 'queued'
                CHECK (status IN ('queued', 'running', 'ready', 'failed', 'expired')),
            progress integer NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 100),
            phase text CHECK (phase IS NULL OR phase IN ({EXPORT_PARTS})),
            attempts integer NOT NULL DEFAULT 0,
            size_bytes bigint CHECK (size_bytes IS NULL OR size_bytes >= 0),
            storage_key text,
            error text NOT NULL DEFAULT '',
            started_at timestamptz,
            finished_at timestamptz,
            expires_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        ),
        "CREATE INDEX data_exports_workspace_id_created_at_idx \
         ON data_exports (workspace_id, created_at DESC, id DESC)"
            .into(),
        // Sapuan worker: hanya baris yang belum selesai.
        "CREATE INDEX data_exports_pending_idx ON data_exports (updated_at) \
         WHERE status IN ('queued', 'running')"
            .into(),
        "CREATE INDEX data_exports_ready_expires_at_idx ON data_exports (expires_at) \
         WHERE status = 'ready'"
            .into(),
        "CREATE INDEX data_exports_requested_by_idx ON data_exports (requested_by)".into(),
        "CREATE INDEX data_exports_recipient_id_idx ON data_exports (recipient_id)".into(),
        // project_id SET NULL: riwayat impor tetap ada saat project dihapus.
        "CREATE TABLE ticket_imports (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            project_id uuid,
            filename text NOT NULL CHECK (char_length(filename) BETWEEN 1 AND 255),
            size_bytes bigint NOT NULL CHECK (size_bytes >= 0),
            source_key text,
            delimiter text NOT NULL CHECK (delimiter IN ('comma', 'semicolon')),
            headers jsonb NOT NULL DEFAULT '[]',
            mapping jsonb NOT NULL DEFAULT '[]',
            row_count integer NOT NULL DEFAULT 0 CHECK (row_count >= 0),
            status text NOT NULL DEFAULT 'draft'
                CHECK (status IN ('draft', 'queued', 'running', 'done', 'failed', 'cancelled')),
            processed integer NOT NULL DEFAULT 0,
            imported integer NOT NULL DEFAULT 0,
            warned integer NOT NULL DEFAULT 0,
            skipped integer NOT NULL DEFAULT 0,
            skipped_rows jsonb NOT NULL DEFAULT '[]',
            report_expires_at timestamptz,
            error text NOT NULL DEFAULT '',
            started_at timestamptz,
            finished_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE SET NULL (project_id)
        )"
        .into(),
        "CREATE INDEX ticket_imports_workspace_id_created_at_idx \
         ON ticket_imports (workspace_id, created_at DESC, id DESC)"
            .into(),
        "CREATE INDEX ticket_imports_pending_idx ON ticket_imports (updated_at) \
         WHERE status IN ('queued', 'running')"
            .into(),
        "CREATE INDEX ticket_imports_project_id_idx ON ticket_imports (project_id)".into(),
        "CREATE INDEX ticket_imports_created_by_idx ON ticket_imports (created_by)".into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "DROP TABLE IF EXISTS ticket_imports".into(),
        "DROP TABLE IF EXISTS data_exports".into(),
        format!("DELETE FROM notifications WHERE kind IN ({NOTIFICATION_KINDS_NEW})"),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check \
             CHECK (kind IN ({NOTIFICATION_KINDS_M10}))"
        ),
        format!(
            "DELETE FROM audit_events WHERE action IN ({AUDIT_ACTIONS_NEW}) \
             OR subject_type IN ({AUDIT_SUBJECTS_NEW})"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M10}))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check \
             CHECK (action IN ({AUDIT_ACTIONS_M10}))"
        ),
    ]
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in up() {
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in down() {
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }
}
