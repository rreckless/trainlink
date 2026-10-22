//! Development bridge: serves the desktop core's API over local HTTP, so the web UI can be tested against it.
//! Usage: trainlink-bridge <port> <database file> <picture folder>. Not part of the app

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use trainlink_lib::api::{handle, Call};
use trainlink_lib::db;
use trainlink_lib::state::AppState;

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}

fn serve(app: &Arc<AppState>, stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));

    let mut content_type = String::new();
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-type" => content_type = value.trim().to_string(),
                "content-length" => length = value.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;

    let reply = handle(app, &Call { method: &method, path, query, body: &body, content_type: &content_type });
    let mut out = stream;
    write!(
        out,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PATCH, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
        reply.status,
        reason(reply.status),
        reply.content_type,
        reply.body.len()
    )?;
    out.write_all(&reply.body)?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).map(String::as_str).unwrap_or("8100");
    let database = PathBuf::from(args.get(2).map(String::as_str).unwrap_or("bridge.db"));
    let images = PathBuf::from(args.get(3).map(String::as_str).unwrap_or("bridge_images"));
    let connection = db::open(&database).map_err(|error| std::io::Error::other(error.to_string()))?;
    let app = Arc::new(AppState::new(connection, images));
    let listener = TcpListener::bind(("127.0.0.1", port.parse::<u16>().unwrap_or(8100)))?;
    eprintln!("TrainLink bridge on http://127.0.0.1:{port}");
    for stream in listener.incoming().flatten() {
        let _ = serve(&app, stream);
    }
    Ok(())
}
