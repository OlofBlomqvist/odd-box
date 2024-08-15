

use axum::{http::StatusCode, response::{IntoResponse, Response}, Json, Router};
use serde::{Deserialize, Serialize};

use tower_http::cors::{Any, CorsLayer};

use crate::global_state::GlobalState;


pub mod sites;
pub mod settings;

pub (crate) async fn routes(state:GlobalState) -> Router {

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
    let sites = Router::new()
        .route("/sites", axum::routing::post(sites::update_handler)).with_state(state.clone())
        .route("/sites", axum::routing::get(sites::list_handler)).with_state(state.clone())
        .route("/sites", axum::routing::delete(sites::delete_handler)).with_state(state.clone())
        .route("/sites/start", axum::routing::put(sites::start_handler)).with_state(state.clone())
        .route("/sites/stop", axum::routing::put(sites::stop_handler)).with_state(state.clone())
        .route("/sites/status", axum::routing::get(sites::status_handler)).with_state(state.clone())
        ;

    let settings = Router::new()
        .route("/settings", axum::routing::post(settings::set_settings_handler)).with_state(state.clone())
        .route("/settings", axum::routing::get(settings::get_settings_handler)).with_state(state.clone());

    let mut router = sites.merge(settings);

    // in some cases one might want to allow CORS from a specific origin. this is not currently allowed to do from the config file
    // so we use an environment variable to set this. might change in the future if it becomes a common use case
    if let Some((_,cors_var)) = std::env::vars().find(|(key,_)| key=="ODDBOX_CORS_ALLOWED_ORIGIN") { 
        router = router.layer(
            CorsLayer::new()
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any))
        .layer(axum::middleware::from_fn(move |request: axum::extract::Request, next: axum::middleware::Next|set_cors(request,next,cors_var.clone())));
    }
       
    router

} 
