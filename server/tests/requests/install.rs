use agentgate_server::app::App;
use loco_rs::testing::prelude::*;

/// Halaman instal CLI publik (tanpa login): perintah instal memakai URL server ini, langkah
/// berikutnya, dan versi minimum.
#[tokio::test]
async fn install_page_is_public_and_points_at_this_server() {
    request::<App, _, _>(|request, ctx| async move {
        let base = agentgate_server::common::settings::Settings::from_ctx(&ctx)
            .unwrap()
            .public_base_url;
        let res = request.get("/install").await;
        assert_eq!(res.status_code(), 200);
        let body = res.text();
        assert!(body.contains("data-install-page"));
        assert!(
            body.contains(&format!("curl -fsSL {base}/install.sh | sh")),
            "{body}"
        );
        assert!(body.contains(&format!("irm {base}/install.ps1 | iex")));
        assert!(body.contains(&format!("agentgate login --server {base}")));
        assert!(body.contains("0.1.0"));
        assert!(body.contains("sha256"));
    })
    .await;
}

/// `/install.sh` dan `/install.ps1` mengarah ke installer di rilis CLI.
#[tokio::test]
async fn installer_scripts_redirect_to_the_release() {
    request::<App, _, _>(|request, _ctx| async move {
        for (path, file) in [
            ("/install.sh", "agentgate-cli-installer.sh"),
            ("/install.ps1", "agentgate-cli-installer.ps1"),
        ] {
            let res = request.get(path).await;
            assert!(res.status_code().is_redirection(), "{path}");
            assert_eq!(
                res.header("location").to_str().unwrap(),
                format!("https://github.com/agentgate/agentgate/releases/latest/download/{file}")
            );
        }
    })
    .await;
}
