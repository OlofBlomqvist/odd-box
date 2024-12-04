use hyper::Version;
use hyper_rustls::ConfigBuilderExt;
use serde::Serialize;
use std::fmt::Debug;
use std::net::IpAddr;
use std::{
    net::SocketAddr,
    sync::Arc,
};
use crate::configuration::BackendFilter;
use crate::global_state::GlobalState;
use crate::tcp_proxy::Peekable;
use crate::types::proc_info::ProcId;
use tokio::net::TcpStream;
use tracing::*;

use super::{ManagedStream, GenericManagedStream};


/// Non-terminating reverse proxy service for HTTP and HTTPS.
/// Achieves TLS passthru by peeking at the ClientHello SNI ext data.
#[derive(Debug,Eq,PartialEq,Hash,Clone,Serialize)]
pub struct ReverseTcpProxyTarget {
    pub remote_target_config: Option<crate::configuration::RemoteSiteConfig>,
    pub hosted_target_config: Option<crate::configuration::InProcessSiteConfig>,
    pub backends: Vec<crate::configuration::Backend>,
    pub host_name: String,
    pub is_hosted : bool,
    pub capture_subdomains: bool,
    pub forward_wildcard: bool,
    // subdomain is filled in otf upon receiving a request for this target and there is a subdomain used in the req
    pub sub_domain: Option<String> ,
    pub disable_tcp_tunnel_mode : bool,
    pub proc_id : Option<ProcId>,
}



#[derive(Debug,Eq,PartialEq)]
pub enum DataType {
    TLS,
    ClearText
}

#[derive(Debug)]
pub struct PeekResult {
    pub typ : DataType,
    #[allow(dead_code)]pub http_version : Option<Version>,
    pub target_host : Option<String>,
    pub is_h2c_upgrade : bool
}
#[allow(dead_code)]
#[derive(Debug)]
pub enum PeekError {
    StreamIsClosed,
    Unknown(String),
    H2PriorKnowledgeNeedsToBeTerminated
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
#[allow(dead_code)]
pub struct ReverseTcpProxy {
    pub socket_addr: SocketAddr,
}
#[derive(Debug)]
pub enum TunnelError{
    /// No backend was found that matched the incoming traffic,
    /// we cannot tunnel the traffic to a backend directly but need to terminate it and 
    /// establish a new connection to the backend.
    NoUsableBackendFound(GenericManagedStream),
    Unknown(String)
}
impl std::error::Error for TunnelError {}
impl std::fmt::Display for TunnelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelError::NoUsableBackendFound(_) => write!(f, "No usable backend found for incoming traffic"),
            TunnelError::Unknown(e) => write!(f, "Unknown error: {}",e),
        }
    }
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


    pub async fn tunnel(
        client_tcp_stream: GenericManagedStream,
        target:Arc<ReverseTcpProxyTarget>,
        incoming_traffic_is_tls:bool,
        state: Arc<GlobalState>,
        client_address: SocketAddr,
        rustls_config : Option<Arc<rustls::ServerConfig>>,
        incoming_host_header_or_sni: String,
        http_version: Option<Version>,
        is_h2c_upgrade_request: bool
    ) -> anyhow::Result<(),TunnelError> {


        let terminate_incoming = if target.disable_tcp_tunnel_mode  {
            // this means we should always terminate incoming tls connections
            incoming_traffic_is_tls
        } else {

            if incoming_traffic_is_tls {

                let at_least_one_backend_is_tls= target.backends.iter().any(|x|x.https.unwrap_or_default());

                if at_least_one_backend_is_tls {
                    // - incoming is tls
                    // - we are allowed to tunnel
                    // - all backends are tls
                    // - we should be fine to tunnel the incoming connection without termination
                    false 
                } else {
                    // - incoming is tls
                    // - we are allowed to tunnel
                    // - there is no backend that speaks tls
                    // - we must terminate the incoming connection and forward the clear text data to the backend
                    true
                }

            } else {
                false // incoming is clear text... so nothing to terminate here...
               
            }
           
        };

        let (client_tls_is_terminated,possibly_terminated_stream,backend_filter) = if terminate_incoming {

            let tls_cfg = if let Some(cfg) = rustls_config {
                cfg
            } else {
                return Err(TunnelError::Unknown("TLS termination is required but no rustls config provided. Stream cannot be processed further.".into()))
            };

            match client_tcp_stream {
                GenericManagedStream::TCP(peekable_tcp_stream) => {
                            
                    let tls_acceptor = TlsAcceptor::from(tls_cfg.clone());
                    match tls_acceptor.accept(peekable_tcp_stream).await {
                        Ok(mut tls_stream) => {
                            tracing::trace!("Terminated TLS connection established!");
                            tls_stream.get_mut().0.is_tls_terminated = true;
                            tls_stream.get_mut().0.events.push("Terminated TLS prior to running bidirectional TCP tunnel".into());
                            let mut gen_stream = GenericManagedStream::from_terminated_tls_stream(ManagedStream::from_tls_stream(tls_stream));

                            let peek_result = gen_stream.peek_managed_stream(client_address).await;
                            gen_stream.seal();
                            match peek_result {
                                Ok(r) => {
                                    let backend_filter = peekresult_to_backend_filter(r,true,is_h2c_upgrade_request);
                                    (true,gen_stream,backend_filter)
                                },
                                Err(e) => {
                                    return Err(TunnelError::Unknown(format!("error peeking stream {e:?}")));
                                },
                            }



                        },
                        Err(e) => {
                            tracing::warn!("Accept_tcp_stream_via_tls_terminating_proxy_service failed with error: {e:?}");
                            // since the incoming traffic is tls and we failed to terminate it, we cannot proceed with the connection
                            // so we just drop it here by returning OK.
                            return Ok(())
                        }
                    }
                },
                GenericManagedStream::TerminatedTLS(_managed_stream) => {
                    tracing::warn!("Wormhole was already spawned.. this is a bug.");
                    return Ok(())
                },
            }

        } else {
            (false,client_tcp_stream,peekresult_to_backend_filter(
                PeekResult {
                    typ: if incoming_traffic_is_tls { DataType::TLS } else { DataType::ClearText },
                    http_version,
                    target_host: Some(incoming_host_header_or_sni.clone()),
                    is_h2c_upgrade: is_h2c_upgrade_request
                },false,is_h2c_upgrade_request
            ))
        };

        let backend_filter = if let Some(f) = backend_filter {
            f
        } else {
            tracing::warn!("failed to generate a backend filter.. falling back to http termination");
            return Err(TunnelError::NoUsableBackendFound(possibly_terminated_stream))
        };

        let backend = 
            match (&target.remote_target_config,&target.hosted_target_config) {
                (Some(rem_conf),None) => {
                    rem_conf.next_backend(&state, backend_filter).await
                },
                (None,Some(proc_conf)) => {
                    proc_conf.next_backend(&state, backend_filter).await
                },
                _ => None
            };
        
        let backend = if backend == None {
            tracing::warn!("No backend found for target {}.. falling back to http termination",target.host_name);
            return Err(TunnelError::NoUsableBackendFound(possibly_terminated_stream))
        } else {
            backend.unwrap()
        };

        // this is just for the tcp tunnel... im assuming it should be as simple as this
        let resolved_address = format!("{}:{}",backend.address,backend.port);
        
        let server_name_for_tls = Some(backend.address.clone()); // "todo";

        // this is the backend tls setting.. should be easy enough to do this
        let backend_is_tls = backend.https.unwrap_or_default();

        let erect_tls_tunnel_to_backend = {
            match (incoming_traffic_is_tls,backend_is_tls,client_tls_is_terminated) {
                (_,false,_) => false, // backend is not tls, clearly we dont need to erect a tls tunnel
                (true,true,true) => true, // incoming is tls, backend is tls, and we have already terminated the incoming tls stream.. so we must erect a new tls tunnel
                (true,true,false) => false, // incoming is tls, backend is tls, but we have not terminated the incoming tls stream.. so we can tunnel the incoming tls stream to the backend
                (false,true,_) => true, // incoming is clear text, backend is tls, so we must erect a tls tunnel
            }
        };

        match TcpStream::connect(resolved_address.clone()).await {
            Ok(rem_stream) => {

                // THIS SHOULD BE THE ONLY PLACE WE INCREMENT THE TUNNEL COUNTER
                match state.app_state.statistics.connections_per_hostname.get_mut(&target.host_name) {
                    Some(mut guard) => {
                        let (_k,v) = guard.pair_mut();
                        v.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    },
                    None => {
                        state.app_state.statistics.connections_per_hostname
                        .insert(target.host_name.clone(), std::sync::atomic::AtomicUsize::new(1));
                    }
                };

                if let Ok(_target_addr_socket) = rem_stream.peer_addr() {
                    
                    
                    possibly_terminated_stream.update_tracked_info(|x|{
                        x.backend = Some(backend);
                        x.target = Some(target.as_ref().to_owned());
                        x.incoming_connection_uses_tls = incoming_traffic_is_tls;
                        x.outgoing_connection_is_tls = backend_is_tls;              
                    });
                    
                    match run_managed_bidirectional_tunnel(
                        possibly_terminated_stream, 
                        rem_stream, 
                        backend_is_tls,
                        server_name_for_tls,
                        erect_tls_tunnel_to_backend,
                        incoming_traffic_is_tls
                    ).await {
                        Ok(_) => {
                            
                        },
                        Err(e) => {
                            tracing::warn!("Tunnel failed with error: {:?}",e);
                        }
                    }
                   

                   
                } else {
                   tracing::warn!("failed to read socket peer address..");
                }
            },
            Err(e) => warn!("failed to connect to target {host} (using addr: {resolved_address}) --> {e:?}",host=target.host_name),
        }

        Ok(())
    }


}



// proxy between original client and remote backend
use tokio_rustls::{rustls, TlsAcceptor, TlsConnector};
use rustls::pki_types::ServerName;
async fn run_managed_bidirectional_tunnel(
    ref mut original_client_stream: GenericManagedStream,
    mut stream_connected_to_some_backend: TcpStream,
    backend_is_tls: bool,
    server_name: Option<String>,
    erect_tls_tunnel: bool,
    incoming_traffic_is_tls: bool
) -> Result<(), Box<dyn std::error::Error>> {
    

    if backend_is_tls && erect_tls_tunnel {

        let server_name = if let Some(s) = server_name {
            s
        } else {
            return Err("no server name provided for tls connection".into());
        };

        let config = tokio_rustls::rustls::ClientConfig::builder_with_protocol_versions(tokio_rustls::rustls::ALL_VERSIONS)
            .with_native_roots()
            .expect("must be able to create tls configuration")
            .with_no_client_auth();

        let arc_config = Arc::new(config);
        let connector = TlsConnector::from(arc_config);

        let server_name = if let Ok(n) = ServerName::try_from(server_name.clone()) {
            n
        } else {
            return Err(format!("failed to create server name from {}",server_name).into());
        };

        // Establish a TLS connection to the backend
        let mut backend_tls_stream = connector
            .connect(server_name, stream_connected_to_some_backend)
            .await?;

        tracing::warn!("New TLS connection established towards the backend");
        
        match original_client_stream {
            GenericManagedStream::TerminatedTLS(peekable_tls_stream) => {     
                match tokio::io::copy_bidirectional( peekable_tls_stream, &mut backend_tls_stream).await {
                    Ok((_bytes_from_client, _bytes_from_backend)) => {}
                    Err(e) => {
                        tracing::warn!("Stream failed with error: {:?}", e);
                    }
                }
                peekable_tls_stream.inspect().await;
            },
            GenericManagedStream::TCP(peekable_tcp_stream) => {
                tracing::trace!("Tunneling from cleartext to tls");
                match tokio::io::copy_bidirectional(peekable_tcp_stream, &mut backend_tls_stream).await {
                    Ok((_bytes_from_client, _bytes_from_backend)) => {}
                    Err(e) => {
                        tracing::warn!("Stream failed with error: {:?}", e);
                    }
                }
                peekable_tcp_stream.inspect().await;
            }
        }
       
    } else {
        match original_client_stream {
            GenericManagedStream::TerminatedTLS(peekable_tls_stream) => {

                if backend_is_tls {
                    tracing::trace!("Unwrapped TLS tunnel established, forwarding inner byte stream to tls backend");
                } else {
                    tracing::trace!("Unwrapped TLS tunnel established, forwarding inner byte stream to cleartext backend");
                }

                // Proxy data between the original client and the backend
                match tokio::io::copy_bidirectional(peekable_tls_stream, &mut stream_connected_to_some_backend).await {
                    Ok((_bytes_from_client, _bytes_from_backend)) => {
                        // Optionally handle the number of bytes transferred
                    }
                    Err(e) => {
                        tracing::warn!("Stream failed with error: {:?}", e);
                    }
                }
                peekable_tls_stream.inspect().await;
            }
            GenericManagedStream::TCP(peekable_tcp_stream) => {

                if incoming_traffic_is_tls {
                    tracing::trace!("Raw TCP tunnel established: tls");
                } else {
                    tracing::trace!("Raw TCP tunnel established: cleartext");
                }
                
                // Proxy data between the original client and the backend
                match tokio::io::copy_bidirectional(peekable_tcp_stream, &mut stream_connected_to_some_backend).await {
                    Ok((_bytes_from_client, _bytes_from_backend)) => {
                        // Optionally handle the number of bytes transferred
                    }
                    Err(e) => {
                        tracing::warn!("Stream failed with error: {:?}", e);
                    }
                }
                peekable_tcp_stream.inspect().await;
            }
        }
    }
    
    Ok(())
}


fn peekresult_to_backend_filter(
    info_about_incoming_data: PeekResult,
    incoming_is_tls_terminated: bool,
    is_h2c_upgrade_request: bool,
) -> Option<BackendFilter> {
    use DataType::*;

    match (
        info_about_incoming_data.http_version,
        info_about_incoming_data.typ,
        incoming_is_tls_terminated,
        is_h2c_upgrade_request
    ) {

        // HTTP/2 over ClearText without TLS termination (HTTP/2 Prior Knowledge)
        (Some(Version::HTTP_2), ClearText, false, false) => Some(BackendFilter::H2CPriorKnowledge), 

        // HTTP 1.1 request with H2C upgrade header
        (Some(Version::HTTP_11), ClearText, false, true) => Some(BackendFilter::H2C),

        // An incoming http2 request over tls that we have not terminated.. we can tunnel this directly to a backend that is
        // known to speak http2
        (Some(Version::HTTP_2), TLS, false, false) => Some(BackendFilter::Http2),

        // HTTP/1.0, HTTP/1.1 - Clear text.. simply tunnel to any backend that speaks http1
        (Some(Version::HTTP_10) | Some(Version::HTTP_11), _, _,false) => Some(BackendFilter::Http1),

        (None,scheme,terminated,_) => {

            let incoming_byte_stream_is_tls = scheme == DataType::TLS && !terminated;

            if incoming_byte_stream_is_tls {
                // we have no insight in to the incoming data, but the site allows tcp tunnelling
                // so lets just tunnel the connection to any tls capable backend..
                return Some(BackendFilter::AnyTLS)
            }
            tracing::warn!("Incoming data has no version info, but byte stream is not tls... something is fishy here..");  
            None
            
        }

        (Some(Version::HTTP_2), ClearText, true, false) => {
            // this means the incoming connection was made over tls, but we terminated it.
            // we should be able to connect to any backend that speaks http2 (creating a new tls connection if needed)
            Some(BackendFilter::Http2)
        },

        // If we cannot determine the HTTP version, we cannot make a decision, meaning we will terminate both tls and http
        // and establish a new connection to the backend.
        (a,b,c,d) => {
            tracing::warn!("Cannot determine backend filter for incoming data: HTTP Version: {:?}, Data Type: {:?}, TLS Terminated: {:?}, H2C Upgrade: {:?}",a,b,c,d);
            None
        },
    
    }
}