use chrono::Local;
use hyper::Version;
use hyper_rustls::ConfigBuilderExt;
use rustls::ClientConnection;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::client::TlsStream;
use tokio_stream::StreamExt;
use std::net::IpAddr;
use std::time::Duration;
use std::{
    net::SocketAddr,
    sync::Arc,
};
use crate::configuration::v2::BackendFilter;
use crate::global_state::GlobalState;
use crate::proxy::SomeSortOfManagedStream;
use crate::tcp_proxy::tls::client_hello::TlsClientHello;
use crate::types::proxy_state::{ProxyActiveConnection, ProxyActiveConnectionType};
use tokio::net::TcpStream;
use tracing::*;

use super::h2_parser;
use super::managed_stream::{self, ManagedStream, Peekable};


/// Non-terminating reverse proxy service for HTTP and HTTPS.
/// Achieves TLS passthru by peeking at the ClientHello SNI ext data.
#[derive(Debug,Eq,PartialEq,Hash,Clone)]
pub struct ReverseTcpProxyTarget {
    pub remote_target_config: Option<crate::configuration::v2::RemoteSiteConfig>,
    pub hosted_target_config: Option<crate::configuration::v2::InProcessSiteConfig>,
    pub backends: Vec<crate::configuration::v2::Backend>,
    pub host_name: String,
    pub is_hosted : bool,
    pub capture_subdomains: bool,
    pub forward_wildcard: bool,
    // subdomain is filled in otf upon receiving a request for this target and there is a subdomain used in the req
    pub sub_domain: Option<String> ,
    pub disable_tcp_tunnel_mode : bool
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
    pub socket_addr: SocketAddr,
}

impl ReverseTcpProxy {
    pub fn get_subdomain(requested_hostname: &str, backend_hostname: &str) -> Option<String> {
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

    #[instrument(skip_all)]
    pub async fn eat_tcp_stream(
         managed_stream: &mut SomeSortOfManagedStream,
        _client_address: SocketAddr,
    ) -> Result<PeekResult, PeekError> 
    where
    {
        
        let mut attempts = 0;
        
        loop {

            if attempts > 2 {
                break;
            }

            let (tcp_stream_closed,buf) = if let Ok(b) = managed_stream.peek_async().await {
                b
            } else {
                return Err(PeekError::Unknown("Failed to read from TCP stream".into()));
            };

            if tcp_stream_closed {
                return Err(PeekError::Unknown("TCP stream has already closed".into()));
            }
            
            if buf.len() == 0 {
                _ = tokio::time::sleep(Duration::from_millis(20)).await;
                attempts+=1;
                continue;

            }

            // Check for TLS handshake (0x16 is the ContentType for Handshake in TLS)
            if buf[0] == 0x16 {

                // note that we do not expect this to happen when processing a SomeSortOfManagedStream::ClearText stream
                // but it will still technically work so... lets just allow clients to connect to the wrong port if they want..?
               
                match TlsClientHello::try_from(&buf[..]) {
                    Ok(v) => {
                        if let Ok(v) = v.read_sni_hostname() {
                            trace!("Got TLS client hello with SNI: {v}");
                            return Ok(PeekResult { 
                                typ: DataType::TLS, 
                                http_version: None, 
                                target_host: Some(v),
                            });
                        }
                    }
                    Err(crate::tcp_proxy::tls::client_hello::TlsClientHelloError::MessageIncomplete(_)) => {
                        tracing::trace!("Incomplete TLS client handshake detected; waiting for more data... (we have got {} bytes)",buf.len());
                        continue;
                    }
                    _ => {
                        return Err(PeekError::Unknown("Invalid TLS client handshake".to_string()));
                    }
                }
                // Continue loop to check for more data if TLS isn't fully validated
                tokio::time::sleep(Duration::from_millis(10)).await;
                attempts += 1;
                continue;
            }

            // Check for valid HTTP/1.x request
            if let Ok(http_version) = super::http1::is_valid_http_request(&buf) {
                if let Ok(str_data) = std::str::from_utf8(&buf) {
                    if let Some(valid_host_name) = super::http1::try_decode_http_host(str_data) {
                        trace!("Found valid HTTP/1 host header while peeking into TCP stream: {valid_host_name}");
                        return Ok(PeekResult { 
                            typ: DataType::ClearText, 
                            http_version: Some(http_version), 
                            target_host: Some(valid_host_name),
                        });
                    } else {
                        tracing::trace!("HTTP/1.x request detected but missing host header; waiting for more data...");
                    }
                } else {
                    tracing::trace!("Incomplete UTF-8 data detected; waiting for more data...");
                }
            } else if super::http2::is_valid_http2_request(&buf) {
                tracing::info!("is valid h2... creating new h2o for buf with len: {}",buf.len());
                let mut hmm = h2_parser::H2Observer::new();
                hmm.write_incoming(&buf);
                tracing::info!("created peek observer");
                let items = hmm.get_all_events() ;
                if items.len() < 2 {
                    tracing::info!("not enough http2 frames found (yet)...");
                } else {

                    for frame in items {
                        match frame {
                            h2_parser::H2Event::IncomingRequest(rq) => {
                                if let Some(host) = rq.headers.get(":authority") {
                                    return Ok(PeekResult { 
                                        typ: DataType::ClearText, 
                                        http_version: Some(Version::HTTP_2), 
                                        target_host: Some(host.to_string()),
                                    });
                                }
                            },
                            _ => {}
                        }
                    }
                    

                    
                }
                // HTTP/2 (h2c) check only after HTTP/1.x check fails
                //return (managed_stream,Err(PeekError::Unknown("odd-box does not currently support h2c for TCP tunnel mode".into())));
            } else {
                tracing::warn!("NOT VALID H1 OR H2");
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
            attempts += 1;
        }
    
        Err(PeekError::Unknown("TCP peek failed to find any useful information".into()))
    }

    pub async fn tunnel(
        client_tcp_stream: crate::proxy::SomeSortOfManagedStream,
        target:Arc<ReverseTcpProxyTarget>,
        incoming_traffic_is_tls:bool,
        state: Arc<GlobalState>,
        client_address: SocketAddr
    ) {

        // THIS SHOULD BE THE ONLY PLACE WE INCREMENT THE TUNNEL COUNTER
        match state.app_state.statistics.tunnelled_tcp_connections_per_hostname.get_mut(&target.host_name) {
            Some(mut guard) => {
                let (_k,v) = guard.pair_mut();
                v.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            },
            None => {
                state.app_state.statistics.tunnelled_tcp_connections_per_hostname
                .insert(target.host_name.clone(), std::sync::atomic::AtomicUsize::new(1));
            }
        };

        // only remotes have more than one backend. hosted processes always have a single backend.
        let primary_backend =  {

            let b = if let Some(remconf) = &target.remote_target_config {
                remconf.next_backend(&state, BackendFilter::Any,true).await
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
                format!("{}.{}:{}", subdomain.expect("we just validated subdomain so it must exist"), primary_backend.address, primary_backend.port )
            } else {
                format!("{}:{}", primary_backend.address, primary_backend.port )
            }
        };
            

        tracing::trace!("tcp tunneling to target: {resolved_target_address} (tls: {incoming_traffic_is_tls})");
        
        match TcpStream::connect(resolved_target_address.clone()).await {
            Ok(mut rem_stream) => {

                if let Ok(target_addr_socket) = rem_stream.peer_addr() {
                    
                    let item = ProxyActiveConnection {
                        target_name: target.host_name.clone(),
                        target_addr: format!("{resolved_target_address} ({})",target_addr_socket.ip()),
                        source_addr: client_address,
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

                    _ = run_managed_bidirectional_tunnel(client_tcp_stream, rem_stream, primary_backend.https.unwrap_or_default()).await; 
                   
                    // DROP FROM ACTIVE STATE ONCE DONE
                    state.app_state.statistics.active_connections.remove(&item_key);

                   
                } else {
                   tracing::warn!("failed to read socket peer address..");
                }
            },
            Err(e) => warn!("failed to connect to target {target:?} (using addr: {resolved_target_address}) --> {e:?}"),
        }
    }

}



// proxy between original client and remote backend
use tokio_rustls::{rustls, TlsConnector};
use rustls::pki_types::ServerName;
async fn run_managed_bidirectional_tunnel(
    mut original_client_stream: SomeSortOfManagedStream,
    mut stream_connected_to_some_backend: TcpStream,
    use_tls: bool
) -> Result<(), Box<dyn std::error::Error>> {
    
    if use_tls {

        let config = tokio_rustls::rustls::ClientConfig::builder_with_protocol_versions(tokio_rustls::rustls::ALL_VERSIONS)
            .with_native_roots()
            .expect("must be able to create tls configuration")
            .with_no_client_auth();

        let arc_config = Arc::new(config);
        let connector = TlsConnector::from(arc_config);

        // Specify the server name for SNI // TODO!!!!! must pass the target info to here
        let server_name = ServerName::try_from("echo.websocket.org").unwrap();

        // Establish a TLS connection to the backend
        let mut backend_tls_stream = connector
            .connect(server_name, stream_connected_to_some_backend)
            .await?;

        tracing::warn!("tls connection established towards the backend");
        
        // Proxy data between the original client and the backend
        match tokio::io::copy_bidirectional(&mut original_client_stream, &mut backend_tls_stream).await {
            Ok((_bytes_from_client, _bytes_from_backend)) => {
                // Optionally handle the number of bytes transferred
            }
            Err(e) => {
                tracing::warn!("Stream failed with error: {:?}", e);
            }
        }
    } else {
     
        // Proxy data between the original client and the backend
        match tokio::io::copy_bidirectional(&mut original_client_stream, &mut stream_connected_to_some_backend).await {
            Ok((_bytes_from_client, _bytes_from_backend)) => {
                // Optionally handle the number of bytes transferred
            }
            Err(e) => {
                tracing::warn!("Stream failed with error: {:?}", e);
            }
        }
    }
    
    original_client_stream.do_inspection_stuff().await;
    Ok(())
}
