//! M8 Knowledge base: dokumen knowledge project dan workspace beserta versinya, usulan perubahan
//! dari AI (harus disetujui pemilik knowledge), sumber dokumen di repo (deteksi basi), run
//! scan/update (job knowledge tanpa tiket di runner), jenis dokumen custom, pengaturan update
//! otomatis, dan backup aturan lama dari `agentgate init`.
//!
//! - `knowledge_docs.project_id` NULL = dokumen workspace (`applies_to` project, `personas`).
//! - `knowledge_docs.version` = nomor versi terakhir (`knowledge_versions.number`).
//! - `knowledge_proposals.doc_id` NULL = usulan dokumen baru (slug/judul/kelompok dari usulan).
//! - `jobs.ticket_id` boleh NULL hanya untuk `kind = 'knowledge'` (job tanpa tiket, spec Runner).
//! - `notifications.project_id`: notifikasi knowledge menaut ke halaman knowledge project.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const NOTIFICATION_KINDS_M7: &str = "'mentioned', 'assigned', 'commented',
        'review_requested', 'approved', 'rejected', 'decision_needed', 'agent_question',
        'job_queued', 'job_permission', 'job_quota', 'job_failed', 'job_terminal',
        'pr_opened', 'ci_failed', 'pr_merged', 'merge_failed'";

const GROUPS: &str = "'gambaran-umum', 'arsitektur', 'domain-bisnis', 'backend-kode', 'frontend',
    'mobile', 'kualitas', 'operasional', 'data', 'riwayat'";

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        format!(
            "CREATE TABLE knowledge_types (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            key text NOT NULL CHECK (key ~ '^[a-z0-9][a-z0-9-]{{0,59}}$'),
            name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
            group_key text NOT NULL CHECK (group_key IN ({GROUPS})),
            template text NOT NULL DEFAULT '',
            instructions text NOT NULL DEFAULT '',
            projects uuid[] NOT NULL DEFAULT '{{}}',
            ai_scan boolean NOT NULL DEFAULT true,
            on_done boolean NOT NULL DEFAULT true,
            archived_at timestamptz,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (workspace_id, key)
        )"
        ),
        format!(
            "CREATE TABLE knowledge_docs (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid,
            slug text NOT NULL CHECK (slug ~ '^[a-z0-9][a-z0-9-]{{0,79}}$'),
            title text NOT NULL CHECK (char_length(title) BETWEEN 1 AND 160),
            group_key text NOT NULL DEFAULT '' CHECK (group_key IN ('', {GROUPS})),
            type_key text NOT NULL DEFAULT '',
            owner_id uuid REFERENCES users (id) ON DELETE SET NULL,
            body text NOT NULL DEFAULT '',
            version integer NOT NULL DEFAULT 1,
            stale_reason text NOT NULL DEFAULT '',
            stale_at timestamptz,
            applies_to uuid[] NOT NULL DEFAULT '{{}}',
            personas text[] NOT NULL DEFAULT '{{}}',
            archived_at timestamptz,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            updated_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE,
            CHECK (project_id IS NULL OR group_key <> '')
        )"
        ),
        "CREATE UNIQUE INDEX knowledge_docs_project_slug_key ON knowledge_docs (project_id, slug)
            WHERE project_id IS NOT NULL"
            .into(),
        "CREATE UNIQUE INDEX knowledge_docs_workspace_slug_key ON knowledge_docs (workspace_id, slug)
            WHERE project_id IS NULL"
            .into(),
        "CREATE INDEX knowledge_docs_workspace_id_idx ON knowledge_docs (workspace_id)".into(),
        "CREATE INDEX knowledge_docs_search_idx ON knowledge_docs
            USING gin (to_tsvector('simple', title || ' ' || body))"
            .into(),
        "CREATE TABLE knowledge_versions (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            doc_id uuid NOT NULL,
            number integer NOT NULL CHECK (number > 0),
            title text NOT NULL,
            body text NOT NULL,
            message text NOT NULL DEFAULT '',
            source text NOT NULL CHECK (source IN ('create', 'edit', 'proposal', 'restore', 'repo')),
            author_id uuid REFERENCES users (id) ON DELETE SET NULL,
            persona text NOT NULL DEFAULT '',
            proposal_id uuid,
            created_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (doc_id, number),
            FOREIGN KEY (doc_id, workspace_id) REFERENCES knowledge_docs (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX knowledge_versions_workspace_id_idx ON knowledge_versions (workspace_id)".into(),
        "CREATE TABLE knowledge_runs (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            kind text NOT NULL CHECK (kind IN ('scan', 'update', 'audit')),
            trigger text NOT NULL CHECK (trigger IN ('manual', 'done', 'merge', 'audit', 'init')),
            depth text NOT NULL DEFAULT 'standar' CHECK (depth IN ('cepat', 'standar', 'mendalam')),
            scope text NOT NULL DEFAULT '',
            doc_ids uuid[] NOT NULL DEFAULT '{}',
            ticket_id uuid,
            trigger_ref text NOT NULL DEFAULT '',
            estimate_tokens bigint NOT NULL DEFAULT 0,
            status text NOT NULL DEFAULT 'queued'
                CHECK (status IN ('queued', 'running', 'done', 'failed', 'cancelled')),
            proposals integer NOT NULL DEFAULT 0,
            requested_by uuid REFERENCES users (id) ON DELETE SET NULL,
            finished_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE SET NULL (ticket_id)
        )"
        .into(),
        "CREATE INDEX knowledge_runs_project_id_idx ON knowledge_runs (project_id, created_at)".into(),
        "CREATE INDEX knowledge_runs_workspace_id_idx ON knowledge_runs (workspace_id)".into(),
        format!(
            "CREATE TABLE knowledge_proposals (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            doc_id uuid,
            slug text NOT NULL DEFAULT '',
            title text NOT NULL,
            group_key text NOT NULL CHECK (group_key IN ({GROUPS})),
            type_key text NOT NULL DEFAULT '',
            base_version integer NOT NULL DEFAULT 0,
            body text NOT NULL,
            reason text NOT NULL DEFAULT '',
            trigger text NOT NULL
                CHECK (trigger IN ('scan', 'done', 'merge', 'audit', 'manual', 'init', 'repo')),
            ticket_id uuid,
            trigger_ref text NOT NULL DEFAULT '',
            run_id uuid,
            persona text NOT NULL DEFAULT '',
            tokens bigint,
            sources jsonb NOT NULL DEFAULT '[]',
            status text NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'approved', 'rejected', 'superseded')),
            decided_by uuid REFERENCES users (id) ON DELETE SET NULL,
            decided_at timestamptz,
            decision_note text NOT NULL DEFAULT '',
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (doc_id, workspace_id) REFERENCES knowledge_docs (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE SET NULL (ticket_id),
            FOREIGN KEY (run_id, workspace_id) REFERENCES knowledge_runs (id, workspace_id)
                ON DELETE SET NULL (run_id),
            CHECK (doc_id IS NOT NULL OR slug <> '')
        )"
        ),
        "CREATE INDEX knowledge_proposals_project_idx ON knowledge_proposals (project_id, status)"
            .into(),
        "CREATE INDEX knowledge_proposals_doc_id_idx ON knowledge_proposals (doc_id)".into(),
        "CREATE INDEX knowledge_proposals_workspace_id_idx ON knowledge_proposals (workspace_id)"
            .into(),
        "CREATE TABLE knowledge_sources (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            doc_id uuid NOT NULL,
            repo_id uuid NOT NULL REFERENCES repos (id) ON DELETE CASCADE,
            path text NOT NULL DEFAULT '',
            UNIQUE (doc_id, repo_id, path),
            FOREIGN KEY (doc_id, workspace_id) REFERENCES knowledge_docs (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX knowledge_sources_repo_id_idx ON knowledge_sources (repo_id)".into(),
        "CREATE INDEX knowledge_sources_workspace_id_idx ON knowledge_sources (workspace_id)".into(),
        "CREATE TABLE knowledge_settings (
            workspace_id uuid PRIMARY KEY REFERENCES workspaces (id) ON DELETE CASCADE,
            on_done boolean NOT NULL DEFAULT true,
            on_merge boolean NOT NULL DEFAULT true,
            audit boolean NOT NULL DEFAULT true,
            audit_day smallint NOT NULL DEFAULT 1 CHECK (audit_day BETWEEN 0 AND 6),
            default_depth text NOT NULL DEFAULT 'standar'
                CHECK (default_depth IN ('cepat', 'standar', 'mendalam')),
            repo_sync boolean NOT NULL DEFAULT true,
            last_audit_at timestamptz,
            updated_by uuid REFERENCES users (id) ON DELETE SET NULL,
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        "CREATE TABLE rule_backups (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            repo text NOT NULL,
            archive text NOT NULL,
            storage_key text NOT NULL,
            size bigint NOT NULL DEFAULT 0,
            files integer NOT NULL DEFAULT 0,
            manifest jsonb NOT NULL DEFAULT '{}',
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX rule_backups_project_id_idx ON rule_backups (project_id, created_at)".into(),
        "CREATE INDEX rule_backups_workspace_id_idx ON rule_backups (workspace_id)".into(),
        "ALTER TABLE projects ADD COLUMN knowledge_owner_id uuid
            REFERENCES users (id) ON DELETE SET NULL"
            .into(),
        "ALTER TABLE personas ADD COLUMN knowledge_types text[] NOT NULL DEFAULT '{}'".into(),
        // Job knowledge: tanpa tiket, menaut ke run.
        "ALTER TABLE jobs ALTER COLUMN ticket_id DROP NOT NULL".into(),
        "ALTER TABLE job_permissions ALTER COLUMN ticket_id DROP NOT NULL".into(),
        "ALTER TABLE jobs DROP CONSTRAINT jobs_kind_check".into(),
        "ALTER TABLE jobs ADD CONSTRAINT jobs_kind_check
            CHECK (kind IN ('stage', 'comment', 'knowledge'))"
            .into(),
        "ALTER TABLE jobs ADD CONSTRAINT jobs_ticket_kind_check
            CHECK ((kind = 'knowledge') = (ticket_id IS NULL))"
            .into(),
        "ALTER TABLE jobs ADD COLUMN knowledge_run_id uuid
            REFERENCES knowledge_runs (id) ON DELETE SET NULL"
            .into(),
        "CREATE INDEX jobs_knowledge_run_id_idx ON jobs (knowledge_run_id)".into(),
        "ALTER TABLE notifications ADD COLUMN project_id uuid
            REFERENCES projects (id) ON DELETE CASCADE"
            .into(),
        "CREATE INDEX notifications_project_id_idx ON notifications (project_id)".into(),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check CHECK (kind IN (
            {NOTIFICATION_KINDS_M7}, 'knowledge_proposal', 'knowledge_job', 'knowledge_rejected'))"
        ),
    ]
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        format!("DELETE FROM notifications WHERE kind NOT IN ({NOTIFICATION_KINDS_M7})"),
        "ALTER TABLE notifications DROP CONSTRAINT notifications_kind_check".into(),
        format!(
            "ALTER TABLE notifications ADD CONSTRAINT notifications_kind_check \
             CHECK (kind IN ({NOTIFICATION_KINDS_M7}))"
        ),
        "ALTER TABLE notifications DROP COLUMN IF EXISTS project_id".into(),
        "DELETE FROM jobs WHERE kind = 'knowledge'".into(),
        "ALTER TABLE jobs DROP COLUMN IF EXISTS knowledge_run_id".into(),
        "ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_ticket_kind_check".into(),
        "ALTER TABLE jobs DROP CONSTRAINT jobs_kind_check".into(),
        "ALTER TABLE jobs ADD CONSTRAINT jobs_kind_check CHECK (kind IN ('stage', 'comment'))"
            .into(),
        "DELETE FROM job_permissions WHERE ticket_id IS NULL".into(),
        "ALTER TABLE job_permissions ALTER COLUMN ticket_id SET NOT NULL".into(),
        "ALTER TABLE jobs ALTER COLUMN ticket_id SET NOT NULL".into(),
        "ALTER TABLE personas DROP COLUMN IF EXISTS knowledge_types".into(),
        "ALTER TABLE projects DROP COLUMN IF EXISTS knowledge_owner_id".into(),
        "DROP TABLE IF EXISTS rule_backups".into(),
        "DROP TABLE IF EXISTS knowledge_settings".into(),
        "DROP TABLE IF EXISTS knowledge_sources".into(),
        "DROP TABLE IF EXISTS knowledge_proposals".into(),
        "DROP TABLE IF EXISTS knowledge_runs".into(),
        "DROP TABLE IF EXISTS knowledge_versions".into(),
        "DROP TABLE IF EXISTS knowledge_docs".into(),
        "DROP TABLE IF EXISTS knowledge_types".into(),
    ]
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for sql in up() {
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for sql in down() {
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }
}
