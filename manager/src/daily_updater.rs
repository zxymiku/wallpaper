// daily_updater.rs
use crate::download::download_single_thread;
use crate::utils::get_app_dir;
use anyhow::Result;
use log::{error, info};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::Duration;
use sysinfo::{Pid, System};

const DAILY_DOWNLOAD_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/daily.exe";
const DAILY_DOWNLOAD_URL_FALLBACK: &str =
    "https://github.com/zxymiku/wallpaper/releases/download/config/daily.exe";
const DAILY_PROCESS_NAME: &str = "daily.exe";

///check and update daily
pub async fn check_and_update_daily() -> Result<()> {
    let daily_path = get_daily_exe_path()?;
    let temp_path = get_app_dir()?.join("daily.exe.tmp");

    // download latest daily
    info!("Downloading latest daily.exe to check version...");
    let new_bytes = match download_single_thread(DAILY_DOWNLOAD_URL).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to download from primary URL: {}", e);
            info!("Trying fallback URL: {}", DAILY_DOWNLOAD_URL_FALLBACK);
            download_single_thread(DAILY_DOWNLOAD_URL_FALLBACK).await?
        }
    };
    std::fs::write(&temp_path, &new_bytes)?;

    // check if daily.exe exists
    if !daily_path.exists() {
        info!("daily.exe not found, using downloaded version");
        std::fs::rename(&temp_path, &daily_path)?;
        return Ok(());
    }

    // check hash
    let current_hash = calculate_file_hash(&daily_path)?;
    let new_hash = calculate_file_hash(&temp_path)?;

    if current_hash == new_hash {
        info!("daily.exe is already up to date");
        std::fs::remove_file(&temp_path)?;
    } else {
        info!("New version of daily.exe detected");
        // stop old process
        info!("Stopping old daily.exe process...");
        stop_daily_process();
        // wait for process stop
        tokio::time::sleep(Duration::from_secs(2)).await;
        // delete old version
        if let Err(e) = std::fs::remove_file(&daily_path) {
            error!("Failed to remove old daily.exe: {}", e);
        }
        // rename temp to final
        std::fs::rename(&temp_path, &daily_path)?;
        info!("daily.exe updated successfully");
    }

    Ok(())
}
///ensure and run daily
pub async fn ensure_and_run_daily() -> Result<()> {
    let daily_path = get_daily_exe_path()?;
    if !daily_path.exists() {
        info!("'daily.exe' not found. Downloading...");
        let bytes = match download_single_thread(DAILY_DOWNLOAD_URL).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to download from primary URL: {}", e);
                info!("Trying fallback URL: {}", DAILY_DOWNLOAD_URL_FALLBACK);
                download_single_thread(DAILY_DOWNLOAD_URL_FALLBACK).await?
            }
        };
        std::fs::write(&daily_path, &bytes)?;
        info!("daily.exe downloaded successfully");
    }

    if !is_daily_process_running() {
        info!("Starting daily.exe...");
        StdCommand::new(&daily_path).spawn()?;
        info!("daily.exe started successfully");
    }

    Ok(())
}

///check hash
fn calculate_file_hash(path: &Path) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hasher.finalize().to_vec())
}

///stop daily process
pub fn stop_daily_process() {
    let mut sys = System::new_all();
    sys.refresh_processes();
    let pids: Vec<Pid> = sys
        .processes_by_name(DAILY_PROCESS_NAME)
        .map(|process| process.pid())
        .collect();
    for pid in pids {
        if let Some(process) = sys.process(pid) {
            info!("Killing daily.exe process with PID: {}", pid);
            process.kill();
        }
    }
}

///check daily process running
pub fn is_daily_process_running() -> bool {
    let mut sys = System::new_all();
    sys.refresh_processes();
    let result = sys.processes_by_name(DAILY_PROCESS_NAME).next().is_some();
    result
}

///get daily exe path
pub fn get_daily_exe_path() -> Result<PathBuf> {
    let daily_app_dir = get_daily_app_dir()?;
    Ok(daily_app_dir.join("daily.exe"))
}

///get daily temp path
fn get_daily_app_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?
        .join("DailyWallpaperService");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
