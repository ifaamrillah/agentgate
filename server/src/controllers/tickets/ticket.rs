//! Satu tiket (`TicketAccess`, `/w/{slug}/t/{code}`): detail (Ringkasan), Aktivitas, ubah
//! sebagian, pindah status, komentar, relasi, lampiran, ikuti, dan buang ke sampah.
//!
//! Penolakan isian (termasuk `ticket-blocked`) → 422 halaman detail dengan pesan di bagian yang
//! bersangkutan. Komentar/relasi/lampiran milik tiket lain → 404.
use axum::{
    body::{Body, Bytes},
    extract::Multipart,
    http::{HeaderValue, StatusCode, header},
};
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    acting,
    board::{custom_for_form, form_view, people_options, ticket_form},
    lookup, safe_return, summary,
};
use crate::{
    controllers::{
        extractors::{IdOr404, RequestId, TicketAccess},
        flash,
        shell::app_shell,
    },
    models::{
        custom_fields::FieldKind,
        permission::Permission,
        ticket_attachments, ticket_comments,
        ticket_events::{self, EventKind},
        ticket_links::{self, LinkKind},
        ticket_type_fields, ticket_types, ticket_watchers,
        tickets::{self, TicketStatus},
    },
    services::tickets::{self as service, TicketFailure, create::TicketForm},
    views::{
        self,
        flash::Flash,
        nav::NavItem,
        tickets::{
            AttachmentView, BoardPaths, ChangeView, CommentView, CustomValueView, EventView,
            LinkView, Lookup, TicketFormView, TicketPaths, TypeView, human_size, priority_options,
            priority_view, status_options, status_view, ticket_href,
        },
    },
};

/// Pesan error per bagian halaman detail (kode Fluent; `""` = tanpa).
#[derive(Debug, Default)]
struct PageErrors {
    form: Option<ModelValidationErrors>,
    status: String,
    status_codes: String,
    comment: String,
    comment_body: String,
    link: String,
    attachment: String,
    /// Penolakan aksi stage (kode Fluent) dan dialog yang dibuka lagi.
    stage: String,
    stage_dialog: &'static str,
    /// Flash dari redirect sebelumnya (hanya GET Ringkasan).
    flash: Option<Flash>,
}

fn first_code(errors: &ModelValidationErrors, field: &str) -> String {
    errors
        .errors
        .get(field)
        .and_then(|l| l.first())
        .map(|e| e.code.clone())
        .unwrap_or_default()
}

/// Isian form tiket dari tiket sekarang (dasar ubah sebagian).
fn current_form(t: &tickets::Model, parent_code: Option<&str>) -> TicketForm {
    let view = TicketFormView::of(t, parent_code);
    TicketForm {
        input: tickets::TicketInput {
            title: view.title,
            description: view.description,
            status: view.status,
            priority: view.priority,
            labels: view.labels,
            due_on: view.due_on,
            story_points: view.story_points,
        },
        type_id: view.type_id,
        assignee: view.assignee,
        parent: view.parent,
        custom: t.custom_values().0,
        stage: String::new(),
    }
}

async fn parent_code(ctx: &AppContext, access: &TicketAccess) -> Result<Option<String>> {
    Ok(match access.ticket.parent_id {
        Some(id) => tickets::Model::find_in(&ctx.db, access.access.tenant.workspace_id(), id)
            .await?
            .map(|t| t.code),
        None => None,
    })
}

pub(super) fn tabs(
    paths: &TicketPaths,
    current: &str,
    events: u64,
    artifacts: (u64, u64),
) -> Vec<NavItem> {
    let mut items = vec![
        NavItem::link("summary", "ticket-tab-summary", "file-text", &paths.show),
        NavItem::link(
            "activity",
            "ticket-tab-activity",
            "history",
            &paths.activity,
        )
        .counted(events, false),
        NavItem::link(
            "artifacts",
            "ticket-tab-artifacts",
            "files",
            &paths.artifacts,
        )
        .counted(artifacts.0, artifacts.1 > 0),
    ];
    for item in &mut items {
        item.current = item.key == current;
    }
    items
}

/// Halaman Ringkasan tiket.
async fn render_show(
    ctx: &AppContext,
    v: &TeraView,
    access: &TicketAccess,
    form: Option<&TicketForm>,
    errors: PageErrors,
    status: StatusCode,
) -> Result<Response> {
    let tenant = &access.access.tenant;
    let wid = tenant.workspace_id();
    let t = &access.ticket;
    let slug = &tenant.workspace.slug;
    let paths = TicketPaths::new(slug, &t.code);
    let can_edit = access.can(Permission::EditProject);

    let children = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::ParentId.eq(t.id))
        .filter(tickets::active())
        .order_by_asc(tickets::Column::Number)
        .all(&ctx.db)
        .await?;
    let links = ticket_links::Model::of_ticket(&ctx.db, wid, t.id).await?;
    let other_ids: Vec<Uuid> = links
        .iter()
        .map(|l| {
            if l.from_id == t.id {
                l.to_id
            } else {
                l.from_id
            }
        })
        .chain(t.parent_id)
        .collect();
    let others = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::Id.is_in(other_ids))
        .all(&ctx.db)
        .await?;
    let comments = ticket_comments::Model::of_ticket(&ctx.db, wid, t.id).await?;
    let attachments = ticket_attachments::Model::of_ticket(&ctx.db, wid, t.id).await?;
    let watchers = ticket_watchers::Model::user_ids(&ctx.db, wid, t.id).await?;
    let events = ticket_events::Model::of_ticket(&ctx.db, wid, t.id).await?;
    let mut people: Vec<Uuid> = comments.iter().filter_map(|c| c.author_id).collect();
    people.extend(attachments.iter().filter_map(|a| a.uploader_id));
    people.extend(watchers.iter().copied());
    let mut all = vec![t.clone()];
    all.extend(children.iter().cloned());
    all.extend(others.iter().cloned());
    let lookup = lookup(&ctx.db, tenant, &all, &people).await?;

    let row = lookup.row(t);
    let parent = t
        .parent_id
        .and_then(|id| others.iter().find(|o| o.id == id))
        .map(|p| lookup.row(p));
    let link_views: Vec<LinkView> = links
        .iter()
        .filter_map(|l| {
            let outgoing = l.from_id == t.id;
            let other = others
                .iter()
                .find(|o| o.id == if outgoing { l.to_id } else { l.from_id })?;
            if other.is_trashed() {
                return None;
            }
            let kind = l.kind().ok()?;
            let label_key = match (kind, outgoing) {
                (LinkKind::Blocks, true) => "ticket-link-blocks",
                (LinkKind::Blocks, false) => "ticket-link-blocked-by",
                (LinkKind::Duplicates, true) => "ticket-link-duplicates",
                (LinkKind::Duplicates, false) => "ticket-link-duplicated-by",
                (LinkKind::Relates, _) => "ticket-link-relates",
            };
            Some(LinkView {
                id: l.id,
                label_key,
                blocking: kind == LinkKind::Blocks
                    && !outgoing
                    && other.status().ok() != Some(TicketStatus::Done),
                other: lookup.row(other),
                delete_action: format!("{}/{}/delete", paths.links, l.id),
            })
        })
        .collect();
    let blockers: Vec<String> = link_views
        .iter()
        .filter(|l| l.blocking)
        .map(|l| l.other.code.clone())
        .collect();
    let user_id = tenant.user.id;
    let comment_views: Vec<CommentView> = comments
        .iter()
        .rev()
        .map(|c| CommentView {
            id: c.id,
            author: lookup.person(c.author_id),
            body: c.body.clone(),
            at: views::time::wib_datetime(&c.created_at),
            edited: c.edited_at.is_some(),
            mine: can_edit && c.author_id == Some(user_id),
            edit_action: format!("{}/{}", paths.comments, c.id),
            delete_action: format!("{}/{}/delete", paths.comments, c.id),
        })
        .collect();
    let attachment_views: Vec<AttachmentView> = attachments
        .iter()
        .map(|a| AttachmentView {
            id: a.id,
            filename: a.filename.clone(),
            size: human_size(a.size),
            uploader: lookup.person(a.uploader_id),
            at: views::time::wib_datetime(&a.created_at),
            href: format!("{}/{}", paths.attachments, a.id),
            delete_action: format!("{}/{}/delete", paths.attachments, a.id),
        })
        .collect();

    // Field custom: nilai tersimpan untuk panel Detail dan isian untuk form ubah.
    let type_fields = ticket_type_fields::Model::fields_of(&ctx.db, wid, t.type_id).await?;
    let values = t.custom_values();
    let custom_values: Vec<CustomValueView> = type_fields
        .iter()
        .map(|f| CustomValueView {
            label: f.name.clone(),
            value: values.0.get(&f.id).cloned().unwrap_or_default(),
            is_url: f.kind().ok() == Some(FieldKind::Url),
        })
        .collect();
    let base_form = current_form(t, parent.as_ref().map(|p| p.code.as_str()));
    let form = form.cloned().unwrap_or(base_form);
    let form_errors = errors.form.as_ref();
    // Dialog Ubah detail (judul, properti, field custom) langsung terbuka bila error ada di
    // luar deskripsi.
    let details_open =
        form_errors.is_some_and(|e| e.errors.keys().any(|k| k.as_str() != "description"));
    let custom = custom_for_form(ctx, &access.access, &form, form_errors).await?;
    let types: Vec<TypeView> = ticket_types::Model::all_in(&ctx.db, wid)
        .await?
        .iter()
        .map(TypeView::of)
        .collect();
    let watcher_views: Vec<_> = watchers
        .iter()
        .filter_map(|id| lookup.person(Some(*id)))
        .collect();
    let board = BoardPaths::new(&super::project_paths(&access.access).home);

    let stage = super::stage::panel(ctx, access).await?;
    let artifact_counts = super::artifact::counts(ctx, access).await?;
    let shell = app_shell(ctx, &tenant.user, Some(tenant), Some(&access.access))
        .await?
        .at("project_ticket");
    let res = views::tickets::render(
        v,
        &shell,
        "tickets/show.html",
        status,
        data!({
            "project": summary(&access.access),
            "ticket": row,
            "description": t.description,
            "reporter": lookup.person(t.reporter_id),
            "created": views::time::wib_datetime(&t.created_at),
            "parent": parent,
            "children": children.iter().map(|c| lookup.row(c)).collect::<Vec<_>>(),
            "children_done": children.iter().filter(|c| c.status().ok() == Some(TicketStatus::Done)).count(),
            "links": link_views,
            "blockers": blockers.join(", "),
            "comments": comment_views,
            "attachments": attachment_views,
            "attachment_max": human_size(i64::try_from(ticket_attachments::MAX_BYTES).unwrap_or_default()),
            "watchers": watcher_views,
            "watching": watchers.contains(&user_id),
            "custom_values": custom_values,
            "form": form_view(&form, form_errors),
            "custom": custom,
            "details_open": details_open,
            "types": types,
            "statuses": status_options(),
            "priorities": priority_options(),
            "people": people_options(ctx, &access.access, None, t.assignee_id).await?,
            "paths": paths,
            "people_src": board.people,
            "board": board,
            "tabs": tabs(&paths, "summary", events.len() as u64, artifact_counts),
            "stage": stage,
            "can_edit": can_edit,
            "errors": {
                "dialogs": super::stage::errors_view(&errors.stage, errors.stage_dialog),
                "status": errors.status,
                "status_codes": errors.status_codes,
                "comment": errors.comment,
                "link": errors.link,
                "attachment": errors.attachment,
            },
            "comment_body": errors.comment_body,
            "flash": errors.flash,
        }),
    )?;
    Ok(res)
}

#[debug_handler]
pub(super) async fn show(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_show(
        &ctx,
        &v,
        &access,
        None,
        PageErrors {
            flash,
            ..PageErrors::default()
        },
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

/// Teks tampil nilai riwayat (status/prioritas jadi kunci Fluent di template; lainnya apa adanya).
fn change_text(
    field: &str,
    value: &serde_json::Value,
    lookup: &Lookup,
    parent_codes: &std::collections::HashMap<Uuid, String>,
) -> String {
    let raw = match value {
        serde_json::Value::Null | serde_json::Value::Object(_) => return String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|i| i.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    };
    match field {
        "parent_id" => Uuid::parse_str(&raw)
            .ok()
            .and_then(|id| parent_codes.get(&id).cloned())
            .unwrap_or(raw),
        "assignee_id" => Uuid::parse_str(&raw)
            .ok()
            .and_then(|id| lookup.person(Some(id)))
            .map_or(raw, |p| p.name),
        "type_id" => Uuid::parse_str(&raw)
            .ok()
            .map_or(raw.clone(), |id| lookup.ticket_type(id).name)
            .clone(),
        _ => raw,
    }
}

#[debug_handler]
pub(super) async fn activity(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
) -> Result<Response> {
    let tenant = &access.access.tenant;
    let wid = tenant.workspace_id();
    let t = &access.ticket;
    let events = ticket_events::Model::of_ticket(&ctx.db, wid, t.id).await?;
    let mut people: Vec<Uuid> = events.iter().filter_map(|e| e.actor_id).collect();
    let mut parents: Vec<Uuid> = Vec::new();
    for e in &events {
        for side in ["from", "to"] {
            if let Some(id) = e.changes["assignee_id"][side]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                people.push(id);
            }
            if let Some(id) = e.changes["parent_id"][side]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                parents.push(id);
            }
        }
    }
    let lookup = lookup(&ctx.db, tenant, std::slice::from_ref(t), &people).await?;
    let parent_codes: std::collections::HashMap<Uuid, String> = tickets::Entity::find()
        .filter(tickets::Column::WorkspaceId.eq(wid.get()))
        .filter(tickets::Column::Id.is_in(parents))
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.code))
        .collect();
    let event_views: Vec<EventView> = events
        .iter()
        .rev()
        .map(|e| {
            let kind = e.kind().unwrap_or(EventKind::Updated);
            let kind_value = sea_orm::ActiveEnum::to_value(&kind);
            let mut changes = Vec::new();
            let all_fields = matches!(kind, EventKind::Updated | EventKind::StatusChanged);
            if let Some(map) = e.changes.as_object() {
                for (field, change) in map {
                    if !all_fields && change.get("to").is_none() {
                        continue;
                    }
                    let mut from = change_text(field, &change["from"], &lookup, &parent_codes);
                    let mut to = change_text(field, &change["to"], &lookup, &parent_codes);
                    let translate = field == "status" || field == "priority";
                    if field == "status" {
                        from = status_key(&from);
                        to = status_key(&to);
                    } else if field == "priority" {
                        from = priority_key(&from);
                        to = priority_key(&to);
                    }
                    changes.push(ChangeView {
                        field_key: format!(
                            "ticket-field-{}",
                            field.trim_end_matches("_id").replace('_', "-")
                        ),
                        from,
                        to,
                        translate,
                    });
                }
            }
            let detail = ["code", "other", "filename", "bug", "stage"]
                .iter()
                .find_map(|k| e.changes[*k].as_str())
                .unwrap_or_default()
                .to_string();
            let note = ["note", "reason"]
                .iter()
                .find_map(|k| e.changes[*k].as_str())
                .unwrap_or_default()
                .to_string();
            EventView {
                label_key: format!("ticket-event-{}", kind_value.replace('_', "-")),
                kind: kind_value,
                actor: lookup.person(e.actor_id),
                at: views::time::wib_datetime(&e.created_at),
                changes,
                detail,
                note,
            }
        })
        .collect();
    let paths = TicketPaths::new(&tenant.workspace.slug, &t.code);
    let shell = app_shell(&ctx, &tenant.user, Some(tenant), Some(&access.access))
        .await?
        .at("project_ticket");
    views::tickets::render(
        &v,
        &shell,
        "tickets/activity.html",
        StatusCode::OK,
        data!({
            "project": summary(&access.access),
            "ticket": lookup.row(t),
            "events": event_views,
            "paths": paths,
            "board": BoardPaths::new(&super::project_paths(&access.access).home),
            "tabs": tabs(&paths, "activity", events.len() as u64, super::artifact::counts(&ctx, &access).await?),
            "stage": super::stage::panel(&ctx, &access).await?,
            "errors": { "dialogs": super::stage::errors_view("", "") },
            "can_edit": access.can(Permission::EditProject),
            "watching": ticket_watchers::Model::user_ids(&ctx.db, wid, t.id)
                .await?
                .contains(&tenant.user.id),
        }),
    )
}

/// Status tersimpan → kunci Fluent (riwayat menerjemahkannya).
fn status_key(raw: &str) -> String {
    TicketStatus::parse(raw)
        .map(|s| status_view(s).label_key.to_string())
        .unwrap_or_else(|| raw.to_string())
}

fn priority_key(raw: &str) -> String {
    tickets::TicketPriority::parse(raw)
        .map(|p| priority_view(p).label_key.to_string())
        .unwrap_or_else(|| raw.to_string())
}

/// Ubah sebagian (form Deskripsi atau panel Detail). `EditProject` sebelum body dibaca.
#[debug_handler]
pub(super) async fn update(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let parent = parent_code(&ctx, &access).await?;
    let base = current_form(&access.ticket, parent.as_deref());
    let form = ticket_form(&pairs, Some(&base));
    let tenant = &access.access.tenant;
    match service::update::run(
        &ctx.db,
        &acting(tenant, request_id),
        access.ticket.id,
        &form,
    )
    .await
    {
        Ok(ticket) => {
            let session = flash::set(session, &Flash::success(views::tickets::FLASH_SAVED))?;
            let to = ticket_href(&tenant.workspace.slug, &ticket.code);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let status = first_code(&errors, "status");
            let codes = blocked_codes(&errors);
            render_show(
                &ctx,
                &v,
                &access,
                Some(&form),
                PageErrors {
                    form: Some(errors),
                    status,
                    status_codes: codes,
                    ..PageErrors::default()
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

/// Kode pemblokir dari error `ticket-blocked`.
fn blocked_codes(errors: &ModelValidationErrors) -> String {
    errors
        .errors
        .get("status")
        .and_then(|l| l.first())
        .and_then(|e| e.params.get("codes"))
        .and_then(|c| c.as_str())
        .unwrap_or_default()
        .to_string()
}

#[derive(Debug, Deserialize)]
pub struct StatusForm {
    #[serde(default)]
    status: String,
    /// Kunci stage tujuan (board per stage, pilihan "Pindah stage"); mengalahkan `status`.
    #[serde(default)]
    stage: String,
    /// Id kartu yang akan berada di bawah kartu ini (drag-drop); kosong = paling bawah.
    #[serde(default)]
    before: String,
    #[serde(default)]
    return_to: Option<String>,
}

#[debug_handler]
pub(super) async fn status(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    Form(form): Form<StatusForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let tenant = &access.access.tenant;
    let target = if form.stage.trim().is_empty() {
        let Some(to) = TicketStatus::parse(&form.status) else {
            return render_show(
                &ctx,
                &v,
                &access,
                None,
                PageErrors {
                    status: tickets::STATUS_INVALID.into(),
                    ..PageErrors::default()
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        };
        service::stages::Target::Category(to)
    } else {
        service::stages::Target::Stage(form.stage.trim().to_string())
    };
    let before = Uuid::parse_str(form.before.trim()).ok();
    match service::stages::move_to(
        &ctx.db,
        &acting(tenant, request_id),
        access.ticket.id,
        target,
        before,
    )
    .await
    {
        Ok(ticket) => {
            let slug = &tenant.workspace.slug;
            let fallback = ticket_href(slug, &ticket.code);
            format::redirect(&safe_return(slug, form.return_to.as_deref(), &fallback))
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_show(
                &ctx,
                &v,
                &access,
                None,
                PageErrors {
                    status: super::stage::stage_error(&errors),
                    status_codes: blocked_codes(&errors),
                    ..PageErrors::default()
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Deserialize)]
pub struct CommentForm {
    #[serde(default)]
    body: String,
}

fn back_to(access: &TicketAccess, anchor: &str) -> String {
    format!(
        "{}#{anchor}",
        ticket_href(&access.access.tenant.workspace.slug, &access.ticket.code)
    )
}

#[debug_handler]
pub(super) async fn comment(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    Form(form): Form<CommentForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let tenant = &access.access.tenant;
    match service::comments::post(
        &ctx.db,
        &acting(tenant, request_id),
        access.ticket.id,
        &form.body,
    )
    .await
    {
        Ok(_) => format::redirect(&back_to(&access, "comments")),
        Err(TicketFailure::Invalid(errors)) => {
            render_show(
                &ctx,
                &v,
                &access,
                None,
                PageErrors {
                    comment: first_code(&errors, "body"),
                    comment_body: form.body,
                    ..PageErrors::default()
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn edit_comment(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_comments::Entity>,
    Form(form): Form<CommentForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let tenant = &access.access.tenant;
    match service::comments::edit(
        &ctx.db,
        &acting(tenant, request_id),
        access.ticket.id,
        id.id,
        &form.body,
    )
    .await
    {
        Ok(_) => format::redirect(&back_to(&access, "comments")),
        Err(TicketFailure::Invalid(errors)) => {
            render_show(
                &ctx,
                &v,
                &access,
                None,
                PageErrors {
                    comment: first_code(&errors, "body"),
                    ..PageErrors::default()
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn delete_comment(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_comments::Entity>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    service::comments::delete(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        id.id,
    )
    .await?;
    format::redirect(&back_to(&access, "comments"))
}

#[derive(Debug, Deserialize)]
pub struct LinkForm {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    target: String,
}

#[debug_handler]
pub(super) async fn link(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    Form(form): Form<LinkForm>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    match service::links::add(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        &form.kind,
        &form.target,
    )
    .await
    {
        Ok(_) => format::redirect(&back_to(&access, "links")),
        Err(TicketFailure::Invalid(errors)) => {
            let code = ["target", "kind"]
                .iter()
                .map(|f| first_code(&errors, f))
                .find(|c| !c.is_empty())
                .unwrap_or_default();
            render_show(
                &ctx,
                &v,
                &access,
                None,
                PageErrors {
                    link: code,
                    ..PageErrors::default()
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn unlink(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_links::Entity>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    service::links::remove(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        id.id,
    )
    .await?;
    format::redirect(&back_to(&access, "links"))
}

/// Field `file` multipart: (nama berkas, content type, isi). Body rusak → 400.
async fn file_field(mut multipart: Multipart) -> Result<(String, Option<String>, Bytes)> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| Error::BadRequest("multipart".into()))?
    {
        if field.name() == Some("file") {
            let name = field.file_name().unwrap_or_default().to_string();
            let mime = field.content_type().map(str::to_string);
            let data = field
                .bytes()
                .await
                .map_err(|_| Error::BadRequest("multipart".into()))?;
            return Ok((name, mime, data));
        }
    }
    Ok((String::new(), None, Bytes::new()))
}

/// `TicketAccess` + `EditProject` sebelum body dibaca. Batas body di route (lihat `mod.rs`);
/// melewati batas → 413 dari axum.
#[debug_handler]
pub(super) async fn upload(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    multipart: Multipart,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    let (name, mime, data) = match file_field(multipart).await {
        Ok(file) => file,
        // Body terpotong karena melebihi batas: tampilkan sebagai berkas terlalu besar.
        Err(Error::BadRequest(_)) => {
            return render_show(
                &ctx,
                &v,
                &access,
                None,
                PageErrors {
                    attachment: ticket_attachments::ATTACHMENT_TOO_LARGE.into(),
                    ..PageErrors::default()
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
        }
        Err(e) => return Err(e),
    };
    match service::attachments::upload(
        &ctx.db,
        &ctx.storage,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        &name,
        mime.as_deref(),
        data,
    )
    .await
    {
        Ok(_) => format::redirect(&back_to(&access, "attachments")),
        Err(TicketFailure::Invalid(errors)) => {
            render_show(
                &ctx,
                &v,
                &access,
                None,
                PageErrors {
                    attachment: first_code(&errors, "file"),
                    ..PageErrors::default()
                },
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

/// Unduh lampiran: selalu `attachment` + `nosniff` (berkas unggahan tidak pernah dirender
/// browser di origin aplikasi).
#[debug_handler]
pub(super) async fn download(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    id: IdOr404<ticket_attachments::Entity>,
) -> Result<Response> {
    let wid = access.access.tenant.workspace_id();
    let attachment = ticket_attachments::Model::find_in(&ctx.db, wid, access.ticket.id, id.id)
        .await?
        .ok_or(Error::NotFound)?;
    let data: Vec<u8> = ctx
        .storage
        .download(std::path::Path::new(&attachment.storage_key))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "lampiran tidak ada di storage");
            Error::NotFound
        })?;
    let disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        attachment
            .filename
            .chars()
            .map(
                |c| if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                    c
                } else {
                    '_'
                }
            )
            .collect::<String>(),
        urlencoding_encode(&attachment.filename)
    );
    let mut res = Response::new(Body::from(data));
    let headers = res.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&attachment.content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).unwrap_or(HeaderValue::from_static("attachment")),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(res)
}

/// Percent-encoding RFC 5987 untuk `filename*`.
fn urlencoding_encode(raw: &str) -> String {
    raw.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[debug_handler]
pub(super) async fn remove_attachment(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_attachments::Entity>,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    service::attachments::remove(
        &ctx.db,
        &ctx.storage,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        id.id,
    )
    .await?;
    format::redirect(&back_to(&access, "attachments"))
}

#[derive(Debug, Deserialize)]
pub struct WatchForm {
    #[serde(default)]
    watch: String,
}

#[debug_handler]
pub(super) async fn watch(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    RequestId(request_id): RequestId,
    Form(form): Form<WatchForm>,
) -> Result<Response> {
    service::set_watching(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
        form.watch == "1",
    )
    .await?;
    format::redirect(&ticket_href(
        &access.access.tenant.workspace.slug,
        &access.ticket.code,
    ))
}

#[debug_handler]
pub(super) async fn trash(
    State(ctx): State<AppContext>,
    access: TicketAccess,
    session: Session,
    RequestId(request_id): RequestId,
) -> Result<Response> {
    access.require(Permission::EditProject)?;
    service::trash::trash(
        &ctx.db,
        &acting(&access.access.tenant, request_id),
        access.ticket.id,
    )
    .await?;
    let session = flash::set(session, &Flash::success(views::tickets::FLASH_TRASHED))?;
    let to = BoardPaths::new(&super::project_paths(&access.access).home).board;
    Ok((session, format::redirect(&to)?).into_response())
}

/// Halaman detail 422 dengan pesan penolakan aksi stage; `dialog` dibuka lagi.
pub(super) async fn render_with_stage_error(
    ctx: &AppContext,
    v: &TeraView,
    access: &TicketAccess,
    code: String,
    dialog: &'static str,
) -> Result<Response> {
    render_show(
        ctx,
        v,
        access,
        None,
        PageErrors {
            stage: code,
            stage_dialog: dialog,
            ..PageErrors::default()
        },
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await
}
