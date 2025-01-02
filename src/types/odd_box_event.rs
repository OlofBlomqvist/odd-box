use serde::Serialize;

use crate::logging::LogMsg;

use super::{proxy_state::{ConnectionKey, ProxyActiveTCPConnection}, site_status::SiteStatusEvent};

#[derive(Debug, Clone, Serialize)]
pub enum GlobalEvent {

    TcpEvent(TCPEvent),

    // this is for when we have terminated http and use a separate hyper client to initiate a new http connection.
    // when we send actual response to client or receive from client, that will end up as a tcpevent instead.
    // here we just make it possible to observe actual communication between odd-box and the backend in the case
    // where we may have modified the packets.
    SentHttpRequestToBackend(ConnectionKey,String), // TODO - use better structure
    GotResponseFromBackend(ConnectionKey,String)    // TODO - use better structure
}

#[derive(Debug, Clone, Serialize)]
pub enum EventForWebsocketClients {
    Log(LogMsg),
    SiteStatusChange(SiteStatusEvent),
    Http1Event(ConnectionKey, crate::observer::obs::DecodedPacket),
    /// u32 here is a stream id
    Http2Event(ConnectionKey, Option<u32>, crate::observer::obs::DecodedPacket),
    SentReqToBackend(ConnectionKey,String),
    ReceivedResFromBackend(ConnectionKey,String),
    Unknown(ConnectionKey,String)
    
}

#[derive(Debug, Clone, Serialize)]
pub struct HTTPRequestEvent {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub version: String
}

#[derive(Debug, Clone, Serialize)]
pub struct HTTPResponseEvent {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub version: String
}


#[derive(Debug, Clone, Serialize)]
pub enum TCPEvent {
    Open(ProxyActiveTCPConnection),
    Close(ConnectionKey),
    Update(ProxyActiveTCPConnection),
    /// bool == is_http2
    RawBytesFromClientToOddBox(ConnectionKey,bool,Vec<u8>),
    /// bool == is_http2
    RawBytesFromOddBoxToClient(ConnectionKey,bool,Vec<u8>)
}