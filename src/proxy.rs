use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use hyper_rustls::ConfigBuilderExt;
use lazy_static::lazy_static;
use socket2::Socket;
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
use crate::tcp_proxy::ManagedStream;
use crate::tcp_proxy::PeekResult;
use crate::types::app_state;


pub async fn listen(
    _cfg: std::sync::Arc<tokio::sync::RwLock<ConfigWrapper>>, 
    bind_addr: SocketAddr,
    bind_addr_tls: SocketAddr, 
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    shutdown_signal: Arc<Notify>
)  {

    
    let client_tls_config = tokio_rustls::rustls::ClientConfig::builder_with_protocol_versions(tokio_rustls::rustls::ALL_VERSIONS)
        // todo - add support for accepting self-signed certificates etc
        // .dangerous()
        // .with_custom_certificate_verifier(verifier)
        
        .with_native_roots()
        .expect("must be able to create tls configuration")
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


    let terminating_proxy_service = ReverseProxyService { 
        resolved_target: None,
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
            shutdown_signal.clone()
        ),

        listen_https(
            bind_addr_tls.clone(),
            tx.clone(),
            state.clone(),
            terminating_proxy_service.clone(),  
            shutdown_signal.clone()
        ),
        
    );


    
} 

lazy_static! {
    static ref ACTIVE_TCP_CONNECTIONS_SEMAPHORE : tokio::sync::Semaphore = tokio::sync::Semaphore::new(555);
} 

async fn listen_http(
    bind_addr: SocketAddr,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    terminating_service_template: ReverseProxyService,
    _shutdown_signal: Arc<Notify> 
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

        // should use semaphore here to limit the number of active connections

        if state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) {
            tracing::debug!("exiting http server loop due to receiving shutdown signal.");
            break;
        }

        let permit = if let Ok(p) = ACTIVE_TCP_CONNECTIONS_SEMAPHORE.acquire().await {
            p
        } else {
            tracing::warn!("Error acquiring semaphore permit.. This is a bug in odd-box :<");
            break
        };



        match tokio_listener.accept().await {
            Ok((tcp_stream,source_addr)) => {
               
                tracing::trace!("accepted connection! current active: {}", 555-ACTIVE_TCP_CONNECTIONS_SEMAPHORE.available_permits() );
                let mut service: ReverseProxyService = terminating_service_template.clone();
                service.remote_addr = Some(source_addr);   
                let tx = tx.clone();
                let state = state.clone();
                tokio::spawn(async move {                   
                    let _moved_permit = permit;          
                    handle_new_tcp_stream(None,service, tcp_stream, source_addr, false,tx.clone(),state.clone())
                        .await;
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
    managed_stream: ManagedStream,
    source_addr: SocketAddr,
    tls_acceptor: TlsAcceptor,
    mut service: ReverseProxyService
) {
    service.remote_addr = Some(source_addr);
    service.is_https_only = true;
    let tls_acceptor = tls_acceptor.clone();
    match tls_acceptor.accept(managed_stream).await {
        Ok(tcp_stream) => 
            http_proxy::serve(service, SomeIo::Https(tcp_stream)).await,
        Err(e) => 
            tracing::warn!("accept_tcp_stream_via_tls_terminating_proxy_service failed with error: {e:?}")
    }
}

async fn listen_https(
    bind_addr: SocketAddr,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    terminating_service_template: ReverseProxyService,
    _shutdown_signal: Arc<Notify>
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

        if state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) {
            tracing::debug!("exiting http server loop due to receiving shutdown signal.");
            break;
        }

        let permit = if let Ok(p) = ACTIVE_TCP_CONNECTIONS_SEMAPHORE.acquire().await {
            p
        } else {
            tracing::warn!("Error acquiring semaphore permit.. This is a bug in odd-box :<");
            break
        };


        match tokio_listener.accept().await {
            Ok((tcp_stream,source_addr)) => {
               
                tracing::trace!("accepted connection! current active: {}", 555-ACTIVE_TCP_CONNECTIONS_SEMAPHORE.available_permits() );
                let mut service: ReverseProxyService = terminating_service_template.clone();
                service.remote_addr = Some(source_addr);  
                let tx = tx.clone();
                let arced_tls_config = Some(arced_tls_config.clone());
                let state = state.clone();
                tokio::spawn(async move {      
                    let _moved_permit = permit;             
                    handle_new_tcp_stream(arced_tls_config,service, tcp_stream, source_addr, true,tx.clone(),state.clone())
                        .await;
                });
                

            }
            Err(e) => {
                tracing::warn!("error accepting tcp connection: {:?}", e);
                //break;
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
    incoming_connection_is_on_tls_port: bool,
    tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>
) {

    
    let (mut managed_stream,peek_result) = 
        tcp_proxy::ReverseTcpProxy::eat_tcp_stream(tcp_stream, source_addr).await;

    managed_stream.seal();

    match peek_result {
        
        // we see that this is cleartext data, and we expect clear text data, and we also extracted a hostname by peeking.
        // at this point, we should check if the target is NOT configured for https (tls) before forwarding.
        Ok(PeekResult {
            typ: DataType::ClearText,
            http_version: _http_version,
            target_host: Some(target)
        }) if incoming_connection_is_on_tls_port == false => {
            
            if let Some(target) = state.try_find_site(&target).await {


                let cloned_target = target.clone();

                fresh_service_template_with_source_info.resolved_target = Some(cloned_target.clone());
                
                if target.disable_tcp_tunnel_mode == false && target.backends.iter().any(|x|{
                    // todo : support checking for h2 hint so that we dont try to connect to a NOH2 backend
                    // if the incoming connections http_version is h2
                    x.https.unwrap_or_default()==false
                }) {

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
                                        tracing::trace!("Using unencrypted tcp tunnel for remote target: {:?}",target.host_name);
                                        tcp_proxy::ReverseTcpProxy::tunnel(managed_stream, cloned_target, false,state.clone(),source_addr).await;
                                        return;
                                    } else {
                                        tracing::trace!("{thn} is still not running...giving up.");
                                        return;
                                    }
                                }
                                , _  => {
                                    tracing::trace!("Using unencrypted tcp tunnel for remote target: {:?}",target.host_name);
                                    tcp_proxy::ReverseTcpProxy::tunnel(managed_stream, cloned_target, false,state.clone(),source_addr).await;
                                    return;
                                }
                            }

                        } else {
                            tracing::trace!("Using unencrypted tcp tunnel for remote target: {:?}",target.host_name);
                            tcp_proxy::ReverseTcpProxy::tunnel(managed_stream, cloned_target, false,state.clone(),source_addr).await;
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


            let host_name = target_host_name.to_lowercase();
            
            if let Some(target) = state.try_find_site(&target_host_name).await {
               
                if target.disable_tcp_tunnel_mode == false && target.backends.iter().any(|x|x.https.unwrap_or_default()) {
                    // at least one backend has https enabled so we will use the tls tunnel mode to there
                    tracing::trace!("USING TCP PROXY FOR TLS TUNNEL TO TARGET {:?}",target.host_name);
                    tcp_proxy::ReverseTcpProxy::tunnel(managed_stream, target, true,state.clone(),source_addr).await;
                    return;
                } else {
                    tracing::trace!("peeked some tls tcp data and found that the target exists but is not configured for https/tls. we will use terminating mode for this..");
                    fresh_service_template_with_source_info.resolved_target = Some(target);
                }


            } else {
                tracing::warn!("We do not have any site configured for '{host_name}' that allows tcp tunnelling.. will use terminating proxy instead.");
            }
        },
        e => {
            tracing::warn!("tcp peek invalid result: {e:?}. this could be because of incoming and outgoing protocol mismatch or configuration - will use terminating proxy mode instead") 
        }
    }



    // If we are unable to peek the tcp packet or the target is not configured in a way which allows for tcp tunnelling
    // we will hand the stream off to the terminating proxy service instead.
    if let Some(tls_cfg) = rustls_config {
        
        let tls_acceptor = TlsAcceptor::from(tls_cfg.clone());
        
        tracing::trace!("handing off tls-tcp stream to terminating proxy for target!");
        accept_tcp_stream_via_tls_terminating_proxy_service(managed_stream, source_addr, tls_acceptor, fresh_service_template_with_source_info).await
    } else {
        tracing::trace!("handing off clear text tcp stream to terminating proxy for target!");     
        http_proxy::serve(fresh_service_template_with_source_info, SomeIo::Http(managed_stream)).await
    };
}

