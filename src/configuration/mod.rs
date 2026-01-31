// ====================================================================
/// This is not expected to be used unless you are working on tests
/// or configuration-file upgrade logic
pub mod legacy;
/// This is not expected to be used unless you are working on tests
/// or configuration-file upgrade logic
pub mod v1;
/// This is not expected to be used unless you are working on tests
/// or configuration-file upgrade logic
pub mod v2;
/// This is not expected to be used unless you are working on tests
/// or configuration-file upgrade logic
pub mod v3;
/// V4 configuration - YAML-based with frontend/backend separation
pub mod v4;
// ====================================================================

pub mod yaml_air;

use anyhow::bail;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

// Re-export the latest config version
pub use v4::*;
// Also re-export V3 types for backwards compatibility during migration
pub use v3::{Backend, Hint, RemoteSiteConfig};

pub use v4::OddBoxV4Config as OddBoxConfig;

use crate::types::proc_info::ProcId;

pub mod reload;

pub trait OddBoxConfiguration<T> {
    fn example() -> T;
    fn to_string(&self) -> anyhow::Result<String> {
        bail!("to_string is not implemented for this configuration version")
    }
    fn write_to_disk(&self) -> anyhow::Result<()> {
        bail!("write_to_disk is not implemented for this configuration version")
    }
}

#[derive(Debug, Clone)]
pub enum AnyOddBoxConfig {
    #[allow(dead_code)]
    Legacy(legacy::OddBoxLegacyConfig),
    V1(v1::OddBoxV1Config),
    V2(v2::OddBoxV2Config),
    V3(v3::OddBoxV3Config),
    V4(v4::OddBoxV4Config),
}

#[derive(
    Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq, Hash, schemars::JsonSchema,
)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(
    Serialize,
    Deserialize,
    Debug,
    Clone,
    ToSchema,
    PartialEq,
    Eq,
    Hash,
    schemars::JsonSchema,
    Default,
)]
#[allow(non_camel_case_types)]
pub enum LogFormat {
    #[default]
    standard,
    dotnet,
}

#[derive(Debug, Serialize, Clone, ToSchema, PartialEq, Eq, Hash, schemars::JsonSchema)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
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

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Default,
    ToSchema,
    PartialEq,
    Eq,
    Hash,
    schemars::JsonSchema,
)]
pub enum OddBoxConfigVersion {
    #[default]
    Unmarked,
    V1,
    V2,
    V3,
    V4,
}

impl AnyOddBoxConfig {
    /// Parse configuration content (auto-detects YAML vs TOML)
    pub fn parse(content: &str) -> Result<AnyOddBoxConfig, String> {
        // Try YAML first (V4 format) - check for YAML indicators
        if content.trim_start().starts_with("version: ")
            || content.contains("\nbackends:")
            || content.contains("\nfrontends:")
        {
            if let Ok(v4_config) = serde_yaml::from_str::<v4::OddBoxV4Config>(content) {
                return Ok(AnyOddBoxConfig::V4(v4_config));
            }
        }

        // Try TOML formats (V3 and earlier)
        let v3_result = toml::from_str::<v3::OddBoxV3Config>(content);
        if let Ok(v3_config) = v3_result {
            return Ok(AnyOddBoxConfig::V3(v3_config));
        };

        let v2_result = toml::from_str::<v2::OddBoxV2Config>(content);
        if let Ok(v2_config) = v2_result {
            return Ok(AnyOddBoxConfig::V2(v2_config));
        };

        let v1_result = toml::from_str::<v1::OddBoxV1Config>(content);
        if let Ok(v1_config) = v1_result {
            return Ok(AnyOddBoxConfig::V1(v1_config));
        };

        let legacy_result = toml::from_str::<legacy::OddBoxLegacyConfig>(&content);
        if let Ok(legacy_config) = legacy_result {
            return Ok(AnyOddBoxConfig::Legacy(legacy_config));
        };

        // Try YAML as last resort
        if let Ok(v4_config) = serde_yaml::from_str::<v4::OddBoxV4Config>(content) {
            return Ok(AnyOddBoxConfig::V4(v4_config));
        }

        if content.contains("version: V4") || content.contains("version: v4") {
            Err(format!(
                "invalid v4 (YAML) configuration file.\n{}",
                serde_yaml::from_str::<v4::OddBoxV4Config>(content)
                    .unwrap_err()
                    .to_string()
            ))
        } else if content.contains("version = \"V3\"") {
            Err(format!(
                "invalid v3 configuration file.\n{}",
                v3_result.unwrap_err().to_string()
            ))
        } else if content.contains("version = \"V2\"") {
            Err(format!(
                "invalid v2 configuration file.\n{}",
                v2_result.unwrap_err().to_string()
            ))
        } else if content.contains("version = \"V1\"") {
            Err(format!(
                "invalid v1 configuration file.\n{}",
                v1_result.unwrap_err().to_string()
            ))
        } else {
            Err(format!(
                "invalid (legacy) configuration file.\n{}",
                legacy_result.unwrap_err().to_string()
            ))
        }
    }

    /// Parse from file path - uses extension to guide parsing
    pub fn parse_file(path: &std::path::Path) -> Result<AnyOddBoxConfig, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "yaml" | "yml" => {
                // YAML files are always V4
                serde_yaml::from_str::<v4::OddBoxV4Config>(&content)
                    .map(AnyOddBoxConfig::V4)
                    .map_err(|e| format!("Invalid YAML configuration: {}", e))
            }
            _ => {
                // TOML files use auto-detection
                Self::parse(&content)
            }
        }
    }

    // Result<(validated_config, original_version, was_upgraded), error>
    pub fn try_upgrade_to_latest_version(
        &self,
    ) -> Result<
        (
            crate::configuration::OddBoxConfig,
            OddBoxConfigVersion,
            bool,
        ),
        String,
    > {
        match self {
            AnyOddBoxConfig::Legacy(legacy_config) => {
                let v1: v1::OddBoxV1Config = legacy_config.to_owned().try_into()?;
                let v2: v2::OddBoxV2Config = v1.to_owned().try_into()?;
                let v3: v3::OddBoxV3Config = v2.to_owned().try_into()?;
                let v4: v4::OddBoxV4Config = v3.try_into()?;
                Ok((v4, OddBoxConfigVersion::Unmarked, true))
            }
            AnyOddBoxConfig::V1(v1_config) => {
                let v2: v2::OddBoxV2Config = v1_config.to_owned().try_into()?;
                let v3: v3::OddBoxV3Config = v2.to_owned().try_into()?;
                let v4: v4::OddBoxV4Config = v3.try_into()?;
                Ok((v4, OddBoxConfigVersion::V1, true))
            }
            AnyOddBoxConfig::V2(v2) => {
                let v3: v3::OddBoxV3Config = v2.to_owned().try_into()?;
                let v4: v4::OddBoxV4Config = v3.try_into()?;
                Ok((v4, OddBoxConfigVersion::V2, true))
            }
            AnyOddBoxConfig::V3(v3) => {
                let v4: v4::OddBoxV4Config = v3.to_owned().try_into()?;
                Ok((v4, OddBoxConfigVersion::V3, true))
            }
            AnyOddBoxConfig::V4(v4) => Ok((v4.clone(), OddBoxConfigVersion::V4, false)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigWrapper {
    internal_configuration: crate::configuration::OddBoxConfig,
    /// Process backends keyed by backend ID (which is also the hostname for routes)
    pub hosted_processes: DashMap<String, v4::ProcessBackend>,
    /// Remote backends keyed by backend ID
    pub remote_sites: DashMap<String, v4::RemoteBackend>,
    /// Static backends keyed by backend ID
    pub static_sites: DashMap<String, v4::StaticBackend>,
    /// Docker containers discovered at runtime
    pub docker_containers: DashMap<String, crate::docker::ContainerProxyTarget>,
    pub wrapper_cache_map_is_dirty: bool,
    pub internal_version: u64,
}

impl std::ops::Deref for ConfigWrapper {
    type Target = crate::configuration::OddBoxConfig;
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
    /// Creates a new ConfigWrapper from the latest version of a configuration file,
    /// initializing the DashMaps from the backends in the config.
    pub fn new(config: crate::configuration::OddBoxConfig) -> Self {
        let hosted_processes = DashMap::new();
        let remote_sites = DashMap::new();
        let static_sites = DashMap::new();

        // Populate DashMaps from backends
        for (id, backend) in &config.backends {
            match backend {
                v4::Backend::Process(p) => {
                    hosted_processes.insert(id.clone(), p.clone());
                }
                v4::Backend::Remote(r) => {
                    remote_sites.insert(id.clone(), r.clone());
                }
                v4::Backend::Static(s) => {
                    static_sites.insert(id.clone(), s.clone());
                }
            }
        }

        ConfigWrapper {
            internal_version: 0,
            internal_configuration: config,
            hosted_processes,
            remote_sites,
            static_sites,
            wrapper_cache_map_is_dirty: false,
            docker_containers: DashMap::new(),
        }
    }

    // re-populate the dashmaps from the internal configuration's backends
    pub fn reload_dashmaps(&mut self) {
        self.hosted_processes.clear();
        self.remote_sites.clear();
        self.static_sites.clear();

        for (id, backend) in &self.internal_configuration.backends {
            match backend {
                v4::Backend::Process(p) => {
                    self.hosted_processes.insert(id.clone(), p.clone());
                }
                v4::Backend::Remote(r) => {
                    self.remote_sites.insert(id.clone(), r.clone());
                }
                v4::Backend::Static(s) => {
                    self.static_sites.insert(id.clone(), s.clone());
                }
            }
        }

        self.wrapper_cache_map_is_dirty = false;
    }

    /// Persists the current state of the DashMaps back into the config's backends.
    /// This method should be called before serialization.
    pub fn persist(&mut self) {
        // Rebuild backends HashMap from DashMaps
        self.internal_configuration.backends.clear();

        for entry in self.hosted_processes.iter() {
            self.internal_configuration.backends.insert(
                entry.key().clone(),
                v4::Backend::Process(entry.value().clone()),
            );
        }

        for entry in self.remote_sites.iter() {
            self.internal_configuration.backends.insert(
                entry.key().clone(),
                v4::Backend::Remote(entry.value().clone()),
            );
        }

        for entry in self.static_sites.iter() {
            self.internal_configuration.backends.insert(
                entry.key().clone(),
                v4::Backend::Static(entry.value().clone()),
            );
        }

        self.wrapper_cache_map_is_dirty = false;
    }

    pub fn set_disk_path(&mut self, cfg_path: &str) -> anyhow::Result<()> {
        let path = std::path::Path::new(cfg_path);
        // If the file exists, canonicalize it. Otherwise, canonicalize the parent dir and append the filename.
        let full_path = if path.exists() {
            path.canonicalize()?
                .to_str()
                .unwrap_or_default()
                .to_string()
        } else if let Some(parent) = path.parent() {
            let parent_canonical = if parent.as_os_str().is_empty() {
                std::env::current_dir()?
            } else {
                parent.canonicalize()?
            };
            let filename = path.file_name().unwrap_or_default();
            parent_canonical
                .join(filename)
                .to_str()
                .unwrap_or_default()
                .to_string()
        } else {
            cfg_path.to_string()
        };
        self.path = Some(full_path);
        Ok(())
    }

    pub fn is_valid(&self) -> anyhow::Result<()> {
        // Check global env vars don't include PORT
        if self.env.contains_key("PORT") || self.env.contains_key("port") {
            anyhow::bail!(
                "Invalid configuration. You cannot use 'port' as a global environment variable"
            );
        }

        let mut route_hosts = std::collections::HashMap::new();
        let mut ports = std::collections::HashMap::new();

        // Collect all route hostnames
        if let Some(http) = &self.frontends.http {
            for (host, target) in &http.routes {
                route_hosts
                    .entry(host.clone())
                    .and_modify(|count| *count += 1)
                    .or_insert(1);

                // Check Let's Encrypt + capture_subdomains conflicts
                if let v4::RouteTarget::Detailed(d) = target {
                    if d.lets_encrypt && d.capture_subdomains {
                        anyhow::bail!(
                            "Invalid configuration for route '{}'. LetsEncrypt cannot be enabled when capture_subdomains is enabled as odd-box does not yet support wildcard certificates",
                            host
                        );
                    }
                }
            }
        }

        // Check for duplicate route hostnames
        let duplicate_hosts: Vec<String> = route_hosts
            .into_iter()
            .filter_map(|(name, count)| if count > 1 { Some(name) } else { None })
            .collect();

        if !duplicate_hosts.is_empty() {
            anyhow::bail!(format!(
                "Duplicate route hostnames found: {}",
                duplicate_hosts.join(", ")
            ));
        }

        // Check process backends for port conflicts
        for (id, backend) in &self.backends {
            if let v4::Backend::Process(p) = backend {
                if let Some(port) = p.port {
                    ports.entry(port).or_insert_with(Vec::new).push(id.clone());
                }

                // Check for PORT env var mismatch
                if let Some(port) = p.port {
                    if let Some(env_port_str) = p.env.get("PORT").or_else(|| p.env.get("port")) {
                        if let Ok(env_port) = env_port_str.parse::<u16>() {
                            if env_port != port {
                                anyhow::bail!(format!(
                                    "Environment variable PORT for backend '{}' does not match the port specified in the configuration.\n\
                                    It is recommended to rely on the port setting - it will automatically inject the port variable to the process-local context.",
                                    id
                                ));
                            }
                        }
                    }
                }
            }
        }

        let duplicate_ports: Vec<(u16, Vec<String>)> = ports
            .into_iter()
            .filter(|(_, backends)| backends.len() > 1)
            .collect();

        if !duplicate_ports.is_empty() {
            let conflict_details: Vec<String> = duplicate_ports
                .into_iter()
                .map(|(port, backends)| format!("Port {}: [{}]", port, backends.join(", ")))
                .collect();

            anyhow::bail!(format!(
                "Duplicate ports found with conflicting backends: {}",
                conflict_details.join("; ")
            ));
        }

        Ok(())
    }

    pub fn get_parent_path(&self) -> anyhow::Result<String> {
        // todo - use cache and clear on path change
        // if let Some(pre_resolved) = &self.1 {
        //     return Ok(pre_resolved.to_string())
        // }
        let p = self
            .path
            .clone()
            .ok_or(anyhow::anyhow!(String::from("Failed to resolve path.")))?;
        if let Some(directory_path_str) = std::path::Path::new(&p)
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

    pub fn busy_ports(&self) -> Vec<(ProcId, u16)> {
        self.hosted_processes
            .iter()
            .flat_map(|entry| {
                let p = entry.value();
                let mut items = Vec::new();

                // manually set ports need to be marked as busy even if the process is not running
                if let Some(port) = p.port {
                    items.push((p.proc_id.clone(), port));
                }

                // active ports means that there is a loop active for this process using that port
                if let Some(port) = p.active_port {
                    items.push((p.proc_id.clone(), port));
                }

                items
            })
            .collect()
    }

    pub async fn find_and_set_unused_port(
        selfy: &mut Self,
        proc: &mut v4::ProcessBackend,
    ) -> anyhow::Result<u16> {
        let used_ports: Vec<u16> = selfy
            .hosted_processes
            .iter()
            .filter_map(|entry| entry.value().port)
            .collect();

        if let Some(manually_chosen_port) = proc.port {
            if used_ports.contains(&manually_chosen_port) {
                bail!("The port configured for this backend is already in use..")
            } else {
                return Ok(manually_chosen_port);
            }
        }

        // if nothing is running and user has not selected any specific one, use the first port from the start range
        Ok(selfy.port_range_start)
    }

    /// Add or replace a process backend
    pub async fn add_or_replace_process_backend(
        &mut self,
        id: &str,
        backend: v4::ProcessBackend,
        _state: Arc<crate::GlobalState>,
    ) -> anyhow::Result<()> {
        // Update the DashMap
        self.hosted_processes
            .insert(id.to_string(), backend.clone());

        // Update the internal configuration
        self.internal_configuration
            .backends
            .insert(id.to_string(), v4::Backend::Process(backend));

        self.write_to_disk()
    }

    /// Add or replace a static backend
    pub async fn add_or_replace_static_backend(
        &mut self,
        id: &str,
        backend: v4::StaticBackend,
        _state: Arc<crate::GlobalState>,
    ) -> anyhow::Result<()> {
        self.static_sites.insert(id.to_string(), backend.clone());

        self.internal_configuration
            .backends
            .insert(id.to_string(), v4::Backend::Static(backend));

        self.write_to_disk()
    }

    /// Add or replace a remote backend
    pub async fn add_or_replace_remote_backend(
        &mut self,
        id: &str,
        backend: v4::RemoteBackend,
        _state: Arc<crate::GlobalState>,
    ) -> anyhow::Result<()> {
        self.remote_sites.insert(id.to_string(), backend.clone());

        self.internal_configuration
            .backends
            .insert(id.to_string(), v4::Backend::Remote(backend));

        self.write_to_disk()
    }

    pub fn port_is_free(port: u16) -> bool {
        match std::net::TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => {
                drop(listener);
                true
            }
            Err(_) => false,
        }
    }

    pub fn get_random_free_port() -> Option<u16> {
        match std::net::TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => match listener.local_addr() {
                Ok(l) => Some(l.port()),
                _ => None,
            },
            Err(e) => {
                tracing::warn!("{:?}", e);
                None
            }
        }
    }

    /// Set the active port for a process backend
    pub fn set_active_port(
        &mut self,
        backend_id: &str,
        proc: &mut v4::ProcessBackend,
    ) -> anyhow::Result<u16> {
        let mut selected_port = proc.active_port;

        // ports in use or configured for use by other backends
        let unavailable_ports: Vec<(ProcId, u16)> = self
            .busy_ports()
            .into_iter()
            .filter(|x| x.0 != proc.proc_id)
            .collect();

        if let Some(currently_selected_port) = selected_port {
            if !unavailable_ports
                .iter()
                .any(|x| x.1 == currently_selected_port)
            {
                if Self::port_is_free(currently_selected_port) {
                    return Ok(currently_selected_port);
                } else {
                    selected_port = None;
                }
            }
        }

        // decide which port to use (ie. which port to add as the environment variable PORT)
        if let Some(preferred_port) = proc.port {
            if preferred_port == 0 {
                selected_port = Self::get_random_free_port()
            } else {
                if let Some(taken_by) = unavailable_ports.iter().find(|x| x.1 == preferred_port) {
                    tracing::warn!(
                        "[{}] The configured port '{}' is unavailable (configured for another backend: '{}')..",
                        backend_id,
                        preferred_port,
                        taken_by.1
                    );
                } else {
                    tracing::info!(
                        "[{}] Starting on port '{}' as configured for the process!",
                        backend_id,
                        preferred_port
                    );
                    selected_port = Some(preferred_port);
                }
            }
        } else if let Some(value) = proc.env.get("PORT").or_else(|| proc.env.get("port")) {
            if let Some(taken_by) = unavailable_ports.iter().find(|x| x.1.to_string() == *value) {
                tracing::warn!(
                    "[{}] The configured port (via env var in cfg) '{}' is unavailable (configured for another backend: '{}')..",
                    backend_id,
                    value,
                    taken_by.1
                );
            } else if let Ok(spbev) = value.parse::<u16>() {
                tracing::info!(
                    "[{}] Starting on port '{}' as selected via a configured environment variable for port!",
                    backend_id,
                    value
                );
                selected_port = Some(spbev)
            } else {
                tracing::info!(
                    "[{}] The env var for port was configured to '{}' which is not a valid u16, ignoring.",
                    backend_id,
                    value
                );
            }
        }

        // if no port manually specified, find the first available port
        if selected_port.is_none() {
            let min_auto_port = self.port_range_start;
            let unavailable: Vec<u16> = unavailable_ports.iter().map(|x| x.1).collect();
            let mut inner_selected_port = min_auto_port;
            loop {
                if unavailable.contains(&inner_selected_port) {
                    inner_selected_port += 1;
                } else if Self::port_is_free(inner_selected_port) {
                    break;
                } else {
                    inner_selected_port += 1;
                }
            }
            tracing::trace!(
                "[{}] Using the first available port found (starting from the configured start port: {min_auto_port}) ---> '{}'",
                backend_id,
                inner_selected_port
            );
            selected_port = Some(inner_selected_port);
        }

        // mark this process as using this port
        if let Some(sp) = selected_port {
            // Update in the DashMap
            if let Some(mut entry) = self.hosted_processes.get_mut(backend_id) {
                entry.active_port = Some(sp);
            } else {
                tracing::error!(
                    "[{}] Could not find backend in hosted_processes DashMap.. This is a bug in odd-box!",
                    backend_id
                );
            }

            // Also update in the internal config
            if let Some(v4::Backend::Process(p)) =
                self.internal_configuration.backends.get_mut(backend_id)
            {
                p.active_port = Some(sp);
            }
        }

        if let Some(p) = selected_port {
            Ok(p)
        } else {
            bail!("Failed to find a port for the process..")
        }
    }

    /// Resolve a static backend configuration with variable substitution
    pub fn resolve_static_backend(
        &self,
        item: &v4::StaticBackend,
    ) -> anyhow::Result<v4::StaticBackend> {
        let mut resolved = item.clone();

        let resolved_home_dir_path = dirs::home_dir().ok_or(anyhow::anyhow!(String::from(
            "Failed to resolve home directory."
        )))?;
        let resolved_home_dir_str =
            resolved_home_dir_path
                .to_str()
                .ok_or(anyhow::anyhow!(String::from(
                    "Failed to parse home directory."
                )))?;

        let cfg_dir = self.get_parent_path()?;

        let root_dir = self.resolve_root_dir(&cfg_dir, resolved_home_dir_str)?;

        let with_vars = |x: &str| -> String {
            x.replace("$root_dir", &root_dir)
                .replace("$cfg_dir", &cfg_dir)
                .replace("~", resolved_home_dir_str)
        };

        resolved.dir = with_vars(&item.dir);

        Ok(resolved)
    }

    /// Helper to resolve $root_dir
    fn resolve_root_dir(&self, cfg_dir: &str, home_dir: &str) -> anyhow::Result<String> {
        if let Some(rd) = &self.root_dir {
            if rd.contains("$root_dir") {
                anyhow::bail!(
                    "it is clearly not a good idea to use $root_dir in the configuration of root dir..."
                )
            }

            let rd_with_vars_replaced = rd.replace("$cfg_dir", cfg_dir).replace("~", home_dir);

            match std::fs::canonicalize(&rd_with_vars_replaced) {
                Ok(resolved_path) => Ok(resolved_path.display().to_string().replace("\\\\?\\", "")),
                Err(e) => {
                    anyhow::bail!(format!(
                        "root_dir item in configuration ({rd}) resolved to this: '{rd_with_vars_replaced}' - error: {}",
                        e
                    ));
                }
            }
        } else {
            let current_directory = std::env::current_dir()?;
            Ok(current_directory.display().to_string())
        }
    }

    /// Resolve a process backend configuration with variable substitution.
    /// This MUST be called by proc_host prior to starting a process.
    pub fn resolve_process_backend(
        &self,
        backend_id: &str,
        proc: &v4::ProcessBackend,
    ) -> anyhow::Result<ResolvedProcessBackend> {
        let resolved_home_dir_path = dirs::home_dir().ok_or(anyhow::anyhow!(String::from(
            "Failed to resolve home directory."
        )))?;
        let resolved_home_dir_str =
            resolved_home_dir_path
                .to_str()
                .ok_or(anyhow::anyhow!(String::from(
                    "Failed to parse home directory."
                )))?;

        let cfg_dir = self.get_parent_path()?;
        let root_dir = self.resolve_root_dir(&cfg_dir, resolved_home_dir_str)?;

        let with_vars = |x: &str| -> String {
            x.replace("$root_dir", &root_dir)
                .replace("$cfg_dir", &cfg_dir)
                .replace("~", resolved_home_dir_str)
        };

        let resolved_args: Vec<String> = proc.args.iter().map(|a| with_vars(a)).collect();
        let resolved_dir = proc.dir.as_ref().map(|d| with_vars(d));
        let resolved_bin = with_vars(&proc.bin);

        // Convert env HashMap to Vec<EnvVar> for compatibility
        let env_vars: Vec<EnvVar> = proc
            .env
            .iter()
            .map(|(k, v)| EnvVar {
                key: k.clone(),
                value: v.clone(),
            })
            .collect();

        Ok(ResolvedProcessBackend {
            backend_id: backend_id.to_string(),
            proc_id: proc.proc_id.clone(),
            active_port: proc.active_port,
            bin: resolved_bin,
            args: resolved_args,
            dir: resolved_dir,
            env_vars,
            protocol: proc.protocol.clone(),
            https: proc.https,
            port: proc.port,
            auto_start: proc.auto_start,
            exclude_from_start_all: proc.exclude_from_start_all,
            log_level: proc.log_level.clone(),
            log_format: proc.log_format.clone(),
        })
    }
}

/// A fully resolved process backend configuration ready for execution
#[derive(Debug, Clone)]
pub struct ResolvedProcessBackend {
    pub backend_id: String,
    pub proc_id: ProcId,
    pub active_port: Option<u16>,
    pub bin: String,
    pub args: Vec<String>,
    pub dir: Option<String>,
    pub env_vars: Vec<EnvVar>,
    pub protocol: v4::Protocol,
    pub https: bool,
    pub port: Option<u16>,
    pub auto_start: Option<bool>,
    pub exclude_from_start_all: bool,
    pub log_level: Option<LogLevel>,
    pub log_format: Option<LogFormat>,
}
