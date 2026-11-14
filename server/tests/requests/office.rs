//! AI Office (M10): halaman kantor (menu samping), JSON keadaan dari job/runner/tiket nyata
//! (agent bekerja di ruangan project, persona menganggur di Pantry, tiket menunggu review,
//! pemilik runner), panel detail karakter, approve/reject dari kantor kembali ke kantor, kustom
//! karakter teraudit dengan izin, dan isolasi akses project/workspace.

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        jobs, tickets,
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel};
use serde_json::Value;
use uuid::Uuid;

use super::{
    runner::running_job,
    tickets::{fixture, stages::member, ticket},
};
use crate::support::{a_user, a_workspace, add_member, newest_audit_events, sign_in_as};

async fn state(
    request: &loco_rs::TestServer,
    cookie: &loco_rs::auth::cookies::Cookie<'static>,
) -> Value {
    let res = request
        .get("/w/acme/office/state")
        .add_cookie(cookie.clone())
        .await;
    assert_eq!(res.status_code(), 200, "{}", res.text());
    assert_eq!(res.header("cache-control"), "no-store");
    serde_json::from_str(&res.text()).unwrap()
}

fn char_of<'a>(s: &'a Value, id: &str) -> &'a Value {
    s["chars"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("karakter {id} tidak ada: {s}"))
}

#[tokio::test]
async fn office_shows_working_agents_idle_personas_and_runner_owner() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, _token, _runner) = running_job(&request, &ctx, &f).await;
        let job_char = format!("j-{}", Uuid::parse_str(&job).unwrap().simple());

        let res = request
            .get("/w/acme/office")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        assert_css_exists(&html, "[data-nav='office'][aria-current='page']");
        assert_css_exists(&html, "[data-office][data-state='/w/acme/office/state']");
        assert_css_exists(&html, "[data-office-tabs] [data-office-room='KSR']");
        assert_css_exists(
            &html,
            "[data-office-panel='overview'] [data-office-runners]",
        );
        assert!(html.contains("<span data-k=\"st-working\">"));
        // R5: panggung SVG (bukan kanvas), sakelar Isometrik/Denah, mini-map SVG, kunci mode per
        // user, dan ikon badge status tersedia untuk label.
        assert_css_exists(
            &html,
            "[data-office-stage] svg[data-office-svg][aria-hidden='true']",
        );
        assert_css_exists(&html, "[data-office-mode='iso'][aria-pressed='true']");
        assert_css_exists(&html, "[data-office-mode='plan'][aria-pressed='false']");
        assert_css_exists(&html, "svg[data-office-mm]");
        assert!(!html.contains("<canvas"), "kanvas lama sudah dihapus");
        assert_css_exists(&html, &format!("[data-office][data-me='{}']", f.owner.id));
        for icon in [
            "eye",
            "alert-triangle",
            "player-pause",
            "terminal-2",
            "coffee",
        ] {
            assert!(html.contains(&format!("data-i=\"{icon}\"")), "ikon {icon}");
        }

        let s = state(&request, &f.cookie).await;
        let dev = char_of(&s, &job_char);
        assert_eq!(dev["bot"], true);
        assert_eq!(dev["key"], "dev");
        assert_eq!(dev["st"], "working");
        assert_eq!(dev["room"], "KSR");
        assert_eq!(dev["tk"], "KSR-1");
        assert_eq!(
            (dev["step"].as_u64(), dev["steps"].as_u64()),
            (Some(2), Some(2))
        );
        assert_eq!(dev["look"]["acc"], "headset");
        // Persona lain menganggur di Pantry; pemilik runner hadir sebagai manusia.
        let qa = char_of(&s, "p-qa");
        assert_eq!(
            (qa["st"].as_str(), qa["room"].as_str()),
            (Some("idle"), Some("PANTRY"))
        );
        let owner = char_of(&s, &format!("u-{}", f.owner.id.simple()));
        assert_eq!(owner["bot"], false);
        assert_eq!(owner["room"], "KSR");
        assert_eq!(s["counts"]["working"], 1);
        let kinds: Vec<&str> = s["rooms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["kind"].as_str().unwrap())
            .collect();
        assert!(
            kinds.contains(&"project") && kinds.contains(&"meeting") && kinds.contains(&"pantry")
        );

        // Panel detail karakter: log job, progres, tautan job; karakter tak dikenal → 404.
        let res = request
            .get(&format!("/w/acme/office/panel?c={job_char}"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let html = res.text();
        assert_css_exists(
            &html,
            &format!("[data-office-char='{job_char}'] [data-office-detail='working']"),
        );
        assert_css_exists(&html, "[data-office-live][hx-get] [data-office-log]");
        assert_css_exists(
            &html,
            &format!("svg.app-of-pv[data-office-pv='{job_char}']"),
        );
        assert!(html.contains("ag_context KSR-1"));
        let res = request
            .get("/w/acme/office/panel?c=j-nope")
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .get(&format!("/w/acme/office?c={job_char}"))
            .add_cookie(f.cookie.clone())
            .await;
        assert_css_exists(
            &res.text(),
            &format!("[data-office][data-selected='{job_char}']"),
        );
    })
    .await;
}

#[tokio::test]
async fn review_ticket_waits_in_its_room_and_reject_returns_to_office() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, _token, _runner) = running_job(&request, &ctx, &f).await;
        // Job selesai, stage diajukan review (jalur runner diuji di runner.rs).
        let mut j = jobs::Entity::find_by_id(Uuid::parse_str(&job).unwrap())
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .into_active_model();
        j.status = Set("succeeded".into());
        j.summary = Set("Checkout QRIS selesai\nDetail".into());
        j.update(&ctx.db).await.unwrap();
        let t = ticket(&ctx, &f.workspace, "KSR-1").await;
        let mut am: tickets::ActiveModel = t.clone().into_active_model();
        am.stage_state = Set("review".into());
        am.update(&ctx.db).await.unwrap();

        let id = format!("t-{}", t.id.simple());
        let s = state(&request, &f.cookie).await;
        let c = char_of(&s, &id);
        assert_eq!(
            (c["st"].as_str(), c["room"].as_str()),
            (Some("review"), Some("KSR"))
        );
        assert_eq!(c["doing"], "Checkout QRIS selesai");
        assert_eq!(c["act"], true);
        assert_eq!(s["counts"]["review"], 1);

        let html = request
            .get("/w/acme/office")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            &format!("[data-office-todo] [data-office-sel='{id}']"),
        );
        let html = request
            .get(&format!("/w/acme/office/panel?c={id}"))
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "form[data-office-approve][action='/w/acme/t/KSR-1/stage/approve']",
        );
        assert_css_exists(&html, "form[data-office-reject] textarea[name='note']");

        let res = request
            .post("/w/acme/t/KSR-1/stage/reject")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("return_to", format!("/w/acme/office?c={id}").as_str()),
                ("note", "Tombol bayar belum sesuai desain"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), format!("/w/acme/office?c={id}"));
        let s = state(&request, &f.cookie).await;
        assert!(
            s["chars"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["id"] != id.as_str())
        );
    })
    .await;
}

#[tokio::test]
async fn looks_are_validated_permission_checked_and_audited() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let res = request
            .post("/w/acme/office/looks")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("subject", "persona"),
                ("key", "dev"),
                ("alias", "Dodi"),
                ("shirt", "#0d9488"),
                ("acc", "cap"),
                ("c", "p-dev"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert_eq!(res.header("location"), "/w/acme/office?c=p-dev");
        let s = state(&request, &f.cookie).await;
        let dev = char_of(&s, "p-dev");
        assert_eq!(
            (
                dev["short"].as_str(),
                dev["look"]["shirt"].as_str(),
                dev["look"]["acc"].as_str()
            ),
            (Some("Dodi"), Some("#0d9488"), Some("cap"))
        );
        let events = newest_audit_events(&ctx.db, wid, 5).await.unwrap();
        let e = events
            .iter()
            .find(|e| e.action().unwrap() == AuditAction::OfficeLookUpdated)
            .expect("kustom karakter teraudit");
        assert_eq!(e.subject().unwrap(), AuditSubject::Workspace);

        // Tanpa perubahan → tidak diaudit lagi.
        request
            .post("/w/acme/office/looks")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("subject", "persona"),
                ("key", "dev"),
                ("alias", "Dodi"),
                ("shirt", "#0d9488"),
                ("acc", "cap"),
            ])
            .await;
        let n = newest_audit_events(&ctx.db, wid, 10)
            .await
            .unwrap()
            .iter()
            .filter(|e| e.action().unwrap() == AuditAction::OfficeLookUpdated)
            .count();
        assert_eq!(n, 1);

        // Warna tidak valid → flash error, data tidak berubah.
        let res = request
            .post("/w/acme/office/looks")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("subject", "persona"),
                ("key", "dev"),
                ("shirt", "red"),
                ("acc", "cap"),
            ])
            .await;
        assert_eq!(res.status_code(), 303);
        let html = request
            .get("/w/acme/office")
            .add_cookie(f.cookie.clone())
            .await
            .text();
        assert!(html.contains("Tampilan karakter tidak valid"), "{html}");
        // Persona tak dikenal → 404.
        let res = request
            .post("/w/acme/office/looks")
            .add_cookie(f.cookie.clone())
            .form(&[
                ("subject", "persona"),
                ("key", "nope"),
                ("shirt", "#0d9488"),
            ])
            .await;
        assert_eq!(res.status_code(), 404);

        // Member: tidak boleh mengubah persona, boleh mengubah dirinya sendiri, bukan orang lain.
        let (mid, mc) = member(&request, &ctx, &f, "Mira Member").await;
        let res = request
            .post("/w/acme/office/looks")
            .add_cookie(mc.clone())
            .form(&[("subject", "persona"), ("key", "dev"), ("shirt", "#123456")])
            .await;
        assert_eq!(res.status_code(), 404);
        let me = mid.to_string();
        let res = request
            .post("/w/acme/office/looks")
            .add_cookie(mc.clone())
            .form(&[
                ("subject", "user"),
                ("key", me.as_str()),
                ("hair", "#111111"),
                ("style", "hijab"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let owner = f.owner.id.to_string();
        let res = request
            .post("/w/acme/office/looks")
            .add_cookie(mc.clone())
            .form(&[
                ("subject", "user"),
                ("key", owner.as_str()),
                ("hair", "#111111"),
            ])
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
async fn office_is_scoped_to_accessible_projects_and_workspace() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let (job, _token, _runner) = running_job(&request, &ctx, &f).await;
        let job_char = format!("j-{}", Uuid::parse_str(&job).unwrap().simple());

        // Anggota tanpa akses project KSR: kantor tanpa ruangan KSR dan tanpa job-nya.
        let outsider = a_user().name("Nina Nogrant").create(&ctx).await;
        add_member(
            &ctx,
            &f.workspace,
            &outsider,
            agentgate_server::models::roles::RoleKey::Member,
        )
        .await;
        let oc = sign_in_as(&request, &outsider).await;
        let s = state(&request, &oc).await;
        assert!(
            s["rooms"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["k"] != "KSR"),
            "{s}"
        );
        assert!(
            s["chars"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["id"] != job_char.as_str())
        );
        let res = request
            .get(&format!("/w/acme/office/panel?c={job_char}"))
            .add_cookie(oc.clone())
            .await;
        assert_eq!(res.status_code(), 404);

        // Pengguna workspace lain → 404.
        let stranger = a_user().name("Sam Stranger").create(&ctx).await;
        a_workspace(&stranger).slug("other").create(&ctx).await;
        let sc = sign_in_as(&request, &stranger).await;
        for url in [
            "/w/acme/office",
            "/w/acme/office/state",
            "/w/acme/office/panel",
        ] {
            let res = request.get(url).add_cookie(sc.clone()).await;
            assert_eq!(res.status_code(), 404, "{url}");
        }
    })
    .await;
}
