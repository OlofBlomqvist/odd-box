
#[derive(Debug, PartialEq, Eq,Clone)]
pub (crate) enum TlsExtensionType {
    ServerName, // 0x0000
    MaxFragmentLength, // 0x0001
    StatusRequest, // 0x0005
    SupportedGroups, // 0x000a
    SignatureAlgorithms, // 0x000d
    ApplicationLayerProtocolNegotiation, // 0x0010
    SignedCertificateTimestamp, // 0x0012
    KeyShare, // 0x0028
    PreSharedKey, // 0x0029
    EarlyData, // 0x002a
    SupportedVersions, // 0x002b
    Cookie, // 0x002c
    PskKeyExchangeModes, // 0x002d
    Unknown(u16), // For unhandled extension types
}

#[derive(Debug)]
pub (crate) struct TlsExtension {
    pub typ : TlsExtensionType,
    pub data : Vec<u8>
}

impl TlsExtension {
    pub fn new(typ:TlsExtensionType,data:Vec<u8>) -> Self {
        TlsExtension {
            typ, data
        }
    }
}

impl From<u16> for TlsExtensionType {
    fn from(value: u16) -> Self {
        match value {
            0x0000 => TlsExtensionType::ServerName,
            0x0001 => TlsExtensionType::MaxFragmentLength,
            0x0005 => TlsExtensionType::StatusRequest,
            0x000a => TlsExtensionType::SupportedGroups,
            0x000d => TlsExtensionType::SignatureAlgorithms,
            0x0010 => TlsExtensionType::ApplicationLayerProtocolNegotiation,
            0x0012 => TlsExtensionType::SignedCertificateTimestamp,
            0x0028 => TlsExtensionType::KeyShare,
            0x0029 => TlsExtensionType::PreSharedKey,
            0x002a => TlsExtensionType::EarlyData,
            0x002b => TlsExtensionType::SupportedVersions,
            0x002c => TlsExtensionType::Cookie,
            0x002d => TlsExtensionType::PskKeyExchangeModes,
            _ => TlsExtensionType::Unknown(value),
        }
    }
}

impl Into<u16> for TlsExtensionType {
    fn into(self) -> u16 {
        match self {
            TlsExtensionType::ServerName => 0x0000,
            TlsExtensionType::MaxFragmentLength => 0x0001,
            TlsExtensionType::StatusRequest => 0x0005,
            TlsExtensionType::SupportedGroups => 0x000a,
            TlsExtensionType::SignatureAlgorithms => 0x000d,
            TlsExtensionType::ApplicationLayerProtocolNegotiation => 0x0010,
            TlsExtensionType::SignedCertificateTimestamp => 0x0012,
            TlsExtensionType::KeyShare => 0x0028,
            TlsExtensionType::PreSharedKey => 0x0029,
            TlsExtensionType::EarlyData => 0x002a,
            TlsExtensionType::SupportedVersions => 0x002b,
            TlsExtensionType::Cookie => 0x002c,
            TlsExtensionType::PskKeyExchangeModes => 0x002d,
            TlsExtensionType::Unknown(value) => value,
        }
    }
}


#[derive(Debug)]
pub (crate) enum SniParserError {
    NoSniFound,
    InvalidExtensionFormat,
    Utf8Error(std::str::Utf8Error),
}
