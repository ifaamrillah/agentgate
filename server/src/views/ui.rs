//! Struct data untuk komponen UI umum (`stats`, `combo`), supaya halaman mengirim bentuk yang sama.
use serde::Serialize;

use super::palette;

/// Satu kartu angka (komponen `stats`). `label_key` dan `hint_key` adalah kunci Fluent;
/// `hint_arg` diteruskan sebagai argumen `$value` ke petunjuk.
#[derive(Debug, Clone, Serialize)]
pub struct Stat {
    pub label_key: &'static str,
    pub value: String,
    /// Penyebut kecil di samping angka ("/12"); `""` = tanpa.
    pub total: String,
    pub icon: &'static str,
    /// `""` | `accent` | `success` | `warn` | `danger`.
    pub tone: &'static str,
    pub hint_key: Option<&'static str>,
    pub hint_arg: String,
    pub href: Option<String>,
}

impl Stat {
    #[must_use]
    pub fn new(label_key: &'static str, value: impl ToString, icon: &'static str) -> Self {
        Self {
            label_key,
            value: value.to_string(),
            total: String::new(),
            icon,
            tone: "",
            hint_key: None,
            hint_arg: String::new(),
            href: None,
        }
    }

    #[must_use]
    pub fn of(mut self, total: impl ToString) -> Self {
        self.total = total.to_string();
        self
    }

    #[must_use]
    pub fn tone(mut self, tone: &'static str) -> Self {
        self.tone = tone;
        self
    }

    #[must_use]
    pub fn hint(mut self, hint_key: &'static str, arg: impl ToString) -> Self {
        self.hint_key = Some(hint_key);
        self.hint_arg = arg.to_string();
        self
    }

    #[must_use]
    pub fn link(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }
}

/// Satu opsi combobox (komponen `combo` dan respons endpoint pencarian opsi).
#[derive(Debug, Clone, Serialize)]
pub struct ComboOption {
    pub value: String,
    pub label: String,
    /// Baris kedua (email, jabatan); `""` = tanpa.
    pub sub: String,
    /// Avatar inisial berwarna (orang); `""` = tanpa avatar.
    pub initials: String,
    /// Warna avatar/titik; `""` = tanpa.
    pub color: String,
    /// Judul kelompok (opsi berurutan dengan kelompok sama digabung); `""` = tanpa.
    pub group: String,
    /// Judul kelompok sebagai kunci Fluent (diterjemahkan template); mengalahkan `group`.
    pub group_key: Option<&'static str>,
}

impl ComboOption {
    #[must_use]
    pub fn new(value: impl ToString, label: impl ToString) -> Self {
        Self {
            value: value.to_string(),
            label: label.to_string(),
            sub: String::new(),
            initials: String::new(),
            color: String::new(),
            group: String::new(),
            group_key: None,
        }
    }

    /// Opsi orang: avatar inisial berwarna (stabil per email) + email sebagai baris kedua.
    #[must_use]
    pub fn person(value: impl ToString, name: &str, email: &str) -> Self {
        Self {
            sub: email.to_string(),
            initials: palette::initials(name, email),
            color: palette::color_for(email).to_string(),
            ..Self::new(value, if name.is_empty() { email } else { name })
        }
    }

    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "builder field `sub` (teks kecil), bukan operator"
    )]
    pub fn sub(mut self, sub: impl ToString) -> Self {
        self.sub = sub.to_string();
        self
    }

    #[must_use]
    pub fn group(mut self, group: impl ToString) -> Self {
        self.group = group.to_string();
        self
    }

    /// Kelompok berlabel kunci Fluent (mis. `grant-subject-teams`).
    #[must_use]
    pub const fn group_key(mut self, key: &'static str) -> Self {
        self.group_key = Some(key);
        self
    }

    #[must_use]
    pub fn color(mut self, color: impl ToString) -> Self {
        self.color = color.to_string();
        self
    }
}
