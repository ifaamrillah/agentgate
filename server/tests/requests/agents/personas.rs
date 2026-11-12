//! `/w/{slug}/settings/personas`: daftar, buat (kosong/clone), ubah per bagian, hapus dengan
//! pengganti, izin (`ManageTicketSettings`), audit.

use agentgate_server::{
    app::App,
    models::{
        audit_events::{AuditAction, AuditSubject},
        personas,
        roles::RoleKey,
    },
};
use loco_rs::testing::prelude::*;

use crate::support::{a_user, a_workspace, add_member, newest_audit_events, sign_in_as};

async fn persona(
    ctx: &loco_rs::app::AppContext,
    wid: agentgate_server::models::workspaces::WorkspaceId,
    key: &str,
) -> personas::Model {
    personas::Model::find_active(&ctx.db, wid, key)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("persona {key}"))
}

#[tokio::test]
async fn personas_are_listed_created_cloned_and_edited() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let c = sign_in_as(&request, &owner).await;

        let page = request
            .get("/w/acme/settings/personas")
            .add_cookie(c.clone())
            .await;
        assert_eq!(page.status_code(), 200, "{}", page.text());
        let html = page.text();
        assert_css_exists(&html, ".app-side a[href=\"/w/acme/settings/personas\"]");
        assert_css_exists(&html, "[data-persona-builtin] [data-persona='dev']");
        assert_css_exists(
            &html,
            "dialog#persona-new form[action=\"/w/acme/settings/personas\"]",
        );

        // Nama kosong → 422, dialog terbuka.
        let res = request
            .post("/w/acme/settings/personas")
            .add_cookie(c.clone())
            .form(&[("name", ""), ("summary", ""), ("source", "")])
            .await;
        assert_eq!(res.status_code(), 422);

        // Clone Code Reviewer.
        let reviewer = persona(&ctx, wid, "reviewer").await;
        let res = request
            .post("/w/acme/settings/personas")
            .add_cookie(c.clone())
            .form(&[
                ("name", "Security Auditor"),
                ("summary", "Audit keamanan sebelum rilis"),
                ("source", &reviewer.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let sec = persona(&ctx, wid, "security-auditor").await;
        assert!(!sec.builtin);
        assert_eq!(sec.instructions, reviewer.instructions);
        assert_eq!(sec.mcp_tools, reviewer.mcp_tools);
        assert_eq!(sec.color, reviewer.color);
        let href = format!("/w/acme/settings/personas/{}", sec.id);
        assert_eq!(res.header("location").to_str().unwrap(), href);

        // Nama sama (beda huruf) → 422.
        let res = request
            .post("/w/acme/settings/personas")
            .add_cookie(c.clone())
            .form(&[
                ("name", "security auditor"),
                ("summary", ""),
                ("source", ""),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        assert!(res.text().contains("Nama persona sudah dipakai"));

        // Setiap bagian halaman edit tampil.
        for section in [
            "identity",
            "instructions",
            "skills",
            "tools",
            "models",
            "danger",
        ] {
            let page = request
                .get(&format!("{href}?section={section}"))
                .add_cookie(c.clone())
                .await;
            assert_eq!(page.status_code(), 200, "{section}: {}", page.text());
            assert_css_exists(&page.text(), &format!("[data-persona-{section}]"));
        }

        // Identitas: ikon tidak dikenal → 422.
        let res = request
            .post(&format!("{href}/identity"))
            .add_cookie(c.clone())
            .form(&[
                ("name", "Security Auditor"),
                ("summary", "x"),
                ("color", "#2563eb"),
                ("icon", "bogus"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .post(&format!("{href}/identity"))
            .add_cookie(c.clone())
            .form(&[
                ("name", "Sec Auditor"),
                ("summary", "Audit"),
                ("color", "#2563eb"),
                ("icon", "shield-lock"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());

        // Instruksi + gaya ringkas.
        let res = request
            .post(&format!("{href}/instructions"))
            .add_cookie(c.clone())
            .form(&[
                ("instructions", "# Peran\r\nAudit OWASP."),
                ("concise", "on"),
            ])
            .await;
        assert_eq!(res.status_code(), 303);

        // Tools: tool tak dikenal → 422; valid → tersimpan urut katalog.
        let res = request
            .post(&format!("{href}/tools"))
            .add_cookie(c.clone())
            .form(&[("mcp_tools", "rm_rf")])
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .post(&format!("{href}/tools"))
            .add_cookie(c.clone())
            .form(&[
                ("mcp_tools", "ag_comment"),
                ("mcp_tools", "ag_context"),
                ("client_tools", "read"),
                ("danger_deploy", "block"),
                ("danger_network", "allow"),
            ])
            .await;
        assert_eq!(res.status_code(), 303);

        // Model per client: model bebas, effort harus dikenal client.
        let res = request
            .post(&format!("{href}/models"))
            .add_cookie(c.clone())
            .form(&[
                ("model_claude", "claude-opus-4-5"),
                ("effort_claude", "turbo"),
            ])
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .post(&format!("{href}/models"))
            .add_cookie(c.clone())
            .form(&[
                ("model_claude", "claude-opus-4-5"),
                ("effort_claude", "high"),
                ("model_cursor", "composer-2"),
            ])
            .await;
        assert_eq!(res.status_code(), 303);

        let sec = persona(&ctx, wid, "security-auditor").await;
        assert_eq!(sec.name, "Sec Auditor");
        assert_eq!(sec.instructions, "# Peran\nAudit OWASP.");
        assert!(sec.concise);
        assert_eq!(sec.mcp_tools, vec!["ag_context", "ag_comment"]);
        assert_eq!(sec.client_tools, vec!["read"]);
        assert_eq!(sec.danger["deploy"], "block");
        assert_eq!(sec.danger["network"], "allow");
        assert_eq!(sec.model_for("claude").unwrap().effort, "high");
        assert_eq!(sec.model_for("cursor").unwrap().model, "composer-2");
        assert!(sec.revision >= 5);

        // Kartu menampilkan model claude.
        let html = request
            .get("/w/acme/settings/personas")
            .add_cookie(c.clone())
            .await
            .text();
        assert_css_exists(
            &html,
            "[data-persona-custom] [data-persona='security-auditor']",
        );
        assert!(html.contains("claude-opus-4-5"));

        let events = newest_audit_events(&ctx.db, wid, 10).await.unwrap();
        let updated = events
            .iter()
            .filter(|e| e.action().unwrap() == AuditAction::PersonaUpdated)
            .count();
        assert_eq!(updated, 4);
        let created = events
            .iter()
            .find(|e| e.action().unwrap() == AuditAction::PersonaCreated)
            .unwrap();
        assert_eq!(created.subject().unwrap(), AuditSubject::Persona);
        assert_eq!(created.changes["source"], "reviewer");
    })
    .await;
}

#[tokio::test]
async fn deleted_persona_points_stages_to_its_replacement() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let wid = workspace.workspace_id();
        let c = sign_in_as(&request, &owner).await;
        let analyst = persona(&ctx, wid, "analyst").await;
        let pm = persona(&ctx, wid, "pm").await;
        let href = format!("/w/acme/settings/personas/{}", analyst.id);

        // Tanpa pengganti / pengganti dirinya sendiri → 422.
        for r in ["", &analyst.id.to_string()] {
            let res = request
                .post(&format!("{href}/delete"))
                .add_cookie(c.clone())
                .form(&[("replacement", r)])
                .await;
            assert_eq!(res.status_code(), 422);
        }

        let res = request
            .post(&format!("{href}/delete"))
            .add_cookie(c.clone())
            .form(&[("replacement", pm.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        assert!(
            personas::Model::find_active(&ctx.db, wid, "analyst")
                .await
                .unwrap()
                .is_none()
        );
        let labels = personas::Model::labels(&ctx.db, wid).await.unwrap();
        assert_eq!(labels["analyst"].key, "pm");
        // Kunci lama tetap dikenal (versi workflow lama), halaman edit persona terhapus 404.
        assert!(
            personas::Model::known_keys(&ctx.db, wid)
                .await
                .unwrap()
                .contains(&"analyst".to_string())
        );
        assert_eq!(
            request.get(&href).add_cookie(c.clone()).await.status_code(),
            404
        );

        // PM dihapus juga → stage "analyst" mengikuti rantai ke pengganti PM.
        let dev = persona(&ctx, wid, "dev").await;
        let res = request
            .post(&format!("/w/acme/settings/personas/{}/delete", pm.id))
            .add_cookie(c.clone())
            .form(&[("replacement", dev.id.to_string())])
            .await;
        assert_eq!(res.status_code(), 303);
        let labels = personas::Model::labels(&ctx.db, wid).await.unwrap();
        assert_eq!(labels["analyst"].key, "dev");
        assert_eq!(labels["pm"].key, "dev");

        let event = &newest_audit_events(&ctx.db, wid, 1).await.unwrap()[0];
        assert_eq!(event.action().unwrap(), AuditAction::PersonaDeleted);
        assert_eq!(event.changes["replacement"], "Developer");
    })
    .await;
}

#[tokio::test]
async fn personas_need_ticket_settings_permission_and_stay_in_workspace() {
    request::<App, _, _>(|request, ctx| async move {
        let owner = a_user().create(&ctx).await;
        let workspace = a_workspace(&owner).slug("acme").create(&ctx).await;
        let member = a_user().email("member@example.com").create(&ctx).await;
        add_member(&ctx, &workspace, &member, RoleKey::Member).await;
        let other_owner = a_user().email("other@example.com").create(&ctx).await;
        let other = a_workspace(&other_owner).slug("other").create(&ctx).await;
        let foreign = persona(&ctx, other.workspace_id(), "dev").await;

        let m = sign_in_as(&request, &member).await;
        assert_eq!(
            request
                .get("/w/acme/settings/personas")
                .add_cookie(m.clone())
                .await
                .status_code(),
            404
        );
        let res = request
            .post("/w/acme/settings/personas")
            .add_cookie(m.clone())
            .form(&[("name", "X"), ("summary", ""), ("source", "")])
            .await;
        assert_eq!(res.status_code(), 404);

        let o = sign_in_as(&request, &owner).await;
        let res = request
            .get(&format!("/w/acme/settings/personas/{}", foreign.id))
            .add_cookie(o.clone())
            .await;
        assert_eq!(res.status_code(), 404);
        let res = request
            .post(&format!(
                "/w/acme/settings/personas/{}/instructions",
                foreign.id
            ))
            .add_cookie(o.clone())
            .form(&[("instructions", "x")])
            .await;
        assert_eq!(res.status_code(), 404);
        // Clone dari persona workspace lain → 404.
        let res = request
            .post("/w/acme/settings/personas")
            .add_cookie(o.clone())
            .form(&[
                ("name", "X"),
                ("summary", ""),
                ("source", &foreign.id.to_string()),
            ])
            .await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
