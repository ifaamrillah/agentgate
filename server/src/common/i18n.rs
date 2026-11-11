//! Loader Fluent satu-satunya (rules §6 i18n): dipakai fungsi `t()` di view engine dan di
//! template email, supaya halaman dan email membaca file `.ftl` yang sama dengan cara yang sama.

use std::sync::Arc;

use fluent_templates::ArcLoader;
use loco_rs::{Error, Result};

const I18N_DIR: &str = "assets/i18n";
// Harus di LUAR `I18N_DIR`: bila di dalamnya, resource terdaftar dua kali dan
// pembuatan bundle gagal ("Failed to add FTL resources to the bundle").
const I18N_SHARED: &str = "assets/shared.ftl";

/// Muat semua file Fluent (locale default `id`).
///
/// # Errors
///
/// File `.ftl` tidak bisa dibaca atau di-parse.
pub fn loader() -> Result<Arc<ArcLoader>> {
    Ok(Arc::new(
        ArcLoader::builder(&I18N_DIR, unic_langid::langid!("id"))
            .shared_resources(Some(&[I18N_SHARED.into()]))
            .customize(|bundle| bundle.set_use_isolating(false))
            .build()
            .map_err(|e| Error::string(&e.to_string()))?,
    ))
}
