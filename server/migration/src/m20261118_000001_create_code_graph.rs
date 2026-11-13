//! M7 Code graph: index per repo (status, statistik, permukaan lintas repo, SCIP, riwayat git),
//! file yang di-index (isi untuk `ag_snippet` + fakta tree-sitter jsonb untuk build ulang tanpa
//! parsing ulang), simbol, edge, dan relasi lintas repo per project.
//!
//! - `graph_repos`: satu baris per repo (dibuat saat index pertama).
//! - `graph_files.facts`: `agentgate_graph::FileFacts`; file dilewati menyimpan `skip_reason`
//!   tanpa isi (rahasia tidak pernah disimpan).
//! - `graph_symbols`/`graph_edges`: ditulis ulang setiap build (id berubah); pencarian memakai
//!   indeks GIN ekspresi `to_tsvector('simple', words || ' ' || doc)` seperti tiket.
//! - `graph_links`: dihitung ulang per project setelah repo mana pun di project di-build.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn up() -> Vec<&'static str> {
    vec![
        "SET lock_timeout = '5s'",
        "CREATE TABLE graph_repos (
            repo_id uuid PRIMARY KEY REFERENCES repos (id) ON DELETE CASCADE,
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            status text NOT NULL DEFAULT 'none'
                CHECK (status IN ('none', 'queued', 'running', 'ready', 'failed')),
            source text NOT NULL DEFAULT '' CHECK (source IN ('', 'cli', 'server')),
            mode text NOT NULL DEFAULT '' CHECK (mode IN ('', 'incremental', 'full')),
            commit_sha text NOT NULL DEFAULT '',
            branch text NOT NULL DEFAULT '',
            files_indexed integer NOT NULL DEFAULT 0,
            files_skipped integer NOT NULL DEFAULT 0,
            files_parsed integer NOT NULL DEFAULT 0,
            symbols integer NOT NULL DEFAULT 0,
            edges integer NOT NULL DEFAULT 0,
            edges_certain integer NOT NULL DEFAULT 0,
            stats jsonb NOT NULL DEFAULT '{}',
            languages jsonb NOT NULL DEFAULT '{}',
            skipped jsonb NOT NULL DEFAULT '{}',
            modules jsonb NOT NULL DEFAULT '[]',
            surface jsonb NOT NULL DEFAULT '{}',
            git_stats jsonb NOT NULL DEFAULT '{}',
            scip jsonb,
            scip_indexer text NOT NULL DEFAULT '',
            scip_by text NOT NULL DEFAULT '',
            scip_at timestamptz,
            error text NOT NULL DEFAULT '',
            requested_by uuid REFERENCES users (id) ON DELETE SET NULL,
            started_at timestamptz,
            indexed_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE
        )",
        "CREATE INDEX graph_repos_project_id_idx ON graph_repos (project_id)",
        "CREATE INDEX graph_repos_workspace_id_idx ON graph_repos (workspace_id)",
        "CREATE TABLE graph_files (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            repo_id uuid NOT NULL REFERENCES repos (id) ON DELETE CASCADE,
            path text NOT NULL,
            lang text NOT NULL,
            sha text NOT NULL DEFAULT '',
            bytes integer NOT NULL DEFAULT 0,
            lines integer NOT NULL DEFAULT 0,
            skip_reason text NOT NULL DEFAULT ''
                CHECK (skip_reason IN ('', 'secret', 'generated', 'too_large', 'binary',
                    'parse_failed')),
            content text NOT NULL DEFAULT '',
            facts jsonb NOT NULL DEFAULT '{}',
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (repo_id, path)
        )",
        "CREATE INDEX graph_files_workspace_id_idx ON graph_files (workspace_id)",
        "CREATE TABLE graph_symbols (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            repo_id uuid NOT NULL REFERENCES repos (id) ON DELETE CASCADE,
            file_id uuid NOT NULL REFERENCES graph_files (id) ON DELETE CASCADE,
            name text NOT NULL,
            qualified text NOT NULL,
            kind text NOT NULL CHECK (kind IN ('file', 'function', 'method', 'class',
                'interface', 'struct', 'enum', 'trait', 'type', 'module', 'route')),
            start_line integer NOT NULL,
            end_line integer NOT NULL,
            exported boolean NOT NULL DEFAULT false,
            test boolean NOT NULL DEFAULT false,
            module text NOT NULL DEFAULT '',
            central boolean NOT NULL DEFAULT false,
            dead text NOT NULL DEFAULT '' CHECK (dead IN ('', 'certain', 'inferred')),
            callers integer NOT NULL DEFAULT 0,
            callees integer NOT NULL DEFAULT 0,
            complexity integer NOT NULL DEFAULT 0,
            signature text NOT NULL DEFAULT '',
            doc text NOT NULL DEFAULT '',
            words text NOT NULL DEFAULT ''
        )",
        "CREATE INDEX graph_symbols_repo_name_idx ON graph_symbols (repo_id, lower(name))",
        "CREATE INDEX graph_symbols_file_id_idx ON graph_symbols (file_id)",
        "CREATE INDEX graph_symbols_workspace_id_idx ON graph_symbols (workspace_id)",
        "CREATE INDEX graph_symbols_search_idx ON graph_symbols
            USING gin (to_tsvector('simple', words || ' ' || doc))",
        "CREATE TABLE graph_edges (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            repo_id uuid NOT NULL REFERENCES repos (id) ON DELETE CASCADE,
            from_id uuid NOT NULL REFERENCES graph_symbols (id) ON DELETE CASCADE,
            to_id uuid NOT NULL REFERENCES graph_symbols (id) ON DELETE CASCADE,
            kind text NOT NULL CHECK (kind IN ('calls', 'inherits', 'imports', 'handles')),
            confidence text NOT NULL CHECK (confidence IN ('certain', 'inferred')),
            source text NOT NULL CHECK (source IN ('tree_sitter', 'scip')),
            line integer NOT NULL DEFAULT 0
        )",
        "CREATE INDEX graph_edges_from_id_idx ON graph_edges (from_id)",
        "CREATE INDEX graph_edges_to_id_idx ON graph_edges (to_id)",
        "CREATE INDEX graph_edges_repo_id_idx ON graph_edges (repo_id)",
        "CREATE INDEX graph_edges_workspace_id_idx ON graph_edges (workspace_id)",
        "CREATE TABLE graph_links (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            kind text NOT NULL CHECK (kind IN ('http', 'event', 'package')),
            confidence text NOT NULL
                CHECK (confidence IN ('certain', 'inferred', 'unmatched', 'external')),
            from_repo_id uuid NOT NULL REFERENCES repos (id) ON DELETE CASCADE,
            from_symbol_id uuid REFERENCES graph_symbols (id) ON DELETE CASCADE,
            to_repo_id uuid REFERENCES repos (id) ON DELETE CASCADE,
            to_symbol_id uuid REFERENCES graph_symbols (id) ON DELETE CASCADE,
            detail text NOT NULL DEFAULT '',
            created_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE
        )",
        "CREATE INDEX graph_links_project_id_idx ON graph_links (project_id)",
        "CREATE INDEX graph_links_workspace_id_idx ON graph_links (workspace_id)",
        "CREATE INDEX graph_links_from_symbol_id_idx ON graph_links (from_symbol_id)",
        "CREATE INDEX graph_links_to_symbol_id_idx ON graph_links (to_symbol_id)",
    ]
}

fn down() -> Vec<&'static str> {
    vec![
        "SET lock_timeout = '5s'",
        "DROP TABLE IF EXISTS graph_links",
        "DROP TABLE IF EXISTS graph_edges",
        "DROP TABLE IF EXISTS graph_symbols",
        "DROP TABLE IF EXISTS graph_files",
        "DROP TABLE IF EXISTS graph_repos",
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
