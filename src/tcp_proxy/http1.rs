pub (crate) fn is_valid_http_request(bytes: &[u8]) -> bool {
    // Convert bytes to string for easy manipulation; HTTP is ASCII based.
    let request_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return false, // Not valid UTF-8, unlikely to be a valid HTTP request.
    };

    // HTTP methods to check against.
    let methods = [
        "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "CONNECT",
    ];

    // Check if the request starts with a known HTTP method followed by a space.
    let valid_start = methods
        .iter()
        .any(|&method| request_str.starts_with(&format!("{method} /")));

    // Check if the request contains a valid HTTP version.
    let valid_version =
        request_str.contains("HTTP/1.1\r\n") || request_str.contains("HTTP/1.0\r\n");

    // Minimum validation for headers: at least one CRLF should be present after the initial request line.
    let has_headers = request_str
        .splitn(2, "\r\n")
        .nth(1)
        .map_or(false, |s| s.contains("\r\n"));

    // The request is considered valid if it starts with a known method, contains a valid HTTP version, and has headers.
    valid_start && valid_version && has_headers
}

pub (crate) fn try_decode_http_host (http_request: &str) -> Option<String> {
    // Split the request into lines
    let lines: Vec<&str> = http_request.split("\r\n").collect();
    // Iterate through each line to find the Host header
    for line in lines {
        if line.to_lowercase().starts_with("host:") {
            // Extract the value part of the Host header
            let parts: Vec<&str> = line.splitn(2, ": ").collect();
            if parts.len() == 2 {
                return Some(parts[1].to_string());
            }
        }
    }
    None
}
