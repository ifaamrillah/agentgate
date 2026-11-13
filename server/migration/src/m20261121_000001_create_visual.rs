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

#[derive(DeriveMigrationName)]
pub struct Migration;

fn up() -> Vec<&'static str> {
    vec![
        "SET lock_timeout = '5s'",
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
}

fn down() -> Vec<&'static str> {
    vec![
        "SET lock_timeout = '5s'",
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
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in up() {
            db.execute_unprepared(sql).await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in down() {
            db.execute_unprepared(sql).await?;
        }
        Ok(())
    }
}
