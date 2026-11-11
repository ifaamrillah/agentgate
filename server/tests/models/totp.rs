//! `services::auth::totp`: secret terenkripsi, kode sekali pakai per langkah, kode pemulihan
//! sebagai digest, dan matikan 2FA.

use agentgate_server::{
    app::App,
    common::{crypto, settings::Settings},
    models::{recovery_codes, users},
    services::auth::{
        sign_in,
        totp::{self, Confirmed},
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use secrecy::SecretString;

use crate::support::{TEST_PASSWORD, TEST_TOTP_SECRET, a_user};

fn now() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp()).unwrap()
}

async fn reload(ctx: &loco_rs::app::AppContext, user: &users::Model) -> users::Model {
    users::Model::find_by_id(&ctx.db, user.id)
        .await
        .unwrap()
        .unwrap()
}

async fn digests(ctx: &loco_rs::app::AppContext, user: &users::Model) -> Vec<String> {
    recovery_codes::Entity::find()
        .filter(recovery_codes::Column::UserId.eq(user.id))
        .all(&ctx.db)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.code_digest)
        .collect()
}

#[tokio::test]
async fn totp_secret_is_stored_encrypted() {
    request::<App, _, _>(|_request, ctx| async move {
        let settings = Settings::from_ctx(&ctx).unwrap();
        let user = a_user().create(&ctx).await;
        let setup = totp::begin_setup(&settings, &user, None).unwrap();
        assert_eq!(setup.secret_base32.len(), 32, "160 bit secret");
        assert!(setup.otpauth_uri.starts_with("otpauth://totp/"));
        assert!(setup.qr_svg.starts_with("<svg"));
        // Secret pending di sesi juga terenkripsi.
        assert!(!setup.sealed.contains(&setup.secret_base32));

        let code = totp::code_at(&setup.secret_base32, now()).unwrap();
        let Confirmed::Enabled(codes) = totp::confirm(&ctx, &user, &setup.sealed, &code)
            .await
            .unwrap()
        else {
            panic!("kode benar mengaktifkan 2FA");
        };
        assert_eq!(codes.len(), 10);
        // Konfirmasi lagi setelah aktif: tidak mengubah apa pun.
        assert_eq!(
            totp::confirm(&ctx, &user, &setup.sealed, &code)
                .await
                .unwrap(),
            Confirmed::AlreadyEnabled
        );

        let user = reload(&ctx, &user).await;
        assert!(user.totp_enabled_at.is_some());
        assert!(
            user.totp_last_step.is_some(),
            "kode konfirmasi tidak bisa dipakai lagi"
        );
        let stored = user.totp_secret_encrypted.unwrap();
        assert_ne!(stored, setup.secret_base32.as_bytes());
        assert!(
            !stored
                .windows(setup.secret_base32.len())
                .any(|w| w == setup.secret_base32.as_bytes())
        );
        let key = settings.auth.encryption_key_bytes().unwrap();
        assert_eq!(
            crypto::decrypt(&key, &stored).unwrap(),
            setup.secret_base32.as_bytes()
        );
    })
    .await;
}

#[tokio::test]
async fn begin_setup_reuses_the_pending_secret_from_the_session() {
    request::<App, _, _>(|_request, ctx| async move {
        let settings = Settings::from_ctx(&ctx).unwrap();
        let user = a_user().create(&ctx).await;
        let first = totp::begin_setup(&settings, &user, None).unwrap();
        let again = totp::begin_setup(&settings, &user, Some(&first.sealed)).unwrap();
        assert_eq!(again.secret_base32, first.secret_base32);
        // Nilai sesi yang sama persis: halaman setup tidak perlu menulis sesi lagi.
        assert_eq!(again.sealed, first.sealed);
        // Isi sesi yang rusak: secret baru, bukan error.
        let fresh = totp::begin_setup(&settings, &user, Some("rusak")).unwrap();
        assert_ne!(fresh.secret_base32, first.secret_base32);
        assert_ne!(fresh.sealed, "rusak");
        // Secret pengganti itu (yang disimpan controller ke sesi) bisa dikonfirmasi.
        let code = totp::code_at(&fresh.secret_base32, now()).unwrap();
        assert_eq!(
            totp::confirm(&ctx, &user, "rusak", &code).await.unwrap(),
            Confirmed::WrongCode
        );
        assert!(matches!(
            totp::confirm(&ctx, &user, &fresh.sealed, &code)
                .await
                .unwrap(),
            Confirmed::Enabled(_)
        ));
    })
    .await;
}

#[tokio::test]
async fn confirm_with_a_wrong_code_enables_nothing() {
    request::<App, _, _>(|_request, ctx| async move {
        let settings = Settings::from_ctx(&ctx).unwrap();
        let user = a_user().create(&ctx).await;
        let setup = totp::begin_setup(&settings, &user, None).unwrap();
        let result = totp::confirm(&ctx, &user, &setup.sealed, "000000")
            .await
            .unwrap();
        // 1 banding 10^6 kode acak ini benar; cukup untuk tes.
        assert_eq!(result, Confirmed::WrongCode);
        let user = reload(&ctx, &user).await;
        assert!(user.totp_enabled_at.is_none());
        assert!(user.totp_secret_encrypted.is_none());
    })
    .await;
}

#[tokio::test]
async fn totp_code_cannot_be_replayed_in_same_step() {
    request::<App, _, _>(|_request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let t = now();
        let code = totp::code_at(TEST_TOTP_SECRET, t).unwrap();
        assert!(totp::verify(&ctx, &user, &code).await.unwrap());
        let user = reload(&ctx, &user).await;
        assert!(!totp::verify(&ctx, &user, &code).await.unwrap(), "replay");
        // Kode langkah sebelumnya masih dalam toleransi ±1, tetapi lebih tua dari yang
        // diterima.
        let previous = totp::code_at(TEST_TOTP_SECRET, t - 30).unwrap();
        assert!(!totp::verify(&ctx, &user, &previous).await.unwrap());
        // Kode di luar toleransi ditolak.
        let old = totp::code_at(TEST_TOTP_SECRET, t - 120).unwrap();
        assert!(!totp::verify(&ctx, &user, &old).await.unwrap());
    })
    .await;
}

#[tokio::test]
async fn recovery_codes_are_stored_as_digests_and_work_once() {
    request::<App, _, _>(|_request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let codes = totp::regenerate_recovery_codes(&ctx, &user)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(codes.len(), 10);
        for code in &codes {
            let (a, b) = code.split_once('-').unwrap();
            assert_eq!((a.len(), b.len()), (5, 5), "{code}");
            assert!(
                code.chars()
                    .filter(|c| *c != '-')
                    .all(|c| "abcdefghjkmnpqrstuvwxyz23456789".contains(c)),
                "{code}"
            );
        }
        let stored = digests(&ctx, &user).await;
        assert_eq!(stored.len(), 10);
        let normalized = codes[0].replace('-', "");
        assert!(stored.contains(&crypto::sha256_hex(&normalized)));
        assert!(!stored.iter().any(|d| codes.contains(d)));

        // Huruf besar, tanpa tanda hubung, dan spasi di tepi diterima.
        let typed = format!(" {} ", normalized.to_uppercase());
        assert!(totp::use_recovery_code(&ctx, &user, &typed).await.unwrap());
        assert!(
            !totp::use_recovery_code(&ctx, &user, &codes[0])
                .await
                .unwrap()
        );
        assert!(
            totp::use_recovery_code(&ctx, &user, &codes[1])
                .await
                .unwrap()
        );
        assert!(
            !totp::use_recovery_code(&ctx, &user, "bukan-kode")
                .await
                .unwrap()
        );
    })
    .await;
}

#[tokio::test]
async fn regenerating_recovery_codes_replaces_the_old_ones() {
    request::<App, _, _>(|_request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        let old = totp::regenerate_recovery_codes(&ctx, &user)
            .await
            .unwrap()
            .unwrap();
        let new = totp::regenerate_recovery_codes(&ctx, &user)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(digests(&ctx, &user).await.len(), 10);
        assert!(!totp::use_recovery_code(&ctx, &user, &old[0]).await.unwrap());
        assert!(totp::use_recovery_code(&ctx, &user, &new[0]).await.unwrap());
    })
    .await;
}

#[tokio::test]
async fn disable_2fa_requires_password_and_clears_everything() {
    request::<App, _, _>(|_request, ctx| async move {
        let user = a_user().with_totp().create(&ctx).await;
        totp::regenerate_recovery_codes(&ctx, &user)
            .await
            .unwrap()
            .unwrap();
        let code = totp::code_at(TEST_TOTP_SECRET, now()).unwrap();
        assert!(totp::verify(&ctx, &user, &code).await.unwrap());
        let user = reload(&ctx, &user).await;

        let wrong = SecretString::from("password yang salah");
        assert!(!totp::disable(&ctx, &user, &wrong).await.unwrap());
        assert!(reload(&ctx, &user).await.totp_enabled_at.is_some());

        let right = SecretString::from(TEST_PASSWORD);
        assert!(totp::disable(&ctx, &user, &right).await.unwrap());
        let user = reload(&ctx, &user).await;
        assert!(user.totp_enabled_at.is_none());
        assert!(user.totp_secret_encrypted.is_none());
        assert!(user.totp_last_step.is_none());
        assert!(digests(&ctx, &user).await.is_empty());
    })
    .await;
}

#[test]
fn pending_2fa_expires_after_5_minutes() {
    assert!(!sign_in::pending_expired(1000, 1000));
    assert!(!sign_in::pending_expired(1000, 1000 + 299));
    assert!(sign_in::pending_expired(1000, 1000 + 300));
    // Waktu mundur (jam server dikoreksi) tidak memperpanjang sesi tanpa batas.
    assert!(sign_in::pending_expired(1000, 1000 - 301));
}

async fn unused_codes(ctx: &loco_rs::app::AppContext, user: &users::Model) -> u64 {
    recovery_codes::count_unused(&ctx.db, user.id)
        .await
        .unwrap()
}

#[tokio::test]
async fn concurrent_confirms_and_regenerates_leave_exactly_ten_codes() {
    request::<App, _, _>(|_request, ctx| async move {
        let settings = Settings::from_ctx(&ctx).unwrap();
        let user = a_user().create(&ctx).await;
        let setup = totp::begin_setup(&settings, &user, None).unwrap();
        let code = totp::code_at(&setup.secret_base32, now()).unwrap();
        // Klik ganda: dua confirm dengan secret dan kode yang sama, bersamaan.
        let (a, b) = tokio::join!(
            totp::confirm(&ctx, &user, &setup.sealed, &code),
            totp::confirm(&ctx, &user, &setup.sealed, &code),
        );
        let mut outcomes = [a.unwrap(), b.unwrap()];
        outcomes.sort_by_key(|o| matches!(o, Confirmed::AlreadyEnabled));
        assert!(matches!(outcomes[0], Confirmed::Enabled(_)), "{outcomes:?}");
        assert_eq!(outcomes[1], Confirmed::AlreadyEnabled);
        assert_eq!(unused_codes(&ctx, &user).await, 10);

        let user = reload(&ctx, &user).await;
        let (a, b) = tokio::join!(
            totp::regenerate_recovery_codes(&ctx, &user),
            totp::regenerate_recovery_codes(&ctx, &user),
        );
        // Keduanya berhasil, berurutan; yang commit terakhir menang.
        let (a, b) = (a.unwrap().unwrap(), b.unwrap().unwrap());
        assert_eq!(unused_codes(&ctx, &user).await, 10);
        let still_valid = [&a[0], &b[0]];
        let mut valid = 0;
        for code in still_valid {
            if totp::use_recovery_code(&ctx, &user, code).await.unwrap() {
                valid += 1;
            }
        }
        assert_eq!(valid, 1, "hanya satu set kode yang berlaku");
    })
    .await;
}
