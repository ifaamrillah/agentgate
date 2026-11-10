//! Tunable aplikasi dari bagian `settings:` di `config/*.yaml`, dibaca sekali ke struct bertipe.

use loco_rs::{Error, Result, app::AppContext};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Nama aplikasi yang tampil di judul halaman dan email.
    pub app_name: String,
}

impl Settings {
    /// Deserialize dari nilai `settings` config. Config tanpa `settings` adalah error:
    /// lebih baik gagal boot daripada jalan dengan nilai default yang tidak disengaja.
    pub fn from_json(value: Option<&serde_json::Value>) -> Result<Self> {
        let value = value.ok_or_else(|| Error::string("config is missing `settings`"))?;
        serde_json::from_value(value.clone())
            .map_err(|e| Error::string(&format!("invalid `settings`: {e}")))
    }

    pub fn from_ctx(ctx: &AppContext) -> Result<Self> {
        Self::from_json(ctx.config.settings.as_ref())
    }
}
