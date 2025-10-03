// api/status.rs
use crate::daily_updater::{is_daily_process_running, stop_daily_process};
use crate::utils::{set_daily_running_status, should_daily_be_running};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use log::{error, info};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct StatusResponse {
    enabled: bool,
    process_status: String,
}

#[derive(Debug, Deserialize)]
pub struct ToggleParams {
    enabled: bool,
}

/// Register status-related routes
pub fn routes() -> Router {
    Router::new()
        .route("/api/status", get(get_status_handler))
        .route("/api/toggle", post(toggle_handler))
}

/// Get service status
async fn get_status_handler() -> impl IntoResponse {
    let enabled = should_daily_be_running();
    let process_status = if is_daily_process_running() {
        "running"
    } else {
        "stopped"
    };
    Json(StatusResponse {
        enabled,
        process_status: process_status.to_string(),
    })
}

/// Toggle daily service on/off
async fn toggle_handler(Query(params): Query<ToggleParams>) -> impl IntoResponse {
    info!("Toggle daily to: {}", params.enabled);
    if let Err(e) = set_daily_running_status(params.enabled) {
        error!("Failed to set status: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to set status").into_response();
    }
    if !params.enabled {
        stop_daily_process();
    }
    (StatusCode::OK, "Status updated").into_response()
}
