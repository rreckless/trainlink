//! The web server: request limits, the path checks, and real requests over a local socket

use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use crate::server::{read_request, run, web_file, Refused, MAX_BODY};

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
    std::thread::spawn(move || {
        let _ = run(listener, app, Some(web));
    });
    port
}

/// Send one raw request and return the status line and the body
fn send(port: u16, request: &str) -> (String, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.lines().next().unwrap_or_default().to_string();
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
    (status, body)
}

#[test]
fn a_request_is_read_with_its_headers_and_body() {
    let text = "POST /api/layouts HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\n\r\n{}";
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
    let (status, body) = send(port, "GET / HTTP/1.1\r\nHost: x\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert_eq!(body, "<html>app</html>");

    let (status, body) = send(port, "GET /assets/app.js HTTP/1.1\r\nHost: x\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert_eq!(body, "console.log(1)");

    let (status, _) = send(port, "GET /assets/../../secret.txt HTTP/1.1\r\nHost: x\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 404 Not Found");

    let json = r#"{"name":"Web","width":800,"height":600,"scale":"N"}"#;
    let post = format!("POST /api/layouts HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{json}", json.len());
    let (status, body) = send(port, &post);
    assert_eq!(status, "HTTP/1.1 201 Created");
    assert!(body.contains("\"name\":\"Web\""));

    let (status, body) = send(port, "GET /api/layouts HTTP/1.1\r\nHost: x\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(body.contains("\"name\":\"Web\""));
}

#[test]
fn writes_to_the_web_folder_are_refused() {
    let port = start(web_folder("writes"));
    let (status, _) = send(port, "POST /assets/app.js HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 405 Method Not Allowed");
}

#[test]
fn only_the_development_pages_may_call_the_api_from_another_port() {
    let port = start(web_folder("cors"));
    let (status, _) = send(port, "OPTIONS /api/layouts HTTP/1.1\r\nHost: x\r\nOrigin: http://localhost:5173\r\nAccess-Control-Request-Method: POST\r\n\r\n");
    assert_eq!(status, "HTTP/1.1 204 No Content");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(b"GET /api/layouts HTTP/1.1\r\nHost: x\r\nOrigin: http://localhost:5173\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.contains("Access-Control-Allow-Origin: http://localhost:5173"));
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(b"GET /api/layouts HTTP/1.1\r\nHost: x\r\nOrigin: https://evil.example\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(!response.contains("Access-Control-Allow-Origin"));
}
