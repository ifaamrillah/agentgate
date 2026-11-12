//! UI "Tenang": preferensi tab tampilan project per user, serta warna dan logo project.
//!
//! - `users.project_views`: kunci tampilan project yang disematkan user di bar tab, berurutan
//!   (`views::nav::PROJECT_VIEWS`); kosong = bawaan. Kunci divalidasi di aplikasi, bukan CHECK,
//!   karena daftarnya bertambah tiap milestone.
//! - `projects.color`: warna project (`#rrggbb`); NULL = warna palet dari kunci project.
//! - `projects.logo_*`: logo terunggah. Database menyimpan key storage (rules §8), bukan URL;
//!   ketiga kolom terisi bersama atau kosong bersama.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &[&str] = &[
    "SET lock_timeout = '5s'",
    "ALTER TABLE users ADD COLUMN project_views text[] NOT NULL DEFAULT '{}'",
    "ALTER TABLE projects
        ADD COLUMN color text CHECK (color ~ '^#[0-9a-f]{6}$'),
        ADD COLUMN logo_key text,
        ADD COLUMN logo_content_type text,
        ADD COLUMN logo_size integer,
        ADD CONSTRAINT projects_logo_all_or_none
            CHECK (num_nonnulls(logo_key, logo_content_type, logo_size) IN (0, 3))",
];

const DOWN: &[&str] = &[
    "SET lock_timeout = '5s'",
    "ALTER TABLE projects
        DROP CONSTRAINT IF EXISTS projects_logo_all_or_none,
        DROP COLUMN IF EXISTS logo_size,
        DROP COLUMN IF EXISTS logo_content_type,
        DROP COLUMN IF EXISTS logo_key,
        DROP COLUMN IF EXISTS color",
    "ALTER TABLE users DROP COLUMN IF EXISTS project_views",
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
