// main1.rs (daily/src/main.rs)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveTime, Weekday};
use hex;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time;
use wallpaper;
use winreg::enums::*;
use winreg::RegKey;

const CONFIG_URL: &str = "https://git.zxymiku.top/https://github.com/zxymiku/wallpaper/releases/download/config/config.json";
const APP_NAME: &str = "DailyWallpaperService";
const DAILY_CHECK_INTERVAL_HOURS: u64 = 6;
const FREQUENT_CHECK_INTERVAL_MINS: u64 = 1;

//create http client
static HTTP_CLIENT: Lazy<Client> = Lazy::new(Client::new);

#[derive(Debug, Deserialize, Clone)]
struct WallpaperConfig {
    wallpapers: Wallpapers,
}

#[derive(Debug, Deserialize, Clone)]
struct Wallpapers {
    days: HashMap<String, String>,
    dates: HashMap<String, String>,
    periods: Vec<Period>,
}

#[derive(Debug, Deserialize, Clone)]
struct Period {
    day: String,
    start: String,
    end: String,
    url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    //clean old config
    if let Err(e) = setup_and_enforce_policy() {
        eprintln!("Failed to setup startup or enforce policy: {}", e);
    }

    let mut config: Option<WallpaperConfig> = None;
    let mut config_check_interval = time::interval(Duration::from_secs(DAILY_CHECK_INTERVAL_HOURS * 3600));
    let mut wallpaper_check_interval = time::interval(Duration::from_secs(FREQUENT_CHECK_INTERVAL_MINS * 60));

    loop {
        tokio::select! {
            _ = config_check_interval.tick() => {
                println!("Periodically fetching new config...");
                match get_config().await {
                    Ok(new_config) => config = Some(new_config),
                    Err(e) => eprintln!("Failed to fetch daily config: {}", e),
                }
            }
            _ = wallpaper_check_interval.tick() => {
                //get config
                if config.is_none() {
                     println!("Config not present, fetching for the first time...");
                     match get_config().await {
                        Ok(new_config) => config = Some(new_config),
                        Err(e) => {
                            eprintln!("Failed to fetch initial config: {}", e);
                            continue;
                        }
                    }
                }

                if let Some(ref conf) = config {
                    if let Err(e) = check_and_set_wallpaper(conf).await {
                         eprintln!("Failed to check and set wallpaper: {}", e);
                    }
                }
            }
        }
    }
}

async fn get_config() -> Result<WallpaperConfig> {
    let config = HTTP_CLIENT.get(CONFIG_URL).send().await?.json::<WallpaperConfig>().await?;
    Ok(config)
}

fn get_current_wallpaper_url(config: &WallpaperConfig) -> Option<String> {
    let now = Local::now();
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

async fn check_and_set_wallpaper(config: &WallpaperConfig) -> Result<()> {
    if let Some(url) = get_current_wallpaper_url(config) {
        let image_path = download_image(&url).await?;
        let image_path_str = image_path.to_str().unwrap_or_default();
        let current_wallpaper_path = wallpaper::get().unwrap_or_default();

        if current_wallpaper_path != image_path_str {
            println!("Setting wallpaper to: {}", image_path_str);
            if let Err(e) = wallpaper::set_from_path(image_path_str) {
                return Err(anyhow::anyhow!("Failed to set wallpaper from path: {}", e));
            }
            if let Err(e) = wallpaper::set_mode(wallpaper::Mode::Stretch) {
                return Err(anyhow::anyhow!("Failed to set wallpaper mode: {}", e));
            }
        }
    }
    Ok(())
}

async fn download_image(url: &str) -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("Could not find data directory")?.join(APP_NAME);
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)?;
    }

    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let file_name = format!("{}.jpg", hex::encode(hasher.finalize()));
    let file_path = data_dir.join(file_name);

    if !file_path.exists() {
        println!("Downloading image from {}", url);
        let bytes = HTTP_CLIENT.get(url).send().await?.bytes().await?;
        std::fs::write(&file_path, &bytes)?;
    }

    Ok(file_path)
}

fn setup_and_enforce_policy() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key_path = Path::new("Software").join("Microsoft").join("Windows").join("CurrentVersion").join("Run");
    let (run_key, _) = hkcu.create_subkey(&run_key_path)?;
    let current_exe = std::env::current_exe()?;
    run_key.set_value(APP_NAME, &current_exe.to_str().unwrap_or("").to_string())?;
    let policy_path = Path::new("Software").join("Microsoft").join("Windows").join("CurrentVersion").join("Policies").join("ActiveDesktop");
    let (policy_key, _) = hkcu.create_subkey(&policy_path)?;
    policy_key.set_value("NoChangingWallPaper", &1u32)?;
    //更激进的设置 暂时不用
    // key.set_value("Wallpaper", &"C:\\path\\to\\default.jpg".to_string())?;
    // key.set_value("WallpaperStyle", &"2".to_string())?; // 2 for Stretch
    Ok(())
}