#![windows_subsystem = "windows"]
use anyhow::Result;
use chrono::{Local, DateTime, Utc};
use log::{info, warn};
use std::time::Duration;
use tokio::{fs, time};
use shared::*;
#[tokio::main]
async fn main() -> Result<()> {
    init_logging("person.log").unwrap_or_else(|e| eprintln!("log warn{}", e));

    if let Ok(app_dir) = get_app_dir() {
        let current_exe_path = std::env::current_exe()?;
        let exe_in_appdata = app_dir.join(PERSON_EXE);
        if current_exe_path != exe_in_appdata {
            std::fs::copy(&current_exe_path, &exe_in_appdata)?;
            run_program_hidden(&exe_in_appdata)?;
            return Ok(());
        }
        set_autostart("SystemWallpaperServicePerson", &exe_in_appdata).ok();
    }

    let mut interval = time::interval(Duration::from_secs(60 * 3));
    loop {
        interval.tick().await;

        if let Ok(app_dir) = get_app_dir() {
            let stop_file = app_dir.join("person.stop");
            if stop_file.exists() {
                if let Ok(content) = fs::read_to_string(&stop_file).await {
                    if let Ok(resume_ts) = content.trim().parse::<i64>() {
                        if Local::now().timestamp() < resume_ts {
                            info!("began stop{}", DateTime::<Utc>::from_timestamp(resume_ts, 0).unwrap());
                            continue;
                        } else {
                            warn!("over stop");
                            fs::remove_file(stop_file).await.ok();
                        }
                    }
                }
            }
        }

        info!("protect");
        guard_process(DAILY_EXE, DAILY_DOWNLOAD_URL).await.ok();
        guard_process(MANAGER_EXE, MANAGER_DOWNLOAD_URL).await.ok();
    }
}