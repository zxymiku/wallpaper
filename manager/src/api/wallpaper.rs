// api/wallpaper.rs
use crate::download::download_single_thread;
use crate::utils::get_app_dir;
use axum::{extract::Query, http::StatusCode, response::IntoResponse, routing::post, Router};
use chrono::Local;
use log::{error, info};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SetWallpaperParams {
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TempWallpaperRecord {
    url: String,
    set_date: String,
}

/// Register wallpaper-related routes
pub fn routes() -> Router {
    Router::new().route("/api/set_wallpaper", post(set_wallpaper_handler))
}

/// Set wallpaper from URL
async fn set_wallpaper_handler(Query(params): Query<SetWallpaperParams>) -> impl IntoResponse {
    info!("API /set_wallpaper called with URL: {}", params.url);

    let data_dir = match get_app_dir() {
        Ok(d) => d,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "No data dir".to_string()).into_response()
        }
    };

    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        error!("failed create data dir: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed create dir: {}", e),
        )
            .into_response();
    }

    let temp_path = data_dir.join("temp.jpg");
    let temp_record_path = data_dir.join("temp_wallpaper.json");

    // Download image
    match download_single_thread(&params.url).await {
        Ok(bytes) => match std::fs::write(&temp_path, &bytes) {
            Ok(_) => match wallpaper::set_from_path(temp_path.to_str().unwrap_or("")) {
                Ok(_) => {
                    // Save temporary wallpaper record with current date
                    let now = Local::now();
                    let record = TempWallpaperRecord {
                        url: params.url.clone(),
                        set_date: now.format("%Y-%m-%d").to_string(),
                    };

                    if let Ok(json) = serde_json::to_string_pretty(&record) {
                        if let Err(e) = std::fs::write(&temp_record_path, json) {
                            error!("Failed to save temp wallpaper record: {}", e);
                        } else {
                            info!(
                                "Temporary wallpaper record saved for date: {}",
                                record.set_date
                            );
                        }
                    }

                    info!("set successfully from: {}", params.url);
                    (StatusCode::OK, "set successfully".to_string()).into_response()
                }
                Err(e) => {
                    error!("faild set from: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("faild because: {}", e),
                    )
                        .into_response()
                }
            },
            Err(e) => {
                error!("failed write file: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed write file: {}", e),
                )
                    .into_response()
            }
        },
        Err(e) => {
            error!("failed download image: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed download: {}", e),
            )
                .into_response()
        }
    }
}
