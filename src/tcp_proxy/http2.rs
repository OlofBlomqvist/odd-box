use hpack::Decoder;

#[allow(dead_code)]
// https://datatracker.ietf.org/doc/html/rfc9113
pub fn find_http2_authority(bytes: &[u8]) -> Option<String> {
    let mut current = 0;
    while current + 9 <= bytes.len() {
        let length =
            u32::from_be_bytes([0, bytes[current], bytes[current + 1], bytes[current + 2]])
                as usize;
        let frame_type = bytes[current + 3];
        let _flags = bytes[current + 4];
        let stream_id = u32::from_be_bytes([
            0,
            bytes[current + 5],
            bytes[current + 6],
            bytes[current + 7],
        ]) & 0x7fffffff;
        current += 9; // Move past the frame header
        if frame_type == 0x1 {
            // HEADERS frame
            // Ensure the frame belongs to a client-initiated stream
            if stream_id % 2 != 0 {
                let header_block_fragment = &bytes[current..current + length];
                // Decompress the header block fragment using HPACK
                if let Ok(headers) = decompress_hpack(header_block_fragment) {
                    // Find the `:authority` pseudo-header
                    for (name, value) in headers {
                        if name == ":authority" {
                            return Some(value);
                        }
                    }
                } else {
                    tracing::trace!("failed to decode headers using hpack!");
                    return None;
                }
            }
        }
        current += length;
    }
    //tracing::trace!("we cant seem to find any authority header");
    None
}


fn decompress_hpack(fragment: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut decoder = Decoder::new();
    match decoder.decode(fragment) {
        Ok(headers) => Ok(headers
            .into_iter()
            .map(|(key, value)| {
                let h = String::from_utf8(key).map_err(|e| format!("{e:?}"))?;
                let v = String::from_utf8(value).map_err(|e| format!("{e:?}"))?;
                Ok((h, v))
            })
            .collect::<Result<Vec<(String, String)>, String>>()?),
        e => Err(format!("{e:?}")),
    }
}

// https://datatracker.ietf.org/doc/html/rfc9113
pub fn is_valid_http2_request(bytes: &[u8]) -> bool {
    // HTTP/2 client connection preface
    let http2_preface = b"PRI * HTTP/2.0"; // ...\r\n\r\nSM\r\n\r\n
    // Check if the bytes start with the HTTP/2 preface
    let result = bytes.starts_with(http2_preface);
    result
}