//! `GET /w/{slug}/settings/audit`: audit log workspace (`Require<ViewAuditLog>`). Keyset 50 baris
//! per halaman (`?before=<kursor>`), filter kelompok aksi (`?group=member`). Nilai query yang
//! tidak dikenal diabaikan (halaman pertama tanpa filter), bukan error: link lama atau URL yang
//! diketik tangan tetap membuka halaman.
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    controllers::{
        extractors::{PermissionMarker, Require, Tenant, require::ViewAuditLog},
        shell::app_shell,
    },
    models::{
        audit_events::{self, AuditCursor, AuditGroup},
        users,
    },
    views::{
        self,
        audit::{AuditPageView, AuditRow},
    },
};

pub(super) const PAGE_SIZE: u64 = 50;

/// Isian query. String, bukan enum: nilai tak dikenal diabaikan, bukan 400.
#[derive(Debug, Default, Deserialize)]
pub struct AuditQuery {
    #[serde(default)]
    group: String,
    #[serde(default)]
    before: String,
    /// Kejadian yang detailnya dibuka di panel (id di halaman ini; selain itu baris pertama).
    #[serde(default)]
    event: String,
}

pub(super) fn audit_path(slug: &str) -> String {
    format!("/w/{slug}/settings/audit")
}

/// Tautan di indeks Pengaturan, menurut guard halaman ini.
pub(super) fn settings_link(tenant: &Tenant) -> Option<String> {
    ViewAuditLog::allows(&tenant.role).then(|| audit_path(&tenant.workspace.slug))
}

/// Path halaman audit dengan filter `group` dan kursor `before`.
fn page_href(base: &str, group: Option<AuditGroup>, before: Option<&AuditCursor>) -> String {
    let mut params = Vec::new();
    if let Some(cursor) = before {
        params.push(format!("before={}", cursor.to_param()));
    }
    if let Some(group) = group {
        params.push(format!("group={}", group.as_str()));
    }
    if params.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", params.join("&"))
    }
}

#[debug_handler]
pub(super) async fn index(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ViewAuditLog>,
    Query(query): Query<AuditQuery>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let group = AuditGroup::parse(query.group.trim());
    let before = AuditCursor::parse(query.before.trim());
    let page =
        audit_events::Model::page(&ctx.db, tenant.workspace_id(), group, before, PAGE_SIZE).await?;
    let mut actor_ids: Vec<_> = page.events.iter().filter_map(|e| e.actor_id).collect();
    actor_ids.sort_unstable();
    actor_ids.dedup();
    let names = users::Model::names_by_id(&ctx.db, &actor_ids).await?;
    let base = audit_path(&tenant.workspace.slug);
    let here = page_href(&base, group, before.as_ref());
    let sep = if here.contains('?') { '&' } else { '?' };
    let rows: Vec<AuditRow> = page
        .events
        .iter()
        .map(|e| {
            let actor = e.actor_id.and_then(|id| names.get(&id)).map(String::as_str);
            let mut row = AuditRow::new(e, actor);
            row.href = format!("{here}{sep}event={}", row.id);
            row
        })
        .collect();
    let wanted = query.event.trim();
    let selected = rows
        .iter()
        .find(|r| r.id == wanted)
        .or_else(|| rows.first())
        .map_or("", |r| r.id.as_str());
    let groups = views::audit::group_options(group);
    let shell = app_shell(&ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("audit");
    views::audit::index(
        &v,
        &shell,
        &AuditPageView {
            rows: &rows,
            selected,
            groups: &groups,
            group: groups.iter().find(|g| g.selected).map_or("", |g| g.value),
            action: &base,
            older_href: page.next.as_ref().map(|c| page_href(&base, group, Some(c))),
            newest_href: before.map(|_| page_href(&base, group, None)),
        },
    )
}
