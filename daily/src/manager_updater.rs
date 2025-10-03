// manager_updater.rs
use anyhow::Result;
use hex;
use log::{error, info};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::Duration;
use sysinfo::{Pid, System};

use crate::download::download_single_thread;
use crate::utils::get_app_dir;

const MANAGER_DOWNLOAD_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/manager.exe";
const MANAGER_DOWNLOAD_URL_FALLBACK: &str =
    "https://github.com/zxymiku/wallpaper/releases/download/config/manager.exe";
const MANAGER_PROCESS_NAME: &str = "manager.exe";

///check and update manager
pub async fn check_and_update_manager() -> Result<()> {
    let manager_path = get_manager_exe_path()?;
    let temp_path = get_manager_temp_path()?;

    //download new version
    info!("Downloading latest manager.exe to check version...");
    let new_bytes = match download_single_thread(MANAGER_DOWNLOAD_URL).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to download from primary URL: {}", e);
            info!("Trying fallback URL: {}", MANAGER_DOWNLOAD_URL_FALLBACK);
            download_single_thread(MANAGER_DOWNLOAD_URL_FALLBACK).await?
        }
    };
    std::fs::write(&temp_path, &new_bytes)?;

    //check new version if exist
    if !manager_path.exists() {
        info!("manager.exe not found, using downloaded version");
        std::fs::rename(&temp_path, &manager_path)?;
        //start manager
        if let Err(e) = StdCommand::new(&manager_path).spawn() {
            error!("Failed to start manager.exe: {}", e);
        }
        return Ok(());
    }

    //check hash
    let current_hash = calculate_file_hash(&manager_path)?;
    let new_hash = calculate_file_hash(&temp_path)?;

    if current_hash == new_hash {
        info!(
            "manager is already up to date (hash: {})",
            hex::encode(&current_hash[..8])
        );
        //delete temp file
        std::fs::remove_file(&temp_path)?;
    } else {
        info!(
            "new version of manager detected (old: {}, new: {})",
            hex::encode(&current_hash[..8]),
            hex::encode(&new_hash[..8])
        );
        //stop old process
        info!("stopping old manager process...");
        stop_manager_process();
        //wait for process stop
        tokio::time::sleep(Duration::from_secs(2)).await;
        //delete old version
        if let Err(e) = std::fs::remove_file(&manager_path) {
            error!("failed remove old manager.exe: {}", e);
        }
        //remove
        std::fs::rename(&temp_path, &manager_path)?;
        info!("manager updated successfully");
        //start new version
        info!("Starting new manager.exe...");
        if let Err(e) = StdCommand::new(&manager_path).spawn() {
            error!("failed start new manager: {}", e);
        }
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

///stop manager process
fn stop_manager_process() {
    let mut sys = System::new_all();
    sys.refresh_processes();
    let pids: Vec<Pid> = sys
        .processes_by_name(MANAGER_PROCESS_NAME)
        .map(|process| process.pid())
        .collect();
    for pid in pids {
        if let Some(process) = sys.process(pid) {
            info!("Killing manager.exe process with PID: {}", pid);
            process.kill();
        }
    }
}

///get manager exe path
fn get_manager_exe_path() -> Result<PathBuf> {
    let app_dir = get_app_dir()?;
    Ok(app_dir.join("manager.exe"))
}

///get manager temp path
fn get_manager_temp_path() -> Result<PathBuf> {
    let app_dir = get_app_dir()?;
    Ok(app_dir.join("manager.exe.tmp"))
}
