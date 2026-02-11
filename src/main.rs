mod configuration;
mod cruma;
pub mod cruma_integration;
mod gui;
mod http_events;
mod logging;
pub mod process_registry;
mod tui;
mod types;
use anyhow::Context;
use anyhow::bail;
use arc_swap::ArcSwap;
use clap::Parser;
use configuration::OddBoxConfigVersion;
use configuration::{ConfigWrapper, LogLevel};
use core::fmt;
use global_state::GlobalState;
use notify::RecommendedWatcher;
use notify::Watcher;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use types::args::Args;
mod proc_host;
use tracing_subscriber::util::SubscriberInitExt;
mod self_update;
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicBool as StdAtomicBool, Ordering as StdOrdering};

mod docker;

pub mod global_state {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64},
        },
        time::SystemTimeError,
    };

    #[derive(Debug, PartialEq, Clone)]
    pub enum ProcState {
        Faulty,
        Stopped,
        Starting,
        Stopping,
        Running,
        Remote,
        DirServer,
        Docker,
    }

    #[derive(Debug, Clone)]
    pub struct CrumaAssignedDomain {
        pub assigned_domain: String,
        pub welcome_message: String,
    }

    #[derive(Debug)]
    pub struct GlobalState {
        pub enable_global_traffic_inspection: AtomicBool,
        pub exit: AtomicBool,
        pub process_registry: Arc<crate::process_registry::ProcessRegistry>,
        pub cruma_assignment: Arc<arc_swap::ArcSwapOption<CrumaAssignedDomain>>,
        pub cruma_transports:
            Arc<arc_swap::ArcSwap<Vec<cruma_tunnels_lib::TransportDescriptor>>>,

        pub started_at_time_stamp: std::time::SystemTime,
        pub log_handle: crate::OddLogHandle,
        pub config: std::sync::Arc<arc_swap::ArcSwap<crate::configuration::ConfigWrapper>>,
        pub target_request_counts: dashmap::DashMap<String, AtomicU64>,
        pub cruma_config: std::sync::Arc<arc_swap::ArcSwap<cruma_proxy_lib::types::Configuration>>,
        pub docker_discovery: std::sync::Arc<arc_swap::ArcSwap<Vec<crate::docker::DiscoveredContainer>>>,
        pub tui_log_buffer: std::sync::Arc<crate::logging::SharedLogBuffer>,
        pub tokio_handle: tokio::runtime::Handle,
    }
    impl GlobalState {
        pub fn uptime(&self) -> Result<std::time::Duration, SystemTimeError> {
            self.started_at_time_stamp.elapsed()
        }
        pub fn new(
            config: std::sync::Arc<arc_swap::ArcSwap<crate::configuration::ConfigWrapper>>,
            cruma_config: std::sync::Arc<arc_swap::ArcSwap<cruma_proxy_lib::types::Configuration>>,
            log_handle: crate::OddLogHandle,
            tui_log_buffer: std::sync::Arc<crate::logging::SharedLogBuffer>,
            tokio_handle: tokio::runtime::Handle,
        ) -> Self {
            Self {
                enable_global_traffic_inspection: AtomicBool::new(false),
                process_registry: Arc::new(crate::process_registry::ProcessRegistry::new()),
                exit: AtomicBool::new(false),
                cruma_assignment: Arc::new(arc_swap::ArcSwapOption::from(None)),
                cruma_transports: Arc::new(arc_swap::ArcSwap::from_pointee(Vec::new())),

                started_at_time_stamp: std::time::SystemTime::now(),
                log_handle,
                config,
                target_request_counts: dashmap::DashMap::new(),
                cruma_config,
                docker_discovery: Arc::new(arc_swap::ArcSwap::from_pointee(Vec::new())),
                tui_log_buffer,
                tokio_handle,
            }
        }

        /// Legacy proxy lookup removed. Placeholder to keep API surface.
        pub async fn try_find_site(
            &self,
            _pre_filter_hostname: &str,
        ) -> Option<std::sync::Arc<()>> {
            None
        }
    }
}

fn async_watcher() -> notify::Result<(
    RecommendedWatcher,
    std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
)> {
    let (tx, rx) = std::sync::mpsc::channel();

    let watcher = <RecommendedWatcher as notify::Watcher>::new(
        move |res| {
            tx.send(res).unwrap();
        },
        notify::Config::default(),
    )?;

    Ok((watcher, rx))
}

lazy_static! {
    static ref RELOADING_CONFIGURATION: tokio::sync::Semaphore = tokio::sync::Semaphore::new(1);
}

async fn config_file_monitor(
    config: Arc<arc_swap::ArcSwap<ConfigWrapper>>,
    global_state: Arc<GlobalState>,
) -> anyhow::Result<()> {
    let cfg_path = config
        .load_full()
        .path
        .clone()
        .expect("odd-box must be using a configuration file.");

    let (mut watcher, rx) = async_watcher()?;

    watcher.watch(Path::new(&cfg_path), notify::RecursiveMode::Recursive)?;

    loop {
        let exit_requested_clone = &global_state.exit;

        if exit_requested_clone.load(Ordering::Relaxed) {
            break;
        }

        match rx.try_recv() {
            Ok(Err(e)) => {
                tracing::warn!("Error while watching config file: {e:?}");
            }
            Ok(Ok(e)) => {
                if RELOADING_CONFIGURATION.available_permits() == 0 {
                    continue;
                }

                let _permit = RELOADING_CONFIGURATION.acquire().await.unwrap();

                match e.kind {
                    notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                        match crate::configuration::reload::reload_from_disk(global_state.clone())
                            .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                tracing::error!("Failed to reload configuration file: {e:?}");
                            }
                        }
                    }
                    notify::EventKind::Remove(_remove_kind) => {
                        tracing::error!(
                            "Configuration file was removed. This is not supported. Please restart odd-box."
                        );
                    }
                    _ => {}
                }
            }
            Err(e) => match e {
                std::sync::mpsc::TryRecvError::Empty => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                std::sync::mpsc::TryRecvError::Disconnected => {
                    tracing::error!(
                        "Config file watcher channel disconnected. This is a bug in odd-box."
                    );
                    break;
                }
            },
        }
    }

    Ok(())
}

fn generate_config(
    file_name: Option<&str>,
    fill_example: bool,
) -> anyhow::Result<crate::configuration::OddBoxV4Config> {
    let current_working_dir = std::env::current_dir()?;
    if let Some(file_name) = file_name {
        let file_path = current_working_dir.join(file_name);
        if std::path::Path::exists(std::path::Path::new(file_name)) {
            return Err(anyhow::anyhow!(format!(
                "File already exists: {file_path:?}"
            )));
        }
    }
    if !fill_example {
        // Load the YAML init config
        let mut init_cfg = include_str!("./init-cfg.yaml").to_string();

        if cfg!(target_os = "macos") {
            // mac os allows for binding to lower ports without root, so we can use the default ports.
            init_cfg = init_cfg
                .replace("ip: 127.0.0.1", "ip: 0.0.0.0")
                .replace("port: 4343", "port: 443")
                .replace("port: 8080", "port: 80");
        }

        let cfg = configuration::v4::OddBoxV4Config::parse_yaml(&init_cfg)
            .map_err(|e| anyhow::anyhow!(format!("Failed to parse initial configuration: {e}")))?;

        if let Some(file_name) = file_name {
            let file_path = current_working_dir.join(file_name);
            std::fs::write(&file_path, init_cfg)?;
            tracing::info!("Configuration file written to {file_path:?}");
        }
        return Ok(cfg);
    }

    let cfg = crate::configuration::OddBoxV4Config::example();
    if let Some(file_name) = file_name {
        let serialized = cfg
            .to_yaml()
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {e}"))?;
        let file_path = current_working_dir.join(file_name);
        std::fs::write(&file_path, serialized)?;
        tracing::info!("Configuration file written to {file_path:?}");
    }
    Ok(cfg)
}

// (validated_cfg, original_version)
fn initialize_configuration(
    args: &Args,
) -> anyhow::Result<(ConfigWrapper, OddBoxConfigVersion, bool)> {
    let cfg_path = if let Some(cfg) = &args.configuration {
        cfg.to_string()
    } else {
        // Prefer YAML files (V4+), fall back to TOML (V3 and earlier)
        if std::fs::metadata("odd-box.yaml").is_ok() {
            "odd-box.yaml".to_owned()
        } else if std::fs::metadata("oddbox.yaml").is_ok() {
            "oddbox.yaml".to_owned()
        } else if std::fs::metadata("odd-box.toml").is_ok() {
            "odd-box.toml".to_owned()
        } else if std::fs::metadata("oddbox.toml").is_ok() {
            "oddbox.toml".to_owned()
        } else {
            "Config.toml".to_owned()
        }
    };

    let mut file = std::fs::File::open(&cfg_path)
        .with_context(|| format!("failed to open configuration file {cfg_path:?}"))
        .with_context(|| format!("failed to open configuration file {cfg_path:?}"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .with_context(|| format!("failed to read data from configuration file {cfg_path:?}"))?;

    let (config, original_version, was_upgraded) =
        match configuration::AnyOddBoxConfig::parse(&contents) {
            Ok(configuration) => {
                let (a, b, c) = configuration
                    .try_upgrade_to_latest_version()
                    .expect("configuration upgrade failed. this is a bug in odd-box");
                (ConfigWrapper::new(a, Some(cfg_path.clone())), b, c)
            }
            Err(e) => anyhow::bail!(e),
        };

    config.is_valid()?;

    Ok((config, original_version, was_upgraded))
}

#[tokio::main(flavor = "multi_thread")]
#[tracing::instrument()]
async fn main() -> anyhow::Result<()> {
    match rustls::crypto::ring::default_provider().install_default() {
        Ok(_) => {}
        Err(e) => {
            bail!("Failed to install default ring provider: {:?}", e)
        }
    }

    let args = Args::parse();

    if args.config_schema {
        let schema = schemars::schema_for!(crate::configuration::OddBoxV4Config);
        println!(
            "{}",
            serde_json::to_string_pretty(&schema).expect("schema should be serializable")
        );
        return Ok(());
    }

    if args.update {
        _ = self_update::update().await;
        return Ok(());
    }

    let gui_flag = args.gui;
    let tui_flag = if gui_flag { false } else { args.tui };

    if args.init {
        generate_config(Some("odd-box.yaml"), false)?;
        return Ok(());
    }

    let (mut config, _original_version, was_upgraded) = initialize_configuration(&args)?;

    if was_upgraded {
        println!("Detected outdated configuration file - updating to latest version (YAML)");
        let original_path = config
            .path
            .clone()
            .expect("original configuration file should exist");

        // Backup the old config file
        let mut i = 1;
        let mut backup_path = format!("{original_path}.backup{i}");
        while std::fs::exists(&backup_path).is_ok_and(|x| x == true) {
            i += 1;
            backup_path = format!("{original_path}.backup{i}");
        }
        std::fs::copy(&original_path, &backup_path)?;
        println!("  Backed up old config to: {backup_path}");

        // Change the config path to .yaml if it was .toml
        let yaml_path = if original_path.ends_with(".toml") {
            original_path.replace(".toml", ".yaml")
        } else {
            format!("{}.yaml", original_path)
        };

        // Update the config's internal path to the new YAML path
        config.set_disk_path(&yaml_path)?;
        config.write_to_disk()?;
        println!("  Saved new V4 config to: {yaml_path}");
    }

    // Clone backends for process spawning
    let cloned_backends = config.backends.clone();

    let log_level: LevelFilter = match config.log_level {
        LogLevel::Info => LevelFilter::INFO,
        LogLevel::Error => LevelFilter::ERROR,
        LogLevel::Warn => LevelFilter::WARN,
        LogLevel::Trace => LevelFilter::TRACE,
        LogLevel::Debug => LevelFilter::DEBUG,
    };

    let shutdown_signal = Arc::new(tokio::sync::Notify::new());
    let shared_config = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(config));

    let cruma_cfg_init = {
        let cfg_guard = shared_config.load_full();
        let (cfg, notes) = cruma_integration::build_config_with_runtime_ports(
            &cfg_guard,
            &std::collections::HashMap::new(),
            &std::collections::HashMap::new(),
        )?;

        if !notes.unsupported.is_empty() {
            tracing::warn!(
                "cruma config placeholders/unsupported: {:?}",
                notes.unsupported
            );
        }
        cfg
    };
    let cruma_config_arc = std::sync::Arc::new(ArcSwap::from_pointee(cruma_cfg_init));
    let tui_log_buffer = std::sync::Arc::new(crate::logging::SharedLogBuffer::new());

    let mut global_state = crate::global_state::GlobalState::new(
        shared_config.clone(),
        cruma_config_arc.clone(),
        OddLogHandle::None,
        tui_log_buffer.clone(),
        tokio::runtime::Handle::current(),
    );

    let gui_log_state = if gui_flag {
        Some(gui::logs::create_shared(1000))
    } else {
        None
    };

    let rust_log = std::env::var("RUST_LOG").ok();
    let has_odd_box_override = rust_log
        .as_ref()
        .map(|v| v.split(',').any(|d| d.trim().starts_with("odd_box")))
        .unwrap_or(false);
    let has_cruma_override = rust_log
        .as_ref()
        .map(|v| v.split(',').any(|d| d.trim().starts_with("odd_box::cruma")))
        .unwrap_or(false);

    let mut cli_filter = EnvFilter::from_default_env();
    if !has_odd_box_override {
        cli_filter = cli_filter.add_directive(
            format!("odd_box={}", log_level)
                .parse()
                .expect("This directive should always work"),
        );
    }
    cli_filter = cli_filter
        .add_directive(
            "odd_box::proc_host=trace"
                .parse()
                .expect("This directive should always work"),
        )
        .add_directive(
            "odd_box::observer=warn"
                .parse()
                .expect("This directive should always work"),
        )
        .add_directive(
            "quinn_proto=warn"
                .parse()
                .expect("This directive should always work"),
        )
        .add_directive(
            "hyper_util=warn"
                .parse()
                .expect("This directive should always work"),
        )
        .add_directive(
            "h2=warn"
                .parse()
                .expect("This directive should always work"),
        );
    if !has_odd_box_override && !has_cruma_override {
        cli_filter = cli_filter.add_directive(
            "odd_box::cruma=info"
                .parse()
                .expect("This directive should always work"),
        );
    }

    let (cli_filter, cli_reload_handle) = tracing_subscriber::reload::Layer::new(cli_filter);

    if tui_flag {
        let mut tui_filter = EnvFilter::from_default_env();
        if !has_odd_box_override {
            tui_filter = tui_filter.add_directive(
                format!("odd_box={}", log_level)
                    .parse()
                    .expect("This directive should always work"),
            );
        }
        tui_filter = tui_filter.add_directive(
            "odd_box::proc_host=trace"
                .parse()
                .expect("This directive should always work"),
        );
        if !has_odd_box_override && !has_cruma_override {
            tui_filter = tui_filter.add_directive(
                "odd_box::cruma=info"
                    .parse()
                    .expect("This directive should always work"),
            );
        }

        let (tui_filter, tui_reload_handle) = tracing_subscriber::reload::Layer::new(tui_filter);

        global_state.log_handle = OddLogHandle::TUI(RwLock::new(tui_reload_handle));
        let tui_layer = crate::logging::TuiLoggerLayer {
            log_buffer: tui_log_buffer.clone(),
        };
        tracing_subscriber::registry()
            .with(tui_filter)
            .with(tui_layer)
            .init();
    } else {
        global_state.log_handle = OddLogHandle::CLI(RwLock::new(cli_reload_handle));
        let fmt_layer = tracing_subscriber::fmt::layer()
            .compact()
            .with_thread_names(true)
            .with_timer(tracing_subscriber::fmt::time::OffsetTime::new(
                time::UtcOffset::from_whole_seconds(
                    chrono::Local::now().offset().local_minus_utc(),
                )
                .expect("time... works"),
                time::macros::format_description!("[hour]:[minute]:[second]"),
            ))
            .boxed();

        if let Some(log_state) = &gui_log_state {
            tracing_subscriber::registry()
                .with(fmt_layer)
                .with(cli_filter)
                .with(gui::logs::GuiLoggerLayer::new(log_state.clone()))
                .init();
        } else {
            tracing_subscriber::registry()
                .with(fmt_layer)
                .with(cli_filter)
                .init();
        }
    }

    let global_state = Arc::new(global_state);

    http_events::install_http_event_sink(global_state.clone());

    let cruma_mode = {
        let cfg_guard = shared_config.load_full();
        cfg_guard.cruma.as_ref().and_then(|c| c.mode())
    };

    if let Some(cfg) = shared_config.load_full().cruma.as_ref() {
        if cfg.mode().is_none() {
            tracing::warn!("cruma config present but invalid; tunnel agent will stay disabled");
        }
    }

    let cfg_monitor = tokio::spawn(config_file_monitor(
        shared_config.clone(),
        global_state.clone(),
    ));

    let mut tui_task: Option<JoinHandle<()>> = None;

    // Capture ctrl-c to shut down cleanly.
    let cstate = global_state.clone();
    ctrlc::set_handler(move || {
        if !CTRL_C_TRIPPED.swap(true, StdOrdering::SeqCst) {
            tracing::warn!("Ctrl-C received. Shutting down..");
        } else {
            tracing::debug!("Ctrl-C received again; shutdown already in progress.");
        }
        cstate.exit.store(true, std::sync::atomic::Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Before starting the proxy thread(s) we need to initialize the tracing system and the tui if enabled.
    if tui_flag {
        tui::init();
        tui_task = Some(tokio::spawn(tui::run(
            global_state.clone(),
            args.theme.clone(),
        )));
    }

    match &cruma_mode {
        None => tracing::info!("Cruma mode: disabled"),
        Some(crate::configuration::v4::CrumaMode::Anonymous) => {
            tracing::info!("Cruma mode: anonymous")
        }
        Some(crate::configuration::v4::CrumaMode::Authenticated { .. }) => {
            tracing::info!("Cruma mode: authenticated")
        }
    }

    // Start cruma-based hosting (primary path). Port offset can be used to avoid clashes when legacy listeners are still around.
    let cruma_task = {
        let shutdown_for_cruma = shutdown_signal.clone();
        let state_for_cruma = global_state.clone();
        tokio::spawn(async move {
            let cruma_cfg_arc = state_for_cruma.cruma_config.clone();

            let persistence = match cruma_proxy_lib::termination::LocalDiskPersistence::new(
                &".odd-box-cruma-cache".into(),
            ) {
                Ok(p) => std::sync::Arc::new(p),
                Err(e) => {
                    tracing::error!(error=%e, "Failed to initialize cruma persistence");
                    return;
                }
            };

            let cancel = CancellationToken::new();
            let cancel_on_shutdown = cancel.clone();
            let shutdown_notifier = shutdown_for_cruma.clone();
            tokio::spawn(async move {
                shutdown_notifier.notified().await;
                cancel_on_shutdown.cancel();
            });

            let ct_clone = cancel.clone();
            if let Err(e) =
                cruma_proxy_lib::hosting::run_from_config(cruma_cfg_arc, persistence, ct_clone)
                    .await
            {
                tracing::error!(error=%e, "cruma hosting failed");
            }
        })
    };

    let cruma_agent_supervisor = tokio::spawn(cruma_agent_supervisor(
        shutdown_signal.clone(),
        global_state.clone(),
    ));

    let config_guard = global_state.config.load_full();

    // Add backends to the process registry based on their type
    for (backend_id, backend) in &cloned_backends {
        match backend {
            configuration::v4::Backend::Process(proc) => {
                // Resolve and spawn process host
                match config_guard.resolve_process_backend(backend_id, proc) {
                    Ok(resolved) => {
                        // Create token externally and register before spawning
                        let token = CancellationToken::new();
                        let enabled = resolved.auto_start.unwrap_or(config_guard.auto_start);
                        global_state.process_registry.register_host(
                            backend_id.clone(),
                            token.clone(),
                            crate::global_state::ProcState::Stopped,
                            enabled,
                            resolved.port,
                        );
                        tokio::task::spawn(proc_host::host(
                            resolved,
                            global_state.process_registry.clone(),
                            global_state.clone(),
                            token,
                        ));
                    }
                    Err(e) => bail!(
                        "Failed to resolve process configuration for backend '{}':\n{:?}",
                        backend_id,
                        e
                    ),
                }
            }
            configuration::v4::Backend::Remote(_) => {
                global_state
                    .process_registry
                    .register_backend(backend_id.clone(), crate::global_state::ProcState::Remote);
            }
            configuration::v4::Backend::Static(_) => {
                global_state.process_registry.register_backend(
                    backend_id.clone(),
                    crate::global_state::ProcState::DirServer,
                );
            }
        }
    }

    drop(config_guard);
    crate::cruma_integration::rebuild_cruma_config(&global_state);

    tokio::task::spawn(docker_thread(global_state.clone()));

    // if in tui mode, we can just hang around until the tui thread exits.
    if let Some(tt) = tui_task {
        _ = tt.await;
    // if in gui mode, run the iced application (blocks on main thread)
    } else if gui_flag {
        tracing::info!("odd-box started successfully. launching GUI...");
        // Determine theme mode from args
        let theme_mode = args
            .theme
            .as_deref()
            .map(gui::ThemeMode::from_str)
            .unwrap_or(gui::ThemeMode::System);
        let Some(log_state) = gui_log_state.clone() else {
            tracing::error!("GUI logging state was not initialized");
            return Ok(());
        };
        // Spawn background task for log filtering (runs expensive filter ops off GUI thread)
        let _filter_task = gui::logs::spawn_filter_task(log_state.clone());
        // Run GUI on main thread - this blocks until window is closed
        if let Err(e) = gui::run(global_state.clone(), theme_mode, log_state) {
            tracing::error!("GUI error: {:?}", e);
        }
        // Signal exit when GUI closes
        global_state.exit.store(true, Ordering::SeqCst);
    // otherwise we will wait for the exit signal set by ctrl-c
    } else {
        tracing::info!("odd-box started successfully. use ctrl-c to quit.");
        while global_state.exit.load(Ordering::Relaxed) == false {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    // ^ Note that after this point when the application has been running in TUI mode, we can no longer use tracing as the subscriber
    //   writes to the TUI buffers, and so we must now use println from here on out.
    if tui_flag || gui_flag {
        println!("odd-box is shutting down.. waiting for processes to stop..");
    } else {
        tracing::warn!("odd-box is shutting down.. waiting for processes to stop..");
    }

    // Mark all proc_hosts for removal and wait for them to exit
    global_state.process_registry.mark_all_for_removal();

    // Wait for all proc_hosts to exit (tokens to be cancelled)
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        global_state.process_registry.cleanup_finished();
        let snapshot = global_state.process_registry.snapshot();
        let remaining: Vec<_> = snapshot
            .entries
            .iter()
            .filter(|e| e.is_marked_for_removal() && !e.is_cancelled())
            .collect();
        if remaining.is_empty() {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            if tui_flag || gui_flag {
                println!("Timeout waiting for {} processes to stop", remaining.len());
            } else {
                tracing::warn!("Timeout waiting for {} processes to stop", remaining.len());
            }
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if tui_flag || gui_flag {
        println!("shutdown sequence for hosted processes completed successfully");
        println!("stopping proxy services..");
    } else {
        tracing::info!("shutdown for hosted processes sequence completed successfully");
        tracing::info!("stopping proxy services..");
    }

    shutdown_signal.notify_waiters();

    _ = cruma_task.abort();
    _ = cruma_task.await;
    _ = cruma_agent_supervisor.abort();
    _ = cruma_agent_supervisor.await;
    _ = cfg_monitor.abort();

    if tui_flag || gui_flag {
        println!("odd-box exited successfully");
    } else {
        tracing::info!("odd-box exited successfully");
    }

    Ok(())
}

async fn cruma_agent_supervisor(
    shutdown: Arc<tokio::sync::Notify>,
    state: Arc<crate::global_state::GlobalState>,
) {
    use crate::configuration::v4::CrumaMode;

    struct RunningAgent {
        mode: CrumaMode,
        shutdown: Arc<tokio::sync::Notify>,
        handle: tokio::task::JoinHandle<anyhow::Result<()>>,
    }

    async fn stop_agent(agent: Option<RunningAgent>) {
        if let Some(agent) = agent {
            agent.shutdown.notify_waiters();
            match agent.handle.await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    tracing::error!(error=%err, "cruma agent stopped with error");
                }
                Err(err) => {
                    tracing::error!(error=%err, "cruma agent task join failed");
                }
            }
        }
    }

    fn spawn_agent(mode: CrumaMode, state: Arc<crate::global_state::GlobalState>) -> RunningAgent {
        let notify = Arc::new(tokio::sync::Notify::new());
        let cfg_arc = state.cruma_config.clone();
        let creds = match &mode {
            CrumaMode::Anonymous => cruma_tunnels_lib::AgentCredentials::anonymous(),
            CrumaMode::Authenticated { id, key } => {
                cruma_tunnels_lib::AgentCredentials::new(id, key)
            }
        };
        let handle = tokio::spawn(crate::cruma::cruma_thread(
            notify.clone(),
            state,
            cfg_arc,
            creds,
        ));
        RunningAgent {
            mode,
            shutdown: notify,
            handle,
        }
    }

    let mut running: Option<RunningAgent> = None;
    let mut last_mode: Option<CrumaMode> = None;

    loop {
        tokio::select! {
            _ = shutdown.notified() => {
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {}
        }

        let mode = state
            .config
            .load_full()
            .cruma
            .as_ref()
            .and_then(|c| c.mode());
        if mode != last_mode {
            stop_agent(running.take()).await;
            if let Some(next) = mode.clone() {
                running = Some(spawn_agent(next, state.clone()));
            }
            last_mode = mode;
        }
    }

    stop_agent(running).await;
}

type CliLogHandle = tracing_subscriber::reload::Handle<
    EnvFilter,
    tracing_subscriber::layer::Layered<
        Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync>,
        tracing_subscriber::Registry,
    >,
>;
type TuiLogHandle = tracing_subscriber::reload::Handle<EnvFilter, tracing_subscriber::Registry>;

pub enum OddLogHandle {
    CLI(RwLock<CliLogHandle>),
    TUI(RwLock<TuiLogHandle>),
    None,
}

impl fmt::Debug for OddLogHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OddLogHandle")
    }
}

// we could probably subscribe to the docker socket instead of having this stupid loop..
// this does however seem to work fine and is rather simple, so keeping it for now :)
fn connect_container_runtimes() -> Vec<(String, bollard::Docker)> {
    let mut clients = Vec::new();
    if let Ok(client) = bollard::Docker::connect_with_local_defaults() {
        clients.push(("default".to_string(), client));
    }

    let mut socket_candidates: Vec<(String, String)> = vec![
        ("docker".to_string(), "/var/run/docker.sock".to_string()),
        ("podman-system".to_string(), "/run/podman/podman.sock".to_string()),
        ("podman-system".to_string(), "/var/run/podman/podman.sock".to_string()),
    ];
    if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        socket_candidates.push((
            "podman-user".to_string(),
            format!("{xdg_runtime_dir}/podman/podman.sock"),
        ));
    }

    let mut seen = std::collections::HashSet::new();
    for (label, socket_path) in socket_candidates {
        if !std::path::Path::new(&socket_path).exists() || !seen.insert(socket_path.clone()) {
            continue;
        }
        let host = format!("unix://{socket_path}");
        match bollard::Docker::connect_with_unix(&host, 120, &bollard::API_DEFAULT_VERSION) {
            Ok(client) => clients.push((label, client)),
            Err(err) => tracing::debug!(
                "container runtime socket {} unavailable: {}",
                socket_path,
                err
            ),
        }
    }

    clients
}

pub async fn docker_thread(state: Arc<GlobalState>) {
    loop {
        let runtime_clients = connect_container_runtimes();
        if !runtime_clients.is_empty() {
            let mut discovered_containers = Vec::new();
            let mut running_container_targets_by_host: std::collections::BTreeMap<
                String,
                crate::docker::ContainerProxyTarget,
            > = std::collections::BTreeMap::new();

            for (runtime, docker) in runtime_clients {
                let mut discovered = docker::discover_containers(&docker).await.unwrap_or_default();
                for item in &mut discovered {
                    item.runtime = runtime.clone();
                }
                discovered_containers.extend(discovered);

                let mut targets = docker::get_container_proxy_targets(&docker)
                    .await
                    .unwrap_or_default();
                for target in &mut targets {
                    target.runtime = runtime.clone();
                }
                for target in targets {
                    let host = target.generate_host_name();
                    match running_container_targets_by_host.entry(host.clone()) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(target);
                        }
                        std::collections::btree_map::Entry::Occupied(_) => {
                            tracing::warn!(
                                "Container host '{}' discovered in multiple runtimes; keeping first and skipping duplicate from runtime '{}'",
                                host,
                                runtime
                            );
                        }
                    }
                }
            }

            state
                .docker_discovery
                .store(std::sync::Arc::new(discovered_containers));

            let running_container_targets_dash_map = dashmap::DashMap::new();
            for (host, target) in running_container_targets_by_host {
                running_container_targets_dash_map.insert(host, target);
            }

            // Mark removed docker containers in the registry
            {
                let snapshot = state.process_registry.snapshot();
                for entry in &snapshot.entries {
                    if entry.proc_state() == crate::global_state::ProcState::Docker
                        && !running_container_targets_dash_map.contains_key(&entry.backend_id)
                    {
                        state.process_registry.mark_for_removal(&entry.backend_id);
                    }
                }
            }
            state.process_registry.cleanup_finished();

            // Register running docker containers
            for guard in &running_container_targets_dash_map {
                let (host_name, _) = guard.pair();
                state.process_registry.register_backend(
                    host_name.to_string(),
                    crate::global_state::ProcState::Docker,
                );
            }
            let mut guard = (*state.config.load_full()).clone();
            guard.docker_containers = running_container_targets_dash_map;

            // Keep cruma config in sync with docker-discovered targets.
            let runtime_ports =
                cruma_integration::runtime_ports_from_registry(&state.process_registry);
            let runtime_states =
                cruma_integration::runtime_states_from_registry(&state.process_registry);
            if let Ok((cfg, notes)) = cruma_integration::build_config_with_runtime_ports(
                &guard,
                &runtime_ports,
                &runtime_states,
            ) {
                if !notes.unsupported.is_empty() {
                    tracing::warn!(
                        "cruma config placeholders/unsupported after docker update: {:?}",
                        notes.unsupported
                    );
                }
                state.cruma_config.store(std::sync::Arc::new(cfg));
            }
            state.config.store(std::sync::Arc::new(guard));
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
static CTRL_C_TRIPPED: StdAtomicBool = StdAtomicBool::new(false);
