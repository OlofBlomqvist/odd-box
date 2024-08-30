mod websockets;
mod service;
mod utils;
use std::sync::Arc;

use hyper::body::Incoming;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
pub use service::*;
use tokio::sync::mpsc::Sender;
pub use utils::*;
pub use crate::configuration::ConfigWrapper;
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
    pub state: Arc<GlobalState>,
    pub remote_addr : Option<std::net::SocketAddr>,
    pub tx: std::sync::Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    pub is_https_only:bool,
    pub client: Client<HttpsConnector<HttpConnector>, Incoming>,
    pub h2_client: Client<HttpsConnector<HttpConnector>, Incoming>
}
