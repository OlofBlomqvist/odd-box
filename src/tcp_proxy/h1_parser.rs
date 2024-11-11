use httparse;

#[derive(Debug)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
    pub possibly_body: String
}

pub fn parse_http_requests(data: &[u8]) -> Result<Vec<HttpRequest>, String> {
    let mut requests = Vec::new();
    let mut index = 0;

    while index < data.len() {
        
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);

        match req.parse(&data[index..]) {
            Ok(httparse::Status::Complete(header_len)) => {

                let method = req.method.ok_or("Missing method")?.to_string();
                let path = req.path.ok_or("Missing path")?.to_string();
                let version = format!("HTTP/1.{}", req.version.unwrap_or(1));

                let headers_map = req.headers.iter()
                    .map(|h| (h.name.to_string(), String::from_utf8_lossy(h.value).trim().to_string()))
                    .collect::<std::collections::HashMap<_, _>>();

                let content_length = headers_map.get("Content-Length")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);

                if data.len() < index + header_len + content_length {
                    break; // Incomplete body; wait for more data
                }

                let body = data[index + header_len..index + header_len + content_length].to_vec();
                let possibly_body = String::from_utf8_lossy(&body).to_string();

                let request = HttpRequest {
                    method,
                    path,
                    version,
                    headers: headers_map,
                    body,
                    possibly_body:possibly_body
                };

                tracing::info!("found a request: {:?}", request);

                requests.push(request);

                index += header_len + content_length;
            }
            Ok(httparse::Status::Partial) => {
                break; 
            }
            Err(e) => {
                return Err(format!("Failed to parse HTTP request: {:?}", e));
            }
        }
    }

    Ok(requests)
}

