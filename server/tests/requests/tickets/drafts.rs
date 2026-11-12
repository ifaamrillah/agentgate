//! Project → Draft tiket: daftar pohon draft, ubah (validasi), hapus, buat tiket dengan
//! hierarki dari draft yang dicentang, izin (Viewer hanya melihat, non-anggota 404).

use agentgate_server::{
    app::App,
    models::{project_grants::GrantSubject, projects, roles::RoleKey, ticket_drafts, tickets},
    services::{drafts, drafts::DraftInput, workspaces::Acting},
};
use loco_rs::testing::prelude::*;

use super::fixture;
use crate::support::{a_user, add_member, builtin_role, grant, sign_in_as};

#[tokio::test]
async fn drafts_are_reviewed_and_created_as_tickets() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let ksr = projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        let (_, role) = agentgate_server::models::workspace_members::Model::find_with_role(
            &ctx.db, wid, f.owner.id,
        )
        .await
        .unwrap()
        .unwrap();
        let acting = Acting {
            workspace: &f.workspace,
            user: &f.owner,
            role: &role,
            request_id: None,
        };
        let input = |r: &str, parent: Option<&str>, ty: &str, title: &str| DraftInput {
            reference: r.into(),
            parent: parent.map(str::to_string),
            type_key: ty.into(),
            title: title.into(),
            description: format!("Deskripsi {title}"),
            story_points: Some(3),
        };
        drafts::save_from_agent(
            &ctx.db,
            &acting,
            ksr.id,
            None,
            &[
                input("e", None, "epic", "Struk digital"),
                input("s1", Some("e"), "story", "Kirim via email"),
                input("t1", Some("s1"), "subtask", "Template email"),
                input("s2", Some("e"), "story", "Kirim via WhatsApp"),
            ],
        )
        .await
        .unwrap();
        let all = ticket_drafts::Model::of_project(&ctx.db, wid, ksr.id)
            .await
            .unwrap();
        let id = |title: &str| all.iter().find(|d| d.title == title).unwrap().id;

        let board = request
            .get("/w/acme/p/KSR/board")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(
            &board.text(),
            "a[data-drafts-link][href=\"/w/acme/p/KSR/drafts\"]",
        );
        let page = request
            .get("/w/acme/p/KSR/drafts")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200, "{}", page.text());
        let html = page.text();
        assert_css_exists(&html, "[data-draft-list] [data-draft='Template email']");
        assert_css_exists(
            &html,
            "form#drafts-create[action=\"/w/acme/p/KSR/drafts/create\"]",
        );
        assert_css_exists(&html, "input[name=draft][form=drafts-create][checked]");

        // Ubah: judul kosong → 422 dialog terbuka; valid → tersimpan.
        let edit = format!("/w/acme/p/KSR/drafts/{}", id("Kirim via WhatsApp"));
        let res = request
            .post(&edit)
            .add_cookie(f.cookie.clone())
            .form(&[
                ("title", ""),
                ("type", "story"),
                ("story_points", "2"),
                ("description", ""),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "dialog[data-open-on-load]");
        let res = request
            .post(&edit)
            .add_cookie(f.cookie.clone())
            .form(&[
                ("title", "Kirim via WA"),
                ("type", "story"),
                ("story_points", "2"),
                ("description", ""),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        // Hapus draft WhatsApp.
        let res = request
            .post(&format!("{edit}/delete"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);

        // Tanpa centang → 422.
        let res = request
            .post("/w/acme/p/KSR/drafts/create")
            .add_cookie(f.cookie.clone())
            .form(&[("draft", "")])
            .await;
        assert_eq!(res.status_code(), 422);

        // Buat epic + story + subtask.
        let chosen = [
            id("Struk digital"),
            id("Kirim via email"),
            id("Template email"),
        ];
        let form: Vec<(&str, String)> = chosen.iter().map(|i| ("draft", i.to_string())).collect();
        let res = request
            .post("/w/acme/p/KSR/drafts/create")
            .add_cookie(f.cookie.clone())
            .form(&form)
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(
            ticket_drafts::Model::of_project(&ctx.db, wid, ksr.id)
                .await
                .unwrap()
                .is_empty()
        );
        let find = |title: &'static str| {
            let db = ctx.db.clone();
            async move {
                use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
                tickets::Entity::find()
                    .filter(tickets::Column::Title.eq(title))
                    .one(&db)
                    .await
                    .unwrap()
                    .unwrap()
            }
        };
        let epic = find("Struk digital").await;
        let story = find("Kirim via email").await;
        let sub = find("Template email").await;
        assert_eq!(story.parent_id, Some(epic.id));
        assert_eq!(sub.parent_id, Some(story.id));
        assert_eq!(story.story_points, Some(3));

        // Viewer: melihat tanpa aksi, POST ditolak; non-anggota 404.
        let viewer = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &viewer, RoleKey::Member).await;
        let viewer_role = builtin_role(&ctx, &f.workspace, RoleKey::Viewer).await;
        grant(
            &ctx,
            &f.workspace,
            &ksr,
            GrantSubject::User(viewer.id),
            &viewer_role,
        )
        .await;
        let vc = sign_in_as(&request, &viewer).await;
        let page = request
            .get("/w/acme/p/KSR/drafts")
            .add_cookie(vc.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_not_exists(&page.text(), "form#drafts-create");
        let res = request
            .post("/w/acme/p/KSR/drafts/create")
            .add_cookie(vc)
            .form(&[("draft", "")])
            .await;
        assert!(
            matches!(res.status_code().as_u16(), 403 | 404),
            "{}",
            res.status_code()
        );
        let stranger = a_user().create(&ctx).await;
        let sc = sign_in_as(&request, &stranger).await;
        let res = request.get("/w/acme/p/KSR/drafts").add_cookie(sc).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
