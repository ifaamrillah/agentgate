//! R2 revisi (keputusan pemilik produk 2026-11-14, `docs/REVIEW-AKHIR.md` §3.0).
//!
//! - `git_connections`: koneksi lewat **GitHub App** (`auth = 'app'`, `installation_id`, izin,
//!   repo yang dapat diakses, `suspended_at`). Koneksi PAT lama = `auth = 'token'`.
//! - `releases.git_tags`: hasil pembuatan git tag per repo saat "Rilis sekarang".
//! - `knowledge_prs`: PR knowledge yang dibuka server setelah usulan di-approve.
//! - `graph_repos.fetch_via|fetch_note`: index server lewat clone dangkal atau tarball cadangan.
//! - `graph_symbols.embed_hash` + `graph_embeddings`: embedding lokal per simbol (pgvector),
//!   dikunci hash teks supaya index ulang hanya meng-embed simbol yang berubah.
//!
//! Kompatibel dengan kode rilis sebelumnya: hanya tabel/kolom baru (nullable atau ber-default).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "ALTER TABLE git_connections
            ADD COLUMN auth text NOT NULL DEFAULT 'token' CHECK (auth IN ('token', 'app')),
            ADD COLUMN installation_id bigint,
            ADD COLUMN app_account_type text NOT NULL DEFAULT '',
            ADD COLUMN app_permissions jsonb NOT NULL DEFAULT '{}',
            ADD COLUMN app_repo_selection text NOT NULL DEFAULT '',
            ADD COLUMN app_repos jsonb NOT NULL DEFAULT '[]',
            ADD COLUMN suspended_at timestamptz"
            .into(),
        "CREATE INDEX git_connections_installation_idx ON git_connections (installation_id)
         WHERE installation_id IS NOT NULL"
            .into(),
        "ALTER TABLE releases ADD COLUMN git_tags jsonb NOT NULL DEFAULT '[]'".into(),
        "CREATE TABLE knowledge_prs (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            project_id uuid NOT NULL,
            doc_id uuid NOT NULL REFERENCES knowledge_docs (id) ON DELETE CASCADE,
            proposal_id uuid REFERENCES knowledge_proposals (id) ON DELETE SET NULL,
            repo_id uuid REFERENCES repos (id) ON DELETE SET NULL,
            branch text NOT NULL DEFAULT '',
            version integer NOT NULL,
            status text NOT NULL DEFAULT 'queued'
                CHECK (status IN ('queued', 'open', 'failed', 'skipped')),
            pull_request_id uuid REFERENCES pull_requests (id) ON DELETE SET NULL,
            error text NOT NULL DEFAULT '',
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX knowledge_prs_doc_idx ON knowledge_prs (doc_id, created_at)".into(),
        "CREATE INDEX knowledge_prs_workspace_id_idx ON knowledge_prs (workspace_id)".into(),
        "ALTER TABLE graph_repos
            ADD COLUMN fetch_via text NOT NULL DEFAULT ''
                CHECK (fetch_via IN ('', 'clone', 'tarball')),
            ADD COLUMN fetch_note text NOT NULL DEFAULT '',
            ADD COLUMN embed_symbols integer NOT NULL DEFAULT 0,
            ADD COLUMN embed_ms integer NOT NULL DEFAULT 0,
            ADD COLUMN embedded_at timestamptz,
            ADD COLUMN embed_error text NOT NULL DEFAULT ''"
            .into(),
        "ALTER TABLE graph_symbols ADD COLUMN embed_hash text NOT NULL DEFAULT ''".into(),
        "CREATE INDEX graph_symbols_embed_idx ON graph_symbols (repo_id, embed_hash)".into(),
        // Dimensi tidak dikunci di kolom: model bisa diganti lewat config (dimensi beda); baris
        // model lama diabaikan query (filter `model`) dan dibersihkan saat index berikutnya.
        "CREATE TABLE graph_embeddings (
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            repo_id uuid NOT NULL REFERENCES repos (id) ON DELETE CASCADE,
            model text NOT NULL,
            hash text NOT NULL,
            embedding vector NOT NULL,
            created_at timestamptz NOT NULL DEFAULT now(),
            PRIMARY KEY (repo_id, model, hash)
        )"
        .into(),
        "CREATE INDEX graph_embeddings_workspace_id_idx ON graph_embeddings (workspace_id)".into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "DROP TABLE IF EXISTS graph_embeddings".into(),
        "DROP INDEX IF EXISTS graph_symbols_embed_idx".into(),
        "ALTER TABLE graph_symbols DROP COLUMN IF EXISTS embed_hash".into(),
        "ALTER TABLE graph_repos DROP COLUMN IF EXISTS fetch_via, DROP COLUMN IF EXISTS fetch_note,
            DROP COLUMN IF EXISTS embed_symbols, DROP COLUMN IF EXISTS embed_ms,
            DROP COLUMN IF EXISTS embedded_at, DROP COLUMN IF EXISTS embed_error"
            .into(),
        "DROP TABLE IF EXISTS knowledge_prs".into(),
        "ALTER TABLE releases DROP COLUMN IF EXISTS git_tags".into(),
        "DROP INDEX IF EXISTS git_connections_installation_idx".into(),
        "DELETE FROM git_connections WHERE auth = 'app'".into(),
        "ALTER TABLE git_connections DROP COLUMN IF EXISTS auth,
            DROP COLUMN IF EXISTS installation_id, DROP COLUMN IF EXISTS app_account_type, DROP COLUMN IF EXISTS app_permissions,
            DROP COLUMN IF EXISTS app_repo_selection, DROP COLUMN IF EXISTS app_repos,
            DROP COLUMN IF EXISTS suspended_at"
            .into(),
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
