#![windows_subsystem = "windows"]
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use log::{error, info};
mod autostart;
mod config;
mod file_manager;
mod logger;
mod state;
mod web_server;
mod wallpaper;
use state::AppState;

const CONFIG_JSON_URL: &str = "https://gh-proxy.com/https://github.com/zxymiku/wallpaper/releases/download/config/config.json";

#[tokio::main]
async fn main() {
    let app_data_dir = match file_manager::init_app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to initialize app data directory: {}", e);
            return;
        }
    };
    
    if let Err(e) = logger::setup_logging(&app_data_dir) {
        eprintln!("Failed to setup logging: {}", e);
    }

    info!("--- Daily Wallpaper aplication started ---");
    if let Err(e) = autostart::set_autostart() {
        error!("Failed to set autostart: {}", e);
    }
let app_state = Arc::new(AppState {
        config: Mutex::new(None),
        temp_wallpaper: Mutex::new(None),
        current_wallpaper_url: Mutex::new(String::new()),
        app_data_dir: app_data_dir.clone(),
        wallpaper_notify: Notify::new(),
        web_wallpaper_pid: Mutex::new(None),
    });
    let autostart_handle = tokio::spawn(async {
        autostart::check_loop().await;
    });
    let cleanup_state = app_state.clone();
    let cleanup_handle = tokio::spawn(async move {
        file_manager::cleanup_loop(cleanup_state, CONFIG_JSON_URL).await;
    });
    let wallpaper_state = app_state.clone();
    let wallpaper_handle = tokio::spawn(async move {
        wallpaper::wallpaper_loop(wallpaper_state).await;
    });
    info!("Starting web server on port 11452");
    if let Err(e) = web_server::start_server(app_state).await {
        error!("Web server failed: {}", e);
    }
    _ = tokio::join!(autostart_handle, cleanup_handle, wallpaper_handle);
    info!("--- Daily Wallpaper application shutting down ---");
}