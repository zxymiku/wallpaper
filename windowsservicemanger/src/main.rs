#![windows_subsystem = "windows"]
use anyhow::Result;
use axum::{extract::Query, response::{Html, IntoResponse}, routing::get, Router};
use chrono::{Local, Timelike};
use log::{error, info};
use serde::Deserialize;
use std::net::SocketAddr;
use std::time::Duration as StdDuration;
use tokio::{fs, time};
use shared::*;
#[derive(Deserialize)]
struct ControlParams {
    days: Option<i64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging("manager.log").unwrap_or_else(|e| eprintln!("日志初始化失败: {}", e));
    if let Ok(app_dir) = get_app_dir() {
        let current_exe_path = std::env::current_exe()?;
        let exe_in_appdata = app_dir.join(MANAGER_EXE);
        if current_exe_path != exe_in_appdata {
            std::fs::copy(&current_exe_path, &exe_in_appdata)?;
            run_program_hidden(&exe_in_appdata)?;
            return Ok(());
        }
        set_autostart("SystemWallpaperServiceManager", &exe_in_appdata).ok();
    }
    
    tokio::spawn(async {
        let app = Router::new()
            .route("/", get(handle_root))
            .route("/stop", get(handle_stop))
            .route("/start", get(handle_start));

        let addr = SocketAddr::from(([127, 0, 0, 1], 9527));
        info!("http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    let mut interval = time::interval(StdDuration::from_secs(60 * 4));
    loop {
        interval.tick().await;
        info!("protect");
        guard_process(DAILY_EXE, DAILY_DOWNLOAD_URL).await.ok();
        guard_process(PERSON_EXE, PERSON_DOWNLOAD_URL).await.ok();
    }
}

async fn handle_root() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn handle_stop(Query(params): Query<ControlParams>) -> impl IntoResponse {
    let days_to_add = params.days.unwrap_or(1);
    if days_to_add <= 0 {
        return "暂停天数必须是正数".into_response();
    }

    let now = Local::now();
    let resume_date = now.date_naive() + chrono::Duration::days(days_to_add);
    let resume_datetime = resume_date.and_hms_opt(10, 0, 0).unwrap();
    let resume_ts = resume_datetime.and_utc().timestamp();

    let app_dir = match get_app_dir() {
        Ok(dir) => dir,
        Err(_) => return "无法获取应用目录".into_response(),
    };

    let ts_str = resume_ts.to_string();
    fs::write(app_dir.join("daily.stop"), &ts_str).await.ok();
    fs::write(app_dir.join("person.stop"), &ts_str).await.ok();
    
    let response_msg = format!("服务已暂停，将于 {} 自动恢复。", resume_datetime.format("%Y-%m-%d %H:%M:%S"));
    info!("{}", response_msg);
    response_msg.into_response()
}

async fn handle_start() -> impl IntoResponse {
    if let Ok(app_dir) = get_app_dir() {
        fs::remove_file(app_dir.join("daily.stop")).await.ok();
        fs::remove_file(app_dir.join("person.stop")).await.ok();
    }
    info!("task have start");
    "have start。".into_response()
}