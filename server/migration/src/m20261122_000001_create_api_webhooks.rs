//! M11 API publik + webhook keluar.
//!
//! - `api_tokens`: token API pribadi (`ag_pat_…`) milik satu anggota workspace. Hanya digest
//!   SHA-256 yang disimpan; `prefix` (12 karakter pertama) untuk dikenali di daftar. `scopes`
//!   membatasi apa yang boleh dilakukan token; `project_id` kosong = semua project yang bisa
//!   diakses pembuatnya. Anggota keluar → token ikut terhapus (FK ke `workspace_members`).
//! - `api_rate_limits`: penghitung jendela per menit per token (rate limit di Postgres, tanpa
//!   Redis, seperti `login_throttles`).
//! - `webhooks`: tujuan webhook keluar per workspace. Secret HMAC terenkripsi
//!   (XChaCha20-Poly1305, `ENCRYPTION_KEY`). `project_ids` kosong = semua project.
//!   `disabled_at` terisi = nonaktif (manual, atau otomatis setelah 5 gagal beruntun:
//!   `consecutive_failures >= 5`).
//! - `outbox_events`: event domain yang ditulis di transaksi yang sama dengan perubahannya
//!   (pola outbox). Dibaca pengirim webhook dan, nanti, connector Jira (mirror).
//! - `webhook_deliveries`: satu baris per event × webhook; `status = 'pending'` dengan
//!   `next_attempt_at` adalah antrean kirim yang disapu worker/scheduler.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

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
const AUDIT_ACTIONS_NEW: &str = "'api_token.created', 'api_token.revoked',
        'webhook.created', 'webhook.updated', 'webhook.deleted', 'webhook.secret_rotated'";

const AUDIT_SUBJECTS_M6: &str = "'workspace', 'member', 'role', 'job_title', 'team', 'project',
        'project_grant', 'repo', 'invitation', 'ticket', 'ticket_type', 'custom_field',
        'workflow', 'persona', 'skill_pack', 'mcp_client', 'runner', 'job',
        'git_connection', 'pull_request'";

const NOTIFICATION_KINDS_M8: &str = "'mentioned', 'assigned', 'commented',
        'review_requested', 'approved', 'rejected', 'decision_needed', 'agent_question',
        'job_queued', 'job_permission', 'job_quota', 'job_failed', 'job_terminal',
        'pr_opened', 'ci_failed', 'pr_merged', 'merge_failed',
        'knowledge_proposal', 'knowledge_job', 'knowledge_rejected'";

/// Cakupan token API (`models::api_tokens::ApiScope`).
const SCOPES: &str = "'tiket:baca', 'tiket:tulis', 'komentar:tulis', 'laporan:baca', 'knowledge:baca', 'webhook:kelola'";

/// Event webhook (`models::outbox_events::OutboxEvent`).
const EVENTS: &str = "'ticket.created', 'ticket.updated', 'ticket.stage_changed',
        'ticket.deleted', 'comment.created', 'sprint.started', 'sprint.completed',
        'release.published', 'gate.rejected', 'job.failed'";

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check \
             CHECK (action IN ({AUDIT_ACTIONS_M10}, {AUDIT_ACTIONS_NEW}))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M6}, 'api_token', 'webhook'))"
        ),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check \
             CHECK (kind IN ({NOTIFICATION_KINDS_M8}, 'webhook_disabled'))"
        ),
        // Token ikut terhapus saat pembuatnya keluar dari workspace (FK keanggotaan) atau
        // project-nya dihapus: token yang tidak lagi bisa dipakai tidak perlu disimpan.
        format!(
            "CREATE TABLE api_tokens (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            user_id uuid NOT NULL,
            project_id uuid,
            name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
            prefix text NOT NULL,
            digest text NOT NULL UNIQUE,
            scopes text[] NOT NULL
                CHECK (cardinality(scopes) > 0 AND scopes <@ ARRAY[{SCOPES}]::text[]),
            expires_at timestamptz,
            last_used_at timestamptz,
            revoked_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id)
                ON DELETE CASCADE,
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE
        )"
        ),
        "CREATE INDEX api_tokens_workspace_id_idx ON api_tokens (workspace_id, created_at DESC)"
            .into(),
        "CREATE INDEX api_tokens_project_id_idx ON api_tokens (project_id)".into(),
        "CREATE TABLE api_rate_limits (
            token_id uuid PRIMARY KEY REFERENCES api_tokens (id) ON DELETE CASCADE,
            window_started_at timestamptz NOT NULL,
            requests integer NOT NULL DEFAULT 0,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        format!(
            "CREATE TABLE webhooks (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            url text NOT NULL CHECK (char_length(url) BETWEEN 1 AND 2000),
            secret_encrypted bytea NOT NULL,
            events text[] NOT NULL
                CHECK (cardinality(events) > 0 AND events <@ ARRAY[{EVENTS}]::text[]),
            project_ids uuid[] NOT NULL DEFAULT '{{}}',
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            disabled_at timestamptz,
            consecutive_failures integer NOT NULL DEFAULT 0,
            last_delivery_at timestamptz,
            last_status integer,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        ),
        "CREATE INDEX webhooks_workspace_id_idx ON webhooks (workspace_id, created_at)".into(),
        format!(
            "CREATE TABLE outbox_events (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid REFERENCES projects (id) ON DELETE CASCADE,
            event text NOT NULL CHECK (event IN ({EVENTS})),
            subject_id uuid NOT NULL,
            payload jsonb NOT NULL DEFAULT '{{}}',
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        ),
        "CREATE INDEX outbox_events_workspace_id_idx ON outbox_events (workspace_id, created_at, id)"
            .into(),
        "CREATE INDEX outbox_events_project_id_idx ON outbox_events (project_id)".into(),
        format!(
            "CREATE TABLE webhook_deliveries (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            webhook_id uuid NOT NULL REFERENCES webhooks (id) ON DELETE CASCADE,
            event_id uuid REFERENCES outbox_events (id) ON DELETE SET NULL,
            event text NOT NULL CHECK (event IN ({EVENTS}, 'ping')),
            body text NOT NULL,
            status text NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'succeeded', 'failed')),
            attempts integer NOT NULL DEFAULT 0,
            next_attempt_at timestamptz,
            response_status integer,
            response_body text NOT NULL DEFAULT '',
            error text NOT NULL DEFAULT '',
            duration_ms integer,
            delivered_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        ),
        // Antrean kirim: hanya baris pending (partial index, predikat timpang).
        "CREATE INDEX webhook_deliveries_due_idx ON webhook_deliveries (next_attempt_at)
            WHERE status = 'pending'"
            .into(),
        "CREATE INDEX webhook_deliveries_workspace_id_idx
            ON webhook_deliveries (workspace_id, created_at DESC, id DESC)"
            .into(),
        "CREATE INDEX webhook_deliveries_webhook_id_idx
            ON webhook_deliveries (webhook_id, created_at DESC)"
            .into(),
        "CREATE INDEX webhook_deliveries_event_id_idx ON webhook_deliveries (event_id)".into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "DROP TABLE IF EXISTS webhook_deliveries".into(),
        "DROP TABLE IF EXISTS outbox_events".into(),
        "DROP TABLE IF EXISTS webhooks".into(),
        "DROP TABLE IF EXISTS api_rate_limits".into(),
        "DROP TABLE IF EXISTS api_tokens".into(),
        "DELETE FROM notifications WHERE kind = 'webhook_disabled'".into(),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check \
             CHECK (kind IN ({NOTIFICATION_KINDS_M8}))"
        ),
        format!(
            "DELETE FROM audit_events WHERE action IN ({AUDIT_ACTIONS_NEW}) \
             OR subject_type IN ('api_token', 'webhook')"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M6}))"
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
