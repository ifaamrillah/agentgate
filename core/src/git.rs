//! Git & PR (spec "Git, code review, testing & deploy"): konvensi git project, status PR/CI,
//! pola nama branch, dan tautan branch → tiket. Dipakai server (gate, webhook, `JobSpec`) dan
//! CLI (runner).

use serde::{Deserialize, Serialize};

macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $v:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($(#[$vm])* $v),+ }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$v),+];

            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$v => $s),+ }
            }

            #[must_use]
            pub fn parse(value: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|x| x.as_str() == value.trim())
            }
        }
    };
}

str_enum!(
    /// Status PR di provider.
    PrState {
        Open => "open",
        Draft => "draft",
        /// Ditutup tanpa merge (tidak dihitung gate).
        Closed => "closed",
        Merged => "merged",
    }
);

str_enum!(
    /// Ringkasan CI satu PR (head sha terbaru).
    CiState {
        /// Belum ada check yang dilaporkan.
        None => "none",
        Pending => "pending",
        Success => "success",
        Failure => "failure",
    }
);

str_enum!(
    /// Hasil kode stage Development.
    CodeOutput {
        /// AI hanya mengubah file; commit, push, dan PR dikerjakan manusia.
        Manual => "manual",
        /// Runner membuat branch, commit, push, lalu server membuka PR.
        Pr => "pr",
    }
);

str_enum!(
    /// Mode merge PR.
    MergeMode {
        /// Manusia menekan merge di provider.
        Manual => "manual",
        /// PR di-merge saat stage Code Review di-approve dan CI hijau.
        AutoOnReview => "auto_on_review",
    }
);

str_enum!(
    /// Format pesan commit dari AI.
    CommitStyle {
        Conventional => "conventional",
        Free => "free",
    }
);

str_enum!(
    /// Bahasa teks yang ditulis AI (commit, PR, komentar kode).
    Language {
        En => "en",
        Id => "id",
    }
);

/// Pola nama branch bawaan (spec: `ag/KSR-123-varian-produk`).
pub const DEFAULT_BRANCH_PATTERN: &str = "ag/{kode}-{slug}";
pub const BRANCH_PATTERN_MAX: usize = 80;
pub const CHECKS_MAX: usize = 20;
pub const CHECK_NAME_MAX: usize = 100;

pub const PATTERN_INVALID: &str = "git-branch-pattern-invalid";
pub const CHECK_INVALID: &str = "git-check-invalid";
pub const ERROR_CODES: &[&str] = &[PATTERN_INVALID, CHECK_INVALID];

/// Konvensi git per project (`projects.git` jsonb). Field baru wajib `#[serde(default)]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitSettings {
    pub branch_pattern: String,
    pub commit_style: CommitStyle,
    pub commit_language: Language,
    pub pr_language: Language,
    pub code_language: Language,
    pub code_output: CodeOutput,
    pub merge_mode: MergeMode,
    /// Gate `ci_green` juga mewajibkan setiap check di [`Self::required_checks`] lulus.
    pub require_ci: bool,
    pub required_checks: Vec<String>,
}

impl Default for GitSettings {
    fn default() -> Self {
        Self {
            branch_pattern: DEFAULT_BRANCH_PATTERN.into(),
            commit_style: CommitStyle::Conventional,
            commit_language: Language::En,
            pr_language: Language::Id,
            code_language: Language::En,
            code_output: CodeOutput::Pr,
            merge_mode: MergeMode::Manual,
            require_ci: true,
            required_checks: Vec::new(),
        }
    }
}

impl GitSettings {
    /// Rapikan lalu validasi; kembalikan kode Fluent per field.
    ///
    /// # Errors
    ///
    /// `(field, kode)` pertama yang gagal.
    pub fn normalize(&mut self) -> Result<(), (&'static str, &'static str)> {
        self.branch_pattern = self.branch_pattern.trim().to_string();
        if self.branch_pattern.is_empty() {
            self.branch_pattern = DEFAULT_BRANCH_PATTERN.into();
        }
        if !valid_pattern(&self.branch_pattern) {
            return Err(("branch_pattern", PATTERN_INVALID));
        }
        let mut checks: Vec<String> = Vec::new();
        for c in self
            .required_checks
            .iter()
            .flat_map(|c| c.split([',', '\n']))
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            if c.chars().count() > CHECK_NAME_MAX || c.chars().any(char::is_control) {
                return Err(("required_checks", CHECK_INVALID));
            }
            if !checks.iter().any(|x| x.eq_ignore_ascii_case(c)) {
                checks.push(c.to_string());
            }
        }
        if checks.len() > CHECKS_MAX {
            return Err(("required_checks", CHECK_INVALID));
        }
        self.required_checks = checks;
        Ok(())
    }
}

/// Pola branch valid: wajib memuat `{kode}` (tautan PR → tiket), hanya variabel yang dikenal,
/// dan sisa teks aman untuk nama ref git.
#[must_use]
pub fn valid_pattern(pattern: &str) -> bool {
    if pattern.chars().count() > BRANCH_PATTERN_MAX || !pattern.contains("{kode}") {
        return false;
    }
    let literal = pattern
        .replace("{kode}", "")
        .replace("{slug}", "")
        .replace("{tipe}", "");
    literal
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
        && !pattern.starts_with('/')
        && !pattern.contains("//")
        && !pattern.contains("..")
}

/// Satu segmen ref aman: ASCII huruf kecil/angka dan `-`.
fn slug(text: &str, words: usize, max: usize) -> String {
    let mut s = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(words)
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-");
    if s.len() > max {
        s.truncate(max);
        s = s.trim_end_matches('-').to_string();
    }
    s
}

/// Nama branch tiket dari pola project (`{kode}`, `{slug}` judul, `{tipe}` kunci tipe tiket).
/// Pola tidak valid → pola bawaan. Hasil tanpa `-`/`/` ganda dan tanpa `-` di ujung segmen.
#[must_use]
pub fn render_branch(pattern: &str, code: &str, title: &str, kind: &str) -> String {
    let pattern = if valid_pattern(pattern) {
        pattern
    } else {
        DEFAULT_BRANCH_PATTERN
    };
    let code: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let raw = pattern
        .replace("{kode}", &code)
        .replace("{slug}", &slug(title, 4, 40))
        .replace("{tipe}", &slug(kind, 2, 20));
    raw.split('/')
        .map(|seg| {
            let mut out = String::new();
            for c in seg.chars() {
                if (c == '-' || c == '_') && out.ends_with(['-', '_']) {
                    continue;
                }
                out.push(c);
            }
            out.trim_matches(['-', '_', '.']).to_string()
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// Kode tiket yang tertulis di nama branch atau judul PR (`ag/KSR-12-x` → `KSR-12`): token
/// pertama berbentuk `HURUF[HURUF/ANGKA]*-ANGKA` (tidak membedakan huruf besar).
#[must_use]
pub fn ticket_code_in(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let boundary = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        if boundary && bytes[i].is_ascii_alphabetic() {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'-' {
                let mut k = j + 1;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                let end_ok = k == bytes.len() || !bytes[k].is_ascii_alphanumeric();
                if k > j + 1 && end_ok {
                    return Some(text[i..k].to_ascii_uppercase());
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

/// `owner/name` repo dari URL clone (`https://github.com/o/r(.git)`, `git@github.com:o/r.git`)
/// beserta host. `None` bila bentuknya lain.
#[must_use]
pub fn repo_path(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        rest.split_once('/')?
    } else {
        url.strip_prefix("git@")?.split_once(':')?
    };
    let host = host.rsplit('@').next()?.to_ascii_lowercase();
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    let (owner, name) = (parts.next()?, parts.next()?);
    if parts.next().is_some() || host.is_empty() {
        return None;
    }
    Some((host, format!("{owner}/{name}")))
}

/// Ringkasan CI dari status check: ada yang gagal → gagal; ada yang belum selesai → berjalan;
/// semua lulus → lulus; kosong → belum ada.
#[must_use]
pub fn summarize(checks: &[CiState]) -> CiState {
    if checks.is_empty() {
        CiState::None
    } else if checks.contains(&CiState::Failure) {
        CiState::Failure
    } else if checks
        .iter()
        .any(|c| matches!(c, CiState::Pending | CiState::None))
    {
        CiState::Pending
    } else {
        CiState::Success
    }
}

/// Konvensi git untuk job (`JobSpec.git`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSpec {
    pub code_output: CodeOutput,
    pub commit_style: CommitStyle,
    pub commit_language: Language,
    pub pr_language: Language,
    pub code_language: Language,
}

impl Default for GitSpec {
    fn default() -> Self {
        GitSettings::default().spec()
    }
}

impl GitSettings {
    #[must_use]
    pub fn spec(&self) -> GitSpec {
        GitSpec {
            code_output: self.code_output,
            commit_style: self.commit_style,
            commit_language: self.commit_language,
            pr_language: self.pr_language,
            code_language: self.code_language,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_round_trip() {
        for s in PrState::ALL {
            assert_eq!(PrState::parse(s.as_str()), Some(*s));
        }
        for s in CiState::ALL {
            assert_eq!(CiState::parse(s.as_str()), Some(*s));
        }
        assert_eq!(
            MergeMode::parse("auto_on_review"),
            Some(MergeMode::AutoOnReview)
        );
    }

    #[test]
    fn branch_pattern_renders_and_sanitizes() {
        assert_eq!(
            render_branch(DEFAULT_BRANCH_PATTERN, "KSR-123", "Varian produk", "story"),
            "ag/KSR-123-varian-produk"
        );
        assert_eq!(
            render_branch("{tipe}/{kode}-{slug}", "KSR-4", "Fix: struk!", "Bug"),
            "bug/KSR-4-fix-struk"
        );
        assert_eq!(
            render_branch("ag/{kode}-{slug}", "KSR-2", "—", "x"),
            "ag/KSR-2"
        );
        // Pola tidak valid jatuh ke bawaan.
        assert_eq!(render_branch("x;rm -rf", "KSR-1", "a", ""), "ag/KSR-1-a");
        assert!(!valid_pattern("feature/{slug}"), "wajib {{kode}}");
        assert!(!valid_pattern("../{kode}"));
        assert!(!valid_pattern("ag/{kode} {slug}"));
        assert!(valid_pattern("feat/{tipe}/{kode}"));
    }

    #[test]
    fn ticket_codes_found_in_branches_and_titles() {
        assert_eq!(
            ticket_code_in("ag/KSR-12-validasi").as_deref(),
            Some("KSR-12")
        );
        assert_eq!(ticket_code_in("feat/ksr-7").as_deref(), Some("KSR-7"));
        assert_eq!(
            ticket_code_in("feat(stok): validasi (KSR-26)").as_deref(),
            Some("KSR-26")
        );
        assert_eq!(ticket_code_in("agentgate/sync-rules"), None);
        assert_eq!(ticket_code_in("v2-beta"), None);
        assert_eq!(ticket_code_in("main"), None);
    }

    #[test]
    fn repo_paths_from_clone_urls() {
        assert_eq!(
            repo_path("https://github.com/pt-contoh/kasir-api.git"),
            Some(("github.com".into(), "pt-contoh/kasir-api".into()))
        );
        assert_eq!(
            repo_path("git@github.com:pt-contoh/kasir-web.git"),
            Some(("github.com".into(), "pt-contoh/kasir-web".into()))
        );
        assert_eq!(
            repo_path("https://GHE.corp.local/infra/gudang/"),
            Some(("ghe.corp.local".into(), "infra/gudang".into()))
        );
        assert_eq!(repo_path("https://gitlab.com/a/b/c"), None);
        assert_eq!(repo_path("ftp://x/a/b"), None);
    }

    #[test]
    fn ci_summary() {
        use CiState::*;
        assert_eq!(summarize(&[]), None);
        assert_eq!(summarize(&[Success, Success]), Success);
        assert_eq!(summarize(&[Success, Pending]), Pending);
        assert_eq!(summarize(&[Pending, Failure]), Failure);
    }

    #[test]
    fn settings_normalize_checks() {
        let mut s = GitSettings {
            required_checks: vec!["build, test".into(), "Test".into(), " clippy ".into()],
            branch_pattern: "  ".into(),
            ..GitSettings::default()
        };
        s.normalize().unwrap();
        assert_eq!(s.required_checks, ["build", "test", "clippy"]);
        assert_eq!(s.branch_pattern, DEFAULT_BRANCH_PATTERN);
        s.branch_pattern = "nope".into();
        assert_eq!(s.normalize(), Err(("branch_pattern", PATTERN_INVALID)));
        let old: GitSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(old, GitSettings::default());
    }
}
