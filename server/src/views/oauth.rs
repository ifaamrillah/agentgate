//! Tampilan OAuth MCP: halaman persetujuan (`oauth/consent.html`, halaman solo) dan Akun →
//! Hubungkan client AI (`account/connect.html`).

use agentgate_core::persona::{MCP_TOOLS, client_from_name};
use serde::Serialize;
use uuid::Uuid;

use super::{nav::NavItem, palette, time::wib_datetime, ui::ComboOption, ui::Stat};
use crate::models::{oauth_clients, oauth_grants, roles, users, workspaces};

pub const FLASH_REVOKED: &str = "connect-flash-revoked";
pub const WORKSPACE_REQUIRED: &str = "oauth-workspace-required";

/// Bagian halaman Hubungkan client AI.
pub const SECTIONS: &[&str] = &["steps", "clients", "files", "tools", "slash"];

/// Apa yang bisa dilakukan client (ikon, kunci Fluent); baris terakhir = yang tidak bisa.
pub const SCOPES: &[(&str, &str)] = &[
    ("ticket", "oauth-scope-read"),
    ("file-text", "oauth-scope-write"),
    ("route", "oauth-scope-transition"),
    ("message-question", "oauth-scope-ask"),
];

/// Halaman persetujuan.
#[derive(Debug, Clone, Serialize)]
pub struct ConsentPage {
    pub client: String,
    pub client_initial: String,
    pub client_uri: String,
    pub redirect_uri: String,
    pub user_name: String,
    pub user_email: String,
    pub user_initials: String,
    pub user_color: &'static str,
    pub workspaces: Vec<ComboOption>,
    /// Workspace terpilih (pertama bila hanya satu).
    pub selected: String,
    /// Tanpa workspace yang bisa dipilih: tombol Izinkan dimatikan.
    pub locked: bool,
    pub error: &'static str,
    pub scopes: Vec<Scope>,
    /// Parameter otorisasi asli, dikirim ulang sebagai hidden field.
    pub hidden: Vec<(&'static str, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Scope {
    pub icon: &'static str,
    pub key: &'static str,
}

impl ConsentPage {
    #[must_use]
    pub fn new(
        client: &oauth_clients::Model,
        user: &users::Model,
        choices: &[(workspaces::Model, roles::Model)],
        hidden: Vec<(&'static str, String)>,
        redirect_uri: &str,
        error: &'static str,
    ) -> Self {
        let workspaces: Vec<ComboOption> = choices
            .iter()
            .map(|(w, role)| {
                let mut o = ComboOption::new(w.id, &w.name).sub(&role.name);
                o.initials = palette::initials(&w.name, &w.slug);
                o.color = palette::color_for(&w.slug).to_string();
                o
            })
            .collect();
        let selected = if workspaces.len() == 1 {
            workspaces[0].value.clone()
        } else {
            String::new()
        };
        Self {
            client: client.name.clone(),
            client_initial: client
                .name
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default(),
            client_uri: client.client_uri.clone().unwrap_or_default(),
            redirect_uri: redirect_uri.to_string(),
            user_name: user.name.clone(),
            user_email: user.email.clone(),
            user_initials: palette::initials(&user.name, &user.email),
            user_color: palette::color_for(&user.email),
            locked: workspaces.is_empty(),
            workspaces,
            selected,
            error,
            scopes: SCOPES
                .iter()
                .map(|(icon, key)| Scope { icon, key })
                .collect(),
            hidden,
        }
    }
}

/// Satu client terhubung (grant aktif).
#[derive(Debug, Clone, Serialize)]
pub struct ClientRow {
    pub id: Uuid,
    pub name: String,
    pub workspace: String,
    /// Kunci client katalog (`claude`, `codex`, …) atau `""`.
    pub kind: &'static str,
    pub connected_at: String,
    pub last_used_at: String,
    pub revoke_href: String,
}

impl ClientRow {
    #[must_use]
    pub fn of(g: &oauth_grants::Model, c: &oauth_clients::Model, w: &workspaces::Model) -> Self {
        Self {
            id: g.id,
            name: c.name.clone(),
            workspace: w.name.clone(),
            kind: client_from_name(&c.name).map_or("", |k| k.key),
            connected_at: wib_datetime(&g.created_at),
            last_used_at: g
                .last_used_at
                .as_ref()
                .map(wib_datetime)
                .unwrap_or_default(),
            revoke_href: format!("/account/connect/{}/revoke", g.id),
        }
    }
}

/// Satu tool MCP di tabel.
#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: &'static str,
    pub hint_key: String,
}

#[must_use]
pub fn tools() -> Vec<ToolInfo> {
    MCP_TOOLS
        .iter()
        .map(|t| ToolInfo {
            name: t,
            hint_key: format!("mcp-tool-{}", t.replace('_', "-")),
        })
        .collect()
}

/// Cara menghubungkan satu client ke server MCP di luar repo yang di-`agentgate init` (repo yang
/// di-init sudah memuat config MCP + hook client ini, lihat `agentgate_core::runner::rules`).
#[derive(Debug, Clone, Serialize)]
pub struct ClientSetup {
    pub key: &'static str,
    pub name: &'static str,
    /// Perintah menambah server MCP, atau isi file config global bila client tidak punya
    /// perintah tambah.
    pub add: String,
    /// File config global yang diisi `add` (kosong = `add` adalah perintah).
    pub file: &'static str,
    /// Perintah login OAuth (kosong = login otomatis saat pertama dipakai).
    pub login: &'static str,
    /// Kunci Fluent petunjuk login.
    pub login_key: String,
}

/// Langkah koneksi MCP + login keenam client untuk server MCP `mcp_url`.
#[must_use]
pub fn client_setup(mcp_url: &str) -> Vec<ClientSetup> {
    let url = serde_json::to_string(mcp_url).unwrap_or_default();
    let row =
        |key: &'static str, add: String, file: &'static str, login: &'static str| ClientSetup {
            key,
            name: agentgate_core::persona::client(key).map_or(key, |c| c.name),
            add,
            file,
            login,
            login_key: format!("connect-login-{key}"),
        };
    vec![
        row(
            "claude",
            format!("claude mcp add --transport http agentgate {mcp_url}"),
            "",
            "",
        ),
        row(
            "codex",
            format!("codex mcp add agentgate --url {mcp_url}"),
            "",
            "codex mcp login agentgate",
        ),
        row(
            "gemini",
            format!("gemini mcp add --transport http agentgate {mcp_url}"),
            "",
            "",
        ),
        row(
            "cursor",
            format!("{{ \"mcpServers\": {{ \"agentgate\": {{ \"url\": {url} }} }} }}"),
            "~/.cursor/mcp.json",
            "agent mcp login agentgate",
        ),
        row(
            "opencode",
            format!(
                "{{ \"mcp\": {{ \"servers\": {{ \"agentgate\": {{ \"type\": \"remote\", \"url\": {url} }} }} }} }}"
            ),
            "~/.config/opencode/opencode.json",
            "opencode mcp auth agentgate",
        ),
        row(
            "copilot",
            format!(
                "{{ \"mcpServers\": {{ \"agentgate\": {{ \"type\": \"http\", \"url\": {url} }} }} }}"
            ),
            "~/.copilot/mcp-config.json",
            "",
        ),
    ]
}

#[must_use]
pub fn stats(clients: usize, workspaces: usize) -> Vec<Stat> {
    vec![
        Stat::new("connect-stat-clients", clients, "plug-connected").tone("success"),
        Stat::new("connect-stat-workspaces", workspaces, "building"),
        Stat::new("connect-stat-tools", MCP_TOOLS.len(), "api"),
    ]
}

/// Menu kiri halaman Hubungkan client AI.
#[must_use]
pub fn subnav(current: &str, clients: usize, slash: usize) -> Vec<NavItem> {
    let link = |key: &'static str, label: &'static str, icon: &'static str, desc: &'static str| {
        NavItem::link(key, label, icon, &format!("/account/connect?section={key}")).describe(desc)
    };
    let items = vec![
        link(
            "steps",
            "connect-section-steps",
            "terminal-2",
            "connect-section-steps-desc",
        ),
        link(
            "clients",
            "connect-section-clients",
            "plug-connected",
            "connect-section-clients-desc",
        )
        .counted(clients as u64, false),
        NavItem::soon("files", "connect-section-files", "file-code"),
        link(
            "tools",
            "connect-section-tools",
            "api",
            "connect-section-tools-desc",
        )
        .counted(MCP_TOOLS.len() as u64, false),
        link(
            "slash",
            "connect-section-slash",
            "hash",
            "connect-section-slash-desc",
        )
        .counted(slash as u64, false),
    ];
    super::nav::mark(items, current)
}

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    let mut keys: Vec<String> = tools().into_iter().map(|t| t.hint_key).collect();
    keys.extend(SCOPES.iter().map(|(_, k)| (*k).to_string()));
    keys.extend(stats(0, 0).into_iter().map(|s| s.label_key.to_string()));
    keys.extend(
        subnav("steps", 0, 0)
            .into_iter()
            .flat_map(|i| [Some(i.label_key), i.desc_key])
            .flatten()
            .map(str::to_string),
    );
    keys.extend([FLASH_REVOKED, WORKSPACE_REQUIRED].map(str::to_string));
    keys
}
