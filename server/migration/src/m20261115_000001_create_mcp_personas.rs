//! M4 MCP & persona: persona per workspace, skill pack + file + pemasangan ke persona, OAuth 2.1
//! untuk MCP (client, grant, kode, token), draft tiket hasil brainstorming, dan kind
//! audit/notifikasi baru.
//!
//! - `personas`: kunci slug unik per workspace (termasuk yang sudah dihapus, supaya kunci di
//!   definisi workflow lama tidak pernah menunjuk persona lain). Hapus = `deleted_at` +
//!   `replaced_by` (pengganti di stage yang memakai persona itu).
//! - Backfill: setiap workspace yang sudah ada mendapat 9 persona bawaan dari
//!   `agentgate_core::persona::BUILTINS` (sumber yang sama dengan seed workspace baru).
//! - OAuth: token/kode hanya disimpan sebagai digest SHA-256 (`*_digest`). `oauth_clients` tidak
//!   milik workspace (didaftarkan sebelum login, RFC 7591); `oauth_grants` = satu client × user ×
//!   workspace yang disetujui, induk kode dan token (cabut grant = hapus token).

use agentgate_core::persona::{BUILTINS, CLIENT_TOOLS};
use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DbBackend, Statement, Value},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const AUDIT_ACTIONS_M3: &str =
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
        'project.workflows_changed', 'ticket.stage_jumped', 'ticket.transition_forced'";
const AUDIT_SUBJECTS_M3: &str = "'workspace', 'member', 'role', 'job_title', 'team', 'project',
        'project_grant', 'repo', 'invitation', 'ticket', 'ticket_type', 'custom_field',
        'workflow'";
const NOTIFICATION_KINDS_M3: &str = "'mentioned', 'assigned', 'commented',
        'review_requested', 'approved', 'rejected', 'decision_needed'";

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check CHECK (action IN (
            {AUDIT_ACTIONS_M3},
            'persona.created', 'persona.updated', 'persona.deleted',
            'skill_pack.created', 'skill_pack.updated', 'skill_pack.submitted',
            'skill_pack.approved', 'skill_pack.rejected', 'skill_pack.deleted',
            'mcp.connected', 'mcp.revoked'))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M3}, 'persona', 'skill_pack', 'mcp_client'))"
        ),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check CHECK (kind IN (
            {NOTIFICATION_KINDS_M3}, 'agent_question'))"
        ),
        // --- persona ---
        // danger: {aksi: allow|ask|block}; models: {client: {model, effort}} (bentuk bebas,
        // divalidasi model). mcp_tools/client_tools tanpa CHECK: daftar tool tumbuh per
        // milestone dan divalidasi di model.
        "CREATE TABLE personas (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            key text NOT NULL,
            name text NOT NULL,
            summary text NOT NULL DEFAULT '',
            color text NOT NULL,
            icon text NOT NULL DEFAULT 'robot',
            builtin boolean NOT NULL DEFAULT false,
            instructions text NOT NULL DEFAULT '',
            concise boolean NOT NULL DEFAULT false,
            mcp_tools text[] NOT NULL DEFAULT '{}',
            client_tools text[] NOT NULL DEFAULT '{}',
            danger jsonb NOT NULL DEFAULT '{}',
            models jsonb NOT NULL DEFAULT '{}',
            revision integer NOT NULL DEFAULT 1,
            deleted_at timestamptz,
            replaced_by uuid,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            UNIQUE (workspace_id, key),
            FOREIGN KEY (replaced_by, workspace_id) REFERENCES personas (id, workspace_id),
            CHECK (deleted_at IS NULL OR replaced_by IS NOT NULL)
        )"
        .into(),
        "CREATE UNIQUE INDEX personas_workspace_id_name_lower_key \
         ON personas (workspace_id, lower(name)) WHERE deleted_at IS NULL"
            .into(),
        // --- skill pack ---
        "CREATE TABLE skill_packs (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            key text NOT NULL,
            name text NOT NULL,
            description text NOT NULL DEFAULT '',
            source text NOT NULL CHECK (source IN ('custom', 'repo')),
            repo_url text,
            repo_ref text,
            repo_path text,
            commit_sha text,
            version text NOT NULL DEFAULT '0.1.0',
            requested_tools text[] NOT NULL DEFAULT '{}',
            status text NOT NULL DEFAULT 'draft'
                CHECK (status IN ('draft', 'pending', 'approved', 'rejected')),
            risk text CHECK (risk IN ('safe', 'attention', 'danger')),
            report jsonb NOT NULL DEFAULT '[]',
            review_note text NOT NULL DEFAULT '',
            submitted_at timestamptz,
            reviewed_by uuid REFERENCES users (id) ON DELETE SET NULL,
            reviewed_at timestamptz,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            UNIQUE (workspace_id, key)
        )"
        .into(),
        "CREATE TABLE skill_files (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            pack_id uuid NOT NULL,
            path text NOT NULL,
            body text NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (pack_id, path),
            FOREIGN KEY (pack_id, workspace_id) REFERENCES skill_packs (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX skill_files_workspace_id_idx ON skill_files (workspace_id)".into(),
        "CREATE TABLE persona_skills (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            persona_id uuid NOT NULL,
            pack_id uuid NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (persona_id, pack_id),
            FOREIGN KEY (persona_id, workspace_id) REFERENCES personas (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (pack_id, workspace_id) REFERENCES skill_packs (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX persona_skills_pack_id_idx ON persona_skills (pack_id)".into(),
        "CREATE INDEX persona_skills_workspace_id_idx ON persona_skills (workspace_id)".into(),
        // --- OAuth 2.1 ---
        "CREATE TABLE oauth_clients (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            client_id text NOT NULL UNIQUE,
            name text NOT NULL,
            redirect_uris text[] NOT NULL,
            client_uri text,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        "CREATE TABLE oauth_grants (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
            client_id uuid NOT NULL REFERENCES oauth_clients (id) ON DELETE CASCADE,
            scope text NOT NULL DEFAULT 'mcp',
            last_used_at timestamptz,
            revoked_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        "CREATE UNIQUE INDEX oauth_grants_active_key ON oauth_grants (client_id, user_id, workspace_id) \
         WHERE revoked_at IS NULL"
            .into(),
        "CREATE INDEX oauth_grants_user_id_idx ON oauth_grants (user_id)".into(),
        "CREATE INDEX oauth_grants_workspace_id_idx ON oauth_grants (workspace_id)".into(),
        "CREATE TABLE oauth_codes (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            grant_id uuid NOT NULL REFERENCES oauth_grants (id) ON DELETE CASCADE,
            code_digest text NOT NULL UNIQUE,
            redirect_uri text NOT NULL,
            code_challenge text NOT NULL,
            resource text NOT NULL,
            expires_at timestamptz NOT NULL,
            used_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        "CREATE INDEX oauth_codes_grant_id_idx ON oauth_codes (grant_id)".into(),
        "CREATE TABLE oauth_tokens (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            grant_id uuid NOT NULL REFERENCES oauth_grants (id) ON DELETE CASCADE,
            kind text NOT NULL CHECK (kind IN ('access', 'refresh')),
            token_digest text NOT NULL UNIQUE,
            expires_at timestamptz NOT NULL,
            used_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        "CREATE INDEX oauth_tokens_grant_id_idx ON oauth_tokens (grant_id)".into(),
        // --- draft tiket brainstorming ---
        // parent_id = draft induk (hapus induk menghapus turunannya). type_key = kunci tipe
        // tiket (di-resolve saat tiket dibuat).
        "CREATE TABLE ticket_drafts (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            source_ticket_id uuid,
            parent_id uuid,
            type_key text NOT NULL,
            title text NOT NULL,
            description text NOT NULL DEFAULT '',
            story_points integer,
            position integer NOT NULL DEFAULT 0,
            approved boolean NOT NULL DEFAULT true,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (source_ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (parent_id, workspace_id) REFERENCES ticket_drafts (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX ticket_drafts_project_id_idx ON ticket_drafts (project_id, position)".into(),
        "CREATE INDEX ticket_drafts_source_ticket_id_idx ON ticket_drafts (source_ticket_id)"
            .into(),
        "CREATE INDEX ticket_drafts_parent_id_idx ON ticket_drafts (parent_id)".into(),
        "CREATE INDEX ticket_drafts_workspace_id_idx ON ticket_drafts (workspace_id)".into(),
    ]
}

/// Daftar kunci dipisah koma untuk `string_to_array` (kunci bawaan tidak berisi koma).
fn joined(items: &[&str]) -> Value {
    Value::from(items.join(","))
}

/// 9 persona bawaan untuk setiap workspace yang sudah ada.
fn backfill() -> Vec<Statement> {
    BUILTINS
        .iter()
        .map(|b| {
            // Kunci dan nilai ASCII bawaan: JSON ditulis langsung tanpa escape.
            let danger = format!(
                "{{{}}}",
                b.danger_levels()
                    .into_iter()
                    .map(|(a, l)| format!("\"{a}\":\"{}\"", l.as_str()))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let client_tools: Vec<&str> = b
                .client_tools
                .iter()
                .copied()
                .filter(|t| CLIENT_TOOLS.contains(t))
                .collect();
            Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO personas (workspace_id, key, name, summary, color, icon, builtin,
                    instructions, mcp_tools, client_tools, danger)
                 SELECT id, $1, $2, $3, $4, $5, true, $6, string_to_array($7, ','),
                    string_to_array($8, ','), $9::jsonb FROM workspaces",
                [
                    Value::from(b.key),
                    Value::from(b.name),
                    Value::from(b.summary),
                    Value::from(b.color),
                    Value::from(b.icon),
                    Value::from(b.instructions),
                    joined(b.mcp_tools),
                    joined(&client_tools),
                    Value::from(danger),
                ],
            )
        })
        .collect()
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "DROP TABLE IF EXISTS ticket_drafts".into(),
        "DROP TABLE IF EXISTS oauth_tokens".into(),
        "DROP TABLE IF EXISTS oauth_codes".into(),
        "DROP TABLE IF EXISTS oauth_grants".into(),
        "DROP TABLE IF EXISTS oauth_clients".into(),
        "DROP TABLE IF EXISTS persona_skills".into(),
        "DROP TABLE IF EXISTS skill_files".into(),
        "DROP TABLE IF EXISTS skill_packs".into(),
        "DROP TABLE IF EXISTS personas".into(),
        "DELETE FROM notifications WHERE kind = 'agent_question'".into(),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check \
             CHECK (kind IN ({NOTIFICATION_KINDS_M3}))"
        ),
        format!(
            "DELETE FROM audit_events WHERE action NOT IN ({AUDIT_ACTIONS_M3}) \
             OR subject_type NOT IN ({AUDIT_SUBJECTS_M3})"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check \
             CHECK (subject_type IN ({AUDIT_SUBJECTS_M3}))"
        ),
        "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check".into(),
        format!(
            "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check \
             CHECK (action IN ({AUDIT_ACTIONS_M3}))"
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
        for statement in backfill() {
            db.execute_raw(statement).await?;
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
