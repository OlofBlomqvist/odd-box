/*

    TODO: Might want to move this to a separate crate at some point... its getting out of hand..

*/

use bytes::{Buf, BytesMut};
use futures::stream::Stream;
use hpack_patched::decoder::Decoder;
use serde::Serialize;
use std::{
    collections::HashMap, fmt, pin::Pin, task::{Context, Poll, Waker}
};

#[derive(Debug, Clone, Copy,Eq,PartialEq,Serialize)]
pub enum H2FrameDirection {
    Incoming,
    Outgoing,
}

/// Main observer struct:  
/// Used for decoding http2 frames in both directions of an observed stream.    
pub struct H2Observer {
    connection_window_size: u32,
    incoming: H2Buffer,
    outgoing: H2Buffer,
    hpack_decoder_incoming: Decoder<'static>, // Separate decoder for incoming.. just to be sure..
    hpack_decoder_outgoing: Decoder<'static>, // Separate decoder for outgoing.. just to be sure.. 
    
    streams: HashMap<u32, StreamState>,

    /// Partial HEADERS that haven’t seen END_HEADERS.  
    /// We track *one* partial state at a time with its direction.  
    // actually... not sure if there can possibly be more than one at a time but this seems to work for now
    partial_headers: Option<PartialHeadersState>, 
    partial_headers_direction: Option<H2FrameDirection>,
}

/// Stores partial HEADERS that we’re still collecting (HEADERS + CONTINUATION).
#[derive(Debug)]
struct PartialHeadersState {
    stream_id: u32,
    flags: u8,
    /// Combined HPACK block so far
    header_block: Vec<u8>,
    /// True if we already parsed PADDED/PRIORITY bits on the HEADERS frame
    #[allow(unused)]
    initial_parsed: bool,
}

/// Simplified HTTP request structure (for `IncomingRequest` / `OutgoingRequest`).
#[derive(Debug,Clone,Serialize)]
pub struct HttpRequest {
    pub stream_id: u32,
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    #[serde(serialize_with = "crate::serde_with::serialize_bytes_as_string")]
    pub body: Vec<u8>,
}

impl std::fmt::Display for H2Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        match self {
            H2Event::IncomingRequest(http_request) => {
                let body_str = match std::str::from_utf8(&http_request.body) {
                    Ok(s) => format!("\"{}\"", s),
                    Err(_) => format!("{:?}", &http_request.body)
                };
                write!(f,
                    "IncomingRequest {{ stream_id: {}, method: \"{}\", path: \"{}\", headers: {:?}, body: {} }}",
                    http_request.stream_id, http_request.method, http_request.path, http_request.headers, body_str
                )
            },
            H2Event::OutgoingRequest(http_request) => {
                let body_str = match std::str::from_utf8(&http_request.body) {
                    Ok(s) => format!("\"{}\"", s),
                    Err(_) => format!("{:?}", &http_request.body)
                };
                write!(f,
                    "OutgoingRequest {{ stream_id: {}, method: \"{}\", path: \"{}\", headers: {:?}, body: {} }}",
                    http_request.stream_id, http_request.method, http_request.path, http_request.headers, body_str
                )
            },
            H2Event::OutgoingResponse(h2_response) => {
                let body_str = match std::str::from_utf8(&h2_response.body) {
                    Ok(s) => format!("\"{}\"", s),
                    Err(_) => format!("{:?}", &h2_response.body)
                };
                write!(f,
                    "OutgoingResponse {{ stream_id: {}, headers: {:?}, status: {:?}, body: {} }}",
                    h2_response.stream_id, h2_response.headers, h2_response.status, body_str
                )
            },
            H2Event::PartialOutgoingResponse(h2_response) => {
                let body_str = match std::str::from_utf8(&h2_response.body) {
                    Ok(s) => format!("\"{}\"", s),
                    Err(_) => format!("{:?}", &h2_response.body)
                };
                write!(f,
                    "PartialOutgoingResponse {{ stream_id: {}, headers: {:?}, status: {:?}, body: {} }}",
                    h2_response.stream_id, h2_response.headers, h2_response.status, body_str
                )
            },
            H2Event::Data { stream_id, data, direction, end_stream } => {
                let data_str = match std::str::from_utf8(&data) {
                    Ok(s) => format!("\"{}\"", s),
                    Err(_) => format!("{:?}", &data)
                };
                write!(f,
                    "Data {{ stream_id: {}, direction: {:?}, end_stream: {:?}, data: {} }}",
                    stream_id, direction, end_stream, data_str
                )
            },
            x => write!(f,"{x:?}")
        }
        
    }
}

/// Minimal HTTP/2 Response structure for the outgoing direction.
#[derive(Debug,Clone,Serialize)]
pub struct H2Response {
    pub stream_id: u32,
    pub status: String,
    pub headers: HashMap<String, String>,
    #[serde(serialize_with = "crate::serde_with::serialize_bytes_as_string")]
    pub body: Vec<u8>,
}

/// Events we can produce after decoding frames..
#[derive(Debug,Clone,Serialize)]
pub enum H2Event {
    /// Received HEADERS with `:method` => we interpret it as an HTTP/2 request.
    IncomingRequest(HttpRequest),

    /// Sent HEADERS with `:method` => treat as an *outgoing* request.
    OutgoingRequest(HttpRequest),

    /// Sent HEADERS with `:status` => treat as a response from us.
    OutgoingResponse(H2Response),

    PartialOutgoingResponse(H2Response),

    OutgoingHeaders {
        stream_id: u32,
        headers: HashMap<String, String>,
    },

    IncomingHeaders {
        stream_id: u32,
        headers: HashMap<String, String>,
    },

    Data {
        stream_id: u32,
        #[serde(serialize_with = "crate::serde_with::serialize_bytes_as_string")]
        data: Vec<u8>,
        direction: H2FrameDirection,
        end_stream: bool,
    },

    Priority {
        stream_id: u32,
        exclusive: bool,
        stream_dependency: u32,
        weight: u8,
    },

    GoAway {
        last_stream_id: u32,
        error_code: u32,
        #[serde(serialize_with = "crate::serde_with::serialize_bytes_as_string")]
        debug_data: Vec<u8>,
    },

    WindowUpdate {
        direction: H2FrameDirection,
        stream_id: u32,
        window_size_increment: u32,
    },

    Settings {
        direction: H2FrameDirection,
        flags: u8,
        settings: Vec<DecodedSettings>
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

    Unknown(H2Frame),
}

/// Represents an individual HTTP/2 Setting.
#[derive(Debug, Clone, PartialEq, Eq,Serialize)]
pub struct DecodedSettings {
    pub identifier: SettingIdentifier,
    pub value: u32,
}

impl fmt::Display for DecodedSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.identifier, self.value)
    }
}

/// Represents known HTTP/2 Settings identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u16)]
pub enum SettingIdentifier {
    HeaderTableSize = 0x1,
    EnablePush = 0x2,
    MaxConcurrentStreams = 0x3,
    InitialWindowSize = 0x4,
    MaxFrameSize = 0x5,
    MaxHeaderListSize = 0x6,
    /// SETTINGS_ENABLE_CONNECT_PROTOCOL (0x8) - from RFC 8441
    EnableConnectProtocol = 0x8,
    /// Unknown (or future settings)
    Unknown(u16),
}
impl SettingIdentifier {
    /// Converts a `u16` identifier into a `SettingIdentifier` enum variant.
    pub fn from_u16(id: u16) -> Self {
        match id {
            0x1 => SettingIdentifier::HeaderTableSize,
            0x2 => SettingIdentifier::EnablePush,
            0x3 => SettingIdentifier::MaxConcurrentStreams,
            0x4 => SettingIdentifier::InitialWindowSize,
            0x5 => SettingIdentifier::MaxFrameSize,
            0x6 => SettingIdentifier::MaxHeaderListSize,
            0x8 => SettingIdentifier::EnableConnectProtocol,
            other => SettingIdentifier::Unknown(other),
        }
    }
}
impl fmt::Display for SettingIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            SettingIdentifier::HeaderTableSize => "Header Table Size",
            SettingIdentifier::EnablePush => "Enable Push",
            SettingIdentifier::MaxConcurrentStreams => "Max Concurrent Streams",
            SettingIdentifier::InitialWindowSize => "Initial Window Size",
            SettingIdentifier::MaxFrameSize => "Max Frame Size",
            SettingIdentifier::MaxHeaderListSize => "Max Header List Size",
            SettingIdentifier::EnableConnectProtocol => "Enable CONNECT Protocol",
            SettingIdentifier::Unknown(id) => return write!(f, "Unknown({})", id),
        };
        write!(f, "{}", name)
    }
}
impl H2Event {
    /// None means this is for the http2 session as a whole, for example Settings flags, ping, goaway etc.
    /// in which case we can just attach it to a specific TCP connection key.
    pub fn stream_id(&self) -> Option<u32> {
        match self {
            H2Event::IncomingRequest(http_request) => Some(http_request.stream_id.clone()),
            H2Event::OutgoingRequest(http_request) =>Some(http_request.stream_id.clone()),
            H2Event::OutgoingResponse(h2_response) => Some(h2_response.stream_id.clone()),
            H2Event::PartialOutgoingResponse(h2_response) => Some(h2_response.stream_id.clone()),
            H2Event::OutgoingHeaders { stream_id, headers:_ } => Some(stream_id.clone()),
            H2Event::IncomingHeaders { stream_id, headers:_ } =>Some(stream_id.clone()),
            H2Event::Data { stream_id, data:_, direction:_, end_stream:_ } => Some(stream_id.clone()),
            H2Event::Priority { stream_id, exclusive:_, stream_dependency:_, weight:_ } => Some(stream_id.clone()),
            H2Event::GoAway { last_stream_id:_, error_code:_, debug_data:_ } => None,
            H2Event::WindowUpdate { direction:_, stream_id, window_size_increment:_ } => Some(stream_id.clone()),
            H2Event::Settings { direction:_, flags:_, settings:_ } => None,
            H2Event::Ping { direction:_, flags:_, opaque_data:_ } => None,
            H2Event::PushPromise { stream_id, promised_stream_id:_, header_block_fragment:_ } => Some(stream_id.clone()),
            H2Event::Unknown(_h2_frame) => None
        }
    }
}

/// Lower-level representation of a frame.
/// Most of these will not be constructed as we do inline parsing and just go straight to a h2event.
/// Leaving here since im thinking it would be nice to parse in to these even if just using as intermediate representations..
#[derive(Debug,Clone,Serialize)]
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
        frame_type: u8,
        stream_id: u32,
        flags: u8,
        payload: Vec<u8>,
    },
}

impl Default for H2Observer {
    fn default() -> Self {
        Self::new()
    }
}

impl H2Observer {
    pub fn new() -> Self {
        H2Observer {
            connection_window_size: 65535,
            // For the incoming buffer, we expect the “PRI * HTTP/2.0” preface
            incoming: H2Buffer::new(true),
            // For the outgoing buffer, we don’t expect to see any preface
            outgoing: H2Buffer::new(false),
            hpack_decoder_incoming: Decoder::new(),
            hpack_decoder_outgoing: Decoder::new(),
            streams: HashMap::new(),
            partial_headers: None,
            partial_headers_direction: None,
        }
    }

    pub fn write_incoming(&mut self, data: &[u8]) {
        self.incoming.write(data);
    }

    pub fn write_outgoing(&mut self, data: &[u8]) {
        self.outgoing.write(data);
    }

    pub fn get_all_events(&mut self) -> Vec<H2Event> {
        // .. perhaps h2event should have been directional instead of only containing the direction :/
        //    this means our caller is gonna have to match on the internal event for now..
        let mut evs = Vec::new();
        while let Some(e) = self.poll_one_frame(H2FrameDirection::Incoming) {
            evs.push(e);
        }
        while let Some(e) = self.poll_one_frame(H2FrameDirection::Outgoing) {
            evs.push(e);
        }
        evs
    }

    /// Attempt to parse exactly one frame from the given direction.  
    /// If we parse or produce an event, return Some(...). If no data is available, return None.
    fn poll_one_frame(&mut self, direction: H2FrameDirection) -> Option<H2Event> {
        
        let buffer = match direction {
            H2FrameDirection::Incoming => &mut self.incoming,
            H2FrameDirection::Outgoing => &mut self.outgoing,
        };

        // Check if we’re in partial HEADERS mode for this direction
        if let Some(ref mut partial) = self.partial_headers {
            // If the partial is for a *different* direction, we might do an error or concurrency approach.
            if self.partial_headers_direction != Some(direction) {
                tracing::trace!("some fishy shit is going on.. seeing inconsistant directions in h2 frame parser..")
            } else {
                // Attempt to parse next frame for the *same* direction
                if let Ok(Some(frame)) = parse_next_frame(&mut buffer.buffer) {
                    // If not CONTINUATION or not same stream => treat it as new
                    if frame.frame_type != 0x9 || frame.stream_id != partial.stream_id {
                        // Release partial HEADERS or treat as protocol error
                        return self.handle_regular_frame(direction, frame);
                    }
                    // Append
                    partial.header_block.extend_from_slice(&frame.payload);
                    let end_headers = (frame.flags & 0x4) != 0; // END_HEADERS
                    if end_headers {
                        let combined = std::mem::take(&mut partial.header_block);
                        let s_id = partial.stream_id;
                        let flags = partial.flags; // original HEADERS flags
                        self.partial_headers = None;
                        self.partial_headers_direction = None;
                        return self.handle_complete_headers(direction, s_id, flags, combined);
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        }

        // Not in partial HEADERS mode, parse a new frame
        match parse_next_frame(&mut buffer.buffer) {
            Ok(Some(frame)) => self.handle_regular_frame(direction, frame),
            Ok(None) => None,    // Need more data
            Err(_e) => None, // Not sure... maybe just retry? 🤷‍♂️
        }
    }

    /// Called once HEADERS+CONTINUATION are done. We'll decode HPACK and produce an event.
    fn handle_complete_headers(
        &mut self,
        direction: H2FrameDirection,
        stream_id: u32,
        flags: u8,
        header_block: Vec<u8>,
    ) -> Option<H2Event> {
        let decode_result = match direction {
            H2FrameDirection::Incoming => self.hpack_decoder_incoming.decode(&header_block),
            H2FrameDirection::Outgoing => self.hpack_decoder_outgoing.decode(&header_block),
        };
        let headers_list = match decode_result {
            Ok(h) => h,
            Err(e) => {
                tracing::trace!("HPACK decode error: {:?}", e);
                return Some(H2Event::Unknown(H2Frame::RstStream {
                    stream_id,
                    error_code: 0x1, // e.g. PROTOCOL_ERROR
                }));
            }
        };

        let headers_map: HashMap<String, String> = headers_list
            .into_iter()
            .map(|(k, v)| (vec_to_string(k), vec_to_string(v)))
            .collect();

        // Insert / update StreamState
        let st = self.streams.entry(stream_id).or_insert_with(StreamState::new);
        st.headers = Some(headers_map.clone());

        let end_stream = (flags & 0x1) != 0;

        match direction {
            H2FrameDirection::Incoming => {
                // If it’s an incoming HEADERS with `end_stream` + a recognized method => produce `IncomingRequest`
                if end_stream && !is_continuous_stream(&headers_map) {
                    let req = st.to_request(stream_id);
                    self.streams.remove(&stream_id);
                    Some(H2Event::IncomingRequest(req))
                } else {
                    Some(H2Event::IncomingHeaders {
                        stream_id: stream_id,
                        headers:headers_map
                    })
                }
            }
            H2FrameDirection::Outgoing => {
                // For outgoing HEADERS, we might see :method => “OutgoingRequest”, or
                // :status => “OutgoingResponse”. If neither, produce “OutgoingHeaders”.
                if let Some(_method) = headers_map.get(":method") {
                    // If end_stream => produce an outgoing request
                    if end_stream && !is_continuous_stream(&headers_map) {
                        let req = st.to_request(stream_id);
                        self.streams.remove(&stream_id);
                        Some(H2Event::OutgoingRequest(req))
                    } else {
                        // HEADERS but not end_stream => partial or chunked?
                        Some(H2Event::OutgoingRequest(st.to_request(stream_id)))
                    }
                } else if let Some(_status) = headers_map.get(":status") {
                    // If end_stream => produce an outgoing response
                    if end_stream {
                        let resp = st.to_response(stream_id);
                        self.streams.remove(&stream_id);
                        Some(H2Event::OutgoingResponse(resp))
                    } else {
                        // HEADERS for a response but not end_stream => partial?
                        Some(H2Event::PartialOutgoingResponse(st.to_response(stream_id)))
                    }
                } else {
                    // Some HEADERS that have no :method or :status => maybe just custom frames
                    Some(H2Event::OutgoingHeaders {
                        stream_id,
                        headers: headers_map,
                    })
                }
            }
        }
    }


    fn handle_priority_frame(&mut self, frame: FrameHeader) -> H2Event {
        if frame.payload.len() == 5 {
            let exclusive = (frame.payload[0] & 0x80) != 0;
            let stream_dependency = ((frame.payload[0] as u32 & 0x7F) << 24)
                | ((frame.payload[1] as u32) << 16)
                | ((frame.payload[2] as u32) << 8)
                | (frame.payload[3] as u32);
            let weight = frame.payload[4];
            H2Event::Priority {
                stream_id: frame.stream_id,
                exclusive,
                stream_dependency,
                weight,
            }
        } else {
            H2Event::Unknown(H2Frame::Unknown {
                stream_id: frame.stream_id,
                flags: frame.flags,
                payload: frame.payload,
                frame_type: frame.frame_type,
            })
        }
    }

    /// The main dispatch for a newly parsed frame (DATA, HEADERS, etc.).
    fn handle_regular_frame(
        &mut self,
        direction: H2FrameDirection,
        frame: FrameHeader,
    ) -> Option<H2Event> {
        match frame.frame_type {
            0x0 => {
                // DATA
                self.handle_data_frame(direction, frame)
            }
            0x1 => {
                // HEADERS
                if let Some((block, end_headers)) = parse_headers_payload(frame.flags, &frame.payload) {
                    if end_headers {
                        // HEADERS all in one shot
                        self.handle_complete_headers(direction, frame.stream_id, frame.flags, block)
                    } else {
                        // partial HEADERS => store until CONTINUATION
                        self.partial_headers = Some(PartialHeadersState {
                            stream_id: frame.stream_id,
                            flags: frame.flags,
                            header_block: block,
                            initial_parsed: true,
                        });
                        self.partial_headers_direction = Some(direction);
                        None
                    }
                } else {
                    // invalid HEADERS parse => produce RST_STREAM or Unknown
                    Some(H2Event::Unknown(H2Frame::RstStream {
                        stream_id: frame.stream_id,
                        error_code: 0x1, // e.g. PROTOCOL_ERROR
                    }))
                }
            }
            0x2 => {
                Some(self.handle_priority_frame(frame))
            }
            0x3 => {
                Some(self.handle_rst_stream(frame))
            }
            0x4 => {
                Some(self.handle_settings_frame(direction, frame))
            }
            0x5 => {
                Some(self.handle_push_promise(direction, frame))
            }
            0x6 => {
                Some(self.handle_ping(direction, frame))
            }
            0x7 => {
                Some(self.handle_goaway(direction, frame))
            }
            0x8 => {
                Some(self.handle_window_update(direction, frame))
            }
            0x9 => {
                // If we’re not currently collecting HEADERS, it’s out-of-place
                tracing::trace!("CONTINUATION received in Ready state (no partial HEADERS).");
                Some(H2Event::Unknown(H2Frame::Unknown {
                    stream_id: frame.stream_id,
                    flags: frame.flags,
                    payload: frame.payload,
                    frame_type: frame.frame_type,
                }))
            }
            // Anything above 0x9 is unknown per the core spec (unless extension frames).
            frame_type => {
                Some(H2Event::Unknown(H2Frame::Unknown {
                    stream_id: frame.stream_id,
                    flags: frame.flags,
                    payload: frame.payload,
                    frame_type,
                }))
            }
        }
    }

    fn handle_push_promise(
        &mut self,
        _direction: H2FrameDirection,
        fh: FrameHeader,
    ) -> H2Event {
        // At minimum, we need 4 bytes to parse the promised_stream_id
        if fh.payload.len() < 4 {
            return H2Event::Unknown(H2Frame::Unknown {
                stream_id: fh.stream_id,
                flags: fh.flags,
                payload: fh.payload,
                frame_type: fh.frame_type,
            });
        }
        let promised_stream_id = ((fh.payload[0] as u32 & 0x7F) << 24)
            | ((fh.payload[1] as u32) << 16)
            | ((fh.payload[2] as u32) << 8)
            | (fh.payload[3] as u32);
        let header_block_fragment = fh.payload[4..].to_vec();
        H2Event::PushPromise {
            stream_id: fh.stream_id,
            promised_stream_id,
            header_block_fragment,
        }
    }

    fn handle_goaway(&mut self, _direction: H2FrameDirection, fh: FrameHeader) -> H2Event {
        if fh.payload.len() >= 8 {
            let last_stream_id = ((fh.payload[0] as u32 & 0x7F) << 24)
                | ((fh.payload[1] as u32) << 16)
                | ((fh.payload[2] as u32) << 8)
                | (fh.payload[3] as u32);
            let error_code = ((fh.payload[4] as u32) << 24)
                | ((fh.payload[5] as u32) << 16)
                | ((fh.payload[6] as u32) << 8)
                | (fh.payload[7] as u32);
            let debug_data = fh.payload[8..].to_vec();
            H2Event::GoAway {
                last_stream_id,
                error_code,
                debug_data,
            }
        } else {
            H2Event::Unknown(H2Frame::Unknown {
                stream_id: fh.stream_id,
                flags: fh.flags,
                payload: fh.payload,
                frame_type: fh.frame_type,
            })
        }
    }
    fn handle_ping(&mut self, direction: H2FrameDirection, fh: FrameHeader) -> H2Event {
        if fh.payload.len() == 8 {
            let mut opaque_data = [0u8; 8];
            opaque_data.copy_from_slice(&fh.payload);
            H2Event::Ping {
                direction,
                flags: fh.flags,
                opaque_data,
            }
        } else {
            H2Event::Unknown(H2Frame::Unknown {
                stream_id: fh.stream_id,
                flags: fh.flags,
                payload: fh.payload,
                frame_type: fh.frame_type,
            })
        }
    }
    fn handle_settings_frame(
        &mut self,
        direction: H2FrameDirection,
        fh: FrameHeader,
    ) -> H2Event {
        let mut cursor = &fh.payload[..];
        let mut settings = Vec::new();

        // Iterate over the payload in 6-byte chunks (2 bytes identifier + 4 bytes value)
        while cursor.len() >= 6 {
            let identifier = ((cursor[0] as u16) << 8) | (cursor[1] as u16);
            let value = ((cursor[2] as u32) << 24)
                | ((cursor[3] as u32) << 16)
                | ((cursor[4] as u32) << 8)
                | (cursor[5] as u32);

            // Convert the raw identifier to a SettingIdentifier enum
            let setting = DecodedSettings {
                identifier: SettingIdentifier::from_u16(identifier),
                value,
            };
            settings.push(setting);

            // Move the cursor forward by 6 bytes
            cursor = &cursor[6..];
        }

        // Check for any leftover bytes indicating a malformed SETTINGS frame
        if !cursor.is_empty() {
            tracing::trace!("Malformed SETTINGS frame: leftover bytes");
            // todo : might just be me fucking up tbh.. should see if we can simplify this a bit
            //        or at least decide if we should actually continue at all here
        }

        // Construct the Settings event with the parsed settings
        H2Event::Settings {
            direction,
            flags: fh.flags,
            settings
        }
    }

    fn handle_window_update(&mut self, direction: H2FrameDirection, fh: FrameHeader) -> H2Event {
        if fh.payload.len() == 4 {
            let window_size_increment = ((fh.payload[0] as u32 & 0x7F) << 24)
                | ((fh.payload[1] as u32) << 16)
                | ((fh.payload[2] as u32) << 8)
                | (fh.payload[3] as u32);
            if fh.stream_id == 0 {
                // connection-level
                self.connection_window_size = self
                    .connection_window_size
                    .checked_add(window_size_increment)
                    .unwrap_or(u32::MAX);
            } else {
                let st = self.streams.entry(fh.stream_id).or_insert_with(StreamState::new);
                st.stream_window_size = st
                    .stream_window_size
                    .checked_add(window_size_increment)
                    .unwrap_or(u32::MAX);
            }
            H2Event::WindowUpdate {
                direction,
                stream_id: fh.stream_id,
                window_size_increment,
            }
        } else {
            H2Event::Unknown(H2Frame::Unknown {
                stream_id: fh.stream_id,
                flags: fh.flags,
                payload: fh.payload,
                frame_type: fh.frame_type,
            })
        }
    }
     fn handle_rst_stream(&mut self, fh: FrameHeader) -> H2Event {
        if fh.payload.len() == 4 {
            let error_code = ((fh.payload[0] as u32) << 24)
                | ((fh.payload[1] as u32) << 16)
                | ((fh.payload[2] as u32) << 8)
                | (fh.payload[3] as u32);
            H2Event::Unknown(H2Frame::RstStream {
                stream_id: fh.stream_id,
                error_code,
            })
        } else {
            H2Event::Unknown(H2Frame::Unknown {
                stream_id: fh.stream_id,
                flags: fh.flags,
                payload: fh.payload,
                frame_type: fh.frame_type,
            })
        }
    }
    /// Handle a DATA frame (including PADDED logic).
    fn handle_data_frame(&mut self, direction: H2FrameDirection, frame: FrameHeader) -> Option<H2Event> {
        if let Some((data, end_stream)) = parse_data_payload(frame.flags, &frame.payload) {
            let st = self.streams.entry(frame.stream_id).or_insert_with(StreamState::new);

            // For typical requests/responses, accumulate the body
            st.body.extend_from_slice(&data);

            // If end_stream => see if we can produce an IncomingRequest/OutgoingRequest/OutgoingResponse
            // depending on direction and presence of :method/:status
            if end_stream && st.headers.is_some() {
                // Distinguish direction
                match direction {
                    H2FrameDirection::Incoming => {
                        if let Some(hm) = &st.headers {
                            if !is_continuous_stream(hm) {
                                
                                // TODO: perhaps we should embed this in to data
                                // or otherwise return it unless we do elsewhere already?
                                let _req = st.to_request(frame.stream_id);

                                self.streams.remove(&frame.stream_id);
                                return Some(H2Event::Data {
                                    stream_id: frame.stream_id,
                                    data,
                                    direction,
                                    end_stream,
                                });
                            }
                        }
                    }
                    H2FrameDirection::Outgoing => {
                        if let Some(hm) = &st.headers {
                            if hm.contains_key(":method") {
                                // OutgoingRequest
                                let req = st.to_request(frame.stream_id);
                                self.streams.remove(&frame.stream_id);
                                return Some(H2Event::OutgoingRequest(req));
                            } else if hm.contains_key(":status") {
                                // OutgoingResponse
                                let resp = st.to_response(frame.stream_id);
                                self.streams.remove(&frame.stream_id);
                                return Some(H2Event::OutgoingResponse(resp));
                            }
                        }
                    }
                }
            }

            // Otherwise, produce a plain Data event
            Some(H2Event::Data {
                stream_id: frame.stream_id,
                data,
                direction,
                end_stream,
            })
        } else {
            // Invalid padding ?
            Some(H2Event::Unknown(H2Frame::Unknown {
                stream_id: frame.stream_id,
                flags: frame.flags,
                payload: frame.payload,
                frame_type: frame.frame_type,
            }))
        }
    }
}

// Implementing `Stream<Item = H2Event>` for async usage
impl Stream for H2Observer {
    type Item = H2Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(e) = this.poll_one_frame(H2FrameDirection::Incoming) {
            return Poll::Ready(Some(e));
        }
        if let Some(e) = this.poll_one_frame(H2FrameDirection::Outgoing) {
            return Poll::Ready(Some(e));
        }

        this.incoming.register_waker(cx.waker());
        this.outgoing.register_waker(cx.waker());

        Poll::Pending
    }
}

// ============== Per-stream State ==================

#[derive(Debug)]
#[allow(unused)]
struct StreamState {
    headers: Option<HashMap<String, String>>,
    header_blocks: Vec<u8>,
    body: Vec<u8>,
    is_continuous_stream: bool,
    incoming_data: Vec<u8>,
    outgoing_data: Vec<u8>,
    stream_window_size: u32,
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

    fn to_request(&self, stream_id: u32) -> HttpRequest {
        let hdrs = self.headers.clone().unwrap_or_default();
        let method = hdrs
            .get(":method")
            .cloned()
            .unwrap_or_else(|| "GET".into());
        let path = hdrs.get(":path").cloned().unwrap_or_else(|| "/".into());
        HttpRequest {
            stream_id,
            method,
            path,
            headers: hdrs,
            body: self.body.clone(),
        }
    }

    fn to_response(&self, stream_id: u32) -> H2Response {
        let hdrs = self.headers.clone().unwrap_or_default();
        let h = hdrs.get(":status").cloned();
        let h2 = h.clone();
        let status = h.unwrap_or_else(||{
            // todo - this kind of sucks... should improve this logic..
            // i mean, we dont want to completely fail unless we have to but this is not great :<
            tracing::warn!("Failed to parse status header ({h2:?})- will fall back to 200");
            "200".into()
        });
        H2Response {
            stream_id,
            status,
            headers: hdrs,
            body: self.body.clone(),
        }
    }
}

// ============= Lower-level buffer & parser =============

#[derive(Debug)]
struct H2Buffer {
    buffer: BytesMut,
    waker: Option<Waker>,
    needs_preface: bool,
    preface_consumed: bool,
}

impl H2Buffer {
    pub fn new(needs_preface: bool) -> Self {
        Self {
            buffer: BytesMut::with_capacity(4096),
            waker: None,
            needs_preface,
            preface_consumed: false,
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        if !self.preface_consumed && self.needs_preface {
            self.consume_preface();
        }
        if let Some(w) = self.waker.take() {
            w.wake();
        }
    }

    pub fn register_waker(&mut self, w: &std::task::Waker) {
        if self.waker.is_none() || !self.waker.as_ref().unwrap().will_wake(w) {
            self.waker = Some(w.clone());
        }
    }

    fn consume_preface(&mut self) {
        const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        if self.buffer.len() < PREFACE.len() {
            return; // Wait for more data
        }
        if &self.buffer[..PREFACE.len()] == PREFACE {
            self.buffer.advance(PREFACE.len());
            self.preface_consumed = true;
            //tracing::debug!("HTTP/2 preface consumed.");
        } else {
            tracing::trace!("Invalid HTTP/2 preface. Discarding buffer.. {:?}",self.buffer);
            self.buffer.advance(self.buffer.len());
        }
    }
}

/// Basic representation of a raw frame header + payload. We only parse the 9-byte header + length.
#[derive(Debug)]
struct FrameHeader {
    frame_type: u8,
    flags: u8,
    stream_id: u32,
    payload: Vec<u8>,
}

fn parse_next_frame(buf: &mut BytesMut) -> Result<Option<FrameHeader>, ()> {
    if buf.len() < 9 {
        return Ok(None);
    }

    let length = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);
    let frame_type = buf[3];
    let flags = buf[4];
    let sid_raw = ((buf[5] as u32) << 24)
        | ((buf[6] as u32) << 16)
        | ((buf[7] as u32) << 8)
        | (buf[8] as u32);
    let stream_id = sid_raw & 0x7FFF_FFFF;

    let total_len = 9 + length as usize;
    if buf.len() < total_len {
        return Ok(None); // partial
    }

    let payload = buf[9..total_len].to_vec();
    buf.advance(total_len);

    Ok(Some(FrameHeader {
        frame_type,
        flags,
        stream_id,
        payload,
    }))
}

// =============== HEADERS / DATA Helpers ===============

/// Parse HEADERS payload, handling optional PADDED or PRIORITY bits.  
/// Returns `(header_block, end_headers)` if valid, or `None` if something’s off.
fn parse_headers_payload(flags: u8, payload: &[u8]) -> Option<(Vec<u8>, bool)> {
    let mut cursor = payload;
    let mut pad_len = 0_usize;

    // PADDED
    if flags & 0x8 != 0 {
        if cursor.is_empty() {
            return None;
        }
        pad_len = cursor[0] as usize;
        cursor = &cursor[1..];
        if cursor.len() < pad_len {
            return None;
        }
    }

    // PRIORITY
    if flags & 0x20 != 0 {
        if cursor.len() < 5 {
            return None;
        }
        // skip the 5 priority bytes
        cursor = &cursor[5..];
    }

    if cursor.len() < pad_len {
        return None;
    }
    let block_len = cursor.len() - pad_len;
    let block = &cursor[..block_len];
    let end_headers = (flags & 0x4) != 0; // bit 0x4 => END_HEADERS
    Some((block.to_vec(), end_headers))
}

/// Parse DATA payload, handling optional PADDED. Return `(data, end_stream)`.
fn parse_data_payload(flags: u8, payload: &[u8]) -> Option<(Vec<u8>, bool)> {
    let mut cursor = payload;
    let mut pad_len = 0_usize;

    // PADDED?
    if flags & 0x8 != 0 {
        if cursor.is_empty() {
            return None;
        }
        pad_len = cursor[0] as usize;
        cursor = &cursor[1..];
        if cursor.len() < pad_len {
            return None;
        }
    }

    let data_len = cursor.len().saturating_sub(pad_len);
    let data = &cursor[..data_len];
    let end_stream = (flags & 0x1) != 0; // END_STREAM bit
    Some((data.to_vec(), end_stream))
}

/// A naive check: if `:method` is a known HTTP method, we treat it as a normal request.  
/// If not, we treat it as “continuous.”
fn is_continuous_stream(headers: &HashMap<String, String>) -> bool {
    if let Some(m) = headers.get(":method") {
        let regulars = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
        if regulars.contains(&m.as_str()) {
            return false;
        }
    }
    true
}

/// Convert a vector to a string (falling back if invalid UTF-8).
fn vec_to_string(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap_or_else(|_| "failed to convert to string..".to_string())
}

// ============= Implementation details / Debug =============

impl std::fmt::Debug for H2Observer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("H2Observer")
            .field("connection_window_size", &self.connection_window_size)
            .field("streams", &self.streams.keys())
            .field("partial_headers", &self.partial_headers)
            .field("partial_headers_direction", &self.partial_headers_direction)
            .finish()
    }
}
