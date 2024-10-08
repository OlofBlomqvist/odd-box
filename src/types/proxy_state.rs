use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;

#[derive(Debug)]
pub struct ProxyStats {
    pub active_connections : dashmap::DashMap<ConnectionKey,ProxyActiveConnection>,
    pub tunnelled_tcp_connections_per_hostname : dashmap::DashMap<String,AtomicUsize>,
    pub terminated_http_connections_per_hostname : dashmap::DashMap<String,AtomicUsize>
}


#[derive(Debug,Clone)]
pub enum ProxyActiveConnectionType {
    TcpTunnelUnencryptedHttp, 
    TcpTunnelTls,
    TerminatingHttp {
        incoming_scheme : String,
        incoming_http_version : String,
        outgoing_scheme : String,
        outgoing_http_version: String
    },
    TerminatingWs {
        incoming_scheme : String,
        incoming_http_version : String,
        outgoing_scheme : String,
        outgoing_http_version: String
    }
}

pub type ConnectionKey = u64;

#[derive(Debug,Clone)]
#[allow(dead_code)]
pub struct ProxyActiveConnection {
    pub target_name : String,
    pub creation_time : chrono::DateTime<chrono::Local>,
    pub description : Option<String>,
    pub connection_type : ProxyActiveConnectionType,
    pub source_addr: SocketAddr,
    pub target_addr: String
}
