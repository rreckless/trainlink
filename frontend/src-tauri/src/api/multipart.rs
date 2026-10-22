//! The file part of a multipart form upload, as the browser build sends a picture

/// The content type and bytes of the part named `file`. Returns None if the body is not a form with that part
pub fn file_part(content_type: &str, body: &[u8]) -> Option<(String, Vec<u8>)> {
    let boundary = content_type
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("boundary="))?
        .trim_matches('"');
    let delimiter = format!("--{boundary}").into_bytes();
    let mut position = find(body, &delimiter, 0)? + delimiter.len();
    loop {
        // The part runs from after the delimiter to the next delimiter
        let next = find(body, &delimiter, position)?;
        let part = &body[position..next];
        position = next + delimiter.len();
        let header_end = find(part, b"\r\n\r\n", 0)?;
        let headers = String::from_utf8_lossy(&part[..header_end]).to_string();
        let content = &part[header_end + 4..];
        let content = content.strip_suffix(b"\r\n").unwrap_or(content);
        if headers.contains("name=\"file\"") {
            let kind = headers
                .lines()
                .find_map(|line| line.strip_prefix("Content-Type:").or_else(|| line.strip_prefix("content-type:")))
                .map(|value| value.trim().to_string())
                .unwrap_or_else(|| "application/octet-stream".into());
            return Some((kind, content.to_vec()));
        }
        if body[position..].starts_with(b"--") {
            return None;
        }
    }
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|index| index + from)
}

#[cfg(test)]
mod tests {
    use super::file_part;

    #[test]
    fn finds_the_file_part_and_its_type() {
        let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG\r\n\x1a\nDATA\r\n--XyZ--\r\n";
        let (kind, data) = file_part("multipart/form-data; boundary=XyZ", body).expect("file part");
        assert_eq!(kind, "image/png");
        assert_eq!(data, b"\x89PNG\r\n\x1a\nDATA");
    }

    #[test]
    fn no_file_part_gives_none() {
        let body = b"--XyZ\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nx\r\n--XyZ--\r\n";
        assert!(file_part("multipart/form-data; boundary=XyZ", body).is_none());
        assert!(file_part("image/png", b"\x89PNG").is_none());
    }
}
