//! Tiket & board (M2).
//!
//! - Project (`ProjectAccess`): board, daftar, tiket baru, sampah, opsi orang project.
//! - Tiket (`TicketAccess`, `/w/{slug}/t/{code}`): detail, aktivitas, ubah, status, komentar,
//!   relasi, lampiran, ikuti, buang ke sampah.
//! - Workspace (`Tenant`): Tiket saya, kotak masuk, pencarian.
//! - Pengaturan (`Require<ManageTicketSettings>`): tipe tiket, field custom, kode tiket; override
//!   kode per project (`ProjectAdmin`).
//!
//! Izin dicek extractor sebelum body dibaca (rules §6), lalu dibaca ulang service di dalam
//! transaksi. Tanpa izin, tiket di sampah, atau workspace lain → 404.
use std::collections::HashSet;

use axum::extract::DefaultBodyLimit;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};
use uuid::Uuid;

mod artifact;
mod board;
mod drafts;
mod inbox;
mod pulls;
mod settings;
mod stage;
mod ticket;
mod workflow;

use super::extractors::{ProjectAccess, Tenant};
use crate::{
    models::{projects, ticket_attachments, ticket_types, tickets, users, workspaces::WorkspaceId},
    services::workspaces::Acting,
    views::{
        projects::{ProjectPaths, ProjectSummary},
        tickets::Lookup,
    },
};

fn acting(tenant: &Tenant, request_id: Option<String>) -> Acting<'_> {
    Acting {
        workspace: &tenant.workspace,
        user: &tenant.user,
        role: &tenant.role,
        request_id,
    }
}

fn project_paths(access: &ProjectAccess) -> ProjectPaths {
    ProjectPaths::new(&access.tenant.workspace.slug, &access.project.key)
}

fn summary(access: &ProjectAccess) -> ProjectSummary {
    ProjectSummary::new(&access.project, access.role.label(), &project_paths(access))
}

/// Tipe dan orang yang dirujuk `tickets` (penerima, pelapor) dan `extra_people`.
async fn lookup<C: ConnectionTrait>(
    db: &C,
    tenant: &Tenant,
    tickets: &[tickets::Model],
    extra_people: &[Uuid],
) -> Result<Lookup, DbErr> {
    let wid = tenant.workspace_id();
    let types = ticket_types::Model::all_in(db, wid).await?;
    let ids: HashSet<Uuid> = tickets
        .iter()
        .flat_map(|t| [t.assignee_id, t.reporter_id])
        .flatten()
        .chain(extra_people.iter().copied())
        .collect();
    let people = users_by_id(db, ids).await?;
    let ticket_ids: Vec<Uuid> = tickets.iter().map(|t| t.id).collect();
    let prs = if ticket_ids.is_empty() {
        Vec::new()
    } else {
        crate::models::pull_requests::Entity::find()
            .filter(crate::models::pull_requests::Column::WorkspaceId.eq(wid.get()))
            .filter(crate::models::pull_requests::Column::TicketId.is_in(ticket_ids))
            .all(db)
            .await?
    };
    Ok(Lookup::new(&tenant.workspace.slug, &types, &people).with_prs(&prs))
}

async fn users_by_id<C: ConnectionTrait>(
    db: &C,
    ids: impl IntoIterator<Item = Uuid>,
) -> Result<Vec<users::Model>, DbErr> {
    let ids: Vec<Uuid> = ids.into_iter().collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    users::Entity::find()
        .filter(users::Column::Id.is_in(ids))
        .all(db)
        .await
}

/// Nama dan key project untuk `ids` di workspace ini.
async fn project_names<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ids: impl IntoIterator<Item = Uuid>,
) -> Result<Vec<projects::Model>, DbErr> {
    let ids: Vec<Uuid> = ids.into_iter().collect();
    projects::Entity::find()
        .filter(projects::Column::WorkspaceId.eq(wid.get()))
        .filter(projects::Column::Id.is_in(ids))
        .all(db)
        .await
}

/// Tujuan kembali setelah POST: hanya path di dalam workspace ini (`/w/{slug}/...`).
fn safe_return(slug: &str, raw: Option<&str>, fallback: &str) -> String {
    let prefix = format!("/w/{slug}/");
    match raw.map(str::trim) {
        Some(path)
            if path.starts_with(&prefix)
                && !path.contains("//")
                && !path.contains('\\')
                && !path.chars().any(char::is_control) =>
        {
            path.to_string()
        }
        _ => fallback.to_string(),
    }
}

/// Query tiket dari keadaan tabel (cari, filter status/prioritas/tipe/orang, sort, halaman).
fn ticket_query(
    state: &crate::views::datatable::TableState,
    user_id: Uuid,
    project_ids: Vec<Uuid>,
) -> tickets::TicketQuery {
    use crate::models::tickets::{PersonFilter, TicketPriority, TicketSort, TicketStatus};
    let people = state
        .filter("who")
        .iter()
        .filter_map(|v| match v.as_str() {
            "me" => Some(PersonFilter::AssignedTo(user_id)),
            "none" => Some(PersonFilter::Unassigned),
            id => Uuid::parse_str(id).ok().map(PersonFilter::AssignedTo),
        })
        .collect();
    tickets::TicketQuery {
        project_ids,
        q: state.query().map(str::to_string),
        statuses: state
            .filter("status")
            .iter()
            .filter_map(|s| TicketStatus::parse(s))
            .collect(),
        priorities: state
            .filter("priority")
            .iter()
            .filter_map(|s| TicketPriority::parse(s))
            .collect(),
        type_ids: state
            .filter("type")
            .iter()
            .filter_map(|s| Uuid::parse_str(s).ok())
            .collect(),
        people,
        sort: Some((
            match state.sort {
                "code" => TicketSort::Code,
                "title" => TicketSort::Title,
                "status" => TicketSort::Status,
                "priority" => TicketSort::Priority,
                _ => TicketSort::Updated,
            },
            state.desc,
        )),
        offset: state.offset(),
        limit: state.per,
    }
}

/// Opsi filter status dan prioritas (kunci Fluent).
fn status_priority_options(
    table: crate::views::datatable::TableView,
) -> crate::views::datatable::TableView {
    use crate::views::{
        datatable::FilterOption,
        tickets::{priority_options, status_options},
    };
    table
        .with_filter_options(
            "status",
            status_options()
                .iter()
                .map(|s| FilterOption::key(s.value, s.label_key))
                .collect(),
        )
        .with_filter_options(
            "priority",
            priority_options()
                .iter()
                .map(|p| FilterOption::key(p.value, p.label_key))
                .collect(),
        )
}

/// Satu halaman tiket menurut `query`; halaman di luar jangkauan dijepit ke halaman terakhir.
async fn table_page<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    spec: &crate::views::datatable::TableSpec,
    state: &crate::views::datatable::TableState,
    base: &str,
    query: tickets::TicketQuery,
) -> Result<(Vec<tickets::Model>, crate::views::datatable::TableView), DbErr> {
    let (rows, total) = tickets::Model::page(db, wid, &query).await?;
    let table = crate::views::datatable::TableView::new(spec, state, base, total);
    if table.page == state.page {
        return Ok((rows, table));
    }
    let clamped = tickets::TicketQuery {
        offset: (table.page - 1) * table.per,
        ..query
    };
    Ok((tickets::Model::page(db, wid, &clamped).await?.0, table))
}

/// Batas body unggah lampiran: ukuran berkas maksimal ditambah ruang multipart.
const ATTACHMENT_BODY_LIMIT: usize = ticket_attachments::MAX_BYTES + 512 * 1024;

pub fn routes() -> Routes {
    let routes = Routes::new()
        // Project
        .add("/w/{slug}/p/{key}/board", get(board::board))
        .add("/w/{slug}/p/{key}/list", get(board::list))
        .add(
            "/w/{slug}/p/{key}/tickets/new",
            get(board::new).post(board::create),
        )
        .add("/w/{slug}/p/{key}/people", get(board::people))
        .add("/w/{slug}/p/{key}/trash", get(board::trash))
        .add("/w/{slug}/p/{key}/drafts", get(drafts::index))
        .add("/w/{slug}/p/{key}/drafts/create", post(drafts::create))
        .add("/w/{slug}/p/{key}/drafts/{id}", post(drafts::update))
        .add("/w/{slug}/p/{key}/drafts/{id}/delete", post(drafts::delete))
        .add("/w/{slug}/p/{key}/trash/{id}/restore", post(board::restore))
        .add("/w/{slug}/p/{key}/trash/{id}/purge", post(board::purge))
        .add(
            "/w/{slug}/p/{key}/settings/ticket-code",
            get(settings::project_code).post(settings::save_project_code),
        )
        // Tiket
        .add("/w/{slug}/t/{code}", get(ticket::show).post(ticket::update))
        .add("/w/{slug}/t/{code}/activity", get(ticket::activity))
        .add("/w/{slug}/t/{code}/pr", get(pulls::index))
        .add("/w/{slug}/t/{code}/status", post(ticket::status))
        .add("/w/{slug}/t/{code}/comments", post(ticket::comment))
        .add(
            "/w/{slug}/t/{code}/comments/{id}",
            post(ticket::edit_comment),
        )
        .add(
            "/w/{slug}/t/{code}/comments/{id}/delete",
            post(ticket::delete_comment),
        )
        .add("/w/{slug}/t/{code}/links", post(ticket::link))
        .add("/w/{slug}/t/{code}/links/{id}/delete", post(ticket::unlink))
        .add(
            "/w/{slug}/t/{code}/attachments",
            post(ticket::upload).layer(DefaultBodyLimit::max(ATTACHMENT_BODY_LIMIT)),
        )
        .add("/w/{slug}/t/{code}/attachments/{id}", get(ticket::download))
        .add(
            "/w/{slug}/t/{code}/attachments/{id}/delete",
            post(ticket::remove_attachment),
        )
        .add("/w/{slug}/t/{code}/watch", post(ticket::watch))
        .add("/w/{slug}/t/{code}/stage/complete", post(stage::complete))
        .add("/w/{slug}/t/{code}/stage/approve", post(stage::approve))
        .add("/w/{slug}/t/{code}/stage/reject", post(stage::reject))
        .add("/w/{slug}/t/{code}/stage/decide", post(stage::decide))
        .add(
            "/w/{slug}/t/{code}/stage/admin-move",
            post(stage::admin_move),
        )
        .add("/w/{slug}/t/{code}/stage/people", post(stage::people))
        .add(
            "/w/{slug}/t/{code}/artifacts",
            get(artifact::index).post(artifact::create),
        )
        .add("/w/{slug}/t/{code}/artifacts/{id}", post(artifact::save))
        .add(
            "/w/{slug}/t/{code}/artifacts/{id}/edit",
            get(artifact::edit),
        )
        .add(
            "/w/{slug}/t/{code}/artifacts/{id}/compare",
            get(artifact::compare),
        )
        .add("/w/{slug}/t/{code}/delete", post(ticket::trash))
        // Workspace
        .add("/w/{slug}/my", get(inbox::my))
        .add("/w/{slug}/inbox", get(inbox::inbox))
        .add("/w/{slug}/inbox/read-all", post(inbox::read_all))
        .add("/w/{slug}/inbox/{id}/read", post(inbox::read))
        .add("/w/{slug}/search", get(inbox::search))
        // Pengaturan workspace
        .add("/w/{slug}/settings/ticket-types", get(settings::types))
        .add(
            "/w/{slug}/settings/ticket-types/new",
            get(settings::new_type).post(settings::create_type),
        )
        .add(
            "/w/{slug}/settings/ticket-types/{id}",
            get(settings::edit_type).post(settings::update_type),
        )
        .add(
            "/w/{slug}/settings/ticket-types/{id}/delete",
            post(settings::delete_type),
        )
        .add("/w/{slug}/settings/fields", post(settings::create_field))
        .add(
            "/w/{slug}/settings/fields/{id}",
            get(settings::edit_field).post(settings::update_field),
        )
        .add(
            "/w/{slug}/settings/fields/{id}/delete",
            post(settings::delete_field),
        )
        .add(
            "/w/{slug}/settings/ticket-code",
            get(settings::code).post(settings::save_code),
        );
    workflow::add(routes)
}

#[cfg(test)]
mod tests {
    use super::safe_return;

    #[test]
    fn return_path_stays_in_workspace() {
        assert_eq!(
            safe_return("acme", Some("/w/acme/p/K/board"), "/x"),
            "/w/acme/p/K/board"
        );
        for bad in [
            "https://evil.example",
            "/w/other/p/K",
            "//evil.example",
            "/w/acme//evil",
            "/w/acme/\\evil",
        ] {
            assert_eq!(safe_return("acme", Some(bad), "/x"), "/x", "{bad}");
        }
        assert_eq!(safe_return("acme", None, "/x"), "/x");
    }
}
