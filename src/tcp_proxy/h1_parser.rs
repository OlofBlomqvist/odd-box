/*

    TODO: Might want to move this to a separate crate at some point... its getting out of hand..

*/

use std::{collections::VecDeque, fmt};
use httparse::{Request, Response, Status};
use serde::Serialize;
use std::str;

/// Represents different types of parsed HTTP and WebSocket data.
#[derive(Debug,Clone,Serialize)]
pub enum HttpData {
    Request(ParsedRequest),
    Response(ParsedResponse),
    WebSocketSend(WebSocketFrame),
    WebSocketReceive(WebSocketFrame),
    WebSocketOpened,
    WebSocketClosed,
    ProtocolSwitchedToWebSocket, // <-- dont care about other protos for now.. could probably have been ProtocolSwitched(String) tho..
}

/// Structure representing a parsed HTTP request.
#[derive(Debug,Clone,Serialize)]
pub struct ParsedRequest {
    method: String,
    path: String,
    version: u8,
    headers: Vec<(String, String)>,
    #[serde(serialize_with = "crate::serde_with::serialize_option_bytes_as_string")]
    body: Option<Vec<u8>>,
}

/// Structure representing a parsed HTTP response.
#[derive(Debug,Clone,Serialize)]
pub struct ParsedResponse {
    version: u8,
    status_code: u16,
    reason: String,
    headers: Vec<(String, String)>,
    #[serde(serialize_with = "crate::serde_with::serialize_option_bytes_as_string")]
    body: Option<Vec<u8>>,
}

/// Structure representing a parsed WebSocket frame.
#[derive(Debug, Clone,Serialize)]
pub struct WebSocketFrame {
    fin: bool,
    opcode: u8,
    masked: bool,
    #[serde(serialize_with = "crate::serde_with::serialize_bytes_as_string")]
    payload: Vec<u8>,
}

/// Enum to track the current state of the connection.
#[derive(Default,Debug,Clone,Serialize)]
pub enum ConnectionState {
    #[default] Http,             // Initial state: parsing HTTP
    UpgradeRequested, // Detected HTTP request for WebSocket upgrade
    WebSocket,        // Connection has been upgraded to WebSocket
}

/// Enum to specify the direction of data flow.
pub enum DataDirection {
    ClientToServer, // Data sent from client to server
    ServerToClient, // Data sent from server to client
}



impl fmt::Display for HttpData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpData::Request(req) => write!(f, "HTTP Request:\n{}", req),
            HttpData::Response(resp) => write!(f, "HTTP Response:\n{}", resp),
            HttpData::WebSocketSend(frame) => write!(f, "WebSocket Send Frame:\n{}", frame),
            HttpData::WebSocketReceive(frame) => write!(f, "WebSocket Receive Frame:\n{}", frame),
            HttpData::WebSocketOpened => write!(f, "WebSocket connection opened."),
            HttpData::WebSocketClosed => write!(f, "WebSocket connection closed."),
            HttpData::ProtocolSwitchedToWebSocket => write!(f, "Protocol switched to WebSocket."),
        }
    }
}

impl fmt::Display for ParsedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {} HTTP/1.{}", self.method, self.path, self.version)?;
        for (name, value) in &self.headers {
            writeln!(f, "{}: {}", name, value)?;
        }
        writeln!(f)?;
        if let Some(body) = &self.body {
            match bytes_to_string(body) {
                Some(text) => writeln!(f, "{}", text),
                None => write!(f, "{:?}", body),
            }
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for ParsedResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "HTTP/1.{} {} {}", self.version, self.status_code, self.reason)?;
        for (name, value) in &self.headers {
            writeln!(f, "{}: {}", name, value)?;
        }
        writeln!(f)?;
        if let Some(body) = &self.body {
            match bytes_to_string(body) {
                Some(text) => writeln!(f, "{}", text),
                None => write!(f, "{:?}", body),
            }
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for WebSocketFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "FIN: {}", self.fin)?;
        writeln!(f, ", Opcode: {}", self.opcode)?;
        writeln!(f, ", Masked: {}", self.masked)?;
        writeln!(f, ", Payload Length: {}", self.payload.len())?;
        if !self.payload.is_empty() {
            match bytes_to_string(&self.payload) {
                Some(text) => writeln!(f, ", Payload: {}", text),
                None => writeln!(f, ", Payload: {:?}", self.payload),
            }
        } else {
            Ok(())
        }
    }
}

fn bytes_to_string(bytes: &[u8]) -> Option<String> {
    match str::from_utf8(bytes) {
        Ok(s) => Some(s.to_string()),
        Err(_) => None,
    }
}


/// The main Parser structure managing buffers and state.
#[derive(Default,Debug)]
pub struct H1Observer {
    client_to_server: VecDeque<u8>, // Buffer for client-to-server data
    server_to_client: VecDeque<u8>, // Buffer for server-to-client data
    state: ConnectionState,         // Current connection state
}

impl H1Observer {
    pub fn push(&mut self, direction: DataDirection, data: &[u8]) {
        match direction {
            DataDirection::ClientToServer => {
                self.client_to_server.extend(data);
            }
            DataDirection::ServerToClient => {
                self.server_to_client.extend(data);
            }
        }
    }

    pub fn parse(&mut self) -> Vec<HttpData> {
        let mut events = Vec::new();

        match self.state {
            ConnectionState::Http => {
                // Parse HTTP Requests from client_to_server
                loop {
                    match parse_http_request(&self.client_to_server) {
                        Ok((req, consumed)) => {
                            events.push(HttpData::Request(req));

                            // Check for WebSocket upgrade in the request
                            if is_websocket_upgrade(&events.last().unwrap()) {
                                events.push(HttpData::ProtocolSwitchedToWebSocket);
                                self.state = ConnectionState::UpgradeRequested;
                            }

                            // Consume the parsed bytes from the buffer
                            for _ in 0..consumed {
                                self.client_to_server.pop_front();
                            }
                        }
                        Err(_) => {
                            // No complete request found
                            break;
                        }
                    }
                }

                // Parse HTTP Responses from server_to_client
                loop {
                    match parse_http_response(&self.server_to_client) {
                        Ok((resp, consumed)) => {
                            events.push(HttpData::Response(resp));

                            // If an upgrade was requested, and the response acknowledges it
                            if let Some(last_event) = events.last() {
                                if matches!(last_event, HttpData::Response(_)) && is_websocket_upgrade(last_event) {
                                    events.push(HttpData::WebSocketOpened);
                                    self.state = ConnectionState::WebSocket;
                                }
                            }

                            // Consume the parsed bytes from the buffer
                            for _ in 0..consumed {
                                self.server_to_client.pop_front();
                            }
                        }
                        Err(_) => {
                            // No complete response found
                            break;
                        }
                    }
                }
            }
            ConnectionState::UpgradeRequested => {
                // Awaiting the server's response to complete the upgrade
                loop {
                    match parse_http_response(&self.server_to_client) {
                        Ok((resp, consumed)) => {
                            events.push(HttpData::Response(resp));

                            // If the response confirms the WebSocket upgrade
                            if is_websocket_upgrade(&events.last().unwrap()) {
                                events.push(HttpData::WebSocketOpened);
                                self.state = ConnectionState::WebSocket;
                            }

                            // Consume the parsed bytes from the buffer
                            for _ in 0..consumed {
                                self.server_to_client.pop_front();
                            }
                        }
                        Err(_) => {
                            // No complete response found
                            break;
                        }
                    }
                }
            }
            ConnectionState::WebSocket => {
                // Parse WebSocket frames from client_to_server as WebSocketSend
                loop {
                    let buf = self.client_to_server.make_contiguous();
                    match parse_websocket_frame(buf) {
                        Ok((frame, consumed)) => {
                            events.push(HttpData::WebSocketSend(frame.clone()));

                            // Handle close frames
                            if frame.opcode == 0x8 {
                                events.push(HttpData::WebSocketClosed);
                                self.state = ConnectionState::Http; // Reset to HTTP or handle closure
                            }

                            // Consume the parsed bytes from the buffer
                            for _ in 0..consumed {
                                self.client_to_server.pop_front();
                            }
                        }
                        Err(_) => {
                            // No complete frame found
                            break;
                        }
                    }
                }

                // Parse WebSocket frames from server_to_client as WebSocketReceive
                loop {
                    let buf = self.server_to_client.make_contiguous();
                    match parse_websocket_frame(buf) {
                        Ok((frame, consumed)) => {
                            events.push(HttpData::WebSocketReceive(frame.clone()));

                            // Handle close frames
                            if frame.opcode == 0x8 {
                                events.push(HttpData::WebSocketClosed);
                                self.state = ConnectionState::Http; // Reset to HTTP or handle closure
                            }

                            // Consume the parsed bytes from the buffer
                            for _ in 0..consumed {
                                self.server_to_client.pop_front();
                            }
                        }
                        Err(_) => {
                            // No complete frame found
                            break;
                        }
                    }
                }
            }
        }

        events
    }
}

/// Parses an HTTP request from the given buffer.
/// Returns the parsed `ParsedRequest` and the number of bytes consumed.
fn parse_http_request(buffer: &VecDeque<u8>) -> Result<(ParsedRequest, usize), &'static str> {
    let buf: Vec<u8> = buffer.iter().cloned().collect();
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = Request::new(&mut headers);

    match req.parse(&buf) {
        Ok(Status::Complete(n)) => {
            let method = req.method.unwrap_or("").to_string();
            let path = req.path.unwrap_or("").to_string();
            let version = req.version.unwrap_or(1);

            let mut parsed_headers = Vec::new();
            for h in req.headers.iter() {
                let name = h.name.to_string();
                let value = match str::from_utf8(h.value) {
                    Ok(v) => v.to_string(),
                    Err(_) => "<invalid UTF-8>".to_string(),
                };
                parsed_headers.push((name, value));
            }

            // Determine body length based on Content-Length
            let mut body = None;
            let mut content_length = 0;
            for (name, value) in &parsed_headers {
                if name.eq_ignore_ascii_case("Content-Length") {
                    if let Ok(cl) = value.parse::<usize>() {
                        content_length = cl;
                    }
                    break;
                }
            }

            // Check if buffer has enough data for the body
            if buf.len() < n + content_length {
                return Err("Incomplete body");
            }

            if content_length > 0 {
                let body_slice = &buf[n..n + content_length];
                body = Some(body_slice.to_vec());
            }

            let parsed_request = ParsedRequest {
                method,
                path,
                version,
                headers: parsed_headers,
                body,
            };

            Ok((parsed_request, n + content_length))
        }
        Ok(Status::Partial) => Err("Incomplete request"),
        Err(_) => Err("Failed to parse request"),
    }
}

/// Parses an HTTP response from the given buffer.
/// Returns the parsed `ParsedResponse` and the number of bytes consumed.
fn parse_http_response(buffer: &VecDeque<u8>) -> Result<(ParsedResponse, usize), &'static str> {
    let buf: Vec<u8> = buffer.iter().cloned().collect();
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut resp = Response::new(&mut headers);

    match resp.parse(&buf) {
        Ok(Status::Complete(n)) => {
            let version = resp.version.unwrap_or(1);
            let status_code = resp.code.unwrap_or(200); // todo - this is ... cheating to say the least ; we should return error instead
            let reason = resp.reason.unwrap_or("").to_string();

            let mut parsed_headers = Vec::new();
            for h in resp.headers.iter() {
                let name = h.name.to_string();
                let value = match str::from_utf8(h.value) {
                    Ok(v) => v.to_string(),
                    Err(_) => "<invalid UTF-8>".to_string(),
                };
                parsed_headers.push((name, value));
            }

            // Determine body length based on Content-Length
            let mut body = None;
            let mut content_length = 0;
            for (name, value) in &parsed_headers {
                if name.eq_ignore_ascii_case("Content-Length") {
                    if let Ok(cl) = value.parse::<usize>() {
                        content_length = cl;
                    }
                    break;
                }
            }

            // Check if buffer has enough data for the body
            if buf.len() < n + content_length {
                return Err("Incomplete body");
            }

            if content_length > 0 {
                let body_slice = &buf[n..n + content_length];
                body = Some(body_slice.to_vec());
            }

            let parsed_response = ParsedResponse {
                version,
                status_code,
                reason,
                headers: parsed_headers,
                body,
            };

            Ok((parsed_response, n + content_length))
        }
        Ok(Status::Partial) => Err("Incomplete response"),
        Err(_) => Err("Failed to parse response"),
    }
}

/// Parses a WebSocket frame from the given byte slice.
/// Returns the parsed `WebSocketFrame` and the number of bytes consumed.
fn parse_websocket_frame(buf: &[u8]) -> Result<(WebSocketFrame, usize), &'static str> {
    if buf.len() < 2 {
        return Err("Incomplete frame header");
    }

    let first_byte = buf[0];
    let fin = (first_byte & 0x80) != 0;
    let opcode = first_byte & 0x0F;

    let second_byte = buf[1];
    let masked = (second_byte & 0x80) != 0;
    let mut payload_len = (second_byte & 0x7F) as usize;

    let mut index = 2;

    // Determine payload length
    if payload_len == 126 {
        if buf.len() < index + 2 {
            return Err("Incomplete extended payload length (126)");
        }
        payload_len = ((buf[index] as usize) << 8) | (buf[index + 1] as usize);
        index += 2;
    } else if payload_len == 127 {
        if buf.len() < index + 8 {
            return Err("Incomplete extended payload length (127)");
        }
        payload_len = 0;
        for i in 0..8 {
            payload_len = (payload_len << 8) | (buf[index + i] as usize);
        }
        index += 8;
    }

    // Extract masking key if present
    let masking_key = if masked {
        if buf.len() < index + 4 {
            return Err("Incomplete masking key");
        }
        let key = [buf[index], buf[index + 1], buf[index + 2], buf[index + 3]];
        index += 4;
        Some(key)
    } else {
        None
    };

    // Check if the buffer has enough data for the payload
    if buf.len() < index + payload_len {
        return Err("Incomplete payload data");
    }

    let mut payload = buf[index..index + payload_len].to_vec();
    if let Some(key) = masking_key {
        for i in 0..payload_len {
            payload[i] ^= key[i % 4];
        }
    }

    let frame = WebSocketFrame {
        fin,
        opcode,
        masked,
        payload,
    };

    Ok((frame, index + payload_len))
}

/// Checks if the given `HttpData` event represents a WebSocket upgrade.
fn is_websocket_upgrade(event: &HttpData) -> bool {
    match event {
        HttpData::Request(req) => {
            req.headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case("Upgrade") && value.eq_ignore_ascii_case("websocket")
            }) && req.headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case("Connection") && value.to_lowercase().contains("upgrade")
            })
        }
        HttpData::Response(resp) => {
            resp.status_code == 101
                && resp.headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("Upgrade") && value.eq_ignore_ascii_case("websocket")
                })
                && resp.headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("Connection") && value.to_lowercase().contains("upgrade")
                })
        }
        _ => false,
    }
}