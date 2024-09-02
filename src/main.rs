#![warn(unused_extern_crates)]

mod configuration;
mod types;
mod tcp_proxy;
mod http_proxy;
mod proxy;
use anyhow::bail;
use configuration::v2::FullyResolvedInProcessSiteConfig;
use dashmap::DashMap;
use global_state::GlobalState;
use configuration::v2::InProcessSiteConfig;
use configuration::v2::RemoteSiteConfig;
use configuration::OddBoxConfiguration;
use http_proxy::ProcMessage;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use self_update::cargo_crate_version;
use tracing_subscriber::layer::SubscriberExt;
use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;
use types::custom_error::*;
use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Weak};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;
mod proc_host;
use tracing_subscriber::util::SubscriberInitExt;
use crate::types::app_state::ProcState;
mod tui;
mod api;
mod logging;
mod tests;
use types::app_state::AppState;
use lazy_static::lazy_static;

#[derive(Eq,PartialEq,Debug,Clone,Hash, Serialize, Deserialize)]
pub struct ProcId { id: String }
impl ProcId {
    pub fn new() -> Self {
        Self { id: uuid::Uuid::new_v4().to_string() }
    }
}

#[derive(Debug)]
pub struct ProcInfo {
    pub liveness_ptr : Weak<AtomicBool>,
    pub config : FullyResolvedInProcessSiteConfig,
    pub pid : Option<String>
}

lazy_static! {
    static ref PROC_THREAD_MAP: Arc<DashMap<ProcId, ProcInfo>> = Arc::new(DashMap::new());
}

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
pub fn generate_unique_id() -> u64 {
    REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub mod global_state {
    use std::sync::atomic::AtomicU64;
    #[derive(Debug)]
    pub struct GlobalState {
        pub app_state: std::sync::Arc<crate::types::app_state::AppState>,
        pub config: std::sync::Arc<tokio::sync::RwLock<crate::configuration::ConfigWrapper>>,
        pub broadcaster: tokio::sync::broadcast::Sender<crate::http_proxy::ProcMessage>,
        pub target_request_counts: dashmap::DashMap<String, AtomicU64>,
        pub request_count: std::sync::atomic::AtomicUsize
    }
    
}


#[derive(Debug)]
struct DynamicCertResolver {
    // todo: dashmap?
    cache: Mutex<HashMap<String, Arc<tokio_rustls::rustls::sign::CertifiedKey>>>,
}

use tokio_rustls::rustls::server::{ClientHello, ResolvesServerCert};

impl ResolvesServerCert for DynamicCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<std::sync::Arc<tokio_rustls::rustls::sign::CertifiedKey>> {
        
        let server_name = client_hello.server_name()?;
        
        {
            let cache = self.cache.lock().expect("should always be able to read cert cache");
            if let Some(certified_key) = cache.get(server_name) {
                tracing::trace!("Returning a cached certificate for {:?}",server_name);
                return Some(certified_key.clone());
            }
        }

        let odd_cache_base = ".odd_box_cache";

        let base_path = std::path::Path::new(odd_cache_base);
        let host_name_cert_path = base_path.join(server_name);
    
        if let Err(e) = std::fs::create_dir_all(&host_name_cert_path) {
            tracing::error!("Could not create directory: {:?}", e);
            return None;
        }

        let cert_path = format!("{}/{}/cert.pem",odd_cache_base,server_name);
        let key_path = format!("{}/{}/key.pem",odd_cache_base,server_name);

        if let Err(e) = generate_cert_if_not_exist(server_name, &cert_path, &key_path) {
            tracing::error!("Could not generate cert: {:?}", e);
            return None
        }

        
        if let Ok(cert_chain) = my_certs(&cert_path) {

            if cert_chain.is_empty() {
                tracing::warn!("EMPTY CERT CHAIN FOR {}",server_name);
                return None
            }
            if let Ok(private_key) = my_rsa_private_keys(&key_path) {
                if let Ok(rsa_signing_key) = tokio_rustls::rustls::crypto::aws_lc_rs::sign::any_supported_type(&private_key) {
                    let result = std::sync::Arc::new(tokio_rustls::rustls::sign::CertifiedKey::new(
                        cert_chain, 
                        rsa_signing_key
                    ));
                    let mut cache = self.cache.lock().expect("should always be able to write to cert cache");
                    cache.insert(server_name.into(), result.clone());
                    Some(result)

                } else {
                    tracing::error!("rustls::crypto::ring::sign::any_supported_type - failed to read cert: {cert_path}");
                    None
                }
            } else {
                tracing::error!("my_rsa_private_keys - failed to read cert: {cert_path}");
                None
            }
        } else {
            tracing::error!("generate_cert_if_not_exist - failed to read cert: {cert_path}");
            None
        }
    }
}

use std::io::BufReader;
use std::fs::File;


fn generate_cert_if_not_exist(hostname: &str, cert_path: &str,key_path: &str) -> Result<(),String> {
    
    let crt_exists = std::fs::metadata(cert_path).is_ok();
    let key_exists = std::fs::metadata(key_path).is_ok();

    if crt_exists && key_exists {
        tracing::debug!("Using existing certificate for {}",hostname);
        return Ok(())
    }
    
    if crt_exists != key_exists {
        return Err(String::from("Missing key or crt for this hostname. Remove both if you want to generate a new set, or add the missing one."))
    }

    tracing::debug!("Generating new certificate for site '{}'",hostname);
    

    match rcgen::generate_simple_self_signed(
        vec![hostname.to_owned()]
    ) {
        Ok(cert) => {
            tracing::trace!("Generating new self-signed certificate for host '{}'!",hostname);
            let _ = std::fs::write(&cert_path, cert.cert.pem());
            let _ = std::fs::write(&key_path, &cert.key_pair.serialize_pem());
            Ok(())               
        },
        Err(e) => Err(e.to_string())
    }
}


fn my_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, std::io::Error> {
    let cert_file = File::open(path)?;
    let mut reader = BufReader::new(cert_file);
    let certs = rustls_pemfile::certs(&mut reader);
    Ok(certs.filter_map(|cert|match cert {
        Ok(x) => Some(x),
        Err(_) => None,
    }).collect())
}

fn my_rsa_private_keys(path: &str) -> Result<PrivateKeyDer, String> {

    let file = File::open(&path).map_err(|e|format!("{e:?}"))?;
    let mut reader = BufReader::new(file);
    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .collect::<Result<Vec<tokio_rustls::rustls::pki_types::PrivatePkcs8KeyDer>,_>>().map_err(|e|format!("{e:?}"))?;

    match keys.len() {
        0 => Err(format!("No PKCS8-encoded private key found in {path}").into()),
        1 => Ok(PrivateKeyDer::Pkcs8(keys.remove(0))),
        _ => Err(format!("More than one PKCS8-encoded private key found in {path}").into()),
    }

}

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = Some("ODD-BOX MAIN REPOSITORY: https://github.com/OlofBlomqvist/odd-box"))]
struct Args {

    /// Path to your configuration file. By default we will look for odd-box.toml and Config.toml.
    #[arg(index = 1)]
    configuration: Option<String>,

    /// Port to listen on. Overrides configuration port. Defaults to 8080
    #[arg(long,short)]
    port: Option<u16>,

    /// Port to listen on for using https. Overrides configuration port. Defaults to 4343
    #[arg(long,short)]
    tls_port: Option<u16>,

    #[arg(long,default_value="true")]
    tui: Option<bool>,

    #[arg(long)]
    enable_site: Option<Vec<String>>,

    #[arg(long)]
    update: bool,

    #[arg(long)]
    generate_example_cfg : bool
}


use serde::{Deserialize, Serialize};
use serde_json::Result as JsonResult;

use crate::configuration::{ConfigWrapper, LogLevel};
#[derive(Deserialize, Debug, Clone)]
struct Release {
    #[allow(dead_code)] html_url: Option<String>,
    tag_name: Option<String>,
}

fn update_from_github(target_tag:&str,current_version:&str) {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("OlofBlomqvist")
        .repo_name("odd-box")
        .bin_name("odd-box")
        .show_download_progress(true)
        .target_version_tag(target_tag)
        .current_version(current_version)
        .build().unwrap()
        .update().unwrap();
    println!("Update status: `{}`!", status.version());
}

async fn update() -> JsonResult<()> {
    let releases_url = "https://api.github.com/repos/OlofBlomqvist/odd-box/releases";   
    let c = reqwest::Client::new();
    let latest_release: Release = c.get(releases_url).header("user-agent", "odd-box").send()
        .await
        .expect("request failed")
        .json::<Vec<Release>>()
        .await
        .expect("failed to deserialize").iter().filter(|x|{
            if let Some(t) = &x.tag_name {
                t.to_lowercase().contains("-preview") == false
            } else {
                false
            }
        }).next().unwrap().clone();

    let current_version = cargo_crate_version!();
    let latest_tag = latest_release.tag_name.unwrap();
    if format!("v{current_version}") == latest_tag {
        println!("already running latest version: {latest_tag}");
        return Ok(())
    }
    _ = tokio::task::spawn_blocking(move || {
        update_from_github(&latest_tag,&current_version)
    }).await;

    Ok(())

}


pub fn initialize_panic_handler() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        _ = crossterm::execute!(std::io::stderr(), crossterm::event::DisableMouseCapture, crossterm::terminal::LeaveAlternateScreen);
        crossterm::terminal::disable_raw_mode().unwrap();
        eprintln!("odd-box crashed :-(");
        original_hook(panic_info);

    }));
}


fn thread_cleaner() {
    PROC_THREAD_MAP.retain(|_k,v| v.liveness_ptr.upgrade().is_some());
}


#[tokio::main(flavor="multi_thread")]
async fn main() -> anyhow::Result<()> {

    let args = Args::parse();
    let tui_flag = args.tui.unwrap_or(true);
    
    if args.update {
        _ = update().await;
        return Ok(());
    }

    initialize_panic_handler();

    let result = inner(&args).await;
    
    if tui_flag {
        use crossterm::{
            event::DisableMouseCapture,
            execute,
            terminal::{disable_raw_mode, LeaveAlternateScreen},
        };
        _ = disable_raw_mode();
        let mut stdout = std::io::stdout();
        _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    }
    
    match result {
        Ok(_) => {
            std::process::exit(0);
        },
        Err(e) => {
            println!("odd-box exited with error: {:?}",e);
            std::process::exit(1);
        }
    }



}

async fn inner(
    args:&Args
) -> anyhow::Result<()> {
    
    
    
    let (filter, reload_handle) = tracing_subscriber::reload::Layer::new(
        EnvFilter::from_default_env()
            .add_directive("h2=info".parse().expect("this directive will always work"))
            .add_directive("tokio_util=info".parse().expect("this directive will always work"))            
            .add_directive("hyper=info".parse().expect("this directive will always work")));
    

    let (tx,_) = tokio::sync::broadcast::channel::<ProcMessage>(33);


    let inner_state = AppState::new();
    let temp_cfg = ConfigWrapper::wrapv2(configuration::v2::OddBoxV2Config { 
        version: configuration::OddBoxConfigVersion::V2, 
        root_dir: None, 
        log_level: None, 
        alpn: None, 
        port_range_start: 4000, 
        default_log_format: configuration::LogFormat::standard,
        ip: None, 
        http_port: None, 
        tls_port: None, 
        auto_start: None, 
        env_vars: vec![], 
        remote_target: None, 
        hosted_process: None, 
        admin_api_port: None, 
        path: None 
    }); {

    };

    let shared_config = std::sync::Arc::new(
        tokio::sync::RwLock::new(temp_cfg)
    );

    let inner_state_arc = std::sync::Arc::new(inner_state);

    let global_state = Arc::new(crate::global_state::GlobalState { 
        app_state: inner_state_arc.clone(), 
        config: shared_config.clone(), 
        broadcaster:tx.clone(),
        target_request_counts: DashMap::new(),
        request_count: std::sync::atomic::AtomicUsize::new(0)
    });



    let tracing_broadcaster = tokio::sync::broadcast::Sender::<String>::new(10);
    
    let mut tui_thread = None;

    // tui is explicit opt out via arg only
    match args.tui {
        Some(false) => {},
        _ => {
            tui::init();
            tui_thread = Some(tokio::task::spawn(tui::run(
                global_state.clone(),
                tx.clone(), 
                tracing_broadcaster.clone(),
                filter
            )))
        },
    }

    
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let cstate = global_state.clone();
    ctrlc::set_handler(move || {
        cstate.app_state.exit.store(false, std::sync::atomic::Ordering::SeqCst);
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");



    if args.generate_example_cfg {
        let cfg = crate::configuration::v2::OddBoxV2Config::example();
        let serialized = toml::to_string_pretty(&cfg).unwrap();
        std::fs::write("odd-box-example-config.toml", serialized).unwrap();
        return Ok(())
    }

    // By default we use odd-box.toml, and otherwise we try to read from Config.toml
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

    // some times we are invoked with specific sites to run
    if let Some(sites) = &args.enable_site {
        if config.hosted_process.as_ref().map_or(true, Vec::is_empty) {
            anyhow::bail!("You have not configured any sites yet..".to_string());
        }
    
        for site in sites {
            let site_config = config.hosted_process.as_ref()
                .and_then(|processes| processes.iter().find(|x| &x.host_name == site));
    
            if site_config.is_none() {
                let allowed = config.hosted_process.as_ref().map_or(Vec::new(), |processes| 
                    processes.iter().map(|x| x.host_name.as_str()).collect::<Vec<&str>>()
                );
                if allowed.is_empty() {
                    anyhow::bail!("You have not configured any sites yet..".to_string());
                }
    
                let allowed = allowed.join(", ");
                anyhow::bail!("No such site '{site}' found in your configuration. Available sites: {allowed}");
            }
        }
    
        config.hosted_process = config.hosted_process.take().map(|processes| 
            processes.into_iter().filter(|x| sites.contains(&x.host_name)).collect()
        );
    }
    



    let log_level : LevelFilter = match config.log_level {
        Some(LogLevel::Info) => LevelFilter::INFO,
        Some(LogLevel::Error) => LevelFilter::ERROR,
        Some(LogLevel::Warn) => LevelFilter::WARN,
        Some(LogLevel::Trace) => LevelFilter::TRACE,
        Some(LogLevel::Debug) => LevelFilter::DEBUG,
        None => LevelFilter::INFO
    };

    reload_handle.reload(EnvFilter::from_default_env()
    .add_directive("h2=info".parse().expect("this directive will always work"))
    .add_directive("tokio_util=info".parse().expect("this directive will always work"))            
    .add_directive("hyper=info".parse().expect("this directive will always work")).add_directive(log_level.into())).expect("Failed to reload filter");



    
    if tui_thread.is_none() {
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
            .add_directive("hyper=info".parse().expect("this directive will always work"))
            .add_directive("h2=info".parse().expect("this directive will always work"));

        // Create a new registry and add layers
        let subscriber = tracing_subscriber::Registry::default()
            .with(fmt_layer)
            .with(filter_layer)
            .with(logging::NonTuiLoggerLayer { broadcaster: tracing_broadcaster.clone() });

        subscriber.init();
    }
   

    config.init(&cfg_path)?;

    let srv_port : u16 = if let Some(p) = args.port { p } else { config.http_port.unwrap_or(8080) } ;
    let srv_tls_port : u16 = if let Some(p) = args.tls_port { p } else { config.tls_port.unwrap_or(4343) } ;

    // Validate that we are allowed to bind prior to attempting to initialize hyper since it will simply on failure otherwise.
    for p in vec![srv_port,srv_tls_port] {
        let srv = std::net::TcpListener::bind(format!("127.0.0.1:{}",p));
        match srv {
            Err(e) => {
                anyhow::bail!("TCP Bind port {} failed. It could be taken by another service like iis,apache,nginx etc, or perhaps you do not have permission to bind. The specific error was: {e:?}",p)
            },
            Ok(_listener) => {
                tracing::debug!("TCP Port {} is available for binding.",p);
            }
        }
    }


    
    for x in config.remote_target.as_ref().unwrap_or(&vec![]) {
        inner_state_arc.site_status_map.insert(x.host_name.to_owned(), ProcState::Remote);
    }

    
    
    // todo: clean this up
    for x in config.hosted_process.clone().iter().flatten() {
        match config.resolve_process_configuration(&x) {
            Ok(x) => {
                tokio::task::spawn(proc_host::host(
                    x,
                    tx.subscribe(),
                    global_state.clone(),
                ));
            }
            Err(e) => bail!("Failed to resolve process configuration for:\n=====================================================\n{:?}.\n=====================================================\n\nThe error was: {:?}",x,e)
        }
    }
    
    
    // replace initial empty config with resolved one
    let mut cfg_write_guard = shared_config.write().await;
    *cfg_write_guard = config;
    
    drop(cfg_write_guard);

    let shared_read_guard = shared_config.read().await;

    

    let srv_ip = if let Some(ip) = shared_read_guard.ip { ip.to_string() } else { "127.0.0.1".to_string() };
    let arced_tx = std::sync::Arc::new(tx.clone());
    let shutdown_signal = Arc::new(tokio::sync::Notify::new());

    let proxy_thread = 
        tokio::spawn(crate::proxy::listen(
            shared_config.clone(),
            format!("{srv_ip}:{srv_port}").parse().expect("bind address for http must be valid.."),
            format!("{srv_ip}:{srv_tls_port}").parse().expect("bind address for https must be valid.."),
            arced_tx.clone(),
            global_state.clone(),
            shutdown_signal
        ));

    let api_port = shared_read_guard.admin_api_port.clone();
    let api_state = global_state.clone();
    let api_broadcaster = tracing_broadcaster.clone();
    

    drop(shared_read_guard);

    tokio::spawn(async move {
        api::run(api_state,api_port, api_broadcaster).await
    });

    // spawn thread cleaner and loop ever 1 second
    tokio::spawn(async move {
        loop {
            thread_cleaner();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
   
    let use_tui = tui_thread.is_some();

    if let Some(tui) = tui_thread{
        _ = tui.await;
    } else {
        loop {
            if running.load(std::sync::atomic::Ordering::SeqCst) != true {
                tracing::info!("leaving main loop");
                break;
            }
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }
    }

    {
        tracing::warn!("Changing application state to EXIT");
        global_state.app_state.exit.store(true, std::sync::atomic::Ordering::SeqCst);
    }


    

    if use_tui {
        println!("Waiting for processes to stop..");
    } else {
        tracing::info!("Waiting for processes to stop..");
    }

    while tx.receiver_count() > 0 {          
        tokio::time::sleep(Duration::from_millis(50)).await;        
    }
 
    {
        for guard in global_state.app_state.site_status_map.iter().filter(|x|x.value()!=&ProcState::Remote) {
            let (name,status) = guard.pair();
            if use_tui {
                println!("{name} ==> {status:?}")
            } else {
                tracing::info!("{name} ==> {status:?}")
            }
        }
    }

    _ = proxy_thread.abort();
    _ = proxy_thread.await.ok();

    Ok(())
}