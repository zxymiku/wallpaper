use flexi_logger::{detailed_format, Cleanup, Criterion, FileSpec, Logger, Naming};
use std::path::Path;

pub fn setup_logging(app_data_dir: &Path) -> Result<flexi_logger::LoggerHandle, flexi_logger::FlexiLoggerError> {
    let log_dir = app_data_dir.join("logs");
    let file_spec = FileSpec::default().directory(log_dir).basename("daily");

    Logger::try_with_str("info")?
        .log_to_file(file_spec)
        .format_for_files(detailed_format) 
        .rotate(
            Criterion::Size(10 * 1024 * 1024),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(1),
        )
        .duplicate_to_stderr(if cfg!(debug_assertions) { 
            flexi_logger::Duplicate::Info
        } else {
            flexi_logger::Duplicate::None
        })
        .start()
}

pub async fn read_logs(app_data_dir: &Path) -> String {
    let log_path = app_data_dir.join("logs").join("daily_rCURRENT.log");
    match tokio::fs::read_to_string(log_path).await {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(200);
            lines[start..].join("\n")
        }
        Err(e) => {
            format!("Failed to read log file: {}", e)
        }
    }
}