use chrono::Local;
use hyper::Version;
use std::net::IpAddr;
use std::{
    net::SocketAddr,
    sync::Arc,
};
use crate::configuration::v2::BackendFilter;
use crate::global_state::GlobalState;
use crate::tcp_proxy::tls::client_hello::TlsClientHello;
use crate::tcp_proxy::tls::client_hello::TlsClientHelloError;
use crate::types::proxy_state::{ProxyActiveConnection, ProxyActiveConnectionType};
use tokio::net::TcpStream;
use tracing::*;


/// Non-terminating reverse proxy service for HTTP and HTTPS.
/// Achieves TLS passthru by peeking at the ClientHello SNI ext data.
#[derive(Debug,Eq,PartialEq,Hash,Clone)]
pub struct ReverseTcpProxyTarget {
    pub remote_target_config: Option<crate::configuration::v2::RemoteSiteConfig>,
    pub backends: Vec<crate::configuration::v2::Backend>,
    pub host_name: String,
    pub is_hosted : bool,
    pub capture_subdomains: bool,
    pub forward_wildcard: bool,
    // subdomain is filled in otf upon receiving a request for this target and there is a subdomain used in the req
    pub sub_domain: Option<String> 
}

pub struct ReverseTcpProxyTargets {
    pub global_state : Arc<GlobalState>
}

impl ReverseTcpProxyTargets {


    pub async fn try_find<F>(&self,filter_fun: F) -> Option<ReverseTcpProxyTarget>
        where F: Fn(&ReverseTcpProxyTarget) -> Option<ReverseTcpProxyTarget>,
    {
        
        let cfg = self.global_state.config.read().await;

        
        for y in cfg.hosted_process.iter().flatten().filter(|xx| 
            xx.disable_tcp_tunnel_mode.unwrap_or_default() == false
        ) {
            
            let port = y.active_port.unwrap_or_default();
            if port > 0 {
                let t = ReverseTcpProxyTarget {
                    remote_target_config: None, // we dont need this for hosted processes
                    capture_subdomains: y.capture_subdomains.unwrap_or_default(),
                    forward_wildcard: y.forward_subdomains.unwrap_or_default(),
                    backends: vec![crate::configuration::v2::Backend {
                        hints: y.hints.clone(),
                        address: y.host_name.to_owned(),
                        https: y.https,
                        port: y.active_port.unwrap_or_default()
                    }],
                    host_name: y.host_name.to_owned(),
                    is_hosted: true,
                    sub_domain: None
                };
                let filtered = filter_fun(&t);
                if filtered.is_some() {
                    return filtered
                }
            }

            
        }
    

        if let Some(x) = &cfg.remote_target {
            for y in x.iter().filter(|xx|
                xx.disable_tcp_tunnel_mode.unwrap_or_default() == false
            ) {

                // we support comma separated hostnames for the same target temporarily for remotes.
                // in this mode we require all backends to have the same scheme and port configuration..
                // this is temporary and will be removed once we have a v2 configuration format that 
                // supports multiple backend configurations for the same hostname.


                let t = ReverseTcpProxyTarget { 
                    remote_target_config: Some(y.clone()),
                    capture_subdomains: y.capture_subdomains.unwrap_or_default(),
                    forward_wildcard: y.forward_subdomains.unwrap_or_default(),
                    backends: y.backends.clone(),
                    host_name: y.host_name.to_owned(),
                    is_hosted: false,
                    sub_domain: None
                };
                let filtered = filter_fun(&t);
                if filtered.is_some() {
                    return filtered
                }
            }
        }

        None
        
    }
}

#[derive(Debug)]
pub enum DataType {
    TLS,
    ClearText
}

#[derive(Debug)]
pub struct PeekResult {
    pub typ : DataType,
    #[allow(dead_code)]pub http_version : Option<Version>,
    pub target_host : Option<String>
}

#[derive(Debug)]
pub enum PeekError {
    Unknown(String)
}

impl ReverseTcpProxyTarget {

    #[allow(dead_code)]
    fn is_valid_ip_or_dns(target: &str) -> bool {
        webpki::DnsNameRef::try_from_ascii_str(target)
            .map(|_| true)
            .or_else(|_| target.parse::<IpAddr>().map(|_| true))
            .unwrap_or(false)
    }

}

pub struct ReverseTcpProxy {
    pub targets: Arc<ReverseTcpProxyTargets>,
    pub socket_addr: SocketAddr,
}

impl ReverseTcpProxy {
    fn get_subdomain(requested_hostname: &str, backend_hostname: &str) -> Option<String> {
        if requested_hostname == backend_hostname { return None };
        if requested_hostname.to_uppercase().ends_with(&backend_hostname.to_uppercase()) {
            let part_to_remove_len = backend_hostname.len();
            let start_index = requested_hostname.len() - part_to_remove_len;
            if start_index == 0 || requested_hostname.as_bytes()[start_index - 1] == b'.' {
                return Some(requested_hostname[..start_index].trim_end_matches('.').to_string());
            }
        }
        None
    }
    pub fn req_target_filter_map(
        target: &ReverseTcpProxyTarget,
        req_host_name: &str,
    ) -> Option<ReverseTcpProxyTarget> {
        
        let parsed_name = if req_host_name.contains(":") {
            req_host_name.split(":").next().expect("if something contains a colon and we split the thing there must be at least one part")
        } else {
            req_host_name
        };


        if target.host_name.to_lowercase().trim() == parsed_name.to_lowercase().trim() {
           Some(ReverseTcpProxyTarget {
            capture_subdomains: target.capture_subdomains,
            forward_wildcard: target.forward_wildcard,
            backends: target.backends.clone(),
            host_name: target.host_name.clone(),
            is_hosted: target.is_hosted,
            sub_domain: None,
            remote_target_config: target.remote_target_config.clone()
        })
        } else {
            match Self::get_subdomain(parsed_name, &target.host_name) {
                Some(subdomain) => Some(ReverseTcpProxyTarget {
                    capture_subdomains: target.capture_subdomains,
                    forward_wildcard: target.forward_wildcard,
                    backends: target.backends.clone(),
                    host_name: target.host_name.clone(),
                    is_hosted: target.is_hosted,
                    sub_domain: Some(subdomain),
                    remote_target_config: target.remote_target_config.clone()
                }),
                None => None,
            }
        }
    }

    #[instrument(skip_all)]
    pub async fn peek_tcp_stream(
        tcp_stream: &TcpStream,
        client_address: SocketAddr,
    ) -> Result<PeekResult,PeekError> {
        
        trace!("Peeking at tcp stream from {:?}", client_address);

        let mut we_know_this_is_not_tls_handshake = false;

        let mut we_know_its_not_h1 = false;
        let mut we_know_its_not_h2 = false;

        let mut count = 0;
        let mut last_peeked = 0;


        let duration = std::time::Duration::from_millis(500); 
        let start_time = time::Instant::now();
        
        let result = tokio::time::timeout(duration, async {
            loop {

                if start_time.elapsed() > duration {
                    tracing::warn!("tcp peek abort after 1 second.");
                    break;
                }

                let mut buf = vec![0; 8000];

                if let Ok(peeked) = tcp_stream.peek(&mut buf).await {

                    if peeked > 0 {
                        if peeked == last_peeked {
                            count+=1;
                        } 
                            
                        if count > 100 {
                            warn!("giving up since we dont seem to make progress anymore.. buf: {buf:?}");
                            break;
                        }

                        if peeked == last_peeked {
                            tokio::task::yield_now().await;
                            continue
                        }
                        last_peeked = peeked;
                    }

                
                    if peeked < 32  {
                        
                        continue;
                    }

                    

                    if !we_know_this_is_not_tls_handshake {
                        if buf[0] != 0x16 {
                            tracing::trace!("detected non tls client handshake request..");
                            we_know_this_is_not_tls_handshake = true;
                        } else {

                            // 0x16 is not valid ascii which we would expect both for preface h2 or normal h1 method calls
                            we_know_its_not_h1 = true;
                            we_know_its_not_h2 = true;                            
                            
                            match TlsClientHello::try_from(&buf[..]) {
                                Ok(v) => {
                                    if let Ok(v) = v.read_sni_hostname() {
                                        trace!("ok got tls client hello with this sni: {v}",);
                                        return Ok(PeekResult { 
                                            typ: DataType::TLS, 
                                            http_version: None, 
                                            target_host: Some(v) 
                                        })
                                    }
                                }
                                Err(TlsClientHelloError::NotClientHello)
                                | Err(TlsClientHelloError::NotTLSHandshake) => {
                                    we_know_this_is_not_tls_handshake = true;
                                }
                                Err(e) => {
                                    trace!("{e:?}")
                                }
                            }
                        }
                    }

                    // if we dont already know this traffic is NOT http1:
                    if we_know_its_not_h1 == false  {
                        match super::http1::is_valid_http_request(&buf) {
                            Ok(http_version) => {
                                we_know_its_not_h2 = true;
                            we_know_this_is_not_tls_handshake = true;
                            if let Ok(str_data) = std::str::from_utf8(&buf) {
    
                                if let Some(valid_host_name) = super::http1::try_decode_http_host(str_data) {
                                    trace!("Found valid http1 host header while peeking in to tcp stream: {valid_host_name}");
                                    return Ok(PeekResult { 
                                        typ: DataType::ClearText, 
                                        http_version: Some(http_version), 
                                        target_host: Some(valid_host_name)
                                    })
                                } else {
                                    tracing::trace!("received an invalid http1 request. missing host header..");
                                    we_know_its_not_h1 = true; 
                                }
                            } else {
                                tracing::trace!("received an invalid http1 request. not valid utf8..");
                                we_know_its_not_h1 = true; 
                            }
                            },
                            Err(e) => {
                                tracing::trace!("received an invalid http1 request: {e:?}");
                                we_know_its_not_h1 = true;   
                            },
                        }
                    }
                    
                    // if we dont already know the traffic is NOT http2: 
                    else if we_know_its_not_h2 == false && super::http2::is_valid_http2_request(&buf) {
                        
                        return Err(PeekError::Unknown("oddbox does not currently support h2c for tcp tunnel mode".into()));
                        
                    }

                    if we_know_this_is_not_tls_handshake {
                        trace!("this is neither clear text nor tls... i give up... buf is {:?}",buf);
                        break;
                    }

                    if peeked > 6666 {
                        trace!(
                            "we have seen over 4000 bytes from this request but still know nothing, giving up at {buf:?}"
                        );
                        break;
                    }

                    //trace!("read {peeked} bytes: {buf:?}");

                } else {
                    break;
                }
            

            }
            Err(PeekError::Unknown("failed to find any useful info about the incoming stream".into()))
        }).await;

        match result {
            Ok(v) => v,
            Err(_e) => Err(PeekError::Unknown("timed out during peek stage".into())),
        }
        
    }

    pub async fn tunnel(
        mut client_tcp_stream:TcpStream,
        target:ReverseTcpProxyTarget,
        incoming_traffic_is_tls:bool,
        state: Arc<GlobalState>,
        client_address: SocketAddr
    ) {

        // only remotes have more than one backend. hosted processes always have a single backend.
        let primary_backend =  {
            let b = if let Some(remconf) = &target.remote_target_config {
                remconf.next_backend(&state, if incoming_traffic_is_tls { BackendFilter::Https } else { BackendFilter::Http }).await
            } else {
                target.backends.first().cloned()
            };
            if let Some(b) = b {
                b
            } else {
                tracing::warn!("no backend found for target {target:?}");
                return;
            }
        };

        if 0 == primary_backend.port {
            tracing::warn!("no active target port found for target {target:?}, wont be able to establish a tcp connection for site {}",target.host_name);
            return
        };


        let resolved_target_address = {
            let subdomain = target.sub_domain.as_ref();
            if target.forward_wildcard && subdomain.is_some() {
                tracing::debug!("tcp tunnel rewrote for subdomain: {:?}", subdomain);
                format!("{}.{}:{}", subdomain.unwrap(), primary_backend.address, primary_backend.port )
            } else {
                format!("{}:{}", primary_backend.address, primary_backend.port )
            }
        };

        tracing::trace!("tcp tunneling to target: {resolved_target_address} (tls: {incoming_traffic_is_tls})");

        match TcpStream::connect(resolved_target_address.clone()).await {
            Ok(mut rem_stream) => {

                
                if let Ok(target_addr_socket) = rem_stream.peer_addr() {
                    let source_addr = client_address.clone();    

                    let item = ProxyActiveConnection {
                        target_name: target.host_name.clone(),
                        target_addr: format!("{resolved_target_address} ({})",target_addr_socket.ip()),
                        source_addr: source_addr.clone(),
                        creation_time: Local::now(),
                        description: None,
                        connection_type: if incoming_traffic_is_tls {
                            ProxyActiveConnectionType::TcpTunnelTls
                        } else {
                            ProxyActiveConnectionType::TcpTunnelUnencryptedHttp
                        },
                    };

                    let item_key = crate::generate_unique_id();
                    
                    // ADD TO STATE BEFORE STARTING THE STREAM
                    state.app_state.statistics.active_connections.insert(item_key, item);

                    match tokio::io::copy_bidirectional(&mut client_tcp_stream, &mut rem_stream).await {
                        Ok(_a) => {
                            // could add this to target stats at some point
                            //debug!("stream completed ok! -- {} <--> {}", a.0, a.1)
                        }
                        Err(e) => {
                            trace!("Stream failed with err: {e:?}")
                        }
                    }
                   
                    // DROP FROM ACTIVE STATE ONCE DONE
                    state.app_state.statistics.active_connections.remove(&item_key);

                   
                } else {
                   tracing::warn!("failed to read socket peer address..");
                }
            },
            Err(e) => warn!("failed to connect to target {target:?} (using addr: {resolved_target_address}) --> {e:?}"),
        }
    }

    // #[instrument(skip_all)]
    // pub async fn listen_tcp_only(&self,shutdown_signal:std::sync::Arc<Notify>,state: Arc<GlobalState>,) -> Result<(), std::io::Error> {

    //     tracing::info!("Starting TCP proxy on {:?}",self.socket_addr);
    //     let listener = TcpListener::bind(self.socket_addr).await?;

    //     loop {
    //         let local_state_clone = state.clone();
    //         tokio::select! {
    //             Ok((tcp_stream, client_address)) = listener.accept() => {
                    
    //                 let peek_result = Self::peek_tcp_stream(&tcp_stream, client_address).await;
                    
    //                 let targets_arc = self.targets.clone();
                    
    //                 tokio::spawn(async move {
    //                     match peek_result {
    //                         Ok(PeekResult {
    //                             typ,
    //                             http_version : _,
    //                             target_host : Some(target_host)
    //                         }) => {
    //                             let is_tls = match typ {
    //                                 DataType::TLS => true,
    //                                 _ => false,
    //                             };
                                
    //                             fn filter_fun(p: &ReverseTcpProxyTarget, target_host: &str) -> Option<ReverseTcpProxyTarget> {
    //                                 ReverseTcpProxy::req_target_filter_map(p, target_host)
    //                             }
                                
    //                             let target_host_str = target_host.as_str();
    //                             if let Some(t) = targets_arc.try_find(|p| filter_fun(p, target_host_str)).await {
    //                                 _ = Self::tunnel(
    //                                     tcp_stream,
    //                                     t,
    //                                     is_tls,
    //                                     local_state_clone,
    //                                     client_address
    //                                 ).await;
    //                             } else {
    //                                 tracing::debug!("no such target is configured: {target_host:?}")
    //                             }
    //                         },
    //                         Ok(_) => {
    //                             tracing::debug!("could not find a host name so we dont know where to proxy this traffic. giving up on this stream!")
    //                         }
    //                         Err(e) => {
    //                             tracing::debug!("giving up on this stream due to error: {e:?}")
    //                         },
    //                     }
    //                 });

    //             },
    //             _ = shutdown_signal.notified() => {
    //                 break;
    //             },
    //         }
    //     }

    //     Ok(())
    // }
}


