// download.rs
use anyhow::Result;
use log::{error, info};
use once_cell::sync::Lazy;
use reqwest::Client;
use std::time::Duration;

///https client
static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("WallpaperDaily/1.0")
        .build()
        .unwrap()
});

///单线程下载喵(实测多线程疯狂被cloudflare拦截,被迫使用单线程)
pub async fn download_single_thread(url: &str) -> Result<Vec<u8>> {
    info!("Starting single-thread download from: {}", url);

    let resp = match HTTP_CLIENT.get(url).send().await {
        Ok(r) => {
            info!(
                "GET request successful, status: {}, content-type: {:?}",
                r.status(),
                r.headers().get("content-type")
            );
            r
        }
        Err(e) => {
            error!("GET request failed: {}", e);
            return Err(e.into());
        }
    };

    let bytes = resp.bytes().await?;
    info!("Downloaded {} bytes successfully", bytes.len());
    Ok(bytes.to_vec())
}
