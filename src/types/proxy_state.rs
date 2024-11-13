use std::sync::Weak;
use std::sync::atomic::AtomicUsize;

use crate::configuration::v2::DirServer;
use crate::{configuration::v2::Backend, tcp_proxy::ReverseTcpProxyTarget};


#[derive(Debug)]
pub struct ProxyStats {
    pub active_connections : dashmap::DashMap<ConnectionKey,ProxyActiveTCPConnection>,
    pub connections_per_hostname : dashmap::DashMap<String,AtomicUsize>
}

pub type ConnectionKey = u64;

use serde::Serialize;

use super::connection_type::ConnectionType;

#[derive(Debug,Clone,Serialize)]
#[allow(dead_code)]
pub struct ProxyActiveTCPConnection {
    pub tcp_peer_addr : String,
    #[serde(skip)]
    pub connection_key_pointer : Weak<ConnectionKey>,
    pub connection_key: ConnectionKey,
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
    pub dir_server: Option<DirServer>,
    pub version : u64,
    pub resolved_connection_type: Option<ConnectionType>,
    pub resolved_connection_type_description: Option<String>,
}

impl ProxyActiveTCPConnection {
    pub fn get_connection_type(&self) -> ConnectionType {
        match (
            self.incoming_connection_uses_tls,
            self.tls_terminated,
            self.http_terminated,
            self.outgoing_connection_is_tls,
        ) {
            (false,false,false,false) => ConnectionType::PlainEndToEndForwarding,
            (true, true, true, true) => ConnectionType::FullTlsTerminatedWithHttpReEncryptedOutgoingTls,
            (true, true, true, false) => ConnectionType::MixedTerminationWithPlainOutgoing,
            (true, true, false, true) => ConnectionType::TlsTerminatedWithReEncryptedOutgoingTls,
            (true, true, false, false) => ConnectionType::TlsTerminatedWithPlainOutgoing,
            (true, false, true, true) => ConnectionType::OpaqueIncomingTlsPassthroughWithOutgoingTls,
            (true, false, true, false) => ConnectionType::OpaqueIncomingTlsPassthroughWithPlainOutgoing,
            (true, false, false, true) => ConnectionType::OpaqueIncomingTlsPassthroughWithOutgoingTls,
            (true, false, false, false) => ConnectionType::OpaqueIncomingTlsPassthroughWithPlainOutgoing,
            (false, true, true, true) => ConnectionType::HttpTerminatedWithOutgoingTls,
            (false, true, true, false) => ConnectionType::HttpTerminatedWithPlainOutgoing,
            (false, true, false, true) => ConnectionType::TlsTerminatedWithReEncryptedOutgoingTls,
            (false, true, false, false) => ConnectionType::TlsTerminatedWithPlainOutgoing,
            (false, false, true, true) => ConnectionType::HttpTerminatedWithOutgoingTls,
            (false, false, true, false) => ConnectionType::HttpTerminatedWithPlainOutgoing,
            (false, false, false, true) => ConnectionType::OpaqueTlsForwarding
        }

    }
}