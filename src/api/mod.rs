use std::net::SocketAddr;
use axum::{extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State}, response::{Html, IntoResponse}, Router};
use futures_util::SinkExt;

use utoipa::{openapi::ExternalDocs, Modify, OpenApi};
use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;


#[derive(Clone)]
pub struct WebSocketGlobalState {
    

    pub broadcast_channel: tokio::sync::broadcast::Sender<String>,

    pub global_state: crate::global_state::GlobalState
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


pub (crate) async fn run(globally_shared_state: crate::global_state::GlobalState,port:Option<u16>,tracing_broadcaster:tokio::sync::broadcast::Sender::<String>) {


    if let Some(p) = port {

        let websocket_state = WebSocketGlobalState {

            broadcast_channel: tokio::sync::broadcast::channel(10).0,
            global_state: globally_shared_state.clone()
        };
        
        let socket_address: SocketAddr = format!("127.0.0.1:{p}").parse().unwrap();
        let listener = tokio::net::TcpListener::bind(socket_address).await.unwrap();

        let app = Router::new()

            // API DOCS
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
            .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
            
             // API ROUTES
            .merge(crate::api::controllers::routes(globally_shared_state.clone()).await)

            .route("/", axum::routing::get(root))
            .route("/script.js", axum::routing::get(script))

            
            // WEBSOCKET ROUTE
            .route("/ws", axum::routing::get(ws_handler).with_state(websocket_state.clone()))

            
        ; 

        tokio::spawn(broadcast_manager(websocket_state,tracing_broadcaster));

        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap()
    }
}

// Define the handler function for the root path
async fn root() -> impl IntoResponse {
    Html(include_str!("../../static/index.html"))
}

// Define the handler function for the JavaScript file
async fn script() -> impl IntoResponse {
    const JS_CONTENT: &str = include_str!("../../static/script.js");

    axum::response::Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/javascript")
        .body::<String>(JS_CONTENT.into())
        .unwrap()
}

/// Simple websocket interface for log messages.
/// Warning: The format of messages emitted is not guaranteed to be stable.
#[utoipa::path(
    operation_id="live_logs",
    get,
    tag = "Logs",
    path = "/live_logs",
)]
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<axum_extra::TypedHeader<axum_extra::headers::UserAgent>>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr> ,
    state : State<WebSocketGlobalState>
) -> impl axum::response::IntoResponse {
    let user_agent = if let Some(axum_extra::TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    tracing::info!("`{user_agent}` at {addr} connected.");
    ws.on_upgrade(move |socket| handle_socket(socket, addr,state.0))
}


async fn handle_socket(socket: WebSocket, who: SocketAddr, state: WebSocketGlobalState) {
    
    let (mut sender, _receiver) = futures_util::StreamExt::split(socket);

    // // wait for client to send initial message 
    // while let Some(Ok(msg)) = receiver.next().await {
    //     tracing::info!("Received message from {who}: {msg:?}");
    //     _ = sender.send(Message::Text(format!("HELLO THERE, YOU SAID {msg:?}"))).await;
    //     break;
    // }

    // tracing::info!("ok client has configured what he needs to do... lets broadcast data to him from the global state");


    let mut broadcast_receiver = state.broadcast_channel.subscribe();

    tracing::info!("Initializing broadcast loop repeater for socket {:?}",who);

    while let Ok(msg) = broadcast_receiver.recv().await {
        _ = sender.send(Message::Text(msg)).await;
    }

    tracing::info!("Websocket context {who} destroyed");
}


// todo - modify tracing layers so we receive all trace events here
async fn broadcast_manager(state: WebSocketGlobalState,tracing_broadcaster:tokio::sync::broadcast::Sender::<String>) {
    // loop {

    //     if state.broadcast_channel.receiver_count() > 0 {
    //         let data = {
    //             let guard = state.global_state.read().await;
    //             let serialized = guard.procs.iter().filter_map(|x|serde_json::to_string_pretty(&x).ok()).collect::<Vec<String>>();
    //             serialized
    //         };
    //         _ = state.broadcast_channel.send(serde_json::to_string_pretty(&vec![data]).unwrap());
    //     }
    //     tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    // }

    let mut broadcast_receiver = tracing_broadcaster.subscribe();

    while let Ok(msg) = broadcast_receiver.recv().await {
        _ = state.broadcast_channel.send(msg);
    }

    //tracing::warn!("leaving broadcast manager main loop")
}
