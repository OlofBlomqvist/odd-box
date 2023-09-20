#![feature(async_closure)]
#![feature(fs_try_exists)]
mod types;
use types::*;
use std::env::args;
use std::io::Read;
use tracing::Level;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::{FmtSubscriber, EnvFilter};
use tracing_subscriber::util::SubscriberInitExt;
mod proc_host;
mod proxy;

#[tokio::main]
async fn main() -> Result<(),String> {

    let args = args().collect::<Vec<String>>();

    // By default we use odd-box.toml, and otherwise we try to read from Config.toml
    let mut cfg_path = if std::fs::try_exists("odd-box.toml").is_err() {
        "odd-box.toml"
    } else {
        "Config.toml"
    };

    // But also, if someone supplies an argument, we use that as the path to the config.
    if let Some(p) =  args.get(1) {
        cfg_path = p
    }


    let mut file = std::fs::File::open(cfg_path).map_err(|_|format!("Could not open configuration file: {cfg_path}"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|_|format!("Could not read configuration file: {cfg_path}"))?;

    let mut config: Config = toml::from_str(&contents).map_err(|e:toml::de::Error| e.message().to_owned() )?;

    let log_level : LevelFilter = match config.log_level {
        Some(LogLevel::info) => LevelFilter::INFO,
        Some(LogLevel::error) => LevelFilter::ERROR,
        Some(LogLevel::warn) => LevelFilter::WARN,
        Some(LogLevel::trace) => LevelFilter::TRACE,
        Some(LogLevel::debug) => LevelFilter::DEBUG,
        None => LevelFilter::INFO
    };
    
    let filter = EnvFilter::from_default_env()
        .add_directive(log_level.into())
        .add_directive("hyper=info".parse().expect("this directive will always work"));
    
    FmtSubscriber::builder()       
        .compact()
        .with_max_level(Level::TRACE)
        .with_env_filter(filter)
        .with_thread_names(true)
        .finish()
        .init();
    
    let resolved_home_dir_path = dirs::home_dir().ok_or(String::from("Failed to resolve home directory."))?;
    let resolved_home_dir_str = resolved_home_dir_path.to_str().ok_or(String::from("Failed to parse home directory."))?;
    tracing::info!("Resolved home directory: {}",&resolved_home_dir_str);

    if let Some(rp) = &config.root_dir {
        if rp.starts_with("~") {
            config.root_dir = Some(rp.replace("~", resolved_home_dir_str));
        }
    }

    for x in config.processes.iter_mut() {
        
        if x.path.len() < 5 { return Err(format!("Invalid path configuration for {:?}",x))}
        if x.path.starts_with("~") {
           x.path = x.path.replace("~", resolved_home_dir_str)
        }
        if x.bin.starts_with("~") {
            x.bin = x.bin.replace("~", resolved_home_dir_str)
         }
        
        if x.path.contains("$root_dir") {
            if let Some(rp) = &config.root_dir {
                x.path = x.path.replace("$root_dir", rp);
            } else {
                return Err(format!("Invalid configuration: {x:?}. Missing root_dir in configuration file but referenced for this item.."))
            }
        }

        // if no log format is specified for the process but there is a global format, override it
        if x.log_format.is_none() {
            if let Some(ref f) = config.default_log_format {
                x.log_format = Some(f.clone())
            }
        }
    }

    let srv_port = config.port.unwrap_or(80);

    // Validate that we are allowed to bind prior to attempting to initialize hyper since it will panic on failure otherwise.
    {
        let srv = std::net::TcpListener::bind(format!("127.0.0.1:{}",srv_port));
        match srv {
            Err(e) => {
                tracing::error!("TCP Bind port {} failed. It could be taken by another service like iis,apache,nginx etc, or perhaps you do not have permission to bind. The specific error was: {e:?}",srv_port);
                return Ok(())
            },
            Ok(_) => {
                tracing::debug!("TCP Port {} is available for binding.",srv_port);
            }
        }
    }

    let (tx,_) = tokio::sync::broadcast::channel::<(String,bool)>(config.processes.len());
    let sites_len = config.processes.len() as u16;
    let mut sites = vec![];

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
        sites.push(tokio::task::spawn(proc_host::host(x.clone(),tx.subscribe())))
        
    }

    proxy::rev_prox_srv(&config,&format!("127.0.0.1:{srv_port}"),tx).await?;

    Ok(())


}

