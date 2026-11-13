//! M9 Planning & laporan: sprint (+ log scope untuk burndown/burnup), rilis (Fix version) dengan
//! release notes berversi, components, estimasi (AI/manusia) per tiket, dan saved filter JQL.
//!
//! - `tickets.sprint_id/release_id/component_id`: FK komposit dengan `ON DELETE SET NULL (kolom)`
//!   supaya `workspace_id` tiket tidak ikut dikosongkan saat sprint/rilis/component dihapus.
//! - `tickets.backlog_rank`: urutan backlog/sprint (kecil = atas), terpisah dari `position` board.
//! - `sprint_scope_events`: log perubahan isi sprint (masuk/keluar, selesai/dibuka lagi, poin)
//!   yang direplay laporan; ditulis di transaksi yang sama dengan perubahannya.
//! - `ticket_drafts.estimate`: estimasi AI dari `ag_draft_tickets` (disalin saat tiket dibuat).
//! - `projects.planning`: aturan components (penerima default).
//! - Persona bawaan mendapat tool MCP baru `ag_planning`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "CREATE TABLE sprints (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
            goal text NOT NULL DEFAULT '',
            starts_on date NOT NULL,
            ends_on date NOT NULL,
            state text NOT NULL DEFAULT 'planned'
                CHECK (state IN ('planned', 'active', 'completed')),
            capacity integer CHECK (capacity IS NULL OR capacity >= 0),
            started_at timestamptz,
            completed_at timestamptz,
            committed_points integer NOT NULL DEFAULT 0,
            completed_points integer NOT NULL DEFAULT 0,
            report jsonb NOT NULL DEFAULT '{}',
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            CHECK (ends_on >= starts_on),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX sprints_project_id_idx ON sprints (project_id, starts_on, id)".into(),
        "CREATE INDEX sprints_workspace_id_idx ON sprints (workspace_id)".into(),
        "CREATE TABLE releases (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 60),
            description text NOT NULL DEFAULT '',
            status text NOT NULL DEFAULT 'planned'
                CHECK (status IN ('planned', 'unreleased', 'released', 'archived')),
            starts_on date,
            target_on date,
            released_at timestamptz,
            notes text NOT NULL DEFAULT '',
            notes_version integer NOT NULL DEFAULT 0,
            notes_source text NOT NULL DEFAULT '' CHECK (notes_source IN ('', 'ai', 'manual')),
            notes_updated_at timestamptz,
            notes_published boolean NOT NULL DEFAULT false,
            created_by uuid REFERENCES users (id) ON DELETE SET NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE UNIQUE INDEX releases_project_name_key ON releases (project_id, lower(name))".into(),
        "CREATE INDEX releases_workspace_id_idx ON releases (workspace_id)".into(),
        "CREATE TABLE release_note_versions (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            release_id uuid NOT NULL,
            number integer NOT NULL CHECK (number > 0),
            body text NOT NULL,
            source text NOT NULL CHECK (source IN ('ai', 'manual')),
            author_id uuid REFERENCES users (id) ON DELETE SET NULL,
            persona text NOT NULL DEFAULT '',
            created_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (release_id, number),
            FOREIGN KEY (release_id, workspace_id) REFERENCES releases (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX release_note_versions_workspace_id_idx ON release_note_versions (workspace_id)"
            .into(),
        "CREATE TABLE components (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
            description text NOT NULL DEFAULT '',
            owner_id uuid REFERENCES users (id) ON DELETE SET NULL,
            modules text[] NOT NULL DEFAULT '{}',
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (id, workspace_id),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE UNIQUE INDEX components_project_name_key ON components (project_id, lower(name))"
            .into(),
        "CREATE INDEX components_workspace_id_idx ON components (workspace_id)".into(),
        "ALTER TABLE tickets
            ADD COLUMN sprint_id uuid,
            ADD COLUMN release_id uuid,
            ADD COLUMN component_id uuid,
            ADD COLUMN backlog_rank bigint NOT NULL DEFAULT 0"
            .into(),
        // Backfill tabel kecil: urutan backlog awal mengikuti nomor tiket.
        "UPDATE tickets SET backlog_rank = number::bigint * 1024".into(),
        "ALTER TABLE tickets
            ADD CONSTRAINT tickets_sprint_fk FOREIGN KEY (sprint_id, workspace_id)
                REFERENCES sprints (id, workspace_id) ON DELETE SET NULL (sprint_id),
            ADD CONSTRAINT tickets_release_fk FOREIGN KEY (release_id, workspace_id)
                REFERENCES releases (id, workspace_id) ON DELETE SET NULL (release_id),
            ADD CONSTRAINT tickets_component_fk FOREIGN KEY (component_id, workspace_id)
                REFERENCES components (id, workspace_id) ON DELETE SET NULL (component_id)"
            .into(),
        "CREATE INDEX tickets_sprint_id_idx ON tickets (sprint_id, backlog_rank)".into(),
        "CREATE INDEX tickets_release_id_idx ON tickets (release_id)".into(),
        "CREATE INDEX tickets_component_id_idx ON tickets (component_id)".into(),
        "CREATE INDEX tickets_backlog_idx ON tickets (project_id, backlog_rank, id)
            WHERE deleted_at IS NULL"
            .into(),
        "CREATE TABLE sprint_scope_events (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            sprint_id uuid NOT NULL,
            ticket_id uuid NOT NULL,
            kind text NOT NULL CHECK (kind IN ('added', 'removed', 'done', 'reopened', 'points')),
            points integer NOT NULL DEFAULT 0,
            created_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (sprint_id, workspace_id) REFERENCES sprints (id, workspace_id)
                ON DELETE CASCADE,
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX sprint_scope_events_sprint_idx ON sprint_scope_events (sprint_id, created_at, id)"
            .into(),
        "CREATE INDEX sprint_scope_events_ticket_idx ON sprint_scope_events (ticket_id)".into(),
        "CREATE INDEX sprint_scope_events_workspace_id_idx ON sprint_scope_events (workspace_id)"
            .into(),
        "CREATE TABLE ticket_estimates (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            ticket_id uuid NOT NULL,
            source text NOT NULL CHECK (source IN ('ai', 'human')),
            points integer CHECK (points IS NULL OR points BETWEEN 0 AND 1000),
            stages jsonb NOT NULL DEFAULT '[]',
            basis text NOT NULL DEFAULT '',
            author_id uuid REFERENCES users (id) ON DELETE SET NULL,
            persona text NOT NULL DEFAULT '',
            created_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX ticket_estimates_ticket_idx ON ticket_estimates (ticket_id, created_at, id)"
            .into(),
        "CREATE INDEX ticket_estimates_workspace_id_idx ON ticket_estimates (workspace_id)".into(),
        "CREATE TABLE saved_filters (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            owner_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
            name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
            query text NOT NULL CHECK (char_length(query) BETWEEN 1 AND 2000),
            visibility text NOT NULL DEFAULT 'private'
                CHECK (visibility IN ('private', 'team', 'workspace')),
            team_id uuid REFERENCES teams (id) ON DELETE CASCADE,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            CHECK ((visibility = 'team') = (team_id IS NOT NULL))
        )"
        .into(),
        "CREATE INDEX saved_filters_workspace_id_idx ON saved_filters (workspace_id, name)".into(),
        "CREATE INDEX saved_filters_owner_id_idx ON saved_filters (owner_id)".into(),
        "ALTER TABLE ticket_drafts ADD COLUMN estimate jsonb NOT NULL DEFAULT '{}'".into(),
        "ALTER TABLE projects ADD COLUMN planning jsonb NOT NULL DEFAULT '{}'".into(),
        "UPDATE personas SET mcp_tools = array_append(mcp_tools, 'ag_planning') \
         WHERE builtin AND NOT ('ag_planning' = ANY (mcp_tools))"
            .into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "UPDATE personas SET mcp_tools = array_remove(mcp_tools, 'ag_planning')".into(),
        "ALTER TABLE projects DROP COLUMN IF EXISTS planning".into(),
        "ALTER TABLE ticket_drafts DROP COLUMN IF EXISTS estimate".into(),
        "DROP TABLE IF EXISTS saved_filters".into(),
        "DROP TABLE IF EXISTS ticket_estimates".into(),
        "DROP TABLE IF EXISTS sprint_scope_events".into(),
        "ALTER TABLE tickets
            DROP COLUMN IF EXISTS sprint_id,
            DROP COLUMN IF EXISTS release_id,
            DROP COLUMN IF EXISTS component_id,
            DROP COLUMN IF EXISTS backlog_rank"
            .into(),
        "DROP TABLE IF EXISTS components".into(),
        "DROP TABLE IF EXISTS release_note_versions".into(),
        "DROP TABLE IF EXISTS releases".into(),
        "DROP TABLE IF EXISTS sprints".into(),
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
