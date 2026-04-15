//! Strongly-typed configuration structs for every odd-box config generation.
//!
//! This module lets us deserialize any historical config format (legacy TOML,
//! V1, V2, V3 TOML) into typed Rust structs, then upgrade through the chain:
//!
//!   legacy → V1 → V2 → V3 → TunnelCliConfiguration (cruma format)
//!
//! The cruma `TunnelCliConfiguration` **is** the current format.  There is no
//! separate "V4" — cruma's config IS V4.

pub mod legacy;
pub mod v1;
pub mod v2;
pub mod v3;

use cruma::{config::TunnelCliConfiguration, cruma_proxy_lib::types::AcmeDirectory};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Shared types used across all config generations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Default)]
#[allow(non_camel_case_types)]
pub enum LogFormat {
    #[default]
    standard,
    dotnet,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub enum OddBoxConfigVersion {
    #[default]
    Unmarked,
    V1,
    V2,
    V3,
}

// ---------------------------------------------------------------------------
// AnyOddBoxConfig — auto-detect + parse any generation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AnyOddBoxConfig {
    Legacy(legacy::LegacyConfig),
    V1(v1::V1Config),
    V2(v2::V2Config),
    V3(v3::V3Config),
}

impl AnyOddBoxConfig {
    /// Parse configuration content (auto-detects YAML vs TOML).
    pub fn parse(content: &str) -> Result<AnyOddBoxConfig, String> {
        // Try TOML formats (V3 and earlier) — newest first
        let v3_result = toml::from_str::<v3::V3Config>(content);
        if let Ok(v3_config) = v3_result {
            return Ok(AnyOddBoxConfig::V3(v3_config));
        };

        let v2_result = toml::from_str::<v2::V2Config>(content);
        if let Ok(v2_config) = v2_result {
            return Ok(AnyOddBoxConfig::V2(v2_config));
        };

        let v1_result = toml::from_str::<v1::V1Config>(content);
        if let Ok(v1_config) = v1_result {
            return Ok(AnyOddBoxConfig::V1(v1_config));
        };

        let legacy_result = toml::from_str::<legacy::LegacyConfig>(content);
        if let Ok(legacy_config) = legacy_result {
            return Ok(AnyOddBoxConfig::Legacy(legacy_config));
        };

        // Build a helpful error message
        if content.contains("version = \"V3\"") {
            Err(format!(
                "invalid v3 configuration file.\n{}",
                v3_result.unwrap_err()
            ))
        } else if content.contains("version = \"V2\"") {
            Err(format!(
                "invalid v2 configuration file.\n{}",
                v2_result.unwrap_err()
            ))
        } else if content.contains("version = \"V1\"") {
            Err(format!(
                "invalid v1 configuration file.\n{}",
                v1_result.unwrap_err()
            ))
        } else {
            Err(format!(
                "invalid (legacy) configuration file.\n{}",
                legacy_result.unwrap_err()
            ))
        }
    }

    /// Upgrade any config generation all the way to a cruma
    /// `TunnelCliConfiguration`.
    ///
    /// Returns `(config, original_version)`.
    pub fn upgrade_to_cruma(
        &self,
    ) -> Result<(TunnelCliConfiguration, OddBoxConfigVersion), String> {
        // First, upgrade through the typed chain to get a V3Config.
        let v3 = match self {
            AnyOddBoxConfig::Legacy(cfg) => {
                let v1: v1::V1Config = cfg.clone().try_into()?;
                let v2: v2::V2Config = v1.try_into()?;
                let v3: v3::V3Config = v2.try_into()?;
                (v3, OddBoxConfigVersion::Unmarked)
            }
            AnyOddBoxConfig::V1(cfg) => {
                let v2: v2::V2Config = cfg.clone().try_into()?;
                let v3: v3::V3Config = v2.try_into()?;
                (v3, OddBoxConfigVersion::V1)
            }
            AnyOddBoxConfig::V2(cfg) => {
                let v3: v3::V3Config = cfg.clone().try_into()?;
                (v3, OddBoxConfigVersion::V2)
            }
            AnyOddBoxConfig::V3(cfg) => (cfg.clone(), OddBoxConfigVersion::V3),
        };

        // Then convert V3 → TunnelCliConfiguration (the cruma/V4 format).
        let cruma_cfg = v3_to_cruma(&v3.0)?;
        Ok((cruma_cfg, v3.1))
    }
}

// ---------------------------------------------------------------------------
// V3 → TunnelCliConfiguration conversion
// ---------------------------------------------------------------------------

/// Convert a typed V3Config into a cruma `TunnelCliConfiguration`.
pub fn v3_to_cruma(v3: &v3::V3Config) -> Result<TunnelCliConfiguration, String> {
    use cruma::config::*;
    use std::collections::HashMap;

    let mut backends = Vec::<BackendDefinition>::new();
    let mut frontends = Vec::<FrontendDefinition>::new();
    let mut processes = Vec::<ProcessDefinition>::new();
    let mut listeners = Vec::<ListenerDefinition>::new();
    let mut global_env = HashMap::<String, String>::new();

    // ── Global env vars ─────────────────────────────────────────────────
    for ev in &v3.env_vars {
        global_env.insert(ev.key.clone(), ev.value.clone());
    }

    let ip = v3
        .ip
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let bind_addr = match ip.as_str() {
        "0.0.0.0" => ListenerBindAddress::All,
        _ => ListenerBindAddress::Localhost,
    };

    // ── Listeners ──────────────────────────────────────────────────────
    let http_port = v3.http_port.unwrap_or(8080);
    let tls_port = v3.tls_port.unwrap_or(4343);

    listeners.push(ListenerDefinition {
        kind: Some(ListenerKind::Http),
        port: http_port,
        addr: bind_addr.clone(),
        tls: false,
        cert_mode: ListenerCertMode::default(),
    });

    listeners.push(ListenerDefinition {
        kind: Some(ListenerKind::Https),
        port: tls_port,
        addr: bind_addr,
        tls: true,
        cert_mode: ListenerCertMode::default(),
    });

    let auto_start_global = v3.auto_start.unwrap_or(true);
    let port_range_start = v3.port_range_start;

    // ── Hosted processes → ProcessDefinition + FrontendDefinition ──────
    let mut port_offset: u16 = 0;
    if let Some(hosted) = &v3.hosted_process {
        for proc in hosted {
            let assigned_port = proc.port.unwrap_or_else(|| {
                let p = port_range_start + port_offset;
                port_offset += 1;
                p
            });

            // Per-process env (merge global + per-process)
            let mut proc_env = global_env.clone();
            if let Some(env_vars) = &proc.env_vars {
                for ev in env_vars {
                    proc_env.insert(ev.key.clone(), ev.value.clone());
                }
            }
            // Inject PORT if not present
            proc_env
                .entry("PORT".to_string())
                .or_insert_with(|| assigned_port.to_string());

            // Convert hints to upstream protocol
            let upstream_protocol = hints_to_upstream_protocol(proc.hints.as_ref());

            let process_id = proc.host_name.clone();

            processes.push(ProcessDefinition {
                binary_watch: None,
                id: process_id.clone(),
                command: proc.bin.clone(),
                args: proc.args.clone().unwrap_or_default(),
                working_directory: proc.dir.as_ref().map(|d| d.into()),
                env: proc_env,
                auto_start: proc.auto_start.unwrap_or(auto_start_global),
                start_on_request: true, // keep legacy behavior
                restart_policy: ProcessRestartPolicy::Never,
                upstream_protocol,
                upstream_tls: proc.https.unwrap_or(false),
                backend_timeout_seconds: Some(60), // legacy behavior was unlimited while new default using None is 10 sec. lets do 1min at least
                idle_timeout_seconds: None,
                binary_watch: None,
            });

            // Frontend routing
            frontends.push(FrontendDefinition {
                hostname: HostName(proc.host_name.clone()),
                backend_id: None,
                path_routes: vec![],
                process_id: Some(process_id.clone()),
                listener_kinds: None,
                form_auth: None,
                api_key_auth: None,
                jwt_auth: None,
                oauth2_auth: None,
                middlewares: Vec::new(),
                alpn: None,
                forward_host_header: true,
                cert_mode_override: None,
                cert_mode_overrides: None,
            });

            // Wildcard companion for capture_subdomains
            if proc.capture_subdomains.unwrap_or(false) {
                frontends.push(FrontendDefinition {
                    hostname: HostName(format!("*.{}", proc.host_name)),
                    backend_id: None,
                    path_routes: vec![],
                    process_id: Some(process_id),
                    listener_kinds: None,
                    form_auth: None,
                    api_key_auth: None,
                    jwt_auth: None,
                    oauth2_auth: None,
                    middlewares: Vec::new(),
                    alpn: None,
                    forward_host_header: true,
                    cert_mode_override: None,
                    cert_mode_overrides: None,
                });
            }
        }
    }

    // ── Remote targets → BackendDefinition + FrontendDefinition ────────
    if let Some(remotes) = &v3.remote_target {
        for remote in remotes {
            let backend_id = remote.host_name.clone();

            let (destination, use_https) = if let Some(first) = remote.backends.first() {
                let port = if first.port == 0 {
                    if first.https.unwrap_or(false) {
                        443
                    } else {
                        80
                    }
                } else {
                    first.port
                };
                (
                    format!("{}:{}", first.address, port),
                    first.https.unwrap_or(false),
                )
            } else {
                ("localhost:80".to_string(), false)
            };

            let kind = if use_https {
                TargetKind::Https
            } else {
                TargetKind::Http
            };

            let upstream_protocol = remote
                .backends
                .first()
                .and_then(|b| b.hints.as_ref())
                .map(|h| hints_to_upstream_protocol(Some(h)))
                .unwrap_or(UpstreamProtocol::H1);

            backends.push(BackendDefinition {
                id: backend_id.clone(),
                kind,
                destination,
                upstream_protocol,
                allow_directory_indexing: None,
                render_markdown: None,
                spa_fallback: None,
                form_auth: None,
                api_key_auth: None,
                jwt_auth: None,
                oauth2_auth: None,
                middlewares: Vec::new(),
                backend_timeout_seconds: None,
            });

            frontends.push(FrontendDefinition {
                hostname: HostName(remote.host_name.clone()),
                backend_id: Some(backend_id.clone()),
                path_routes: vec![],
                process_id: None,
                listener_kinds: None,
                form_auth: None,
                api_key_auth: None,
                jwt_auth: None,
                oauth2_auth: None,
                middlewares: Vec::new(),
                alpn: None,
                forward_host_header: remote.keep_original_host_header.unwrap_or(true),
                cert_mode_override: None,
                cert_mode_overrides: None,
            });

            if remote.capture_subdomains.unwrap_or(false) {
                frontends.push(FrontendDefinition {
                    hostname: HostName(format!("*.{}", remote.host_name)),
                    backend_id: Some(backend_id),
                    path_routes: vec![],
                    process_id: None,
                    listener_kinds: None,
                    form_auth: None,
                    api_key_auth: None,
                    jwt_auth: None,
                    oauth2_auth: None,
                    middlewares: Vec::new(),
                    alpn: None,
                    forward_host_header: remote.keep_original_host_header.unwrap_or(true),
                    cert_mode_override: None,
                    cert_mode_overrides: None,
                });
            }
        }
    }

    // ── Dir servers → BackendDefinition + FrontendDefinition ───────────
    if let Some(dirs) = &v3.dir_server {
        for dir in dirs {
            let backend_id = dir.host_name.clone();

            backends.push(BackendDefinition {
                id: backend_id.clone(),
                kind: TargetKind::LocalDirectory,
                destination: dir.dir.clone(),
                upstream_protocol: UpstreamProtocol::H1,
                allow_directory_indexing: Some(dir.enable_directory_browsing.unwrap_or(false)),
                render_markdown: Some(dir.render_markdown.unwrap_or(false)),
                spa_fallback: None,
                form_auth: None,
                api_key_auth: None,
                jwt_auth: None,
                oauth2_auth: None,
                middlewares: Vec::new(),
                backend_timeout_seconds: None,
            });

            frontends.push(FrontendDefinition {
                hostname: HostName(dir.host_name.clone()),
                backend_id: Some(backend_id.clone()),
                path_routes: vec![],
                process_id: None,
                listener_kinds: None,
                form_auth: None,
                api_key_auth: None,
                jwt_auth: None,
                oauth2_auth: None,
                middlewares: Vec::new(),
                alpn: None,
                forward_host_header: true,
                cert_mode_override: None,
                cert_mode_overrides: None,
            });

            if dir.capture_subdomains.unwrap_or(false) {
                frontends.push(FrontendDefinition {
                    hostname: HostName(format!("*.{}", dir.host_name)),
                    backend_id: Some(backend_id),
                    path_routes: vec![],
                    process_id: None,
                    listener_kinds: None,
                    form_auth: None,
                    api_key_auth: None,
                    jwt_auth: None,
                    oauth2_auth: None,
                    middlewares: Vec::new(),
                    alpn: None,
                    forward_host_header: true,
                    cert_mode_override: None,
                    cert_mode_overrides: None,
                });
            }
        }
    }

    Ok(TunnelCliConfiguration {
        config_path: None,
        pre_expansion_snapshot: None,
        root_dir: v3.root_dir.clone(),
        backends,
        frontends,
        processes,
        global_env,
        listeners,
        local_only: true,
        tunnel_secret: "ANON".to_string(),
        tunnel_id: "ANON".to_string(),
        tower_server: "tower.cruma.io:443".to_string(),
        temp: false,
        profile: None,
        custom_pages: Default::default(),
        dir_listing_branding: None,
        acme_directory: AcmeDirectory::LetsEncrypt { staging: false },
        acme_eab: None,
    })
}

/// Convert V3 hints to cruma UpstreamProtocol.
fn hints_to_upstream_protocol(hints: Option<&Vec<v3::Hint>>) -> cruma::config::UpstreamProtocol {
    use cruma::config::UpstreamProtocol;
    let hints = match hints {
        Some(h) => h,
        None => return UpstreamProtocol::H1,
    };

    if hints.iter().any(|h| matches!(h, v3::Hint::H2)) {
        UpstreamProtocol::H2
    } else if hints.iter().any(|h| matches!(h, v3::Hint::H2CPK)) {
        UpstreamProtocol::H2PK
    } else {
        UpstreamProtocol::H1
    }
}
