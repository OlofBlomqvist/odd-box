use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use super::EnvVar;
use super::LogFormat;
use super::LogLevel;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct DirServer {
    pub dir: String,
    pub host_name: String,
    pub capture_subdomains: Option<bool>,
    pub enable_lets_encrypt: Option<bool>,
    pub enable_directory_browsing: Option<bool>,
    pub redirect_to_https: Option<bool>,
    pub render_markdown: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct InProcessSiteConfig {
    #[serde(skip)]
    _proc_id: (),

    #[serde(skip)]
    pub active_port: Option<u16>,

    pub disable_tcp_tunnel_mode: Option<bool>,
    pub hints: Option<Vec<Hint>>,
    pub host_name: String,
    pub dir: Option<String>,
    pub bin: String,
    pub args: Option<Vec<String>>,
    pub env_vars: Option<Vec<EnvVar>>,
    pub log_format: Option<LogFormat>,
    pub auto_start: Option<bool>,
    pub port: Option<u16>,
    pub https: Option<bool>,
    pub capture_subdomains: Option<bool>,
    pub forward_subdomains: Option<bool>,
    pub exclude_from_start_all: Option<bool>,
    pub enable_lets_encrypt: Option<bool>,
    pub log_level: Option<LogLevel>,
}

impl PartialEq for InProcessSiteConfig {
    fn eq(&self, other: &Self) -> bool {
        self.log_level == other.log_level
            && self.disable_tcp_tunnel_mode == other.disable_tcp_tunnel_mode
            && self.hints == other.hints
            && self.host_name == other.host_name
            && self.dir == other.dir
            && self.bin == other.bin
            && self.args == other.args
            && self.env_vars == other.env_vars
            && self.log_format == other.log_format
            && self.auto_start == other.auto_start
            && self.port == other.port
            && self.https == other.https
            && self.capture_subdomains == other.capture_subdomains
            && self.forward_subdomains == other.forward_subdomains
            && self.exclude_from_start_all == other.exclude_from_start_all
    }
}

impl Eq for InProcessSiteConfig {}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
pub enum Hint {
    H2,
    H2C,
    H2CPK,
    NOH2,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Backend {
    pub address: String,
    pub port: u16,
    pub https: Option<bool>,
    pub hints: Option<Vec<Hint>>,
}

#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
pub struct RemoteSiteConfig {
    pub host_name: String,
    pub backends: Vec<Backend>,
    pub capture_subdomains: Option<bool>,
    pub disable_tcp_tunnel_mode: Option<bool>,
    pub forward_subdomains: Option<bool>,
    pub enable_lets_encrypt: Option<bool>,
    pub keep_original_host_header: Option<bool>,
}

impl PartialEq for RemoteSiteConfig {
    fn eq(&self, other: &Self) -> bool {
        self.host_name == other.host_name
            && self.backends == other.backends
            && self.keep_original_host_header == other.keep_original_host_header
            && self.enable_lets_encrypt == other.enable_lets_encrypt
            && self.capture_subdomains == other.capture_subdomains
            && self.disable_tcp_tunnel_mode == other.disable_tcp_tunnel_mode
            && self.forward_subdomains == other.forward_subdomains
    }
}

impl Eq for RemoteSiteConfig {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct V2Config {
    #[serde(skip)]
    pub path: Option<String>,

    pub version: super::OddBoxConfigVersion,
    pub root_dir: Option<String>,

    #[serde(default = "default_log_level")]
    pub log_level: Option<LogLevel>,

    #[serde(default = "true_option")]
    pub alpn: Option<bool>,

    #[serde(default = "default_port_range_start")]
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

    #[serde(default = "Vec::<EnvVar>::new")]
    pub env_vars: Vec<EnvVar>,

    pub remote_target: Option<Vec<RemoteSiteConfig>>,
    pub hosted_process: Option<Vec<InProcessSiteConfig>>,
    pub dir_server: Option<Vec<DirServer>>,
    pub admin_api_port: Option<u16>,
    pub lets_encrypt_account_email: Option<String>,
    pub odd_box_url: Option<String>,
    pub odd_box_password: Option<String>,
}

fn default_log_level() -> Option<LogLevel> {
    Some(LogLevel::Info)
}
fn default_log_format() -> LogFormat {
    LogFormat::standard
}
fn default_port_range_start() -> u16 {
    4200
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

// V1 → V2
impl TryFrom<super::v1::V1Config> for V2Config {
    type Error = String;

    fn try_from(old: super::v1::V1Config) -> Result<Self, Self::Error> {
        Ok(V2Config {
            odd_box_password: None,
            odd_box_url: None,
            dir_server: None,
            lets_encrypt_account_email: None,
            path: None,
            version: super::OddBoxConfigVersion::V2,
            admin_api_port: None,
            alpn: Some(false),
            auto_start: old.auto_start,
            default_log_format: old.default_log_format,
            env_vars: old.env_vars,
            ip: old.ip,
            log_level: old.log_level,
            http_port: old.http_port,
            port_range_start: old.port_range_start,
            hosted_process: Some(
                old.hosted_process
                    .unwrap_or_default()
                    .into_iter()
                    .map(|x| InProcessSiteConfig {
                        log_level: None,
                        enable_lets_encrypt: Some(false),
                        _proc_id: (),
                        active_port: None,
                        forward_subdomains: x.forward_subdomains,
                        disable_tcp_tunnel_mode: x.disable_tcp_tunnel_mode,
                        args: if x.args.is_empty() {
                            None
                        } else {
                            Some(x.args)
                        },
                        auto_start: x.auto_start,
                        bin: x.bin,
                        capture_subdomains: x.capture_subdomains,
                        env_vars: if x.env_vars.is_empty() {
                            None
                        } else {
                            Some(x.env_vars)
                        },
                        host_name: x.host_name,
                        port: x.port,
                        log_format: x.log_format,
                        dir: if x.dir.is_empty() { None } else { Some(x.dir) },
                        https: x.https,
                        hints: match x.h2_hint {
                            Some(super::v1::H2Hint::H2) => Some(vec![Hint::H2]),
                            Some(super::v1::H2Hint::H2C) => Some(vec![Hint::H2C]),
                            None => None,
                        },
                        exclude_from_start_all: x.disabled,
                    })
                    .collect(),
            ),
            remote_target: Some(
                old.remote_target
                    .unwrap_or_default()
                    .iter()
                    .map(|x| RemoteSiteConfig {
                        keep_original_host_header: None,
                        enable_lets_encrypt: Some(false),
                        disable_tcp_tunnel_mode: x.disable_tcp_tunnel_mode,
                        capture_subdomains: x.capture_subdomains,
                        forward_subdomains: x.forward_subdomains,
                        backends: vec![Backend {
                            hints: match &x.h2_hint {
                                Some(super::v1::H2Hint::H2) => Some(vec![Hint::H2]),
                                Some(super::v1::H2Hint::H2C) => Some(vec![Hint::H2C]),
                                None => None,
                            },
                            address: x.target_hostname.clone(),
                            port: x.port.unwrap_or(if x.https.unwrap_or_default() {
                                443
                            } else {
                                80
                            }),
                            https: x.https,
                        }],
                        host_name: x.host_name.clone(),
                    })
                    .collect(),
            ),
            root_dir: old.root_dir,
            tls_port: old.tls_port,
        })
    }
}
