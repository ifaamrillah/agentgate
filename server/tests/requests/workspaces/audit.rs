//! `GET /w/{slug}/settings/audit`: audit log workspace (`ViewAuditLog`), keyset 50 baris per
//! halaman, filter kelompok aksi.

use std::collections::HashSet;

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        permission::Permission,
        roles::RoleKey,
        workspace_members, workspaces,
    },
    services::audit::{self, AuditEntry},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use loco_rs::{app::AppContext, testing::prelude::*};

use crate::support::{a_user, a_workspace, add_member, builtin_role, custom_role, sign_in_as};

const AUDIT: &str = "/w/acme/settings/audit";

/// Nilai atribut `name` dari setiap potongan HTML (hasil `select`).
fn attr_values(fragments: &[String], name: &str) -> Vec<String> {
    let marker = format!("{name}=\"");
    fragments
        .iter()
        .filter_map(|f| {
            let start = f.find(&marker)? + marker.len();
            let end = f[start..].find('"')?;
            Some(f[start..start + end].replace("&amp;", "&"))
        })
        .collect()
}

/// `count` baris audit sistem (tanpa pelaku) di `workspace`, bergantian `member.invited` dan
/// `role.created`.
async fn record_many(ctx: &AppContext, workspace: &workspaces::Model, count: usize) {
    for i in 0..count {
        let (action, subject) = if i % 2 == 0 {
            (AuditAction::MemberInvited, AuditSubject::Invitation)
        } else {
            (AuditAction::RoleCreated, AuditSubject::Role)
        };
        audit::record(
            &ctx.db,
            AuditEntry {
                workspace_id: workspace.workspace_id(),
                actor_id: None,
                action,
                subject: (subject, uuid::Uuid::now_v7()),
                changes: serde_json::json!({ "name": format!("baris {i}") }),
                request_id: None,
            },
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn audit_rows_written_for_invite_and_role_change() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().name("Olivia Owner").create(&ctx).await;
        let budi = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let member = add_member(&ctx, &workspace, &budi, RoleKey::Member).await;
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &owner).await;

        let res = request
            .post("/w/acme/settings/members/invitations")
            .add_cookie(cookie.clone())
            .form(&[
                ("email", "tamu@example.test"),
                ("role_id", &admin_role.id.to_string()),
                ("job_title_id", ""),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let res = request
            .post(&format!("/w/acme/settings/members/{}/role", member.id))
            .add_cookie(cookie.clone())
            .form(&[("role_id", admin_role.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        let events = crate::support::newest_audit_events(&ctx.db, workspace.workspace_id(), 10)
            .await
            .unwrap();
        let actions: Vec<_> = events.iter().map(|e| e.action().unwrap()).collect();
        assert_eq!(
            actions,
            [
                AuditAction::MemberRoleChanged,
                AuditAction::MemberInvited,
                AuditAction::WorkspaceCreated
            ]
        );
        assert!(events.iter().all(|e| e.actor_id == Some(owner.id)));

        let page = request.get(AUDIT).add_cookie(cookie.clone()).await;
        assert_eq!(page.status_code(), 200, "{}", page.text());
        let html = page.text();
        assert_css_exists(&html, "[data-audit-log]");
        // Panel detail: default baris pertama; `?event=` membuka kejadian lain di halaman ini.
        assert_css_exists(&html, &format!("[data-audit-detail=\"{}\"]", events[0].id));
        let invited = request
            .get(&format!("{AUDIT}?event={}", events[1].id))
            .add_cookie(cookie)
            .await
            .text();
        assert_css_exists(
            &invited,
            &format!(
                "tr[data-audit-id=\"{}\"][aria-selected=\"true\"]",
                events[1].id
            ),
        );
        assert_css_exists(&invited, "[data-audit-changes]");
        assert!(invited.contains("tamu@example.test"), "{invited}");
        let rows = select(&html, "tr[data-audit-row]");
        assert_eq!(
            attr_values(&rows, "data-action"),
            ["member.role_changed", "member.invited", "workspace.created"]
        );
        // Label aksi dari Fluent, nama pelaku, dan ringkasan perubahan.
        assert!(html.contains("Role anggota diganti"), "{html}");
        assert!(html.contains("Anggota diundang"), "{html}");
        assert!(html.contains("Olivia Owner"), "{html}");
        assert_css_exists(&html, "tr[data-audit-row] time[datetime]");
        assert_css_not_exists(&html, "a[data-audit-older]");
    })
    .await;
}

#[tokio::test]
async fn audit_page_paginates_and_filters() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        // 1 baris workspace.created + 60 = 61 baris: halaman 50 + 11.
        record_many(&ctx, &workspace, 60).await;
        let cookie = sign_in_as(&request, &owner).await;

        let first = request.get(AUDIT).add_cookie(cookie.clone()).await.text();
        let rows = select(&first, "tr[data-audit-row]");
        assert_eq!(rows.len(), 50);
        // Baris tanpa pelaku ditulis "Sistem".
        assert!(first.contains("Sistem"));
        let older = attr_values(&select(&first, "a[data-audit-older]"), "href");
        assert_eq!(older.len(), 1, "{first}");
        assert!(older[0].starts_with("/w/acme/settings/audit?before="));

        let second = request.get(&older[0]).add_cookie(cookie.clone()).await;
        assert_eq!(second.status_code(), 200);
        let second = second.text();
        let rest = select(&second, "tr[data-audit-row]");
        assert_eq!(rest.len(), 11);
        assert_css_not_exists(&second, "a[data-audit-older]");
        let ids: HashSet<String> = attr_values(&rows, "data-audit-id")
            .into_iter()
            .chain(attr_values(&rest, "data-audit-id"))
            .collect();
        assert_eq!(ids.len(), 61, "tanpa baris ganda atau terlewat");
        assert_eq!(
            attr_values(&rest, "data-action").last().map(String::as_str),
            Some("workspace.created"),
            "halaman terakhir berakhir di kejadian paling lama"
        );

        // Filter kelompok: hanya aksi role.*; link halaman berikutnya membawa filternya.
        let filtered = request
            .get(&format!("{AUDIT}?group=role"))
            .add_cookie(cookie.clone())
            .await
            .text();
        let actions = attr_values(&select(&filtered, "tr[data-audit-row]"), "data-action");
        assert_eq!(actions.len(), 30);
        assert!(actions.iter().all(|a| a == "role.created"));
        assert_css_exists(
            &filtered,
            "select[name=\"group\"] option[value=\"role\"][selected]",
        );
        assert_css_not_exists(&filtered, "a[data-audit-older]");

        let members = request
            .get(&format!("{AUDIT}?group=member"))
            .add_cookie(cookie.clone())
            .await
            .text();
        let actions = attr_values(&select(&members, "tr[data-audit-row]"), "data-action");
        assert_eq!(actions.len(), 30);
        assert!(actions.iter().all(|a| a == "member.invited"));

        // Nilai tak dikenal (filter maupun kursor) diabaikan: halaman pertama tanpa filter.
        // Kursor buatan dengan waktu di luar rentang (negatif, jauh di masa depan) atau yang
        // tidak di-encode tidak boleh sampai ke database (dulu: 500).
        let id = uuid::Uuid::now_v7().simple().to_string();
        let crafted = |payload: String| format!("?before={}", URL_SAFE_NO_PAD.encode(payload));
        let future = (chrono::Utc::now() + chrono::Duration::days(2)).timestamp_micros();
        // Sekitar 7500 SM: masih bisa dibaca chrono, tetapi di luar rentang `timestamptz` Postgres.
        let before_postgres: i64 = -300_000_000_000_000_000;
        for query in [
            "?group=hacker".to_string(),
            "?before=bukan-kursor".to_string(),
            "?before=".to_string(),
            "?group=".to_string(),
            crafted(format!("-1_{id}")),
            crafted(format!("{before_postgres}_{id}")),
            crafted(format!("{}_{id}", i64::MIN)),
            crafted(format!("{future}_{id}")),
            crafted(format!("{}_{id}", i64::MAX)),
            format!("?before={before_postgres}_{id}"),
            format!("?before={}_{id}", chrono::Utc::now().timestamp_micros()),
            "?before=%25%25%25".to_string(),
        ] {
            let res = request
                .get(&format!("{AUDIT}{query}"))
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{query}");
            assert_eq!(
                select(&res.text(), "tr[data-audit-row]").len(),
                50,
                "{query}"
            );
        }
    })
    .await;
}

#[tokio::test]
async fn audit_page_only_shows_this_workspace() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        a_workspace(&owner).slug("acme").create(&ctx).await;
        let other = a_workspace(&owner).slug("lain").create(&ctx).await;
        record_many(&ctx, &other, 3).await;
        let cookie = sign_in_as(&request, &owner).await;
        let html = request.get(AUDIT).add_cookie(cookie).await.text();
        assert_eq!(select(&html, "tr[data-audit-row]").len(), 1);
    })
    .await;
}

#[tokio::test]
async fn member_cannot_see_audit_log_gets_404() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let member = a_user().create(&ctx).await;
        let admin = a_user().create(&ctx).await;
        let auditor = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        let role = custom_role(&ctx, &workspace, "Auditor", &[Permission::ViewAuditLog]).await;
        workspace_members::ActiveModel::add(&ctx.db, workspace.workspace_id(), auditor.id, role.id)
            .await
            .unwrap();

        let cookie = sign_in_as(&request, &member).await;
        assert_eq!(
            request
                .get(AUDIT)
                .add_cookie(cookie.clone())
                .await
                .status_code(),
            404
        );
        let settings = request.get("/w/acme/settings").add_cookie(cookie).await;
        assert_eq!(settings.status_code(), 404);

        // Admin bawaan memegang ViewAuditLog: tautan di indeks Pengaturan dan halamannya.
        let cookie = sign_in_as(&request, &admin).await;
        let html = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "a[data-settings-audit][href=\"/w/acme/settings/audit\"]",
        );
        assert_eq!(
            request.get(AUDIT).add_cookie(cookie).await.status_code(),
            200
        );

        // Role custom yang hanya memegang ViewAuditLog tetap bisa membuka Pengaturan dan
        // menemukan tautannya.
        let cookie = sign_in_as(&request, &auditor).await;
        let html = request
            .get("/w/acme/settings")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "a[data-settings-audit][href=\"/w/acme/settings/audit\"]",
        );
        assert_css_not_exists(&html, "[data-settings-danger]");
        assert_eq!(
            request.get(AUDIT).add_cookie(cookie).await.status_code(),
            200
        );
    })
    .await;
}
