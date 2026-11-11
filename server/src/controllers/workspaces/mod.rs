//! Workspace: pilih (`/`), buat (`/w/new`), beranda (`/w/{slug}`), dan Pengaturan
//! (`/w/{slug}/settings`, dengan zona berbahaya, serta Anggota, Role, Jabatan, Tim, dan Audit log
//! di bawahnya). Semua route di bawah
//! `/w/{slug}` memakai extractor `Tenant`.
use loco_rs::prelude::*;
use serde::Deserialize;

mod audit;
mod home;
mod invitations;
mod job_titles;
mod members;
mod roles;
mod settings;
mod teams;

/// Isian form nama + slug (buat workspace dan Pengaturan). Field yang tidak dikirim dianggap
/// kosong, supaya form yang dipotong tetap dirender ulang dengan pesan validasi.
#[derive(Debug, Deserialize)]
pub struct WorkspaceForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
}

fn settings_path(slug: &str) -> String {
    format!("/w/{slug}/settings")
}

fn members_path(slug: &str) -> String {
    format!("/w/{slug}/settings/members")
}

fn invitations_path(slug: &str) -> String {
    format!("/w/{slug}/settings/members/invitations")
}

fn leave_path(slug: &str) -> String {
    format!("/w/{slug}/leave")
}

fn roles_path(slug: &str) -> String {
    format!("/w/{slug}/settings/roles")
}

fn job_titles_path(slug: &str) -> String {
    format!("/w/{slug}/settings/job-titles")
}

fn teams_path(slug: &str) -> String {
    format!("/w/{slug}/settings/teams")
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/", get(home::choose))
        // Rute statis menang atas `/w/{slug}`; slug `new` dilarang (workspaces::RESERVED_SLUGS).
        .add("/w/new", get(home::new).post(home::create))
        .add("/w/{slug}", get(home::show))
        .add(
            "/w/{slug}/leave",
            get(members::confirm_leave).post(members::leave),
        )
        .add(
            "/w/{slug}/settings",
            get(settings::show).post(settings::update),
        )
        .add(
            "/w/{slug}/settings/require-2fa",
            post(settings::require_2fa),
        )
        .add(
            "/w/{slug}/settings/delete",
            post(settings::schedule_deletion),
        )
        .add(
            "/w/{slug}/settings/delete/cancel",
            post(settings::cancel_deletion),
        )
        .add("/w/{slug}/settings/audit", get(audit::index))
        .add("/w/{slug}/settings/members", get(members::index))
        // Rute statis `invitations` menang atas `{id}`.
        .add(
            "/w/{slug}/settings/members/invitations",
            post(invitations::create),
        )
        .add(
            "/w/{slug}/settings/members/invitations/{id}/resend",
            post(invitations::resend),
        )
        .add(
            "/w/{slug}/settings/members/invitations/{id}/revoke",
            post(invitations::revoke),
        )
        .add(
            "/w/{slug}/settings/members/{id}/role",
            post(members::change_role),
        )
        .add(
            "/w/{slug}/settings/members/{id}/job-title",
            post(members::set_job_title),
        )
        .add(
            "/w/{slug}/settings/members/{id}/remove",
            get(members::confirm_remove).post(members::remove),
        )
        .add("/w/{slug}/settings/roles", get(roles::index))
        // Rute statis `new` menang atas `{id}`.
        .add(
            "/w/{slug}/settings/roles/new",
            get(roles::new).post(roles::create),
        )
        .add(
            "/w/{slug}/settings/roles/{id}/edit",
            get(roles::edit).post(roles::update),
        )
        .add("/w/{slug}/settings/roles/{id}/delete", post(roles::delete))
        .add(
            "/w/{slug}/settings/job-titles",
            get(job_titles::index).post(job_titles::create),
        )
        .add(
            "/w/{slug}/settings/job-titles/{id}",
            post(job_titles::rename),
        )
        .add(
            "/w/{slug}/settings/job-titles/{id}/delete",
            post(job_titles::delete),
        )
        .add(
            "/w/{slug}/settings/teams",
            get(teams::index).post(teams::create),
        )
        .add(
            "/w/{slug}/settings/teams/{id}",
            get(teams::show).post(teams::rename),
        )
        .add("/w/{slug}/settings/teams/{id}/delete", post(teams::delete))
        .add(
            "/w/{slug}/settings/teams/{id}/members",
            post(teams::add_member),
        )
        // Orang yang dikeluarkan datang dari isian form (`user_id`), bukan path: `IdOr404`
        // hanya membaca `{id}`.
        .add(
            "/w/{slug}/settings/teams/{id}/members/remove",
            post(teams::remove_member),
        )
}
