use crate::hyper_reverse_proxy::ReverseProxy;

use super::types;
use hyper::client::HttpConnector;
use hyper_trust_dns::TrustDnsResolver;
use types::*;
use hyper::{Server, Response, Body, StatusCode, Version, Client};
use hyper::server::conn::AddrStream;

use futures_util::StreamExt;
use futures_util::SinkExt;


lazy_static::lazy_static! {
    static ref  PROXY_CLIENT_HTTPS_RUSTLS: ReverseProxy<hyper_trust_dns::RustlsHttpsConnector> = {
        ReverseProxy::new(
            hyper::Client::builder().build(TrustDnsResolver::default().into_rustls_webpki_https_connector()),
        )
    };
}

use hyper_tls::HttpsConnector;
lazy_static::lazy_static! {
    static ref  PROXY_CLIENT_HTTPS: ReverseProxy<HttpsConnector<HttpConnector>> = {
            ReverseProxy::new(
                Client::builder().build::<_, hyper::Body>(HttpsConnector::new())
            )
    };
}


lazy_static::lazy_static! {
    static ref  PROXY_CLIENT_HTTP: ReverseProxy<HttpConnector> = {
        ReverseProxy::new(
            hyper::Client::builder().build_http()
        )
    };
}


lazy_static::lazy_static! {
    static ref  PROXY_CLIENT_H2C: ReverseProxy<HttpConnector> = {
        ReverseProxy::new(
            hyper::Client::builder().http2_only(true).build_http()
        )
    };
}


pub(crate) async fn rev_prox_srv(cfg: &Config, bind_addr: &str, tx: tokio::sync::broadcast::Sender<(String, bool)>) -> Result<(), String> {
    tracing::info!("Starting reverse proxy!");

    let addr: std::net::SocketAddr = bind_addr.parse().map_err(|_| "Could not parse ip:port.")?;

    let make_svc = hyper::service::make_service_fn(|socket: &AddrStream| {

        let remote_addr = socket.remote_addr();
        let cfg = cfg.clone();
        let tx = tx.clone();
        
        async move {
            Ok::<_, CustomError>(hyper::service::service_fn(move |req: hyper::Request<Body>| {
                let cfg = cfg.clone();
                let tx = tx.clone();
                async move {
                    if req.headers().get("upgrade").map(|v| v.to_str().ok() == Some("websocket")).unwrap_or(false) {
                        //info!("accepting websocket connection");
                        handle_ws(cfg,remote_addr.ip(),req,tx).await
                    } else {
                        //info!("accepting connection");
                        handle(cfg, remote_addr.ip(), req, tx).await
                    }
                }
            }))
        }
    });
      
    tracing::info!("Starting proxy service on {:?}. Press ctrl-c to exit.", addr);
    _ = Server::bind(&addr).serve(make_svc).await;

    Ok(())
    
}

async fn handle_ws(
    cfg:Config,
    _client_ip: std::net::IpAddr, 
    req: hyper::Request<hyper::Body>,
    _tx:tokio::sync::broadcast::Sender<(String,bool)>
)  -> Result<hyper::Response<hyper::Body>, CustomError>  {

    let req_host_name = 
        if let Some(hh) = req.headers().get("host") { 
            hh.to_str().map_err(|e|CustomError(format!("{e:?}")))?.to_string()
        } else { 
            req.uri().authority().ok_or(CustomError(format!("No hostname and no Authority found")))?.host().to_string()
        } ;
        
    let req_path = req.uri().path();
    
    tracing::debug!("Handling websocket request: {req_host_name:?} --> {req_path}");

    let proc = cfg.processes.iter().find(|p| { req_host_name == p.host_name })
        .ok_or(CustomError(format!("No target is configured to handle requests to {req_host_name}")))?;
    
    let proto = if let Some(true) = proc.https { "wss" } else { "ws" };
    let ws_url = format!("{proto}://127.0.0.1:{}",proc.port);

    let ws_stream = tokio_tungstenite::connect_async(&ws_url).await;
    match ws_stream {
        Ok((mut ws_stream, _response)) => {
            tokio::task::spawn(async move {
                while let Some(Ok(msg)) = ws_stream.next().await {
                    if let Err(e) = ws_stream.send(msg).await {
                        eprintln!("Error forwarding message: {}", e);
                        break;
                    }
                }
            });
            let response = hyper::Response::builder()
                .status(101)
                .body(hyper::Body::from("WebSocket proxy established"))
                .expect("body building always works");

            Ok(response)
        },
        Err(e) => {
            Err(CustomError::from(e))
        },
    }
}

async fn handle(
    cfg:Config,
    client_ip: std::net::IpAddr, 
    req:hyper::Request<hyper::Body>,
    tx:tokio::sync::broadcast::Sender<(String,bool)>
) -> Result<hyper::Response<hyper::Body>, CustomError> {
    
    let req_host_name = 
        if let Some(hh) = req.headers().get("host") { 
            hh.to_str().map_err(|e|CustomError(format!("{e:?}")))?.to_string()
        } else { 
            req.uri().authority().ok_or(CustomError(format!("No hostname and no Authority found")))?.host().to_string()
        } ;

    tracing::debug!("Handling request: {req_host_name:?}");
    
    let req_path = req.uri().path();
    
    let params: std::collections::HashMap<String, String> = req
        .uri()
        .query()
        .map(|v| {
            url::form_urlencoded::parse(v.as_bytes())
                .into_owned()
                .collect()
        })
        .unwrap_or_else(std::collections::HashMap::new);

    if req_path.eq("/STOP") {
        let target : Option<&String> = params.get("proc");
        
        let s = target.unwrap_or(&String::from("all")).clone();
        tracing::warn!("Handling order STOP ({})",s);
        let result = tx.send((s,false)).map_err(|e|format!("{e:?}"));
        
        if let Err(e) = result {
            let mut response = Response::new(Body::from(format!("{e:?}")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Ok(response)
        }

        let html = r#"
            <center>
                <h2>All sites stopped by your command</h2>
                
                <form action="/START">
                    <input type="submit" value="Resume" />
                </form>            

                <p>The proxy will also resume if you visit any of the sites</p>
            </center>
        "#;
        return Ok(Response::new(Body::from(html)))
    } 
    
    if req_path.eq("/START") {
        

        let target : Option<&String> = params.get("proc");
        
        let s = target.unwrap_or(&String::from("all")).clone();
        tracing::warn!("Handling order START ({})",s);
        let result: Result<usize, String> = tx.send((s,true)).map_err(|e|format!("{e:?}"));
        
        if let Err(e) = result {
            let mut response = Response::new(Body::from(format!("{e:?}")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Ok(response)
        }

        let html = r#"
            <center>
                <h2>All sites resumed</h2>
                
                <form action="/STOP">
                    <input type="submit" value="Stop" />
                </form>            

            </center>
        "#;
        return Ok(Response::new(Body::from(html)))
    }
    
    if let Some(target_cfg) = cfg.processes.iter().find(|p| {
            req_host_name == p.host_name
    }) {
        
        // auto start site in case its been disabled by other requests
        _ = tx.send((target_cfg.host_name.to_owned(),true)).map_err(|e|format!("{e:?}"));

        let scheme = if let Some(true) = target_cfg.https { "https" } else { "http" };

        let target_url = format!("{scheme}://{}:{}",target_cfg.host_name,target_cfg.port);
        
        let result =  if req.version() == Version::HTTP_2 {
            if scheme == "http" {
                // http2 over http (h2c)
                PROXY_CLIENT_H2C.call(client_ip, &target_url, req).await
            } else {
                // http2 with tls (h2)
                PROXY_CLIENT_HTTPS.call(client_ip, &target_url, req).await
            }
            
        } else {
            if scheme == "http" {
                PROXY_CLIENT_HTTP.call(client_ip, &target_url, req).await
            } else {
                PROXY_CLIENT_HTTPS.call(client_ip, &target_url, req).await
            }
        };

        match result {
            Ok(response) => {
                tracing::trace!("Proxy call to {} succeeded", &target_cfg.host_name);
                Ok(response)
            }
            Err(error) => {
                tracing::warn!("Failed to call {}: {error:?}", &target_cfg.host_name);
                Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(format!("{error:?}").into())
                    .expect("body building always works"))
            }
        }
    }

    else {
        tracing::warn!("Received request that does not match any known target: {:?}", req_host_name);
        let body_str = format!("Sorry, I don't know how to proxy this request.. {:?}", req);
        Ok(Response::new(Body::from(body_str)))
    }

}
