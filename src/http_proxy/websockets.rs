use std::sync::Arc;
use hyper_tungstenite::HyperWebsocket;
use hyper::{body::Incoming as IncomingBody, Request};
use tokio_rustls::rustls::ClientConfig;
use crate::CustomError;
use futures_util::{SinkExt,StreamExt};
use super::ReverseProxyService;
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
    
    let cfg = { service.state.config.read().await.clone() };
    
    
    let target = {

        if let Some(proc) = cfg.hosted_process.iter().flatten().find(|p| { 
            req_host_name == p.host_name 
            || p.capture_subdomains.unwrap_or_default() && req_host_name.ends_with(&format!(".{}",p.host_name)) 
        }) {
            crate::http_proxy::utils::Target::Proc(proc.clone())
        } else if let Some(remsite) = cfg.remote_target.iter().flatten().find(|x| { 
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
             let next_backend = x.next_backend(&service.state, crate::configuration::BackendFilter::Any).await
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
    

    let svc_scheme = if service.is_https {"wss"} else { "ws" };
   
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
    
    tracing::trace!("Successfully upgraded websocket connection from client");

    let (mut ws_up_write,mut ws_up_read) = upstream_client.0.split();

    tracing::trace!("Setting up tunnel between client and upstream websocket");

    let (mut websocket_write,mut websocket_read) = ws.await
        .map_err(|e|CustomError(format!("{e:?}")))?.split();
    
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
    };

    _ = tokio::join!(client_to_upstream, upstream_to_client);
    Ok(())

}
