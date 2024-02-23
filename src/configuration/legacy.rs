use serde::Serialize;
use serde::Deserialize;

use super::EnvVar;
use super::LogFormat;
use super::LogLevel;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub (crate) struct SiteConfig{
    pub host_name : String,
    pub path : String,
    pub bin : String,
    pub args : Vec<String>,
    pub env_vars : Vec<EnvVar>,
    pub log_format: Option<LogFormat>,
    /// Set this to false if you do not want this site to start automatically
    pub auto_start: Option<bool>,
    /// Set this to true in case your backend service uses https
    pub https : Option<bool>,
    pub capture_subdomains : Option<bool>,
    #[serde(skip)] pub (crate) port : u16,
    // BACKPORTING FOR V1 CONFIGS
    pub h2_hint : Option<super::v1::H2Hint>,
    pub disable_tcp_tunnel_mode : Option<bool>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub (crate) struct Config {
    pub processes : Vec<SiteConfig>,
    pub env_vars : Vec<EnvVar>,
    pub root_dir : Option<String>,
    pub log_level : Option<LogLevel>,
    pub port_range_start : u16,
    pub default_log_format : Option<LogFormat>,
    pub port : Option<u16>,
    pub tls_port : Option<u16>,
    pub auto_start : Option<bool>,
    // BACKPORTING FOR V1 CONFIGS
    pub (crate) ip : Option<std::net::IpAddr>,
    pub (crate) remote_sites : Option<Vec<super::v1::RemoteSiteConfig>>,
}
