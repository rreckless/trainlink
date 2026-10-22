//! TrainLink web server: the API and the built web UI, from one address.
//!
//! Usage: trainlink-server [--port 8000] [--host 127.0.0.1] [--data <folder>] [--web <built frontend folder>]

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use trainlink_lib::{db, server, state::AppState, sync};

struct Options {
    host: String,
    port: u16,
    data: PathBuf,
    web: Option<PathBuf>,
}

fn default_data() -> PathBuf {
    // Windows keeps app data in the roaming AppData folder; macOS keeps it in Application Support
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"));
    match base {
        Some(base) => base.join("com.trainlink.web"),
        None => PathBuf::from("data"),
    }
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        host: "127.0.0.1".into(),
        port: 8000,
        data: default_data(),
        web: ["../frontend/dist", "frontend/dist"].iter().map(PathBuf::from).find(|path| path.is_dir()),
    };
    let mut rest = args.iter().skip(1);
    while let Some(flag) = rest.next() {
        let mut value = || rest.next().cloned().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--host" => options.host = value()?,
            "--port" => options.port = value()?.parse().map_err(|_| "--port must be a number".to_string())?,
            "--data" => options.data = PathBuf::from(value()?),
            "--web" => options.web = Some(PathBuf::from(value()?)),
            other => return Err(format!("Unknown option {other}")),
        }
    }
    Ok(options)
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let options = parse(&args).map_err(std::io::Error::other)?;
    std::fs::create_dir_all(&options.data)?;
    let connection = db::open(&options.data.join("trainlink.db")).map_err(|error| std::io::Error::other(error.to_string()))?;
    let app = Arc::new(AppState::new(connection, options.data.join("locomotive_images")));
    sync::spawn_loop(app.clone());
    match &options.web {
        Some(path) => eprintln!("Web UI from {}", path.display()),
        None => eprintln!("No built web UI found. Run npm run build in frontend/, or pass --web"),
    }
    let listener = TcpListener::bind((options.host.as_str(), options.port))?;
    eprintln!("TrainLink server on http://{}:{}", options.host, options.port);
    server::run(listener, app, options.web)
}
