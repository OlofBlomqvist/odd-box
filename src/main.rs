mod configuration;
mod migrate;

use anyhow::{Context, Result, bail};
use clap::Parser;
use cruma::bootstrap::{ApplicationRuntime, BootstrapOptions};
use cruma::config::{TunnelCliConfiguration, load_config_from_path};
use cruma::gui::{DashboardViewMode, GuiOptions, Page, ThemeMode};
use cruma::tui::TuiOptions;
use cruma::versions::{AppUpdateProvider, InstallMethod, detect_install_method};
use semver::Version;

use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// odd-box implementation of [`AppUpdateProvider`].
///
/// Provides update instructions and About-page links tailored to odd-box
/// rather than the cruma tunnel agent.
struct OddBoxUpdateProvider {
    latest: Option<Version>,
    method: InstallMethod,
}

impl AppUpdateProvider for OddBoxUpdateProvider {
    fn latest_version(&self) -> Option<&Version> {
        self.latest.as_ref()
    }

    // detect_install_method() is application-agnostic — it only inspects the
    // exe path and env vars — so we can reuse it directly.  The default trait
    // implementations for install_method_label() and is_managed_install() are
    // derived from this, so no boilerplate needed.
    fn install_method(&self) -> InstallMethod {
        self.method
    }

    fn update_instructions(&self, new_version: &Version) -> String {
        match self.method {
            InstallMethod::Homebrew => {
                format!("Update to {new_version} by running:\n  brew upgrade odd-box")
            }
            InstallMethod::Cargo => {
                format!("Update to {new_version} by running:\n  cargo install odd-box")
            }
            InstallMethod::Nix => {
                format!(
                    "A new version ({new_version}) is available. \
                     Update your Nix flake input or nixpkgs pin to pick it up."
                )
            }
            InstallMethod::MacOsApp | InstallMethod::Direct | InstallMethod::Npm => {
                format!(
                    "odd-box {new_version} is available. \
                     Download it from https://github.com/OlofBlomqvist/odd-box/releases"
                )
            }
        }
    }

    fn about_links(&self) -> Vec<(String, String)> {
        vec![
            (
                "GitHub".into(),
                "https://github.com/OlofBlomqvist/odd-box".into(),
            ),
            (
                "Releases".into(),
                "https://github.com/OlofBlomqvist/odd-box/releases".into(),
            ),
        ]
    }
}

// Re-export rustls so we can install the crypto provider.
use rustls;

pub const NAME: &str = "odd-box";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// odd-box — a dead simple reverse proxy server
#[derive(Parser)]
#[command(
    name = "odd-box",
    version = VERSION,
    about = "A simple reverse proxy and process manager",
    long_about = "odd-box is a lightweight reverse proxy that manages backends, \
                  frontends, hosted processes, and TLS certificates.\n\n\
                  It delegates all heavy lifting to the cruma agent library."
)]
struct Args {
    /// Path to configuration file (YAML).
    /// Can be passed as a positional argument or with -c / --config.
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    /// Config file path (positional, same as --config).
    /// When both are given, --config takes precedence.
    #[arg(value_name = "CONFIG_FILE", conflicts_with = "config")]
    config_positional: Option<String>,

    /// Run the graphical user interface
    #[arg(long, conflicts_with_all = ["tui", "headless"])]
    gui: bool,

    /// Run the terminal user interface
    #[arg(long, conflicts_with_all = ["gui", "headless"])]
    tui: bool,

    /// Run in headless mode (no UI)
    #[arg(long, conflicts_with_all = ["gui", "tui"])]
    headless: bool,

    /// Run self-update
    #[arg(long)]
    update: bool,

    /// Initialize a new config file
    #[arg(long)]
    init: bool,

    /// Print JSON schema for the config format
    #[arg(long)]
    config_schema: bool,

    /// Theme: light, dark, system
    #[arg(long, value_name = "MODE")]
    theme: Option<String>,
}

// We intentionally use a synchronous `fn main()` rather than `#[tokio::main]`.
//
// The cruma GUI creates its own tokio runtime internally, so if we were
// already inside a tokio runtime (as `#[tokio::main]` provides) the nested
// `Runtime::new()` call inside iced would panic with "Cannot start a
// runtime from within a runtime."
//
// For the GUI path we use `run_gui_with_config` which bootstraps the agent
// runtime inside the GUI's own tokio runtime — no temporary runtime needed.
// For the TUI / headless paths we create a runtime that lives for the
// entire duration of the process.
fn main() -> Result<()> {
    // Install the rustls crypto provider before any TLS operations.
    // This matches what the cruma binary does in its main().
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install default crypto provider");

    // ── Odd-box embedded assets ────────────────────────────────────────
    const ODD_BOX_404: &[u8] = include_bytes!("assets/404.html");

    let args = Args::parse();

    // ── One-shot commands (no config needed) ───────────────────────────

    if args.init {
        return init_config();
    }

    if args.config_schema {
        let schema = schemars::schema_for!(TunnelCliConfiguration);
        println!("{}", serde_json::to_string_pretty(&schema)?);
        return Ok(());
    }

    // ── Load configuration ─────────────────────────────────────────────

    // Track whether the user gave us an explicit path.  When they did we
    // never auto-create — a missing explicit path is always an error.
    let explicit_config = args.config.or(args.config_positional);
    let explicit = explicit_config.is_some();
    let config_path = match explicit_config {
        Some(p) => p,
        // No path given: find an existing config or create a default one.
        None => find_or_create_default_config()?,
    };

    // Try loading directly (supports YAML, TOML, JSON).  If that fails
    // and the file looks like a legacy odd-box TOML config, auto-migrate
    // it to the current format, back up the original, and continue.
    let (mut config, config_path) = match load_config_from_path(std::path::Path::new(&config_path))
    {
        Ok(cfg) => (cfg, config_path),
        Err(load_err) if looks_like_legacy_toml(&config_path) => {
            let (cfg, new_path) = migrate::auto_migrate(&config_path)?;
            let new_path_str = new_path.to_string_lossy().into_owned();
            (cfg, new_path_str)
        }
        Err(load_err) if explicit => {
            return Err(load_err.context(format!(
                "failed to load config from '{config_path}'"
            )));
        }
        Err(load_err) => return Err(load_err.into()),
    };
    config.config_path = Some(config_path.clone().into());

    // ── Odd-box branding for directory listing / dir-server error pages ─
    config.dir_listing_branding = Some(cruma::cruma_proxy_lib::types::DirListingBranding {
        logo_url: None,
        logo_link_url: Some("https://github.com/OlofBlomqvist/odd-box".into()),
        logo_png_bytes: Some(ODD_BOX_ICON.to_vec()),
    });

    // ── Odd-box custom 404 page (embedded at compile time) ─────────────
    {
        let not_found_path = std::env::temp_dir().join("odd-box-404.html");
        if std::fs::write(&not_found_path, ODD_BOX_404).is_ok() {
            config.custom_pages.not_found_page = Some(not_found_path);
        }
    }

    // ── Build bootstrap options ────────────────────────────────────────

    let cancel = CancellationToken::new();
    let bootstrap_options = BootstrapOptions {
        skip_cache_lock: true,
        protocol: cruma::config::Protocol::Auto,
        cache_dir: None,
        temp: config.temp,
        profile: config.profile.clone(),
        tower_server: "tower.cruma.io:443".to_string(),
    };

    let theme = ThemeMode::from_cli_or_env(args.theme.as_deref());
    let want_gui = args.gui || (!args.tui && !args.headless && !is_headless_environment());

    // ── Odd-box icon (embedded PNG) for tray + window branding ─────────
    const ODD_BOX_ICON: &[u8] = include_bytes!("assets/odd-box-icon.png");

    // ── Check for a newer stable odd-box release (best-effort, ~5 s timeout) ─
    // We do this synchronously before handing control to either the GUI or TUI
    // runtime so the result is available immediately on startup.  A tiny
    // single-thread runtime is used here; it is fully dropped before the GUI
    // creates its own multi-thread runtime, avoiding any nested-runtime issues.
    let latest_odd_box_version: Option<Version> =
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt.block_on(find_latest_version_of_odd_box()),
            Err(_) => None,
        };
    let install_method = detect_install_method();

    if want_gui {
        // ── GUI path ───────────────────────────────────────────────────
        //
        // `run_gui_with_config` bootstraps the agent runtime (orchestrator,
        // transport workers, cert manager) inside the GUI's own tokio
        // runtime.  This avoids the old temp-runtime pattern where
        // `drop(rt)` killed the orchestrator task, leaving transport
        // workers sending into dead channels and preventing domain
        // assignment.
        let gui_options = GuiOptions {
            app_name: NAME.into(),
            app_version: VERSION.into(),
            logo_light: None,
            logo_dark: None,
            tray_icon_shape: cruma::gui::TrayIconShape::Box,
            window_icon: Some(ODD_BOX_ICON.to_vec()),
            pages: vec![
                Page::Dashboard,
                Page::Backends,
                Page::Frontends,
                Page::Listeners,
                Page::Graph,
                Page::Processes,
                Page::Requests,
                Page::Certificates,
                Page::Observations,
            ],
            theme,
            default_dashboard_view_mode: DashboardViewMode::Classic,
            update_provider: Some(Arc::new(OddBoxUpdateProvider {
                latest: latest_odd_box_version,
                method: install_method,
            })),
            linux_application_id: Some("odd-box".into()),
            notification_app_name: Some("odd-box".into()),
            custom_pages: vec![],
            on_bootstrap: Some(Box::new(register_oddbox_resolver)),
        };
        cruma::gui::run_gui_with_config(config, bootstrap_options, cancel, gui_options)?;
    } else {
        // ── TUI / Headless path ────────────────────────────────────────
        //
        // Both `run_tui_with_runtime` and `run_headless_with_runtime` are
        // async, so we create a long-lived runtime and block_on them.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        rt.block_on(async {
            let runtime = Arc::new(
                cruma::bootstrap::bootstrap_from_config(config, bootstrap_options, cancel.clone())
                    .await?,
            );

            register_oddbox_resolver(&runtime);

            if args.tui {
                let tui_options = TuiOptions {
                    app_name: NAME.into(),
                    app_version: VERSION.into(),
                    update_provider: Some(Arc::new(OddBoxUpdateProvider {
                        latest: latest_odd_box_version,
                        method: install_method,
                    })),
                };
                cruma::tui::run_tui_with_runtime(runtime, cancel, tui_options).await
            } else {
                // Headless mode
                cruma::headless::init_headless_tracing();
                tracing::info!("{NAME} v{VERSION} started (headless). Press Ctrl-C to quit.");
                cruma::headless::run_headless_with_runtime(runtime, cancel).await
            }
        })?;
    }

    Ok(())
}

/// Register odd-box's custom dynamic backend resolver on the live runtime.
///
/// This is called from all three entry paths (GUI via `on_bootstrap`, TUI,
/// and headless) so the resolver is always available regardless of mode.
///
/// We have this here to support a legacy-feature of odd-box used in
/// CI and Pipelines that are still in use which we do not want to break.
///
fn register_oddbox_resolver(runtime: &Arc<ApplicationRuntime>) {
    use cruma::cruma_proxy_lib::types::{
        DynamicBackendId, DynamicBackendResolver, ResolvedBackend, Target,
    };
    use cruma::process_hosting::{DefaultProcessHost, ProcessHost};

    // Grab a handle to the process host from the runtime.  We wrap it in
    // an Arc so the closure (which must be Fn + Send + Sync) can share it.
    let process_host = Arc::new(DefaultProcessHost::with_orchestrator(
        runtime.process_host.orchestrator().clone(),
    ));

    let resolver: DynamicBackendResolver = Arc::new(move |ctx| {
        let process_host = process_host.clone();
        Box::pin(async move {
            // Only allow from the loopback adapter.
            if !ctx.client_addr.ip().is_loopback() {
                return Ok(None);
            }
            // Match "/STOP?proc=<name>"
            if ctx.path != "/STOP" {
                return Ok(None);
            }
            let proc_name = ctx
                .uri
                .query()
                .and_then(|q| q.split('&').find_map(|pair| pair.strip_prefix("proc=")))
                .map(|s| s.to_string());

            let Some(name) = proc_name else {
                return Ok(Some(Arc::new(ResolvedBackend::Target(Target::Respond {
                    status: 400,
                    body: Some(b"missing ?proc= parameter".to_vec()),
                    content_type: Some("text/plain".into()),
                }))));
            };

            match process_host.stop_process(&name) {
                Ok(()) => Ok(Some(Arc::new(ResolvedBackend::Target(Target::Respond {
                    status: 200,
                    body: Some(format!("stopped process '{name}'").into_bytes()),
                    content_type: Some("text/plain".into()),
                })))),
                Err(e) => Ok(Some(Arc::new(ResolvedBackend::Target(Target::Respond {
                    status: 500,
                    body: Some(format!("failed to stop '{name}': {e}").into_bytes()),
                    content_type: Some("text/plain".into()),
                })))),
            }
        })
    });

    // Register the resolver into the live proxy configuration.
    let cfg_handle = &runtime.proxy_configuration;
    let mut cfg = cfg_handle.load().as_ref().clone();
    cfg.dynamic_backend_resolvers
        .insert(DynamicBackendId::from("odd-box"), resolver);
    cfg_handle.store(Arc::new(cfg));
}

/// Search for a config file in the current directory.
/// Search for an existing config file and return its path.
/// If no config is found anywhere, create a starter config at the default
/// platform location and return that path.
///
/// Search order:
///   1. Current working directory — several conventional file names.
///   2. Platform config directory:
///        Linux:   `$XDG_CONFIG_HOME/odd-box/` (defaults to `~/.config/odd-box/`)
///        macOS:   `~/Library/Application Support/odd-box/`
///                 AND `~/.config/odd-box/` (many devs use dotfiles here)
///        Windows: `%APPDATA%\\odd-box\\`
///   3. Auto-create a starter config in the platform config dir so that
///      first-run (e.g. launching the .app bundle) always succeeds.
fn find_or_create_default_config() -> Result<String> {
    const CWD_CANDIDATES: &[&str] = &[
        "odd-box.toml",
        "oddbox.toml",
        "odd-box.yaml",
        "oddbox.yaml",
        "odd-box.yml",
        "oddbox.yml",
        "config.yaml",
    ];

    // 1. Current working directory.
    for candidate in CWD_CANDIDATES {
        if std::fs::metadata(candidate).is_ok() {
            return Ok(candidate.to_string());
        }
    }

    // 2. Platform config directories.
    //    Build the list of directories to probe in preference order.
    let mut config_dirs_to_check: Vec<std::path::PathBuf> = Vec::new();
    if let Some(d) = dirs::config_dir() {
        config_dirs_to_check.push(d.join("odd-box"));
    }
    // On macOS dirs::config_dir() returns ~/Library/Application Support.
    // Also probe ~/.config/odd-box/ since that is where many developers
    // keep dotfiles and where the terminal build naturally lands.
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        let dotconfig = home.join(".config").join("odd-box");
        if !config_dirs_to_check.contains(&dotconfig) {
            config_dirs_to_check.push(dotconfig);
        }
    }
    for odd_box_dir in &config_dirs_to_check {
        for candidate in ["odd-box.toml", "odd-box.yaml", "odd-box.yml"] {
            let path = odd_box_dir.join(candidate);
            if path.exists() {
                return Ok(path.to_string_lossy().into_owned());
            }
        }
    }

    // 3. Nothing found — create a starter config at the first writable
    //    platform config directory so that first-run always has something
    //    to load (e.g. launching the macOS .app bundle for the first time).
    let create_dir = config_dirs_to_check.into_iter().next().ok_or_else(|| {
        anyhow::anyhow!(
            "No config file found and could not determine a platform config directory.\n\
             Pass --config <path> or create odd-box.toml in the current directory."
        )
    })?;
    let create_path = create_dir.join("odd-box.toml");
    std::fs::create_dir_all(&create_dir)
        .with_context(|| format!("failed to create config directory {:?}", create_dir))?;
    std::fs::write(&create_path, default_config_toml())
        .with_context(|| format!("failed to write default config to {:?}", create_path))?;
    eprintln!(
        "No config file found — created a starter config at {}\n\
         Edit it to add your backends and frontends, then restart odd-box.",
        create_path.display()
    );
    Ok(create_path.to_string_lossy().into_owned())
}

/// Generate a minimal starter config.
/// Returns the contents of a minimal starter config file.
fn default_config_toml() -> String {
    format!(
        r#"# odd-box configuration
# Generated by {NAME} v{VERSION}
# See https://github.com/OlofBlomqvist/odd-box for documentation

backends = []
frontends = []

[[listeners]]
port = 8080
addr = "localhost"
kind = "http"
tls = false

[[listeners]]
port = 4343
addr = "localhost"
kind = "https"
tls = true
"#
    )
}

fn init_config() -> Result<()> {
    let target = "odd-box.toml";
    if std::fs::metadata(target).is_ok() {
        bail!("{target} already exists. Remove it first or use a different directory.");
    }
    let toml = default_config_toml();
    std::fs::write(target, &toml)?;
    println!("Created {target}");
    println!();
    println!("Next steps:");
    println!("  1. Edit {target} to add your backends, processes, and frontends");
    println!("  2. Run `odd-box` to start the proxy");
    Ok(())
}

/// Check whether a config file looks like a **legacy** odd-box TOML config
/// (V1/V2/V3) as opposed to a new-format cruma TOML config.
///
/// The distinction matters because `.toml` files now also serve as a valid
/// config format for the current cruma schema.  We look for telltale
/// legacy markers (`version = "V…"`, `[[hosted_process]]`, etc.) so that
/// new-format TOML files are loaded directly by cruma and only genuine
/// legacy files go through the auto-migration path.
fn looks_like_legacy_toml(path: &str) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // YAML / YML files are never legacy TOML.
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        return false;
    }

    // Look for markers that only appear in legacy odd-box configs.
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Explicit version tag from V1/V2/V3 configs.
        if trimmed.starts_with("version") && trimmed.contains('"') {
            return true;
        }
        // Section headers unique to the legacy schema.
        if trimmed == "[[hosted_process]]"
            || trimmed == "[[remote_target]]"
            || trimmed == "[[dir_server]]"
            || trimmed.starts_with("[[hosted_process.") // e.g. [[hosted_process.backends]]
            || trimmed.starts_with("[[remote_target.")
        {
            return true;
        }
        // Legacy top-level keys that don't exist in the new format.
        if trimmed.starts_with("root_dir")
            || trimmed.starts_with("port_range_start")
            || trimmed.starts_with("hosted_process")
            || trimmed.starts_with("remote_target")
        {
            return true;
        }
    }

    false
}

/// Detect whether we are running in an environment without a display.
fn is_headless_environment() -> bool {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
            return true;
        }
    }
    false
}

/// Fetch the latest stable (non-pre-release) odd-box release from GitHub.
///
/// Queries the GitHub releases API, ignores anything with `prerelease: true`
/// or an unparseable semver tag, and returns the highest remaining version.
/// Returns `None` on any network or parse failure so callers always get a
/// best-effort result without ever blocking startup indefinitely.
async fn find_latest_version_of_odd_box() -> Option<Version> {
    #[derive(serde::Deserialize)]
    struct GhRelease {
        tag_name: String,
        prerelease: bool,
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent(concat!("odd-box/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

    let releases: Vec<GhRelease> = client
        .get("https://api.github.com/repos/OlofBlomqvist/odd-box/releases")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;

    let current = Version::parse(env!("CARGO_PKG_VERSION").trim_start_matches('v')).ok()?;

    let latest = releases
        .into_iter()
        .filter(|r| !r.prerelease)
        .filter_map(|r| {
            let tag = r.tag_name.trim_start_matches('v');
            Version::parse(tag).ok()
        })
        .max()?;

    if latest > current { Some(latest) } else { None }
}
