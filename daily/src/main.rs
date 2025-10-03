// main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod config;
mod download;
mod manager_updater;
mod utils;
mod wallpaper_manager;
use anyhow::Result;
use config::{get_config, load_local_config, should_use_local_config};
use log::{error, info};
use manager_updater::check_and_update_manager;
use std::time::Duration;
use tokio::time;
use utils::{
    check_and_enforce_policy, get_log_check_interval_hours, get_policy_check_interval_mins,
    init_logger, monitor_log_size, setup_and_enforce_policy,
};
use wallpaper_manager::check_and_set_wallpaper;

const DAILY_CHECK_INTERVAL_HOURS: u64 = 6;
const FREQUENT_CHECK_INTERVAL_MINS: u64 = 1;
const MANAGER_VERSION_CHECK_INTERVAL_HOURS: u64 = 24;

#[tokio::main]
async fn main() -> Result<()> {
    //log start
    if let Err(e) = init_logger() {
        eprintln!("Failed to initialize logger: {}", e);
    }
    info!("Daily wallpaper service starting...");
    if let Err(e) = setup_and_enforce_policy() {
        error!("Failed to setup startup or enforce policy: {}", e);
    }
    let mut config: Option<config::WallpaperConfig> = None;
    // set config check interval
    let mut config_check_interval =
        time::interval(Duration::from_secs(DAILY_CHECK_INTERVAL_HOURS * 3600));
    let mut wallpaper_check_interval =
        time::interval(Duration::from_secs(FREQUENT_CHECK_INTERVAL_MINS * 60));
    let mut manager_version_check_interval = time::interval(Duration::from_secs(
        MANAGER_VERSION_CHECK_INTERVAL_HOURS * 3600,
    ));
    let mut log_size_check_interval =
        time::interval(Duration::from_secs(get_log_check_interval_hours() * 3600));
    let mut policy_check_interval =
        time::interval(Duration::from_secs(get_policy_check_interval_mins() * 60));

    loop {
        tokio::select! {
            //get config regually
            _ = config_check_interval.tick() => {
                info!("Periodically fetching new config...");
                // Check if should use local config first
                if let Some(local_path) = should_use_local_config() {
                    match load_local_config(&local_path).await {
                        Ok(new_config) => {
                            info!("Config loaded from local file");
                            config = Some(new_config);
                        }
                        Err(e) => {
                            error!("Failed to load local config: {}", e);
                            // Fallback to remote
                            match get_config().await {
                                Ok(new_config) => {
                                    info!("Config updated from remote successfully");
                                    config = Some(new_config);
                                }
                                Err(e) => error!("Failed to fetch daily config: {}", e),
                            }
                        }
                    }
                } else {
                    match get_config().await {
                        Ok(new_config) => {
                            info!("Config updated from remote successfully");
                            config = Some(new_config);
                        }
                        Err(e) => error!("Failed to fetch daily config: {}", e),
                    }
                }
            }
            //check manager version
            _ = manager_version_check_interval.tick() => {
                info!("Checking for manager.exe version updates...");
                if let Err(e) = check_and_update_manager().await {
                    error!("Failed to check/update manager.exe: {}", e);
                }
            }

            //check log size
            _ = log_size_check_interval.tick() => {
                monitor_log_size().await;
            }

            //check and enforce policy
            _ = policy_check_interval.tick() => {
                if let Err(e) = check_and_enforce_policy() {
                    error!("Failed to check/enforce policy: {}", e);
                }
            }

            //check
            _ = wallpaper_check_interval.tick() => {
                //not exist and get
                if config.is_none() {
                    info!("Config not present, fetching for the first time...");
                    // Check if should use local config first
                    if let Some(local_path) = should_use_local_config() {
                        match load_local_config(&local_path).await {
                            Ok(new_config) => {
                                info!("Initial config loaded from local file");
                                config = Some(new_config);
                            }
                            Err(e) => {
                                error!("Failed to load local config: {}", e);
                                // Fallback to remote
                                match get_config().await {
                                    Ok(new_config) => {
                                        config = Some(new_config);
                                    }
                                    Err(e) => {
                                        error!("Failed to fetch initial config: {}", e);
                                        continue;
                                    }
                                }
                            }
                        }
                    } else {
                        match get_config().await {
                            Ok(new_config) => {
                                config = Some(new_config);
                            }
                            Err(e) => {
                                error!("Failed to fetch initial config: {}", e);
                                continue;
                            }
                        }
                    }
                }
                //check and set
                if let Some(ref conf) = config {
                    if let Err(e) = check_and_set_wallpaper(conf).await {
                        error!("Failed to check and set wallpaper: {}", e);
                    }
                }
            }
        }
    }
}
