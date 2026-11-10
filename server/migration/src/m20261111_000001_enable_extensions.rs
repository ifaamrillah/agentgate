//! Extension Postgres yang dibutuhkan sejak awal: `vector` (pgvector) untuk embedding
//! code graph dan knowledge. Database tanpa pgvector harus gagal di sini, bukan di fitur
//! pertama yang memakai vektor.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_unprepared("SET lock_timeout = '5s'").await?;
        db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS vector")
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared("DROP EXTENSION IF EXISTS vector")
            .await?;
        Ok(())
    }
}
