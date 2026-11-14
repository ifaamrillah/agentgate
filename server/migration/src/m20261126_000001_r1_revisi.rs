//! R1 revisi (keputusan pemilik produk 2026-11-14, `docs/REVIEW-AKHIR.md` §3.0).
//!
//! - `manual_evidence`: bukti gate yang diunggah anggota (screenshot/log/teks), berlabel manual,
//!   memenuhi gate `tests_passed`/`screenshot_exists` hanya setelah disetujui approver stage.
//! - `oauth_clients.approved_at`: client dinamis (DCR) baru boleh mendapat token setelah seorang
//!   user menyetujuinya; client tanpa persetujuan dihapus cleanup setelah 24 jam. Backfill: client
//!   yang sudah punya grant dianggap disetujui saat grant pertamanya dibuat.
//! - `runners.team|auto_claim|max_parallel`: runner tim (ditandai Owner/Admin) dengan auto-ambil
//!   dan batas paralel per runner. Runner pribadi tetap memakai `runner_prefs`.
//! - `api_tokens.rate_limit_per_minute`: limit per token (NULL = default instance).
//! - `pull_requests.merge_ready_at`: stage Code Reviewer sudah di-approve; mode merge manual
//!   menampilkan tombol Merge untuk PR ini bila CI hijau.
//! - izin `manage_knowledge` (Owner/Admin bawaan mendapatkannya).
//! - project dengan merge otomatis (`auto_on_review`) dikembalikan ke `manual` (default mati).
//!
//! Kompatibel dengan kode rilis sebelumnya: hanya tabel/kolom baru (nullable atau ber-default) dan
//! CHECK yang diperluas (ditambah, bukan ditulis ulang; lihat `extend_check`).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const AUDIT_ACTIONS: &[&str] = &[
    "evidence.submitted",
    "evidence.approved",
    "evidence.rejected",
    "runner.updated",
    "api_token.updated",
];
const AUDIT_SUBJECTS: &[&str] = &["evidence"];
const EVENT_KINDS: &[&str] = &["evidence_submitted", "evidence_decided"];
const NOTIFICATION_KINDS: &[&str] = &["evidence_review"];
const PERMISSIONS: &[&str] = &["manage_knowledge"];

/// `, 'a'::text, 'b'::text` (bentuk keluaran `pg_get_constraintdef` untuk `IN (...)`/`ARRAY[...]`).
fn suffix(values: &[&str]) -> String {
    values
        .iter()
        .map(|v| format!(", ''{v}''::text"))
        .collect::<String>()
}

/// Tambah `values` ke CHECK `IN (...)`/`<@ ARRAY[...]` constraint `name` di tabel `table`.
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

fn up() -> Vec<String> {
    vec![
        "SET lock_timeout = '5s'".into(),
        extend_check("audit_events", "audit_events_action_check", AUDIT_ACTIONS),
        extend_check(
            "audit_events",
            "audit_events_subject_type_check",
            AUDIT_SUBJECTS,
        ),
        extend_check("ticket_events", "ticket_events_kind_check", EVENT_KINDS),
        extend_check(
            "notifications",
            "notifications_kind_check",
            NOTIFICATION_KINDS,
        ),
        extend_check("roles", "roles_permissions_check", PERMISSIONS),
        // Owner dan Admin bawaan mengelola knowledge workspace (models::permission).
        "UPDATE roles SET permissions = permissions || ARRAY['manage_knowledge']::text[]
         WHERE key IN ('owner', 'admin') AND NOT ('manage_knowledge' = ANY (permissions))"
            .into(),
        // Bukti manual: berkas (screenshot/log) di storage `manual-evidence/<ticket>/<id>` atau teks
        // langsung di `body`. Pengunggah dihapus → bukti tetap (riwayat), pengunggah NULL.
        "CREATE TABLE manual_evidence (
            id uuid PRIMARY KEY DEFAULT uuidv7(),
            workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
            ticket_id uuid NOT NULL,
            stage_key text NOT NULL,
            gate text NOT NULL CHECK (gate IN ('tests_passed', 'screenshot_exists')),
            kind text NOT NULL CHECK (kind IN ('screenshot', 'log', 'text')),
            name text NOT NULL,
            content_type text NOT NULL DEFAULT '',
            size_bytes integer NOT NULL DEFAULT 0,
            storage_key text,
            body text NOT NULL DEFAULT '',
            note text NOT NULL DEFAULT '',
            submitted_by uuid REFERENCES users (id) ON DELETE SET NULL,
            status text NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'approved', 'rejected')),
            decided_by uuid REFERENCES users (id) ON DELETE SET NULL,
            decided_at timestamptz,
            decision_note text NOT NULL DEFAULT '',
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now(),
            FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
                ON DELETE CASCADE
        )"
        .into(),
        "CREATE INDEX manual_evidence_ticket_idx ON manual_evidence (ticket_id, stage_key, created_at)"
            .into(),
        "CREATE INDEX manual_evidence_workspace_id_idx ON manual_evidence (workspace_id)".into(),
        "ALTER TABLE oauth_clients ADD COLUMN approved_at timestamptz".into(),
        "UPDATE oauth_clients c SET approved_at = g.first
         FROM (SELECT client_id, min(created_at) AS first FROM oauth_grants GROUP BY client_id) g
         WHERE g.client_id = c.id AND c.approved_at IS NULL"
            .into(),
        // Client CLI bawaan (`services::device::CLI_CLIENT_ID`) didaftarkan server sendiri.
        "UPDATE oauth_clients SET approved_at = created_at
         WHERE client_id = 'agentgate-cli' AND approved_at IS NULL"
            .into(),
        "CREATE INDEX oauth_clients_unapproved_idx ON oauth_clients (created_at)
         WHERE approved_at IS NULL"
            .into(),
        "ALTER TABLE runners ADD COLUMN team boolean NOT NULL DEFAULT false".into(),
        "ALTER TABLE runners ADD COLUMN auto_claim boolean NOT NULL DEFAULT false".into(),
        "ALTER TABLE runners ADD COLUMN max_parallel integer".into(),
        "ALTER TABLE api_tokens ADD COLUMN rate_limit_per_minute integer".into(),
        "ALTER TABLE pull_requests ADD COLUMN merge_ready_at timestamptz".into(),
        // Keputusan pemilik: merge otomatis default mati; project yang sudah menyalakannya
        // dikembalikan ke manual (tombol Merge), bisa dinyalakan lagi di Konvensi git.
        "UPDATE projects SET git = jsonb_set(git, '{merge_mode}', '\"manual\"')
         WHERE git->>'merge_mode' = 'auto_on_review'"
            .into(),
    ]
}

fn down() -> Vec<String> {
    vec![
        "ALTER TABLE pull_requests DROP COLUMN IF EXISTS merge_ready_at".into(),
        "ALTER TABLE api_tokens DROP COLUMN IF EXISTS rate_limit_per_minute".into(),
        "ALTER TABLE runners DROP COLUMN IF EXISTS max_parallel".into(),
        "ALTER TABLE runners DROP COLUMN IF EXISTS auto_claim".into(),
        "ALTER TABLE runners DROP COLUMN IF EXISTS team".into(),
        "DROP INDEX IF EXISTS oauth_clients_unapproved_idx".into(),
        "ALTER TABLE oauth_clients DROP COLUMN IF EXISTS approved_at".into(),
        "DROP TABLE IF EXISTS manual_evidence".into(),
        "UPDATE roles SET permissions = array_remove(permissions, 'manage_knowledge')".into(),
        shrink_check("roles", "roles_permissions_check", PERMISSIONS),
        "DELETE FROM notifications WHERE kind = 'evidence_review'".into(),
        shrink_check(
            "notifications",
            "notifications_kind_check",
            NOTIFICATION_KINDS,
        ),
        "DELETE FROM ticket_events WHERE kind IN ('evidence_submitted', 'evidence_decided')".into(),
        shrink_check("ticket_events", "ticket_events_kind_check", EVENT_KINDS),
        "DELETE FROM audit_events WHERE subject_type = 'evidence'
            OR action IN ('runner.updated', 'api_token.updated')"
            .into(),
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
