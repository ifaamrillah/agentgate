//! Rilis: daftar + versi baru, detail (tiket, angka), release notes berversi (simpan, pulihkan,
//! terbitkan, susun dengan AI), rilis sekarang (tiket belum selesai dipindah), arsip, hapus.

use agentgate_server::{
    app::App,
    models::{jobs, release_note_versions, releases, tickets},
    services::planning::releases as service,
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};

use super::{create_ticket, fixture, ksr, ticket};

async fn release(ctx: &loco_rs::app::AppContext, wid: agentgate_server::models::workspaces::WorkspaceId, name: &str) -> releases::Model {
    releases::Entity::find()
        .filter(releases::Column::WorkspaceId.eq(wid.get()))
        .filter(releases::Column::Name.eq(name))
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("rilis ada")
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn release_lifecycle_with_versioned_notes() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let base = "/w/acme/p/KSR/releases";

        // Daftar kosong.
        let page = request.get(base).add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        let page = page.text();
        assert_css_exists(&page, "[data-releases-page]");
        assert_css_exists(&page, "#release-new form");

        // Nama kosong → 422, dialog terbuka dengan error.
        let res = request.post(base).add_cookie(f.cookie.clone()).form(&[("name", " ")]).await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "dialog#release-new[data-open-on-load]");

        // Buat v1.4 dan v1.5.
        let res = request
            .post(base)
            .add_cookie(f.cookie.clone())
            .form(&[("name", "v1.4"), ("description", "QRIS <b>struk</b>"), ("target_on", "2026-11-20")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let v14 = release(&ctx, wid, "v1.4").await;
        assert!(res.headers().get("location").unwrap().to_str().unwrap().ends_with(&v14.id.to_string()));
        request.post(base).add_cookie(f.cookie.clone()).form(&[("name", "v1.5")]).await;
        let v15 = release(&ctx, wid, "v1.5").await;
        // Nama sama → 422.
        let res = request.post(base).add_cookie(f.cookie.clone()).form(&[("name", "v1.4")]).await;
        assert_eq!(res.status_code(), 422);

        // Dua tiket di v1.4, satu selesai.
        let a = create_ticket(&request, &f.cookie, &[("title", "Bayar QRIS")]).await;
        let b = create_ticket(&request, &f.cookie, &[("title", "Struk nama kasir")]).await;
        for code in [&a, &b] {
            let t = ticket(&ctx, &f.workspace, code).await;
            let mut m = t.into_active_model();
            m.release_id = Set(Some(v14.id));
            m.update(&ctx.db).await.unwrap();
        }
        let ta = ticket(&ctx, &f.workspace, &a).await;
        let mut m = ta.into_active_model();
        m.status = Set("done".into());
        m.story_points = Set(Some(3));
        m.update(&ctx.db).await.unwrap();

        let page = request.get(base).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "tr[data-release='v1.4']");
        assert_css_exists(&page, "tr[data-release='v1.5']");
        assert!(page.contains("1 dari 2 tiket selesai"));
        assert!(!page.contains("<b>struk</b>"), "deskripsi di-escape");
        // Filter status: v1.4 sudah ada tiket dikerjakan → Belum dirilis.
        let page = request.get(&format!("{base}?status=planned")).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "tr[data-release='v1.5']");
        assert_css_not_exists(&page, "tr[data-release='v1.4']");
        let page = request.get(&format!("{base}?q=1.4")).add_cookie(f.cookie.clone()).await.text();
        assert_css_not_exists(&page, "tr[data-release='v1.5']");

        // Detail.
        let show = format!("{base}/{}", v14.id);
        let page = request.get(&show).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "[data-release-page='unreleased']");
        assert_css_exists(&page, &format!("[data-release-ticket='{a}'][data-done]"));
        assert_css_exists(&page, &format!("[data-release-ticket='{b}']"));
        assert_css_exists(&page, "#release-now form");
        assert_css_exists(&page, "[data-release-notes] [data-notes-ai]");

        // Ubah: tanggal tidak valid → 422 dialog ubah terbuka.
        let res = request
            .post(&show)
            .add_cookie(f.cookie.clone())
            .form(&[("name", "v1.4"), ("starts_on", "2026-12-01"), ("target_on", "2026-11-20")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "dialog#release-edit[data-open-on-load]");

        // Release notes: simpan v1, sama → tidak ada versi baru, simpan v2, pulihkan v1 → v3.
        let notes = format!("{show}/notes");
        let page = request.get(&notes).add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(&page.text(), "textarea[name=body]");
        let res = request.post(&notes).add_cookie(f.cookie.clone()).form(&[("body", "## v1.4\n\n- QRIS")]).await;
        assert_eq!(res.status_code(), 303);
        request.post(&notes).add_cookie(f.cookie.clone()).form(&[("body", "## v1.4\n\n- QRIS")]).await;
        request.post(&notes).add_cookie(f.cookie.clone()).form(&[("body", "## v1.4\n\n- QRIS\n- Struk")]).await;
        let r = release(&ctx, wid, "v1.4").await;
        assert_eq!(r.notes_version, 2);
        let res = request.post(&format!("{notes}/restore")).add_cookie(f.cookie.clone()).form(&[("number", "1")]).await;
        assert_eq!(res.status_code(), 303);
        let r = release(&ctx, wid, "v1.4").await;
        assert_eq!(r.notes_version, 3);
        assert_eq!(r.notes, "## v1.4\n\n- QRIS");
        let versions = release_note_versions::Model::of_release(&ctx.db, wid, r.id, 10).await.unwrap();
        assert_eq!(versions.len(), 3);
        let page = request.get(&notes).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "[data-notes-version='1'] [data-notes-restore]");
        let res = request.post(&format!("{notes}/restore")).add_cookie(f.cookie.clone()).form(&[("number", "99")]).await;
        assert_eq!(res.status_code(), 404);

        // Terlalu panjang → 422 dengan isian tetap.
        let long = "x".repeat(100_001);
        let res = request.post(&notes).add_cookie(f.cookie.clone()).form(&[("body", long.as_str())]).await;
        assert_eq!(res.status_code(), 422);

        // Susun dengan AI → job writer; kedua kali → sibuk, tidak ada job baru.
        let res = request.post(&format!("{notes}/ai")).add_cookie(f.cookie.clone()).form(&[("back", "notes")]).await;
        assert_eq!(res.status_code(), 303);
        request.post(&format!("{notes}/ai")).add_cookie(f.cookie.clone()).form(&[("back", "notes")]).await;
        let jobs = jobs::Entity::find()
            .filter(jobs::Column::WorkspaceId.eq(wid.get()))
            .filter(jobs::Column::StageKey.eq(service::NOTES_STAGE))
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].persona, "writer");
        let page = request.get(&show).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "[data-notes-running]");
        assert_css_exists(&page, "[data-notes-publish]");

        // Rilis sekarang: tiket belum selesai pindah ke v1.5, notes terbit.
        let res = request
            .post(&format!("{show}/release"))
            .add_cookie(f.cookie.clone())
            .form(&[("target", v15.id.to_string().as_str()), ("publish", "1")])
            .await;
        assert_eq!(res.status_code(), 303);
        let r = release(&ctx, wid, "v1.4").await;
        assert_eq!(r.status, "released");
        assert!(r.notes_published);
        assert_eq!(ticket(&ctx, &f.workspace, &b).await.release_id, Some(v15.id));
        assert_eq!(ticket(&ctx, &f.workspace, &a).await.release_id, Some(v14.id));
        let page = request.get(&show).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "[data-release-page='released']");
        assert_css_not_exists(&page, "[data-release-now]");
        assert_css_exists(&page, "[data-release-reopen]");
        // Rilis lagi → tidak berubah (sudah dirilis).
        request.post(&format!("{show}/release")).add_cookie(f.cookie.clone()).form(&[("target", "none")]).await;
        assert_eq!(release(&ctx, wid, "v1.4").await.status, "released");

        // Arsip lalu buka.
        request.post(&format!("{show}/status")).add_cookie(f.cookie.clone()).form(&[("status", "archived")]).await;
        assert_eq!(release(&ctx, wid, "v1.4").await.status, "archived");
        request.post(&format!("{show}/status")).add_cookie(f.cookie.clone()).form(&[("status", "unreleased")]).await;
        assert_eq!(release(&ctx, wid, "v1.4").await.status, "unreleased");

        // Hapus v1.5: tiketnya kehilangan Fix version.
        let res = request.post(&format!("{base}/{}/delete", v15.id)).add_cookie(f.cookie.clone()).form(&[("x", "")]).await;
        assert_eq!(res.status_code(), 303);
        assert!(releases::Entity::find_by_id(v15.id).one(&ctx.db).await.unwrap().is_none());
        assert_eq!(ticket(&ctx, &f.workspace, &b).await.release_id, None);
        let res = request.get(&format!("{base}/{}", v15.id)).add_cookie(f.cookie.clone()).await;
        assert_eq!(res.status_code(), 404);
        let _ = (ksr(&ctx, &f).await, tickets::Entity::find());
    })
    .await;
}
