//! Data navigasi halaman (tab project, menu kiri Pengaturan/Akun/Pengaturan project) untuk
//! komponen `tabs` dan `subnav`. Satu-satunya tempat daftar menu ditulis: template hanya
//! merender `items`, tidak pernah menulis ulang daftarnya.
use serde::Serialize;

use super::{
    layout::{AppShell, SettingsLinks},
    projects::ProjectPaths,
};

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
    /// Baris kedua di menu subpage (Fluent), mis. "Logo, nama, alamat URL".
    pub desc_key: Option<&'static str>,
    /// Angka badge (tab dan subpage); `None` = tanpa badge.
    pub count: Option<u64>,
    /// Badge berwarna peringatan (mis. anggota tanpa 2FA).
    pub alert: bool,
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
            desc_key: None,
            count: None,
            alert: false,
        }
    }

    /// Baris kedua (Fluent) untuk menu subpage.
    #[must_use]
    pub fn describe(mut self, desc_key: &'static str) -> Self {
        self.desc_key = Some(desc_key);
        self
    }

    /// Angka badge; `alert` = warna peringatan.
    #[must_use]
    pub fn counted(mut self, count: u64, alert: bool) -> Self {
        self.count = Some(count);
        self.alert = alert;
        self
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
            desc_key: None,
            count: None,
            alert: false,
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
    "workflows",
    "ticket_types",
    "ticket_code",
    "personas",
    "skills",
    "runner",
];
/// Bagian Akun.
pub const ACCOUNT_SECTIONS: &[&str] = &["profile", "password", "two_factor", "connect"];
/// Bagian Pengaturan project (sidebar menu Pengaturan, project aktif terbuka).
pub const PROJECT_SETTINGS_SECTIONS: &[&str] = &[
    "project_general",
    "project_access",
    "project_repos",
    "project_workflow",
    "project_ticket_code",
];

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
        (
            "workflows",
            "settings-workflows-title",
            "git-merge",
            &links.workflows,
        ),
        (
            "ticket_types",
            "settings-ticket-types-title",
            "category-2",
            &links.ticket_types,
        ),
        (
            "ticket_code",
            "settings-ticket-code-title",
            "hash",
            &links.ticket_code,
        ),
        (
            "personas",
            "settings-personas-title",
            "robot",
            &links.personas,
        ),
        ("skills", "settings-skills-title", "package", &links.skills),
        ("runner", "runner-title", "device-laptop", &links.runner),
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
        NavItem::link(
            "connect",
            "account-connect",
            "plug-connected",
            "/account/connect",
        ),
    ]
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
            NavItem::link(
                "workflow",
                "settings-workflows-title",
                "git-merge",
                &paths.workflow,
            ),
            NavItem::link(
                "ticket_code",
                "settings-ticket-code-title",
                "hash",
                &paths.ticket_code,
            ),
        ],
        current,
    )
}

/// Satu item sidebar. Label dari Fluent (`label_key`) atau teks apa adanya (`label`, mis. nama
/// project). `color` = titik warna project di depan label (menggantikan ikon). `href` `None` =
/// belum tersedia (tampil redup dengan "segera").
#[derive(Debug, Clone, Serialize)]
pub struct SideItem {
    pub key: String,
    pub label_key: Option<&'static str>,
    pub label: String,
    pub icon: Option<&'static str>,
    pub color: Option<String>,
    pub href: Option<String>,
    pub current: bool,
    /// Sub-item (menjorok), mis. bagian di bawah project aktif.
    pub sub: bool,
    /// Angka badge (mis. notifikasi belum dibaca); `None` = tanpa.
    pub count: Option<u64>,
}

impl SideItem {
    fn nav(item: &NavItem, sub: bool) -> Self {
        Self {
            key: item.key.to_string(),
            label_key: Some(item.label_key),
            label: String::new(),
            icon: Some(item.icon),
            color: None,
            href: item.href.clone(),
            current: item.current,
            sub,
            count: item.count,
        }
    }
}

/// Kelompok sidebar: judul opsional (Fluent `title_key` dengan argumen `title_arg`), tombol tambah
/// opsional (`add_href`, mis. Project baru), dan pesan bila kosong.
#[derive(Debug, Clone, Serialize)]
pub struct SideGroup {
    pub title_key: Option<&'static str>,
    pub title_arg: String,
    pub add_href: Option<String>,
    pub empty_key: Option<&'static str>,
    pub items: Vec<SideItem>,
}

impl SideGroup {
    fn new(title_key: Option<&'static str>, items: Vec<SideItem>) -> Self {
        Self {
            title_key,
            title_arg: String::new(),
            add_href: None,
            empty_key: None,
            items,
        }
    }
}

/// Menu sidebar untuk `shell`: menu aplikasi (Beranda + project, project aktif terbuka dengan
/// tampilan yang menunggu milestone-nya), atau menu Pengaturan (Akun, Workspace, Project) saat
/// halaman berada di Pengaturan/Akun (aturan UX 15: tidak ada item "Pengaturan" di sidebar).
#[must_use]
pub fn side_nav(shell: &AppShell) -> Vec<SideGroup> {
    if shell.in_settings() {
        settings_side(shell)
    } else {
        app_side(shell)
    }
}

fn app_side(shell: &AppShell) -> Vec<SideGroup> {
    let home_href = shell
        .current_workspace
        .as_ref()
        .map_or("/", |w| w.href.as_str());
    let mut home = NavItem::link("home", "nav-home", "home", home_href);
    home.current = shell.area == "home";
    let Some(ws) = shell.current_workspace.as_ref() else {
        return vec![SideGroup::new(None, vec![SideItem::nav(&home, false)])];
    };
    let mut inbox = NavItem::link("inbox", "nav-inbox", "inbox", &format!("{}/inbox", ws.href));
    if shell.inbox_unread > 0 {
        inbox = inbox.counted(shell.inbox_unread, false);
    }
    let my = NavItem::link(
        "my_tickets",
        "nav-my-tickets",
        "user",
        &format!("{}/my", ws.href),
    );
    let search = NavItem::link(
        "search",
        "nav-search",
        "search",
        &format!("{}/search", ws.href),
    );
    let top = mark(vec![home, inbox, my, search], shell.section)
        .into_iter()
        .map(|mut i| {
            i.current = i.current || (i.key == "home" && shell.area == "home");
            SideItem::nav(&i, false)
        })
        .collect();
    let mut groups = vec![SideGroup::new(None, top)];
    let current_key = shell.current_project.as_ref().map(|p| p.key.as_str());
    let mut items = Vec::new();
    for project in &shell.projects {
        let open = current_key == Some(project.key.as_str());
        items.push(SideItem {
            key: format!("project-{}", project.key),
            label_key: None,
            label: project.name.clone(),
            icon: None,
            color: Some(project.color.clone()),
            href: Some(project.href.clone()),
            current: open && shell.section == "project_overview",
            sub: false,
            count: None,
        });
        if open {
            items.extend(
                project_side_views(&project.href, shell.section)
                    .iter()
                    .map(|item| SideItem::nav(item, true)),
            );
        }
    }
    let mut projects = SideGroup::new(Some("nav-projects"), items);
    projects.add_href.clone_from(&shell.new_project_href);
    projects.empty_key = Some("nav-no-projects");
    groups.push(projects);
    groups
}

/// Bagian project di sidebar (di bawah project aktif): Board dan Daftar (M2), sisanya menunggu
/// milestone-nya.
fn project_side_views(home: &str, section: &str) -> Vec<NavItem> {
    let mut board = NavItem::link(
        "board",
        "nav-board",
        "layout-kanban",
        &format!("{home}/board"),
    );
    board.current = matches!(section, "project_board" | "project_ticket");
    let mut list = NavItem::link("list", "nav-list", "list", &format!("{home}/list"));
    list.current = section == "project_list";
    vec![
        board,
        list,
        NavItem::soon("knowledge", "nav-knowledge", "book"),
        NavItem::soon("code_graph", "nav-code-graph", "topology-star-3"),
    ]
}

fn settings_side(shell: &AppShell) -> Vec<SideGroup> {
    let back_href = shell.current_project.as_ref().map_or_else(
        || {
            shell
                .current_workspace
                .as_ref()
                .map_or("/", |w| w.href.as_str())
        },
        |p| p.href.as_str(),
    );
    let back = SideItem {
        key: "back".into(),
        label_key: Some("nav-back-to-app"),
        label: String::new(),
        icon: Some("chevron-left"),
        color: None,
        href: Some(back_href.to_string()),
        current: false,
        sub: false,
        count: None,
    };
    let mut groups = vec![SideGroup::new(None, vec![back])];
    let account = mark(account_nav(), shell.section);
    groups.push(SideGroup::new(
        Some("nav-group-account"),
        account.iter().map(|i| SideItem::nav(i, false)).collect(),
    ));
    if let (Some(links), Some(ws)) = (&shell.settings, &shell.current_workspace) {
        let items = mark(settings_nav(&ws.settings_href, links), shell.section);
        let mut group = SideGroup::new(
            Some("nav-group-workspace"),
            items.iter().map(|i| SideItem::nav(i, false)).collect(),
        );
        group.title_arg.clone_from(&ws.name);
        groups.push(group);
    }
    let current_key = shell.current_project.as_ref().map(|p| p.key.as_str());
    let mut items = Vec::new();
    for project in &shell.projects {
        let open = current_key == Some(project.key.as_str()) && shell.area == "project_settings";
        if !shell.admin_all_projects && !open {
            continue;
        }
        items.push(SideItem {
            key: format!("project-{}", project.key),
            label_key: None,
            label: project.name.clone(),
            icon: None,
            color: Some(project.color.clone()),
            href: Some(project.settings_href.clone()),
            current: false,
            sub: false,
            count: None,
        });
        if open {
            let section = shell.section.trim_start_matches("project_");
            let paths = ProjectPaths::from_home(&project.href);
            items.extend(
                project_settings_nav(&paths, section)
                    .iter()
                    .map(|i| SideItem::nav(i, true)),
            );
        }
    }
    if !items.is_empty() {
        groups.push(SideGroup::new(Some("nav-projects"), items));
    }
    groups
}

/// Tampilan project di bar tab (aturan UX 9): urutan bawaan; Ringkasan, Board, dan Daftar
/// tersedia (M2), sisanya menunggu milestone-nya.
pub const PROJECT_VIEWS: &[(&str, &str, &str)] = &[
    ("overview", "project-view-overview", "home"),
    ("board", "project-view-board", "layout-kanban"),
    ("list", "project-view-list", "list"),
    ("backlog", "project-view-backlog", "stack-2"),
    ("roadmap", "project-view-roadmap", "timeline"),
    ("calendar", "project-view-calendar", "calendar"),
    ("releases", "project-view-releases", "rocket"),
    ("reports", "project-view-reports", "chart-line"),
];
/// Kunci tampilan yang dikenal dari isian user, urut dan tanpa duplikat (POST "Atur tab").
#[must_use]
pub fn known_views<'a>(keys: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for key in keys {
        if PROJECT_VIEWS.iter().any(|(k, ..)| *k == key) && !out.iter().any(|o| o == key) {
            out.push(key.to_string());
        }
    }
    out
}

/// Tab yang disematkan bila user belum mengatur sendiri.
pub const DEFAULT_PINNED_VIEWS: &[&str] = &["overview", "board", "list", "backlog"];

/// Tab tampilan project untuk komponen `project_tabs`: `pinned` tampil di bar (urutan user, plus
/// tab aktif bila tidak disematkan), `more` masuk menu "Lainnya", `all` = semua dalam urutan user
/// (tab mobile dan dialog "Atur tab"), `pinned_keys` = kunci yang disematkan.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectViews {
    pub pinned: Vec<NavItem>,
    pub more: Vec<NavItem>,
    pub all: Vec<NavItem>,
    pub pinned_keys: Vec<&'static str>,
}

/// Bangun tab tampilan project dari preferensi user (`pinned`: kunci tersimpan, urut; kunci tak
/// dikenal diabaikan; kosong = bawaan).
#[must_use]
pub fn project_views(home_href: &str, pinned: &[String], current: &str) -> ProjectViews {
    let known = |k: &str| {
        PROJECT_VIEWS
            .iter()
            .find(|(key, ..)| *key == k)
            .map(|(key, ..)| *key)
    };
    let mut pinned_keys: Vec<&'static str> = pinned.iter().filter_map(|k| known(k)).collect();
    pinned_keys.dedup();
    if pinned_keys.is_empty() {
        pinned_keys = DEFAULT_PINNED_VIEWS.to_vec();
    }
    let item = |key: &'static str| {
        let (_, label, icon) = PROJECT_VIEWS
            .iter()
            .find(|(k, ..)| *k == key)
            .copied()
            .unwrap_or(PROJECT_VIEWS[0]);
        let mut nav = match key {
            "overview" => NavItem::link(key, label, icon, home_href),
            "board" | "list" => NavItem::link(key, label, icon, &format!("{home_href}/{key}")),
            _ => NavItem::soon(key, label, icon),
        };
        nav.current = key == current;
        nav
    };
    let mut order: Vec<&'static str> = pinned_keys.clone();
    order.extend(
        PROJECT_VIEWS
            .iter()
            .map(|(k, ..)| *k)
            .filter(|k| !pinned_keys.contains(k)),
    );
    let all: Vec<NavItem> = order.iter().map(|k| item(k)).collect();
    let mut shown: Vec<NavItem> = pinned_keys.iter().map(|k| item(k)).collect();
    if !pinned_keys.contains(&current) && known(current).is_some() {
        shown.push(item(known(current).unwrap_or("overview")));
    }
    let more = all
        .iter()
        .filter(|i| !shown.iter().any(|s| s.key == i.key))
        .cloned()
        .collect();
    ProjectViews {
        pinned: shown,
        more,
        all,
        pinned_keys,
    }
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
    fn project_views_default_pins_and_more_menu() {
        let v = project_views("/p", &[], "overview");
        let keys: Vec<_> = v.pinned.iter().map(|i| i.key).collect();
        assert_eq!(keys, DEFAULT_PINNED_VIEWS);
        assert_eq!(
            v.more.len(),
            PROJECT_VIEWS.len() - DEFAULT_PINNED_VIEWS.len()
        );
        assert!(v.pinned[0].current);
    }

    #[test]
    fn project_views_follow_user_order_and_ignore_unknown() {
        let v = project_views(
            "/p",
            &["reports".into(), "nope".into(), "overview".into()],
            "overview",
        );
        let keys: Vec<_> = v.pinned.iter().map(|i| i.key).collect();
        assert_eq!(keys, ["reports", "overview"]);
        assert_eq!(v.all[0].key, "reports");
    }
}
