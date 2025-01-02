use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use anyhow::bail;
use axum::{body::Body, extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State}, response::{IntoResponse, Response}, routing::get, Router};
use futures_util::{SinkExt, StreamExt};
use hyper::{body::Incoming, StatusCode};
use hyper_util::rt::TokioIo;
use include_dir::Dir;
use tower::{Service, ServiceExt};
use tower_http::cors::{Any, CorsLayer};
use utoipa::{openapi::ExternalDocs, Modify, OpenApi};
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;


#[derive(Clone)]
pub struct WebSocketGlobalState {
    

    pub broadcast_channel_to_all_websocket_clients: tokio::sync::broadcast::Sender<EventForWebsocketClients>,

    pub global_state: Arc<crate::global_state::GlobalState>
}


mod controllers;

use utoipauto::utoipauto;

use crate::{tcp_proxy::{GenericManagedStream, ManagedStream, Peekable}, types::odd_box_event::EventForWebsocketClients};

#[utoipauto]
#[derive(OpenApi)]
#[openapi(
    modifiers(&HaxAddon)
   
)]
struct ApiDoc;
struct HaxAddon;

impl Modify for HaxAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {

        let mut ed = ExternalDocs::new("https://github.com/OlofBlomqvist/odd-box");
        ed.description = Some("odd-box git repository".into());
        openapi.external_docs = Some(ed);
        
        openapi.tags = Some(vec![
            utoipa::openapi::tag::TagBuilder::new().name("Site management")
                .description("Manage your sites".into()).build(),
        ]);

        openapi.info.description = Some(
            "A basic management api for odd-box reverse proxy."
        .into());
        
        openapi.info.title = "ODD-BOX ADMIN-API 🤯".into();
        

    }
}


async fn set_cors(request: axum::extract::Request, next: axum::middleware::Next, cors_var: String) -> axum::response::Response {
        
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        axum::http::HeaderValue::from_str(&cors_var).expect("Invalid CORS value"),
    );
    
    response.headers_mut().insert(hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
    axum::http::HeaderValue::from_static("GET, PUT, POST, DELETE, HEAD, OPTIONS")
    );

    response
}



// Define the handler function for the root path
// async fn root() -> impl IntoResponse {
//     Html(include_str!("../../static/index.html"))
// }

static WEBUI_DIR: Dir = include_dir::include_dir!("web-ui/dist");
async fn serve_static_file(axum::extract::Path(file): axum::extract::Path<String>) -> impl IntoResponse {
    let file = if file.contains(".") {
        file
    } else {
        "index.html".to_string()
    };
    match WEBUI_DIR.get_file(&file) {
        Some(file) => {
            let mime = mime_guess::from_path(&file.path()).first_or_octet_stream();
            let body = file.contents();
            axum::response::Response::builder()
                .header(hyper::header::CONTENT_TYPE, mime.as_ref())
                .body(body.into())
                .expect("must be able to create response")
        }
        None =>  
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!("404 - File not found; these exist: {}.. you looked for : {file}", WEBUI_DIR.files()
                    .map(|f| f.path().to_str().unwrap()).collect::<Vec<_>>().join(", "))))
                .expect("must be able to create response")
    }
}

/// Simple websocket interface for log messages.
/// Warning: The format of messages emitted is not guaranteed to be stable.
#[utoipa::path(
    operation_id="event_stream",
    get,
    tag = "Events",
    path = "/ws/event_stream",
)]
async fn ws_log_messages_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<axum_extra::TypedHeader<axum_extra::headers::UserAgent>>,
    origin: Option<axum_extra::TypedHeader<axum_extra::headers::Origin>>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr> ,
    state : State<WebSocketGlobalState>,
    cors_env_var : Option<String>
) -> impl axum::response::IntoResponse {
    
    // we only care about limiting these connections if we receive an origin header which most browsers will send.
    // if no custom env var is set, we will only allow connections from the admin api port on localhost.
    // if a custom env var is set for cors, we will only allow connections from that origin.
    // this check was added based on microsoft recomendations: 
    // https://learn.microsoft.com/en-us/aspnet/core/fundamentals/websockets?view=aspnetcore-8.0
    match origin {
        Some(origin_header) => {

            let lower_cased_orgin_from_client = origin_header.to_string().to_lowercase();

            if cors_env_var == Some("*".to_string()) {
                tracing::trace!("CORS variable is '*', allowing connection from '{addr:?}' using origin: {lower_cased_orgin_from_client}");
            } else if let Some(lower_cased_cors_var) = cors_env_var {
                
                if &lower_cased_orgin_from_client != &lower_cased_cors_var {
                    tracing::warn!("Client origin does not match cors env var, denying connection");
                    return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header("reason", "bad origin")
                    .body(Body::from("origin not allowed."))
                    .expect("must be able to create response")
                } else {
                    tracing::debug!("Client origin matches cors env var, allowing connection");
                }
            } else {

                let mut valid_origins = 
                    if let Some(p) = state.global_state.config.read().await.tls_port {
                            if p == 443 {
                                vec![
                                format!("https://localhost"),
                                format!("https://odd-box.localhost"),
                                format!("https://oddbox.localhost")
                            ]
                        } else {
                            vec![
                                format!("https://localhost:{p}"),
                                format!("https://odd-box.localhost:{p}"),
                                format!("https://oddbox.localhost:{p}")
                            ]
                        }
                    } else {
                        vec![
                            format!("https://localhost"),
                            format!("https://odd-box.localhost"),
                            format!("https://oddbox.localhost")
                        ]
                    };

                if let Some(ourl) = state.global_state.config.read().await.odd_box_url.clone() {
                    valid_origins.push(ourl);
                }

                if !valid_origins.contains(&lower_cased_orgin_from_client) {
                    tracing::warn!("Client origin is not in the allowed list of '{valid_origins:?}', denying connection");
                    return Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .header("reason", "bad origin")
                        .body(Body::from("origin not allowed."))
                        .expect("must be able to create response")
                    
                }
                    

            
            }
        },
        None => tracing::debug!("No origin header received, allowing connection") 
        
    } 


    let user_agent = if let Some(axum_extra::TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown client")
    };

    tracing::trace!("`{user_agent}` at {addr} connected.");
    
    let response = ws.on_upgrade(move |socket| handle_socket(socket, addr,state.0));
    return response;

}


async fn handle_socket(client_socket: WebSocket, who: SocketAddr, state: WebSocketGlobalState) {
    let (mut sender, mut receiver) = client_socket.split();
    let mut broadcast_receiver = state.broadcast_channel_to_all_websocket_clients.subscribe();

    loop {
        tokio::select! {
            message = receiver.next() => match message {
                Some(Ok(Message::Close(_))) => {
                    tracing::trace!("Client {who} closed connection");
                    break;
                },
                Some(Err(e)) => {
                    tracing::trace!("Error receiving message from client {who}: {:?}", e);
                    break;
                },
                None => {
                    tracing::trace!("Client {who} disconnected");
                    break;
                },
                Some(Ok(Message::Text(text))) => {
                    tracing::trace!("Received websocket message from client {who}: {text}");
                },
                _ => {}
            },
            broadcast_message = broadcast_receiver.recv() => {
                match broadcast_message {
                    Ok(msg) => {
                        if let Ok(msg_json) = serde_json::to_string(&msg) {
                            if sender.send(Message::Text(msg_json)).await.is_err() {
                                tracing::trace!("Failed to send message to client {who}, disconnecting...");
                                break;
                            }
                        } else {
                            tracing::trace!("Failed to serialize message to json for client {who}");
                        }
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        tracing::trace!("Client {who} lagged behind on receiving broadcast messages");
                    },
                    Err(_) => {
                        tracing::trace!("Broadcast channel closed or an error occurred for client {who}");
                        break;
                    },
                }
            }

        }
    }

    tracing::info!("Websocket context {who} destroyed");
}


async fn broadcast_manager(state: WebSocketGlobalState,tracing_broadcaster:tokio::sync::broadcast::Sender::<EventForWebsocketClients>) {
    // need this in a loop as we will drop all senders when log level is changed at runtime
    // and we dont want to stop sending messages to the websocket clients just because the log level was changed..
    loop {
        let mut odd_box_broadcast_channel = tracing_broadcaster.subscribe();
        while let Ok(msg) = odd_box_broadcast_channel.recv().await {
            _ = state.broadcast_channel_to_all_websocket_clients.send(msg);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

}


#[derive(Debug)]
pub struct OddBoxAPI {
    pub service: axum::extract::connect_info::IntoMakeServiceWithConnectInfo<Router, SocketAddr>,
    pub _state: Arc<crate::global_state::GlobalState>
}

impl OddBoxAPI {

    async fn validate_request(
        req: axum::extract::Request<axum::body::Body>, 
        next: axum::middleware::Next,
        state: Arc<crate::global_state::GlobalState>
    ) -> Result<Response, axum::http::StatusCode> {

        let path = req.uri().path();
        if req.method() == hyper::Method::GET {
            if path.starts_with("/api-docs/") {
                return Ok(next.run(req).await);
            }
            if path.starts_with("/swagger-ui") {
                return Ok(next.run(req).await);
            }
            if path.starts_with("/rapidoc") {
                return Ok(next.run(req).await);
            }
            if path.starts_with("/redoc") {
                return Ok(next.run(req).await);
            }
        }

        let possibly_password = { state.config.read().await.odd_box_password.clone() };
        
        if let Some(pwd) = &possibly_password {
            match req.headers().get("Authorization") {
                Some(value) if value == pwd => {
                    Ok(next.run(req).await)
                }
                _ => {
                    tracing::warn!("Invalid password, denying request");
                    Err(StatusCode::FORBIDDEN)
                }
            }
        } else {
            //tracing::warn!("No password set, allowing all requests");
            Ok(next.run(req).await)
        }
    }
    
    /// IT IS VERY IMPORTANT ONLY ONE INSTANCE OF THIS IS EVER INSTANTIATED
    /// AS IT SPAWNS A BACKGROUND TASK THAT WILL RUN FOREVER
    pub fn new(state: Arc<crate::global_state::GlobalState>) -> Self {

        let validation_state = state.clone();
        let websocket_state = WebSocketGlobalState {
            broadcast_channel_to_all_websocket_clients: tokio::sync::broadcast::channel(10).0,
            global_state: state.clone()
        };


        let cors_env_var = std::env::vars().find(|(key,_)| key=="ODDBOX_CORS_ALLOWED_ORIGIN").map(|x|x.1.to_lowercase());
        let cors_env_var_cloned_for_ws = cors_env_var.clone();
        let mut router = Router::new()
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
            .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
            .merge(Router::new()
                    .route("/STOP", axum::routing::get(stop_handler).with_state(state.clone()))
                    .route("/START",  axum::routing::get(start_handler).with_state(state.clone()))
            )
            .merge(crate::api::controllers::routes(state.clone()))
            .layer(axum::middleware::from_fn(
                    move | req: axum::extract::Request<Body>, next: axum::middleware::Next | {
                        Self::validate_request(
                            req,
                            next,
                            validation_state.clone()
                        )
                    }
                )
            )
            .route("/ws/event_stream", axum::routing::get( move|ws,user_agent,origin,addr,state|
                ws_log_messages_handler(ws,user_agent,origin,addr,state, cors_env_var_cloned_for_ws)).with_state(websocket_state.clone()))
            .route("/", get(|| async { serve_static_file(axum::extract::Path("index.html".to_string())).await }))
            .route("/*file", get(serve_static_file));
        if let Some(cors_var) = cors_env_var { 
            router = router.layer(
                CorsLayer::new()
                    .allow_methods(Any)
                    .allow_headers(Any)
                    .expose_headers(Any))
            .layer(axum::middleware::from_fn(move |request: axum::extract::Request, next: axum::middleware::Next|
                set_cors(request,next,cors_var.clone()))
            );
        }; 


        let svc = router.into_make_service_with_connect_info::<SocketAddr>();

        tokio::spawn(broadcast_manager(websocket_state,state.websockets_broadcast_channel.clone()));
        OddBoxAPI {
            service: svc,
            _state: state
        }
    }

    pub async fn handle_stream(&self,stream: GenericManagedStream,rustls_config: Option<Arc<rustls::ServerConfig>>) -> anyhow::Result<()> {
        
        let rustls_config = if let Some(rustls_config) = rustls_config {
            rustls_config
        } else {
            anyhow::bail!("No rustls config provided");
        };

        let mut svc = self.service.clone();
        
        let (generic_stream,peer_addr) = match stream {
            GenericManagedStream::TCP(mut managed_stream) => {
                
                managed_stream.seal();

                let tls_acceptor = tokio_rustls::TlsAcceptor::from(rustls_config);
                let peer_addr = managed_stream.stream.peer_addr()?;
                match tls_acceptor.accept(managed_stream).await {
                    Ok(tls_stream) => {
                        let mut s = ManagedStream::from_tls_stream(tls_stream);
                        crate::proxy::mutate_tracked_connection(&self._state, &s.tcp_connection_id, |c|{
                            c.tls_terminated = true;
                            c.http_terminated = true;
                        });
                        s.seal();                        
                        (GenericManagedStream::from_terminated_tls_stream(s),peer_addr)
                    }
                    Err(e) => {
                        anyhow::bail!("Failed to accept TLS connection: {:?}",e);
                    }
                }
            },
            GenericManagedStream::TerminatedTLS(s) => {
                let peer_addr = s.stream.get_ref().0.stream.peer_addr()?;
                crate::proxy::mutate_tracked_connection(&self._state, &s.tcp_connection_id, |c|{
                    c.tls_terminated = true;
                    c.http_terminated = true;
                });
                (GenericManagedStream::from_terminated_tls_stream(s),peer_addr)
            }
        };

        let tower_service = svc.call(peer_addr).await.unwrap();
        let hyper_service = hyper::service::service_fn(move |request: axum::extract::Request<Incoming>| {
            tower_service.clone().oneshot(request)
        });

        match generic_stream {
            GenericManagedStream::TCP(_) => unreachable!(),
            GenericManagedStream::TerminatedTLS(mut s) => {
                s.seal();
                if let Err(err) = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                    .serve_connection_with_upgrades(TokioIo::new(s), hyper_service)
                    .await
                {
                    bail!("failed to serve connection: {err:#}");
                }
                Ok(())
            
            }
        }
       

    }

}


// note: not sure if we should keep these start/stop handlers around at all..
// the /START and /STOP commands have always worked like this
// so it seems irresponsible to just drop them but perhaps 
// we should opt for redirecting to the api methods instead ?


pub async fn stop_handler(
    axum::extract::State(global_state): axum::extract::State<Arc<crate::GlobalState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse  {
    let target = params.get("proc");
    let s = target.unwrap_or(&String::from("all")).clone();
    tracing::warn!("Handling order STOP ({})", s);
    let result = global_state.proc_broadcaster.send(crate::http_proxy::ProcMessage::Stop(s)).map_err(|e| format!("{e:?}"));

    match result {
        Ok(_) => {
            let html = r#"
                <center>
                    <h2>Stop signal received.</h2>
                    
                    <form action="/START">
                        <input type="submit" value="Resume" />
                    </form>            

                    <p>The proxy will also resume if you visit any of the stopped sites</p>
                </center>
            "#;
            crate::http_proxy::EpicResponse::new(crate::http_proxy::create_epic_string_full_body(html))
        }
        Err(e) => {
            let mut response = crate::http_proxy::EpicResponse::new(crate::http_proxy::create_epic_string_full_body(&format!("{e:?}")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        }
    }
    // Ok::<(),(StatusCode,String)>(())
}

pub async fn start_handler(
    axum::extract::State(global_state): axum::extract::State<Arc<crate::GlobalState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {

    // Ok::<(),(StatusCode,String)>(())

    let target = params.get("proc");
    let s = target.unwrap_or(&String::from("all")).clone();
    tracing::warn!("Handling order START ({})", s);
    let result = global_state.proc_broadcaster.send(crate::http_proxy::ProcMessage::Start(s)).map_err(|e| format!("{e:?}"));

    match result {
        Ok(_) => {
            let html = r#"
                <center>
                    <h2>Start signal received.</h2>
                    <form action="/STOP">
                        <input type="submit" value="Stop" />
                    </form>            

                </center>
            "#;
            crate::http_proxy::EpicResponse::new(crate::http_proxy::create_epic_string_full_body(html))
        }
        Err(e) => {
            let mut response = crate::http_proxy::EpicResponse::new(crate::http_proxy::create_epic_string_full_body(&format!("{e:?}")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        }
    }
}
