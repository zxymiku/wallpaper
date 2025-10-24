use crate::config::Config;
use chrono::{DateTime, Local};
use std::path::PathBuf;
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Clone)]
pub struct TempWallpaper {
    pub url: String,
    pub expiry: DateTime<Local>,
}

pub struct AppState {
    pub config: Mutex<Option<Config>>,
    pub temp_wallpaper: Mutex<Option<TempWallpaper>>,
    pub current_wallpaper_url: Mutex<String>,
    pub app_data_dir: PathBuf,
    pub wallpaper_notify: Notify,
}