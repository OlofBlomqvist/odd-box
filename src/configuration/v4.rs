use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{LogFormat, LogLevel};
use crate::{configuration::yaml_air, types::proc_info::ProcId};

// ============================================================================
// V4 Configuration - YAML-based with Frontend/Backend separation
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash, JsonSchema)]
pub enum V4VersionEnum {
    #[default]
    V4,
}

/// Root configuration structure for odd-box V4
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct OddBoxV4Config {
    /// Schema version marker
    #[serde(default)]
    pub version: V4VersionEnum,

    /// Path to the configuration file (internal use, not serialized)
    #[serde(skip)]
    pub path: Option<String>,

    // ========================================================================
    // Global Settings
    // ========================================================================
    /// Log level for odd-box itself (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,

    /// Starting port for auto-assigned process ports
    #[serde(default = "default_port_range_start")]
    pub port_range_start: u16,

    /// IP address to bind listeners to (e.g. "127.0.0.1" or "0.0.0.0")
    #[serde(default = "default_ip")]
    pub ip: String,

    /// Global environment variables available to all process backends
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Root directory variable ($root_dir) expansion
    pub root_dir: Option<String>,

    /// Default log format for process output
    #[serde(default)]
    pub default_log_format: LogFormat,

    /// Auto-start processes on odd-box startup
    #[serde(default = "default_true")]
    pub auto_start: bool,

    /// Cruma tunnel agent configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cruma: Option<CrumaConfig>,

    // ========================================================================
    // Admin Interface
    // ========================================================================
    /// Hostname for the admin UI/API (e.g., "admin.localhost")
    pub admin_api_host: Option<String>,

    /// Password for admin API access
    pub admin_api_password: Option<String>,

    // ========================================================================
    // Backends & Frontends
    // ========================================================================
    /// Backend definitions (the upstream targets)
    #[serde(default)]
    pub backends: HashMap<String, Backend>,

    /// Frontend definitions (the listeners)
    #[serde(default)]
    pub frontends: Frontends,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(untagged)]
pub enum CrumaConfig {
    /// Shorthand mode string (e.g. "anon")
    Mode(String),
    /// Authenticated tunnel credentials
    Auth { id: String, key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrumaMode {
    Anonymous,
    Authenticated { id: String, key: String },
}

impl CrumaConfig {
    pub fn mode(&self) -> Option<CrumaMode> {
        match self {
            CrumaConfig::Mode(value) => match value.trim().to_ascii_lowercase().as_str() {
                "anon" | "anonymous" => Some(CrumaMode::Anonymous),
                _ => None,
            },
            CrumaConfig::Auth { id, key } => Some(CrumaMode::Authenticated {
                id: id.clone(),
                key: key.clone(),
            }),
        }
    }
}

// ============================================================================
// Backend Types
// ============================================================================

/// A backend defines an upstream target that can receive proxied requests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Backend {
    /// A process managed by odd-box
    Process(ProcessBackend),
    /// A remote HTTP(S) server
    Remote(RemoteBackend),
    /// A static file directory
    Static(StaticBackend),
}

impl Backend {
    pub fn get_process(&self) -> Option<&ProcessBackend> {
        match self {
            Backend::Process(p) => Some(p),
            _ => None,
        }
    }

    pub fn get_process_mut(&mut self) -> Option<&mut ProcessBackend> {
        match self {
            Backend::Process(p) => Some(p),
            _ => None,
        }
    }
}

/// A process backend - odd-box spawns and manages this process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ProcessBackend {
    /// Internal process ID (not serialized)
    #[serde(skip, default = "ProcId::new")]
    pub proc_id: ProcId,

    /// Binary to execute
    pub bin: String,

    /// Arguments to pass to the binary
    #[serde(default)]
    pub args: Vec<String>,

    /// Working directory for the process
    pub dir: Option<String>,

    /// Environment variables specific to this process
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Upstream protocol (h1, h2, h2c, h2cpk)
    #[serde(default)]
    pub protocol: Protocol,

    /// Whether the backend speaks HTTPS
    #[serde(default)]
    pub https: bool,

    /// Fixed port (if not set, auto-assigned from port_range_start)
    pub port: Option<u16>,

    /// Auto-start this process with odd-box
    pub auto_start: Option<bool>,

    /// Exclude from "start all" command
    #[serde(default)]
    pub exclude_from_start_all: bool,

    /// Log level for this process's output
    pub log_level: Option<LogLevel>,

    /// Log format for parsing process output
    pub log_format: Option<LogFormat>,
}

/// A remote backend - proxy to external server(s)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct RemoteBackend {
    /// List of upstream endpoints (for load balancing)
    pub endpoints: Vec<Endpoint>,

    /// Upstream protocol
    #[serde(default)]
    pub protocol: Protocol,

    /// Whether the backend speaks HTTPS
    #[serde(default)]
    pub https: bool,

    /// Forward the original Host header instead of the backend's
    #[serde(default)]
    pub keep_original_host_header: bool,
}

/// A static file backend - serve files from a directory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, JsonSchema)]
pub struct StaticBackend {
    /// Directory to serve files from
    pub dir: String,

    /// Index file name (default: index.html)
    #[serde(default = "default_index")]
    pub index: String,

    /// Enable directory listing
    #[serde(default)]
    pub list_dir: bool,

    /// Render markdown files as HTML
    #[serde(default)]
    pub render_markdown: bool,

    /// Cache-Control max-age header value in seconds
    pub cache_max_age: Option<u64>,

    /// When enabled, requests that would result in a 404 will instead
    /// serve the nearest ancestor index file. This is the standard
    /// behaviour needed by single-page applications (React, Vue, Svelte, etc.)
    /// whose client-side router handles all navigation paths.
    #[serde(default)]
    pub spa_fallback: bool,
}

/// An upstream endpoint (address + port)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, JsonSchema)]
pub struct Endpoint {
    /// Hostname or IP address
    pub addr: String,
    /// Port number
    pub port: u16,
}

/// Upstream protocol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// HTTP/1.1
    #[default]
    H1,
    /// HTTP/2 over TLS (ALPN negotiated)
    H2,
    /// HTTP/2 over cleartext with Upgrade header
    H2C,
    /// HTTP/2 over cleartext with prior knowledge
    H2CPK,
}

// ============================================================================
// Frontend Types
// ============================================================================

/// Frontend definitions - the listeners
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, JsonSchema)]
pub struct Frontends {
    /// HTTP listener configuration
    pub http: Option<HttpFrontend>,

    /// HTTPS listener configuration
    pub https: Option<HttpsFrontend>,
}

/// HTTP frontend listener
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct HttpFrontend {
    /// Port to listen on (default: 80)
    #[serde(default = "default_http_port")]
    pub port: u16,

    /// Routing rules: hostname -> backend
    #[serde(default)]
    pub routes: HashMap<String, RouteTarget>,
}

/// HTTPS frontend listener
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct HttpsFrontend {
    /// Port to listen on (default: 443)
    #[serde(default = "default_https_port")]
    pub port: u16,

    /// TLS certificate mode
    #[serde(default)]
    pub cert: CertMode,

    /// Routing rules - if not specified, inherits from HTTP frontend
    pub routes: Option<HttpsRoutes>,
}

/// HTTPS route configuration - either inherit from HTTP or define explicitly
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(untagged)]
pub enum HttpsRoutes {
    /// Inherit routes from HTTP frontend
    Inherit(InheritMarker),
    /// Explicit route definitions
    Explicit(HashMap<String, RouteTarget>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, JsonSchema)]
pub enum InheritMarker {
    #[serde(rename = "inherit")]
    Inherit,
}

/// A route target - can be a simple backend name or detailed config
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(untagged)]
pub enum RouteTarget {
    /// Simple backend reference by name
    Simple(String),
    /// Detailed route configuration
    Detailed(DetailedRoute),
}

impl RouteTarget {
    pub fn backend_id(&self) -> &str {
        match self {
            RouteTarget::Simple(s) => s,
            RouteTarget::Detailed(d) => &d.backend,
        }
    }

    pub fn capture_subdomains(&self) -> bool {
        match self {
            RouteTarget::Simple(_) => false,
            RouteTarget::Detailed(d) => d.capture_subdomains,
        }
    }

    pub fn redirect_to_https(&self) -> bool {
        match self {
            RouteTarget::Simple(_) => false,
            RouteTarget::Detailed(d) => d.redirect_to_https,
        }
    }

    pub fn enable_cruma(&self) -> bool {
        match self {
            RouteTarget::Simple(_) => false,
            RouteTarget::Detailed(d) => d.enable_cruma,
        }
    }

    pub fn lets_encrypt(&self) -> bool {
        match self {
            RouteTarget::Simple(_) => false,
            RouteTarget::Detailed(d) => d.lets_encrypt,
        }
    }
}

/// Detailed route configuration with additional options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, JsonSchema)]
pub struct DetailedRoute {
    /// Backend to route to
    pub backend: String,

    /// Capture subdomains (*.example.com)
    #[serde(default)]
    pub capture_subdomains: bool,

    /// Forward subdomain to backend (test.example.com -> test.backend)
    #[serde(default)]
    pub forward_subdomains: bool,

    /// Redirect HTTP to HTTPS
    #[serde(default)]
    pub redirect_to_https: bool,

    /// Use Let's Encrypt for this route
    #[serde(default)]
    pub lets_encrypt: bool,

    /// Expose this route through the cruma tunnel
    #[serde(default)]
    pub enable_cruma: bool,
}

/// TLS certificate mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum CertMode {
    /// Use self-signed certificates
    #[default]
    SelfSigned,
    /// Use ACME/Let's Encrypt
    Acme,
    // Future: custom cert files
    // Custom { cert: String, key: String },
}

// ============================================================================
// Default Value Functions
// ============================================================================

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_port_range_start() -> u16 {
    4200
}

fn default_ip() -> String {
    "127.0.0.1".to_string()
}

fn default_true() -> bool {
    true
}

fn default_index() -> String {
    "index.html".to_string()
}

fn default_http_port() -> u16 {
    80
}

fn default_https_port() -> u16 {
    443
}

// ============================================================================
// Implementation
// ============================================================================

// ============================================================================
// V3 -> V4 Upgrade
// ============================================================================

impl TryFrom<super::v3::OddBoxV3Config> for OddBoxV4Config {
    type Error = String;

    fn try_from(v3: super::v3::OddBoxV3Config) -> Result<Self, Self::Error> {
        let mut backends = HashMap::new();
        let mut routes = HashMap::new();

        // Convert hosted_process -> Process backends + routes
        if let Some(processes) = v3.hosted_process {
            for proc in processes {
                let backend_id = proc.host_name.clone();

                // Convert hints to protocol
                let protocol = hints_to_protocol(proc.hints.as_ref());

                // Convert env_vars from Vec<EnvVar> to HashMap
                let env: HashMap<String, String> = proc
                    .env_vars
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| (e.key, e.value))
                    .collect();

                backends.insert(
                    backend_id.clone(),
                    Backend::Process(ProcessBackend {
                        proc_id: ProcId::new(),
                        bin: proc.bin,
                        args: proc.args.unwrap_or_default(),
                        dir: proc.dir,
                        env,
                        protocol,
                        https: proc.https.unwrap_or(false),
                        port: proc.port,
                        auto_start: proc.auto_start,
                        exclude_from_start_all: proc.exclude_from_start_all.unwrap_or(false),
                        log_level: proc.log_level,
                        log_format: proc.log_format,
                    }),
                );

                // Create route for this backend
                let capture_subdomains = proc.capture_subdomains.unwrap_or(false);
                let forward_subdomains = proc.forward_subdomains.unwrap_or(false);
                let redirect_to_https = proc.redirect_to_https.unwrap_or(false);
                let lets_encrypt = proc.enable_lets_encrypt.unwrap_or(false);

                if capture_subdomains || forward_subdomains || redirect_to_https || lets_encrypt {
                    routes.insert(
                        proc.host_name,
                        RouteTarget::Detailed(DetailedRoute {
                            backend: backend_id,
                            capture_subdomains,
                            forward_subdomains,
                            redirect_to_https,
                            lets_encrypt,
                            enable_cruma: false,
                        }),
                    );
                } else {
                    routes.insert(proc.host_name, RouteTarget::Simple(backend_id));
                }
            }
        }

        // Convert remote_target -> Remote backends + routes
        if let Some(remotes) = v3.remote_target {
            for remote in remotes {
                let backend_id = remote.host_name.clone();

                // Convert backends to endpoints
                let endpoints: Vec<Endpoint> = remote
                    .backends
                    .iter()
                    .map(|b| Endpoint {
                        addr: b.address.clone(),
                        port: if b.port == 0 {
                            if b.https.unwrap_or(false) { 443 } else { 80 }
                        } else {
                            b.port
                        },
                    })
                    .collect();

                // Get protocol from first backend's hints
                let protocol = remote
                    .backends
                    .first()
                    .and_then(|b| b.hints.as_ref())
                    .map(|h| hints_to_protocol(Some(h)))
                    .unwrap_or(Protocol::H1);

                let https = remote
                    .backends
                    .first()
                    .and_then(|b| b.https)
                    .unwrap_or(false);

                backends.insert(
                    backend_id.clone(),
                    Backend::Remote(RemoteBackend {
                        endpoints,
                        protocol,
                        https,
                        keep_original_host_header: remote
                            .keep_original_host_header
                            .unwrap_or(false),
                    }),
                );

                // Create route
                let capture_subdomains = remote.capture_subdomains.unwrap_or(false);
                let forward_subdomains = remote.forward_subdomains.unwrap_or(false);
                let redirect_to_https = remote.redirect_to_https.unwrap_or(false);
                let lets_encrypt = remote.enable_lets_encrypt.unwrap_or(false);

                if capture_subdomains || forward_subdomains || redirect_to_https || lets_encrypt {
                    routes.insert(
                        remote.host_name,
                        RouteTarget::Detailed(DetailedRoute {
                            backend: backend_id,
                            capture_subdomains,
                            forward_subdomains,
                            redirect_to_https,
                            lets_encrypt,
                            enable_cruma: false,
                        }),
                    );
                } else {
                    routes.insert(remote.host_name, RouteTarget::Simple(backend_id));
                }
            }
        }

        // Convert dir_server -> Static backends + routes
        if let Some(dirs) = v3.dir_server {
            for dir in dirs {
                let backend_id = dir.host_name.clone();

                backends.insert(
                    backend_id.clone(),
                    Backend::Static(StaticBackend {
                        dir: dir.dir,
                        index: "index.html".to_string(),
                        list_dir: dir.enable_directory_browsing.unwrap_or(false),
                        render_markdown: dir.render_markdown.unwrap_or(false),
                        cache_max_age: dir.cache_control_max_age_in_seconds,
                        spa_fallback: false,
                    }),
                );

                // Create route
                let capture_subdomains = dir.capture_subdomains.unwrap_or(false);
                let redirect_to_https = dir.redirect_to_https.unwrap_or(false);
                let lets_encrypt = dir.enable_lets_encrypt.unwrap_or(false);

                if capture_subdomains || redirect_to_https || lets_encrypt {
                    routes.insert(
                        dir.host_name,
                        RouteTarget::Detailed(DetailedRoute {
                            backend: backend_id,
                            capture_subdomains,
                            forward_subdomains: false,
                            redirect_to_https,
                            lets_encrypt,
                            enable_cruma: false,
                        }),
                    );
                } else {
                    routes.insert(dir.host_name, RouteTarget::Simple(backend_id));
                }
            }
        }

        // Convert global env_vars to HashMap
        let env: HashMap<String, String> =
            v3.env_vars.into_iter().map(|e| (e.key, e.value)).collect();

        // Build frontends
        let http_port = v3.http_port.unwrap_or(8080);
        let https_port = v3.tls_port.unwrap_or(4343);

        let frontends = Frontends {
            http: Some(HttpFrontend {
                port: http_port,
                routes: routes.clone(),
            }),
            https: Some(HttpsFrontend {
                port: https_port,
                // todo: we dont support certmode on site-level in v4 (yet) and so we will instead now default to use self-signed for the listener
                // and the user will need to toggle the new flag once migration is complete
                cert: CertMode::SelfSigned,
                routes: Some(HttpsRoutes::Inherit(InheritMarker::Inherit)),
            }),
        };

        Ok(OddBoxV4Config {
            version: V4VersionEnum::V4,
            path: v3.path,
            log_level: v3.log_level.unwrap_or(LogLevel::Info),
            port_range_start: v3.port_range_start,
            ip: v3
                .ip
                .map(|addr| addr.to_string())
                .unwrap_or_else(default_ip),
            env,
            root_dir: v3.root_dir,
            default_log_format: v3.default_log_format,
            auto_start: v3.auto_start.unwrap_or(true),
            cruma: None,
            admin_api_host: v3.odd_box_url,
            admin_api_password: v3.odd_box_password,
            backends,
            frontends,
        })
    }
}

/// Convert V3 hints to V4 protocol
fn hints_to_protocol(hints: Option<&Vec<super::v3::Hint>>) -> Protocol {
    let hints = match hints {
        Some(h) => h,
        None => return Protocol::H1,
    };

    // Priority: H2 > H2CPK > H2C > H1
    if hints.iter().any(|h| matches!(h, super::v3::Hint::H2)) {
        Protocol::H2
    } else if hints.iter().any(|h| matches!(h, super::v3::Hint::H2CPK)) {
        Protocol::H2CPK
    } else if hints.iter().any(|h| matches!(h, super::v3::Hint::H2C)) {
        Protocol::H2C
    } else {
        Protocol::H1
    }
}

impl OddBoxV4Config {
    /// Parse a YAML configuration string
    pub fn parse_yaml(content: &str) -> Result<Self, String> {
        serde_yaml::from_str(content).map_err(|e| e.to_string())
    }

    /// Serialize to YAML string
    pub fn to_yaml(&self) -> Result<String, String> {
        let yaml = serde_pretty_yaml::to_string_pretty(self).map_err(|e| e.to_string())?;
        Ok(yaml_air::format_str(&yaml))
    }

    /// Write configuration to disk
    pub fn write_to_disk(&self) -> anyhow::Result<()> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No path set for configuration"))?;

        let yaml = self
            .to_yaml()
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;

        std::fs::write(path, yaml)?;
        Ok(())
    }

    /// Get all process backends
    pub fn process_backends(&self) -> impl Iterator<Item = (&String, &ProcessBackend)> {
        self.backends.iter().filter_map(|(k, v)| match v {
            Backend::Process(p) => Some((k, p)),
            _ => None,
        })
    }

    /// Get all remote backends
    pub fn remote_backends(&self) -> impl Iterator<Item = (&String, &RemoteBackend)> {
        self.backends.iter().filter_map(|(k, v)| match v {
            Backend::Remote(r) => Some((k, r)),
            _ => None,
        })
    }

    /// Get all static backends
    pub fn static_backends(&self) -> impl Iterator<Item = (&String, &StaticBackend)> {
        self.backends.iter().filter_map(|(k, v)| match v {
            Backend::Static(s) => Some((k, s)),
            _ => None,
        })
    }

    /// Get HTTP routes, returning empty map if no HTTP frontend
    pub fn http_routes(&self) -> &HashMap<String, RouteTarget> {
        static EMPTY: std::sync::OnceLock<HashMap<String, RouteTarget>> =
            std::sync::OnceLock::new();
        self.frontends
            .http
            .as_ref()
            .map(|f| &f.routes)
            .unwrap_or_else(|| EMPTY.get_or_init(HashMap::new))
    }

    /// Get HTTPS routes, inheriting from HTTP if configured
    pub fn https_routes(&self) -> &HashMap<String, RouteTarget> {
        static EMPTY: std::sync::OnceLock<HashMap<String, RouteTarget>> =
            std::sync::OnceLock::new();

        if let Some(https) = &self.frontends.https {
            match &https.routes {
                Some(HttpsRoutes::Explicit(routes)) => routes,
                Some(HttpsRoutes::Inherit(_)) | None => self.http_routes(),
            }
        } else {
            EMPTY.get_or_init(HashMap::new)
        }
    }

    /// Create an example configuration
    pub fn example() -> Self {
        let mut backends = HashMap::new();

        backends.insert(
            "my-app".to_string(),
            Backend::Process(ProcessBackend {
                proc_id: ProcId::new(),
                bin: "node".to_string(),
                args: vec!["server.js".to_string()],
                dir: Some("$root_dir/my-app".to_string()),
                env: [("NODE_ENV".to_string(), "production".to_string())].into(),
                protocol: Protocol::H1,
                https: false,
                port: None,
                auto_start: Some(true),
                exclude_from_start_all: false,
                log_level: None,
                log_format: None,
            }),
        );

        backends.insert(
            "api-servers".to_string(),
            Backend::Remote(RemoteBackend {
                endpoints: vec![
                    Endpoint {
                        addr: "10.0.0.1".to_string(),
                        port: 8080,
                    },
                    Endpoint {
                        addr: "10.0.0.2".to_string(),
                        port: 8080,
                    },
                ],
                protocol: Protocol::H2,
                https: true,
                keep_original_host_header: false,
            }),
        );

        backends.insert(
            "docs".to_string(),
            Backend::Static(StaticBackend {
                dir: "/var/www/docs".to_string(),
                index: "index.html".to_string(),
                list_dir: true,
                render_markdown: true,
                cache_max_age: Some(3600),
                spa_fallback: false,
            }),
        );

        let mut routes = HashMap::new();
        routes.insert(
            "myapp.local".to_string(),
            RouteTarget::Simple("my-app".to_string()),
        );
        routes.insert(
            "api.local".to_string(),
            RouteTarget::Simple("api-servers".to_string()),
        );
        routes.insert(
            "docs.local".to_string(),
            RouteTarget::Detailed(DetailedRoute {
                backend: "docs".to_string(),
                capture_subdomains: true,
                forward_subdomains: false,
                redirect_to_https: true,
                lets_encrypt: false,
                enable_cruma: false,
            }),
        );

        Self {
            version: V4VersionEnum::V4,
            path: None,
            log_level: LogLevel::Info,
            port_range_start: 4200,
            ip: default_ip(),
            env: HashMap::new(),
            root_dir: Some("~".to_string()),
            default_log_format: LogFormat::standard,
            auto_start: true,
            cruma: None,
            admin_api_host: Some("admin.localhost".to_string()),
            admin_api_password: None,
            backends,
            frontends: Frontends {
                http: Some(HttpFrontend {
                    port: 80,
                    routes: routes.clone(),
                }),
                https: Some(HttpsFrontend {
                    port: 443,
                    cert: CertMode::SelfSigned,
                    routes: Some(HttpsRoutes::Inherit(InheritMarker::Inherit)),
                }),
            },
        }
    }
}
