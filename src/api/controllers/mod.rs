use std::sync::Arc;

use axum::{http::StatusCode, response::{IntoResponse, Response}, Json, Router};
use serde::{Deserialize, Serialize};
use crate::global_state::GlobalState;

pub mod sites;
pub mod settings;

pub fn routes(state:Arc<GlobalState>) -> Router {

    let sites = Router::new()
        .route("/api/sites", axum::routing::post(sites::update_handler)).with_state(state.clone())
        .route("/api/sites", axum::routing::get(sites::list_handler)).with_state(state.clone())
        .route("/api/sites", axum::routing::delete(sites::delete_handler)).with_state(state.clone())
        .route("/api/sites/start", axum::routing::put(sites::start_handler)).with_state(state.clone())
        .route("/api/sites/stop", axum::routing::put(sites::stop_handler)).with_state(state.clone())
        .route("/api/sites/status", axum::routing::get(sites::status_handler)).with_state(state.clone())
        ;

    let settings = Router::new()
        .route("/api/settings", axum::routing::post(settings::set_settings_handler)).with_state(state.clone())
        .route("/api/settings", axum::routing::get(settings::get_settings_handler)).with_state(state.clone());

    sites.merge(settings)

} 
