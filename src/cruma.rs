use std::sync::Arc;

use cruma_proxy_lib::{proxying::ProxyService, termination::*};
use cruma_tunnels_lib::IncomingCrumaTlsStream;
use tokio::sync::RwLock;

use crate::global_state::GlobalState;

/// Collect allowed hostnames from the current odd-box config.
pub async fn extract_allowed_domain_names(state: Arc<GlobalState>) -> Vec<String> {
    let odd_box_config = state.config.read().await.clone();

    let hosted_domain_names = odd_box_config
        .hosted_processes
        .iter()
        .map(|x| x.host_name.clone())
        .collect::<Vec<String>>();
    let remote_domains = odd_box_config
        .remote_sites
        .iter()
        .map(|x| x.host_name.clone())
        .collect::<Vec<String>>();
    let dir_site_domains = odd_box_config
        .dir_server
        .iter()
        .flatten()
        .map(|x| x.host_name.clone())
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
    cruma_conf: Arc<RwLock<cruma_proxy_lib::types::Configuration>>,
) -> anyhow::Result<()> {
    let ct = tokio_util::sync::CancellationToken::new();
    use cruma_tunnels_lib::*;
    let config = AgentRuntimeConfig::default(AgentCredentials::anonymous())?;
    let port = state.config.read().await.tls_port.unwrap_or(4343);
    let mut runtime = agent_runtime::start_agent_runtime(config, ct.clone()).await?;

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
    port: u16,
    cruma_conf: Arc<RwLock<cruma_proxy_lib::types::Configuration>>,
) -> anyhow::Result<()> {

    // TODO: dont recreate per stream
    let terminator = cruma_proxy_lib::termination::Terminator::new(p.clone(), cruma_conf.clone());
    let proxy_service = ProxyService::new(cruma_conf,terminator);

    if cruma_stream.is_tls() {
        // proxy_service.proxy_traffic(s, client_addr);
        todo! ()
    } else {
        todo!()
    }

    Ok(())
}
