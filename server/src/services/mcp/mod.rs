//! Server MCP Agent Gate: tools dan prompts (slash command) di atas service domain. Semua akses
//! memakai principal OAuth (user + workspace); user, keanggotaan, dan role dibaca ulang di setiap
//! panggilan, jadi role yang diturunkan atau anggota yang dikeluarkan langsung berlaku. Izin
//! menulis tetap dicek service tiket (`EditProject`), membaca lewat `ViewProject`.
pub mod prompts;
pub mod tools;

use sea_orm::{ConnectionTrait, DbErr};

use crate::{
    models::{roles, users, workspace_members, workspaces},
    services::{oauth::Principal, workspaces::Acting},
};

/// Konteks satu panggilan MCP: user, workspace, dan role terbaru pemegang token.
#[derive(Debug, Clone)]
pub struct Session {
    pub user: users::Model,
    pub workspace: workspaces::Model,
    pub role: roles::Model,
    pub client: String,
}

impl Session {
    /// Muat ulang dari principal. `None` bila user dinonaktifkan, bukan lagi anggota,
    /// workspace tertutup baginya, atau workspace mewajibkan 2FA yang belum ia aktifkan.
    ///
    /// # Errors
    ///
    /// Error database.
    pub async fn load<C: ConnectionTrait>(
        db: &C,
        principal: &Principal,
    ) -> Result<Option<Self>, DbErr> {
        let Some(user) = users::Model::find_by_id(db, principal.user_id)
            .await?
            .filter(users::Model::is_active)
        else {
            return Ok(None);
        };
        let Some(workspace) = workspaces::Model::find_by_id(db, principal.workspace_id).await?
        else {
            return Ok(None);
        };
        let Some((_, role)) =
            workspace_members::Model::find_with_role(db, workspace.workspace_id(), user.id).await?
        else {
            return Ok(None);
        };
        if !workspace.is_open_to(&role) || workspace.requires_two_factor_from(&user) {
            return Ok(None);
        }
        Ok(Some(Self {
            user,
            workspace,
            role,
            client: principal.client_name.clone(),
        }))
    }

    #[must_use]
    pub fn acting(&self) -> Acting<'_> {
        Acting {
            workspace: &self.workspace,
            user: &self.user,
            role: &self.role,
            request_id: None,
        }
    }
}

/// Error tool yang ditampilkan ke model (hasil `isError`), bukan error protokol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError(pub String);

impl ToolError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl From<DbErr> for ToolError {
    fn from(e: DbErr) -> Self {
        tracing::error!(error = %e, "mcp tool db error");
        Self::new("Terjadi kesalahan server. Coba lagi.")
    }
}

impl From<crate::services::tickets::TicketFailure> for ToolError {
    fn from(e: crate::services::tickets::TicketFailure) -> Self {
        use crate::services::tickets::TicketFailure;
        match e {
            TicketFailure::NotFound => Self::new(
                "Tidak ditemukan, atau role Anda tidak mengizinkan aksi ini di project tersebut.",
            ),
            TicketFailure::Invalid(errors) => {
                let codes: Vec<String> = errors
                    .errors
                    .iter()
                    .flat_map(|(field, errs)| {
                        errs.iter().map(move |e| format!("{field}: {}", e.code))
                    })
                    .collect();
                Self::new(format!("Ditolak: {}", codes.join(", ")))
            }
            TicketFailure::Db(e) => e.into(),
        }
    }
}
