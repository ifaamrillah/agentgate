//! Tiga mode sinkron terhadap Jira palsu: mirror end-to-end (tiket baru, field, stage, komentar,
//! rekonsiliasi menimpa Jira), dua arah (status yang melanggar gate ditolak + dikembalikan,
//! perubahan terakhir menang, issue/komentar baru dari Jira, tanpa gema/loop), impor sekali
//! (alias, komentar, lampiran besar dilewati, user tak dipetakan, stage bergate, idempoten),
//! dan gangguan Jira (last_error + satu notifikasi).

use agentgate_server::{
    app::App,
    models::{
        jira_connections, jira_links, notifications, sprints, ticket_attachments, ticket_comments,
        tickets,
    },
};
use loco_rs::{TestServer, app::AppContext, auth::cookies::Cookie, testing::prelude::*};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait,
    QueryFilter,
};
use serde_json::json;

use super::{connect, connection, fixture, logs, messages, set_mode, sync_now};
use crate::{
    requests::tickets::{
        Fixture, create_ticket,
        stages::{FLOW_YAML, use_workflow},
        ticket,
    },
    support::fake_jira::{self, DONE, FakeJira, IN_PROGRESS, TODO},
};

async fn post(request: &TestServer, cookie: &Cookie<'static>, path: &str, form: &[(&str, &str)]) {
    let res = request
        .post(path)
        .add_cookie(cookie.clone())
        .form(form)
        .await;
    assert_eq!(res.status_code(), 303, "{path}: {}", res.text());
}

async fn link_of(ctx: &AppContext, f: &Fixture, code: &str) -> jira_links::Model {
    let t = ticket(ctx, &f.workspace, code).await;
    jira_links::Entity::find()
        .filter(jira_links::Column::TicketId.eq(t.id))
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("{code} belum tertaut"))
}

async fn comments_of(ctx: &AppContext, f: &Fixture, code: &str) -> Vec<String> {
    let t = ticket(ctx, &f.workspace, code).await;
    ticket_comments::Model::of_ticket(&ctx.db, f.workspace.workspace_id(), t.id)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.body)
        .collect()
}

fn summary(fake: &FakeJira, key: &str) -> String {
    fake.issue(key).fields["summary"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn mirror_pushes_tickets_fields_stages_and_comments() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = fixture(&request, &ctx).await;
        create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        connect(&request, &f.cookie, &fake).await;
        // Belum ada mode tersimpan: koneksi saja tidak mendorong apa pun.
        assert!(fake.writes().is_empty());

        // Putaran pertama mirror: tiket yang sudah ada menjadi issue.
        set_mode(&request, &f.cookie, "mirror").await;
        let l1 = link_of(&ctx, &f, "KSR-1").await;
        assert_eq!(summary(&fake, &l1.issue_key), "Checkout QRIS");
        assert_eq!(l1.jira_status_id, TODO);

        // Tiket baru → issue baru (setelah request, tanpa scheduler).
        create_ticket(&request, &f.cookie, &[("title", "Struk digital")]).await;
        let l2 = link_of(&ctx, &f, "KSR-2").await;
        assert_eq!(summary(&fake, &l2.issue_key), "Struk digital");

        // Judul/label/prioritas/story point berubah → update issue (hanya field yang berubah).
        fake.clear_requests();
        post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1",
            &[
                ("title", "Checkout QRIS dinamis"),
                ("labels", "pembayaran, qris"),
                ("priority", "high"),
                ("story_points", "5"),
            ],
        )
        .await;
        let issue = fake.issue(&l1.issue_key);
        assert_eq!(issue.fields["summary"], "Checkout QRIS dinamis");
        assert_eq!(issue.fields["labels"], json!(["pembayaran", "qris"]));
        assert_eq!(issue.fields["priority"]["name"], "High");
        assert_eq!(issue.fields["customfield_10016"], json!(5));
        let puts: Vec<_> = fake
            .writes()
            .into_iter()
            .filter(|r| r.method == "PUT")
            .collect();
        assert_eq!(puts.len(), 1, "{puts:?}");
        assert!(puts[0].body["fields"].get("description").is_none());

        // Stage berubah → transisi Jira ke status yang dipetakan.
        post(&request, &f.cookie, "/w/acme/t/KSR-1/stage/complete", &[]).await;
        assert_eq!(
            ticket(&ctx, &f.workspace, "KSR-1").await.stage_key,
            "in_progress"
        );
        assert_eq!(fake.issue(&l1.issue_key).status_id, IN_PROGRESS);

        // Komentar baru → komentar Jira (sekali).
        post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/comments",
            &[("body", "Sudah dicek")],
        )
        .await;
        let comments = fake.issue(&l1.issue_key).comments;
        assert_eq!(comments.len(), 1);
        let text = fake_jira::text_of(&comments[0]["body"]);
        assert!(
            text.contains("Sudah dicek") && text.contains("Olivia Owner"),
            "{text}"
        );

        // Putaran berikutnya tanpa perubahan tidak menulis apa pun.
        fake.clear_requests();
        sync_now(&request, &f.cookie).await;
        assert!(fake.writes().is_empty(), "{:?}", fake.writes());

        // Mirror: perubahan di Jira tidak ditarik …
        fake.edit(&l1.issue_key, json!({ "summary": "Diubah di Jira" }), 0);
        sync_now(&request, &f.cookie).await;
        assert_eq!(
            ticket(&ctx, &f.workspace, "KSR-1").await.title,
            "Checkout QRIS dinamis"
        );
        // … dan Sinkron ulang semua menimpanya.
        post(&request, &f.cookie, "/w/acme/settings/jira/resync", &[]).await;
        assert_eq!(summary(&fake, &l1.issue_key), "Checkout QRIS dinamis");
        assert!(connection(&ctx).await.full_sync_requested_at.is_none());
        assert_eq!(fake.issues().len(), 2);
        assert_eq!(fake.issue(&l1.issue_key).comments.len(), 1);

        let msgs = messages(&ctx).await;
        for m in [
            "jira-log-issue-created",
            "jira-log-issue-updated",
            "jira-log-status-pushed",
            "jira-log-comment-pushed",
        ] {
            assert!(msgs.iter().any(|x| x == m), "{m}: {msgs:?}");
        }
        let conn = connection(&ctx).await;
        assert!(conn.last_sync_at.is_some());
        assert_eq!(conn.last_error, "");
        assert_eq!(conn.stats().tickets, 2);
        let res = request
            .get("/w/acme/settings/jira/overview")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(&res.text(), "[data-stat='jira-stat-tickets']");
    })
    .await;
}

/// Dua arah di workflow bergate (todo → prd [gate + approval] → dev → done).
async fn two_way(request: &TestServer, ctx: &AppContext, fake: &FakeJira) -> Fixture {
    let f = fixture(request, ctx).await;
    use_workflow(request, ctx, &f, FLOW_YAML).await;
    create_ticket(request, &f.cookie, &[("title", "Checkout QRIS")]).await;
    connect(request, &f.cookie, fake).await;
    set_mode(request, &f.cookie, "two_way").await;
    f
}

#[tokio::test]
async fn two_way_rejects_gate_violations_and_reverts_jira() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = two_way(&request, &ctx, &fake).await;
        let link = link_of(&ctx, &f, "KSR-1").await;
        assert_eq!(ticket(&ctx, &f.workspace, "KSR-1").await.stage_key, "todo");

        // Done melompati PRD (gate + approval) → ditolak, status Jira kembali ke To Do.
        fake.set_status(&link.issue_key, DONE);
        sync_now(&request, &f.cookie).await;
        assert_eq!(ticket(&ctx, &f.workspace, "KSR-1").await.stage_key, "todo");
        assert_eq!(fake.issue(&link.issue_key).status_id, TODO);
        let gate: Vec<_> = logs(&ctx)
            .await
            .into_iter()
            .filter(|l| l.result == "gate")
            .collect();
        assert_eq!(gate.len(), 1);
        assert_eq!(gate[0].message, "jira-log-gate-rejected");
        assert_eq!(gate[0].direction, "in");
        assert_eq!(gate[0].args().back, "To Do");
        assert_eq!(gate[0].args().reason, "ticket-jump-admin-only");
        // Pesan dirender lengkap di Log sinkron.
        let res = request
            .get("/w/acme/settings/jira/log?result=gate")
            .add_cookie(f.cookie.clone())
            .await;
        let html = res.text();
        assert!(
            html.contains("ditolak") && html.contains("Dikembalikan ke To Do"),
            "{html}"
        );

        // In Progress → PRD diizinkan (todo tanpa gate) lewat gate service.
        fake.set_status(&link.issue_key, IN_PROGRESS);
        sync_now(&request, &f.cookie).await;
        assert_eq!(ticket(&ctx, &f.workspace, "KSR-1").await.stage_key, "prd");
        assert_eq!(fake.issue(&link.issue_key).status_id, IN_PROGRESS);
        // Done dari PRD butuh approval → ditolak, kembali ke In Progress.
        fake.set_status(&link.issue_key, DONE);
        sync_now(&request, &f.cookie).await;
        assert_eq!(ticket(&ctx, &f.workspace, "KSR-1").await.stage_key, "prd");
        assert_eq!(fake.issue(&link.issue_key).status_id, IN_PROGRESS);
        assert_eq!(
            logs(&ctx)
                .await
                .iter()
                .filter(|l| l.result == "gate")
                .count(),
            2
        );

        // Tanpa perubahan baru: tidak ada tulis ke Jira (status yang dikembalikan tidak memantul).
        fake.clear_requests();
        sync_now(&request, &f.cookie).await;
        sync_now(&request, &f.cookie).await;
        assert!(fake.writes().is_empty(), "{:?}", fake.writes());
    })
    .await;
}

#[tokio::test]
async fn two_way_last_writer_wins_without_loops() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = two_way(&request, &ctx, &fake).await;
        let link = link_of(&ctx, &f, "KSR-1").await;

        // Jira mengubah judul & label → ditarik; tidak ada tulis balik ke Jira.
        fake.clear_requests();
        fake.edit(
            &link.issue_key,
            json!({ "summary": "Judul dari Jira", "labels": ["kasir"] }),
            0,
        );
        sync_now(&request, &f.cookie).await;
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!(t.title, "Judul dari Jira");
        assert_eq!(t.labels, ["kasir"]);
        assert!(fake.writes().is_empty(), "gema: {:?}", fake.writes());

        // Bentrok pada field yang sama, Jira lebih baru → Jira menang.
        post(&request, &f.cookie, "/w/acme/settings/jira/pause", &[]).await;
        post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1",
            &[("title", "Judul Agent Gate")],
        )
        .await;
        fake.edit(
            &link.issue_key,
            json!({ "summary": "Judul Jira terbaru" }),
            60,
        );
        post(&request, &f.cookie, "/w/acme/settings/jira/resume", &[]).await;
        assert_eq!(
            ticket(&ctx, &f.workspace, "KSR-1").await.title,
            "Judul Jira terbaru"
        );
        assert_eq!(summary(&fake, &link.issue_key), "Judul Jira terbaru");

        // Bentrok, Agent Gate lebih baru → Agent Gate menang dan Jira diperbarui; field berbeda
        // digabung (label dari Jira tetap masuk).
        post(&request, &f.cookie, "/w/acme/settings/jira/pause", &[]).await;
        fake.edit(
            &link.issue_key,
            json!({ "summary": "Judul Jira lama", "labels": ["kasir", "promo"] }),
            -30,
        );
        // Jeda ≥ 1 detik tidak diperlukan: updated tiket = sekarang > updated Jira (−30 dtk).
        post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1",
            &[("title", "Judul AG terbaru")],
        )
        .await;
        post(&request, &f.cookie, "/w/acme/settings/jira/resume", &[]).await;
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        assert_eq!(t.title, "Judul AG terbaru");
        assert_eq!(t.labels, ["kasir", "promo"]);
        assert_eq!(summary(&fake, &link.issue_key), "Judul AG terbaru");

        // Stabil: putaran berikutnya tidak menulis ke salah satu sisi.
        fake.clear_requests();
        let before = ticket(&ctx, &f.workspace, "KSR-1").await.updated_at;
        sync_now(&request, &f.cookie).await;
        sync_now(&request, &f.cookie).await;
        assert!(fake.writes().is_empty(), "loop: {:?}", fake.writes());
        assert_eq!(ticket(&ctx, &f.workspace, "KSR-1").await.updated_at, before);
    })
    .await;
}

#[tokio::test]
async fn two_way_pulls_new_issues_and_comments() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = two_way(&request, &ctx, &fake).await;
        let link = link_of(&ctx, &f, "KSR-1").await;

        // Issue baru di Jira (status In Progress) → tiket baru di stage awal, lalu status
        // dipindah lewat gate (todo → prd diizinkan).
        let key = fake.add_issue("Laporan harian", IN_PROGRESS, json!({}));
        fake.add_comment(&key, fake_jira::ACCOUNT, "Dibuat di Jira");
        fake.add_comment(&link.issue_key, fake_jira::ACCOUNT, "Tolong cek ulang");
        sync_now(&request, &f.cookie).await;
        let new = tickets::Entity::find()
            .filter(tickets::Column::Title.eq("Laporan harian"))
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("tiket dari Jira");
        assert_eq!(new.stage_key, "prd");
        let new_link = link_of(&ctx, &f, &new.code).await;
        assert_eq!(new_link.issue_key, key);
        let c1 = comments_of(&ctx, &f, "KSR-1").await;
        assert_eq!(c1.len(), 1);
        assert!(
            c1[0].contains("(Jira)") && c1[0].contains("Tolong cek ulang"),
            "{c1:?}"
        );
        assert_eq!(comments_of(&ctx, &f, &new.code).await.len(), 1);
        // Komentar dari Jira tidak dikirim balik; tidak ada issue ganda.
        assert_eq!(fake.issue(&link.issue_key).comments.len(), 1);
        assert_eq!(fake.issues().len(), 2);

        // Komentar Agent Gate → Jira (sekali); tarikan berikutnya tidak menyalinnya balik.
        post(
            &request,
            &f.cookie,
            "/w/acme/t/KSR-1/comments",
            &[("body", "Sudah")],
        )
        .await;
        assert_eq!(fake.issue(&link.issue_key).comments.len(), 2);
        sync_now(&request, &f.cookie).await;
        assert_eq!(comments_of(&ctx, &f, "KSR-1").await.len(), 2);
        assert!(
            messages(&ctx)
                .await
                .iter()
                .any(|m| m == "jira-log-ticket-created")
        );
    })
    .await;
}

#[tokio::test]
async fn import_once_brings_issues_comments_attachments_and_sprints() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        fake.add_user("acc-budi", "Budi Santoso", "budi@sevima-retail.id");
        let olivia = json!({ "accountId": fake_jira::ACCOUNT, "displayName": "Olivia Owner",
            "emailAddress": fake_jira::EMAIL });
        let a = fake.add_issue(
            "Bayar pakai QRIS",
            IN_PROGRESS,
            json!({
                "assignee": olivia,
                "labels": ["pembayaran"],
                "priority": { "name": "High" },
                "customfield_10016": 5,
                "customfield_10020": [{ "id": 7, "name": "Sprint 12",
                    "startDate": "2026-11-02T00:00:00.000Z", "endDate": "2026-11-16T00:00:00.000Z" }],
                "description": { "type": "doc", "version": 1, "content": [
                    { "type": "paragraph", "content": [{ "type": "text", "text": "Pakai QRIS dinamis" }] }] },
            }),
        );
        fake.add_comment(&a, fake_jira::ACCOUNT, "Komentar pertama");
        fake.add_comment(&a, "acc-budi", "Komentar kedua");
        fake.add_attachment(&a, "catatan.txt", b"isi catatan", 11);
        fake.add_attachment(&a, "video.mp4", b"besar", 32 * 1024 * 1024);
        let b = fake.add_issue(
            "Struk tanpa nama kasir",
            DONE,
            json!({ "assignee": { "accountId": "acc-budi", "displayName": "Budi Santoso",
                "emailAddress": "budi@sevima-retail.id" } }),
        );
        let f = fixture(&request, &ctx).await;
        use_workflow(&request, &ctx, &f, FLOW_YAML).await;
        connect(&request, &f.cookie, &fake).await;
        set_mode(&request, &f.cookie, "once").await;

        let wid = f.workspace.workspace_id();
        let (ta, _) = tickets::Model::find_by_code(&ctx.db, wid, &a).await.unwrap().expect("alias A");
        assert_eq!(ta.title, "Bayar pakai QRIS");
        assert_eq!(ta.description, "Pakai QRIS dinamis");
        assert_eq!(ta.stage_key, "prd");
        assert_eq!(ta.assignee_id, Some(f.owner.id));
        assert_eq!(ta.labels, ["pembayaran"]);
        assert_eq!(ta.priority, "high");
        assert_eq!(ta.story_points, Some(5));
        let sprint = sprints::Entity::find().one(&ctx.db).await.unwrap().expect("sprint");
        assert_eq!(sprint.name, "Sprint 12");
        assert_eq!(ta.sprint_id, Some(sprint.id));
        let comments = comments_of(&ctx, &f, &ta.code).await;
        assert_eq!(comments.len(), 2);
        assert!(comments.iter().any(|c| c.contains("Budi Santoso") && c.contains("Komentar kedua")));
        let files = ticket_attachments::Model::of_ticket(&ctx.db, wid, ta.id).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "catatan.txt");

        // Done melompati gate → stage awal + log; penerima tak dipetakan → kosong + log.
        let (tb, _) = tickets::Model::find_by_code(&ctx.db, wid, &b).await.unwrap().expect("alias B");
        assert_eq!(tb.stage_key, "todo");
        assert_eq!(tb.assignee_id, None);
        let all = logs(&ctx).await;
        let has = |m: &str| all.iter().any(|l| l.message == m);
        assert!(has("jira-log-attachment-skipped"));
        assert!(has("jira-log-user-unmapped"));
        assert!(has("jira-log-stage-gated"));
        let skipped = all
            .iter()
            .find(|l| l.message == "jira-log-attachment-skipped")
            .unwrap();
        assert_eq!(skipped.args().size, "32");
        assert_eq!(skipped.result, "fail");
        let conn = connection(&ctx).await;
        assert!(conn.imported_at.is_some());
        assert_eq!(conn.stats().tickets, 2);
        assert_eq!(conn.stats().attachments, 1);
        // Notifikasi per tiket impor tidak membanjiri kotak masuk.
        assert_eq!(notifications::Entity::find().count(&ctx.db).await.unwrap(), 0);
        // Impor sekali tidak menulis ke Jira.
        assert!(fake.writes().is_empty(), "{:?}", fake.writes());

        // Idempoten: impor ulang tidak menggandakan tiket, komentar, atau lampiran.
        post(&request, &f.cookie, "/w/acme/settings/jira/resync", &[]).await;
        sync_now(&request, &f.cookie).await;
        assert_eq!(
            tickets::Entity::find()
                .filter(tickets::Column::WorkspaceId.eq(wid.get()))
                .count(&ctx.db)
                .await
                .unwrap(),
            2
        );
        assert_eq!(comments_of(&ctx, &f, &ta.code).await.len(), 2);
        assert_eq!(
            ticket_attachments::Model::of_ticket(&ctx.db, wid, ta.id).await.unwrap().len(),
            1
        );
        // Ringkasan menampilkan hasil impor.
        let res = request
            .get("/w/acme/settings/jira/overview")
            .add_cookie(f.cookie.clone())
            .await;
        assert!(res.text().contains("2 tiket, 2 komentar, 1 lampiran, 1 sprint"), "{}", res.text());
    })
    .await;
}

#[tokio::test]
async fn unreachable_jira_sets_last_error_and_notifies_once() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = fixture(&request, &ctx).await;
        connect(&request, &f.cookie, &fake).await;
        set_mode(&request, &f.cookie, "mirror").await;
        let cursor = connection(&ctx).await.outbox_cursor();
        let mut row = connection(&ctx).await.into_active_model();
        row.base_url = Set("http://127.0.0.1:9".into());
        row.update(&ctx.db).await.unwrap();

        create_ticket(&request, &f.cookie, &[("title", "Checkout QRIS")]).await;
        sync_now(&request, &f.cookie).await;
        let conn = connection(&ctx).await;
        assert_eq!(conn.last_error, "jira-unreachable");
        // Cursor tidak maju: event tidak hilang.
        assert_eq!(conn.outbox_cursor(), cursor);
        let notes = notifications::Entity::find()
            .filter(notifications::Column::Kind.eq("jira_failed"))
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].user_id, f.owner.id);
        let res = request
            .get("/w/acme/settings/jira/overview")
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(&res.text(), "[data-jira-last-error]");

        // Kembali normal: tiket yang tertunda terdorong, error hilang.
        let mut row = connection(&ctx).await.into_active_model();
        row.base_url = Set(fake.url.clone());
        row.update(&ctx.db).await.unwrap();
        sync_now(&request, &f.cookie).await;
        let conn = connection(&ctx).await;
        assert_eq!(conn.last_error, "");
        assert_eq!(fake.issues().len(), 1);
        assert_eq!(
            notifications::Entity::find()
                .filter(notifications::Column::Kind.eq("jira_failed"))
                .count(&ctx.db)
                .await
                .unwrap(),
            1
        );
        let _ = jira_connections::Entity::find().count(&ctx.db).await;
    })
    .await;
}

#[tokio::test]
async fn data_center_uses_rest_v2_and_bearer_token() {
    request::<App, _, _>(|request, ctx| async move {
        let fake = fake_jira::start().await;
        let f = fixture(&request, &ctx).await;
        create_ticket(
            &request,
            &f.cookie,
            &[("title", "Checkout QRIS"), ("description", "Pakai QRIS")],
        )
        .await;
        let res = request
            .post("/w/acme/settings/jira/conn")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("env", "data_center"),
                ("base_url", fake.url.as_str()),
                ("email", ""),
                ("token", fake_jira::TOKEN),
                ("jira_project", "SR"),
                ("project", "KSR"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        set_mode(&request, &f.cookie, "mirror").await;
        let reqs = fake.requests();
        assert!(
            reqs.iter().all(|r| !r.path.contains("/rest/api/3/")),
            "{reqs:?}"
        );
        let link = link_of(&ctx, &f, "KSR-1").await;
        // Data Center: deskripsi teks wiki, bukan ADF.
        assert_eq!(
            fake.issue(&link.issue_key).fields["description"],
            "Pakai QRIS"
        );
    })
    .await;
}
