//! Relasi `blocks` ditegakkan: tiket yang diblokir tidak bisa mulai (board, form ubah) sebelum
//! pemblokirnya selesai. Relasi lain tidak membatasi.

use agentgate_server::{
    app::App,
    models::{ticket_links, tickets::TicketStatus},
};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ticket};

#[tokio::test]
async fn blocked_ticket_cannot_start_until_blocker_is_done() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let blocker = create_ticket(&request, &f.cookie, &[("title", "Integrasi QRIS")]).await;
        let blocked = create_ticket(&request, &f.cookie, &[("title", "Pajak PPN")]).await;

        // KSR-2 diblokir oleh KSR-1.
        let res = request
            .post(&format!("/w/acme/t/{blocked}/links"))
            .add_cookie(f.cookie.clone())
            .form(&[("kind", "blocked_by"), ("target", &blocker)])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let t = ticket(&ctx, &f.workspace, &blocked).await;
        assert_eq!(
            ticket_links::Model::of_ticket(&ctx.db, wid, t.id)
                .await
                .unwrap()
                .len(),
            1
        );
        // Relasi yang sama dua kali dan ke diri sendiri ditolak.
        let res = request
            .post(&format!("/w/acme/t/{blocked}/links"))
            .add_cookie(f.cookie.clone())
            .form(&[("kind", "blocked_by"), ("target", &blocker)])
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .post(&format!("/w/acme/t/{blocked}/links"))
            .add_cookie(f.cookie.clone())
            .form(&[("kind", "relates"), ("target", &blocked)])
            .await;
        assert_eq!(res.status_code(), 422);

        // Pindah ke in_progress ditolak (board dan form ubah).
        let res = request
            .post(&format!("/w/acme/t/{blocked}/status"))
            .add_cookie(f.cookie.clone())
            .form(&[("status", "in_progress")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains(&blocker), "pesan menyebut pemblokir");
        let res = request
            .post(&format!("/w/acme/t/{blocked}"))
            .add_cookie(f.cookie.clone())
            .form(&[("status", "done")])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_eq!(
            ticket(&ctx, &f.workspace, &blocked).await.status().unwrap(),
            TicketStatus::Todo
        );
        // Pindah antar-kolom yang belum mulai tetap boleh.
        let res = request
            .post(&format!("/w/acme/t/{blocked}/status"))
            .add_cookie(f.cookie.clone())
            .form(&[("status", "backlog")])
            .await;
        assert_eq!(res.status_code(), 303);

        // Pemblokir selesai → boleh mulai.
        let res = request
            .post(&format!("/w/acme/t/{blocker}/status"))
            .add_cookie(f.cookie.clone())
            .form(&[("status", "done")])
            .await;
        assert_eq!(res.status_code(), 303);
        let res = request
            .post(&format!("/w/acme/t/{blocked}/status"))
            .add_cookie(f.cookie.clone())
            .form(&[("status", "in_progress")])
            .await;
        assert_eq!(res.status_code(), 303);
        assert_eq!(
            ticket(&ctx, &f.workspace, &blocked).await.status().unwrap(),
            TicketStatus::InProgress
        );

        // Hapus relasi.
        let link = ticket_links::Model::of_ticket(&ctx.db, wid, t.id)
            .await
            .unwrap()
            .remove(0);
        let res = request
            .post(&format!("/w/acme/t/{blocked}/links/{}/delete", link.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            ticket_links::Model::of_ticket(&ctx.db, wid, t.id)
                .await
                .unwrap()
                .is_empty()
        );
    })
    .await;
}
