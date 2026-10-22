//! The HTTP server for the web UI. It serves the API at /api, and the built web UI from a folder, from the same
//! address. Requests are read with size limits, so a client cannot exhaust memory with a long request

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::api::{handle, Call};
use crate::http::Reply;
use crate::state::AppState;

/// Largest request body: a layout XML file is up to 5 million characters, and JSON escapes make it larger
pub const MAX_BODY: usize = 16 * 1024 * 1024;
const MAX_HEADER_LINE: usize = 8 * 1024;
const MAX_HEADERS: usize = 100;
const READ_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Why a request could not be read, with the status to answer with
#[derive(Debug, PartialEq)]
pub struct Refused {
    pub status: u16,
    pub detail: &'static str,
}

fn refuse(status: u16, detail: &'static str) -> Refused {
    Refused { status, detail }
}

/// One line of the request, without its line ending, refusing lines that are too long
fn read_line(reader: &mut impl BufRead) -> Result<String, Refused> {
    let mut line = Vec::new();
    let read = reader
        .take(MAX_HEADER_LINE as u64 + 1)
        .read_until(b'\n', &mut line)
        .map_err(|_| refuse(400, "Could not read the request"))?;
    if read == 0 {
        return Err(refuse(400, "The request ended early"));
    }
    if line.len() > MAX_HEADER_LINE {
        return Err(refuse(431, "A header line is too long"));
    }
    Ok(String::from_utf8_lossy(&line).trim_end().to_string())
}

/// Read one request: the request line, the headers, and a body of the length the headers give
pub fn read_request(reader: &mut impl BufRead) -> Result<Request, Refused> {
    let request_line = read_line(reader)?;
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(target), Some(version)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(refuse(400, "The request line is not valid"));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(refuse(400, "Only HTTP/1.x is supported"));
    }

    let mut headers = Vec::new();
    loop {
        let line = read_line(reader)?;
        if line.is_empty() {
            break;
        }
        if headers.len() >= MAX_HEADERS {
            return Err(refuse(431, "Too many headers"));
        }
        let (name, value) = line.split_once(':').ok_or_else(|| refuse(400, "A header is not valid"))?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }

    let mut request = Request { method: method.to_string(), target: target.to_string(), headers, body: Vec::new() };
    let length = match request.header("content-length") {
        Some(text) => text.parse::<usize>().map_err(|_| refuse(400, "Content-Length is not valid"))?,
        None => 0,
    };
    if length > MAX_BODY {
        return Err(refuse(413, "The request is too large"));
    }
    request.body = vec![0u8; length];
    reader
        .read_exact(&mut request.body)
        .map_err(|_| refuse(400, "The request body ended early"))?;
    Ok(request)
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Entity",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}

/// The development pages that may call the API from another port. The same origin needs no header
pub const DEV_ORIGINS: [&str; 4] = ["http://localhost:5173", "http://localhost:3000", "http://127.0.0.1:5173", "http://127.0.0.1:3000"];

/// The origin to allow for this request, if it is one of the development pages
pub fn allowed_origin(request: &Request) -> Option<&'static str> {
    let origin = request.header("origin")?;
    DEV_ORIGINS.iter().copied().find(|allowed| *allowed == origin)
}

fn write_reply(stream: &mut impl Write, reply: &Reply, origin: Option<&str>) -> std::io::Result<()> {
    let cors = match origin {
        Some(origin) => format!(
            "Access-Control-Allow-Origin: {origin}\r\nAccess-Control-Allow-Methods: GET, POST, PATCH, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nVary: Origin\r\n"
        ),
        None => String::new(),
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{cors}Connection: close\r\n\r\n",
        reply.status,
        reason(reply.status),
        reply.content_type,
        reply.body.len()
    )?;
    stream.write_all(&reply.body)
}

/// The content type of a file, from its extension
pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()).unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// The file a web address asks for, inside the web folder. Anything that would leave the folder is refused, and
/// an address with no file extension that is not a file gets the app's page, for the app's own routes
pub fn web_file(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split('?').next().unwrap_or_default();
    let mut file = root.to_path_buf();
    for part in path.split('/').filter(|part| !part.is_empty()) {
        let component = Path::new(part).components().next();
        if part.contains('\\') || !matches!(component, Some(Component::Normal(_))) {
            return None;
        }
        file.push(part);
    }
    let base = root.canonicalize().ok()?;
    if file.is_dir() {
        file.push("index.html");
    }
    if file.is_file() {
        let resolved = file.canonicalize().ok()?;
        return resolved.starts_with(&base).then_some(resolved);
    }
    // A route inside the app, such as /editor, is answered by the page itself
    let has_extension = Path::new(path).extension().is_some();
    (!has_extension).then(|| base.join("index.html")).filter(|index| index.is_file())
}

/// Answer one request: the API under /api, the web UI everywhere else
pub fn answer(app: &Arc<AppState>, web: Option<&Path>, request: &Request) -> Reply {
    let (path, query) = request.target.split_once('?').unwrap_or((request.target.as_str(), ""));
    if path == "/api" || path.starts_with("/api/") || path == "/health" {
        return handle(app, &Call {
            method: &request.method,
            path,
            query,
            body: &request.body,
            content_type: request.header("content-type").unwrap_or_default(),
        });
    }
    if request.method != "GET" && request.method != "HEAD" {
        return Reply::json(405, serde_json::json!({ "detail": "Method not allowed" }));
    }
    let Some(root) = web else {
        return Reply::json(404, serde_json::json!({ "detail": "The web UI is not built. Run npm run build in frontend/" }));
    };
    match web_file(root, path).and_then(|file| std::fs::read(&file).ok().map(|bytes| (file, bytes))) {
        Some((file, bytes)) => Reply::file(content_type(&file), bytes),
        None => Reply::json(404, serde_json::json!({ "detail": "Not found" })),
    }
}

fn handle_connection(app: &Arc<AppState>, web: Option<&Path>, stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    });
    let mut out = stream;
    let (reply, origin) = match read_request(&mut reader) {
        Ok(request) => {
            let origin = allowed_origin(&request);
            let reply = if request.method == "OPTIONS" { Reply::empty(204) } else { answer(app, web, &request) };
            (reply, origin)
        }
        Err(refused) => (Reply::json(refused.status, serde_json::json!({ "detail": refused.detail })), None),
    };
    let _ = write_reply(&mut out, &reply, origin);
}

/// Serve requests until the listener fails. Each connection is answered on its own thread
pub fn run(listener: TcpListener, app: Arc<AppState>, web: Option<PathBuf>) -> std::io::Result<()> {
    for stream in listener.incoming() {
        let stream = stream?;
        let app = app.clone();
        let web = web.clone();
        std::thread::spawn(move || handle_connection(&app, web.as_deref(), stream));
    }
    Ok(())
}
