//! Who may talk to the server. Every request passes [`guard`] before it reaches a route.
//!
//! ```text
//!  browser tab on this Mac ──▶ Host is localhost / an IP? ──▶ same-origin POST? ──▶ routes
//!  other website (DNS rebinding) ─▶ Host is evil.example ✕ 421
//!  other website (form POST)      ─▶ Origin / Sec-Fetch-Site cross-site ✕ 403
//!  tailnet via `tailscale serve`  ─▶ Host *.ts.net ─▶ /api/ingest/* only, unless
//!                                     Tailscale-User-Login is WORTHWATCH_TAILSCALE_OWNER ✕ 403
//! ```
//!
//! The checks are pure ([`check`]) so tests cover them without a socket. They protect the
//! default setup, where the server listens on localhost. LAN mode (`WORTHWATCH_BIND=0.0.0.0`)
//! still lets anyone on the network in: there is no login.

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// What the environment allows beyond localhost.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    /// `WORTHWATCH_ALLOWED_HOSTS`: extra host names, comma separated (`mac-mini.local`).
    pub extra_hosts: Vec<String>,
    /// `WORTHWATCH_TAILSCALE_OWNER`: Tailscale logins that may open the whole app over
    /// `tailscale serve`. Empty: the tailnet reaches `/api/ingest/*` only.
    pub tailscale_owners: Vec<String>,
}

impl Policy {
    pub fn from_env() -> Self {
        let list = |k: &str| -> Vec<String> {
            std::env::var(k)
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().trim_end_matches('.').to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        };
        Policy {
            extra_hosts: list("WORTHWATCH_ALLOWED_HOSTS"),
            tailscale_owners: list("WORTHWATCH_TAILSCALE_OWNER"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// The Host header names a site that isn't this server: DNS rebinding, or a typo.
    BadHost,
    /// A state-changing request, or an API read, started by another site.
    CrossSite,
    /// Over Tailscale without an owner login: only the Apple Pay ingest route is open.
    TailnetIngestOnly,
}

/// `example.com:7777` -> `example.com`, `[::1]:7777` -> `::1`. Lowercase, no trailing dot.
fn host_name(authority: &str) -> String {
    let a = authority.trim();
    let name = if let Some(rest) = a.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else if a.matches(':').count() > 1 {
        a // a bare IPv6 address
    } else {
        a.split(':').next().unwrap_or_default()
    };
    name.trim_end_matches('.').to_ascii_lowercase()
}

fn is_tailnet(name: &str) -> bool {
    name.ends_with(".ts.net")
}

fn host_ok(p: &Policy, name: &str) -> bool {
    // An IP address can't be rebound: the browser talks to that address and no other.
    name == "localhost"
        || name.parse::<std::net::IpAddr>().is_ok()
        || is_tailnet(name)
        || p.extra_hosts.iter().any(|h| h == name)
}

fn header<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|v| v.to_str().ok())
}

/// Decide one request from its method, path and headers.
pub fn check(p: &Policy, method: &Method, path: &str, h: &HeaderMap) -> Verdict {
    let host = header(h, "host").map(host_name);
    // `tailscale serve` keeps the original Host; other proxies say it in X-Forwarded-Host.
    let fwd = header(h, "x-forwarded-host").map(host_name);
    for name in host.iter().chain(fwd.iter()) {
        if !host_ok(p, name) {
            return Verdict::BadHost;
        }
    }

    let api = path.starts_with("/api/");
    let ingest = path.starts_with("/api/ingest/");
    let via_tailnet = host.iter().chain(fwd.iter()).any(|n| is_tailnet(n))
        || h.contains_key("tailscale-funnel-request")
        || h.contains_key("tailscale-user-login");
    if via_tailnet && !ingest {
        let funnel = h.contains_key("tailscale-funnel-request");
        let login = header(h, "tailscale-user-login").map(|l| l.trim().to_ascii_lowercase());
        let owner = login.is_some_and(|l| p.tailscale_owners.contains(&l));
        if funnel || !owner {
            return Verdict::TailnetIngestOnly;
        }
    }

    let safe = matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);
    // Browsers since 2023 say where a request came from. Other sites may link to the app
    // (a top-level visit), but not call its API or change anything.
    if let Some(site) = header(h, "sec-fetch-site") {
        let same = site == "same-origin" || site == "none";
        if !same && (api || !safe) {
            return Verdict::CrossSite;
        }
    }
    if !safe {
        if let Some(origin) = header(h, "origin") {
            let from = origin.split_once("://").map(|(_, a)| a.trim_end_matches('/').to_ascii_lowercase());
            let to = header(h, "host").map(|a| a.trim().to_ascii_lowercase());
            let to_fwd = header(h, "x-forwarded-host").map(|a| a.trim().to_ascii_lowercase());
            let matches = from.is_some_and(|f| Some(&f) == to.as_ref() || Some(&f) == to_fwd.as_ref());
            if !matches {
                return Verdict::CrossSite;
            }
        }
    }
    // No Origin and no Sec-Fetch-Site: not a browser (curl, the MCP bridge, the Shortcut).
    Verdict::Allow
}

/// Headers on every response: no framing (clickjacking an Approve button), no sniffing,
/// no referrer, scripts only from this server, API answers never cached.
pub fn harden(path: &str, h: &mut HeaderMap) {
    let set = |h: &mut HeaderMap, k: header::HeaderName, v: &'static str| {
        h.insert(k, HeaderValue::from_static(v));
    };
    set(h, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(h, header::X_FRAME_OPTIONS, "DENY");
    set(h, header::REFERRER_POLICY, "no-referrer");
    set(
        h,
        header::CONTENT_SECURITY_POLICY,
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
         font-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; \
         frame-ancestors 'none'",
    );
    h.insert("cross-origin-opener-policy", HeaderValue::from_static("same-origin"));
    h.insert("cross-origin-resource-policy", HeaderValue::from_static("same-origin"));
    h.insert("permissions-policy", HeaderValue::from_static("camera=(), microphone=(), geolocation=()"));
    if path.starts_with("/api/") {
        set(h, header::CACHE_CONTROL, "no-store");
    }
}

fn refuse(v: Verdict) -> Response {
    let (status, message) = match v {
        Verdict::BadHost => (
            StatusCode::MISDIRECTED_REQUEST,
            "This address isn't allowed. Open Worthwatch at http://127.0.0.1:7777, or add the name to WORTHWATCH_ALLOWED_HOSTS.",
        ),
        Verdict::CrossSite => (StatusCode::FORBIDDEN, "Another website tried to use Worthwatch. Nothing was changed."),
        Verdict::TailnetIngestOnly => (
            StatusCode::FORBIDDEN,
            "Over Tailscale, only the Apple Pay feed is open. To open the dashboard here, set WORTHWATCH_TAILSCALE_OWNER to your Tailscale login.",
        ),
        Verdict::Allow => (StatusCode::OK, ""),
    };
    (status, axum::Json(serde_json::json!({ "ok": false, "message": message }))).into_response()
}

/// The middleware: refuse, or run the route and harden its response.
pub async fn guard(State(p): State<std::sync::Arc<Policy>>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let verdict = check(&p, req.method(), &path, req.headers());
    let mut res = if verdict == Verdict::Allow {
        next.run(req).await
    } else {
        let host: String = header(req.headers(), "host").unwrap_or("").chars().take(80).collect();
        tracing::warn!("refused {} {path} (host {host:?}): {verdict:?}", req.method());
        refuse(verdict)
    };
    harden(&path, res.headers_mut());
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.insert(*k, HeaderValue::from_static(v));
        }
        m
    }

    fn run(p: &Policy, m: Method, path: &str, pairs: &[(&'static str, &'static str)]) -> Verdict {
        check(p, &m, path, &h(pairs))
    }

    #[test]
    fn localhost_and_ip_hosts_pass_and_rebinding_names_do_not() {
        let p = Policy::default();
        for host in ["127.0.0.1:7777", "localhost:7777", "[::1]:7777", "192.168.1.20:7777", "LOCALHOST"] {
            assert_eq!(run(&p, Method::GET, "/api/view", &[("host", host)]), Verdict::Allow, "{host}");
        }
        for host in ["attacker.example:7777", "localhost.attacker.example", "127.0.0.1.nip.io:7777"] {
            assert_eq!(run(&p, Method::GET, "/api/view", &[("host", host)]), Verdict::BadHost, "{host}");
        }
        assert_eq!(
            run(&p, Method::GET, "/", &[("host", "127.0.0.1:7777"), ("x-forwarded-host", "evil.example")]),
            Verdict::BadHost
        );
    }

    #[test]
    fn extra_hosts_come_from_the_environment_list() {
        let p = Policy { extra_hosts: vec!["mac-mini.local".into()], ..Policy::default() };
        assert_eq!(run(&p, Method::GET, "/", &[("host", "Mac-Mini.local.:7777")]), Verdict::Allow);
    }

    #[test]
    fn cross_site_posts_and_api_reads_are_refused_same_origin_ones_pass() {
        let p = Policy::default();
        let local = ("host", "127.0.0.1:7777");
        // A form POST from another site: no body, no JSON, still refused.
        assert_eq!(
            run(&p, Method::POST, "/api/pending/1/approve", &[local, ("origin", "https://attacker.example")]),
            Verdict::CrossSite
        );
        assert_eq!(
            run(&p, Method::POST, "/api/backups", &[local, ("sec-fetch-site", "cross-site")]),
            Verdict::CrossSite
        );
        assert_eq!(
            run(&p, Method::POST, "/api/settings/token", &[local, ("origin", "null")]),
            Verdict::CrossSite
        );
        // Another port on localhost is another site too.
        assert_eq!(
            run(&p, Method::POST, "/api/apply", &[local, ("origin", "http://127.0.0.1:8000")]),
            Verdict::CrossSite
        );
        assert_eq!(
            run(&p, Method::GET, "/api/view", &[local, ("sec-fetch-site", "same-site")]),
            Verdict::CrossSite
        );
        // The app itself.
        assert_eq!(
            run(
                &p,
                Method::POST,
                "/api/apply",
                &[local, ("origin", "http://127.0.0.1:7777"), ("sec-fetch-site", "same-origin")]
            ),
            Verdict::Allow
        );
        // A link from another site to the dashboard still opens it.
        assert_eq!(run(&p, Method::GET, "/", &[local, ("sec-fetch-site", "cross-site")]), Verdict::Allow);
        // curl, the MCP bridge and the Shortcut send neither header.
        assert_eq!(run(&p, Method::POST, "/api/propose", &[local]), Verdict::Allow);
    }

    #[test]
    fn the_tailnet_reaches_ingest_only_unless_the_owner_is_logged_in() {
        let ts = ("host", "mac-mini.tail1234.ts.net");
        let p = Policy::default();
        assert_eq!(run(&p, Method::POST, "/api/ingest/transaction", &[ts]), Verdict::Allow);
        assert_eq!(run(&p, Method::GET, "/api/view", &[ts]), Verdict::TailnetIngestOnly);
        assert_eq!(
            run(&p, Method::GET, "/", &[ts, ("tailscale-user-login", "zai@example.com")]),
            Verdict::TailnetIngestOnly
        );

        let p = Policy { tailscale_owners: vec!["zai@example.com".into()], ..Policy::default() };
        assert_eq!(
            run(&p, Method::GET, "/", &[ts, ("tailscale-user-login", "Zai@Example.com")]),
            Verdict::Allow
        );
        assert_eq!(
            run(&p, Method::GET, "/", &[ts, ("tailscale-user-login", "someone@else.com")]),
            Verdict::TailnetIngestOnly
        );
        // Funnel is the public internet: never more than ingest.
        assert_eq!(
            run(
                &p,
                Method::GET,
                "/",
                &[ts, ("tailscale-user-login", "zai@example.com"), ("tailscale-funnel-request", "?1")]
            ),
            Verdict::TailnetIngestOnly
        );
        // A proxy that rewrites Host to localhost still says it came over the tailnet.
        assert_eq!(
            run(
                &p,
                Method::GET,
                "/api/view",
                &[("host", "127.0.0.1:7777"), ("x-forwarded-host", "mac-mini.tail1234.ts.net")]
            ),
            Verdict::TailnetIngestOnly
        );
    }

    #[test]
    fn host_names_parse_ports_and_ipv6() {
        assert_eq!(host_name("[::1]:7777"), "::1");
        assert_eq!(host_name("::1"), "::1");
        assert_eq!(host_name("Example.COM.:80"), "example.com");
    }
}
