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
    nav::ProjectViews,
    palette,
    ui::ComboOption,
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
pub const FLASH_LOGO_SAVED: &str = "flash-project-logo-saved";
pub const FLASH_LOGO_REMOVED: &str = "flash-project-logo-removed";
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
    FLASH_LOGO_SAVED,
    FLASH_LOGO_REMOVED,
];

pub const PROJECT_FIELDS: &[&str] = &["key", "name", "description", "color"];
/// Field form unggah logo.
pub const LOGO_FIELDS: &[&str] = &["logo"];
pub const REPO_FIELDS: &[&str] = &["name", "provider", "url", "default_branch"];

/// Path halaman project `/w/{slug}/p/{key}` dan turunannya.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectPaths {
    pub home: String,
    pub settings: String,
    pub archive: String,
    pub access: String,
    pub repos: String,
    /// GET gambar logo (cek akses project).
    pub logo: String,
    /// POST unggah logo (multipart) dan POST hapus logo.
    pub logo_upload: String,
    pub logo_delete: String,
    /// Pengaturan project → Kode tiket.
    pub ticket_code: String,
    /// Pengaturan project → Workflow.
    pub workflow: String,
    /// Pengaturan project → Konvensi git (M6).
    pub git: String,
    /// Tab project → Pull request (M6).
    pub prs: String,
    /// Tab project → Code graph (M7): cari simbol; `graph_index` = status index.
    pub graph: String,
    pub graph_index: String,
    /// Tab project → Knowledge (M8).
    pub knowledge: String,
    /// Pengaturan project → Components (M9).
    pub components: String,
    /// Pengaturan project → Penghemat token (M10).
    pub tokens: String,
}

impl ProjectPaths {
    #[must_use]
    pub fn new(workspace_slug: &str, key: &str) -> Self {
        Self::from_home(&format!("/w/{workspace_slug}/p/{key}"))
    }

    /// Path dari URL beranda project (`/w/{slug}/p/{key}`).
    #[must_use]
    pub fn from_home(home: &str) -> Self {
        let home = home.to_string();
        Self {
            settings: format!("{home}/settings"),
            archive: format!("{home}/settings/archive"),
            access: format!("{home}/settings/access"),
            repos: format!("{home}/settings/repos"),
            logo: format!("{home}/logo"),
            logo_upload: format!("{home}/settings/logo"),
            logo_delete: format!("{home}/settings/logo/delete"),
            ticket_code: format!("{home}/settings/ticket-code"),
            workflow: format!("{home}/settings/workflow"),
            git: format!("{home}/settings/git"),
            prs: format!("{home}/prs"),
            graph: format!("{home}/graph"),
            graph_index: format!("{home}/graph/index"),
            knowledge: format!("{home}/knowledge"),
            components: format!("{home}/settings/components"),
            tokens: format!("{home}/settings/tokens"),
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
    /// Warna tersimpan atau isian (`""` = warna palet bawaan).
    pub color: String,
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
            color: String::new(),
            errors: field_errors(PROJECT_FIELDS, errors),
        }
    }

    /// Isian warna (dari form atau tersimpan).
    #[must_use]
    pub fn colored(mut self, color: &str) -> Self {
        self.color = color.trim().to_ascii_lowercase();
        self
    }

    #[must_use]
    pub fn of(project: &projects::Model) -> Self {
        Self::new(&project.key, &project.name, &project.description, None)
            .colored(project.color.as_deref().unwrap_or_default())
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
    /// Logo: gambar (`logo_href`, `""` = belum ada) atau inisial di atas `color`.
    pub initials: String,
    pub color: String,
    pub logo_href: String,
    /// Warna disimpan sendiri (bukan warna palet bawaan dari key).
    pub custom_color: bool,
}

impl ProjectSummary {
    #[must_use]
    pub fn new(project: &projects::Model, role_label: &str, paths: &ProjectPaths) -> Self {
        Self {
            key: project.key.clone(),
            name: project.name.clone(),
            description: project.description.clone(),
            archived: project.is_archived(),
            role_label: role_label.to_string(),
            initials: palette::initials(&project.name, &project.key),
            color: color_of(project),
            logo_href: logo_href(&paths.home, project).unwrap_or_default(),
            custom_color: project.color.is_some(),
        }
    }
}

/// Warna tampil project: tersimpan, atau warna palet dari key.
#[must_use]
pub fn color_of(project: &projects::Model) -> String {
    project
        .color
        .clone()
        .unwrap_or_else(|| palette::color_for(&project.key).to_string())
}

/// URL gambar logo project (`home` = `/w/{slug}/p/{key}`), dengan versi dari key storage supaya
/// logo baru tidak tertahan cache browser; `None` = belum ada logo.
#[must_use]
pub fn logo_href(home: &str, project: &projects::Model) -> Option<String> {
    let logo = project.logo()?;
    let version = logo.key.rsplit('-').next().unwrap_or_default();
    Some(format!("{home}/logo?v={version}"))
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

/// Tujuan POST preferensi tab tampilan project (`controllers::account`).
pub const PROJECT_VIEWS_ACTION: &str = "/account/project-views";

/// Satu orang/tim dengan akses project (kartu Anggota di ringkasan).
#[derive(Debug, Serialize)]
pub struct MemberRow {
    pub is_team: bool,
    pub name: String,
    pub detail: String,
    pub role_name: String,
    pub initials: String,
    pub color: &'static str,
}

impl MemberRow {
    #[must_use]
    pub fn of(grant: &GrantRow) -> Self {
        Self {
            is_team: grant.is_team,
            name: grant.subject_name.clone(),
            detail: grant.subject_detail.clone(),
            role_name: grant.role_name.clone(),
            initials: palette::initials(&grant.subject_name, &grant.subject_detail),
            color: palette::color_for(if grant.subject_detail.is_empty() {
                &grant.subject_name
            } else {
                &grant.subject_detail
            }),
        }
    }
}

/// Data `projects/show.html`.
#[derive(Debug, Serialize)]
pub struct ShowPage<'a> {
    pub project: &'a ProjectSummary,
    pub repos: &'a [RepoView],
    pub members: Vec<MemberRow>,
    /// Tautan pengaturan, hanya untuk Admin project.
    pub paths: Option<&'a ProjectPaths>,
    pub views: ProjectViews,
    /// Jumlah tiket per status (kolom board), dengan tautan ke daftar terfilter.
    pub ticket_stats: Vec<TicketStat>,
}

/// Satu baris kartu Progres di ringkasan project.
#[derive(Debug, Serialize)]
pub struct TicketStat {
    pub status: super::tickets::StatusView,
    pub count: i64,
    /// Persentase dari total (0–100) untuk bar.
    pub percent: i64,
    pub href: String,
}

/// Ringkasan project. `pinned` = tampilan yang disematkan user (`users.project_views`).
pub fn show(v: &impl ViewRenderer, shell: &AppShell, page: &ShowPage<'_>) -> Result<Response> {
    let home = &page.project_home();
    format::render().view(
        v,
        "projects/show.html",
        app_page(
            shell,
            data!({
                "project": page.project,
                "repos": page.repos,
                "members": page.members,
                "paths": page.paths,
                "views": page.views,
                "views_action": PROJECT_VIEWS_ACTION,
                "return_to": home,
                "ticket_stats": page.ticket_stats,
                "ticket_total": page.ticket_stats.iter().map(|s| s.count).sum::<i64>(),
                "board_href": format!("{home}/board"),
                "new_ticket_href": format!("{home}/tickets/new"),
            }),
        ),
    )
}

impl ShowPage<'_> {
    fn project_home(&self) -> String {
        self.views
            .all
            .iter()
            .find(|v| v.key == "overview")
            .and_then(|v| v.href.clone())
            .unwrap_or_default()
    }
}

/// Isi halaman Pengaturan project → Umum.
#[derive(Debug, Serialize)]
pub struct SettingsPage<'a> {
    pub project: &'a ProjectSummary,
    pub paths: &'a ProjectPaths,
    pub form: &'a ProjectFormView,
    /// Kode error unggah logo; `""` = tanpa.
    pub logo_error: &'a str,
}

/// Pengaturan project → Umum (logo, nama, deskripsi, warna, arsip).
pub fn settings(
    v: &impl ViewRenderer,
    shell: &AppShell,
    page: &SettingsPage<'_>,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "projects/settings.html",
        app_page(
            shell,
            data!({
                "project": page.project,
                "paths": page.paths,
                "form": page.form,
                "logo_error": page.logo_error,
                "flash": flash,
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

/// Data `projects/access.html`. `subjects` = subjek yang belum punya grant; `roles` = role
/// yang boleh diberikan pelaku.
#[derive(Debug, Serialize)]
pub struct AccessPage {
    pub grants: Vec<GrantRow>,
    /// Tim dan orang yang belum punya grant, untuk combobox (grup Tim / Orang).
    pub subjects: Vec<ComboOption>,
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
