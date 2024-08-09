use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use socket2::Socket;
use tokio::net::TcpSocket;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio_rustls::TlsAcceptor;

use crate::configuration::ConfigWrapper;
use crate::global_state::GlobalState;
use crate::http_proxy::SomeIo;
use crate::http_proxy::ProcMessage;
use crate::http_proxy::ReverseProxyService;
use crate::tcp_proxy;
use crate::http_proxy;
use crate::tcp_proxy::DataType;
use crate::tcp_proxy::PeekResult;
use crate::tcp_proxy::ReverseTcpProxyTarget;
use crate::tcp_proxy::ReverseTcpProxyTargets;
use crate::types::app_state;
use crate::types::app_state::ProcState;

pub async fn listen(
    _cfg: std::sync::Arc<tokio::sync::RwLock<ConfigWrapper>>, 
    bind_addr: SocketAddr,
    bind_addr_tls: SocketAddr, 
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: GlobalState,
    shutdown_signal: Arc<Notify> 
)  {

    
    
    // create this from the state.
    let tcp_targets: Arc<ReverseTcpProxyTargets> = Arc::new(ReverseTcpProxyTargets {
        global_state: state.clone()
    });

    let terminating_proxy_service = ReverseProxyService { 
        state:state.clone(), 
        remote_addr: None, 
        tx:tx.clone(), 
        is_https_only:false
    };
    
  
     tokio::join!(


        listen_http(
            bind_addr.clone(),
            tx.clone(),
            state.clone(),
            terminating_proxy_service.clone(),    
            tcp_targets.clone(),
            shutdown_signal.clone()
        ),

        listen_https(
            bind_addr_tls.clone(),
            tx.clone(),
            state.clone(),
            terminating_proxy_service.clone(),    
            tcp_targets.clone(),
            shutdown_signal.clone()
        ),
        
    );


    
} 


async fn listen_http(
    bind_addr: SocketAddr,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: GlobalState,
    terminating_service_template: ReverseProxyService,
    targets: Arc<ReverseTcpProxyTargets>,
    shutdown_signal: Arc<Notify> 
) {
    
    let socket = TcpSocket::new_v4().expect("new v4 socket should always work");
    socket.set_reuseaddr(true).expect("set reuseaddr fail?");
    socket.bind(bind_addr).expect(&format!("must be able to bind http serveraddr {bind_addr:?}"));
    let listener = socket.listen(128).expect("must be able to bind http listener.");

    loop {

        //tracing::trace!("waiting for new http connection..");
       
        tokio::select!{ 
            Ok((tcp_stream, source_addr)) = listener.accept() => {
                let shutdown_signal = shutdown_signal.clone();
                tracing::trace!("tcp listener accepted a new http connection");
                let mut service: ReverseProxyService = terminating_service_template.clone();
                service.remote_addr = Some(source_addr);
                let targets = targets.clone().reverse_tcp_proxy_targets().await;         
                let tx = tx.clone();
                let state = state.clone();
                tokio::spawn( async move { 
                    tokio::select!{ 
                        _ = handle_new_tcp_stream(None,service, tcp_stream, source_addr, targets, false,tx.clone(),state.clone()) => {
                            tracing::trace!("http tcp stream handled")
                        }
                        _ = shutdown_signal.notified() => {
                            eprintln!("stream aborted due to app shutdown.");
                        }
                    };
                });
            },
            _ = shutdown_signal.notified() => {
                tracing::debug!("exiting http server loop due to receiving shutdown signal.");
                break;
            }
        }
    }

}

async fn accept_tcp_stream_via_tls_terminating_proxy_service(
    tcp_stream: TcpStream,
    source_addr: SocketAddr,
    tls_acceptor: TlsAcceptor,
    service_template: ReverseProxyService
) {
    let mut service: ReverseProxyService = service_template.clone();
    service.remote_addr = Some(source_addr);
    service.is_https_only = true;
    let tls_acceptor = tls_acceptor.clone();
    match tls_acceptor.accept(tcp_stream).await {
        Ok(tcp_stream) => {
            let io = hyper_util::rt::TokioIo::new(tcp_stream);       
            http_proxy::serve(service, SomeIo::Https(io)).await;
        },
        Err(e) => {
            tracing::warn!("accept_tcp_stream_via_tls_terminating_proxy_service failed with error: {e:?}")
        },
    }
}

async fn listen_https(
    bind_addr: SocketAddr,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: GlobalState,
    terminating_service_template: ReverseProxyService,
    targets: Arc<ReverseTcpProxyTargets>,
    shutdown_signal: Arc<Notify>
) {

    use socket2::{Domain,Type};

    let socket = Socket::new(Domain::IPV4, Type::STREAM, None).expect("should always be possible to create a tcp socket for tls");
    match socket.set_only_v6(false) {
        Ok(_) => {},
        Err(e) => tracing::warn!("Failed to set_only_vs: {e:?}")
    };
    match socket.set_reuse_address(true) { // annoying as hell otherwise for quick resets
        Ok(_) => {},
        Err(e) => tracing::warn!("Failed to set_reuse_address: {e:?}")
    }
    socket.bind(&bind_addr.into()).expect("we must be able to bind to https addr socket..");
    socket.listen(128).expect("we must be able to listen to https addr socket..");
    let listener: std::net::TcpListener = socket.into();
    listener.set_nonblocking(true).expect("must be able to set_nonblocking on https listener");
    let tcp_listener = tokio::net::TcpListener::from_std(listener).expect("we must be able to listen to https port..");
    

    let mut rustls_config = 
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(crate::DynamicCertResolver {
                cache: std::sync::Mutex::new(HashMap::new())
            }));
        
    if let Some(true) = state.1.read().await.alpn {
        rustls_config.alpn_protocols.push("h2".into());
        rustls_config.alpn_protocols.push("http/1.1".into());
    }

    let arced_tls_config = std::sync::Arc::new(rustls_config);

    loop {
        //tracing::trace!("waiting for new https connection..");
        let targets = targets.clone();   

        tokio::select!{ 
            Ok((tcp_stream, source_addr)) = tcp_listener.accept() => {
                tracing::trace!("tcp listener accepted a new https connection");
                let mut service: ReverseProxyService = terminating_service_template.clone();
                service.remote_addr = Some(source_addr);
                let shutdown_signal = shutdown_signal.clone();
                let targets = targets.clone().reverse_tcp_proxy_targets().await;         
                let tx = tx.clone();
                let arced_tls_config = arced_tls_config.clone();
                let state = state.clone();
                tokio::task::spawn(async move {
                    tokio::select!{ 
                        _ = handle_new_tcp_stream(Some(arced_tls_config),service, tcp_stream, source_addr, targets, true,tx.clone(),state.clone()) => {
                            tracing::trace!("https tcp stream handled");

                        }
                        _ = shutdown_signal.notified() => {
                            eprintln!("https tcp stream aborted due to app shutdown.");
                        }
                    };
                });
               
            },
            _ = shutdown_signal.notified() => {
                tracing::debug!("exiting https server loop due to receiving shutdown signal.");
                break;
            }
        }
    }
}

// this will peek in to the incoming tcp stream and either create a direct tcp tunnel (passthru mode)
// or hand it off to the terminating http/https hyper services
async fn handle_new_tcp_stream(
    rustls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    service:ReverseProxyService,
    tcp_stream:TcpStream,
    source_addr:SocketAddr,
    targets: Vec<ReverseTcpProxyTarget>,
    expect_tls: bool,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: GlobalState,
) {

    //tracing::warn!("handle_new_tcp_stream!");

    {
        let s = state.0.read().await;
        let mut guard = s.statistics.write().expect("must always be able to write stats");
        guard.received_tcp_connections += 1;
    }

    //tracing::info!("handle_new_tcp_stream called with expect tls: {expect_tls}");

    let targets = targets.clone();
    
    
    match tcp_proxy::ReverseTcpProxy::peek_tcp_stream(&tcp_stream, source_addr).await {
        
        // we see that this is cleartext data, and we expect clear text data, and we also extracted a hostname by peeking.
        // at this point, we should check if the target is NOT configured for https (tls) before forwarding.
        Ok(PeekResult {
            typ: DataType::ClearText,
            http_version:_,
            target_host: Some(target)
        }) if !expect_tls => {
            if let Some(target) = tcp_proxy::ReverseTcpProxy::try_get_target_from_vec(targets, &target) {
                if target.target_http_port.is_some() {
                    
                    
                    if target.is_hosted {
                        
                        let proc_state = {
                            let guard = state.0.read().await;
                            match guard.site_states_map.get(&target.host_name) {
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
                                for _ in 0..2 {
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                    tracing::debug!("handling an incoming request to a stopped target, waiting for up to 10 seconds for {thn} to spin up - after this we will release the request to the terminating proxy and show a 'please wait' page instaead.");
                                    {
                                        let guard = state.0.read().await;
                                        match guard.site_states_map.get(&target.host_name) {
                                            Some(&ProcState::Running) => {
                                                has_started = true;
                                                break
                                            },
                                            _ => { }
                                        }
                                    }
                                }
                                if has_started {
                                    tracing::trace!("Using unencrypted tcp tunnel for remote target: {target:?}");
                                    tcp_proxy::ReverseTcpProxy::tunnel(tcp_stream, target, false,state.clone(),source_addr).await;
                                    return;
                                } else {
                                    tracing::trace!("{thn} is still not running... handing this request over to the terminating proxy.")
                                }
                            }
                            ,_=> {
                                tracing::trace!("Using unencrypted tcp tunnel for remote target: {target:?}");
                                tcp_proxy::ReverseTcpProxy::tunnel(tcp_stream, target, false,state.clone(),source_addr).await;
                                return;
                             }
                        }

                    } else {
                        tracing::trace!("Using unencrypted tcp tunnel for remote target: {target:?}");
                        tcp_proxy::ReverseTcpProxy::tunnel(tcp_stream, target, false,state.clone(),source_addr).await;
                        return;
                    }
                } else {
                    tracing::debug!("peeked some clear text tcp data and found that the target exists but is not configured for clear text. we will use terminating mode for this..")
                }
            }
        },
        
        // we see that this is tls data, and we expect tls data, and we also extracted a hostname by peeking.
        // at this point, we should check if the target is configured for https (tls) before forwarding.
        Ok(PeekResult {
            typ: DataType::TLS,
            http_version:_,
            target_host: Some(target)
        }) if expect_tls => {
            if let Some(target) = tcp_proxy::ReverseTcpProxy::try_get_target_from_vec(targets, &target) {
                
                if target.target_tls_port.is_some() {
                    _ = tx.send(ProcMessage::Start(target.host_name.clone()));
                    tracing::info!("USING TCP PROXY FOR TLS TUNNEL TO TARGET {target:?}");
                    tcp_proxy::ReverseTcpProxy::tunnel(tcp_stream, target, true,state.clone(),source_addr).await;
                    return;
                } else {
                    tracing::debug!("peeked some tls tcp data and found that the target exists but is not configured for https/tls. we will use terminating mode for this..")
                }
            }
        },
        e => {
            tracing::trace!("tcp peek invalid result: {e:?}. this could be because of incoming and outgoing protocol mismatch or configuration - will use terminating proxy mode instead") 

        },
    }

    // If we are unable to peek the tcp packet or the target is not configured in a way which allows for tcp tunnelling
    // we will hand the stream off to the terminating proxy service instead.
    if let Some(tls_cfg) = rustls_config {
        let tls_acceptor = TlsAcceptor::from(tls_cfg.clone());
        tracing::trace!("handing off tls-tcp stream to terminating proxy!");
        accept_tcp_stream_via_tls_terminating_proxy_service(tcp_stream, source_addr, tls_acceptor, service).await
    } else {
        tracing::trace!("handing off clear text tcp stream to terminating proxy!");
        let io = hyper_util::rt::TokioIo::new(tcp_stream);    
        http_proxy::serve(service, SomeIo::Http(io)).await;
    }
    



}

