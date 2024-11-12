use anyhow::bail;
use hyper::http::Version;

use super::h1_initial_parser::ParsedHttpRequest;

// note: this method is performance critical, be careful when changing it
// todo: add as many tests as possible to ensure this method is correct and performant
pub fn is_valid_http_request(bytes: &[u8]) -> anyhow::Result<Version> {
    
    const METHODS: &[&[u8]] = &[
        b"GET ", b"POST ", b"PUT ", b"DELETE ", b"HEAD ", b"OPTIONS ", b"PATCH ", b"CONNECT ", b"TRACE ",
    ];

    let mut method_found = false;
    for &method in METHODS {
        if bytes.starts_with(method) {
            method_found = true;
            break;
        }
    }
    if !method_found {
        bail!("this is not a http request. no method found");
    }

    let version = if let Some(_pos) = memchr::memmem::find(bytes, b" HTTP/1.1\r\n") {
        Version::HTTP_11
    } else if let Some(_pos) = memchr::memmem::find(bytes, b" HTTP/1.0\r\n") {
        Version::HTTP_10
    } else if let Some(_pos) = memchr::memmem::find(bytes, b" HTTP/2.0\r\n") {
        Version::HTTP_2
    } else if let Some(_pos) = memchr::memmem::find(bytes, b" HTTP/3.0\r\n") {
        Version::HTTP_3
    } else if let Some(_pos) = memchr::memmem::find(bytes, b" HTTP/0.9\r\n") {
        Version::HTTP_09
    } else {
        if let Some(start) = bytes.windows(6).position(|window| window.starts_with(b"HTTP/")) {
            let end = start + 8; // "HTTP/x.x"
            if end <= bytes.len() {
                let version_str = String::from_utf8_lossy(&bytes[start..end]);
                bail!("unsupported http method: {}", version_str);
            }
        }
        bail!("this is not a http request. no method found");
    };

    let has_headers = memchr::memmem::find(bytes, b"\r\n\r\n").is_some();
    let is_valid = has_headers || matches!(version, Version::HTTP_09); // <-- no headers required in HTTP/0.9

    if is_valid {
        Ok(version)
    } else {
        bail!("invalid http request");
    }
    
}

// note: this method is performance critical, be careful when changing it
// todo: add as many tests as possible to ensure this method is correct and performant
// pub fn try_decode_http_host(http_request: &str) -> Option<String> {
//     for line in http_request.split("\r\n") {
//         if line.len() > 5 && line[..5].eq_ignore_ascii_case("Host:") {
//             if let Some((_, host)) = line.split_once(": ") {
//                 return Some(host.to_string());
//             }
//         }
//     }
//     None
// }
pub fn try_decode_http_host_and_h2c(http_request: &[u8]) -> anyhow::Result<ParsedHttpRequest> {
    super::h1_initial_parser::parse_http_request_fast(http_request)
}