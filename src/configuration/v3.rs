use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::vec;
use schemars::JsonSchema;
use anyhow::bail;
use serde::Serialize;
use serde::Deserialize;
use utoipa::ToSchema;
use crate::global_state::GlobalState;
use crate::types::proc_info::ProcId;

use super::EnvVar;
use super::LogFormat;
use super::LogLevel;

/// A directory server configuration allows you to serve files from a directory on the local filesystem.
/// Both unencrypted (http) and encrypted (https) connections are supported, either self-signed or thru lets-encrypt.
/// You can specify rules for how the cache should behave, and you can also specify rules for how the files should be served.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash, JsonSchema, PartialEq, Eq)]
pub struct DirServer {
    pub dir : String,
    /// This is the hostname that the site will respond to.
    pub host_name : String,
    /// Instead of only listening to yourdomain.com, you can capture subdomains which means this site will also respond to requests for *.yourdomain.com
    pub capture_subdomains : Option<bool>,
    pub enable_lets_encrypt: Option<bool>,
    pub enable_directory_browsing: Option<bool>,
    pub redirect_to_https: Option<bool>,
    //pub rules: Option<Vec<ReqRule>>,
    // --- todo --------------------------------------
    pub render_markdown: Option<bool>,
}

// note: there is no implementation using these yet..
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash, JsonSchema, PartialEq, Eq)]
pub struct ReqRule {
    
    /// The max age in seconds for the cache. If this is set to None, the cache will be disabled.
    /// This setting causes odd-box to add a Cache-Control header to the response.
    pub max_age_in_seconds: Option<u64>,
    /// Full url path of the file this rule should apply to, or a regex pattern for the url.
    /// For example: /index.html or /.*\.html
    pub path_pattern: Option<String>,
    /// If no index.html is found, you can set this to true to allow directory browsing.
    pub allow_directory_browsing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash, JsonSchema)]
pub struct InProcessSiteConfig {
    
    #[serde(skip, default = "crate::types::proc_info::ProcId::new")] 
    proc_id : ProcId,

    /// This is set automatically each time we start a process so that we know which ports are in use
    /// and can avoid conflicts when starting new processes. settings this in toml conf file will have no effect.
    #[serde(skip)] // <-- dont want to even read or write this to the config file, nor exposed in the api docs
    pub active_port : Option<u16>,
    /// This is mostly useful in case the target uses SNI sniffing/routing
    pub disable_tcp_tunnel_mode : Option<bool>,
    /// H1,H2,H2C,H2CPK,H3 - empty means H1 is expected to work with passthru: everything else will be 
    /// using terminating mode.
    pub hints : Option<Vec<Hint>>,
    pub host_name : String,
    /// Working directory for the process. If this is not set, the current working directory will be used.
    pub dir : Option<String>,
    /// The binary to start. This can be a path to a binary or a command that is in the PATH.
    pub bin : String,
    /// Arguments to pass to the binary when starting it.
    pub args : Option<Vec<String>>,
    /// Environment variables to set for the process.
    pub env_vars : Option<Vec<EnvVar>>,
    /// The log format to use for this site. If this is not set, the default log format will be used.
    /// Currently the only supported log formats are "standard" and "dotnet".
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
    pub exclude_from_start_all: Option<bool>,
    /// If you want to use lets-encrypt for generating certificates automatically for this site.
    /// Defaults to false. This feature will disable tcp tunnel mode.
    pub enable_lets_encrypt: Option<bool>,
    /// If you wish to set a specific loglevel for this hosted process.
    /// Defaults to "Info".
    /// If this level is lower than the global log_level you will get the message elevated to the global log level instead but tagged with the actual log level.
    pub log_level: Option<LogLevel>
}
impl InProcessSiteConfig {
    pub fn set_id(&mut self,id:ProcId){
        self.proc_id = id;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Hash,Eq,PartialEq)]
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
    pub log_level: Option<LogLevel>,
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
        self.log_level.eq(&other.log_level) &&
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
        compare_option_bool(self.forward_subdomains, other.forward_subdomains) &&
        compare_option_bool(self.exclude_from_start_all, other.exclude_from_start_all)
        
    }
}

impl Eq for InProcessSiteConfig {}
fn compare_option_bool(a: Option<bool>, b: Option<bool>) -> bool {
    let result = match (a, b) {
        (None, Some(false)) | (Some(false), None) => true,
        _ => a == b,
    };
    //println!("Comparing Option<bool>: {:?} vs {:?} -- result: {result}", a, b);
    result
}

fn compare_option_log_format(a: &Option<LogFormat>, b: &Option<LogFormat>) -> bool {
    let result = match (a, b) {
        (None, Some(LogFormat::standard)) | (Some(LogFormat::standard), None) => true,
        _ => a == b,
    };
    //println!("Comparing Option<LogFormat>: {:?} vs {:?} -- result: {result}", a, b);
    result
}


#[derive(Debug, Eq,PartialEq,Hash, Clone, Serialize, Deserialize, ToSchema, JsonSchema)]
pub enum Hint {
    /// Server supports http2 over tls
    H2,
    /// Server supports http2 via clear text by using an upgrade header
    H2C,
    /// Server supports http2 via clear text by using prior knowledge
    H2CPK,
    /// Server supports http1.x
    H1,
    /// Server supports http3
    H3
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema,Eq,PartialEq,Hash, JsonSchema)]
pub struct Backend {
    pub address : String,
    /// This can be zero in case the backend is a hosted process, in which case we will need to resolve the current active_port
    pub port: u16,
    pub https : Option<bool>,
    /// H2C,H2,H2CPK - used to signal use of prior knowledge http2 or http2 over clear text. 
    pub hints : Option<Vec<Hint>>,
}

#[derive(Debug, Hash, Clone, Serialize, Deserialize, ToSchema, JsonSchema)]
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
    pub forward_subdomains : Option<bool>,
    /// If you want to use lets-encrypt for generating certificates automatically for this site.
    /// Defaults to false. This feature will disable tcp tunnel mode.
    pub enable_lets_encrypt: Option<bool>,
    /// If you wish to pass along the incoming request host header to the backend
    /// rather than the host name of the backends. Defaults to false.
    pub keep_original_host_header: Option<bool>,
}

impl PartialEq for RemoteSiteConfig {
    fn eq(&self, other: &Self) -> bool {
        self.host_name == other.host_name &&
        self.backends == other.backends &&
        compare_option_bool(self.keep_original_host_header,other.keep_original_host_header) &&
        compare_option_bool(self.enable_lets_encrypt,other.enable_lets_encrypt) &&
        compare_option_bool(self.capture_subdomains, other.capture_subdomains) &&
        compare_option_bool(self.disable_tcp_tunnel_mode, other.disable_tcp_tunnel_mode) &&
        compare_option_bool(self.forward_subdomains, other.forward_subdomains)
    }
}

impl Eq for RemoteSiteConfig {}

#[derive(Debug, Clone,)]
pub enum BackendFilter {
    Any,
    Http2, // implies tls
    Http1,
    H2CPriorKnowledge,
    H2C,
    AnyTLS
}


fn filter_backend(backend: &Backend, filter: &BackendFilter) -> bool {

    let hints = backend.hints.iter().flatten().collect::<Vec<&Hint>>();
    

    match filter {
        BackendFilter::Any => true,
        BackendFilter::AnyTLS => backend.https.unwrap_or_default(),
        BackendFilter::Http2 => 
            // only allow http2 if the backend explicitly supports it
            hints.iter().any(|h|**h == Hint::H2),
        BackendFilter::Http1 =>
            // if no hints are set, we assume http1 is supported
            hints.len() == 0 || hints.iter().any(|h|**h == Hint::H1) 
        ,
        BackendFilter::H2CPriorKnowledge => 
            hints.iter().any(|h|**h == Hint::H2CPK),
        BackendFilter::H2C => 
            hints.iter().any(|h|**h == Hint::H2C),
            
        
    }
}

impl InProcessSiteConfig {


    // TODO - MAJOR:

    // we removed the "tunneled" arg from the other next_backend...
    // now we dont properly add statistics for the tunnelled connections when we are in remote tunnel mode...
    // need to add back.


    pub async fn next_backend(&self,state:&GlobalState, backend_filter: BackendFilter) -> Option<Backend> {
        
        // port needs to be: 
        // - self.active_port, if it is set.
        // - otherwise, self.port if it is set.
        // - otherwise, 443 if https is set.
        // - otherwise, 80
        let port = if let Some(p) = self.active_port { p } else {
            if let Some(p) = self.port { p } else {
                if self.https.unwrap_or_default() { 443 } else { 80 }
            }
        };

        let backends = vec![Backend {
            address: "localhost".to_string(), // we are always connecting to localhost since we are the ones hosting this process
            port: port,
            https: self.https,
            hints: self.hints.clone(),
        }];

          
        let filtered_backends = backends.iter().filter(|x|filter_backend(x,&backend_filter))
            .collect::<Vec<&Backend>>();

        if filtered_backends.len() == 1 { return Some(filtered_backends[0].clone()) };
        if filtered_backends.len() == 0 { return None };
        
        let current_req_count_for_target_host_name = {
            state.app_state.statistics.connections_per_hostname
            .get(&self.host_name).and_then(|x|Some(x.load(std::sync::atomic::Ordering::SeqCst)))
            .unwrap_or(0)
        };

        let selected_backend = filtered_backends.get((current_req_count_for_target_host_name % (filtered_backends.len() as usize)) as usize );

        if let Some(b) = selected_backend{
            Some((*b).clone())
        } else {
            tracing::error!("Could not find a backend for host: {:?}",self.host_name);
            None
        }
        

    }
}
impl RemoteSiteConfig {


    pub async fn next_backend(&self,state:&GlobalState, backend_filter: BackendFilter) -> Option<Backend> {
            
        let filtered_backends = self.backends.iter().filter(|x|filter_backend(x,&backend_filter))
            .collect::<Vec<&Backend>>();

        if filtered_backends.len() == 1 { return Some(filtered_backends[0].clone()) };
        if filtered_backends.len() == 0 { return None };
        
        let current_req_count_for_target_host_name = {
            state.app_state.statistics.connections_per_hostname
            .get(&self.host_name).and_then(|x|Some(x.load(std::sync::atomic::Ordering::SeqCst)))
            .unwrap_or(0)
        };

        let selected_backend = filtered_backends.get((current_req_count_for_target_host_name % (filtered_backends.len() as usize)) as usize );

        if let Some(b) = selected_backend{
            Some((*b).clone())
        } else {
            tracing::error!("Could not find a backend for host: {:?}",self.host_name);
            None
        }
        

    }
}


#[derive(Debug,Clone,Serialize,Deserialize,Default,ToSchema,PartialEq, Eq, Hash, schemars::JsonSchema)]
pub enum V3VersionEnum {
    #[default] V3
}

#[derive(Debug, Clone, Serialize, Deserialize,ToSchema, PartialEq, Eq, Hash, JsonSchema)]
pub struct OddBoxV3Config {
    
    #[serde(skip)] // only used internally by odd-box to keep track of where the configuration file is located
    pub path : Option<String>, 
    
    /// The schema version - you do not normally need to set this, it is set automatically when you save the configuration.
    #[schema(value_type = String)]
    pub version : V3VersionEnum,

    /// Optionally configure the $root_dir variable which you can use in environment variables, paths and other settings.
    /// By default $root_dir will be $pwd (dir where odd-box is started).
    pub root_dir : Option<String>, 
    /// Log level of the odd-box application itself. Defaults to Info.
    /// For hosted processes, you can instead set the log level for site individually.
    #[serde(default = "default_log_level")]
    pub log_level : Option<LogLevel>,
    /// Defaults to true. Lets you enable/disable h2/http11 tls alpn algs during initial connection phase. 
    #[serde(default = "true_option")]
    pub alpn : Option<bool>,
    /// The port range start is used to determine which ports to use for hosted processes.
    #[serde(default = "default_port_range_start")]
    pub port_range_start : u16,
    #[serde(default = "default_log_format")]
    pub default_log_format : LogFormat,
    #[schema(value_type = String)]
    pub ip : Option<IpAddr>,
    /// The port on which to listen for http requests. Defaults to 8080.
    #[serde(default = "default_http_port_8080")]
    pub http_port : Option<u16>,
    /// The port on which to listen for https requests. Defaults to 4343.
    #[serde(default = "default_https_port_4343")]
    pub tls_port : Option<u16>,
    /// If this is set to false, odd-box will not start any hosted processes automatically when it starts
    /// unless they are set to auto_start individually. Same with true, it will start all processes that
    /// have not been specifically configured with auto_start=false.
    #[serde(default = "true_option")]
    pub auto_start : Option<bool>,
    /// Environment variables configured here will be made available to all processes started by odd-box.
    #[serde(default = "Vec::<EnvVar>::new")]
    pub env_vars : Vec<EnvVar>,
    /// Used to configure remote (or local sites not managed by odd-box) as a targets for requests.
    pub remote_target : Option<Vec<RemoteSiteConfig>>,
    /// Used to set up processes to keep running and serve requests on a specific hostname.
    /// This can be used to run a web server, a proxy, or any other kind of process that can handle http requests.
    /// It can also be used even if the process is not a web server and you just want to keep it running..
    pub hosted_process : Option<Vec<InProcessSiteConfig>>,
    /// Used for static websites.
    pub dir_server : Option<Vec<DirServer>>,
    /// If you want to use lets-encrypt for generating certificates automatically for your sites
    pub lets_encrypt_account_email: Option<String>,
    /// If you want to use a specific odd-box url for the admin api and web-interface you can 
    /// configure the host_name to listen on here. This is useful if you want to use a specific domain
    /// for the admin interface and the api. If you do not set this, the admin interface will be available
    /// on https://localhost and https://odd-box.localhost by default.
    /// If you configure this, you should also configure the odd_box_password property.
    pub odd_box_url : Option<String>,
    /// Used for securing the admin api and web-interface. If you do not set this, anyone can access the admin api.
    pub odd_box_password: Option<String>,

}


fn default_port_range_start() -> u16 {
    4200
}

impl crate::configuration::OddBoxConfiguration<OddBoxV3Config> for OddBoxV3Config {



    fn write_to_disk(&self) -> anyhow::Result<()> {
    
        let current_path = if let Some(p) = &self.path {p} else {
            bail!(ConfigurationUpdateError::Bug("No path found to the current configuration".into()))
        };
    
        let formatted_toml = self.to_string()?;
        
        if let Err(e) = std::fs::write(current_path, formatted_toml) {
            bail!("Failed to write config to disk: {e}")
        } else {
            Ok(())
        }
    }
    
    // note: this method exists because we want to keep a consistent format for the configuration file
    //       which is not guaranteed by the serde toml serializer.
    //       it unfortunately means that we have to maintain this method manually.
    // todo: should move to upper abstraction level, not reimplementing the whole thing?
    fn to_string(&self) -> anyhow::Result<String>  {

        if self.version != V3VersionEnum::V3  {
            panic!("This is a bug in odd-box. The configuration version is not V3. This should not happen.");
        }

        let mut formatted_toml = Vec::new();

        // this is to nudge editor plugins to use the correct schema for validation and intellisense
        formatted_toml.push(format!("#:schema https://raw.githubusercontent.com/OlofBlomqvist/odd-box/main/odd-box-schema-v2.2.json"));
        
        // this is for our own use to know which version of the configuration we are using
        formatted_toml.push(format!("version = \"{:?}\"", self.version));
        
        if let Some(alpn) = self.alpn {
            formatted_toml.push(format!("alpn = {}", alpn));
        } else {
            formatted_toml.push(format!("alpn = {}", "false"));
        }

        if let Some(port) = self.http_port {
            formatted_toml.push(format!("http_port = {}", port));
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
        
        if let Some(odd_box_url) = &self.odd_box_url {
            formatted_toml.push(format!("odd_box_url = {:?}", odd_box_url));
        }

        if let Some(odd_box_password) = &self.odd_box_password {
            formatted_toml.push(format!("odd_box_password = {:?}", odd_box_password));
        }

        formatted_toml.push(format!("port_range_start = {}", self.port_range_start));

     
        formatted_toml.push(format!("default_log_format = \"{:?}\"", self.default_log_format ));
       
        if let Some(email) = &self.lets_encrypt_account_email {
            formatted_toml.push(format!("lets_encrypt_account_email = \"{email}\""));
        }

        if &self.env_vars.len() > &0 {
            formatted_toml.push("env_vars = [".to_string());
            for env_var in &self.env_vars {
                formatted_toml.push(format!(
                    "\t{{ key = {:?}, value = {:?} }},",
                    env_var.key, env_var.value
                ));
            }
            formatted_toml.push("]".to_string());
        } else {
            formatted_toml.push("env_vars = []".to_string());
        }

        if let Some(dir_sites) = &self.dir_server {
            for s in dir_sites {
                formatted_toml.push("\n[[dir_server]]".to_string());
                formatted_toml.push(format!("host_name = {:?}", s.host_name));
                formatted_toml.push(format!("dir = {:?}", s.dir));
                if let Some(true) = s.capture_subdomains {
                    formatted_toml.push(format!("capture_subdomains = true"));
                }
                if let Some(true) = s.enable_directory_browsing {
                    formatted_toml.push(format!("enable_directory_browsing = true"));
                }
                if let Some(true) = s.enable_lets_encrypt {
                    formatted_toml.push(format!("enable_lets_encrypt = true"));
                }
                if let Some(true) = s.render_markdown {
                    formatted_toml.push(format!("render_markdown = true"));
                }
                if let Some(true) = s.redirect_to_https {
                    formatted_toml.push(format!("redirect_to_https = true"));
                }
            }
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

                if let Some(true) = site.enable_lets_encrypt {
                    formatted_toml.push(format!("enable_lets_encrypt = {}", true));
                }


                formatted_toml.push("backends = [".to_string());

                let backend_strings = site.backends.iter().map(|b| {
                    let https = if let Some(true) = b.https { format!("https = true, ") } else { format!("") };
                    
                    let hints = if let Some(hints) = &b.hints {
                        format!(", hints = [{}]",hints.iter().map(|h|format!("'{h:?}'")).collect::<Vec<String>>().join(", "))
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
                    let joint = hint.iter().map(|h| format!("'{:?}'", h)).collect::<Vec<String>>().join(", ");
                    formatted_toml.push(joint);
                    formatted_toml.push("]".to_string());
                }
                
                let args = process.args.iter().flatten()
                    .map(|arg| format!("\n  {:?}", arg)).collect::<Vec<_>>().join(", ");

                formatted_toml.push(format!("args = [{}\n]", args));
                
             
                if let Some(auto_start) = process.auto_start {
                    formatted_toml.push(format!("auto_start = {}", auto_start));
                }

                if let Some(log_level) = &process.log_level {
                    formatted_toml.push(format!("log_level = \"{:?}\"", log_level));
                }

                if let Some(true) = process.enable_lets_encrypt {
                    formatted_toml.push(format!("enable_lets_encrypt = {}", true));
                }

                if let Some(true) = process.exclude_from_start_all {
                    formatted_toml.push(format!("exclude_from_start_all = {}", true));
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
    fn example() -> OddBoxV3Config {
        OddBoxV3Config {
            odd_box_password: None,
            odd_box_url: None,
            dir_server: None,
            lets_encrypt_account_email: None,
            path: None,
            version: V3VersionEnum::V3,
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
                    log_level: None,
                    enable_lets_encrypt: Some(false),
                    proc_id: ProcId::new(),
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
                    keep_original_host_header: None,
                    enable_lets_encrypt: Some(false),
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
                    keep_original_host_header: None,
                    enable_lets_encrypt: Some(false),
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
impl TryFrom<super::v2::OddBoxV2Config> for OddBoxV3Config{

    type Error = String;

    fn try_from(old_config: super::v2::OddBoxV2Config) -> Result<Self, Self::Error> {
        let new_config = Self {
            odd_box_password: None,
            odd_box_url: None,
            dir_server: None,
            lets_encrypt_account_email: None,
            path: None,
            version: V3VersionEnum::V3,
            alpn: Some(false), // allowing alpn would be a breaking change for h2c when using old configuration format
            auto_start: old_config.auto_start,
            default_log_format: old_config.default_log_format,
            env_vars: old_config.env_vars,
            ip: old_config.ip,
            log_level: old_config.log_level,
            http_port: old_config.http_port,
            port_range_start: old_config.port_range_start,
            hosted_process: Some(old_config.hosted_process.unwrap_or_default().into_iter().map(|x|{
                let mut new_hints : Vec<Hint> = x.hints.iter().flatten().filter_map(|x| match x {
                    &crate::configuration::v2::Hint::H2 => Some(Hint::H2),
                    &crate::configuration::v2::Hint::H2C => Some(Hint::H2C),
                    &crate::configuration::v2::Hint::H2CPK => Some(Hint::H2CPK),
                    &crate::configuration::v2::Hint::NOH2 => None, // we dont support this anymore          
                }).collect();

                // in v3 when we have any hints, we only assume h1 is supported if it is explicitly set
                // so since this was not the case in v2, we need to add h1 if we have any hints at all..
                // and then user can remove it if they dont want it.
                if new_hints.len() > 0 {
                    new_hints.push(Hint::H1);
                }

                let new_hints = if new_hints.len() == 0 { None } else { Some(new_hints) };

                InProcessSiteConfig {
                    log_level: None,
                    enable_lets_encrypt: Some(false),
                    proc_id: ProcId::new(),
                    active_port: None,
                    forward_subdomains: x.forward_subdomains,
                    disable_tcp_tunnel_mode: x.disable_tcp_tunnel_mode,
                    args: x.args,
                    auto_start: x.auto_start,
                    bin: x.bin,
                    capture_subdomains: x.capture_subdomains,
                    env_vars: x.env_vars,
                    host_name: x.host_name,
                    port: x.port,
                    log_format: x.log_format,
                    dir: x.dir,
                    https: x.https,
                    hints: new_hints,
                    exclude_from_start_all: x.exclude_from_start_all
                    
                }
            }).collect()),
            remote_target: Some(old_config.remote_target.unwrap_or_default().iter().map(|x|{

                RemoteSiteConfig {
                    keep_original_host_header: None,
                    enable_lets_encrypt: Some(false),
                    disable_tcp_tunnel_mode: x.disable_tcp_tunnel_mode,
                    capture_subdomains: x.capture_subdomains,
                    forward_subdomains: x.forward_subdomains,
                    backends: x.backends.iter().map(|b| {

                        
                        let mut new_hints : Vec<Hint> = b.hints.iter().flatten().filter_map(|x| match x {
                            &crate::configuration::v2::Hint::H2 => Some(Hint::H2),
                            &crate::configuration::v2::Hint::H2C => Some(Hint::H2C),
                            &crate::configuration::v2::Hint::H2CPK => Some(Hint::H2CPK),
                            &crate::configuration::v2::Hint::NOH2 => None, // we dont support this anymore          
                        }).collect();

                        // in v3 when we have any hints, we only assume h1 is supported if it is explicitly set
                        // so since this was not the case in v2, we need to add h1 if we have any hints at all..
                        // and then user can remove it if they dont want it.
                        if new_hints.len() > 0 {
                            new_hints.push(Hint::H1);
                        }

                        let new_hints = if new_hints.len() == 0 { None } else { Some(new_hints) };

                        
                        
                        Backend {
                            address: b.address.clone(),
                            port: b.port,
                            https: b.https,
                            hints: new_hints
                        }
                    }).collect(),
                    host_name: x.host_name.clone(),                    
                }
            }).collect()),
            root_dir: old_config.root_dir,
            tls_port: old_config.tls_port

        };
        Ok(new_config)
    }
}