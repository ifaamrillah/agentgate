//! Tiket & board (M2): buat/ubah, board dan daftar, relasi `blocks`, komentar dan @mention,
//! lampiran, sampah, pencarian, kotak masuk, pengaturan tipe/field/kode, dan isolasi akses.

mod access;
mod attachments;
mod blocks;
mod comments;
mod drafts;
mod flow;
mod manual_evidence;
mod mockup;
mod pages;
mod search;
mod settings;
pub mod stages;
mod trash;
mod workflows;

use agentgate_server::models::{tickets, users, workspaces};
use loco_rs::{TestServer, app::AppContext, auth::cookies::Cookie};

use crate::support::{a_project, a_user, a_workspace, sign_in_as};

/// Workspace `acme` milik `owner` dengan project `KSR`.
pub struct Fixture {
    pub owner: users::Model,
    pub workspace: workspaces::Model,
    pub cookie: Cookie<'static>,
}

pub async fn fixture(request: &TestServer, ctx: &AppContext) -> Fixture {
    let owner = a_user().name("Olivia Owner").create(ctx).await;
    let workspace = a_workspace(&owner).slug("acme").create(ctx).await;
    a_project(ctx, &workspace, "KSR").await;
    let cookie = sign_in_as(request, &owner).await;
    Fixture {
        owner,
        workspace,
        cookie,
    }
}

/// Buat tiket lewat form dan kembalikan kodenya.
pub async fn create_ticket(
    request: &TestServer,
    cookie: &Cookie<'static>,
    fields: &[(&str, &str)],
) -> String {
    let res = request
        .post("/w/acme/p/KSR/tickets/new")
        .add_cookie(cookie.clone())
        .form(fields)
        .await;
    assert_eq!(res.status_code(), 303, "{}", res.text());
    let location = res.header("location").to_str().unwrap().to_string();
    location
        .strip_prefix("/w/acme/t/")
        .expect("redirect ke halaman tiket")
        .to_string()
}

/// Tiket dengan kode `code` di workspace fixture.
pub async fn ticket(ctx: &AppContext, workspace: &workspaces::Model, code: &str) -> tickets::Model {
    tickets::Model::find_by_code(&ctx.db, workspace.workspace_id(), code)
        .await
        .unwrap()
        .expect("tiket ada")
        .0
}
