//! Agent: Pengaturan → Persona agent dan Skill pack (`ManageTicketSettings`).
use axum::routing::{get, post};
use loco_rs::prelude::Routes;

mod personas;

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/w/{slug}/settings/personas",
            get(personas::index).post(personas::create),
        )
        .add("/w/{slug}/settings/personas/{id}", get(personas::edit))
        .add(
            "/w/{slug}/settings/personas/{id}/identity",
            post(personas::save_identity),
        )
        .add(
            "/w/{slug}/settings/personas/{id}/instructions",
            post(personas::save_instructions),
        )
        .add(
            "/w/{slug}/settings/personas/{id}/tools",
            post(personas::save_tools),
        )
        .add(
            "/w/{slug}/settings/personas/{id}/models",
            post(personas::save_models),
        )
        .add(
            "/w/{slug}/settings/personas/{id}/skills",
            post(personas::save_skills),
        )
        .add(
            "/w/{slug}/settings/personas/{id}/delete",
            post(personas::delete),
        )
}

/// Tautan menu Pengaturan (persona, skill pack) bila role memegang `ManageTicketSettings`.
pub(crate) fn settings_links(
    tenant: &crate::controllers::extractors::Tenant,
) -> (Option<String>, Option<String>) {
    let allowed = tenant.can(crate::models::permission::Permission::ManageTicketSettings);
    let slug = &tenant.workspace.slug;
    (
        allowed.then(|| personas::base(slug)),
        allowed.then(|| format!("/w/{slug}/settings/skills")),
    )
}
