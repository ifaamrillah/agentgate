//! Data shell aplikasi (sidebar) untuk `layouts/app.html`: user, switcher workspace, daftar
//! project, dan izin Pengaturan. Setiap halaman ber-layout aplikasi menerima `shell`; datanya
//! dibangun sekali per request oleh `controllers::shell::app_shell`, bukan di setiap handler.
//!
//! Struct di sini sengaja berisi nilai polos (nama, slug/key, href) supaya tidak ada entity
//! utuh yang sampai ke template.
use serde::Serialize;

use super::nav::{self, NavItem};
use crate::models::users;

/// User yang login, seperlunya untuk menu profil.
#[derive(Debug, Clone, Serialize)]
pub struct UserView {
    pub name: String,
    pub email: String,
    /// Inisial avatar: huruf pertama dua kata pertama nama, atau huruf pertama email.
    pub initials: String,
}

impl UserView {
    #[must_use]
    pub fn new(name: &str, email: &str) -> Self {
        Self {
            name: name.to_string(),
            email: email.to_string(),
            initials: initials(name, email),
        }
    }
}

pub(crate) fn initials(name: &str, email: &str) -> String {
    let from_name: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    if from_name.is_empty() {
        email
            .chars()
            .next()
            .into_iter()
            .flat_map(char::to_uppercase)
            .collect()
    } else {
        from_name
    }
}

/// Satu workspace di switcher.
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceLink {
    pub name: String,
    pub slug: String,
    pub href: String,
}

impl WorkspaceLink {
    #[must_use]
    pub fn new(name: &str, slug: &str) -> Self {
        Self {
            name: name.to_string(),
            slug: slug.to_string(),
            href: workspace_href(slug),
        }
    }
}

/// Workspace yang sedang dibuka: judul switcher, tujuan Beranda, Pengaturan, dan "Keluar dari
/// workspace" (menu profil, terbuka untuk semua anggota). `deletion` terisi bila workspace
/// terjadwal hapus: layout menampilkan banner merah dengan tombol batalkan (hanya Owner yang
/// bisa membuka workspace seperti itu).
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceView {
    pub name: String,
    pub slug: String,
    pub href: String,
    pub settings_href: String,
    pub leave_href: String,
    pub deletion: Option<DeletionBanner>,
}

/// Banner workspace terjadwal hapus: kapan hapus permanen mulai (tanggal dan jam WIB) dan form
/// batalkan.
#[derive(Debug, Clone, Serialize)]
pub struct DeletionBanner {
    pub purge_at: String,
    pub cancel_action: String,
}

impl WorkspaceView {
    #[must_use]
    pub fn new(name: &str, slug: &str) -> Self {
        Self {
            name: name.to_string(),
            slug: slug.to_string(),
            href: workspace_href(slug),
            settings_href: format!("{}/settings", workspace_href(slug)),
            leave_href: format!("{}/leave", workspace_href(slug)),
            deletion: None,
        }
    }

    /// Workspace terjadwal hapus permanen mulai `purge_at` (`None` = tidak terjadwal).
    #[must_use]
    pub fn scheduled_for_deletion<Tz: chrono::TimeZone>(
        mut self,
        purge_at: Option<&chrono::DateTime<Tz>>,
    ) -> Self {
        self.deletion = purge_at.map(|at| DeletionBanner {
            purge_at: super::time::wib_datetime(at),
            cancel_action: format!("{}/settings/delete/cancel", self.href),
        });
        self
    }
}

/// Satu project di bagian PROJECT (dan daftar project beranda). `key` membedakan project
/// aktif; `archived` menandai project yang diarsipkan.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectLink {
    pub name: String,
    pub key: String,
    pub href: String,
    pub archived: bool,
    /// Deskripsi project (tabel beranda workspace); `""` = tanpa deskripsi.
    pub description: String,
}

impl ProjectLink {
    #[must_use]
    pub fn new(workspace_slug: &str, name: &str, key: &str) -> Self {
        Self {
            name: name.to_string(),
            key: key.to_string(),
            href: format!("{}/p/{key}", workspace_href(workspace_slug)),
            archived: false,
            description: String::new(),
        }
    }

    #[must_use]
    pub fn described(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    #[must_use]
    pub fn archived(mut self, archived: bool) -> Self {
        self.archived = archived;
        self
    }
}

fn workspace_href(slug: &str) -> String {
    format!("/w/{slug}")
}

/// Data sidebar untuk `layouts/app.html` (variabel `shell`).
#[derive(Debug, Clone, Serialize)]
pub struct AppShell {
    pub user: UserView,
    pub workspaces: Vec<WorkspaceLink>,
    pub current_workspace: Option<WorkspaceView>,
    pub projects: Vec<ProjectLink>,
    pub current_project: Option<ProjectLink>,
    /// Tampilkan Pengaturan workspace (butuh `current_workspace`).
    pub can_manage_workspace: bool,
    /// Menu kiri Pengaturan workspace: halaman yang boleh dibuka role (`None` bila role tidak
    /// punya izin Pengaturan atau di luar workspace). Bagian Umum = `current_workspace.settings_href`.
    pub settings: Option<SettingsLinks>,
    /// Bagian halaman yang sedang dibuka (`views::nav::SETTINGS_SECTIONS` atau
    /// `ACCOUNT_SECTIONS`); `""` = tidak ada.
    pub section: &'static str,
    /// Kelompok bagian untuk breadcrumb: `settings`, `account`, atau `""`.
    pub area: &'static str,
    /// Menu kiri halaman ini (Pengaturan workspace atau Akun), item `section` ditandai aktif;
    /// kosong = halaman tanpa menu kiri.
    pub subnav: Vec<NavItem>,
}

/// Tautan halaman Pengaturan workspace; `None` bila role tidak membuka halaman itu.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SettingsLinks {
    pub members: Option<String>,
    pub roles: Option<String>,
    pub job_titles: Option<String>,
    pub teams: Option<String>,
    pub audit: Option<String>,
}

impl AppShell {
    /// Shell tanpa workspace dan project.
    #[must_use]
    pub fn new(user: UserView) -> Self {
        Self {
            user,
            workspaces: Vec::new(),
            current_workspace: None,
            projects: Vec::new(),
            current_project: None,
            can_manage_workspace: false,
            settings: None,
            section: "",
            area: "",
            subnav: Vec::new(),
        }
    }

    #[must_use]
    pub fn for_user(user: &users::Model) -> Self {
        Self::new(UserView::new(&user.name, &user.email))
    }

    #[must_use]
    pub fn with_workspaces(mut self, workspaces: Vec<WorkspaceLink>) -> Self {
        self.workspaces = workspaces;
        self
    }

    #[must_use]
    pub fn with_current_workspace(mut self, workspace: WorkspaceView) -> Self {
        self.current_workspace = Some(workspace);
        self
    }

    #[must_use]
    pub fn with_projects(mut self, projects: Vec<ProjectLink>) -> Self {
        self.projects = projects;
        self
    }

    #[must_use]
    pub fn with_current_project(mut self, project: ProjectLink) -> Self {
        self.current_project = Some(project);
        self
    }

    #[must_use]
    pub fn can_manage_workspace(mut self, allowed: bool) -> Self {
        self.can_manage_workspace = allowed;
        self
    }

    #[must_use]
    pub fn with_settings(mut self, links: Option<SettingsLinks>) -> Self {
        self.settings = links;
        self
    }

    /// Tandai bagian halaman yang sedang dibuka dan pasang menu kirinya: bagian Akun → menu
    /// Akun; bagian Pengaturan (di workspace, dengan izin Pengaturan) → menu Pengaturan.
    #[must_use]
    pub fn at(mut self, section: &'static str) -> Self {
        self.section = section;
        if nav::ACCOUNT_SECTIONS.contains(&section) {
            self.area = "account";
            self.subnav = nav::mark(nav::account_nav(), section);
        } else if nav::SETTINGS_SECTIONS.contains(&section) {
            self.area = "settings";
            if let (Some(links), Some(ws)) = (&self.settings, &self.current_workspace) {
                self.subnav = nav::mark(nav::settings_nav(&ws.settings_href, links), section);
            }
        }
        self
    }
}

/// Data halaman ber-layout aplikasi: field halaman ditambah `shell`. Dipakai fungsi render di
/// `views/` supaya setiap template `layouts/app.html` menerima shell dengan cara yang sama.
#[derive(Debug, Serialize)]
pub struct AppPage<'a, T: Serialize> {
    pub shell: &'a AppShell,
    #[serde(flatten)]
    pub page: T,
}

#[must_use]
pub fn app_page<T: Serialize>(shell: &AppShell, page: T) -> AppPage<'_, T> {
    AppPage { shell, page }
}
