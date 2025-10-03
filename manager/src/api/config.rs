// api/config.rs
use axum::{
    body::Bytes,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::{Local, NaiveDate, TimeZone};
use log::{error, info};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SetLocalConfigParams {
    json_content: String,
    expire_date: String,
}

///register
pub fn routes() -> Router {
    Router::new()
        .route("/api/config", get(get_config_handler))
        .route(
            "/api/force_download_config",
            post(force_download_config_handler),
        )
        .route("/api/set_local_config", post(set_local_config_handler))
        .route(
            "/api/upload_local_config",
            post(upload_local_config_handler),
        )
}

///get config file
async fn get_config_handler() -> impl IntoResponse {
    let config_path = match dirs::data_dir() {
        Some(d) => d.join("DailyWallpaperService").join("config.json"),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get data directory".to_string(),
            )
                .into_response()
        }
    };

    match std::fs::read_to_string(&config_path) {
        Ok(content) => (StatusCode::OK, content).into_response(),
        Err(e) => {
            error!("Failed to read config file {:?}: {}", config_path, e);
            (
                StatusCode::NOT_FOUND,
                format!("Config file not found: {}", e),
            )
                .into_response()
        }
    }
}

///download config from remote
async fn force_download_config_handler() -> impl IntoResponse {
    info!("API: Force download config requested");

    let daily_data_dir = match dirs::data_dir() {
        Some(d) => d.join("DailyWallpaperService"),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get data directory".to_string(),
            )
                .into_response()
        }
    };
    let temp_wallpaper_record = daily_data_dir.join("temp_wallpaper.json");
    if temp_wallpaper_record.exists() {
        if let Err(e) = std::fs::remove_file(&temp_wallpaper_record) {
            error!("Failed to remove temp wallpaper record: {}", e);
        } else {
            info!("Temporary wallpaper record removed");
        }
    }
    let force_download_flag = daily_data_dir.join("force_download_config.flag");

    match std::fs::write(&force_download_flag, "1") {
        Ok(_) => {
            info!("Force download flag created");
            (StatusCode::OK, "Config download scheduled".to_string()).into_response()
        }
        Err(e) => {
            error!("Failed to create force download flag: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to schedule download: {}", e),
            )
                .into_response()
        }
    }
}

async fn set_local_config_handler(
    axum::extract::Json(params): axum::extract::Json<SetLocalConfigParams>,
) -> impl IntoResponse {
    info!(
        "API: Set local config requested, expire_date: {}",
        params.expire_date
    );

    if let Err(e) = serde_json::from_str::<serde_json::Value>(&params.json_content) {
        error!("Invalid JSON content: {}", e);
        return (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)).into_response();
    }

    let expire_date = match NaiveDate::parse_from_str(&params.expire_date, "%Y-%m-%d") {
        Ok(date) => {
            let naive_datetime = date.and_hms_opt(23, 59, 59).unwrap();
            Local.from_local_datetime(&naive_datetime).unwrap()
        }
        Err(e) => {
            error!("Invalid date format: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid date format, expected YYYY-MM-DD: {}", e),
            )
                .into_response();
        }
    };

    let expire_time_rfc3339 = expire_date.to_rfc3339();
    let daily_data_dir = match dirs::data_dir() {
        Some(d) => d.join("DailyWallpaperService"),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get data directory".to_string(),
            )
                .into_response()
        }
    };

    if let Err(e) = std::fs::create_dir_all(&daily_data_dir) {
        error!("Failed to create directory: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create directory: {}", e),
        )
            .into_response();
    }
    //save local config
    let local_config_path = daily_data_dir.join("local_config.json");
    let expire_time_path = daily_data_dir.join("local_config_expire.txt");
    if let Err(e) = std::fs::write(&local_config_path, &params.json_content) {
        error!("Failed to save local config: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save config: {}", e),
        )
            .into_response();
    }

    // save expire time
    if let Err(e) = std::fs::write(&expire_time_path, expire_time_rfc3339) {
        error!("Failed to save expire time: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save expire time: {}", e),
        )
            .into_response();
    }

    info!("Local config set successfully, expires at: {}", expire_date);
    (
        StatusCode::OK,
        format!("Local config set until {}", params.expire_date),
    )
        .into_response()
}

/// Upload local config file
async fn upload_local_config_handler(body: Bytes) -> impl IntoResponse {
    info!("API: Upload local config file requested");

    let json_content = match String::from_utf8(body.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            error!("Invalid UTF-8 content: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                "Invalid file encoding, expected UTF-8".to_string(),
            )
                .into_response();
        }
    };
    if let Err(e) = serde_json::from_str::<serde_json::Value>(&json_content) {
        error!("Invalid JSON content: {}", e);
        return (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)).into_response();
    }
    let daily_data_dir = match dirs::data_dir() {
        Some(d) => d.join("DailyWallpaperService"),
        None => {
            error!("Failed to get data directory");
            return (StatusCode::OK, json_content).into_response();
        }
    };

    let temp_wallpaper_record = daily_data_dir.join("temp_wallpaper.json");
    if temp_wallpaper_record.exists() {
        if let Err(e) = std::fs::remove_file(&temp_wallpaper_record) {
            error!("Failed to remove temp wallpaper record: {}", e);
        } else {
            info!("Temporary wallpaper record removed");
        }
    }

    info!(
        "Config file uploaded successfully, {} bytes",
        json_content.len()
    );
    (StatusCode::OK, json_content).into_response()
}
