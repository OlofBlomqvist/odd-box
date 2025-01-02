use std::sync::Weak;
use std::sync::atomic::AtomicUsize;

use crate::{configuration::Backend, tcp_proxy::ReverseTcpProxyTarget};


/// Unlike the monitoring station, this will clear out old information when a connection goes away.
/// For any sort of tracing or historic data, use the state.monitoring_station.
#[derive(Debug)]
pub struct ProxyLiveStats {
    
    pub total_accepted_tcp_connections : AtomicUsize,

    pub active_connections : dashmap::DashMap<ConnectionKey,ProxyActiveTCPConnection>,

    /// This is NOT meant for statistical use but internally to do basic loadbalancing, 
    /// it just happens to live here for convenience
    pub lb_access_count_per_hostname : dashmap::DashMap<String,AtomicUsize>,
    
}


pub type ConnectionKey = u64;

use serde::Serialize;

use super::connection_type::ConnectionType;

#[derive(Debug,Clone,Serialize)]
pub enum OutgoingTunnelType {
    TLS(ReverseTcpProxyTarget,Backend),
    Raw(ReverseTcpProxyTarget,Backend)
}

#[derive(Debug,Clone,Serialize)]
#[allow(dead_code)]
pub struct ProxyActiveTCPConnection {
    pub incoming_sni : Option<String>,
    pub client_socket_address : Option<std::net::SocketAddr>,
    pub odd_box_socket : Option<std::net::SocketAddr>,
    #[serde(skip)]
    pub connection_key_pointer : Weak<ConnectionKey>,
    pub connection_key: ConnectionKey,
    pub client_addr_string : String,
    /// This means that the data inside of this TCP connection is encrypted using tls
    pub incoming_connection_uses_tls: bool,
    /// This means we have terminated the incoming TLS session, meaning we can see the data between the client and the proxy.
    pub tls_terminated: bool,
    /// This means we have terminated the http (and tls if used) connection and have established a new http(s) connection to the target
    pub http_terminated: bool,

    pub version : u64,
    pub resolved_connection_type: Option<ConnectionType>,
    pub resolved_connection_type_description: Option<String>,
    pub is_grpc : Option<bool>,
    pub http_version : Option<bool>,
    pub is_websocket : Option<bool>,

    /// If we are not actually terminating http, we will normally establish a single
    /// long-lived tcp connection towards a specific target. In that case this will be populated
    /// with either cleartext or tls depending on what was used when establishing that tunnel.
    pub outgoing_tunnel_type: Option<OutgoingTunnelType>
    
}

impl ProxyActiveTCPConnection {
    pub fn get_connection_type(&self) -> ConnectionType {
        if self.version == 1 {
            return ConnectionType::PendingInit
        }
        
        match (
            self.incoming_connection_uses_tls,
            self.tls_terminated,
            self.http_terminated,
            &self.outgoing_tunnel_type,
        ) {
            // === HTTP =====================================
            
            // Clear incoming with a raw tunnel established
            (false,_,_,Some(OutgoingTunnelType::Raw(target,backend))) => ConnectionType::HttpPassthru(target.clone(),backend.clone()),

            // Clear incoming with outgoing tls tunnel established
            (false,_,_,Some(OutgoingTunnelType::TLS(target,backend))) => ConnectionType::HttpWithOutgoingTLS(target.clone(),backend.clone()),

            // Clear incoming with no outgoing tunnel.  Each http request will be handled separately.
            (false,false,true,None) => ConnectionType::HttpTermination,


            // === TLS =====================================

            // Encrypted incoming with a new tls connection created to a specific backend
            (true, true, false, Some(OutgoingTunnelType::TLS(target,backend))) => ConnectionType::TlsTerminatedWithOutgoingTLS(target.clone(),backend.clone()),
            
            // Encrypted incoming and terminated with a raw rcp tunnel created to a specific backend
            (true, true, false, Some(OutgoingTunnelType::Raw(target,backend))) => ConnectionType::TlsTerminatedWithOutgoingClearText(target.clone(),backend.clone()),

            // Encrypted incoming with no termination and a raw tcp connection to a specific backend
            (true,false,false,Some(OutgoingTunnelType::Raw(target,backend))) => ConnectionType::TlsPassthru(target.clone(),backend.clone()),

            // Encrypted incoming and fully terminated. Each http request will be handled separately.
            (true,true,true,None) => ConnectionType::TlsTermination,
            
            // === IMPOSSIBLE =====================================
            
            _ => ConnectionType::Invalid(format!("odd-box bug - invalid contype: {incoming_connection_uses_tls} + {tls_terminated} + {http_terminated} x + {outgoing_tunnel_type:?}",
                incoming_connection_uses_tls=self.incoming_connection_uses_tls,
                tls_terminated = self.tls_terminated,
                http_terminated = self.http_terminated,
                outgoing_tunnel_type = self.outgoing_tunnel_type
            ).to_string())
        }

    }
}


