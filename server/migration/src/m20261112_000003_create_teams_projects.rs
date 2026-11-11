//! Tim, project, akses per project, dan repo. Semua tabel milik workspace (`workspace_id NOT
//! NULL`, `ON DELETE CASCADE` ke `workspaces`, seperti migrasi workspaces: purge workspace di
//! Task 13 hanya menghapus baris `workspaces`).
//!
//! Relasi antar-tabel memakai FK komposit `(…, workspace_id)`, sehingga database menolak tim,
//! project, role, atau anggota milik workspace lain. Orang (anggota tim, grant per orang) diacu
//! lewat `(workspace_id, user_id)` ke `workspace_members` dengan `ON DELETE CASCADE`: anggota
//! yang keluar atau dikeluarkan kehilangan keanggotaan tim dan grant langsungnya di statement
//! `DELETE` yang sama (jadi di transaksi yang sama), dan user yang bukan anggota tidak bisa
//! dimasukkan.
//!
//! Himpunan nilai enum dijaga CHECK (rules §4): `repos.provider` (dijaga
//! `tests/models/enums.rs`). Format key project, URL repo, dan panjang teks ada di validator
//! model (CHECK hanya untuk enum).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &[&str] = &[
    "SET lock_timeout = '5s'",
    "CREATE TABLE teams (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        name text NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    )",
    "CREATE UNIQUE INDEX teams_workspace_id_name_lower_key ON teams (workspace_id, lower(name))",
    // Target FK komposit (team_id, workspace_id) dari team_members dan project_grants.
    "ALTER TABLE teams ADD CONSTRAINT teams_id_workspace_id_key UNIQUE (id, workspace_id)",
    // team ON DELETE CASCADE: anggota tim tidak berarti tanpa timnya.
    // (workspace_id, user_id) ON DELETE CASCADE ke workspace_members: anggota tim harus anggota
    // workspace, dan keluar dari workspace menghapus keanggotaan timnya.
    "CREATE TABLE team_members (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        team_id uuid NOT NULL,
        user_id uuid NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (team_id, user_id),
        FOREIGN KEY (team_id, workspace_id) REFERENCES teams (id, workspace_id)
            ON DELETE CASCADE,
        FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id)
            ON DELETE CASCADE
    )",
    // Tim milik user (akses project lewat tim) dan cascade dari workspace_members.
    "CREATE INDEX team_members_workspace_id_user_id_idx ON team_members (workspace_id, user_id)",
    // key disimpan huruf besar oleh model; unik per workspace lewat upper(key), dan URL
    // /w/{slug}/p/{key} dicari lewat ekspresi yang sama.
    // created_by ON DELETE SET NULL: project tetap ada walau user pembuatnya dihapus.
    "CREATE TABLE projects (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        key text NOT NULL,
        name text NOT NULL,
        description text NOT NULL DEFAULT '',
        created_by uuid REFERENCES users (id) ON DELETE SET NULL,
        archived_at timestamptz,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    )",
    "CREATE UNIQUE INDEX projects_workspace_id_key_upper_key \
     ON projects (workspace_id, upper(key))",
    // Target FK komposit (project_id, workspace_id) dari project_grants dan repos.
    "ALTER TABLE projects ADD CONSTRAINT projects_id_workspace_id_key UNIQUE (id, workspace_id)",
    // Satu grant = satu subjek (tim ATAU orang) dengan satu role di satu project.
    // project ON DELETE CASCADE: grant tidak berarti tanpa project-nya.
    // team ON DELETE CASCADE: hapus tim mencabut semua akses project yang diberikan lewat tim itu.
    // (workspace_id, user_id) ON DELETE CASCADE ke workspace_members: grant per orang hanya
    // untuk anggota workspace, dan hilang saat ia keluar atau dikeluarkan.
    // role FK tanpa ON DELETE (NO ACTION), seperti workspace_members: role yang masih dipakai
    // grant tidak bisa dihapus (Task 10 `role-in-use`, lewat error FK yang sama).
    // FK komposit dengan kolom NULL (team_id atau user_id) tidak dicek (MATCH SIMPLE), jadi
    // hanya subjek yang terisi yang divalidasi.
    "CREATE TABLE project_grants (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        project_id uuid NOT NULL,
        team_id uuid,
        user_id uuid,
        role_id uuid NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        CHECK (num_nonnulls(team_id, user_id) = 1),
        FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
            ON DELETE CASCADE,
        FOREIGN KEY (team_id, workspace_id) REFERENCES teams (id, workspace_id)
            ON DELETE CASCADE,
        FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members (workspace_id, user_id)
            ON DELETE CASCADE,
        FOREIGN KEY (role_id, workspace_id) REFERENCES roles (id, workspace_id)
    )",
    // Satu grant per subjek per project.
    "CREATE UNIQUE INDEX project_grants_project_id_team_id_key \
     ON project_grants (project_id, team_id) WHERE team_id IS NOT NULL",
    "CREATE UNIQUE INDEX project_grants_project_id_user_id_key \
     ON project_grants (project_id, user_id) WHERE user_id IS NOT NULL",
    // Cek FK saat role dihapus, cascade saat tim dihapus, dan grant milik user (akses project).
    "CREATE INDEX project_grants_role_id_idx ON project_grants (role_id)",
    "CREATE INDEX project_grants_team_id_idx ON project_grants (team_id) \
     WHERE team_id IS NOT NULL",
    "CREATE INDEX project_grants_workspace_id_user_id_idx ON project_grants (workspace_id, user_id) \
     WHERE user_id IS NOT NULL",
    // project ON DELETE CASCADE: repo tidak berarti tanpa project-nya.
    "CREATE TABLE repos (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        workspace_id uuid NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
        project_id uuid NOT NULL,
        name text NOT NULL,
        provider text NOT NULL CHECK (provider IN ('github', 'gitlab', 'bitbucket', 'other')),
        url text NOT NULL,
        default_branch text NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        FOREIGN KEY (project_id, workspace_id) REFERENCES projects (id, workspace_id)
            ON DELETE CASCADE
    )",
    "CREATE UNIQUE INDEX repos_project_id_name_lower_key ON repos (project_id, lower(name))",
];

const DOWN: &[&str] = &[
    "SET lock_timeout = '5s'",
    "DROP TABLE IF EXISTS repos",
    "DROP TABLE IF EXISTS project_grants",
    "DROP TABLE IF EXISTS projects",
    "DROP TABLE IF EXISTS team_members",
    "DROP TABLE IF EXISTS teams",
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
