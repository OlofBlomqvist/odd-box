use serde::Serializer;
use std::str;

pub fn serialize_bytes_as_string<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match str::from_utf8(bytes) {
        Ok(s) => serializer.serialize_str(s),
        Err(_) => serializer.serialize_bytes(bytes),
    }
}
pub fn serialize_option_bytes_as_string<S>(bytes: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(bytes) = bytes {
        match str::from_utf8(bytes) {
            Ok(s) => serializer.serialize_str(s),
            Err(_) => serializer.serialize_bytes(bytes),
        }
    } else {
        serializer.serialize_str("\"\"")
    }
}
