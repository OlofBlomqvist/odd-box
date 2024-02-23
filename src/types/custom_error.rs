
use std::fmt;

#[derive(Debug)]
pub struct CustomError(pub String);
impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for CustomError {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        CustomError(format!("WebSocket error: {}", e))
    }
}

impl std::error::Error for CustomError {}

impl From<hyper::Error> for CustomError {
    fn from(err: hyper::Error) -> CustomError {
        CustomError(format!("Hyper error: {}", err))
    }
}

