// daily/src/file_manager.rs
use crate::config::Config;
use crate::state::AppState;
use log::{debug, error, info, warn};
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;

const APP_DIR_NAME: &str = "DailyWallpaper";


pub fn init_app_data_dir() -> Result<PathBuf, std::io::Error> {
    let app_data_path = if let Ok(s) = std::env::var("APPDATA") {
        PathBuf::from(s).join(APP_DIR_NAME)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join(APP_DIR_NAME)
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Failed to determine app data directory",
        ));
    };

    std::fs::create_dir_all(app_data_path.join("wallpapers"))?;
    std::fs::create_dir_all(app_data_path.join("logs"))?;
    Ok(app_data_path)
}


pub async fn download_file(client: &Client, url: &str, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        debug!("File {} already exists, skipping download.", dest.display());
        return Ok(());
    }

    debug!("Downloading {} to {}", url, dest.display());
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to download {}: Status {}", url, resp.status()));
    }
    
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    fs::write(dest, bytes).await.map_err(|e| e.to_string())?;
    Ok(())
}


pub async fn download_config(
    client: &Client,
    state: Arc<AppState>,
    config_url: &str,
) -> Result<(), String> {
    let config_path = state.app_data_dir.join("config.json");
    info!("Downloading new config from {}", config_url);

    let resp = client.get(config_url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to download config: Status {}", resp.status()));
    }
    
    let config_text = resp.text().await.map_err(|e| e.to_string())?;
    let new_config: Config = serde_json::from_str(&config_text).map_err(|e| e.to_string())?;
    fs::write(&config_path, config_text).await.map_err(|e| e.to_string())?;
    let mut config_lock = state.config.lock().await;
    *config_lock = Some(new_config);
    state.wallpaper_notify.notify_one();
    info!("Config updated successfully.");
    Ok(())
}

pub async fn cleanup_loop(state: Arc<AppState>, config_url: &str) {
    let client = Client::new();
    loop {
        info!("Running daily cleanup and config download...");
        if let Err(e) = download_config(&client, state.clone(), config_url).await {
            error!("Failed to download new config: {}", e);
            if state.config.lock().await.is_none() {
                warn!("No config loaded. Attempting to load from local file.");
                let config_path = state.app_data_dir.join("config.json");
                if let Ok(config_text) = fs::read_to_string(config_path).await {
                    if let Ok(local_config) = serde_json::from_str::<Config>(&config_text) {
                        *state.config.lock().await = Some(local_config);
                        info!("Loaded local config successfully.");
                    } else {
                        error!("Failed to parse local config file.");
                    }
                } else {
                    error!("No local config file found. Waiting for next cycle.");
                }
            }
        }
        let current_url = state.current_wallpaper_url.lock().await.clone();
        let current_filename = if !current_url.is_empty() && !current_url.starts_with("special") {
        wallpaper_url_to_path(&state.app_data_dir, &current_url)
            .file_name()
            .map(|s| s.to_os_string())
        } else {
            None
        };
        let wallpaper_dir = state.app_data_dir.join("wallpapers");
        if let Ok(mut entries) = fs::read_dir(wallpaper_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ref current) = current_filename {
                        if entry.file_name() == *current {
                            debug!("Skipping cleanup for active wallpaper: {:?}", entry.file_name());
                            continue;
                        }
                    }
                    if let Ok(metadata) = entry.metadata().await {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(duration) = modified.elapsed() {
                                if duration > Duration::from_secs(60 * 60 * 48) { 
                                    info!("Cleaning up old wallpaper: {:?}", path.file_name());
                                    if let Err(e) = fs::remove_file(path).await {
                                        error!("Failed to delete old wallpaper: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        sleep(Duration::from_secs(60 * 60 * 24)).await;
    }
}


pub fn wallpaper_url_to_path(app_data_dir: &Path, url: &str) -> PathBuf {
    let hash = sha256::digest(url);
    let extension = Path::new(url)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.split('?').next().unwrap_or(s)) 
        .unwrap_or("jpg");
    
    app_data_dir.join("wallpapers").join(format!("{}.{}", hash, extension))
}