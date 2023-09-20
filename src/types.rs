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
    pub default_log_format : Option<LogFormat>,
    pub port : Option<u16>
    
}


impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl From
<tokio_tungstenite::tungstenite::Error> for CustomError {
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


impl Config {
        
    // Validates and populates variables in the configuration
    pub fn init(&mut self,cfg_path:&str) -> Result<(),String>  {

        let resolved_home_dir_path = dirs::home_dir().ok_or(String::from("Failed to resolve home directory."))?;
        let resolved_home_dir_str = resolved_home_dir_path.to_str().ok_or(String::from("Failed to parse home directory."))?;

        tracing::info!("Resolved home directory: {}",&resolved_home_dir_str);

        let cfg_dir = 
            if let Some(directory_path_str) = 
                std::path::Path::new(cfg_path)
                .parent()
                .map(|p| p.to_str().unwrap_or_default()) 
            {
                tracing::debug!("$cfg_dir resolved to {directory_path_str}");
                directory_path_str
            } else {
                return Err(format!("Failed to resolve $cfg_dir"));
            };   

        let cloned_root_dir = self.root_dir.clone();

        let with_vars = |x:&str| -> String {
            x.replace("$root_dir", & if let Some(rd) = &cloned_root_dir { rd.to_string() } else { "$root_dir".to_string() })
            .replace("$cfg_dir", cfg_dir)
            .replace("~", resolved_home_dir_str)
        };
            
        if let Some(rp) = self.root_dir.as_mut() {
            if rp.contains("$root_dir") {
                panic!("it is clearly not a good idea to use $root_dir in the configuration of root dir...")
            }
            *rp = with_vars(rp);
            tracing::debug!("$root_dir resolved to: {rp}")
        }

        let log_format = self.default_log_format.clone();

        for x in &mut self.processes.iter_mut() {
            
            if x.path.len() < 5 { return Err(format!("Invalid path configuration for {:?}",x))}
            
            x.path = with_vars(&x.path);
            x.bin = with_vars(&x.bin);

            for a in &mut x.args {
                *a = with_vars(a)
            }

            // basic sanity check..
            if x.path.contains("$root_dir") {
                return Err(format!("Invalid configuration: {x:?}. Missing root_dir in configuration file but referenced for this item.."))
            }

            // if no log format is specified for the process but there is a global format, override it
            if x.log_format.is_none() {
                if let Some(f) = &log_format {
                    x.log_format = Some(f.clone())
                }
            }
        }

        Ok(())
    }

}