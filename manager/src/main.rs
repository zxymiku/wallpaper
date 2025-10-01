#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use dirs;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use sysinfo::{Pid,System};
use std::time::Duration;
use wallpaper;
use winreg::enums::*;
use winreg::RegKey;
use log::{info, error};

const DAILY_DOWNLOAD_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/daily.exe";
const DAILY_PROCESS_NAME: &str = "daily.exe";
const MANAGER_APP_NAME: &str = "WallpaperManager";
const SERVER_PORT: u16 = 11452;
const LOG_FILE_NAME: &str = "manager.log"; 
const LOG_CHECK_INTERVAL_HOURS: u64 = 6;
const MAX_LOG_SIZE_BYTES: u64 = 10 * 1024 * 1024;
static HTTP_CLIENT: Lazy<Client> = Lazy::new(Client::new);

pub fn main() -> Result<()> {
    // loading log
    init_logger()?;

    if let Err(e) = setup_manager_startup() {
        error!("Manager failed to set up startup: {}", e);
    }

    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let server_url = format!("http://127.0.0.1:{}", SERVER_PORT);
            info!("Manager web server listening on {}", server_url);
            tokio::spawn(monitor_daily_process());
            tokio::spawn(monitor_log_size());
            api_service().await;
        });
    });

    std::thread::park();

    Ok(())
}

fn init_logger() -> Result<()> {
    let log_path = get_app_dir()?.join(LOG_FILE_NAME);
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .chain(fern::log_file(log_path)?)
        .apply()?;
    info!("Logger initialized at {:?}", get_app_dir()?);
    Ok(())
}

// clean log >10mb
async fn monitor_log_size() {
    let mut interval = tokio::time::interval(Duration::from_secs(LOG_CHECK_INTERVAL_HOURS * 3600));
    loop {
        interval.tick().await;
        let log_path = get_app_dir().unwrap_or_else(|_| PathBuf::from(".")).join(LOG_FILE_NAME);
        match std::fs::metadata(&log_path) {
            Ok(metadata) if metadata.len() > MAX_LOG_SIZE_BYTES => {
                if let Err(e) = std::fs::write(&log_path, "") {
                    error!("Failed to clear log file: {}", e);
                } else {
                    info!("Log file cleared (exceeded {} bytes)", MAX_LOG_SIZE_BYTES);
                }
            }
            _ => {}
        }
    }
}

async fn api_service() {
    let app = Router::new()
        // 类似于config_creator：编译时嵌入外置HTML
        .route("/", get(serve_html))
        .route("/api/status", get(get_status_handler))
        .route("/api/toggle", post(toggle_handler))
        .route("/api/set_wallpaper", get(set_wallpaper_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], SERVER_PORT));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// 嵌入外置HTML的handler（编译时include_str!）
async fn serve_html() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

#[derive(Debug, Deserialize)]
struct ApiParams { url: String }

#[derive(Serialize)]
struct StatusResponse {
    enabled: bool,
    process_status: String,
}

async fn get_status_handler() -> impl IntoResponse {
    let enabled = should_daily_be_running().unwrap_or(true);
    let process_status = if enabled {
        if is_daily_process_running() { "运行中".to_string() } else { "已停止 (正在尝试启动...)".to_string() }
    } else {
        "已禁用".to_string()
    };

    info!("API /status called: enabled={}, process={}", enabled, process_status);
    Json(StatusResponse { enabled, process_status })
}

#[derive(Deserialize)]
struct ToggleParams {
    enabled: bool,
}

async fn toggle_handler(Json(params): Json<ToggleParams>) -> impl IntoResponse {
    match set_daily_running_status(params.enabled) {
        Ok(_) => {
            info!("API /toggle: enabled set to {}", params.enabled);
            (StatusCode::OK, "Toggled successfully".to_string())
        }
        Err(e) => {
            error!("API /toggle failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to toggle: {}", e))
        }
    }
}

async fn set_wallpaper_handler(Query(params): Query<ApiParams>) -> impl IntoResponse {
    info!("API /set_wallpaper called with URL: {}", params.url);
    let data_dir = match dirs::data_dir() {
        Some(d) => d.join(MANAGER_APP_NAME),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "No data dir".to_string()).into_response(),
    };
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        error!("Failed to create data dir: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create dir: {}", e)).into_response();
    }
    let temp_path = data_dir.join("temp.jpg");
    match HTTP_CLIENT.get(&params.url).send().await {
        Ok(resp) => match resp.bytes().await {
            Ok(bytes) => match std::fs::write(&temp_path, &bytes) {
                Ok(_) => match wallpaper::set_from_path(temp_path.to_str().unwrap_or("")) {
                    Ok(_) => {
                        info!("Wallpaper set successfully from URL: {}", params.url);
                        (StatusCode::OK, "Wallpaper set successfully.".to_string()).into_response()
                    }
                    Err(e) => {
                        error!("Failed to set wallpaper: {}", e); 
                        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to set wallpaper: {}", e)).into_response()
                    }
                },
                Err(e) => {
                    error!("Failed to write temp file: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {}", e)).into_response()
                }
            },
            Err(e) => {
                error!("Failed to download bytes: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to download: {}", e)).into_response()
            }
        },
        Err(e) => {
            error!("Failed to send HTTP request: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to send request: {}", e)).into_response()
        }
    }
}

async fn monitor_daily_process() {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        if let Ok(true) = should_daily_be_running() {
            if !is_daily_process_running() {
                info!("'daily.exe' is not running. Starting it...");
                if let Err(e) = ensure_and_run_daily().await {
                    error!("Failed to start 'daily.exe': {}", e);
                }
            }
        }
    }
}

async fn ensure_and_run_daily() -> Result<()> {
    let daily_path = get_daily_exe_path()?;
    if !daily_path.exists() {
        info!("'daily.exe' not found. Downloading...");
        let bytes = HTTP_CLIENT.get(DAILY_DOWNLOAD_URL).send().await?.bytes().await?;
        std::fs::write(&daily_path, &bytes)?;
    }
    StdCommand::new(&daily_path).spawn()?;
    info!("'daily.exe' started successfully");
    Ok(())
}

fn is_daily_process_running() -> bool {
    let mut sys = System::new_all();
    sys.refresh_processes();
    let exists = sys.processes_by_name(DAILY_PROCESS_NAME).next().is_some();
    exists
}

fn set_daily_running_status(enabled: bool) -> Result<()> {
    let path = get_status_file_path()?;
    std::fs::write(&path, if enabled { "enabled" } else { "disabled" })?;
    if !enabled {
        let mut sys = System::new_all();
        sys.refresh_processes();
        let pids: Vec<Pid> = sys
            .processes_by_name(DAILY_PROCESS_NAME)
            .map(|process| process.pid())
            .collect();
        for pid in pids {
            if let Some(process) = sys.process(pid) {
                process.kill();
            }
        }
        info!("'daily.exe' killed (disabled)");
    }
    Ok(())
}

fn get_app_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir().context("Could not find data directory")?.join(MANAGER_APP_NAME);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn get_daily_exe_path() -> Result<PathBuf> {
    Ok(get_app_dir()?.join(DAILY_PROCESS_NAME))
}

fn get_status_file_path() -> Result<PathBuf> {
    Ok(get_app_dir()?.join("daily_status.flag"))
}

fn should_daily_be_running() -> Result<bool> {
    let path = get_status_file_path()?;
    if !path.exists() {
        std::fs::write(&path, "enabled")?;
        return Ok(true);
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(content.trim() == "enabled")
}
fn setup_manager_startup() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = Path::new("Software").join("Microsoft").join("Windows").join("CurrentVersion").join("Run");
    let (key, _) = hkcu.create_subkey(&path)?;
    let current_exe = std::env::current_exe()?;
    key.set_value(MANAGER_APP_NAME, &current_exe.to_str().unwrap_or_default().to_string())?;
    info!("Manager startup registry set"); 
    Ok(())
}