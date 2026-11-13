//! Kelola koneksi Jira workspace (M11, `ManageIntegrations`, dicek ulang di dalam transaksi lewat
//! `lock_as`): simpan/uji kredensial dan project, mode, pemetaan status/field/user, jeda,
//! sinkron ulang semua, putus. Panggilan Jira (uji, katalog) dilakukan **sebelum** transaksi
//! dibuka; transaksi hanya menulis hasilnya. Setiap perubahan dicatat di audit log (tanpa
//! token).
//!
//! Mengganti pasangan project Jira ↔ Agent Gate menghapus tautan tiket/komentar lama dan
//! mengulang cursor: tautan ke project lama tidak bermakna lagi. Tiket yang sudah diimpor tetap.

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, ModelTrait, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use super::{
    api::{JiraApi, JiraProject},
    mapping,
};
use crate::{
    common::{crypto, settings::Settings},
    models::{
        custom_fields, jira_comment_links,
        jira_connections::{
            self, Catalog, CatalogProject, FieldMap, FieldTarget, JiraEnv, StatusMap, SyncMode,
            UserMap,
        },
        jira_links,
        permission::Permission,
        projects, workspace_members,
        workspaces::WorkspaceId,
    },
    services::{
        audit::{self, AuditAction, AuditSubject},
        tickets::TicketFailure,
        workspaces::{Acting, lock_as},
    },
};

/// Isian panel Koneksi.
#[derive(Debug, Clone, Default)]
pub struct ConnInput {
    pub env: String,
    pub base_url: String,
    pub email: String,
    /// Kosong = token tersimpan dipakai (koneksi sudah ada).
    pub token: String,
    /// Kunci project Jira; kosong = belum dipilih.
    pub jira_project: String,
    /// Kunci project Agent Gate; kosong = belum dipilih.
    pub project: String,
}

/// Hasil uji koneksi.
#[derive(Debug, Clone)]
pub struct Tested {
    pub account: String,
    pub projects: Vec<JiraProject>,
}

fn field(field: &'static str, code: &'static str) -> TicketFailure {
    TicketFailure::field(field, code)
}

async fn require<C: sea_orm::ConnectionTrait>(
    txn: &C,
    acting: &Acting<'_>,
) -> Result<(), TicketFailure> {
    lock_as(txn, acting, |r| r.has(Permission::ManageIntegrations))
        .await?
        .ok_or(TicketFailure::NotFound)?;
    Ok(())
}

fn require_role(acting: &Acting<'_>) -> Result<(), TicketFailure> {
    if acting.role.has(Permission::ManageIntegrations) {
        Ok(())
    } else {
        Err(TicketFailure::NotFound)
    }
}

/// Isian kredensial tervalidasi: (env, url, email, token).
async fn credentials(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    input: &ConnInput,
) -> Result<(JiraEnv, String, String, String), TicketFailure> {
    let env = JiraEnv::parse(&input.env).ok_or_else(|| field("env", super::ENV_INVALID))?;
    let url =
        super::webhooks_url(&input.base_url, settings).map_err(|code| field("base_url", code))?;
    let email = input.email.trim().to_lowercase();
    if env == JiraEnv::Cloud && (email.is_empty() || !email.contains('@')) {
        return Err(field("email", super::EMAIL_REQUIRED));
    }
    let token = match input.token.trim() {
        "" => {
            let existing = jira_connections::Model::of_workspace(db, acting.workspace_id())
                .await?
                .ok_or_else(|| field("token", super::TOKEN_REQUIRED))?;
            existing
                .token(&super::key(settings).map_err(|c| field("token", c))?)
                .map_err(|_| field("token", super::TOKEN_REQUIRED))?
        }
        t => t.to_string(),
    };
    Ok((env, url, email, token))
}

/// Uji kredensial isian (tanpa menyimpan): nama akun dan project yang terlihat.
///
/// # Errors
///
/// Validasi isian, kode Jira (`jira-token-invalid`, `jira-unreachable`, …) di field `token`.
pub async fn test(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    input: &ConnInput,
) -> Result<Tested, TicketFailure> {
    require_role(acting)?;
    let (env, url, email, token) = credentials(db, acting, settings, input).await?;
    let api = super::client(settings, &url, env, super::auth(env, &email, &token))
        .await
        .map_err(|c| field("base_url", c))?;
    probe(&api).await
}

async fn probe(api: &dyn JiraApi) -> Result<Tested, TicketFailure> {
    let me = api
        .myself()
        .await
        .map_err(|e| field("token", super::error_code(&e)))?;
    let projects = api
        .projects()
        .await
        .map_err(|e| field("token", super::error_code(&e)))?;
    Ok(Tested {
        account: if me.name.is_empty() {
            me.account
        } else {
            me.name
        },
        projects,
    })
}

/// Katalog pemetaan dari Jira untuk project `key`.
struct Mapped {
    statuses: StatusMap,
    fields: FieldMap,
    users: UserMap,
}

async fn refresh_maps(
    db: &DatabaseConnection,
    wid: WorkspaceId,
    api: &dyn JiraApi,
    project: &projects::Model,
    jira_key: &str,
    current: Option<&jira_connections::Model>,
) -> Result<Mapped, TicketFailure> {
    let code = |e: super::api::JiraError| field("jira_project", super::error_code(&e));
    let statuses = api.statuses(jira_key).await.map_err(code)?;
    let fields = api.fields().await.map_err(code)?;
    // User: best effort (Data Center lama menolak pencarian tanpa nama).
    let users = api.users(jira_key).await.unwrap_or_default();
    let stages = mapping::project_stages(db, wid, project).await?;
    let custom = custom_fields::Model::all_in(db, wid).await?;
    let members: Vec<(Uuid, String)> = workspace_members::Model::list_in(db, wid)
        .await?
        .into_iter()
        .map(|(_, u, _)| (u.id, u.email.to_lowercase()))
        .collect();
    let (cs, cf, cu) = current.map_or_else(
        || {
            (
                StatusMap::default(),
                FieldMap::default(),
                UserMap::default(),
            )
        },
        |c| (c.status_map(), c.field_map(), c.user_map()),
    );
    Ok(Mapped {
        statuses: mapping::statuses(&statuses, &stages, &cs),
        fields: mapping::fields(&fields, &custom, &cf),
        users: mapping::users(&users, &members, &cu),
    })
}

/// Simpan koneksi (buat atau ubah): kredensial diverifikasi ke Jira, katalog project disimpan;
/// bila kedua project dipilih, peta status/field/user diperbarui (pilihan lama dipertahankan).
///
/// # Errors
///
/// Validasi isian, kode Jira, `NotFound` (tanpa izin), error database.
pub async fn save(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    settings: &Settings,
    input: &ConnInput,
) -> Result<jira_connections::Model, TicketFailure> {
    require_role(acting)?;
    let wid = acting.workspace_id();
    let (env, url, email, token) = credentials(db, acting, settings, input).await?;
    let api = super::client(settings, &url, env, super::auth(env, &email, &token))
        .await
        .map_err(|c| field("base_url", c))?;
    let tested = probe(&api).await?;
    let jira_key = input.jira_project.trim().to_uppercase();
    let jira_project = if jira_key.is_empty() {
        None
    } else {
        Some(
            tested
                .projects
                .iter()
                .find(|p| p.key.eq_ignore_ascii_case(&jira_key))
                .cloned()
                .ok_or_else(|| field("jira_project", super::JIRA_PROJECT_INVALID))?,
        )
    };
    let project = match input.project.trim() {
        "" => None,
        key => Some(
            projects::Model::find_by_key(db, wid, key)
                .await?
                .filter(|p| p.archived_at.is_none())
                .ok_or_else(|| field("project", super::PROJECT_INVALID))?,
        ),
    };
    let existing = jira_connections::Model::of_workspace(db, wid).await?;
    let mapped = match (&project, &jira_project) {
        (Some(p), Some(j)) => {
            Some(refresh_maps(db, wid, &api, p, &j.key, existing.as_ref()).await?)
        }
        _ => None,
    };
    let encrypted = crypto::encrypt(
        &super::key(settings).map_err(|c| field("token", c))?,
        token.as_bytes(),
    )
    .map_err(|_| field("token", super::TOKEN_INVALID))?;
    let catalog = Catalog {
        projects: tested
            .projects
            .iter()
            .map(|p| CatalogProject {
                id: p.id.clone(),
                key: p.key.clone(),
                name: p.name.clone(),
            })
            .collect(),
    };

    let txn = db.begin().await?;
    require(&txn, acting).await?;
    let existing = jira_connections::Model::of_workspace(&txn, wid).await?;
    let created = existing.is_none();
    let project_id = project.as_ref().map(|p| p.id);
    let jira_key = jira_project
        .as_ref()
        .map(|p| p.key.clone())
        .unwrap_or_default();
    let pair_changed = existing
        .as_ref()
        .is_some_and(|e| e.project_id != project_id || e.jira_project_key != jira_key);
    let mut row = match existing.clone() {
        Some(e) => e.into_active_model(),
        None => jira_connections::ActiveModel {
            workspace_id: Set(wid.get()),
            owner_id: Set(Some(acting.user.id)),
            ..Default::default()
        },
    };
    row.env = Set(env.as_str());
    row.base_url = Set(url.clone());
    row.email = Set(if env == JiraEnv::Cloud {
        email
    } else {
        String::new()
    });
    row.token_encrypted = Set(encrypted);
    row.account_name = Set(tested.account.clone());
    row.catalog = Set(jira_connections::ActiveModel::json(&catalog));
    row.jira_project_key = Set(jira_key.clone());
    row.jira_project_name = Set(jira_project
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_default());
    row.project_id = Set(project_id);
    row.last_error = Set(String::new());
    if let Some(m) = &mapped {
        row.set_status_map(&m.statuses);
        row.set_field_map(&m.fields);
        row.set_user_map(&m.users);
    }
    if pair_changed {
        row.outbox_after_at = Set(None);
        row.outbox_after_id = Set(None);
        row.pulled_at = Set(None);
        row.imported_at = Set(None);
        row.stats = Set(serde_json::json!({}));
    }
    let conn = if created {
        row.insert(&txn).await?
    } else {
        row.update(&txn).await?
    };
    if pair_changed {
        jira_links::Entity::delete_many()
            .filter(jira_links::Column::ConnectionId.eq(conn.id))
            .exec(&txn)
            .await?;
        jira_comment_links::Entity::delete_many()
            .filter(jira_comment_links::Column::ConnectionId.eq(conn.id))
            .exec(&txn)
            .await?;
    }
    audit::record(
        &txn,
        acting.actor().audit(
            if created {
                AuditAction::JiraConnected
            } else {
                AuditAction::JiraUpdated
            },
            (AuditSubject::JiraConnection, conn.id),
            serde_json::json!({
                "env": conn.env, "host": conn.host(), "jira_project": conn.jira_project_key,
                "project_id": conn.project_id, "account": conn.account_name,
            }),
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(conn)
}

/// Ubah satu koneksi workspace di transaksi dengan cek izin + audit `jira.updated`.
async fn change(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    detail: serde_json::Value,
    apply: impl FnOnce(
        &jira_connections::Model,
        &mut jira_connections::ActiveModel,
    ) -> Result<(), TicketFailure>,
) -> Result<jira_connections::Model, TicketFailure> {
    require_role(acting)?;
    let txn = db.begin().await?;
    require(&txn, acting).await?;
    let conn = jira_connections::Model::of_workspace(&txn, acting.workspace_id())
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let mut row = conn.clone().into_active_model();
    apply(&conn, &mut row)?;
    let conn = row.update(&txn).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::JiraUpdated,
            (AuditSubject::JiraConnection, conn.id),
            detail,
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(conn)
}

/// Simpan mode sinkron.
///
/// # Errors
///
/// `jira-mode-invalid`, `NotFound`, error database.
pub async fn save_mode(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    mode: &str,
) -> Result<jira_connections::Model, TicketFailure> {
    let mode = SyncMode::parse(mode).ok_or_else(|| field("mode", super::MODE_INVALID))?;
    change(db, acting, serde_json::json!({ "mode": mode }), |_, row| {
        row.mode = Set(mode.as_str());
        Ok(())
    })
    .await
}

/// Simpan peta status: `(id status Jira, kunci stage atau "")`.
///
/// # Errors
///
/// `jira-stage-invalid` (stage bukan milik project), `NotFound`, error database.
pub async fn save_statuses(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    choices: &[(String, String)],
) -> Result<jira_connections::Model, TicketFailure> {
    let wid = acting.workspace_id();
    let conn = jira_connections::Model::of_workspace(db, wid)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let project = match conn.project_id {
        Some(id) => projects::Model::find_in(db, wid, id).await?,
        None => None,
    }
    .ok_or_else(|| field("status", super::NOT_CONNECTED))?;
    let stages = mapping::project_stages(db, wid, &project).await?;
    let mut map = conn.status_map();
    for entry in &mut map.statuses {
        if let Some((_, stage)) = choices.iter().find(|(id, _)| *id == entry.id) {
            entry.stage = match stage.trim() {
                "" => None,
                key if stages.iter().any(|s| s.key == key) => Some(key.to_string()),
                _ => return Err(field("status", super::STAGE_INVALID)),
            };
        }
    }
    change(
        db,
        acting,
        serde_json::json!({ "status_map": map.statuses.iter().map(|s| (s.name.clone(), s.stage.clone())).collect::<Vec<_>>() }),
        |_, row| {
            row.set_status_map(&map);
            Ok(())
        },
    )
    .await
}

/// Simpan peta field (`(id field Jira, target)`) dan user (`(akun Jira, id user atau "")`).
///
/// # Errors
///
/// `jira-field-invalid`, `jira-user-invalid` (bukan anggota), `NotFound`, error database.
pub async fn save_fields(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    fields: &[(String, String)],
    users: &[(String, String)],
) -> Result<jira_connections::Model, TicketFailure> {
    let wid = acting.workspace_id();
    let conn = jira_connections::Model::of_workspace(db, wid)
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let custom = custom_fields::Model::all_in(db, wid).await?;
    let mut fm = conn.field_map();
    for entry in &mut fm.fields {
        if let Some((_, raw)) = fields.iter().find(|(id, _)| *id == entry.id) {
            let target =
                FieldTarget::parse(raw).ok_or_else(|| field("fields", super::FIELD_INVALID))?;
            if let FieldTarget::Custom(id) = target
                && !custom.iter().any(|c| c.id == id)
            {
                return Err(field("fields", super::FIELD_INVALID));
            }
            entry.target = target;
        }
    }
    let members: Vec<Uuid> = workspace_members::Model::list_in(db, wid)
        .await?
        .into_iter()
        .map(|(_, u, _)| u.id)
        .collect();
    let mut um = conn.user_map();
    for entry in &mut um.users {
        if let Some((_, raw)) = users.iter().find(|(acc, _)| *acc == entry.account) {
            entry.user_id = match raw.trim() {
                "" => None,
                id => Some(
                    Uuid::parse_str(id)
                        .ok()
                        .filter(|id| members.contains(id))
                        .ok_or_else(|| field("users", super::USER_INVALID))?,
                ),
            };
        }
    }
    change(
        db,
        acting,
        serde_json::json!({
            "field_map": fm.fields.iter().map(|f| (f.id.clone(), f.target)).collect::<Vec<_>>(),
            "users_mapped": um.mapped(),
        }),
        |_, row| {
            row.set_field_map(&fm);
            row.set_user_map(&um);
            Ok(())
        },
    )
    .await
}

/// Jeda / lanjutkan sinkron.
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn set_paused(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
    paused: bool,
) -> Result<jira_connections::Model, TicketFailure> {
    change(
        db,
        acting,
        serde_json::json!({ "paused": paused }),
        |_, row| {
            row.paused_at = Set(paused.then(|| chrono::Utc::now().into()));
            Ok(())
        },
    )
    .await
}

/// Minta sinkron ulang semua (rekonsiliasi penuh pada sinkron berikutnya).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn request_full(
    db: &DatabaseConnection,
    acting: &Acting<'_>,
) -> Result<jira_connections::Model, TicketFailure> {
    change(
        db,
        acting,
        serde_json::json!({ "full_sync": true }),
        |c, row| {
            if !c.is_ready() {
                return Err(field("sync", super::NOT_CONNECTED));
            }
            row.full_sync_requested_at = Set(Some(chrono::Utc::now().into()));
            Ok(())
        },
    )
    .await
}

/// Putuskan koneksi: baris koneksi, tautan, dan log dihapus; tiket/komentar/lampiran yang sudah
/// diimpor tetap ada (alias kode Jira tetap bisa dicari).
///
/// # Errors
///
/// `NotFound`, error database.
pub async fn disconnect(db: &DatabaseConnection, acting: &Acting<'_>) -> Result<(), TicketFailure> {
    require_role(acting)?;
    let txn = db.begin().await?;
    require(&txn, acting).await?;
    let conn = jira_connections::Model::of_workspace(&txn, acting.workspace_id())
        .await?
        .ok_or(TicketFailure::NotFound)?;
    let detail = serde_json::json!({ "host": conn.host(), "jira_project": conn.jira_project_key });
    let id = conn.id;
    conn.delete(&txn).await?;
    audit::record(
        &txn,
        acting.actor().audit(
            AuditAction::JiraDisconnected,
            (AuditSubject::JiraConnection, id),
            detail,
        ),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}
