//! Tiket → PR & CI (M6, `/w/{slug}/t/{code}/pr`): PR tiket di setiap repo, status PR, dan check
//! CI head commit terbaru. Diisi runner (PR dibuka) dan webhook provider (status/CI).
use agentgate_core::git::{CiState, CodeOutput};
use axum::http::StatusCode;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::{lookup, summary, ticket::tabs};
use crate::{
    controllers::{extractors::TicketAccess, shell::app_shell},
    models::{permission::Permission, pull_requests, repos, ticket_events, ticket_watchers},
    views::{
        self,
        git::{PrView, TicketRef},
        tickets::{BoardPaths, TicketPaths},
    },
};

/// Jumlah PR tiket (tanpa yang ditutup tanpa merge) dan apakah ada CI gagal (badge tab).
pub(super) async fn counts(ctx: &AppContext, access: &TicketAccess) -> Result<(u64, bool)> {
    let prs = pull_requests::Model::of_ticket(
        &ctx.db,
        access.access.tenant.workspace_id(),
        access.ticket.id,
    )
    .await?;
    let live: Vec<_> = prs.iter().filter(|p| p.counts()).collect();
    let failing = live.iter().any(|p| p.ci() == CiState::Failure);
    Ok((live.len() as u64, failing))
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
) -> Result<Response> {
    let tenant = &access.access.tenant;
    let wid = tenant.workspace_id();
    let slug = &tenant.workspace.slug;
    let t = &access.ticket;
    let prs = pull_requests::Model::of_ticket(&ctx.db, wid, t.id).await?;
    let repo_ids: Vec<_> = prs.iter().map(|p| p.repo_id).collect();
    let repo_rows = repos::Entity::find()
        .filter(repos::Column::WorkspaceId.eq(wid.get()))
        .filter(repos::Column::Id.is_in(repo_ids))
        .all(&ctx.db)
        .await?;
    let reference = TicketRef {
        code: t.code.clone(),
        title: t.title.clone(),
        href: views::tickets::ticket_href(slug, &t.code),
    };
    let git = access.access.project.git_settings();
    let can_edit = access.can(Permission::EditProject);
    let paths = TicketPaths::new(slug, &t.code);
    let pulls: Vec<PrView> = prs
        .iter()
        .map(|p| {
            let repo = repo_rows.iter().find(|r| r.id == p.repo_id);
            let mut view = PrView::of(slug, p, repo, Some(reference.clone()));
            if can_edit && crate::services::git::pulls::can_merge_now(&git, p) {
                view.merge_href = Some(format!("{}/pr/{}/merge", paths.show, p.id));
            }
            view
        })
        .collect();
    let events = ticket_events::Model::of_ticket(&ctx.db, wid, t.id).await?;
    let lookup = lookup(&ctx.db, tenant, std::slice::from_ref(t), &[]).await?;
    let shell = app_shell(&ctx, &tenant.user, Some(tenant), Some(&access.access))
        .await?
        .at("project_ticket");
    let project_paths = super::project_paths(&access.access);
    views::tickets::render(
        &v,
        &shell,
        "tickets/pulls.html",
        StatusCode::OK,
        data!({
            "project": summary(&access.access),
            "ticket": lookup.row(t),
            "pulls": pulls,
            "manual": git.code_output == CodeOutput::Manual,
            "git_settings": project_paths.git,
            "can_admin": access.can(Permission::AdministerProject),
            "paths": paths,
            "board": BoardPaths::new(&project_paths.home),
            "tabs": tabs(
                &paths,
                "pr",
                events.len() as u64,
                super::artifact::counts(&ctx, &access).await?,
                counts(&ctx, &access).await?,
            ),
            "stage": super::stage::panel(&ctx, &access).await?,
            "errors": { "dialogs": super::stage::errors_view("", "") },
            "can_edit": access.can(Permission::EditProject),
            "watching": ticket_watchers::Model::user_ids(&ctx.db, wid, t.id)
                .await?
                .contains(&tenant.user.id),
        }),
    )
}

/// Tombol Merge (mode merge manual, R1): merge PR lewat provider, kembali ke tab PR & CI.
#[debug_handler]
pub(super) async fn merge(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    session: Session,
    crate::controllers::extractors::RequestId(request_id): crate::controllers::extractors::RequestId,
    Path((_, _, pr_id)): Path<(String, String, uuid::Uuid)>,
) -> Result<Response> {
    use crate::{controllers::flash, views::flash::Flash};
    let tenant = &access.access.tenant;
    let settings = crate::common::settings::Settings::from_ctx(&ctx)?;
    let acting = crate::services::workspaces::Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    };
    // Hanya PR milik tiket ini.
    let belongs = pull_requests::Model::of_ticket(&ctx.db, tenant.workspace_id(), access.ticket.id)
        .await?
        .iter()
        .any(|p| p.id == pr_id);
    if !belongs {
        return Err(Error::NotFound);
    }
    let message =
        match crate::services::git::pulls::merge_now(&ctx.db, &settings, &acting, pr_id).await {
            Ok(_) => Flash::success("pr-flash-merged"),
            Err(crate::services::tickets::TicketFailure::Invalid(errors)) => {
                let code = errors
                    .errors
                    .values()
                    .flat_map(|es| es.iter().map(|e| e.code.to_string()))
                    .next()
                    .unwrap_or_else(|| "error-generic-title".into());
                Flash::error(&code)
            }
            Err(e) => return Err(e.into()),
        };
    let session = flash::set(session, &message)?;
    let to = format!(
        "{}/pr",
        TicketPaths::new(&tenant.workspace.slug, &access.ticket.code).show
    );
    Ok((session, format::redirect(&to)?).into_response())
}
