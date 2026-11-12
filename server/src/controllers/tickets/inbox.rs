//! Halaman tiket tingkat workspace (`Tenant`): Tiket saya, kotak masuk, pencarian. Hanya tiket
//! di project yang boleh dibuka user (`projects::Model::accessible_for`, aturan `project_access`).
use std::collections::HashSet;

use axum::http::StatusCode;
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use super::{lookup, project_names, status_priority_options, table_page, ticket_query};
use crate::{
    controllers::{
        extractors::{IdOr404, Tenant},
        shell::app_shell,
    },
    models::{
        notifications::{self, NotificationKind},
        projects,
        tickets::{self, PersonFilter},
    },
    views::{
        self,
        datatable::{TableSpec, TableState},
        nav::NavItem,
        tickets::{MY_TABLE, NotificationView, SEARCH_TABLE, ticket_href},
    },
};

/// Project yang boleh dibuka user (id, nama, key).
async fn accessible(ctx: &AppContext, tenant: &Tenant) -> Result<Vec<projects::Model>> {
    Ok(projects::Model::accessible_for(&ctx.db, &tenant.accessor()).await?)
}

fn with_tab(base: &str, tab: &str) -> String {
    if tab.is_empty() {
        base.to_string()
    } else {
        format!("{base}?tab={tab}")
    }
}

/// Tabel tiket lintas project untuk Tiket saya dan pencarian.
async fn cross_table(
    ctx: &AppContext,
    tenant: &Tenant,
    spec: &TableSpec,
    state: &TableState,
    base: &str,
    people: Vec<PersonFilter>,
) -> Result<(Vec<views::tickets::TicketRow>, views::datatable::TableView)> {
    let wid = tenant.workspace_id();
    let projects = accessible(ctx, tenant).await?;
    let mut query = ticket_query(
        state,
        tenant.user.id,
        projects.iter().map(|p| p.id).collect(),
    );
    query.people = people;
    let (rows, table) = table_page(&ctx.db, wid, spec, state, base, query).await?;
    let lookup = lookup(&ctx.db, tenant, &rows, &[]).await?.with_projects(
        projects
            .iter()
            .map(|p| (p.id, p.name.as_str(), p.key.as_str())),
    );
    Ok((
        rows.iter().map(|t| lookup.row(t)).collect(),
        status_priority_options(table),
    ))
}

#[debug_handler]
pub(super) async fn my(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    let tab = query
        .iter()
        .find(|(k, _)| k == "tab")
        .map_or("", |(_, v)| v.as_str());
    let (tab, filter) = match tab {
        "reported" => ("reported", PersonFilter::ReportedBy(tenant.user.id)),
        "watching" => ("watching", PersonFilter::WatchedBy(tenant.user.id)),
        _ => ("", PersonFilter::AssignedTo(tenant.user.id)),
    };
    let base_path = format!("/w/{}/my", tenant.workspace.slug);
    let base = with_tab(&base_path, tab);
    let state = MY_TABLE.parse(&query);
    let (tickets, table) =
        cross_table(&ctx, &tenant, &MY_TABLE, &state, &base, vec![filter]).await?;
    let mut table = table;
    if !tab.is_empty() {
        table.hidden.push(("tab".into(), tab.into()));
    }
    let tabs: Vec<NavItem> = [
        ("", "my-tab-assigned"),
        ("reported", "my-tab-reported"),
        ("watching", "my-tab-watching"),
    ]
    .iter()
    .map(|(key, label)| {
        let mut item = NavItem::link(
            if key.is_empty() { "assigned" } else { key },
            label,
            "ticket",
            &with_tab(&base_path, key),
        );
        item.current = *key == tab;
        item
    })
    .collect();
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant), None)
        .await?
        .at("my_tickets");
    views::tickets::render(
        &v,
        &shell,
        "tickets/my.html",
        StatusCode::OK,
        data!({ "table": table, "tickets": tickets, "tabs": tabs }),
    )
}

#[debug_handler]
pub(super) async fn search(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Response> {
    let base = format!("/w/{}/search", tenant.workspace.slug);
    let state = SEARCH_TABLE.parse(&query);
    let searched = state.query().is_some();
    let (tickets, table) = if searched {
        cross_table(&ctx, &tenant, &SEARCH_TABLE, &state, &base, Vec::new()).await?
    } else {
        (
            Vec::new(),
            status_priority_options(views::datatable::TableView::new(
                &SEARCH_TABLE,
                &state,
                &base,
                0,
            )),
        )
    };
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant), None)
        .await?
        .at("search");
    views::tickets::render(
        &v,
        &shell,
        "tickets/search.html",
        StatusCode::OK,
        data!({ "table": table, "tickets": tickets, "searched": searched, "q": state.q }),
    )
}

#[derive(Debug, Default, Deserialize)]
pub struct InboxQuery {
    #[serde(default)]
    tab: String,
    #[serde(default)]
    page: Option<u64>,
}

const INBOX_PER_PAGE: u64 = 30;

#[debug_handler]
pub(super) async fn inbox(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    Query(query): Query<InboxQuery>,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let user_id = tenant.user.id;
    let tab = match query.tab.as_str() {
        "unread" | "mentions" | "decisions" => query.tab.as_str(),
        _ => "",
    };
    let kinds: &[NotificationKind] = if tab == "mentions" {
        &[NotificationKind::Mentioned]
    } else {
        &[]
    };
    let page = query.page.unwrap_or(1).max(1);
    let (rows, total) = notifications::Model::inbox(
        &ctx.db,
        wid,
        user_id,
        kinds,
        tab == "unread",
        (page - 1) * INBOX_PER_PAGE,
        INBOX_PER_PAGE,
    )
    .await?;
    let unread = notifications::Model::unread_count(&ctx.db, wid, user_id).await?;
    let ticket_ids: HashSet<Uuid> = rows.iter().filter_map(|n| n.ticket_id).collect();
    // Hanya tiket aktif di project yang masih boleh dibuka.
    let projects = accessible(&ctx, &tenant).await?;
    let project_ids: HashSet<Uuid> = projects.iter().map(|p| p.id).collect();
    let tickets: Vec<tickets::Model> = {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        tickets::Entity::find()
            .filter(tickets::Column::WorkspaceId.eq(wid.get()))
            .filter(tickets::Column::Id.is_in(ticket_ids))
            .filter(tickets::active())
            .all(&ctx.db)
            .await?
            .into_iter()
            .filter(|t| project_ids.contains(&t.project_id))
            .collect()
    };
    let actors: Vec<Uuid> = rows.iter().filter_map(|n| n.actor_id).collect();
    let names = project_names(&ctx.db, wid, projects.iter().map(|p| p.id)).await?;
    // Butuh keputusan: tiket menunggu review/keputusan pelaku.
    let waiting = crate::services::tickets::stages::awaiting(
        &ctx.db,
        wid,
        user_id,
        &projects.iter().map(|p| p.id).collect::<Vec<_>>(),
    )
    .await?;
    let mut all_tickets = tickets.clone();
    all_tickets.extend(waiting.iter().cloned());
    let tickets = all_tickets;
    let lookup = lookup(&ctx.db, &tenant, &tickets, &actors)
        .await?
        .with_projects(
            names
                .iter()
                .map(|p| (p.id, p.name.as_str(), p.key.as_str())),
        );
    let base = format!("/w/{}/inbox", tenant.workspace.slug);
    let items: Vec<NotificationView> = rows
        .iter()
        .map(|n| {
            let kind = n.kind().unwrap_or(NotificationKind::Commented);
            let kind_value = sea_orm::ActiveEnum::to_value(&kind);
            NotificationView {
                id: n.id,
                label_key: format!("notification-{kind_value}"),
                kind: kind_value,
                actor: lookup.person(n.actor_id),
                ticket: n
                    .ticket_id
                    .and_then(|id| tickets.iter().find(|t| t.id == id))
                    .map(|t| lookup.row(t)),
                at: views::time::wib_datetime(&n.created_at),
                unread: n.read_at.is_none(),
                read_action: format!("{base}/{}/read", n.id),
            }
        })
        .collect();
    let mut decisions = Vec::new();
    for t in &waiting {
        let flow = crate::services::tickets::stages::Flow::load(&ctx.db, wid, t).await?;
        decisions.push(serde_json::json!({
            "ticket": lookup.row(t),
            "stage": flow.rule(&t.stage_key).map_or_else(|| t.stage_key.clone(), |s| s.name.clone()),
            "state": t.stage_state,
            "since": views::time::wib_datetime(&t.stage_entered_at),
            "fails": t.fail_count,
        }));
    }
    let tabs: Vec<NavItem> = [
        ("", "inbox-tab-all", None),
        ("unread", "inbox-tab-unread", Some(unread)),
        (
            "decisions",
            "inbox-tab-decisions",
            Some(waiting.len() as u64),
        ),
        ("mentions", "inbox-tab-mentions", None),
    ]
    .iter()
    .map(|(key, label, count)| {
        let mut item = NavItem::link(
            if key.is_empty() { "all" } else { key },
            label,
            "inbox",
            &with_tab(&base, key),
        );
        if let Some(n) = count {
            item = item.counted(*n, *key == "decisions" && *n > 0);
        }
        item.current = *key == tab;
        item
    })
    .collect();
    let pages = total.div_ceil(INBOX_PER_PAGE).max(1);
    let page_href = |p: u64| {
        let mut pairs: Vec<(&str, String)> = Vec::new();
        if !tab.is_empty() {
            pairs.push(("tab", tab.to_string()));
        }
        pairs.push(("page", p.to_string()));
        format!(
            "{base}?{}",
            serde_urlencoded::to_string(&pairs).unwrap_or_default()
        )
    };
    let shell = app_shell(&ctx, &tenant.user, Some(&tenant), None)
        .await?
        .at("inbox");
    views::tickets::render(
        &v,
        &shell,
        "tickets/inbox.html",
        StatusCode::OK,
        data!({
            "items": items,
            "tab": tab,
            "decisions": decisions,
            "tabs": tabs,
            "total": total,
            "unread": unread,
            "read_all_action": format!("{base}/read-all"),
            "prev_href": (page > 1).then(|| page_href(page - 1)),
            "next_href": (page < pages).then(|| page_href(page + 1)),
            "page": page,
            "pages": pages,
        }),
    )
}

/// Tandai satu notifikasi dibaca lalu buka tiketnya (atau kembali ke kotak masuk).
#[debug_handler]
pub(super) async fn read(
    State(ctx): State<AppContext>,
    tenant: Tenant,
    id: IdOr404<notifications::Entity>,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let notification = {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        notifications::Entity::find_by_id(id.id)
            .filter(notifications::Column::WorkspaceId.eq(wid.get()))
            .filter(notifications::Column::UserId.eq(tenant.user.id))
            .one(&ctx.db)
            .await?
            .ok_or(Error::NotFound)?
    };
    notifications::Model::mark_read(&ctx.db, wid, tenant.user.id, Some(notification.id)).await?;
    let slug = &tenant.workspace.slug;
    let target = match notification.ticket_id {
        Some(ticket_id) => tickets::Model::find_in(&ctx.db, wid, ticket_id)
            .await?
            .filter(|t| !t.is_trashed())
            .map(|t| ticket_href(slug, &t.code)),
        None => None,
    };
    format::redirect(&target.unwrap_or_else(|| format!("/w/{slug}/inbox")))
}

#[debug_handler]
pub(super) async fn read_all(State(ctx): State<AppContext>, tenant: Tenant) -> Result<Response> {
    notifications::Model::mark_read(&ctx.db, tenant.workspace_id(), tenant.user.id, None).await?;
    format::redirect(&format!("/w/{}/inbox", tenant.workspace.slug))
}
