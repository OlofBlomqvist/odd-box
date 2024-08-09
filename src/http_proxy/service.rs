use std::sync::Arc;
use std::time::Duration;
use bytes::Bytes;
use http_body::Frame;
use http_body_util::{Either, Full, StreamBody};
use hyper::service::Service;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use hyper_util::rt::TokioExecutor;
use tokio::net::TcpStream;
use tokio_stream::wrappers::ReceiverStream;
use std::future::Future;
use std::pin::Pin;
use crate::global_state::GlobalState;
use crate::CustomError;
use hyper::{Method, StatusCode};

use super::{ProcMessage, ReverseProxyService, WrappedNormalResponse};
use super::proxy;


pub enum SomeIo {
    Https(hyper_util::rt::TokioIo<tokio_rustls::server::TlsStream<TcpStream>>),
    Http(hyper_util::rt::TokioIo<TcpStream>)

}

pub (crate) async fn serve(service:ReverseProxyService,io:SomeIo) {

    let result = match io {
        SomeIo::Https(tls_stream) => {
            hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(tls_stream, service).await
        },
        SomeIo::Http(tcp_stream) => {
            
            hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
               .serve_connection_with_upgrades(tcp_stream, service).await
        }
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

pub(crate) type EpicResponse = hyper::Response<EpicBody>;


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
        let f = handle(
            self.remote_addr.expect("there must always be a client"),
            req,
            self.tx.clone(),
            self.state.clone(),
            self.is_https_only
        );
        
        return Box::pin(async move {
            f.await
        })
    

    }
}

#[allow(dead_code)]
async fn handle(
    client_ip: std::net::SocketAddr, 
    req: Request<hyper::body::Incoming>,
    tx: Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: GlobalState,
    is_https:bool
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
    
    let guarded = state.1.read().await;

    let processes = guarded.hosted_process.clone().unwrap_or_default();
    if let Some(target_cfg) = processes.iter().find(|p| {
            req_host_name == p.host_name
            || p.capture_subdomains.unwrap_or_default() && req_host_name.ends_with(&format!(".{}",p.host_name))
    }) {

        let current_target_status : Option<crate::ProcState> = {
            let guard = state.0.read().await;
            let info = guard.site_states_map.iter().find(|x|x.0==&target_cfg.host_name);
            match info {
                Some((_,target_state)) => Some(target_state.clone()),
                None => None,
            }
        };

        // auto start site in case its been disabled by other requests
        _ = tx.send(super::ProcMessage::Start(target_cfg.host_name.to_owned())).map_err(|e|format!("{e:?}"));

        
        if let Some(cts) = current_target_status {
            if cts == crate::ProcState::Stopped || cts == crate::ProcState::Starting {
                match req.method() {
                    &Method::GET => {
                        return Ok(EpicResponse::new(create_epic_string_full_body(&please_wait_response())))
                    }  
                    _ => {
                        // we do this to give services some time to wake up instead of failing requests while cold-starting sites
                        tokio::time::sleep(Duration::from_secs(3)).await
                    }           
                }                 
            }
        }
        
        // auto start site in case its been disabled by other requests
        _ = tx.send(super::ProcMessage::Start(target_cfg.host_name.to_owned())).map_err(|e|format!("{e:?}"));
        let enforce_https = target_cfg.https.is_some_and(|x|x);
        let scheme = if enforce_https { "https" } else { "http" };

        let mut original_path_and_query = req.uri().path_and_query()
            .and_then(|x| Some(x.as_str())).unwrap_or_default();
        if original_path_and_query == "/" { original_path_and_query = ""}

        let default_port = if enforce_https { 443 } else { 80 };

        let resolved_host_name = {
            let forward_subdomains = target_cfg.forward_subdomains.unwrap_or_default();
            if forward_subdomains {
                if let Some(subdomain) = get_subdomain(&req_host_name, &target_cfg.host_name) {
                    tracing::debug!("in-proc forward terminating proxy rewrote subdomain: {subdomain}!");
                    format!("{subdomain}.{}", &target_cfg.host_name)
                } else {
                    target_cfg.host_name.clone()
                }
            } else {
                target_cfg.host_name.clone()
            }
        };
        


        // TODO - also support this mode for tcp tunnelling and remote sites ?
        // need to be opt-in so that we either
        // direct *.blah.com -> mysite.com
        // or *.blah.com -> *.mysite.com (would obviously not work if target is ip?)

        tracing::info!("USING THIS RESOLVED TARGET: {resolved_host_name}");
        let target_url = format!("{scheme}://{}:{}{}",
            resolved_host_name,
            target_cfg.port.unwrap_or(default_port),
            original_path_and_query
        );
    

        let target = crate::http_proxy::Target::Proc(target_cfg.clone());
        
        let result = 
            proxy(&req_host_name,is_https,state.clone(),req,&target_url,target,client_ip).await;

        map_result(&req_host_name,result).await
    }

    else {
        
        let config = &state.1.read().await.0;
        if let Some(remote_target_cfg) = config.remote_target.clone().unwrap_or_default().iter().find(|p|{
            //tracing::info!("comparing incoming req: {} vs {} ",req_host_name,p.host_name);
            req_host_name == p.host_name
            || p.capture_subdomains.unwrap_or_default() && req_host_name.ends_with(&format!(".{}",p.host_name))
        }) {

            return perform_remote_forwarding(req_host_name,is_https,state.clone(),client_ip,remote_target_cfg,req).await
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
    is_https:bool,
    state: GlobalState,
    client_ip:std::net::SocketAddr,
    remote_target_config:&crate::configuration::v1::RemoteSiteConfig,
    req:hyper::Request<IncomingBody>
) -> Result<EpicResponse,CustomError> {
    
    
    // if a target is marked with http, we wont try to use http
    let enforce_https = remote_target_config.https.is_some_and(|x|x);
   
    let scheme = if enforce_https { "https" } else { "http" }; 

    let mut original_path_and_query = req.uri().path_and_query()
        .and_then(|x| Some(x.as_str())).unwrap_or_default();
    if original_path_and_query == "/" { original_path_and_query = ""}

    let default_port = if enforce_https { 443 } else { 80 };

    
    let resolved_host_name = {

        let forward_subdomains = remote_target_config.forward_subdomains.unwrap_or_default();
        let subdomain = get_subdomain(&req_host_name, &remote_target_config.host_name);
        
        if forward_subdomains && subdomain.is_some() {
            let subdomain = subdomain.unwrap(); 
            tracing::debug!("remote forward terminating proxy rewrote subdomain: {subdomain}!");
            format!("{subdomain}.{}", &remote_target_config.target_hostname)
        } else {
            remote_target_config.target_hostname.clone()
        }
    };
        

        
    let target_url = format!("{scheme}://{}:{}{}",
        resolved_host_name,
        remote_target_config.port.unwrap_or(default_port),
        original_path_and_query
    );

    tracing::info!("Incoming request to '{}' for remote proxy target {target_url}",remote_target_config.host_name);
    let result = 
        proxy(
            &req_host_name,
            is_https,
            state.clone(),
            req,
            &target_url,
            crate::http_proxy::Target::Remote(remote_target_config.clone()),
            client_ip
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
            tracing::info!("HyperLegacyError - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("HyperLegacyError - {error:?}")))
                .expect("body building always works"))
        },
        Err(crate::http_proxy::ProxyError::HyperError(error)) => {
            tracing::info!("HyperError - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("HyperError - {error:?}")))
                .expect("body building always works"))
        },
        Err(crate::http_proxy::ProxyError::OddBoxError(error)) => {
            tracing::info!("OddBoxError - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("ODD-BOX-ERROR: {error:?}")))
                .expect("body building always works"))
        },
        Err(crate::http_proxy::ProxyError::ForwardHeaderError) => {
            tracing::info!("ForwardHeaderError - Failed to call {}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body("ForwardHeaderError"))
                .expect("body building always works"))
        },
        Err(crate::http_proxy::ProxyError::InvalidUri(error)) => {
            tracing::info!("InvalidUri - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("InvalidUri: {error:?}")))
                .expect("body building always works"))
        }, 
        Err(crate::http_proxy::ProxyError::UpgradeError(error)) => {
            tracing::info!("UpgradeError - Failed to call {}: {error:?}", &target_url);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(create_epic_string_full_body(&format!("UpgradeError: {error:?}")))
                .expect("body building always works"))
        }
    }
}


async fn intercept_local_commands(
    req_host_name:&str,
    params:&std::collections::HashMap<String, String>,
    req_path:&str,
    tx:Arc<tokio::sync::broadcast::Sender<ProcMessage>>
) -> Option<EpicResponse> {
    
    if (req_host_name == "127.0.0.1"||req_host_name == "localhost") && req_path.eq("/STOP") {
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
                <h2>All sites stopped by your command</h2>
                
                <form action="/START">
                    <input type="submit" value="Resume" />
                </form>            

                <p>The proxy will also resume if you visit any of the sites</p>
            </center>
        "#;
        return Some(EpicResponse::new(create_epic_string_full_body(html)))
    } 
    
    if (req_host_name == "127.0.0.1"||req_host_name == "localhost") && req_path.eq("/START") {
        

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
                <h2>All sites resumed</h2>
                
                <form action="/STOP">
                    <input type="submit" value="Stop" />
                </form>            

            </center>
        "#;
        return Some(EpicResponse::new(create_epic_string_full_body(html)))
    }

    None
}

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






