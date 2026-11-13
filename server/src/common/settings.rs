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
    /// Basis URL domain pratinjau mockup (M10); host-nya wajib berbeda dari `public_base_url`
    /// supaya mockup tidak pernah berjalan di origin aplikasi.
    pub preview_base_url: String,
    /// Pengirim (`From`) semua email aplikasi, mis. `Agent Gate <noreply@example.com>`.
    pub mail_from: String,
    pub auth: AuthSettings,
    /// Impor skill pack (opsional di config; tes mengarahkan ke server palsu).
    #[serde(default)]
    pub skills: SkillSettings,
    /// Provider git (opsional di config; tes mengarahkan ke GitHub palsu).
    #[serde(default)]
    pub git: GitSettings,
    /// API publik + webhook keluar (M11; opsional di config).
    #[serde(default)]
    pub api: ApiSettings,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiSettings {
    /// Request per menit per token API (header `X-RateLimit-Remaining`, 429 bila lewat).
    pub rate_limit_per_minute: i32,
    /// Batas waktu satu pengiriman webhook (detik).
    pub webhook_timeout_seconds: u64,
    /// Izinkan tujuan webhook `http://` ke host loopback (localhost, 127.0.0.1, ::1). Hanya
    /// development/test; di production semua alamat privat/loopback ditolak (cegah SSRF).
    pub webhook_allow_loopback: bool,
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            rate_limit_per_minute: 600,
            webhook_timeout_seconds: 10,
            webhook_allow_loopback: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitSettings {
    /// Basis REST API github.com.
    pub github_api_base: String,
    /// Basis web github.com (tautan repo/PR).
    pub github_web_base: String,
    /// Batas waktu satu panggilan API provider (detik).
    pub timeout_seconds: u64,
}

impl Default for GitSettings {
    fn default() -> Self {
        Self {
            github_api_base: "https://api.github.com".to_string(),
            github_web_base: "https://github.com".to_string(),
            timeout_seconds: 15,
        }
    }
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
        if self.preview_base_url.ends_with('/') {
            return Err(Error::string(
                "invalid `settings.preview_base_url`: must not end with `/`",
            ));
        }
        if host_of(&self.preview_base_url).is_empty()
            || host_of(&self.preview_base_url) == host_of(&self.public_base_url)
        {
            return Err(Error::string(
                "invalid `settings.preview_base_url`: host must differ from public_base_url",
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

/// `host[:port]` dari URL (`http://a.b:5150/x` → `a.b:5150`), huruf kecil.
#[must_use]
pub fn host_of(url: &str) -> String {
    url.split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

impl Settings {
    /// Host domain pratinjau (`preview.localhost:5150`).
    #[must_use]
    pub fn preview_host(&self) -> String {
        host_of(&self.preview_base_url)
    }

    /// Origin domain pratinjau (`http://preview.localhost:5150`).
    #[must_use]
    pub fn preview_origin(&self) -> String {
        self.preview_base_url.clone()
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
