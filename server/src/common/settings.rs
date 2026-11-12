//! Tunable aplikasi dari bagian `settings:` di `config/*.yaml`, dibaca sekali ke struct bertipe.
//! Semua key divalidasi sekaligus saat boot (rules §7, §10), bukan gagal satu per satu saat
//! pertama dipakai.

use base64::{Engine, engine::general_purpose::STANDARD};
use loco_rs::{Error, Result, app::AppContext};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Nama aplikasi yang tampil di judul halaman dan email.
    pub app_name: String,
    /// URL publik aplikasi tanpa `/` di akhir, untuk link di email (verifikasi, reset, undangan).
    pub public_base_url: String,
    /// Pengirim (`From`) semua email aplikasi, mis. `Agent Gate <noreply@example.com>`.
    pub mail_from: String,
    pub auth: AuthSettings,
    /// Impor skill pack (opsional di config; tes mengarahkan ke server palsu).
    #[serde(default)]
    pub skills: SkillSettings,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillSettings {
    /// Basis URL tarball repo GitHub: `{base}/{owner}/{repo}/tar.gz/{ref}`.
    pub fetch_base: String,
    /// Batas waktu unduh (detik).
    pub fetch_timeout_seconds: u64,
}

impl Default for SkillSettings {
    fn default() -> Self {
        Self {
            fetch_base: "https://codeload.github.com".to_string(),
            fetch_timeout_seconds: 20,
        }
    }
}

/// Setting autentikasi milik aplikasi. Setting sesi milik Loco ada di `auth.session`.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthSettings {
    pub argon2_memory_kib: u32,
    pub argon2_iterations: u32,
    pub argon2_parallelism: u32,
    /// Batas hash argon2 yang berjalan bersamaan (memori = jumlah × `argon2_memory_kib`).
    pub max_concurrent_hashes: usize,
    /// Lama menunggu izin hash sebelum request dijawab 503.
    pub hash_wait_seconds: u64,
    /// Umur mutlak sesi sejak login, terlepas dari idle `auth.session.ttl`.
    pub absolute_lifetime_hours: i64,
    pub login_max_failures_per_email: i32,
    pub login_max_failures_per_ip: i32,
    pub login_window_minutes: i64,
    pub login_lockout_minutes: i64,
    pub reset_token_minutes: i64,
    pub verification_token_hours: i64,
    pub invitation_days: i64,
    /// Pendaftaran publik lewat `/register`.
    pub registration_open: bool,
    /// Kunci XChaCha20-Poly1305 (base64, 32 byte) untuk secret TOTP. Pakai
    /// [`AuthSettings::encryption_key_bytes`], bukan string ini.
    pub encryption_key: String,
}

impl Settings {
    /// Deserialize dan validasi dari nilai `settings` config. Config tanpa `settings` adalah
    /// error: lebih baik gagal boot daripada jalan dengan nilai default yang tidak disengaja.
    pub fn from_json(value: Option<&serde_json::Value>) -> Result<Self> {
        let value = value.ok_or_else(|| Error::string("config is missing `settings`"))?;
        let settings: Self = serde_json::from_value(value.clone())
            .map_err(|e| Error::string(&format!("invalid `settings`: {e}")))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn from_ctx(ctx: &AppContext) -> Result<Self> {
        Self::from_json(ctx.config.settings.as_ref())
    }

    fn validate(&self) -> Result<()> {
        if self.public_base_url.ends_with('/') {
            return Err(Error::string(
                "invalid `settings.public_base_url`: must not end with `/`",
            ));
        }
        let auth = &self.auth;
        auth.encryption_key_bytes()?;
        argon2::Params::new(
            auth.argon2_memory_kib,
            auth.argon2_iterations,
            auth.argon2_parallelism,
            None,
        )
        .map_err(|e| Error::string(&format!("invalid `settings.auth` argon2 params: {e}")))?;
        if auth.max_concurrent_hashes == 0 {
            return Err(Error::string(
                "invalid `settings.auth.max_concurrent_hashes`: must be at least 1",
            ));
        }
        Ok(())
    }
}

impl AuthSettings {
    /// Kunci enkripsi hasil decode. Gagal bila bukan base64 atau panjangnya bukan 32 byte;
    /// dipanggil saat boot, jadi pemakai setelahnya bisa mengandalkan nilai yang valid.
    pub fn encryption_key_bytes(&self) -> Result<[u8; 32]> {
        STANDARD
            .decode(&self.encryption_key)
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
            .ok_or_else(|| {
                Error::string(
                    "invalid `settings.auth.encryption_key`: expected base64 of exactly 32 bytes",
                )
            })
    }
}

/// `Debug` ditulis tangan supaya `encryption_key` tidak pernah masuk log.
impl std::fmt::Debug for AuthSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthSettings")
            .field("argon2_memory_kib", &self.argon2_memory_kib)
            .field("argon2_iterations", &self.argon2_iterations)
            .field("argon2_parallelism", &self.argon2_parallelism)
            .field("max_concurrent_hashes", &self.max_concurrent_hashes)
            .field("hash_wait_seconds", &self.hash_wait_seconds)
            .field("absolute_lifetime_hours", &self.absolute_lifetime_hours)
            .field(
                "login_max_failures_per_email",
                &self.login_max_failures_per_email,
            )
            .field("login_max_failures_per_ip", &self.login_max_failures_per_ip)
            .field("login_window_minutes", &self.login_window_minutes)
            .field("login_lockout_minutes", &self.login_lockout_minutes)
            .field("reset_token_minutes", &self.reset_token_minutes)
            .field("verification_token_hours", &self.verification_token_hours)
            .field("invitation_days", &self.invitation_days)
            .field("registration_open", &self.registration_open)
            .field("encryption_key", &"[redacted]")
            .finish()
    }
}
