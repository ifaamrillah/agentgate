//! Pengaturan → Tim: daftar tim dengan form tambah, dan halaman satu tim (ganti nama, anggota,
//! hapus).
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
    layout::{AppShell, app_page},
    ui::ComboOption,
};
use crate::models::{teams, users};

pub const FLASH_CREATED: &str = "flash-team-created";
pub const FLASH_UPDATED: &str = "flash-team-updated";
pub const FLASH_DELETED: &str = "flash-team-deleted";
pub const FLASH_MEMBER_ADDED: &str = "flash-team-member-added";
pub const FLASH_MEMBER_REMOVED: &str = "flash-team-member-removed";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_CREATED,
    FLASH_UPDATED,
    FLASH_DELETED,
    FLASH_MEMBER_ADDED,
    FLASH_MEMBER_REMOVED,
];

/// Isian form nama tim (tambah dan ganti nama).
#[derive(Debug, Serialize)]
pub struct TeamFormView {
    pub name: String,
    pub errors: FieldErrors,
}

impl TeamFormView {
    #[must_use]
    pub fn new(name: &str, errors: Option<&ModelValidationErrors>) -> Self {
        Self {
            name: name.to_string(),
            errors: field_errors(&["name"], errors),
        }
    }
}

/// Satu tim di daftar.
#[derive(Debug, Serialize)]
pub struct TeamLink {
    pub name: String,
    pub href: String,
    pub members: u64,
    pub initials: String,
    pub color: &'static str,
}

impl TeamLink {
    /// `base` = path daftar tim (`/w/{slug}/settings/teams`).
    #[must_use]
    pub fn new(team: &teams::Model, base: &str, members: u64) -> Self {
        let id = team.id.to_string();
        Self {
            name: team.name.clone(),
            href: format!("{base}/{id}"),
            members,
            initials: super::palette::initials(&team.name, &id),
            color: super::palette::color_for(&id),
        }
    }
}

pub fn index(
    v: &impl ViewRenderer,
    shell: &AppShell,
    teams: &[TeamLink],
    create: &TeamFormView,
    action: &str,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "teams/index.html",
        app_page(
            shell,
            data!({ "teams": teams, "create": create, "action": action, "flash": flash }),
        ),
    )
}

/// Satu anggota tim.
#[derive(Debug, Serialize)]
pub struct TeamMemberRow {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub initials: String,
    pub color: &'static str,
}

impl TeamMemberRow {
    #[must_use]
    pub fn new(user: &users::Model) -> Self {
        Self {
            user_id: user.id.to_string(),
            name: user.name.clone(),
            email: user.email.clone(),
            initials: super::layout::initials(&user.name, &user.email),
            color: super::palette::color_for(&user.email),
        }
    }
}

/// Data `teams/show.html`. `candidates` = anggota workspace yang belum di tim ini.
#[derive(Debug, Serialize)]
pub struct TeamPage {
    pub title: String,
    pub form: TeamFormView,
    pub members: Vec<TeamMemberRow>,
    pub candidates: Vec<ComboOption>,
    /// Pencarian opsi orang di server (`GET /w/{slug}/people?not_in_team=`).
    pub people_src: String,
    pub action: String,
    pub delete_action: String,
    pub add_action: String,
    pub remove_action: String,
    pub back: String,
    pub flash: Option<Flash>,
}

impl TeamPage {
    /// `base` = path daftar tim; `form` = isian nama (nilai tersimpan atau isian yang gagal).
    #[must_use]
    pub fn new(
        team: &teams::Model,
        base: &str,
        form: TeamFormView,
        members: Vec<TeamMemberRow>,
        candidates: Vec<ComboOption>,
        people_src: String,
        flash: Option<Flash>,
    ) -> Self {
        let action = format!("{base}/{}", team.id);
        Self {
            title: team.name.clone(),
            form,
            members,
            candidates,
            people_src,
            delete_action: format!("{action}/delete"),
            add_action: format!("{action}/members"),
            remove_action: format!("{action}/members/remove"),
            action,
            back: base.to_string(),
            flash,
        }
    }
}

pub fn show(
    v: &impl ViewRenderer,
    shell: &AppShell,
    page: &TeamPage,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "teams/show.html",
        app_page(shell, data!({ "page": page, "flash": page.flash })),
    )
}
