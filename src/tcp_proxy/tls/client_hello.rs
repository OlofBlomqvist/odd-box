use super::extension::TlsExtension;
use super::extension::SniParserError;
use super::extension::TlsExtensionType;

#[derive(Debug)]
pub enum TlsClientHelloError {
    NotTLSHandshake,
    NotClientHello,
    MessageIncomplete(#[allow(dead_code)]usize),
}

#[derive(Debug)]
pub struct TlsClientHello {
    protocol_version: (u8, u8),
    random: Vec<u8>,
    session_id: Vec<u8>,
    cipher_suites: Vec<u16>,
    compression_methods: Vec<u8>,
    extensions: Vec<TlsExtension>,
}


#[allow(dead_code)]
impl TlsClientHello {

    pub fn get_protocol_version_str(&self) -> String {
        match self.protocol_version {
            (3, 3) => "TLS 1.2".to_string(),
            (3, 4) => "TLS 1.3".to_string(),
            _ => format!(
                "Unknown TLS Version {:?}.{:?}",
                self.protocol_version.0, self.protocol_version.1
            ),
        }
    }

    pub fn get_session_id_hex(&self) -> String {
        self.session_id
            .iter()
            .map(|byte| format!("{:02X}", byte))
            .collect::<Vec<String>>()
            .join("")
    }

    pub fn get_random_hex(&self) -> String {
        self.random
            .iter()
            .map(|byte| format!("{:02X}", byte))
            .collect::<Vec<String>>()
            .join("")
    }

    pub fn get_cipher_suites_str(&self) -> Vec<String> {
        self.cipher_suites
            .iter()
            .map(|&cs| Self::cipher_suite_to_str(cs))
            .collect()
    }

    pub fn get_compression_methods_str(&self) -> Vec<String> {
        self.compression_methods
            .iter()
            .map(|&cm| Self::compression_method_to_str(cm))
            .collect()
    }

    pub fn cipher_suite_to_str(suite: u16) -> String {
        match suite {
            // TLS 1.3 Cipher Suites
            0x1301 => "TLS_AES_128_GCM_SHA256",
            0x1302 => "TLS_AES_256_GCM_SHA384",
            0x1303 => "TLS_CHACHA20_POLY1305_SHA256",
            0x1304 => "TLS_AES_128_CCM_SHA256",
            0x1305 => "TLS_AES_128_CCM_8_SHA256",
            // ECDHE_ECDSA Cipher Suites
            0xC02B => "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
            0xC02C => "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
            0xC023 => "TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA256",
            0xC024 => "TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA384",
            // ECDHE_RSA Cipher Suites
            0xC02F => "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
            0xC030 => "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
            0xC027 => "TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA256",
            0xC028 => "TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA384",
            // DHE_RSA Cipher Suites
            0x009E => "TLS_DHE_RSA_WITH_AES_128_GCM_SHA256",
            0x009F => "TLS_DHE_RSA_WITH_AES_256_GCM_SHA384",
            0x0067 => "TLS_DHE_RSA_WITH_AES_128_CBC_SHA256",
            0x006B => "TLS_DHE_RSA_WITH_AES_256_CBC_SHA256",
            // RSA Cipher Suites
            0x009C => "TLS_RSA_WITH_AES_128_GCM_SHA256",
            0x009D => "TLS_RSA_WITH_AES_256_GCM_SHA384",
            0x003C => "TLS_RSA_WITH_AES_128_CBC_SHA256",
            0x003D => "TLS_RSA_WITH_AES_256_CBC_SHA256",
            // PSK Cipher Suites
            0x00AE => "TLS_PSK_WITH_AES_128_GCM_SHA256",
            0x00AF => "TLS_PSK_WITH_AES_256_GCM_SHA384",
            0x008C => "TLS_PSK_WITH_AES_128_CBC_SHA256",
            0x008D => "TLS_PSK_WITH_AES_256_CBC_SHA384",
            // Historical Cipher Suites
            0x0005 => "TLS_RSA_WITH_RC4_128_SHA",
            0x000A => "TLS_RSA_WITH_3DES_EDE_CBC_SHA",
            0x002F => "TLS_RSA_WITH_AES_128_CBC_SHA",
            0x0035 => "TLS_RSA_WITH_AES_256_CBC_SHA",
            _ => "UNKNOWN",
        }
        .to_string()
    }

    pub fn compression_method_to_str(method: u8) -> String {
        match method {
            0 => "null (no compression)".to_string(),
            _ => format!("Unknown Compression Method 0x{:02X}", method),
        }
    }

    pub fn read_sni_hostname(&self) -> Result<String, SniParserError> {
        for extension in &self.extensions {
            if let TlsExtensionType::ServerName = extension.typ {
                      // SNI extension type
                    // SNI extension structure: List length (2 bytes) | Name Type (1 byte) | Name Length (2 bytes) | Name
                    if extension.data.len() < 5 {
                        return Err(SniParserError::InvalidExtensionFormat);
                    }
                    let name_type = extension.data[2];
                    if name_type != 0x00 {
                        // 0x00==Host Name Type
                        continue;
                    }
                    let name_length =
                        u16::from_be_bytes([extension.data[3], extension.data[4]]) as usize;
                    if name_length > extension.data.len() - 5 {
                        return Err(SniParserError::InvalidExtensionFormat);
                    }
                    let hostname = std::str::from_utf8(&extension.data[5..5 + name_length])
                        .map_err(SniParserError::Utf8Error)?;
                    return Ok(hostname.to_string());
                }
        }
        Err(SniParserError::NoSniFound)
    }
}



impl TryFrom<&[u8]> for TlsClientHello{
    type Error = TlsClientHelloError;
    fn try_from(data: &[u8]) -> Result<Self,Self::Error> {
        let mut current = 0;

        if data.len() < 6 || data[current] != 0x16 {
            return Err(TlsClientHelloError::NotTLSHandshake);
        }
        current += 1; // Skip handshake type
        current += 4; // Skip version and length of the record layer
    
        if data[current] != 0x01 {
            return Err(TlsClientHelloError::NotClientHello);
        }
        current += 1; // Skip HandshakeType
    
        if data.len() < current + 3 {
            return Err(TlsClientHelloError::MessageIncomplete(current));
        }
        current += 3; // Skip length of the ClientHello message
    
        let protocol_version = (data[current], data[current + 1]);
        current += 2; // Skip ProtocolVersion
    
        let random = data[current..current + 32].to_vec();
        current += 32; // Skip Random
    
        let session_id_length = data[current] as usize;
        let session_id = data[current + 1..current + 1 + session_id_length].to_vec();
        current += 1 + session_id_length; // Skip SessionID
    
        let cipher_suites_length = u16::from_be_bytes([data[current], data[current + 1]]) as usize;
        let cipher_suites = (0..cipher_suites_length / 2)
            .map(|i| u16::from_be_bytes([data[current + 2 + i * 2], data[current + 3 + i * 2]]))
            .collect();
        current += 2 + cipher_suites_length; // Skip CipherSuites
    
        let compression_methods_length = data[current] as usize;
        let compression_methods =
            data[current + 1..current + 1 + compression_methods_length].to_vec();
        current += 1 + compression_methods_length; // Skip CompressionMethods
    
        if data.len() < current + 2 {
            return Err(TlsClientHelloError::MessageIncomplete(current));
        }
        let extensions_length = u16::from_be_bytes([data[current], data[current + 1]]) as usize;
        current += 2; // Skip ExtensionsLength
    
        let mut extensions = Vec::new();
        let extensions_end = current + extensions_length;
    
        while current + 4 <= extensions_end {
            let typ = u16::from_be_bytes([data[current], data[current + 1]]);
            let extension_length =
                u16::from_be_bytes([data[current + 2], data[current + 3]]) as usize;
            current += 4; // Skip ExtensionType and ExtensionLength
    
            if current + extension_length > extensions_end {
                return Err(TlsClientHelloError::MessageIncomplete(current));
            }
            if current + extension_length > data.len() {
                return Err(TlsClientHelloError::MessageIncomplete(current));
            }
            let data = data[current..current + extension_length].to_vec();
            extensions.push(TlsExtension::new(typ.into(),data));
            current += extension_length; // Move past this extension's data
        }
    
        Ok(TlsClientHello {
            protocol_version,
            random,
            session_id,
            cipher_suites,
            compression_methods,
            extensions,
        })
    }
}
