// download.rs
use anyhow::Result;
use log::info;
use once_cell::sync::Lazy;
use reqwest::Client;
use std::time::Duration;

///http client
static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
});

/// 单线程下载喵
pub async fn download_single_thread(url: &str) -> Result<Vec<u8>> {
    info!("Starting single-thread download from: {}", url);

    let resp = HTTP_CLIENT.get(url).send().await?;
    info!("GET request successful, status: {}", resp.status());

    let bytes = resp.bytes().await?;
    info!("Downloaded {} bytes successfully", bytes.len());

    Ok(bytes.to_vec())
}
