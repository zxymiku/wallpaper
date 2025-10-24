use crate::autostart::to_wide_string;
use crate::file_manager::{download_file, wallpaper_url_to_path};
use crate::state::AppState;
use chrono::{Local, NaiveTime, Datelike, Weekday};
use log::{debug, error, info, warn};
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::time::sleep;
use windows::Win32::System::Com;
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
};

pub async fn wallpaper_loop(state: Arc<AppState>) {
    let client = Client::new();
    unsafe {
        if Com::CoInitializeEx(None, Com::COINIT_APARTMENTTHREADED).is_err() {
            error!("Failed to initialize COM");
            return;
        }
    }

    loop {
        let (target_url, is_temp) = match determine_target_url(&state).await {
            Some(url) => url,
            None => {
                warn!("Could not determine target wallpaper. Config may be missing.");
                wait_for_next_check(&state, Duration::from_secs(300)).await;
                continue;
            }
        };

        let mut current_url_lock = state.current_wallpaper_url.lock().await;

        if *current_url_lock != target_url {
            info!("New wallpaper target determined: {}", target_url);
            let image_path = wallpaper_url_to_path(&state.app_data_dir, &target_url);
            match download_file(&client, &target_url, &image_path).await {
                Ok(_) => {
                    info!("Setting wallpaper to: {:?}", image_path);
                    if let Err(e) = set_wallpaper(&image_path) {
                        error!("Failed to set wallpaper: {}", e);
                    } else {
                        if let Err(e) = set_wallpaper_lock(true) {
                            error!("Failed to lock wallpaper setting: {}", e);
                        }
                        *current_url_lock = target_url;
                        info!("Wallpaper change successful.");
                    }
                }
                Err(e) => {
                    error!("Failed to download wallpaper {}: {}", target_url, e);
                }
            }
        } else {
            debug!("Wallpaper is up to date.");
        }
        drop(current_url_lock);
        let wait_duration = if is_temp {
            if let Some(temp) = state.temp_wallpaper.lock().await.clone() {
                let duration = temp.expiry - Local::now();
                Duration::from_secs(duration.num_seconds().max(1) as u64)
            } else {
                Duration::from_secs(60)
            }
        } else {
            Duration::from_secs(60)
        };
        
        wait_for_next_check(&state, wait_duration).await;
    }
}

async fn wait_for_next_check(state: &Arc<AppState>, duration: Duration) {
    debug!("Waiting for {:.1?} or notification", duration);
    select! {
        _ = sleep(duration) => {
            debug!("Timer expired, re-checking wallpaper.");
        }
        _ = state.wallpaper_notify.notified() => {
            info!("Wallpaper notification received, re-checking immediately.");
        }
    }
}

async fn determine_target_url(state: &Arc<AppState>) -> Option<(String, bool)> {
    let now = Local::now();
    
    {
        let mut temp_lock = state.temp_wallpaper.lock().await;
        if let Some(temp) = temp_lock.as_ref() {
            if now < temp.expiry {
                debug!("Using Temp Wallpaper: {}", temp.url);
                return Some((temp.url.clone(), true));
            } else {
                info!("Temp wallpaper expired.");
                *temp_lock = None;
            }
        }
    }

    let config_lock = state.config.lock().await;
    let config = match config_lock.as_ref() {
        Some(c) => c,
        None => return None,
    };

    let current_weekday = now.weekday();
    let current_time = now.time();
    for period in &config.wallpapers.periods { //
        if weekday_from_str(&period.day) == Some(current_weekday) { //
            if let (Ok(start), Ok(end)) = (
                NaiveTime::parse_from_str(&period.start, "%H:%M"), //
                NaiveTime::parse_from_str(&period.end, "%H:%M"), //
            ) {
                if current_time >= start && current_time < end {
                    debug!("Using Period Wallpaper: {}", period.url);
                    return Some((period.url.clone(), false)); //
                }
            }
        }
    }
    let date_key = now.format("%m-%d").to_string();
    if let Some(url) = config.wallpapers.dates.get(&date_key) { 
        debug!("Using Date Wallpaper: {}", url);
        return Some((url.clone(), false));
    }
    let day_url = match current_weekday {
        Weekday::Mon => &config.wallpapers.days.monday, 
        Weekday::Tue => &config.wallpapers.days.tuesday, 
        Weekday::Wed => &config.wallpapers.days.wednesday, 
        Weekday::Thu => &config.wallpapers.days.thursday, 
        Weekday::Fri => &config.wallpapers.days.friday, 
        Weekday::Sat => &config.wallpapers.days.saturday, 
        Weekday::Sun => &config.wallpapers.days.sunday, 
    };
    debug!("Using Day Wallpaper: {}", day_url);
    Some((day_url.clone(), false))
}

fn weekday_from_str(s: &str) -> Option<Weekday> {
    match s.to_lowercase().as_str() {
        "monday" => Some(Weekday::Mon),
        "tuesday" => Some(Weekday::Tue),
        "wednesday" => Some(Weekday::Wed),
        "thursday" => Some(Weekday::Thu),
        "friday" => Some(Weekday::Fri),
        "saturday" => Some(Weekday::Sat),
        "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

fn set_wallpaper(path: &Path) -> Result<(), String> {
    let path_str = path.to_str().ok_or("Invalid path string")?;
    let path_wide = to_wide_string(path_str);
    set_wallpaper_lock(false).map_err(|e| format!("Failed to unlock wallpaper: {}", e))?;
    let result = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(path_wide.as_ptr() as *mut _),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    };

    if result.is_ok() {
        Ok(())
    } else {
        Err("SystemParametersInfoW call failed".to_string())
    }
}

fn set_wallpaper_lock(lock: bool) -> Result<(), String> {
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\ActiveDesktop";
    let value = "NoChangingWallPaper";
    let data = if lock { "1" } else { "0" };
    let output = std::process::Command::new("reg")
        .args(["add", key, "/v", value, "/t", "REG_DWORD", "/d", data, "/f"]) 
        .output()
        .map_err(|e| format!("Failed to run reg add for wallpaper lock: {}", e))?;

    if output.status.success() {
        debug!("Wallpaper lock set to: {}", lock);
        Ok(())
    } else {
        Err(format!("Failed to set NoChangingWallPaper: {}", String::from_utf8_lossy(&output.stderr)))
    }
}