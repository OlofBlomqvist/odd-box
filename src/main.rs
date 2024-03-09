#![feature(async_closure)]
#![feature(fs_try_exists)]
#![feature(let_chains)]
#![feature(ascii_char)]
#![feature(impl_trait_in_assoc_type)]
#![feature(fn_traits)]

mod configuration;
mod types;
mod tcp_proxy;
mod http_proxy;
mod proxy;
use http_proxy::ProcMessage;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use self_update::cargo_crate_version;
use std::{borrow::BorrowMut, sync::Mutex};
use std::time::Duration;
use types::custom_error::*;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;
mod proc_host;
use tracing_subscriber::util::SubscriberInitExt;
use crate::types::app_state::ProcState;
mod tui;

use types::app_state::AppState;


#[derive(Debug)]
struct DynamicCertResolver {
    cache: Mutex<HashMap<String, Arc<rustls::sign::CertifiedKey>>>,
}

use rustls::server::{ClientHello, ResolvesServerCert};

impl ResolvesServerCert for DynamicCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<std::sync::Arc<rustls::sign::CertifiedKey>> {
        
        let server_name = client_hello.server_name()?;
        
        {
            let cache = self.cache.lock().expect("should always be able to read cert cache");
            if let Some(certified_key) = cache.get(server_name) {
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
            tracing::error!("Failed to generate certificate for {}! - Error reason: {}", server_name, e);
            return None
        }

        let cert_chain = if let Ok(c) = my_certs(&cert_path) {
            c
        } else {
            tracing::error!("failed to read cert: {cert_path}");
            return None
        };

        if cert_chain.is_empty() {
            tracing::warn!("EMPTY CERT CHAIN FOR {}",server_name);
            return None
        }
       
        if let Ok(private_key) = my_rsa_private_keys(&key_path) {
            if let Ok(rsa_signing_key) = rustls::crypto::ring::sign::any_supported_type(&private_key) {
                Some(std::sync::Arc::new(rustls::sign::CertifiedKey::new(
                    cert_chain, 
                    rsa_signing_key
                )))

            } else {
                tracing::error!("rustls::crypto::ring::sign::any_supported_type - failed to read cert: {cert_path}");
                None
            }
        } else {
            tracing::error!("my_rsa_private_keys - failed to read cert: {cert_path}");
            None
        }
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
        .collect::<Result<Vec<rustls::pki_types::PrivatePkcs8KeyDer>,_>>().map_err(|e|format!("{e:?}"))?;

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


use serde::Deserialize;
use serde_json::Result as JsonResult;

use crate::configuration::{ConfigWrapper, EnvVar, LogLevel};
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
        .expect("failed to deserialize").first().unwrap().clone();
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

#[tokio::main(flavor="multi_thread")]
async fn main() -> Result<(),String> {
    
    initialize_panic_handler();

    let args = Args::parse();

    if args.update {
        _ = update().await;
        return Ok(());
    }

    if args.generate_example_cfg {
        let cfg = crate::configuration::v1::example_v1();
        let serialized = toml::to_string_pretty(&cfg).unwrap();
        std::fs::write("odd-box-example-config.toml", serialized).unwrap();
        return Ok(())
    }

    // By default we use odd-box.toml, and otherwise we try to read from Config.toml
    let cfg_path = 
        if let Some(cfg) = args.configuration {
            cfg
        } else {
            if std::fs::try_exists("odd-box.toml").is_ok() {
                "odd-box.toml".to_owned()
            } else if std::fs::try_exists("oddbox.toml").is_ok() {
                "oddbox.toml".to_owned()
            } else {
                "Config.toml".to_owned()
            }
        };


    let mut file = std::fs::File::open(&cfg_path).map_err(|_|format!("Could not open configuration file: {cfg_path}"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|_|format!("Could not read configuration file: {cfg_path}"))?;

    let mut config: ConfigWrapper = 
        ConfigWrapper(match configuration::Config::parse(&contents) {
            Ok(configuration::Config::V1(configuration)) => {
                Ok(configuration)
            },
            Ok(old_config) => {
                eprintln!("Warning: you are using a legacy style configuration file, it will be automatically updated");
                match old_config.try_upgrade() {
                    Ok(configuration::Config::V1(configuration)) => {       
                              
                        configuration.write_to_disk(&cfg_path);
                        Ok(configuration)
                    },
                    Ok(_) => return Err(format!("Unable to update the configuration file to new schema")),
                    Err(e) => return Err(e),
                }
            },
            Err(e) => Err(e),
        }?);
    
    config.is_valid()?;

    // some times we are invoked with specific sites to run
    if let Some(sites) = args.enable_site {
        if config.hosted_process.as_ref().map_or(true, Vec::is_empty) {
            return Err("You have not configured any sites yet..".to_string());
        }
    
        for site in &sites {
            let site_config = config.hosted_process.as_ref()
                .and_then(|processes| processes.iter().find(|x| &x.host_name == site));
    
            if site_config.is_none() {
                let allowed = config.hosted_process.as_ref().map_or(Vec::new(), |processes| 
                    processes.iter().map(|x| x.host_name.as_str()).collect::<Vec<&str>>()
                );
                if allowed.is_empty() {
                    return Err("You have not configured any sites yet..".to_string());
                }
    
                let allowed = allowed.join(", ");
                return Err(format!("No such site '{site}' found in your configuration. Available sites: {allowed}"));
            }
        }
    
        config.hosted_process = config.hosted_process.take().map(|processes| 
            processes.into_iter().filter(|x| sites.contains(&x.host_name)).collect()
        );
    }
    



    let log_level : LevelFilter = match config.log_level {
        Some(LogLevel::info) => LevelFilter::INFO,
        Some(LogLevel::error) => LevelFilter::ERROR,
        Some(LogLevel::warn) => LevelFilter::WARN,
        Some(LogLevel::trace) => LevelFilter::TRACE,
        Some(LogLevel::debug) => LevelFilter::DEBUG,
        None => LevelFilter::INFO
    };

    let use_tui = args.tui.unwrap_or_default();
    
    if !use_tui {
        tracing_subscriber::FmtSubscriber::builder()       
            .compact()
            .with_max_level(tracing::Level::TRACE)
            .with_env_filter(EnvFilter::from_default_env()
            .add_directive(log_level.into())
            .add_directive("hyper=info".parse().expect("this directive will always work"))
            .add_directive("h2=info".parse().expect("this directive will always work")))
            .with_thread_names(true)
            .with_timer(
                tracing_subscriber::fmt::time::OffsetTime::new(
                    time::UtcOffset::from_whole_seconds(
                        chrono::Local::now().offset().local_minus_utc()
                    ).expect("time... works"), 
                    time::macros::format_description!("[hour]:[minute]:[second]")
                )
            )
            .finish()
            .init();
    }
    


    config.init(&cfg_path)?;

    let srv_port : u16 = if let Some(p) = args.port { p } else { config.http_port.unwrap_or(8080) } ;
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

    let sites_len = config.hosted_process.as_ref().and_then(|x|Some(x.len())).unwrap_or_default() as u16;

    let (tx,_) = tokio::sync::broadcast::channel::<ProcMessage>(sites_len.max(1).into());

    
    
    let mut sites = vec![];
    let mut inner_state = AppState::new();
    for x in config.remote_target.as_ref().unwrap_or(&vec![]) {
        inner_state.procs.insert(x.host_name.to_owned(), ProcState::Remote);
    }
    let shared_state : Arc<tokio::sync::RwLock<AppState>> = 
        std::sync::Arc::new(
            tokio::sync::RwLock::new(
                inner_state
            )
        );
    
    let global_auto_start_default_value = config.auto_start.clone();
    let port_range_start = config.port_range_start.clone();
    let global_env_vars = config.env_vars.clone();

    let mut_procs = config.hosted_process.borrow_mut();
    if let Some(processes) = mut_procs {
        for (i, x) in processes.iter_mut().enumerate() {
            
            let auto_port = port_range_start + i as u16;
            // if a custom PORT variable is set, we use it
            if let Some(cp) = x.env_vars.iter().find(|x| x.key.to_lowercase() == "port") {
                let custom_port = cp.value.parse::<u16>()
                    .map_err(|e| format!("Invalid port configured for {}. {:?}", &x.host_name, e))?;
                
                if custom_port > sites_len {
                    tracing::warn!("Using custom port for {} as specified in configuration! ({})", x.host_name, custom_port);
                    x.set_port(custom_port);
                } else if custom_port < 1 {
                    return Err(format!("Invalid port configured for {}: {}.", x.host_name, cp.value));
                } else {
                    return Err(format!("Invalid port configured for {}: {}. Please use a port number above {}", 
                        x.host_name, cp.value, sites_len + port_range_start));
                }
            // otherwise we assign one from the auto range
            } else {
                x.set_port(auto_port);
                x.env_vars.push(EnvVar { key: "PORT".to_string(), value: auto_port.to_string() });
            }

    
            tracing::trace!("Initializing {} on port {:?}", x.host_name, x.port);
            x.env_vars = [global_env_vars.clone(), x.env_vars.clone()].concat();
    
            if x.auto_start.is_none() {
                x.auto_start = global_auto_start_default_value;
            }
    
            sites.push(tokio::task::spawn(proc_host::host(
                x.clone(),
                tx.subscribe(),
                shared_state.clone()
            )));
        }
    }

    let srv_ip = if let Some(ip) = config.ip { ip.to_string() } else { "127.0.0.1".to_string() };
    let arced_tx = std::sync::Arc::new(tx.clone());
    let shutdown_signal = Arc::new(tokio::sync::Notify::new());

    let proxy_thread = 
        tokio::spawn(crate::proxy::listen(
            config.clone(),
            format!("{srv_ip}:{srv_port}").parse().expect("bind address for http must be valid.."),
            format!("{srv_ip}:{srv_tls_port}").parse().expect("bind address for https must be valid.."),
            arced_tx.clone(),
            shared_state.clone(),
            shutdown_signal
        ));
       

    if use_tui {
        tui::init();
        tui::run(EnvFilter::from_default_env()
            .add_directive(log_level.into())
            .add_directive("h2=info".parse().expect("this directive will always work"))
            .add_directive("tokio_util=info".parse().expect("this directive will always work"))            
            .add_directive("hyper=info".parse().expect("this directive will always work")),shared_state.clone(),tx.clone()).await;
    } else {
       let mut stdin = std::io::stdin();
       let mut buf : [u8;1] = [0;1];
       loop {
            _ = stdin.read_exact(&mut buf);
            if buf[0] == 3 || buf[0] == 113 {
                tracing::info!("bye: {:?}",buf);
                break;
            } else {
                tracing::info!("press 'q' or ctrl-c to quit, not {:?}",buf);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            
       }
    }

    {
        tracing::warn!("Changing application state to EXIT");
        let mut state = shared_state.write().await;
        state.exit = true;
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
        let state = shared_state.read().await;
            for (name,status) in state.procs.iter().filter(|x|x.1!=&ProcState::Remote) {
                if use_tui {
                    println!("{name} ==> {status:?}")
                } else {
                    tracing::info!("{name} ==> {status:?}")
                }
            }
    }

    if use_tui {
        println!("Performing cleanup, please wait..");
                
        use crossterm::{
            event::DisableMouseCapture,
            execute,
            terminal::{disable_raw_mode, LeaveAlternateScreen},
        };
        _ = disable_raw_mode();
        let mut stdout = std::io::stdout();
        _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    } else {
        tracing::info!("Performing cleanup, please wait..");
    } 
    
    _ = proxy_thread.abort();
    _ = proxy_thread.await.ok();

    Ok(())


}


