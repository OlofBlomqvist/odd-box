use chrono::Local;
use futures_util::FutureExt;
use http_body::Frame;
use http_body_util::BodyExt;
use hyper::{
    body::Incoming, header::{HeaderName, HeaderValue, InvalidHeaderValue, ToStrError}, upgrade::OnUpgrade, HeaderMap, Request, Response, StatusCode, Version
};
use hyper_rustls::HttpsConnector;
use hyper_util::{client::legacy::{connect::HttpConnector, Client}, rt::TokioIo};
use std::{net::SocketAddr, sync::Arc, task::Poll, time::Duration};
use tungstenite::http;

use lazy_static::lazy_static;

use crate::{
    configuration::v2::Hint, global_state::GlobalState, http_proxy::EpicResponse, types::proxy_state::{ ConnectionKey, ProxyActiveConnection, ProxyActiveConnectionType }, CustomError
};
lazy_static! {
    static ref TE_HEADER: HeaderName = HeaderName::from_static("te");
    static ref CONNECTION_HEADER: HeaderName = HeaderName::from_static("connection");
    static ref UPGRADE_HEADER: HeaderName = HeaderName::from_static("upgrade");
    static ref TRAILER_HEADER: HeaderName = HeaderName::from_static("trailer");
    static ref TRAILERS_HEADER: HeaderName = HeaderName::from_static("trailers");
    static ref HOP_HEADERS: [HeaderName; 9] = [
        CONNECTION_HEADER.clone(),
        TE_HEADER.clone(),
        TRAILER_HEADER.clone(),
        HeaderName::from_static("keep-alive"),
        HeaderName::from_static("proxy-connection"),
        HeaderName::from_static("proxy-authenticate"),
        HeaderName::from_static("proxy-authorization"),
        HeaderName::from_static("transfer-encoding"),
        HeaderName::from_static("upgrade"),
    ];

    static ref X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");
}

pub enum ProxyCallResult {
    NormalResponse(WrappedNormalResponse),
    EpicResponse(crate::http_proxy::service::EpicResponse),
}

#[derive(Debug)]

pub enum ProxyError {
    InvalidUri(http::uri::InvalidUri),
    ForwardHeaderError,
    UpgradeError(String),
    HyperError(hyper::Error),
    LegacyError(hyper_util::client::legacy::Error),
    OddBoxError(String),
}

#[derive(Debug)]
pub enum Target {
    Remote(crate::configuration::v2::RemoteSiteConfig),
    Proc(crate::configuration::v2::InProcessSiteConfig),
}

// We don't care about the original call scheme, version, etc.
// The target_url is the full URL to the target, including the scheme, it is expected that 
// our caller has already determined if the target is http or https depending on whatever backend was selected.
// The job of this method is simply to create a new request with the target url and the original request's headers.
// while also selecting http version and handling upgraded connections.  
// TODO: simplify the signature, we dont need it to be this complicated..
pub async fn proxy(
    req_host_name: &str,
    original_connection_is_https:bool,
    state: Arc<GlobalState>,
    mut req: hyper::Request<hyper::body::Incoming>,
    target_url: &str,
    target: Target,
    client_ip: SocketAddr,
    client:  Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
    h2_only_client: Client<HttpsConnector<HttpConnector>, hyper::body::Incoming>,
    _fallback_url: &str,
    use_https_to_backend_target: bool,
    backend: crate::configuration::v2::Backend
) -> Result<ProxyCallResult, ProxyError> {

    
    let incoming_http_version = req.version();
    let request_upgrade_type = get_upgrade_type(req.headers());
    let request_upgraded = req.extensions_mut().remove::<OnUpgrade>();



    tracing::trace!(
        "Incoming {incoming_http_version:?} request to terminating proxy from {client_ip:?} with target url: {target_url}!"
    );
    
    
    let mut backend_supports_prior_knowledge_http2_over_tls = false;
    let mut backend_supports_http2_over_clear_text_via_h2c_upgrade_header = false;
    let mut _backend_supports_http2_h2c_using_prior_knowledge = false;
    let mut use_prior_knowledge_http2 = false;
    let mut use_h2c_upgrade_header = false;
    let mut backend_might_support_h2 = true;
    
    for x in &backend.hints.iter().flatten().collect::<Vec<&Hint>>() {
        match x {
            Hint::H2 => {
                backend_supports_prior_knowledge_http2_over_tls = true;
            },
            Hint::H2C => {
                backend_supports_http2_over_clear_text_via_h2c_upgrade_header = true;
            },
            Hint::H2CPK => {
                _backend_supports_http2_h2c_using_prior_knowledge = true;
            },
            Hint::NOH2 => {
                backend_might_support_h2 = false
            }
        }
    }
    
    // Handle upgrade headers
    if let Some(typ) = &request_upgrade_type {
        if typ.to_uppercase()=="H2C" {
            // if backend_supports_http2_over_clear_text_via_h2c_upgrade_header {
            //     tracing::trace!("Client used h2c header and backend supports h2c upgrades, this should be fine!")
            // } else {
            //     tracing::trace!("Client used {typ:?} header. The backend has no hint that it supports h2c but we will attempt to upgrade anyway.");
            // }
            use_h2c_upgrade_header = true;
        } else {
            //tracing::trace!("Client requested upgrade to {typ:?}. We don't know if the backend supports it, but we will try anyway.");
            // note: wont be websocket here as that is handled in another route
        }
    }

    
    let mut proxied_request =
        create_proxied_request(&target_url, req, request_upgrade_type.as_ref(), &req_host_name)?;

    
    if proxied_request.version() == Version::HTTP_2 {
        // if client connected to us with http2, we will attempt to do so with the backend as well..
        // todo: not sure this is what we want to do but this is how the old code worked and i dont want to change it right now.
        use_prior_knowledge_http2 = true;
    } else if backend_supports_prior_knowledge_http2_over_tls && use_https_to_backend_target {
        use_prior_knowledge_http2 = true;
    } else if backend_supports_http2_over_clear_text_via_h2c_upgrade_header && !use_https_to_backend_target {
        use_prior_knowledge_http2 = true;
    } else if backend_supports_http2_over_clear_text_via_h2c_upgrade_header && !use_https_to_backend_target {
        if use_h2c_upgrade_header {
            use_prior_knowledge_http2 = false;
        } else {
            tracing::warn!("Backend supports h2c but client did not request it. Falling back to http1.1.");
        }
    }

    // FFR:
    // ---------------------------------------------------------------------------------------------
    // H2 THRU ALPN -- SUPPORTS HTTP2 OVER TLS
    // H2 PRIOR KNOWLEDGE -- SUPPORTS HTTP2 OVER TLS
    // H2C PRIOR KNOWLEDGE -- SUPPORTS HTTP2 OVER CLEAR TEXT
    // H2C UPGRADE HEADER -- SUPPORTS HTTP2 OVER CLEAR TEXT VIA UPGRADE HEADER
    // if backend does not support http2, we should just use http1.1 and act like nothing happened.
    // ---------------------------------------------------------------------------------------------
    

    let client = if backend_might_support_h2 && use_prior_knowledge_http2 {
        *proxied_request.version_mut() = Version::HTTP_2;
        &h2_only_client // this requires the backend to support h2 prior knowledge or h2 selection by alpn 
    } else {
        *proxied_request.version_mut() = Version::HTTP_11;
        &client // this will use the default http1 client, which will upgrade to h2 if the backend supports it thru upgrade header or alpn
    };

    
    let req_is_https = proxied_request.uri().scheme().is_some_and(|x|*x==http::uri::Scheme::HTTPS);
    let target_scheme_info_str = if use_https_to_backend_target != req_is_https {
        tracing::warn!("Target URL scheme does not match use_https_to_backend_target setting. This is a bug in odd-box, please report it. Will fallback to using the target URL scheme ({}).",target_url);
        if req_is_https {
            "https"
        } else {
            "http"
        }
    } else if use_https_to_backend_target {
        "https" 
    } else {
        "http"
    };

    
    let con: ProxyActiveConnection = create_connection(
        &proxied_request, 
        incoming_http_version,
        target, 
        &client_ip, 
        target_scheme_info_str, 
        proxied_request.version(), 
        &target_url, 
        original_connection_is_https,
        req_host_name.to_string()
    );


    tracing::trace!("Sending request:\n{:?}", proxied_request);


    // todo - prevent making a connection if client already has too many tcp connections open
    let mut response = {
        client
            .request(proxied_request)
            .await
            .map_err(ProxyError::LegacyError)?
    };

    tracing::trace!(
        "GOT THIS RESPONSE FROM REQ TO '{target_url}' : {:?}",response
    );
    
    // if the backend agreed to upgrade to some other protocol, we will create a bidirectional tunnel for the client and backend to communicate directly.
    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let response_upgrade_type = get_upgrade_type(response.headers());
        tracing::trace!("RESPONSE IS TO UPGRADE TO : {response_upgrade_type:?}.");
        if request_upgrade_type == response_upgrade_type {
            if let Some(request_upgraded) = request_upgraded {

                let mut response_upgraded = TokioIo::new(
                    response
                        .extensions_mut()
                        .remove::<OnUpgrade>()
                        .expect("response does not have an upgrade extension")
                        .await?,
                );

                tokio::spawn(async move {

                    let upgraded = match request_upgraded.await {
                        Err(e) => {
                            tracing::trace!("failed to upgrade req: {e:?}");
                            
                            return;
                        }
                        Ok(v) => v
                    };

                    let mut request_upgraded =
                        TokioIo::new(upgraded);

                    tracing::debug!("Starting bidirectional stream copy for upgraded request.");

                    match tokio::io::copy_bidirectional(&mut response_upgraded, &mut request_upgraded)
                        .await {
                            Ok(_) => {},
                            Err(e) => {
                                tracing::warn!("coping between upgraded connections failed: {e:?}")
                            }
                        }

                    tracing::debug!("Upgraded stream finished");
                });

                            

                let response = super::create_simple_response_from_incoming(
                        WrappedNormalResponse::new(response,state.clone(),con)
                    )
                    .await.map_err(|e|ProxyError::OddBoxError(format!("{e:?}")))?;

                Ok(ProxyCallResult::EpicResponse(response))
            } else {
                Err(ProxyError::UpgradeError(
                    "request does not have an upgrade extension".to_string(),
                ))
            }
        } else {
            Err(ProxyError::UpgradeError(format!(
                "backend tried to switch to protocol {:?} when {:?} was requested",
                response_upgrade_type, request_upgrade_type
            )))
        }
    } else {
        // Got a normal response from the backend, we will just forward it to the client!       
        let proxied_response = create_proxied_response(response);
        Ok(ProxyCallResult::NormalResponse(WrappedNormalResponse::new(proxied_response,state.clone(),con)))
    }
}




pub struct  WrappedNormalResponseBody {
    b : Incoming,
    on_drop : Option<Box<dyn FnOnce() + Send + 'static>>,
}
impl Drop for WrappedNormalResponseBody {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop.take() {
            //tracing::trace!("dropping active connection due to body drop");
            on_drop();
        }   
        
    }
}
pub struct WrappedNormalResponse {
    a : http::response::Parts,
    b : WrappedNormalResponseBody,
}
impl WrappedNormalResponse {
    pub fn into_parts(self) -> (http::response::Parts,WrappedNormalResponseBody) {
        (self.a,self.b)
    }

    
    pub fn new(res:Response<Incoming>,state: Arc<GlobalState>,con: ProxyActiveConnection) -> Self {
        tracing::trace!("Adding connection for this WrappedNormalResponse.");
        let con_key = add_connection(state.clone(), con);
        let drop_state = state.clone();        
        
        let on_drop: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
            let state = drop_state.clone();
            let con_key = con_key.clone();
            tokio::spawn(async move {
                //tracing::trace!("Dropping connection for this WrappedNormalResponse (with 1s delay for visibility in ui).");
                tokio::time::sleep(Duration::from_secs(1)).await;
                del_connection(state, &con_key);
            });

        });

        let (a,b) = res.into_parts();
        Self {
            a, b: WrappedNormalResponseBody { b,on_drop: Some(on_drop) }
        }
    }
}

impl hyper::body::Body for WrappedNormalResponseBody {
    type Data = bytes::Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.b.frame().poll_unpin(cx) {
            Poll::Ready(Some(Ok(data))) => Poll::Ready(Some(Ok(data))),
            Poll::Ready(Some(Err(e))) => {
                // Handle error properly here
                tracing::error!("Error while polling frame: {:?}", e);
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
fn get_upgrade_type(headers: &HeaderMap) -> Option<String> {
    // note: this is not really legal for http/1, but in reallity it is used when doing h2c upgrade from http/1 -> http/2..
    // (http1 normally would only allow in connect but we dont care here)
    let h = headers
        .get(&*UPGRADE_HEADER)
        .map(|value| value.to_str());

    if let Some(Ok(header)) = h {
        Some(header.to_owned())
    }  else {
        None
    }

}

fn map_to_err<T:core::fmt::Debug>(x:T) -> ProxyError {
    ProxyError::OddBoxError(format!("{x:?}"))
}

fn create_proxied_request<B>(
    target_url: &str,
    mut request: Request<B>,
    upgrade_type: Option<&String>,
    req_host_name: &str
) -> Result<Request<B>, ProxyError> {
    
    // replace the uri
    let target_uri = target_url.parse::<http::Uri>()
        .map_err(|e| ProxyError::InvalidUri(e))?;
    *request.uri_mut() = target_uri;
    
    
    // we want to pass the original host header to the backend (the one that the client requested)
    // and not the one we are connecting to as that might as well just be an internal name or IP.
    if let Ok(v) = HeaderValue::from_str(req_host_name) {
        _ = request.headers_mut().insert("host",v);
    } else {
        tracing::warn!("Failed to insert host header for '{req_host_name}'. Falling back to direct hostname call rather than 127.0.0.1.");
        _ = request.uri_mut().host().replace(req_host_name);
    }    

    // we will decide to use https or not to the backend ourselves, no need to forward this.
    _ = request
        .headers_mut()
        .remove("upgrade-insecure-requests");

    // add the upgrade headers back if we are upgrading, so that the backend also knows what to do.
    if let Some(value) = upgrade_type {
        tracing::trace!("Re-populate upgrade headers! :: {value}");
        let value_header = HeaderValue::from_str(value).map_err(map_to_err)?;
        let headers = request.headers_mut();
        headers.insert(&*UPGRADE_HEADER, value_header.clone());
        headers.insert(&*CONNECTION_HEADER, value_header);
    }
    Ok(request)
}


impl From<hyper_util::client::legacy::Error> for ProxyError {
    fn from(err: hyper_util::client::legacy::Error) -> ProxyError {
        ProxyError::LegacyError(err)
    }
}
impl From<hyper::Error> for ProxyError {
    fn from(err: hyper::Error) -> ProxyError {
        ProxyError::HyperError(err)
    }
}

impl From<http::uri::InvalidUri> for ProxyError {
    fn from(err: http::uri::InvalidUri) -> ProxyError {
        ProxyError::InvalidUri(err)
    }
}

impl From<ToStrError> for ProxyError {
    fn from(_err: ToStrError) -> ProxyError {
        ProxyError::ForwardHeaderError
    }
}

impl From<InvalidHeaderValue> for ProxyError {
    fn from(_err: InvalidHeaderValue) -> ProxyError {
        ProxyError::ForwardHeaderError
    }
}

fn remove_hop_headers(headers: &mut HeaderMap) {
    for header in &*HOP_HEADERS {
        headers.remove(header);
    }
}

fn create_proxied_response<B>(mut response: Response<B>) -> Response<B> {
    remove_hop_headers(response.headers_mut());
    remove_connection_headers(response.headers_mut());
    response
}

fn remove_connection_headers(headers: &mut HeaderMap) {
    if let Some(value) = headers.get(&*CONNECTION_HEADER) {
        for name in value.clone().to_str().expect("cloning headers should always work").split(',') {
            if !name.trim().is_empty() {
                headers.remove(name.trim());
            }
        }
    }
}


// ====================== HTTP2 STREAM TEST =================================================================

/// Create a response that can be sent back to the client
/// along with rx/tx channels for two way communication
#[allow(dead_code)]
pub fn create_channels_with_connected_streaming_response(
    mut req: hyper::Request<hyper::body::Incoming>,
) -> Result<
    (
        tokio::sync::mpsc::Sender<Result<Frame<bytes::Bytes>, CustomError>>,
        tokio::sync::mpsc::Receiver<Result<Frame<bytes::Bytes>, CustomError>>,
        EpicResponse,
    ),
    CustomError,
> {
    // we want a channel for receiving data FROM the client
    let (internal_tx, rx_from_client) = super::create_response_channel(4);

    // and a channel for sending data TO the client
    let (tx_to_client, internal_rx) = super::create_response_channel(4);

    // read the incoming frames from client
    tokio::spawn(async move {
        let the_body = req.body_mut();
        while let Some(x) = the_body.frame().await {
            if internal_tx
                .send(x.map_err(CustomError::from))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let epic_response: EpicResponse = super::create_stream_response(internal_rx);
    Ok((tx_to_client, rx_from_client, epic_response))
}
#[allow(dead_code)]
pub async fn h2_stream_test(
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<EpicResponse, CustomError> {
    if req.version() < Version::HTTP_2 {
        return Ok(super::EpicResponse::new(
            super::create_epic_string_full_body("Nah, just modern talking here"),
        ));
    }

    let (tx_to_client, mut rx_from_client, res) =
        create_channels_with_connected_streaming_response(req)?;

    tokio::spawn(async move {
        loop {
            if tx_to_client
                .send(Ok(Frame::data("heyyyy!".into())))
                .await
                .is_err()
            {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await
        }
    });

    tokio::spawn(async move {
        while let Some(Ok(frame)) = rx_from_client.recv().await {
            tracing::info!("msg from client: {frame:?}");
        }
    });

    Ok(res)
}









fn add_connection(state:Arc<GlobalState>,connection:ProxyActiveConnection) -> ConnectionKey {
    
    let id: u64 = crate::generate_unique_id();
    let app_state = state.app_state.clone();
    _ = app_state.statistics.active_connections.insert(id, connection);
    id
}

fn del_connection(state:Arc<GlobalState>,key:&ConnectionKey) {
    let app_state = state.app_state.clone();
    let guard = app_state.statistics.clone();
    _ = guard.active_connections.remove(key);
}

fn create_connection(
    req:&Request<Incoming>,
    incoming_http_version: Version,
    _target:Target,
    client_addr:&SocketAddr,
    target_scheme: &str,
    target_http_version: hyper::http::Version,
    target_addr: &str,
    incoming_known_tls_only: bool,
    target_host_name : String
) -> ProxyActiveConnection {
    let uri = req.uri();
    let typ_info = 
        ProxyActiveConnectionType::TerminatingHttp { 
            incoming_scheme: uri.scheme_str().unwrap_or(if incoming_known_tls_only { "HTTPS" } else {"HTTP"} ).to_owned(), 
            incoming_http_version: format!("{:?}",incoming_http_version), 
            outgoing_http_version: format!("{:?}",target_http_version), 
            outgoing_scheme: target_scheme.to_owned()
        };

    ProxyActiveConnection {
        target_name: target_host_name,
        source_addr: client_addr.clone(),
        target_addr: target_addr.to_owned(),
        //target: ReverseTcpProxyTarget::from_target(target),
        creation_time: Local::now(),
        description: None,
        connection_type: typ_info
    }
}