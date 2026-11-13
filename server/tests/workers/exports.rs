//! Worker ekspor data dan sapuan `export_sweep` (M11): baris macet diantrekan ulang dan
//! diselesaikan, ekspor yang terlalu sering gagal ditandai `failed` + notifikasi, impor macet
//! dilanjutkan; isi ZIP hanya project yang dipilih.

use std::io::Read;

use agentgate_server::{
    app::App,
    models::{
        data_exports::{self, ExportPart, ExportStatus, MAX_ATTEMPTS, NewExport},
        notifications,
        ticket_imports::{self, ImportStatus},
    },
    services::{exports, imports},
    tasks::export_sweep::ExportSweep,
};
use chrono::{Duration, Utc};
use loco_rs::{
    app::AppContext,
    task::{Task, Vars},
    testing::prelude::*,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
};

use crate::support::{a_project, a_user, a_workspace};

async fn reload(ctx: &AppContext, id: uuid::Uuid) -> data_exports::Model {
    data_exports::Entity::find_by_id(id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
}

/// Mundurkan `updated_at` supaya baris dianggap macet.
async fn stale(ctx: &AppContext, row: data_exports::Model, status: &str, attempts: i32) {
    let mut active = row.into_active_model();
    active.status = Set(status.into());
    active.attempts = Set(attempts);
    active.updated_at = Set((Utc::now() - Duration::hours(2)).into());
    active.update(&ctx.db).await.unwrap();
}

#[tokio::test]
async fn sweep_requeues_stuck_exports_and_fails_exhausted_ones() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let ws = a_workspace(&owner).create(ctx).await;
    let a = a_project(ctx, &ws, "AAA").await;
    a_project(ctx, &ws, "BBB").await;
    let new = || NewExport {
        requested_by: owner.id,
        recipient_id: owner.id,
        parts: vec![ExportPart::Tickets, ExportPart::Knowledge],
        project_ids: vec![a.id],
    };
    // Enqueue "gagal": baris dibuat tanpa worker, macet di queued.
    let stuck = data_exports::ActiveModel::create(&ctx.db, ws.workspace_id(), new())
        .await
        .unwrap();
    stale(ctx, stuck.clone(), "queued", 0).await;
    // Worker mati berkali-kali di tengah jalan.
    let dead = data_exports::ActiveModel::create(&ctx.db, ws.workspace_id(), new())
        .await
        .unwrap();
    stale(ctx, dead.clone(), "running", MAX_ATTEMPTS).await;
    // Baru saja diantrekan: tidak disentuh.
    let fresh = data_exports::ActiveModel::create(&ctx.db, ws.workspace_id(), new())
        .await
        .unwrap();

    ExportSweep.run(ctx, &Vars::default()).await.unwrap();

    let stuck = reload(ctx, stuck.id).await;
    assert_eq!(stuck.status(), ExportStatus::Ready, "{}", stuck.error);
    let dead = reload(ctx, dead.id).await;
    assert_eq!(dead.status(), ExportStatus::Failed);
    assert_eq!(dead.error, data_exports::FAILED_ATTEMPTS);
    assert_eq!(reload(ctx, fresh.id).await.status(), ExportStatus::Queued);
    let failed_note = notifications::Entity::find()
        .filter(notifications::Column::UserId.eq(owner.id))
        .filter(notifications::Column::Kind.eq("export_failed"))
        .one(&ctx.db)
        .await
        .unwrap();
    assert!(failed_note.is_some());

    // Isi ZIP: knowledge dan tiket project AAA saja (cakupan bukan "semua": tanpa knowledge
    // workspace).
    let bytes: Vec<u8> = ctx
        .storage
        .download(std::path::Path::new(stuck.storage_key.as_deref().unwrap()))
        .await
        .unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.name_for_index(i).unwrap().unwrap().to_string())
        .collect();
    assert!(
        names.contains(&"agentgate-export/projects/AAA/tickets.json".to_string()),
        "{names:?}"
    );
    assert!(!names.iter().any(|n| n.contains("/BBB/")), "{names:?}");
    assert!(
        !names
            .iter()
            .any(|n| n.starts_with("agentgate-export/workspace/")),
        "{names:?}"
    );
    let mut manifest = String::new();
    zip.by_name("agentgate-export/manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    assert!(manifest.contains("\"AAA\""), "{manifest}");
    assert!(!manifest.contains("\"BBB\""), "{manifest}");

    // Idempoten: sapuan kedua tidak mengubah apa pun.
    assert_eq!(exports::sweep(ctx, Utc::now()).await.unwrap(), 0);
}

#[tokio::test]
async fn sweep_resumes_stuck_imports() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let ws = a_workspace(&owner).create(ctx).await;
    let project = a_project(ctx, &ws, "IMP").await;
    // Draf lewat service (berkas tersimpan), lalu dibuat macet di `queued` tanpa worker.
    let acting_role = agentgate_server::models::roles::Model::find_builtin(
        &ctx.db,
        ws.workspace_id(),
        agentgate_server::models::roles::RoleKey::Owner,
    )
    .await
    .unwrap()
    .unwrap();
    let acting = agentgate_server::services::workspaces::Acting {
        workspace: &ws,
        user: &owner,
        role: &acting_role,
        request_id: None,
    };
    let row = imports::upload(ctx, &acting, "a.csv", b"Summary\nSatu\nDua\n")
        .await
        .unwrap();
    imports::save_mapping(
        &ctx.db,
        &acting,
        row.id,
        Some(project.id),
        vec![ticket_imports::ImportField::Title],
    )
    .await
    .unwrap();
    let mut active = ticket_imports::Entity::find_by_id(row.id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    active.status = Set("queued".into());
    active.updated_at = Set((Utc::now() - Duration::hours(1)).into());
    active.update(&ctx.db).await.unwrap();

    assert_eq!(imports::sweep(ctx, Utc::now()).await.unwrap(), 1);
    let done = ticket_imports::Entity::find_by_id(row.id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.status(), ImportStatus::Done, "{}", done.error);
    assert_eq!(done.imported, 2);
    assert_eq!(imports::sweep(ctx, Utc::now()).await.unwrap(), 0);
}
