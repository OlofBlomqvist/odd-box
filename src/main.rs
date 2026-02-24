mod configuration;
mod migrate;
mod pages;
mod self_update;

use anyhow::{Result, bail};
use clap::Parser;
use cruma::bootstrap::{ApplicationRuntime, BootstrapOptions};
use cruma::config::{TunnelCliConfiguration, load_config_from_path};
use cruma::gui::{DashboardViewMode, GuiOptions, Page, ThemeMode};
use cruma::tui::TuiOptions;

use std::sync::Arc;
use tokio_util::sync::CancellationToken;

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

    if args.update {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let result = rt.block_on(self_update::update())?;
        match result {
            self_update::UpdateAction::Updated => {
                println!("odd-box updated successfully. Please restart.");
            }
            self_update::UpdateAction::NoUpdateNeeded => {}
        }
        return Ok(());
    }

    if args.init {
        return init_config();
    }

    if args.config_schema {
        let schema = schemars::schema_for!(TunnelCliConfiguration);
        println!("{}", serde_json::to_string_pretty(&schema)?);
        return Ok(());
    }

    // ── Load configuration ─────────────────────────────────────────────

    let config_path = args.config
        .or(args.config_positional)
        .unwrap_or_else(find_config_file);

    // Try loading directly (supports YAML, TOML, JSON).  If that fails
    // and the file looks like a legacy odd-box TOML config, auto-migrate
    // it to the current format, back up the original, and continue.
    let (mut config, config_path) = match load_config_from_path(std::path::Path::new(&config_path)) {
        Ok(cfg) => (cfg, config_path),
        Err(load_err) if looks_like_legacy_toml(&config_path) => {
            let (cfg, new_path) = migrate::auto_migrate(&config_path)?;
            let new_path_str = new_path.to_string_lossy().into_owned();
            (cfg, new_path_str)
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
            update_info: None,
            linux_application_id: Some("odd-box".into()),
            notification_app_name: Some("odd-box".into()),
            custom_pages: pages::custom_gui_pages(),
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
                    update_info: None,
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
                .and_then(|q| {
                    q.split('&')
                        .find_map(|pair| pair.strip_prefix("proc="))
                })
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
fn find_config_file() -> String {
    for candidate in [
        "odd-box.toml",
        "oddbox.toml",
        "odd-box.yaml",
        "oddbox.yaml",
        "odd-box.yml",
        "oddbox.yml",
        "config.yaml",
    ] {
        if std::fs::metadata(candidate).is_ok() {
            return candidate.to_string();
        }
    }
    "odd-box.toml".to_string()
}

/// Generate a minimal starter config.
fn init_config() -> Result<()> {
    let target = "odd-box.toml";
    if std::fs::metadata(target).is_ok() {
        bail!("{target} already exists. Remove it first or use a different directory.");
    }

    let toml = format!(
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
    );

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
