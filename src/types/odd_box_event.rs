use serde::Serialize;

use crate::logging::LogMsg;

use super::{proxy_state::{ConnectionKey, ProxyActiveTCPConnection}, site_status::SiteStatusEvent};

#[derive(Debug, Clone, Serialize)]
pub enum Event {
    Log(LogMsg),
    TcpEvent(TCPEvent),
    SiteStatusChange(SiteStatusEvent),
    // --- Not sure how to properly handle these yet ---
    Http1Event(HTTP1Event),
    Http2Event(
        String // TODO
    ),
    WebSocketEvent(WebSocketEvent)
}


#[derive(Debug, Clone, Serialize)]
pub enum WebSocketEvent {
    Incoming(String),
    Outgoing(String) 
}

#[derive(Debug, Clone, Serialize)]
pub enum HTTP1Event {
    Request(HTTPRequestEvent),
    Response(HTTPResponseEvent)
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
    Update(ProxyActiveTCPConnection)
}