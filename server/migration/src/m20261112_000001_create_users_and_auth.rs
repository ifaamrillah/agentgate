//! Tabel identitas milik user (bukan milik workspace, jadi tanpa `workspace_id`): `users`,
//! token email sekali pakai, penghitung rate limit login, identitas OAuth, dan kode pemulihan
//! 2FA. Kolom TOTP di `users` dibuat sekarang supaya Task 2FA tidak butuh migrasi `users` lagi.
//! Tabel sesi (`loco_sessions`, `loco_remember_tokens`) bukan milik migrasi ini: Loco
//! membuatnya sendiri (rules §7).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &[&str] = &[
    "SET lock_timeout = '5s'",
    // password_hash NULL: user yang hanya login lewat OAuth tidak punya password lokal.
    // totp_secret_encrypted: nonce + ciphertext XChaCha20-Poly1305 (crypto::encrypt).
    // totp_last_step: langkah TOTP terakhir yang diterima, menolak kode yang diputar ulang.
    "CREATE TABLE users (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        email text NOT NULL,
        name text NOT NULL,
        password_hash text,
        email_verified_at timestamptz,
        password_changed_at timestamptz,
        disabled_at timestamptz,
        totp_secret_encrypted bytea,
        totp_enabled_at timestamptz,
        totp_last_step bigint,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    )",
    // Unik case-insensitive; model juga menormalisasi ke huruf kecil (rules §4).
    "CREATE UNIQUE INDEX users_email_lower_key ON users (lower(email))",
    // ON DELETE CASCADE: token tidak berarti apa-apa tanpa user-nya.
    "CREATE TABLE user_tokens (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        purpose text NOT NULL CHECK (purpose IN ('password_reset', 'email_verification')),
        token_digest text NOT NULL UNIQUE,
        expires_at timestamptz NOT NULL,
        used_at timestamptz,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    )",
    // issue() membatalkan token lama per (user, purpose).
    "CREATE INDEX user_tokens_user_id_purpose_idx ON user_tokens (user_id, purpose)",
    // Pembersihan harian token kedaluwarsa (`services::cleanup`).
    "CREATE INDEX user_tokens_expires_at_idx ON user_tokens (expires_at)",
    // key: `email:<lower>` atau `ip:<addr>`; tanpa FK karena email tak terdaftar ikut dikunci.
    "CREATE TABLE login_throttles (
        key text PRIMARY KEY,
        failures integer NOT NULL DEFAULT 0,
        window_started_at timestamptz NOT NULL,
        locked_until timestamptz,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    )",
    // ON DELETE CASCADE: identitas OAuth hanya penaut ke user. email NULL: provider bisa
    // tidak memberikan email (mis. email GitHub privat).
    "CREATE TABLE identities (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        provider text NOT NULL CHECK (provider IN ('google', 'github')),
        provider_uid text NOT NULL,
        email text,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (provider, provider_uid)
    )",
    "CREATE INDEX identities_user_id_idx ON identities (user_id)",
    // ON DELETE CASCADE: kode pemulihan hanya berlaku untuk user-nya.
    "CREATE TABLE recovery_codes (
        id uuid PRIMARY KEY DEFAULT uuidv7(),
        user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        code_digest text NOT NULL,
        used_at timestamptz,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (user_id, code_digest)
    )",
];

const DOWN: &[&str] = &[
    "SET lock_timeout = '5s'",
    "DROP TABLE IF EXISTS recovery_codes",
    "DROP TABLE IF EXISTS identities",
    "DROP TABLE IF EXISTS login_throttles",
    "DROP TABLE IF EXISTS user_tokens",
    "DROP TABLE IF EXISTS users",
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
