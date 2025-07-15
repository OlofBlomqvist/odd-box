use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use axum::http::{HeaderName, HeaderValue};
use bytes::Bytes;
use http_body::Frame;
use http_body_util::{Either, Full, StreamBody};
use hyper::service::Service;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tokio_stream::wrappers::ReceiverStream;
use std::future::Future;
use std::pin::Pin;
use crate::global_state::GlobalState;
use crate::tcp_proxy::{GenericManagedStream, ReverseTcpProxyTarget};
use crate::types::app_state::ProcState;
use crate::types::proxy_state::ConnectionKey;
use crate::CustomError;
use hyper::{HeaderMap, Method, StatusCode};
use lazy_static::lazy_static;
use super::{ProcMessage, ReverseProxyService, WrappedNormalResponse};
use super::proxy;



lazy_static! {
    static ref SERVER_ONE: hyper_util::server::conn::auto::Builder<TokioExecutor> = 
        hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
}
pub async fn serve(service:ReverseProxyService,io:GenericManagedStream) {
    
    let result = match io {
        // GenericManagedStream::TLS(peekable_tls_stream) => 
        //     SERVER_ONE.serve_connection_with_upgrades(hyper_util::rt::TokioIo::new(peekable_tls_stream.managed_tls_stream), service).await,
        GenericManagedStream::TerminatedTLS(stream) => 
            SERVER_ONE.serve_connection_with_upgrades(hyper_util::rt::TokioIo::new(stream), service).await,
        GenericManagedStream::TCP(peekable_tcp_stream) => 
            SERVER_ONE.serve_connection_with_upgrades(hyper_util::rt::TokioIo::new(peekable_tcp_stream), service).await,

            
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

pub struct OddResponse(EpicResponse);
impl OddResponse {
    pub fn new(body:EpicBody) -> Self {
        Self(EpicResponse::new(body))
    }
    pub fn empty() -> Self {
        Self(EpicResponse::new(Either::Right(FullOrStreamBody::Left(Full::new(Bytes::new())))))
    }
}
impl From<EpicResponse> for OddResponse {
    fn from(r:EpicResponse) -> Self {
        OddResponse(r)
    }
}
impl From<OddResponse> for EpicResponse {
    fn from(r:OddResponse) -> Self {
        r.0
    }
}
impl OddResponse {
    pub fn with_status(self, status:StatusCode) -> Self {
        let (parts,body) = self.0.into_parts();
        let mut response = Response::from_parts(parts,body);
        *response.status_mut() = status;
        Self(response)
    }
    pub fn with_headers(self, headers:Vec<(&str,&str)>) -> Self {
        let (parts,body) = self.0.into_parts();
        let mut response = Response::from_parts(parts,body);
        let headers = HeaderMap::from_iter(headers.iter().filter_map(|(k,v)| {
            match (HeaderName::from_str(k), HeaderValue::from_str(v)) {
                (Ok(k), Ok(v)) => Some((k,v)),
                _ => { None }
            }
        }));
        *response.headers_mut() = headers;
        Self(response)
    }
}


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



pub fn create_simple_response_from_bytes(res:Response<bytes::Bytes>) -> Result<EpicResponse,CustomError> {
    let (a,b) = res.into_parts();
    Ok(EpicResponse::from_parts(a,Either::Right(Either::Left(b.into()))))
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
    Ok(EpicResponse::from_parts(p,Either::Right(Either::Left(b.into()))))
}


impl<'a> Service<Request<hyper::body::Incoming>> for ReverseProxyService {
    type Response = EpicResponse;
    type Error = CustomError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    
    fn call(&self, req: hyper::Request<hyper::body::Incoming>) -> Self::Future {    
        
        // tracing::trace!("INCOMING REQ: {:?}",req);
        // tracing::trace!("VERSION: {:?}",req.version());

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

        // handle letsencrypt verification requests
        if req.uri().path().starts_with("/.well-known/acme-challenge/") {

            let host = match req.headers().get("host") {
                Some(value) => match value.to_str() {
                    Ok(host_str) => host_str.to_string(),
                    Err(e) => {
                        tracing::error!("Failed to convert host header to string: {:?}", e);
                        return Box::pin(async move { Err(CustomError(format!("{e:?}"))) });
                    }
                },
                None => {
                    tracing::error!("Host header not found");
                    return Box::pin(async move { Err(CustomError(format!("Host header not found"))) });
                }
            };

            // strip any :port from host:
            let host = host.split(":").collect::<Vec<&str>>()[0].to_string();
            let f = async move {
                let host = host;
                tracing::info!("Got incoming LE req for {host:?}");
                // http://whatever_domain/.well-known/acme-challenge/<TOKEN>
                crate::letsencrypt::DOMAIN_TO_CHALLENGE_TOKEN_MAP.get(&host)
                    .map(|x| {
                        let (_domain_name,challenge_token) = x.pair();
                        tracing::info!("Got incoming LE req and found challenge for {host} : {challenge_token:?}");
                        if let Some(response) = crate::letsencrypt::CHALLENGE_MAP.get(challenge_token) {
                            let (_,response) = response.pair();
                            tracing::info!("Responding with challenge response: {response:?}");
                            Ok(EpicResponse::new(create_epic_string_full_body(response)))
                        } else {
                            tracing::info!("Responding no response... because no challenge found for {host}");
                            Ok(EpicResponse::new(create_epic_string_full_body("nothing here 1")))
                        } 
                    })
                    .unwrap_or_else(|| {
                        tracing::warn!("Got incoming LE req but no challenge found for {host:?}");
                        Ok(EpicResponse::new(create_epic_string_full_body("nothing here 2")))
                    })
            };
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
            self.h2_client.clone(),
            self.resolved_target.clone(),
            self.configuration.clone(),
            self.connection_key.clone(),
            self.host_header.clone(),
            self.sni.clone()
        );
        
        return Box::pin(async move {
            match f.await {
                Ok(x) => {
                    //x.headers_mut().insert("odd-box", HeaderValue::from_static("YEAH BABY YEAH"));
                    Ok(x)
                },
                Err(e) => {
                    //Err(CustomError(format!("{e:?}")))
                    let body = create_epic_string_full_body(&format!("{e:?}"));
                    let mut response = EpicResponse::new(body);
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    Ok(response)
                },
            }
        })
    

    }
}

async fn handle_http_request(
    client_ip: std::net::SocketAddr, 
    req: Request<hyper::body::Incoming>,
    tx: Arc<tokio::sync::broadcast::Sender<ProcMessage>>,
    state: Arc<GlobalState>,
    is_https:bool,
    client:  Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
    h2_client: Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
    peeked_target: Option<Arc<ReverseTcpProxyTarget>>,
    configuration: Arc<crate::configuration::ConfigWrapper>,
    connection_key: ConnectionKey,
    host_header: Option<String>,
    sni: Option<String>

) -> Result<EpicResponse, CustomError> {
    
    // This will either point to the backend hostname or whatever the caller used as hostname
    let resolved_host_name = 
        if let Some(t) = &peeked_target {
            t.host_name.to_string()
        } else if let Some(hh) = req.headers().get("host") { 
            let hostname_and_port = hh.to_str().map_err(|e|CustomError(format!("{e:?}")))?.to_string();
            hostname_and_port.split(":").collect::<Vec<&str>>()[0].to_owned()
        } else { 
            req.uri().authority().ok_or(CustomError(format!("No hostname and no Authority found")))?.host().to_string()
        };

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

    if peeked_target.is_none() && !is_https { 

            
        if let Some(r) = intercept_local_commands(&resolved_host_name,&params,req_path,tx.clone()).await {
            return Ok(r)
        }
        
        // if no target was found and the request is for the odd-box ui, it means we have gotten a request to the odd-box ui
        // over a non-encrypted channel. if so we will just redirect to https.
        let odd_box_url = configuration.odd_box_url.clone().unwrap_or("!".to_string());
        if resolved_host_name == odd_box_url || resolved_host_name == "localhost" || resolved_host_name == "oddbox.localhost" || resolved_host_name == "odd-box.localhost"  {
            let p = configuration.tls_port.unwrap_or(4343);
            return Ok(
                OddResponse::empty()
                    .with_headers(vec![
                        ("Location", &format!("https://{}:{p}",resolved_host_name))
                    ])
                    .with_status(StatusCode::TEMPORARY_REDIRECT)
                    .into()
            ); 
        }
    }
    
    //tracing::trace!("Handling request from {client_ip:?} on hostname {req_host_name:?}");
    
    // THIS SHOULD BE THE ONLY PLACE WE INCREMENT THE TERMINATION COUNTER
    match state.app_state.statistics.lb_access_count_per_hostname.get_mut(&resolved_host_name) {
        Some(mut guard) => {
            let (_k,v) = guard.pair_mut();
            1 + v.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        },
        None => {
            state.app_state.statistics.lb_access_count_per_hostname.insert(resolved_host_name.clone(), std::sync::atomic::AtomicUsize::new(1));
            1
        }
    };
    


    if let Some(r) = intercept_local_commands(&resolved_host_name,&params,req_path,tx.clone()).await {
        return Ok(r)
    }
    
    // todo - cache this?
    let dir_target = if peeked_target.is_some() { None } else { configuration.dir_server.iter().flatten().find_map(|x| {
        if x.host_name == resolved_host_name {
            match configuration.resolve_dir_server_configuration(x) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::warn!("Failed to resolve dir server configuration: {e:?}");
                    None
                },
            }
        } else {
            None
        }
    })};

    if let Some(t) = dir_target {
        if let Some(true) = t.redirect_to_https {
            if !is_https {
                let mut rr = EpicResponse::new(create_epic_string_full_body(&format!("Please use https://{resolved_host_name} instead of http://{resolved_host_name}")));
                *rr.status_mut() = StatusCode::PERMANENT_REDIRECT;
                rr.headers_mut().insert("Location", hyper::header::HeaderValue::from_str(&format!("https://{resolved_host_name}:{}{}",configuration.tls_port.unwrap_or(4343),req.uri())).unwrap());
                return Ok(rr)
            }
        }
        match crate::custom_servers::directory::handle(t.clone(),req).await {
            Ok(r) => return Ok(r),
            Err(e) => {
                tracing::error!("Failed to handle file request: {e:?}");
                let mut rr = EpicResponse::new(create_epic_string_full_body(&e.0));
                *rr.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                return Ok(rr)
            },
        }
    }

    if peeked_target.is_none() && dir_target.is_none() {
        let fresh_container_info = { state.config.read().await.docker_containers.clone()} ;
        if let Some(rc) = fresh_container_info.get(&resolved_host_name).and_then(|x|Some(x.generate_remote_config())) {
            return perform_remote_forwarding(
                resolved_host_name,is_https,
                state.clone(),
                client_ip,
                &rc,
                req,client.clone(),
                h2_client.clone(),
                connection_key,
                host_header.clone(),
                sni.clone()
            ).await
        }        
    }

    let found_hosted_target = 
        if let Some(p) = peeked_target.as_ref().and_then(|x| x.hosted_target_config.clone()) {
            //tracing::trace!("Found peeked target for {req_host_name}");
            Some(p)
        } else {
            if let Some(processes) = &configuration.hosted_process {
                if let Some(pp) = processes.iter().find(|p| {
                    resolved_host_name == p.host_name
                    || p.capture_subdomains.unwrap_or_default() 
                    && resolved_host_name.ends_with(&format!(".{}",p.host_name))
                }) {
                    //tracing::trace!("Found hosted target for {req_host_name}");
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
                        if let Some(ua) = req.headers().get("user-agent") {
                            let hv = ua.to_str().unwrap_or_default().to_uppercase() ;
                            // we only want to risk showing the please wait page to browsers..
                            // not perfect but should be good enough for now..?
                            // todo: opt in/out thru config ?
                            if hv.contains("MOZILLA") || hv.contains("SAFARI") || hv.contains("CHROME") || hv.contains("EDGE") {
                                return Ok(EpicResponse::new(create_epic_string_full_body(&please_wait_response())))
                            }
                            // todo - we dont seem to set the content type here..?
                            
                        } else {
                            tracing::trace!("waiting for site to start.. 5 seconds..");
                            tokio::time::sleep(Duration::from_secs(5)).await
                        }
                    }  
                    _ => {
                        // we do this to give services some time to wake up instead of failing requests while cold-starting sites
                        tracing::trace!("waiting for site to start.. 5 seconds...");
                        tokio::time::sleep(Duration::from_secs(5)).await
                    }           
                }                 
            }
        }

        let port;
        let mut wait_count = 0;
        loop {
            if wait_count > 10 {
                return Err(CustomError(format!("No active port found for {resolved_host_name}.")))
            } else {
                // let id: &crate::types::proc_info::ProcId = target_proc_cfg.get_id();
                // let name = target_proc_cfg.host_name.clone();
                //tracing::trace!("Attempting to find active port found for {req_host_name}... id: {id:?} - name: {name:?}");
            }
            if let Some(info) = crate::PROC_THREAD_MAP.get(target_proc_cfg.get_id()) {
                if info.pid.is_some() {
                    if let Some(p) = info.config.active_port {
                        port = p;
                        break;
                    } else {
                        tracing::warn!("No active port found for {resolved_host_name}.");
                    }
                };
            } else {
                return Err(CustomError(format!("No site info found! (proc id missing: {:?})",target_proc_cfg.get_id())))
            };
            wait_count += 1;
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }


        let enforce_https = target_proc_cfg.https.is_some_and(|x|x);
        let scheme = if enforce_https { "https" } else { "http" };

        let mut original_path_and_query = req.uri().path_and_query()
            .and_then(|x| Some(x.as_str())).unwrap_or_default();
        if original_path_and_query == "/" { original_path_and_query = ""}


        // todo: we dont really handle subdomain passing here anymore
        let (parsed_host_name,subdomain) = {
            let forward_subdomains = target_proc_cfg.forward_subdomains.unwrap_or_default();
            let sub = get_subdomain(&resolved_host_name, &target_proc_cfg.host_name);
            if forward_subdomains {
                if let Some(subdomain) = sub {
                    (Cow::Owned(format!("{subdomain}.{}", &target_proc_cfg.host_name)),Some(subdomain))
                } else {
                    (Cow::Borrowed(&target_proc_cfg.host_name),sub)
                }
            } else {
                (Cow::Borrowed(&target_proc_cfg.host_name),sub)
            }
        };

        let local_addr = if configuration.use_loopback_ip_for_procs.unwrap_or_default() {
            "127.0.0.1" // explicitly use ipv4 loopback for processes
        } else {
            "localhost" // ipv6 enabled systems resolve this to ::1 and otherwise 127.0.0.1
        };
        
        // we add the host flag manually in proxy method, this is only to avoid dns lookup for local targets.
        // todo: opt in/out via cfg (?)
        let skip_dns_for_local_target_url = format!("{scheme}://{}:{}{}",
            local_addr,
            port,
            original_path_and_query
        );

        let target_cfg = target_proc_cfg.clone();
        let hints = target_cfg.hints.clone();
        let target = crate::http_proxy::Target::Proc(target_cfg.clone());
        
        // we do this just to increment the statistics counter
        _ = target_cfg.next_backend(&state, crate::configuration::BackendFilter::Any).await;

        let backend = crate::configuration::Backend {
            hints: hints,
            // we are hosting this service so clearly it is local
            address: local_addr.to_string(),
            port: port,
            https: Some(enforce_https)
        };

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
                enforce_https,
                backend,
                &connection_key,
                host_header,
                sni,
                subdomain
            ).await;

        map_result(&skip_dns_for_local_target_url,result).await
    }

    else {
        if let Some(peeked_remote_config) = peeked_target.and_then(|x| x.remote_target_config.clone() ) {

            return perform_remote_forwarding(
                resolved_host_name,is_https,
                state.clone(),
                client_ip,
                &peeked_remote_config,
                req,client.clone(),
                h2_client.clone(),
                connection_key,
                host_header.clone(),
                sni.clone()
            ).await
        }
        else if let Some(remote_target_cfg) = &configuration.remote_target.iter().flatten().find(|p|{
            //tracing::info!("comparing incoming req: {} vs {} ",req_host_name,p.host_name);
            resolved_host_name == p.host_name
            || p.capture_subdomains.unwrap_or_default() && resolved_host_name.ends_with(&format!(".{}",p.host_name))
        }) {
            return perform_remote_forwarding(
                resolved_host_name,is_https,
                state.clone(),
                client_ip,
                remote_target_cfg,
                req,client.clone(),
                h2_client.clone(),
                connection_key,
                host_header.clone(),
                sni.clone()
            ).await
        };

        tracing::warn!("Received request that does not match any known target: {:?}", resolved_host_name);
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

#[test]
fn subdomain_handling_works() {
    let example = "example1.123091238123-3333.tower.cruma.io";
    let backend = "tower.cruma.io";
    let subdomain = get_subdomain(example, backend);
    println!("Subdomain: {:?}", subdomain);
    assert_eq!(subdomain, Some("example1.123091238123-3333".to_string()));
}

async fn perform_remote_forwarding(
    req_host_name:String,
    is_https:bool,
    state: Arc<GlobalState>,
    client_ip:std::net::SocketAddr,
    remote_target_config:&crate::configuration::RemoteSiteConfig,
    req:hyper::Request<IncomingBody>,
    client:  Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
    h2_client: Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
    connection_key: ConnectionKey,
    host_header: Option<String>,
    sni: Option<String>
) -> Result<EpicResponse,CustomError> {
    
    
    let mut original_path_and_query = req.uri().path_and_query()
        .and_then(|x| Some(x.as_str())).unwrap_or_default();
    if original_path_and_query == "/" { original_path_and_query = ""}
   
    let next_backend_target = if let Some(b) = remote_target_config.next_backend(&state, crate::configuration::BackendFilter::Any).await {
        b
    } else {
        return Err(CustomError("No backend found".to_string()))
    };
    
    // if a target is marked with http, we wont try to use http
    let enforce_https = next_backend_target.https.unwrap_or_default();
   
    let scheme = if enforce_https { "https" } else { "http" }; 
    
    // todo - we dont handle subdomain passing correctly here
    let (resolved_host_name,subdomain_part) = {
        let sub = get_subdomain(&req_host_name, &remote_target_config.host_name);
        if remote_target_config.forward_subdomains.unwrap_or_default() {
            if let Some(subdomain) = sub {
            //tracing::debug!("remote forward terminating proxy rewrote subdomain: {subdomain}!");
                (format!("{subdomain}.{}", &next_backend_target.address),Some(subdomain))
            } else {
                (next_backend_target.address.clone(),sub)
            }
        } else {
            (next_backend_target.address.clone(),sub)
        }
    };
        
    let target_url = format!("{scheme}://{}:{}{}",
        resolved_host_name,
        next_backend_target.port,
        original_path_and_query
    );

    //map_result(&target_url,Err(super::ProxyError::OddBoxError(String::from("meh")))).await

    //tracing::info!("Incoming request to '{}' for remote proxy target {target_url}",next_backend_target.address);
    let result = 
        proxy( 
            &req_host_name,
            is_https,
            state.clone(),
            req,
            &target_url,
            crate::http_proxy::Target::Remote(remote_target_config.clone()),
            client_ip,
            client,
            h2_client,
            next_backend_target.https.unwrap_or_default(),
            next_backend_target,
            &connection_key,
            host_header.clone(),
            sni.clone(),
            subdomain_part
        ).await;

    map_result(&target_url,result).await

}

async fn map_result(target_url:&str,result:Result<crate::http_proxy::ProxyCallResult,crate::http_proxy::ProxyError>) -> Result<EpicResponse,CustomError> {
    
    match result {
        // Ok(crate::http_proxy::ProxyCallResult::Bytes(response)) => {
        //     return create_simple_response_from_bytes(response);
        // }
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
pub async fn intercept_local_commands(
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
                <h2>Stop signal received!</h2>
                
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
            <meta charset="UTF-8">
            <meta http-equiv="refresh" content="5;">
            <title>please wait...</title>
            <style>
                body {
                    margin: 0;
                    padding: 0;
                    display: flex;
                    flex-direction: column;
                    justify-content: center;
                    align-items: center;
                    height: 100vh;
                    background-color: #111;
                    color: white;
                    font-family: Arial, sans-serif;
                    text-align: center;
                }

                .loading-text {
                    font-size: 34px;
                    margin-bottom: 20px;
                }

                .subtitle {
                    font-size: 16px;
                    margin-bottom: 40px;
                    color: #cccccc;
                }

                .spinner {
                    border: 4px solid rgba(255, 255, 255, 0.3);
                    border-radius: 50%;
                    border-top: 4px solid white;
                    width: 40px;
                    height: 40px;
                    animation: spin 1s linear infinite;
                }

                @keyframes spin {
                    0% { transform: rotate(0deg); }
                    100% { transform: rotate(360deg); }
                }
            </style>
        </head>
        <body>
            <div class="loading-text">Initializing sub-systems, please wait..</div>
            <div class="subtitle">The backend server responsible for handling this request is not awake, we are attempting to wake it up now!</div>
            <div class="spinner"></div>
        </body>
        </html>
    "#.to_string()
}
