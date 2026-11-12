//! M2 Tiket & board: tipe tiket, field custom, tiket, kode tiket (+ alias), penghitung nomor,
//! relasi, komentar, riwayat, lampiran, watcher, dan notifikasi kotak masuk. Juga izin baru
//! `manage_ticket_settings` (Owner/Admin bawaan) dan aksi/subjek audit tiket.
//!
//! Semua tabel milik workspace (`workspace_id NOT NULL`, `ON DELETE CASCADE` ke `workspaces`),
//! dan relasi antar-tabel memakai FK komposit `(…, workspace_id)` seperti migrasi M1, sehingga
//! database menolak tiket, tipe, atau field milik workspace lain.
//!
//! `tickets.status` adalah kolom board M2 dan sekaligus *kategori* stage di M3 (lihat rencana
//! M2): himpunannya tetap, M3 hanya menambah `stage_id`.
//!
//! Himpunan nilai enum dijaga CHECK (rules §4, dijaga `tests/models/enums.rs`).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &[&str] = &[
    "SET lock_timeout = '5s'",
    // --- izin baru ---
    "ALTER TABLE roles DROP CONSTRAINT roles_permissions_check",
    "ALTER TABLE roles ADD CONSTRAINT roles_permissions_check CHECK (permissions <@ ARRAY[
        'manage_workspace', 'manage_members', 'manage_roles', 'manage_teams',
        'create_projects', 'view_audit_log', 'administer_project', 'edit_project',
        'view_project', 'manage_ticket_settings'
    ]::text[])",
    // Owner dan Admin bawaan memegang semua izin pengaturan tiket (models::permission).
    "UPDATE roles SET permissions = permissions || ARRAY['manage_ticket_settings']::text[]
     WHERE key IN ('owner', 'admin') AND NOT ('manage_ticket_settings' = ANY (permissions))",
    // --- audit tiket ---
    "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check",
    "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check CHECK (action IN (
        'workspace.created', 'workspace.updated', 'workspace.require_2fa_changed',
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
        'ticket_code.updated'
    ))",
    "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check",
    "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check CHECK (subject_type IN (
        'workspace', 'member', 'role', 'job_title', 'team', 'project', 'project_grant',
        'repo', 'invitation', 'ticket', 'ticket_type', 'custom_field'
    ))",
    // --- template kode tiket: default workspace, override per project (NULL = ikut workspace) ---
    "ALTER TABLE workspaces
        ADD COLUMN ticket_code_template text NOT NULL DEFAULT '{PROJECT}-{NUM}',
        ADD COLUMN ticket_counter_scope text NOT NULL DEFAULT 'project'
            CHECK (ticket_counter_scope IN ('project', 'type', 'year'))",
    // team_code: nilai token {TEAM} (NULL = key project).
    "ALTER TABLE projects
        ADD COLUMN ticket_code_template text,
        ADD COLUMN ticket_counter_scope text
            CHECK (ticket_counter_scope IN ('project', 'type', 'year')),
        ADD COLUMN team_code text",
    // --- tipe tiket ---
    // icon divalidasi aplikasi (daftar ikon pilihan bertambah tanpa migrasi), seperti warna.
    "CREATE TABLE ticket_types (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        key text NOT NULL,
        name text NOT NULL,
        description text NOT NULL DEFAULT '',
        icon text NOT NULL,
        color text NOT NULL CHECK (color ~ '^#[0-9a-f]{6}$'),
        builtin boolean NOT NULL DEFAULT false,
        position integer NOT NULL DEFAULT 0,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (id, workspace_id)
    )",
    "CREATE UNIQUE INDEX ticket_types_workspace_id_key_lower_key \
     ON ticket_types (workspace_id, lower(key))",
    "CREATE UNIQUE INDEX ticket_types_workspace_id_name_lower_key \
     ON ticket_types (workspace_id, lower(name))",
    // Tipe bawaan untuk workspace yang sudah ada (workspace baru: services::workspaces::create).
    "INSERT INTO ticket_types (workspace_id, key, name, description, icon, color, builtin, position)
     SELECT w.id, t.key, t.name, t.description, t.icon, t.color, true, t.position
     FROM workspaces w CROSS JOIN (VALUES
        ('epic', 'Epic', 'Kumpulan story besar lintas sprint', 'bolt', '#7c3aed', 0),
        ('story', 'Story', 'Kebutuhan dari sisi pengguna', 'bookmark', '#0f8a5f', 1),
        ('task', 'Task', 'Pekerjaan teknis atau operasional', 'square-check', '#2563eb', 2),
        ('bug', 'Bug', 'Perilaku yang salah di aplikasi', 'bug', '#dc2626', 3),
        ('subtask', 'Subtask', 'Pecahan kecil dari tiket lain', 'subtask', '#64748b', 4)
     ) AS t(key, name, description, icon, color, position)",
    // --- field custom ---
    "CREATE TABLE custom_fields (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        name text NOT NULL,
        kind text NOT NULL CHECK (kind IN ('text', 'number', 'select', 'date', 'url')),
        options text[] NOT NULL DEFAULT '{}',
        required boolean NOT NULL DEFAULT false,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (id, workspace_id)
    )",
    "CREATE UNIQUE INDEX custom_fields_workspace_id_name_lower_key \
     ON custom_fields (workspace_id, lower(name))",
    // Field mana dipakai tipe mana. Hapus tipe atau field menghapus pemakaiannya.
    "CREATE TABLE ticket_type_fields (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        type_id uuid NOT NULL,
        field_id uuid NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (type_id, field_id),
        FOREIGN KEY (type_id, workspace_id) REFERENCES ticket_types (id, workspace_id)
            ON DELETE CASCADE,
        FOREIGN KEY (field_id, workspace_id) REFERENCES custom_fields (id, workspace_id)
            ON DELETE CASCADE
    )",
    "CREATE INDEX ticket_type_fields_field_id_idx ON ticket_type_fields (field_id)",
    "CREATE INDEX ticket_type_fields_workspace_id_idx ON ticket_type_fields (workspace_id)",
    // --- tiket ---
    // project ON DELETE CASCADE: tiket tidak berarti tanpa project-nya.
    // type tanpa ON DELETE (NO ACTION): tipe yang masih dipakai tiket tidak bisa dihapus.
    // assignee/reporter/created_by/deleted_by ON DELETE SET NULL: tiket tetap ada walau user dihapus.
    // parent ON DELETE SET NULL: subtask tetap ada walau induknya dihapus permanen.
    // custom_values: nilai field custom per id field (models::tickets::CustomValues).
    // position: urutan kartu di kolom board (kecil = atas).
    "CREATE TABLE tickets (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        project_id uuid NOT NULL,
        type_id uuid NOT NULL,
        number integer NOT NULL,
        code text NOT NULL,
        title text NOT NULL,
        description text NOT NULL DEFAULT '',
        status text NOT NULL DEFAULT 'todo'
            CHECK (status IN ('backlog', 'todo', 'in_progress', 'done')),
        priority text NOT NULL DEFAULT 'medium'
            CHECK (priority IN ('urgent', 'high', 'medium', 'low')),
        assignee_id uuid REFERENCES users (id) ON DELETE SET NULL,
        reporter_id uuid REFERENCES users (id) ON DELETE SET NULL,
        parent_id uuid REFERENCES tickets (id) ON DELETE SET NULL,
        labels text[] NOT NULL DEFAULT '{}',
        due_on date,
        story_points integer,
        custom_values jsonb NOT NULL DEFAULT '{}',
        position bigint NOT NULL DEFAULT 0,
        done_at timestamptz,
        deleted_at timestamptz,
        deleted_by uuid REFERENCES users (id) ON DELETE SET NULL,
        created_by uuid REFERENCES users (id) ON DELETE SET NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (id, workspace_id),
        FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
            ON DELETE CASCADE,
        FOREIGN KEY (type_id, workspace_id) REFERENCES ticket_types (id, workspace_id)
    )",
    // Board dan daftar: tiket aktif per project per kolom, urut posisi.
    "CREATE INDEX tickets_project_id_status_position_idx \
     ON tickets (project_id, status, position) WHERE deleted_at IS NULL",
    // Tiket saya (ditugaskan / dilaporkan).
    "CREATE INDEX tickets_assignee_id_idx ON tickets (assignee_id) WHERE deleted_at IS NULL",
    "CREATE INDEX tickets_reporter_id_idx ON tickets (reporter_id) WHERE deleted_at IS NULL",
    // Sampah per project dan purge 30 hari (sisi jarang: partial).
    "CREATE INDEX tickets_deleted_at_idx ON tickets (deleted_at) WHERE deleted_at IS NOT NULL",
    "CREATE INDEX tickets_type_id_idx ON tickets (type_id)",
    "CREATE INDEX tickets_parent_id_idx ON tickets (parent_id) WHERE parent_id IS NOT NULL",
    "CREATE INDEX tickets_workspace_id_idx ON tickets (workspace_id)",
    // Pencarian dasar: ekspresi yang sama dengan models::tickets::search_vector.
    "CREATE INDEX tickets_search_idx ON tickets USING gin \
     (to_tsvector('simple', code || ' ' || title || ' ' || description))",
    // Kode sekarang + alias lama; satu kode milik satu tiket di seluruh workspace.
    "CREATE TABLE ticket_codes (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        ticket_id uuid NOT NULL,
        code text NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
            ON DELETE CASCADE
    )",
    "CREATE UNIQUE INDEX ticket_codes_workspace_id_code_upper_key \
     ON ticket_codes (workspace_id, upper(code))",
    "CREATE INDEX ticket_codes_ticket_id_idx ON ticket_codes (ticket_id)",
    // Penghitung nomor per project per skop ('' = project, 'type:<id>', 'year:<yyyy>').
    "CREATE TABLE ticket_counters (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        project_id uuid NOT NULL,
        scope text NOT NULL,
        next_number integer NOT NULL DEFAULT 1,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (project_id, scope),
        FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
            ON DELETE CASCADE
    )",
    "CREATE INDEX ticket_counters_workspace_id_idx ON ticket_counters (workspace_id)",
    // Relasi: from `kind` to (from memblokir to). Hapus salah satu tiket menghapus relasinya.
    "CREATE TABLE ticket_links (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        from_id uuid NOT NULL,
        to_id uuid NOT NULL,
        kind text NOT NULL CHECK (kind IN ('blocks', 'duplicates', 'relates')),
        created_by uuid REFERENCES users (id) ON DELETE SET NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        CHECK (from_id <> to_id),
        UNIQUE (from_id, to_id, kind),
        FOREIGN KEY (from_id, workspace_id) REFERENCES tickets (id, workspace_id)
            ON DELETE CASCADE,
        FOREIGN KEY (to_id, workspace_id) REFERENCES tickets (id, workspace_id)
            ON DELETE CASCADE
    )",
    "CREATE INDEX ticket_links_to_id_idx ON ticket_links (to_id)",
    "CREATE INDEX ticket_links_workspace_id_idx ON ticket_links (workspace_id)",
    // Komentar. author ON DELETE SET NULL: diskusi tetap terbaca.
    "CREATE TABLE ticket_comments (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        ticket_id uuid NOT NULL,
        author_id uuid REFERENCES users (id) ON DELETE SET NULL,
        body text NOT NULL,
        edited_at timestamptz,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (id, workspace_id),
        FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
            ON DELETE CASCADE
    )",
    "CREATE INDEX ticket_comments_ticket_id_created_at_idx \
     ON ticket_comments (ticket_id, created_at, id)",
    "CREATE INDEX ticket_comments_workspace_id_idx ON ticket_comments (workspace_id)",
    // Riwayat aktivitas tiket. Append-only: tanpa updated_at (seperti audit_events).
    "CREATE TABLE ticket_events (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        ticket_id uuid NOT NULL,
        actor_id uuid REFERENCES users (id) ON DELETE SET NULL,
        kind text NOT NULL CHECK (kind IN (
            'created', 'updated', 'status_changed', 'linked', 'unlinked',
            'attachment_added', 'attachment_removed', 'trashed', 'restored'
        )),
        changes jsonb NOT NULL DEFAULT '{}',
        created_at timestamptz NOT NULL DEFAULT now(),
        FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
            ON DELETE CASCADE
    )",
    "CREATE INDEX ticket_events_ticket_id_created_at_idx \
     ON ticket_events (ticket_id, created_at, id)",
    "CREATE INDEX ticket_events_workspace_id_idx ON ticket_events (workspace_id)",
    // Lampiran: DB menyimpan key storage (rules §8). Purge tiket menghapus barisnya lewat
    // cascade; file-nya dihapus services::cleanup setelah commit.
    "CREATE TABLE ticket_attachments (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        ticket_id uuid NOT NULL,
        uploader_id uuid REFERENCES users (id) ON DELETE SET NULL,
        filename text NOT NULL,
        content_type text NOT NULL,
        size bigint NOT NULL,
        storage_key text NOT NULL UNIQUE,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
            ON DELETE CASCADE
    )",
    "CREATE INDEX ticket_attachments_ticket_id_idx ON ticket_attachments (ticket_id)",
    "CREATE INDEX ticket_attachments_workspace_id_idx ON ticket_attachments (workspace_id)",
    // Watcher harus anggota workspace; keluar dari workspace berhenti mengikuti.
    "CREATE TABLE ticket_watchers (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        ticket_id uuid NOT NULL,
        user_id uuid NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (ticket_id, user_id),
        FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
            ON DELETE CASCADE,
        FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id)
            ON DELETE CASCADE
    )",
    "CREATE INDEX ticket_watchers_workspace_id_user_id_idx \
     ON ticket_watchers (workspace_id, user_id)",
    // Notifikasi in-app (spec: notifikasi hanya di dalam aplikasi). Penerima harus anggota
    // workspace; tiket/komentar yang dihapus permanen ikut menghapus notifikasinya.
    "CREATE TABLE notifications (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        user_id uuid NOT NULL,
        actor_id uuid REFERENCES users (id) ON DELETE SET NULL,
        kind text NOT NULL CHECK (kind IN ('mentioned', 'assigned', 'commented')),
        ticket_id uuid,
        comment_id uuid,
        read_at timestamptz,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id)
            ON DELETE CASCADE,
        FOREIGN KEY (ticket_id, workspace_id) REFERENCES tickets (id, workspace_id)
            ON DELETE CASCADE,
        FOREIGN KEY (comment_id, workspace_id) REFERENCES ticket_comments (id, workspace_id)
            ON DELETE CASCADE
    )",
    // Kotak masuk: terbaru dulu per penerima per workspace; badge belum dibaca.
    "CREATE INDEX notifications_inbox_idx \
     ON notifications (workspace_id, user_id, created_at DESC, id DESC)",
    "CREATE INDEX notifications_unread_idx \
     ON notifications (workspace_id, user_id) WHERE read_at IS NULL",
    "CREATE INDEX notifications_ticket_id_idx ON notifications (ticket_id)",
    "CREATE INDEX notifications_comment_id_idx ON notifications (comment_id)",
];

const DOWN: &[&str] = &[
    "SET lock_timeout = '5s'",
    "DROP TABLE IF EXISTS notifications",
    "DROP TABLE IF EXISTS ticket_watchers",
    "DROP TABLE IF EXISTS ticket_attachments",
    "DROP TABLE IF EXISTS ticket_events",
    "DROP TABLE IF EXISTS ticket_comments",
    "DROP TABLE IF EXISTS ticket_links",
    "DROP TABLE IF EXISTS ticket_counters",
    "DROP TABLE IF EXISTS ticket_codes",
    "DROP TABLE IF EXISTS tickets",
    "DROP TABLE IF EXISTS ticket_type_fields",
    "DROP TABLE IF EXISTS custom_fields",
    "DROP TABLE IF EXISTS ticket_types",
    "ALTER TABLE projects DROP COLUMN IF EXISTS team_code,
        DROP COLUMN IF EXISTS ticket_counter_scope, DROP COLUMN IF EXISTS ticket_code_template",
    "ALTER TABLE workspaces DROP COLUMN IF EXISTS ticket_counter_scope,
        DROP COLUMN IF EXISTS ticket_code_template",
    "DELETE FROM audit_events WHERE subject_type IN ('ticket', 'ticket_type', 'custom_field')
        OR action = 'ticket_code.updated'",
    "ALTER TABLE audit_events DROP CONSTRAINT audit_events_subject_type_check",
    "ALTER TABLE audit_events ADD CONSTRAINT audit_events_subject_type_check CHECK (subject_type IN (
        'workspace', 'member', 'role', 'job_title', 'team', 'project', 'project_grant',
        'repo', 'invitation'
    ))",
    "ALTER TABLE audit_events DROP CONSTRAINT audit_events_action_check",
    "ALTER TABLE audit_events ADD CONSTRAINT audit_events_action_check CHECK (action IN (
        'workspace.created', 'workspace.updated', 'workspace.require_2fa_changed',
        'workspace.deletion_scheduled', 'workspace.deletion_cancelled',
        'member.invited', 'member.joined', 'member.role_changed', 'member.removed',
        'member.left', 'member.job_title_changed', 'member.invitation_revoked',
        'role.created', 'role.updated', 'role.deleted',
        'job_title.created', 'job_title.updated', 'job_title.deleted',
        'team.created', 'team.updated', 'team.deleted', 'team.member_added',
        'team.member_removed',
        'project.created', 'project.updated', 'project.archived',
        'project.access_granted', 'project.access_changed', 'project.access_revoked',
        'repo.added', 'repo.updated', 'repo.removed'
    ))",
    "UPDATE roles SET permissions = array_remove(permissions, 'manage_ticket_settings')",
    "ALTER TABLE roles DROP CONSTRAINT roles_permissions_check",
    "ALTER TABLE roles ADD CONSTRAINT roles_permissions_check CHECK (permissions <@ ARRAY[
        'manage_workspace', 'manage_members', 'manage_roles', 'manage_teams',
        'create_projects', 'view_audit_log', 'administer_project', 'edit_project',
        'view_project'
    ]::text[])",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in UP {
            db.execute_unprepared(sql).await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in DOWN {
            db.execute_unprepared(sql).await?;
        }
        Ok(())
    }
}
