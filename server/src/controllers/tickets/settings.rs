//! Pengaturan tiket workspace (`Require<ManageTicketSettings>`): Tipe tiket & field (pustaka
//! field custom), Kode tiket; dan Pengaturan project → Kode tiket (`ProjectAdmin`).
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use uuid::Uuid;

use super::{acting, project_paths, summary};
use crate::{
    controllers::{
        extractors::{IdOr404, ProjectAdmin, RequestId, Require, require::ManageTicketSettings},
        flash,
        shell::app_shell,
    },
    models::{
        custom_fields::{self, FieldInput, FieldKind},
        ticket_code, ticket_type_fields,
        ticket_types::{self, TYPE_ICONS, TypeInput},
        tickets,
    },
    services::tickets::{
        TicketFailure,
        create::effective_template,
        settings::{self as service, ProjectCodeInput},
    },
    views::{
        self,
        flash::Flash,
        nav,
        tickets::{
            CodeFormView, FieldFormView, SelectOption, TypeFormView, TypeView, code_tokens,
            field_kind_options, scope_options,
        },
    },
};

fn base(slug: &str) -> String {
    format!("/w/{slug}/settings")
}

fn get<'a>(pairs: &'a [(String, String)], name: &str) -> &'a str {
    pairs
        .iter()
        .find(|(k, _)| k == name)
        .map_or("", |(_, v)| v.as_str())
}

fn ids(pairs: &[(String, String)], name: &str) -> Vec<Uuid> {
    pairs
        .iter()
        .filter(|(k, _)| k == name)
        .filter_map(|(_, v)| Uuid::parse_str(v.trim()).ok())
        .collect()
}

fn type_input(pairs: &[(String, String)]) -> TypeInput {
    TypeInput {
        key: get(pairs, "key").to_string(),
        name: get(pairs, "name").to_string(),
        description: get(pairs, "description").to_string(),
        icon: get(pairs, "icon").to_string(),
        color: get(pairs, "color").to_string(),
    }
}

fn type_form(pairs: &[(String, String)], errors: Option<&ModelValidationErrors>) -> TypeFormView {
    let input = type_input(pairs);
    TypeFormView {
        key: input.key.trim().to_ascii_lowercase(),
        name: input.name.trim().to_string(),
        description: input.description.trim().to_string(),
        icon: input.icon,
        color: input.color.trim().to_ascii_lowercase(),
        errors: Default::default(),
    }
    .with_errors(errors)
}

fn icon_options() -> Vec<SelectOption> {
    TYPE_ICONS
        .iter()
        .map(|i| SelectOption {
            id: (*i).to_string(),
            name: (*i).to_string(),
        })
        .collect()
}

/// Halaman Tipe tiket & field: daftar tipe, pustaka field, form tipe/field baru (dialog).
#[allow(clippy::too_many_arguments)]
async fn render_types(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &crate::controllers::extractors::Tenant,
    new_type: TypeFormView,
    type_open: bool,
    new_field: FieldFormView,
    field_open: bool,
    errors: Option<(&str, &ModelValidationErrors)>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let slug = &tenant.workspace.slug;
    let types = ticket_types::Model::all_in(&ctx.db, wid).await?;
    let fields = custom_fields::Model::all_in(&ctx.db, wid).await?;
    let links = ticket_type_fields::Model::all_in(&ctx.db, wid).await?;
    let type_rows: Vec<_> = types
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": TypeView::of(t),
                "fields": links.iter().filter(|l| l.type_id == t.id).count(),
                "href": format!("{}/ticket-types/{}", base(slug), t.id),
            })
        })
        .collect();
    let field_rows: Vec<_> = fields
        .iter()
        .map(|f| {
            let used: Vec<String> = links
                .iter()
                .filter(|l| l.field_id == f.id)
                .filter_map(|l| types.iter().find(|t| t.id == l.type_id))
                .map(|t| t.name.clone())
                .collect();
            serde_json::json!({
                "id": f.id,
                "name": f.name,
                "kind_key": format!("custom-field-kind-{}", f.kind),
                "required": f.required,
                "types": used.join(", "),
                "href": format!("{}/fields/{}", base(slug), f.id),
            })
        })
        .collect();
    let error = errors
        .and_then(|(field, e)| e.errors.get(field).and_then(|l| l.first()))
        .map(|e| e.code.clone())
        .unwrap_or_default();
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("ticket_types");
    let res = views::tickets::render(
        v,
        &shell,
        "tickets/settings/types.html",
        status,
        data!({
            "types": type_rows,
            "fields": field_rows,
            "type_options": types.iter().map(|t| SelectOption { id: t.id.to_string(), name: t.name.clone() }).collect::<Vec<_>>(),
            "new_type": new_type,
            "type_open": type_open,
            "new_field": new_field,
            "field_open": field_open,
            "icons": icon_options(),
            "kinds": field_kind_options(),
            "create_type_action": format!("{}/ticket-types/new", base(slug)),
            "create_field_action": format!("{}/fields", base(slug)),
            "error": error,
            "flash": flash,
        }),
    )?;
    Ok(res)
}

#[debug_handler]
pub(super) async fn types(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let res = render_types(
        &ctx,
        &v,
        &access.tenant,
        TypeFormView::empty(),
        false,
        FieldFormView::empty(),
        false,
        None,
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn new_type(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
) -> Result<Response> {
    render_types(
        &ctx,
        &v,
        &access.tenant,
        TypeFormView::empty(),
        true,
        FieldFormView::empty(),
        false,
        None,
        None,
        StatusCode::OK,
    )
    .await
}

#[debug_handler]
pub(super) async fn create_type(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    match service::create_type(&ctx.db, &acting(tenant, request_id), &type_input(&pairs)).await {
        Ok(created) => {
            let session = flash::set(session, &Flash::success(views::tickets::FLASH_TYPE_SAVED))?;
            let to = format!(
                "{}/ticket-types/{}",
                base(&tenant.workspace.slug),
                created.id
            );
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_types(
                &ctx,
                &v,
                tenant,
                type_form(&pairs, Some(&errors)),
                true,
                FieldFormView::empty(),
                false,
                None,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

async fn render_type(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &crate::controllers::extractors::Tenant,
    ticket_type: &ticket_types::Model,
    form: TypeFormView,
    selected: &[Uuid],
    delete_error: &str,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let fields = custom_fields::Model::all_in(&ctx.db, wid).await?;
    let in_use = sea_orm::PaginatorTrait::count(
        sea_orm::QueryFilter::filter(
            <tickets::Entity as sea_orm::EntityTrait>::find(),
            sea_orm::ColumnTrait::eq(&tickets::Column::TypeId, ticket_type.id),
        ),
        &ctx.db,
    )
    .await?;
    let href = format!(
        "{}/ticket-types/{}",
        base(&tenant.workspace.slug),
        ticket_type.id
    );
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("ticket_types");
    let res = views::tickets::render(
        v,
        &shell,
        "tickets/settings/type.html",
        status,
        data!({
            "type": TypeView::of(ticket_type),
            "form": form,
            "fields": fields.iter().map(|f| serde_json::json!({
                "id": f.id,
                "name": f.name,
                "kind_key": format!("custom-field-kind-{}", f.kind),
                "required": f.required,
                "checked": selected.contains(&f.id),
            })).collect::<Vec<_>>(),
            "icons": icon_options(),
            "action": href,
            "delete_action": format!("{href}/delete"),
            "back_href": format!("{}/ticket-types", base(&tenant.workspace.slug)),
            "in_use": in_use,
            "delete_error": delete_error,
            "flash": flash,
        }),
    )?;
    Ok(res)
}

async fn find_type(
    ctx: &AppContext,
    access: &Require<ManageTicketSettings>,
    id: Uuid,
) -> Result<ticket_types::Model> {
    ticket_types::Model::find_in(&ctx.db, access.tenant.workspace_id(), id)
        .await?
        .ok_or(Error::NotFound)
}

async fn type_field_ids(
    ctx: &AppContext,
    access: &Require<ManageTicketSettings>,
    id: Uuid,
) -> Result<Vec<Uuid>> {
    Ok(
        ticket_type_fields::Model::fields_of(&ctx.db, access.tenant.workspace_id(), id)
            .await?
            .iter()
            .map(|f| f.id)
            .collect(),
    )
}

#[debug_handler]
pub(super) async fn edit_type(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    id: IdOr404<ticket_types::Entity>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let t = find_type(&ctx, &access, id.id).await?;
    let selected = type_field_ids(&ctx, &access, t.id).await?;
    let res = render_type(
        &ctx,
        &v,
        &access.tenant,
        &t,
        TypeFormView::of(&t),
        &selected,
        "",
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn update_type(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_types::Entity>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let t = find_type(&ctx, &access, id.id).await?;
    // Daftar field hanya diganti bila form field ikut dikirim (penanda `fields_form`).
    let field_ids = ids(&pairs, "field_ids");
    let replace = pairs.iter().any(|(k, _)| k == "fields_form");
    let mut input = type_input(&pairs);
    input.key = t.key.clone();
    match service::update_type(
        &ctx.db,
        &acting(&access.tenant, request_id),
        t.id,
        &input,
        replace.then_some(field_ids.as_slice()),
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::tickets::FLASH_TYPE_SAVED))?;
            let to = format!(
                "{}/ticket-types/{}",
                base(&access.tenant.workspace.slug),
                t.id
            );
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let mut form = type_form(&pairs, Some(&errors));
            form.key = t.key.clone();
            render_type(
                &ctx,
                &v,
                &access.tenant,
                &t,
                form,
                &field_ids,
                "",
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn delete_type(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<ticket_types::Entity>,
) -> Result<Response> {
    let t = find_type(&ctx, &access, id.id).await?;
    match service::delete_type(&ctx.db, &acting(&access.tenant, request_id), t.id).await {
        Ok(()) => {
            let session = flash::set(session, &Flash::success(views::tickets::FLASH_TYPE_DELETED))?;
            let to = format!("{}/ticket-types", base(&access.tenant.workspace.slug));
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let code = errors
                .errors
                .get("type")
                .and_then(|l| l.first())
                .map(|e| e.code.clone())
                .unwrap_or_default();
            let selected = type_field_ids(&ctx, &access, t.id).await?;
            render_type(
                &ctx,
                &v,
                &access.tenant,
                &t,
                TypeFormView::of(&t),
                &selected,
                &code,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

fn field_input(pairs: &[(String, String)]) -> Option<FieldInput> {
    let kind =
        <FieldKind as sea_orm::ActiveEnum>::try_from_value(&get(pairs, "kind").trim().to_string())
            .ok()?;
    Some(FieldInput {
        name: get(pairs, "name").to_string(),
        kind,
        options: get(pairs, "options").lines().map(str::to_string).collect(),
        required: !get(pairs, "required").is_empty(),
    })
}

fn field_form(pairs: &[(String, String)], errors: Option<&ModelValidationErrors>) -> FieldFormView {
    FieldFormView {
        name: get(pairs, "name").trim().to_string(),
        kind: get(pairs, "kind").to_string(),
        options: get(pairs, "options").to_string(),
        required: !get(pairs, "required").is_empty(),
        type_ids: ids(pairs, "type_ids").iter().map(Uuid::to_string).collect(),
        errors: Default::default(),
    }
    .with_errors(errors)
}

fn kind_error() -> ModelValidationErrors {
    let mut errors = validator::ValidationErrors::new();
    errors.add(
        "kind",
        validator::ValidationError::new(custom_fields::FIELD_VALUE_INVALID),
    );
    errors.into()
}

#[debug_handler]
pub(super) async fn create_field(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let tenant = &access.tenant;
    let result = match field_input(&pairs) {
        Some(input) => {
            service::create_field(
                &ctx.db,
                &acting(tenant, request_id),
                &input,
                &ids(&pairs, "type_ids"),
            )
            .await
        }
        None => Err(TicketFailure::Invalid(kind_error())),
    };
    match result {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::tickets::FLASH_FIELD_SAVED))?;
            let to = format!("{}/ticket-types", base(&tenant.workspace.slug));
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            render_types(
                &ctx,
                &v,
                tenant,
                TypeFormView::empty(),
                false,
                field_form(&pairs, Some(&errors)),
                true,
                None,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

async fn render_field(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &crate::controllers::extractors::Tenant,
    field: &custom_fields::Model,
    form: FieldFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let types = ticket_types::Model::all_in(&ctx.db, tenant.workspace_id()).await?;
    let href = format!("{}/fields/{}", base(&tenant.workspace.slug), field.id);
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("ticket_types");
    let res = views::tickets::render(
        v,
        &shell,
        "tickets/settings/field.html",
        status,
        data!({
            "field": { "id": field.id, "name": field.name, "kind_key": format!("custom-field-kind-{}", field.kind) },
            "form": form,
            "types": types.iter().map(TypeView::of).collect::<Vec<_>>(),
            "action": href,
            "delete_action": format!("{href}/delete"),
            "back_href": format!("{}/ticket-types", base(&tenant.workspace.slug)),
            "flash": flash,
        }),
    )?;
    Ok(res)
}

async fn find_field(
    ctx: &AppContext,
    access: &Require<ManageTicketSettings>,
    id: Uuid,
) -> Result<custom_fields::Model> {
    custom_fields::Model::find_in(&ctx.db, access.tenant.workspace_id(), id)
        .await?
        .ok_or(Error::NotFound)
}

#[debug_handler]
pub(super) async fn edit_field(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    id: IdOr404<custom_fields::Entity>,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let field = find_field(&ctx, &access, id.id).await?;
    let used: Vec<String> =
        ticket_type_fields::Model::all_in(&ctx.db, access.tenant.workspace_id())
            .await?
            .iter()
            .filter(|l| l.field_id == field.id)
            .map(|l| l.type_id.to_string())
            .collect();
    let form = FieldFormView::of(&field, used);
    let res = render_field(
        &ctx,
        &v,
        &access.tenant,
        &field,
        form,
        flash,
        StatusCode::OK,
    )
    .await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn update_field(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<custom_fields::Entity>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let field = find_field(&ctx, &access, id.id).await?;
    let input = FieldInput {
        name: get(&pairs, "name").to_string(),
        kind: field.kind().map_err(|e| Error::DB(e))?,
        options: get(&pairs, "options").lines().map(str::to_string).collect(),
        required: !get(&pairs, "required").is_empty(),
    };
    match service::update_field(
        &ctx.db,
        &acting(&access.tenant, request_id),
        field.id,
        &input,
        &ids(&pairs, "type_ids"),
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::tickets::FLASH_FIELD_SAVED))?;
            let to = format!(
                "{}/fields/{}",
                base(&access.tenant.workspace.slug),
                field.id
            );
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let mut form = field_form(&pairs, Some(&errors));
            form.kind = field.kind.clone();
            render_field(
                &ctx,
                &v,
                &access.tenant,
                &field,
                form,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

#[debug_handler]
pub(super) async fn delete_field(
    State(ctx): State<AppContext>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    id: IdOr404<custom_fields::Entity>,
) -> Result<Response> {
    let field = find_field(&ctx, &access, id.id).await?;
    service::delete_field(&ctx.db, &acting(&access.tenant, request_id), field.id).await?;
    let session = flash::set(
        session,
        &Flash::success(views::tickets::FLASH_FIELD_DELETED),
    )?;
    let to = format!("{}/ticket-types", base(&access.tenant.workspace.slug));
    Ok((session, format::redirect(&to)?).into_response())
}

/// Contoh kode untuk pratinjau (project/tipe contoh, nomor 42).
fn preview(template: &str, project_key: &str, team: Option<&str>) -> String {
    if template.trim().is_empty() {
        return String::new();
    }
    let year = chrono::Datelike::year(&chrono::Utc::now());
    ticket_code::render(
        template,
        &ticket_code::CodeParts {
            project: project_key,
            team: team.unwrap_or(project_key),
            type_key: "task",
            year,
            number: 42,
        },
    )
}

async fn render_code(
    ctx: &AppContext,
    v: &TeraView,
    tenant: &crate::controllers::extractors::Tenant,
    form: CodeFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let shell = app_shell(ctx, &tenant.user, Some(tenant), None)
        .await?
        .at("ticket_code");
    let res = views::tickets::render(
        v,
        &shell,
        "tickets/settings/code.html",
        status,
        data!({
            "form": form,
            "scopes": scope_options(),
            "tokens": code_tokens(),
            "action": format!("{}/ticket-code", base(&tenant.workspace.slug)),
            "flash": flash,
        }),
    )?;
    Ok(res)
}

#[debug_handler]
pub(super) async fn code(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let ws = &access.tenant.workspace;
    let form = CodeFormView {
        custom: true,
        template: ws.ticket_code_template.clone(),
        scope: ws.ticket_counter_scope.clone(),
        preview: preview(&ws.ticket_code_template, "KSR", None),
        ..CodeFormView::default()
    }
    .with_errors(None);
    let res = render_code(&ctx, &v, &access.tenant, form, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

#[debug_handler]
pub(super) async fn save_code(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    access: Require<ManageTicketSettings>,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let template = get(&pairs, "template");
    let scope = get(&pairs, "scope");
    match service::update_workspace_code(
        &ctx.db,
        &acting(&access.tenant, request_id),
        template,
        scope,
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::tickets::FLASH_CODE_SAVED))?;
            let to = format!("{}/ticket-code", base(&access.tenant.workspace.slug));
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let form = CodeFormView {
                custom: true,
                template: template.trim().to_string(),
                scope: scope.to_string(),
                preview: String::new(),
                ..CodeFormView::default()
            }
            .with_errors(Some(&errors));
            render_code(
                &ctx,
                &v,
                &access.tenant,
                form,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}

async fn render_project_code(
    ctx: &AppContext,
    v: &TeraView,
    access: &crate::controllers::extractors::ProjectAccess,
    form: CodeFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    let paths = project_paths(access);
    let ws = &access.tenant.workspace;
    let (inherited, _) = effective_template(
        ws,
        &crate::models::projects::Model {
            ticket_code_template: None,
            ticket_counter_scope: None,
            ..access.project.clone()
        },
    );
    let shell = app_shell(ctx, &access.tenant.user, Some(&access.tenant), Some(access))
        .await?
        .at("project_ticket_code");
    let res = views::tickets::render(
        v,
        &shell,
        "tickets/settings/project_code.html",
        status,
        data!({
            "project": summary(access),
            "paths": paths,
            "subnav": nav::project_settings_nav(&paths, "ticket_code"),
            "form": form,
            "scopes": scope_options(),
            "tokens": code_tokens(),
            "inherited": inherited,
            "inherited_preview": preview(&inherited, &access.project.key, access.project.team_code.as_deref()),
            "action": format!("{}/settings/ticket-code", paths.home),
            "flash": flash,
        }),
    )?;
    Ok(res)
}

#[debug_handler]
pub(super) async fn project_code(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
) -> Result<Response> {
    let (session, flash) = flash::take(session);
    let p = &access.project;
    let (template, scope) = effective_template(&access.tenant.workspace, p);
    let form = CodeFormView {
        custom: p.ticket_code_template.is_some(),
        preview: preview(&template, &p.key, p.team_code.as_deref()),
        template,
        scope: sea_orm::ActiveEnum::to_value(&scope),
        team_code: p.team_code.clone().unwrap_or_default(),
        ..CodeFormView::default()
    }
    .with_errors(None);
    let res = render_project_code(&ctx, &v, &access, form, flash, StatusCode::OK).await?;
    Ok((session, res).into_response())
}

/// `ProjectAdmin` sebelum body dibaca (rules §6).
#[debug_handler]
pub(super) async fn save_project_code(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    ProjectAdmin(access): ProjectAdmin,
    session: Session,
    RequestId(request_id): RequestId,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response> {
    let input = ProjectCodeInput {
        custom: !get(&pairs, "custom").is_empty(),
        template: get(&pairs, "template").to_string(),
        scope: get(&pairs, "scope").to_string(),
        team_code: get(&pairs, "team_code").to_string(),
    };
    match service::update_project_code(
        &ctx.db,
        &acting(&access.tenant, request_id),
        access.project.id,
        &input,
    )
    .await
    {
        Ok(_) => {
            let session = flash::set(session, &Flash::success(views::tickets::FLASH_CODE_SAVED))?;
            let to = format!("{}/settings/ticket-code", project_paths(&access).home);
            Ok((session, format::redirect(&to)?).into_response())
        }
        Err(TicketFailure::Invalid(errors)) => {
            let form = CodeFormView {
                custom: input.custom,
                template: input.template.trim().to_string(),
                scope: input.scope.clone(),
                team_code: input.team_code.trim().to_string(),
                preview: String::new(),
                ..CodeFormView::default()
            }
            .with_errors(Some(&errors));
            render_project_code(
                &ctx,
                &v,
                &access,
                form,
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
        Err(e) => Err(e.into()),
    }
}
