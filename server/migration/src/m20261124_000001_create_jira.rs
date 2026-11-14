//! M11 Connector Jira.
//!
//! - `jira_connections`: satu koneksi per workspace (MVP). Jira Cloud (email + API token, REST v3)
//!   atau Data Center (Personal Access Token, REST v2); token terenkripsi (XChaCha20-Poly1305,
//!   `ENCRYPTION_KEY`). Project Jira ↔ project Agent Gate, mode `once|mirror|two_way`, pemetaan
//!   status/field/user (jsonb bertipe di model), katalog Jira terakhir (project, status, field,
//!   user) supaya halaman GET tidak memanggil Jira, cursor outbox (mirror), watermark tarik
//!   (dua arah), jeda, dan klaim sinkron yang sedang berjalan (`sync_started_at`).
//! - `jira_links`: tiket ↔ issue. `snapshot` = nilai field terakhir yang disepakati kedua sisi
//!   (dasar penggabungan tiga arah dan pencegah gema), `jira_status_id` = status Jira terakhir
//!   yang diketahui.
//! - `jira_comment_links`: komentar ↔ komentar Jira beserta asalnya, supaya komentar dari Jira
//!   tidak didorong balik (dan sebaliknya).
//! - `jira_sync_logs`: log sinkron (arah `in|out`, hasil `ok|gate|fail`, kunci pesan Fluent +
//!   argumen).
//!
//! CHECK aksi/subjek audit dan jenis notifikasi diperluas dengan **menambah** nilai ke
//! constraint yang sedang terpasang (dibaca lewat `pg_get_constraintdef`), bukan menulis ulang
//! daftar lengkapnya: migrasi lain di milestone yang sama boleh memperluas constraint yang sama
//! tanpa saling menimpa.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const AUDIT_ACTIONS: &[&str] = &["jira.connected", "jira.updated", "jira.disconnected"];
const AUDIT_SUBJECTS: &[&str] = &["jira_connection"];
const NOTIFICATION_KINDS: &[&str] = &["jira_failed"];

/// `, 'a'::text, 'b'::text` (bentuk keluaran `pg_get_constraintdef` untuk `IN (...)`).
fn suffix(values: &[&str]) -> String {
    values
        .iter()
        .map(|v| format!(", ''{v}''::text"))
        .collect::<String>()
}

/// Tambah `values` ke CHECK `IN (...)` constraint `name` di tabel `table`.
fn extend_check(table: &str, name: &str, values: &[&str]) -> String {
    let add = suffix(values);
    format!(
        "DO $$
        DECLARE def text;
        BEGIN
          SELECT pg_get_constraintdef(c.oid) INTO def FROM pg_constraint c
            WHERE c.conname = '{name}' AND c.conrelid = '{table}'::regclass;
          EXECUTE 'ALTER TABLE {table} DROP CONSTRAINT {name}';
          EXECUTE 'ALTER TABLE {table} ADD CONSTRAINT {name} '
            || regexp_replace(def, '\\]', '{add}]');
        END $$"
    )
}

/// Kebalikan [`extend_check`]: buang `values` dari constraint.
fn shrink_check(table: &str, name: &str, values: &[&str]) -> String {
    let add = suffix(values);
    format!(
        "DO $$
        DECLARE def text;
        BEGIN
          SELECT pg_get_constraintdef(c.oid) INTO def FROM pg_constraint c
            WHERE c.conname = '{name}' AND c.conrelid = '{table}'::regclass;
          EXECUTE 'ALTER TABLE {table} DROP CONSTRAINT {name}';
          EXECUTE 'ALTER TABLE {table} ADD CONSTRAINT {name} ' || replace(def, '{add}', '');
        END $$"
    )
}

fn list(values: &[&str]) -> String {
    values
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        extend_check("audit_events", "audit_events_action_check", AUDIT_ACTIONS),
        extend_check(
            "audit_events",
            "audit_events_subject_type_check",
            AUDIT_SUBJECTS,
        ),
        extend_check(
            "notifications",
            "notifications_kind_check",
            NOTIFICATION_KINDS,
        ),
        // Satu koneksi per workspace (MVP): `workspace_id UNIQUE`. Project Agent Gate dihapus →
        // koneksi tetap ada tanpa project (sinkron berhenti sampai project dipilih lagi).
        "CREATE TABLE jira_connections (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL UNIQUE REFERENCES workspaces (id) ON DELETE CASCADE,
            env text NOT NULL CHECK (env IN ('cloud', 'data_center')),
            base_url text NOT NULL CHECK (char_length(base_url) BETWEEN 1 AND 2000),
            email text NOT NULL DEFAULT '',
            token_encrypted bytea NOT NULL,
            account_name text NOT NULL DEFAULT '',
            jira_project_key text NOT NULL DEFAULT '',
            jira_project_name text NOT NULL DEFAULT '',
            project_id uuid REFERENCES projects (id) ON DELETE SET NULL,
            mode text NOT NULL DEFAULT 'mirror' CHECK (mode IN ('once', 'mirror', 'two_way')),
            status_map jsonb NOT NULL DEFAULT '{}',
            field_map jsonb NOT NULL DEFAULT '{}',
            user_map jsonb NOT NULL DEFAULT '{}',
            catalog jsonb NOT NULL DEFAULT '{}',
            stats jsonb NOT NULL DEFAULT '{}',
            owner_id uuid REFERENCES users (id) ON DELETE SET NULL,
            outbox_after_at timestamptz,
            outbox_after_id uuid,
            pulled_at timestamptz,
            last_sync_at timestamptz,
            last_error text NOT NULL DEFAULT '',
            paused_at timestamptz,
            imported_at timestamptz,
            sync_started_at timestamptz,
            full_sync_requested_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        "CREATE INDEX jira_connections_project_id_idx ON jira_connections (project_id)".into(),
        "CREATE INDEX jira_connections_owner_id_idx ON jira_connections (owner_id)".into(),
        // Tiket dihapus permanen → tautannya ikut hilang.
        "CREATE TABLE jira_links (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            connection_id uuid NOT NULL REFERENCES jira_connections (id) ON DELETE CASCADE,
            ticket_id uuid NOT NULL,
            issue_id text NOT NULL,
            issue_key text NOT NULL,
            jira_status_id text NOT NULL DEFAULT '',
            snapshot jsonb NOT NULL DEFAULT '{}',
            jira_updated_at timestamptz,
            synced_at timestamptz,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE,
            UNIQUE (connection_id, issue_id),
            UNIQUE (connection_id, ticket_id)
        )"
        .into(),
        "CREATE INDEX jira_links_ticket_id_idx ON jira_links (ticket_id)".into(),
        // Komentar dihapus → tautannya ikut hilang.
        "CREATE TABLE jira_comment_links (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            connection_id uuid NOT NULL REFERENCES jira_connections (id) ON DELETE CASCADE,
            comment_id uuid NOT NULL UNIQUE REFERENCES ticket_comments (id) ON DELETE CASCADE,
            jira_comment_id text NOT NULL,
            origin text NOT NULL CHECK (origin IN ('agent_gate', 'jira')),
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            UNIQUE (connection_id, jira_comment_id)
        )"
        .into(),
        // Tiket dihapus → baris log tetap (kode tiket tersimpan sebagai teks).
        "CREATE TABLE jira_sync_logs (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            connection_id uuid NOT NULL REFERENCES jira_connections (id) ON DELETE CASCADE,
            direction text NOT NULL CHECK (direction IN ('in', 'out')),
            result text NOT NULL CHECK (result IN ('ok', 'gate', 'fail')),
            ticket_id uuid REFERENCES tickets (id) ON DELETE SET NULL,
            ticket_code text NOT NULL DEFAULT '',
            issue_key text NOT NULL DEFAULT '',
            message text NOT NULL,
            args jsonb NOT NULL DEFAULT '{}',
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )"
        .into(),
        "CREATE INDEX jira_sync_logs_connection_id_idx
            ON jira_sync_logs (connection_id, created_at DESC, id DESC)"
            .into(),
        "CREATE INDEX jira_sync_logs_workspace_id_idx ON jira_sync_logs (workspace_id)".into(),
        "CREATE INDEX jira_sync_logs_ticket_id_idx ON jira_sync_logs (ticket_id)".into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        "DROP TABLE IF EXISTS jira_sync_logs".into(),
        "DROP TABLE IF EXISTS jira_comment_links".into(),
        "DROP TABLE IF EXISTS jira_links".into(),
        "DROP TABLE IF EXISTS jira_connections".into(),
        format!(
            "DELETE FROM notifications WHERE kind IN ({})",
            list(NOTIFICATION_KINDS)
        ),
        shrink_check(
            "notifications",
            "notifications_kind_check",
            NOTIFICATION_KINDS,
        ),
        format!(
            "DELETE FROM audit_events WHERE action IN ({}) OR subject_type IN ({})",
            list(AUDIT_ACTIONS),
            list(AUDIT_SUBJECTS)
        ),
        shrink_check(
            "audit_events",
            "audit_events_subject_type_check",
            AUDIT_SUBJECTS,
        ),
        shrink_check("audit_events", "audit_events_action_check", AUDIT_ACTIONS),
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
