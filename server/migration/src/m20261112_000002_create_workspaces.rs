//! Tenancy: `workspaces` dan tabel milik workspace (`roles`, `job_titles`,
//! `workspace_members`, `invitations`, `audit_events`). Setiap tabel milik workspace punya
//! `workspace_id NOT NULL` dengan `ON DELETE CASCADE`: penghapusan workspace setelah masa
//! tenggang (job pembersihan, M1 Task 13) hanya menghapus baris `workspaces` dan bergantung pada
//! cascade ini untuk membersihkan semua baris turunannya.
//!
//! Himpunan nilai enum dijaga CHECK (rules §4): `roles.key`, setiap elemen `roles.permissions`
//! (`<@` terhadap daftar permission), `audit_events.action`, dan `audit_events.subject_type`. Menambah varian berarti satu
//! migrasi yang mengganti constraint-nya, di commit yang sama dengan enum Rust-nya (dijaga
//! `tests/models/{permission,enums}.rs`).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &[&str] = &[
    "SET lock_timeout = '5s'",
    // slug disimpan huruf kecil oleh model; unik case-insensitive lewat lower(slug) (rules §4).
    // created_by ON DELETE SET NULL: workspace tetap ada walau user pembuatnya dihapus.
    "CREATE TABLE workspaces (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        name text NOT NULL,
        slug text NOT NULL,
        require_2fa_at timestamptz,
        deletion_scheduled_at timestamptz,
        created_by uuid REFERENCES users (id) ON DELETE SET NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    )",
    "CREATE UNIQUE INDEX workspaces_slug_lower_key ON workspaces (lower(slug))",
    // key terisi hanya untuk role bawaan; role custom (Task 10) key NULL.
    // ON DELETE CASCADE ke workspaces: purge workspace (Task 13) bergantung padanya.
    "CREATE TABLE roles (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        key text CHECK (key IN ('owner', 'admin', 'member', 'viewer')),
        name text NOT NULL,
        permissions text[] NOT NULL DEFAULT '{}' CHECK (permissions <@ ARRAY[
            'manage_workspace', 'manage_members', 'manage_roles', 'manage_teams',
            'create_projects', 'view_audit_log', 'administer_project', 'edit_project',
            'view_project'
        ]::text[]),
        builtin boolean NOT NULL DEFAULT false,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        -- Role bawaan tepat yang punya key; role custom tanpa key.
        CHECK (builtin = (key IS NOT NULL))
    )",
    "CREATE UNIQUE INDEX roles_workspace_id_name_lower_key ON roles (workspace_id, lower(name))",
    // Target FK komposit (role_id, workspace_id) dari workspace_members dan invitations.
    "ALTER TABLE roles ADD CONSTRAINT roles_id_workspace_id_key UNIQUE (id, workspace_id)",
    // Satu role bawaan per key per workspace.
    "CREATE UNIQUE INDEX roles_workspace_id_key_key ON roles (workspace_id, key) \
     WHERE key IS NOT NULL",
    // ON DELETE CASCADE ke workspaces: purge workspace (Task 13) bergantung padanya.
    "CREATE TABLE job_titles (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        name text NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    )",
    "CREATE UNIQUE INDEX job_titles_workspace_id_name_lower_key \
     ON job_titles (workspace_id, lower(name))",
    // Target FK komposit (job_title_id, workspace_id).
    "ALTER TABLE job_titles ADD CONSTRAINT job_titles_id_workspace_id_key \
     UNIQUE (id, workspace_id)",
    // workspace_id ON DELETE CASCADE: purge workspace (Task 13) bergantung padanya.
    // user_id ON DELETE CASCADE: keanggotaan tidak berarti tanpa user-nya.
    // Role dan jabatan lewat FK komposit (…, workspace_id): database menolak role atau jabatan
    // milik workspace lain.
    // role FK tanpa ON DELETE (NO ACTION): role yang masih dipakai tidak bisa dihapus (Task 10
    // `role-in-use`). NO ACTION, bukan RESTRICT: dicek di akhir statement, sehingga cascade
    // dari workspaces yang menghapus member dan role sekaligus tetap lolos.
    // jabatan ON DELETE SET NULL (job_title_id): hapus jabatan membuat member tanpa jabatan
    // (Task 10); hanya kolom itu yang di-NULL-kan, workspace_id tetap.
    "CREATE TABLE workspace_members (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        role_id uuid NOT NULL,
        job_title_id uuid,
        joined_at timestamptz NOT NULL DEFAULT now(),
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (workspace_id, user_id),
        FOREIGN KEY (role_id, workspace_id) REFERENCES roles (id, workspace_id),
        FOREIGN KEY (job_title_id, workspace_id) REFERENCES job_titles (id, workspace_id)
            ON DELETE SET NULL (job_title_id)
    )",
    // Daftar workspace milik user (switcher, `/`).
    "CREATE INDEX workspace_members_user_id_idx ON workspace_members (user_id)",
    "CREATE INDEX workspace_members_role_id_idx ON workspace_members (role_id)",
    // workspace_id ON DELETE CASCADE: purge workspace (Task 13) bergantung padanya.
    // Role dan jabatan: FK komposit seperti workspace_members, dengan aturan hapus yang sama.
    // invited_by ON DELETE SET NULL: undangan tetap berlaku walau pengundangnya dihapus.
    "CREATE TABLE invitations (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        email text NOT NULL,
        role_id uuid NOT NULL,
        job_title_id uuid,
        token_digest text NOT NULL UNIQUE,
        invited_by uuid REFERENCES users (id) ON DELETE SET NULL,
        expires_at timestamptz NOT NULL,
        accepted_at timestamptz,
        revoked_at timestamptz,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        FOREIGN KEY (role_id, workspace_id) REFERENCES roles (id, workspace_id),
        FOREIGN KEY (job_title_id, workspace_id) REFERENCES job_titles (id, workspace_id)
            ON DELETE SET NULL (job_title_id)
    )",
    // Satu undangan aktif per email per workspace.
    "CREATE UNIQUE INDEX invitations_workspace_id_email_lower_active_key \
     ON invitations (workspace_id, lower(email)) \
     WHERE accepted_at IS NULL AND revoked_at IS NULL",
    "CREATE INDEX invitations_role_id_idx ON invitations (role_id)",
    // Append-only: tanpa updated_at; aplikasi tidak punya jalur update/delete (rules "Audit log").
    // workspace_id ON DELETE CASCADE: purge workspace (Task 13) bergantung padanya.
    // actor_id ON DELETE SET NULL: jejak tetap ada walau user pelakunya dihapus.
    "CREATE TABLE audit_events (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        actor_id uuid REFERENCES users (id) ON DELETE SET NULL,
        action text NOT NULL CHECK (action IN (
            'workspace.created', 'workspace.updated', 'workspace.require_2fa_changed',
            'workspace.deletion_scheduled', 'workspace.deletion_cancelled',
            'member.invited', 'member.joined', 'member.role_changed', 'member.removed',
            'member.left',
            'role.created', 'role.updated', 'role.deleted',
            'team.created', 'team.updated', 'team.deleted', 'team.member_added',
            'team.member_removed',
            'project.created', 'project.updated', 'project.archived',
            'project.access_granted', 'project.access_changed', 'project.access_revoked',
            'repo.added', 'repo.updated', 'repo.removed'
        )),
        subject_type text NOT NULL CHECK (subject_type IN (
            'workspace', 'member', 'role', 'job_title', 'team', 'project', 'project_grant',
            'repo', 'invitation'
        )),
        subject_id uuid NOT NULL,
        changes jsonb NOT NULL DEFAULT '{}',
        request_id text,
        created_at timestamptz NOT NULL DEFAULT now()
    )",
    // Halaman audit (Task 13): terbaru dulu, keyset (created_at, id).
    "CREATE INDEX audit_events_workspace_id_created_at_id_idx \
     ON audit_events (workspace_id, created_at DESC, id DESC)",
];

const DOWN: &[&str] = &[
    "SET lock_timeout = '5s'",
    "DROP TABLE IF EXISTS audit_events",
    "DROP TABLE IF EXISTS invitations",
    "DROP TABLE IF EXISTS workspace_members",
    "DROP TABLE IF EXISTS job_titles",
    "DROP TABLE IF EXISTS roles",
    "DROP TABLE IF EXISTS workspaces",
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
