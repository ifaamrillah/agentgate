//! Pengaturan → Anggota (`ManageMembers`): ganti role dan jabatan, keluarkan anggota, dan
//! keluar dari workspace. Owner terakhir tidak bisa diturunkan, dikeluarkan, atau keluar
//! (`workspace-last-owner`); non-Owner tidak bisa menyentuh Owner atau memberi role di atas
//! izinnya sendiri (`member-role-not-assignable`).

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        job_titles,
        roles::{self, RoleKey},
        users, workspace_members, workspaces,
    },
    services::workspaces::{
        Acting,
        membership::{self, MembershipError},
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::ConnectionTrait;

use crate::support::{a_user, a_workspace, add_member, builtin_role, sign_in_as};

const MEMBERS: &str = "/w/acme/settings/members";

async fn role_of(
    ctx: &AppContext,
    workspace: &workspaces::Model,
    user: &users::Model,
) -> Option<RoleKey> {
    workspace_members::Model::find_with_role(&ctx.db, workspace.workspace_id(), user.id)
        .await
        .unwrap()
        .map(|(_, role)| role.role_key().unwrap().unwrap())
}

#[tokio::test]
async fn owner_can_change_role_and_job_title() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let budi = a_user().name("Budi Anggota").create(&ctx).await;
        let member = add_member(&ctx, &workspace, &budi, RoleKey::Member).await;
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let title = job_titles::ActiveModel::create(&ctx.db, wid, "QA")
            .await
            .unwrap();
        let cookie = sign_in_as(&request, &owner).await;

        let html = request.get(MEMBERS).add_cookie(cookie.clone()).await.text();
        assert_css_exists(&html, "[data-members-index]");
        assert!(html.contains("Budi Anggota") && html.contains(&budi.email));
        let row = format!("[data-member=\"{}\"]", member.id);
        assert_css_exists(
            &html,
            &format!("{row} form[action=\"{MEMBERS}/{}/role\"] select[name=\"role_id\"]", member.id),
        );
        assert_css_exists(
            &html,
            &format!(
                "{row} form[action=\"{MEMBERS}/{}/job-title\"] select[name=\"job_title_id\"] option[value=\"{}\"]",
                member.id, title.id
            ),
        );
        assert_css_exists(&html, &format!("{row} a[href=\"{MEMBERS}/{}/remove\"]", member.id));

        let res = request
            .post(&format!("{MEMBERS}/{}/role", member.id))
            .add_cookie(cookie.clone())
            .form(&[("role_id", admin_role.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), MEMBERS);
        assert_eq!(role_of(&ctx, &workspace, &budi).await, Some(RoleKey::Admin));

        let res = request
            .post(&format!("{MEMBERS}/{}/job-title", member.id))
            .add_cookie(cookie.clone())
            .form(&[("job_title_id", title.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let (updated, _) = workspace_members::Model::find_with_role(&ctx.db, wid, budi.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.job_title_id, Some(title.id));
        // Kosong = tanpa jabatan.
        let res = request
            .post(&format!("{MEMBERS}/{}/job-title", member.id))
            .add_cookie(cookie.clone())
            .form(&[("job_title_id", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        let (updated, _) = workspace_members::Model::find_with_role(&ctx.db, wid, budi.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.job_title_id, None);

        let events = crate::support::newest_audit_events(&ctx.db, wid, 3)
            .await
            .unwrap();
        let actions: Vec<_> = events
            .iter()
            .map(|e| (e.action().unwrap(), e.subject().unwrap(), e.subject_id))
            .collect();
        assert_eq!(
            actions,
            vec![
                (AuditAction::MemberJobTitleChanged, AuditSubject::Member, member.id),
                (AuditAction::MemberJobTitleChanged, AuditSubject::Member, member.id),
                (AuditAction::MemberRoleChanged, AuditSubject::Member, member.id),
            ]
        );
        assert_eq!(events[2].changes["role"]["from"], "Member");
        assert_eq!(events[2].changes["role"]["to"], "Admin");
        assert_eq!(events[2].actor_id, Some(owner.id));

        // Member dari workspace lain atau id tidak valid → 404.
        let other = a_workspace(&owner).create(&ctx).await;
        let stranger = a_user().create(&ctx).await;
        let foreign = add_member(&ctx, &other, &stranger, RoleKey::Member).await;
        for path in [
            format!("{MEMBERS}/{}/role", foreign.id),
            format!("{MEMBERS}/bukan-uuid/role"),
        ] {
            let res = request
                .post(&path)
                .add_cookie(cookie.clone())
                .form(&[("role_id", admin_role.id.to_string())])
                .await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        assert_eq!(role_of(&ctx, &other, &stranger).await, Some(RoleKey::Member));
    })
    .await;
}

#[tokio::test]
async fn last_owner_cannot_demote_or_leave() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let (owner_member, _) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
            .await
            .unwrap()
            .unwrap();
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &owner).await;

        let res = request
            .post(&format!("{MEMBERS}/{}/role", owner_member.id))
            .add_cookie(cookie.clone())
            .form(&[("role_id", admin_role.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Owner terakhir"), "{}", res.text());
        let res = request
            .post(&format!("{MEMBERS}/{}/remove", owner_member.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .post("/w/acme/leave")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Owner terakhir"));
        assert_eq!(
            role_of(&ctx, &workspace, &owner).await,
            Some(RoleKey::Owner)
        );

        // Dengan Owner kedua, Owner pertama boleh turun.
        let second = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &second, RoleKey::Owner).await;
        let res = request
            .post(&format!("{MEMBERS}/{}/role", owner_member.id))
            .add_cookie(cookie.clone())
            .form(&[("role_id", admin_role.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            role_of(&ctx, &workspace, &owner).await,
            Some(RoleKey::Admin)
        );
    })
    .await;
}

/// Dua Owner menurunkan dirinya bersamaan: kunci baris workspace membuat keduanya berurutan,
/// jadi yang kedua melihat dirinya Owner terakhir. Tanpa kunci, keduanya menghitung dua Owner
/// dan workspace kehilangan semua Owner. Diulang beberapa kali dengan pool yang sudah membuka
/// beberapa koneksi, supaya kedua transaksi benar-benar berjalan bersamaan.
#[tokio::test]
async fn concurrent_owner_demotions_keep_one_owner() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    for _ in 0..5 {
        let warm = "SELECT pg_sleep(0.02)";
        let (w1, w2) = tokio::join!(
            ctx.db.execute_unprepared(warm),
            ctx.db.execute_unprepared(warm)
        );
        w1.unwrap();
        w2.unwrap();

        let a = a_user().create(ctx).await;
        let workspace = a_workspace(&a).create(ctx).await;
        let wid = workspace.workspace_id();
        let b = a_user().create(ctx).await;
        let b_member = add_member(ctx, &workspace, &b, RoleKey::Owner).await.id;
        let (a_member, _) = workspace_members::Model::find_with_role(&ctx.db, wid, a.id)
            .await
            .unwrap()
            .unwrap();
        let owner_role = builtin_role(ctx, &workspace, RoleKey::Owner).await;
        let admin_role = builtin_role(ctx, &workspace, RoleKey::Admin).await;
        let acting = |user| Acting {
            workspace: &workspace,
            user,
            role: &owner_role,
            request_id: None,
        };
        let (acting_a, acting_b) = (acting(&a), acting(&b));

        let (ra, rb) = tokio::join!(
            membership::change_role(ctx, &acting_a, a_member.id, admin_role.id),
            membership::change_role(ctx, &acting_b, b_member, admin_role.id),
        );
        let failures: Vec<_> = [ra, rb].into_iter().filter_map(Result::err).collect();
        assert!(
            matches!(failures.as_slice(), [MembershipError::LastOwner]),
            "{failures:?}"
        );
        assert_eq!(
            workspace_members::Model::owner_count(&ctx.db, wid)
                .await
                .unwrap(),
            1
        );
    }
}

#[tokio::test]
async fn removed_member_loses_access_immediately() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let budi = a_user().create(&ctx).await;
        let member = add_member(&ctx, &workspace, &budi, RoleKey::Member).await;
        let budi_cookie = sign_in_as(&request, &budi).await;
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(budi_cookie.clone())
                .await
                .status_code(),
            200
        );

        let cookie = sign_in_as(&request, &owner).await;
        // Konfirmasi lewat halaman biasa (tanpa JS), lalu POST.
        let page = request
            .get(&format!("{MEMBERS}/{}/remove", member.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(
            &page.text(),
            &format!(
                "form[method=\"post\"][action=\"{MEMBERS}/{}/remove\"] button[type=\"submit\"]",
                member.id
            ),
        );
        let res = request
            .post(&format!("{MEMBERS}/{}/remove", member.id))
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), MEMBERS);

        // Sesi Budi masih berlaku, tetapi request berikutnya ke workspace → 404.
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(budi_cookie.clone())
                .await
                .status_code(),
            404
        );
        assert_eq!(
            request
                .get("/")
                .add_cookie(budi_cookie.clone())
                .await
                .status_code(),
            303
        );
        let event = &crate::support::newest_audit_events(&ctx.db, wid, 1)
            .await
            .unwrap()[0];
        assert_eq!(event.action().unwrap(), AuditAction::MemberRemoved);
        assert_eq!(event.subject().unwrap(), AuditSubject::Member);
        assert_eq!(event.subject_id, member.id);
        assert_eq!(event.actor_id, Some(owner.id));
        assert_eq!(event.changes["user_id"], budi.id.to_string());
    })
    .await;
}

#[tokio::test]
async fn any_member_can_leave() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let viewer = a_user().create(&ctx).await;
        let member = add_member(&ctx, &workspace, &viewer, RoleKey::Viewer).await;
        let cookie = sign_in_as(&request, &viewer).await;

        // Viewer tidak punya Pengaturan, tetapi menu profil menautkan halaman keluar.
        let html = request
            .get("/w/acme")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert_css_exists(&html, "a[href=\"/w/acme/leave\"]");
        let page = request
            .get("/w/acme/leave")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(page.status_code(), 200);
        assert_css_exists(
            &page.text(),
            "form[method=\"post\"][action=\"/w/acme/leave\"] button[type=\"submit\"]",
        );
        let res = request
            .post("/w/acme/leave")
            .add_cookie(cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/");
        assert_eq!(role_of(&ctx, &workspace, &viewer).await, None);
        assert_eq!(
            request
                .get("/w/acme")
                .add_cookie(cookie.clone())
                .await
                .status_code(),
            404
        );
        let event = &crate::support::newest_audit_events(&ctx.db, wid, 1)
            .await
            .unwrap()[0];
        assert_eq!(event.action().unwrap(), AuditAction::MemberLeft);
        assert_eq!(event.subject_id, member.id);
        assert_eq!(event.actor_id, Some(viewer.id));
    })
    .await;
}

#[tokio::test]
async fn non_owner_cannot_touch_owner_or_assign_above_own_role() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let (owner_member, _) = workspace_members::Model::find_with_role(&ctx.db, wid, owner.id)
            .await
            .unwrap()
            .unwrap();
        let admin = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &admin, RoleKey::Admin).await;
        let budi = a_user().create(&ctx).await;
        let member = add_member(&ctx, &workspace, &budi, RoleKey::Member).await;
        let owner_role = builtin_role(&ctx, &workspace, RoleKey::Owner).await;
        let viewer_role = builtin_role(&ctx, &workspace, RoleKey::Viewer).await;
        let cookie = sign_in_as(&request, &admin).await;

        // Baris Owner tanpa form ubah atau tombol keluarkan untuk Admin.
        let html = request.get(MEMBERS).add_cookie(cookie.clone()).await.text();
        let owner_row = format!("[data-member=\"{}\"]", owner_member.id);
        assert_css_exists(&html, &owner_row);
        assert_css_not_exists(&html, &format!("{owner_row} form"));
        assert_css_not_exists(&html, &format!("{owner_row} a[href$=\"/remove\"]"));

        let attempts = [
            (
                format!("{MEMBERS}/{}/role", owner_member.id),
                viewer_role.id,
            ),
            (format!("{MEMBERS}/{}/role", member.id), owner_role.id),
            (
                format!("{MEMBERS}/{}/remove", owner_member.id),
                viewer_role.id,
            ),
        ];
        for (path, role_id) in attempts {
            let res = request
                .post(&path)
                .add_cookie(cookie.clone())
                .form(&[("role_id", role_id.to_string())])
                .await;
            assert_eq!(res.status_code(), 422, "{path}");
            assert!(res.text().contains("tidak bisa memberikan"), "{path}");
        }
        assert_eq!(
            role_of(&ctx, &workspace, &owner).await,
            Some(RoleKey::Owner)
        );
        assert_eq!(
            role_of(&ctx, &workspace, &budi).await,
            Some(RoleKey::Member)
        );

        // Role custom dengan izin di luar milik Admin juga tidak bisa diberikan.
        let custom = roles::ActiveModel::create_custom(
            &ctx.db,
            wid,
            "Auditor",
            vec![agentgate_server::models::permission::Permission::ManageWorkspace],
        )
        .await
        .unwrap();
        let res = request
            .post(&format!("{MEMBERS}/{}/role", member.id))
            .add_cookie(cookie.clone())
            .form(&[("role_id", custom.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 422);
        // Role yang dipegang Admin sendiri boleh.
        let res = request
            .post(&format!("{MEMBERS}/{}/role", member.id))
            .add_cookie(cookie.clone())
            .form(&[("role_id", viewer_role.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(
            role_of(&ctx, &workspace, &budi).await,
            Some(RoleKey::Viewer)
        );
    })
    .await;
}

#[tokio::test]
async fn member_cannot_manage_members_gets_404() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let budi = a_user().create(&ctx).await;
        let target = add_member(&ctx, &workspace, &budi, RoleKey::Viewer).await;
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let member = a_user().create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &member).await;

        assert_eq!(
            request
                .get(MEMBERS)
                .add_cookie(cookie.clone())
                .await
                .status_code(),
            404
        );
        for action in ["role", "job-title", "remove"] {
            let res = request
                .post(&format!("{MEMBERS}/{}/{action}", target.id))
                .add_cookie(cookie.clone())
                .form(&[("role_id", admin_role.id.to_string())])
                .await;
            assert_eq!(res.status_code(), 404, "{action}");
        }
        assert_eq!(
            request
                .get(&format!("{MEMBERS}/{}/remove", target.id))
                .add_cookie(cookie.clone())
                .await
                .status_code(),
            404
        );
        assert_eq!(
            role_of(&ctx, &workspace, &budi).await,
            Some(RoleKey::Viewer)
        );
    })
    .await;
}

/// Role pelaku dibaca ulang di bawah kunci: pelaku yang kehilangan `ManageMembers` di tengah
/// jalan (role-nya diganti setelah `Tenant` di-resolve) ditolak seperti tanpa izin.
#[tokio::test]
async fn actor_who_lost_manage_members_is_rejected() {
    let boot = boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let owner = a_user().create(ctx).await;
    let workspace = a_workspace(&owner).create(ctx).await;
    let admin = a_user().create(ctx).await;
    let admin_member = add_member(ctx, &workspace, &admin, RoleKey::Admin).await;
    let budi = a_user().create(ctx).await;
    let target = add_member(ctx, &workspace, &budi, RoleKey::Member).await;
    let admin_role = builtin_role(ctx, &workspace, RoleKey::Admin).await;
    let viewer_role = builtin_role(ctx, &workspace, RoleKey::Viewer).await;
    // Role di DB sudah Viewer, tetapi Acting masih membawa role Admin dari awal request.
    admin_member
        .set_role(&ctx.db, viewer_role.id)
        .await
        .unwrap();
    let acting = Acting {
        workspace: &workspace,
        user: &admin,
        role: &admin_role,
        request_id: None,
    };
    let res = membership::change_role(ctx, &acting, target.id, viewer_role.id).await;
    assert!(matches!(res, Err(MembershipError::NotFound)), "{res:?}");
    let res = membership::remove(ctx, &acting, target.id).await;
    assert!(matches!(res, Err(MembershipError::NotFound)), "{res:?}");
    assert_eq!(role_of(ctx, &workspace, &budi).await, Some(RoleKey::Member));
    // Keluar tetap boleh.
    membership::leave(ctx, &acting).await.unwrap();
    assert_eq!(role_of(ctx, &workspace, &admin).await, None);
}

/// Izin role bawaan Admin, untuk role custom "wakil admin" dengan izin yang sama.
fn admin_permissions() -> Vec<agentgate_server::models::permission::Permission> {
    agentgate_server::models::permission::BUILTIN_ROLES
        .iter()
        .find(|r| r.key == RoleKey::Admin)
        .unwrap()
        .permissions
        .to_vec()
}

/// Role custom dengan semua izin Admin tidak menjadi Admin di semua project lewat role bawaan
/// Admin: menjadikan dirinya sendiri (atau anggota lain) Admin ditolak.
#[tokio::test]
async fn custom_role_with_admin_permissions_cannot_take_builtin_admin() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let co_admin =
            roles::ActiveModel::create_custom(&ctx.db, wid, "Wakil", admin_permissions())
                .await
                .unwrap();
        let wakil = a_user().create(&ctx).await;
        let own = workspace_members::ActiveModel::add(&ctx.db, wid, wakil.id, co_admin.id)
            .await
            .unwrap();
        let budi = a_user().create(&ctx).await;
        let other = add_member(&ctx, &workspace, &budi, RoleKey::Member).await;
        let admin_role = builtin_role(&ctx, &workspace, RoleKey::Admin).await;
        let cookie = sign_in_as(&request, &wakil).await;

        for member in [&own, &other] {
            let res = request
                .post(&format!("{MEMBERS}/{}/role", member.id))
                .add_cookie(cookie.clone())
                .form(&[("role_id", admin_role.id.to_string())])
                .await;
            assert_eq!(res.status_code(), 422, "{}", res.text());
            assert!(res.text().contains("tidak bisa memberikan"));
        }
        let (_, role) = workspace_members::Model::find_with_role(&ctx.db, wid, wakil.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(role.id, co_admin.id);
        assert_eq!(
            role_of(&ctx, &workspace, &budi).await,
            Some(RoleKey::Member)
        );
    })
    .await;
}
