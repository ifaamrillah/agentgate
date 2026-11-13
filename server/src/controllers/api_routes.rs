//! Daftar path API, satu-satunya: endpoint yang dipanggil klien non-browser (client MCP, runner,
//! integrasi) dan selalu membalas JSON. Layer `html_errors` tidak mengubah respons path ini
//! menjadi halaman HTML; error tanpa body JSON diberi body `{"error": "..."}` (bentuk RFC 6749).
//!
//! `/oauth/authorize` sengaja tidak di sini: itu halaman consent di browser.

/// Path yang cocok persis.
const EXACT: &[&str] = &[
    "/mcp",
    "/oauth/token",
    "/oauth/register",
    "/oauth/revoke",
    "/oauth/device_authorization",
];

/// Prefix (tanpa `/` akhir): path itu sendiri dan semua path di bawahnya.
const PREFIXES: &[&str] = &[
    "/mcp",
    "/oauth/token",
    "/oauth/register",
    "/oauth/revoke",
    "/oauth/device_authorization",
    // Metadata OAuth (RFC 8414, RFC 9728).
    "/.well-known",
    // API publik (M11).
    "/api",
    // Webhook provider git (M6), diautentikasi tanda tangan HMAC.
    "/webhooks",
];

#[must_use]
pub fn is_api(path: &str) -> bool {
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
    fn api_paths() {
        for path in [
            "/mcp",
            "/oauth/token",
            "/oauth/register",
            "/oauth/revoke",
            "/oauth/device_authorization",
            "/api/runner/ws",
            "/.well-known/oauth-authorization-server",
            "/api/v1/tickets",
            "/webhooks/github/0198",
        ] {
            assert!(is_api(path), "{path}");
        }
        for path in [
            "/oauth/authorize",
            "/device",
            "/mcpx",
            "/apis",
            "/",
            "/w/acme",
        ] {
            assert!(!is_api(path), "{path}");
        }
    }
}
