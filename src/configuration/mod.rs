use serde::{Deserialize, Serialize};

pub (crate) mod legacy;
pub (crate) mod v1;

#[derive(Debug,Clone)]
pub (crate) enum Config {
    #[allow(dead_code)]Legacy(legacy::Config),
    V1(v1::OddBoxConfig)
}

#[derive(Debug,Clone)]
pub struct ConfigWrapper(pub v1::OddBoxConfig);
impl std::ops::Deref for ConfigWrapper {
    type Target = v1::OddBoxConfig;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ConfigWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub (crate) struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Serialize,Deserialize,Debug,Clone)]
#[allow(non_camel_case_types)]
pub enum LogFormat {
    standard,
    dotnet
}

#[derive(Serialize,Deserialize,Debug,Clone)]
#[allow(non_camel_case_types)]
pub enum LogLevel {
    trace,
    debug,
    info,
    warn,
    error
}


#[derive(Debug,Clone,Serialize,Deserialize,Default)]
pub enum OddBoxConfigVersion {
    #[default] Unmarked,
    V1
}

impl Config {
    pub fn parse(content:&str) -> Result<Config,String> {
        let v1_result = toml::from_str::<v1::OddBoxConfig>(content);
        if let Ok(v1_config) = v1_result {
            return Ok(Config::V1(v1_config))
        };
        let legacy_result = toml::from_str::<legacy::Config>(&content);
        if let Ok(legacy_config) = legacy_result {
            return Ok(Config::Legacy(legacy_config))
        };
        Err(format!("invalid v1 configuration file. {v1_result:?} ...\n\n ------- could not parse as legacy either due to error: {legacy_result:?}"))
    }

    pub fn try_upgrade(&self) -> Result<Config,String> {
        match self {
            Config::Legacy(old_cfg) => {
                let updated : v1::OddBoxConfig = old_cfg.to_owned().try_into()?;
                Ok(Config::V1(updated))
            },
            Config::V1(_) => Err(format!("already in v1 format")),
        }
    }
}

// LEGACY ---> V1
impl TryFrom<legacy::Config> for v1::OddBoxConfig {
    
    type Error = String;

    fn try_from(old_config: legacy::Config) -> Result<Self, Self::Error> {
        let new_config = v1::OddBoxConfig {
            version: OddBoxConfigVersion::V1,
            alpn: Some(false), // allowing alpn would be a breaking change for h2c when using old configuration format
            auto_start: old_config.auto_start,
            default_log_format: old_config.default_log_format,
            env_vars: old_config.env_vars,
            ip: old_config.ip,
            log_level: old_config.log_level,
            http_port: old_config.port,
            port_range_start: old_config.port_range_start,
            hosted_process: Some(old_config.processes.into_iter().map(|x|{
                v1::InProcessSiteConfig {
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
            }).collect::<Vec<v1::InProcessSiteConfig>>()),
            remote_target: old_config.remote_sites,
            root_dir: old_config.root_dir,
            tls_port: old_config.tls_port

        };
        Ok(new_config)
    }
}