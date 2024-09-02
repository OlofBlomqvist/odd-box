use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::path::Path;

use anyhow::bail;
use dashmap::DashMap;
use serde::Serialize;
use serde::Deserialize;
use utoipa::ToSchema;
use crate::global_state::GlobalState;
use crate::types::app_state::ProcState;
use crate::ProcId;

use super::ConfigWrapper;
use super::EnvVar;
use super::LogFormat;
use super::LogLevel;
use super::OddBoxConfiguration;


#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash)]
pub struct InProcessSiteConfig {
    
    #[serde(skip, default = "crate::ProcId::new")] 
    proc_id : ProcId,

    /// This is set automatically each time we start a process so that we know which ports are in use
    /// and can avoid conflicts when starting new processes. settings this in toml conf file will have no effect.
    #[serde(skip)] // <-- dont want to even read or write this to the config file, nor exposed in the api docs
    pub active_port : Option<u16>,

    /// This is mostly useful in case the target uses SNI sniffing/routing
    pub disable_tcp_tunnel_mode : Option<bool>,
    /// H2C or H2 - used to signal use of prior knowledge http2 or http2 over clear text. 
    pub hints : Option<Vec<Hint>>,
    pub host_name : String,
    pub dir : Option<String>,
    pub bin : String,
    pub args : Option<Vec<String>>,
    pub env_vars : Option<Vec<EnvVar>>,
    pub log_format: Option<LogFormat>,
    /// Set this to false if you do not want this site to start automatically when odd-box starts.
    /// This also means that the site is excluded from the start_all command.
    pub auto_start: Option<bool>,
    /// If this is set to None, the next available port will be used. Starting from the global port_range_start
    pub port: Option<u16>,
    pub https : Option<bool>,
    /// If you wish to use wildcard routing for any subdomain under the 'host_name'
    pub capture_subdomains : Option<bool>,
    /// If you wish to use the subdomain from the request in forwarded requests:
    /// test.example.com -> internal.site
    /// vs
    /// test.example.com -> test.internal.site 
    pub forward_subdomains : Option<bool>,
    /// If you wish to exclude this site from the start_all command.
    /// This setting was previously called "disable" but has been renamed for clarity
    pub exclude_from_start_all: Option<bool>
}


#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash)]
pub struct FullyResolvedInProcessSiteConfig {
    pub excluded_from_start_all: bool,
    pub proc_id : ProcId,
    pub active_port : Option<u16>,
    pub disable_tcp_tunnel_mode : Option<bool>,
    pub hints : Option<Vec<Hint>>,
    pub host_name : String,
    pub dir : Option<String>,
    pub bin : String,
    pub args : Option<Vec<String>>,
    pub env_vars : Option<Vec<EnvVar>>,
    pub log_format: Option<LogFormat>,
    pub auto_start: Option<bool>,
    pub port: Option<u16>,
    pub https : Option<bool>,
    pub capture_subdomains : Option<bool>,
    pub forward_subdomains : Option<bool>,
}

impl InProcessSiteConfig {
    pub fn get_id(&self) -> &ProcId {
        &self.proc_id
    }
}

impl PartialEq for InProcessSiteConfig {
    
    fn eq(&self, other: &Self) -> bool {
        compare_option_bool(self.disable_tcp_tunnel_mode,other.disable_tcp_tunnel_mode) &&
        self.hints == other.hints &&
        self.host_name == other.host_name &&
        self.dir == other.dir &&
        self.bin == other.bin &&
        self.args == other.args &&
        self.env_vars == other.env_vars &&
        compare_option_log_format(&self.log_format,& other.log_format) &&
        compare_option_bool(self.auto_start, other.auto_start) &&
        self.port == other.port &&
        self.https == other.https &&
        compare_option_bool(self.capture_subdomains, other.capture_subdomains) &&
        compare_option_bool(self.forward_subdomains, other.forward_subdomains)
    }
}

impl Eq for InProcessSiteConfig {}
fn compare_option_bool(a: Option<bool>, b: Option<bool>) -> bool {
    let result = match (a, b) {
        (None, Some(false)) | (Some(false), None) => true,
        _ => a == b,
    };
    println!("Comparing Option<bool>: {:?} vs {:?} -- result: {result}", a, b);
    result
}

fn compare_option_log_format(a: &Option<LogFormat>, b: &Option<LogFormat>) -> bool {
    let result = match (a, b) {
        (None, Some(LogFormat::standard)) | (Some(LogFormat::standard), None) => true,
        _ => a == b,
    };
    println!("Comparing Option<LogFormat>: {:?} vs {:?} -- result: {result}", a, b);
    result
}

#[derive(Debug, Eq,PartialEq,Hash, Clone, Serialize, Deserialize, ToSchema)]
pub enum Hint {
    /// Server supports http2 over tls
    H2,
    /// Server supports http2 via clear text by using an upgrade header
    H2C,
    /// Server supports http2 via clear text by using prior knowledge
    H2CPK
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema,Eq,PartialEq,Hash,)]
pub struct Backend {
    pub address : String,
    /// This can be zero in case the backend is a hosted process, in which case we will need to resolve the current active_port
    pub port: u16,
    pub https : Option<bool>,
    /// H2C,H2,H2CPK - used to signal use of prior knowledge http2 or http2 over clear text. 
    pub hints : Option<Vec<Hint>>,
}

#[derive(Debug, Hash, Clone, Serialize, Deserialize, ToSchema)]
pub struct RemoteSiteConfig{
    pub host_name : String,
    pub backends : Vec<Backend>,
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

impl PartialEq for RemoteSiteConfig {
    fn eq(&self, other: &Self) -> bool {
        self.host_name == other.host_name &&
        self.backends == other.backends &&
        compare_option_bool(self.capture_subdomains, other.capture_subdomains) &&
        compare_option_bool(self.disable_tcp_tunnel_mode, other.disable_tcp_tunnel_mode) &&
        compare_option_bool(self.forward_subdomains, other.forward_subdomains)
    }
}

impl Eq for RemoteSiteConfig {}

pub enum BackendFilter {
    Http,
    Https,
    Any
}
fn filter_backend(backend: &Backend, filter: &BackendFilter) -> bool {
    match filter {
        BackendFilter::Http => backend.https.unwrap_or_default() == false,
        BackendFilter::Https => backend.https.unwrap_or_default() == true,
        BackendFilter::Any => true
    }
}

impl RemoteSiteConfig {


    pub async fn next_backend(&self,state:&GlobalState, backend_filter: BackendFilter) -> Option<Backend> {
            
        let filtered_backends = self.backends.iter().filter(|x|filter_backend(x,&backend_filter))
            .collect::<Vec<&crate::configuration::v2::Backend>>();

        if filtered_backends.len() == 1 { return Some(filtered_backends[0].clone()) };
        if filtered_backends.len() == 0 { return None };
        
       
        let count = match state.app_state.statistics.remote_targets_stats.get_mut(&self.host_name) {
            Some(mut guard) => {
                let (_k,v) = guard.pair_mut();
                1 + v.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            },
            None => {
                state.app_state.statistics.remote_targets_stats.insert(self.host_name.clone(), std::sync::atomic::AtomicUsize::new(1));
                1
            }
        } as usize;

        let selected_backend = *filtered_backends.get((count % (filtered_backends.len() as usize)) as usize )
            .expect("we should always have at least one backend but found none. this is a bug in oddbox <service.rs>.");



        Some(selected_backend.clone())
        

    }
}

#[derive(Debug, Clone, Serialize, Deserialize,ToSchema, PartialEq, Eq, Hash)]
pub struct OddBoxV2Config {
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

impl crate::configuration::OddBoxConfiguration<OddBoxV2Config> for OddBoxV2Config {



    fn write_to_disk(&self) -> anyhow::Result<()> {
    
        let current_path = if let Some(p) = &self.path {p} else {
            bail!(ConfigurationUpdateError::Bug("No path found to the current configuration".into()))
        };
    
        let formatted_toml = self.to_string()?;
    
        let original_path = Path::new(&current_path);
        let backup_path = original_path.with_extension("toml.backup");
        std::fs::rename(original_path, &backup_path)?;
    
        if let Err(e) = std::fs::write(current_path, formatted_toml) {
            bail!("Failed to write config to disk: {e}")
        } else {
            Ok(())
        }
    
    }
    
    fn to_string(&self) -> anyhow::Result<String>  {
        let mut formatted_toml = Vec::new();

        formatted_toml.push(format!("version = \"{:?}\"", self.version));
        
        if let Some(alpn) = self.alpn {
            formatted_toml.push(format!("alpn = {}", alpn));
        } else {
            formatted_toml.push(format!("alpn = {}", "false"));
        }

        if let Some(port) = self.http_port {
            formatted_toml.push(format!("http_port = {}", port));
        }
        if let Some(port) = self.admin_api_port {
            formatted_toml.push(format!("admin_api_port = {}", port));
        }
        if let Some(ip) = &self.ip {
            formatted_toml.push(format!("ip = \"{:?}\"", ip));
        } else {
            formatted_toml.push(format!("ip = \"127.0.0.1\""));
        }
        if let Some(tls_port) = self.tls_port {
            formatted_toml.push(format!("tls_port = {}", tls_port));
        }
        if let Some(auto_start) = self.auto_start {
            formatted_toml.push(format!("auto_start = {}", auto_start));
        } else {
            formatted_toml.push(format!("auto_start = false"));
        }

        if let Some(root_dir) = &self.root_dir {
            formatted_toml.push(format!("root_dir = {:?}", root_dir));
        } else {
            formatted_toml.push(format!("root_dir = \"~\""));
        }
        if let Some(log_level) = &self.log_level {
            formatted_toml.push(format!("log_level = \"{:?}\"", log_level));
        }
        formatted_toml.push(format!("port_range_start = {}", self.port_range_start));

     
        formatted_toml.push(format!("default_log_format = \"{:?}\"", self.default_log_format ));
       

        if &self.env_vars.len() > &0 {
            formatted_toml.push("env_vars = [".to_string());
            for env_var in &self.env_vars {
                formatted_toml.push(format!(
                    "\t{{ key = {:?}, value = {:?} }},",
                    env_var.key, env_var.value
                ));
            }
            formatted_toml.push("]".to_string());
        }
        
        if let Some(remote_sites) = &self.remote_target {
            for site in remote_sites {
                formatted_toml.push("\n[[remote_target]]".to_string());
                formatted_toml.push(format!("host_name = {:?}", site.host_name));
           
                if let Some(true) = site.forward_subdomains {
                    formatted_toml.push(format!("forward_subdomains = true"));
                }
                
                if let Some(true) = site.capture_subdomains {
                    formatted_toml.push(format!("capture_subdomains = true"));
                }
                
                if let Some(true) = site.disable_tcp_tunnel_mode {
                    formatted_toml.push(format!("disable_tcp_tunnel_mode = {}", true));
                }

                formatted_toml.push("backends = [".to_string());

                let backend_strings = site.backends.iter().map(|b| {
                    let https = if let Some(true) = b.https { format!("https = true, ") } else { format!("") };
                    
                    let hints = if let Some(hints) = &b.hints {
                        format!(", hints = [{}]",hints.iter().map(|h|format!("{h:?}")).collect::<Vec<String>>().join(", "))
                    } else {
                        String::new()
                    };
                    
                    format!("\t{{ {}address=\"{}\", port={}{hints}}}",https,b.address, b.port)}

                ).collect::<Vec<String>>();

                formatted_toml.push(backend_strings.join(",\n"));

                formatted_toml.push("]".to_string());
           
            }
        }

        if let Some(processes) = &self.hosted_process {
            for process in processes {
                formatted_toml.push("\n[[hosted_process]]".to_string());
                formatted_toml.push(format!("host_name = {:?}", process.host_name));
                if let Some(d) = &process.dir {
                    formatted_toml.push(format!("dir = {:?}", d));
                }

                formatted_toml.push(format!("bin = {:?}", process.bin));

                if let Some(hint) = &process.hints {
                    formatted_toml.push("hints = [".to_string());
                    let joint = hint.iter().map(|h| format!("{:?}", h)).collect::<Vec<String>>().join(", ");
                    formatted_toml.push(joint);
                    formatted_toml.push("]".to_string());
                }
                
                let args = process.args.clone().unwrap_or_default().iter()
                    .map(|arg| format!("\n  {:?}", arg)).collect::<Vec<_>>().join(", ");

                formatted_toml.push(format!("args = [{}\n]", args));
                
             
                if let Some(auto_start) = process.auto_start {
                    formatted_toml.push(format!("auto_start = {}", auto_start));
                }
                
                
                if let Some(b) = process.https {
                    formatted_toml.push(format!("https = {}", b));
                }
                if let Some(port) = process.port {
                    formatted_toml.push(format!("port = {}", port));
                }
                
                if let Some(true) = process.capture_subdomains {
                    formatted_toml.push(format!("capture_subdomains = {}", "true"));
                }

                if let Some(evars) = &process.env_vars {
                    formatted_toml.push("env_vars = [".to_string());
                    for env_var in evars {
                        formatted_toml.push(format!(
                            "\t{{ key = {:?}, value = {:?} }},",
                            env_var.key, env_var.value
                        ));
                    }
                    formatted_toml.push("]".to_string());
                }
                

            }
        }
        Ok(formatted_toml.join("\n"))
    }
    fn example() -> OddBoxV2Config {
        OddBoxV2Config {
            path: None,
            admin_api_port: None,
            version: super::OddBoxConfigVersion::V2,
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
                    proc_id: crate::ProcId::new(),
                    active_port: None,
                    forward_subdomains: None,
                    disable_tcp_tunnel_mode: Some(false),
                    args: Some(vec!["--test".to_string()]),
                    auto_start: Some(true),
                    bin: "my_bin".into(),
                    capture_subdomains: None,
                    env_vars: Some(vec![
                        EnvVar { key: "some_key".into(), value:"some_val".into() },
                        EnvVar { key: "another_key".into(), value:"another_val".into() },
                    ]),
                    host_name: "some_host.local".into(),
                    port: Some(443) ,
                    log_format: Some(LogFormat::standard),
                    dir: None,
                    https: Some(true),
                    hints: None,
                    exclude_from_start_all: None
                    
                }
            ]),
            remote_target: Some(vec![
                RemoteSiteConfig { 
                    forward_subdomains: None,
                    host_name: "lobsters.local".into(), 
                    backends: vec![
                        Backend {
                            hints: None, 
                            address: "lobste.rs".into(), 
                            port: 443, 
                            https: Some(true)
                        }
                    ], 
                    capture_subdomains: Some(false), 
                    disable_tcp_tunnel_mode: Some(false)
                },
                RemoteSiteConfig { 
                    forward_subdomains: Some(true),                    
                    host_name: "google.local".into(), 
                    backends: vec![
                        Backend {
                            hints: None, 
                            address: "google.com".into(), 
                            port: 443, 
                            https: Some(true)
                        }
                    ], 
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


#[derive(Debug)]
enum ConfigurationUpdateError {
    Bug(String)
}


impl std::fmt::Display for ConfigurationUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ConfigurationUpdateError::NotFound => {
            //     f.write_str("No such hosted process found.")
            // },
            // ConfigurationUpdateError::FailedToSave(e) => {
            //     f.write_fmt(format_args!("Failed to save due to error: {}",e))
            // },
            ConfigurationUpdateError::Bug(e) => {
                f.write_fmt(format_args!("Failed to save due to a bug in odd-box: {}",e))
            }
        }
    }
}





// V1 ---> V2
impl TryFrom<super::v1::OddBoxV1Config> for super::v2::OddBoxV2Config{

    type Error = String;

    fn try_from(old_config: super::v1::OddBoxV1Config) -> Result<Self, Self::Error> {
        let new_config = super::v2::OddBoxV2Config {
            path: None,
            version: super::OddBoxConfigVersion::V2,
            admin_api_port: None,
            alpn: Some(false), // allowing alpn would be a breaking change for h2c when using old configuration format
            auto_start: old_config.auto_start,
            default_log_format: old_config.default_log_format,
            env_vars: old_config.env_vars,
            ip: old_config.ip,
            log_level: old_config.log_level,
            http_port: old_config.http_port,
            port_range_start: old_config.port_range_start,
            hosted_process: Some(old_config.hosted_process.unwrap_or_default().into_iter().map(|x|{
                super::v2::InProcessSiteConfig {
                    exclude_from_start_all: None,
                    proc_id: crate::ProcId::new(),
                    active_port: None,
                    forward_subdomains: x.forward_subdomains,
                    disable_tcp_tunnel_mode: x.disable_tcp_tunnel_mode,
                    args: if x.args.len() > 0 { Some(x.args) } else { None },
                    auto_start: {
                        if x.disabled != x.auto_start {
                            tracing::warn!("Your configuration contains both auto_start and disabled for the same process. The auto_start setting will be used. Please remove the disabled setting as it is no longer used.")
                        }
                        if let Some(d) = x.disabled {
                            Some(!d)
                        } else if let Some(a) = x.auto_start { 
                            Some(a) 
                        } else { 
                            None 
                        }
                    },
                    bin: x.bin,
                    capture_subdomains: x.capture_subdomains,
                    env_vars: if x.env_vars.len() > 0 { Some(x.env_vars) } else { None },
                    host_name: x.host_name,
                    port: x.port,
                    log_format: x.log_format,
                    dir: if x.dir.is_empty() { None } else { Some(x.dir) },
                    https: x.https,
                    hints: match x.h2_hint {
                        Some(super::H2Hint::H2) => Some(vec![crate::configuration::v2::Hint::H2]),
                        Some(super::H2Hint::H2C) => Some(vec![crate::configuration::v2::Hint::H2C]),               
                        None => None,
                    }
                    
                }
            }).collect()),
            remote_target: Some(old_config.remote_target.unwrap_or_default().iter().map(|x|{
                super::v2::RemoteSiteConfig {
                    disable_tcp_tunnel_mode: x.disable_tcp_tunnel_mode,
                    capture_subdomains: x.capture_subdomains,
                    forward_subdomains: x.forward_subdomains,
                    backends: vec![
                        super::v2::Backend {
                            hints: match x.h2_hint {
                                Some(super::H2Hint::H2) => Some(vec![crate::configuration::v2::Hint::H2]),
                                Some(super::H2Hint::H2C) => Some(vec![crate::configuration::v2::Hint::H2C]),               
                                None => None,
                            },
                            address: x.target_hostname.clone(),
                            port: if let Some(p) = x.port {p} else {
                                if x.https.unwrap_or_default() { 443 } else { 80 }
                            },
                            https: x.https
                        }
                    ],
                    host_name: x.host_name.clone(),                    
                }
            }).collect()),
            root_dir: old_config.root_dir,
            tls_port: old_config.tls_port

        };
        Ok(new_config)
    }
}