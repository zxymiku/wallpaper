use crate::logger::read_logs;
use crate::state::{AppState, TempWallpaper};
use axum::{
    extract::State,
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Local};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn start_server(state: Arc<AppState>) -> Result<(), String> {
    let app = Router::new()
        .route("/", get(handle_root))
        .route("/api/temp_wallpaper", post(handle_set_temp_wallpaper))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 11452));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| e.to_string())?;

    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| e.to_string())
}


#[derive(Deserialize)]
pub struct TempWallpaperPayload {
    url: String,
    hours: Option<u32>, //
}

#[derive(Serialize)]
pub struct ApiResponse {
    success: bool,
    message: String,
}

async fn handle_set_temp_wallpaper(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TempWallpaperPayload>,
) -> (StatusCode, Json<ApiResponse>) {
    let hours = payload.hours.unwrap_or(1); // 默认为1小时
    let expiry = Local::now() + Duration::hours(hours as i64);

    let mut temp_lock = state.temp_wallpaper.lock().await;
    *temp_lock = Some(TempWallpaper {
        url: payload.url.clone(),
        expiry,
    });

    state.wallpaper_notify.notify_one();

    let message = format!(
        "Temporary wallpaper set to {} for {} hours.",
        payload.url, hours
    );
    log::info!("{}", message);

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message,
        }),
    )
}

async fn handle_root(State(state): State<Arc<AppState>>) -> Html<String> {
    let current_url = state.current_wallpaper_url.lock().await.clone();
    let config_json = state.config.lock().await
        .as_ref()
        .and_then(|c| serde_json::to_string_pretty(c).ok())
        .unwrap_or_else(|| "Config not loaded.".to_string());
    
    let logs = read_logs(&state.app_data_dir).await;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Daily Wallpaper Status</title>
            <style>
                body {{ font-family: sans-serif; line-height: 1.6; padding: 20px; background: #f4f4f4; }}
                h1, h2 {{ color: #333; }}
                pre {{ background: #eee; padding: 15px; border-radius: 5px; overflow-x: auto; }}
                .container {{ max-width: 900px; margin: auto; background: #fff; padding: 20px; border-radius: 8px; box-shadow: 0 2px 5px rgba(0,0,0,0.1); }}
                .section {{ margin-bottom: 20px; }}
                form input[type="text"], form input[type="number"] {{ width: 300px; padding: 8px; margin-right: 10px; }}
                form button {{ padding: 8px 12px; }}
            </style>
        </head>
        <body>
            <div class="container">
                <h1>Daily Wallpaper Status</h1>

                <div class="section">
                    <h2>Current Wallpaper URL</h2>
                    <pre><code>{current_url}</code></pre>
                </div>

                <div class="section">
                    <h2>Set Temporary Wallpaper (API)</h2>
                    <form id="tempForm">
                        <label for="url">URL:</label>
                        <input type="text" id="url" name="url" required>
                        <label for="hours">Hours (default 1):</label>
                        <input type="number" id="hours" name="hours" min="1" value="1">
                        <button type="submit">Set</button>
                    </form>
                    <p id="apiResponse"></p>
                </div>

                <div class="section">
                    <h2>Current Config (config.json)</h2>
                    <pre><code>{config_json}</code></pre>
                </div>

                <div class="section">
                    <h2>Recent Logs (Last 200 lines)</h2>
                    <pre><code>{logs}</code></pre>
                </div>
            </div>

            <script>
                document.getElementById('tempForm').addEventListener('submit', async function(e) {{
                    e.preventDefault();
                    const url = document.getElementById('url').value;
                    const hours = parseInt(document.getElementById('hours').value) || 1;
                    const responseEl = document.getElementById('apiResponse');
                    
                    try {{
                        const response = await fetch('/api/temp_wallpaper', {{
                            method: 'POST',
                            headers: {{ 'Content-Type': 'application/json' }},
                            body: JSON.stringify({{ url, hours }})
                        }});
                        const data = await response.json();
                        if (response.ok) {{
                            responseEl.style.color = 'green';
                            responseEl.textContent = 'Success: ' + data.message;
                            // 稍后刷新以显示新URL
                            setTimeout(() => location.reload(), 2000);
                        }} else {{
                            responseEl.style.color = 'red';
                            responseEl.textContent = 'Error: ' + data.message;
                        }}
                    }} catch (err) {{
                        responseEl.style.color = 'red';
                        responseEl.textContent = 'Network error: ' + err;
                    }}
                }});
            </script>
        </body>
        </html>
        "#,
        current_url = html_escape(&current_url),
        config_json = html_escape(&config_json),
        logs = html_escape(&logs)
    );

    Html(html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}