//! Halaman login perangkat `/device` (prototype "Login perangkat (device code)"): tiga langkah
//! dalam satu halaman solo — masukkan kode, setujui, selesai.

use serde::Serialize;

use super::{palette, time::wib_datetime, ui::ComboOption};
use crate::models::{
    oauth_device_codes::{self, format_user_code},
    roles, users, workspaces,
};

#[derive(Debug, Clone, Serialize)]
pub struct DevicePage {
    /// 0 = masukkan kode, 1 = setujui, 2 = selesai.
    pub step: u8,
    pub code: String,
    pub error: &'static str,
    pub device_name: String,
    pub requested_at: String,
    pub user_name: String,
    pub user_email: String,
    pub user_initials: String,
    pub user_color: &'static str,
    pub workspaces: Vec<ComboOption>,
    pub selected: String,
    pub locked: bool,
    /// Langkah 2: disetujui (`true`) atau ditolak.
    pub approved: bool,
}

impl DevicePage {
    fn base(step: u8) -> Self {
        Self {
            step,
            code: String::new(),
            error: "",
            device_name: String::new(),
            requested_at: String::new(),
            user_name: String::new(),
            user_email: String::new(),
            user_initials: String::new(),
            user_color: "",
            workspaces: Vec::new(),
            selected: String::new(),
            locked: false,
            approved: false,
        }
    }

    #[must_use]
    pub fn enter(code: &str, error: &'static str) -> Self {
        Self {
            code: code.trim().chars().take(12).collect(),
            error,
            ..Self::base(0)
        }
    }

    #[must_use]
    pub fn confirm(
        user: &users::Model,
        row: &oauth_device_codes::Model,
        choices: &[(workspaces::Model, roles::Model)],
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
            code: format_user_code(&row.user_code),
            error,
            device_name: row.device_name.clone(),
            requested_at: wib_datetime(&row.created_at),
            user_name: user.name.clone(),
            user_email: user.email.clone(),
            user_initials: palette::initials(&user.name, &user.email),
            user_color: palette::color_for(&user.email),
            locked: workspaces.is_empty(),
            workspaces,
            selected,
            ..Self::base(1)
        }
    }

    #[must_use]
    pub fn done(user: &users::Model, approved: bool) -> Self {
        Self {
            approved,
            user_email: user.email.clone(),
            ..Self::base(2)
        }
    }
}
