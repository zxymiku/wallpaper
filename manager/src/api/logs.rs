// api/logs.rs
use crate::utils::{get_app_dir, get_daily_log_file_name};
use axum::{extract::Path, http::StatusCode, response::IntoResponse, routing::get, Router};
use log::error;

/// Register logs-related routes
pub fn routes() -> Router {
    Router::new().route("/api/logs/:type", get(get_logs_handler))
}

/// Get logs for specified program type
async fn get_logs_handler(Path(log_type): Path<String>) -> impl IntoResponse {
    let log_type = log_type.as_str();

    let log_file_name = match log_type {
        "daily" => get_daily_log_file_name(),
        _ => "manager.log",
    };

    let log_path = if log_type == "daily" {
        match dirs::data_dir() {
            Some(d) => d.join("DailyWallpaperService").join(log_file_name),
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to get data directory".to_string(),
                )
                    .into_response()
            }
        }
    } else {
        match get_app_dir() {
            Ok(dir) => dir.join(log_file_name),
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to get app dir: {}", e),
                )
                    .into_response()
            }
        }
    };

    match std::fs::read_to_string(&log_path) {
        Ok(content) => (StatusCode::OK, content).into_response(),
        Err(e) => {
            error!("Failed to read log file {:?}: {}", log_path, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read log: {}", e),
            )
                .into_response()
        }
    }
}
