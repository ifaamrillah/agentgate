//! Instal CLI (M12, publik): `GET /install` (halaman petunjuk), `GET /install.sh` dan
//! `GET /install.ps1` (redirect ke installer di rilis CLI, `settings.system.cli_release_url`;
//! kosong → 404). Dengan begitu perintah instal selalu memakai URL server ini:
//! `curl -fsSL https://server/install.sh | sh`.

use loco_rs::prelude::*;

use crate::{common::settings::Settings, views};

/// Nama file installer buatan `dist` (`dist-workspace.toml`, paket `agentgate-cli`).
const SHELL_INSTALLER: &str = "agentgate-cli-installer.sh";
const POWERSHELL_INSTALLER: &str = "agentgate-cli-installer.ps1";

/// URL installer di rilis, atau `None` bila rilis belum dikonfigurasi.
#[must_use]
pub fn installer_url(release_url: &str, file: &str) -> Option<String> {
    let base = release_url.trim().trim_end_matches('/');
    (!base.is_empty()).then(|| format!("{base}/{file}"))
}

#[debug_handler]
async fn page(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
) -> Result<Response> {
    let settings = Settings::from_ctx(&ctx)?;
    views::system::install(
        &v,
        &settings.public_base_url,
        !settings.system.cli_release_url.trim().is_empty(),
        &settings.system.cli_min_version,
    )
}

fn redirect_to(ctx: &AppContext, file: &str) -> Result<Response> {
    let settings = Settings::from_ctx(ctx)?;
    let to = installer_url(&settings.system.cli_release_url, file).ok_or(Error::NotFound)?;
    format::redirect(&to)
}

#[debug_handler]
async fn shell_script(State(ctx): State<AppContext>) -> Result<Response> {
    redirect_to(&ctx, SHELL_INSTALLER)
}

#[debug_handler]
async fn powershell_script(State(ctx): State<AppContext>) -> Result<Response> {
    redirect_to(&ctx, POWERSHELL_INSTALLER)
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/install", get(page))
        .add("/install.sh", get(shell_script))
        .add("/install.ps1", get(powershell_script))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_url_needs_a_release() {
        assert_eq!(installer_url("  ", SHELL_INSTALLER), None);
        assert_eq!(
            installer_url("https://x.test/dl/", SHELL_INSTALLER).as_deref(),
            Some("https://x.test/dl/agentgate-cli-installer.sh")
        );
    }
}
