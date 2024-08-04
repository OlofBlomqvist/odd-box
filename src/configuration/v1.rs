use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::path::Path;

use anyhow::bail;
use serde::Serialize;
use serde::Deserialize;
use utoipa::ToSchema;
use crate::global_state::GlobalState;

use super::EnvVar;
use super::LogFormat;
use super::LogLevel;


impl InProcessSiteConfig {
    pub fn set_port(&mut self, port : u16) { 
        self.port = Some(port) 
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub (crate) struct InProcessSiteConfig{
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
    /// Set this to false if you do not want this site to start automatically
    pub auto_start: Option<bool>,
    pub port: Option<u16>,
    pub https : Option<bool>,
    /// If you wish to use wildcard routing for any subdomain under the 'host_name'
    pub capture_subdomains : Option<bool>,
    /// If you wish to use the subdomain from the request in forwarded requests:
    /// test.example.com -> internal.site
    /// vs
    /// test.example.com -> test.internal.site 
    pub forward_subdomains : Option<bool>
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub (crate) enum H2Hint {
    H2,
    H2C
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub (crate) struct RemoteSiteConfig{
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
pub struct OddBoxConfig {
    #[schema(value_type = String)]
    pub (crate) version : super::OddBoxConfigVersion,
    pub (crate) root_dir : Option<String>, 
    #[serde(default = "default_log_level")]
    pub (crate) log_level : Option<LogLevel>,
    /// Defaults to true. Lets you enable/disable h2/http11 tls alpn algs during initial connection phase. 
    #[serde(default = "true_option")]
    pub (crate) alpn : Option<bool>,
    pub (crate) port_range_start : u16,
    #[serde(default = "default_log_format")]
    pub (crate) default_log_format : LogFormat,
    #[schema(value_type = String)]
    pub (crate) ip : Option<IpAddr>,
    #[serde(default = "default_http_port_8080")]
    pub (crate) http_port : Option<u16>,
    #[serde(default = "default_https_port_4343")]
    pub (crate) tls_port : Option<u16>,
    #[serde(default = "true_option")]
    pub (crate) auto_start : Option<bool>,
    pub (crate) env_vars : Vec<EnvVar>,
    pub (crate) remote_target : Option<Vec<RemoteSiteConfig>>,
    pub (crate) hosted_process : Option<Vec<InProcessSiteConfig>>,
    pub (crate) admin_api_port : Option<u16>,
    pub (crate) path : Option<String>

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

impl OddBoxConfig {
       

    // Validates and populates variables in the configuration
    pub fn init(&mut self,cfg_path:&str) -> anyhow::Result<()>  {
        
        self.path = Some(std::path::Path::new(&cfg_path).canonicalize()?.to_str().unwrap_or_default().into());


        let resolved_home_dir_path = dirs::home_dir().ok_or(anyhow::anyhow!(String::from("Failed to resolve home directory.")))?;
        let resolved_home_dir_str = resolved_home_dir_path.to_str().ok_or(anyhow::anyhow!(String::from("Failed to parse home directory.")))?;

        tracing::info!("Resolved home directory: {}",&resolved_home_dir_str);

        let cfg_dir = Self::get_parent(cfg_path)?;

        if let Some(rd) = self.root_dir.as_mut() {
            
            if rd.contains("$root_dir") {
                anyhow::bail!("it is clearly not a good idea to use $root_dir in the configuration of root dir...")
            }

            let rd_with_vars_replaced = rd
                .replace("$cfg_dir", &cfg_dir)
                .replace("~", resolved_home_dir_str);

            let canonicalized_with_vars = 
                match std::fs::canonicalize(rd_with_vars_replaced.clone()) {
                    Ok(resolved_path) => {
                        resolved_path.display().to_string()
                            // we dont want to use ext path def on windows
                            .replace("\\\\?\\", "")
                    }
                    Err(e) => {
                        anyhow::bail!(format!("root_dir item in configuration ({rd}) resolved to this: '{rd_with_vars_replaced}' - error: {}", e));
                    }
                };
            
            *rd = canonicalized_with_vars;

            tracing::debug!("$root_dir resolved to: {rd}")
        }
           
        let cloned_root_dir = self.root_dir.clone();




        if let Some(procs) = self.hosted_process.as_deref_mut() {
            for x in &mut procs.iter_mut() {
                
                if x.dir.len() < 5 { anyhow::bail!(format!("Invalid path configuration for {:?}",x))}
                
                Self::massage_proc(cfg_path, &cloned_root_dir, x)?;
                

                // basic sanity check..
                if x.dir.contains("$root_dir") {
                    anyhow::bail!("Invalid configuration: {x:?}. Missing root_dir in configuration file but referenced for this item..")
                }

                // if no log format is specified for the process but there is a global format, override it
                if x.log_format.is_none() {
                   x.log_format = Some(self.default_log_format.clone())
                }
            }
        }

       

        Ok(())
    }

    pub fn is_valid(&self) -> anyhow::Result<()> {
        
        let mut all_host_names: Vec<&str> = vec![
            self.remote_target.as_ref().and_then(|p|Some(p.iter().map(|x|x.host_name.as_str()).collect::<Vec<&str>>())).unwrap_or_default(), 
            self.hosted_process.as_ref().and_then(|p|Some(p.iter().map(|x|x.host_name.as_str()).collect::<Vec<&str>>())).unwrap_or_default()      

        ].concat();
        
        all_host_names.sort();
        
        let all_count = all_host_names.len();

        all_host_names.dedup();

        let unique_count = all_host_names.len();

        if all_count != unique_count {
            anyhow::bail!(format!("duplicated host names detected in config."))
        }

        Ok(())

    }


    fn get_parent(p:&str) -> anyhow::Result<String> {
        if let Some(directory_path_str) = 
            std::path::Path::new(&p)
            .parent()
            .map(|p| p.to_str().unwrap_or_default()) 
        {
            if directory_path_str.eq("") {
                tracing::debug!("$cfg_dir resolved to '.'");
                Ok(".".into())
            } else {
                tracing::debug!("$cfg_dir resolved to {directory_path_str}");
                Ok(directory_path_str.into())
            } 
            
        } else {
            bail!(format!("Failed to resolve $cfg_dir"));
        }   
    }

    fn massage_proc(cfg_path:&str,root_dir:&Option<String>, proc:&mut InProcessSiteConfig) -> anyhow::Result<()> {

        let cfg_dir = Self::get_parent(&cfg_path)?;

        let resolved_home_dir_path = dirs::home_dir().ok_or(anyhow::anyhow!(String::from("Failed to resolve home directory.")))?;
        let resolved_home_dir_str = resolved_home_dir_path.to_str().ok_or(anyhow::anyhow!(String::from("Failed to parse home directory.")))?;
        
        let with_vars = |x:&str| -> String {
            x.replace("$root_dir", & if let Some(rd) = &root_dir { rd.to_string() } else { "$root_dir".to_string() })
            .replace("$cfg_dir", &cfg_dir)
            .replace("~", resolved_home_dir_str)
        };

        for a in &mut proc.args {
            *a = with_vars(a)
        }

        proc.dir = with_vars(&proc.dir);
        proc.bin = with_vars(&proc.bin);

        Ok(())

    }

    pub (crate) async fn add_or_replace_hosted_process(&mut self,hostname:&str,mut item:InProcessSiteConfig,state:GlobalState) -> anyhow::Result<()> {
        
        Self::massage_proc(
            &self.path.clone().unwrap_or_default(),
            &self.root_dir,
            &mut item
        )?;

        if let Some(hosted_site_configs) = &mut self.hosted_process {
            
           

            for x in hosted_site_configs.iter_mut() {
                if hostname == x.host_name {

                    let (tx,mut rx) = tokio::sync::mpsc::channel(1);

                    state.2.send(crate::http_proxy::ProcMessage::Delete(hostname.into(),tx))?;
                            
                    if rx.recv().await == Some(0) {
                        // when we get this message, we know that the process has been stopped
                        // and that the loop has been exited as well.
                        tracing::debug!("Received a confirmation that the process was deleted");
                    } else {
                        tracing::debug!("Failed to receive a confirmation that the process was deleted. This is a bug in odd-box.");
                    };


                    break;
                }
            };

            tracing::debug!("Pushing a new process to the configuration thru the admin api");
            hosted_site_configs.retain(|x| x.host_name != item.host_name);
            hosted_site_configs.retain(|x| x.host_name != hostname);
            hosted_site_configs.push(item.clone());
            
            
            // todo: auto port wont be respected here anymore as it is only used during init in main
            // might need to make v2 config have port be required?
            tokio::task::spawn(crate::proc_host::host(
                item.clone(),
                state.2.subscribe(),
                state.clone(),
            ));
            tracing::trace!("Spawned a new thread for site: {:?}",hostname);
            
            let mut guard = state.0.write().await;
            guard.site_states_map.retain(|k,_v| k != hostname);
            guard.site_states_map.insert(hostname.to_owned(), crate::types::app_state::ProcState::Stopped);    
        }
    
        
    
        if let Some(p) = &self.path {
            self.write_to_disk(&p)
        } else {
            bail!(ConfigurationUpdateError::Bug("No path found to the current configuration".into()))
        }
    
       
    
    }
    
    
    pub (crate) async fn add_or_replace_remote_site(&mut self,hostname:&str,item:RemoteSiteConfig,state:GlobalState) -> anyhow::Result<()> {
        

        if let Some(sites) = self.remote_target.as_mut() {
            // out with the old, in with the new
            sites.retain(|x| x.host_name != hostname);
            sites.retain(|x| x.host_name != item.host_name);
            sites.push(item.clone());

            // same as above but for the TUI state
            let mut guard = state.0.write().await;
            guard.site_states_map.retain(|k,_v| *k != item.host_name);
            guard.site_states_map.retain(|k,_v| k != hostname);
            guard.site_states_map.insert(hostname.to_owned(), crate::types::app_state::ProcState::Remote);
        }
    
    
        if let Some(p) = &self.path {
            self.write_to_disk(&p)
        } else {
            bail!(ConfigurationUpdateError::Bug("No path found to the current configuration".into()))
        }
    
    
    }
    


}

#[derive(Debug)]
enum ConfigurationUpdateError {
    FailedToSave(String),
    NotFound,
    Bug(String)
}


impl std::fmt::Display for ConfigurationUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigurationUpdateError::NotFound => {
                f.write_str("No such hosted process found.")
            },
            ConfigurationUpdateError::FailedToSave(e) => {
                f.write_fmt(format_args!("Failed to save due to error: {}",e))
            },
            ConfigurationUpdateError::Bug(e) => {
                f.write_fmt(format_args!("Failed to save due to a bug in odd-box: {}",e))
            }
        }
    }
}

impl OddBoxConfig {
    
    // note: this seems silly but its needed because neither toml-rs nor toml_edit supports any decent
    // formatting customization and ends up with spread out arrays of tables rather
    // than inlining like we usually do for odd-box configs.
    pub fn write_to_disk(&self,current_path:&str) -> anyhow::Result<()> {
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
       

        formatted_toml.push("env_vars = [".to_string());
        for env_var in &self.env_vars {
            formatted_toml.push(format!(
                "\t{{ key = {:?}, value = {:?} }},",
                env_var.key, env_var.value
            ));
        }
        formatted_toml.push("]".to_string());


        if let Some(remote_sites) = &self.remote_target {
            for site in remote_sites {
                formatted_toml.push("\n[[remote_target]]".to_string());
                formatted_toml.push(format!("host_name = {:?}", site.host_name));
                formatted_toml.push(format!("target_hostname = {:?}", site.target_hostname));
                if let Some(hint) = &site.h2_hint {
                    formatted_toml.push(format!("h2_hint = \"{:?}\"", hint));
                }
                
                
                if let Some(capture_subdomains) = site.capture_subdomains {
                    formatted_toml.push(format!("capture_subdomains = {}", capture_subdomains));
                }
                
                if let Some(b) = site.https {
                    formatted_toml.push(format!("https = {}", b));
                }
                if let Some(http) = site.port {
                    formatted_toml.push(format!("port = {}", http));
                }

                if let Some(disable_tcp_tunnel_mode) = site.disable_tcp_tunnel_mode {
                    formatted_toml.push(format!("disable_tcp_tunnel_mode = {}", disable_tcp_tunnel_mode));
                }
            }
        }

        if let Some(processes) = &self.hosted_process {
            for process in processes {
                formatted_toml.push("\n[[hosted_process]]".to_string());
                formatted_toml.push(format!("host_name = {:?}", process.host_name));
                formatted_toml.push(format!("dir = {:?}", process.dir));
                formatted_toml.push(format!("bin = {:?}", process.bin));
                if let Some(hint) = &process.h2_hint {
                    formatted_toml.push(format!("h2_hint = \"{:?}\"", hint));
                }
                
                let args = process.args.iter().map(|arg| format!("{:?}", arg)).collect::<Vec<_>>().join(", ");
                formatted_toml.push(format!("args = [{}]", args));
                
             
          
            
                
                if let Some(auto_start) = process.auto_start {
                    formatted_toml.push(format!("auto_start = {}", auto_start));
                }
                
                
                if let Some(b) = process.https {
                    formatted_toml.push(format!("https = {}", b));
                }
                if let Some(http) = process.port {
                    formatted_toml.push(format!("port = {}", http));
                }
                
                if let Some(capture_subdomains) = process.capture_subdomains {
                    formatted_toml.push(format!("capture_subdomains = {}", capture_subdomains));
                } else {
                    formatted_toml.push(format!("capture_subdomains = {}", "false"));
                }

                formatted_toml.push("env_vars = [".to_string());
                for env_var in &process.env_vars {
                    formatted_toml.push(format!(
                        "\t{{ key = {:?}, value = {:?} }},",
                        env_var.key, env_var.value
                    ));
                }
                formatted_toml.push("]".to_string());

            }
        }

        let original_path = Path::new(current_path);
        let backup_path = original_path.with_extension("toml.backup");
        std::fs::rename(original_path, &backup_path)?;

        if let Err(e) = std::fs::write(current_path, formatted_toml.join("\n")) {
            bail!("Failed to write config to disk: {e}")
        } else {
            Ok(())
        }

    }
}



pub fn example_v1() -> OddBoxConfig {
    OddBoxConfig {
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