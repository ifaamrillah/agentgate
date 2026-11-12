//! Komentar, @mention → notifikasi kotak masuk, watcher, dan tandai dibaca.

use agentgate_server::{
    app::App,
    models::{
        notifications::{self, NotificationKind},
        project_grants::GrantSubject,
        roles::RoleKey,
        ticket_comments, ticket_watchers,
    },
};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ticket};
use crate::support::{a_user, add_member, builtin_role, grant, sign_in_as};

#[tokio::test]
async fn mention_in_comment_notifies_only_people_with_access() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let dewi = a_user()
            .name("Dewi Dev")
            .email("dewi@contoh.id")
            .create(&ctx)
            .await;
        let luar = a_user()
            .name("Lia Luar")
            .email("lia@contoh.id")
            .create(&ctx)
            .await;
        add_member(&ctx, &f.workspace, &dewi, RoleKey::Member).await;
        add_member(&ctx, &f.workspace, &luar, RoleKey::Member).await;
        let ksr = agentgate_server::models::projects::Model::find_by_key(&ctx.db, wid, "KSR")
            .await
            .unwrap()
            .unwrap();
        let member = builtin_role(&ctx, &f.workspace, RoleKey::Member).await;
        grant(
            &ctx,
            &f.workspace,
            &ksr,
            GrantSubject::User(dewi.id),
            &member,
        )
        .await;

        let code = create_ticket(&request, &f.cookie, &[("title", "Struk pajak")]).await;
        let t = ticket(&ctx, &f.workspace, &code).await;

        let res = request
            .post(&format!("/w/acme/t/{code}/comments"))
            .add_cookie(f.cookie.clone())
            .form(&[("body", "")])
            .await;
        assert_eq!(res.status_code(), 422);

        let res = request
            .post(&format!("/w/acme/t/{code}/comments"))
            .add_cookie(f.cookie.clone())
            .form(&[("body", "Tolong cek @dewi@contoh.id dan @lia@contoh.id")])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        // Dewi (punya akses) mendapat notifikasi; Lia (tanpa akses project) tidak; pelaku tidak.
        let (dewi_inbox, _) = notifications::Model::inbox(&ctx.db, wid, dewi.id, &[], false, 0, 10)
            .await
            .unwrap();
        assert_eq!(dewi_inbox.len(), 1);
        assert_eq!(dewi_inbox[0].kind().unwrap(), NotificationKind::Mentioned);
        assert_eq!(dewi_inbox[0].ticket_id, Some(t.id));
        for user in [&luar, &f.owner] {
            assert_eq!(
                notifications::Model::unread_count(&ctx.db, wid, user.id)
                    .await
                    .unwrap(),
                0
            );
        }

        // Dewi melihat notifikasi di kotak masuk dan badge sidebar, lalu membacanya.
        let cookie = sign_in_as(&request, &dewi).await;
        let page = request
            .get("/w/acme/inbox")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        let html = page.text();
        assert_css_exists(&html, "[data-notification][data-unread]");
        assert_css_exists(&html, "[data-nav='inbox'] .app-count");
        let res = request
            .post(&format!("/w/acme/inbox/{}/read", dewi_inbox[0].id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(res.header("location"), format!("/w/acme/t/{code}"));
        assert_eq!(
            notifications::Model::unread_count(&ctx.db, wid, dewi.id)
                .await
                .unwrap(),
            0
        );

        // Dewi berkomentar: dia jadi watcher, pemilik (watcher) dapat notifikasi "commented".
        let res = request
            .post(&format!("/w/acme/t/{code}/comments"))
            .add_cookie(cookie.clone())
            .form(&[("body", "Sudah dicek")])
            .await;
        assert_eq!(res.status_code(), 303);
        let watchers = ticket_watchers::Model::user_ids(&ctx.db, wid, t.id)
            .await
            .unwrap();
        assert!(watchers.contains(&dewi.id));
        let (owner_inbox, _) =
            notifications::Model::inbox(&ctx.db, wid, f.owner.id, &[], true, 0, 10)
                .await
                .unwrap();
        assert_eq!(owner_inbox[0].kind().unwrap(), NotificationKind::Commented);

        // Hanya penulis yang bisa mengubah/menghapus komentarnya.
        let comments = ticket_comments::Model::of_ticket(&ctx.db, wid, t.id)
            .await
            .unwrap();
        let dewis = comments
            .iter()
            .find(|c| c.author_id == Some(dewi.id))
            .unwrap();
        let res = request
            .post(&format!("/w/acme/t/{code}/comments/{}", dewis.id))
            .add_cookie(f.cookie.clone())
            .form(&[("body", "Dibajak")])
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post(&format!("/w/acme/t/{code}/comments/{}", dewis.id))
            .add_cookie(cookie.clone())
            .form(&[("body", "Sudah dicek ulang")])
            .await;
        assert_eq!(res.status_code(), 303);
        let res = request
            .post(&format!("/w/acme/t/{code}/comments/{}/delete", dewis.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);

        // Tandai semua dibaca.
        let owner_cookie = f.cookie.clone();
        let res = request
            .post("/w/acme/inbox/read-all")
            .add_cookie(owner_cookie)
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            notifications::Model::unread_count(&ctx.db, wid, f.owner.id)
                .await
                .unwrap(),
            0
        );
    })
    .await;
}

#[tokio::test]
async fn assigning_notifies_the_assignee() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let dewi = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &dewi, RoleKey::Admin).await;
        let code = create_ticket(
            &request,
            &f.cookie,
            &[("title", "Tugas"), ("assignee", &dewi.id.to_string())],
        )
        .await;
        let (inbox, _) = notifications::Model::inbox(&ctx.db, wid, dewi.id, &[], false, 0, 10)
            .await
            .unwrap();
        assert_eq!(inbox[0].kind().unwrap(), NotificationKind::Assigned);
        // Penerima tugas tanpa akses project ditolak.
        let outsider = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &outsider, RoleKey::Member).await;
        let res = request
            .post(&format!("/w/acme/t/{code}"))
            .add_cookie(f.cookie.clone())
            .form(&[("assignee", &outsider.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 422);
        // Endpoint orang project hanya memuat yang punya akses.
        let res = request
            .get("/w/acme/p/KSR/people?q=")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(html.contains(&dewi.id.to_string()));
        assert!(!html.contains(&outsider.id.to_string()));
    })
    .await;
}
