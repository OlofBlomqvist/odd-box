use std::sync::Arc;

use arc_swap::ArcSwap;
use cruma_proxy_lib::{proxying::ProxyService, termination::*};
use cruma_tunnels_lib::IncomingCrumaTlsStream;

use crate::global_state::GlobalState;

/// Collect allowed hostnames from the current odd-box config.
pub async fn extract_allowed_domain_names(state: Arc<GlobalState>) -> Vec<String> {
    let odd_box_config = state.config.read().await.clone();

    // In V4, the DashMap key is the backend_id which serves as the hostname
    let hosted_domain_names = odd_box_config
        .hosted_processes
        .iter()
        .map(|x| x.key().clone())
        .collect::<Vec<String>>();
    let remote_domains = odd_box_config
        .remote_sites
        .iter()
        .map(|x| x.key().clone())
        .collect::<Vec<String>>();
    let dir_site_domains = odd_box_config
        .static_sites
        .iter()
        .map(|x| x.key().clone())
        .collect::<Vec<String>>();
    let docker_containers = odd_box_config
        .docker_containers
        .iter()
        .map(|x| x.generate_host_name())
        .collect::<Vec<String>>();

    hosted_domain_names
        .into_iter()
        .chain(remote_domains)
        .chain(dir_site_domains)
        .chain(docker_containers)
        .collect::<Vec<String>>()
}

/// Spawn the Cruma tunnel agent and dispatch incoming streams through cruma_proxy_lib.
pub async fn cruma_thread(
    notify: Arc<tokio::sync::Notify>,
    state: Arc<GlobalState>,
    cruma_conf: Arc<ArcSwap<cruma_proxy_lib::types::Configuration>>,
) -> anyhow::Result<()> {
    let ct = tokio_util::sync::CancellationToken::new();
    use cruma_tunnels_lib::*;
    let config = AgentRuntimeConfig::default(AgentCredentials::anonymous())?;
    // Get TLS port from frontends config
    let port = state
        .config
        .read()
        .await
        .frontends
        .https
        .as_ref()
        .map(|h| h.port)
        .unwrap_or(4343);
    let reconnect = Arc::new(tokio::sync::Notify::new());
    let runtime = agent_runtime::start_agent_runtime(reconnect, config, ct.clone()).await?;

    let mut events = runtime.subscribe();
    let p = Arc::new(
        cruma_proxy_lib::termination::LocalDiskPersistence::new(&".odd-box-cruma-cache".into())
            .unwrap(),
    );

    loop {
        let state = state.clone();
        let persistence = p.clone();

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
                        | cruma_tunnels_lib::AgentEvent::AuthenticatedTunnelAssigned { assigned_domain, welcome_message } => {
                                tracing::info!(assigned_domain, welcome_message);
                                {
                                    let mut slot = state.app_state.cruma_assignment.write().await;
                                    *slot = Some(crate::types::app_state::CrumaAssignedDomain {
                                        assigned_domain,
                                        welcome_message,
                                    });
                                }
                            },
                            evt => {
                                tracing::trace!("Received event from server: {:#?}", evt);
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
                    let conf = cruma_conf.clone();
                    tokio::spawn(handle_stream(state,persistence,cruma_stream,port, conf));
                }
                None => {
                    tracing::info!("Runtime stopped");
                    break;
                }
            }
        }
        _ = ct.cancelled() => {
            tracing::info!("Agent runtime cancelled");
                break;
            }
        }
    }

    Ok(())
}

pub async fn handle_stream(
    _state: Arc<GlobalState>,
    p: Arc<LocalDiskPersistence>,
    cruma_stream: IncomingCrumaTlsStream,
    port: u16, // we dont care about this atm
    cruma_conf: Arc<ArcSwap<cruma_proxy_lib::types::Configuration>>,
) -> anyhow::Result<()> {
    let terminator = cruma_proxy_lib::termination::Terminator::new(p.clone(), cruma_conf.clone());
    let proxy_service = ProxyService::new(cruma_conf, terminator);

    let preface = match &cruma_stream {
        IncomingCrumaTlsStream::Quic { preface, .. }
        | IncomingCrumaTlsStream::Http2 { preface, .. } => preface.clone(),
    };

    match if cruma_stream.is_tls() {
        // If we are receiving tls streams in here, we need to terminate it ourselves prior to proxying.
        proxy_service
            .terminate_and_proxy(cruma_stream, port, preface.src.parse()?)
            .await
    } else {
        // Although our connection with cruma is TLS encrypted, we can still get non-TLS streams forwarded to us.
        // This means they were terminated on the cruma.io servers, so we need to proxy them as non-TLS here.
        let eport = _state
            .config
            .read()
            .await
            .frontends
            .http
            .as_ref()
            .map(|h| h.port)
            .unwrap_or(8080);
        proxy_service
            .proxy_non_tls(cruma_stream, eport, preface.src.parse()?)
            .await
    } {
        Ok(_) => {
            tracing::info!("Successfully proxied connection for {}", preface.src);
        }
        Err(e) => {
            tracing::error!(error=%e, "Failed to proxy connection for {}", preface.src);
        }
    }

    Ok(())
}
