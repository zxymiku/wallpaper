// utils.rs
use anyhow::{Context, Result};
use dirs;
use log::{error, info};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

const APP_NAME: &str = "DailyWallpaperService";
const LOG_FILE_NAME: &str = "daily.log";
const LOG_CHECK_INTERVAL_HOURS: u64 = 6;
const MAX_LOG_SIZE_BYTES: u64 = 10 * 1024 * 1024;

///get data directory
pub fn get_app_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .context("Failed to get data directory")?
        .join(APP_NAME);
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

///set startup policy
pub fn setup_and_enforce_policy() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key_path = Path::new("Software")
        .join("Microsoft")
        .join("Windows")
        .join("CurrentVersion")
        .join("Run");
    let (run_key, _) = hkcu.create_subkey(&run_key_path)?;
    let exe_path = std::env::current_exe()?;
    let exe_path_str = exe_path.to_str().unwrap_or("");
    run_key.set_value(APP_NAME, &exe_path_str)?;
    info!("Startup registry entry set to: {}", exe_path_str);
    let policy_path = Path::new("Software")
        .join("Microsoft")
        .join("Windows")
        .join("CurrentVersion")
        .join("Policies")
        .join("ActiveDesktop");
    let (policy_key, _) = hkcu.create_subkey(&policy_path)?;
    policy_key.set_value("NoChangingWallPaper", &1u32)?;
    info!("Wallpaper change policy enforced: NoChangingWallPaper = 1");
    Ok(())
}
pub fn check_and_enforce_policy() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key_path = Path::new("Software")
        .join("Microsoft")
        .join("Windows")
        .join("CurrentVersion")
        .join("Run");
    let (run_key, _) = hkcu.create_subkey(&run_key_path)?;
    let exe_path = std::env::current_exe()?;
    let exe_path_str = exe_path.to_str().unwrap_or("");
    let current_value: Result<String, _> = run_key.get_value(APP_NAME);
    match current_value {
        Ok(val) if val == exe_path_str => {}
        _ => {
            run_key.set_value(APP_NAME, &exe_path_str)?;
            info!("Startup registry entry re-enforced: {}", exe_path_str);
        }
    }
    let policy_path = Path::new("Software")
        .join("Microsoft")
        .join("Windows")
        .join("CurrentVersion")
        .join("Policies")
        .join("ActiveDesktop");
    let (policy_key, _) = hkcu.create_subkey(&policy_path)?;
    let current_policy: Result<u32, _> = policy_key.get_value("NoChangingWallPaper");
    match current_policy {
        Ok(1) => {}
        _ => {
            policy_key.set_value("NoChangingWallPaper", &1u32)?;
            info!("Wallpaper change policy re-enforced: NoChangingWallPaper = 1");
        }
    }

    Ok(())
}

///check log stauts
pub const fn get_log_check_interval_hours() -> u64 {
    LOG_CHECK_INTERVAL_HOURS
}
pub const fn get_policy_check_interval_mins() -> u64 {
    5
}
