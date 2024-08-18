use std::collections::HashMap;
use std::net::SocketAddr;
use crate::tcp_proxy::ReverseTcpProxyTarget;

#[derive(Debug)]
pub (crate) struct ProxyStats {
    pub (crate) received_tcp_connections : usize,
    pub (crate) active_connections : HashMap<ConnectionKey,ProxyActiveConnection>
}


#[derive(Debug,Clone)]
pub (crate) enum ProxyActiveConnectionType {
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

pub type ConnectionKey = (SocketAddr,uuid::Uuid);

#[derive(Debug,Clone)]
#[allow(dead_code)]
pub (crate) struct ProxyActiveConnection {
    pub (crate) target : ReverseTcpProxyTarget,
    pub (crate) creation_time : chrono::DateTime<chrono::Local>,
    pub (crate) description : Option<String>,
    pub (crate) connection_type : ProxyActiveConnectionType,
    pub (crate) source_addr: SocketAddr,
    pub (crate) target_addr: String
}
