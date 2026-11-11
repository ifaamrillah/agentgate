//! Data navigasi halaman (tab project, menu kiri Pengaturan/Akun/Pengaturan project) untuk
//! komponen `tabs` dan `subnav`. Satu-satunya tempat daftar menu ditulis: template hanya
//! merender `items`, tidak pernah menulis ulang daftarnya.
use serde::Serialize;

use super::{layout::SettingsLinks, projects::ProjectPaths};

/// Satu item tab atau menu kiri. `key` dirender sebagai `data-nav` (penanda tes dan bagian aktif);
/// `href` `None` = belum tersedia (`soon`, tidak bisa diklik).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NavItem {
    pub key: &'static str,
    pub label_key: &'static str,
    pub icon: &'static str,
    pub href: Option<String>,
    pub current: bool,
    pub soon: bool,
}

impl NavItem {
    #[must_use]
    pub fn link(
        key: &'static str,
        label_key: &'static str,
        icon: &'static str,
        href: &str,
    ) -> Self {
        Self {
            key,
            label_key,
            icon,
            href: Some(href.to_string()),
            current: false,
            soon: false,
        }
    }

    /// Item fitur yang menunggu milestone-nya: tampil dengan label "segera", tanpa tautan.
    #[must_use]
    pub fn soon(key: &'static str, label_key: &'static str, icon: &'static str) -> Self {
        Self {
            key,
            label_key,
            icon,
            href: None,
            current: false,
            soon: true,
        }
    }
}

/// Tandai item `current` (yang lain tidak).
#[must_use]
pub fn mark(mut items: Vec<NavItem>, current: &str) -> Vec<NavItem> {
    for item in &mut items {
        item.current = item.key == current;
    }
    items
}

/// Bagian Pengaturan workspace (kunci = `AppShell::section`).
pub const SETTINGS_SECTIONS: &[&str] = &[
    "general",
    "members",
    "teams",
    "job_titles",
    "roles",
    "audit",
];
/// Bagian Akun.
pub const ACCOUNT_SECTIONS: &[&str] = &["profile", "password", "two_factor"];

/// Menu kiri Pengaturan workspace: Umum selalu ada (indeks Pengaturan), sisanya menurut izin role.
#[must_use]
pub fn settings_nav(settings_href: &str, links: &SettingsLinks) -> Vec<NavItem> {
    let mut items = vec![NavItem::link(
        "general",
        "workspace-settings-general",
        "settings",
        settings_href,
    )];
    let optional = [
        ("members", "settings-members-title", "users", &links.members),
        ("teams", "settings-teams-title", "users-group", &links.teams),
        (
            "job_titles",
            "settings-job-titles-title",
            "id-badge-2",
            &links.job_titles,
        ),
        ("roles", "settings-roles-title", "key", &links.roles),
        (
            "audit",
            "settings-audit-title",
            "list-details",
            &links.audit,
        ),
    ];
    items.extend(optional.into_iter().filter_map(|(key, label, icon, href)| {
        href.as_deref().map(|h| NavItem::link(key, label, icon, h))
    }));
    items
}

/// Menu kiri Akun.
#[must_use]
pub fn account_nav() -> Vec<NavItem> {
    vec![
        NavItem::link("profile", "account-profile", "user", "/account"),
        NavItem::link("password", "account-password", "lock", "/account/password"),
        NavItem::link(
            "two_factor",
            "account-two-factor",
            "shield-check",
            "/account/2fa",
        ),
    ]
}

/// Tab halaman project. Board, Backlog, Knowledge, AI Office menunggu milestone-nya; Pengaturan
/// hanya untuk Admin project (`settings_href` = `None` untuk lainnya).
#[must_use]
pub fn project_tabs(home_href: &str, settings_href: Option<&str>, current: &str) -> Vec<NavItem> {
    let mut items = vec![
        NavItem::link("overview", "project-tab-overview", "home", home_href),
        NavItem::soon("board", "nav-board", "layout-dashboard"),
        NavItem::soon("backlog", "nav-backlog", "ticket"),
        NavItem::soon("knowledge", "nav-knowledge", "book"),
        NavItem::soon("ai_office", "nav-ai-office", "robot"),
    ];
    if let Some(href) = settings_href {
        items.push(NavItem::link("settings", "nav-settings", "settings", href));
    }
    mark(items, current)
}

/// Menu kiri Pengaturan project.
#[must_use]
pub fn project_settings_nav(paths: &ProjectPaths, current: &str) -> Vec<NavItem> {
    mark(
        vec![
            NavItem::link(
                "general",
                "project-settings-general",
                "settings",
                &paths.settings,
            ),
            NavItem::link("access", "project-settings-access", "users", &paths.access),
            NavItem::link(
                "repos",
                "project-settings-repos",
                "git-branch",
                &paths.repos,
            ),
        ],
        current,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_nav_follows_links_and_marks_current() {
        let links = SettingsLinks {
            members: Some("/m".into()),
            audit: Some("/a".into()),
            ..SettingsLinks::default()
        };
        let items = mark(settings_nav("/s", &links), "audit");
        let keys: Vec<_> = items.iter().map(|i| i.key).collect();
        assert_eq!(keys, ["general", "members", "audit"]);
        assert!(items.iter().all(|i| i.current == (i.key == "audit")));
    }

    #[test]
    fn project_tabs_hide_settings_for_non_admin() {
        let tabs = project_tabs("/p", None, "overview");
        assert!(tabs.iter().all(|t| t.key != "settings"));
        assert!(tabs.iter().filter(|t| t.soon).all(|t| t.href.is_none()));
        assert!(tabs[0].current);
    }
}
