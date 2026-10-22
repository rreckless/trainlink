//! The HTTP server for the web UI. It serves the API at /api, and the built web UI from a folder, from the same
//! address. Requests are read with size limits, so a client cannot exhaust memory with a long request

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
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
        401 => "Unauthorized",
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

/// The name of the cookie that keeps a device signed in to the server
pub const TOKEN_COOKIE: &str = "trainlink_token";

/// Who may reach the server. Only this computer can reach a loopback address, so it needs no token. Any other address
/// is reachable from the network, so each device must sign in with the token
#[derive(Clone)]
pub struct Access {
    pub loopback: bool,
    pub token: String,
}

/// The host name from a Host header: "127.0.0.1:8000" gives "127.0.0.1", and "[::1]:8000" gives "::1"
fn host_name(value: &str) -> &str {
    match value.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or_default(),
        None => value.split(':').next().unwrap_or_default(),
    }
}

/// Whether the request names this server by an address. A host name other than localhost is refused: a page on
/// another site can only reach this server under its own name by rebinding that name to this address
fn host_allowed(request: &Request, access: &Access) -> bool {
    let Some(host) = request.header("host") else { return false };
    let name = host_name(host);
    if name.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match name.parse::<IpAddr>() {
        Ok(ip) => !access.loopback || ip.is_loopback(),
        Err(_) => false,
    }
}

/// Whether the page that sent the request is this server's own page, or a development page. A page on another site
/// is refused, so a web page cannot send commands from the browser
fn origin_allowed(request: &Request) -> bool {
    let Some(origin) = request.header("origin") else { return true };
    DEV_ORIGINS.contains(&origin) || request.header("host").is_some_and(|host| origin == format!("http://{host}"))
}

/// Writes must say what they send. A plain form from another site cannot make one of these
fn body_type_allowed(request: &Request) -> bool {
    let writes = matches!(request.method.as_str(), "POST" | "PATCH" | "PUT" | "DELETE");
    if !writes || request.body.is_empty() {
        return true;
    }
    let kind = request.header("content-type").unwrap_or_default().split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
    kind == "application/json" || kind == "multipart/form-data" || kind.starts_with("image/")
}

/// Whether the target is part of the API
fn is_api(target: &str) -> bool {
    let path = target.split('?').next().unwrap_or_default();
    path == "/api" || path.starts_with("/api/")
}

/// Compares the token in constant time, so the answer does not show how much of it was right
fn same_token(given: &str, token: &str) -> bool {
    !token.is_empty() && given.len() == token.len() && given.bytes().zip(token.bytes()).fold(0, |diff, (a, b)| diff | (a ^ b)) == 0
}

/// Whether the request is signed in: with the Authorization header, or with the cookie that signing in sets
fn signed_in(request: &Request, access: &Access) -> bool {
    if access.loopback {
        return true;
    }
    let bearer = request.header("authorization").and_then(|value| value.strip_prefix("Bearer ")).unwrap_or_default();
    let prefix = format!("{TOKEN_COOKIE}=");
    let cookie = request
        .header("cookie")
        .and_then(|cookies| cookies.split(';').map(str::trim).find_map(|pair| pair.strip_prefix(prefix.as_str())))
        .unwrap_or_default();
    same_token(bearer, &access.token) || same_token(cookie, &access.token)
}

/// The checks every request passes before it is answered
fn guard(request: &Request, access: &Access) -> Result<(), Refused> {
    if !host_allowed(request, access) {
        return Err(refuse(403, "This address is not allowed"));
    }
    if !origin_allowed(request) {
        return Err(refuse(403, "Requests from other sites are refused"));
    }
    if !body_type_allowed(request) {
        return Err(refuse(415, "A write must send JSON or a form"));
    }
    if is_api(&request.target) && request.method != "OPTIONS" && !signed_in(request, access) {
        return Err(refuse(401, "Sign in with the server token"));
    }
    Ok(())
}

/// Signs a device in with the token. The cookie it sets is HttpOnly and SameSite=Strict, so pages cannot read it
/// and other sites cannot send it
fn sign_in(request: &Request, access: &Access) -> (Reply, Option<String>) {
    if access.loopback {
        return (Reply::empty(204), None);
    }
    let given = crate::fields::object(&request.body)
        .ok()
        .and_then(|body| body.get("token").and_then(|value| value.as_str().map(str::to_string)))
        .unwrap_or_default();
    if !same_token(&given, &access.token) {
        return (Reply::json(401, serde_json::json!({ "detail": "The token is not right" })), None);
    }
    (Reply::empty(204), Some(format!("{TOKEN_COOKIE}={given}; HttpOnly; SameSite=Strict; Path=/")))
}

pub(crate) fn write_reply(stream: &mut impl Write, reply: &Reply, origin: Option<&str>, cookie: Option<&str>) -> std::io::Result<()> {
    let cors = match origin {
        Some(origin) => format!(
            "Access-Control-Allow-Origin: {origin}\r\nAccess-Control-Allow-Credentials: true\r\nAccess-Control-Allow-Methods: GET, POST, PATCH, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nVary: Origin\r\n"
        ),
        None => String::new(),
    };
    let set_cookie = cookie.map_or(String::new(), |cookie| format!("Set-Cookie: {cookie}\r\n"));
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{cors}{set_cookie}Connection: close\r\n\r\n",
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
    if path == "/auth/callback" && request.method == "GET" {
        return crate::auth::google::callback(app, query, true);
    }
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

fn handle_connection(app: &Arc<AppState>, web: Option<&Path>, access: &Access, stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    });
    let mut out = stream;
    let (reply, origin, cookie) = match read_request(&mut reader) {
        Ok(request) => {
            let origin = allowed_origin(&request);
            match guard(&request, access) {
                Err(refused) => (Reply::json(refused.status, serde_json::json!({ "detail": refused.detail })), origin, None),
                Ok(()) if request.method == "OPTIONS" => (Reply::empty(204), origin, None),
                Ok(()) if request.method == "POST" && request.target == "/login" => {
                    let (reply, cookie) = sign_in(&request, access);
                    (reply, origin, cookie)
                }
                Ok(()) => (answer(app, web, &request), origin, None),
            }
        }
        Err(refused) => (Reply::json(refused.status, serde_json::json!({ "detail": refused.detail })), None, None),
    };
    let _ = write_reply(&mut out, &reply, origin, cookie.as_deref());
}

/// Serve requests until the listener fails. Each connection is answered on its own thread
pub fn run(listener: TcpListener, app: Arc<AppState>, web: Option<PathBuf>, access: Access) -> std::io::Result<()> {
    for stream in listener.incoming() {
        let stream = stream?;
        let app = app.clone();
        let web = web.clone();
        let access = access.clone();
        std::thread::spawn(move || handle_connection(&app, web.as_deref(), &access, stream));
    }
    Ok(())
}
