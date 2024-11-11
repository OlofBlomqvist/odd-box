use std::str;

pub struct ParsedHttpRequest {
    pub host: String,
    pub is_h2c_upgrade: bool,
}

pub fn parse_http_request_fast(request: &[u8]) -> anyhow::Result<ParsedHttpRequest> {
    let mut host = None;
    let mut is_h2c_upgrade = false;

    // Flags for h2c detection
    let mut has_upgrade_h2c = false;
    let mut has_connection_upgrade = false;
    let mut has_http2_settings = false;

    let mut pos = 0;
    let len = request.len();

    while pos < len {
        // Find the end of the line
        let line_end = match memchr::memchr(b'\n', &request[pos..]) {
            Some(idx) => pos + idx + 1, // Include the newline character
            None => len,
        };

        // Extract the line
        let line = &request[pos..line_end];

        // Check for empty line (end of headers)
        if line == b"\r\n" || line == b"\n" {
            break;
        }

        // Trim trailing CRLF
        let line = if line.ends_with(b"\r\n") {
            &line[..line.len() - 2]
        } else if line.ends_with(b"\n") {
            &line[..line.len() - 1]
        } else {
            line
        };

        // Skip leading whitespace
        let line = trim_start(line);

        // Process headers
        if line.len() >= 5 && eq_ignore_ascii_case(&line[..5], b"Host:") {
            let value = get_header_value(&line[5..]);
            host = Some(value.to_string());
        } else if line.len() >= 8 && eq_ignore_ascii_case(&line[..8], b"Upgrade:") {
            let value = get_header_value(&line[8..]);
            if eq_ignore_ascii_case(value.as_bytes(), b"h2c") {
                has_upgrade_h2c = true;
            }
        } else if line.len() >= 11 && eq_ignore_ascii_case(&line[..11], b"Connection:") {
            let value = get_header_value(&line[11..]);
            if contains_ignore_ascii_case(value.as_bytes(), b"upgrade") {
                has_connection_upgrade = true;
            }
        } else if line.len() >= 15 && eq_ignore_ascii_case(&line[..15], b"HTTP2-Settings:") {
            has_http2_settings = true;
        }

        // Move to the next line
        pos = line_end;
    }

    is_h2c_upgrade = has_upgrade_h2c && has_connection_upgrade && has_http2_settings;
    if let Some(host) = host {
        Ok(ParsedHttpRequest { host, is_h2c_upgrade })
    } else {
        bail!("no host header found");
    }
}

use anyhow::bail;


/// Trims leading ASCII whitespace from a byte slice.
fn trim_start(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    &bytes[start..]
}

/// Retrieves the header value from a header line, trimming leading and trailing whitespace.
fn get_header_value(bytes: &[u8]) -> &str {
    // Skip colon and whitespace
    let mut pos = 0;
    while pos < bytes.len() && (bytes[pos] == b':' || bytes[pos].is_ascii_whitespace()) {
        pos += 1;
    }
    let value = &bytes[pos..];
    // Remove any trailing whitespace
    let value = trim_end(value);
    str::from_utf8(value).unwrap_or("")
}

/// Trims trailing ASCII whitespace from a byte slice.
fn trim_end(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[..end]
}

/// Compares two byte slices for ASCII case-insensitive equality.
fn eq_ignore_ascii_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(ac, bc)| ac.eq_ignore_ascii_case(bc))
}

/// Checks if a byte slice contains another byte slice, case-insensitive.
fn contains_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|window| eq_ignore_ascii_case(window, needle))
}
