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

    // Get TLS port from frontends config
    let port = state
        .config
        .load_full()
        .frontends
        .https
        .as_ref()
        .map(|h| h.port)
        .unwrap_or(4343);
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
                                state.cruma_assignment.store(Some(std::sync::Arc::new(
                                    crate::global_state::CrumaAssignedDomain {
                                        assigned_domain,
                                        welcome_message,
                                    },
                                )));
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
                        match handle_stream(state, persistence, cruma_stream, port, conf).await {
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
    _state: Arc<GlobalState>,
    p: Arc<LocalDiskPersistence>,
    cruma_stream: IncomingCrumaTlsStream,
    port: u16, // we dont care about this atm
    cruma_conf: Arc<ArcSwap<cruma_proxy_lib::types::Configuration>>,
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
        proxy_stream(_state, p, cruma_stream, port, cruma_conf, is_tls),
    )
    .await;

    match result {
        Ok(Ok(())) => {
            tracing::info!("Successfully proxied connection for {}", src);
        }
        Ok(Err(e)) => {
            tracing::error!(
                error = %e,
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
    p: Arc<LocalDiskPersistence>,
    cruma_stream: IncomingCrumaTlsStream,
    port: u16,
    cruma_conf: Arc<ArcSwap<cruma_proxy_lib::types::Configuration>>,
    is_tls: bool,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let terminator = cruma_proxy_lib::termination::Terminator::new(p.clone(), cruma_conf.clone());
    let mut proxy_service = ProxyService::new(cruma_conf, terminator);

    // Wire the shared HTTP capture store into the proxy so that every proxied
    // exchange is recorded when traffic inspection is enabled.
    proxy_service.set_capture_store(state.http_capture_store.clone());

    let preface = match &cruma_stream {
        IncomingCrumaTlsStream::Quic { preface, .. }
        | IncomingCrumaTlsStream::Http2 { preface, .. } => preface.clone(),
    };

    if is_tls {
        // If we are receiving tls streams in here, we need to terminate it ourselves prior to proxying.
        let src_addr = preface.src.parse().with_context(|| {
            format!(
                "Failed to parse source address '{}' for TLS stream",
                preface.src
            )
        })?;

        proxy_service
            .terminate_and_proxy(cruma_stream, port, src_addr)
            .await
            .with_context(|| {
                format!(
                    "TLS termination + proxy failed for stream from '{}' on port {}",
                    preface.src, port
                )
            })?;
    } else {
        // Although our connection with cruma is TLS encrypted, we can still get non-TLS streams forwarded to us.
        // This means they were terminated on the cruma.io servers, so we need to proxy them as non-TLS here.
        let cfg = state.config.load_full();
        let eport = cfg.frontends.http.as_ref().map(|h| h.port).unwrap_or(8080);

        let src_addr = preface.src.parse().with_context(|| {
            format!(
                "Failed to parse source address '{}' for non-TLS stream",
                preface.src
            )
        })?;

        proxy_service
            .proxy_non_tls(cruma_stream, eport, src_addr)
            .await
            .with_context(|| {
                format!(
                    "Non-TLS proxy failed for stream from '{}' on port {}",
                    preface.src, eport
                )
            })?;
    }

    Ok(())
}
