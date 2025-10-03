// wallpaper_manager.rs
use crate::config::WallpaperConfig;
use crate::download::download_single_thread;
use crate::utils::get_app_dir;
use anyhow::Result;
use chrono::{Datelike, Local, NaiveTime, Weekday};
use hex;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use wallpaper;

#[derive(Debug, Serialize, Deserialize)]
struct TempWallpaperRecord {
    url: String,
    set_date: String, // Format: YYYY-MM-DD
}

///get url
pub fn get_current_wallpaper_url(config: &WallpaperConfig) -> Option<String> {
    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();

    // Check if there's a temporary wallpaper set for today
    if let Ok(app_dir) = get_app_dir() {
        let temp_record_path = app_dir.join("temp_wallpaper.json");
        if temp_record_path.exists() {
            match std::fs::read_to_string(&temp_record_path) {
                Ok(content) => {
                    match serde_json::from_str::<TempWallpaperRecord>(&content) {
                        Ok(record) => {
                            if record.set_date == today {
                                info!(
                                    "Using temporary wallpaper set on {}: {}",
                                    record.set_date, record.url
                                );
                                return Some(record.url);
                            } else {
                                warn!(
                                    "Temporary wallpaper expired (set on {}, today is {}), removing record",
                                    record.set_date, today
                                );
                                // Remove expired temporary wallpaper record
                                let _ = std::fs::remove_file(&temp_record_path);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse temp wallpaper record: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read temp wallpaper record: {}", e);
                }
            }
        }
    }

    // Use config file URLs if no valid temporary wallpaper
    let current_time = now.time();
    let weekday_str = match now.weekday() {
        Weekday::Mon => "monday",
        Weekday::Tue => "tuesday",
        Weekday::Wed => "wednesday",
        Weekday::Thu => "thursday",
        Weekday::Fri => "friday",
        Weekday::Sat => "saturday",
        Weekday::Sun => "sunday",
    };
    let month_day_str = now.format("%m-%d").to_string();
    for period in &config.wallpapers.periods {
        if period.day == weekday_str {
            if let (Ok(start), Ok(end)) = (
                NaiveTime::parse_from_str(&period.start, "%H:%M"),
                NaiveTime::parse_from_str(&period.end, "%H:%M"),
            ) {
                if current_time >= start && current_time <= end {
                    return Some(period.url.clone());
                }
            }
        }
    }
    if let Some(url) = config.wallpapers.dates.get(&month_day_str) {
        return Some(url.clone());
    }
    config.wallpapers.days.get(weekday_str).cloned()
}

///check and set wallpaper
pub async fn check_and_set_wallpaper(config: &WallpaperConfig) -> Result<()> {
    if let Some(url) = get_current_wallpaper_url(config) {
        let image_path = download_image(&url).await?;
        let image_path_str = image_path.to_str().unwrap_or_default();
        let current_wallpaper_path = wallpaper::get().unwrap_or_default();
        if current_wallpaper_path != image_path_str {
            info!("setting wallpaper to: {}", image_path_str);
            if let Err(e) = wallpaper::set_from_path(image_path_str) {
                return Err(anyhow::anyhow!("failed set wallpaper from path: {}", e));
            }
            if let Err(e) = wallpaper::set_mode(wallpaper::Mode::Stretch) {
                return Err(anyhow::anyhow!("failed set wallpaper mode: {}", e));
            }
        }
    }
    Ok(())
}

///download image
async fn download_image(url: &str) -> Result<PathBuf> {
    let data_dir = get_app_dir()?;
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let file_name = format!("{}.jpg", hex::encode(hasher.finalize()));
    let file_path = data_dir.join(file_name);
    if !file_path.exists() {
        info!("downloading image from {}", url);
        let bytes = download_single_thread(url).await?;
        std::fs::write(&file_path, &bytes)?;
        info!("image saved to {:?}", file_path);
    } else {
        info!("image already exists at {:?}", file_path);
    }
    Ok(file_path)
}
