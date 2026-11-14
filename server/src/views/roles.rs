//! Pengaturan → Role: daftar role (bawaan dan custom) beserta izinnya, dan form role custom
//! dengan pemilih izin. Label dan deskripsi izin adalah kunci Fluent
//! (`permission-<nama>`, `permission-<nama>-desc`) dari [`permission_keys`].
use axum::http::StatusCode;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use sea_orm::Iterable;
use serde::Serialize;

use super::{
    flash::Flash,
    form::{FieldErrors, field_errors},
    layout::{AppShell, app_page},
};
use crate::models::{permission::Permission, roles};

pub const FLASH_CREATED: &str = "flash-role-created";
pub const FLASH_UPDATED: &str = "flash-role-updated";
pub const FLASH_DELETED: &str = "flash-role-deleted";
const NEW_TITLE: &str = "role-new-title";
const NEW_SUBMIT: &str = "role-create-submit";
const EDIT_TITLE: &str = "role-edit-title";
const EDIT_SUBMIT: &str = "role-save";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n, bersama
/// [`permission_keys`] untuk setiap izin).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_CREATED,
    FLASH_UPDATED,
    FLASH_DELETED,
    NEW_TITLE,
    NEW_SUBMIT,
    EDIT_TITLE,
    EDIT_SUBMIT,
];

pub const ROLE_FIELDS: &[&str] = &["name", "permissions"];

/// Kunci Fluent (label, deskripsi) satu izin.
#[must_use]
pub fn permission_keys(permission: Permission) -> (&'static str, &'static str) {
    match permission {
        Permission::ManageWorkspace => (
            "permission-manage-workspace",
            "permission-manage-workspace-desc",
        ),
        Permission::ManageMembers => (
            "permission-manage-members",
            "permission-manage-members-desc",
        ),
        Permission::ManageRoles => ("permission-manage-roles", "permission-manage-roles-desc"),
        Permission::ManageTeams => ("permission-manage-teams", "permission-manage-teams-desc"),
        Permission::CreateProjects => (
            "permission-create-projects",
            "permission-create-projects-desc",
        ),
        Permission::ViewAuditLog => (
            "permission-view-audit-log",
            "permission-view-audit-log-desc",
        ),
        Permission::AdministerProject => (
            "permission-administer-project",
            "permission-administer-project-desc",
        ),
        Permission::EditProject => ("permission-edit-project", "permission-edit-project-desc"),
        Permission::ViewProject => ("permission-view-project", "permission-view-project-desc"),
        Permission::ManageTicketSettings => (
            "permission-manage-ticket-settings",
            "permission-manage-ticket-settings-desc",
        ),
        Permission::ManageIntegrations => (
            "permission-manage-integrations",
            "permission-manage-integrations-desc",
        ),
        Permission::ManageKnowledge => (
            "permission-manage-knowledge",
            "permission-manage-knowledge-desc",
        ),
    }
}

/// Satu baris daftar role. Role bawaan tanpa `edit_href`/`delete_action`.
#[derive(Debug, Serialize)]
pub struct RoleRow {
    pub name: String,
    pub builtin: bool,
    /// Kunci Fluent label izin, urut [`Permission`].
    pub permission_keys: Vec<&'static str>,
    pub edit_href: Option<String>,
    pub delete_action: Option<String>,
}

impl RoleRow {
    /// `base` = path daftar role (`/w/{slug}/settings/roles`).
    #[must_use]
    pub fn new(role: &roles::Model, base: &str) -> Self {
        let editable = roles::ensure_role_editable(role).is_ok();
        Self {
            name: role.name.clone(),
            builtin: role.builtin,
            permission_keys: Permission::iter()
                .filter(|p| role.has(*p))
                .map(|p| permission_keys(p).0)
                .collect(),
            edit_href: editable.then(|| format!("{base}/{}/edit", role.id)),
            delete_action: editable.then(|| format!("{base}/{}/delete", role.id)),
        }
    }
}

/// Satu checkbox izin. `grantable = false`: pelaku tidak memegang izin ini, jadi checkbox
/// nonaktif dan nilainya (bila terpilih) dikirim lewat input hidden supaya tetap utuh.
#[derive(Debug, Serialize)]
pub struct PermissionOption {
    pub value: String,
    pub label_key: &'static str,
    pub description_key: &'static str,
    pub checked: bool,
    pub grantable: bool,
}

/// Isian form role: nama, semua izin dengan tanda terpilih, error per field.
#[derive(Debug, Serialize)]
pub struct RoleFormView {
    pub name: String,
    pub permissions: Vec<PermissionOption>,
    pub errors: FieldErrors,
}

impl RoleFormView {
    /// `selected` = nilai izin terpilih (string dari form atau dari role); `actor` = role
    /// pelaku (izin yang tidak ia pegang tampil nonaktif, aturan
    /// `roles::ensure_permissions_grantable`).
    #[must_use]
    pub fn new(
        name: &str,
        selected: &[String],
        actor: &roles::Model,
        errors: Option<&ModelValidationErrors>,
    ) -> Self {
        Self {
            name: name.to_string(),
            permissions: Permission::iter()
                .map(|p| {
                    let (label_key, description_key) = permission_keys(p);
                    let value = p.to_string();
                    PermissionOption {
                        checked: selected.contains(&value),
                        grantable: actor.has(p),
                        value,
                        label_key,
                        description_key,
                    }
                })
                .collect(),
            errors: field_errors(ROLE_FIELDS, errors),
        }
    }
}

/// Daftar role. `flash` = pesan sekali tampil, atau penolakan (`role-builtin`, `role-in-use`)
/// dengan status 422.
pub fn index(
    v: &impl ViewRenderer,
    shell: &AppShell,
    roles: &[RoleRow],
    new_href: &str,
    flash: Option<Flash>,
    status: StatusCode,
) -> Result<Response> {
    format::render().status(status).view(
        v,
        "workspaces/roles/index.html",
        app_page(
            shell,
            data!({ "roles": roles, "new_href": new_href, "flash": flash }),
        ),
    )
}

/// Form buat (`editing = false`) atau ubah role.
pub fn form(
    v: &impl ViewRenderer,
    shell: &AppShell,
    form: &RoleFormView,
    action: &str,
    back_href: &str,
    editing: bool,
    status: StatusCode,
) -> Result<Response> {
    let (title_key, submit_key) = if editing {
        (EDIT_TITLE, EDIT_SUBMIT)
    } else {
        (NEW_TITLE, NEW_SUBMIT)
    };
    format::render().status(status).view(
        v,
        "workspaces/roles/form.html",
        app_page(
            shell,
            data!({
                "form": form,
                "action": action,
                "back_href": back_href,
                "title_key": title_key,
                "submit_key": submit_key,
            }),
        ),
    )
}
