#![feature(async_closure)]
#![feature(fs_try_exists)]
#![feature(let_chains)]
mod types;
mod hyper_reverse_proxy;
use rustls::PrivateKey;
use self_update::cargo_crate_version;
use std::sync::Mutex;
use std::time::Duration;
use types::*;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;
mod proc_host;
mod proxy;

use tracing_subscriber::util::SubscriberInitExt;

#[cfg(feature = "TUI")]
mod tui;

#[cfg(feature = "TUI")]
use tui::AppState;


#[cfg(not(feature = "TUI"))]
struct AppState {
    procs: HashMap<String,ProcState>
}
#[cfg(not(feature = "TUI"))]
impl AppState {
    pub fn new() -> Self {
        Self {
            procs: HashMap::new()
        }
    }
}

struct DynamicCertResolver {
    cache: Mutex<HashMap<String, Arc<rustls::sign::CertifiedKey>>>,
}

use rustls::sign::any_supported_type;

use rustls::server::{ClientHello, ResolvesServerCert};

impl ResolvesServerCert for DynamicCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<std::sync::Arc<rustls::sign::CertifiedKey>> {
        
        let server_name = client_hello.server_name()?;
        
        {
            let cache = self.cache.lock().unwrap();
            if let Some(certified_key) = cache.get(server_name) {
                return Some(certified_key.clone());
            }
        }

        let odd_cache_base = ".odd_box_cache";

        let base_path = std::path::Path::new(odd_cache_base);
        let host_name_cert_path = base_path.join(server_name);
    
        if let Err(e) = std::fs::create_dir_all(&host_name_cert_path) {
            eprintln!("Could not create directory: {:?}", e);
            return None;
        }

        let cert_path = format!("{}/{}/cert.pem",odd_cache_base,server_name);
        let key_path = format!("{}/{}/key.pem",odd_cache_base,server_name);

        if let Err(e) = generate_cert_if_not_exist(server_name, &cert_path, &key_path) {
            tracing::error!("Failed to generate certificate for {}! - Error reason: {}", server_name, e);
            return None
        }

        let cert_chain = my_certs(&cert_path).unwrap();

        if cert_chain.is_empty() {
            tracing::warn!("EMPTY CERT CHAIN FOR {}",server_name);
            return None
        }
       
        let private_key = my_rsa_private_keys(&key_path).unwrap();
        
        let rsa_signing_key = any_supported_type(&private_key).unwrap();

        // let rsa_signing_key = 
        //     RsaSigningKey::new(&private_key).map_err(|e| {
        //         tracing::warn!("{}",e.to_string());
        //         std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to create signing key")
        //     }).unwrap();

        Some(std::sync::Arc::new(rustls::sign::CertifiedKey::new(
            cert_chain, 
            rsa_signing_key
        )))
    }
}

use std::io::BufReader;
use std::fs::File;


fn generate_cert_if_not_exist(hostname: &str, cert_path: &str,key_path: &str) -> Result<(),String> {
    
    let crt_exists = std::fs::try_exists(cert_path).unwrap_or_default();
    let key_exists = std::fs::try_exists(key_path).unwrap_or_default();

    if crt_exists && key_exists {
        tracing::info!("Using existing certificate for {}",hostname);
        return Ok(())
    }
    
    if crt_exists != key_exists {
        return Err(String::from("Missing key or crt for this hostname. Remove both if you want to generate a new set, or add the missing one."))
    }

    tracing::info!("Generating new certificate for site '{}'",hostname);
    

    match rcgen::generate_simple_self_signed(
        vec![hostname.to_owned()]
    ) {
        Ok(cert) => {
            match cert.serialize_pem() {
                Ok(contents) => {
                    tracing::info!("Generating new self-signed certificate for host '{}'!",hostname);
                    let _ = std::fs::write(&cert_path, contents);
                    let _ = std::fs::write(&key_path, &cert.serialize_private_key_pem());
                    Ok(())
                } 
                Err(e) => Err(e.to_string())
            }
        },
        Err(e) => Err(e.to_string())
    }
}

fn my_certs(path: &str) -> Result<Vec<rustls::Certificate>, std::io::Error> {
    let cert_file = File::open(path)?;
    let mut reader = BufReader::new(cert_file);
    let certs = rustls_pemfile::certs(&mut reader).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to load certificate"))?;
    Ok(certs.into_iter().map(rustls::Certificate).collect())
}

fn my_rsa_private_keys(path: &str) -> Result<PrivateKey, String> {

    let file = File::open(&path).unwrap();
    let mut reader = BufReader::new(file);
    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut reader).unwrap();

    match keys.len() {
        0 => Err(format!("No PKCS8-encoded private key found in {path}").into()),
        1 => Ok(PrivateKey(keys.remove(0))),
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

    #[cfg(feature = "TUI")]
    #[arg(long,default_value="true")]
    tui: Option<bool>,

    #[arg(long)]
    enable_site: Option<Vec<String>>,

    #[arg(long)]
    update: bool,
}

#[derive(Debug,PartialEq,Clone)]
pub enum ProcState {
    Faulty,
    Stopped,    
    Starting,
    Stopping,
    Running
}

use reqwest;
use serde::Deserialize;
use serde_json::Result as JsonResult;
#[derive(Deserialize, Debug, Clone)]
struct Release {
    html_url: Option<String>,
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
        .expect("failed to deserialize").first().unwrap().clone();
    let current_version = cargo_crate_version!();
    let latest_tag = latest_release.tag_name.unwrap();
    if format!("v{current_version}") == latest_tag {
        println!("already running latest version: {latest_tag}");
        return Ok(())
    }
    tokio::task::spawn_blocking(move || {
        update_from_github(&latest_tag,&current_version)
    }).await;

    Ok(())

}

#[tokio::main]
async fn main() -> Result<(),String> {
    
    
    let args = Args::parse();

    if args.update {
        update().await;
        return Ok(());
    }


    // By default we use odd-box.toml, and otherwise we try to read from Config.toml
    let cfg_path = 
        if let Some(cfg) = args.configuration {
            cfg
        } else {
            if std::fs::try_exists("odd-box.toml").is_ok() {
                "odd-box.toml".to_owned()
            } else {
                "Config.toml".to_owned()
            }
        };


    let mut file = std::fs::File::open(&cfg_path).map_err(|_|format!("Could not open configuration file: {cfg_path}"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|_|format!("Could not read configuration file: {cfg_path}"))?;

    let mut config: Config = toml::from_str(&contents).map_err(|e:toml::de::Error| e.message().to_owned() )?;

    if let Some(sites) = args.enable_site {
        for site in &sites {
            let site_config = config.processes.iter().find(|x|&x.host_name==site);
            if site_config.is_none() {
                let allowed =  config.processes.iter().map(|x|x.host_name.as_str()).collect::<Vec<&str>>();
                if allowed.len() == 0 {
                    return Err(format!("You have not configured any sites yet.."))
                }
                let allowed = allowed.join(", ");
                return Err(format!("No such site '{site}' found in your configuration. Available sites: {allowed}"))
            }
        }
        config.processes = config.processes.into_iter().filter(|x:&SiteConfig| {
            sites.contains(&x.host_name)
        }).collect()
    }


    let log_level : LevelFilter = match config.log_level {
        Some(LogLevel::info) => LevelFilter::INFO,
        Some(LogLevel::error) => LevelFilter::ERROR,
        Some(LogLevel::warn) => LevelFilter::WARN,
        Some(LogLevel::trace) => LevelFilter::TRACE,
        Some(LogLevel::debug) => LevelFilter::DEBUG,
        None => LevelFilter::INFO
    };

    
    #[cfg(not(feature = "TUI"))]
    let use_tui = false;
    
    #[cfg(feature = "TUI")]
    let use_tui = if args.tui.unwrap_or_default() {
        tui::init();
        true
    } else {
        false
    };
    
    if !use_tui {
        tracing_subscriber::FmtSubscriber::builder()       
            .compact()
            .with_max_level(tracing::Level::TRACE)
            .with_env_filter(EnvFilter::from_default_env()
            .add_directive(log_level.into())
            .add_directive("hyper=info".parse().expect("this directive will always work")))
            .with_thread_names(true)
            .with_timer(
                tracing_subscriber::fmt::time::OffsetTime::new(
                    time::UtcOffset::from_whole_seconds(
                        chrono::Local::now().offset().local_minus_utc()
                    ).unwrap(), 
                    time::macros::format_description!("[hour]:[minute]:[second]")
                )
            )
            .finish()
            .init();
    }
    


    config.init(&cfg_path)?;

    let srv_port : u16 = if let Some(p) = args.port { p } else { config.port.unwrap_or(8080) } ;
    let srv_tls_port : u16 = if let Some(p) = args.tls_port { p } else { config.tls_port.unwrap_or(4343) } ;

    // Validate that we are allowed to bind prior to attempting to initialize hyper since it will simply on failure otherwise.
    for p in vec![srv_port,srv_tls_port] {
        let srv = std::net::TcpListener::bind(format!("127.0.0.1:{}",p));
        match srv {
            Err(e) => {
                return Err(format!("TCP Bind port {} failed. It could be taken by another service like iis,apache,nginx etc, or perhaps you do not have permission to bind. The specific error was: {e:?}",p))
            },
            Ok(_listener) => {
                tracing::debug!("TCP Port {} is available for binding.",p);
            }
        }
    }

    let (tx,_) = tokio::sync::broadcast::channel::<(String,bool)>(config.processes.len());

    let sites_len = config.processes.len() as u16;
    
    let mut sites = vec![];
    
    let shared_state : Arc<tokio::sync::Mutex<AppState>> = 
        std::sync::Arc::new(
            tokio::sync::Mutex::new(
                AppState::new()
            )
        );
        

    for (i,x) in config.processes.iter_mut().enumerate() {
        let auto_port = config.port_range_start + i as u16;
        

        if let Some(cp) = x.env_vars.iter().find(|x|x.key.to_lowercase()=="port"){
            let custom_port = cp.value.parse::<u16>().map_err(|e|format!("Invalid port configured for {}. {e:?}",&x.host_name))?;
            if custom_port > sites_len {                
                tracing::warn!("Using custom port for {} as specified in configuration! ({})", x.host_name,custom_port);
                x.set_port(custom_port as u16);
            }
            else if custom_port < 1 {
                return Err(format!("Invalid port configured for {}: {}.", x.host_name,cp.value))
            }
            else {
                return Err(format!("Invalid port configured for {}: {}. Please use a port number above {}", 
                    x.host_name,cp.value, (sites_len + config.port_range_start)))
            }
        } else {
            x.set_port(auto_port);
            x.env_vars.push(EnvVar { key: String::from("PORT"), value: auto_port.to_string() });
        }

        tracing::trace!("Initializing {} on port {}",x.host_name,x.port);
        x.env_vars = [ config.env_vars.clone(), x.env_vars.clone() ].concat();
        sites.push(tokio::task::spawn(
            proc_host::host(
                x.clone(),
                tx.subscribe(),
                shared_state.clone()
            ))
        )
        
    }

    let arced_tx = std::sync::Arc::new(tx.clone());
    let child = tokio::spawn(proxy::rev_prox_srv(
        config,
        format!("127.0.0.1:{srv_port}"),
        format!("127.0.0.1:{srv_tls_port}"),
        arced_tx,
        shared_state.clone()
    ));


    if use_tui {
        #[cfg(feature="TUI")]
        tui::run(EnvFilter::from_default_env()
            .add_directive(log_level.into())
            .add_directive("hyper=info".parse().expect("this directive will always work")),shared_state.clone(),tx.clone()).await;
    } else {
        use tokio::task;
        
                
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

        #[cfg(feature="device_query")] {
            use device_query::{DeviceQuery, DeviceState, Keycode};
            let r2 = running.clone();
            let t2 = tx.clone();
            task::spawn(async move {
                while r2.load(std::sync::atomic::Ordering::SeqCst) {
                    let keys = { DeviceState::new().get_keys() };
                    if keys.contains(&Keycode::Q)  || keys.contains(&Keycode::Escape) || (keys.contains(&Keycode::C) && keys.contains(&Keycode::LControl)) {
                        _ = t2.send(("exit".to_owned(),false)).ok();
                        r2.store(false, std::sync::atomic::Ordering::SeqCst);
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }).await.unwrap();
        }

        while running.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

    }

    _ = tx.send(("exit".to_owned(),false));

    let s = shared_state.clone();
    while tx.receiver_count() > 0 {

        let state = s.lock().await;
        let mut nrwaiting = 0;
        for (name,state) in &state.procs {
            if state == &ProcState::Running {
                
                if use_tui {
                    println!(">>> Waiting for {} to stop..",name);
                } else {
                    tracing::debug!("Waiting for {} to stop..",name);
                }

                nrwaiting += 1;
            }
        }
        if nrwaiting == 0 {
            break
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
        
    }
    let state = shared_state.lock().await;
    for (p,s) in state.procs.iter() {

        if use_tui {
            println!(">> {} --> {:?}", p,s);
        } else {
            tracing::debug!("{} --> {:?}",p,s);
        }        
    
    }


    if tx.receiver_count() == 0 {
        if use_tui {
            println!("Something seems to have gone wrong - there may be child-processes still running even after exiting odd-box..");
        } else {
            tracing::error!("Something seems to have gone wrong - there may be child-processes still running even after exiting odd-box..");
        }        
    } else {

        if use_tui {        
            println!(">>> Graceful shutdown successful!");
        } else {
            tracing::debug!("Graceful shutdown successful!");
        }    
    }

    
    _ = child.abort();
    _ = child.await.ok();
    
    
    Ok(())


}


