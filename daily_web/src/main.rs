#![windows_subsystem = "windows"]
use url::Url;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = if args.len() > 1 && (args[1].starts_with("http") || args[1].starts_with("file")) {
        args[1].clone()
    } else {
        "https://www.google.com".to_string()
    };

    if Url::parse(&url).is_err() {
        eprintln!("Invalid URL provided");
        return;
    }

    // On Windows, use `start` to open the default browser. Keep this minimal so the binary compiles
    // without pulling in GUI dependencies. This is a fallback for the web wallpaper helper.
    if let Err(e) = std::process::Command::new("cmd")
        .args(["/C", "start", &url])
        .spawn()
    {
        eprintln!("Failed to open URL {}: {}", url, e);
    }
}