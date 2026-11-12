//! Setiap halaman tiket ter-render (200) dengan data lengkap: komentar, relasi, lampiran,
//! subtask, field custom, riwayat perubahan, notifikasi, sampah, dan pengaturan.

use agentgate_server::{app::App, models::custom_fields};
use axum_test::multipart::{MultipartForm, Part};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ticket};
use crate::support::{a_user, add_member, sign_in_as};
use agentgate_server::models::roles::RoleKey;

#[tokio::test]
async fn every_ticket_page_renders_with_rich_data() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let post = |path: String, form: Vec<(&'static str, String)>| {
            let request = &request;
            let cookie = f.cookie.clone();
            async move {
                let res = request.post(&path).add_cookie(cookie).form(&form).await;
                assert!(
                    res.status_code() == 303,
                    "{path}: {} {}",
                    res.status_code(),
                    res.text()
                );
            }
        };

        // Field custom dipasang ke tipe Task.
        post(
            "/w/acme/settings/fields".into(),
            vec![
                ("name", "Toko pilot".into()),
                ("kind", "select".into()),
                ("options", "Toko Pusat\nToko Cabang".into()),
            ],
        )
        .await;
        let field = custom_fields::Model::all_in(&ctx.db, wid).await.unwrap()[0].clone();

        let epic = create_ticket(&request, &f.cookie, &[("title", "Rilis kasir")]).await;
        let code = create_ticket(
            &request,
            &f.cookie,
            &[
                ("title", "Cetak struk"),
                ("labels", "kasir, printer"),
                ("story_points", "3"),
                ("due_on", "2026-01-02"),
                ("priority", "high"),
                ("assignee", &f.owner.id.to_string()),
                ("parent", &epic),
            ],
        )
        .await;
        let t = ticket(&ctx, &f.workspace, &code).await;
        post(
            format!("/w/acme/t/{code}"),
            vec![
                ("title", "Cetak struk thermal".into()),
                ("description", "Langkah:\n1. Buka kasir".into()),
                ("status", "in_progress".into()),
                (
                    Box::leak(format!("custom_{}", field.id).into_boxed_str()),
                    "Toko Pusat".into(),
                ),
            ],
        )
        .await;
        let blocker = create_ticket(&request, &f.cookie, &[("title", "Driver printer")]).await;
        post(
            format!("/w/acme/t/{code}/links"),
            vec![("kind", "blocked_by".into()), ("target", blocker.clone())],
        )
        .await;
        post(
            format!("/w/acme/t/{code}/comments"),
            vec![("body", "Cek dulu @olivia".into())],
        )
        .await;
        let res = request
            .post(&format!("/w/acme/t/{code}/attachments"))
            .add_cookie(f.cookie.clone())
            .multipart(MultipartForm::new().add_part(
                "file",
                Part::bytes(b"isi".to_vec())
                    .file_name("catatan.txt")
                    .mime_type("text/plain"),
            ))
            .await;
        assert_eq!(res.status_code(), 303);

        // Notifikasi untuk member: ditugaskan.
        let member = a_user().name("Mira Member").create(&ctx).await;
        add_member(&ctx, &f.workspace, &member, RoleKey::Admin).await;
        post(
            format!("/w/acme/t/{epic}"),
            vec![("assignee", member.id.to_string())],
        )
        .await;
        let trashed = create_ticket(&request, &f.cookie, &[("title", "Dibuang")]).await;
        post(format!("/w/acme/t/{trashed}/delete"), vec![]).await;
        let type_id = t.type_id;

        let mut pages = vec![
            format!("/w/acme/t/{code}"),
            format!("/w/acme/t/{code}/activity"),
            format!("/w/acme/t/{epic}"),
            "/w/acme/p/KSR".to_string(),
            "/w/acme/p/KSR/board".into(),
            "/w/acme/p/KSR/board?who=me&q=struk".into(),
            "/w/acme/p/KSR/list".into(),
            "/w/acme/p/KSR/list?sort=-priority&status=in_progress".into(),
            "/w/acme/p/KSR/tickets/new".into(),
            format!("/w/acme/p/KSR/tickets/new?status=todo&parent={epic}"),
            "/w/acme/p/KSR/trash".into(),
            "/w/acme/p/KSR/settings/ticket-code".into(),
            "/w/acme/my".into(),
            "/w/acme/my?tab=reported".into(),
            "/w/acme/my?tab=watching".into(),
            "/w/acme/search".into(),
            "/w/acme/search?q=struk".into(),
            "/w/acme/inbox".into(),
            "/w/acme/settings/ticket-types".into(),
            "/w/acme/settings/ticket-types/new".into(),
            format!("/w/acme/settings/ticket-types/{type_id}"),
            format!("/w/acme/settings/fields/{}", field.id),
            "/w/acme/settings/ticket-code".into(),
        ];
        for path in pages.drain(..) {
            let res = request.get(&path).add_cookie(f.cookie.clone()).await;
            assert_eq!(res.status_code(), 200, "{path}: {}", res.text());
        }

        // Kotak masuk member berisi notifikasi; semua tab ter-render.
        let cookie = sign_in_as(&request, &member).await;
        for path in ["/w/acme/inbox", "/w/acme/inbox?tab=unread", "/w/acme/inbox?tab=mentions"] {
            let res = request.get(path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 200, "{path}: {}", res.text());
        }
        let res = request.get("/w/acme/inbox").add_cookie(cookie).await;
        assert_css_exists(&res.text(), "[data-notification='assigned'][data-unread]");
    })
    .await;
}
