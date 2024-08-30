use std::net::IpAddr;
use std::net::Ipv4Addr;
use anyhow::bail;
use serde::Serialize;
use serde::Deserialize;
use utoipa::ToSchema;

use super::EnvVar;
use super::LogFormat;
use super::LogLevel;


#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct InProcessSiteConfig{
    /// This is mostly useful in case the target uses SNI sniffing/routing
    pub disable_tcp_tunnel_mode : Option<bool>,
    /// H2C or H2 - used to signal use of prior knowledge http2 or http2 over clear text. 
    pub h2_hint : Option<H2Hint>,
    pub host_name : String,
    pub dir : String,
    pub bin : String,
    pub args : Vec<String>,
    pub env_vars : Vec<EnvVar>,
    pub log_format: Option<LogFormat>,
    /// Set this to false if you do not want this site to start automatically when odd-box starts
    pub auto_start: Option<bool>,
    pub port: Option<u16>,
    pub https : Option<bool>,
    /// If you wish to use wildcard routing for any subdomain under the 'host_name'
    pub capture_subdomains : Option<bool>,
    /// If you wish to use the subdomain from the request in forwarded requests:
    /// test.example.com -> internal.site
    /// vs
    /// test.example.com -> test.internal.site 
    pub forward_subdomains : Option<bool>,
    /// Set to true to prevent odd-box from starting this site automatically when it starts or using the 'start' command.
    /// It can still be manually started by ctrl-clicking in the TUI. 
    pub disabled: Option<bool>
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum H2Hint {
    H2,
    H2C
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RemoteSiteConfig{
    /// H2C or H2 - used to signal use of prior knowledge http2 or http2 over clear text. 
    pub h2_hint : Option<H2Hint>,
    pub host_name : String,
    pub target_hostname : String,
    pub port: Option<u16>,
    pub https : Option<bool>,
    /// If you wish to use wildcard routing for any subdomain under the 'host_name'
    pub capture_subdomains : Option<bool>,
    /// This is mostly useful in case the target uses SNI sniffing/routing
    pub disable_tcp_tunnel_mode : Option<bool>,
    /// If you wish to use the subdomain from the request in forwarded requests:
    /// test.example.com -> internal.site
    /// vs
    /// test.example.com -> test.internal.site 
    pub forward_subdomains : Option<bool>
}

#[derive(Debug, Clone, Serialize, Deserialize,ToSchema)]
pub struct OddBoxV1Config {
    #[schema(value_type = String)]
    pub version : super::OddBoxConfigVersion,
    pub root_dir : Option<String>, 
    #[serde(default = "default_log_level")]
    pub log_level : Option<LogLevel>,
    /// Defaults to true. Lets you enable/disable h2/http11 tls alpn algs during initial connection phase. 
    #[serde(default = "true_option")]
    pub alpn : Option<bool>,
    pub port_range_start : u16,
    #[serde(default = "default_log_format")]
    pub default_log_format : LogFormat,
    #[schema(value_type = String)]
    pub ip : Option<IpAddr>,
    #[serde(default = "default_http_port_8080")]
    pub http_port : Option<u16>,
    #[serde(default = "default_https_port_4343")]
    pub tls_port : Option<u16>,
    #[serde(default = "true_option")]
    pub auto_start : Option<bool>,
    pub env_vars : Vec<EnvVar>,
    pub remote_target : Option<Vec<RemoteSiteConfig>>,
    pub hosted_process : Option<Vec<InProcessSiteConfig>>,
    pub admin_api_port : Option<u16>,
    pub path : Option<String>

}
impl crate::configuration::OddBoxConfiguration<OddBoxV1Config> for OddBoxV1Config {
    

    #[allow(unused)]
    fn example() -> OddBoxV1Config {
        OddBoxV1Config {
            path: None,
            admin_api_port: None,
            version: super::OddBoxConfigVersion::V1,
            alpn: Some(false),
            auto_start: Some(true),
            default_log_format: LogFormat::standard,
            env_vars: vec![
                EnvVar { key: "some_key".into(), value:"some_val".into() },
                EnvVar { key: "another_key".into(), value:"another_val".into() },
            ],
            ip: Some(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
            log_level: Some(LogLevel::Info),
            http_port: Some(80),
            port_range_start: 4200,
            hosted_process: Some(vec![
                InProcessSiteConfig {
                    forward_subdomains: None,
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
                    port: Some(443) ,
                    log_format: Some(LogFormat::standard),
                    dir: "/tmp".into(),
                    https: Some(true),
                    h2_hint: None,
                    disabled :None
                    
                }
            ]),
            remote_target: Some(vec![
                RemoteSiteConfig { 
                    forward_subdomains: None,
                    h2_hint: None, 
                    host_name: "lobsters.local".into(), 
                    target_hostname: "lobste.rs".into(), 
                    port: None, 
                    https: Some(true), 
                    capture_subdomains: Some(false), 
                    disable_tcp_tunnel_mode: Some(false)
                },
                RemoteSiteConfig { 
                    forward_subdomains: Some(true),
                    h2_hint: None, 
                    host_name: "google.local".into(), 
                    target_hostname: "google.com".into(), 
                    port: Some(443), 
                    https: Some(true), 
                    capture_subdomains: Some(false), 
                    disable_tcp_tunnel_mode: Some(true)
                }
            ]),
            root_dir: Some("/tmp".into()),
            tls_port: Some(443)

        }
    }

}

fn default_log_level() -> Option<LogLevel> {
    Some(LogLevel::Info)
}
fn default_log_format() -> LogFormat {
    LogFormat::standard
}
fn default_https_port_4343() -> Option<u16> {
    Some(4343)
}
fn default_http_port_8080() -> Option<u16> {
    Some(8080)
}

fn true_option() -> Option<bool> {
    Some(true)
}


// LEGACY ---> V1
impl TryFrom<crate::configuration::legacy::OddBoxLegacyConfig> for crate::configuration::v1::OddBoxV1Config {
    
    type Error = String;

    fn try_from(old_config: crate::configuration::legacy::OddBoxLegacyConfig) -> Result<Self, Self::Error> {
        let new_config = crate::configuration::v1::OddBoxV1Config {
            path: None,
            version: crate::configuration::OddBoxConfigVersion::V1,
            admin_api_port: None,
            alpn: Some(false), // allowing alpn would be a breaking change for h2c when using old configuration format
            auto_start: old_config.auto_start,
            default_log_format: old_config.default_log_format.unwrap_or(LogFormat::standard),
            env_vars: old_config.env_vars,
            ip: old_config.ip,
            log_level: old_config.log_level,
            http_port: old_config.port,
            port_range_start: old_config.port_range_start,
            hosted_process: Some(old_config.processes.into_iter().map(|x|{
                crate::configuration::v1::InProcessSiteConfig {
                    forward_subdomains: None,
                    disable_tcp_tunnel_mode: x.disable_tcp_tunnel_mode,
                    args: x.args,
                    auto_start: x.auto_start,
                    bin: x.bin,
                    capture_subdomains: None,
                    env_vars: x.env_vars,
                    host_name: x.host_name,
                    port: if x.https.unwrap_or_default() { Some(x.port) } else { None } ,
                    log_format: x.log_format,
                    dir: x.path,
                    https: x.https,
                    h2_hint: x.h2_hint,
                    disabled: None
                    
                }
            }).collect::<Vec<crate::configuration::v1::InProcessSiteConfig>>()),
            remote_target: old_config.remote_sites,
            root_dir: old_config.root_dir,
            tls_port: old_config.tls_port

        };
        Ok(new_config)
    }
}

