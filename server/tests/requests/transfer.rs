//! Ekspor data dan Impor CSV (M11): izin (Owner/Admin, ManageIntegrations), 404 lintas
//! workspace, worker ekspor (isi ZIP, tanpa bocor project/workspace lain), unduhan diaudit,
//! kedaluwarsa lewat cleanup, wizard impor 4 langkah tanpa JS, alias kode lama, pemetaan
//! otomatis, baris dilewati + laporan, dan penolakan berkas (bukan UTF-8, terlalu besar).

use std::io::Read;

use agentgate_server::{
    app::App,
    models::{
        data_exports::{self, ExportStatus},
        notifications, projects,
        roles::RoleKey,
        ticket_imports::{self, ImportStatus},
        tickets, workspaces,
    },
    services::cleanup::CleanupStep,
    workers::cleanup,
};
use axum::http::HeaderMap;
use axum_test::multipart::{MultipartForm, Part};
use chrono::{Duration, Utc};
use loco_rs::{TestServer, app::AppContext, auth::cookies::Cookie, testing::prelude::*};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder,
};

use super::tickets::{Fixture, create_ticket, fixture};
use crate::support::{a_project, a_user, a_workspace, add_member, newest_audit_events, sign_in_as};

const EXPORT: &str = "/w/acme/settings/export";
const IMPORT: &str = "/w/acme/settings/import-csv";

async fn project(ctx: &AppContext, ws: &workspaces::Model, key: &str) -> projects::Model {
    projects::Model::find_by_key(&ctx.db, ws.workspace_id(), key)
        .await
        .unwrap()
        .unwrap()
}

/// Anggota biasa (role Member) dengan cookie login.
async fn plain_member(
    request: &TestServer,
    ctx: &AppContext,
    f: &Fixture,
    name: &str,
    role: RoleKey,
) -> (uuid::Uuid, Cookie<'static>) {
    let user = a_user().name(name).create(ctx).await;
    add_member(ctx, &f.workspace, &user, role).await;
    let cookie = sign_in_as(request, &user).await;
    (user.id, cookie)
}

async fn exports_of(ctx: &AppContext, ws: &workspaces::Model) -> Vec<data_exports::Model> {
    data_exports::Entity::find()
        .filter(data_exports::Column::WorkspaceId.eq(ws.id))
        .order_by_desc(data_exports::Column::CreatedAt)
        .all(&ctx.db)
        .await
        .unwrap()
}

fn zip_names(bytes: &[u8]) -> Vec<String> {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    (0..archive.len())
        .map(|i| archive.name_for_index(i).unwrap().unwrap().to_string())
        .collect()
}

fn zip_text(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut out = String::new();
    file.read_to_string(&mut out).unwrap();
    out
}

fn header(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default()
}

#[tokio::test]
async fn export_is_owner_admin_only_and_scoped_to_the_workspace() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (_, member) = plain_member(&request, &ctx, &f, "Mira", RoleKey::Member).await;
        for path in ["/new", "/hist"] {
            let res = request
                .get(&format!("{EXPORT}{path}"))
                .add_cookie(member.clone())
                .await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .post(&format!("{EXPORT}/new"))
            .add_cookie(member.clone())
            .form(&[("parts", "tickets"), ("projects", "all")])
            .await;
        assert_eq!(res.status_code(), 404);
        assert!(exports_of(&ctx, &f.workspace).await.is_empty());

        // Menu Pengaturan workspace: tautan ekspor hanya untuk Owner/Admin.
        let res = request
            .get(&format!("{EXPORT}/new"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(&html, "[data-export-page='new']");
        assert_css_exists(&html, "[data-export-parts] input[value='tickets'][checked]");
        assert_css_exists(
            &html,
            "[data-export-parts] input[value='attachments']:not([checked])",
        );
        assert_css_exists(&html, "[data-nav='export']");

        // Ekspor workspace ini tidak bisa diunduh dari workspace lain.
        let res = request
            .post(&format!("{EXPORT}/new"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("parts", "tickets"),
                ("projects", "all"),
                ("recipient", &f.owner.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let id = exports_of(&ctx, &f.workspace).await[0].id;
        let other = a_user().name("Oscar").create(&ctx).await;
        a_workspace(&other).slug("lain").create(&ctx).await;
        let other_cookie = sign_in_as(&request, &other).await;
        let res = request
            .post(&format!("/w/lain/settings/export/{id}/download"))
            .add_cookie(other_cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post(&format!("{EXPORT}/{id}/download"))
            .add_cookie(other_cookie)
            .await;
        assert_eq!(res.status_code(), 404);
        // Member workspace yang sama juga tidak.
        let res = request
            .post(&format!("{EXPORT}/{id}/download"))
            .add_cookie(member)
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn export_builds_a_zip_of_chosen_parts_without_leaking_other_projects() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        a_project(&ctx, &f.workspace, "GDG").await;
        create_ticket(&request, &f.cookie, &[("title", "Bayar pakai QRIS")]).await;
        let gdg = project(&ctx, &f.workspace, "GDG").await;
        let res = request
            .post("/w/acme/p/GDG/tickets/new")
            .add_cookie(f.cookie.clone())
            .form(&[("title", "Rahasia gudang")])
            .await;
        assert_eq!(res.status_code(), 303);
        // Workspace lain dengan tiket berjudul khas.
        let other = a_user().name("Oscar").create(&ctx).await;
        let ows = a_workspace(&other).slug("lain").create(&ctx).await;
        a_project(&ctx, &ows, "LIN").await;
        let other_cookie = sign_in_as(&request, &other).await;
        let res = request
            .post("/w/lain/p/LIN/tickets/new")
            .add_cookie(other_cookie)
            .form(&[("title", "Rahasia tetangga")])
            .await;
        assert_eq!(res.status_code(), 303);

        let ksr = project(&ctx, &f.workspace, "KSR").await;
        let admin = a_user().name("Ana Admin").create(&ctx).await;
        add_member(&ctx, &f.workspace, &admin, RoleKey::Admin).await;
        let res = request
            .post(&format!("{EXPORT}/new"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("parts", "tickets"),
                ("parts", "config"),
                ("projects", &ksr.id.to_string()),
                ("recipient", &admin.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(header(res.headers(), "location"), format!("{EXPORT}/hist"));

        // Worker berjalan (ForegroundBlocking di tes): ekspor siap, penerima dinotifikasi.
        let row = &exports_of(&ctx, &f.workspace).await[0];
        assert_eq!(row.status(), ExportStatus::Ready, "{:?}", row.error);
        assert_eq!(row.progress, 100);
        assert!(row.expires_at.is_some());
        let note = notifications::Entity::find()
            .filter(notifications::Column::UserId.eq(admin.id))
            .filter(notifications::Column::Kind.eq("export_ready"))
            .one(&ctx.db)
            .await
            .unwrap();
        assert!(note.is_some());

        let res = request
            .get(&format!("{EXPORT}/hist"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(
            &html,
            &format!(
                "[data-export='{}'][data-status='ready'] [data-download]",
                row.id
            ),
        );
        assert_css_not_exists(&html, "[data-export-live][data-pending]");

        let res = request
            .post(&format!("{EXPORT}/{}/download", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(header(res.headers(), "content-type"), "application/zip");
        assert!(
            header(res.headers(), "content-disposition")
                .starts_with("attachment; filename=\"agentgate-acme-")
        );
        assert!(header(res.headers(), "cache-control").contains("no-store"));
        let bytes = res.as_bytes().to_vec();
        let names = zip_names(&bytes);
        assert!(
            names.contains(&"agentgate-export/manifest.json".to_string()),
            "{names:?}"
        );
        assert!(
            names.contains(&"agentgate-export/projects/KSR/tickets.json".to_string()),
            "{names:?}"
        );
        assert!(
            names
                .iter()
                .any(|n| n.starts_with("agentgate-export/config/workflows/")),
            "{names:?}"
        );
        assert!(
            !names
                .iter()
                .any(|n| n.contains("/GDG/") || n.contains("/LIN/")),
            "{names:?}"
        );
        assert!(
            !names
                .iter()
                .any(|n| n.starts_with("agentgate-export/audit/")),
            "{names:?}"
        );
        let tickets_json = zip_text(&bytes, "agentgate-export/projects/KSR/tickets.json");
        assert!(tickets_json.contains("Bayar pakai QRIS"));
        for name in &names {
            if name.ends_with('/') {
                continue;
            }
            let text = zip_text(&bytes, name);
            assert!(!text.contains("Rahasia gudang"), "{name}");
            assert!(!text.contains("Rahasia tetangga"), "{name}");
        }
        let _ = gdg;

        let events = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 5)
            .await
            .unwrap();
        let actions: Vec<&str> = events.iter().map(|e| e.action.as_str()).collect();
        assert!(actions.contains(&"export.requested"), "{actions:?}");
        assert!(actions.contains(&"export.downloaded"), "{actions:?}");
    })
    .await;
}

#[tokio::test]
async fn export_form_rejects_empty_parts_and_non_admin_recipient() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (mid, _) = plain_member(&request, &ctx, &f, "Mira", RoleKey::Member).await;
        let res = request
            .post(&format!("{EXPORT}/new"))
            .add_cookie(f.cookie.clone())
            .form(&[("projects", "all"), ("recipient", &f.owner.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Pilih minimal satu isi ekspor."));
        let res = request
            .post(&format!("{EXPORT}/new"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("parts", "tickets"),
                ("projects", "all"),
                ("recipient", &mid.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Penerima harus Owner atau Admin"));
        assert!(exports_of(&ctx, &f.workspace).await.is_empty());
    })
    .await;
}

#[tokio::test]
async fn failed_export_can_be_retried_and_expired_export_is_cleaned() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let res = request
            .post(&format!("{EXPORT}/new"))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("parts", "tickets"),
                ("projects", "all"),
                ("recipient", &f.owner.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303);
        let row = exports_of(&ctx, &f.workspace).await.remove(0);
        assert_eq!(row.status(), ExportStatus::Ready);

        // Ulangi hanya untuk yang gagal.
        let res = request
            .post(&format!("{EXPORT}/{}/retry", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let mut active = row.clone().into_active_model();
        active.status = Set("failed".into());
        active.error = Set("export-error-build".into());
        active.update(&ctx.db).await.unwrap();
        let res = request
            .get(&format!("{EXPORT}/hist"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(
            &res.text(),
            &format!("[data-export='{}'] [data-retry]", row.id),
        );
        let res = request
            .post(&format!("{EXPORT}/{}/retry", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let row = exports_of(&ctx, &f.workspace).await.remove(0);
        assert_eq!(row.status(), ExportStatus::Ready, "{}", row.error);
        let key = row.storage_key.clone().unwrap();
        assert!(
            ctx.storage
                .download::<Vec<u8>>(std::path::Path::new(&key))
                .await
                .is_ok()
        );

        // Lewat 7 hari: cleanup menandai expired dan menghapus berkasnya.
        let mut active = row.clone().into_active_model();
        active.expires_at = Set(Some((Utc::now() - Duration::hours(1)).into()));
        active.update(&ctx.db).await.unwrap();
        let report = cleanup::run(&ctx).await.unwrap();
        assert_eq!(report.count(CleanupStep::Exports), 1);
        let row = exports_of(&ctx, &f.workspace).await.remove(0);
        assert_eq!(row.status(), ExportStatus::Expired);
        assert!(
            ctx.storage
                .download::<Vec<u8>>(std::path::Path::new(&key))
                .await
                .is_err()
        );
        let res = request
            .post(&format!("{EXPORT}/{}/download", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let again = cleanup::run(&ctx).await.unwrap();
        assert_eq!(again.count(CleanupStep::Exports), 0);
    })
    .await;
}

// ---------------------------------------------------------------------------------------------
// Impor CSV

fn csv_file(bytes: &[u8], name: &str) -> MultipartForm {
    MultipartForm::new().add_part(
        "file",
        Part::bytes(bytes.to_vec())
            .file_name(name)
            .mime_type("text/csv"),
    )
}

const JIRA_CSV: &str = "Issue key,Summary,Issue Type,Status,Assignee,Priority,Story Points,Labels,Created,Toko\n\
SR-311,Bayar pakai QRIS,Story,,olivia@example.com,High,8,,2026-10-02,Cabang Dago\n\
SR-312,Validasi stok,Task,Analysis,orang@luar.id,Whatever,banyak,,kemarin,Cabang Dago\n\
SR-313,,Story,To Do,,,,,,\n\
SR-311,Bayar pakai QRIS,Story,,olivia@example.com,High,8,,2026-10-02,Cabang Dago\n";

async fn imports_of(ctx: &AppContext, ws: &workspaces::Model) -> Vec<ticket_imports::Model> {
    ticket_imports::Entity::find()
        .filter(ticket_imports::Column::WorkspaceId.eq(ws.id))
        .order_by_desc(ticket_imports::Column::CreatedAt)
        .all(&ctx.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn csv_import_wizard_runs_four_steps_with_aliases_and_skipped_report() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let ksr = project(&ctx, &f.workspace, "KSR").await;

        // Langkah 1: unggah.
        let res = request
            .get(&format!("{IMPORT}/new"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(
            &html,
            "[data-import-page='upload'] [data-import-upload] input[type='file'][name='file']",
        );
        assert_css_exists(&html, "[data-wizard-steps] [aria-current='step']");
        let csv = JIRA_CSV.replace("olivia@example.com", &f.owner.email);
        let res = request
            .post(&format!("{IMPORT}/new"))
            .add_cookie(f.cookie.clone())
            .multipart(csv_file(csv.as_bytes(), "tiket-kasir.csv"))
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let row = imports_of(&ctx, &f.workspace).await.remove(0);
        assert_eq!(row.status(), ImportStatus::Draft);
        assert_eq!(row.row_count, 4);
        assert_eq!(
            header(res.headers(), "location"),
            format!("{IMPORT}/{}/mapping", row.id)
        );

        // Langkah 2: pemetaan otomatis dari header Jira, "Toko" dilewati.
        let res = request
            .get(&format!("{IMPORT}/{}/mapping", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(&html, "[data-col='0'] option[value='alias'][selected]");
        assert_css_exists(&html, "[data-col='1'] option[value='title'][selected]");
        assert_css_exists(&html, "[data-col='3'] option[value='stage'][selected]");
        assert_css_exists(&html, "[data-col='9'] option[value='skip'][selected]");
        assert!(html.contains("pemisah koma"));

        // Tanpa project → 422, pilihan tersimpan.
        let mut form: Vec<(String, String)> = [
            "alias",
            "title",
            "type",
            "stage",
            "assignee",
            "priority",
            "story_points",
            "labels",
            "created_at",
            "skip",
        ]
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("col_{i}"), (*v).to_string()))
        .collect();
        let res = request
            .post(&format!("{IMPORT}/{}/mapping", row.id))
            .add_cookie(f.cookie.clone())
            .form(&form)
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Pilih project tujuan."));
        form.push(("project".into(), ksr.id.to_string()));
        let res = request
            .post(&format!("{IMPORT}/{}/mapping", row.id))
            .add_cookie(f.cookie.clone())
            .form(&form)
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            header(res.headers(), "location"),
            format!("{IMPORT}/{}/preview", row.id)
        );

        // Langkah 3: pratinjau per baris.
        let res = request
            .get(&format!("{IMPORT}/{}/preview", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(&html, "[data-row='2'][data-outcome='ready']");
        assert_css_exists(&html, "[data-row='3'][data-outcome='warning']");
        assert_css_exists(&html, "[data-row='4'][data-outcome='skipped']");
        assert_css_exists(&html, "[data-row='5'][data-outcome='skipped']");
        assert!(html.contains("Judul kosong, dilewati"), "{html}");
        assert!(html.contains("bukan anggota project"), "{html}");
        let res = request
            .get(&format!("{IMPORT}/{}/preview?res=skipped", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        let html = res.text();
        assert_css_not_exists(&html, "[data-row='2']");
        assert_css_exists(&html, "[data-row='4']");

        // Langkah 4: impor lewat worker (inline di tes).
        let res = request
            .post(&format!("{IMPORT}/{}/run", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            header(res.headers(), "location"),
            format!("{IMPORT}/{}", row.id)
        );
        let done = imports_of(&ctx, &f.workspace).await.remove(0);
        assert_eq!(done.status(), ImportStatus::Done, "{}", done.error);
        assert_eq!((done.imported, done.warned, done.skipped), (2, 1, 2));
        let res = request
            .get(&format!("{IMPORT}/{}", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert_css_exists(&html, "[data-import-result='done'] [data-board]");
        assert_css_exists(&html, "[data-import-result='done'] [data-report]");

        // Kode lama menjadi alias; tiket lewat service (stage awal, penerima, prioritas).
        let (t, current) =
            tickets::Model::find_by_code(&ctx.db, f.workspace.workspace_id(), "SR-311")
                .await
                .unwrap()
                .expect("alias SR-311");
        assert!(!current, "SR-311 alias, bukan kode sekarang");
        assert!(t.code.starts_with("KSR-"), "{}", t.code);
        assert_eq!(t.title, "Bayar pakai QRIS");
        assert_eq!(t.project_id, ksr.id);
        assert_eq!(t.assignee_id, Some(f.owner.id));
        assert_eq!(t.story_points, Some(8));
        let res = request
            .get("/w/acme/t/SR-311")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(
            matches!(res.status_code().as_u16(), 200 | 301 | 303 | 308),
            "{}",
            res.status_code()
        );
        let (t2, _) = tickets::Model::find_by_code(&ctx.db, f.workspace.workspace_id(), "SR-312")
            .await
            .unwrap()
            .expect("alias SR-312");
        assert_eq!(t2.assignee_id, None);
        assert!(
            tickets::Model::find_by_code(&ctx.db, f.workspace.workspace_id(), "SR-313")
                .await
                .unwrap()
                .is_none()
        );

        // Laporan baris dilewati.
        let res = request
            .get(&format!("{IMPORT}/{}/report.csv", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert!(header(res.headers(), "content-disposition").starts_with("attachment"));
        let report = res.text();
        assert!(report.contains("Baris,Alasan,Issue key"), "{report}");
        assert!(report.contains("Judul kosong, dilewati"), "{report}");
        assert!(report.contains("Baris duplikat dari baris 2"), "{report}");
        assert!(report.contains("SR-313"), "{report}");

        // Riwayat + audit; draf yang sudah jalan tidak bisa dijalankan ulang.
        let res = request
            .get(&format!("{IMPORT}/hist"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_css_exists(
            &res.text(),
            &format!(
                "[data-import='{}'][data-status='done'] [data-report]",
                row.id
            ),
        );
        let events = newest_audit_events(&ctx.db, f.workspace.workspace_id(), 3)
            .await
            .unwrap();
        assert!(events.iter().any(|e| e.action == "import.completed"));
        let res = request
            .get(&format!("{IMPORT}/{}/mapping", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);

        // Laporan disimpan 30 hari, lalu dibersihkan cleanup.
        let mut active = done.clone().into_active_model();
        active.report_expires_at = Set(Some((Utc::now() - Duration::hours(1)).into()));
        active.update(&ctx.db).await.unwrap();
        let report = cleanup::run(&ctx).await.unwrap();
        assert_eq!(report.count(CleanupStep::ImportReports), 1);
        let res = request
            .get(&format!("{IMPORT}/{}/report.csv", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn csv_upload_rejects_non_utf8_and_oversized_files_and_detects_semicolons() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        // Latin-1 "é" = 0xE9 tanpa byte lanjutan: bukan UTF-8.
        let latin1 = b"Summary;Status\nCaf\xe9;To Do\n";
        let res = request
            .post(&format!("{IMPORT}/new"))
            .add_cookie(f.cookie.clone())
            .multipart(csv_file(latin1, "latin1.csv"))
            .await;
        assert_eq!(res.status_code(), 422);
        let html = res.text();
        assert!(html.contains("Berkas bukan UTF-8"), "{html}");
        assert_css_exists(&html, "[data-import-page='upload'] [data-invalid]");

        let big = vec![b'a'; 20 * 1024 * 1024 + 10];
        let res = request
            .post(&format!("{IMPORT}/new"))
            .add_cookie(f.cookie.clone())
            .multipart(csv_file(&big, "besar.csv"))
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("lebih dari 20 MB"));
        assert!(imports_of(&ctx, &f.workspace).await.is_empty());

        let res = request
            .post(&format!("{IMPORT}/new"))
            .add_cookie(f.cookie.clone())
            .multipart(csv_file(
                "\u{feff}Card Name;List Name\nKartu satu;Doing\n".as_bytes(),
                "trello.csv",
            ))
            .await;
        assert_eq!(res.status_code(), 303);
        let row = imports_of(&ctx, &f.workspace).await.remove(0);
        assert_eq!(row.headers(), ["Card Name", "List Name"]);
        let res = request
            .get(&format!("{IMPORT}/{}/mapping", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        let html = res.text();
        assert!(html.contains("pemisah titik koma"));
        assert_css_exists(&html, "[data-col='0'] option[value='title'][selected]");

        // Tanpa kolom Judul → 422.
        let ksr = project(&ctx, &f.workspace, "KSR").await;
        let res = request
            .post(&format!("{IMPORT}/{}/mapping", row.id))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("project", ksr.id.to_string()),
                ("col_0", "skip".into()),
                ("col_1", "stage".into()),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Petakan satu kolom ke Judul."));

        // Batal: draf jadi cancelled, berkas sumber dihapus.
        let key = row.source_key.clone().unwrap();
        let res = request
            .post(&format!("{IMPORT}/{}/cancel", row.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        let row = imports_of(&ctx, &f.workspace).await.remove(0);
        assert_eq!(row.status(), ImportStatus::Cancelled);
        assert!(
            ctx.storage
                .download::<Vec<u8>>(std::path::Path::new(&key))
                .await
                .is_err()
        );

        // Template CSV.
        let res = request
            .get(&format!("{IMPORT}/template.csv"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert!(res.text().starts_with("Issue key,Summary"));
    })
    .await;
}

#[tokio::test]
async fn csv_import_needs_manage_integrations_and_hides_other_workspaces() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (_, member) = plain_member(&request, &ctx, &f, "Mira", RoleKey::Member).await;
        for path in ["/new", "/hist", "/template.csv"] {
            let res = request
                .get(&format!("{IMPORT}{path}"))
                .add_cookie(member.clone())
                .await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .post(&format!("{IMPORT}/new"))
            .add_cookie(member.clone())
            .multipart(csv_file(b"Summary\nSatu\n", "a.csv"))
            .await;
        assert_eq!(res.status_code(), 404);

        let res = request
            .post(&format!("{IMPORT}/new"))
            .add_cookie(f.cookie.clone())
            .multipart(csv_file(b"Summary\nSatu\n", "a.csv"))
            .await;
        assert_eq!(res.status_code(), 303);
        let id = imports_of(&ctx, &f.workspace).await[0].id;
        let other = a_user().name("Oscar").create(&ctx).await;
        a_workspace(&other).slug("lain").create(&ctx).await;
        let other_cookie = sign_in_as(&request, &other).await;
        for path in ["mapping", "preview"] {
            let res = request
                .get(&format!("/w/lain/settings/import-csv/{id}/{path}"))
                .add_cookie(other_cookie.clone())
                .await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .post(&format!("/w/lain/settings/import-csv/{id}/run"))
            .add_cookie(other_cookie)
            .await;
        assert_eq!(res.status_code(), 404);
        assert_eq!(
            imports_of(&ctx, &f.workspace).await[0].status(),
            ImportStatus::Draft
        );
    })
    .await;
}
