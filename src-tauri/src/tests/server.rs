//! The web server: request limits, the path checks, and real requests over a local socket

use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

use crate::server::{read_request, run, web_file, Access, Refused, MAX_BODY};

use super::app;

/// A web folder with a page, a script, and a file outside it that must never be served
fn web_folder(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("trainlink-web-{}-{name}", std::process::id()));
    let web = root.join("web");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("index.html"), "<html>app</html>").unwrap();
    std::fs::write(web.join("assets/app.js"), "console.log(1)").unwrap();
    std::fs::write(root.join("secret.txt"), "do not serve").unwrap();
    web
}

fn start(web: PathBuf) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = app();
    let access = Access { loopback: true, token: String::new() };
    std::thread::spawn(move || {
        let _ = run(listener, app, Some(web), access);
    });
    port
}

/// Send one raw request and return the whole response
fn exchange(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// Send one raw request and return the status line and the body
fn send(port: u16, request: &str) -> (String, String) {
    let response = exchange(port, request);
    let status = response.lines().next().unwrap_or_default().to_string();
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
    (status, body)
}

#[test]
fn a_request_is_read_with_its_headers_and_body() {
    let text = "POST /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 2\r\n\r\n{}";
    let request = read_request(&mut BufReader::new(text.as_bytes())).unwrap();
    assert_eq!((request.method.as_str(), request.target.as_str()), ("POST", "/api/layouts"));
    assert_eq!(request.header("content-length"), Some("2"));
    assert_eq!(request.body, b"{}");
}

#[test]
fn oversized_and_malformed_requests_are_refused() {
    let big = format!("POST /api HTTP/1.1\r\nContent-Length: {}\r\n\r\n", MAX_BODY + 1);
    assert_eq!(
        read_request(&mut BufReader::new(big.as_bytes())).unwrap_err(),
        Refused { status: 413, detail: "The request is too large" }
    );
    let bad = "GET\r\n\r\n";
    assert_eq!(read_request(&mut BufReader::new(bad.as_bytes())).unwrap_err().status, 400);
    let version = "GET / SPDY/3\r\n\r\n";
    assert_eq!(read_request(&mut BufReader::new(version.as_bytes())).unwrap_err().status, 400);
    let long = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(9000));
    assert_eq!(read_request(&mut BufReader::new(long.as_bytes())).unwrap_err().status, 431);
}

#[test]
fn only_files_inside_the_web_folder_are_served() {
    let web = web_folder("paths");
    assert!(web_file(&web, "/../secret.txt").is_none());
    assert!(web_file(&web, "/assets/../../secret.txt").is_none());
    // Encoded dots are not decoded, so this is a folder name that does not exist
    assert!(web_file(&web, "/assets/%2e%2e/secret.txt").is_none());
    assert!(web_file(&web, "/assets\\..\\secret.txt").is_none());
    assert_eq!(web_file(&web, "/assets/app.js").unwrap().file_name().unwrap(), "app.js");
    // A route inside the app gets the page, and a missing file with an extension does not
    assert_eq!(web_file(&web, "/editor").unwrap().file_name().unwrap(), "index.html");
    assert!(web_file(&web, "/assets/missing.js").is_none());
}

#[test]
fn the_api_and_the_web_ui_answer_on_one_address() {
    let port = start(web_folder("serve"));
    let (status, body) = send(port, "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert_eq!(body, "<html>app</html>");

    let (status, body) = send(port, "GET /assets/app.js HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert_eq!(body, "console.log(1)");

    let (status, _) = send(port, "GET /assets/../../secret.txt HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 404 Not Found");

    let json = r#"{"name":"Web","width":800,"height":600,"scale":"N"}"#;
    let post = format!("POST /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{json}", json.len());
    let (status, body) = send(port, &post);
    assert_eq!(status, "HTTP/1.1 201 Created");
    assert!(body.contains("\"name\":\"Web\""));

    let (status, body) = send(port, "GET /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(body.contains("\"name\":\"Web\""));
}

#[test]
fn writes_to_the_web_folder_are_refused() {
    let port = start(web_folder("writes"));
    let (status, _) = send(port, "POST /assets/app.js HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 405 Method Not Allowed");
}

#[test]
fn only_the_development_pages_may_call_the_api_from_another_port() {
    let port = start(web_folder("cors"));
    let (status, _) = send(port, "OPTIONS /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: http://localhost:5173\r\nAccess-Control-Request-Method: POST\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 204 No Content");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(b"GET /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: http://localhost:5173\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.contains("Access-Control-Allow-Origin: http://localhost:5173"));
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(b"GET /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: https://evil.example\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(!response.contains("Access-Control-Allow-Origin"));
}

#[test]
fn a_host_name_other_than_localhost_is_refused() {
    let port = start(web_folder("rebind"));
    let (status, _) = send(port, "GET / HTTP/1.1\r\nHost: evil.example:80\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 403 Forbidden");
    let (status, _) = send(port, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 200 OK");
}

#[test]
fn a_page_from_another_site_cannot_write() {
    let port = start(web_folder("csrf"));
    let json = r#"{"name":"Attack","width":800,"height":600,"scale":"N"}"#;
    let from_site = format!("POST /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: https://evil.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{json}", json.len());
    assert_eq!(send(port, &from_site).0, "HTTP/1.1 403 Forbidden");
    let plain = format!("POST /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{json}", json.len());
    assert_eq!(send(port, &plain).0, "HTTP/1.1 415 Unsupported Media Type");
    let (_, body) = send(port, "GET /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    assert!(!body.contains("Attack"));
}

#[test]
fn a_device_on_the_network_signs_in_with_the_token() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let access = Access { loopback: false, token: "secret-token".into() };
    let web = web_folder("network");
    std::thread::spawn(move || {
        let _ = run(listener, app(), Some(web), access);
    });

    // The pages are open; the API is not
    assert_eq!(send(port, "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").0, "HTTP/1.1 200 OK");
    assert_eq!(send(port, "GET /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").0, "HTTP/1.1 401 Unauthorized");
    let wrong = "GET /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer wrong\r\n\r\n";
    assert_eq!(send(port, wrong).0, "HTTP/1.1 401 Unauthorized");
    let bearer = "GET /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret-token\r\n\r\n";
    assert_eq!(send(port, bearer).0, "HTTP/1.1 200 OK");

    // Signing in with the wrong token is refused and sets no cookie
    let bad_login = r#"{"token":"wrong"}"#;
    let bad = format!("POST /login HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{bad_login}", bad_login.len());
    let response = exchange(port, &bad);
    assert_eq!(response.lines().next().unwrap(), "HTTP/1.1 401 Unauthorized");
    assert!(!response.contains("Set-Cookie"));

    // Signing in with the right token sets a cookie that the browser keeps for this server
    let login = r#"{"token":"secret-token"}"#;
    let good = format!("POST /login HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{login}", login.len());
    let response = exchange(port, &good);
    assert_eq!(response.lines().next().unwrap(), "HTTP/1.1 204 No Content");
    assert!(response.contains("Set-Cookie: trainlink_token=secret-token; HttpOnly; SameSite=Strict; Path=/"));
    let with_cookie = "GET /api/layouts HTTP/1.1\r\nHost: 127.0.0.1\r\nCookie: theme=dark; trainlink_token=secret-token\r\n\r\n";
    assert_eq!(send(port, with_cookie).0, "HTTP/1.1 200 OK");
}
