//! Satu-satunya aturan "siapa boleh apa di project ini" (spec "Organisasi, tim & akses"):
//!
//! - Anggota ber-role workspace bawaan **Owner atau Admin** (`roles.key IN ('owner','admin')`)
//!   adalah Admin di semua project: memegang semua [`PROJECT_PERMISSIONS`].
//! - Selain itu, izin project adalah **gabungan** izin project (`ViewProject`, `EditProject`,
//!   `AdministerProject`) dari role pada grant langsung ke user dan pada grant ke setiap tim yang
//!   ia ikuti. Izin workspace role non-admin (termasuk role custom yang memegang
//!   `AdministerProject`) tidak memberi akses project dengan sendirinya. Role grant tanpa izin
//!   project tidak memberi akses.
//! - Izin project bertingkat: `AdministerProject` mencakup `EditProject`, dan keduanya mencakup
//!   `ViewProject` ([`implied`]), sehingga setiap akses berarti boleh melihat project.
//!
//! [`effective_role`] menjawab untuk satu project (extractor `ProjectAccess`, service project);
//! `projects::Model::accessible_for` menjawab "project mana saja" (sidebar, beranda) dengan
//! query yang sama ([`grants_reaching`] + [`grants_with_project_permission`]), tanpa N+1.

use sea_orm::{
    Condition, QuerySelect, QueryTrait,
    entity::prelude::*,
    sea_query::{ExprTrait, SelectStatement, extension::postgres::PgBinOper},
};

use super::{
    permission::Permission,
    project_grants,
    roles::{self, RoleKey},
    team_members,
    workspaces::WorkspaceId,
};

/// Izin yang berlaku per project; sisanya izin workspace.
pub const PROJECT_PERMISSIONS: [Permission; 3] = [
    Permission::ViewProject,
    Permission::EditProject,
    Permission::AdministerProject,
];

/// Siapa yang bertanya: user di workspace, dengan role workspace-nya. Dibangun dari `Tenant`
/// (controller) atau `Acting` (service).
#[derive(Debug, Clone, Copy)]
pub struct Accessor<'a> {
    pub workspace_id: WorkspaceId,
    pub user_id: Uuid,
    pub role: &'a roles::Model,
}

/// Izin project efektif seorang user di satu project, dengan label untuk ditampilkan (nama
/// role, atau gabungannya bila datang dari beberapa grant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveRole {
    permissions: Vec<Permission>,
    label: String,
}

impl EffectiveRole {
    /// Memegang `permission` (sudah termasuk izin yang tercakup, [`implied`]).
    #[must_use]
    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    /// Izin project yang dipegang, urut [`PROJECT_PERMISSIONS`].
    #[must_use]
    pub fn permissions(&self) -> &[Permission] {
        &self.permissions
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Setiap izin project `role` juga dipegang di sini: syarat memberikan `role` lewat grant
    /// (pelaku tidak bisa memberi akses project melebihi aksesnya sendiri).
    #[must_use]
    pub fn covers(&self, role: &roles::Model) -> bool {
        project_permissions_of(role)
            .iter()
            .all(|p| self.permissions.contains(p))
    }
}

/// Role workspace bawaan Owner atau Admin: Admin di semua project. Role custom tidak pernah,
/// walau memegang `AdministerProject`.
#[must_use]
pub fn is_admin_everywhere(role: &roles::Model) -> bool {
    role.is_admin_everywhere()
}

/// Satu-satunya aturan "pelaku boleh memberikan atau mencabut grant ber-role `role` di
/// project ini": `role` boleh ia tugaskan di workspace (`roles::can_assign`: Owner hanya oleh
/// Owner, Admin bawaan hanya oleh Owner/Admin bawaan, role lain hanya bila semua izinnya dipegang
/// role workspace pelaku), dan izin project
/// `role` tidak melebihi akses project pelaku (`access`). Berlaku untuk role tujuan (beri, ganti)
/// dan role asal grant (ganti, cabut). Syarat `AdministerProject` dicek pemanggil lebih dulu.
#[must_use]
pub fn can_grant(actor_role: &roles::Model, access: &EffectiveRole, role: &roles::Model) -> bool {
    roles::can_assign(actor_role, role) && access.covers(role)
}

/// Izin project role `role`, ditambah yang tercakup ([`implied`]), urut
/// [`PROJECT_PERMISSIONS`].
#[must_use]
pub fn project_permissions_of(role: &roles::Model) -> Vec<Permission> {
    implied(
        PROJECT_PERMISSIONS
            .iter()
            .filter(|p| role.has(**p))
            .copied(),
    )
}

/// Tutup izin project ke atas: `AdministerProject` ⇒ `EditProject` ⇒ `ViewProject`.
fn implied(held: impl IntoIterator<Item = Permission>) -> Vec<Permission> {
    let held: Vec<Permission> = held.into_iter().collect();
    let rank = |p: Permission| PROJECT_PERMISSIONS.iter().position(|q| *q == p);
    let top = held.iter().filter_map(|p| rank(*p)).max();
    match top {
        Some(top) => PROJECT_PERMISSIONS[..=top].to_vec(),
        None => Vec::new(),
    }
}

/// Izin project efektif `who` di project `project_id`, atau `None` bila ia tidak punya akses.
/// Project harus sudah diketahui milik `who.workspace_id` (pemanggil memuatnya lewat tenant).
///
/// # Errors
///
/// Error database.
pub async fn effective_role<C: ConnectionTrait>(
    db: &C,
    who: &Accessor<'_>,
    project_id: Uuid,
) -> Result<Option<EffectiveRole>, DbErr> {
    if is_admin_everywhere(who.role) {
        return Ok(Some(EffectiveRole {
            permissions: PROJECT_PERMISSIONS.to_vec(),
            label: RoleKey::Admin.default_name().to_string(),
        }));
    }
    let granted = roles::Entity::find()
        .filter(roles::Column::WorkspaceId.eq(who.workspace_id))
        .filter(
            roles::Column::Id.in_subquery(
                grants_reaching(who)
                    .filter(project_grants::Column::ProjectId.eq(project_id))
                    .select_only()
                    .column(project_grants::Column::RoleId)
                    .into_query(),
            ),
        )
        .all(db)
        .await?;
    let permissions = implied(granted.iter().flat_map(project_permissions_of));
    if permissions.is_empty() {
        return Ok(None);
    }
    let mut names: Vec<&str> = granted
        .iter()
        .filter(|r| !project_permissions_of(r).is_empty())
        .map(|r| r.name.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    Ok(Some(EffectiveRole {
        permissions,
        label: names.join(", "),
    }))
}

/// Grant di workspace `who` yang menjangkau user-nya: grant langsung ke user, atau ke tim yang
/// ia ikuti.
pub(super) fn grants_reaching(who: &Accessor<'_>) -> Select<project_grants::Entity> {
    let teams_of_user: SelectStatement = team_members::Entity::find()
        .select_only()
        .column(team_members::Column::TeamId)
        .filter(team_members::Column::WorkspaceId.eq(who.workspace_id))
        .filter(team_members::Column::UserId.eq(who.user_id))
        .into_query();
    project_grants::Entity::find()
        .filter(project_grants::Column::WorkspaceId.eq(who.workspace_id))
        .filter(
            Condition::any()
                .add(project_grants::Column::UserId.eq(who.user_id))
                .add(project_grants::Column::TeamId.in_subquery(teams_of_user)),
        )
}

/// `project_id` dari grant yang menjangkau `who` dan role-nya memegang setidaknya satu izin
/// project: aturan [`effective_role`] untuk banyak project sekaligus.
pub(super) fn grants_with_project_permission(who: &Accessor<'_>) -> SelectStatement {
    let project_permissions: Vec<String> =
        PROJECT_PERMISSIONS.iter().map(|p| p.to_value()).collect();
    grants_reaching(who)
        .inner_join(roles::Entity)
        .filter(
            Expr::col((roles::Entity, roles::Column::Permissions))
                .binary(PgBinOper::Overlap, Expr::val(project_permissions)),
        )
        .select_only()
        .column(project_grants::Column::ProjectId)
        .into_query()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_permissions_imply_lower_ones() {
        use Permission::{AdministerProject, EditProject, ManageTeams, ViewProject};
        assert_eq!(implied([AdministerProject]), PROJECT_PERMISSIONS.to_vec());
        assert_eq!(implied([EditProject]), vec![ViewProject, EditProject]);
        assert_eq!(implied([ViewProject, ViewProject]), vec![ViewProject]);
        assert_eq!(implied([ManageTeams]), Vec::<Permission>::new());
        assert!(implied([]).is_empty());
    }
}
