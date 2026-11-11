//! Email aplikasi (rules §8). Setiap email adalah satu folder di `assets/views/mailers/`
//! berisi `subject.t`, `text.t`, dan `html.t` (versi teks biasa dan HTML). Teksnya dari
//! Fluent lewat fungsi `t()` yang sama dengan halaman, sehingga template email dirender di
//! sini, bukan lewat `Mailer::mail_template` Loco (Tera milik Loco tidak punya `t()`).
//!
//! Email selalu dikirim lewat job (`deliver_later`, `MailerWorker` Loco), tidak pernah di
//! dalam request: SMTP yang lambat atau mati tidak membuat request ikut lambat atau gagal.

use std::sync::OnceLock;

use fluent_templates::FluentLoader;
use loco_rs::{
    Error, Result,
    app::AppContext,
    mailer::{Email, Mailer},
};
use tera::{Context, Tera};

use crate::common::{i18n, settings::Settings};

pub mod auth;
pub mod invitation;

/// Glob semua template email. Nama template relatif terhadap `assets/views/mailers/`
/// (mis. `auth/verify/html.t`), sehingga `html.t` bisa `extends "_layout.html.t"`.
const TEMPLATE_GLOB: &str = "assets/views/mailers/**/*.t";

static ENGINE: OnceLock<Tera> = OnceLock::new();

/// Hasil render satu email.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub subject: String,
    pub text: String,
    pub html: String,
}

/// Tera khusus email, dibuat sekali: semua template email beserta `t()`. Autoescape hanya
/// untuk versi HTML (`*html.t`); versi teks dan subjek tidak di-escape.
fn engine() -> Result<&'static Tera> {
    if let Some(tera) = ENGINE.get() {
        return Ok(tera);
    }
    let mut tera = Tera::default();
    tera.autoescape_on(["html.t"]);
    tera.register_function("t", FluentLoader::new(i18n::loader()?));
    tera.load_from_glob(TEMPLATE_GLOB)
        .map_err(|e| Error::string(&format!("mail templates: {e}")))?;
    Ok(ENGINE.get_or_init(|| tera))
}

/// Render email `dir` (mis. `auth/verify`) dengan `locals`. Subjek diratakan menjadi satu
/// baris: header email tidak boleh berisi baris baru.
///
/// # Errors
///
/// Template tidak ada atau gagal dirender (mis. variabel yang tidak dikirim).
pub fn render(dir: &str, locals: &serde_json::Value) -> Result<Rendered> {
    let tera = engine()?;
    let context =
        Context::from_serialize(locals).map_err(|e| Error::string(&format!("mail {dir}: {e}")))?;
    let part = |file: &str| {
        tera.render(&format!("{dir}/{file}"), &context)
            .map_err(|e| Error::string(&format!("mail {dir}/{file}: {e}")))
    };
    Ok(Rendered {
        subject: part("subject.t")?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        text: part("text.t")?,
        html: part("html.t")?,
    })
}

/// Pengirim untuk `Mailer::mail` Loco; alamat `From` diisi dari `settings.mail_from`.
struct AppMailer;

impl Mailer for AppMailer {}

/// Render email `dir` lalu antrekan pengirimannya ke `to` (job `MailerWorker`).
///
/// # Errors
///
/// Render gagal atau job tidak bisa diantrekan (mode `ForegroundBlocking`: pengiriman gagal).
pub async fn deliver(
    ctx: &AppContext,
    settings: &Settings,
    to: &str,
    dir: &str,
    locals: &serde_json::Value,
) -> Result<()> {
    let content = render(dir, locals)?;
    AppMailer::mail(
        ctx,
        &Email {
            from: Some(settings.mail_from.clone()),
            to: to.to_string(),
            subject: content.subject,
            text: content.text,
            html: content.html,
            ..Default::default()
        },
    )
    .await
}
