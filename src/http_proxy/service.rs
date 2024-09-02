use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use bytes::Bytes;
use http_body::Frame;

use http_body_util::{Either, Full, StreamBody};
use hyper::service::Service;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tokio::net::TcpStream;
use tokio_stream::wrappers::ReceiverStream;
use std::future::Future;
use std::pin::Pin;
use crate::global_state::GlobalState;
use crate::types::app_state::ProcState;
use crate::CustomError;
use hyper::{Method, StatusCode};
use lazy_static::lazy_static;
use super::{ProcMessage, ReverseProxyService, WrappedNormalResponse};
use super::proxy;



pub enum SomeIo {
    Https(hyper_util::rt::TokioIo<tokio_rustls::server::TlsStream<TcpStream>>),
    Http(hyper_util::rt::TokioIo<TcpStream>)

}

lazy_static! {
    static ref SERVER_ONE: hyper_util::server::conn::auto::Builder<TokioExecutor> = 
        hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
}
pub async fn serve(service:ReverseProxyService,io:SomeIo) {
    
    let result = match io {
        SomeIo::Https(tls_stream) => {
            SERVER_ONE.serve_connection_with_upgrades(tls_stream, service).await
        },
        SomeIo::Http(tcp_stream) => {
            SERVER_ONE
               .serve_connection_with_upgrades(tcp_stream, service).await
        },
        
    };
    match result {
        Ok(_) => {},
        Err(e) => {
            tracing::warn!("{e:?}")
        }
    }
}

type FullOrStreamBody = 
    http_body_util::Either<
        Full<bytes::Bytes>, 
        StreamBody<
            ReceiverStream<
                Result<
                    hyper::body::Frame<bytes::Bytes>, 
                    CustomError
                >
            >
        >
    >;

pub type EpicBody = 
    http_body_util::Either<
        super::WrappedNormalResponseBody,
        FullOrStreamBody
    >;

pub type EpicResponse = hyper::Response<EpicBody>;


pub fn create_response_channel(buf_size:usize) -> (
    tokio::sync::mpsc::Sender<Result<Frame<Bytes>, CustomError>>,
    tokio::sync::mpsc::Receiver<Result<Frame<Bytes>, CustomError>>,
 ) { tokio::sync::mpsc::channel(buf_size) }

pub fn create_epic_string_full_body(text:&str) -> EpicBody {
    EpicBody::Right(FullOrStreamBody::Left(Full::new(Bytes::from(text.to_owned()))))
}

pub fn create_stream_response(rx:tokio::sync::mpsc::Receiver<Result<Frame<Bytes>, CustomError>>) -> EpicResponse {
    EpicResponse::new(EpicBody::Right(Either::Right(StreamBody::new(ReceiverStream::new(rx)))))
}


pub async fn create_simple_response_from_incoming(res:WrappedNormalResponse) -> Result<EpicResponse,CustomError> {
    let (p,b) = res.into_parts();
    Ok(EpicResponse::from_parts(p,Either::Left(b)))
}

fn handle_ws(svc:ReverseProxyService,mut req:hyper::Request<hyper::body::Incoming>) -> Result<EpicResponse,CustomError> {

    let (response, websocket) = hyper_tungstenite::upgrade(&mut req, None)
        .map_err(|e|CustomError(format!("{e:?}")))?;
    
    tokio::spawn(async move {crate::http_proxy::websockets::handle_ws(req, svc,websocket).await });
    let (p,b) = response.into_parts();
    Ok(EpicResponse::from_parts(p,Either::Right(Either::Left(b))))
}

impl<'a> Service<Request<hyper::body::Incoming>> for ReverseProxyService {
    type Response = EpicResponse;
    type Error = CustomError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    
    fn call(&self, req: hyper::Request<hyper::body::Incoming>) -> Self::Future {    

        tracing::trace!("INCOMING REQ: {:?}",req);
        tracing::trace!("VERSION: {:?}",req.version());

        // handle websocket upgrades separately
        if hyper_tungstenite::is_upgrade_request(&req) {
            let res =  handle_ws(self.clone(),req);
            return Box::pin(async move { res })         
        }

        //handle h2 stream handler test req
        if req.version() == hyper::Version::HTTP_2 && req.uri().query().filter(|x|x.contains("stream_test_odd_box")).is_some() {
            let f= crate::http_proxy::utils::h2_stream_test(req);
            return Box::pin(async move {
               f.await
            })
        }
        
        // handle normal proxy path
        let f = handle_http_request(
            self.remote_addr.expect("there must always be a client"),
            req,
            self.tx.clone(),
            self.state.clone(),
            self.is_https_only,
            self.client.clone(),
            self.h2_client.clone()
        );
        
        return Box::pin(async move {
            match f.await {
                Ok(x) => {
                    Ok(x)
                },
                Err(e) => {
                    Err(CustomError(format!("{e:?}")))
                },
            }
        })
    

    }
}

#[allow(dead_code)]
async fn handle_http_request(
    client_ip: std::net::SocketAddr, 
    req: Request<hyper::body::Incoming>,
    tx: Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    is_https:bool,
    client:  Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
    h2_client: Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,

) -> Result<EpicResponse, CustomError> {
    


    let req_host_name = 
        if let Some(hh) = req.headers().get("host") { 
            let hostname_and_port = hh.to_str().map_err(|e|CustomError(format!("{e:?}")))?.to_string();
            hostname_and_port.split(":").collect::<Vec<&str>>()[0].to_owned()
        } else { 
            req.uri().authority().ok_or(CustomError(format!("No hostname and no Authority found")))?.host().to_string()
        };

    
    tracing::trace!("Handling request from {client_ip:?} on hostname {req_host_name:?}");
    
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

    if let Some(r) = intercept_local_commands(&req_host_name,&params,req_path,tx.clone()).await {
        return Ok(r)
    }
    
    let found_hosted_target = {
        let cfg_guard = state.config.read().await;
        if let Some(processes) = &cfg_guard.hosted_process {
            if let Some(pp) = processes.iter().find(|p| {
                req_host_name == p.host_name
                || p.capture_subdomains.unwrap_or_default() 
                && req_host_name.ends_with(&format!(".{}",p.host_name))
            }) {
                Some(pp.clone())
            } else {
                None
            }
        } else {
            None
        }
    };


    if let Some(target_proc_cfg) = found_hosted_target {

        let current_target_status : Option<crate::ProcState> = {
            let info = state.app_state.site_status_map.get(&target_proc_cfg.host_name);
            match info {
                Some(data) => Some(data.value().clone()),
                None => None,
            }
        };

        match current_target_status {
            Some(ProcState::Running) | Some(ProcState::Faulty) | Some(ProcState::Starting) => {},
            _ => {
                // auto start site in case its been disabled by other requests
                _ = tx.send(super::ProcMessage::Start(target_proc_cfg.host_name.to_owned())).map_err(|e|format!("{e:?}"));
            }
        }

        
        if let Some(cts) = current_target_status {
            if cts == crate::ProcState::Stopped || cts == crate::ProcState::Starting || cts == crate::ProcState::Faulty {
                match req.method() {
                    &Method::GET => {
                        // todo - opt in/out via cfg ?
                        return Ok(EpicResponse::new(create_epic_string_full_body(&please_wait_response())))
                    }  
                    _ => {
                        // we do this to give services some time to wake up instead of failing requests while cold-starting sites
                        tokio::time::sleep(Duration::from_secs(3)).await
                    }           
                }                 
            }
        }


        let port = if let Some(active_port) = target_proc_cfg.active_port {
            active_port
        } else {
            return Err(CustomError(format!("No active port found for {req_host_name}.")))
        };
        
        let enforce_https = target_proc_cfg.https.is_some_and(|x|x);
        let scheme = if enforce_https { "https" } else { "http" };

        let mut original_path_and_query = req.uri().path_and_query()
            .and_then(|x| Some(x.as_str())).unwrap_or_default();
        if original_path_and_query == "/" { original_path_and_query = ""}


        let parsed_host_name = {
            let forward_subdomains = target_proc_cfg.forward_subdomains.unwrap_or_default();
            if forward_subdomains {
                if let Some(subdomain) = get_subdomain(&req_host_name, &target_proc_cfg.host_name) {
                    Cow::Owned(format!("{subdomain}.{}", &target_proc_cfg.host_name))
                } else {
                    Cow::Borrowed(&target_proc_cfg.host_name)
                }
            } else {
                Cow::Borrowed(&target_proc_cfg.host_name)
            }
        };

        let target_url = format!("{scheme}://{}:{}{}",
            parsed_host_name,
            port,
            original_path_and_query
        );
        
        // we add the host flag manually in proxy method, this is only to avoid dns lookup for local targets.
        // todo: opt in/out via cfg
        let skip_dns_for_local_target_url = format!("{scheme}://{}:{}{}",
            "127.0.0.1",
            port,
            original_path_and_query
        );

        let target_cfg = target_proc_cfg.clone();
        let hints = target_cfg.hints.clone();
        let target = crate::http_proxy::Target::Proc(target_cfg);

        let result = 
            proxy(
                &parsed_host_name,
                is_https,
                state.clone(),
                req,
                &skip_dns_for_local_target_url,
                target,
                client_ip,
                client,
                h2_client,
                &target_url,
                enforce_https,
                crate::configuration::v2::Backend {
                    hints: hints,
                    address: parsed_host_name.to_string(),
                    port: port,
                    https: Some(enforce_https)
                }
            ).await;

        map_result(&target_url,result).await
    }

    else {
        
        if let Some(remote_target_cfg) = &state.config.read().await.remote_target.clone().unwrap_or_default().iter().find(|p|{
            //tracing::info!("comparing incoming req: {} vs {} ",req_host_name,p.host_name);
            req_host_name == p.host_name
            || p.capture_subdomains.unwrap_or_default() && req_host_name.ends_with(&format!(".{}",p.host_name))
        }) {

            return perform_remote_forwarding(req_host_name,is_https,state.clone(),client_ip,remote_target_cfg,req,client.clone(),h2_client.clone()).await
        }

        tracing::warn!("Received request that does not match any known target: {:?}", req_host_name);
        let body_str = format!("Sorry, I don't know how to proxy this request.. {:?}", req);
        
        let mut response = EpicResponse::new(create_epic_string_full_body(&body_str));
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        Ok(response)
    }

}

fn get_subdomain(requested_hostname: &str, backend_hostname: &str) -> Option<String> {
    if requested_hostname == backend_hostname { return None };
    if requested_hostname.to_uppercase().ends_with(&backend_hostname.to_uppercase()) {
        let part_to_remove_len = backend_hostname.len();
        let start_index = requested_hostname.len() - part_to_remove_len;
        if start_index == 0 || requested_hostname.as_bytes()[start_index - 1] == b'.' {
            return Some(requested_hostname[..start_index].trim_end_matches('.').to_string());
        }
    }
    None
}

async fn perform_remote_forwarding(
    req_host_name:String,
    _is_https:bool,
    state: Arc<GlobalState>,
    client_ip:std::net::SocketAddr,
    remote_target_config:&crate::configuration::v2::RemoteSiteConfig,
    req:hyper::Request<IncomingBody>,
    client:  Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
    h2_client: Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
) -> Result<EpicResponse,CustomError> {
    
    
    let mut original_path_and_query = req.uri().path_and_query()
        .and_then(|x| Some(x.as_str())).unwrap_or_default();
    if original_path_and_query == "/" { original_path_and_query = ""}
   
    let next_backend_target = if let Some(b) = remote_target_config.next_backend(&state, crate::configuration::v2::BackendFilter::Any).await {
        b
    } else {
        return Err(CustomError("No backend found".to_string()))
    };
    
    // if a target is marked with http, we wont try to use http
    let enforce_https = next_backend_target.https.unwrap_or_default();
   
    let scheme = if enforce_https { "https" } else { "http" }; 
    
    let resolved_host_name = {

        if remote_target_config.forward_subdomains.unwrap_or_default() {
            if let Some(subdomain) = get_subdomain(&req_host_name, &remote_target_config.host_name) {
            //tracing::debug!("remote forward terminating proxy rewrote subdomain: {subdomain}!");
                format!("{subdomain}.{}", &next_backend_target.address)
            } else {
                next_backend_target.address.clone()
            }
        } else {
            next_backend_target.address.clone()
        }
    };
        
    let target_url = format!("{scheme}://{}:{}{}",
        resolved_host_name,
        next_backend_target.port,
        original_path_and_query
    );

    //tracing::info!("Incoming request to '{}' for remote proxy target {target_url}",next_backend_target.address);
    let result = 
        proxy(
            &req_host_name,
            next_backend_target.https.unwrap_or_default(),
            state.clone(),
            req,
            &target_url,
            crate::http_proxy::Target::Remote(remote_target_config.clone()),
            client_ip,
            client,
            h2_client,
            &target_url,
            next_backend_target.https.unwrap_or_default(),
            next_backend_target
        ).await;

    map_result(&target_url,result).await

}

async fn map_result(target_url:&str,result:Result<crate::http_proxy::ProxyCallResult,crate::http_proxy::ProxyError>) -> Result<EpicResponse,CustomError> {
    
    match result {
        Ok(super::ProxyCallResult::EpicResponse(epic_response)) => {
            return Ok(epic_response)
        }
        Ok(crate::http_proxy::ProxyCallResult::NormalResponse(response)) => {
                return create_simple_response_from_incoming(response).await;
        }
        Err(crate::http_proxy::ProxyError::LegacyError(error)) => {
            tracing::debug!("HyperLegacyError - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("HyperLegacyError - {error:?}")))
                .expect("body building always works"))
        },
        Err(crate::http_proxy::ProxyError::HyperError(error)) => {
            tracing::debug!("HyperError - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("HyperError - {error:?}")))
                .expect("body building always works"))
        },
        Err(crate::http_proxy::ProxyError::OddBoxError(error)) => {
            tracing::debug!("OddBoxError - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("ODD-BOX-ERROR: {error:?}")))
                .expect("body building always works"))
        },
        Err(crate::http_proxy::ProxyError::ForwardHeaderError) => {
            tracing::debug!("ForwardHeaderError - Failed to call {}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body("ForwardHeaderError"))
                .expect("body building always works"))
        },
        Err(crate::http_proxy::ProxyError::InvalidUri(error)) => {
            tracing::debug!("InvalidUri - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("InvalidUri: {error:?}")))
                .expect("body building always works"))
        }, 
        Err(crate::http_proxy::ProxyError::UpgradeError(error)) => {
            tracing::debug!("UpgradeError - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("UpgradeError: {error:?}")))
                .expect("body building always works"))
        }
    }
}


// TODO:
//        make this it opt-in with cfg v2.
//        make it true when coming from legacy or v1 to have backward compatibility
//        (we have admin-api for this now and so it is bad for performance to use this instead)
async fn intercept_local_commands(
    req_host_name:&str,
    params:&std::collections::HashMap<String, String>,
    req_path:&str,
    tx:Arc<tokio::sync::broadcast::Sender<ProcMessage>>
) -> Option<EpicResponse> {

    if req_host_name != "127.0.0.1" && req_host_name != "localhost" {
        return None
    }
    
    if req_path.eq("/STOP") {
        let target : Option<&String> = params.get("proc");
        
        let s = target.unwrap_or(&String::from("all")).clone();
        tracing::warn!("Handling order STOP ({})",s);
        let result = tx.send(ProcMessage::Stop(s)).map_err(|e|format!("{e:?}"));
        
        if let Err(e) = result {
            let mut response = EpicResponse::new(create_epic_string_full_body(&format!("{e:?}")))
;            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Some(response)
        }

        let html = r#"
            <center>
                <h2>Stop signal received.</h2>
                
                <form action="/START">
                    <input type="submit" value="Resume" />
                </form>            

                <p>The proxy will also resume if you visit any of the stopped sites</p>
            </center>
        "#;
        return Some(EpicResponse::new(create_epic_string_full_body(html)))
    } 
    
    if req_path.eq("/START") {
        

        let target : Option<&String> = params.get("proc");
        
        let s = target.unwrap_or(&String::from("all")).clone();
        tracing::warn!("Handling order START ({})",s);
        let result: Result<usize, String> = tx.send(ProcMessage::Start(s)).map_err(|e|format!("{e:?}"));
        
        if let Err(e) = result {
            let mut response = EpicResponse::new(create_epic_string_full_body(&format!("{e:?}")))
;            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Some(response)
        }

        let html = r#"
            <center>
                <h2>Start signal received.</h2>
                
                <form action="/STOP">
                    <input type="submit" value="Stop" />
                </form>            

            </center>
        "#;
        return Some(EpicResponse::new(create_epic_string_full_body(html)))
    }

    None
}

// TODO - package these mjs/jsons with the binary if we want to keep it as is
// otherwise get rid of the deps
fn please_wait_response() -> String {
    r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8" http-equiv="refresh" content="5;">
            <title>Starting site, please wait..</title>
            <style>
                body {
                    margin: 0;
                    padding: 0;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    height: 100vh;
                }
                .lottie-container {
                    width: 300px;
                    height: 300px;
                }
            </style>
        </head>
        <body>
            <div class="lottie-container">
                <script src="https://unpkg.com/@dotlottie/player-component@latest/dist/dotlottie-player.mjs" type="module"></script> 
                <dotlottie-player src="https://lottie.host/fb304345-633f-4b4a-a49f-541b7abbf165/rn9UiCEOBN.json" background="transparent" speed="0.5" loop autoplay></dotlottie-player>
            </div>
        </body>
        </html>
    "#.to_string()
}






