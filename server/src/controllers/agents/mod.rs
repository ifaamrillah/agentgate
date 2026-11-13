//! Agent: Pengaturan → Persona agent dan Skill pack (`ManageTicketSettings`).
use axum::routing::{get, post};
use loco_rs::prelude::Routes;

mod personas;
mod skills;

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
            "/w/{slug}/settings/personas/{id}/knowledge",
            post(personas::save_knowledge),
        )
        .add(
            "/w/{slug}/settings/personas/{id}/delete",
            post(personas::delete),
        )
        .add(
            "/w/{slug}/settings/skills",
            get(skills::index).post(skills::create),
        )
        .add("/w/{slug}/settings/skills/import", post(skills::import))
        .add(
            "/w/{slug}/settings/skills/{id}",
            get(skills::show).post(skills::update),
        )
        .add(
            "/w/{slug}/settings/skills/{id}/files",
            post(skills::save_file),
        )
        .add(
            "/w/{slug}/settings/skills/{id}/files/new",
            post(skills::add_file),
        )
        .add(
            "/w/{slug}/settings/skills/{id}/files/delete",
            post(skills::delete_file),
        )
        .add(
            "/w/{slug}/settings/skills/{id}/submit",
            post(skills::submit),
        )
        .add(
            "/w/{slug}/settings/skills/{id}/approve",
            post(skills::approve),
        )
        .add(
            "/w/{slug}/settings/skills/{id}/reject",
            post(skills::reject),
        )
        .add(
            "/w/{slug}/settings/skills/{id}/delete",
            post(skills::delete),
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
        allowed.then(|| skills::base(slug)),
    )
}
