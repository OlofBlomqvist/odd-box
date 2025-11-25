use std::{num::NonZero, sync::Arc};

use cruma_proxy_lib::{termination::*, types::*};
use cruma_tunnels_lib::AgentStream;
use tokio::sync::RwLock;

use crate::{global_state::GlobalState, http_proxy::ReverseProxyService};


pub async fn extract_allowed_domain_names(state: Arc<GlobalState>) -> Vec<String> {

    let odd_box_config = state.config.read().await.clone();

    let hosted_domain_names = odd_box_config.hosted_processes.iter().map(|x|x.host_name.clone()).collect::<Vec<String>>();
    let remote_domains = odd_box_config.remote_sites.iter().map(|x|x.host_name.clone()).collect::<Vec<String>>();
    let dir_site_domains = odd_box_config.dir_server.iter().flatten().map(|x| x.host_name.clone()).collect::<Vec<String>>();
    let docker_containers = odd_box_config.docker_containers.iter().map(|x| x.generate_host_name()).collect::<Vec<String>>();

    hosted_domain_names
        .into_iter()
        .chain(remote_domains)
        .chain(dir_site_domains)
        .chain(docker_containers)
        .collect::<Vec<String>>()
}

pub async fn cruma_thread(
    notify: Arc<tokio::sync::Notify>,
    state: Arc<GlobalState>,
    terminating_proxy_service:  ReverseProxyService
) -> anyhow::Result<()> {

    let ct = tokio_util::sync::CancellationToken::new();
    use cruma_tunnels_lib::*;
    let config = AgentRuntimeConfig::default(AgentCredentials::anonymous())?;
    let port = state.config.read().await.tls_port.unwrap_or(4343);
    let mut runtime = agent_runtime::start_agent_runtime(config, ct.clone()).await?;

    let mut events = runtime.subscribe();
    let p = Arc::new(
        cruma_proxy_lib::termination::LocalDiskPersistence::new(
            &".odd-box-cruma-cache".into()
        ).unwrap()
    );

    loop {

        let state = state.clone();
        let persistence = p.clone();
        let service: ReverseProxyService = terminating_proxy_service.clone();

        tokio::select! {
            () = notify.notified() => {
                tracing::info!("App seems to be closing down - cancelling agent runtime");
                ct.cancel();
                break;
            },
            evt = events.recv() => {
                match evt {
                    Ok(agent_event) => {
                        match agent_event {
                            cruma_tunnels_lib::AgentEvent::AnonymousTunnelAssigned { assigned_domain, welcome_message }
                            | cruma_tunnels_lib::AgentEvent::AuthenticatedTunnelAssigned { assigned_domain, welcome_message
                                } => {
                                    tracing::info!(assigned_domain, welcome_message);
                                },
                                evt => {
                                    tracing::info!("Received event from server: {:#?}", evt);
                                }
                        }
                    },
                    Err(err) => {
                        tracing::error!(error=%err, "Failed to receive agent event");
                    },
                }
            }
            value = runtime.next() => {
                tracing::info!("Received agent stream");
                match value {
                    Some(cruma_stream) => {
                        // spawn bg so we can handle multiple concurrent requests
                        tokio::spawn(handle_stream(state,persistence,cruma_stream,service,port));
                    }
                    None => {
                        tracing::info!("Runtime stopped");
                        break;
                    }
                }
            }
            // Allow the application to quit
            _ = ct.cancelled() => {
                tracing::info!("Agent runtime cancelled");
                    break;
                }
            }
    }

    Ok(())
}

pub fn add_or_update(
    state: Arc<GlobalState>,
    terminating_proxy_service:  ReverseProxyService,
    my_id: u64,
    sni: String,
    src: String,
    shared_id: Arc<u64>
) {
    crate::proxy::add_or_update_connection(
        state.clone(),
        crate::types::proxy_state::ProxyActiveTCPConnection {
            is_grpc: None,
            is_websocket: None,
            http_version: None,
            incoming_sni: Some(sni.clone()),
            resolved_connection_type: Some(crate::types::connection_type::ConnectionType::TlsTermination),
            resolved_connection_type_description: Some("CRUMA STREAM".into()),
            odd_box_socket: None,
            client_socket_address: terminating_proxy_service.source_addr.clone(),
            connection_key: my_id.clone(),
            connection_key_pointer: std::sync::Arc::<u64>::downgrade(&shared_id),
            client_addr_string: src,
            incoming_connection_uses_tls: true,
            tls_terminated: true,
            http_terminated:true,
            outgoing_tunnel_type: None,
            version: 2

        }
    );
}
pub async fn handle_stream(
    state: Arc<GlobalState>,
    p: Arc<LocalDiskPersistence>,
    cruma_stream: AgentStream,
    mut terminating_proxy_service:  ReverseProxyService,
    port: u16,

) -> anyhow::Result<()> {


    let active_domains = extract_allowed_domain_names(state.clone()).await.iter().map(|domain| {
        HostPattern::Exact { value: domain.to_string() }
    }).collect::<Vec<HostPattern>>();


    let cruma_proxy_conf = Arc::new(RwLock::new(cruma_proxy_lib::types::Configuration {
        listeners: vec![
            cruma_proxy_lib::types::Listener::Tls(cruma_proxy_lib::types::TlsListener {
                port: NonZero::new(port).unwrap(),
                alpn: cruma_proxy_lib::types::AlpnMode::Http1_1AndHttp2,
                routes: NonEmptyVec(vec![
                    TlsRoute {
                        name: "*".into(),
                        rule: TlsMatch {
                            sni: Some(HostPattern::OneOf(active_domains)),
                            alpn: None
                        },
                        action: cruma_proxy_lib::types::TlsAction::TerminateForHTTP {
                            cert_mode: CertMode::AcmeAlpn,
                            http: NonEmptyVec::new(HttpRoute {
                                name: "default".into(),
                                priority: 0,
                                filter: cruma_proxy_lib::types::HttpMatch::Any,
                                middlewares: vec![],
                                target: cruma_proxy_lib::types::Target::Backend { backend: "default".into() }
                            })
                        }
                    }
                ])
            })
        ],
        web_backends: std::collections::HashMap::new(),
        tcp_backends: std::collections::HashMap::new(),
    }));

    let terminator = Arc::new(cruma_proxy_lib::termination::Terminator::new(p.clone(), cruma_proxy_conf.clone()));

    let my_id = crate::generate_unique_id();
    let shared_id = Arc::new(my_id);

    match cruma_stream {
        AgentStream::Quic { stream, preface } => {
            tracing::info!("Received QUIC stream");
            match terminator.handle_tls_request(port, PeekableStream::new(stream)).await? {
                cruma_proxy_lib::termination::IncomingStream::TlsTerminatedForHttpBackends { sni, _negotiated_alpn, _issuer, stream } => {
                    terminating_proxy_service.is_https = true;
                    terminating_proxy_service.sni = Some(sni.clone());
                    terminating_proxy_service.configuration = Arc::new(state.config.read().await.clone());
                    terminating_proxy_service.source_addr = Some(preface.src.parse()?);
                    _ = state.app_state.statistics.total_accepted_tcp_connections.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let permit = crate::proxy::ACTIVE_TCP_CONNECTIONS_SEMAPHORE.acquire().await?;
                    add_or_update(state, terminating_proxy_service.clone(), my_id, sni, preface.src, shared_id);
                    match crate::http_proxy::SERVER_ONE.serve_connection_with_upgrades(hyper_util::rt::TokioIo::new(stream), terminating_proxy_service).await {
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("Error serving CRUMA-QUIC connection: {}", e);
                        }
                    };
                    drop(permit);
                },
                x => {
                    tracing::warn!("Refusing to serve CRUMA connection - {:?}", x);
                }
            }
        },
        AgentStream::Http2 { stream, preface } => {
            terminating_proxy_service.is_https = true;
            terminating_proxy_service.sni = Some(preface.sni.clone());
            terminating_proxy_service.configuration = Arc::new(state.config.read().await.clone());
            terminating_proxy_service.source_addr = Some(preface.src.parse()?);
            _ = state.app_state.statistics.total_accepted_tcp_connections.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let permit = crate::proxy::ACTIVE_TCP_CONNECTIONS_SEMAPHORE.acquire().await?;
            add_or_update(state, terminating_proxy_service.clone(), my_id, preface.sni.clone(), preface.src, shared_id);
            match crate::http_proxy::SERVER_ONE.serve_connection_with_upgrades(hyper_util::rt::TokioIo::new(stream), terminating_proxy_service).await {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Error serving CRUMA-H2 connection: {}", e);
                }
            };
            drop(permit);
        },
    }
    Ok(())
}
