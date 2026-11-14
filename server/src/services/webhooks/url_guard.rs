//! Penjaga URL webhook keluar (cegah SSRF). Server Agent Gate memanggil URL pilihan user, jadi
//! URL yang menunjuk jaringan internal (loopback, privat, link-local termasuk metadata cloud
//! `169.254.169.254`, CGNAT, ULA IPv6) ditolak dua kali:
//!
//! 1. saat disimpan ([`validate`]): skema `https`, atau `http` hanya ke loopback bila
//!    `settings.api.webhook_allow_loopback` (development/test); IP literal terlarang ditolak;
//! 2. saat dikirim ([`resolve`]): nama host di-resolve, **semua** alamatnya diperiksa, lalu
//!    klien HTTP dipaku ke alamat hasil resolve itu (cegah DNS rebinding antara cek dan kirim).
//!    Redirect tidak diikuti.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use reqwest::Url;

pub const URL_INVALID: &str = "webhook-url-invalid";
pub const URL_FORBIDDEN: &str = "webhook-url-forbidden";
/// Panjang URL maksimal (CHECK `webhooks.url`).
pub const URL_MAX_LEN: usize = 2_000;

/// Host yang dianggap loopback walau berupa nama.
fn is_loopback_name(host: &str) -> bool {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    h == "localhost" || h.ends_with(".localhost")
}

/// Alamat yang tidak boleh dituju webhook. Loopback dikecualikan bila `allow_loopback`.
#[must_use]
pub fn is_forbidden(ip: IpAddr, allow_loopback: bool) -> bool {
    if ip.is_loopback() {
        return !allow_loopback;
    }
    match ip {
        IpAddr::V4(v4) => forbidden_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return if v4.is_loopback() {
                    !allow_loopback
                } else {
                    forbidden_v4(v4)
                };
            }
            forbidden_v6(v6)
        }
    }
}

fn forbidden_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        // 0.0.0.0/8, CGNAT 100.64.0.0/10, benchmark 198.18.0.0/15, cadangan 240.0.0.0/4.
        || a == 0
        || (a == 100 && (64..128).contains(&b))
        || (a == 198 && (b == 18 || b == 19))
        || a >= 240
}

fn forbidden_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        // ULA fc00::/7, link-local fe80::/10, site-local fec0::/10, dokumentasi 2001:db8::/32.
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
        || (first & 0xffc0) == 0xfec0
        || (first == 0x2001 && ip.segments()[1] == 0x0db8)
}

/// Validasi URL saat disimpan. Kembalikan URL ter-parse atau kode error Fluent.
///
/// # Errors
///
/// [`URL_INVALID`] (bukan URL http(s) dengan host, terlalu panjang, membawa kredensial) atau
/// [`URL_FORBIDDEN`] (alamat internal).
pub fn validate(raw: &str, allow_loopback: bool) -> Result<Url, &'static str> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > URL_MAX_LEN {
        return Err(URL_INVALID);
    }
    let url = Url::parse(raw).map_err(|_| URL_INVALID)?;
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(URL_INVALID);
    }
    let host = url.host_str().ok_or(URL_INVALID)?.to_string();
    let loopback_target =
        is_loopback_name(&host) || literal_ip(&url).is_some_and(|ip| ip.is_loopback());
    match url.scheme() {
        "https" => {}
        // http hanya untuk server lokal saat development/test.
        "http" if allow_loopback && loopback_target => {}
        "http" => return Err(URL_INVALID),
        _ => return Err(URL_INVALID),
    }
    if is_loopback_name(&host) && !allow_loopback {
        return Err(URL_FORBIDDEN);
    }
    if let Some(ip) = literal_ip(&url)
        && is_forbidden(ip, allow_loopback)
    {
        return Err(URL_FORBIDDEN);
    }
    Ok(url)
}

/// IP literal di host URL, bila host-nya IP.
fn literal_ip(url: &Url) -> Option<IpAddr> {
    let host = url.host_str()?;
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .ok()
}

/// Resolve host URL dan pastikan semua alamatnya boleh dituju. Kembalikan (host, alamat) untuk
/// dipaku ke klien HTTP.
///
/// # Errors
///
/// Kode error pendek untuk log pengiriman: `url_invalid`, `url_forbidden`, `dns_failed`.
pub async fn resolve(
    raw: &str,
    allow_loopback: bool,
) -> Result<(String, SocketAddr), &'static str> {
    let url = validate(raw, allow_loopback).map_err(|code| {
        if code == URL_FORBIDDEN {
            "url_forbidden"
        } else {
            "url_invalid"
        }
    })?;
    let host = url.host_str().ok_or("url_invalid")?.to_string();
    let port = url.port_or_known_default().ok_or("url_invalid")?;
    if let Some(ip) = literal_ip(&url) {
        return Ok((host, SocketAddr::new(ip, port)));
    }
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|_| "dns_failed")?
        .collect();
    let first = *addrs.first().ok_or("dns_failed")?;
    if addrs.iter().any(|a| is_forbidden(a.ip(), allow_loopback)) {
        return Err("url_forbidden");
    }
    Ok((host, first))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_rejects_internal_targets() {
        for url in [
            "http://example.com/hook",
            "https://127.0.0.1/hook",
            "https://localhost/hook",
            "https://10.0.0.5/hook",
            "https://192.168.1.10/hook",
            "https://172.16.0.1/hook",
            "https://169.254.169.254/latest/meta-data",
            "https://100.64.0.1/x",
            "https://0.0.0.0/x",
            "https://[::1]/x",
            "https://[fd00::1]/x",
            "https://[fe80::1]/x",
            "https://[::ffff:10.0.0.1]/x",
            "ftp://example.com/x",
            "https://user:pass@example.com/x",
            "not a url",
        ] {
            assert!(validate(url, false).is_err(), "{url}");
        }
        assert!(validate("https://hooks.example.com/agentgate?x=1", false).is_ok());
        assert!(validate("https://8.8.8.8/hook", false).is_ok());
    }

    #[test]
    fn development_allows_only_loopback_http() {
        assert!(validate("http://127.0.0.1:8080/hook", true).is_ok());
        assert!(validate("http://localhost:8080/hook", true).is_ok());
        assert!(validate("http://[::1]:8080/hook", true).is_ok());
        assert_eq!(validate("http://example.com/hook", true), Err(URL_INVALID));
        assert_eq!(validate("https://10.0.0.1/hook", true), Err(URL_FORBIDDEN));
        assert_eq!(
            validate("https://169.254.169.254/x", true),
            Err(URL_FORBIDDEN)
        );
    }

    #[tokio::test]
    async fn resolve_rejects_names_pointing_inside() {
        assert_eq!(
            resolve("https://localhost/x", false).await,
            Err("url_forbidden")
        );
        assert!(resolve("http://127.0.0.1:9/x", true).await.is_ok());
    }
}
