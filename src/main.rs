

pub mod cruma_integration;
mod configuration;
mod control;
mod cruma;
mod types;
mod tui;
mod gui;
use anyhow::bail;
use clap::Parser;
use configuration::OddBoxConfigVersion;
use configuration::{ConfigWrapper, LogLevel};
use core::fmt;
use anyhow::Context;
use dashmap::DashMap;
use global_state::GlobalState;
use control::ProcMessage;
use arc_swap::ArcSwap;
use cruma_integration::{build_config as build_cruma_config, apply_port_offset as apply_cruma_port_offset};
use notify::RecommendedWatcher;
use notify::Watcher;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use types::args::Args;
use types::odd_box_event::EventForWebsocketClients;
use types::odd_box_event::GlobalEvent;
use types::proc_info::BgTaskInfo;
use types::proc_info::ProcId;
use types::proc_info::ProcInfo;
mod proc_host;
use tracing_subscriber::util::SubscriberInitExt;
use types::app_state::ProcState;
mod logging;

mod self_update;
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicBool as StdAtomicBool, Ordering as StdOrdering};
use types::app_state::AppState;

mod docker;

lazy_static! {
    static ref PROC_THREAD_MAP: Arc<DashMap<ProcId, ProcInfo>> = Arc::new(DashMap::new());
    static ref BG_WORKER_THREAD_MAP: Arc<DashMap<String, BgTaskInfo>> = Arc::new(DashMap::new());
}

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
pub fn generate_unique_id() -> u64 {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub mod global_state {
    use std::{
        sync::atomic::AtomicU64,
        time::SystemTimeError,
    };

    use crate::{
        types::odd_box_event::{EventForWebsocketClients, GlobalEvent},
    };
    #[derive(Debug)]
    pub struct GlobalState {
        pub started_at_time_stamp: std::time::SystemTime,
        pub log_handle: crate::OddLogHandle,
        pub app_state: std::sync::Arc<crate::types::app_state::AppState>,
        pub config: std::sync::Arc<tokio::sync::RwLock<crate::configuration::ConfigWrapper>>,
        pub proc_broadcaster: tokio::sync::broadcast::Sender<crate::control::ProcMessage>,
        pub target_request_counts: dashmap::DashMap<String, AtomicU64>,
        pub global_broadcast_channel: tokio::sync::broadcast::Sender<GlobalEvent>,
        pub websockets_broadcast_channel: tokio::sync::broadcast::Sender<EventForWebsocketClients>,
        pub cruma_config: std::sync::Arc<arc_swap::ArcSwap<cruma_proxy_lib::types::Configuration>>,
    }
    impl GlobalState {
        pub fn uptime(&self) -> Result<std::time::Duration, SystemTimeError> {
            self.started_at_time_stamp.elapsed()
        }
        pub fn new(
            app_state: std::sync::Arc<crate::types::app_state::AppState>,
            config: std::sync::Arc<tokio::sync::RwLock<crate::configuration::ConfigWrapper>>,
            tx_to_process_hosts: tokio::sync::broadcast::Sender<crate::control::ProcMessage>,
            global_broadcast_channel: tokio::sync::broadcast::Sender<GlobalEvent>,
            websockets_broadcast_channel: tokio::sync::broadcast::Sender<EventForWebsocketClients>,
            cruma_config: std::sync::Arc<arc_swap::ArcSwap<cruma_proxy_lib::types::Configuration>>,
            log_handle: crate::OddLogHandle,
        ) -> Self {
            Self {
                started_at_time_stamp: std::time::SystemTime::now(),
                log_handle,
                app_state,
                config,
                proc_broadcaster: tx_to_process_hosts,
                target_request_counts: dashmap::DashMap::new(),
                global_broadcast_channel,
                websockets_broadcast_channel,
                cruma_config,
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
    config: Arc<RwLock<ConfigWrapper>>,
    global_state: Arc<GlobalState>,
) -> anyhow::Result<()> {
    let guard = config.read().await;
    let cfg_path = guard
        .path
        .clone()
        .expect("odd-box must be using a configuration file.");
    drop(guard);

    let (mut watcher, rx) = async_watcher()?;

    watcher.watch(Path::new(&cfg_path), notify::RecursiveMode::Recursive)?;

    loop {
        let exit_requested_clone = &global_state.app_state.exit;

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
                        tracing::error!("Configuration file was removed. This is not supported. Please restart odd-box.");
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

async fn generic_cleanup_thread(_state: Arc<GlobalState>) {
    let liveness_token: Arc<bool> = Arc::new(true);
    crate::BG_WORKER_THREAD_MAP.insert(
        "The Janitor".into(),
        BgTaskInfo {
            liveness_ptr: Arc::downgrade(&liveness_token),
            status: "Managing active tasks..".into(),
        },
    );

    let mut every_30_seconds_counter = 0;

    loop {
        {
            every_30_seconds_counter += 1;

            PROC_THREAD_MAP.retain(|_k, v| v.liveness_ptr.upgrade().is_some());
            BG_WORKER_THREAD_MAP.retain(|_k, v| v.liveness_ptr.upgrade().is_some());

            if every_30_seconds_counter > 30 {
                every_30_seconds_counter = 0;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await
    }
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
        let serialized = cfg.to_yaml()
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

    let (mut config, original_version, was_upgraded) =
        match configuration::AnyOddBoxConfig::parse(&contents) {
            Ok(configuration) => {
                let (a, b, c) = configuration
                    .try_upgrade_to_latest_version()
                    .expect("configuration upgrade failed. this is a bug in odd-box");
                (ConfigWrapper::new(a), b, c)
            }
            Err(e) => anyhow::bail!(e),
        };

    config.is_valid()?;
    config.set_disk_path(&cfg_path)?;

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
    let tui_flag = if gui_flag { false } else { args.tui.unwrap_or(true) };

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

    let global_event_broadcaster = tokio::sync::broadcast::Sender::<GlobalEvent>::new(1024);
    let global_websockets_event_broadcaster =
        tokio::sync::broadcast::Sender::<EventForWebsocketClients>::new(1024);

    let (proc_msg_tx, _) = tokio::sync::broadcast::channel::<ProcMessage>(100);
    let inner_state = AppState::new();

    let inner_state_arc = std::sync::Arc::new(inner_state);
    let arced_tx = std::sync::Arc::new(proc_msg_tx.clone());
    let shutdown_signal = Arc::new(tokio::sync::Notify::new());
    let shared_config = std::sync::Arc::new(tokio::sync::RwLock::new(config));

    // Build initial cruma configuration from current odd-box config.
    let cruma_port_offset = std::env::var("ODD_BOX_CRUMA_PORT_OFFSET")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let cruma_cfg_init = {
        let cfg_guard = shared_config.read().await;
        let (mut cfg, notes) = build_cruma_config(&cfg_guard)?;
        if let Err(e) = apply_cruma_port_offset(&mut cfg, cruma_port_offset) {
            tracing::error!(error=%e, "Failed to apply port offset for cruma config");
        }
        if !notes.unsupported.is_empty() {
            tracing::warn!("cruma config placeholders/unsupported: {:?}", notes.unsupported);
        }
        cfg
    };
    let cruma_config_arc =
        std::sync::Arc::new(ArcSwap::from_pointee(cruma_cfg_init));

    let mut global_state = crate::global_state::GlobalState::new(
        inner_state_arc.clone(),
        shared_config.clone(),
        proc_msg_tx.clone(),
        global_event_broadcaster.clone(),
        global_websockets_event_broadcaster.clone(),
        cruma_config_arc.clone(),
        OddLogHandle::None,
    );

    let (cli_filter, cli_reload_handle) = tracing_subscriber::reload::Layer::new(
        EnvFilter::from_default_env()
            .add_directive(
                format!("odd_box={}", log_level)
                    .parse()
                    .expect("This directive should always work"),
            )
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
            )
    );

    if tui_flag {
        let (tui_filter, tui_reload_handle) = tracing_subscriber::reload::Layer::new(
            EnvFilter::from_default_env()
                .add_directive(
                    format!("odd_box={}", log_level)
                        .parse()
                        .expect("This directive should always work"),
                )
                .add_directive(
                    "odd_box::proc_host=trace"
                        .parse()
                        .expect("This directive should always work"),
                ),
        );

        global_state.log_handle = OddLogHandle::TUI(RwLock::new(tui_reload_handle));
        tracing_subscriber::registry()
            .with(logging::TuiLoggerLayer {
                log_buffer: std::sync::Arc::new(std::sync::Mutex::new(
                    logging::SharedLogBuffer::new(),
                )),
                broadcaster: global_websockets_event_broadcaster.clone(),
            })
            .with(tui_filter)
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

        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(logging::NonTuiLoggerLayer {
                broadcaster: global_websockets_event_broadcaster.clone(),
            })
            .with(cli_filter)
            .init();
    }

    let global_state = Arc::new(global_state);

    // Spawn thread cleaner (removes dead threads from the proc_thread_map)
    let cleanup_thread = tokio::spawn(generic_cleanup_thread(global_state.clone()));
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
        cstate
            .app_state
            .exit
            .store(true, std::sync::atomic::Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Before starting the proxy thread(s) we need to initialize the tracing system and the tui if enabled.
    if tui_flag {
        tui::init();
        tui_task = Some(tokio::spawn(tui::run(
            global_state.clone(),
            proc_msg_tx.clone(),
            global_websockets_event_broadcaster.clone(),
        )));
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

            // Spawn CRUMA tunnel handler using the same configuration as the hosting stack.
            {
                let notify = shutdown_for_cruma.clone();
                let state = state_for_cruma.clone();
                let cfg_arc = cruma_cfg_arc.clone();
                tokio::spawn(crate::cruma::cruma_thread(notify, state, cfg_arc));
            }

            let ct_clone = cancel.clone();
            if let Err(e) = cruma_proxy_lib::hosting::run_from_config(
                cruma_cfg_arc,
                persistence,
                ct_clone,
            )
            .await {
                tracing::error!(error=%e, "cruma hosting failed");
            }
        })
    };

    let config_guard = global_state.config.read().await;

    // Add backends to the site status map based on their type
    for (backend_id, backend) in &cloned_backends {
        match backend {
            configuration::v4::Backend::Process(proc) => {
                // Resolve and spawn process host
                match config_guard.resolve_process_backend(backend_id, proc) {
                    Ok(resolved) => {
                        // Add to site status as stopped initially
                        inner_state_arc
                            .site_status_map
                            .insert(backend_id.clone(), ProcState::Stopped);

                        tokio::task::spawn(proc_host::host(
                            resolved,
                            arced_tx.subscribe(),
                            global_state.clone(),
                        ));
                    }
                    Err(e) => bail!(
                        "Failed to resolve process configuration for backend '{}':\n{:?}",
                        backend_id, e
                    )
                }
            }
            configuration::v4::Backend::Remote(_) => {
                inner_state_arc
                    .site_status_map
                    .insert(backend_id.clone(), ProcState::Remote);
            }
            configuration::v4::Backend::Static(_) => {
                inner_state_arc
                    .site_status_map
                    .insert(backend_id.clone(), ProcState::DirServer);
            }
        }
    }

    drop(config_guard);

    tokio::task::spawn(docker_thread(global_state.clone()));

    // // if on a released/stable version, we notify the user when there is a later stable version
    // // available for them to update to. current_is_latest will not include any -rc,-pre or -dev releases
    // // and so we wont run this unless user is also on stable.
    // if !self_update::current_version().contains("-") {
    //     match self_update::current_is_latest().await {
    //         Err(e) => {
    //             tracing::warn!("It was not possible to retrieve information regarding the latest available version of odd-box: {e:?}");
    //         },
    //         Ok(Some(v)) => {
    //             tracing::info!("There is a newer version of odd-box available - please consider upgrading to {v:?}. For unmanaged installations you can run 'odd-box --update' otherwise see your package manager for upgrade instructions.");
    //         },
    //         Ok(None) => {
    //             tracing::info!("You are running the latest version of odd-box :D");
    //         }
    //     }
    // }

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
        // Run GUI on main thread - this blocks until window is closed
        if let Err(e) = gui::run(global_state.clone(), theme_mode) {
            tracing::error!("GUI error: {:?}", e);
        }
        // Signal exit when GUI closes
        global_state.app_state.exit.store(true, Ordering::SeqCst);
    // otherwise we will wait for the exit signal set by ctrl-c
    } else {
        tracing::info!("odd-box started successfully. use ctrl-c to quit.");
        while global_state.app_state.exit.load(Ordering::Relaxed) == false {
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

    // All worker loops listen for messages thru these channels. We need for wait until they have stopped their processes
    // before we can safely exit the application.
    let mut i = 0;
    while arced_tx.receiver_count() > 0 {
        if i > 30 {
            if PROC_THREAD_MAP.is_empty() {
                if tui_flag || gui_flag {
                    eprintln!(
                        "Shutdown sequence completed with warning: mismatch between PTM and ATX.."
                    )
                } else {
                    tracing::warn!(
                        "Shutdown sequence completed with warning: mismatch between PTM and ATX.."
                    )
                }
                break;
            }
            let mut awaited_processed = vec![];

            for (name, pid) in PROC_THREAD_MAP.iter().filter_map(|x| {
                if let Some(pid) = &x.pid {
                    Some((x.backend_id.clone(), pid.clone()))
                } else {
                    None
                }
            }) {
                awaited_processed.push(format!("- {} (pid: {})", name, pid))
            }

            if tui_flag || gui_flag {
                println!("Waiting for processes to die..");
                println!("{}", awaited_processed.join("\n"));
            } else {
                tracing::warn!("Waiting for hosted processes to die..");
                for p in awaited_processed {
                    tracing::warn!("{p}");
                }
            }

            i = 0;
        } else {
            i += 1;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    if tui_flag || gui_flag {
        println!("shutdown sequence for hosted processes completed successfully");
        println!("stopping proxy services..");
    } else {
        tracing::info!("shutdown for hosted processes sequence completed successfully");
        tracing::info!("stopping proxy services..");
    }

    _ = cruma_task.abort();
    _ = cruma_task.await;
    _ = cfg_monitor.abort();
    _ = cleanup_thread.abort();

    if tui_flag || gui_flag {
        println!("odd-box exited successfully");
    } else {
        tracing::info!("odd-box exited successfully");
    }

    Ok(())
}

type CliLogHandle = tracing_subscriber::reload::Handle<
    EnvFilter,
    tracing_subscriber::layer::Layered<
        logging::NonTuiLoggerLayer,
        tracing_subscriber::layer::Layered<
            Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync>,
            tracing_subscriber::Registry,
        >,
    >,
>;
type TuiLogHandle = tracing_subscriber::reload::Handle<
    EnvFilter,
    tracing_subscriber::layer::Layered<logging::TuiLoggerLayer, tracing_subscriber::Registry>,
>;

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

// fn get_user_confirmation(prompt: &str) -> bool {
//     let mut input = String::new();
//     loop {
//         print!("{} (y/n)", prompt);
//         std::io::stdout().flush().unwrap();
//         input.clear();
//         std::io::stdin().read_line(&mut input).unwrap();
//         match input.trim().to_lowercase().as_str() {
//             "y" => return true,
//             "n" => return false,
//             _ => {
//                 println!("Invalid input. Please enter 'y' or 'n'.");
//             }
//         }
//     }
// }

// we could probably subscribe to the docker socket instead of having this stupid loop..
// this does however seem to work fine and is rather simple, so keeping it for now :)
pub async fn docker_thread(state: Arc<GlobalState>) {
    loop {
        if let Ok(docker) = bollard::Docker::connect_with_local_defaults() {
            let running_container_targets = docker::get_container_proxy_targets(&docker)
                .await
                .unwrap_or_default();
            let running_container_targets_dash_map = DashMap::new();
            for x in running_container_targets {
                running_container_targets_dash_map.insert(x.generate_host_name(), x);
            }
            state.app_state.site_status_map.retain(|a, b| {
                b != &ProcState::Docker || running_container_targets_dash_map.contains_key(a)
            });
            for guard in &running_container_targets_dash_map {
                let (host_name, _) = guard.pair();
                state
                    .app_state
                    .site_status_map
                    .insert(host_name.to_string(), ProcState::Docker);
            }
            let mut guard = state.config.write().await;
            guard.docker_containers = running_container_targets_dash_map;

            // Keep cruma config in sync with docker-discovered targets.
            if let Ok((mut cfg, notes)) = build_cruma_config(&guard) {
                let offset = std::env::var("ODD_BOX_CRUMA_PORT_OFFSET")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                if let Err(e) = apply_cruma_port_offset(&mut cfg, offset) {
                    tracing::error!(error=%e, "Failed to apply port offset when updating cruma config from docker changes");
                }
                if !notes.unsupported.is_empty() {
                    tracing::warn!("cruma config placeholders/unsupported after docker update: {:?}", notes.unsupported);
                }
                state.cruma_config.store(std::sync::Arc::new(cfg));
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
static CTRL_C_TRIPPED: StdAtomicBool = StdAtomicBool::new(false);
