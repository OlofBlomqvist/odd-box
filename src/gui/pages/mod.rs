mod backends;
mod dashboard;
mod frontends;
mod monitoring;
mod processes;

pub(super) use monitoring::CachedLogLine;

use std::sync::Arc;

use crate::configuration;
use crate::global_state::{GlobalState, ProcState};

/// Cached process info for display
#[derive(Clone, Debug)]
pub struct CachedProcess {
    pub name: String,
    pub bin: String,
    pub port: String,
    pub protocol: String,
    pub state: ProcState,
    pub auto_start: bool,
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
}

/// Async function to fetch configuration data
pub async fn fetch_config(state: Arc<GlobalState>) -> CachedConfig {
    let config_guard = state.config.read().await;
    let snapshot = state.process_registry.snapshot();

    // Fetch processes
    let mut processes: Vec<CachedProcess> = config_guard
        .hosted_processes
        .iter()
        .map(|entry| {
            let name = entry.key().clone();
            let proc = entry.value();
            let handle = snapshot.get(&name);
            let proc_state = handle
                .map(|h| h.proc_state())
                .unwrap_or(ProcState::Stopped);
            let port = handle
                .and_then(|h| h.active_port())
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            CachedProcess {
                name,
                bin: proc.bin.clone(),
                port,
                protocol: format!("{:?}", proc.protocol),
                state: proc_state,
                auto_start: proc.auto_start.unwrap_or(true),
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
    }
}
