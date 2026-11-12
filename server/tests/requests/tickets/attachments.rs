//! Lampiran tiket ke storage (memori di tes; S3/SeaweedFS di dev): unggah, unduh sebagai
//! attachment, batas ukuran, hapus, dan akses.

use agentgate_server::{
    app::App,
    models::{roles::RoleKey, ticket_attachments},
};
use axum_test::multipart::{MultipartForm, Part};
use loco_rs::testing::prelude::*;

use super::{create_ticket, fixture, ticket};
use crate::support::{a_user, add_member, sign_in_as};

fn file(bytes: Vec<u8>, name: &str, mime: &str) -> MultipartForm {
    MultipartForm::new().add_part("file", Part::bytes(bytes).file_name(name).mime_type(mime))
}

#[tokio::test]
async fn upload_download_and_remove_attachment() {
    request::<App, _, _>(|request, ctx| async move {
        let f = fixture(&request, &ctx).await;
        let wid = f.workspace.workspace_id();
        let code = create_ticket(&request, &f.cookie, &[("title", "Ada berkas")]).await;
        let t = ticket(&ctx, &f.workspace, &code).await;

        let res = request
            .post(&format!("/w/acme/t/{code}/attachments"))
            .add_cookie(f.cookie.clone())
            .multipart(file(
                b"halo dunia".to_vec(),
                "../catatan rapat.txt",
                "text/plain",
            ))
            .await;
        assert_eq!(res.status_code(), 303, "{}", res.text());
        let list = ticket_attachments::Model::of_ticket(&ctx.db, wid, t.id)
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        let a = &list[0];
        assert_eq!(a.filename, "catatan rapat.txt");
        assert!(a.storage_key.starts_with(&format!("tickets/{}/", t.id)));
        let stored: Vec<u8> = ctx
            .storage
            .download(std::path::Path::new(&a.storage_key))
            .await
            .unwrap();
        assert_eq!(stored, b"halo dunia");

        let res = request
            .get(&format!("/w/acme/t/{code}/attachments/{}", a.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 200);
        assert_eq!(res.as_bytes().as_ref(), b"halo dunia");
        assert!(
            res.header("content-disposition")
                .to_str()
                .unwrap()
                .starts_with("attachment")
        );
        assert_eq!(res.header("x-content-type-options"), "nosniff");

        // Anggota tanpa akses project: 404.
        let other = a_user().create(&ctx).await;
        add_member(&ctx, &f.workspace, &other, RoleKey::Member).await;
        let cookie = sign_in_as(&request, &other).await;
        let res = request
            .get(&format!("/w/acme/t/{code}/attachments/{}", a.id))
            .add_cookie(cookie)
            .await;
        assert_eq!(res.status_code(), 404);

        // Terlalu besar dan kosong → 422.
        let res = request
            .post(&format!("/w/acme/t/{code}/attachments"))
            .add_cookie(f.cookie.clone())
            .multipart(file(
                vec![0u8; ticket_attachments::MAX_BYTES + 1],
                "besar.bin",
                "application/octet-stream",
            ))
            .await;
        assert_eq!(res.status_code(), 422);
        let res = request
            .post(&format!("/w/acme/t/{code}/attachments"))
            .add_cookie(f.cookie.clone())
            .multipart(file(Vec::new(), "kosong.txt", "text/plain"))
            .await;
        assert_eq!(res.status_code(), 422);

        let res = request
            .post(&format!("/w/acme/t/{code}/attachments/{}/delete", a.id))
            .add_cookie(f.cookie.clone())
            .await;
        assert_eq!(res.status_code(), 303);
        assert!(
            ticket_attachments::Model::of_ticket(&ctx.db, wid, t.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            ctx.storage
                .download::<Vec<u8>>(std::path::Path::new(&a.storage_key))
                .await
                .is_err()
        );
    })
    .await;
}
