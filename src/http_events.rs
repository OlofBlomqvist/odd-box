use std::sync::Arc;

use cruma_proxy_lib::proxying::{self, HttpEvent, HttpEventSink};

use crate::configuration::v4::{Backend, RouteTarget};
use crate::global_state::{GlobalState, ProcState};

pub fn install_http_event_sink(state: Arc<GlobalState>) {
    let sink: Arc<dyn HttpEventSink> = Arc::new(OddBoxHttpEventSink { state });
    proxying::set_http_event_sink(Some(sink));
}

struct OddBoxHttpEventSink {
    state: Arc<GlobalState>,
}

impl HttpEventSink for OddBoxHttpEventSink {
    fn on_event(&self, event: HttpEvent) {
        match event {
            HttpEvent::RequestStarted {
                method,
                url,
                host,
                path,
                client_addr,
                http_version,
                ..
            } => {
                if let Some(proc_host) =
                    parse_stop_command(&method, host.as_deref(), &url, &path)
                {
                    handle_stop_command(&self.state, &proc_host, host.as_deref(), &path);
                }

                tracing::trace!(
                    method = %method,
                    host = ?host,
                    path = %path,
                    client_addr = ?client_addr,
                    http_version = ?http_version,
                    "cruma http request started"
                );

                let Some(host) = host.as_deref() else {
                    return;
                };
                let host = normalize_host(host);
                if host.is_empty() {
                    return;
                }

                let cfg = self.state.config.load_full();
                let backend_id = match find_backend_for_host(&cfg, &host) {
                    Some(id) => id,
                    None => return,
                };

                let Some(backend) = cfg.backends.get(backend_id) else {
                    return;
                };
                let Backend::Process(proc_backend) = backend else {
                    return;
                };

                let registry = &self.state.process_registry;
                let active_port = registry
                    .snapshot()
                    .get(backend_id)
                    .and_then(|h| h.active_port());
                let configured_port = proc_backend.port;
                tracing::trace!(
                    backend_id = %backend_id,
                    host = %host,
                    active_port = ?active_port,
                    configured_port = ?configured_port,
                    "process route ports for incoming request"
                );

                if proc_backend.exclude_from_start_all {
                    tracing::warn!(
                        backend_id = %backend_id,
                        host = %host,
                        "process is marked exclude_from_start_all; auto-start on request is still enabled"
                    );
                }

                let was_disabled = !registry.is_enabled(backend_id);
                if was_disabled {
                    tracing::warn!(
                        backend_id = %backend_id,
                        host = %host,
                        "process is disabled; overriding and auto-starting on request"
                    );
                }

                let Some(state) = registry.snapshot().state_of(backend_id) else {
                    return;
                };

                if matches!(state, ProcState::Stopped | ProcState::Faulty) {
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
                    tracing::info!(
                        backend_id = %backend_id,
                        host = %host,
                        "auto-starting process due to incoming request"
                    );
                }
            }
            HttpEvent::UpgradeDetected { kind, .. } => {
                tracing::debug!(kind = ?kind, "cruma http upgrade detected");
            }
            HttpEvent::RequestFinished {
                status,
                duration_ms,
                ..
            } => {
                tracing::debug!(
                    status = ?status,
                    duration_ms = %duration_ms,
                    "cruma http request finished"
                );
            }
        }
    }
}

fn parse_stop_command(
    method: &str,
    host: Option<&str>,
    url: &str,
    path: &str,
) -> Option<String> {
    if method != "GET" {
        return None;
    }
    let host = host
        .and_then(|h| {
            let normalized = normalize_host(h);
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        })
        .or_else(|| parse_host_from_url(url))?;

    if host != "localhost" && host != "127.0.0.1" {
        return None;
    }

    let path_and_query = extract_path_and_query(url).unwrap_or(path);
    let (path_only, query) = path_and_query.split_once('?')?;
    if path_only != "/STOP" {
        return None;
    }
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == "proc" && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn handle_stop_command(
    state: &Arc<GlobalState>,
    proc_host: &str,
    cmd_host: Option<&str>,
    path: &str,
) {
    let cfg = state.config.load_full();
    let backend_id = match find_backend_for_host(&cfg, proc_host) {
        Some(id) => id,
        None => {
            tracing::warn!(
                command = "STOP",
                proc = %proc_host,
                cmd_host = ?cmd_host,
                path = %path,
                "stop command received but no matching frontend route found"
            );
            return;
        }
    };

    let Some(backend) = cfg.backends.get(backend_id) else {
        tracing::warn!(
            command = "STOP",
            proc = %proc_host,
            backend_id = %backend_id,
            "stop command received but backend missing in config"
        );
        return;
    };
    let Backend::Process(_) = backend else {
        tracing::warn!(
            command = "STOP",
            proc = %proc_host,
            backend_id = %backend_id,
            "stop command received but backend is not a process"
        );
        return;
    };

    let registry = &state.process_registry;
    let Some(current) = registry.snapshot().state_of(backend_id) else {
        tracing::warn!(
            command = "STOP",
            proc = %proc_host,
            backend_id = %backend_id,
            "stop command received but process not registered"
        );
        return;
    };

    tracing::info!(
        command = "STOP",
        proc = %proc_host,
        backend_id = %backend_id,
        state = ?current,
        cmd_host = ?cmd_host,
        path = %path,
        "stop command received"
    );

    if matches!(current, ProcState::Stopped | ProcState::Stopping) {
        return;
    }

    registry.update_state(
        backend_id,
        ProcState::Stopping,
        None,
        None,
        Some(false),
        None,
        None,
        None,
        None,
    );
}

fn normalize_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.');
    if host.is_empty() {
        return String::new();
    }
    let host = strip_port(host);
    host.to_ascii_lowercase()
}

fn strip_port(host: &str) -> &str {
    if host.starts_with('[') {
        return host;
    }
    if let Some(idx) = host.rfind(':') {
        if host[idx + 1..].chars().all(|c| c.is_ascii_digit()) {
            return &host[..idx];
        }
    }
    host
}

fn parse_host_from_url(url: &str) -> Option<String> {
    let scheme_idx = url.find("://")?;
    let mut rest = &url[scheme_idx + 3..];
    if let Some(at_idx) = rest.rfind('@') {
        rest = &rest[at_idx + 1..];
    }
    let host_port = rest
        .split('/')
        .next()
        .unwrap_or_default()
        .trim();
    if host_port.is_empty() {
        return None;
    }
    let normalized = normalize_host(host_port);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn extract_path_and_query(url: &str) -> Option<&str> {
    if url.starts_with('/') {
        return Some(url);
    }
    let scheme_idx = url.find("://")?;
    let rest = &url[scheme_idx + 3..];
    if let Some(slash_idx) = rest.find('/') {
        return Some(&rest[slash_idx..]);
    }
    if let Some(query_idx) = rest.find('?') {
        return Some(&rest[query_idx..]);
    }
    None
}

fn find_backend_for_host<'a>(
    cfg: &'a crate::configuration::ConfigWrapper,
    host: &str,
) -> Option<&'a str> {
    if let Some(target) = match_route(cfg.http_routes(), host) {
        return Some(target.backend_id());
    }
    if let Some(target) = match_route(cfg.https_routes(), host) {
        return Some(target.backend_id());
    }
    None
}

fn match_route<'a>(
    routes: &'a std::collections::HashMap<String, RouteTarget>,
    host: &str,
) -> Option<&'a RouteTarget> {
    for (route_host, target) in routes {
        if !target.capture_subdomains() {
            if route_host.eq_ignore_ascii_case(host) {
                return Some(target);
            }
        }
    }

    for (route_host, target) in routes {
        if !target.capture_subdomains() {
            continue;
        }
        if host_matches_subdomain(route_host, host) {
            return Some(target);
        }
    }
    None
}

fn host_matches_subdomain(route_host: &str, host: &str) -> bool {
    let route_host = route_host.trim_end_matches('.').to_ascii_lowercase();
    if route_host.is_empty() {
        return false;
    }
    if host.eq_ignore_ascii_case(&route_host) {
        return false;
    }
    host.ends_with(&format!(".{route_host}"))
}
