//! Validasi rujukan planning di form tiket (sprint, rilis, component): harus milik project tiket;
//! sprint yang sudah selesai dan rilis yang diarsipkan hanya boleh bila memang nilai sekarang.
use sea_orm::{ConnectionTrait, DbErr};
use uuid::Uuid;

use crate::models::{
    components, releases,
    releases::ReleaseStatus,
    sprints::{self, SprintState},
    tickets,
    workspaces::WorkspaceId,
};

/// Hasil cek: `Err(())` = rujukan tidak valid.
#[derive(Debug, Clone, Copy)]
pub struct Checked {
    pub sprint: Result<Option<Uuid>, ()>,
    pub release: Result<Option<Uuid>, ()>,
    pub component: Result<Option<Uuid>, ()>,
}

/// Cek tiga rujukan (masukan `Err(())` = isian bukan UUID).
///
/// # Errors
///
/// Error database.
pub async fn check<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    sprint: Result<Option<Uuid>, ()>,
    release: Result<Option<Uuid>, ()>,
    component: Result<Option<Uuid>, ()>,
    current: Option<&tickets::Model>,
) -> Result<Checked, DbErr> {
    let sprint = match sprint {
        Ok(Some(id)) if current.and_then(|t| t.sprint_id) != Some(id) => {
            match sprints::Model::find_in(db, wid, id).await? {
                Some(s) if s.project_id == project_id && s.state() != SprintState::Completed => {
                    Ok(Some(id))
                }
                _ => Err(()),
            }
        }
        other => other,
    };
    let release = match release {
        Ok(Some(id)) if current.and_then(|t| t.release_id) != Some(id) => {
            match releases::Model::find_in(db, wid, id).await? {
                Some(r) if r.project_id == project_id && r.status() != ReleaseStatus::Archived => {
                    Ok(Some(id))
                }
                _ => Err(()),
            }
        }
        other => other,
    };
    let component = match component {
        Ok(Some(id)) => match components::Model::find_in(db, wid, id).await? {
            Some(c) if c.project_id == project_id => Ok(Some(id)),
            _ => Err(()),
        },
        other => other,
    };
    Ok(Checked {
        sprint,
        release,
        component,
    })
}

/// Perubahan rujukan planning untuk riwayat tiket (`{from, to}` berupa nama), `None` bila sama.
#[derive(Debug, Default)]
pub struct Names {
    pub sprint: Option<serde_json::Value>,
    pub release: Option<serde_json::Value>,
    pub component: Option<serde_json::Value>,
}

/// Nama sebelum/sesudah untuk setiap pasangan yang berubah.
///
/// # Errors
///
/// Error database.
pub async fn names<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    sprint: [Option<Uuid>; 2],
    release: [Option<Uuid>; 2],
    component: [Option<Uuid>; 2],
) -> Result<Names, DbErr> {
    let mut out = Names::default();
    if sprint[0] != sprint[1] {
        let mut n = [None, None];
        for (i, id) in sprint.iter().enumerate() {
            if let Some(id) = id {
                n[i] = sprints::Model::find_in(db, wid, *id).await?.map(|s| s.name);
            }
        }
        out.sprint = Some(serde_json::json!({ "from": n[0], "to": n[1] }));
    }
    if release[0] != release[1] {
        let mut n = [None, None];
        for (i, id) in release.iter().enumerate() {
            if let Some(id) = id {
                n[i] = releases::Model::find_in(db, wid, *id).await?.map(|s| s.name);
            }
        }
        out.release = Some(serde_json::json!({ "from": n[0], "to": n[1] }));
    }
    if component[0] != component[1] {
        let mut n = [None, None];
        for (i, id) in component.iter().enumerate() {
            if let Some(id) = id {
                n[i] = components::Model::find_in(db, wid, *id).await?.map(|s| s.name);
            }
        }
        out.component = Some(serde_json::json!({ "from": n[0], "to": n[1] }));
    }
    Ok(out)
}
