use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::path::Path;

use serde::Serialize;
use serde::Deserialize;
use super::EnvVar;
use super::LogFormat;
use super::LogLevel;


impl InProcessSiteConfig {
    pub fn set_port(&mut self, port : u16) { 
        self.port = Some(port) 
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub (crate) enum H2Hint {
    H2,
    H2C
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OddBoxConfig {
    pub (crate) version : super::OddBoxConfigVersion,
    pub (crate) root_dir : Option<String>, 
    pub (crate) log_level : Option<LogLevel>,
    /// Defaults to true. Lets you enable/disable h2/http11 tls alpn algs during initial connection phase. 
    #[serde(default = "true_option")]
    pub (crate) alpn : Option<bool>,
    pub (crate) port_range_start : u16,
    pub (crate) default_log_format : Option<LogFormat>,
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
                if directory_path_str.eq("") {
                    tracing::debug!("$cfg_dir resolved to '.'");
                    "."
                } else {
                    tracing::debug!("$cfg_dir resolved to {directory_path_str}");
                    directory_path_str
                } 
                
            } else {
                return Err(format!("Failed to resolve $cfg_dir"));
            };   

        if let Some(rd) = self.root_dir.as_mut() {
            
            if rd.contains("$root_dir") {
                panic!("it is clearly not a good idea to use $root_dir in the configuration of root dir...")
            }

            let rd_with_vars_replaced = rd
                .replace("$cfg_dir", cfg_dir)
                .replace("~", resolved_home_dir_str);

            let canonicalized_with_vars = 
                match std::fs::canonicalize(rd_with_vars_replaced.clone()) {
                    Ok(resolved_path) => {
                        resolved_path.display().to_string()
                            // we dont want to use ext path def on windows
                            .replace("\\\\?\\", "")
                    }
                    Err(e) => {
                        return Err(format!("root_dir item in configuration ({rd}) resolved to this: '{rd_with_vars_replaced}' - error: {}", e));
                    }
                };
            
            *rd = canonicalized_with_vars;

            tracing::debug!("$root_dir resolved to: {rd}")
        }
           
        let cloned_root_dir = self.root_dir.clone();


        let with_vars = |x:&str| -> String {
            x.replace("$root_dir", & if let Some(rd) = &cloned_root_dir { rd.to_string() } else { "$root_dir".to_string() })
            .replace("$cfg_dir", cfg_dir)
            .replace("~", resolved_home_dir_str)
        };
           

        let log_format = self.default_log_format.clone();

        if let Some(procs) = self.hosted_process.as_deref_mut() {
            for x in &mut procs.iter_mut() {
                
                if x.dir.len() < 5 { return Err(format!("Invalid path configuration for {:?}",x))}
                
                x.dir = with_vars(&x.dir);
                x.bin = with_vars(&x.bin);

                for a in &mut x.args {
                    *a = with_vars(a)
                }

                // basic sanity check..
                if x.dir.contains("$root_dir") {
                    return Err(format!("Invalid configuration: {x:?}. Missing root_dir in configuration file but referenced for this item.."))
                }

                // if no log format is specified for the process but there is a global format, override it
                if x.log_format.is_none() {
                    if let Some(f) = &log_format {
                        x.log_format = Some(f.clone())
                    }
                }
            }
        }

        Ok(())
    }

    pub fn is_valid(&self) -> Result<(),String> {
        
        let mut all_host_names: Vec<&str> = vec![
            self.remote_target.as_ref().and_then(|p|Some(p.iter().map(|x|x.host_name.as_str()).collect::<Vec<&str>>())).unwrap_or_default(), 
            self.hosted_process.as_ref().and_then(|p|Some(p.iter().map(|x|x.host_name.as_str()).collect::<Vec<&str>>())).unwrap_or_default()      

        ].concat();
        
        all_host_names.sort();
        
        let all_count = all_host_names.len();

        all_host_names.dedup();

        let unique_count = all_host_names.len();

        if all_count != unique_count {
            return Err(format!("duplicated host names detected in config."))
        }

        Ok(())

    }

}


impl OddBoxConfig {
    
    // note: this seems silly but its needed because neither toml-rs nor toml_edit supports any decent
    // formatting customization and ends up with spread out arrays of tables rather
    // than inlining like we usually do for odd-box configs.
    pub fn write_to_disk(&self,current_path:&str) {
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

        if let Some(default_log_format) = &self.default_log_format {
            formatted_toml.push(format!("default_log_format = \"{:?}\"", default_log_format));
        } else {
            formatted_toml.push(format!("default_log_format = \"standard\""));
        }

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
        std::fs::rename(original_path, &backup_path).expect("must be able to backup old config");

        if let Err(e) = std::fs::write(current_path, formatted_toml.join("\n")) {
            eprintln!("Failed to write config to disk: {}", e);
        } else {
            println!("Your odd-box configuration file was updated to the V1 format. Your old configuration was backed up here: {}.backup",current_path);
            tracing::warn!("Your odd-box configuration file was updated to the V1 format. Your old configuration was backed up here: {}.backup",current_path);
        }
    }
}



pub fn example_v1() -> OddBoxConfig {
    OddBoxConfig {
        version: super::OddBoxConfigVersion::V1,
        alpn: Some(false),
        auto_start: Some(true),
        default_log_format: Some(LogFormat::standard),
        env_vars: vec![
            EnvVar { key: "some_key".into(), value:"some_val".into() },
            EnvVar { key: "another_key".into(), value:"another_val".into() },
        ],
        ip: Some(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
        log_level: Some(LogLevel::info),
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