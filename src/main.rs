#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod configuration;
mod migrate;
mod profile_page;
mod profiles;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, FromArgMatches};
use cruma::bootstrap::{ApplicationRuntime, BootstrapOptions};
use cruma::config::{
    ProxyCmd, StartCmd, TunnelCli, TunnelCliCmd, TunnelCliConfiguration, load_config_from_path,
};
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
    #[cfg(target_os = "windows")]
    windows_attach_parent_console_if_present();

    // Install the rustls crypto provider before any TLS operations.
    // This matches what the cruma binary does in its main().
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install default crypto provider");

    // Brand tunnel registration with the embedding application name.
    cruma_tunnels_lib::init(NAME);

    // ── Odd-box embedded assets ────────────────────────────────────────
    const ODD_BOX_404: &[u8] = include_bytes!("assets/404.html");
    const ODD_BOX_502: &[u8] = include_bytes!("assets/502.html");
    const ODD_BOX_504: &[u8] = include_bytes!("assets/504.html");
    const ODD_BOX_OFFLINE: &[u8] = include_bytes!("assets/offline.html");
    const ODD_BOX_503: &[u8] = include_bytes!("assets/503.html");

    let cli = parse_cli()?;

    // ── One-shot commands (no config needed) ───────────────────────────
    if cli.generate_completions.is_some() {
        bail!("`--generate-completions` is not supported by odd-box.");
    }

    match &cli.command {
        Some(TunnelCliCmd::Schema) => {
            let schema = schemars::schema_for!(TunnelCliConfiguration);
            println!("{}", serde_json::to_string_pretty(&schema)?);
            return Ok(());
        }
        Some(TunnelCliCmd::ShowCache) => {
            let cache_dir = cruma::bootstrap::get_cache_dir(&cli)?;
            println!("Cache directory: {}", cache_dir.display());
            return Ok(());
        }
        Some(TunnelCliCmd::ClearCache) => {
            return cruma::bootstrap::clear_cache_dir(&cli);
        }
        Some(TunnelCliCmd::Config(_)) => bail!("`odd-box config ...` is not supported."),
        _ => {}
    }

    // ── Determine UI mode early (needed for profile ask_on_startup) ───────
    let want_gui = should_launch_gui(&cli);

    let (mut config, config_path_for_runtime, initial_gui_page, saved_profiles) =
        load_runtime_config(&cli, want_gui)?;
    let config_path_for_runtime: String = config_path_for_runtime;

    if !config_path_for_runtime.is_empty() {
        config.config_path = Some(config_path_for_runtime.clone().into());
    }

    // ── Odd-box branding for directory listing / dir-server error pages ─
    config.dir_listing_branding = Some(cruma::cruma_proxy_lib::types::DirListingBranding {
        logo_url: None,
        logo_url_light: None,
        logo_link_url: Some("https://github.com/OlofBlomqvist/odd-box".into()),
        logo_png_bytes: Some(ODD_BOX_ICON.to_vec()),
        logo_png_bytes_light: Some(ODD_BOX_ICON_LIGHT.to_vec()),
    });

    // ── Odd-box custom error pages (embedded at compile time) ──────────
    {
        let tmp = std::env::temp_dir();
        let pages: &[(&[u8], &str, fn(&mut cruma::config::CustomPages, std::path::PathBuf))] = &[
            (ODD_BOX_404,     "odd-box-404.html",     |cp, p| cp.not_found_page      = Some(p)),
            (ODD_BOX_502,     "odd-box-502.html",     |cp, p| cp.bad_gateway_page     = Some(p)),
            (ODD_BOX_504,     "odd-box-504.html",     |cp, p| cp.gateway_timeout_page = Some(p)),
            (ODD_BOX_OFFLINE, "odd-box-offline.html", |cp, p| cp.process_offline_page      = Some(p)),
            (ODD_BOX_503,     "odd-box-503.html",     |cp, p| cp.service_unavailable_page  = Some(p)),
        ];
        for (bytes, filename, set) in pages {
            let path = tmp.join(filename);
            if std::fs::write(&path, bytes).is_ok() {
                set(&mut config.custom_pages, path);
            }
        }
    }

    // ── Build bootstrap options ────────────────────────────────────────

    let cancel = CancellationToken::new();
    let bootstrap_options = BootstrapOptions {
        skip_cache_lock: true,
        protocol: cli_protocol(&cli),
        cache_dir: None,
        temp: config.temp,
        profile: config.profile.clone(),
        tower_server: cli.tower_server.clone(),
    };

    let theme = ThemeMode::from_cli_or_env(cli.theme.as_deref());

    // ── Odd-box icon (embedded PNG) for tray + window branding ─────────
    const ODD_BOX_ICON: &[u8] = include_bytes!("assets/odd-box-icon.png");
    // Light-mode variant (dark lines on transparent, visible on white backgrounds)
    const ODD_BOX_ICON_LIGHT: &[u8] = include_bytes!("../ob3_black.png");

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
                Page::Notices,
            ],
            theme,
            default_dashboard_view_mode: DashboardViewMode::Classic,
            update_provider: Some(Arc::new(OddBoxUpdateProvider {
                latest: latest_odd_box_version,
                method: install_method,
            })),
            linux_application_id: Some("odd-box".into()),
            notification_app_name: Some("odd-box".into()),
            custom_pages: vec![Box::new(profile_page::ProfilePage::new(saved_profiles))],
            on_bootstrap: Some(Box::new(register_oddbox_resolver)),
            initial_page: initial_gui_page,
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

            if cli.tui || command_is_gui_incompatible(&cli) {
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

#[cfg(target_os = "windows")]
fn windows_attach_parent_console_if_present() {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, FreeConsole, GetConsoleMode, GetConsoleProcessList,
        GetConsoleWindow, GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};

    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            return;
        }

        let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
        if stdout != null_mut() && stdout != INVALID_HANDLE_VALUE {
            let mut mode = 0;
            if GetConsoleMode(stdout, &mut mode) != 0 {
                let mut process_ids = [0u32; 4];
                let attached_processes =
                    GetConsoleProcessList(process_ids.as_mut_ptr(), process_ids.len() as u32);
                if attached_processes > 1 {
                    return;
                }
            }
        }

        let stdin = GetStdHandle(STD_INPUT_HANDLE);
        if stdin != null_mut() && stdin != INVALID_HANDLE_VALUE {
            let mut mode = 0;
            if GetConsoleMode(stdin, &mut mode) != 0 {
                let mut process_ids = [0u32; 4];
                let attached_processes =
                    GetConsoleProcessList(process_ids.as_mut_ptr(), process_ids.len() as u32);
                if attached_processes > 1 {
                    return;
                }
            }
        }

        let stderr = GetStdHandle(STD_ERROR_HANDLE);
        if stderr != null_mut() && stderr != INVALID_HANDLE_VALUE {
            let mut mode = 0;
            if GetConsoleMode(stderr, &mut mode) != 0 {
                let mut process_ids = [0u32; 4];
                let attached_processes =
                    GetConsoleProcessList(process_ids.as_mut_ptr(), process_ids.len() as u32);
                if attached_processes > 1 {
                    return;
                }
            }
        }

        let console_window = GetConsoleWindow();
        if console_window != null_mut() {
            FreeConsole();
            ShowWindow(console_window, SW_HIDE);
        }
    }
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
    let runtime_for_resolver = runtime.clone();
    let source_listeners = runtime.source_listeners.clone();

    let resolver: DynamicBackendResolver = Arc::new(move |ctx| {
        let process_host = process_host.clone();
        let runtime = runtime_for_resolver.clone();
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

            let Some(mut name) = proc_name else {
                let config = runtime.proxy_config.load();
                let orchestrator = process_host.orchestrator();
                let mut stopped = 0usize;
                let mut failed = 0usize;

                for def in config.processes.iter() {
                    let Some(host) = orchestrator.find_host(&def.id) else {
                        continue;
                    };

                    if !host.status().running {
                        continue;
                    }

                    match process_host.stop_process(&def.id) {
                        Ok(()) => {
                            stopped += 1;
                        }
                        Err(err) => {
                            tracing::error!(
                                process_id = %def.id,
                                error = %err,
                                "Failed to stop hosted process from /STOP"
                            );
                            failed += 1;
                        }
                    }
                }

                let body = if failed > 0 {
                    format!("stopped {stopped} process(es), {failed} failed")
                } else {
                    format!("stopped {stopped} process(es)")
                };

                return Ok(Some(Arc::new(ResolvedBackend::Target(Target::Respond {
                    status: 200,
                    body: Some(body.into_bytes()),
                    content_type: Some("text/plain".into()),
                }))));
            };

            if !name.is_empty() {
                // build candidate names: the original, plus a dot-to-dash variant if applicable
                let mut candidates = vec![name.clone()];
                if name.contains('.') {
                    candidates.push(name.replace('.', "-"));
                }

                let mut resolved: Option<String> = None;
                if let Ok(procs) = process_host.list_processes() {
                    for candidate in &candidates {
                        if procs.iter().any(|p| p.id == *candidate) {
                            resolved = Some(candidate.clone());
                            break;
                        }
                        let prefix_matches: Vec<_> = procs
                            .iter()
                            .filter(|p| p.id.starts_with(candidate.as_str()))
                            .collect();
                        if prefix_matches.len() == 1 {
                            resolved = Some(prefix_matches[0].id.clone());
                            break;
                        }
                    }
                }

                match resolved {
                    Some(new_name) if new_name != name => {
                        tracing::debug!(
                            "overriding proc stop target as there is a single match: {} --> {}",
                            name,
                            new_name
                        );
                        name = new_name;
                    }
                    Some(_) => { /* exact match on original name, nothing to override */ }
                    None => {
                        let tried = candidates.join("' or '");
                        tracing::debug!(
                            "The /STOP command failed because there is no process matching '{tried}' exactly or as a prefix.",
                        );
                        return Ok(Some(Arc::new(ResolvedBackend::Target(Target::Respond {
                            status: 500,
                            body: Some(format!("The /STOP command failed because there is no process matching '{tried}' exactly or as a prefix.").into_bytes()),
                            content_type: Some("text/plain".into()),
                        }))));
                    }
                }
            }

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

    runtime.register_proxy_config_mutator(Arc::new(move |cfg| {
        cfg.dynamic_backend_resolvers
            .insert(DynamicBackendId::from("odd-box"), resolver.clone());
        inject_local_stop_routes(cfg, &source_listeners.load());
    }));
}

fn inject_local_stop_routes(
    cfg: &mut cruma::cruma_proxy_lib::types::Configuration,
    _source_listeners: &[cruma::config::ListenerDefinition],
) {
    const STOP_ROUTE_NAME: &str = "odd-box-stop-hook";

    let stop_route = || cruma::cruma_proxy_lib::types::HttpRoute {
        name: STOP_ROUTE_NAME.to_string(),
        priority: 10_000,
        filter: cruma::cruma_proxy_lib::types::HttpMatch::HostAndPath {
            hosts: vec![
                cruma::cruma_proxy_lib::types::HostPattern::Exact {
                    value: "localhost".to_string(),
                },
                cruma::cruma_proxy_lib::types::HostPattern::Ip {
                    addr: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                },
                cruma::cruma_proxy_lib::types::HostPattern::Ip {
                    addr: std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
                },
            ],
            prefixes: vec!["/STOP".to_string()],
        },
        middlewares: Vec::new(),
        target: cruma::cruma_proxy_lib::types::Target::DynamicBackend {
            resolver: cruma::cruma_proxy_lib::types::DynamicBackendId::from("odd-box"),
        },
    };

    for built_listener in &mut cfg.listeners {
        if let cruma::cruma_proxy_lib::types::Listener::Http(http_listener) = built_listener {
            http_listener
                .routes
                .0
                .retain(|route| route.name != STOP_ROUTE_NAME);
            http_listener.routes.0.insert(0, stop_route());
        }
    }
}

fn parse_cli() -> Result<TunnelCli> {
    let args: Vec<_> = std::env::args_os().collect();
    let rewritten_args = cruma::config::try_rewrite_bare_config_path(&args);
    let args_to_parse = rewritten_args.as_deref().unwrap_or(&args);

    reject_unsupported_cli_surface(args_to_parse)?;

    let cmd = TunnelCli::command()
        .name(NAME)
        .bin_name(NAME)
        .about("A simple reverse proxy and process manager")
        .long_about(
            "odd-box is a lightweight reverse proxy that manages backends, \
             frontends, hosted processes, and TLS certificates.\n\n\
             It delegates all heavy lifting to the cruma agent library.",
        )
        .mut_arg("generate_completions", |arg| arg.hide(true))
        .mut_subcommands(|sub| {
            if sub.get_name() == "config" {
                sub.hide(true)
            } else {
                sub
            }
        });
    let matches = match cmd.try_get_matches_from(args_to_parse) {
        Ok(matches) => matches,
        Err(err) => {
            err.print()?;
            std::process::exit(err.exit_code());
        }
    };
    let mut cli = TunnelCli::from_arg_matches(&matches)?;

    if cli.command.is_none() {
        if let Some(config_path) = cli.config.take() {
            cli.command = Some(TunnelCliCmd::Start(StartCmd {
                path: Some(config_path),
            }));
        } else {
            cli.command = Some(TunnelCliCmd::Start(StartCmd { path: None }));
        }
    }

    Ok(cli)
}

fn reject_unsupported_cli_surface(args: &[std::ffi::OsString]) -> Result<()> {
    let raw: Vec<&str> = args.iter().filter_map(|arg| arg.to_str()).collect();

    if raw.iter().any(|arg| *arg == "--generate-completions") {
        bail!("`--generate-completions` is not supported by odd-box.");
    }

    if raw.get(1) == Some(&"config")
        || (raw.get(1) == Some(&"help") && raw.get(2) == Some(&"config"))
    {
        bail!("`odd-box config ...` is not supported.");
    }

    Ok(())
}
fn command_is_gui_incompatible(cli: &TunnelCli) -> bool {
    matches!(
        &cli.command,
        Some(TunnelCliCmd::Proxy(_)) | Some(TunnelCliCmd::Serve(_))
    )
}

fn should_launch_gui(cli: &TunnelCli) -> bool {
    if cli.headless {
        return false;
    }
    if cli.tui || command_is_gui_incompatible(cli) {
        return false;
    }
    !is_headless_environment()
}

fn cli_protocol(cli: &TunnelCli) -> cruma::config::Protocol {
    match &cli.command {
        Some(TunnelCliCmd::Proxy(ProxyCmd { protocol, .. })) => protocol.clone(),
        Some(TunnelCliCmd::Serve(serve_args)) => serve_args.protocol.clone(),
        _ => cruma::config::Protocol::Auto,
    }
}

fn load_runtime_config(
    cli: &TunnelCli,
    want_gui: bool,
) -> Result<(
    TunnelCliConfiguration,
    String,
    Option<String>,
    profiles::ProfilesConfig,
)> {
    match &cli.command {
        Some(TunnelCliCmd::Start(start_cmd)) => load_start_config(start_cmd, want_gui),
        Some(TunnelCliCmd::Proxy(_)) | Some(TunnelCliCmd::Serve(_)) => {
            let config = TunnelCliConfiguration::new(cli)?;
            let config_path = config
                .config_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok((config, config_path, None, profiles::load_profiles()))
        }
        _ => bail!("no runtime command provided"),
    }
}

fn load_start_config(
    start_cmd: &StartCmd,
    want_gui: bool,
) -> Result<(
    TunnelCliConfiguration,
    String,
    Option<String>,
    profiles::ProfilesConfig,
)> {
    let explicit_config = start_cmd
        .path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let explicit = explicit_config.is_some();

    let mut saved_profiles = profiles::load_profiles();
    let mut initial_gui_page: Option<String> = None;
    let resolved_config_path: Option<String>;

    if let Some(explicit_path) = explicit_config {
        resolved_config_path = Some(explicit_path.clone());

        let path_obj = std::path::Path::new(&explicit_path);
        if !profiles::path_already_registered(path_obj, &saved_profiles.profiles) {
            let name = profiles::derive_profile_name(path_obj, &saved_profiles.profiles);
            saved_profiles.profiles.push(profiles::ProfileEntry {
                name,
                path: path_obj.to_path_buf(),
            });
            if let Err(e) = profiles::save_profiles(&saved_profiles) {
                tracing::warn!("Could not save profiles.toml: {e}");
            }
        }
    } else if saved_profiles.ask_on_startup {
        if want_gui {
            resolved_config_path = None;
            initial_gui_page = Some("profiles".to_string());
        } else {
            resolved_config_path = Some(terminal_profile_picker(&saved_profiles)?);
        }
    } else if let Some(default_name) = &saved_profiles.default_profile.clone() {
        match saved_profiles
            .profiles
            .iter()
            .find(|e| &e.name == default_name)
        {
            Some(entry) => {
                resolved_config_path = Some(entry.path.to_string_lossy().into_owned());
            }
            None => {
                tracing::warn!(
                    "Default profile '{default_name}' not found in profiles.toml; \
                     falling back to auto-discovery."
                );
                let path = find_or_create_default_config()?;
                ensure_main_profile_registered(&mut saved_profiles, &path);
                resolved_config_path = Some(path);
            }
        }
    } else {
        let path = find_or_create_default_config()?;
        ensure_main_profile_registered(&mut saved_profiles, &path);
        resolved_config_path = Some(path);
    }

    let (config, config_path_for_runtime) = if let Some(config_path) = resolved_config_path {
        let result = match load_config_from_path(std::path::Path::new(&config_path)) {
            Ok(cfg) => Ok((cfg, config_path.clone())),
            Err(_load_err)
                if migrate::looks_like_legacy_toml(std::path::Path::new(&config_path)) =>
            {
                let (cfg, new_path) = migrate::auto_migrate(&config_path)?;
                let new_path_str = new_path.to_string_lossy().into_owned();
                Ok((cfg, new_path_str))
            }
            Err(load_err) if explicit => {
                Err(load_err.context(format!("failed to load config from '{config_path}'")))
            }
            Err(load_err) => Err(load_err.into()),
        };
        result?
    } else {
        let stub: TunnelCliConfiguration =
            toml::from_str("backends = []\nfrontends = []").expect("valid minimal config");
        (stub, String::new())
    };

    Ok((
        config,
        config_path_for_runtime,
        initial_gui_page,
        saved_profiles,
    ))
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

#[allow(dead_code)]
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

/// Ensure a `"main"` profile entry exists for `config_path` and save
/// `profiles.toml` if anything changed.
fn ensure_main_profile_registered(saved: &mut profiles::ProfilesConfig, config_path: &str) {
    let path_obj = std::path::Path::new(config_path);
    if !profiles::path_already_registered(path_obj, &saved.profiles) {
        // Check if we already have a profile named "main"; if so derive a name.
        let name = if saved.profiles.iter().any(|e| e.name == "main") {
            profiles::derive_profile_name(path_obj, &saved.profiles)
        } else {
            "main".to_string()
        };
        saved.profiles.push(profiles::ProfileEntry {
            name,
            path: path_obj.to_path_buf(),
        });
        if let Err(e) = profiles::save_profiles(saved) {
            tracing::warn!("Could not persist profiles.toml: {e}");
        }
    }
}

/// Interactive terminal profile picker for TUI / headless `ask_on_startup`.
///
/// Prints a numbered list and reads a line from stdin.  Empty input
/// selects the default profile (if configured).  Returns the chosen
/// profile path as a string.
fn terminal_profile_picker(saved: &profiles::ProfilesConfig) -> Result<String> {
    use std::io::{BufRead, Write};

    if saved.profiles.is_empty() {
        anyhow::bail!(
            "ask_on_startup is enabled but no profiles are configured in profiles.toml.\n\
             Run with --config <path> to register a profile, or edit profiles.toml manually."
        );
    }

    let default_name = saved.default_profile.as_deref();

    eprintln!("\nSelect a profile:");
    for (i, entry) in saved.profiles.iter().enumerate() {
        let marker = if default_name == Some(&entry.name) {
            " [default]"
        } else {
            ""
        };
        eprintln!(
            "  {}. {}  {}{}",
            i + 1,
            entry.name,
            entry.path.display(),
            marker
        );
    }

    let default_idx = default_name
        .and_then(|name| saved.profiles.iter().position(|e| e.name == name))
        .map(|i| i + 1);

    let prompt = match default_idx {
        Some(d) => format!("Enter number (or press Enter for default) [{}]: ", d),
        None => "Enter number: ".to_string(),
    };

    eprint!("{prompt}");
    std::io::stderr().flush().ok();

    let stdin = std::io::stdin();
    let line = stdin
        .lock()
        .lines()
        .next()
        .transpose()
        .unwrap_or(None)
        .unwrap_or_default();
    let line = line.trim().to_string();

    let chosen_index: usize = if line.is_empty() {
        match default_idx {
            Some(d) => d - 1,
            None => {
                anyhow::bail!("No default profile configured; please enter a number.");
            }
        }
    } else {
        let n: usize = line
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid selection: '{line}'"))?;
        if n == 0 || n > saved.profiles.len() {
            anyhow::bail!("Selection {n} out of range (1–{})", saved.profiles.len());
        }
        n - 1
    };

    Ok(saved.profiles[chosen_index]
        .path
        .to_string_lossy()
        .into_owned())
}
