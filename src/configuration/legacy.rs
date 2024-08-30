use std::net::IpAddr;
use std::net::Ipv4Addr;

use anyhow::bail;
use serde::Serialize;
use serde::Deserialize;

use super::EnvVar;
use super::LogFormat;
use super::LogLevel;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig{
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
    #[serde(skip)] pub port : u16,
    // BACKPORTING FOR V1 CONFIGS
    pub h2_hint : Option<super::v1::H2Hint>,
    pub disable_tcp_tunnel_mode : Option<bool>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OddBoxLegacyConfig {
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
    pub ip : Option<std::net::IpAddr>,
    pub remote_sites : Option<Vec<super::v1::RemoteSiteConfig>>,
}

impl crate::configuration::OddBoxConfiguration<OddBoxLegacyConfig> for OddBoxLegacyConfig {
    

    #[allow(unused)]
    fn example() -> OddBoxLegacyConfig {
        OddBoxLegacyConfig {
            auto_start: Some(true),
            default_log_format: Some(LogFormat::standard),
            env_vars: vec![
                EnvVar { key: "some_key".into(), value:"some_val".into() },
                EnvVar { key: "another_key".into(), value:"another_val".into() },
            ],
            ip: Some(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
            log_level: Some(LogLevel::Info),
            port: Some(80),
            port_range_start: 4200,
            processes: vec![
                SiteConfig {
                    disable_tcp_tunnel_mode: Some(false),
                    args: vec!["--test".to_string()],
                    auto_start: Some(true),
                    bin: "my_bin".into(),
                    capture_subdomains: None,
                    env_vars: vec![
                        EnvVar { key: "some_key".into(), value:"some_val".into() },
                        EnvVar { key: "another_key".into(), value:"another_val".into() },
                    ],
                    host_name: "some_host.local".into(),
                    port: 443 ,
                    log_format: Some(LogFormat::standard),
                    path: "/tmp".into(),
                    https: Some(true),
                    h2_hint: None
                    
                }
            ],
            remote_sites: Some(vec![
                super::v1::RemoteSiteConfig { 
                    h2_hint: None, 
                    host_name: "lobsters.localtest.me".into(), 
                    target_hostname: "lobsters.rs".into(), 
                    port: Some(443), 
                    https: Some(true), 
                    capture_subdomains: Some(false), 
                    disable_tcp_tunnel_mode: Some(true), 
                    forward_subdomains: None 
                },
                super::v1::RemoteSiteConfig { 
                    h2_hint: None, 
                    host_name: "google.localtest.me".into(), 
                    target_hostname: "google.com".into(), 
                    port: Some(443), 
                    https: Some(true), 
                    capture_subdomains: Some(false), 
                    disable_tcp_tunnel_mode: Some(true), 
                    forward_subdomains: None 
                }
            ]),
            root_dir: Some("/tmp".into()),
            tls_port: Some(443)

        }
    }
    

}


