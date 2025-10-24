use log::{error, info, warn};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use std::process::Command;


fn get_current_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Failed to get current exe path: {}", e))
}


fn check_autostart() -> Result<bool, String> {
    let exe_path = get_current_exe_path()?;
    let exe_str = exe_path.to_str().unwrap_or("");
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let output = Command::new("reg")
        .args(["query", key, "/v", "DailyWallpaper"]) 
        .output()
        .map_err(|e| format!("Failed to run reg query: {}", e))?;

    if !output.status.success() {
        return Ok(false);
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout.contains(exe_str))
}
pub fn set_autostart() -> Result<(), String> {
    let exe_path = get_current_exe_path()?;
    let exe_path_str = exe_path.to_str().unwrap_or("");
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let output = Command::new("reg")
        .args([
            "add",
            key,
            "/v",
            "DailyWallpaper",
            "/t",
            "REG_SZ",
            "/d",
            exe_path_str,
            "/f",
        ])
        .output()
        .map_err(|e| format!("Failed to run reg add: {}", e))?;
    if output.status.success() {
        info!("Autostart set successfully for: {}", exe_path_str);
        Ok(())
    } else {
        Err(format!(
            "reg add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

pub async fn check_loop() {
    loop {
        match check_autostart() {
            Ok(true) => { /* All good */ }
            Ok(false) => {
                warn!("Autostart disabled or incorrect. Re-enabling...");
                if let Err(e) = set_autostart() {
                    error!("Failed to re-enable autostart: {}", e);
                }
            }
            Err(e) => {
                error!("Error checking autostart: {}", e);
            }
        }
        sleep(Duration::from_secs(3600)).await;
    }
}
pub fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}