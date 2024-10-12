use std::{net::SocketAddr, sync::Arc};
use axum::{body::Body, extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State}, response::{IntoResponse, Response}, routing::get, Router};
use futures_util::{SinkExt, StreamExt};
use hyper::StatusCode;
use include_dir::Dir;
use tower_http::cors::{Any, CorsLayer};
use utoipa::{openapi::ExternalDocs, Modify, OpenApi};
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;


#[derive(Clone)]
pub struct WebSocketGlobalState {
    

    pub broadcast_channel: tokio::sync::broadcast::Sender<String>,

    pub global_state: Arc<crate::global_state::GlobalState>
}


mod controllers;

use utoipauto::utoipauto;

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


pub async fn run(globally_shared_state: Arc<crate::global_state::GlobalState>,port:u16,tracing_broadcaster:tokio::sync::broadcast::Sender::<String>) {

    let websocket_state = WebSocketGlobalState {

        broadcast_channel: tokio::sync::broadcast::channel(10).0,
        global_state: globally_shared_state.clone()
    };
    
    let socket_address: SocketAddr = format!("127.0.0.1:{port}").parse().expect("failed to parse socket address");
    let listener = tokio::net::TcpListener::bind(socket_address).await.expect("failed to bind to socket address");


    let cors_env_var = std::env::vars().find(|(key,_)| key=="ODDBOX_CORS_ALLOWED_ORIGIN").map(|x|x.1.to_lowercase());
    let cors_env_var_cloned_for_ws = cors_env_var.clone();

    let mut router = Router::new()

        // API DOCS
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
        
        // API ROUTES
        .merge(crate::api::controllers::routes(globally_shared_state.clone()).await)

        // STATIC FILES
        //.route("/", axum::routing::get(root))
        //.route("/script.js", axum::routing::get(script))

        // WEBSOCKET ROUTE FOR LOGS
        .route("/ws/live_logs", axum::routing::get( move|ws,user_agent,origin,addr,state|
            ws_log_messages_handler(ws,user_agent,origin,addr,state, cors_env_var_cloned_for_ws)).with_state(websocket_state.clone()))

        // STATIC FILES FOR WEB-UI
        .route("/", get(|| async { serve_static_file(axum::extract::Path("index.html".to_string())).await }))
        .route("/*file", get(serve_static_file));

    // in some cases one might want to allow CORS from a specific origin. this is not currently allowed to do from the config file
    // so we use an environment variable to set this. might change in the future if it becomes a common use case
    if let Some(cors_var) = cors_env_var { 
        router = router.layer(
            CorsLayer::new()
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any))
        .layer(axum::middleware::from_fn(move |request: axum::extract::Request, next: axum::middleware::Next|set_cors(request,next,cors_var.clone())));
    }; 

    tokio::spawn(broadcast_manager(websocket_state,tracing_broadcaster));

    axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("must be able to serve")
    
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
    operation_id="live_logs",
    get,
    tag = "Logs",
    path = "/ws/live_logs",
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
                tracing::trace!("CORS variable is '*', allowing connection from host: {lower_cased_orgin_from_client}");
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

                let possibly_admin_port = state.global_state.config.read().await.admin_api_port;
                
                if let Some(p) = possibly_admin_port {
                    let expected_origin = format!("http://localhost:{p}");
                    if lower_cased_orgin_from_client != expected_origin {
                        tracing::warn!("Client origin does not match '{expected_origin}', denying connection");
                        return Response::builder()
                            .status(StatusCode::FORBIDDEN)
                            .header("reason", "bad origin")
                            .body(Body::from("origin not allowed."))
                            .expect("must be able to create response")
                    } else {
                        tracing::debug!("Client origin matches '{expected_origin}', allowing connection");
                    }
                } else {
                    
                    tracing::warn!("No admin api port set in config file even though the admin api is clearly active. This could be because the admin api has been disabled at runtime without having restarted; otherwise it is a bug in oddbox.");
                    
                    return Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .header("reason", "bad origin or server misconfguration")
                        .body(Body::from("something went wrong."))
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
    let mut broadcast_receiver = state.broadcast_channel.subscribe();

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
                _ => {
                    // we dont care about incoming messages atm
                }
            },
            broadcast_message = broadcast_receiver.recv() => {
                match broadcast_message {
                    Ok(msg) => {
                        if sender.send(Message::Text(msg)).await.is_err() {
                            tracing::trace!("Failed to send message to client {who}, disconnecting...");
                            break;
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


async fn broadcast_manager(state: WebSocketGlobalState,tracing_broadcaster:tokio::sync::broadcast::Sender::<String>) {

    let mut broadcast_receiver = tracing_broadcaster.subscribe();

    while let Ok(msg) = broadcast_receiver.recv().await {
        _ = state.broadcast_channel.send(msg);
    }

    //tracing::warn!("leaving broadcast manager main loop")
}
