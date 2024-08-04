use chrono::Local;
use futures_util::FutureExt;
use http_body::Frame;
use http_body_util::BodyExt;
use hyper::{
    body::Incoming,
    header::{HeaderName, HeaderValue, InvalidHeaderValue, ToStrError, HOST},
    upgrade::OnUpgrade,
    HeaderMap, Request, Response, StatusCode, Version,
};
use hyper_rustls::ConfigBuilderExt;
use hyper_util::rt::{TokioExecutor, TokioIo};

use rustls::{ClientConfig, ALL_VERSIONS};
use std::{net::SocketAddr, task::Poll, time::Duration};
use tungstenite::http;

use lazy_static::lazy_static;

use crate::{
    configuration::v1::H2Hint, global_state::GlobalState, http_proxy::EpicResponse, tcp_proxy::ReverseTcpProxyTarget, types::{app_state::AppState, proxy_state::{ ConnectionKey, ProxyActiveConnection, ProxyActiveConnectionType }}, CustomError
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
    Remote(crate::configuration::v1::RemoteSiteConfig),
    Proc(crate::configuration::v1::InProcessSiteConfig),
}

pub async fn proxy(
    _req_host_name: &str,
    is_https:bool,
    state: GlobalState,
    mut req: hyper::Request<hyper::body::Incoming>,
    target_url: &str,
    target: Target,
    client_ip: SocketAddr,
) -> Result<ProxyCallResult, ProxyError> {
    
    let v = req.version();

    tracing::info!(
        "Incoming {v:?} request to proxy from {client_ip:?} with target url: {target_url}"
    );

    let client_tls_config = ClientConfig::builder_with_protocol_versions(ALL_VERSIONS)
        .with_native_roots()
        .expect("should always be able to build a tls client")
        .with_no_client_auth();

    let https_builder =
        hyper_rustls::HttpsConnectorBuilder::default().with_tls_config(client_tls_config);

    let mut connector = { https_builder.https_or_http().enable_all_versions().build() };

    let mut enforce_https = match &target {
        Target::Remote(x) => x.https.unwrap_or_default(),
        Target::Proc(x) => x.https.unwrap_or_default(),
    };

    let request_upgrade_type = get_upgrade_type(req.headers());

    let request_upgraded = req.extensions_mut().remove::<OnUpgrade>();

    let target_h2_hint = match &target {
        Target::Remote(x) => x.h2_hint.clone(),
        Target::Proc(x) => x.h2_hint.clone(),
    };

    let mut enforce_http2 = false;
    let mut target_url = target_url.to_string();
    
    if let Some(hint) = target_h2_hint {

        match hint {
            H2Hint::H2 => {
                tracing::debug!("H2 HINT DETECTED");
                *req.version_mut() = Version::HTTP_2;
                enforce_http2 = true;
            }
            H2Hint::H2C => {
                tracing::debug!("H2C HINT DETECTED");
                *req.version_mut() = Version::HTTP_2;
                target_url = target_url.replace("https://", "http://").to_string();
                if enforce_https {
                    tracing::warn!("Suspicious configuration for target: {target_url}. the domain is marked both with https and h2c.. will connect using h2c..")
                }
                enforce_https = false;
                enforce_http2 = true;
            }
        }

    } else {
        if !is_https && req.version() == Version::HTTP_2 {
            *req.version_mut() = Version::HTTP_2;
                target_url = target_url.replace("https://", "http://").to_string();
                if enforce_https {
                    tracing::warn!("Suspicious request: h2c request incoming to proxy but target is https.. this is bound to fail..")
                } else {
                    tracing::debug!("Incoming prior knowledge h2c request to {target_url}")
                }
                enforce_https = false;
                enforce_http2 = true;
        } else {
            
            // in most other cases it seems safe to just start from http/1.1
            *req.version_mut() = Version::HTTP_11;
        }
    }

    if enforce_http2 && req.version() != Version::HTTP_2 {
        return Err(ProxyError::OddBoxError(format!("connection to {target_url} is only allowed over http2 due to h2/h2c hint on target site.")));
    }

    if enforce_http2 {
        tracing::warn!("enforcing http2!");
    }
    if enforce_https {
        tracing::trace!("enforcing https!");
        connector.enforce_https();
    }

    let mut proxied_request =
        create_proxied_request(&target_url, req, request_upgrade_type.as_ref())?;

    
    let target_scheme = if enforce_https || target_url.to_lowercase().starts_with("https") {
        "https"
    } else {
        "http(s?)" // stupidest thing I've ever seen
    };


    let con: ProxyActiveConnection = create_connection(
        &proxied_request, 
        target, 
        &client_ip, 
        target_scheme, 
        Version::HTTP_11, 
        &target_url, 
        is_https
    );


    tracing::trace!("Sending request:\n{:?}", proxied_request);

    if enforce_https {
        _ = proxied_request
            .headers_mut()
            .remove("upgrade-insecure-requests");
        _ = proxied_request.headers_mut().remove("host");
    }
    
    let executor = TokioExecutor::new();
    let mut response = {
        hyper_util::client::legacy::Builder::new(executor)
            .http2_only(enforce_http2)
            .build(connector)
            .request(proxied_request)
            .await
            .map_err(ProxyError::LegacyError)?
    };

    

    tracing::trace!(
        "GOT THIS RESPONSE FROM REQ TO '{target_url}' : {:?}",
        response
    );

    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let response_upgrade_type = get_upgrade_type(response.headers());
        tracing::info!("RESPONSE IS TO UPGRADE TO : {response_upgrade_type:?}!!!");
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
                            tracing::warn!("failed to upgrade req: {e:?}");
                            
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
                        WrappedNormalResponse::new(response,state.clone(),con).await
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
                
        let proxied_response = create_proxied_response(response);
        Ok(ProxyCallResult::NormalResponse(WrappedNormalResponse::new(proxied_response,state.clone(),con).await))
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
    pub async fn new(res:Response<Incoming>,state: GlobalState,con: ProxyActiveConnection) -> Self {
        //tracing::trace!("Adding connection for this WrappedNormalResponse.");
        let con_key = add_connection(state.clone(), con).await;
        let drop_state = state.clone();        
        
        let on_drop: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
            let state = drop_state.clone();
            let con_key = con_key.clone();
            tokio::spawn(async move {
                //tracing::trace!("Dropping connection for this WrappedNormalResponse (with 1s delay for visibility in ui).");
                tokio::time::sleep(Duration::from_secs(1)).await;
                del_connection(state, &con_key).await;
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
    ) -> std::task::Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
         match self.b.frame().poll_unpin(cx) {
            Poll::Ready(Some(data)) => Poll::Ready(Some(data)),
            Poll::Ready(None) =>  Poll::Ready(None),            
            Poll::Pending => Poll::Pending
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
) -> Result<Request<B>, ProxyError> {
    // replace the target uri
    *request.uri_mut() = target_url
        .parse()
        .expect(&format!("the target url is not valid: {:?}", target_url));

    let uri = request.uri().clone();

    // replace the host header if it exists
    let headers = request.headers_mut();
    if let Some(x) = headers.get_mut(HOST) {
        if let Some(new_host) = uri.host() {
            tracing::trace!("Replaced original host header: {:?} with {}", x, new_host);
            *x = HeaderValue::from_str(new_host).map_err(map_to_err)?;
        }
    };

    if let Some(value) = upgrade_type {
        tracing::trace!("Repopulate upgrade headers! :: {value}");

        request
            .headers_mut()
            .insert(&*UPGRADE_HEADER, value.parse().map_err(map_to_err)?);
        request
            .headers_mut()
            .insert(&*CONNECTION_HEADER, HeaderValue::from_str(value).map_err(map_to_err)?);
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









async fn add_connection(state:GlobalState,connection:ProxyActiveConnection) -> ConnectionKey {
    let id = uuid::Uuid::new_v4();
    let global_state = state.0.read().await;
    let mut guard = global_state.statistics.write().expect("should always be able to add connections to state.");
    let key = (
        connection.source_addr.clone(),
        id
    );
    _ = guard.active_connections.insert(key, connection);
    key
}

async fn del_connection(state:GlobalState,key:&ConnectionKey) {
    let global_state = state.0.read().await;
    let mut guard = global_state.statistics.write().expect("should always be able to delete connections from state.");
    _ = guard.active_connections.remove(key);
}

fn create_connection(
    req:&Request<Incoming>,
    target:Target,
    client_addr:&SocketAddr,
    target_scheme: &str,
    target_version: hyper::http::Version,
    target_addr: &str,
    incoming_known_tls_only: bool
) -> ProxyActiveConnection {
    let typ_info = 
        ProxyActiveConnectionType::TerminatingHttp { 
            incoming_scheme: req.uri().scheme_str().unwrap_or(if incoming_known_tls_only { "HTTPS" } else {"HTTP"} ).to_owned(), 
            incoming_http_version: format!("{:?}",req.version()), 
            outgoing_http_version: format!("{:?}",target_version), 
            outgoing_scheme: target_scheme.to_owned()
        };

    ProxyActiveConnection {
        source_addr: client_addr.clone(),
        target_addr: target_addr.to_owned(),
        target: ReverseTcpProxyTarget::from_target(target),
        creation_time: Local::now(),
        description: None,
        connection_type: typ_info
    }
}