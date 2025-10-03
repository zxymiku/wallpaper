// config.rs
use crate::download::download_single_thread;
use crate::utils::get_app_dir;
use anyhow::Result;
use chrono::{DateTime, Local};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const CONFIG_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/config.json";
const CONFIG_URL_FALLBACK: &str =
    "https://github.com/zxymiku/wallpaper/releases/download/config/config.json";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WallpaperConfig {
    pub wallpapers: Wallpapers,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Wallpapers {
    pub days: HashMap<String, String>,
    pub dates: HashMap<String, String>,
    pub periods: Vec<Period>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Period {
    pub day: String,
    pub start: String,
    pub end: String,
    pub url: String,
}

///get config
pub async fn get_config() -> Result<WallpaperConfig> {
    //使用代理站
    info!("Downloading config from primary URL: {}", CONFIG_URL);
    let config = match download_json::<WallpaperConfig>(CONFIG_URL).await {
        Ok(config) => {
            info!("Config downloaded successfully from primary URL");
            config
        }
        Err(e) => {
            error!("Failed to download from primary URL: {}", e);
            error!("Error details: {:?}", e);
            //fuckcloudflare 镜像站无法使用
            info!("Trying fallback URL: {}", CONFIG_URL_FALLBACK);
            match download_json::<WallpaperConfig>(CONFIG_URL_FALLBACK).await {
                Ok(config) => {
                    info!("Config downloaded successfully from fallback URL");
                    config
                }
                Err(e) => {
                    error!("Failed to download from fallback URL: {}", e);
                    error!("Error details: {:?}", e);
                    return Err(anyhow::anyhow!(
                        "Failed to download config from both primary and fallback URLs"
                    ));
                }
            }
        }
    };
    //save config
    if let Err(e) = save_config_to_file(&config).await {
        error!("Failed to save config to file: {}", e);
    }

    Ok(config)
}
///download json
async fn download_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T> {
    let bytes = download_single_thread(url).await?;
    //record data
    let preview = if bytes.len() > 500 {
        String::from_utf8_lossy(&bytes[..500]).to_string()
    } else {
        String::from_utf8_lossy(&bytes).to_string()
    };
    info!("Downloaded {} bytes from {}", bytes.len(), url);
    info!("Content preview: {}", preview);

    match serde_json::from_slice(&bytes) {
        Ok(config) => {
            info!("JSON parsed successfully");
            Ok(config)
        }
        Err(e) => {
            error!("=========================================");
            error!("CONFIG FILE PARSE ERROR!");
            error!("=========================================");
            error!("Error type: {}", e);
            error!("Error line: {}", e.line());
            error!("Error column: {}", e.column());
            error!("Error classification: {:?}", e.classify());
            error!("-----------------------------------------");
            error!("Downloaded content (full if < 500 bytes):");
            error!("{}", preview);
            error!("-----------------------------------------");
            Err(e.into())
        }
    }
}

///save config
async fn save_config_to_file(config: &WallpaperConfig) -> Result<()> {
    let app_dir = get_app_dir()?;
    let config_path = app_dir.join("config.json");

    info!("Saving config to: {:?}", config_path);

    let json_str = serde_json::to_string_pretty(config)?;
    std::fs::write(&config_path, json_str)?;

    info!("Config saved successfully to {:?}", config_path);
    Ok(())
}

///check if using local config and if it's expired
pub fn should_use_local_config() -> Option<PathBuf> {
    let app_dir = match get_app_dir() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    let local_config_path = app_dir.join("local_config.json");
    let expire_time_path = app_dir.join("local_config_expire.txt");

    // Check if local config exists
    if !local_config_path.exists() {
        return None;
    }

    // Check if expire time file exists
    if !expire_time_path.exists() {
        return None;
    }
    let expire_time_str = match std::fs::read_to_string(&expire_time_path) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read expire time: {}", e);
            return None;
        }
    };
    let expire_time = match DateTime::parse_from_rfc3339(expire_time_str.trim()) {
        Ok(dt) => dt,
        Err(e) => {
            error!("Failed to parse expire time: {}", e);
            return None;
        }
    };

    let now = Local::now();

    if now < expire_time {
        info!("Using local config until {}", expire_time);
        Some(local_config_path)
    } else {
        warn!(
            "Local config expired at {}, will download from remote",
            expire_time
        );
        // Clean up expired local config
        let _ = std::fs::remove_file(&local_config_path);
        let _ = std::fs::remove_file(&expire_time_path);
        None
    }
}
///load config from local file
pub async fn load_local_config(path: &PathBuf) -> Result<WallpaperConfig> {
    info!("Loading config from local file: {:?}", path);
    let content = std::fs::read_to_string(path)?;
    let config: WallpaperConfig = serde_json::from_str(&content)?;
    info!("Local config loaded successfully");
    Ok(config)
}
