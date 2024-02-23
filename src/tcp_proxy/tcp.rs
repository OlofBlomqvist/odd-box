use chrono::Local;
use hyper::Version;
use tokio::sync::Notify;
use std::net::IpAddr;
use std::{
    net::SocketAddr,
    sync::Arc,
};
use crate::tcp_proxy::tls::client_hello::TlsClientHello;
use crate::tcp_proxy::tls::client_hello::TlsClientHelloError;
use crate::types::proxy_state::{ProxyActiveConnection, ProxyActiveConnectionType};
use tokio::net::{TcpListener, TcpStream};
use tracing::*;


/// Non-terminating reverse proxy service for HTTP and HTTPS.
/// Achieves TLS passthru by peeking at the ClientHello SNI ext data.
#[derive(Debug,Eq,PartialEq,Hash,Clone)]
pub struct ReverseTcpProxyTarget {
    pub target_hostname: String,
    pub target_http_port: Option<u16>,
    pub target_tls_port: Option<u16>,
    pub host_name: String,
    pub is_hosted : bool // we have this here to not need to check actual state for non hosted sites (for the "please wait" page feature)
}


impl ReverseTcpProxyTarget {
    pub fn from_target(target:crate::http_proxy::Target) -> Self {
        match &target {
            crate::http_proxy::Target::Remote(x) => ReverseTcpProxyTarget {
                target_hostname: x.target_hostname.clone(),
                target_http_port: if x.https.unwrap_or_default() {None} else { x.port },
                target_tls_port:if x.https.unwrap_or_default() {x.port} else { None },
                host_name: x.host_name.clone(),
                is_hosted: false
            },
            crate::http_proxy::Target::Proc(x) => ReverseTcpProxyTarget {
                target_hostname: x.host_name.clone(),
                target_http_port: if x.https.unwrap_or_default() {None} else { x.port },
                target_tls_port:if x.https.unwrap_or_default() {x.port} else { None },
                host_name: x.host_name.clone(),
                is_hosted: true
            },
        }
    }
}

#[derive(Debug)]
pub enum DataType {
    TLS,
    ClearText
}

#[derive(Debug)]
pub struct PeekResult {
    pub (crate) typ : DataType,
    #[allow(dead_code)]pub (crate) http_version : Option<Version>,
    pub (crate) target_host : Option<String>
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
    pub targets: Arc<Vec<ReverseTcpProxyTarget>>,
    pub socket_addr: SocketAddr,
}

impl ReverseTcpProxy {

    pub fn try_get_target_from_vec(
        targets: Arc<Vec<ReverseTcpProxyTarget>>,
        name: &str,
    ) -> Option<ReverseTcpProxyTarget> {
        let parsed_name = if name.contains(":") {
            name.split(":").next().expect("if something contains a colon and we split the thing there must be at least one part")
        } else {
            name
        };
        targets.iter().find_map(|x| {
            if x.host_name.to_lowercase().trim() == parsed_name.to_lowercase().trim() {
                // we dont want to impl clone on this so we just create it manually for now
                // altough we could return refs but I don't have time for lifetimes atm
                Some(ReverseTcpProxyTarget { 
                    target_hostname: x.target_hostname.clone(), 
                    target_http_port: x.target_http_port, 
                    target_tls_port: x.target_tls_port,
                    host_name: x.host_name.clone(),
                    is_hosted: x.is_hosted
                })
            } else {
                None
            }
        })
    }

    #[instrument(skip_all)]
    pub (crate) async fn peek_tcp_stream(
        tcp_stream: &TcpStream,
        client_address: SocketAddr,
    ) -> Result<PeekResult,PeekError> {
        
        trace!("Peeking at tcp stream from {:?}", client_address);

        let mut we_know_this_is_not_tls_handshake = false;

        let mut we_know_its_not_h1 = false;
        let mut we_know_its_not_h2 = false;

        let mut count = 0;
        let mut last_peeked = 0;


        let duration = std::time::Duration::from_secs(1); 
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
                                Err(_e) => {
                                    //trace!("{e:?}")
                                }
                            }
                        }
                    }

                    // if we dont already know this traffic is NOT http1:
                    if we_know_its_not_h1 == false && super::http1::is_valid_http_request(&buf) {
                        
                        we_know_its_not_h2 = true;
                        we_know_this_is_not_tls_handshake = true;
                        
                        if let Ok(str_data) = std::str::from_utf8(&buf) {

                            if let Some(valid_host_name) = super::http1::try_decode_http_host(str_data) {
                                trace!("Found valid http1 host header while peeking in to tcp stream: {valid_host_name}");
                                return Ok(PeekResult { 
                                    typ: DataType::ClearText, 
                                    // todo : use version from the peeked tcp bytes
                                    http_version: Some(Version::HTTP_11), 
                                    target_host: Some(valid_host_name)
                                })
                            } else {
                                trace!("well, its not a valid http request (yet)..");
                            }
                        } else {
                            trace!("seems to be a valid http request, yet not valid utf8... strange!")
                        }
                    }
                    // if we dont already know the traffic is NOT http1: 
                    else if we_know_its_not_h2 == false && super::http2::is_valid_http2_request(&buf) {
                        we_know_its_not_h1 = true;
                        if let Some(valid_host_name) = super::http2::find_http2_authority(&buf) {
                            trace!("Found valid http2 authority while peeking in to tcp stream: {valid_host_name}");
                            return Ok(PeekResult { 
                                typ: DataType::ClearText, 
                                // todo : use version from the peeked tcp bytes
                                http_version: Some(Version::HTTP_2), 
                                target_host: Some(valid_host_name)
                            });
                        } else {
                            trace!("it is a valid http2 request but no authority is yet to be found");
                            we_know_this_is_not_tls_handshake = true;
                        }
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
        state: std::sync::Arc<tokio::sync::RwLock<crate::AppState>>,
        client_address: SocketAddr
    ) {

        let target_addr = {
            if incoming_traffic_is_tls && let Some(tls_port) = target.target_tls_port {
                format!("{}:{}",target.target_hostname, tls_port)
            } else if let Some(http_port) = target.target_http_port {
                format!("{}:{}",target.target_hostname, http_port)
            } else {
                unreachable!()
            }
        };        
        match TcpStream::connect(target_addr.clone()).await {
            Ok(mut rem_stream) => {

                
                if let Ok(target_addr_socket) = rem_stream.peer_addr() {
                    let source_addr = client_address.clone();    

                    let item = ProxyActiveConnection {
                        target,
                        target_addr: format!("{target_addr} ({})",target_addr_socket.ip()),
                        source_addr: source_addr.clone(),
                        creation_time: Local::now(),
                        description: None,
                        connection_type: if incoming_traffic_is_tls {
                            ProxyActiveConnectionType::TcpTunnelTls
                        } else {
                            ProxyActiveConnectionType::TcpTunnelUnencryptedHttp
                        },
                    };
                    
                    let item_key = (source_addr,uuid::Uuid::new_v4());

                    {   // ADD THIS CONNECTION TO STATE
                        let s = state.read().await;
                        let mut guard = s.statistics.write().expect("should always be able to add connections to state");
                        _=guard.active_connections.insert(item_key, item);
                    }

                    match tokio::io::copy_bidirectional(&mut client_tcp_stream, &mut rem_stream).await
                            {
                                Ok(_a) => {
                                    // could add this to target stats at some point
                                    //debug!("stream completed ok! -- {} <--> {}", a.0, a.1)
                                }
                                Err(e) => {
                                    trace!("Stream failed with err: {e:?}")
                                }
                            }
                            
                    {   // DROP THIS CONNECTION FROM STATE
                        let s = state.read().await;
                        let mut guard = s.statistics.write().expect("should always be able to drop connections from state");
                        _ = guard.active_connections.remove(&item_key);
                    }

                   
                } else {
                   tracing::warn!("failed to read socket peer address..");
                }
            },
            Err(e) => warn!("failed to connect to target {target:?} (using addr: {target_addr}) --> {e:?}"),
        }
    }

    #[instrument(skip_all)]
    pub async fn listen(&self,shutdown_signal:std::sync::Arc<Notify>,state: std::sync::Arc<tokio::sync::RwLock<crate::AppState>>,) -> Result<(), std::io::Error> {

        tracing::info!("Starting TCP proxy on {:?}",self.socket_addr);
        let listener = TcpListener::bind(self.socket_addr).await?;

        loop {
            let local_state_clone = state.clone();
            tokio::select! {
                Ok((tcp_stream, client_address)) = listener.accept() => {
                    
                    let peek_result = Self::peek_tcp_stream(&tcp_stream, client_address).await;
                    
                    let cloned_list = self.targets.clone();
                    tokio::spawn(async move {
                        match peek_result {
                            Ok(PeekResult {
                                typ,
                                http_version : _,
                                target_host : Some(target_host)
                            }) => {
                                let is_tls = match typ {
                                    DataType::TLS => true,
                                    _ => false,
                                };
                                if let Some(t) =  Self::try_get_target_from_vec(cloned_list, &target_host) {
                                    _ = Self::tunnel(
                                        tcp_stream,
                                        t,
                                        is_tls,
                                        local_state_clone,
                                        client_address
                                    ).await;
                                } else {
                                    tracing::warn!("no such target is configured: {target_host:?}")
                                }
                            },
                            Ok(_) => {
                                tracing::info!("could not find a host name so we dont know where to proxy this traffic. giving up on this stream!")
                            }
                            Err(e) => {
                                tracing::info!("giving up on this stream due to error: {e:?}")
                            },
                        }
                    });

                },
                _ = shutdown_signal.notified() => {
                    break;
                },
            }
        }

        Ok(())
    }
}


