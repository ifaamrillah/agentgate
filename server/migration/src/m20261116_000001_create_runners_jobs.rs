//! M5 CLI & runner: kode perangkat (device code flow), runner, pengaturan runner per user, job
//! (stage tiket / `@agent`), log, evidence bertanda tangan, permintaan izin aksi berbahaya, hasil
//! cek drift aturan, dan kind audit/notifikasi baru.
//!
//! - `oauth_device_codes`: kode perangkat hanya disimpan sebagai digest; `user_code` (8 huruf)
//!   unik selama masih `pending`. Disetujui → grant OAuth biasa (client × user × workspace).
//! - `runners`: perangkat milik user di satu workspace, dengan kunci publik Ed25519 untuk
//!   memverifikasi evidence. Dicabut = `revoked_at` (riwayat job tetap menunjuk runner).
//! - `jobs`: satu job terbuka per tiket untuk kind `stage` (job lama dibatalkan saat tiket
//!   masuk stage lain), dijaga indeks unik parsial.
//! - `job_logs`, `job_evidence`: append-only (tanpa `updated_at`).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const AUDIT_ACTIONS_M4: &str =
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
        'mcp.connected', 'mcp.revoked'";
const AUDIT_SUBJECTS_M4: &str = "'workspace', 'member', 'role', 'job_title', 'team', 'project',
        'project_grant', 'repo', 'invitation', 'ticket', 'ticket_type', 'custom_field',
        'workflow', 'persona', 'skill_pack', 'mcp_client'";
const NOTIFICATION_KINDS_M4: &str = "'mentioned', 'assigned', 'commented',
        'review_requested', 'approved', 'rejected', 'decision_needed', 'agent_question'";

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check CHECK (action IN (
            {AUDIT_ACTIONS_M4},
            'runner.registered', 'runner.revoked', 'job.cancelled', 'job.permission_decided'))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M4}, 'runner', 'job'))"
        ),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check CHECK (kind IN (
            {NOTIFICATION_KINDS_M4}, 'job_queued', 'job_permission', 'job_quota', 'job_failed',
            'job_terminal'))"
        ),
        "CREATE TABLE oauth_device_codes (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            client_id uuid NOT NULL REFERENCES oauth_clients (id) ON DELETE CASCADE,
            device_code_digest text NOT NULL UNIQUE,
            user_code text NOT NULL,
            device_name text NOT NULL DEFAULT '',
            status text NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'approved', 'denied', 'used')),
            user_id uuid REFERENCES users (id) ON DELETE CASCADE,
            workspace_id uuid REFERENCES workspaces (id) ON DELETE CASCADE,
            grant_id uuid REFERENCES oauth_grants (id) ON DELETE CASCADE,
            interval_seconds integer NOT NULL DEFAULT 5,
            last_polled_at timestamptz,
            expires_at timestamptz NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        "CREATE UNIQUE INDEX oauth_device_codes_pending_user_code_key \
         ON oauth_device_codes (user_code) WHERE status = 'pending'"
            .into(),
        "CREATE INDEX oauth_device_codes_expires_at_idx ON oauth_device_codes (expires_at)".into(),
        "CREATE TABLE runners (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
            grant_id uuid REFERENCES oauth_grants (id) ON DELETE SET NULL,
            name text NOT NULL,
            os text NOT NULL DEFAULT '',
            client text NOT NULL DEFAULT 'claude',
            version text NOT NULL DEFAULT '',
            public_key text NOT NULL,
            connected_at timestamptz,
            last_seen_at timestamptz,
            revoked_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id)
        )"
        .into(),
        "CREATE INDEX runners_workspace_id_user_id_idx ON runners (workspace_id, user_id)".into(),
        "CREATE TABLE runner_prefs (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
            auto_claim boolean NOT NULL DEFAULT false,
            max_parallel integer CHECK (max_parallel IS NULL OR max_parallel BETWEEN 1 AND 20),
            default_client text NOT NULL DEFAULT 'claude',
            quota_policy text NOT NULL DEFAULT 'wait' CHECK (quota_policy IN ('wait', 'release')),
            terminals jsonb NOT NULL DEFAULT '{}',
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (workspace_id, user_id)
        )"
        .into(),
        "CREATE INDEX runner_prefs_user_id_idx ON runner_prefs (user_id)".into(),
        "CREATE TABLE jobs (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            ticket_id uuid NOT NULL,
            kind text NOT NULL CHECK (kind IN ('stage', 'comment')),
            mode text NOT NULL CHECK (mode IN ('auto', 'interactive')),
            stage_key text NOT NULL,
            persona text NOT NULL,
            status text NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'claimed',
                'running', 'paused', 'quota', 'succeeded', 'failed', 'cancelled')),
            comment_id uuid,
            prompt_note text NOT NULL DEFAULT '',
            requested_by uuid REFERENCES users (id) ON DELETE SET NULL,
            claimed_by uuid REFERENCES users (id) ON DELETE SET NULL,
            runner_id uuid REFERENCES runners (id) ON DELETE SET NULL,
            client text NOT NULL DEFAULT '',
            model text NOT NULL DEFAULT '',
            branch text NOT NULL DEFAULT '',
            steps jsonb NOT NULL DEFAULT '[]',
            summary text NOT NULL DEFAULT '',
            tokens bigint,
            quota_reset_at timestamptz,
            claimed_at timestamptz,
            started_at timestamptz,
            finished_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (comment_id, workspace_id) REFERENCES ticket_comments (id, workspace_id)
                ON DELETE SET NULL (comment_id)
        )"
        .into(),
        "CREATE UNIQUE INDEX jobs_one_open_stage_job_per_ticket \
         ON jobs (ticket_id) WHERE kind = 'stage' \
         AND status IN ('queued', 'claimed', 'running', 'paused', 'quota')"
            .into(),
        "CREATE INDEX jobs_workspace_id_status_idx ON jobs (workspace_id, status, created_at)".into(),
        "CREATE INDEX jobs_ticket_id_idx ON jobs (ticket_id, created_at)".into(),
        "CREATE INDEX jobs_runner_id_idx ON jobs (runner_id)".into(),
        "CREATE INDEX jobs_claimed_by_idx ON jobs (claimed_by)".into(),
        "CREATE TABLE job_logs (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            job_id uuid NOT NULL,
            stream text NOT NULL CHECK (stream IN ('info', 'out', 'err', 'tool', 'ok', 'warn',
                'fail')),
            body text NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (job_id, workspace_id) REFERENCES jobs (id, workspace_id) ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX job_logs_job_id_idx ON job_logs (job_id, id)".into(),
        "CREATE INDEX job_logs_workspace_id_idx ON job_logs (workspace_id)".into(),
        "CREATE TABLE job_evidence (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            job_id uuid NOT NULL,
            ticket_id uuid NOT NULL,
            stage_key text NOT NULL,
            runner_id uuid REFERENCES runners (id) ON DELETE SET NULL,
            kind text NOT NULL CHECK (kind IN ('test_report', 'screenshot', 'log')),
            name text NOT NULL,
            content_type text NOT NULL,
            size_bytes integer NOT NULL,
            sha256 text NOT NULL,
            storage_key text NOT NULL,
            passed boolean,
            total integer,
            failed integer,
            note text NOT NULL DEFAULT '',
            signature text NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (job_id, workspace_id) REFERENCES jobs (id, workspace_id) ON DELETE CASCADE,
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX job_evidence_ticket_id_idx ON job_evidence (ticket_id, stage_key, created_at)"
            .into(),
        "CREATE INDEX job_evidence_job_id_idx ON job_evidence (job_id)".into(),
        "CREATE INDEX job_evidence_workspace_id_idx ON job_evidence (workspace_id)".into(),
        "CREATE TABLE job_permissions (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            job_id uuid NOT NULL,
            ticket_id uuid NOT NULL,
            action text NOT NULL,
            command text NOT NULL,
            reason text NOT NULL DEFAULT '',
            status text NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'allowed', 'allowed_ticket', 'denied')),
            decided_by uuid REFERENCES users (id) ON DELETE SET NULL,
            decided_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (job_id, workspace_id) REFERENCES jobs (id, workspace_id) ON DELETE CASCADE,
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX job_permissions_job_id_idx ON job_permissions (job_id)".into(),
        "CREATE INDEX job_permissions_ticket_id_idx ON job_permissions (ticket_id, action)".into(),
        "CREATE INDEX job_permissions_workspace_id_idx ON job_permissions (workspace_id, status)"
            .into(),
        "CREATE TABLE rule_checks (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            repo text NOT NULL,
            checksum text NOT NULL,
            changed text[] NOT NULL DEFAULT '{}',
            checked_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (workspace_id, repo)
        )"
        .into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "DROP TABLE IF EXISTS rule_checks".into(),
        "DROP TABLE IF EXISTS job_permissions".into(),
        "DROP TABLE IF EXISTS job_evidence".into(),
        "DROP TABLE IF EXISTS job_logs".into(),
        "DROP TABLE IF EXISTS jobs".into(),
        "DROP TABLE IF EXISTS runner_prefs".into(),
        "DROP TABLE IF EXISTS runners".into(),
        "DROP TABLE IF EXISTS oauth_device_codes".into(),
        format!("DELETE FROM notifications WHERE kind NOT IN ({NOTIFICATION_KINDS_M4})"),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check \
             CHECK (kind IN ({NOTIFICATION_KINDS_M4}))"
        ),
        format!(
            "DELETE FROM audit_events WHERE action NOT IN ({AUDIT_ACTIONS_M4}) \
             OR subject_type NOT IN ({AUDIT_SUBJECTS_M4})"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M4}))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check \
             CHECK (action IN ({AUDIT_ACTIONS_M4}))"
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
