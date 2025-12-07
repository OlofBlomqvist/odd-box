use std::collections::HashMap;
use std::num::NonZeroU16;

use anyhow::{bail, Context};
use cruma_proxy_lib::types::*;

use crate::configuration::{ConfigWrapper, Hint};
use crate::docker::ContainerProxyTarget;

#[derive(Debug, Default, Clone)]
pub struct BuildNotes {
    pub unsupported: Vec<String>,
}

fn non_zero_port(port: u16, label: &str) -> anyhow::Result<NonZeroU16> {
    NonZeroU16::new(port).with_context(|| format!("{label} must be non-zero"))
}

fn host_pattern(host: &str, capture_subdomains: Option<bool>) -> HostPattern {
    if capture_subdomains.unwrap_or(false) {
        HostPattern::Base {
            value: host.to_string(),
        }
    } else {
        HostPattern::Exact {
            value: host.to_string(),
        }
    }
}

fn upstream_proto(hints: Option<&Vec<Hint>>, https: bool) -> HttpUpstreamProto {
    let hints = hints.map(|h| h.as_slice()).unwrap_or_default();
    if hints.iter().any(|h| matches!(h, Hint::H2)) {
        HttpUpstreamProto::H2
    } else if hints.iter().any(|h| matches!(h, Hint::H2C)) {
        HttpUpstreamProto::H2C
    } else if hints.iter().any(|h| matches!(h, Hint::H2CPK)) {
        HttpUpstreamProto::H2CPK
    } else if https {
        HttpUpstreamProto::H11
    } else {
        HttpUpstreamProto::H11
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
        },
    }
}

fn to_endpoint(addr: &str, port: u16) -> Option<Endpoint> {
    NonZeroU16::new(port).map(|p| Endpoint {
        addr: addr.to_string(),
        port: p,
    })
}

/// Build a cruma_proxy_lib Configuration from the current OddBox config.
///
/// This focuses on HTTP/HTTPS hosting; TCP passthrough and advanced features are not mapped yet.
pub fn build_config(cfg: &ConfigWrapper) -> anyhow::Result<(Configuration, BuildNotes)> {
    let mut notes = BuildNotes::default();
    let mut web_backends: HashMap<WebBackendId, WebBackend> = HashMap::new();
    let mut http_routes: Vec<HttpRoute> = Vec::new();

    let http_port = cfg.http_port.unwrap_or(8080);
    let tls_port = cfg.tls_port.unwrap_or(4343);
    let loopback_addr = if cfg.use_loopback_ip_for_procs.unwrap_or(true) {
        "127.0.0.1"
    } else {
        "localhost"
    };

    // Hosted processes
    for hosted in cfg.hosted_processes.iter() {
        let hosted = hosted.value().clone();
        let backend_id = WebBackendId(format!("hosted::{}", hosted.host_name));
        let port = hosted
            .active_port
            .or(hosted.port)
            .unwrap_or(if hosted.https.unwrap_or(false) {
                443
            } else {
                80
            });
        let Some(ep) = to_endpoint(loopback_addr, port) else {
            continue;
        };
        let backend = WebBackend {
            id: backend_id.clone(),
            protocol: upstream_proto(hosted.hints.as_ref(), hosted.https.unwrap_or(false)),
            endpoints: NonEmptyVec(vec![ep]),
            origin_tls: hosted
                .https
                .unwrap_or(false)
                .then_some(default_origin_tls()),
        };
        web_backends.insert(backend_id.clone(), backend);
        http_routes.push(http_route(
            hosted.host_name.clone(),
            host_pattern(&hosted.host_name, hosted.capture_subdomains),
            backend_id,
        ));
    }

    // Remote targets
    for remote in cfg.remote_sites.iter() {
        let remote = remote.value().clone();
        let backend_id = WebBackendId(format!("remote::{}", remote.host_name));

        let mut endpoints = Vec::new();
        for be in remote.backends.iter() {
            let port = if be.port == 0 {
                if be.https.unwrap_or(false) {
                    443
                } else {
                    80
                }
            } else {
                be.port
            };
            if let Some(ep) = to_endpoint(&be.address, port) {
                endpoints.push(ep);
            }
        }
        if endpoints.is_empty() {
            notes
                .unsupported
                .push(format!("Remote target '{}' has no endpoints", remote.host_name));
            continue;
        }

        let backend = WebBackend {
            id: backend_id.clone(),
            protocol: upstream_proto(
                remote.backends.first().and_then(|b| b.hints.as_ref()),
                remote
                    .backends
                    .first()
                    .and_then(|b| b.https)
                    .unwrap_or(false),
            ),
            endpoints: NonEmptyVec(endpoints),
            origin_tls: remote
                .backends
                .first()
                .and_then(|b| b.https)
                .unwrap_or(false)
                .then_some(OriginTls {
                    // TODO : we need to implement actual logic for sending the expected/configured/correct SNI
                    sni: OriginTlsSni::Custom("lobste.rs".into()),
                    trust_insecure_certificates: false,
                    ca_file: None,
                    client_cert: None,
                }), // default_origin_tls()
        };
        web_backends.insert(backend_id.clone(), backend);
        http_routes.push(http_route(
            remote.host_name.clone(),
            host_pattern(&remote.host_name, remote.capture_subdomains),
            backend_id,
        ));
    }

    // Dir servers are not implemented in cruma_proxy_lib; respond with 501 for now.
    if let Some(dir_servers) = &cfg.dir_server {
        for dir in dir_servers {
            // TODO: implement static hosting via cruma backends instead of placeholder.
            http_routes.push(respond_route(
                dir.host_name.clone(),
                host_pattern(&dir.host_name, dir.capture_subdomains),
                501,
                "dir_server not yet supported via cruma proxy",
            ));
            notes.unsupported.push(format!(
                "dir_server '{}' routed to placeholder 501 response",
                dir.host_name
            ));
        }
    }

    // Docker containers (treated like remotes)
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
                notes
                    .unsupported
                    .push(format!("Docker target '{}' has invalid port {}", host, cont.port));
                continue;
            }
        };

        let backend = WebBackend {
            id: backend_id.clone(),
            protocol: upstream_proto(Some(&cont.hints), cont.tls),
            endpoints: NonEmptyVec(vec![ep]),
            origin_tls: cont.tls.then_some(default_origin_tls()),
        };
        web_backends.insert(backend_id.clone(), backend);
        http_routes.push(http_route(
            host.clone(),
            host_pattern(&host, cont.capture_subdomains),
            backend_id,
        ));
    }

    // Admin/API hostnames fallback
    for admin_host in [
        Some("oddbox.localhost".to_string()),
        Some("odd-box.localhost".to_string()),
        cfg.odd_box_url.clone(),
    ]
    .into_iter()
    .flatten()
    {
        // TODO: route admin API/UI through cruma backends rather than hardcoded 501.
        http_routes.push(respond_route(
            admin_host.clone(),
            host_pattern(&admin_host, Some(false)),
            501,
            "admin API not yet wired through cruma proxy",
        ));
    }

    if http_routes.is_empty() {
        http_routes.push(HttpRoute {
            name: "fallback-404".into(),
            priority: 0,
            filter: HttpMatch::Any,
            middlewares: Vec::new(),
            target: Target::Respond {
                status: 404,
                body: None,
            },
        });
    }

    let http_routes = NonEmptyVec(http_routes);

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
                    cert_mode: CertMode::SelfSigned,
                    http: http_routes,
                },
            }]),
        }),
    ];

    let config = Configuration {
        listeners,
        web_backends,
        tcp_backends: HashMap::new(),
    };

    notes
        .unsupported
        .push("TCP passthrough/tunnel mode not yet mapped to cruma".into());

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
                        .checked_add(offset as u16)
                        .context("http port offset overflow")?,
                )
                .context("http port became zero after offset")?;
            }
            Listener::Tls(tls) => {
                tls.port = NonZeroU16::new(
                    tls.port
                        .get()
                        .checked_add(offset as u16)
                        .context("tls port offset overflow")?,
                )
                .context("tls port became zero after offset")?;
            }
            Listener::Tcp(tcp) => {
                tcp.port = NonZeroU16::new(
                    tcp.port
                        .get()
                        .checked_add(offset as u16)
                        .context("tcp port offset overflow")?,
                )
                .context("tcp port became zero after offset")?;
            }
        }
    }

    Ok(())
}
