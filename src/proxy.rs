use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use futures_util::task::UnsafeFutureObj;
use hyper::body::Incoming;
use hyper::Method;
use hyper::Uri;
use hyper_rustls::ConfigBuilderExt;
use hyper_util::client::legacy::connect::Connection;
use lazy_static::lazy_static;
use reqwest::Request;
use socket2::Socket;
use tokio::net::TcpSocket;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio_rustls::TlsAcceptor;
use tungstenite::util::NonBlockingResult;
use url::Url;

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
    state: Arc<GlobalState>,
    shutdown_signal: Arc<Notify>
)  {

    
    
    // create this from the state.
    let tcp_targets: Arc<ReverseTcpProxyTargets> = Arc::new(ReverseTcpProxyTargets {
        global_state: state.clone()
    });

    let client_tls_config = rustls::ClientConfig::builder_with_protocol_versions(rustls::ALL_VERSIONS)
        // todo - add support for accepting self-signed certificates etc
        // .dangerous()
        // .with_custom_certificate_verifier(verifier)
        
        .with_native_roots()
        .unwrap()
        .with_no_client_auth();

        
        

    let https_builder =
        hyper_rustls::HttpsConnectorBuilder::default().with_tls_config(client_tls_config);
    
    let connector: hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector> = 
        https_builder.https_or_http().enable_all_versions().build();
    
    let executor = hyper_util::rt::TokioExecutor::new();
    
    
    let client : hyper_util::client::legacy::Client<hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, hyper::body::Incoming>  = 
        hyper_util::client::legacy::Builder::new(executor.clone())
        .http2_only(false)
        .build(connector.clone());

    let h2_client : hyper_util::client::legacy::Client<hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, hyper::body::Incoming>  = 
        hyper_util::client::legacy::Builder::new(executor)
        .http2_only(true)
        .build(connector);

 
    // let c2 = reqwest::Client::builder().build().unwrap();
    // let what = c2.execute(Request::new(Method::DELETE, Url::parse("what").unwrap()));
    
  
    let terminating_proxy_service = ReverseProxyService { 
        state:state.clone(), 
        remote_addr: None, 
        tx:tx.clone(), 
        is_https_only:false,
        client,
        h2_client
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

// a lazy static atomic usize to keep count of active tcp connections:
lazy_static! {
    static ref ACTIVE_TCP_CONNECTIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
} 

async fn listen_http(
    bind_addr: SocketAddr,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    terminating_service_template: ReverseProxyService,
    targets: Arc<ReverseTcpProxyTargets>,
    shutdown_signal: Arc<Notify> 
) {
    
    let socket = TcpSocket::new_v4().expect("new v4 socket should always work");
    socket.set_reuseaddr(true).expect("set reuseaddr fail?");
    socket.bind(bind_addr).expect(&format!("must be able to bind http serveraddr {bind_addr:?}"));
   
    let listener = socket.listen(3000).expect("must be able to bind http listener.");

    // // TODO - let shutdown_signal = shutdown_signal.clone();
    // _ = shutdown_signal.notified() => {
    //     //                     eprintln!("stream aborted due to app shutdown."); break
    //     //                 }

    loop {

        if state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) {
            tracing::debug!("exiting http server loop due to receiving shutdown signal.");
            break;
        }

        if ACTIVE_TCP_CONNECTIONS.load(std::sync::atomic::Ordering::SeqCst) >= 666 {
               tokio::time::sleep(Duration::from_millis(10)).await;
               continue;  
        }

        match listener.accept().await {
            Ok((tcp_stream,source_addr)) => {
               
                let c = ACTIVE_TCP_CONNECTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tracing::trace!("accepted connection! current active: {}", 1+c);
                let mut service: ReverseProxyService = terminating_service_template.clone();
                service.remote_addr = Some(source_addr);
                let arc_clone_targets = targets.clone();     
                let tx = tx.clone();
                let state = state.clone();
                tokio::spawn(async move {                   
                    handle_new_tcp_stream(None,service, tcp_stream, source_addr, arc_clone_targets, false,tx.clone(),state.clone())
                        .await;
                    ACTIVE_TCP_CONNECTIONS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                });
                

            }
            Err(e) => {
                tracing::warn!("error accepting tcp connection: {:?}", e);
                //break;
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
    state: Arc<GlobalState>,
    terminating_service_template: ReverseProxyService,
    targets: Arc<ReverseTcpProxyTargets>,
    shutdown_signal: Arc<Notify>
) {

    use socket2::{Domain,Type};

    let socket = Socket::new(Domain::IPV4, Type::STREAM, None).expect("should always be possible to create a tcp socket for tls");
    match socket.set_only_v6(false) {
        Ok(_) => {},
        Err(e) => tracing::trace!("Failed to set_only_vs: {e:?}")
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
        
    if let Some(true) = state.config.read().await.alpn {
        rustls_config.alpn_protocols.push("h2".into());
        rustls_config.alpn_protocols.push("http/1.1".into());
    }

    let arced_tls_config = std::sync::Arc::new(rustls_config);

    loop {
        //tracing::trace!("waiting for new https connection..");
        let targets_arc = targets.clone();   

        tokio::select!{ 
            Ok((tcp_stream, source_addr)) = tcp_listener.accept() => {
                tracing::trace!("tcp listener accepted a new https connection");
                let mut service: ReverseProxyService = terminating_service_template.clone();
                service.remote_addr = Some(source_addr);
                let shutdown_signal = shutdown_signal.clone();
                let arc_targets_clone = targets_arc.clone();       
                let tx = tx.clone();
                let arced_tls_config = arced_tls_config.clone();
                let state = state.clone();
                tokio::task::spawn(async move {
                    tokio::select!{ 
                        _ = handle_new_tcp_stream(Some(arced_tls_config),service, tcp_stream, source_addr, arc_targets_clone, true,tx.clone(),state.clone()) => {
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
    tracing::warn!("listen_https went bye bye.")
}

// this will peek in to the incoming tcp stream and either create a direct tcp tunnel (passthru mode)
// or hand it off to the terminating http/https hyper services
async fn handle_new_tcp_stream(
    rustls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    service:ReverseProxyService,
    tcp_stream:TcpStream,
    source_addr:SocketAddr,
    targets: Arc<ReverseTcpProxyTargets>,
    incoming_connection_is_on_tls_port: bool,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
) {


    let n = state.request_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    //tracing::warn!("handle_new_tcp_stream ({})!",n+1);
    //tracing::info!("handle_new_tcp_stream called with expect tls: {expect_tls}");

    let targets = targets.clone();
    _ = tcp_stream.set_linger(None);
    
    match tcp_proxy::ReverseTcpProxy::peek_tcp_stream(&tcp_stream, source_addr).await {
        
        // we see that this is cleartext data, and we expect clear text data, and we also extracted a hostname by peeking.
        // at this point, we should check if the target is NOT configured for https (tls) before forwarding.
        Ok(PeekResult {
            typ: DataType::ClearText,
            http_version:_,
            target_host: Some(target)
        }) if incoming_connection_is_on_tls_port == false => {

            if let Some(target) = targets.try_find(move |p|tcp_proxy::ReverseTcpProxy::req_target_filter_map(p,&target )).await {
                
                if target.backends.iter().any(|x|x.https.unwrap_or_default()==false) {

                        if target.is_hosted {
                            
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
                                    for _ in 0..2 {
                                        tokio::time::sleep(Duration::from_secs(5)).await;
                                        tracing::debug!("handling an incoming request to a stopped target, waiting for up to 10 seconds for {thn} to spin up - after this we will release the request to the terminating proxy and show a 'please wait' page instaead.");
                                        {
                                            match state.app_state.site_status_map.get(&target.host_name) {
                                                Some(my_ref) => {
                                                    match my_ref.value() {
                                                        app_state::ProcState::Running => {
                                                            has_started = true;
                                                            break
                                                        },
                                                        _ => { }
                                                    }                                               },
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
                                , _  => {
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
                }
            }
        },
        
        // we see that this is tls data, and we expect tls data, and we also extracted a hostname by peeking.
        // at this point, we should check if the target is configured for https (tls) before forwarding.
        Ok(PeekResult {
            typ: DataType::TLS,
            http_version:_,
            target_host: Some(target_host_name)
        }) if incoming_connection_is_on_tls_port => {
            if let Some(target) = targets.try_find(move |p|tcp_proxy::ReverseTcpProxy::req_target_filter_map(&p, &target_host_name)).await {
                 
                if target.backends.iter().any(|x|x.https.unwrap_or_default()) {
                    // at least one backend has https enabled so we will use the tls tunnel mode to there
                    tracing::trace!("USING TCP PROXY FOR TLS TUNNEL TO TARGET {target:?}");
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
        
        http_proxy::serve(service, SomeIo::Http(io)).await
    };
}

