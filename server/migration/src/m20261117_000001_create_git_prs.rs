//! M6 Git & PR: koneksi provider git per workspace (token terenkripsi), tautan repo ke koneksi
//! (+ webhook), pull request yang dilacak dari webhook provider, dan konvensi git per project.
//!
//! - `git_connections`: satu koneksi per host per workspace. Token dan secret webhook disimpan
//!   terenkripsi (XChaCha20-Poly1305, `ENCRYPTION_KEY`), tidak pernah dalam bentuk mentah.
//! - `repos.connection_id`: koneksi dihapus → repo kembali tidak tertaut (`SET NULL`).
//! - `pull_requests`: unik per repo × nomor; tiket boleh kosong (PR tanpa kode tiket).
//! - `projects.git`: `agentgate_core::git::GitSettings` (jsonb, `{}` = bawaan).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const AUDIT_ACTIONS_M5: &str =
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
        'runner.registered', 'runner.revoked', 'job.cancelled', 'job.permission_decided'";
const AUDIT_SUBJECTS_M5: &str = "'workspace', 'member', 'role', 'job_title', 'team', 'project',
        'project_grant', 'repo', 'invitation', 'ticket', 'ticket_type', 'custom_field',
        'workflow', 'persona', 'skill_pack', 'mcp_client', 'runner', 'job'";
const NOTIFICATION_KINDS_M5: &str = "'mentioned', 'assigned', 'commented',
        'review_requested', 'approved', 'rejected', 'decision_needed', 'agent_question', 'job_queued', 'job_permission', 'job_quota', 'job_failed',
        'job_terminal'";

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check CHECK (action IN (
            {AUDIT_ACTIONS_M5},
            'git.connected', 'git.updated', 'git.disconnected', 'repo.linked',
            'project.git_updated', 'pull_request.merged'))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M5}, 'git_connection', 'pull_request'))"
        ),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check CHECK (kind IN (
            {NOTIFICATION_KINDS_M5}, 'pr_opened', 'ci_failed', 'pr_merged', 'merge_failed'))"
        ),
        "CREATE TABLE git_connections (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            provider text NOT NULL CHECK (provider IN ('github', 'github_enterprise')),
            host text NOT NULL,
            api_base text NOT NULL,
            web_base text NOT NULL,
            account text NOT NULL DEFAULT '',
            token_encrypted bytea NOT NULL,
            webhook_secret_encrypted bytea NOT NULL,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            last_event_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (workspace_id, host)
        )"
        .into(),
        "ALTER TABLE repos
            ADD COLUMN connection_id uuid REFERENCES git_connections (id) ON DELETE SET NULL,
            ADD COLUMN full_name text NOT NULL DEFAULT '',
            ADD COLUMN webhook_id bigint,
            ADD COLUMN webhook_error text NOT NULL DEFAULT '',
            ADD COLUMN last_event_at timestamptz"
            .into(),
        "CREATE INDEX repos_connection_id_idx ON repos (connection_id, lower(full_name))".into(),
        "ALTER TABLE projects ADD COLUMN git jsonb NOT NULL DEFAULT '{}'".into(),
        "CREATE TABLE pull_requests (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            repo_id uuid NOT NULL REFERENCES repos (id) ON DELETE CASCADE,
            ticket_id uuid,
            number bigint NOT NULL,
            title text NOT NULL,
            url text NOT NULL,
            head_branch text NOT NULL,
            base_branch text NOT NULL,
            head_sha text NOT NULL DEFAULT '',
            state text NOT NULL CHECK (state IN ('open', 'draft', 'closed', 'merged')),
            ci_state text NOT NULL DEFAULT 'none'
                CHECK (ci_state IN ('none', 'pending', 'success', 'failure')),
            checks jsonb NOT NULL DEFAULT '[]',
            additions integer NOT NULL DEFAULT 0,
            deletions integer NOT NULL DEFAULT 0,
            author text NOT NULL DEFAULT '',
            job_id uuid REFERENCES jobs (id) ON DELETE SET NULL,
            merged_at timestamptz,
            closed_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (repo_id, number),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE SET NULL (ticket_id)
        )"
        .into(),
        "CREATE INDEX pull_requests_ticket_id_idx ON pull_requests (ticket_id)".into(),
        "CREATE INDEX pull_requests_project_id_idx ON pull_requests (project_id, updated_at)"
            .into(),
        "CREATE INDEX pull_requests_head_sha_idx ON pull_requests (repo_id, head_sha)".into(),
        "CREATE INDEX pull_requests_workspace_id_idx ON pull_requests (workspace_id)".into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "DROP TABLE IF EXISTS pull_requests".into(),
        "ALTER TABLE projects DROP COLUMN IF EXISTS git".into(),
        "DROP INDEX IF EXISTS repos_connection_id_idx".into(),
        "ALTER TABLE repos DROP COLUMN IF EXISTS connection_id, DROP COLUMN IF EXISTS full_name,
            DROP COLUMN IF EXISTS webhook_id, DROP COLUMN IF EXISTS webhook_error,
            DROP COLUMN IF EXISTS last_event_at"
            .into(),
        "DROP TABLE IF EXISTS git_connections".into(),
        format!("DELETE FROM notifications WHERE kind NOT IN ({NOTIFICATION_KINDS_M5})"),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check \
             CHECK (kind IN ({NOTIFICATION_KINDS_M5}))"
        ),
        format!(
            "DELETE FROM audit_events WHERE action NOT IN ({AUDIT_ACTIONS_M5}) \
             OR subject_type NOT IN ({AUDIT_SUBJECTS_M5})"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M5}))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check \
             CHECK (action IN ({AUDIT_ACTIONS_M5}))"
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
