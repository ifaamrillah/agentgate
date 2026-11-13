//! Persona agent (spec "Persona agent, skill pack & model"): data bawaan yang dipakai server
//! (seed workspace, migrasi backfill) dan CLI/runner (izin aksi berbahaya, katalog model).
//!
//! - **Persona bawaan**: 9 persona yang dibuat bersama setiap workspace. Setelah dibuat, persona
//!   sepenuhnya milik workspace (bisa diubah, dihapus, di-clone); data di sini hanya titik awal.
//! - **Tool MCP** yang bisa diizinkan per persona, **tools client** (kemampuan client AI), dan
//!   **aksi berbahaya** dengan tingkat izin. Penegakan tools dan izin aksi terjadi di runner (M5).
//! - **Katalog client**: client AI yang didukung beserta model dan effort yang dikenal. Model
//!   boleh diketik bebas di luar katalog.
//!
//! Teks yang tampil di UI (nama tool, aksi, client) diterjemahkan server lewat Fluent memakai
//! kunci di sini; nama dan instruksi persona adalah data (bisa diubah user), jadi ditulis di sini.

use serde::{Deserialize, Serialize};

/// Tool MCP yang disediakan server (urutan tampil). Persona memilih subset-nya.
pub const MCP_TOOLS: &[&str] = &[
    "ag_context",
    "ag_ticket",
    "ag_draft_tickets",
    "ag_artifact",
    "ag_transition",
    "ag_ask",
    "ag_comment",
    "ag_search",
    "ag_trace",
    "ag_snippet",
    "ag_impact",
    "ag_graph",
    "ag_knowledge",
    "ag_planning",
];

/// Tool code graph (M7), dipakai semua persona bawaan yang menyentuh kode atau analisis.
const GRAPH_TOOLS: [&str; 5] = [
    "ag_search",
    "ag_trace",
    "ag_snippet",
    "ag_impact",
    "ag_graph",
];

/// Tools client AI (kemampuan bawaan client) yang bisa diizinkan per persona.
pub const CLIENT_TOOLS: &[&str] = &["read", "edit", "shell", "browser", "web", "subagent"];

/// Tingkat izin aksi berbahaya untuk job otomatis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Aksi berjalan tanpa bertanya.
    Allow,
    /// Job dijeda sampai executor mengizinkan atau menolak.
    Ask,
    /// Aksi selalu ditolak.
    Block,
}

impl Level {
    pub const ALL: [Self; 3] = [Self::Allow, Self::Ask, Self::Block];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Block => "block",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == value.trim())
    }
}

/// Aksi berbahaya yang diatur per persona, dengan tingkat bawaan.
pub const DANGER_ACTIONS: &[(&str, Level)] = &[
    ("push_branch", Level::Allow),
    ("db_migrate", Level::Ask),
    ("install_package", Level::Ask),
    ("network", Level::Ask),
    ("deploy", Level::Block),
    ("delete_outside", Level::Block),
];

/// Aksi yang selalu diblokir untuk semua persona (tidak bisa diubah).
pub const ALWAYS_BLOCKED: &[&str] = &[
    "force_push_main",
    "drop_production_db",
    "read_secrets",
    "change_global_config",
];

/// Satu client AI yang didukung: kunci, nama tampilan, model katalog, effort yang dikenal.
#[derive(Debug, Clone, Copy)]
pub struct Client {
    pub key: &'static str,
    pub name: &'static str,
    pub models: &'static [&'static str],
    pub efforts: &'static [&'static str],
    /// Perintah headless (informasi untuk halaman Client AI).
    pub headless: &'static str,
}

/// Katalog client (spec "Client yang didukung"). Diperbarui bersama rilis Agent Gate.
pub const CLIENTS: &[Client] = &[
    Client {
        key: "claude",
        name: "Claude Code",
        models: &["claude-opus-4-5", "claude-sonnet-4-5", "claude-haiku-4-5"],
        efforts: &["low", "medium", "high"],
        headless: "claude -p",
    },
    Client {
        key: "codex",
        name: "Codex CLI",
        models: &["gpt-5-codex", "gpt-5", "gpt-5-mini"],
        efforts: &["minimal", "low", "medium", "high"],
        headless: "codex exec",
    },
    Client {
        key: "gemini",
        name: "Gemini CLI",
        models: &["gemini-2.5-pro", "gemini-2.5-flash"],
        efforts: &[],
        headless: "gemini -p",
    },
    Client {
        key: "cursor",
        name: "Cursor",
        models: &["auto", "claude-sonnet-4-5", "gpt-5"],
        efforts: &[],
        headless: "cursor-agent",
    },
    Client {
        key: "opencode",
        name: "OpenCode",
        models: &["anthropic/claude-sonnet-4-5", "openai/gpt-5"],
        efforts: &[],
        headless: "opencode run",
    },
    Client {
        key: "copilot",
        name: "Copilot CLI",
        models: &["claude-sonnet-4-5", "gpt-5"],
        efforts: &[],
        headless: "copilot -p",
    },
];

/// Client katalog dengan kunci `key`.
#[must_use]
pub fn client(key: &str) -> Option<&'static Client> {
    CLIENTS.iter().find(|c| c.key == key)
}

/// Tebak client katalog dari nama client OAuth (mis. "Claude Code" → `claude`).
#[must_use]
pub fn client_from_name(name: &str) -> Option<&'static Client> {
    let lower = name.to_lowercase();
    CLIENTS.iter().find(|c| {
        lower.contains(c.key) || lower.contains(&c.name.to_lowercase().replace(" cli", ""))
    })
}

/// Model dan effort satu client di persona. Kosong = default client.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChoice {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub effort: String,
}

/// Persona bawaan.
#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub key: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub color: &'static str,
    pub icon: &'static str,
    pub instructions: &'static str,
    pub mcp_tools: &'static [&'static str],
    pub client_tools: &'static [&'static str],
    /// Override tingkat aksi berbahaya dari bawaan [`DANGER_ACTIONS`].
    pub danger: &'static [(&'static str, Level)],
}

const READ_TOOLS: &[&str] = &[
    "ag_context",
    "ag_ticket",
    "ag_artifact",
    "ag_comment",
    "ag_ask",
    GRAPH_TOOLS[0],
    GRAPH_TOOLS[1],
    GRAPH_TOOLS[2],
    GRAPH_TOOLS[3],
    GRAPH_TOOLS[4],
    "ag_knowledge",
    "ag_planning",
];
const WORK_TOOLS: &[&str] = &[
    "ag_context",
    "ag_ticket",
    "ag_artifact",
    "ag_transition",
    "ag_ask",
    "ag_comment",
    GRAPH_TOOLS[0],
    GRAPH_TOOLS[1],
    GRAPH_TOOLS[2],
    GRAPH_TOOLS[3],
    GRAPH_TOOLS[4],
    "ag_knowledge",
];

/// 9 persona bawaan (tabel spec), warna dan ikon mengikuti prototype.
pub const BUILTINS: [Builtin; 9] = [
    Builtin {
        key: "analyst",
        name: "Analyst",
        summary: "Riset dan brainstorming",
        color: "#0891b2",
        icon: "bulb",
        instructions: "# Peran\nAnda adalah Analyst. Anda membantu manusia meriset masalah dan \
            mengubah ide menjadi tiket yang jelas.\n\n## Cara kerja\n- Panggil `ag_context` \
            sebelum mulai.\n- Ajukan pertanyaan satu per satu sampai masalah, pengguna, dan batasan \
            jelas.\n- Usulkan 2–3 pendekatan beserta untung-ruginya.\n- Pecah hasilnya menjadi \
            Epic → Story → Subtask sesuai ukuran, lalu kirim lewat `ag_draft_tickets` untuk \
            direview manusia.",
        mcp_tools: &[
            "ag_context",
            "ag_ticket",
            "ag_draft_tickets",
            "ag_artifact",
            "ag_ask",
            "ag_comment",
            "ag_search",
            "ag_trace",
            "ag_snippet",
            "ag_knowledge",
            "ag_planning",
        ],
        client_tools: &["read", "web"],
        danger: &[],
    },
    Builtin {
        key: "pm",
        name: "Product Manager",
        summary: "PRD",
        color: "#7c3aed",
        icon: "file-text",
        instructions: "# Peran\nAnda adalah Product Manager. Anda menulis PRD yang ringkas dan \
            bisa diuji.\n\n## Cara kerja\n- Panggil `ag_context` dan baca artifact input.\n- Tulis \
            masalah, tujuan, ruang lingkup, dan acceptance criteria berupa checklist `- [ ]`.\n- \
            Simpan PRD lewat `ag_artifact`, lalu ajukan lewat `ag_transition`.",
        mcp_tools: WORK_TOOLS,
        client_tools: &["read"],
        danger: &[],
    },
    Builtin {
        key: "uiux",
        name: "UI/UX Designer",
        summary: "Alur, wireframe, mockup HTML",
        color: "#db2777",
        icon: "layout-sidebar",
        instructions: "# Peran\nAnda adalah UI/UX Designer. Anda merancang alur dan mockup HTML \
            yang bisa diklik, mengikuti dokumen \"Komponen UI & Design System\".\n\n## Cara kerja\n\
            - Panggil `ag_context` dan baca PRD.\n- Simpan mockup sebagai artifact lewat \
            `ag_artifact`.",
        mcp_tools: WORK_TOOLS,
        client_tools: &["read", "edit", "browser"],
        danger: &[],
    },
    Builtin {
        key: "architect",
        name: "Architect",
        summary: "Tech spec",
        color: "#4f46e5",
        icon: "hierarchy-2",
        instructions: "# Peran\nAnda adalah Architect. Anda menulis tech spec: komponen yang \
            berubah, data, API, risiko, dan rencana uji.\n\n## Cara kerja\n- Panggil `ag_context` \
            dan baca PRD.\n- Simpan tech spec lewat `ag_artifact`.",
        mcp_tools: WORK_TOOLS,
        client_tools: &["read"],
        danger: &[],
    },
    Builtin {
        key: "dev",
        name: "Developer",
        summary: "Implementasi kode",
        color: "#0f8a5f",
        icon: "code",
        instructions: "# Peran\nAnda adalah Developer. Anda mengimplementasikan tiket sesuai PRD, \
            acceptance criteria, dan tech spec.\n\n## Cara kerja\n- Panggil `ag_context` sebelum \
            mulai.\n- Kerjakan dengan TDD: tes gagal dulu, lalu implementasi.\n- Commit kecil dengan \
            format Conventional Commits.\n- Bila ragu soal aturan bisnis, panggil `ag_ask`.",
        mcp_tools: WORK_TOOLS,
        client_tools: &["read", "edit", "shell", "subagent"],
        danger: &[],
    },
    Builtin {
        key: "reviewer",
        name: "Code Reviewer",
        summary: "Review PR",
        color: "#2563eb",
        icon: "git-compare",
        instructions: "# Peran\nAnda adalah Code Reviewer. Anda memeriksa kebenaran, keamanan, dan \
            kesesuaian dengan acceptance criteria.\n\n## Cara kerja\n- Panggil `ag_context`.\n- Tulis \
            temuan sebagai komentar lewat `ag_comment`, satu temuan per poin.",
        mcp_tools: READ_TOOLS,
        client_tools: &["read", "shell"],
        danger: &[],
    },
    Builtin {
        key: "qa",
        name: "QA",
        summary: "Testing (Playwright + screenshot)",
        color: "#c2410c",
        icon: "test-pipe",
        instructions: "# Peran\nAnda adalah QA. Anda menguji tiket terhadap acceptance criteria \
            dengan Playwright dan screenshot.\n\n## Cara kerja\n- Panggil `ag_context`.\n- Tulis \
            langkah reproduksi yang jelas untuk setiap kegagalan.",
        mcp_tools: WORK_TOOLS,
        client_tools: &["read", "shell", "browser"],
        danger: &[],
    },
    Builtin {
        key: "devops",
        name: "DevOps",
        summary: "Deploy",
        color: "#475569",
        icon: "rocket",
        instructions: "# Peran\nAnda adalah DevOps. Anda menyiapkan dan menjalankan deploy sesuai \
            panduan rilis project.\n\n## Cara kerja\n- Panggil `ag_context`.\n- Catat langkah deploy \
            dan hasilnya sebagai komentar.",
        mcp_tools: WORK_TOOLS,
        client_tools: &["read", "edit", "shell"],
        danger: &[("deploy", Level::Ask)],
    },
    Builtin {
        key: "writer",
        name: "Tech Writer",
        summary: "Knowledge dan release notes",
        color: "#a16207",
        icon: "writing",
        instructions: "# Peran\nAnda adalah Tech Writer. Anda memperbarui knowledge dan menulis \
            release notes yang ringkas.\n\n## Cara kerja\n- Panggil `ag_context`, lalu \
            `ag_knowledge` action=list untuk melihat dokumen yang ada.\n- Baca kode lewat code \
            graph (`ag_search`, `ag_trace`, `ag_snippet`, `ag_graph`) sebelum menulis; tulis fakta, \
            bukan tebakan, dan sebut path file penting.\n- Kirim dokumen sebagai usulan lewat \
            `ag_knowledge` action=propose (isi Markdown lengkap, `reason`, `sources`); jangan \
            mengubah file repo.\n- Release notes dan tulisan tiket disimpan lewat `ag_artifact`.",
        mcp_tools: WORK_TOOLS,
        client_tools: &["read", "edit"],
        danger: &[],
    },
];

/// Persona bawaan dengan kunci `key`.
#[must_use]
pub fn builtin(key: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.key == key)
}

impl Builtin {
    /// Tingkat aksi berbahaya lengkap (bawaan + override persona ini).
    #[must_use]
    pub fn danger_levels(&self) -> Vec<(&'static str, Level)> {
        DANGER_ACTIONS
            .iter()
            .map(|(action, level)| {
                let level = self
                    .danger
                    .iter()
                    .find(|(a, _)| a == action)
                    .map_or(*level, |(_, l)| *l);
                (*action, level)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::{definition::valid_key, templates};

    #[test]
    fn builtins_are_valid_and_known_tools() {
        for b in BUILTINS {
            assert!(valid_key(b.key), "{}", b.key);
            assert!(
                b.mcp_tools.iter().all(|t| MCP_TOOLS.contains(t)),
                "{}",
                b.key
            );
            assert!(
                b.client_tools.iter().all(|t| CLIENT_TOOLS.contains(t)),
                "{}",
                b.key
            );
            assert!(
                b.danger
                    .iter()
                    .all(|(a, _)| DANGER_ACTIONS.iter().any(|(d, _)| d == a))
            );
        }
    }

    #[test]
    fn templates_only_use_builtin_personas() {
        for t in templates::TEMPLATES.iter().chain([&templates::SUBTASK]) {
            for s in &t.document().definition.stages {
                if let Some(p) = &s.persona {
                    assert!(builtin(p).is_some(), "{} {p}", t.key);
                }
            }
        }
    }

    #[test]
    fn client_from_oauth_name() {
        assert_eq!(
            client_from_name("Claude Code").map(|c| c.key),
            Some("claude")
        );
        assert_eq!(
            client_from_name("codex-mcp-client").map(|c| c.key),
            Some("codex")
        );
        assert_eq!(
            client_from_name("Gemini CLI MCP Client").map(|c| c.key),
            Some("gemini")
        );
        assert!(client_from_name("MCP Inspector").is_none());
    }

    #[test]
    fn devops_may_ask_to_deploy() {
        let devops = builtin("devops")
            .map(Builtin::danger_levels)
            .unwrap_or_default();
        assert!(devops.contains(&("deploy", Level::Ask)));
        let dev = builtin("dev")
            .map(Builtin::danger_levels)
            .unwrap_or_default();
        assert!(dev.contains(&("deploy", Level::Block)));
    }
}
