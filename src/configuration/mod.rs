use std::sync::Arc;

use anyhow::bail;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use v1::H2Hint;
use v2::FullyResolvedInProcessSiteConfig;

pub mod legacy;
pub mod v1;
pub mod v2;

pub trait OddBoxConfiguration<T> { 
    fn example() -> T;
    fn to_string(&self) -> anyhow::Result<String> {
        bail!("to_string is not implemented for this configuration version")
    }
    fn write_to_disk(&self) -> anyhow::Result<()> {
        bail!("write_to_disk is not implemented for this configuration version")
    }
}

#[derive(Debug,Clone)]
pub enum OddBoxConfig {
    #[allow(dead_code)]Legacy(legacy::OddBoxLegacyConfig),
    V1(v1::OddBoxV1Config),
    V2(v2::OddBoxV2Config)
}


#[derive(Debug, Clone, Serialize, Deserialize,ToSchema,PartialEq, Eq, Hash, schemars::JsonSchema)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Serialize,Deserialize,Debug,Clone,ToSchema,PartialEq, Eq, Hash, schemars::JsonSchema)]
#[allow(non_camel_case_types)]
pub enum LogFormat {
    standard,
    dotnet
}

#[derive(Debug,Serialize,Clone,ToSchema, PartialEq, Eq, Hash, schemars::JsonSchema)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error
}

impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct LogLevelVisitor;

        impl<'de> serde::de::Visitor<'de> for LogLevelVisitor {
            type Value = LogLevel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a log level (trace, debug, info, warn, error)")
            }

            fn visit_str<E>(self, value: &str) -> Result<LogLevel, E>
            where
                E: serde::de::Error,
            {
                match value.to_lowercase().as_str() {
                    "trace" => Ok(LogLevel::Trace),
                    "debug" => Ok(LogLevel::Debug),
                    "info" => Ok(LogLevel::Info),
                    "warn" => Ok(LogLevel::Warn),
                    "error" => Ok(LogLevel::Error),
                    _ => Err(E::custom(format!("unknown log level: {}", value))),
                }
            }
        }

        deserializer.deserialize_str(LogLevelVisitor)
    }
}


#[derive(Debug,Clone,Serialize,Deserialize,Default,ToSchema,PartialEq, Eq, Hash, schemars::JsonSchema)]
pub enum OddBoxConfigVersion {
    #[default] Unmarked,
    V1,
    V2
}



impl OddBoxConfig {
    
    pub fn parse(content:&str) -> Result<OddBoxConfig,String> {
        
        let v2_result = toml::from_str::<v2::OddBoxV2Config>(content);
        if let Ok(v2_config) = v2_result {
            return Ok(OddBoxConfig::V2(v2_config))
        };

        let v1_result = toml::from_str::<v1::OddBoxV1Config>(content);
        if let Ok(v1_config) = v1_result {
            return Ok(OddBoxConfig::V1(v1_config))
        };

        let legacy_result = toml::from_str::<legacy::OddBoxLegacyConfig>(&content);
        if let Ok(legacy_config) = legacy_result {
            return Ok(OddBoxConfig::Legacy(legacy_config))
        };

        if content.contains("version = \"V2\"") {
            Err(format!("invalid v2 configuration file.\n{}", v2_result.unwrap_err().to_string()))
        } else if content.contains("version = \"V1\"") {
            Err(format!("invalid v1 configuration file.\n{}", v1_result.unwrap_err().to_string()))
        } else {
            Err(format!("invalid (legacy) configuration file.\n{}", legacy_result.unwrap_err().to_string()))
        }
    }

    // Result<(validated_config,original_version),error>
    pub fn try_upgrade_to_latest_version(&self) -> Result<(v2::OddBoxV2Config,OddBoxConfigVersion),String> {
        match self {
            OddBoxConfig::Legacy(legacy_config) => {
                let v1 : v1::OddBoxV1Config = legacy_config.to_owned().try_into()?;
                let v2 : v2::OddBoxV2Config = v1.to_owned().try_into()?;
                Ok((v2,OddBoxConfigVersion::Unmarked))
            },
            OddBoxConfig::V1(v1_config) => {
                let v2 : v2::OddBoxV2Config = v1_config.to_owned().try_into()?;
                Ok((v2,OddBoxConfigVersion::V1))
            },
            OddBoxConfig::V2(v2) => {
                Ok((v2.clone(),OddBoxConfigVersion::V2))
            },
        }
    }
}



#[derive(Debug,Clone)]
pub struct ConfigWrapper {
    internal_configuration : v2::OddBoxV2Config,
    pub remote_sites: DashMap<String, v2::RemoteSiteConfig>,
    pub hosted_processes: DashMap<String, v2::InProcessSiteConfig>,
    pub wrapper_cache_map_is_dirty: bool
}


impl std::ops::Deref for ConfigWrapper {
    type Target = v2::OddBoxV2Config;
    fn deref(&self) -> &Self::Target {
        &self.internal_configuration
    }
}
impl std::ops::DerefMut for ConfigWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.internal_configuration
    }
}


// This is meant to simplify the process of upgrading from one configuration version to another.
// It is also used as a runtime-cache for the configuration such that one can change the config
// during runtime without having to save it to disk, and so that it becomes easier to 
// work with from code in general..
impl ConfigWrapper {

    /// Creates a new ConfigWrapper from an OddBoxV2Config,
    /// initializing the DashMaps from the vectors in the config.
    pub fn new(config: v2::OddBoxV2Config) -> Self {
        let remote_sites = DashMap::new();
        if let Some(remote_targets) = &config.remote_target {
            for site in remote_targets {
                remote_sites.insert(site.host_name.clone(), site.clone());
            }
        }

        let hosted_processes = DashMap::new();
        if let Some(hosted_procs) = &config.hosted_process {
            for proc in hosted_procs {
                hosted_processes.insert(proc.host_name.clone(), proc.clone());
            }
        }

        ConfigWrapper {
            internal_configuration: config,
            remote_sites,
            hosted_processes,
            wrapper_cache_map_is_dirty: false
        }
    }

    // re-populate the dashmaps from the internal configuration vectors
    pub fn reload(&mut self) {
        
        self.hosted_processes.clear();
        self.remote_sites.clear();

        if let Some(remote_targets) = &self.remote_target {
            for site in remote_targets {
                self.remote_sites.insert(site.host_name.clone(), site.clone());
            }
        }

        if let Some(hosted_procs) = &self.hosted_process {
            for proc in hosted_procs {
                self.hosted_processes.insert(proc.host_name.clone(), proc.clone());
            }
        }

        self.wrapper_cache_map_is_dirty = false;
        

    } 

    /// Persists the current state of the DashMaps back into the config vectors.
    /// This method should be called before serialization.
    pub fn persist(&mut self) {
        let remote_targets: Vec<_> = self
            .remote_sites
            .iter()
            .map(|kv| kv.value().clone())
            .collect();
        self.internal_configuration.remote_target = if remote_targets.is_empty() {
            None
        } else {
            Some(remote_targets)
        };

        let hosted_processes: Vec<_> = self
            .hosted_processes
            .iter()
            .map(|kv| kv.value().clone())
            .collect();
        self.internal_configuration.hosted_process = if hosted_processes.is_empty() {
            None
        } else {
            Some(hosted_processes)
        };

        self.wrapper_cache_map_is_dirty = false;
    }

    pub fn set_disk_path(&mut self,cfg_path:&str) -> anyhow::Result<()>  {
        self.path = Some(std::path::Path::new(&cfg_path).canonicalize()?.to_str().unwrap_or_default().into());
        Ok(())
    }

    pub fn is_valid(&self) -> anyhow::Result<()> {

        // TODO:
        // - check for invalid hint combinations

        if self.env_vars.iter().any(|x| x.key.eq_ignore_ascii_case("port")) {
            anyhow::bail!("Invalid configuration. You cannot use 'port' as a global environment variable");
        }

    
        let mut host_names = std::collections::HashMap::new();
        let mut ports = std::collections::HashMap::new();
    

        for target in self.remote_target.iter().flatten() {
            if target.enable_lets_encrypt.unwrap_or(false) {
                if !target.disable_tcp_tunnel_mode.unwrap_or(false) {
                    anyhow::bail!(format!("Invalid configuration for '{}'. LetsEncrypt cannot be enabled when TCP tunnel mode is enabled.", target.host_name));
                }
            }
        }

        for process in self.hosted_process.iter().flatten() {
  
            host_names
                .entry(process.host_name.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
    
            if let Some(port) = process.port {
                ports
                    .entry(port)
                    .or_insert_with(Vec::new)
                    .push(process.host_name.clone());
            }

            if process.enable_lets_encrypt.unwrap_or(false) {
                if !process.disable_tcp_tunnel_mode.unwrap_or(false) {
                    anyhow::bail!(format!("Invalid configuration for '{}'. LetsEncrypt cannot be enabled when TCP tunnel mode is enabled.", process.host_name));
                }
            }
    
            if let Some(port) = process.port {
                if let Some(env_vars) = &process.env_vars {
                    for env_var in env_vars {
                        if env_var.key.eq_ignore_ascii_case("port") {
                            if let Ok(env_port) = env_var.value.parse::<u16>() {
                                if env_port != port {
                                    anyhow::bail!(format!(
                                        "Environment variable PORT for '{}' does not match the port specified in the configuration.\n\
                                        It is recommended to rely on the port setting - it will automatically inject the port variable to the process-local context.",
                                        process.host_name
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    
        let duplicate_host_names: Vec<String> = host_names
            .into_iter()
            .filter_map(|(name, count)| if count > 1 { Some(name) } else { None })
            .collect();
    
        if !duplicate_host_names.is_empty() {
            anyhow::bail!(format!(
                "Duplicate host names found: {}",
                duplicate_host_names.join(", ")
            ));
        }
    
        let duplicate_ports: Vec<(u16, Vec<String>)> = ports
            .into_iter()
            .filter(|(_, sites)| sites.len() > 1)
            .collect();
    
        if !duplicate_ports.is_empty() {
            let conflict_details: Vec<String> = duplicate_ports
                .into_iter()
                .map(|(port, sites)| format!("Port {}: [{}]", port, sites.join(", ")))
                .collect();
    
            anyhow::bail!(format!(
                "Duplicate ports found with conflicting sites: {}",
                conflict_details.join("; ")
            ));
        }
    
        Ok(())
    }
    

    pub fn get_parent_path(&mut self) -> anyhow::Result<String> {
        // todo - use cache and clear on path change
        // if let Some(pre_resolved) = &self.1 {
        //     return Ok(pre_resolved.to_string())
        // }
        let p = self.path.clone().ok_or(anyhow::anyhow!(String::from("Failed to resolve path.")))?;
        if let Some(directory_path_str) = 
            std::path::Path::new(&p)
            .parent()
            .map(|p| p.to_str().unwrap_or_default()) 
        {
            if directory_path_str.eq("") {
                //tracing::trace!("$cfg_dir resolved to '.'");
                let xx = ".".to_string();
                //self.1 = Some(xx.clone());
                Ok(xx)
            } else {
                //tracing::trace!("$cfg_dir resolved to {directory_path_str}");
                let xx = directory_path_str.to_string();
                //self.1 = Some(xx.clone());
                Ok(xx)
            }
            
        } else {
            bail!(format!("Failed to resolve $cfg_dir"));
        }   
    }

    
    // ---> port-mapping...
    // todo: work with the wrapper dashmap instead
    pub async fn add_or_replace_hosted_process(&mut self,hostname:&str,item:crate::InProcessSiteConfig,state:Arc<crate::GlobalState>) -> anyhow::Result<()> {
        
        if let Some(hosted_site_configs) = &mut self.hosted_process {
            
            for x in hosted_site_configs.iter_mut() {
                if hostname == x.host_name {



                    let (tx,mut rx) = tokio::sync::mpsc::channel(1);

                    state.broadcaster.send(crate::http_proxy::ProcMessage::Delete(hostname.into(),tx))?;
                            
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
            

            let resolved_proc = self.resolve_process_configuration(&item)?;

            tokio::task::spawn(crate::proc_host::host(
                resolved_proc,
                state.broadcaster.subscribe(),
                state.clone(),
            ));
            tracing::trace!("Spawned a new thread for site: {:?}",hostname);
            
            let guard = &state.app_state.site_status_map;
            guard.retain(|k,_v| k != hostname);
            guard.insert(hostname.to_owned(), crate::types::app_state::ProcState::Stopped);    
        }
    
        self.reload();
    
        self.write_to_disk()
       
    
       
    
    }

    pub fn busy_ports(&self) -> Vec<(String,u16)> {
        self.hosted_process.iter().flatten().flat_map(|x| {
            
            let mut items = Vec::new();
            
            // manually set ports needs to be marked as busy even if the process is not running
            if let Some(p) = x.port { 
                items.push((x.host_name.clone(),p))
            }


            // active ports means that there is a loop active for this process using that port
            if let Some(p) = x.active_port { 
                items.push((x.host_name.clone(),p))
            }

            if items.len() > 0 {
                Some(items)
            } else {
                None
            }

        }).flatten().collect::<Vec<(String,u16)>>()
    }
    
    pub async fn find_and_set_unused_port(selfy : &mut Self, proc:&mut crate::InProcessSiteConfig) -> anyhow::Result<u16> {
        
        if let Some(procs) = &selfy.hosted_process {
            
            let used_ports = procs.iter().filter_map(|x|x.port).collect::<Vec<u16>>();

            if let Some(manually_chosen_port) = proc.port {
                if used_ports.contains(&manually_chosen_port) {
                    // this port is already in use
                    bail!("The port configured for this site is already in use..")
                } else {
                    return Ok(manually_chosen_port)
                }
            }

        };

        if let Some(manually_chosen_port) = proc.port {
            // clearly this port is not in use yet
            Ok(manually_chosen_port)
        } else {
            // if nothing is running and user has not selected any specific one lets just use the first port from the start range
            Ok(selfy.port_range_start)
        }
    }
    
    // todo: work with the wrapper dashmap instead
    pub async fn add_or_replace_remote_site(&mut self,hostname:&str,item:crate::RemoteSiteConfig,state:Arc<crate::GlobalState>) -> anyhow::Result<()> {
        

        if let Some(sites) = self.remote_target.as_mut() {
            // out with the old, in with the new
            sites.retain(|x| x.host_name != hostname);
            sites.retain(|x| x.host_name != item.host_name);
            sites.push(item.clone());

            // same as above but for the TUI state
            let map_guard = &state.app_state.site_status_map;
            map_guard.retain(|k,_v| *k != item.host_name);
            map_guard.retain(|k,_v| k != hostname);
            map_guard.insert(hostname.to_owned(), crate::types::app_state::ProcState::Remote);
        }
    
        self.reload();
        self.write_to_disk()
    
    }

    // todo: work with the wrapper dashmap instead
    pub fn set_active_port(&mut self, resolved_proc:&mut FullyResolvedInProcessSiteConfig) -> anyhow::Result<u16> {
      
    
        let mut selected_port = None;

        // ports in use or configured for use by other sites
        let unavailable_ports = self.busy_ports().into_iter().filter(|x|{
                x.0 != resolved_proc.host_name 
        }).collect::<Vec<(String,u16)>>();

        // decide which port to use (ie. which port to add as the environment variable PORT)
        if let Some(prefered_port) = resolved_proc.port {
            if let Some(taken_by) = unavailable_ports.iter().find(|x|x.1 == prefered_port) {
                tracing::warn!("[{}] The configured port '{}' is unavailable (configured for another site: '{}').. ",&resolved_proc.host_name,prefered_port,taken_by.1);
            } else {
                tracing::info!("[{}] Starting on port '{}' as configured for the process!",&resolved_proc.host_name,prefered_port);
                selected_port = Some(prefered_port);
            }
        } else if let Some(EnvVar { key: _, value }) = resolved_proc.env_vars.iter().flatten().find(|x|x.key.to_lowercase()=="port") { 
            if let Some(taken_by) = unavailable_ports.iter().find(|x|x.1.to_string() == *value) {
                tracing::warn!("[{}] The configured port (via env var in cfg) '{}' is unavailable (configured for another site: '{}').. ",&resolved_proc.host_name,value,taken_by.1);
            } else {
                if let Ok(spbev) = value.parse::<u16>() {
                    tracing::info!("[{}] Starting on port '{}' as selected via a configured environment variable for port!",&resolved_proc.host_name,value);
                    selected_port = Some(spbev)
                } else {
                    tracing::info!("[{}] The env var for port was configured to '{}' which is not a valid u16, ignoring.",&resolved_proc.host_name,value);
                }
            }
        }

        // if no port manually specified, find the first available port
        if selected_port.is_none() {
            let min_auto_port = self.port_range_start;
            let unavailable = unavailable_ports.iter().map(|x|x.1).collect::<Vec<u16>>();
            // find first port that is not in use starting from min_auto_port, looking at the unavailable_ports list:
            let mut inner_selected_port = min_auto_port;
            loop {
                if unavailable.contains(&inner_selected_port) {
                    inner_selected_port += 1;
                } else {
                    break
                }
            }
            
            tracing::info!("[{}] Using the first available port found (starting from the configured start port: {min_auto_port}) ---> '{}'",&resolved_proc.host_name,inner_selected_port);
            selected_port = Some(inner_selected_port);
        }

        // make sure nobody else is using this port before returning it to caller.
        // mark this process as using this port
        if let Some(sp) = selected_port {
            if let Some(hosted_processes) = &mut self.hosted_process {
                if let Some(mm) = hosted_processes.iter_mut().find(|x| x.host_name == resolved_proc.host_name) {
                    // save the selected port in the globally shared state
                    mm.active_port = Some(sp);
                } else {
                    tracing::error!("[{}] Could not find an active site in the hosted process list.. This is a bug in odd-box!",&resolved_proc.host_name);
                }
            } else {
                tracing::error!("[{}] The site proc list is empty! Most likely this is a bug in odd-box.",&resolved_proc.host_name);
            }
        }

        if let Some(p) = selected_port {
            Ok(p)
        } else {
            bail!("Failed to find a port for the process..")
        }
    }


    // this MUST be called by proc_host prior to starting a process in order to resolve all variables.
    // it is done this way in order to avoid changing the global state of the configuration in to the resolved state
    // since that would then be saved to disk and we would lose the original configuration with dynamic variables
    // making configuration files less portable.
    // todo - cache resolved configurations by hash?
    // todo: work with the wrapper dashmap instead
    pub fn resolve_process_configuration(&mut self,proc:&crate::InProcessSiteConfig) -> anyhow::Result<crate::FullyResolvedInProcessSiteConfig> {

        let mut resolved_proc = crate::FullyResolvedInProcessSiteConfig {
            excluded_from_start_all: proc.exclude_from_start_all.unwrap_or(false),
            proc_id: proc.get_id().clone(),
            active_port: proc.active_port,
            disable_tcp_tunnel_mode: proc.disable_tcp_tunnel_mode,
            hints: proc.hints.clone(),
            host_name: proc.host_name.clone(),
            dir: proc.dir.clone(),
            bin: proc.bin.clone(),
            args: proc.args.clone(),
            env_vars: proc.env_vars.clone(),
            log_format: proc.log_format.clone(),
            auto_start: proc.auto_start,
            port: proc.port,
            https: proc.https,
            capture_subdomains: proc.capture_subdomains,
            forward_subdomains: proc.forward_subdomains
        };

        let resolved_home_dir_path = dirs::home_dir().ok_or(anyhow::anyhow!(String::from("Failed to resolve home directory.")))?;
        let resolved_home_dir_str = resolved_home_dir_path.to_str().ok_or(anyhow::anyhow!(String::from("Failed to parse home directory.")))?;

        // tracing::info!("Resolved home directory: {}",&resolved_home_dir_str);

        let cfg_dir = self.get_parent_path()?;


        let root_dir = if let Some(rd) = &self.root_dir {
            
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
            
            // tracing::debug!("$root_dir resolved to: {rd}");
            canonicalized_with_vars
        } else {
            "$root_dir".to_string()
        };

        let resolved_home_dir_path = dirs::home_dir().ok_or(anyhow::anyhow!(String::from("Failed to resolve home directory.")))?;
        let resolved_home_dir_str = resolved_home_dir_path.to_str().ok_or(anyhow::anyhow!(String::from("Failed to parse home directory.")))?;
        
        let with_vars = |x:&str| -> String {
            x.replace("$root_dir", &root_dir)
            .replace("$cfg_dir", &cfg_dir)
            .replace("~", resolved_home_dir_str)
        };

        if let Some(args) = &mut resolved_proc.args {
            for argument in args {
                *argument = with_vars(argument)
            }
        }
       
        if let Some(dir) = &mut resolved_proc.dir {
            *dir = with_vars(&dir);
        }

        resolved_proc.bin = with_vars(&resolved_proc.bin);
        

        Ok(resolved_proc)

    }


}