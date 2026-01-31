use std::collections::HashMap;
use std::num::NonZeroU16;

use anyhow::{Context, bail};
use cruma_proxy_lib::types::*;

use crate::configuration::{ConfigWrapper, v4};
use crate::docker::ContainerProxyTarget;

const DEFAULT_404_HTML: &[u8] = include_bytes!("assets/404.html");

#[derive(Debug, Default, Clone)]
pub struct BuildNotes {
    pub unsupported: Vec<String>,
}

fn non_zero_port(port: u16, label: &str) -> anyhow::Result<NonZeroU16> {
    NonZeroU16::new(port).with_context(|| format!("{label} must be non-zero"))
}

fn host_pattern(host: &str, capture_subdomains: bool) -> HostPattern {
    if capture_subdomains {
        HostPattern::Base {
            value: host.to_string(),
        }
    } else {
        HostPattern::Exact {
            value: host.to_string(),
        }
    }
}

fn protocol_to_upstream(protocol: &v4::Protocol) -> HttpUpstreamProto {
    match protocol {
        v4::Protocol::H1 => HttpUpstreamProto::H11,
        v4::Protocol::H2 => HttpUpstreamProto::H2,
        v4::Protocol::H2C => HttpUpstreamProto::H2C,
        v4::Protocol::H2CPK => HttpUpstreamProto::H2CPK,
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

fn http_route(name: String, pat: HostPattern, backend: WebBackendId) -> HttpRoute {
    HttpRoute {
        name,
        priority: 0,
        filter: HttpMatch::Host { hosts: vec![pat] },
        middlewares: Vec::new(),
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

fn serve_dir_route(
    name: String,
    pat: HostPattern,
    directory: String,
    index: String,
    list_dir: bool,
    render_markdown: bool,
    cache_max_age: Option<u64>,
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
        },
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
    let mut notes = BuildNotes::default();
    let mut web_backends: HashMap<WebBackendId, WebBackend> = HashMap::new();
    let mut http_routes: Vec<HttpRoute> = Vec::new();

    // Get ports from frontends
    let http_port = cfg.frontends.http.as_ref().map(|f| f.port).unwrap_or(80);
    let tls_port = cfg.frontends.https.as_ref().map(|f| f.port).unwrap_or(443);

    // Determine loopback address for process backends
    let loopback_addr = "127.0.0.1"; // V4 doesn't have use_loopback_ip_for_procs, always use 127.0.0.1

    // Build backends and routes from V4 config
    // Process routes from HTTP frontend
    if let Some(http_frontend) = &cfg.frontends.http {
        for (host, target) in &http_frontend.routes {
            let backend_id_str = target.backend_id();
            let capture_subdomains = target.capture_subdomains();

            // Look up the backend
            if let Some(backend) = cfg.backends.get(backend_id_str) {
                let cruma_backend_id = WebBackendId(format!("backend::{}", backend_id_str));

                match backend {
                    v4::Backend::Process(proc) => {
                        // Get port from active_port or configured port
                        let port = proc.active_port.or(proc.port).unwrap_or(if proc.https {
                            443
                        } else {
                            80
                        });

                        if let Some(ep) = to_endpoint(loopback_addr, port) {
                            let web_backend = WebBackend {
                                id: cruma_backend_id.clone(),
                                protocol: protocol_to_upstream(&proc.protocol),
                                endpoints: NonEmptyVec(vec![ep]),
                                origin_tls: proc.https.then_some(default_origin_tls()),
                            };
                            web_backends.insert(cruma_backend_id.clone(), web_backend);
                        }

                        http_routes.push(http_route(
                            host.clone(),
                            host_pattern(host, capture_subdomains),
                            cruma_backend_id,
                        ));
                    }

                    v4::Backend::Remote(remote) => {
                        let endpoints: Vec<Endpoint> = remote
                            .endpoints
                            .iter()
                            .filter_map(|ep| to_endpoint(&ep.addr, ep.port))
                            .collect();

                        if endpoints.is_empty() {
                            notes.unsupported.push(format!(
                                "Remote backend '{}' has no valid endpoints",
                                backend_id_str
                            ));
                            continue;
                        }

                        let web_backend = WebBackend {
                            id: cruma_backend_id.clone(),
                            protocol: protocol_to_upstream(&remote.protocol),
                            endpoints: NonEmptyVec(endpoints),
                            origin_tls: remote.https.then_some(default_origin_tls()),
                        };
                        web_backends.insert(cruma_backend_id.clone(), web_backend);

                        http_routes.push(http_route(
                            host.clone(),
                            host_pattern(host, capture_subdomains),
                            cruma_backend_id,
                        ));
                    }

                    v4::Backend::Static(static_backend) => {
                        // Resolve the directory path
                        let resolved = match cfg.resolve_static_backend(static_backend) {
                            Ok(r) => r,
                            Err(err) => {
                                notes.unsupported.push(format!(
                                    "Static backend '{}' could not resolve directory: {err}",
                                    backend_id_str
                                ));
                                continue;
                            }
                        };

                        http_routes.push(serve_dir_route(
                            host.clone(),
                            host_pattern(host, capture_subdomains),
                            resolved.dir,
                            resolved.index,
                            resolved.list_dir,
                            resolved.render_markdown,
                            resolved.cache_max_age,
                        ));
                    }
                }
            } else {
                notes.unsupported.push(format!(
                    "Route '{}' references unknown backend '{}'",
                    host, backend_id_str
                ));
            }
        }
    }

    // Docker containers (treated like remote backends)
    for cont in cfg.docker_containers.iter() {
        let cont: ContainerProxyTarget = cont.value().clone();
        let host = cont.generate_host_name();
        let backend_id = WebBackendId(format!("docker::{}", host));

        if cont.port == 0 {
            notes
                .unsupported
                .push(format!("Docker target '{}' has no port", host));
            continue;
        }

        let ep = match to_endpoint(&cont.target_addr, cont.port) {
            Some(ep) => ep,
            None => {
                notes.unsupported.push(format!(
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
        };
        web_backends.insert(backend_id.clone(), backend);
        http_routes.push(http_route(
            host.clone(),
            host_pattern(&host, cont.capture_subdomains.unwrap_or(false)),
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
        // TODO: route admin API/UI through cruma backends rather than hardcoded 501.
        http_routes.push(respond_route(
            admin_host.clone(),
            host_pattern(&admin_host, false),
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

    let http_routes = NonEmptyVec(http_routes);

    // Determine TLS cert mode from config
    let cert_mode = match cfg.frontends.https.as_ref().map(|h| &h.cert) {
        Some(v4::CertMode::Acme) => CertMode::AcmeAlpn {
            cert_target: Default::default(),
        },
        _ => CertMode::SelfSigned,
    };

    let listeners = vec![
        Listener::Http(HttpListener {
            port: non_zero_port(http_port, "http_port")?,
            routes: http_routes.clone(),
        }),
        Listener::Tls(TlsListener {
            port: non_zero_port(tls_port, "tls_port")?,
            routes: NonEmptyVec(vec![TlsRoute {
                name: "tls-default".into(),
                rule: TlsMatch {
                    sni: None,
                    alpn: None,
                },
                action: TlsAction::TerminateForHTTP {
                    cert_mode,
                    http: http_routes,
                },
            }]),
        }),
    ];

    // Build ACME config from environment/defaults (cruma-proxy-lib doesn't expose email field)
    let acme = AcmeAccountConfig::from_env();

    let config = Configuration {
        listeners,
        web_backends,
        tcp_backends: HashMap::new(),
        acme,
    };

    if let Err(errs) = config.validate() {
        bail!("cruma configuration validation failed: {errs:?}");
    }

    Ok((config, notes))
}

/// Offset listener ports to avoid clashes when running alongside the legacy stack.
pub fn apply_port_offset(cfg: &mut Configuration, offset: u16) -> anyhow::Result<()> {
    if offset == 0 {
        return Ok(());
    }

    for listener in cfg.listeners.iter_mut() {
        match listener {
            Listener::Http(h) => {
                h.port = NonZeroU16::new(
                    h.port
                        .get()
                        .checked_add(offset)
                        .context("http port offset overflow")?,
                )
                .context("http port became zero after offset")?;
            }
            Listener::Tls(tls) => {
                tls.port = NonZeroU16::new(
                    tls.port
                        .get()
                        .checked_add(offset)
                        .context("tls port offset overflow")?,
                )
                .context("tls port became zero after offset")?;
            }
            Listener::Tcp(tcp) => {
                tcp.port = NonZeroU16::new(
                    tcp.port
                        .get()
                        .checked_add(offset)
                        .context("tcp port offset overflow")?,
                )
                .context("tcp port became zero after offset")?;
            }
        }
    }

    Ok(())
}
