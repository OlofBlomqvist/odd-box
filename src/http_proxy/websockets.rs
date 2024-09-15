use std::{net::SocketAddr, sync::Arc};
use chrono::Local;
use hyper_tungstenite::HyperWebsocket;
use hyper::{body::Incoming as IncomingBody, Request};
use tokio_rustls::rustls::ClientConfig;
use crate::{global_state::GlobalState, CustomError};
use futures_util::{SinkExt,StreamExt};
use super::{ReverseProxyService, Target};
use crate::types::proxy_state::{
    ConnectionKey, 
    ProxyActiveConnection, 
    ProxyActiveConnectionType
};
use hyper_rustls::ConfigBuilderExt;

pub async fn handle_ws(req:Request<IncomingBody>,service:ReverseProxyService,ws:HyperWebsocket) -> Result<(),CustomError> {

    
    let req_host_name = 
        if let Some(hh) = req.headers().get("host") { 
            let hostname_and_port = hh.to_str().map_err(|e|CustomError(format!("{e:?}")))?.to_string();
            hostname_and_port.split(":").collect::<Vec<&str>>()[0].to_owned()
        } else { 
            req.uri().authority().ok_or(CustomError(format!("No hostname and no Authority found")))?.host().to_string()
        } ;
        
    let req_path_and_uri = req.uri().path_and_query();

    let req_path = if let Some(rpu) = req_path_and_uri { rpu.to_string() } else {
        "".to_owned()
    };
    
    tracing::trace!("Handling websocket request: {req_host_name:?} --> {req_path}");
    
    let read_guard = service.state.config.read().await;
    
    
    let target = {

        if let Some(proc) = read_guard.hosted_process.iter().flatten().find(|p| { 
            req_host_name == p.host_name 
            || p.capture_subdomains.unwrap_or_default() && req_host_name.ends_with(&format!(".{}",p.host_name)) 
        }) {
            crate::http_proxy::utils::Target::Proc(proc.clone())
        } else if let Some(remsite) = read_guard.remote_target.iter().flatten().find(|x| { 
            req_host_name == x.host_name 
            || x.capture_subdomains.unwrap_or_default() && req_host_name.ends_with(&format!(".{}",x.host_name)) 
        }) {
            crate::http_proxy::utils::Target::Remote(remsite.clone())
        } else {
            return Err(CustomError(format!("No target is configured to handle requests to {req_host_name}")))
        }
    };

    let (target_host,port,enforce_https) = match &target {
        
        crate::http_proxy::Target::Remote(x) => {
             let next_backend = x.next_backend(&service.state, crate::configuration::v2::BackendFilter::Any,false).await
                .ok_or(CustomError(format!("no backend found")))?;
             (
                next_backend.address.clone(),
                next_backend.port,
                next_backend.https.unwrap_or_default()
             )
        },
        crate::http_proxy::Target::Proc(x) => {
            let backend_is_https = x.https.unwrap_or_default();
            (
                x.host_name.clone(),
                x.active_port.unwrap_or_default(),
                backend_is_https
            )
        }
    };

    if 0 == port { 
        tracing::warn!("No port found for target {target_host}");
        return Err(CustomError(format!("no active port found for target {target_host} - possibly process is not running")));
    };
    

    let svc_scheme = if service.is_https_only {"wss"} else { "ws" };
   
    let proto = if enforce_https { "wss" } else { req.uri().scheme_str().unwrap_or(svc_scheme) };

    let ws_url = format!("{proto}://{target_host}:{}{}",port,req_path);
    
    tracing::debug!("initiating websocket tunnel to {}",ws_url);

    let client_tls_config = ClientConfig::builder_with_protocol_versions(tokio_rustls::rustls::ALL_VERSIONS)
        .with_native_roots()
        .expect("should always be able to build a tls client")
        .with_no_client_auth();
    
    let upstream_client = match tokio_tungstenite::connect_async_tls_with_config(
        ws_url.clone(),
        None,
        true,
        Some(tokio_tungstenite::Connector::Rustls(Arc::new(client_tls_config)))
    ).await {
        Ok(x) => {
            tracing::debug!("Successfully connected to target websocket");
            x
        },
        Err(e) => {
          
            tracing::warn!("FAILED TO CONNECT TO TARGET WEBSOCKET: {:?}",e);
            return Err(CustomError(format!("failed to connect to target websocket: {e:?}")))
        }
    };
    
    let target_version = upstream_client.1.version();
    
    let target_is_tls = match upstream_client.0.get_ref() {
        tokio_tungstenite::MaybeTlsStream::Rustls(_) => true,
        _ => false        
    };

    tracing::trace!("Successfully upgraded websocket connection from client");

    let (mut ws_up_write,mut ws_up_read) = upstream_client.0.split();

    tracing::trace!("Setting up tunnel between client and upstream websocket");

    let (mut websocket_write,mut websocket_read) = ws.await
        .map_err(|e|CustomError(format!("{e:?}")))?.split();
    
    let con = create_connection(
        req,
        target,
        &service.remote_addr.expect("there must be a client socket.."),
        target_is_tls,
        target_version,
        &ws_url,
        service.is_https_only
    );

    let con_key = add_connection(service.state.clone(),  con.clone()).await;

    // Forward messages from client to upstream
    let client_to_upstream = async {
        while let Some(message) = websocket_read.next().await {
            tracing::trace!("[WS] Got this message from a client: {:?}",message);
            match message {
                Ok(msg) => {
                    if ws_up_write.send(msg.clone()).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    // Forward messages from upstream to client
    let upstream_to_client = async {
        while let Some(message) = ws_up_read.next().await {
            tracing::trace!("[WS] Got this message from a server: {:?}",message);
            match message {
                Ok(msg) => {
                    if websocket_write.send(msg.clone()).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        del_connection(service.state.clone(), &con_key).await;

    };

    _ = tokio::join!(client_to_upstream, upstream_to_client);
    Ok(())

}


async fn add_connection(global_state:Arc<GlobalState>,connection:ProxyActiveConnection) -> ConnectionKey {
    let key = crate::generate_unique_id();
    _ = global_state.app_state.statistics.active_connections.insert(key, connection);
    key
}

async fn del_connection(global_state:Arc<GlobalState>,key:&ConnectionKey) {
    _ = global_state.app_state.statistics.active_connections.remove(key);
}

fn create_connection(
    req:Request<IncomingBody>,
    target:Target,
    client_addr:&SocketAddr,
    target_is_tls:bool,
    target_version: hyper::http::Version,
    target_addr: &str,
    known_tls_only: bool
) -> ProxyActiveConnection {
    
    let typ_info = 
        ProxyActiveConnectionType::TerminatingWs { 
            incoming_scheme: req.uri().scheme_str().unwrap_or(if known_tls_only { "WSS" } else {"WS"} ).to_owned(), 
            incoming_http_version: format!("{:?}",req.version()), 
            outgoing_http_version: format!("{:?}",target_version), 
            outgoing_scheme:(if target_is_tls { "WSS" } else { "WS" }).into()
        };

    ProxyActiveConnection {
        target_name: match target {
            Target::Proc(p) => p.host_name.clone(),
            Target::Remote(r) => r.host_name.clone()
        },
        source_addr: client_addr.clone(),
        target_addr: target_addr.to_owned(),
        creation_time: Local::now(),
        description: Some(format!("websocket connection")),
        connection_type: typ_info
    }
}