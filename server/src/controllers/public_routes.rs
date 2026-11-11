//! Daftar route publik, satu-satunya (rules §7 Gate). Semua route lain butuh sesi login: gate
//! (`middleware::gate`) menolak request tanpa sesi, dan handler tetap memakai `CurrentUser`.

/// Path yang cocok persis.
const EXACT: &[&str] = &[
    "/login",
    // Kode 2FA: sesi pending belum punya user_id (M1 Task 6).
    "/login/2fa",
    "/register",
    // "Cek email kamu" setelah daftar, lupa password, atau kirim ulang verifikasi.
    "/check-email",
    "/verify-email",
    // Kirim ulang verifikasi: sesi menunggu verifikasi belum punya user_id.
    "/verify-email/resend",
    // Logout selalu lolos gate, apa pun state sesinya.
    "/logout",
    // Health check bawaan Loco.
    "/_ping",
    "/_health",
];

/// Prefix (tanpa `/` akhir): cocok untuk path itu sendiri dan semua path di bawahnya.
const PREFIXES: &[&str] = &[
    // Lupa dan reset password.
    "/password",
    // OAuth: redirect ke provider dan callback-nya (M1 Task 5).
    "/auth",
    // Halaman undangan dibuka sebelum login (M1 Task 8).
    "/invitations",
    "/static",
    // Katalog UI, hanya ada di build debug (development/test).
    "/_dev",
];

#[must_use]
pub fn is_public(path: &str) -> bool {
    EXACT.contains(&path)
        || PREFIXES.iter().any(|prefix| {
            path.strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_routes_are_public() {
        for path in [
            "/login",
            "/login/2fa",
            "/register",
            "/verify-email",
            "/verify-email/resend",
            "/check-email",
            "/logout",
            "/_ping",
            "/_health",
            "/password/forgot",
            "/password/reset",
            "/auth/github/callback",
            "/invitations/abc",
            "/static/css/app.css",
            "/_dev/ui",
        ] {
            assert!(is_public(path), "{path}");
        }
    }

    #[test]
    fn everything_else_is_protected() {
        for path in [
            "/",
            "/projects",
            "/login/other",
            "/loginx",
            "/passwords",
            "/authx",
            "/static-evil",
            "/_devx",
            "/register/x",
        ] {
            assert!(!is_public(path), "{path}");
        }
    }
}
