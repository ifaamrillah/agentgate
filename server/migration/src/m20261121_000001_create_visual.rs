//! M10 Visual: pemakaian token rinci per job (dari laporan client lewat runner), file yang diubah
//! job (sorot tiket di graph viewer), catatan penghematan token per kejadian, pengaturan penghemat
//! token per project, dan tampilan karakter AI Office per workspace.
//!
//! - `jobs.tokens_*` / `cost_usd_micros`: rincian `FinishReport.usage`; `jobs.tokens` tetap total.
//! - `jobs.changed_files`: `[{"repo": "kasir-web", "path": "src/a.ts"}]` (git diff base…HEAD).
//! - `token_savings`: satu baris per kejadian hemat yang terukur server (mis. `ag_snippet` mengirim
//!   potongan, bukan file utuh). `job_id` kosong untuk sesi MCP interaktif.
//! - `projects.token_saver`: toggle fitur opsional (`concise`, `persona_models`); kunci yang tidak ada
//!   = aktif.
//! - `office_looks`: tampilan karakter (persona per `key`, manusia per user id) yang berlaku untuk
//!   semua orang di workspace.

use sea_orm_migration::prelude::*;

const AUDIT_ACTIONS_M9: &str =
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
        'project.git_updated', 'pull_request.merged'";

#[derive(DeriveMigrationName)]
pub struct Migration;

fn up() -> Vec<String> {
    let mut v: Vec<String> = vec![
        "SET lock_timeout = '5s'".into(),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check CHECK (action IN (
            {AUDIT_ACTIONS_M9}, 'project.token_saver_updated', 'office.look_updated'))"
        ),
    ];
    v.extend(
        [
            "ALTER TABLE jobs
            ADD COLUMN tokens_input bigint CHECK (tokens_input IS NULL OR tokens_input >= 0),
            ADD COLUMN tokens_output bigint CHECK (tokens_output IS NULL OR tokens_output >= 0),
            ADD COLUMN tokens_cache_read bigint
                CHECK (tokens_cache_read IS NULL OR tokens_cache_read >= 0),
            ADD COLUMN tokens_cache_write bigint
                CHECK (tokens_cache_write IS NULL OR tokens_cache_write >= 0),
            ADD COLUMN cost_usd_micros bigint
                CHECK (cost_usd_micros IS NULL OR cost_usd_micros >= 0),
            ADD COLUMN changed_files jsonb NOT NULL DEFAULT '[]'",
            "CREATE INDEX jobs_project_id_finished_at_idx ON jobs (project_id, finished_at)",
            "CREATE TABLE token_savings (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            job_id uuid REFERENCES jobs (id) ON DELETE SET NULL,
            ticket_id uuid,
            feature text NOT NULL CHECK (feature IN ('code_graph', 'layered', 'mcp_rules',
                'handoff')),
            tool text NOT NULL DEFAULT '',
            tokens bigint NOT NULL CHECK (tokens >= 0),
            created_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE SET NULL (ticket_id)
        )",
            "CREATE INDEX token_savings_project_id_idx ON token_savings (project_id, created_at)",
            "CREATE INDEX token_savings_workspace_id_idx ON token_savings (workspace_id)",
            "CREATE INDEX token_savings_job_id_idx ON token_savings (job_id)",
            "ALTER TABLE projects ADD COLUMN token_saver jsonb NOT NULL DEFAULT '{}'",
            "CREATE TABLE office_looks (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            subject text NOT NULL CHECK (subject IN ('persona', 'user')),
            subject_key text NOT NULL CHECK (char_length(subject_key) BETWEEN 1 AND 64),
            look jsonb NOT NULL DEFAULT '{}',
            updated_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (workspace_id, subject, subject_key)
        )",
        ]
        .map(String::from),
    );
    v
}

fn down() -> Vec<String> {
    let mut v: Vec<String> = vec![
        "SET lock_timeout = '5s'".into(),
        "DELETE FROM audit_events WHERE action IN ('project.token_saver_updated', \
         'office.look_updated')"
            .into(),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check \
             CHECK (action IN ({AUDIT_ACTIONS_M9}))"
        ),
    ];
    v.extend(
        [
            "DROP TABLE IF EXISTS office_looks",
            "ALTER TABLE projects DROP COLUMN IF EXISTS token_saver",
            "DROP TABLE IF EXISTS token_savings",
            "DROP INDEX IF EXISTS jobs_project_id_finished_at_idx",
            "ALTER TABLE jobs
            DROP COLUMN IF EXISTS tokens_input,
            DROP COLUMN IF EXISTS tokens_output,
            DROP COLUMN IF EXISTS tokens_cache_read,
            DROP COLUMN IF EXISTS tokens_cache_write,
            DROP COLUMN IF EXISTS cost_usd_micros,
            DROP COLUMN IF EXISTS changed_files",
        ]
        .map(String::from),
    );
    v
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
