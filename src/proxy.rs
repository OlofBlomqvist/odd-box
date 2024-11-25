use core::panic;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use hyper::Version;
use hyper_rustls::ConfigBuilderExt;
use lazy_static::lazy_static;
use socket2::Socket;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::sync::RwLock;
use tokio_rustls::TlsAcceptor;
use crate::api::OddBoxAPI;
use crate::configuration::Hint;
use crate::configuration::ConfigWrapper;
use crate::global_state::GlobalState;
use crate::http_proxy::ProcMessage;
use crate::http_proxy::ReverseProxyService;
use crate::tcp_proxy;
use crate::http_proxy;
use crate::tcp_proxy::DataType;
use crate::tcp_proxy::ManagedStream;
use crate::tcp_proxy::PeekResult;
use crate::tcp_proxy::Peekable;
use crate::tcp_proxy::GenericManagedStream;
use crate::tcp_proxy::TunnelError;
use crate::types::app_state;
use crate::types::odd_box_event::Event;
use crate::types::proxy_state::ConnectionKey;
use crate::types::proxy_state::ProxyActiveTCPConnection;


use tokio_util::sync::CancellationToken;
use tokio::task::JoinHandle;

pub async fn listen(
    cfg: Arc<RwLock<ConfigWrapper>>, 
    initial_bind_addr: SocketAddr,
    initial_bind_addr_tls: SocketAddr, 
    tx: Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    shutdown_signal: Arc<Notify>
) {
    
    let mut previous_bind_addr = initial_bind_addr;
    let mut previous_bind_addr_tls = initial_bind_addr_tls;

    let mut http_cancel_token: Option<CancellationToken> = None;
    let mut https_cancel_token: Option<CancellationToken> = None;

    let mut http_task : Option<JoinHandle<()>> = None;
    let mut https_task : Option<JoinHandle<()>> = None;

    loop {
        
        // Read the current configuration
        let (current_bind_addr, current_bind_addr_tls) = {

            let config_read = cfg.read().await;

            let srv_ip = config_read.ip.clone().unwrap_or(initial_bind_addr.ip());

            let srv_port: u16 = config_read.http_port.unwrap_or(previous_bind_addr.port());
            let srv_tls_port: u16 = config_read.tls_port.unwrap_or(previous_bind_addr_tls.port());

            let http_bind_addr = SocketAddr::new(srv_ip, srv_port);
            let https_bind_addr = SocketAddr::new(srv_ip, srv_tls_port);

            (http_bind_addr, https_bind_addr)
        };

        // Check if the bind addresses have changed or if this is the first iteration
        if http_task.is_none() || https_task.is_none() || previous_bind_addr != current_bind_addr || previous_bind_addr_tls != current_bind_addr_tls {

            // Addresses have changed; need to restart the listeners
            if let Some(token) = http_cancel_token.take() {
                tracing::info!("http port has changed from {} to {}, shutting down http listener..",previous_bind_addr.port(),current_bind_addr.port());
                token.cancel(); 
                if let Some(http_task) = http_task.take() {
                    tracing::info!("waiting for http task to finish..");
                    http_task.await.expect("http task failed");
                    
                    tracing::info!("http task finished.");
                }
                
            }         

            if let Some(token) = https_cancel_token.take() {
                tracing::info!("https port has changed from {} to {}, shutting down http listener..",previous_bind_addr_tls.port(),current_bind_addr_tls.port());
                token.cancel();
                if let Some(https_task) = https_task.take() {
                    tracing::info!("waiting for https task to finish..");
                    https_task.await.expect("http task failed");
                    tracing::info!("https task finished.");
                }
            }

            

            let client_tls_config = tokio_rustls::rustls::ClientConfig::builder_with_protocol_versions(tokio_rustls::rustls::ALL_VERSIONS)
                // todo - add support for accepting self-signed certificates etc
                // .dangerous()
                // .with_custom_certificate_verifier(verifier)
                .with_native_roots()
                .expect("must be able to create tls configuration")
                .with_no_client_auth();

            let https_builder = hyper_rustls::HttpsConnectorBuilder::new()
                .with_tls_config(client_tls_config);
            
            let connector: hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector> = 
                https_builder.https_or_http().enable_all_versions().build();

            let executor = hyper_util::rt::TokioExecutor::new();

            let client = hyper_util::client::legacy::Client::builder(executor.clone())
                .http2_only(false)
                .build(connector.clone());

            let h2_client = hyper_util::client::legacy::Client::builder(executor)
                .http2_only(true)
                .build(connector);

            let terminating_proxy_service = ReverseProxyService { 
                connection_key: 0,
                configuration: Arc::new(cfg.read().await.clone()),
                resolved_target: None,
                state: state.clone(), 
                remote_addr: None, 
                tx: tx.clone(), 
                is_https_only: false,
                client,
                h2_client,
            };

            let new_http_cancel_token = CancellationToken::new();
            let new_https_cancel_token = CancellationToken::new();

            // Start new listeners with the new bind addresses
            let http_future = listen_http(
                current_bind_addr,
                tx.clone(),
                state.clone(),
                terminating_proxy_service.clone(),
                shutdown_signal.clone(),
                new_http_cancel_token.clone()
            );

            let https_future = listen_https(
                current_bind_addr_tls,
                tx.clone(),
                state.clone(),
                terminating_proxy_service.clone(),
                shutdown_signal.clone(),
                new_https_cancel_token.clone(),
            );

            let cloned_ct = new_http_cancel_token.clone();
            http_task = Some(tokio::spawn(async move {
                tokio::select! {
                    _ = http_future => {},
                    _ = cloned_ct.cancelled() => {
                        tracing::warn!("http listener cancelled");
                    },
                }
            }));

            let cloned_ct2 = new_https_cancel_token.clone();
            https_task = Some(tokio::spawn(async move {
                tokio::select! {
                    _ = https_future => {},
                    _ = cloned_ct2.cancelled() => {
                        tracing::warn!("https listener cancelled");
                    },
                }
            }));

            http_cancel_token = Some(new_http_cancel_token.clone());
            https_cancel_token = Some(new_https_cancel_token);

            previous_bind_addr = current_bind_addr;
            previous_bind_addr_tls = current_bind_addr_tls;
            
        }

        // Sleep for a while before checking again
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

lazy_static! {
    static ref ACTIVE_TCP_CONNECTIONS_SEMAPHORE : tokio::sync::Semaphore = tokio::sync::Semaphore::new(200);
} 

async fn listen_http(
    bind_addr: SocketAddr,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    terminating_service_template: ReverseProxyService,
    _shutdown_signal: Arc<Notify> ,
    cancel_token: CancellationToken,
) {
    
    use socket2::{Domain,Type};


    let socket = Socket::new(Domain::for_address(bind_addr), Type::STREAM, None).expect("should always be possible to create a tcp socket for tls");
    match socket.set_only_v6(false) {
        Ok(_) => {},
        Err(e) => tracing::trace!("Failed to set_only_vs: {e:?}")
    };
    match socket.set_reuse_address(true) { // annoying as hell otherwise for quick resets
        Ok(_) => {},
        Err(e) => tracing::warn!("Failed to set_reuse_address: {e:?}")
    }
    socket.bind(&bind_addr.into()).expect("we must be able to bind to https addr socket..");
    socket.listen(1024).expect("must be able to bind http listener.");
    let listener: std::net::TcpListener = socket.into();
    listener.set_nonblocking(true).expect("must be able to set_nonblocking on http listener");
    let tokio_listener = tokio::net::TcpListener::from_std(listener).expect("we must be able to listen to https port..");


    
    loop {

        {
            if cancel_token.is_cancelled() {
                tracing::warn!("exiting http server loop due to receiving cancel signal.");
                break;
            }

            if state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) {
                tracing::debug!("exiting http server loop due to receiving shutdown signal.");
                break;
            }
        }

        let permit = if let Ok(p) = ACTIVE_TCP_CONNECTIONS_SEMAPHORE.acquire().await {
            p
        } else {
            tracing::error!("Error acquiring semaphore permit!");
            break;
        };

        permit.forget();
        
        //tracing::info!("accepting http connection..");
        tokio::select! {
            _ = cancel_token.cancelled() => {
                tracing::info!("Cancellation token triggered, shutting down HTTP listener.");
                break;
            }
            x = tokio_listener.accept() => {
                match x {
                    Ok((tcp_stream,source_addr)) => {
                        
                        let mut service: ReverseProxyService = terminating_service_template.clone();
                        service.configuration = Arc::new(state.config.read().await.clone());
                        service.remote_addr = Some(source_addr);   
                        let tx = tx.clone();
                        let state = state.clone();
                        tokio::spawn(async move {           
                            handle_new_tcp_stream(None, service,tcp_stream, source_addr,tx.clone(),state.clone(),None)
                                .await;
                            ACTIVE_TCP_CONNECTIONS_SEMAPHORE.add_permits(1);
                        });
                        

                    }
                    Err(e) => {
                        tracing::warn!("error accepting tcp connection: {:?}", e);
                        ACTIVE_TCP_CONNECTIONS_SEMAPHORE.add_permits(1);
                        //break;
                    }
                }
            }
        }       
    }
    tracing::warn!("listen_http went bye bye.")

}


async fn listen_https(
    bind_addr: SocketAddr,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    terminating_service_template: ReverseProxyService,
    _shutdown_signal: Arc<Notify>,
    cancel_token: CancellationToken,
) {

    use socket2::{Domain,Type};

    let socket = Socket::new(Domain::for_address(bind_addr), Type::STREAM, None).expect("should always be possible to create a tcp socket for tls");
    match socket.set_only_v6(false) {
        Ok(_) => {},
        Err(e) => tracing::trace!("Failed to set_only_vs: {e:?}")
    };
    match socket.set_reuse_address(true) { // annoying as hell otherwise for quick resets
        Ok(_) => {},
        Err(e) => tracing::warn!("Failed to set_reuse_address: {e:?}")
    }
    socket.bind(&bind_addr.into()).expect("we must be able to bind to https addr socket..");
    socket.listen(1024).expect("we must be able to listen to https addr socket..");
    let listener: std::net::TcpListener = socket.into();
    listener.set_nonblocking(true).expect("must be able to set_nonblocking on https listener");
    let tokio_listener = tokio::net::TcpListener::from_std(listener).expect("we must be able to listen to https port..");
    
    let mut rustls_config = 
        tokio_rustls::rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_cert_resolver(state.cert_resolver.clone());
    
    if let Some(true) = state.config.read().await.alpn {
        rustls_config.alpn_protocols.push("h2".into());
        rustls_config.alpn_protocols.push("http/1.1".into());
    }

    let arced_tls_config = std::sync::Arc::new(rustls_config);

    loop {

        if cancel_token.is_cancelled() {
            tracing::warn!("exiting https server loop due to receiving cancel signal.");
            break;
        }

        if state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) {
            tracing::debug!("exiting http server loop due to receiving shutdown signal.");
            break;
        }

     
        let permit = if let Ok(p) = ACTIVE_TCP_CONNECTIONS_SEMAPHORE.acquire().await {
            p
        } else {
            tracing::error!("Error acquiring semaphore permit!");
            break;
        };

        permit.forget();

        
    
        let api = OddBoxAPI::new(state.clone());
        
        tokio::select! {
            _ = cancel_token.cancelled() => {
                tracing::info!("Cancellation token triggered, shutting down HTTPS listener.");
                break;
            }
            x = tokio_listener.accept() => {
                match x {
                    Ok((tcp_stream,source_addr)) => {
                    
                        let mut service: ReverseProxyService = terminating_service_template.clone();
                        service.configuration = Arc::new(state.config.read().await.clone());
                        service.remote_addr = Some(source_addr);  
                        let tx = tx.clone();
                        let arced_tls_config = Some(arced_tls_config.clone());
                        let state = state.clone();
                        tokio::spawn(async move {               
                            handle_new_tcp_stream(arced_tls_config,service,tcp_stream, source_addr,tx.clone(),state.clone(),Some(api))
                                .await;
                            ACTIVE_TCP_CONNECTIONS_SEMAPHORE.add_permits(1);
                        });
                        

                    }
                    Err(e) => {
                        tracing::warn!("error accepting tcp connection: {:?}", e);
                        ACTIVE_TCP_CONNECTIONS_SEMAPHORE.add_permits(1);
                        //break;
                    }
                }
            }
        }
    }
    
    tracing::warn!("listen_https went bye bye.")
}

// this will peek in to the incoming tcp stream and either create a direct tcp tunnel (passthru mode)
// or hand it off to the terminating http/https hyper services
async fn handle_new_tcp_stream(
    rustls_config: Option<std::sync::Arc<tokio_rustls::rustls::ServerConfig>>,
    mut fresh_service_template_with_source_info: ReverseProxyService,
    tcp_stream: TcpStream,
    source_addr:SocketAddr,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    api: Option<OddBoxAPI>
) {

    let mut peekable_tcp_stream = GenericManagedStream::from_tcp_stream(tcp_stream,state.clone());
    let peek_result =  peekable_tcp_stream.peek_managed_stream(source_addr).await;
    peekable_tcp_stream.seal();

    fresh_service_template_with_source_info.connection_key = *peekable_tcp_stream.get_id();

    // add to global tracking. we will update the state of this connection as it progresses through the system
    match &peekable_tcp_stream {
        GenericManagedStream::TCP(peekable_tcp_stream) => {
            tracing::trace!("Accepted TCP connection from {source_addr} - tls: {:?} ", peekable_tcp_stream.is_tls);
        },
        GenericManagedStream::TerminatedTLS(_managed_stream) => {
            tracing::trace!("Terminated TLS connection from {source_addr}.");
        },
    }

    peekable_tcp_stream.track();
    
    
    match peek_result {
        
        // we see that this is cleartext data, and we expect clear text data, and we also extracted a hostname by peeking.
        // at this point, we should check if the target is NOT configured for https (tls) before forwarding.
        Ok(PeekResult {
            typ,
            http_version,
            target_host: h2_authority_or_h1_host_header,
            is_h2c_upgrade
        }) => {


            let is_tls = typ == DataType::TLS;
            let ourl = state.config.read().await.odd_box_url.clone().unwrap_or(String::from("!"));
            match h2_authority_or_h1_host_header.as_ref().map(|x| x.as_str()) {
                Some("oddbox.localhost") |
                Some("odd-box.localhost") |
                Some("localhost") => {
                    if let Some(api) = api {
                        _ = api.handle_stream(peekable_tcp_stream,rustls_config).await;
                        return;
                    }
                },
                Some(x) => {
                    if x == &ourl {
                        tracing::trace!("handling incoming request from '{source_addr:?}' to odd-box system services thru odd-box-url: '{x}'.");
                        if let Some(api) = api {
                            _ = api.handle_stream(peekable_tcp_stream,rustls_config).await;
                            return;
                        }
                    }
                }
                _ => {}
            }


            // if is_tls {
            //     tracing::trace!("tls peeked type: {typ:?} - v:{http_version:?} - host: {h2_authority_or_h1_host_header:?}");
            // } else {
            //     tracing::trace!("tcp peeked type: {typ:?} - v:{http_version:?} - host: {h2_authority_or_h1_host_header:?}");

            // }

            let target_host_name = if let Some(n) = h2_authority_or_h1_host_header {
                n
            } else {
                tracing::warn!("No target host found in peeked data.. will use terminating proxy mode instead.");
                http_proxy::serve(fresh_service_template_with_source_info, peekable_tcp_stream).await;
                return;
            };
            
          
            if let Some(target) = state.try_find_site(&target_host_name).await {

                let cloned_target = target.clone();

                fresh_service_template_with_source_info.resolved_target = Some(cloned_target.clone());
                
                if target.is_hosted {

                    if let Some(cfg) = &target.hosted_target_config {
                        let hints : Vec<&crate::configuration::Hint> = cfg.hints.iter().flatten().collect();
                        if cfg.disable_tcp_tunnel_mode.unwrap_or_default() {
                            return use_fallback_mode(rustls_config, peekable_tcp_stream, fresh_service_template_with_source_info, FallbackReason::TunnelModeDisabled).await;
                        }
                        if let Some(Version::HTTP_2) = http_version {
                            if hints.iter().any(|h| **h==Hint::H2 ) {
                                tracing::trace!("Incoming http version is 2.0 and target supports it thru hints. Proceeding with tunnel mode.");
                            } else {
                                tracing::trace!("Incoming http version is 2.0 but no hints are provided for the target to support it. Falling back to terminating mode.");
                                return use_fallback_mode(rustls_config, peekable_tcp_stream, fresh_service_template_with_source_info,
                                    FallbackReason::IncomingHttp2ButTargetDoesNotSupportIt
                                ).await;
                            }
                        }
                    }
                    
                    let proc_state = {
                        match state.app_state.site_status_map.get(&target.host_name) {
                            Some(v) => Some(v.clone()),
                            _ => None
                        }
                    };

                    match proc_state {
                        None => {
                            tracing::warn!("error 0001 has occurred")
                        },
                        Some(app_state::ProcState::Stopped) 
                        | Some(app_state::ProcState::Starting) => {
                            _ = tx.send(ProcMessage::Start(target.host_name.clone()));
                            let thn = target.host_name.clone();
                            let mut has_started = false;
                            // done here to allow non-browser clients to reach the target socket without receiving unexpected loading screen html blobs
                            // as long as we are able to start the backing process within 10 seconds
                            tracing::debug!("handling an incoming request to a stopped target, waiting for {thn} to spin up - after this we will release the request to the terminating proxy and show a 'please wait' page instaead.");
                                
                            for _ in 1..100 {
                                match state.app_state.site_status_map.get(&target.host_name) {
                                    Some(my_ref) => {
                                        match my_ref.value() {
                                            app_state::ProcState::Running => {
                                                tracing::info!("{thn} is now running!");
                                                has_started = true;
                                                // give the hosted target some time to set up it's tcp listener
                                                tokio::time::sleep(Duration::from_millis(3000)).await;
                                                break
                                            },
                                            _ => { }
                                        }
                                    },
                                    _ => { }
                                }
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                            if has_started {

                                match tcp_proxy::ReverseTcpProxy::tunnel(
                                    peekable_tcp_stream,
                                     cloned_target, 
                                     is_tls,
                                     state.clone(),
                                     source_addr, 
                                     rustls_config.clone(),
                                     target_host_name,
                                     http_version,
                                     is_h2c_upgrade,
                                ).await {
                                    Ok(_) => {
                                        return;
                                    },
                                    Err(e) => {
                                        match e {
                                            TunnelError::NoUsableBackendFound(s) => {
                                                return use_fallback_mode(rustls_config, s, fresh_service_template_with_source_info, FallbackReason::NoBackendFound).await;

                                            },
                                            TunnelError::Unknown(e) => {
                                                tracing::warn!("Tunnel error: {e:?}");
                                                return;
                                            },
                                        };
                                    },
                                }
                                
                            } else {
                                tracing::trace!("{thn} is still not running...giving up.");
                                return;
                            }
                        }
                        , _  => {
                            match tcp_proxy::ReverseTcpProxy::tunnel(
                                peekable_tcp_stream, 
                                cloned_target, 
                                is_tls,
                                state.clone(),
                                source_addr, 
                                rustls_config.clone(),
                                target_host_name,
                                http_version,
                                is_h2c_upgrade
                            ).await {
                                Ok(_) => {
                                    return;
                                },
                                Err(e) => {
                                    match e {
                                        TunnelError::NoUsableBackendFound(s) => {
                                            return use_fallback_mode(rustls_config, s, fresh_service_template_with_source_info, FallbackReason::NoBackendFound).await;
                                        },
                                        TunnelError::Unknown(e) => {
                                            tracing::warn!("Tunnel error: {e:?}");
                                            return;
                                        }
                                    };
                                },
                            }
                        }
                    }

                } else {
                    
                    if let Some(cfg) = &target.remote_target_config {
                        if cfg.disable_tcp_tunnel_mode.unwrap_or_default() {
                            return use_fallback_mode(rustls_config, peekable_tcp_stream, fresh_service_template_with_source_info, FallbackReason::TunnelModeDisabled).await;
                        }
                        if let Some(Version::HTTP_2) = http_version {
                            let mut hints = cfg.backends.iter()
                                .flat_map(|b| b.hints.clone().unwrap_or_default());
                            if hints.any(|x|x==Hint::H2) {
                                tracing::trace!("Incoming http version is 2.0 and target supports it thru hints. Proceeding with tunnel mode.");
                            } else {
                                tracing::trace!("Incoming http version is 2.0, but all backends explicitly disallow H2, falling back to terminating mode.");
                                return use_fallback_mode(rustls_config, peekable_tcp_stream, fresh_service_template_with_source_info,
                                    FallbackReason::NoBackendFound
                                ).await;
                            }
                        }
                    }

                    match tcp_proxy::ReverseTcpProxy::tunnel(
                        peekable_tcp_stream, 
                        cloned_target, 
                        is_tls,
                        state.clone(),
                        source_addr,rustls_config.clone(),
                        target_host_name,
                        http_version,
                        is_h2c_upgrade
                    ).await {
                        Ok(_) => {
                            return;
                        },
                        Err(e) => {
                           match e {
                                TunnelError::NoUsableBackendFound(s) => {
                                    return use_fallback_mode(rustls_config, s, fresh_service_template_with_source_info, FallbackReason::NoBackendFound).await;
                                },
                                TunnelError::Unknown(e) => {
                                    tracing::warn!("Tunnel error: {e:?}");
                                    return;
                                }
                            };
                        },
                    }
                }
                
            } else {
                // fallback mode also handles directory services, and other non-hosted targets
                return use_fallback_mode(rustls_config, peekable_tcp_stream, fresh_service_template_with_source_info, FallbackReason::NoTargetFound).await;

            }
        }
        Err(e) => {
            match e {
                tcp_proxy::PeekError::H2PriorKnowledgeNeedsToBeTerminated => {
                    return use_fallback_mode(rustls_config, peekable_tcp_stream, fresh_service_template_with_source_info, FallbackReason::H2CPriorKnowledge).await;
                },
                tcp_proxy::PeekError::StreamIsClosed => {
                   return;
                },
                e => {
                    return use_fallback_mode(rustls_config, peekable_tcp_stream, fresh_service_template_with_source_info, FallbackReason::Unknown(
                        format!("Peek error: {:?}",e)
                    )).await;

                }
            }
        }
    }


    
}


#[derive(Debug)]
pub enum FallbackReason {
    TunnelModeDisabled,
    IncomingHttp2ButTargetDoesNotSupportIt,
    H2CPriorKnowledge, // when a clear text connection comes in with http2 prior knowledge and client did not pass a host/authority header
                       // we have to engage in the http2 session negotiation dance.. this can be handled by the terminating proxy service.
    Unknown(String),
    // This means there was no backend that can accept the incoming http request as is.
    // We will need to terminate the http session and establish new http connections to the backend.
    NoBackendFound,
    // Could be directory services etc. or just wrong host name
    NoTargetFound,
}

async fn use_fallback_mode(
    rustls_config: Option<std::sync::Arc<tokio_rustls::rustls::ServerConfig>>, 
    mut generic_managed_stream: GenericManagedStream, 
    mut fresh_service_template_with_source_info: ReverseProxyService,
    reason: FallbackReason
) {

    generic_managed_stream.add_event(format!("using fallback_mode - reason: {:?}",reason));
    
    match reason {
        FallbackReason::IncomingHttp2ButTargetDoesNotSupportIt => {
            tracing::debug!("Falling back to http terminating mode as the incoming connection is HTTP2, but the target does not support HTTP2");
        },
        FallbackReason::H2CPriorKnowledge => {
            tracing::debug!("Falling back to http terminating mode for http2 prior knowledge request");
        },
        FallbackReason::Unknown(reason) => {
           // tracing::warn!("falling back to terminating proxy mode because: {reason}");
            //tracing::error!("NOT ALLOWED DURING TESTING");
            tracing::warn!("ignoring incoming tcp connection because: {reason}");
            return;
        },
        FallbackReason::NoTargetFound => {
            // this is no problem as we expect incoming requests for dir servers etc.
            tracing::trace!("Falling back to terminating proxy mode because no hosted or remote target was found, no need for warnings");
        },
        FallbackReason::NoBackendFound => {
            tracing::trace!("Falling back to terminating proxy mode because no backend exists that can handle the incoming requests as is.");
        },
        FallbackReason::TunnelModeDisabled => {
            tracing::trace!("Using http termination as the target is configured to disallow tunnel mode")
        }
    }
    

    // // Neither TCP Tunnel mode nor Worm Hole mode is NOT possible if we got here!
    // //  - At this point we have determined that we are not going to use the tcp tunnel mode, and we will use the terminating proxy mode instead.
    // //  - If the incoming connection is a tls stream we will first terminate it here.


    match rustls_config {
        Some(tls_cfg) => {

            match generic_managed_stream {
                // GenericManagedStream::TLS(_peekable_tls_stream) => {
                //     tracing::error!("unexpected state: tls stream in handle_new_tcp_stream");
                // },
                GenericManagedStream::TCP(peekable_tcp_stream) => {
                            
                    let tls_acceptor = TlsAcceptor::from(tls_cfg.clone());
                    match tls_acceptor.accept(peekable_tcp_stream).await {
                        Ok(tls_stream) => {
                            fresh_service_template_with_source_info.is_https_only = true;
                            tracing::warn!("falling back to TLS termination combined with legacy http terminating mode");
                            let mut new_peekable = GenericManagedStream::from_terminated_tls_stream(ManagedStream::from_tls_stream(tls_stream));
                            new_peekable.seal();
                            new_peekable.update_tracked_info(|x| {
                                x.http_terminated = true;
                                x.tls_terminated = true;
                                x.incoming_connection_uses_tls = true;
                            });
                            new_peekable.add_event("Terminated incoming tls, redirecting tcp stream in to http terminating proxy service".to_string());
                            http_proxy::serve(fresh_service_template_with_source_info, new_peekable).await;
                        },
                        Err(e) => {
                            tracing::warn!("accept_tcp_stream_via_tls_terminating_proxy_service failed with error: {e:?}");
                            return 
                        }
                    }
                },
                terminated_stream => {
                    fresh_service_template_with_source_info.is_https_only = true;
                    terminated_stream.update_tracked_info(|x| {
                        x.http_terminated = true;
                        x.tls_terminated = true;
                        x.target = if let Some(v) = fresh_service_template_with_source_info.resolved_target.as_ref() {
                            let inner_target = (*v.as_ref()).clone();
                            Some(inner_target)
                        } else {
                            None
                        };
                    });
                    http_proxy::serve(fresh_service_template_with_source_info, terminated_stream).await;
                },
            }


        }, 
        _ => {
            generic_managed_stream.update_tracked_info(|x| {
                x.http_terminated = true;
                x.target = if let Some(v) = fresh_service_template_with_source_info.resolved_target.as_ref() {
                    let inner_target = (*v.as_ref()).clone();
                    Some(inner_target)
                } else {
                    None
                };
            });
            http_proxy::serve(fresh_service_template_with_source_info, generic_managed_stream).await;
        }
    }
}














pub fn add_or_update_connection(state:Arc<GlobalState>,mut connection:ProxyActiveTCPConnection) {
    if connection.resolved_connection_type.is_none() {
        let result = connection.get_connection_type();
        let result_str = result.to_string();
        connection.resolved_connection_type = Some(result);
        connection.resolved_connection_type_description = Some(result_str);
    }
    if let Some(key) = connection.connection_key_pointer.upgrade() {
        let app_state = state.app_state.clone();
        _ = app_state.statistics.active_connections.insert(*key, connection.clone());
        _ = state.global_broadcast_channel.send(Event::TcpEvent(crate::types::odd_box_event::TCPEvent::Open(connection)));    
    } else {
        tracing::warn!("Failed to add connection to global state, connection key was dropped.");
    }
}

pub fn mutate_tracked_connection(
    state:&Arc<GlobalState>,
    key:&ConnectionKey,
    mutator: impl FnOnce(&mut ProxyActiveTCPConnection) -> ()
)  {
    let app_state = state.app_state.clone();
    let guard = app_state.statistics.clone();
    let item = guard.active_connections.get_mut(key);
    if let Some(mut conn) = item {
        if conn.resolved_connection_type.is_none() {
            tracing::warn!("Resolved connection type is None, this should not happen.");
        }
        let v = conn.version;
        mutator(conn.value_mut());
        conn.version = v + 1;
        _ = state.global_broadcast_channel.send(Event::TcpEvent(crate::types::odd_box_event::TCPEvent::Update(conn.clone())));   
    }
}

pub fn del_connection(state:Arc<GlobalState>,key:&ConnectionKey) {
    let app_state = state.app_state.clone();
    let guard = app_state.statistics.clone();
    _ = guard.active_connections.remove(key);
    _ = state.global_broadcast_channel.send(Event::TcpEvent(crate::types::odd_box_event::TCPEvent::Close(*key))); 
}
