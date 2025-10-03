// utils.rs
use anyhow::Result;
use dirs;
use log::{error, info};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const MANAGER_APP_NAME: &str = "WallpaperManager";
const LOG_FILE_NAME: &str = "manager.log";
const LOG_CHECK_INTERVAL_HOURS: u64 = 6;
const MAX_LOG_SIZE_BYTES: u64 = 10 * 1024 * 1024;

///get data dir
pub fn get_app_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
        .join(MANAGER_APP_NAME);
    std::fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

///start log
pub fn init_logger() -> Result<()> {
    let log_path = get_app_dir()?.join(LOG_FILE_NAME);
    let file = File::create(&log_path)?;
    env_logger::Builder::new()
        .target(env_logger::Target::Pipe(Box::new(file)))
        .filter_level(log::LevelFilter::Info)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    Ok(())
}

///check log size
pub async fn monitor_log_size() {
    let log_path = match get_app_dir() {
        Ok(dir) => dir.join(LOG_FILE_NAME),
        Err(e) => {
            error!("Failed to get app dir: {}", e);
            return;
        }
    };
    if let Ok(metadata) = std::fs::metadata(&log_path) {
        if metadata.len() > MAX_LOG_SIZE_BYTES {
            info!("Log file exceeded max size, clearing...");
            let _ = std::fs::write(&log_path, b"");
        }
    }
}
pub fn setup_manager_startup() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let (key, _) = hkcu.create_subkey(path)?;
    let exe_path = std::env::current_exe()?;
    key.set_value(MANAGER_APP_NAME, &exe_path.to_string_lossy().as_ref())?;
    info!(
        "Manager startup registry entry set to: {}",
        exe_path.display()
    );
    Ok(())
}

///get status file path
pub fn get_status_file_path() -> Result<PathBuf> {
    Ok(get_app_dir()?.join("status.txt"))
}
///check daily status
pub fn should_daily_be_running() -> bool {
    if let Ok(status_path) = get_status_file_path() {
        if let Ok(content) = std::fs::read_to_string(&status_path) {
            return content.trim() == "enabled";
        }
    }
    true
}

///set daily status
pub fn set_daily_running_status(enabled: bool) -> Result<()> {
    let status_path = get_status_file_path()?;
    let content = if enabled { "enabled" } else { "disabled" };
    std::fs::write(&status_path, content)?;
    info!("Daily running status set to: {}", content);
    Ok(())
}
pub const fn get_log_check_interval_hours() -> u64 {
    LOG_CHECK_INTERVAL_HOURS
}

///get daily log file name
pub const fn get_daily_log_file_name() -> &'static str {
    "daily.log"
}
