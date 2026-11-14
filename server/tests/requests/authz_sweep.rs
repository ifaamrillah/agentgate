//! Audit M12: sapuan otorisasi lintas workspace atas **semua** route `/w/{slug}/…` yang
//! terdaftar (dibaca dari router, jadi route baru otomatis ikut). Anggota workspace lain
//! selalu mendapat 404 (spec M1: "akses lintas workspace menghasilkan 404"), untuk setiap
//! method, dengan parameter yang menunjuk data asli workspace korban.

use agentgate_server::app::App;
use loco_rs::{app::Hooks, testing::prelude::*};

use super::tickets::{create_ticket, fixture};
use crate::support::{a_user, a_workspace, sign_in_as};

fn fill(uri: &str, ticket: &str) -> String {
    let id = uuid::Uuid::now_v7().to_string();
    uri.replace("{slug}", "acme")
        .replace("{key}", "KSR")
        .replace("{code}", ticket)
        .replace("{id}", &id)
        .replace("{pid}", &id)
        .replace("{doc}", "arsitektur")
        .replace("{group}", "teknis")
        .replace("{n}", "1")
        .replace("{panel}", "settings")
        .replace("{provider}", "github")
        .replace("{token}", "x")
}

#[tokio::test]
async fn every_workspace_route_is_404_for_members_of_other_workspaces() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let ticket = create_ticket(&request, &f.cookie, &[("title", "Rahasia")]).await;
        // Orang luar: Owner di workspace-nya sendiri (izin penuh di sana), bukan anggota acme.
        let bob = a_user().create(&ctx).await;
        a_workspace(&bob).slug("milik-bob").create(&ctx).await;
        let cookie = sign_in_as(&request, &bob).await;

        let mut checked = 0;
        let mut leaks = Vec::new();
        for route in App::routes(&ctx).collect() {
            if !route.uri.starts_with("/w/{slug}") {
                continue;
            }
            let path = fill(&route.uri, &ticket);
            for method in &route.actions {
                let res = match method.as_str() {
                    "GET" => request.get(&path).add_cookie(cookie.clone()).await,
                    "POST" => {
                        request
                            .post(&path)
                            .add_cookie(cookie.clone())
                            .form(&[("x", "")])
                            .await
                    }
                    "PUT" => request.put(&path).add_cookie(cookie.clone()).await,
                    "PATCH" => request.patch(&path).add_cookie(cookie.clone()).await,
                    "DELETE" => request.delete(&path).add_cookie(cookie.clone()).await,
                    _ => continue,
                };
                checked += 1;
                if res.status_code() != 404 {
                    leaks.push(format!("{method} {path} → {}", res.status_code()));
                }
            }
        }
        assert!(checked > 200, "route tersapu: {checked}");
        assert!(leaks.is_empty(), "bukan 404:\n{}", leaks.join("\n"));
    })
    .await;
}

/// Tanpa login, semua GET `/w/{slug}/…` diarahkan ke halaman masuk (gate), tidak ada yang 200.
#[tokio::test]
async fn every_workspace_page_requires_login() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let ticket = create_ticket(&request, &f.cookie, &[("title", "Rahasia")]).await;
        let mut leaks = Vec::new();
        for route in App::routes(&ctx).collect() {
            if !route.uri.starts_with("/w/{slug}")
                || !route.actions.contains(&axum::http::Method::GET)
            {
                continue;
            }
            let path = fill(&route.uri, &ticket);
            let res = request.get(&path).await;
            let to_login = res.status_code() == 303
                && res
                    .maybe_header("location")
                    .and_then(|l| l.to_str().ok().map(str::to_string))
                    .is_some_and(|l| l.starts_with("/login"));
            if !to_login {
                leaks.push(format!("GET {path} → {}", res.status_code()));
            }
        }
        assert!(leaks.is_empty(), "tidak ke /login:\n{}", leaks.join("\n"));
    })
    .await;
}
