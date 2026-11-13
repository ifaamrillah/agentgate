//! Pengaturan project → Components: buat (validasi, nama unik), ubah pemilik + modul, hapus,
//! aturan pemilik jadi penerima default, dan akses hanya Admin project.

use agentgate_server::{
    app::App,
    models::{components, projects, roles::RoleKey},
    services::planning::components::PlanningRules,
};
use loco_rs::testing::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::{fixture, ksr};
use crate::support::{a_user, add_member, sign_in_as};

#[tokio::test]
async fn admin_manages_components_and_rules() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let base = "/w/acme/p/KSR/settings/components";

        let page = request.get(base).add_cookie(f.cookie.clone()).await;
        assert_eq!(page.status_code(), 200);
        let page = page.text();
        assert_css_exists(&page, "[data-components='list']");
        assert_css_exists(&page, "#component-new form");
        assert_css_exists(&page, "[data-nav='components'][aria-current]");

        // Nama kosong → 422 dialog terbuka.
        let res = request.post(base).add_cookie(f.cookie.clone()).form(&[("name", "")]).await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), "dialog#component-new[data-open-on-load]");

        let owner = f.owner.id.to_string();
        let res = request
            .post(base)
            .add_cookie(f.cookie.clone())
            .form(&[
                ("name", "Pembayaran"),
                ("description", "QRIS & <i>tunai</i>"),
                ("owner", owner.as_str()),
                ("modules", "kasir-api/src/payment"),
                ("modules", "kasir-web/src/pay"),
            ])
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let c = components::Entity::find()
            .filter(components::Column::WorkspaceId.eq(wid.get()))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(c.owner_id, Some(f.owner.id));
        assert_eq!(c.modules, vec!["kasir-api/src/payment", "kasir-web/src/pay"]);
        // Nama sama (beda huruf) → 422.
        let res = request.post(base).add_cookie(f.cookie.clone()).form(&[("name", "pembayaran")]).await;
        assert_eq!(res.status_code(), 422);

        let page = request.get(base).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "tr[data-component='Pembayaran']");
        assert!(!page.contains("<i>tunai</i>"));
        assert_css_exists(&page, &format!("#component-edit-{} form", c.id));
        let page = request.get(&format!("{base}?graph=unlinked")).add_cookie(f.cookie.clone()).await.text();
        assert_css_not_exists(&page, "tr[data-component='Pembayaran']");

        // Pemilik bukan anggota → 422 di dialog ubah.
        let stranger = a_user().create(&ctx).await;
        let res = request
            .post(&format!("{base}/{}", c.id))
            .add_cookie(f.cookie.clone())
            .form(&[("name", "Pembayaran"), ("owner", stranger.id.to_string().as_str())])
            .await;
        assert_eq!(res.status_code(), 422);
        assert_css_exists(&res.text(), &format!("dialog#component-edit-{}[data-open-on-load]", c.id));
        let res = request
            .post(&format!("{base}/{}", c.id))
            .add_cookie(f.cookie.clone())
            .form(&[("name", "Bayar"), ("owner", "")])
            .await;
        assert_eq!(res.status_code(), 303);
        let c = components::Entity::find_by_id(c.id).one(&ctx.db).await.unwrap().unwrap();
        assert_eq!(c.name, "Bayar");
        assert!(c.owner_id.is_none() && c.modules.is_empty());

        // Aturan: matikan pemilik jadi penerima default.
        let page = request.get(&format!("{base}/rules")).add_cookie(f.cookie.clone()).await.text();
        assert_css_exists(&page, "[data-component-rules] input[name=component_assignee][checked]");
        let res = request.post(&format!("{base}/rules")).add_cookie(f.cookie.clone()).form(&[("x", "")]).await;
        assert_eq!(res.status_code(), 303);
        let p: projects::Model = ksr(&ctx, &f).await;
        assert!(!PlanningRules::of(&p).component_assignee);

        // Hapus.
        let res = request.post(&format!("{base}/{}/delete", c.id)).add_cookie(f.cookie.clone()).form(&[("x", "")]).await;
        assert_eq!(res.status_code(), 303);
        assert!(components::Entity::find_by_id(c.id).one(&ctx.db).await.unwrap().is_none());

        // Anggota biasa tanpa admin project → 404.
        let member = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &member, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &member).await;
        let res = request.get(base).add_cookie(cookie.clone()).await;
        assert_eq!(res.status_code(), 404);
        let res = request.post(base).add_cookie(cookie).form(&[("name", "X")]).await;
        assert_eq!(res.status_code(), 404);
    })
    .await;
}
