# M1 Identitas & Tenancy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Orang bisa mendaftar, login (email + password, Google, GitHub, 2FA), membuat workspace, mengundang anggota, mengatur tim/project/repo dengan role per project, dan setiap akses lintas workspace menghasilkan 404.

**Architecture:** Sesi server-side Loco (`auth_session`, `store: database`) dibungkus satu extractor `CurrentUser` aplikasi. Semua jalur login (password, OAuth, `dev_login`) berakhir di `services::auth::sign_in`, yang memutuskan apakah langkah 2FA dibutuhkan. Tenancy berbasis path `/w/{workspace_slug}/…`: extractor `Tenant` me-resolve workspace + keanggotaan sekali; setiap tabel domain punya `workspace_id` dan setiap query model menerima `WorkspaceId` (trait `TenantEntity` Loco, feature `multi-tenancy`). Izin dinyatakan sebagai enum `Permission`; role (bawaan + custom) adalah himpunan permission per workspace; guard `Require<P>` ditaruh sebelum extractor body.

**Tech Stack:** Loco 1.3.1 (`auth_session`, `auth_oauth` hanya untuk building block `provider`/`state`/`cookies`, `multi-tenancy`, `mailer`), SeaORM 2.0.4, `argon2`, `sha2`, `subtle`, `rand`, `secrecy`, `totp-rs`, `chacha20poly1305`, `hmac`, `validator`, Basecoat 1.1.0 gaya Vega.

**Spec:** `docs/spec.md` §"Organisasi, tim & akses", §"Keamanan, privasi & akun", baris M1 di §"Urutan build MVP"; konvensi `docs/reference/loco-project-rules.md` §4–§10 dan "Ditunda" (OIDC, 2FA, audit log, multi-tenancy). Keputusan user: gaya Vega, layout **sidebar kiri** (switcher workspace di atas, daftar project, menu project, pengaturan + profil di bawah; drawer di mobile).

## Global Constraints

- Semua id UUID v7 (`DEFAULT uuidv7()` + `Uuid::now_v7()` di `before_save`); semua timestamp `timestamptz`; enum = string + CHECK constraint + tes `EnumIter` (§4).
- Setiap tabel domain punya `workspace_id uuid NOT NULL` FK ke `workspaces`, kecuali tabel milik user (`users`, `user_tokens`, `identities`, `recovery_codes`, `login_throttles`).
- Setiap migrasi diawali `SET lock_timeout = '5s'`; `down` selalu ada.
- Password: argon2id, 12–128 karakter (konstanta `PASSWORD_MIN_LEN`/`PASSWORD_MAX_LEN` di `services/auth/password.rs`), dev = production (m=19456, t=2, p=1), test m=8, t=1, p=1; hash di `spawn_blocking` dibatasi `Semaphore` (`max_concurrent_hashes`), tunggu > `hash_wait_seconds` → 503.
- Token email: 32 byte `OsRng`, DB hanya simpan SHA-256 hex; reset 30 menit, verifikasi 24 jam, undangan 7 hari; sekali pakai lewat claim bersyarat.
- Gagal login apa pun → status 422 + satu pesan `auth-invalid-credentials`; email tak dikenal tetap memverifikasi hash dummy. Terkunci rate limit → 429 + `auth-too-many-attempts` (sama untuk email terdaftar atau tidak).
- Rate limit: 5 gagal / 15 menit / email dan 20 gagal / 15 menit / IP → kunci 15 menit (angka di `settings.auth`).
- Cookie sesi: dev `agentgate_session<SESSION_COOKIE_SUFFIX>`, staging/production `__Host-agentgate_session` + `secure: true`; idle `ttl` 86400 detik; umur mutlak 720 jam; remember-me mati.
- Rahasia hanya dari env: `SESSION_SECRET`, `ENCRYPTION_KEY` (32 byte base64, untuk secret TOTP), `GOOGLE_CLIENT_ID/SECRET`, `GITHUB_CLIENT_ID/SECRET`. Dev/test punya default di yaml; staging/production tanpa default.
- Resource milik workspace lain / project tanpa akses / id tidak valid → 404 (bukan 403). User belum login → 303 ke `/login?return_to=…`.
- GET tidak pernah menulis (kecuali `/_dev/login`, debug saja). Sukses tulis → 303; gagal validasi → 422 dengan form terisi (password kosong).
- Semua teks lewat Fluent `id`; kode Rust hanya mengirim kode error (`field`, `code`, args).
- Efek samping (email) setelah commit, lewat job `deliver_later`; job args tidak berisi password.
- Audit log ditulis di transaksi yang sama dengan perubahannya, lewat `services::audit::record`.

## Review Focus

1. **Owner terakhir** — Owner menurunkan role-nya sendiri, keluar, atau dihapus saat ia satu-satunya Owner → ditolak dengan pesan `workspace-last-owner`, workspace tidak pernah tanpa Owner → tes di Task 11.
2. **Undangan untuk email yang sudah jadi anggota / undangan ulang** — undang email anggota → 422 `invite-already-member`; undang ulang email yang undangannya masih aktif → token lama dibatalkan, satu undangan aktif → tes di Task 11.
3. **Email OAuth belum terverifikasi dengan email yang sama dengan akun lokal** — tidak boleh menautkan ke akun lama (account takeover); harus ditolak dengan pesan `oauth-email-unverified` → tes di Task 7.
4. **Slug/kode yang bentrok beda huruf besar** (`Acme` vs `acme`, project `KSR` vs `ksr`) — unik case-insensitive, dinormalisasi, dan URL dengan huruf besar tetap me-resolve → tes di Task 9 dan Task 12.
5. **Workspace yang dijadwalkan hapus** — selama masa tenggang anggota non-Owner mendapat 404, Owner melihat banner dan tombol batalkan; setelah 30 hari job menghapusnya → tes di Task 13.

---

## File Structure

```
server/migration/src/
  m20261112_000001_create_users_and_auth.rs     users, user_tokens, login_throttles, identities, recovery_codes
  m20261112_000002_create_workspaces.rs          workspaces, roles, job_titles, workspace_members, invitations, audit_events
  m20261112_000003_create_teams_projects.rs      teams, team_members, projects, project_grants, repos
server/src/
  common/settings.rs        Settings { app_name, public_base_url, auth: AuthSettings }
  common/crypto.rs          random_token, sha256_hex, ct_eq, encrypt/decrypt (chacha20poly1305)
  common/text.rs            validator berparameter: nama, slug, kode project
  models/                   satu file per tabel (+ _entities/ hasil generate)
  models/permission.rs      enum Permission + BUILTIN_ROLES
  services/auth/            password.rs, sign_in.rs, throttle.rs, tokens.rs, totp.rs, oauth.rs, dev_login.rs
  services/workspaces/      create.rs, invite.rs, accept_invite.rs, membership.rs, deletion.rs
  services/audit.rs         record(txn, AuditEntry)
  controllers/extractors/   current_user.rs, tenant.rs, project_access.rs, require.rs, valid_form.rs, id_or_404.rs
  controllers/auth/         login.rs, logout.rs, register.rs, password.rs, two_factor.rs, oauth.rs, dev_login.rs
  controllers/account.rs    profil, ganti password, 2FA
  controllers/workspaces/   home.rs (pilih/buat), settings.rs, members.rs, invitations.rs, roles.rs, job_titles.rs, teams.rs, audit.rs
  controllers/projects/     projects.rs, access.rs, repos.rs
  controllers/public_routes.rs   daftar route publik (satu tempat)
  mailers/auth.rs, mailers/invitation.rs
  workers/cleanup.rs        purge sesi, throttle, token kedaluwarsa, workspace lewat masa tenggang
  tasks/                    dev_login.rs, user_create.rs, user_set_password.rs
  fixtures/                 users.yaml, workspaces.yaml (seed dev, tanpa password)
server/assets/views/
  layouts/app.html          shell sidebar kiri
  components/               field, card, empty_state, flash, badge, table, sidebar_nav, page_header
  auth/ account/ workspaces/ projects/ mailers/
server/tests/
  support/                  builders (a_user, a_workspace, a_project, a_team), sign_in_as, mailer helpers, fake_oauth
  models/ requests/auth/ requests/workspaces/ requests/projects/ workers/ tasks/
```

---

### Task 1: Dependency, config auth, settings bertipe, support tes

**Files:**
- Modify: `server/Cargo.toml`, `Cargo.toml` (profile `opt-level = 3` untuk `argon2`, `blake2`), `server/config/{development,test,staging,production}.yaml`, `server/src/common/settings.rs`, `server/src/app.rs` (truncate), `server/tests/contracts/config.rs`, `bin/_lib.sh` (`SESSION_COOKIE_SUFFIX` = `_<AG_ID>` di worktree, kosong di checkout utama)
- Create: `server/src/common/crypto.rs`, `server/tests/support/mod.rs`

**Interfaces:**
- Produces: `Settings { app_name, public_base_url: String, auth: AuthSettings }`; `AuthSettings { argon2_memory_kib: u32, argon2_iterations: u32, argon2_parallelism: u32, max_concurrent_hashes: usize, hash_wait_seconds: u64, absolute_lifetime_hours: i64, login_max_failures_per_email: i32, login_max_failures_per_ip: i32, login_window_minutes: i64, login_lockout_minutes: i64, reset_token_minutes: i64, verification_token_hours: i64, invitation_days: i64, registration_open: bool, encryption_key: String }`; `Settings::from_ctx(&AppContext) -> Result<Settings>` memvalidasi `encryption_key` (base64 32 byte) saat boot.
- Produces: `crypto::random_token() -> String` (32 byte, base64url tanpa padding), `crypto::sha256_hex(&str) -> String`, `crypto::ct_eq(&[u8], &[u8]) -> bool`, `crypto::encrypt(key: &[u8;32], plaintext: &[u8]) -> Vec<u8>` (nonce 24 byte XChaCha20-Poly1305 di depan ciphertext), `crypto::decrypt(key, data) -> Result<Vec<u8>, CryptoError>`.

- [ ] **Step 1:** Tes kontrak config ditambah: setiap env punya `auth.session.store == database`, `settings.auth` ter-deserialize; staging/production: `secure: true`, cookie diawali `__Host-`, `argon2_memory_kib >= 19456`, `argon2_iterations >= 2`; test memakai `mailer.stub: true` dan `workers.mode: ForegroundBlocking`. Unit test `crypto`: `encrypt` lalu `decrypt` = plaintext; ciphertext diubah 1 byte → error; dua `random_token()` berbeda dan panjang 43.
- [ ] **Step 2:** `bin/test contracts` → FAIL.
- [ ] **Step 3:** Feature Loco `auth_session`, `auth_oauth`, `multi-tenancy`; crate baru (komentar satu baris masing-masing): `argon2 0.5`, `sha2 0.10`, `subtle 2`, `rand 0.9`, `secrecy 0.10` (serde), `totp-rs 5` (feature `qr`, `gen_secret`), `chacha20poly1305 0.10`, `hmac 0.12`, `base64 0.22`, `validator 0.20` (derive), `uuid 1` (v7, serde), `chrono 0.4`. Config `auth.session` di keempat env; `settings` lengkap dengan komentar per key (apa, kenapa angkanya).
- [ ] **Step 4:** `bin/test contracts` → PASS; `bin/ci` hijau.
- [ ] **Step 5:** Commit `feat(server): add auth config, typed settings and crypto helpers`.

### Task 2: Migrasi user & model `users`

**Files:**
- Create: `server/migration/src/m20261112_000001_create_users_and_auth.rs`, `server/src/models/{users,user_tokens,login_throttles,identities,recovery_codes}.rs`, `server/src/models/_entities/*`, `server/src/common/text.rs`, `server/src/services/auth/password.rs`
- Test: `server/tests/models/{users,user_tokens,enums}.rs`

**Interfaces:**
- Tabel: `users(id, email, name, password_hash NULL, email_verified_at, password_changed_at, disabled_at, totp_secret_encrypted bytea NULL, totp_enabled_at, totp_last_step bigint NULL, created_at, updated_at)` + unique `lower(email)`; `user_tokens(id, user_id FK CASCADE, purpose CHECK IN ('password_reset','email_verification'), token_digest UNIQUE, expires_at, used_at)`; `login_throttles(key PK text, failures int, window_started_at, locked_until)`; `identities(id, user_id FK CASCADE, provider CHECK IN ('google','github'), provider_uid, email, created_at)` unique `(provider, provider_uid)`; `recovery_codes(id, user_id FK CASCADE, code_digest, used_at)`. `password_hash` nullable: user OAuth-only.
- Produces: `users::Model::find_by_email(db, &str) -> Result<Option<Model>>` (normalisasi `trim().to_lowercase()`), `users::ActiveModel::create(db, NewUser { email, name, password: Option<SecretString> }) -> Result<Model, UserError>` (`UserError::EmailTaken` dari unique violation), `users::Model::is_active(&self) -> bool`.
- Produces: `password::hash(&AuthSettings, SecretString) -> Result<String>`, `password::verify(&AuthSettings, &SecretString, &str) -> Result<Verify>` dengan `Verify::{Ok, OkNeedsRehash, Mismatch}`, `password::verify_dummy(&AuthSettings, &SecretString)`; semua lewat semaphore + `spawn_blocking`.
- Produces: `user_tokens::issue(db, user_id, TokenPurpose, ttl) -> Result<String /*raw*/>` (membatalkan token lama purpose sama), `user_tokens::consume(db, raw, TokenPurpose) -> Result<Option<Uuid /*user_id*/>>` (claim bersyarat), `user_tokens::peek(db, raw, purpose) -> Result<bool>` (tanpa konsumsi, untuk GET reset).

- [ ] **Step 1:** Tes: `email_is_normalized_and_unique_case_insensitive`, `password_shorter_than_12_is_rejected` (`code = "password-too-short"`), `rehash_is_reported_when_params_increase`, `reset_token_cannot_be_used_twice`, `expired_token_is_rejected`, `new_reset_token_invalidates_previous`, `peek_does_not_consume_token`; tes enum: setiap varian `TokenPurpose`/`IdentityProvider` tersimpan dan literal `'bogus'` ditolak lewat raw SQL.
- [ ] **Step 2:** `bin/test models` → FAIL.
- [ ] **Step 3:** Migrasi, `bin/loco db migrate`, `bin/loco db entities`, model; `common/text.rs` berisi `validate_name(entity, value)` (1–100 karakter, tanpa karakter tak terlihat) dan `NAME_MAX_LEN`. Daftarkan semua tabel + `loco_sessions`/`loco_remember_tokens` di hook `truncate`.
- [ ] **Step 4:** `bin/test models` → PASS.
- [ ] **Step 5:** Commit `feat(server): add users, tokens and password hashing`.

### Task 3: Login, logout, `CurrentUser`, gate, rate limit

**Files:**
- Create: `server/src/services/auth/{mod,sign_in,throttle}.rs`, `server/src/controllers/extractors/{mod,current_user,valid_form}.rs`, `server/src/controllers/public_routes.rs`, `server/src/controllers/auth/{mod,login,logout}.rs`, `server/src/views/auth.rs`, `server/assets/views/auth/{login,_layout_card}.html`, `server/assets/views/components/{field,flash}.html`
- Modify: `server/src/controllers/mod.rs`, `server/src/app.rs` (layer gate), `server/assets/i18n/id/main.ftl`
- Test: `server/tests/requests/auth/{login,logout,gate,throttle,session}.rs`, `server/tests/support/{builders,session}.rs`

**Interfaces:**
- Produces: `sign_in::complete(ctx, session: Session, user: &users::Model) -> Result<SignInStep>` dengan `SignInStep::{Done(Session), NeedsSecondFactor(Session)}`; `Done` = `renew().set_user_id(id)` + `signed_in_at`; `NeedsSecondFactor` = `renew()` + `pending_user_id` + `pending_since` (tanpa `user_id`). Satu-satunya tempat sesi login dibuat.
- Produces: extractor `CurrentUser(pub users::Model)`: menolak (dan `clear()`) sesi tanpa user, user `disabled_at`, `signed_in_at` lebih tua dari `absolute_lifetime_hours`; penolakan = 303 ke `/login?return_to=<path>` untuk GET, 303 ke `/login` untuk non-GET. `MaybeCurrentUser(pub Option<users::Model>)`.
- Produces: `public_routes::is_public(path) -> bool` (`/login`, `/login/2fa`, `/register`, `/verify-email`, `/password/*`, `/auth/*`, `/invitations/*`, `/_ping`, `/_health`, `/static/*`, `/_dev/*`, `/logout`); layer gate di `after_routes` me-redirect request non-publik tanpa sesi valid (pengaman kedua; handler tetap memakai `CurrentUser`).
- Produces: `throttle::check(db, keys: &[ThrottleKey]) -> Result<ThrottleState>`, `throttle::record_failure(db, keys)`, `throttle::reset(db, ThrottleKey)`; `ThrottleKey::{Email(String), Ip(IpAddr)}`. IP dari `RemoteIP`; `RemoteIP::None` → login ditolak + `tracing::error!`.
- Produces: `ValidForm<T>`: form tak bisa di-parse → 422 halaman form yang sama tidak mungkin, jadi rejection → 400 halaman error HTML; `safe_return_to(&str) -> Option<String>` (diawali `/`, bukan `//`, tanpa `\`).
- Produces (tes): `support::a_user() -> UserBuilder` (`.email()`, `.name()`, `.password()`, `.unverified()`, `.disabled()`, `.with_totp()`, `.create(&ctx).await -> users::Model`), `support::sign_in_as(&request, &user) -> Cookie`, `support::TEST_PASSWORD`.

- [ ] **Step 1:** Tes (nama = klaim): `unknown_email_and_wrong_password_get_same_422_message`, `disabled_user_cannot_sign_in`, `disabled_user_existing_session_is_rejected`, `session_cookie_from_before_login_is_not_valid_after`, `external_return_to_is_dropped`, `return_to_internal_path_is_followed`, `signed_in_user_visiting_login_is_redirected`, `email_is_locked_after_5_failures_even_if_unregistered`, `successful_login_resets_email_failures`, `get_logout_does_not_sign_out`, `logout_with_stale_cookie_still_redirects`, `session_older_than_absolute_lifetime_is_rejected`, `protected_page_redirects_to_login_with_return_to`, `unverified_user_gets_verify_page_not_session`, `login_page_has_csp_header`.
- [ ] **Step 2:** `bin/test auth` → FAIL.
- [ ] **Step 3:** Implementasi sesuai Interfaces; halaman login memakai layout `public.html` (card terpusat, logo, form, tombol Google/GitHub hanya bila provider dikonfigurasi, link daftar/lupa password). Rehash saat `OkNeedsRehash`.
- [ ] **Step 4:** `bin/test auth` → PASS.
- [ ] **Step 5:** Commit `feat(server): add password login, sessions and route gate`.

### Task 4: Daftar, verifikasi email, lupa/reset/ganti password, mailer

**Files:**
- Create: `server/src/controllers/auth/{register,password,verify_email}.rs`, `server/src/controllers/account.rs`, `server/src/mailers/{mod,auth}.rs`, `server/assets/views/mailers/auth/{verify,already_registered,password_reset}/{subject,html,text}.t`, `server/assets/views/auth/{register,check_email,verify_pending,forgot,reset}.html`, `server/assets/views/account/{show,password}.html`
- Test: `server/tests/requests/auth/{register,verify_email,password_reset,change_password}.rs`, `server/tests/mailers/auth.rs`

**Interfaces:**
- Consumes: `user_tokens::{issue, consume, peek}`, `sign_in::complete`, `password::*`.
- Produces: `AuthMailer::{send_verification, send_already_registered, send_password_reset}(ctx, &users::Model, raw_token)` — link dari `settings.public_base_url`, teks dari Fluent.
- Rute: `GET|POST /register`, `GET /verify-email?token=` (tampilkan tombol) + `POST /verify-email` (konsumsi), `POST /verify-email/resend` (ikut throttle), `GET|POST /password/forgot`, `GET|POST /password/reset`, `GET /account`, `POST /account/profile`, `GET|POST /account/password`. Route bertoken mengirim `Referrer-Policy: no-referrer`.

- [ ] **Step 1:** Tes: `register_with_existing_email_responds_same_as_new` (status, body selector, satu email terkirim berisi link reset), `register_sends_verification_email`, `verification_link_get_does_not_consume_token`, `verified_user_can_sign_in`, `forgot_password_responds_same_for_unknown_email`, `reset_get_does_not_consume_token`, `reset_revokes_other_sessions`, `reset_sets_email_verified_at`, `expired_reset_token_shows_invalid_page`, `change_password_requires_current_password`, `change_password_keeps_only_current_device_signed_in`, `registration_closed_hides_form_returns_404`, `every_auth_email_template_renders`.
- [ ] **Step 2:** FAIL → **Step 3:** implementasi → **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(server): add registration, email verification and password reset`.

### Task 5: `dev_login`, seed, task user

**Files:**
- Create: `server/src/services/auth/dev_login.rs`, `server/src/controllers/auth/dev_login.rs`, `server/src/tasks/{mod,dev_login,user_create,user_set_password}.rs`, `server/src/fixtures/{users,workspaces,workspace_members}.yaml`
- Modify: `server/src/app.rs` (`register_tasks`, `seed`), `README.md` (email user seed + cara `dev_login`)
- Test: `server/tests/tasks/{dev_login,user_create}.rs`, `server/tests/requests/auth/dev_login.rs`

**Interfaces:**
- Produces: `dev_login::sign(secret: &str, user_id: Uuid, now) -> String` (HMAC-SHA256, kedaluwarsa 10 menit), `dev_login::verify(secret, token, now) -> Option<Uuid>`.
- Seed: `owner@agentgate.test` (Owner "PT Contoh"), `admin@`, `dev@`, `qa@`, `viewer@` dengan jabatan PM/Dev/QA; project `KSR` dengan tim Backend dan QA. Tanpa password.

- [ ] **Step 1:** Tes: `dev_login_route_is_404_in_test_env`, `dev_login_task_refuses_outside_development`, `dev_login_token_expires_after_10_minutes`, `tampered_dev_login_token_is_rejected`, `user_create_task_creates_verified_user`.
- [ ] **Step 2–4:** FAIL → implementasi (route hanya `cfg(debug_assertions)` + `LOCO_ENV=development`; `user:set_password` membaca prompt tanpa echo lewat `rpassword`) → PASS.
- [ ] **Step 5:** Commit `feat(server): add dev login, seed data and user tasks`.

### Task 6: 2FA (TOTP + kode pemulihan)

**Files:**
- Create: `server/src/services/auth/totp.rs`, `server/src/controllers/auth/two_factor.rs`, `server/assets/views/auth/two_factor.html`, `server/assets/views/account/{two_factor_setup,recovery_codes}.html`
- Test: `server/tests/requests/auth/two_factor.rs`, `server/tests/models/totp.rs`

**Interfaces:**
- Produces: `totp::begin_setup(settings, &user) -> SetupSecret { secret_base32, otpauth_uri, qr_svg }` (secret pending disimpan terenkripsi di data sesi, bukan DB, sampai dikonfirmasi); `totp::confirm(ctx, &user, pending, code) -> Result<Vec<String> /*10 recovery codes*/>`; `totp::verify(ctx, &user, code) -> Result<bool>` (TOTP SHA1, 6 digit, 30 detik, toleransi ±1 langkah, kode yang sama tidak bisa dipakai dua kali dalam langkahnya: simpan `totp_last_step` di users); `totp::use_recovery_code(ctx, &user, code) -> Result<bool>`; `totp::disable(ctx, &user, current_password)`.
- Rute: `GET|POST /login/2fa` (sesi `pending_user_id` maks 5 menit; throttle sendiri: 5 gagal → sesi pending dibuang), `GET|POST /account/2fa` (setup, QR SVG inline), `POST /account/2fa/disable`, `POST /account/2fa/recovery-codes` (generate ulang).
- Kolom TOTP sudah dibuat migrasi Task 2.

- [ ] **Step 1:** Tes: `user_with_totp_needs_second_step_before_session`, `pending_2fa_session_cannot_open_protected_pages`, `wrong_totp_five_times_drops_pending_login`, `totp_code_cannot_be_replayed_in_same_step`, `recovery_code_works_once`, `pending_2fa_expires_after_5_minutes`, `disable_2fa_requires_password`, `totp_secret_is_stored_encrypted` (kolom ≠ secret base32).
- [ ] **Step 2–4:** FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat(server): add TOTP two-factor authentication`.

### Task 7: Login Google & GitHub

**Files:**
- Create: `server/src/services/auth/oauth.rs`, `server/src/controllers/auth/oauth.rs`, `server/tests/support/fake_oauth.rs`
- Modify: config keempat env (`auth.oauth.providers.google|github` hanya bila env client id ada; dev/test memakai provider `generic` ke fake), CSP `form-action 'self' https://accounts.google.com https://github.com` dengan komentar
- Test: `server/tests/requests/auth/oauth.rs`

**Interfaces:**
- Consumes: building block publik Loco `auth::oauth::{provider::{Provider, exchange_code, fetch_identity}, state::{FlowState, cookie_name, STATE_TTL_SECONDS}}`, `auth::cookies::{seal, unseal, SignedCookies}`. **Tidak** memakai `oauth::routes` Loco: callback-nya langsung membuat sesi sehingga melewati 2FA dan `signed_in_at`.
- Produces: `oauth::resolve_user(ctx, &Identity) -> Result<OAuthOutcome>`: identitas dikenal → user itu; email `email_verified == Some(true)` cocok dengan user lokal → tautkan; email tak dikenal + verified + `registration_open` → buat user (verified, tanpa password); selain itu `OAuthOutcome::Rejected(code)` dengan code `oauth-email-unverified` | `oauth-registration-closed` | `oauth-user-disabled`. Hasil sukses masuk `sign_in::complete`.
- Rute: `POST /auth/{provider}` (form `hx-boost="false"`), `GET /auth/{provider}/callback`. Kegagalan → redirect `/login?oauth_error=<kind>` dengan pesan Fluent.
- `fake_oauth::start() -> FakeProvider { base_url, set_identity(json) }` — server Axum di `127.0.0.1:0` di dalam tes (authorize, token, userinfo).

- [ ] **Step 1:** Tes: `oauth_start_requires_post`, `callback_with_wrong_state_is_rejected`, `new_verified_identity_creates_user`, `verified_email_links_existing_account`, `unverified_email_does_not_link_existing_account`, `oauth_user_with_totp_still_needs_second_step`, `oauth_sign_in_renews_session`, `oauth_buttons_hidden_when_provider_not_configured`.
- [ ] **Step 2–4:** FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat(server): add Google and GitHub sign-in`.

### Task 8: Shell aplikasi (sidebar kiri) dan komponen

**Files:**
- Modify: `server/assets/views/layouts/app.html`, `server/assets/views/dev/ui.html`, `server/src/views/dev.rs`
- Create: `server/assets/views/components/{card,empty_state,badge,table,page_header,sidebar_nav,avatar}.html`, `server/assets/css/components/sidebar.css` (bila perlu), `server/src/views/layout.rs`
- Test: `server/tests/requests/dev_ui.rs` (tambah varian), `server/tests/contracts/views.rs`

**Interfaces:**
- Produces: `layout::AppShell { user: UserView, workspaces: Vec<WorkspaceLink>, current_workspace: Option<WorkspaceView>, projects: Vec<ProjectLink>, current_project: Option<ProjectLink>, can_manage_workspace: bool }` dibangun **satu kali** oleh `layout::app_shell(ctx, &CurrentUser, Option<&Tenant>, Option<&ProjectAccess>)`; setiap halaman aplikasi menerima `shell` dan meneruskannya ke `layouts/app.html`.
- Layout: sidebar Basecoat (`aside.sidebar`) berisi dropdown switcher workspace (nama + "Buat workspace"), Beranda, PROJECT (daftar project yang bisa diakses; project aktif terbuka dengan menu Board/Backlog/Knowledge/AI Office sebagai item nonaktif bertanda "segera" sampai milestone-nya), lalu Pengaturan (bila boleh) dan menu profil (Akun, Keluar = form POST). `hx-preload:inherited="mouseover"` di nav. Mobile: tombol menu membuka sidebar (perilaku bawaan Basecoat).
- Komponen baru dicatat di katalog `/_dev/ui` dengan semua variannya.

- [ ] **Step 1:** Tes: katalog merender semua komponen baru; `app_layout_renders_with_minimal_shell` (tanpa workspace) dan dengan workspace + project.
- [ ] **Step 2–4:** FAIL → implementasi → PASS. Screenshot desktop 1280 dan mobile 390 dengan harness Playwright untuk dicek manual.
- [ ] **Step 5:** Commit `feat(server): add sidebar app shell and shared components`.

### Task 9: Workspace, role bawaan, extractor `Tenant`

**Files:**
- Create: `server/migration/src/m20261112_000002_create_workspaces.rs` (termasuk `audit_events`), `server/src/models/{workspaces,workspace_members,roles,job_titles,invitations,permission,audit_events}.rs`, `server/src/services/audit.rs`, `server/src/services/workspaces/{mod,create}.rs`, `server/src/controllers/extractors/{tenant,require,id_or_404}.rs`, `server/src/controllers/workspaces/{mod,home,settings}.rs`, `server/assets/views/workspaces/{choose,new,settings}.html`
- Test: `server/tests/requests/workspaces/{create,tenant_isolation,home}.rs`, `server/tests/models/{workspaces,permission}.rs`

**Interfaces:**
- Tabel: `workspaces(id, name, slug, require_2fa_at, deletion_scheduled_at, created_by)` unique `lower(slug)`; `roles(id, workspace_id, key NULL /*builtin: owner|admin|member|viewer*/, name, permissions text[] , builtin bool)` unique `(workspace_id, lower(name))`; `job_titles(id, workspace_id, name)`; `workspace_members(id, workspace_id, user_id, role_id, job_title_id NULL, joined_at)` unique `(workspace_id, user_id)`; `invitations(id, workspace_id, email, role_id, job_title_id NULL, token_digest UNIQUE, invited_by, expires_at, accepted_at, revoked_at)` + partial unique `(workspace_id, lower(email)) WHERE accepted_at IS NULL AND revoked_at IS NULL`.
- `permission.rs`: `enum Permission { ManageWorkspace, ManageMembers, ManageRoles, ManageTeams, CreateProjects, ViewAuditLog, AdministerProject, EditProject /*tiket di M2*/, ViewProject }` disimpan sebagai string `snake_case` (CHECK lewat fungsi `permission_valid` tidak perlu: array divalidasi di model + tes). `BUILTIN_ROLES`: Owner = semua; Admin = semua kecuali `ManageWorkspace`(hapus/2FA wajib/transfer); Member = `ViewProject, EditProject, CreateProjects`; Viewer = `ViewProject`. Role workspace berlaku sebagai role default; role per project dari Task 12. Owner/Admin workspace otomatis `AdministerProject` di semua project.
- Produces: `create::run(ctx, &users::Model, NewWorkspace { name, slug }) -> Result<workspaces::Model, WorkspaceError>` (satu transaksi: workspace + 4 role bawaan + member Owner + audit); slug default dari nama (`slugify`), 3–40 karakter `[a-z0-9-]`, daftar terlarang (`new`, `settings`, `admin`, `api`, `auth`).
- Produces: extractor `Tenant { workspace: workspaces::Model, member: workspace_members::Model, role: roles::Model }` dari path `/w/{slug}`: bukan anggota / workspace tidak ada / dijadwalkan hapus (kecuali Owner) → 404; `require_2fa_at` terisi dan user belum 2FA → 303 `/account/2fa?required=<slug>`. `Tenant::workspace_id() -> WorkspaceId` (newtype Uuid). `impl TenantEntity` untuk setiap entity ber-`workspace_id`.
- Produces: `Require<P: PermissionMarker>` (marker type per permission, mis. `Require<ManageMembers>`) memakai `Tenant`; tanpa izin → 404.
- Produces: `IdOr404<T>` (UUID tidak valid → 404).
- Rute: `GET /` (belum punya workspace → `/w/new`; satu → `/w/{slug}`; lebih → halaman pilih), `GET|POST /w/new`, `GET /w/{slug}` (beranda workspace: daftar project yang bisa diakses, empty state), `GET|POST /w/{slug}/settings` (nama, slug).

- [ ] **Step 1:** Tes: `signup_then_create_workspace_makes_user_owner`, `workspace_slug_unique_case_insensitive`, `reserved_slug_rejected`, `uppercase_slug_in_url_resolves`, `non_member_gets_404_for_workspace`, `other_workspace_member_gets_404_for_settings`, `viewer_cannot_open_settings_gets_404`, `root_redirects_to_only_workspace`, `builtin_roles_created_with_workspace`, `permission_strings_roundtrip_and_unknown_rejected`.
- [ ] **Step 2–4:** FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat(server): add workspaces, built-in roles and tenant extractor`.

### Task 10: Role custom dan jabatan tim

**Files:**
- Create: `server/src/controllers/workspaces/{roles,job_titles}.rs`, `server/assets/views/workspaces/roles/{index,_form}.html`, `server/assets/views/workspaces/job_titles/index.html`
- Test: `server/tests/requests/workspaces/{roles,job_titles}.rs`

**Interfaces:**
- Consumes: `Require<ManageRoles>`, `roles`, `job_titles`.
- Produces: `roles::ActiveModel::create_custom(db, WorkspaceId, name, Vec<Permission>)`, `update_custom`, `delete_custom` (ditolak bila masih dipakai member/grant: `role-in-use`); `ensure_role_editable(&roles::Model) -> Result<(), RoleError>` (role bawaan tidak bisa diubah/dihapus: `role-builtin`). Semua handler role memanggil fungsi ini.
- Jabatan: CRUD sederhana (nama unik per workspace, hapus → member yang memakainya menjadi tanpa jabatan, `ON DELETE SET NULL` dengan komentar).

- [ ] **Step 1:** Tes: `custom_role_with_permissions_can_be_created`, `builtin_role_cannot_be_edited_or_deleted`, `role_in_use_cannot_be_deleted`, `member_without_manage_roles_gets_404`, `duplicate_role_name_gets_422`, `job_title_crud_and_delete_unassigns_members`.
- [ ] **Step 2–4:** FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat(server): add custom roles and job titles`.

### Task 11: Anggota dan undangan email

**Files:**
- Create: `server/src/services/workspaces/{invite,accept_invite,membership}.rs`, `server/src/mailers/invitation.rs`, `server/src/controllers/workspaces/{members,invitations}.rs`, `server/src/controllers/invitations.rs` (publik), `server/assets/views/workspaces/members/index.html`, `server/assets/views/invitations/{show,invalid}.html`, `server/assets/views/mailers/invitation/*`
- Test: `server/tests/requests/workspaces/{members,invitations}.rs`

**Interfaces:**
- Produces: `invite::run(ctx, &Tenant, actor, InviteParams { email, role_id, job_title_id }) -> Result<invitations::Model, InviteError>` (`invite-already-member`, undangan aktif lama di-revoke lalu dibuat baru; email lewat job setelah commit; audit `member.invited`).
- Produces: `accept_invite::run(ctx, raw_token, &users::Model) -> Result<workspaces::Model, AcceptError>` — email user harus sama (case-insensitive) dengan undangan (`invite-email-mismatch`), claim bersyarat; audit `member.joined`.
- Alur publik: `GET /invitations/{token}` menampilkan nama workspace + tombol. Belum login → tombol "Daftar" (form register dengan email terkunci; register lewat undangan menandai email terverifikasi karena token membuktikan inbox) dan "Masuk" (`return_to` kembali ke undangan). Sudah login → `POST /invitations/{token}/accept`.
- Produces: `membership::change_role(ctx, &Tenant, actor, member_id, role_id)`, `membership::remove(ctx, &Tenant, actor, member_id)`, `membership::leave(ctx, &Tenant)` — semuanya memanggil `ensure_not_last_owner` (`workspace-last-owner`) dan menulis audit.
- Halaman anggota: tabel (nama, email, jabatan, role, bergabung), undangan tertunda (kirim ulang / batalkan), form undang di dialog Basecoat.

- [ ] **Step 1:** Tes: `owner_can_invite_and_email_is_sent`, `invite_existing_member_gets_422`, `reinvite_revokes_previous_token`, `invite_link_get_does_not_accept`, `invite_accept_requires_matching_email`, `register_via_invite_marks_email_verified_and_joins`, `expired_invite_shows_invalid_page`, `last_owner_cannot_demote_or_leave`, `removed_member_loses_access_immediately` (request berikutnya 404), `member_cannot_invite_gets_404`.
- [ ] **Step 2–4:** FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat(server): add members and email invitations`.

### Task 12: Tim, project, akses per project, repo

**Files:**
- Create: `server/migration/src/m20261112_000003_create_teams_projects.rs`, `server/src/models/{teams,team_members,projects,project_grants,repos}.rs`, `server/src/controllers/extractors/project_access.rs`, `server/src/controllers/workspaces/teams.rs`, `server/src/controllers/projects/{mod,projects,access,repos}.rs`, `server/assets/views/teams/*`, `server/assets/views/projects/{index,new,show,settings,access,repos}.html`
- Test: `server/tests/requests/projects/{create,access,repos}.rs`, `server/tests/requests/workspaces/teams.rs`, `server/tests/models/project_access.rs`

**Interfaces:**
- Tabel: `teams(id, workspace_id, name)` unique `(workspace_id, lower(name))`; `team_members(id, workspace_id, team_id CASCADE, user_id, ...)` unique `(team_id, user_id)`; `projects(id, workspace_id, key, name, description, created_by, archived_at)` unique `(workspace_id, upper(key))`, key `^[A-Z][A-Z0-9]{1,9}$`; `project_grants(id, workspace_id, project_id CASCADE, team_id NULL, user_id NULL, role_id)` + CHECK tepat satu dari `team_id`/`user_id`, partial unique per subjek; `repos(id, workspace_id, project_id CASCADE, name, provider CHECK IN ('github','gitlab','bitbucket','other'), url, default_branch)`.
- Produces: `project_access::effective_role(db, &Tenant, project_id) -> Result<Option<EffectiveRole>>`: Owner/Admin workspace → Admin; selain itu permission gabungan (union) dari grant user langsung dan grant semua tim user; tanpa grant → `None`. Satu fungsi, dipakai extractor dan daftar project di sidebar (`projects::accessible_for(db, &Tenant) -> Vec<projects::Model>`, satu query, tanpa N+1).
- Produces: extractor `ProjectAccess { project, role: EffectiveRole }` dari path `/w/{slug}/p/{key}`; tanpa akses → 404. `ProjectAccess::require(Permission) -> Result<(), Error /*404*/>`.
- Pembuat project otomatis mendapat grant Admin (role bawaan Admin) di project itu.
- Halaman: daftar & buat tim (anggota tim dari anggota workspace), daftar & buat project (`CreateProjects`), project show (ringkasan + repo + empty state "Board hadir di M2"), pengaturan akses project (tambah tim/orang + role, ubah, cabut; `AdministerProject`), repo (CRUD; `AdministerProject`; URL harus https atau ssh `git@`).

- [ ] **Step 1:** Tes: `project_key_unique_case_insensitive_and_normalized_upper`, `workspace_admin_is_admin_on_all_projects`, `member_without_grant_gets_404_for_project`, `team_grant_gives_access_to_team_members`, `user_in_two_teams_gets_union_of_permissions`, `viewer_grant_cannot_edit_project_settings_gets_404`, `removing_team_grant_revokes_access`, `project_of_other_workspace_gets_404_even_with_same_key`, `repo_url_must_be_https_or_ssh`, `sidebar_lists_only_accessible_projects`.
- [ ] **Step 2–4:** FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat(server): add teams, projects, per-project access and repos`.

### Task 13: Audit log, 2FA wajib, hapus workspace, job pembersihan

**Files:**
- Create: `server/src/services/workspaces/deletion.rs`, `server/src/workers/cleanup.rs`, `server/src/controllers/workspaces/audit.rs`, `server/assets/views/workspaces/{audit,danger}.html`
- Modify: `server/config/*.yaml` (scheduler: cleanup harian 03:00 UTC, komentar alasannya), `server/src/app.rs` (`connect_workers`)
- Test: `server/tests/requests/workspaces/{audit,require_2fa,deletion}.rs`, `server/tests/workers/cleanup.rs`

**Interfaces:**
- Tabel (dibuat di Task 9): `audit_events(id, workspace_id, actor_id NULL, action, subject_type, subject_id, changes jsonb, request_id, created_at)` index `(workspace_id, created_at DESC, id DESC)`; tanpa `updated_at`; model tidak punya update/delete.
- Produces: `audit::record(txn, AuditEntry { workspace_id, actor_id, action: AuditAction, subject: (&str, Uuid), changes: serde_json::Value, request_id: Option<String> })`; `AuditAction` enum (`workspace.created`, `workspace.updated`, `workspace.require_2fa_changed`, `workspace.deletion_scheduled`, `workspace.deletion_cancelled`, `member.invited`, `member.joined`, `member.role_changed`, `member.removed`, `member.left`, `role.created|updated|deleted`, `team.*`, `project.created|updated|archived`, `project.access_granted|access_changed|access_revoked`, `repo.added|updated|removed`). Tabel, model, dan fungsi ini dibuat di Task 9 (dipakai Task 9–12); Task 13 menambah halaman dan tesnya.
- Halaman audit (`ViewAuditLog`): tabel keyset pagination 50 baris, filter action.
- Pengaturan workspace (Owner): toggle "Wajibkan 2FA" (`require_2fa_at`), "Hapus workspace" (konfirmasi ketik slug) → `deletion_scheduled_at = now()`; banner merah untuk Owner + tombol batalkan; non-Owner 404.
- `workers::cleanup::run(ctx)`: `SessionStore::purge_expired`, hapus `login_throttles` yang tidak terkunci dan jendelanya lewat, hapus `user_tokens` kedaluwarsa > 7 hari, hapus workspace dengan `deletion_scheduled_at < now() - 30 hari` (satu transaksi per workspace; urutan FK dengan `ON DELETE CASCADE` dari `workspaces` untuk tabel domain, dikomentari). Idempoten.

- [ ] **Step 1:** Tes: `audit_rows_written_for_invite_and_role_change`, `audit_page_paginates_and_filters`, `member_cannot_see_audit_log_gets_404`, `require_2fa_redirects_member_without_totp_to_setup`, `member_with_totp_unaffected_by_require_2fa`, `scheduled_deletion_hides_workspace_from_non_owner`, `owner_can_cancel_scheduled_deletion`, `cleanup_deletes_workspace_after_30_days_only`, `cleanup_is_idempotent`.
- [ ] **Step 2–4:** FAIL → implementasi → PASS.
- [ ] **Step 5:** Commit `feat(server): add audit log, required 2FA and workspace deletion`.

### Task 14: Penutup milestone

- [ ] `bin/ci` hijau; `bin/loco db reset` + `bin/loco db seed` jalan; alur manual di `bin/dev` lewat Playwright: daftar → cek Mailpit → verifikasi → buat workspace → undang → terima di browser kedua → buat project + tim → akses lintas workspace 404. Screenshot desktop/mobile disimpan di scratchpad.
- [ ] Review kode seluruh branch (subagent reviewer) dan perbaiki temuan Critical/Important.
- [ ] `docs/PROGRESS.md`: status M1, keputusan (URL `/w/{slug}`, OAuth tanpa `oauth::routes` Loco, role = himpunan permission, `ENCRYPTION_KEY`), kebutuhan user (OAuth app Google/GitHub). README: user seed, `dev_login`, env baru.
- [ ] Merge `feature/m1-identitas` → `staging` `--no-ff`.
