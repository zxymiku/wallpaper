// api/mod.rs
mod config;
mod logs;
mod status;
mod wallpaper;

use anyhow::Result;
use axum::{response::Html, routing::get, Router};
use log::info;
use std::net::SocketAddr;

const SERVER_PORT: u16 = 11452;

/// Start API service
pub async fn api_service() -> Result<()> {
    let app = Router::new()
        .route("/", get(serve_html))
        .merge(status::routes())
        .merge(wallpaper::routes())
        .merge(logs::routes())
        .merge(config::routes());

    let addr = SocketAddr::from(([127, 0, 0, 1], SERVER_PORT));
    info!("Manager API server listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

/// Serve HTML interface
async fn serve_html() -> Html<&'static str> {
    Html(include_str!("../index.html"))
}
