//! Pengaturan tiket: tipe tiket (bawaan tidak bisa dihapus, tipe terpakai tidak bisa dihapus),
//! field custom (wajib, validasi nilai), template kode workspace dan override project (kode lama
//! menjadi alias yang di-redirect), dan izin `ManageTicketSettings`.

use agentgate_server::{
    app::App,
    models::{audit_events::AuditAction, custom_fields, roles::RoleKey, ticket_types},
};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ticket};
use crate::support::{a_user, add_member, newest_audit_events, sign_in_as};

#[tokio::test]
async fn ticket_types_and_custom_fields() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();

        let res = request
            .get("/w/acme/settings/ticket-types")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        for key in ["epic", "story", "task", "bug", "subtask"] {
            assert_css_exists(&res.text(), &format!("[data-ticket-type='{key}']"));
        }

        // Buat tipe custom.
        let res = request
            .post("/w/acme/settings/ticket-types/new")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("key", "spike"),
                ("name", "Spike"),
                ("description", "Riset singkat"),
                ("icon", "bulb"),
                ("color", "#0891b2"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let spike = ticket_types::Model::all_in(&ctx.db, wid)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.key == "spike")
            .unwrap();
        // Kunci kembar → 422.
        let res = request
            .post("/w/acme/settings/ticket-types/new")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("key", "spike"),
                ("name", "Spike 2"),
                ("icon", "bulb"),
                ("color", "#0891b2"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);

        // Field custom wajib untuk Spike.
        let res = request
            .post("/w/acme/settings/fields")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Toko pilot"),
                ("kind", "select"),
                ("options", "Semua toko\nToko Pusat"),
                ("required", "on"),
                ("type_ids", &spike.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let field = custom_fields::Model::all_in(&ctx.db, wid)
            .await
            .unwrap()
            .remove(0);

        // Tiket Spike tanpa nilai field wajib → 422; nilai bukan pilihan → 422; benar → 303.
        let key = format!("custom_{}", field.id);
        let res = request
            .post("/w/acme/p/KSR/tickets/new")
            .add_cookie(f.cookie.clone())
            .form(&[("title", "Riset"), ("type_id", &spike.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), &format!("#ticket-{key}-error"));
        let res = request
            .post("/w/acme/p/KSR/tickets/new")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("title", "Riset"),
                ("type_id", &spike.id.to_string()),
                (&key, "Toko Cabang"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        let code = create_ticket(
            &request,
            &f.cookie,
            &[
                ("title", "Riset"),
                ("type_id", &spike.id.to_string()),
                (&key, "Toko Pusat"),
            ],
        )
        .await;
        let t = ticket(&ctx, &f.workspace, &code).await;
        assert_eq!(t.custom_values().0[&field.id], "Toko Pusat");

        // Tipe terpakai dan tipe bawaan tidak bisa dihapus.
        let res = request
            .post(&format!(
                "/w/acme/settings/ticket-types/{}/delete",
                spike.id
            ))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 422, "{}", res.text());
        let task = ticket_types::Model::all_in(&ctx.db, wid)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.key == "task")
            .unwrap();
        let res = request
            .post(&format!("/w/acme/settings/ticket-types/{}/delete", task.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 422);

        // Ubah tipe bawaan (nama, warna) boleh.
        let res = request
            .post(&format!("/w/acme/settings/ticket-types/{}", task.id))
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Tugas"),
                ("icon", "square-check"),
                ("color", "#2563eb"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        let events = newest_audit_events(&ctx.db, wid, 3).await.unwrap();
        assert_eq!(events[0].action().unwrap(), AuditAction::TicketTypeUpdated);
        assert_eq!(events[1].action().unwrap(), AuditAction::CustomFieldCreated);
        assert_eq!(events[2].action().unwrap(), AuditAction::TicketTypeCreated);

        // Hapus field.
        let res = request
            .post(&format!("/w/acme/settings/fields/{}/delete", field.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            custom_fields::Model::all_in(&ctx.db, wid)
                .await
                .unwrap()
                .is_empty()
        );

        // Member biasa: 404.
        let member = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &member, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &member).await;
        for path in [
            "/w/acme/settings/ticket-types",
            "/w/acme/settings/ticket-code",
        ] {
            let res = request.get(path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request
            .post("/w/acme/settings/ticket-types/new")
            .add_cookie(cookie)
            .form(&[
                ("key", "x"),
                ("name", "X"),
                ("icon", "bulb"),
                ("color", "#0891b2"),
            ])
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn ticket_code_template_recodes_and_keeps_aliases() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let code = create_ticket(&request, &f.cookie, &[("title", "Lama")]).await;
        assert_eq!(code, "KSR-1");

        // Template tanpa {NUM} ditolak.
        let res = request
            .post("/w/acme/settings/ticket-code")
            .add_cookie(f.cookie.clone())
            .form(&[("template", "{PROJECT}"), ("scope", "project")])
            .await;
        assert_eq!(res.status_code(), 422);

        let res = request
            .post("/w/acme/settings/ticket-code")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("template", "{PROJECT}-{TYPE}-{NUM:3}"),
                ("scope", "project"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!(t.code, "KSR-TASK-001");
        // Kode lama di-redirect ke kode baru.
        let res = request
            .get("/w/acme/t/KSR-1")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 301);
        assert_eq!(res.header("location"), "/w/acme/t/KSR-TASK-001");
        let next = create_ticket(&request, &f.cookie, &[("title", "Baru")]).await;
        assert_eq!(next, "KSR-TASK-002");

        // Override project dengan kode tim.
        let res = request
            .post("/w/acme/p/KSR/settings/ticket-code")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("custom", "on"),
                ("template", "{TEAM}-{NUM}"),
                ("scope", "project"),
                ("team_code", "pay"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(ticket(&ctx, &f.workspace, "KSR-1").await.code, "PAY-1");
        let res = request
            .get("/w/acme/p/KSR/settings/ticket-code")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);

        let events = newest_audit_events(&ctx.db, wid, 2).await.unwrap();
        assert!(
            events
                .iter()
                .all(|e| e.action().unwrap() == AuditAction::TicketCodeUpdated)
        );
    })
    .await;
}
