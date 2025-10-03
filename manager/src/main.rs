// main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod api;
mod daily_updater;
mod download;
mod utils;
use anyhow::Result;
use api::api_service;
use daily_updater::{check_and_update_daily, ensure_and_run_daily};
use log::{error, info};
use std::time::Duration;
use utils::{get_log_check_interval_hours, init_logger, monitor_log_size, setup_manager_startup};

const DAILY_VERSION_CHECK_INTERVAL_HOURS: u64 = 24;

pub fn main() -> Result<()> {
    //start log
    init_logger()?;
    info!("Manager service starting...");
    if let Err(e) = setup_manager_startup() {
        error!("Manager failed to set up startup: {}", e);
    }
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            info!("Manager async runtime started");
            //start
            tokio::spawn(monitor_daily_process());
            tokio::spawn(monitor_log_size_task());
            tokio::spawn(monitor_daily_version());
            //start api
            if let Err(e) = api_service().await {
                error!("API service error: {}", e);
            }
        });
    });

    //to keep main thread running
    std::thread::park();
    Ok(())
}

///check daily process
async fn monitor_daily_process() {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        if utils::should_daily_be_running() {
            if let Err(e) = ensure_and_run_daily().await {
                error!("Failed to ensure daily is running: {}", e);
            }
        }
    }
}

///check log size
async fn monitor_log_size_task() {
    let mut interval =
        tokio::time::interval(Duration::from_secs(get_log_check_interval_hours() * 3600));
    loop {
        interval.tick().await;
        monitor_log_size().await;
    }
}

///check daily version
async fn monitor_daily_version() {
    let mut interval = tokio::time::interval(Duration::from_secs(
        DAILY_VERSION_CHECK_INTERVAL_HOURS * 3600,
    ));
    loop {
        interval.tick().await;
        info!("Checking for daily.exe version updates...");
        if let Err(e) = check_and_update_daily().await {
            error!("Failed to check/update daily.exe: {}", e);
        }
    }
}
