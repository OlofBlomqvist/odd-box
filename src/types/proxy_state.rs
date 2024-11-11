use std::{net::SocketAddr, sync::Weak};
use std::sync::atomic::AtomicUsize;

use crate::configuration::v2::DirServer;
use crate::{configuration::v2::Backend, tcp_proxy::{GenericManagedStream, ReverseTcpProxyTarget}};


#[derive(Debug)]
pub struct ProxyStats {
    pub active_connections : dashmap::DashMap<ConnectionKey,ProxyActiveTCPConnection>,
    pub connections_per_hostname : dashmap::DashMap<String,AtomicUsize>
}

pub type ConnectionKey = u64;

#[derive(Debug,Clone)]
#[allow(dead_code)]
pub struct ProxyActiveTCPConnection {
    pub connection_key_pointer : Weak<ConnectionKey>,
    pub client_addr : String,
    pub target: Option<ReverseTcpProxyTarget>,
    pub backend: Option<Backend>,
    /// This means that the data inside of this TCP connection is encrypted using tls
    pub incoming_connection_uses_tls: bool,
    /// This means we have terminated the incoming TLS session, meaning we can see the data between the client and the proxy.
    pub tls_terminated: bool,
    /// This means we have terminated the http (and tls if used) connection and have established a new http(s) connection to the target
    pub http_terminated: bool,
    pub outgoing_connection_is_tls: bool,
    pub is_odd_box_admin_api_req: bool,
    pub dir_server: Option<DirServer>
}