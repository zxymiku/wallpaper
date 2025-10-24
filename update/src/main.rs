#![windows_subsystem = "windows"]

use log::{error, info, warn};
use reqwest::Client;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
    TH32CS_SNAPPROCESS, PROCESSENTRY32W,
};
use windows::Win32::System::Threading::{
    OpenProcess, TerminateProcess, PROCESS_TERMINATE,
};

const UPDATE_EXE_URL: &str = "https://gh-proxy.com/https://github.com/zxymiku/wallpaper/releases/download/config/daily.exe";
const UPDATE_HASH_URL: &str = "https://gh-proxy.com/https://github.com/zxymiku/wallpaper/releases/download/config/daily.sha256";
const APP_DIR_NAME: &str = "DailyWallpaper";
const DAILY_EXE_NAME: &str = "daily.exe";
const DAILY_PROC_NAME: &str = "daily.exe";

#[tokio::main]
async fn main() {
    let app_data_dir = match init_app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Update: Failed to init app data dir: {}", e);
            return;
        }
    };
    if let Err(e) = setup_logging(&app_data_dir) {
        eprintln!("Update: Failed to setup logging: {}", e);
    }
    info!("--- Update application started ---");
    if let Err(e) = set_autostart() {
        error!("Update: Failed to set autostart: {}", e);
    }
    update_loop(&app_data_dir).await;
}

async fn update_loop(app_data_dir: &Path) {
    let client = Client::new();
    let daily_exe_path = app_data_dir.join(DAILY_EXE_NAME);
    loop {
        info!("Checking for updates...");
        match check_for_updates(&client, &daily_exe_path).await {
            Ok(true) => {
                info!("New version detected. Starting update process.");
                if let Err(e) = perform_update(&client, &daily_exe_path).await {
                    error!("Update failed: {}", e);
                } else {
                    info!("Update successful.");
                }
            }
            Ok(false) => {
                info!("Application is up to date.");
            }
            Err(e) => {
                error!("Failed to check for updates: {}", e);
            }
        }
        sleep(Duration::from_secs(60 * 60 * 4)).await;
    }
}
async fn check_for_updates(client: &Client, local_path: &Path) -> Result<bool, String> {
    let remote_hash = client
        .get(UPDATE_HASH_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?
        .trim()
        .to_lowercase(); //

    if remote_hash.is_empty() {
        return Err("Remote hash is empty".to_string());
    }
    let local_hash = if local_path.exists() {
        let data = fs::read(local_path).await.map_err(|e| e.to_string())?;
        sha256::digest(&data[..]).to_lowercase()
    } else {
        info!("Local {} not found, forcing update.", DAILY_EXE_NAME);
        "local_file_missing".to_string()
    };
    Ok(remote_hash != local_hash)
}


async fn perform_update(client: &Client, daily_exe_path: &Path) -> Result<(), String> {
    info!("Terminating {} process...", DAILY_PROC_NAME);
    kill_process_by_name(DAILY_PROC_NAME)?;
    let temp_exe_path = daily_exe_path.with_extension("exe.new");
    info!("Downloading new version to {}", temp_exe_path.display());
    let resp = client
        .get(UPDATE_EXE_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    fs::write(&temp_exe_path, bytes)
        .await
        .map_err(|e| e.to_string())?;
    info!("Replacing old executable...");
    if daily_exe_path.exists() {
        fs::remove_file(daily_exe_path)
            .await
            .map_err(|e| e.to_string())?;
    }
    fs::rename(&temp_exe_path, daily_exe_path)
        .await
        .map_err(|e| e.to_string())?;
    info!("Starting new {}...", daily_exe_path.display());
    Command::new(daily_exe_path)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}


fn kill_process_by_name(process_name: &str) -> Result<(), String> {
    let wide_name = to_wide_string(process_name);
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| e.to_string())?;
        
        let mut pe32 = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut pe32).is_err() {
            return Err("Failed to get first process".to_string());
        }
        loop {
            let proc_name_slice = std::slice::from_raw_parts(
                pe32.szExeFile.as_ptr(),
                wide_name.len()
            );

            if proc_name_slice == wide_name {
                info!("Found process {} (PID: {})", process_name, pe32.th32ProcessID);
                let process_handle = OpenProcess(PROCESS_TERMINATE, false, pe32.th32ProcessID)
                    .map_err(|e| e.to_string())?;
                
                if TerminateProcess(process_handle, 1).is_ok() {
                    info!("Process terminated.");
                } else {
                    warn!("Failed to terminate process.");
                }
            }

            if Process32NextW(snapshot, &mut pe32).is_err() {
                break;
            }
        }
    }
    Ok(())
}

fn init_app_data_dir() -> Result<PathBuf, std::io::Error> {
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

    std::fs::create_dir_all(app_data_path.join("logs"))?;
    Ok(app_data_path)
}

fn setup_logging(app_data_dir: &Path) -> Result<flexi_logger::LoggerHandle, flexi_logger::FlexiLoggerError> {
    let log_dir = app_data_dir.join("logs");
    let file_spec = flexi_logger::FileSpec::default().directory(log_dir).basename("update");

    flexi_logger::Logger::try_with_str("info")?
        .log_to_file(file_spec)
        .format_for_files(flexi_logger::detailed_format)
        .rotate(
            flexi_logger::Criterion::Size(5 * 1024 * 1024),
            flexi_logger::Naming::Timestamps,
            flexi_logger::Cleanup::KeepLogFiles(1),
        )
        .duplicate_to_stderr(if cfg!(debug_assertions) {
            flexi_logger::Duplicate::Info
        } else {
            flexi_logger::Duplicate::None
        })
    .start()
}

fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn set_autostart() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_path_str = exe_path.to_str().unwrap_or("");

    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let output = Command::new("reg")
        .args([
            "add",
            key,
            "/v",
            "DailyWallpaperUpdater",
            "/t",
            "REG_SZ",
            "/d",
            exe_path_str,
            "/f",
        ])
        .output()
        .map_err(|e| format!("Failed to run reg add: {}", e))?;

    if output.status.success() {
        info!("Update: Autostart set successfully.");
        Ok(())
    } else {
        Err(format!("reg add failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}