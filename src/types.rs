use std::fmt;

use serde::Serialize;
use serde::Deserialize;

#[derive(Debug)]
pub (crate) struct CustomError(pub (crate) String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub (crate) struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Serialize,Deserialize,Debug,Clone)]
#[allow(non_camel_case_types)]
pub enum LogFormat {
    standard,
    dotnet
}
#[derive(Serialize,Deserialize,Debug,Clone)]
#[allow(non_camel_case_types)]
pub enum LogLevel {
    trace,
    debug,
    info,
    warn,
    error
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub (crate) struct SiteConfig{
    pub host_name : String,
    pub path : String,
    pub bin : String,
    pub args : Vec<String>,
    pub env_vars : Vec<EnvVar>,
    pub log_format: Option<LogFormat>,
    pub https : Option<bool>,
    #[serde(skip)] pub (crate) port : u16
}

impl SiteConfig {
    pub fn set_port(&mut self, port : u16) { 
        self.port = port 
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub (crate) struct Config {
    pub processes : Vec<SiteConfig>,
    pub env_vars : Vec<EnvVar>,
    pub root_dir : Option<String>,
    pub log_level : Option<LogLevel>,
    pub port_range_start : u16,
    pub default_log_format : Option<LogFormat>
    
}


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
