use serde::Serialize;

use super::site_status::SiteStatusEvent;
use crate::logging::LogMsg;

#[derive(Debug, Clone, Serialize)]
pub enum GlobalEvent {
    TcpEvent(TCPEvent),

    // this is for when we have terminated http and use a separate hyper client to initiate a new http connection.
    // when we send actual response to client or receive from client, that will end up as a tcpevent instead.
    // here we just make it possible to observe actual communication between odd-box and the backend in the case
    // where we may have modified the packets.
    SentHttpRequestToBackend(u64, String), // TODO - use better structure
    GotResponseFromBackend(u64, String),   // TODO - use better structure
}

#[derive(Debug, Clone, Serialize)]
pub enum EventForWebsocketClients {
    Log(LogMsg),
    SiteStatusChange(SiteStatusEvent),
    Http1Event(u64, String),
    /// u32 here is a stream id
    Http2Event(u64, Option<u32>, String),
    SentReqToBackend(u64, String),
    ReceivedResFromBackend(u64, String),
    Unknown(u64, String),
}

#[derive(Debug, Clone, Serialize)]
pub enum TCPEvent {
    Open(()),
    Close(u64),
    Update(()),
    /// bool == is_http2
    RawBytesFromClientToOddBox(u64, bool, Vec<u8>),
    /// bool == is_http2
    RawBytesFromOddBoxToClient(u64, bool, Vec<u8>),
}
