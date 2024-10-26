use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use hyper_rustls::ConfigBuilderExt;
use lazy_static::lazy_static;
use socket2::Socket;
use tokio::io;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::sync::RwLock;
use tokio_rustls::TlsAcceptor;
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
use crate::types::app_state;


use tokio_util::sync::CancellationToken;
use tokio::task::JoinHandle;

pub async fn listen(
    cfg: Arc<RwLock<ConfigWrapper>>, 
    initial_bind_addr: SocketAddr,
    initial_bind_addr_tls: SocketAddr, 
    tx: Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    shutdown_signal: Arc<Notify>,
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
                new_http_cancel_token.clone(),
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

        // let permit = if let Ok(p) = ACTIVE_TCP_CONNECTIONS_SEMAPHORE.acquire().await {
        //     p
        // } else {
        //     tracing::warn!("Error acquiring semaphore permit.. This is a bug in odd-box :<");
        //     break
        // };

        //tracing::info!("accepting http connection..");
        tokio::select! {
            _ = cancel_token.cancelled() => {
                tracing::info!("Cancellation token triggered, shutting down HTTP listener.");
                break;
            }
            x = tokio_listener.accept() => {
                match x {
                    Ok((tcp_stream,source_addr)) => {
                    
                        //tracing::trace!("Accepted connection! current active: {}", 200-ACTIVE_TCP_CONNECTIONS_SEMAPHORE.available_permits() );
                        let mut service: ReverseProxyService = terminating_service_template.clone();
                        service.configuration = Arc::new(state.config.read().await.clone());
                        service.remote_addr = Some(source_addr);   
                        let tx = tx.clone();
                        let state = state.clone();
                        tokio::spawn(async move {                   
                            //let _moved_permit = permit;          
                            handle_new_tcp_stream(None, service,tcp_stream, source_addr, false,tx.clone(),state.clone())
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
            tracing::warn!("Error acquiring semaphore permit.. This is a bug in odd-box :<");
            break
        };

        //tracing::info!("accepting https connection..");

        tokio::select! {
            _ = cancel_token.cancelled() => {
                tracing::info!("Cancellation token triggered, shutting down HTTPS listener.");
                break;
            }
            x = tokio_listener.accept() => {
                match x {
                    Ok((tcp_stream,source_addr)) => {
                    
                        tracing::trace!("accepted connection! current active: {}", 200-ACTIVE_TCP_CONNECTIONS_SEMAPHORE.available_permits() );
                        let mut service: ReverseProxyService = terminating_service_template.clone();
                        service.configuration = Arc::new(state.config.read().await.clone());

                        service.remote_addr = Some(source_addr);  
                        let tx = tx.clone();
                        let arced_tls_config = Some(arced_tls_config.clone());
                        let state = state.clone();
                        tokio::spawn(async move {      
                            let _moved_permit = permit;             
                            handle_new_tcp_stream(arced_tls_config,service,tcp_stream, source_addr, true,tx.clone(),state.clone())
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
    }
    
    tracing::warn!("listen_https went bye bye.")
}

pub enum SomeSortOfManagedStream{
    ClearText(ManagedStream<TcpStream>),
    TLS(ManagedStream<tokio_rustls::server::TlsStream<TcpStream>>)
}

impl crate::tcp_proxy::Peekable for SomeSortOfManagedStream {
    fn seal(&mut self) {
        match self {
            SomeSortOfManagedStream::ClearText(managed_stream) => managed_stream.seal(),
            SomeSortOfManagedStream::TLS(managed_stream) => managed_stream.seal(),
        }
    }
    async fn peek_async(&mut self) -> Result<(bool, Vec<u8>), io::Error> {
        match self {
            SomeSortOfManagedStream::ClearText(managed_stream) => managed_stream.peek_async().await,
            SomeSortOfManagedStream::TLS(managed_stream) => managed_stream.peek_async().await,
        }
    }
}
impl SomeSortOfManagedStream {
    pub async fn do_inspection_stuff(&mut self) {
        match self {
            SomeSortOfManagedStream::ClearText(managed_stream) => {
                managed_stream.inspect().await;
            },
            SomeSortOfManagedStream::TLS(managed_stream) => {
                managed_stream.inspect().await;
            }
        }
    }
}

impl AsyncRead for SomeSortOfManagedStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            SomeSortOfManagedStream::ClearText(managed_stream) => 
                Pin::new(managed_stream).poll_read(cx,buf),
            SomeSortOfManagedStream::TLS(managed_stream) => 
                Pin::new(managed_stream).poll_read(cx,buf),
        }
    }
}

impl AsyncWrite for SomeSortOfManagedStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {

        match self.get_mut() {
            SomeSortOfManagedStream::ClearText(managed_stream) => 
                Pin::new(managed_stream).poll_write(cx,buf),
            SomeSortOfManagedStream::TLS(managed_stream) => 
                Pin::new(managed_stream).poll_write(cx,buf),
        }
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            SomeSortOfManagedStream::ClearText(managed_stream) => 
                Pin::new(managed_stream).poll_flush(cx),
            SomeSortOfManagedStream::TLS(managed_stream) => 
                Pin::new(managed_stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            SomeSortOfManagedStream::ClearText(managed_stream) => 
                Pin::new(managed_stream).poll_shutdown(cx),
            SomeSortOfManagedStream::TLS(managed_stream) => 
                Pin::new(managed_stream).poll_shutdown(cx),
        }
    }
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

   // fresh_service_template_with_source_info.remote_addr = Some(source_addr);
    
    let mut managed_stream : SomeSortOfManagedStream;
    let mut sni_server_name = None;

    let peek_result = if let Some(tls_cfg) = rustls_config {
        //fresh_service_template_with_source_info.is_https_only = true;
        let tls_acceptor = TlsAcceptor::from(tls_cfg.clone());
        match tls_acceptor.accept(tcp_stream).await {
            Ok(tls_stream) => {
                
                if let Some(sni) = tls_stream.get_ref().1.server_name() {
                    sni_server_name = Some(sni.to_string());
                }

                managed_stream = crate::proxy::SomeSortOfManagedStream::TLS(ManagedStream::from_tls_stream(tls_stream));
                tcp_proxy::ReverseTcpProxy::eat_tcp_stream(&mut managed_stream, source_addr).await
            },
            Err(e) => {
                tracing::warn!("accept_tcp_stream_via_tls_terminating_proxy_service failed with error: {e:?}");
                return 
            }
        }
    } else {
        managed_stream = crate::proxy::SomeSortOfManagedStream::ClearText(ManagedStream::from_tcp_stream(tcp_stream));
        tcp_proxy::ReverseTcpProxy::eat_tcp_stream(&mut managed_stream, source_addr).await
    };
    
    managed_stream.seal();

    match peek_result {
        
        // we see that this is cleartext data, and we expect clear text data, and we also extracted a hostname by peeking.
        // at this point, we should check if the target is NOT configured for https (tls) before forwarding.
        Ok(PeekResult {
            typ: _, // <-- will always be cleartext as we have already wrapped any outer tls stream in a managed stream
            http_version: _http_version,
            target_host: h2_authority_or_h1_host_header
        }) => {

            let target_host_name = if let Some(n) = sni_server_name.or(h2_authority_or_h1_host_header) {
                n
            } else {
                tracing::warn!("No target host found in peeked data.. will use terminating proxy mode instead.");
               // http_proxy::serve(fresh_service_template_with_source_info, managed_stream).await;
                return;
            };
            
            if let Some(target) = state.try_find_site(&target_host_name).await {


                let cloned_target = target.clone();

               // fresh_service_template_with_source_info.resolved_target = Some(cloned_target.clone());
                
                if target.disable_tcp_tunnel_mode == false {

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
                                        tcp_proxy::ReverseTcpProxy::tunnel(managed_stream, cloned_target, false,state.clone(),source_addr).await;
                                        return;
                                    } else {
                                        tracing::trace!("{thn} is still not running...giving up.");
                                        return;
                                    }
                                }
                                , _  => {
                                    tcp_proxy::ReverseTcpProxy::tunnel(managed_stream, cloned_target, false,state.clone(),source_addr).await;
                                    return;
                                }
                            }

                        } else {
                            tcp_proxy::ReverseTcpProxy::tunnel(managed_stream, cloned_target, false,state.clone(),source_addr).await;
                            return;
                        }
                } else {
                    tracing::trace!("got incoming clear text connection for {} but the target is configured for tls tunneling.. will use terminating proxy instead.",cloned_target.host_name);

                }
            } else {
                tracing::trace!("We do not have any site configured for '{target_host_name}' that allows tcp tunnelling.. will use terminating proxy instead.");
            }
        },
        
        // // we see that this is tls data, and we expect tls data, and we also extracted a hostname by peeking.
        // // at this point, we should check if the target is configured for https (tls) before forwarding.
        // Ok(PeekResult {
        //     typ: DataType::TLS,
        //     http_version:_,
        //     target_host: Some(target_host_name)
        // }) if incoming_connection_is_on_tls_port => {


        //     let host_name = target_host_name.to_lowercase();
            
        //     if let Some(target) = state.try_find_site(&target_host_name).await {
               
        //         if target.disable_tcp_tunnel_mode == false && target.backends.iter().any(|x|x.https.unwrap_or_default()) {
        //             // at least one backend has https enabled so we will use the tls tunnel mode to there
        //             tracing::trace!("USING TCP PROXY FOR TLS TUNNEL TO TARGET {:?}",target.host_name);
        //             tcp_proxy::ReverseTcpProxy::tunnel(managed_stream, target, true,state.clone(),source_addr).await;
        //             return;
        //         } else {
        //             tracing::trace!("peeked some tls tcp data and found that the target exists but is not configured for https/tls. we will use terminating mode for this..");
        //             fresh_service_template_with_source_info.resolved_target = Some(target);
        //         }


        //     } else {
        //         tracing::trace!("We do not have any site configured for '{host_name}' that allows tcp tunnelling.. will use terminating proxy instead.");
        //     }
        // },
        e => {
            tracing::warn!("tcp peek invalid result: {e:?}. this could be because of incoming and outgoing protocol mismatch or configuration - will use terminating proxy mode instead") 
        }
    }



    // // at this point we have failed to use direct tunnel mode (or the target was not configured for it)
    tracing::trace!("handing off clear text tcp stream to terminating proxy for target!");     
    http_proxy::serve(fresh_service_template_with_source_info, managed_stream).await;
    

    
}
