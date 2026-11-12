//! Tabel anggota server-side (Pengaturan → Anggota) dan endpoint opsi combobox orang
//! (`GET /w/{slug}/people`): cari, filter role/tim/2FA, sort, halaman, kolom Tim; endpoint
//! hanya untuk anggota, tidak membocorkan workspace lain, dan tidak menulis.

use agentgate_server::{
    app::App,
    models::{audit_events, roles::RoleKey, users},
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel, PaginatorTrait};

use crate::support::{a_team, a_user, a_workspace, add_member, builtin_role, sign_in_as};

const MEMBERS: &str = "/w/acme/settings/members";

async fn enable_totp(ctx: &AppContext, user: &users::Model) {
    let mut active = user.clone().into_active_model();
    active.totp_enabled_at = Set(Some(chrono::Utc::now().into()));
    active.update(&ctx.db).await.unwrap();
}

fn names(html: &str) -> Vec<String> {
    select(html, "#members-table tbody tr[data-member] .app-who b")
        .iter()
        .map(|b| b.trim_start_matches("<b>").trim_end_matches("</b>").to_string())
        .collect()
}

#[tokio::test]
async fn members_table_searches_filters_sorts_and_pages() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().name("Ana Owner").create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let mut members = Vec::new();
        for i in 0..27 {
            let user = a_user()
                .name(&format!("Anggota {i:02}"))
                .email(&format!("anggota{i:02}@agentgate.test"))
                .create(&ctx)
                .await;
            add_member(&ctx, &workspace, &user, RoleKey::Member).await;
            members.push(user);
        }
        let admin = a_user().name("Zaki Admin").create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        a_team(&ctx, &workspace, "Backend", &[&members[0], &members[1]]).await;
        let cookie = sign_in_as(&request, &owner).await;
        enable_totp(&ctx, &owner).await;

        // Halaman 1: 25 baris (bawaan), total 29, urut nama; kolom Tim tampil.
        let html = request.get(MEMBERS).add_cookie(cookie.clone()).await.text();
        assert_count(&html, "#members-table tbody tr[data-member]", 25);
        assert!(html.contains("1–25 dari 29"), "{html}");
        assert_eq!(names(&html).first().map(String::as_str), Some("Ana Owner"));
        assert_css_exists(&html, "#members-table [data-member-team]");
        assert_css_exists(&html, "#members-table .app-dt-pg a[href=\"/w/acme/settings/members?page=2\"]");

        // Halaman 2: sisa 4; halaman di luar jangkauan dijepit ke halaman terakhir.
        let html = request.get(&format!("{MEMBERS}?page=2")).add_cookie(cookie.clone()).await.text();
        assert_count(&html, "#members-table tbody tr[data-member]", 4);
        let html = request.get(&format!("{MEMBERS}?page=99")).add_cookie(cookie.clone()).await.text();
        assert_count(&html, "#members-table tbody tr[data-member]", 4);

        // Sort nama menurun.
        let html = request
            .get(&format!("{MEMBERS}?sort=name&dir=desc"))
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_eq!(names(&html).first().map(String::as_str), Some("Zaki Admin"));

        // Cari (nama atau email, tanpa beda huruf besar), `%` tidak dianggap wildcard.
        let html = request.get(&format!("{MEMBERS}?q=ANGGOTA0")).add_cookie(cookie.clone()).await.text();
        assert_count(&html, "#members-table tbody tr[data-member]", 10);
        let html = request.get(&format!("{MEMBERS}?q=%25")).add_cookie(cookie.clone()).await.text();
        assert_css_exists(&html, "#members-table .app-dt-empty");

        // Filter role Admin, tim Backend, dan 2FA aktif.
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let html = request
            .get(&format!("{MEMBERS}?role={}", admin_role.id))
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_eq!(names(&html), vec!["Zaki Admin".to_string()]);
        assert_css_exists(&html, "#members-table .app-fpill");
        let team = agentgate_server::models::teams::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let html = request
            .get(&format!("{MEMBERS}?team={}", team.id))
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_eq!(names(&html), vec!["Anggota 00".to_string(), "Anggota 01".to_string()]);
        let html = request.get(&format!("{MEMBERS}?tfa=on")).add_cookie(cookie.clone()).await.text();
        assert_eq!(names(&html), vec!["Ana Owner".to_string()]);
        let html = request.get(&format!("{MEMBERS}?tfa=off&per=10")).add_cookie(cookie.clone()).await.text();
        assert_count(&html, "#members-table tbody tr[data-member]", 10);
        assert!(html.contains("1–10 dari 28"), "{html}");
    })
    .await;
}

#[tokio::test]
async fn people_options_match_query_for_members_only() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().name("Ana Owner").create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let budi = a_user().name("Budi Santoso").email("budi@agentgate.test").create(&ctx).await;
        add_member(&ctx, &workspace, &budi, RoleKey::Viewer).await;
        let citra = a_user().name("Citra Lestari").create(&ctx).await;
        add_member(&ctx, &workspace, &citra, RoleKey::Member).await;
        // Orang di workspace lain tidak boleh muncul.
        let outsider = a_user().name("Budi Lain").create(&ctx).await;
        a_workspace(&outsider).slug("lain").create(&ctx).await;
        let team = a_team(&ctx, &workspace, "QA", &[&citra]).await;

        // Viewer pun boleh mencari (opsi orang untuk form apa pun di workspace).
        let cookie = sign_in_as(&request, &budi).await;
        let audit_before = audit_events::Entity::find().count(&ctx.db).await.unwrap();
        let res = request
            .get("/w/acme/people?q=budi")
            .add_header("HX-Request", "true")
            .add_header("HX-Request-Type", "partial")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        let html = res.text();
        assert!(!html.contains("<html"), "{html}");
        assert_count(&html, "option", 1);
        assert_css_exists(&html, &format!("option[value=\"{}\"][data-sub=\"budi@agentgate.test\"]", budi.id));
        assert!(!html.contains("Budi Lain"));
        // GET tidak menulis.
        assert_eq!(audit_events::Entity::find().count(&ctx.db).await.unwrap(), audit_before);

        // not_in_team membuang anggota tim itu.
        let html = request
            .get(&format!("/w/acme/people?not_in_team={}", team.id))
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_count(&html, "option", 2);
        assert!(!html.contains("Citra Lestari"), "{html}");

        // Bukan anggota workspace → 404.
        let res = request.get("/w/lain/people?q=a").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 404);
        let cookie = sign_in_as(&request, &outsider).await;
        let res = request.get("/w/acme/people?q=a").add_cookie(cookie).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
