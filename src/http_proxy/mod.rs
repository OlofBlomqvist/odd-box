mod websockets;
mod service;
mod utils;
use rustls::ClientConfig;
pub (crate) use service::*;
use tokio::sync::mpsc::Sender;
pub (crate) use utils::*;
pub (crate) use crate::configuration::ConfigWrapper;
use crate::global_state::GlobalState;

#[derive(Clone,Debug)]
pub enum ProcMessage {
    StartAll,
    StopAll,
    Start(String),
    Stop(String),
    Delete(String,Sender<u8>)
}

#[derive(Debug, Clone)]
pub struct ReverseProxyService {
    pub(crate) state: GlobalState,
    pub(crate) remote_addr : Option<std::net::SocketAddr>,
    pub(crate) tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    pub(crate) is_https_only:bool,
    pub(crate) client_tls_config: ClientConfig
}
