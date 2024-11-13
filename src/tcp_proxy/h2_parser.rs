use bytes::{BytesMut, Buf};
use futures::stream::Stream;
use std::collections::HashMap;
use std::task::{Context, Poll, Waker};
use std::pin::Pin;
use hpack_patched::decoder::Decoder;


pub struct H2Observer {
    connection_window_size: u32,
    incoming: H2Buffer,
    outgoing: H2Buffer,
    hpack_decoder: Decoder<'static>,
    streams: HashMap<u32, StreamState>,
}
impl std::fmt::Debug for H2Observer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("H2Observer")
        .field("incoming", &self.incoming)
        .field("outgoing", &self.outgoing)
        .field("streams", &self.streams).finish()
    }
}

// TODO - Should probably make it so that we can rip out the bytes as we transform them in to events
//        instead of having to keep them in memory
impl H2Observer {
    pub fn new() -> Self {
        H2Observer {
            connection_window_size: 65535,
            incoming: H2Buffer::new(),
            outgoing: H2Buffer::new(),
            hpack_decoder: Decoder::new(),
            streams: HashMap::new(),
        }
    }

    pub fn write_incoming(&mut self, data: &[u8]) {
        self.incoming.write(data);
    }
    #[allow(dead_code)]
    pub fn write_outgoing(&mut self, data: &[u8]) {
        self.outgoing.write(data);
    }
    pub fn get_all_events(&mut self) -> Vec<H2Event> {
        let mut events = Vec::new();
        while let Some(event) = self.process_frames(H2FrameDirection::Incoming) {
            events.push(event);
        }
        while let Some(event) = self.process_frames(H2FrameDirection::Outgoing) {
            events.push(event);
        }
        tracing::trace!("Streams: {:?}", self.streams);
        events
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum H2Event {
    Priority {
        stream_id: u32,
        exclusive: bool,
        stream_dependency: u32,
        weight: u8,
    },
    GoAway {
        last_stream_id: u32,
        error_code: u32,
        debug_data: Vec<u8>,
    },
    WindowUpdate {
        direction: H2FrameDirection,
        stream_id: u32,
        window_size_increment: u32,
    },
    Continuation {
        stream_id: u32,
        flags: u8,
        header_block_fragment: Vec<u8>,
    },
    Unknown(H2Frame),
    IncomingRequest(HttpRequest),
    Data {
        stream_id: u32,
        data: Vec<u8>,
        direction: H2FrameDirection,
        end_stream: bool,
    },
    Settings {
        direction: H2FrameDirection,
        flags: u8,
        settings: Vec<(u16, u32)>,
    },
    Ping {
        direction: H2FrameDirection,
        flags: u8,
        opaque_data: [u8; 8],
    },
    PushPromise {
        stream_id: u32,
        promised_stream_id: u32,
        header_block_fragment: Vec<u8>,
    },
}


#[derive(Debug, Clone, Copy)]
pub enum H2FrameDirection {
    Incoming,
    Outgoing,
}

impl Stream for H2Observer {
    type Item = H2Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        tracing::trace!("poll_next called..");
        let this = self.get_mut();

        if let Some(event) = this.process_frames(H2FrameDirection::Incoming) {
            tracing::trace!("returning event: {:?}", event);
            return Poll::Ready(Some(event));
        }

        if let Some(event) = this.process_frames(H2FrameDirection::Outgoing) {
            tracing::trace!("returning event: {:?}", event);
            return Poll::Ready(Some(event));
        }

        tracing::trace!("no frames available..");

        this.incoming.register_waker(cx.waker());
        this.outgoing.register_waker(cx.waker());

        Poll::Pending
    }
}

fn vec_to_string(vec: Vec<u8>) -> String {
    String::from_utf8(vec).unwrap_or_else(|_| {
        // todo: handle invalid UTF-8 ??
        format!("failed to convert to string..")
    })
}
    
impl H2Observer {
    /// Process frames from a buffer (incoming or outgoing)
    fn process_frames(&mut self, direction: H2FrameDirection) -> Option<H2Event> {
        while let Some(frame) = {
            let buffer = if let H2FrameDirection::Incoming = direction {
                &mut self.incoming
            } else {
                &mut self.outgoing
            };
            buffer.read_next_frame()
        } {
            tracing::trace!("processing frame: {:?} in direction: {direction:?}", frame);
            match frame {
                H2Frame::PushPromise { stream_id, promised_stream_id, header_block_fragment } => {
                    tracing::trace!(
                        "Received PUSH_PROMISE: stream_id={}, promised_stream_id={}",
                        stream_id,
                        promised_stream_id
                    );
                    return Some(H2Event::PushPromise {
                        stream_id,
                        promised_stream_id,
                        header_block_fragment,
                    });
                }
                H2Frame::RstStream { stream_id, error_code } => {
                    tracing::trace!(
                        "Received RST_STREAM: stream_id={}, error_code={}",
                        stream_id,
                        error_code
                    );
                    // TODO: Handle the RST_STREAM frame instead of sending UNKNOWN!
                    return Some(H2Event::Unknown(frame));
                }
                H2Frame::Headers { stream_id, flags, payload } => {
                    let end_headers = flags & 0x4 != 0;
                    let end_stream = flags & 0x1 != 0;

                    let state = self.streams.entry(stream_id).or_insert_with(StreamState::new);

                    state.header_blocks.extend_from_slice(&payload);

                    if end_headers {
                        // Decode headers
                        let headers = match self.hpack_decoder.decode(&state.header_blocks) {
                            Ok(hdrs) => hdrs,
                            Err(e) => {
                                tracing::warn!("HPACK decoding error: {:?}", e);
                                continue;
                            }
                        };

                    

                        // Convert headers to HashMap
                        let headers_map = headers
                            .into_iter()
                            .map(|(k, v)| (vec_to_string(k), vec_to_string(v)))
                            .collect::<HashMap<String, String>>();

                        state.headers = Some(headers_map.clone());
                        state.header_blocks.clear();

                        // Determine if this is a regular request or a continuous data stream
                        if Self::is_continuous_stream(&headers_map) {
                            state.is_continuous_stream = true;
                        }
                    }

                    if end_stream && state.is_request_complete() {
                        if state.is_continuous_stream {
                            // For continuous streams, we may not emit an event yet
                        } else {
                            // Assemble the request
                            let request = state.to_request(stream_id);
                            self.streams.remove(&stream_id);

                            return Some(H2Event::IncomingRequest(request));
                        }
                    }
                }
                H2Frame::Data { stream_id, flags, payload } => {
                    let end_stream = flags & 0x1 != 0;
                    let state = self.streams.entry(stream_id).or_insert_with(StreamState::new);

                    // Always collect raw data
                    match direction {
                        H2FrameDirection::Incoming => {
                            state.incoming_data.extend_from_slice(&payload);
                        }
                        H2FrameDirection::Outgoing => {
                            state.outgoing_data.extend_from_slice(&payload);
                        }
                    }

                    // For regular requests, collect body data
                    if !state.is_continuous_stream {
                        state.body.extend_from_slice(&payload);

                        if end_stream && state.is_request_complete() {
                            // Assemble the request
                            let request = state.to_request(stream_id);
                            self.streams.remove(&stream_id);

                            return Some(H2Event::IncomingRequest(request));
                        }
                    }

                    return Some(H2Event::Data {
                        stream_id,
                        data: payload,
                        direction,
                        end_stream,
                    });
                }
                H2Frame::Settings { flags, settings } => {
                    return Some(H2Event::Settings {
                        direction,
                        flags,
                        settings,
                    });
                }
                H2Frame::Ping { flags, opaque_data } => {
                    return Some(H2Event::Ping {
                        direction,
                        flags,
                        opaque_data,
                    });
                }
                H2Frame::WindowUpdate { stream_id, window_size_increment } => {
                    tracing::trace!(
                        "Received WINDOW_UPDATE: stream_id={}, window_size_increment={}",
                        stream_id,
                        window_size_increment
                    );
    
                    if window_size_increment == 0 {
                        tracing::warn!(
                            "Received WINDOW_UPDATE with window_size_increment=0, which is a protocol error... i think.."
                        );
                        continue;
                    }
    
                    if stream_id == 0 {
                        // Update connection-level flow control window
                        self.connection_window_size = self
                            .connection_window_size
                            .checked_add(window_size_increment)
                            .expect("Connection flow control window size overflow");
                    } else {
                        // Update stream-level flow control window
                        let state = self.streams.entry(stream_id).or_insert_with(StreamState::new);
                        state.stream_window_size = state
                            .stream_window_size
                            .checked_add(window_size_increment)
                            .expect("Stream flow control window size overflow");
                    }
    
                    return Some(H2Event::WindowUpdate {
                        direction,
                        stream_id,
                        window_size_increment,
                    });
                }
                H2Frame::Goaway { last_stream_id, error_code, debug_data } => {
                    tracing::trace!(
                        "Received GOAWAY: last_stream_id={}, error_code={}, debug_data={:?}",
                        last_stream_id,
                        error_code,
                        String::from_utf8_lossy(&debug_data)
                    );
                    return Some(H2Event::GoAway { last_stream_id, error_code, debug_data  });
                }
                H2Frame::Unknown { stream_id, flags, payload:_, frame_type } => {
                    tracing::warn!(
                        "Received unknown frame type: {:#x}, stream_id={}, flags={:#x}",
                        frame_type,
                        stream_id,
                        flags
                    );
                    return Some(H2Event::Unknown(frame));
                }
                H2Frame::Continuation { stream_id, flags, header_block_fragment } => {
                    tracing::trace!("Received CONTINUATION: stream_id={}, flags={:#x}", stream_id, flags);
                    return Some(H2Event::Continuation { stream_id, flags, header_block_fragment });
                }
                H2Frame::Priority { stream_id, exclusive, stream_dependency, weight } => {
                    tracing::trace!("Received PRIORITY: stream_id={}, exclusive={}, stream_dependency={}, weight={}", stream_id, exclusive, stream_dependency, weight);
                    return Some(H2Event::Priority { stream_id, exclusive, stream_dependency, weight })
                }
            }
        }
        None
    }

    // todo: not sure this is correct..
    fn is_continuous_stream(headers: &HashMap<String, String>) -> bool {
        // Treat streams as continuous unless certain they're regular requests.. ?
        if let Some(method) = headers.get(":method") {
            tracing::trace!("method: {}", method);
            let regular_methods = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
            if regular_methods.contains(&method.as_str()) {
                return false;
            }
        }
        tracing::trace!("continuous stream detected!");
        true
    }

}

#[derive(Debug)]
struct StreamState {
    headers: Option<HashMap<String, String>>,
    header_blocks: Vec<u8>,
    body: Vec<u8>,
    is_continuous_stream: bool,
    incoming_data: Vec<u8>,
    outgoing_data: Vec<u8>,
    stream_window_size: u32
}

impl StreamState {
    fn new() -> Self {
        StreamState {
            stream_window_size: 65535,
            headers: None,
            header_blocks: Vec::new(),
            body: Vec::new(),
            is_continuous_stream: false,
            incoming_data: Vec::new(),
            outgoing_data: Vec::new(),
        }
    }

    fn is_request_complete(&self) -> bool {
        self.headers.is_some()
    }

    fn to_request(&self, stream_id: u32) -> HttpRequest {
        let headers = self.headers.clone().unwrap_or_default();
        let method = headers
            .get(":method")
            .cloned()
            .unwrap_or_else(|| "GET".to_string());
        let path = headers
            .get(":path")
            .cloned()
            .unwrap_or_else(|| "/".to_string());

        HttpRequest {
            stream_id,
            method,
            path,
            headers,
            body: self.body.clone(),
        }
    }
}

#[derive(Debug)]
struct H2Buffer {
    buffer: BytesMut,
    preface_consumed: bool,
    waker: Option<Waker>,
}

impl H2Buffer {
    pub fn new() -> Self {
        H2Buffer {
            buffer: BytesMut::with_capacity(4096),
            waker: None,
            preface_consumed: false, 
        }
    }
    fn consume_preface(&mut self) -> bool {
        const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        if self.buffer.len() < PREFACE.len() {
            // Not enough data yet
            return false;
        }

        if &self.buffer[..PREFACE.len()] == PREFACE {
            self.buffer.advance(PREFACE.len());
            self.preface_consumed = true;
            tracing::trace!("Connection preface consumed.");
            true
        } else {
            // Invalid preface.. not sure what to do here if anything..
            tracing::warn!("Invalid connection preface.");
            self.buffer.advance(self.buffer.len());
            false
        }
    }
    
    pub fn read_next_frame(&mut self) -> Option<H2Frame> {
        if !self.preface_consumed {
            if !self.consume_preface() {
                return None; // Not enough data to consume preface
            }
        }        
        parse_frame(&mut self.buffer)
    }

    pub fn write(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    pub fn register_waker(&mut self, waker: &Waker) {
        if self.waker.is_none() || !self.waker.as_ref().unwrap().will_wake(waker) {
            self.waker = Some(waker.clone());
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct HttpRequest {
    pub stream_id: u32,
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum H2Frame {
    Data {
        stream_id: u32,
        flags: u8,
        payload: Vec<u8>,
    },
    Headers {
        stream_id: u32,
        flags: u8,
        payload: Vec<u8>,
    },
    Priority {
        stream_id: u32,
        exclusive: bool,
        stream_dependency: u32,
        weight: u8,
    },
    RstStream {
        stream_id: u32,
        error_code: u32,
    },
    Settings {
        flags: u8,
        settings: Vec<(u16, u32)>,
    },
    PushPromise {
        stream_id: u32,
        promised_stream_id: u32,
        header_block_fragment: Vec<u8>,
    },
    Ping {
        flags: u8,
        opaque_data: [u8; 8],
    },
    Goaway {
        last_stream_id: u32,
        error_code: u32,
        debug_data: Vec<u8>,
    },
    WindowUpdate {
        stream_id: u32,
        window_size_increment: u32,
    },
    Continuation {
        stream_id: u32,
        flags: u8,
        header_block_fragment: Vec<u8>,
    },
    Unknown {
        stream_id: u32,
        flags: u8,
        payload: Vec<u8>,
        frame_type: u8,
    },
}

fn parse_frame(buffer: &mut BytesMut) -> Option<H2Frame> {


    // HTTP/2 frame header is 9 bytes
    if buffer.len() < 9 {
        return None; // Not enough data
    }

    // Frame Header:
    // Length (24 bits)
    // Type (8 bits)
    // Flags (8 bits)
    // Reserved (1 bit) + Stream Identifier (31 bits)

    let length = ((buffer[0] as u32) << 16)
        | ((buffer[1] as u32) << 8)
        | (buffer[2] as u32);
    let frame_type = buffer[3];
    let flags = buffer[4];
    let stream_id = ((buffer[5] as u32 & 0x7F) << 24)
        | ((buffer[6] as u32) << 16)
        | ((buffer[7] as u32) << 8)
        | (buffer[8] as u32);


    let total_length = 9 + length as usize;
    if buffer.len() < total_length {
        return None; // Not enough data for the payload
    }

    // Extract the payload
    let payload = buffer[9..total_length].to_vec();

    let frame = match frame_type {
        0x0 => Some(H2Frame::Data {
            stream_id,
            flags,
            payload,
        }),
        0x1 => {
            
            // HEADERS frame
            let mut payload_buf = &payload[..];
            let mut header_block_fragment = Vec::new();
            let mut pad_length = 0;
    
            // Handle PADDED flag
            if flags & 0x8 != 0 {
                if payload_buf.len() < 1 {
                    return None; // Not enough data
                }
                pad_length = payload_buf[0] as usize;
                payload_buf = &payload_buf[1..];
            }

            // Handle PRIORITY flag
            if flags & 0x20 != 0 {
                if payload_buf.len() < 5 {
                    return None; // Not enough data
                }
                // Extract priority fields
                let _exclusive = (payload_buf[0] & 0x80) != 0;
                let _stream_dependency = ((payload_buf[0] as u32 & 0x7F) << 24)
                    | ((payload_buf[1] as u32) << 16)
                    | ((payload_buf[2] as u32) << 8)
                    | (payload_buf[3] as u32);
                let _weight = payload_buf[4];
                payload_buf = &payload_buf[5..];
            }
    
            // Check if payload has enough data after accounting for padding
            if payload_buf.len() < pad_length {
                return None; // Not enough data
            }
    
            let header_block_len = payload_buf.len() - pad_length;
            header_block_fragment.extend_from_slice(&payload_buf[..header_block_len]);

            // Handle CONTINUATION frames if END_HEADERS flag is not set
            if flags & 0x4 == 0 {
                // Collect header fragments from CONTINUATION frames
                if let Some(mut continuation_fragment) = collect_continuation_frames(buffer, stream_id) {
                    header_block_fragment.append(&mut continuation_fragment);
                } else {
                    return None; // Not enough data
                }
            }
    
            Some(H2Frame::Headers {
                stream_id,
                flags,
                payload: header_block_fragment,
            })
        },
        0x2 => {
            // PRIORITY frame
            if payload.len() != 5 {
                return None; // Invalid PRIORITY frame.. wth is this?
            }
            let exclusive = (payload[0] & 0x80) != 0;
            let stream_dependency = ((payload[0] as u32 & 0x7F) << 24)
                | ((payload[1] as u32) << 16)
                | ((payload[2] as u32) << 8)
                | (payload[3] as u32);
            let weight = payload[4];
            Some(H2Frame::Priority {
                stream_id,
                exclusive,
                stream_dependency,
                weight,
            })
        }
        0x3 => {
            // RST_STREAM frame
            if payload.len() != 4 {
                return None; // Invalid RST_STREAM frame??
            }
            let error_code = ((payload[0] as u32) << 24)
                | ((payload[1] as u32) << 16)
                | ((payload[2] as u32) << 8)
                | (payload[3] as u32);
            Some(H2Frame::RstStream {
                stream_id,
                error_code,
            })
        }
        0x4 => {
            // SETTINGS frame
            let mut settings = Vec::new();
            let mut payload_buf = &payload[..];
            while payload_buf.len() >= 6 {
                let identifier = ((payload_buf[0] as u16) << 8) | (payload_buf[1] as u16);
                let value = ((payload_buf[2] as u32) << 24)
                    | ((payload_buf[3] as u32) << 16)
                    | ((payload_buf[4] as u32) << 8)
                    | (payload_buf[5] as u32);
                settings.push((identifier, value));
                payload_buf = &payload_buf[6..];
            }
            Some(H2Frame::Settings { flags, settings })
        }
        0x5 => {
            // PUSH_PROMISE frame
            if payload.len() < 4 {
                return None; // Invalid PUSH_PROMISE frame!?
            }
            let promised_stream_id = ((payload[0] as u32 & 0x7F) << 24)
                | ((payload[1] as u32) << 16)
                | ((payload[2] as u32) << 8)
                | (payload[3] as u32);
            let header_block_fragment = payload[4..].to_vec();
            Some(H2Frame::PushPromise {
                stream_id,
                promised_stream_id,
                header_block_fragment,
            })
        }
        0x6 => {
            // PING frame
            if payload.len() != 8 {
                // Invalid PING frame!?
                return None;
            }
            let mut opaque_data = [0u8; 8];
            opaque_data.copy_from_slice(&payload);
            Some(H2Frame::Ping { flags, opaque_data })
        }
        0x7 => {
            // GOAWAY frame
            if payload.len() < 8 {
                // Invalid GOAWAY frame!??
                return None;
            }
            let last_stream_id = ((payload[0] as u32 & 0x7F) << 24)
                | ((payload[1] as u32) << 16)
                | ((payload[2] as u32) << 8)
                | (payload[3] as u32);
            let error_code = ((payload[4] as u32) << 24)
                | ((payload[5] as u32) << 16)
                | ((payload[6] as u32) << 8)
                | (payload[7] as u32);
            let debug_data = payload[8..].to_vec();
            
            Some(H2Frame::Goaway {
                last_stream_id,
                error_code,
                debug_data,
            })
        }
        0x8 => {
            // WINDOW_UPDATE frame
            if payload.len() != 4 {
                // Invalid WINDOW_UPDATE frame.. ?
                return None;
            }
            let window_size_increment = ((payload[0] as u32 & 0x7F) << 24)
                | ((payload[1] as u32) << 16)
                | ((payload[2] as u32) << 8)
                | (payload[3] as u32);
            
            Some(H2Frame::WindowUpdate {
                stream_id,
                window_size_increment,
            })
        }
        0x9 => {
            // CONTINUATION frame
            let header_block_fragment = payload.to_vec();
            Some(H2Frame::Continuation {
                stream_id,
                flags,
                header_block_fragment,
            })
        }
        _ => {
            Some(H2Frame::Unknown {
                frame_type,
                flags,
                stream_id,
                payload,
            })
        }
    };

    buffer.advance(total_length);
    frame
    
    
}

// todo: clean this up a bit
fn collect_continuation_frames(buffer: &mut BytesMut, expected_stream_id: u32) -> Option<Vec<u8>> {
    let mut header_block_fragment = Vec::new();

    loop {
        if buffer.len() < 9 {
            return None; // Not enough data
        }

        // Parse the continuation frame header
        // todo: this is a bit of a mess.. should make it clearer
        let length = ((buffer[0] as u32) << 16)
            | ((buffer[1] as u32) << 8)
            | (buffer[2] as u32);
        let frame_type = buffer[3];
        let flags = buffer[4];
        let stream_id = ((buffer[5] as u32 & 0x7F) << 24)
            | ((buffer[6] as u32) << 16)
            | ((buffer[7] as u32) << 8)
            | (buffer[8] as u32);

        if frame_type != 0x9 || stream_id != expected_stream_id {
            // Invalid CONTINUATION frame
            return None;
        }

        let total_length = 9 + length as usize;
        if buffer.len() < total_length {
            return None; // Not enough data
        }

        let payload = buffer[9..total_length].to_vec();

        // Advance the buffer for the CONTINUATION frame
        buffer.advance(total_length);

        header_block_fragment.extend_from_slice(&payload);

        if flags & 0x4 != 0 {
            // END_HEADERS flag is set
            break;
        }
    }

    Some(header_block_fragment)
}
