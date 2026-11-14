//! Status sistem (M12): hanya admin instance (`settings.system.admin_emails`), selain itu 404;
//! semua panel tampil; riwayat backup dari `system_backups`.

use agentgate_server::app::App;
use loco_rs::testing::prelude::*;
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::support::{a_user, a_workspace, sign_in_as};

#[tokio::test]
async fn system_status_is_only_for_instance_admins() {
    request::<App, _, _>(|request, ctx| async move {
        // Owner workspace bukan admin instance.
        let owner = a_user().create(&ctx).await;
        a_workspace(&owner).slug("acme").create(&ctx).await;
        let cookie = sign_in_as(&request, &owner).await;
        for path in ["/system", "/system/backups", "/system/errors"] {
            let res = request.get(path).add_cookie(cookie.clone()).await;
            assert_eq!(res.status_code(), 404, "{path}");
        }
        let res = request.get("/account").add_cookie(cookie).await;
        assert!(!res.text().contains("href=\"/system\""));
        // Tanpa login → halaman masuk.
        let res = request.get("/system").await;
        assert_eq!(res.status_code(), 303);
    })
    .await;
}

#[tokio::test]
async fn instance_admin_sees_every_panel_and_backup_history() {
    request::<App, _, _>(|request, ctx| async move {
        let admin = a_user().email("sysadmin@agentgate.test").create(&ctx).await;
        let cookie = sign_in_as(&request, &admin).await;
        ctx.db
            .execute_raw(Statement::from_string(
                DbBackend::Postgres,
                "INSERT INTO system_backups (started_at, finished_at, status, size_bytes, location, message)
                 VALUES (now() - interval '2 days', now() - interval '2 days' + interval '4 minutes', 'ok', 1181116006, 's3://b/x.dump', ''),
                        (now() - interval '1 hour', now() - interval '1 hour', 'failed', NULL, '', 'S3 AccessDenied')",
            ))
            .await
            .unwrap();
        let res = request.get("/system").add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        assert!(html.contains("data-system-page=\"summary\""));
        assert!(html.contains("data-health=\"warn\""), "backup basi + gagal → peringatan");
        assert!(html.contains("data-warning=\"system-warn-backup-failed\""));
        // Menu Pengaturan memuat Status sistem untuk admin.
        assert!(html.contains("href=\"/system\""));
        for panel in ["services", "database", "backups", "errors"] {
            let res = request
                .get(&format!("/system/{panel}"))
                .add_cookie(cookie.clone())
                .await;
            assert_eq!(res.status_code(), 200, "{panel}: {}", res.text());
        }
        let html = request
            .get("/system/backups")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert!(html.contains("data-backup=\"ok\""));
        assert!(html.contains("data-backup=\"failed\""));
        assert!(html.contains("S3 AccessDenied"));
        assert!(html.contains("1,1 GB"));
        let html = request
            .get("/system/database")
            .add_cookie(cookie.clone())
            .await
            .text();
        assert!(html.contains("m20261125_000002_create_system"));
        assert_eq!(
            request
                .get("/system/tidak-ada")
                .add_cookie(cookie)
                .await
                .status_code(),
            404
        );
    })
    .await;
}
