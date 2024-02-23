mod websockets;
mod service;
mod utils;
pub (crate) use service::*;
pub (crate) use utils::*;
pub (crate) use crate::configuration::ConfigWrapper;

#[derive(Clone,Debug)]
pub enum ProcMessage {
    StartAll,
    StopAll,
    Start(String),
    Stop(String)
}

#[derive(Debug, Clone)]
pub struct ReverseProxyService {
    pub(crate) cfg :  ConfigWrapper,
    pub(crate) state: std::sync::Arc<tokio::sync::RwLock<crate::AppState>>,
    pub(crate) remote_addr : Option<std::net::SocketAddr>,
    pub(crate) tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    pub(crate) is_https_only:bool
}
