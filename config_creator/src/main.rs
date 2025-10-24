#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use axum::{response::Html, routing::get, Router};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(serve_html));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:11451").await.unwrap();

    let server_url = format!("http://{}", listener.local_addr().unwrap());
    println!("Configuration server running at: {}", server_url);
    if let Err(e) = open::that(&server_url) {
        eprintln!("Failed to open browser: {}. Please navigate to the URL manually.", e);
    }
    
    axum::serve(listener, app).await.unwrap();
}

async fn serve_html() -> Html<&'static str> {
    Html(include_str!("./index.html"))
}
