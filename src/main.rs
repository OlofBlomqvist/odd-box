#![warn(unused_extern_crates)]

mod configuration;
mod types;
mod tcp_proxy;
mod http_proxy;
mod proxy;
use anyhow::bail;
use anyhow::Context;
use clap::Parser;
use configuration::v2::FullyResolvedInProcessSiteConfig;
use configuration::OddBoxConfigVersion;
use dashmap::DashMap;
use global_state::GlobalState;
use configuration::v2::InProcessSiteConfig;
use configuration::v2::RemoteSiteConfig;
use configuration::OddBoxConfiguration;
use http_proxy::ProcMessage;
use notify::RecommendedWatcher;
use notify::Watcher;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing_subscriber::layer::SubscriberExt;
use types::args::Args;
use types::proc_info::BgTaskInfo;
use types::proc_info::ProcId;
use types::proc_info::ProcInfo;
use configuration::{ConfigWrapper, LogLevel};
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::path::Path;
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
mod custom_servers;

lazy_static! {
    static ref PROC_THREAD_MAP: Arc<DashMap<ProcId, ProcInfo>> = Arc::new(DashMap::new());
    static ref BG_WORKER_THREAD_MAP: Arc<DashMap<String, BgTaskInfo>> = Arc::new(DashMap::new());
    
}

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
pub fn generate_unique_id() -> u64 {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub mod global_state {
    use std::sync::{atomic::AtomicU64, Arc};

    use crate::{certs::DynamicCertResolver, tcp_proxy::{ReverseTcpProxy, ReverseTcpProxyTarget}};
    #[derive(Debug)]
    pub struct GlobalState {
        pub app_state: std::sync::Arc<crate::types::app_state::AppState>,
        pub config: std::sync::Arc<tokio::sync::RwLock<crate::configuration::ConfigWrapper>>,
        pub broadcaster: tokio::sync::broadcast::Sender<crate::http_proxy::ProcMessage>,
        pub target_request_counts: dashmap::DashMap<String, AtomicU64>,
        pub cert_resolver: std::sync::Arc<DynamicCertResolver>,
        reverse_tcp_proxy_target_cache : dashmap::DashMap<String,Arc<ReverseTcpProxyTarget>>
    }
    impl GlobalState {
        
        pub fn new(
            app_state: std::sync::Arc<crate::types::app_state::AppState>,
            config: std::sync::Arc<tokio::sync::RwLock<crate::configuration::ConfigWrapper>>,
            broadcaster: tokio::sync::broadcast::Sender<crate::http_proxy::ProcMessage>,
            cert_resolver: std::sync::Arc<DynamicCertResolver>        
        ) -> Self {

            Self {
                app_state,
                config,
                broadcaster,
                target_request_counts: dashmap::DashMap::new(),
                cert_resolver,
                reverse_tcp_proxy_target_cache: dashmap::DashMap::new(),
            }
        }
        
        pub fn invalidate_cache(&self) {
            self.reverse_tcp_proxy_target_cache.clear();
        }

        pub fn invalidate_cache_for(&self,host_name:&str) {
            self.reverse_tcp_proxy_target_cache.remove(host_name);
        }

        // returns None if the target does not match fully or subdomain. 
        // returns Some(Some(subdomain_name)) if the target matches the subdomain
        // returns Some(None) if the target matches fully
        fn filter_fun(req_host_name:&str,target_host_name:&str,allow_subdomains:bool) -> Option<Option<String>> {
            
            let parsed_name = if req_host_name.contains(":") {
                req_host_name.split(":").next().expect("if something contains a colon and we split the thing there must be at least one part")
            } else {
                req_host_name
            };

            
            if target_host_name.eq_ignore_ascii_case(parsed_name) {
            Some(None)
            } else if allow_subdomains {
                match ReverseTcpProxy::get_subdomain(parsed_name, &target_host_name) {
                    Some(subdomain) => Some(Some(subdomain))
                    ,
                    None => None,
                }
            } else {
                None
            }
        }

        pub async fn try_find_site(&self,pre_filter_hostname:&str) -> Option<Arc<ReverseTcpProxyTarget>> {

            if let Some(pt) = self.reverse_tcp_proxy_target_cache.get(pre_filter_hostname) {
                let (_k,v)  = pt.pair();
                if let Some(cached_proc_id) = &v.proc_id {
                    if let Some(_proc_info) = crate::PROC_THREAD_MAP.get(cached_proc_id) {
                        tracing::debug!("Cache hit for {pre_filter_hostname}");
                        return Some(v.clone());
                    } else {
                        tracing::trace!("Cache miss for {pre_filter_hostname} due to missing proc info");
                    }
                }
                
            } else {
                tracing::trace!("Cache miss for {pre_filter_hostname}");
            }

            let mut result = None;
            let cfg = self.config.read().await;

            for y in cfg.hosted_process.iter().flatten() {

                let filter_result = Self::filter_fun(pre_filter_hostname, &y.host_name, y.capture_subdomains.unwrap_or_default());
                if filter_result.is_none() { continue };
                let sub_domain = filter_result.and_then(|x|x);
                
                let port = y.active_port.unwrap_or_default();
                if port > 0 {
                    let t = ReverseTcpProxyTarget {
                        proc_id : Some(y.get_id().clone()),
                        disable_tcp_tunnel_mode: y.disable_tcp_tunnel_mode.unwrap_or_default(),
                        remote_target_config: None, // we dont need this for hosted processes
                        hosted_target_config: Some(y.clone()),
                        capture_subdomains: y.capture_subdomains.unwrap_or_default(),
                        forward_wildcard: y.forward_subdomains.unwrap_or_default(),
                        backends: vec![crate::configuration::v2::Backend {
                            hints: y.hints.clone(),
                            
                            // use dns name to avoid issues where hyper uses ipv6 for 127.0.0.1 since tcp tunnel mode uses ipv4.
                            // not keeping them the same means the target backend will see different ip's for the same client
                            // and possibly invalidate sessions in some cases.
                            address: "localhost".to_string(), //y.host_name.to_owned(), // --- configurable
                            https: y.https,
                            port: y.active_port.unwrap_or_default()
                        }],
                        host_name: y.host_name.to_string(),
                        is_hosted: true,
                        sub_domain: sub_domain
                    };
                    let shared_result = Arc::new(t);
                    self.reverse_tcp_proxy_target_cache.insert(pre_filter_hostname.into(), shared_result.clone());
                    result = Some(shared_result);
                    break;
                }

                
            }
        

            if let Some(x) = &cfg.remote_target {
                for y in x.iter().filter(|x|pre_filter_hostname.to_uppercase().contains(&x.host_name.to_uppercase()))  {
                    
                    let filter_result = Self::filter_fun(pre_filter_hostname, &y.host_name, y.capture_subdomains.unwrap_or_default());
                    if filter_result.is_none() { continue };
                    let sub_domain = filter_result.and_then(|x|x);

                    let t = ReverseTcpProxyTarget { 
                        proc_id : None,
                        disable_tcp_tunnel_mode: y.disable_tcp_tunnel_mode.unwrap_or_default(),
                        hosted_target_config: None,
                        remote_target_config: Some(y.clone()),
                        capture_subdomains: y.capture_subdomains.unwrap_or_default(),
                        forward_wildcard: y.forward_subdomains.unwrap_or_default(),
                        backends: y.backends.clone(),
                        host_name: y.host_name.to_owned(),
                        is_hosted: false,
                        sub_domain: sub_domain
                    };
                    let shared_result = Arc::new(t);
                    self.reverse_tcp_proxy_target_cache.insert(pre_filter_hostname.into(), shared_result.clone());
                    result = Some(shared_result);
                    break;
                    
                }
            }

            result
            
        }
    }
    
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, std::sync::mpsc::Receiver<notify::Result<notify::Event>>)> {
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
    static ref RELOADING_CONFIGURATION : tokio::sync::Semaphore = tokio::sync::Semaphore::new(1);
} 


async fn config_file_monitor(
    config: Arc<RwLock<ConfigWrapper>>, 
    global_state: Arc<GlobalState>
) -> anyhow::Result<()> {
    
    let guard = config.read().await;
    let cfg_path = guard.path.clone().expect("odd-box must be using a configuration file.");
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
                            match crate::configuration::reload::reload_from_disk(global_state.clone()).await {
                                Ok(_) => {},
                                Err(e) => {
                                    tracing::error!("Failed to reload configuration file: {e:?}");
                                }
                            }
                    },
                    notify::EventKind::Remove(_remove_kind) => {
                        tracing::error!("Configuration file was removed. This is not supported. Please restart odd-box.");
                    
                    },
                    _ => {},
                }

            },
            Err(e) => {
                match e {
                    std::sync::mpsc::TryRecvError::Empty => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    },
                    std::sync::mpsc::TryRecvError::Disconnected => {
                        tracing::error!("Config file watcher channel disconnected. This is a bug in odd-box.");
                        break;
                    }
                }
            },
        }
    }

    Ok(())
}

async fn thread_cleaner() {
    let liveness_token = Arc::new(true);
    crate::BG_WORKER_THREAD_MAP.insert("The Janitor".into(), BgTaskInfo {
        liveness_ptr: Arc::downgrade(&liveness_token),
        status: "Managing active tasks..".into()
    }); 
    loop {
        {
            PROC_THREAD_MAP.retain(|_k,v| v.liveness_ptr.upgrade().is_some());
            BG_WORKER_THREAD_MAP.retain(|_k,v| v.liveness_ptr.upgrade().is_some());
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await
    }
}



fn generate_config(file_name:Option<&str>, fill_example:bool) -> anyhow::Result<crate::configuration::v2::OddBoxV2Config> {
    let current_working_dir = std::env::current_dir()?;
    if let Some(file_name) = file_name {
        let current_working_dir = std::env::current_dir()?;
        let file_path = current_working_dir.join(file_name);
        if std::path::Path::exists(std::path::Path::new(file_name)) {
            return Err(anyhow::anyhow!(format!("File already exists: {file_path:?}")));
        }
    }
    if fill_example == false {
        let mut init_cfg = include_str!("./init-cfg.toml").to_string();

        if cfg!(target_os = "macos") {
            // mac os allows for binding to lower ports without root, so we can use the default ports.
            // notably it only allows it when binding to 0.0.0.0 so we need to change the ip to
            init_cfg = init_cfg
                .replace("ip = \"127.0.0.1\" ","ip = \"0.0.0.0\" ")
                .replace("tls_port = 4343","tls_port = 443")
                .replace("http_port = 8080","http_port = 80");
        }
          
        let cfg = configuration::OddBoxConfig::parse(&init_cfg).map_err(|e| {
            anyhow::anyhow!(format!("Failed to parse initial configuration: {e}"))
        })?;
        match cfg {
            configuration::OddBoxConfig::V2(parsed_config) => {
                if let Some(file_name) = file_name {
                    let file_path = current_working_dir.join(file_name);
                    std::fs::write(&file_path, init_cfg)?;
                    tracing::info!("Configuration file written to {file_path:?}");    
                }
                return Ok(parsed_config)
            },
            _ => {
                return Err(anyhow::anyhow!("Failed to parse initial configuration"))
            }
        }
    }     
    let cfg = crate::configuration::v2::OddBoxV2Config::example();
    if let Some(file_name) = file_name {
        let serialized = cfg.to_string()?;
        let file_path = current_working_dir.join(file_name);
        std::fs::write(&file_path, serialized)?;
        tracing::info!("Configuration file written to {file_path:?}");
    }
    return Ok(cfg)

}

// (validated_cfg, original_version)
fn initialize_configuration(args:&Args) -> anyhow::Result<(ConfigWrapper,OddBoxConfigVersion)> {

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


    let mut file = std::fs::File::open(&cfg_path).with_context(||format!("failed to open configuration file {cfg_path:?}"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).with_context(||format!("failed to read data from configuration file {cfg_path:?}"))?;   
    
    let (mut config,original_version) = 
        match configuration::OddBoxConfig::parse(&contents) {
            Ok(configuration) => {
                let (a,b) = 
                    configuration
                        .try_upgrade_to_latest_version()
                        .expect("configuration upgrade failed. this is a bug in odd-box");
                (ConfigWrapper::new(a),b)
            },
            Err(e) => anyhow::bail!(e),
        };
    
    config.is_valid()?;
    config.set_disk_path(&cfg_path)?;

    let srv_port : u16 = if let Some(p) = args.port { p } else { config.http_port.unwrap_or(8080) } ;
    let srv_tls_port : u16 = if let Some(p) = args.tls_port { p } else { config.tls_port.unwrap_or(4343) } ;


    let socket_addr_http = SocketAddr::new(config.ip.unwrap_or(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),srv_port);
    let socket_addr_https = SocketAddr::new(config.ip.unwrap_or(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),srv_tls_port);

    match std::net::TcpListener::bind(socket_addr_http) {
        Err(e) => {
            anyhow::bail!("TCP Bind port for http {socket_addr_http:?} failed. It could be taken by another service like iis,apache,nginx etc, or perhaps you do not have permission to bind. The specific error was: {e:?}")
        },
        Ok(_listener) => {
            tracing::debug!("TCP Port for http {srv_port} is available for binding.");
        }
    }
    match std::net::TcpListener::bind(socket_addr_https) {
        Err(e) => {
            anyhow::bail!("TCP Bind port for https {socket_addr_https:?} failed. It could be taken by another service like iis,apache,nginx etc, or perhaps you do not have permission to bind. The specific error was: {e:?}")
        },
        Ok(_listener) => {
            tracing::debug!("TCP Port for https {srv_tls_port} is available for binding.");
        }
    }

    Ok((config,original_version))
}

#[tokio::main(flavor="multi_thread")]
async fn main() -> anyhow::Result<()> {
    match rustls::crypto::ring::default_provider().install_default() {
        Ok(_) => {},
        Err(e) => {
            bail!("Failed to install default ring provider: {:?}",e)
        }
    }
    
    let args = Args::parse();
    
    if args.config_schema {
        let schema = schemars::schema_for!(crate::configuration::v2::OddBoxV2Config);
        println!("{}", serde_json::to_string_pretty(&schema).expect("schema should be serializable"));
        return Ok(());
    }

    if args.update {
        _ = self_update::update().await;
        return Ok(());
    }
    
    let tui_flag = args.tui.unwrap_or(true);

    if args.generate_example_cfg {
        generate_config(Some("odd-box-example-config.toml"),true)?;
        return Ok(())
    } else if args.init {
        generate_config(Some("odd-box.toml"),false)?;
        return Ok(())
    }

    let (config,original_version) = initialize_configuration(&args)?;

    if args.upgrade_config {
        match original_version {
            OddBoxConfigVersion::V2 => {
                println!("Configuration file is already at the latest version.");
                return Ok(())
            },
            _ => {}
        }
        config.write_to_disk()?;
        println!("Configuration file upgraded successfully!");
        return Ok(());
    } else if let OddBoxConfigVersion::V2 = original_version {
        // do nothing
    } else {
        println!("Your configuration file is using an old schema ({:?}), consider upgrading it using the '--upgrade-config' command.",original_version);
    }



    let cloned_procs = config.hosted_process.clone();
    let cloned_remotes = config.remote_target.clone();
    let cloned_custom_dir = config.dir_server.clone();
    
    
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
    
    let mut srv_ip = if let Some(ip) = config.ip { ip.to_string() } else { "127.0.0.1".to_string() };

    // now if srv_ip is ipv6, we need to wrap it in square brackets:
    if srv_ip.contains(":") {
        srv_ip = format!("[{}]",srv_ip);
    }


    let http_bind_addr = format!("{srv_ip}:{srv_port}");
    let https_bind_addr = format!("{srv_ip}:{srv_tls_port}");

    let http_port = http_bind_addr.parse().context(format!("Invalid http listen addr configured: '{http_bind_addr}'."))?;
    let https_port = https_bind_addr.parse().context(format!("Invalid https listen addr configured: '{https_bind_addr}'."))?;
    
    let enable_lets_encrypt = config.lets_encrypt_account_email.is_some();

    let arced_tx = std::sync::Arc::new(tx.clone());
    let shutdown_signal = Arc::new(tokio::sync::Notify::new());
    let api_broadcaster = tracing_broadcaster.clone();
    let le_acc_mail = config.lets_encrypt_account_email.clone();
    let shared_config = std::sync::Arc::new(tokio::sync::RwLock::new(config));
    
    let global_state = Arc::new(crate::global_state::GlobalState::new( 
        inner_state_arc.clone(), 
        shared_config.clone(), 
        tx.clone(),
        Arc::new(certs::DynamicCertResolver::new(
            enable_lets_encrypt,le_acc_mail).await.context("could not create cert resolver for the global state!")?
        ),
        
    ));


    tokio::task::spawn(crate::letsencrypt::bg_worker_for_lets_encrypt_certs(global_state.clone()));
    
    // Spawn task for the admin api if enabled
    if let Some(api_port) = api_port {
        let api_state = global_state.clone();
        tokio::spawn(async move {
            api::run(api_state,api_port, api_broadcaster).await
        });
    }

    // Spawn thread cleaner (removes dead threads from the proc_thread_map)
    let cleanup_thread = tokio::spawn(thread_cleaner());
    let cfg_monitor = tokio::spawn(config_file_monitor(shared_config.clone(),global_state.clone()));
   
    let mut tui_task : Option<JoinHandle<()>> = None;


    let intial_log_filter = EnvFilter::from_default_env()
        .add_directive(format!("odd_box={}",log_level).parse().expect("this directive will always work"));

    // Before starting the proxy thread(s) we need to initialize the tracing system and the tui if enabled.
    if tui_flag {
        
        // note: we use reload-handle because we plan to implement support for switching log level at runtime at least in tui mode.
        let (filter, _reload_handle) = tracing_subscriber::reload::Layer::new(intial_log_filter);
            // ^ todo: perhaps invert this logic
        tui::init();
        tui_task = Some(tokio::spawn(tui::run(
            global_state.clone(),
            tx, 
            tracing_broadcaster,
            filter
        )));
    } else {

        init_logging(intial_log_filter, Some(tracing_broadcaster));

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
            http_port,
            https_port,
            arced_tx.clone(),
            global_state.clone(),
            shutdown_signal
        ));

    
    let mut config_guard = global_state.config.write().await;

    // Add any remotes to the site list
    for x in cloned_remotes.iter().flatten() {
        inner_state_arc.site_status_map.insert(x.host_name.to_owned(), ProcState::Remote);
    }

    // Add any hosted dirs to site list
    for x in cloned_custom_dir.iter().flatten() {
        inner_state_arc.site_status_map.insert(x.host_name.to_owned(), ProcState::Dynamic);
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
        
        tracing::info!("odd-box started successfully. use ctrl-c to quit.");
        while global_state.app_state.exit.load(Ordering::Relaxed) == false {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
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
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
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
    _ = cfg_monitor.abort();
    _ = cleanup_thread.abort();


    if tui_flag {
        println!("odd-box exited successfully");
    } else {
        tracing::info!("odd-box exited successfully");
    }

    Ok(())

}




pub fn init_logging(intial_log_filter:EnvFilter,tracing_broadcaster:Option<tokio::sync::broadcast::Sender<String>>) {
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

    if let Some(tracing_broadcaster) = tracing_broadcaster {

    tracing_subscriber::Registry::default()
        .with(fmt_layer)
        .with(intial_log_filter)
        .with(logging::NonTuiLoggerLayer { broadcaster: tracing_broadcaster.clone() })
        .init()
    } else {
        tracing_subscriber::Registry::default()
        .with(fmt_layer)
        .with(intial_log_filter)
        .init()
    };
}