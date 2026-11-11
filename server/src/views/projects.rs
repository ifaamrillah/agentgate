//! Halaman project: buat project, beranda project (ringkasan, repo, board menyusul di M2), dan
//! pengaturan project (Umum, Akses, Repo) dengan navigasi tab yang sama.
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use sea_orm::{ActiveEnum, Iterable};
use serde::Serialize;
use uuid::Uuid;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
    layout::{AppShell, app_page},
    members::OptionView,
    nav,
};
use crate::models::{
    project_grants::{self, GrantSubject},
    projects,
    repos::{self, RepoInput, RepoProvider},
    roles,
};

pub const FLASH_SAVED: &str = "flash-project-saved";
pub const FLASH_ARCHIVED: &str = "flash-project-archived";
pub const FLASH_ACCESS_GRANTED: &str = "flash-project-access-granted";
pub const FLASH_ACCESS_CHANGED: &str = "flash-project-access-changed";
pub const FLASH_ACCESS_REVOKED: &str = "flash-project-access-revoked";
pub const FLASH_REPO_ADDED: &str = "flash-repo-added";
pub const FLASH_REPO_UPDATED: &str = "flash-repo-updated";
pub const FLASH_REPO_REMOVED: &str = "flash-repo-removed";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_SAVED,
    FLASH_ARCHIVED,
    FLASH_ACCESS_GRANTED,
    FLASH_ACCESS_CHANGED,
    FLASH_ACCESS_REVOKED,
    FLASH_REPO_ADDED,
    FLASH_REPO_UPDATED,
    FLASH_REPO_REMOVED,
];

pub const PROJECT_FIELDS: &[&str] = &["key", "name", "description"];
pub const REPO_FIELDS: &[&str] = &["name", "provider", "url", "default_branch"];

/// Path halaman project `/w/{slug}/p/{key}` dan turunannya.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectPaths {
    pub home: String,
    pub settings: String,
    pub archive: String,
    pub access: String,
    pub repos: String,
}

impl ProjectPaths {
    #[must_use]
    pub fn new(workspace_slug: &str, key: &str) -> Self {
        let home = format!("/w/{workspace_slug}/p/{key}");
        Self {
            settings: format!("{home}/settings"),
            archive: format!("{home}/settings/archive"),
            access: format!("{home}/settings/access"),
            repos: format!("{home}/settings/repos"),
            home,
        }
    }
}

/// Isian form project (buat: key, nama, deskripsi; pengaturan: nama, deskripsi).
#[derive(Debug, Serialize)]
pub struct ProjectFormView {
    pub key: String,
    pub name: String,
    pub description: String,
    pub errors: FieldErrors,
}

impl ProjectFormView {
    #[must_use]
    pub fn new(
        key: &str,
        name: &str,
        description: &str,
        errors: Option<&ModelValidationErrors>,
    ) -> Self {
        Self {
            key: key.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            errors: field_errors(PROJECT_FIELDS, errors),
        }
    }

    #[must_use]
    pub fn of(project: &projects::Model) -> Self {
        Self::new(&project.key, &project.name, &project.description, None)
    }
}

/// Ringkasan project untuk judul halaman project.
#[derive(Debug, Serialize)]
pub struct ProjectSummary {
    pub key: String,
    pub name: String,
    pub description: String,
    pub archived: bool,
    /// Label izin project user (nama role, atau gabungannya).
    pub role_label: String,
}

impl ProjectSummary {
    #[must_use]
    pub fn new(project: &projects::Model, role_label: &str) -> Self {
        Self {
            key: project.key.clone(),
            name: project.name.clone(),
            description: project.description.clone(),
            archived: project.is_archived(),
            role_label: role_label.to_string(),
        }
    }
}

/// Satu repo di beranda project.
#[derive(Debug, Serialize)]
pub struct RepoView {
    pub name: String,
    pub provider_key: &'static str,
    pub url: String,
    pub default_branch: String,
}

impl RepoView {
    #[must_use]
    pub fn new(repo: &repos::Model) -> Self {
        Self {
            name: repo.name.clone(),
            provider_key: provider_key(repo.provider().ok()),
            url: repo.url.clone(),
            default_branch: repo.default_branch.clone(),
        }
    }
}

/// Kunci Fluent nama penyedia repo.
#[must_use]
pub fn provider_key(provider: Option<RepoProvider>) -> &'static str {
    match provider {
        Some(RepoProvider::Github) => "repo-provider-github",
        Some(RepoProvider::Gitlab) => "repo-provider-gitlab",
        Some(RepoProvider::Bitbucket) => "repo-provider-bitbucket",
        Some(RepoProvider::Other) | None => "repo-provider-other",
    }
}

/// Pilihan `<select>` penyedia: nilai disimpan dan kunci Fluent labelnya.
#[derive(Debug, Serialize)]
pub struct ProviderOption {
    pub value: String,
    pub label_key: &'static str,
}

#[must_use]
pub fn provider_options() -> Vec<ProviderOption> {
    RepoProvider::iter()
        .map(|p| ProviderOption {
            value: p.to_value(),
            label_key: provider_key(Some(p)),
        })
        .collect()
}

pub fn new(
    v: &impl ViewRenderer,
    shell: &AppShell,
    form: &ProjectFormView,
    action: &str,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "projects/new.html",
        app_page(shell, data!({ "form": form, "action": action })),
    )
}

/// Tab project untuk halaman di dalam project (`shell.current_project` terisi).
fn tabs(shell: &AppShell, paths: Option<&ProjectPaths>, current: &str) -> Vec<nav::NavItem> {
    let home = shell
        .current_project
        .as_ref()
        .map_or("", |p| p.href.as_str());
    nav::project_tabs(home, paths.map(|p| p.settings.as_str()), current)
}

/// Beranda project. `paths` hanya untuk Admin project (tautan pengaturan), `None` untuk lainnya.
pub fn show(
    v: &impl ViewRenderer,
    shell: &AppShell,
    project: &ProjectSummary,
    repos: &[RepoView],
    paths: Option<&ProjectPaths>,
) -> Result<Response> {
    format::render().view(
        v,
        "projects/show.html",
        app_page(
            shell,
            data!({
                "project": project,
                "repos": repos,
                "paths": paths,
                "tabs": tabs(shell, paths, "overview"),
            }),
        ),
    )
}

/// Pengaturan project → Umum (nama, deskripsi, arsip).
pub fn settings(
    v: &impl ViewRenderer,
    shell: &AppShell,
    project: &ProjectSummary,
    paths: &ProjectPaths,
    form: &ProjectFormView,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "projects/settings.html",
        app_page(
            shell,
            data!({
                "project": project,
                "paths": paths,
                "form": form,
                "flash": flash,
                "tabs": tabs(shell, Some(paths), "settings"),
                "subnav": nav::project_settings_nav(paths, "general"),
            }),
        ),
    )
}

/// Satu grant di Pengaturan project → Akses. `manageable` = pelaku boleh mengganti role dan
/// mencabutnya (`project_access::can_grant`, dihitung controller).
#[derive(Debug, Serialize)]
pub struct GrantRow {
    pub is_team: bool,
    pub subject_name: String,
    pub subject_detail: String,
    pub role_id: String,
    pub role_name: String,
    pub manageable: bool,
    pub role_action: String,
    pub revoke_action: String,
}

impl GrantRow {
    /// `base` = path halaman akses (`…/settings/access`).
    /// `subject` = (nama, keterangan) tim atau orang pada grant itu.
    #[must_use]
    pub fn new(
        grant: &project_grants::Model,
        subject: (&str, &str),
        role: &roles::Model,
        manageable: bool,
        base: &str,
    ) -> Self {
        let id = grant.id;
        Self {
            is_team: grant.team_id.is_some(),
            subject_name: subject.0.to_string(),
            subject_detail: subject.1.to_string(),
            role_id: role.id.to_string(),
            role_name: role.name.clone(),
            manageable,
            role_action: format!("{base}/{id}/role"),
            revoke_action: format!("{base}/{id}/revoke"),
        }
    }
}

/// Nilai pilihan subjek di form beri akses (`team:<id>` atau `user:<id>`).
#[must_use]
pub fn subject_value(subject: GrantSubject) -> String {
    match subject {
        GrantSubject::Team(id) => format!("team:{id}"),
        GrantSubject::User(id) => format!("user:{id}"),
    }
}

/// Kebalikan [`subject_value`]; `None` untuk nilai lain (form diubah di luar browser).
#[must_use]
pub fn parse_subject(raw: &str) -> Option<GrantSubject> {
    let (kind, id) = raw.trim().split_once(':')?;
    let id = Uuid::try_parse(id).ok()?;
    match kind {
        "team" => Some(GrantSubject::Team(id)),
        "user" => Some(GrantSubject::User(id)),
        _ => None,
    }
}

/// Data `projects/access.html`. `teams`/`people` = subjek yang belum punya grant; `roles` = role
/// yang boleh diberikan pelaku.
#[derive(Debug, Serialize)]
pub struct AccessPage {
    pub grants: Vec<GrantRow>,
    pub teams: Vec<OptionView>,
    pub people: Vec<OptionView>,
    pub roles: Vec<OptionView>,
    pub action: String,
    pub flash: Option<Flash>,
}

pub fn access(
    v: &impl ViewRenderer,
    shell: &AppShell,
    project: &ProjectSummary,
    paths: &ProjectPaths,
    page: &AccessPage,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "projects/access.html",
        app_page(
            shell,
            data!({
                "project": project,
                "paths": paths,
                "page": page,
                "flash": page.flash,
                "tabs": tabs(shell, Some(paths), "settings"),
                "subnav": nav::project_settings_nav(paths, "access"),
            }),
        ),
    )
}

/// Isian form repo dengan error per field. `field_id` = awalan id input (`repo` untuk form
/// tambah, `repo-<id>` untuk baris ubah).
#[derive(Debug, Serialize)]
pub struct RepoFormView {
    pub field_id: String,
    pub name: String,
    pub provider: String,
    pub url: String,
    pub default_branch: String,
    pub action: String,
    pub delete_action: String,
    pub errors: FieldErrors,
}

impl RepoFormView {
    /// Form tambah kosong (penyedia GitHub, branch `main`).
    #[must_use]
    pub fn empty(action: &str) -> Self {
        Self::from_input(
            "repo",
            &RepoInput {
                name: String::new(),
                provider: RepoProvider::Github.to_value(),
                url: String::new(),
                default_branch: "main".to_string(),
            },
            action,
            "",
            None,
        )
    }

    /// Baris ubah repo yang tersimpan; `base` = path halaman repo.
    #[must_use]
    pub fn of(repo: &repos::Model, base: &str) -> Self {
        Self::from_input(
            &format!("repo-{}", repo.id),
            &RepoInput {
                name: repo.name.clone(),
                provider: repo.provider.clone(),
                url: repo.url.clone(),
                default_branch: repo.default_branch.clone(),
            },
            &format!("{base}/{}", repo.id),
            &format!("{base}/{}/delete", repo.id),
            None,
        )
    }

    #[must_use]
    pub fn from_input(
        field_id: &str,
        input: &RepoInput,
        action: &str,
        delete_action: &str,
        errors: Option<&ModelValidationErrors>,
    ) -> Self {
        Self {
            field_id: field_id.to_string(),
            name: input.name.trim().to_string(),
            provider: input.provider.trim().to_string(),
            url: input.url.trim().to_string(),
            default_branch: input.default_branch.trim().to_string(),
            action: action.to_string(),
            delete_action: delete_action.to_string(),
            errors: field_errors(REPO_FIELDS, errors),
        }
    }
}

/// Data `projects/repos.html`.
#[derive(Debug, Serialize)]
pub struct ReposPage {
    pub repos: Vec<RepoFormView>,
    pub create: RepoFormView,
    pub providers: Vec<ProviderOption>,
    pub flash: Option<Flash>,
}

pub fn repos(
    v: &impl ViewRenderer,
    shell: &AppShell,
    project: &ProjectSummary,
    paths: &ProjectPaths,
    page: &ReposPage,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "projects/repos.html",
        app_page(
            shell,
            data!({
                "project": project,
                "paths": paths,
                "page": page,
                "flash": page.flash,
                "tabs": tabs(shell, Some(paths), "settings"),
                "subnav": nav::project_settings_nav(paths, "repos"),
            }),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_value_round_trips_and_rejects_others() {
        let id = Uuid::now_v7();
        for subject in [GrantSubject::Team(id), GrantSubject::User(id)] {
            assert_eq!(parse_subject(&subject_value(subject)), Some(subject));
        }
        for bad in ["", "team", "role:x", &format!("group:{id}"), "user:nope"] {
            assert_eq!(parse_subject(bad), None, "{bad}");
        }
    }
}
