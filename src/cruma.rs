use std::sync::Arc;

use arc_swap::ArcSwap;
use cruma_proxy_lib::{proxying::ProxyService, termination::*};
use cruma_tunnels_lib::{AgentCredentials, IncomingCrumaTlsStream};

use crate::global_state::GlobalState;

/// Spawn the Cruma tunnel agent and dispatch incoming streams through cruma_proxy_lib.
pub async fn cruma_thread(
    notify: Arc<tokio::sync::Notify>,
    state: Arc<GlobalState>,
    cruma_conf: Arc<ArcSwap<cruma_proxy_lib::types::Configuration>>,
    credentials: AgentCredentials,
) -> anyhow::Result<()> {
    let ct = tokio_util::sync::CancellationToken::new();
    use cruma_tunnels_lib::*;
    let config = AgentRuntimeConfig::default(credentials)?;

    let reconnect = Arc::new(tokio::sync::Notify::new());
    let runtime = agent_runtime::start_agent_runtime(reconnect, config, ct.clone()).await?;
    let mut transport_rx = runtime.transport_snapshot_rx();
    let transport_state = state.clone();
    tokio::spawn(async move {
        loop {
            if transport_rx.changed().await.is_err() {
                break;
            }
            let snapshot = transport_rx.borrow().clone();
            transport_state.cruma_transports.store(Arc::new(snapshot));
        }
    });

    let mut events = runtime.subscribe();
    let p = Arc::new(
        cruma_proxy_lib::termination::LocalDiskPersistence::new(&".odd-box-cruma-cache".into())
            .unwrap(),
    );
    // Keep one proxy service for the lifetime of the cruma runtime so outbound
    // HTTP client pools/cache are reused across incoming streams.
    let terminator = cruma_proxy_lib::termination::Terminator::new(p.clone(), cruma_conf.clone());
    let mut proxy_service = ProxyService::new(cruma_conf.clone(), terminator);
    proxy_service.set_capture_store(state.http_capture_store.clone());
    let proxy_service = Arc::new(proxy_service);

    loop {
        let state = state.clone();
        let proxy_service = proxy_service.clone();

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
                                state.cruma_assignment.store(Some(std::sync::Arc::new(
                                    crate::global_state::CrumaAssignedDomain {
                                        assigned_domain,
                                        welcome_message,
                                    },
                                )));
                                // Rebuild proxy configs immediately so tunnel host patterns include
                                // the newly assigned cruma domain (e.g. <host>.<assigned-domain>).
                                crate::cruma_integration::rebuild_cruma_config(state.clone());
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
                    tokio::spawn(async move {
                        match handle_stream(state, cruma_stream, conf, proxy_service).await {
                            Ok(()) => {}
                            Err(e) => {
                                tracing::error!(error=%e, "Cruma stream handler failed");
                            }
                        }
                    });
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

/// Maximum time we allow a single cruma stream to be handled before we
/// give up. This prevents hung TLS handshakes or misbehaving clients
/// from tying up resources forever.
const STREAM_HANDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

pub async fn handle_stream(
    state: Arc<GlobalState>,
    cruma_stream: IncomingCrumaTlsStream,
    cruma_conf: Arc<ArcSwap<cruma_proxy_lib::types::Configuration>>,
    proxy_service: Arc<ProxyService<LocalDiskPersistence>>,
) -> anyhow::Result<()> {
    let preface = match &cruma_stream {
        IncomingCrumaTlsStream::Quic { preface, .. }
        | IncomingCrumaTlsStream::Http2 { preface, .. } => preface.clone(),
    };

    let is_tls = cruma_stream.is_tls();
    let src = preface.src.clone();

    tracing::debug!(
        src = %src,
        is_tls = is_tls,
        "Handling incoming cruma stream"
    );

    // Wrap the actual proxying work inside a timeout and catch_unwind so
    // that panics inside cruma-proxy-lib (e.g. during TLS termination of
    // an unexpected stream) do not kill the task silently.
    let result = tokio::time::timeout(
        STREAM_HANDLE_TIMEOUT,
        proxy_stream(state, cruma_stream, cruma_conf, proxy_service, is_tls),
    )
    .await;

    match result {
        Ok(Ok(())) => {
            tracing::info!("Successfully proxied connection for {}", src);
        }
        Ok(Err(e)) => {
            tracing::error!("{:#?}", e);
            for x in e.chain() {
                tracing::error!("Caused by: {}", x);
            }
            tracing::error!(
                error = ?e,
                src = %src,
                is_tls = is_tls,
                "Failed to proxy cruma connection"
            );
        }
        Err(_elapsed) => {
            tracing::warn!(
                src = %src,
                is_tls = is_tls,
                timeout_secs = STREAM_HANDLE_TIMEOUT.as_secs(),
                "Cruma stream handler timed out — dropping connection"
            );
        }
    }

    Ok(())
}

/// Inner helper that does the actual proxying work.
/// Separated from `handle_stream` so we can wrap it in timeout cleanly.
async fn proxy_stream(
    state: Arc<GlobalState>,
    cruma_stream: IncomingCrumaTlsStream,
    cruma_conf: Arc<ArcSwap<cruma_proxy_lib::types::Configuration>>,
    proxy_service: Arc<ProxyService<LocalDiskPersistence>>,
    is_tls: bool,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let preface = match &cruma_stream {
        IncomingCrumaTlsStream::Quic { preface, .. }
        | IncomingCrumaTlsStream::Http2 { preface, .. } => preface.clone(),
    };

    // For cruma ingress we always route against the HTTPS listener port.
    // This applies to both TLS and already-terminated non-TLS streams.
    let incoming_port = cruma_conf
        .load_full()
        .listeners
        .iter()
        .find_map(|listener| match listener {
            cruma_proxy_lib::types::Listener::Tls(tls) => Some(tls.port.get()),
            _ => None,
        })
        .or_else(|| state.config.load().frontends.https.as_ref().map(|h| h.port))
        .unwrap_or(443);

    if is_tls {
        // If we are receiving tls streams in here, we need to terminate it ourselves prior to proxying.
        let src_addr = preface.src.parse().with_context(|| {
            format!(
                "Failed to parse source address '{}' for TLS stream",
                preface.src
            )
        })?;

        proxy_service
            .terminate_and_proxy(cruma_stream, incoming_port, src_addr)
            .await
            .with_context(|| {
                format!(
                    "TLS termination + proxy failed for stream from '{}' on port {}",
                    preface.src, incoming_port
                )
            })?;
    } else {
        // Cruma can forward already-terminated streams to us.
        // Route those through the HTTPS listener port as well.

        let src_addr = preface.src.parse().with_context(|| {
            format!(
                "Failed to parse source address '{}' for non-TLS stream",
                preface.src
            )
        })?;

        proxy_service
            .proxy_edge_terminated(
                cruma_stream,
                incoming_port,
                src_addr,
                preface.sni.clone(),
                None,
            )
            .await
            .with_context(|| {
                format!(
                    "Non-TLS proxy failed for stream from '{}' on port {}",
                    preface.src, incoming_port
                )
            })?;
    }

    Ok(())
}
