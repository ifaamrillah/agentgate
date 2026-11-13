//! Relasi lintas repo satu project: HTTP call ↔ route, event publish ↔ subscribe, dan shared
//! package (manifest). Masukan = [`Surface`] tiap repo; keluaran merujuk indeks node per repo.
use serde::{Deserialize, Serialize};

use crate::build::Surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    Http,
    Event,
    Package,
}

impl LinkKind {
    pub const ALL: [Self; 3] = [Self::Http, Self::Event, Self::Package];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Event => "event",
            Self::Package => "package",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// `certain` = cocok persis dari hasil parsing, `inferred` = cocok lewat parameter/template,
/// `unmatched` = call ke path yang tidak punya route di project, `external` = host luar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkConfidence {
    Certain,
    Inferred,
    Unmatched,
    External,
}

impl LinkConfidence {
    pub const ALL: [Self; 4] = [
        Self::Certain,
        Self::Inferred,
        Self::Unmatched,
        Self::External,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Certain => "certain",
            Self::Inferred => "inferred",
            Self::Unmatched => "unmatched",
            Self::External => "external",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub kind: LinkKind,
    pub confidence: LinkConfidence,
    pub from_repo: usize,
    pub from_node: Option<u32>,
    pub to_repo: Option<usize>,
    pub to_node: Option<u32>,
    /// `POST /v1/pay`, topik event, nama paket, atau host.
    pub detail: String,
}

fn segs(p: &str) -> Vec<&str> {
    p.split('/').filter(|s| !s.is_empty()).collect()
}

/// Cocok path: `Some(true)` persis, `Some(false)` lewat parameter.
fn path_match(call: &str, route: &str) -> Option<bool> {
    let (a, b) = (segs(call), segs(route));
    if a.len() != b.len() {
        return None;
    }
    let mut exact = true;
    for (x, y) in a.iter().zip(&b) {
        if x == y {
            continue;
        }
        if *x == ":p" || *y == ":p" {
            exact = exact && *x == ":p" && *y == ":p";
            continue;
        }
        return None;
    }
    Some(exact)
}

/// Hubungkan permukaan semua repo (`(id repo, surface)`).
#[must_use]
pub fn link(repos: &[(usize, &Surface)]) -> Vec<Link> {
    let mut out = Vec::new();
    let any_route = repos.iter().any(|(_, s)| !s.routes.is_empty());
    for &(ra, sa) in repos {
        for h in &sa.http {
            if !h.host.is_empty() {
                out.push(Link {
                    kind: LinkKind::Http,
                    confidence: LinkConfidence::External,
                    from_repo: ra,
                    from_node: Some(h.node),
                    to_repo: None,
                    to_node: None,
                    detail: format!("{} {}{}", h.method, h.host, h.path),
                });
                continue;
            }
            let mut matched = false;
            for &(rb, sb) in repos {
                for r in &sb.routes {
                    let method_ok = r.method == "ANY" || h.method == r.method;
                    if !method_ok {
                        continue;
                    }
                    if let Some(exact) = path_match(&h.path, &r.path) {
                        matched = true;
                        out.push(Link {
                            kind: LinkKind::Http,
                            confidence: if exact && h.exact {
                                LinkConfidence::Certain
                            } else {
                                LinkConfidence::Inferred
                            },
                            from_repo: ra,
                            from_node: Some(h.node),
                            to_repo: Some(rb),
                            to_node: Some(r.node),
                            detail: format!("{} {}", r.method, r.path),
                        });
                    }
                }
            }
            if !matched && any_route {
                out.push(Link {
                    kind: LinkKind::Http,
                    confidence: LinkConfidence::Unmatched,
                    from_repo: ra,
                    from_node: Some(h.node),
                    to_repo: None,
                    to_node: None,
                    detail: format!("{} {}", h.method, h.path),
                });
            }
        }
        for e in sa.events.iter().filter(|e| e.publish) {
            for &(rb, sb) in repos {
                for s in sb
                    .events
                    .iter()
                    .filter(|s| !s.publish && s.topic == e.topic)
                {
                    out.push(Link {
                        kind: LinkKind::Event,
                        confidence: LinkConfidence::Certain,
                        from_repo: ra,
                        from_node: Some(e.node),
                        to_repo: Some(rb),
                        to_node: Some(s.node),
                        detail: e.topic.clone(),
                    });
                }
            }
        }
        for dep in &sa.deps {
            for &(rb, sb) in repos {
                if rb != ra && sb.packages.contains(dep) {
                    out.push(Link {
                        kind: LinkKind::Package,
                        confidence: LinkConfidence::Certain,
                        from_repo: ra,
                        from_node: None,
                        to_repo: Some(rb),
                        to_node: None,
                        detail: dep.clone(),
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{SurfaceEvent, SurfaceHttp, SurfaceRoute};

    fn http(node: u32, method: &str, path: &str, exact: bool) -> SurfaceHttp {
        SurfaceHttp {
            node,
            method: method.into(),
            path: path.into(),
            exact,
            host: String::new(),
        }
    }

    #[test]
    fn links_http_events_packages() {
        let web = Surface {
            http: vec![
                http(1, "POST", "/v1/payments/qris", true),
                http(2, "GET", "/v1/stock/:p", false),
                http(3, "GET", "/v1/report/csv", true),
                SurfaceHttp {
                    node: 4,
                    method: "POST".into(),
                    path: "/v2/qr".into(),
                    exact: true,
                    host: "api.xendit.co".into(),
                },
            ],
            deps: vec!["@acme/printer".into(), "react".into()],
            ..Surface::default()
        };
        let api = Surface {
            routes: vec![
                SurfaceRoute {
                    node: 10,
                    method: "POST".into(),
                    path: "/v1/payments/qris".into(),
                },
                SurfaceRoute {
                    node: 11,
                    method: "GET".into(),
                    path: "/v1/stock/:p".into(),
                },
            ],
            events: vec![SurfaceEvent {
                node: 12,
                topic: "payment.settled".into(),
                publish: true,
            }],
            ..Surface::default()
        };
        let printer = Surface {
            packages: vec!["@acme/printer".into()],
            events: vec![SurfaceEvent {
                node: 20,
                topic: "payment.settled".into(),
                publish: false,
            }],
            ..Surface::default()
        };
        let links = link(&[(0, &web), (1, &api), (2, &printer)]);
        let got: Vec<String> = links
            .iter()
            .map(|l| {
                format!(
                    "{}:{} {}→{:?} {}",
                    l.kind.as_str(),
                    l.confidence.as_str(),
                    l.from_repo,
                    l.to_repo,
                    l.detail
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                "http:certain 0→Some(1) POST /v1/payments/qris",
                "http:inferred 0→Some(1) GET /v1/stock/:p",
                "http:unmatched 0→None GET /v1/report/csv",
                "http:external 0→None POST api.xendit.co/v2/qr",
                "package:certain 0→Some(2) @acme/printer",
                "event:certain 1→Some(2) payment.settled",
            ]
        );
    }
}
