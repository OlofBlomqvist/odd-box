use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use super::EnvVar;
use super::LogFormat;
use super::LogLevel;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InProcessSiteConfig {
    pub disable_tcp_tunnel_mode: Option<bool>,
    pub h2_hint: Option<H2Hint>,
    pub host_name: String,
    pub dir: String,
    pub bin: String,
    pub args: Vec<String>,
    pub env_vars: Vec<EnvVar>,
    pub log_format: Option<LogFormat>,
    pub auto_start: Option<bool>,
    pub port: Option<u16>,
    pub https: Option<bool>,
    pub capture_subdomains: Option<bool>,
    pub forward_subdomains: Option<bool>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum H2Hint {
    H2,
    H2C,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSiteConfig {
    pub h2_hint: Option<H2Hint>,
    pub host_name: String,
    pub target_hostname: String,
    pub port: Option<u16>,
    pub https: Option<bool>,
    pub capture_subdomains: Option<bool>,
    pub disable_tcp_tunnel_mode: Option<bool>,
    pub forward_subdomains: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1Config {
    pub version: super::OddBoxConfigVersion,
    pub root_dir: Option<String>,
    #[serde(default = "default_log_level")]
    pub log_level: Option<LogLevel>,
    #[serde(default = "true_option")]
    pub alpn: Option<bool>,
    pub port_range_start: u16,
    #[serde(default = "default_log_format")]
    pub default_log_format: LogFormat,
    pub ip: Option<IpAddr>,
    #[serde(default = "default_http_port_8080")]
    pub http_port: Option<u16>,
    #[serde(default = "default_https_port_4343")]
    pub tls_port: Option<u16>,
    #[serde(default = "true_option")]
    pub auto_start: Option<bool>,
    pub env_vars: Vec<EnvVar>,
    pub remote_target: Option<Vec<RemoteSiteConfig>>,
    pub hosted_process: Option<Vec<InProcessSiteConfig>>,
    pub admin_api_port: Option<u16>,
    #[serde(skip)]
    #[allow(dead_code)]
    pub path: Option<String>,
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

// LEGACY → V1
impl TryFrom<super::legacy::LegacyConfig> for V1Config {
    type Error = String;

    fn try_from(old: super::legacy::LegacyConfig) -> Result<Self, Self::Error> {
        Ok(V1Config {
            path: None,
            version: super::OddBoxConfigVersion::V1,
            admin_api_port: None,
            alpn: Some(false),
            auto_start: old.auto_start,
            default_log_format: old.default_log_format.unwrap_or(LogFormat::standard),
            env_vars: old.env_vars,
            ip: old.ip,
            log_level: old.log_level,
            http_port: old.port,
            port_range_start: old.port_range_start,
            hosted_process: Some(
                old.processes
                    .into_iter()
                    .map(|x| InProcessSiteConfig {
                        forward_subdomains: None,
                        disable_tcp_tunnel_mode: x.disable_tcp_tunnel_mode,
                        args: x.args,
                        auto_start: x.auto_start,
                        bin: x.bin,
                        capture_subdomains: None,
                        env_vars: x.env_vars,
                        host_name: x.host_name,
                        port: if x.https.unwrap_or_default() {
                            Some(x.port)
                        } else {
                            None
                        },
                        log_format: x.log_format,
                        dir: x.path,
                        https: x.https,
                        h2_hint: x.h2_hint,
                        disabled: None,
                    })
                    .collect(),
            ),
            remote_target: old.remote_sites,
            root_dir: old.root_dir,
            tls_port: old.tls_port,
        })
    }
}
