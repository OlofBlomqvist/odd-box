use std::sync::Arc;

use axum::{extract::{Path, Query}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::global_state::GlobalState;


pub mod sites;
pub mod settings;

pub (crate) fn routes(state:GlobalState) -> Router {

    let sites = Router::new()
        .route("/sites", axum::routing::post(sites::update_handler)).with_state(state.clone())
        .route("/sites", axum::routing::get(sites::list_handler)).with_state(state.clone())
        .route("/sites", axum::routing::delete(sites::delete_handler)).with_state(state.clone())
        .route("/sites/start", axum::routing::put(sites::start_handler)).with_state(state.clone())
        .route("/sites/stop", axum::routing::put(sites::stop_handler)).with_state(state.clone());

    let settings = Router::new()
        .route("/settings", axum::routing::get(settings::get_settings_handler)).with_state(state.clone());

    sites.merge(settings)

} 
