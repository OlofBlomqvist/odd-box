

use axum::{extract::{Query}, http::StatusCode, response::{IntoResponse, Response}, Json, Router};
use serde::{Deserialize, Serialize};


use crate::global_state::GlobalState;


pub mod sites;
pub mod settings;

pub (crate) async fn routes(state:GlobalState) -> Router {

  
    let adm_port = state.1.read().await.admin_api_port.expect("Admin API port not set even though the admin api is being started.. this is a bug in odd-box");

    async fn set_static_cache_control(request: axum::extract::Request, next: axum::middleware::Next, port: u16) -> axum::response::Response {
        
        let default_cors_origin = format!("http://localhost:{}",port);

        // during development we want to allow setting cors options such that the frontend can be served from a different port than the api
        let cors_var = 
            std::env::vars().find(|(a,_b)|a=="ODDBOX_CORS_ALLOWED_ORIGIN").map(|(_a,b)|b).unwrap_or(default_cors_origin);
        

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
        .route("/settings", axum::routing::get(settings::get_settings_handler)).with_state(state.clone());

    sites.merge(settings).layer(axum::middleware::from_fn(move |request: axum::extract::Request, next: axum::middleware::Next|set_static_cache_control(request,next,adm_port)))

} 
