mod backends;
mod cruma;
mod dashboard;
mod edit_backend;
mod edit_frontend;
mod frontends;
mod monitoring;
mod processes;
mod traffic_inspection;
mod updates;

use std::sync::Arc;

use crate::configuration;
use crate::global_state::{GlobalState, ProcState};

/// Cached process info for display
#[derive(Clone, Debug)]
pub struct CachedProcess {
    pub name: String,
    pub bin: String,
    pub port: String,
    pub configured_port: Option<u16>,
    pub protocol: String,
    pub state: ProcState,
    pub auto_start: bool,
    pub args: String,
    pub dir: String,
    pub env: Vec<(String, String)>,
}

/// Cached remote backend info for display
#[derive(Clone, Debug)]
pub struct CachedRemoteBackend {
    pub name: String,
    pub endpoints: String,
    pub protocol: String,
    pub https: bool,
    pub state: ProcState,
}

/// Cached static backend info for display
#[derive(Clone, Debug)]
pub struct CachedStaticBackend {
    pub name: String,
    pub dir: String,
    pub list_dir: bool,
    pub state: ProcState,
}

/// Cached route info for display
#[derive(Clone, Debug)]
pub struct CachedRoute {
    pub hostname: String,
    pub backend: String,
    pub https_redirect: bool,
    pub capture_subdomains: bool,
}

/// All cached config data
#[derive(Clone, Debug, Default)]
pub struct CachedConfig {
    pub processes: Vec<CachedProcess>,
    pub remote_backends: Vec<CachedRemoteBackend>,
    pub static_backends: Vec<CachedStaticBackend>,
    pub routes: Vec<CachedRoute>,
    pub http_port: Option<u16>,
    pub https_port: Option<u16>,
    pub global_env: Vec<(String, String)>,
}

/// Async function to fetch configuration data
pub async fn fetch_config(state: Arc<GlobalState>) -> CachedConfig {
    let config_guard = state.config.load_full();
    let snapshot = state.process_registry.snapshot();
    let http_port = config_guard.frontends.http.as_ref().map(|h| h.port);
    let https_port = config_guard.frontends.https.as_ref().map(|h| h.port);

    // Fetch global environment variables
    let mut global_env: Vec<(String, String)> = config_guard
        .env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    global_env.sort_by(|a, b| a.0.cmp(&b.0));

    // Fetch processes
    let mut processes: Vec<CachedProcess> = config_guard
        .hosted_processes
        .iter()
        .map(|entry| {
            let name = entry.key().clone();
            let proc = entry.value();
            let handle = snapshot.get(&name);
            let state = handle.map(|h| h.state());
            let proc_state = state
                .as_ref()
                .map(|s| s.proc_state.clone())
                .unwrap_or(ProcState::Stopped);
            let port = state
                .as_ref()
                .and_then(|s| s.active_port)
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            let resolved_bin = state
                .as_ref()
                .and_then(|s| s.resolved_bin.clone())
                .unwrap_or_else(|| proc.bin.clone());
            let resolved_args = state
                .as_ref()
                .and_then(|s| s.resolved_args.clone())
                .unwrap_or_else(|| proc.args.clone());
            let resolved_dir = state
                .as_ref()
                .and_then(|s| s.resolved_dir.clone())
                .unwrap_or_else(|| proc.dir.clone().unwrap_or_default());
            let resolved_env = state
                .as_ref()
                .and_then(|s| s.resolved_env.clone())
                .unwrap_or_else(|| {
                    proc.env
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                });
            CachedProcess {
                name,
                bin: resolved_bin,
                port,
                configured_port: proc.port,
                protocol: format!("{:?}", proc.protocol),
                state: proc_state,
                auto_start: proc.auto_start.unwrap_or(true),
                args: resolved_args.join(" "),
                dir: resolved_dir,
                env: resolved_env,
            }
        })
        .collect();
    processes.sort_by(|a, b| a.name.cmp(&b.name));

    // Fetch remote backends
    let mut remote_backends: Vec<CachedRemoteBackend> = config_guard
        .remote_sites
        .iter()
        .map(|entry| {
            let name = entry.key().clone();
            let remote = entry.value();
            let endpoints = remote
                .endpoints
                .iter()
                .map(|e| format!("{}:{}", e.addr, e.port))
                .collect::<Vec<_>>()
                .join(", ");
            let state = snapshot
                .get(&name)
                .map(|h| h.proc_state())
                .unwrap_or(ProcState::Remote);
            CachedRemoteBackend {
                name,
                endpoints,
                protocol: format!("{:?}", remote.protocol),
                https: remote.https,
                state,
            }
        })
        .collect();
    remote_backends.sort_by(|a, b| a.name.cmp(&b.name));

    // Fetch static backends
    let mut static_backends: Vec<CachedStaticBackend> = config_guard
        .static_sites
        .iter()
        .map(|entry| {
            let name = entry.key().clone();
            let static_site = entry.value();
            let state = snapshot
                .get(&name)
                .map(|h| h.proc_state())
                .unwrap_or(ProcState::DirServer);
            CachedStaticBackend {
                name,
                dir: static_site.dir.clone(),
                list_dir: static_site.list_dir,
                state,
            }
        })
        .collect();
    static_backends.sort_by(|a, b| a.name.cmp(&b.name));

    // Fetch routes from frontends
    let mut routes: Vec<CachedRoute> = Vec::new();

    // Get routes from HTTP frontend
    if let Some(http) = &config_guard.frontends.http {
        for (hostname, target) in &http.routes {
            routes.push(CachedRoute {
                hostname: hostname.clone(),
                backend: target.backend_id().to_string(),
                https_redirect: target.redirect_to_https(),
                capture_subdomains: target.capture_subdomains(),
            });
        }
    }

    // Add any HTTPS-only routes
    if let Some(https) = &config_guard.frontends.https {
        if let Some(configuration::HttpsRoutes::Explicit(https_routes)) = &https.routes {
            for (hostname, target) in https_routes {
                // Only add if not already in the list from HTTP
                if !routes.iter().any(|r| r.hostname == *hostname) {
                    routes.push(CachedRoute {
                        hostname: hostname.clone(),
                        backend: target.backend_id().to_string(),
                        https_redirect: false,
                        capture_subdomains: target.capture_subdomains(),
                    });
                }
            }
        }
    }
    routes.sort_by(|a, b| a.hostname.cmp(&b.hostname));

    CachedConfig {
        processes,
        remote_backends,
        static_backends,
        routes,
        http_port,
        https_port,
        global_env,
    }
}
