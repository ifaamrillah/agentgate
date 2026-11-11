//! Email yang "terkirim" lewat stub mailer (`mailer.stub: true` di config test, job berjalan
//! sinkron lewat `ForegroundBlocking`), dan token yang dibawa link di dalamnya.

use loco_rs::app::AppContext;

/// Pesan mentah (RFC 5322) yang sudah dikirim, berurutan, dengan soft line break
/// quoted-printable dibuang dan `=3D` dikembalikan menjadi `=`, supaya link bisa dicari.
#[must_use]
pub fn sent_mail(ctx: &AppContext) -> Vec<String> {
    ctx.mailer
        .as_ref()
        .expect("mailer stub tidak aktif")
        .deliveries()
        .messages
        .iter()
        .map(|m| m.replace("=\r\n", "").replace("=3D", "="))
        .collect()
}

/// Email yang dikirim ke `to` (header `To:`).
#[must_use]
pub fn mail_to(ctx: &AppContext, to: &str) -> Vec<String> {
    sent_mail(ctx)
        .into_iter()
        .filter(|m| m.lines().any(|l| l.starts_with("To:") && l.contains(to)))
        .collect()
}

/// Token mentah di link `<path>?token=<token>` di dalam `message`, bila ada.
#[must_use]
pub fn token_in(message: &str, path: &str) -> Option<String> {
    let marker = format!("{path}?token=");
    let start = message.find(&marker)? + marker.len();
    let token: String = message[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (!token.is_empty()).then_some(token)
}

/// Token mentah di link undangan `/invitations/<token>` di dalam `message`, bila ada (token
/// ada di path, bukan query string).
#[must_use]
pub fn invitation_token_in(message: &str) -> Option<String> {
    let marker = "/invitations/";
    let start = message.find(marker)? + marker.len();
    let token: String = message[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (!token.is_empty()).then_some(token)
}
