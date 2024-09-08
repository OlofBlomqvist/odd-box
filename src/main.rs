#![warn(unused_extern_crates)]

mod configuration;
mod types;
mod tcp_proxy;
mod http_proxy;
mod proxy;
use anyhow::bail;
use clap::Parser;
use configuration::v2::FullyResolvedInProcessSiteConfig;
use configuration::LogFormat;
use dashmap::DashMap;
use global_state::GlobalState;
use configuration::v2::InProcessSiteConfig;
use configuration::v2::RemoteSiteConfig;
use configuration::OddBoxConfiguration;
use http_proxy::ProcMessage;
use tokio::task::JoinHandle;
use tracing_subscriber::layer::SubscriberExt;
use types::args::Args;
use types::proc_info::ProcId;
use types::proc_info::ProcInfo;
use configuration::{ConfigWrapper, LogLevel};
use std::net::Ipv4Addr;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use types::custom_error::*;
use std::io::Read;
use std::sync::Arc;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;
mod proc_host;
use tracing_subscriber::util::SubscriberInitExt;
use types::app_state::ProcState;
mod tui;
mod api;
mod logging;
mod tests;
mod certs;
mod self_update;
use types::app_state::AppState;
use lazy_static::lazy_static;
mod letsencrypt;

lazy_static! {
    static ref PROC_THREAD_MAP: Arc<DashMap<ProcId, ProcInfo>> = Arc::new(DashMap::new());
}

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
pub fn generate_unique_id() -> u64 {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub mod global_state {
    use std::sync::atomic::AtomicU64;

    use crate::certs::DynamicCertResolver;
    #[derive(Debug)]
    pub struct GlobalState {
        pub app_state: std::sync::Arc<crate::types::app_state::AppState>,
        pub config: std::sync::Arc<tokio::sync::RwLock<crate::configuration::ConfigWrapper>>,
        pub broadcaster: tokio::sync::broadcast::Sender<crate::http_proxy::ProcMessage>,
        pub target_request_counts: dashmap::DashMap<String, AtomicU64>,
        pub request_count: std::sync::atomic::AtomicUsize,
        pub cert_resolver: std::sync::Arc<DynamicCertResolver>
    }
    
}

async fn thread_cleaner() {
    loop {
        PROC_THREAD_MAP.retain(|_k,v| v.liveness_ptr.upgrade().is_some());
        tokio::time::sleep(Duration::from_secs(1)).await
    }
}

fn generate_config(file_name:&str, fill_example:bool) -> anyhow::Result<()> {

    let current_working_dir = std::env::current_dir()?;
    let file_path = current_working_dir.join(file_name);

    if std::path::Path::exists(std::path::Path::new(file_name)) {
        return Err(anyhow::anyhow!(format!("File already exists: {file_path:?}")));
    }

    let mut cfg = crate::configuration::v2::OddBoxV2Config::example();
    
    if fill_example == false {
        cfg.hosted_process = None;
        cfg.remote_target = None;
        cfg.env_vars = vec![];
        cfg.alpn = None;
        cfg.admin_api_port = None;
        cfg.auto_start = None;
        cfg.http_port = None;
        cfg.tls_port = None;
        cfg.ip = None;
        cfg.log_level = None;
        cfg.default_log_format = LogFormat::standard;
    }

    let serialized = cfg.to_string()?;
    std::fs::write(&file_path, serialized).unwrap();
    tracing::info!("Configuration file written to {file_path:?}");
    return Ok(())

}


fn initialize_configuration(args:&Args) -> anyhow::Result<ConfigWrapper> {

    let cfg_path = 
        if let Some(cfg) = &args.configuration {
            cfg.to_string()
        } else {
            if std::fs::metadata("odd-box.toml").is_ok() {
                "odd-box.toml".to_owned()
            } else if std::fs::metadata("oddbox.toml").is_ok() {
                "oddbox.toml".to_owned()
            } else {
                "Config.toml".to_owned()
            }
        };


    let mut file = std::fs::File::open(&cfg_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;    
    
    let mut config: ConfigWrapper = 
        ConfigWrapper::new(match configuration::OddBoxConfig::parse(&contents) {
            Ok(configuration) => configuration.try_upgrade_to_latest_version().expect("configuration upgrade failed. this is a bug in odd-box"),
            Err(e) => anyhow::bail!(e),
        });
    
    config.is_valid()?;
    config.init(&cfg_path)?;

    // Validate that we are allowed to bind prior to attempting to initialize hyper since it will simply on failure otherwise.
    let srv_port : u16 = if let Some(p) = args.port { p } else { config.http_port.unwrap_or(8080) } ;
    let srv_tls_port : u16 = if let Some(p) = args.tls_port { p } else { config.tls_port.unwrap_or(4343) } ;
    for p in vec![srv_port,srv_tls_port] {
        let srv = std::net::TcpListener::bind(
            format!("{}:{}",config.ip.unwrap_or(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),p)
        );
        match srv {
            Err(e) => {
                anyhow::bail!("TCP Bind port {} failed. It could be taken by another service like iis,apache,nginx etc, or perhaps you do not have permission to bind. The specific error was: {e:?}",p)
            },
            Ok(_listener) => {
                tracing::debug!("TCP Port {} is available for binding.",p);
            }
        }
    }

    Ok(config)
}

#[tokio::main(flavor="multi_thread")]
async fn main() -> anyhow::Result<()> {

    let args = Args::parse();
    
    if args.update {
        _ = self_update::update().await;
        return Ok(());
    }
    
    let tui_flag = args.tui.unwrap_or(true);

    if args.generate_example_cfg {
        generate_config("odd-box-example-config.toml",true)?;
        return Ok(())
    } else if args.init {
        generate_config("odd-box.toml",false)?;
        return Ok(())
    }

    let config = initialize_configuration(&args)?;

    if args.upgrade_config {
        config.write_to_disk()?;
    }

    let cloned_procs = config.hosted_process.clone();
    let cloned_remotes = config.remote_target.clone();
    
    let log_level : LevelFilter = match config.log_level{
        Some(LogLevel::Info) => LevelFilter::INFO,
        Some(LogLevel::Error) => LevelFilter::ERROR,
        Some(LogLevel::Warn) => LevelFilter::WARN,
        Some(LogLevel::Trace) => LevelFilter::TRACE,
        Some(LogLevel::Debug) => LevelFilter::DEBUG,
        _ => LevelFilter::INFO
    };
    let tracing_broadcaster = tokio::sync::broadcast::Sender::<String>::new(10);
    let (tx,_) = tokio::sync::broadcast::channel::<ProcMessage>(33);
    let inner_state = AppState::new();  
    let api_port = config.admin_api_port.clone();
    let inner_state_arc = std::sync::Arc::new(inner_state);
    let srv_port : u16 = if let Some(p) = args.port { p } else { config.http_port.unwrap_or(8080) } ;
    let srv_tls_port : u16 = if let Some(p) = args.tls_port { p } else { config.tls_port.unwrap_or(4343) } ;
    let srv_ip = if let Some(ip) = config.ip { ip.to_string() } else { "127.0.0.1".to_string() };
    let arced_tx = std::sync::Arc::new(tx.clone());
    let shutdown_signal = Arc::new(tokio::sync::Notify::new());
    let api_broadcaster = tracing_broadcaster.clone();
    let shared_config = std::sync::Arc::new(tokio::sync::RwLock::new(config));
    let global_state = Arc::new(crate::global_state::GlobalState { 
        cert_resolver: Arc::new(certs::DynamicCertResolver::new().await),
        app_state: inner_state_arc.clone(), 
        config: shared_config.clone(), 
        broadcaster:tx.clone(),
        target_request_counts: DashMap::new(),
        request_count: std::sync::atomic::AtomicUsize::new(0)
    });

    // Spawn task for the admin api if enabled
    if let Some(api_port) = api_port {
        let api_state = global_state.clone();
        tokio::spawn(async move {
            api::run(api_state,api_port, api_broadcaster).await
        });
    }

    // Spawn thread cleaner (removes dead threads from the proc_thread_map)
    tokio::spawn(thread_cleaner());
   
    let mut tui_task : Option<JoinHandle<()>> = None;

    // Before starting the proxy thread(s) we need to initialize the tracing system and the tui if enabled.
    if tui_flag {
        
        // note: we use reload-handle because we plan to implement support for switching log level at runtime at least in tui mode.
        let (filter, _reload_handle) = tracing_subscriber::reload::Layer::new(
            EnvFilter::from_default_env()
                .add_directive(log_level.into())
                .add_directive("h2=info".parse().expect("this directive will always work"))
                .add_directive("tokio_util=info".parse().expect("this directive will always work"))     
                .add_directive("rustls=info".parse().expect("this directive will always work"))
                .add_directive("mio=info".parse().expect("this directive will always work"))                            
                .add_directive("hyper=info".parse().expect("this directive will always work")));
            // ^ todo: perhaps invert this logic
        tui::init();
        tui_task = Some(tokio::spawn(tui::run(
            global_state.clone(),
            tx, 
            tracing_broadcaster,
            filter
        )));
    } else {
        let fmt_layer = tracing_subscriber::fmt::layer()
            .compact()
            .with_thread_names(true)
            .with_timer(
                tracing_subscriber::fmt::time::OffsetTime::new(
                    time::UtcOffset::from_whole_seconds(
                        chrono::Local::now().offset().local_minus_utc()
                    ).expect("time... works"), 
                    time::macros::format_description!("[hour]:[minute]:[second]")
                )
            );
        let filter_layer = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(log_level.into())
            .add_directive("h2=info".parse().expect("this directive will always work"))
            .add_directive("tokio_util=info".parse().expect("this directive will always work"))     
            .add_directive("rustls=info".parse().expect("this directive will always work"))
            .add_directive("mio=info".parse().expect("this directive will always work"))                            
            .add_directive("hyper=info".parse().expect("this directive will always work"));
        // ^ todo: perhaps invert this logic

        let subscriber = tracing_subscriber::Registry::default()
            .with(fmt_layer)
            .with(filter_layer)
            .with(logging::NonTuiLoggerLayer { broadcaster: tracing_broadcaster.clone() });

        subscriber.init();

        // From now on, we need to capture ctrl-c and make sure to shut down the application gracefully
        // as we are about to spawn a bunch of processes that we need to shut down properly.
        let cstate = global_state.clone();
        ctrlc::set_handler(move || {
            tracing::warn!("Ctrl-C received. Shutting down..");
            cstate.app_state.exit.store(true, std::sync::atomic::Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");

        // ^ Note that we only set this while not in tui mode. In tui mode we have a separate handler for this.

    }


    // Now that tracing is initialized we can spawn the main proxy thread
    let proxy_thread = 
        tokio::spawn(crate::proxy::listen(
            shared_config.clone(),
            format!("{srv_ip}:{srv_port}").parse().expect("bind address for http must be valid.."),
            format!("{srv_ip}:{srv_tls_port}").parse().expect("bind address for https must be valid.."),
            arced_tx.clone(),
            global_state.clone(),
            shutdown_signal
        ));

    
    let mut config_guard = global_state.config.write().await;

    // Add any remotes to the site list
    for x in cloned_remotes.iter().flatten() {
        inner_state_arc.site_status_map.insert(x.host_name.to_owned(), ProcState::Remote);
    }

    // And spawn the hosted process worker loops
    for x in cloned_procs.iter().flatten() {
        match config_guard.resolve_process_configuration(&x) {
            Ok(x) => {
                tokio::task::spawn(proc_host::host(
                    x,
                    arced_tx.subscribe(),
                    global_state.clone(),
                ));
            }
            Err(e) => bail!("Failed to resolve process configuration for:\n=====================================================\n{:?}.\n=====================================================\n\nThe error was: {:?}",x,e)
        }
    }

    drop(config_guard);

    // if in tui mode, we can just hang around until the tui thread exits.
    if let Some(tt) = tui_task {        
        _ = tt.await;
    // otherwise we will wait for the exit signal set by ctrl-c
    } else {
        while global_state.app_state.exit.load(Ordering::Relaxed) == false {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    // ^ Note that after this point when the application has been running in TUI mode, we can no longer use tracing as the subscriber
    //   writes to the TUI buffers, and so we must now use println from here on out.
    if tui_flag {
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
                if tui_flag {
                    eprintln!("Shutdown sequence completed with warning: mismatch between PTM and ATX..")
                } else {
                    tracing::warn!("Shutdown sequence completed with warning: mismatch between PTM and ATX..")
                }
                break;
            }
            let mut awaited_processed = vec![];

            for (name,pid) in PROC_THREAD_MAP.iter().filter_map(|x|{
                if let Some(pid) = &x.pid {
                    Some((x.config.host_name.clone(),pid.clone()))
                } else {
                    None
                }
            }) {
                awaited_processed.push(format!("- {} (pid: {})",name,pid))
            }

            if tui_flag {
                println!("Waiting for processes to die..");
                println!("{}",awaited_processed.join("\n"));
            } else {
                tracing::warn!("Waiting for hosted processes to die..");
                for p in awaited_processed {
                    tracing::warn!("{p}");
                }
            }
            
            i = 0;
        } else {
            i+=1;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
 
    if tui_flag {
        println!("shutdown sequence for hosted processes completed successfully");
        println!("stopping proxy services..");
    } else {
        tracing::info!("shutdown for hosted processes sequence completed successfully");
        tracing::info!("stopping proxy services..");
    }

    _ = proxy_thread.abort();
    _ = proxy_thread.await;
 

    if tui_flag {
        println!("odd-box exited successfully");
    } else {
        tracing::info!("odd-box exited successfully");
    }

    Ok(())

}

