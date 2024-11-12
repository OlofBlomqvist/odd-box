use serde::Serialize;

use crate::logging::LogMsg;

use super::proxy_state::ConnectionKey;

#[derive(Debug, Clone, Serialize)]
pub enum Event {
    Log(LogMsg),
    TcpOpen(crate::types::site_status::SiteStatus),
    TcpClose(ConnectionKey),
    TcpEvent {
        connection_key : ConnectionKey,
        event : TCPEvent
    },
}



#[derive(Debug, Clone, Serialize)]
pub struct TCPSessionInfo {
    pub connection_key : ConnectionKey,
    pub remote_address : std::net::SocketAddr,
    pub local_address : std::net::SocketAddr,
    pub active : bool
}

#[derive(Debug, Clone, Serialize)]
pub enum TCPEvent {
    SomethingHappend // placeholder. 
                     // we will add events like "data_received", "data_sent" with the contents
}