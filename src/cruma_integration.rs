use std::collections::{BTreeMap, HashMap, HashSet};
use std::num::NonZeroU16;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use bytes::Bytes;
use cruma_proxy_lib::hyper::body::Frame;
use cruma_proxy_lib::types::*;
use cruma_tunnels_lib::hostname::HostName;
use http_body_util::StreamBody;
use tokio_stream::wrappers::ReceiverStream;

use crate::configuration::{ConfigWrapper, v4};
use crate::docker::ContainerProxyTarget;
use crate::global_state::GlobalState;
use crate::global_state::ProcState;
use crate::process_hosting::ProcessRegistry;

const DEFAULT_404_HTML: &[u8] = include_bytes!("assets/404.html");
const DEFAULT_STARTING_HTML: &str = include_str!("assets/starting.html");

/// The ID used for the shared "backend offline" hyper handler.
const OFFLINE_HANDLER_ID: &str = "odd-box::backend-offline";

/// If the backend process is stopped or faulty, auto-start it by setting
/// its state to Starting and enabling it — same as `http_events.rs` does.
fn try_auto_start(state: &GlobalState, backend_id: &str) {
    let registry = &state.process_registry;
    let Some(proc_state) = registry.snapshot().state_of(backend_id) else {
        return;
    };
    if matches!(proc_state, ProcState::Stopped | ProcState::Faulty) {
        tracing::info!(
            backend_id = %backend_id,
            "auto-starting process from offline handler due to incoming request"
        );
        registry.update_state(
            backend_id,
            ProcState::Starting,
            None,
            None,
            Some(true),
            None,
            None,
            None,
            None,
        );
    }
}

/// Extract request host robustly across protocol/proxy variants.
/// Some ingress/proxy paths may not populate the plain `Host` header (for
/// example when authority is carried elsewhere), so we fall back through
/// forwarded headers and URI authority to avoid spurious `unknown` backend
/// lookups on the pending-start page.
fn extract_request_host<B>(req: &cruma_proxy_lib::hyper::Request<B>) -> String {
    fn header_host<B>(req: &cruma_proxy_lib::hyper::Request<B>, name: &str) -> Option<String> {
        let raw = req.headers().get(name)?.to_str().ok()?.trim();
        if raw.is_empty() {
            return None;
        }
        // Forwarded headers may contain a list; use the original host.
        let first = raw.split(',').next().unwrap_or(raw).trim();
        if first.is_empty() {
            None
        } else {
            Some(first.to_string())
        }
    }

    header_host(req, "host")
        .or_else(|| header_host(req, "x-forwarded-host"))
        .or_else(|| header_host(req, "x-original-host"))
        .or_else(|| req.uri().authority().map(|a| a.as_str().to_string()))
        .or_else(|| req.uri().host().map(|h| h.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

#[derive(Debug, Clone)]
struct OfflineBackendStatus {
    backend_id: String,
    status: String,
    ready_for_requests: bool,
}

/// Probe the backend's active port directly before asking the browser to retry.
/// We only gate process backends with this probe; non-process backends are
/// considered ready based on their static state (Remote/DirServer/Docker).
async fn tcp_port_ready(port: u16) -> bool {
    let connect = tokio::net::TcpStream::connect(("127.0.0.1", port));
    matches!(
        tokio::time::timeout(Duration::from_millis(250), connect).await,
        Ok(Ok(_))
    )
}

/// Resolve backend status for the offline page/SSE flow.
/// For process backends, this combines registry state with an active-port TCP
/// probe so we only trigger reload when the process is actually accepting
/// connections.
async fn resolve_offline_backend_status(
    state: &GlobalState,
    backend_id: Option<&str>,
) -> OfflineBackendStatus {
    let Some(id) = backend_id else {
        return OfflineBackendStatus {
            backend_id: "unknown".to_string(),
            status: "Not found".to_string(),
            ready_for_requests: false,
        };
    };

    let snapshot = state.process_registry.snapshot();
    let Some(handle) = snapshot.get(id) else {
        return OfflineBackendStatus {
            backend_id: id.to_string(),
            status: "Unknown".to_string(),
            ready_for_requests: false,
        };
    };

    let proc_state = handle.proc_state();
    let process_state = handle.state();

    // Non-process backends do not have a local process port to probe.
    if matches!(
        proc_state,
        ProcState::Remote | ProcState::Docker | ProcState::DirServer
    ) {
        return OfflineBackendStatus {
            backend_id: id.to_string(),
            status: format!("{:?}", proc_state),
            ready_for_requests: true,
        };
    }

    // For local processes, confirm the active port accepts a TCP handshake.
    // This avoids reloading too early while the process is still warming up.
    if matches!(proc_state, ProcState::Running | ProcState::Starting) {
        if let Some(port) = process_state.active_port {
            let port_ready = tcp_port_ready(port).await;
            let status_suffix = if port_ready { "ready" } else { "warming up" };
            return OfflineBackendStatus {
                backend_id: id.to_string(),
                status: format!("{:?} (port {} {})", proc_state, port, status_suffix),
                ready_for_requests: port_ready,
            };
        }

        return OfflineBackendStatus {
            backend_id: id.to_string(),
            status: format!("{:?} (waiting for active port)", proc_state),
            ready_for_requests: false,
        };
    }

    OfflineBackendStatus {
        backend_id: id.to_string(),
        status: format!("{:?}", proc_state),
        ready_for_requests: false,
    }
}

/// Build an SSE streaming response that monitors backend state and tells the
/// browser to reload once the backend comes online (or periodically sends
/// status updates so the page can show live info).
fn sse_response(
    state: Arc<GlobalState>,
    request_host: String,
) -> cruma_proxy_lib::hyper::Response<HyperResponseBody> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Frame<Bytes>, HyperHandlerError>>(4);

    tokio::spawn(async move {
        // Send an initial comment to flush connection headers
        let _ = tx
            .send(Ok(Frame::data(Bytes::from(": connected\n\n"))))
            .await;

        let mut last_status = String::new();
        let mut tried_auto_start = false;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;

            let cfg = state.config.load();
            let backend_id = lookup_backend_for_host(&cfg, &request_host);

            // On the first tick, try to auto-start a stopped/faulty process
            if !tried_auto_start {
                tried_auto_start = true;
                if let Some(id) = &backend_id {
                    try_auto_start(&state, id);
                }
            }

            let status = resolve_offline_backend_status(&state, backend_id.as_deref()).await;

            if status.ready_for_requests {
                // Backend is up – tell the browser to reload
                let msg = format!("event: reload\ndata: {{}}\n\n");
                let _ = tx.send(Ok(Frame::data(Bytes::from(msg)))).await;
                break;
            }

            // Only send a status event when the status text actually changed
            if status.status != last_status {
                last_status = status.status.clone();
                let data = format!(
                    "event: status\ndata: {{\"backend_id\":\"{}\",\"status\":\"{}\"}}\n\n",
                    status.backend_id, status.status
                );
                if tx.send(Ok(Frame::data(Bytes::from(data)))).await.is_err() {
                    break; // client disconnected
                }
            }
        }
    });

    let stream = ReceiverStream::new(rx);
    use http_body_util::BodyExt as _;
    let body: HyperResponseBody = StreamBody::new(stream).boxed();

    cruma_proxy_lib::hyper::Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .header("x-accel-buffering", "no")
        .body(body)
        .expect("building SSE response should not fail")
}

/// Render the offline HTML template with the given placeholders filled in.
fn render_offline_html(backend_id: &str, status: &str, host: &str) -> String {
    DEFAULT_STARTING_HTML
        .replace("@@BACKEND_ID@@", backend_id)
        .replace("@@STATUS@@", status)
        .replace("@@HOST@@", host)
}

/// Look up the backend_id for a given request host from the config.
/// This is called dynamically on each request to ensure fresh data.
///
/// Uses [`HostName::to_host_pattern`] so that hostname pattern syntax
/// (`*.example.com`, `app-*`, `*`, etc.) is honoured consistently with
/// the proxy routing layer.
fn lookup_backend_for_host(cfg: &ConfigWrapper, request_host: &str) -> Option<String> {
    // Strip port if present
    let host = request_host
        .split(':')
        .next()
        .unwrap_or(request_host)
        .to_lowercase();

    // Check HTTP frontend routes
    if let Some(http_frontend) = &cfg.frontends.http {
        for (route_host, target) in &http_frontend.routes {
            if host_pattern_matches(route_host, target.capture_subdomains(), &host) {
                return Some(target.backend_id().to_string());
            }
        }
    }

    // Check HTTPS frontend routes (if explicit, not inherited)
    if let Some(https_frontend) = &cfg.frontends.https {
        if let Some(routes) = &https_frontend.routes {
            if let v4::HttpsRoutes::Explicit(route_map) = routes {
                for (route_host, target) in route_map {
                    if host_pattern_matches(route_host, target.capture_subdomains(), &host) {
                        return Some(target.backend_id().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Creates the shared HyperHandler for the "backend is offline" page.
///
/// This handler has access to:
/// - `state`: The global application state including process registry and config
///
/// The handler dynamically looks up the host-to-backend mapping from the current
/// config on each request, ensuring it always has fresh data even if backends
/// are added/removed without a full config rebuild.
fn create_offline_handler(state: Arc<GlobalState>) -> HyperHandler {
    Arc::new(move |req| {
        let state = state.clone();

        Box::pin(async move {
            // Extract the host from the request
            let request_host = extract_request_host(&req);

            // SSE endpoint – stream status updates to the browser
            if req.uri().path() == "/__odd-box-sse" {
                return Ok(sse_response(state, request_host));
            }

            // Look up which backend this host maps to (dynamically from current config)
            let cfg = state.config.load();
            let backend_id = lookup_backend_for_host(&cfg, &request_host);

            // Auto-start the process if it's stopped or faulty
            if let Some(id) = &backend_id {
                try_auto_start(&state, id);
            }

            let status = resolve_offline_backend_status(&state, backend_id.as_deref()).await;

            let body = if req
                .headers()
                .get("accept")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("text/html"))
                .unwrap_or(false)
            {
                render_offline_html(&status.backend_id, &status.status, &request_host)
            } else {
                format!(
                    "Service Unavailable: backend '{}' is {}",
                    status.backend_id, status.status
                )
            };

            let mut response = hyper_response_with_content_type(
                cruma_proxy_lib::hyper::StatusCode::SERVICE_UNAVAILABLE,
                "text/html; charset=utf-8",
                body,
            );

            response
                .headers_mut()
                .insert("Retry-After", "2".parse().unwrap());
            response
                .headers_mut()
                .insert("Cache-Control", "no-store".parse().unwrap());

            // Close this connection only once the backend is actually accepting
            // TCP connections. The browser then retries on a fresh connection.
            if status.ready_for_requests {
                // Internal signal consumed by cruma-proxy-lib to trigger a
                // per-connection graceful shutdown that also works for HTTP/2.
                response
                    .headers_mut()
                    .insert("x-cruma-force-close", "1".parse().unwrap());

                // Keep the standard HTTP/1.x hint as well.
                response
                    .headers_mut()
                    .insert("Connection", "close".parse().unwrap());
            }

            Ok(response)
        })
    })
}

/// Creates a simple offline handler without state access (for tests).
fn create_offline_handler_simple() -> HyperHandler {
    Arc::new(|req| {
        Box::pin(async move {
            let request_host = extract_request_host(&req);

            let body = render_offline_html("unknown", "Starting", &request_host);

            let mut response = hyper_response_with_content_type(
                cruma_proxy_lib::hyper::StatusCode::SERVICE_UNAVAILABLE,
                "text/html; charset=utf-8",
                body,
            );

            response
                .headers_mut()
                .insert("Retry-After", "2".parse().unwrap());
            response
                .headers_mut()
                .insert("Cache-Control", "no-store".parse().unwrap());

            Ok(response)
        })
    })
}

#[derive(Debug, Default, Clone)]
pub struct BuildNotes {
    /// Backends or routes that could not be fully wired up during this config
    /// build — e.g. a process backend that hasn't started yet and has no port,
    /// a remote backend with no valid endpoints, or a route referencing an
    /// unknown backend. These are typically transient conditions that resolve
    /// once the relevant process starts or the config is corrected.
    pub warnings: Vec<String>,
}

fn non_zero_port(port: u16, label: &str) -> anyhow::Result<NonZeroU16> {
    NonZeroU16::new(port).with_context(|| format!("{label} must be non-zero"))
}

/// Build the set of [`HostPattern`]s for a route.
///
/// Uses [`HostName::to_host_pattern`] from the cruma SDK so that the full
/// hostname-pattern syntax (`*.example.com`, `app-*`, `*`, etc.) is
/// honoured for **all** routes — not just cruma tunnel routes.
///
/// When the legacy `capture_subdomains` flag is set and the hostname does
/// not already contain a wildcard, an additional `*.host` deep-glob
/// pattern is emitted to preserve backward-compatible behaviour.
fn host_patterns_for_route(
    host: &str,
    capture_subdomains: bool,
    cruma_assigned_domain: Option<&str>,
) -> Vec<HostPattern> {
    let hn = HostName::new(host);
    // For local routing we pass &None – single-label expansion via an
    // assigned FQDN only happens for the cruma-domain companion patterns.
    let mut pats = vec![hn.to_host_pattern(&None)];

    // Legacy backward-compat: capture_subdomains adds a wildcard pattern
    // so that `example.com` + capture_subdomains matches both `example.com`
    // and `*.example.com` (equivalent to the old `Base` variant).
    if capture_subdomains && !host.contains('*') {
        pats.push(HostName::new(format!("*.{}", host)).to_host_pattern(&None));
    }

    // When a cruma domain is assigned, also match
    // `<host>.<cruma_domain>` so that requests arriving through the
    // tunnel are routed correctly.
    if let Some(cruma_domain) = cruma_assigned_domain {
        let cruma_host = format!("{}.{}", host, cruma_domain);
        pats.push(HostName::new(&cruma_host).to_host_pattern(&None));

        if capture_subdomains && !host.contains('*') {
            pats.push(HostName::new(format!("*.{}.{}", host, cruma_domain)).to_host_pattern(&None));
        }
    }

    pats
}

/// Check whether `request_host` matches the pattern described by the route's
/// hostname key, taking the legacy `capture_subdomains` flag into account.
///
/// This mirrors the logic used by [`host_patterns_for_route`] so that the
/// dynamic backend-lookup path stays consistent with the proxy routing layer.
fn host_pattern_matches(route_host: &str, capture_subdomains: bool, request_host: &str) -> bool {
    let patterns = host_patterns_for_route(route_host, capture_subdomains, None);
    for pat in &patterns {
        match pat {
            HostPattern::Exact { value } => {
                if request_host.eq_ignore_ascii_case(value) {
                    return true;
                }
            }
            HostPattern::Base { value } => {
                let value_lower = value.to_lowercase();
                if request_host == value_lower
                    || request_host.ends_with(&format!(".{}", value_lower))
                {
                    return true;
                }
            }
            HostPattern::DeepGlob { value } => {
                let suffix = format!(".{}", value.to_lowercase());
                if request_host.ends_with(&suffix) {
                    return true;
                }
            }
            HostPattern::Glob { value } => {
                // Simple glob: split on '*' and check prefix/suffix.
                // Handles patterns like "app-*" and "*-staging".
                let value_lower = value.to_lowercase();
                if let Some((prefix, suffix)) = value_lower.split_once('*') {
                    if request_host.starts_with(prefix) && request_host.ends_with(suffix) {
                        return true;
                    }
                }
            }
            HostPattern::Any => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn protocol_to_upstream(protocol: &v4::Protocol) -> HttpUpstreamProto {
    match protocol {
        v4::Protocol::H1 => HttpUpstreamProto::H11,
        v4::Protocol::H2 => HttpUpstreamProto::H2,
        v4::Protocol::H2C => HttpUpstreamProto::H2C,
        v4::Protocol::H2CPK => HttpUpstreamProto::H2CPK,
    }
}

fn upstream_proto_and_tls(
    protocol: &v4::Protocol,
    use_tls: bool,
) -> (HttpUpstreamProto, Option<OriginTls>) {
    let mut upstream = protocol_to_upstream(protocol);
    if use_tls {
        if matches!(upstream, HttpUpstreamProto::H2C | HttpUpstreamProto::H2CPK) {
            upstream = HttpUpstreamProto::H2;
        }
        (upstream, Some(default_origin_tls()))
    } else {
        (upstream, None)
    }
}

fn default_origin_tls() -> OriginTls {
    OriginTls {
        sni: OriginTlsSni::TryFromClientHelloThenHostHeaderThenBackendAddr,
        trust_insecure_certificates: false,
        ca_file: None,
        client_cert: None,
    }
}

fn origin_tls_with_sni(sni: Option<OriginTlsSni>) -> OriginTls {
    let mut tls = default_origin_tls();
    if let Some(sni) = sni {
        tls.sni = sni;
    }
    tls
}

fn http_route(name: String, pat: HostPattern, backend: WebBackendId) -> HttpRoute {
    HttpRoute {
        name,
        priority: 0,
        filter: HttpMatch::Host { hosts: vec![pat] },
        middlewares: Vec::new(),
        target: Target::Backend { backend },
    }
}

fn http_route_with_middlewares(
    name: String,
    pat: HostPattern,
    backend: WebBackendId,
    middlewares: Vec<HttpMiddleware>,
) -> HttpRoute {
    HttpRoute {
        name,
        priority: 0,
        filter: HttpMatch::Host { hosts: vec![pat] },
        middlewares,
        target: Target::Backend { backend },
    }
}

fn respond_route(name: String, pat: HostPattern, status: u16, body: &str) -> HttpRoute {
    HttpRoute {
        name,
        priority: 0,
        filter: HttpMatch::Host { hosts: vec![pat] },
        middlewares: Vec::new(),
        target: Target::Respond {
            status,
            body: Some(body.as_bytes().to_vec()),
            content_type: None,
        },
    }
}

fn respond_route_with_html(
    name: String,
    pat: HostPattern,
    status: u16,
    body: &[u8],
    extra_headers: Vec<(String, String)>,
) -> HttpRoute {
    let mut middlewares = Vec::new();
    for (name, value) in extra_headers {
        middlewares.push(HttpMiddleware::AddRespHeader { name, value });
    }
    HttpRoute {
        name,
        priority: 0,
        filter: HttpMatch::Host { hosts: vec![pat] },
        middlewares,
        target: Target::Respond {
            status,
            body: Some(body.to_vec()),
            content_type: Some("text/html; charset=utf-8".to_string()),
        },
    }
}

/// Creates an HTTP route that delegates to a HyperService handler.
fn hyper_service_route(name: String, pat: HostPattern, backend: HyperBackendId) -> HttpRoute {
    HttpRoute {
        name,
        priority: 0,
        filter: HttpMatch::Host { hosts: vec![pat] },
        middlewares: vec![],
        target: Target::HyperService { backend },
    }
}

fn serve_dir_route(
    name: String,
    pat: HostPattern,
    directory: String,
    index: String,
    list_dir: bool,
    render_markdown: bool,
    cache_max_age: Option<u64>,
    spa_fallback: bool,
) -> HttpRoute {
    let mut middlewares = Vec::new();
    if let Some(max_age) = cache_max_age {
        middlewares.push(HttpMiddleware::AddRespHeader {
            name: "Cache-Control".to_string(),
            value: format!("max-age={max_age}"),
        });
    }
    HttpRoute {
        name,
        priority: 0,
        filter: HttpMatch::Host { hosts: vec![pat] },
        middlewares,
        target: Target::ServeDir {
            directory,
            index: Some(index),
            list_dir,
            render_markdown,
            spa_fallback,
        },
    }
}

/// Extract all [`HostPattern`]s from a set of HTTP routes so they can be
/// used as an SNI filter on the TLS listener.  Routes that use
/// [`HttpMatch::Any`] (e.g. the fallback 404) are intentionally skipped
/// so that we never produce a catch-all SNI pattern.
fn extract_sni_patterns(routes: &[HttpRoute]) -> Vec<HostPattern> {
    routes
        .iter()
        .filter_map(|route| match &route.filter {
            HttpMatch::Host { hosts } => Some(hosts.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Build an optional SNI filter from a set of host patterns.
/// Returns `None` only when no patterns are available (which means
/// there are no configured routes and connections should be rejected).
fn build_sni_filter(patterns: Vec<HostPattern>) -> Option<HostPattern> {
    match patterns.len() {
        0 => None,
        1 => Some(patterns.into_iter().next().unwrap()),
        _ => Some(HostPattern::OneOf(patterns)),
    }
}

fn to_endpoint(addr: &str, port: u16) -> Option<Endpoint> {
    NonZeroU16::new(port).map(|p| Endpoint {
        addr: addr.to_string(),
        port: p,
    })
}

/// Build a cruma_proxy_lib Configuration from the current OddBox V4 config.
pub fn build_config(cfg: &ConfigWrapper) -> anyhow::Result<(Configuration, BuildNotes)> {
    build_config_with_runtime_ports(cfg, &HashMap::new(), &HashMap::new(), None, false, None)
}

/// Build a cruma_proxy_lib Configuration with access to global state.
///
/// When `state` is provided, the offline handler will have access to:
/// - Process registry for live process state
/// - Host-to-backend mapping for identifying which process is being requested
///
/// When `cruma_tunnel_only` is true, only routes with `enable_cruma: true` are
/// included.  If `cruma_assigned_domain` is provided, each cruma-enabled route
/// also gets an additional host pattern matching
/// `<hostname>.<cruma_assigned_domain>` so that requests arriving through the
/// tunnel on that subdomain are routed correctly.
pub fn build_config_with_runtime_ports(
    cfg: &ConfigWrapper,
    runtime_ports: &HashMap<String, u16>,
    runtime_states: &HashMap<String, ProcState>,
    cruma_assigned_domain: Option<&str>,
    cruma_tunnel_only: bool,
    state: Option<Arc<GlobalState>>,
) -> anyhow::Result<(Configuration, BuildNotes)> {
    let mut notes = BuildNotes::default();
    let mut web_backends: HashMap<WebBackendId, WebBackend> = HashMap::new();
    let mut hyper_backends: HashMap<HyperBackendId, HyperHandler> = HashMap::new();
    let mut http_routes: Vec<HttpRoute> = Vec::new();
    let mut lets_encrypt_hosts: HashSet<String> = HashSet::new();
    let mut lets_encrypt_host_patterns: Vec<HostPattern> = Vec::new();

    // Register the shared offline handler with state access
    // The handler looks up host-to-backend mappings dynamically from the config,
    // so it always has fresh data even if backends are added/removed.
    let offline_handler = if let Some(state) = state {
        create_offline_handler(state)
    } else {
        // Fallback handler without state (for tests or when state isn't available)
        create_offline_handler_simple()
    };

    hyper_backends.insert(HyperBackendId::from(OFFLINE_HANDLER_ID), offline_handler);

    // Get ports from frontends
    let http_port = cfg.frontends.http.as_ref().map(|f| f.port).unwrap_or(80);
    let tls_port = cfg.frontends.https.as_ref().map(|f| f.port).unwrap_or(443);

    // Determine loopback address for process backends
    let loopback_addr = "127.0.0.1"; // V4 doesn't have use_loopback_ip_for_procs, always use 127.0.0.1

    // Build backends and routes from V4 config.
    // Merge HTTP routes with explicit HTTPS routes. If the same hostname is
    // present in both maps, the explicit HTTPS route wins.
    let mut merged_routes: BTreeMap<String, v4::RouteTarget> = BTreeMap::new();
    if let Some(http_frontend) = &cfg.frontends.http {
        for (host, target) in &http_frontend.routes {
            merged_routes.insert(host.clone(), target.clone());
        }
    }
    if let Some(https_frontend) = &cfg.frontends.https {
        if let Some(v4::HttpsRoutes::Explicit(explicit_https_routes)) = &https_frontend.routes {
            for (host, target) in explicit_https_routes {
                merged_routes.insert(host.clone(), target.clone());
            }
        }
    }

    for (host, target) in &merged_routes {
        // When building the tunnel config, skip routes not marked for cruma.
        if cruma_tunnel_only && !target.enable_cruma() {
            notes.warnings.push(format!(
                "Skipping route '{}' in cruma tunnel config because enable_cruma is false",
                host
            ));
            continue;
        }

        let backend_id_str = target.backend_id();
        let capture_subdomains = target.capture_subdomains();

        // Build an optional FormAuth middleware for this route.
        let form_auth_middleware: Option<HttpMiddleware> = {
            let users = target.form_auth_users();
            if users.is_empty() {
                None
            } else {
                Some(HttpMiddleware::FormAuth {
                    users: users
                        .iter()
                        .map(|u| (u.username.clone(), u.password.clone()))
                        .collect(),
                    secret: target.form_auth_secret().to_string(),
                    login_path: "/auth/login".to_string(),
                    logout_path: "/auth/logout".to_string(),
                    cookie_name: "odd_box_session".to_string(),
                    cookie_secure: false,
                    cookie_same_site: cruma_proxy_lib::types::CookieSameSite::Lax,
                    cookie_domain: None,
                    session_ttl_secs: 86400,
                })
            }
        };

        // Build the full set of host patterns using the SDK's
        // pattern syntax (supports *.example.com, app-*, *, etc.)
        // plus optional cruma-domain expansion.
        let host_patterns =
            host_patterns_for_route(host, capture_subdomains, cruma_assigned_domain);

        // Track hosts that have Let's Encrypt enabled
        if target.lets_encrypt() {
            lets_encrypt_hosts.insert(host.clone());
            lets_encrypt_host_patterns.push(HostName::new(host).to_host_pattern(&None));
        }

        // Look up the backend
        if let Some(backend) = cfg.backends.get(backend_id_str) {
            let cruma_backend_id = WebBackendId(format!("backend::{}", backend_id_str));

            match backend {
                v4::Backend::Process(proc) => {
                    let is_running = runtime_states
                        .get(backend_id_str)
                        .map(|state| matches!(state, ProcState::Running))
                        .unwrap_or(true);

                    if !is_running {
                        for pat in &host_patterns {
                            http_routes.push(hyper_service_route(
                                format!("{host}-starting"),
                                pat.clone(),
                                HyperBackendId::from(OFFLINE_HANDLER_ID),
                            ));
                        }
                        continue;
                    }

                    let port = proc
                        .port
                        .or_else(|| runtime_ports.get(backend_id_str).copied());
                    let Some(port) = port else {
                        notes.warnings.push(format!(
                            "Process backend '{}' has no port; using starting response for route '{}'",
                            backend_id_str, host
                        ));
                        for pat in &host_patterns {
                            http_routes.push(hyper_service_route(
                                format!("{host}-starting"),
                                pat.clone(),
                                HyperBackendId::from(OFFLINE_HANDLER_ID),
                            ));
                        }
                        continue;
                    };

                    if let Some(ep) = to_endpoint(loopback_addr, port) {
                        let (protocol, origin_tls) =
                            upstream_proto_and_tls(&proc.protocol, proc.https);
                        let web_backend = WebBackend {
                            id: cruma_backend_id.clone(),
                            protocol,
                            endpoints: NonEmptyVec(vec![ep]),
                            origin_tls,
                            timeout_seconds: None,
                        };
                        web_backends.insert(cruma_backend_id.clone(), web_backend);
                    }

                    // For process backends on loopback, preserve the original host header
                    let mut middlewares = vec![HttpMiddleware::RewriteHost { to: host.clone() }];
                    if let Some(fa) = form_auth_middleware.clone() {
                        middlewares.push(fa);
                    }

                    for pat in &host_patterns {
                        http_routes.push(http_route_with_middlewares(
                            host.clone(),
                            pat.clone(),
                            cruma_backend_id.clone(),
                            middlewares.clone(),
                        ));
                    }
                }

                v4::Backend::Remote(remote) => {
                    let endpoints: Vec<Endpoint> = remote
                        .endpoints
                        .iter()
                        .filter_map(|ep| to_endpoint(&ep.addr, ep.port))
                        .collect();

                    if endpoints.is_empty() {
                        notes.warnings.push(format!(
                            "Remote backend '{}' has no valid endpoints",
                            backend_id_str
                        ));
                        continue;
                    }

                    let (protocol, mut origin_tls) =
                        upstream_proto_and_tls(&remote.protocol, remote.https);
                    let backend_host = endpoints.first().map(|ep| ep.addr.clone());
                    if remote.https && !remote.keep_original_host_header {
                        if let Some(host) = backend_host.as_deref() {
                            origin_tls = Some(origin_tls_with_sni(Some(OriginTlsSni::Custom(
                                host.to_string(),
                            ))));
                        }
                    }
                    let web_backend = WebBackend {
                        id: cruma_backend_id.clone(),
                        protocol,
                        endpoints: NonEmptyVec(endpoints),
                        origin_tls,
                        timeout_seconds: None,
                    };
                    web_backends.insert(cruma_backend_id.clone(), web_backend);

                    let mut middlewares = Vec::new();
                    if !remote.keep_original_host_header {
                        if let Some(host) = backend_host {
                            middlewares.push(HttpMiddleware::RewriteHost { to: host });
                        }
                    }
                    if let Some(fa) = form_auth_middleware.clone() {
                        middlewares.push(fa);
                    }
                    for pat in &host_patterns {
                        http_routes.push(http_route_with_middlewares(
                            host.clone(),
                            pat.clone(),
                            cruma_backend_id.clone(),
                            middlewares.clone(),
                        ));
                    }
                }

                v4::Backend::Static(static_backend) => {
                    // Resolve the directory path
                    let resolved = match cfg.resolve_static_backend(static_backend) {
                        Ok(r) => r,
                        Err(err) => {
                            notes.warnings.push(format!(
                                "Static backend '{}' could not resolve directory: {err}",
                                backend_id_str
                            ));
                            continue;
                        }
                    };

                    for pat in &host_patterns {
                        let mut route = serve_dir_route(
                            host.clone(),
                            pat.clone(),
                            resolved.dir.clone(),
                            resolved.index.clone(),
                            resolved.list_dir,
                            resolved.render_markdown,
                            resolved.cache_max_age,
                            resolved.spa_fallback,
                        );
                        if let Some(fa) = form_auth_middleware.clone() {
                            route.middlewares.push(fa);
                        }
                        http_routes.push(route);
                    }
                }
            }
        } else {
            notes.warnings.push(format!(
                "Route '{}' references unknown backend '{}'",
                host, backend_id_str
            ));
        }
    }

    // Docker containers (treated like remote backends)
    for cont in cfg.docker_containers.iter() {
        let cont: ContainerProxyTarget = cont.value().clone();
        let host = cont.generate_host_name();
        let backend_id = WebBackendId(format!("docker::{}", host));

        if cont.port == 0 {
            notes
                .warnings
                .push(format!("Docker target '{}' has no port", host));
            continue;
        }

        let ep = match to_endpoint(&cont.target_addr, cont.port) {
            Some(ep) => ep,
            None => {
                notes.warnings.push(format!(
                    "Docker target '{}' has invalid port {}",
                    host, cont.port
                ));
                continue;
            }
        };

        // Convert docker hints to protocol
        let protocol = if cont
            .hints
            .iter()
            .any(|h| matches!(h, crate::configuration::Hint::H2))
        {
            HttpUpstreamProto::H2
        } else if cont
            .hints
            .iter()
            .any(|h| matches!(h, crate::configuration::Hint::H2CPK))
        {
            HttpUpstreamProto::H2CPK
        } else if cont
            .hints
            .iter()
            .any(|h| matches!(h, crate::configuration::Hint::H2C))
        {
            HttpUpstreamProto::H2C
        } else {
            HttpUpstreamProto::H11
        };

        let backend = WebBackend {
            id: backend_id.clone(),
            protocol,
            endpoints: NonEmptyVec(vec![ep]),
            origin_tls: cont.tls.then_some(default_origin_tls()),
            timeout_seconds: None,
        };
        web_backends.insert(backend_id.clone(), backend);
        http_routes.push(http_route(
            host.clone(),
            HostName::new(&host).to_host_pattern(&None),
            backend_id,
        ));
    }

    // Admin/API hostnames fallback
    for admin_host in [
        Some("oddbox.localhost".to_string()),
        Some("odd-box.localhost".to_string()),
        cfg.admin_api_host.clone(),
    ]
    .into_iter()
    .flatten()
    {
        http_routes.push(respond_route(
            admin_host.clone(),
            HostName::new(&admin_host).to_host_pattern(&None),
            501,
            "admin API not yet wired through cruma proxy",
        ));
    }

    // Fallback 404 route
    http_routes.push(HttpRoute {
        name: "fallback-404".into(),
        priority: 0,
        filter: HttpMatch::Any,
        middlewares: Vec::new(),
        target: Target::Respond {
            status: 404,
            body: Some(DEFAULT_404_HTML.to_vec()),
            content_type: Some("text/html; charset=utf-8".to_string()),
        },
    });

    // Collect all known host patterns from the routes BEFORE wrapping them
    // in NonEmptyVec. These patterns are used as SNI filters on the TLS
    // listeners so that we only terminate TLS (and potentially trigger ACME
    // certificate issuance) for hostnames we actually have routes for.
    // This prevents abuse where an attacker sends many unknown hostnames
    // to exhaust ACME rate limits.
    let all_sni_patterns = extract_sni_patterns(&http_routes);

    let http_routes = NonEmptyVec(http_routes);

    // Determine TLS cert mode from config
    let cert_mode = match cfg.frontends.https.as_ref().map(|h| &h.cert) {
        Some(v4::CertMode::Acme) => CertMode::AcmeAlpn {
            cert_target: Default::default(),
        },
        Some(v4::CertMode::SelfSigned) => CertMode::SelfSigned,
        _ => CertMode::SelfSigned,
    };

    let listeners = if cruma_tunnel_only {
        let mut tls_routes: Vec<TlsRoute> = Vec::new();

        // Route 1 (highest priority): DNS-01 via tunnel control channel for subdomains of the
        // cruma-assigned domain (e.g. *.demo-tunnel-one.tun.cruma.io).
        //
        // TLS-ALPN-01 cannot work for these: Let's Encrypt would try to connect back to the
        // domain on port 443, which hits the cruma server — not us.  The cruma server instead
        // sets the required TXT record when we send it a Dns01Request control message.
        if let Some(assigned_domain) = cruma_assigned_domain {
            let cruma_wildcard = format!("*.{}", assigned_domain);
            let cruma_sni = Some(HostName::new(&cruma_wildcard).to_host_pattern(&None));
            tracing::debug!(
                cruma_wildcard,
                "Tunnel TLS: DNS-01 route registered for cruma-assigned subdomains"
            );
            tls_routes.push(TlsRoute {
                name: "tls-tunnel-dns01".into(),
                rule: TlsMatch {
                    sni: cruma_sni,
                    alpn: Some(vec![Alpn::H2, Alpn::Http11]),
                },
                action: TlsAction::TerminateForHTTP {
                    cert_mode: CertMode::AcmeDns01 {
                        provider: AcmeDns01Provider::External {
                            id: cruma_tunnels_lib::dns::provider_id().to_string(),
                        },
                        cert_target: CertTarget::Wildcard,
                    },
                    http: http_routes.clone(),
                },
            });
        }

        // Route 2 (catch-all): TLS-ALPN-01 for custom CNAMEd domains that point at the tunnel.
        // For these, Let's Encrypt can reach us directly on port 443 through the tunnel forward,
        // so the standard ALPN challenge works fine.
        //
        // The SNI filter is restricted to host patterns we actually have routes for, preventing
        // an attacker from exhausting ACME rate limits by sending arbitrary hostnames.
        let sni = build_sni_filter(all_sni_patterns);
        if sni.is_none() && cruma_assigned_domain.is_none() {
            tracing::warn!(
                "No SNI patterns and no cruma assigned domain — \
                 all tunnel TLS connections will be rejected"
            );
        }
        tls_routes.push(TlsRoute {
            name: "tls-tunnel-alpn".into(),
            rule: TlsMatch {
                sni,
                alpn: Some(vec![Alpn::H2, Alpn::Http11]),
            },
            action: TlsAction::TerminateForHTTP {
                cert_mode: CertMode::AcmeAlpn {
                    cert_target: Default::default(),
                },
                http: http_routes,
            },
        });

        vec![Listener::Tls(TlsListener {
            port: non_zero_port(tls_port, "tls_port")?,
            routes: NonEmptyVec(tls_routes),
        })]
    } else {
        // Split http_routes into ACME (Let's Encrypt) routes and self-signed routes
        // based on whether the route's host was marked with lets_encrypt in the config.
        let (acme_routes, self_signed_routes): (Vec<HttpRoute>, Vec<HttpRoute>) = http_routes
            .0
            .into_iter()
            .partition(|route| lets_encrypt_hosts.contains(&route.name));

        let all_routes = NonEmptyVec([self_signed_routes.clone(), acme_routes.clone()].concat());

        // Build TLS routes with ACME first (specific SNI match) then self-signed (catch-all).
        // This ordering ensures ACME hosts get real certs while everything else falls through
        // to self-signed.
        let mut tls_routes = Vec::new();

        if !acme_routes.is_empty() {
            // Build an SNI filter from the collected Let's Encrypt host patterns so that
            // only those hosts are matched by the ACME TLS route.
            let sni_filter = if lets_encrypt_host_patterns.len() == 1 {
                lets_encrypt_host_patterns.into_iter().next().unwrap()
            } else {
                HostPattern::OneOf(lets_encrypt_host_patterns)
            };

            tls_routes.push(TlsRoute {
                name: "tls-acme".into(),
                rule: TlsMatch {
                    sni: Some(sni_filter),
                    alpn: Some(vec![Alpn::Http11, Alpn::H2]),
                },
                action: TlsAction::TerminateForHTTP {
                    cert_mode: CertMode::AcmeAlpn {
                        cert_target: Default::default(),
                    },
                    http: NonEmptyVec(acme_routes),
                },
            });
        }

        // Build an SNI filter for the self-signed route from the non-LE
        // host patterns so that we don't generate self-signed certificates
        // (or worse, ACME certificates if cert_mode is AcmeAlpn) for
        // completely unknown hostnames.
        let self_signed_sni = build_sni_filter(extract_sni_patterns(&self_signed_routes));
        tls_routes.push(TlsRoute {
            name: "tls-self-signed".into(),
            rule: TlsMatch {
                sni: self_signed_sni,
                alpn: None,
            },
            action: TlsAction::TerminateForHTTP {
                cert_mode,
                http: NonEmptyVec(self_signed_routes),
            },
        });

        vec![
            Listener::Http(HttpListener {
                port: non_zero_port(http_port, "http_port")?,
                routes: all_routes,
            }),
            Listener::Tls(TlsListener {
                port: non_zero_port(tls_port, "tls_port")?,
                routes: NonEmptyVec(tls_routes),
            }),
        ]
    };

    // Build ACME config from environment/defaults (cruma-proxy-lib doesn't expose email field)
    let acme = AcmeAccountConfig::from_env();

    let config = Configuration {
        listeners,
        web_backends,
        tcp_backends: HashMap::new(),
        hyper_backends,
        dynamic_backend_resolvers: HashMap::new(),
        acme,
    };

    if let Err(errs) = config.validate() {
        bail!("cruma configuration validation failed: {errs:?}");
    }

    Ok((config, notes))
}

pub fn runtime_ports_from_registry(registry: &ProcessRegistry) -> HashMap<String, u16> {
    let snapshot = registry.snapshot();
    snapshot
        .entries
        .iter()
        .filter_map(|entry| {
            entry
                .active_port()
                .map(|port| (entry.backend_id.clone(), port))
        })
        .collect()
}

pub fn runtime_states_from_registry(registry: &ProcessRegistry) -> HashMap<String, ProcState> {
    let snapshot = registry.snapshot();
    snapshot
        .entries
        .iter()
        .map(|entry| (entry.backend_id.clone(), entry.proc_state()))
        .collect()
}

pub fn rebuild_cruma_config(state: Arc<GlobalState>) {
    let cfg = state.config.load_full();
    let runtime_ports = runtime_ports_from_registry(&state.process_registry);
    let runtime_states = runtime_states_from_registry(&state.process_registry);

    // Resolve the currently assigned cruma domain (if any) for tunnel host matching.
    let assignment = state.cruma_assignment.load_full();
    let cruma_domain = assignment.as_ref().map(|a| a.assigned_domain.as_str());

    // Rebuild the local hosting config (all routes).
    match build_config_with_runtime_ports(
        &cfg,
        &runtime_ports,
        &runtime_states,
        None,
        false,
        Some(state.clone()),
    ) {
        Ok((new_cfg, notes)) => {
            if !notes.warnings.is_empty() {
                tracing::trace!(
                    "some backends not yet routable after process state change: {:?}",
                    notes.warnings
                );
            }
            state.cruma_config.store(std::sync::Arc::new(new_cfg));
        }
        Err(e) => {
            tracing::error!(error=%e, "Failed to rebuild cruma config after process update");
        }
    }

    // Rebuild the tunnel config (only enable_cruma routes, with cruma domain matching).
    match build_config_with_runtime_ports(
        &cfg,
        &runtime_ports,
        &runtime_states,
        cruma_domain,
        true,
        Some(state.clone()),
    ) {
        Ok((tunnel_cfg, notes)) => {
            if !notes.warnings.is_empty() {
                tracing::trace!(
                    "some tunnel backends not yet routable after process state change: {:?}",
                    notes.warnings
                );
            }
            state
                .cruma_tunnel_config
                .store(std::sync::Arc::new(tunnel_cfg));
        }
        Err(e) => {
            tracing::error!(error=%e, "Failed to rebuild cruma tunnel config after process update");
        }
    }
}
