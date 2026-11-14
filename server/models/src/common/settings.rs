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
    /// Connector Jira (M11; opsional di config; tes mengarahkan ke Jira palsu lokal).
    #[serde(default)]
    pub jira: JiraSettings,
    /// Operasi instance (M12): metrik, admin instance, rilis CLI, backup.
    #[serde(default)]
    pub system: SystemSettings,
    /// Code graph di server (R2): clone dangkal dan embedding lokal.
    #[serde(default)]
    pub graph: GraphSettings,
}

/// Code graph di server (R2). Semua opsional.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GraphSettings {
    pub clone: CloneSettings,
    pub embedding: EmbeddingSettings,
}

/// Clone dangkal repo untuk index + statistik git (R2). Gagal → arsip tarball (cadangan).
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CloneSettings {
    pub enabled: bool,
    /// Volume cache clone (`<cache_dir>/<workspace>/<repo>`). Kosong = `<tmp>/agentgate-repos`.
    pub cache_dir: String,
    /// Riwayat yang diambil (`git fetch --shallow-since`), bulan.
    pub history_months: u32,
    /// Batas ukuran satu clone (MB); lewat → clone dihapus dan index memakai tarball.
    pub max_size_mb: u64,
    /// Batas waktu satu `git fetch` (detik).
    pub timeout_seconds: u64,
    /// Ruang disk minimum yang harus tersisa di `cache_dir` sebelum clone (MB).
    pub min_free_mb: u64,
    /// Basis URL clone (`<base>/<owner>/<repo>.git`). Kosong = basis web koneksi provider. Tes
    /// memakai `file://` ke repo bare lokal.
    pub base_url: String,
}

impl Default for CloneSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_dir: String::new(),
            history_months: 12,
            max_size_mb: 1024,
            timeout_seconds: 600,
            min_free_mb: 2048,
            base_url: String::new(),
        }
    }
}

impl CloneSettings {
    /// Direktori cache efektif.
    #[must_use]
    pub fn dir(&self) -> std::path::PathBuf {
        if self.cache_dir.trim().is_empty() {
            std::env::temp_dir().join("agentgate-repos")
        } else {
            std::path::PathBuf::from(self.cache_dir.trim())
        }
    }
}

/// Embedding lokal untuk pencarian semantik code graph (R2).
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EmbeddingSettings {
    pub enabled: bool,
    /// `fastembed` (ONNX lokal, butuh feature Cargo `semantic`) atau `fake` (vektor hash
    /// deterministik untuk tes/dev tanpa model).
    pub provider: String,
    /// Nama model fastembed (mis. `multilingual-e5-small`, `bge-small-en-v1.5`).
    pub model: String,
    /// Volume cache model (diunduh saat start proses, bukan saat request).
    pub cache_dir: String,
    /// Simbol per batch embed.
    pub batch_size: usize,
    /// Batas simbol yang di-embed per repo (repo raksasa dibatasi, sisanya leksikal saja).
    pub max_symbols: usize,
}

impl Default for EmbeddingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: "fastembed".into(),
            model: "multilingual-e5-small".into(),
            cache_dir: String::new(),
            batch_size: 64,
            max_symbols: 50_000,
        }
    }
}

impl EmbeddingSettings {
    /// Direktori cache model efektif.
    #[must_use]
    pub fn dir(&self) -> std::path::PathBuf {
        if self.cache_dir.trim().is_empty() {
            std::env::temp_dir().join("agentgate-models")
        } else {
            std::path::PathBuf::from(self.cache_dir.trim())
        }
    }
}

/// Pengaturan tingkat instance (M12). Semua opsional: kosong = fiturnya mati.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SystemSettings {
    /// Token bearer untuk `/_metrics` (Prometheus). Kosong = endpoint 404.
    pub metrics_token: String,
    /// Email admin instance (dipisah koma) yang boleh membuka Status sistem (`/system`). Bukan
    /// Owner workspace: data instance lintas tenant.
    pub admin_emails: String,
    /// Basis unduhan rilis CLI (mis. `https://github.com/org/agentgate/releases/latest/download`);
    /// `/install.sh` dan `/install.ps1` mengarah ke installer di sana. Kosong = 404.
    pub cli_release_url: String,
    /// Versi CLI minimum yang didukung server (tampil di halaman instal dan Status sistem).
    pub cli_min_version: String,
    /// Tujuan backup Postgres yang tampil di Status sistem (mis. `s3://bucket/postgres`).
    pub backup_target: String,
    /// Jadwal backup yang tampil di Status sistem (mis. `setiap hari 02.00 WIB`).
    pub backup_schedule: String,
    /// DSN Sentry (R1). Kosong = error tracking server dan browser mati.
    pub sentry_dsn: String,
    /// Environment Sentry (mis. `production`). Kosong = environment Loco.
    pub sentry_environment: String,
}

impl SystemSettings {
    /// `email` termasuk admin instance (tanpa beda huruf besar).
    #[must_use]
    pub fn is_admin(&self, email: &str) -> bool {
        let email = email.trim();
        !email.is_empty()
            && self
                .admin_emails
                .split(',')
                .any(|a| a.trim().eq_ignore_ascii_case(email))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JiraSettings {
    /// Batas waktu satu panggilan REST Jira (detik). Unduh lampiran memakai 4× nilai ini.
    pub timeout_seconds: u64,
    /// Izinkan URL Jira `http://` ke host loopback (server Jira palsu saat development/test).
    /// Production: hanya https ke alamat publik (penjaga SSRF yang sama dengan webhook).
    pub allow_loopback: bool,
}

impl Default for JiraSettings {
    fn default() -> Self {
        Self {
            timeout_seconds: 20,
            allow_loopback: false,
        }
    }
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
#[serde(default, deny_unknown_fields)]
pub struct GitSettings {
    /// Basis REST API github.com.
    pub github_api_base: String,
    /// Basis web github.com (tautan repo/PR).
    pub github_web_base: String,
    /// Batas waktu satu panggilan API provider (detik).
    pub timeout_seconds: u64,
    /// Izinkan URL GitHub Enterprise `http://` ke host loopback (server GitHub palsu saat
    /// development/test). Production: hanya https ke alamat publik (penjaga SSRF webhook, M12).
    pub allow_loopback: bool,
    /// GitHub App (R2), cara utama menghubungkan github.com. Kosong = hanya PAT.
    pub app: GithubAppSettings,
}

/// GitHub App milik instance (R2). Dibuat pemilik instance di GitHub (`docs/admin/github-app.md`).
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GithubAppSettings {
    /// App ID (angka di halaman App).
    pub app_id: String,
    /// Slug App (`https://github.com/apps/<slug>`), untuk URL instalasi.
    pub slug: String,
    /// Client ID + secret OAuth App: memastikan user yang memasang benar-benar punya akses ke
    /// instalasi (`GET /user/installations`).
    pub client_id: String,
    pub client_secret: String,
    /// Kunci privat App (PEM, `-----BEGIN RSA PRIVATE KEY-----`), dari env.
    pub private_key: String,
    /// Secret webhook App (satu endpoint `/webhooks/github-app`).
    pub webhook_secret: String,
}

impl GithubAppSettings {
    /// App dikonfigurasi lengkap.
    #[must_use]
    pub fn enabled(&self) -> bool {
        [
            &self.app_id,
            &self.slug,
            &self.client_id,
            &self.client_secret,
            &self.private_key,
            &self.webhook_secret,
        ]
        .iter()
        .all(|v| !v.trim().is_empty())
    }
}

impl std::fmt::Debug for GithubAppSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubAppSettings")
            .field("app_id", &self.app_id)
            .field("slug", &self.slug)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[redacted]")
            .field("private_key", &"[redacted]")
            .field("webhook_secret", &"[redacted]")
            .finish()
    }
}

impl Default for GitSettings {
    fn default() -> Self {
        Self {
            github_api_base: "https://api.github.com".to_string(),
            github_web_base: "https://github.com".to_string(),
            timeout_seconds: 15,
            allow_loopback: false,
            app: GithubAppSettings::default(),
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
