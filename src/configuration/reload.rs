// this module is responsible for reloading the configuration file at runtime (hot-reload)

use std::{io::Read, sync::Arc, time::Duration};
use anyhow::{bail, Result};
use tracing::{info, level_filters::LevelFilter, trace, warn};
use tracing_subscriber::EnvFilter;

use crate::{configuration::{DirServer, InProcessSiteConfig, RemoteSiteConfig, LogLevel}, global_state::GlobalState, proc_host, types::app_state::ProcState};

use super::{ConfigWrapper, AnyOddBoxConfig};

pub async fn reload_from_disk(global_state: Arc<GlobalState>) -> Result<()> {
 
    trace!("Reading configuration from disk");

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let active_configuration = { global_state.config.read().await.clone() };
    let mut file = std::fs::File::open(&active_configuration.path.clone().ok_or(anyhow::Error::msg("cfg path not valid"))?)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;    
    drop(file);
    let (mut new_configuration,_original_version) = 
        match AnyOddBoxConfig::parse(&contents) {
            Ok(configuration) => {
                let (a,b,_) = 
                    configuration
                        .try_upgrade_to_latest_version()
                        .expect("configuration upgrade failed. this is a bug in odd-box");
                (ConfigWrapper::new(a),b)
            },
            Err(e) => anyhow::bail!(e),
        };
    
    new_configuration.internal_version = active_configuration.internal_version + 1;


    new_configuration.is_valid()?;
    new_configuration.set_disk_path(&active_configuration.path.clone().ok_or(anyhow::Error::msg("cfg path not valid"))?)?;
    
    if new_configuration.eq(&active_configuration) {
        trace!("Configuration has not changed, skipping reload");
        return Ok(());
    } else {
        warn!("Configuration has changed on disk, reloading");
    }

    let has_changed_le_mail = new_configuration.lets_encrypt_account_email != active_configuration.lets_encrypt_account_email;
    

    let mut all_cloned_new_procs : Vec<InProcessSiteConfig> = new_configuration.hosted_process.iter().flatten().cloned().collect();
    let mut abort = false;

    // lets filter out any processes that are already running and have the same configuration..
    // we dont need to restart them..
    let cloned_modified_procs : Vec<InProcessSiteConfig> = all_cloned_new_procs.iter_mut().filter_map(|new_proc_conf|{
        let existing = crate::PROC_THREAD_MAP.iter().find(|y|y.config.host_name == new_proc_conf.host_name);
        if let Some(existing) = existing {
            if let Ok(mut new_resolved_siteproc) = new_configuration.resolve_process_configuration(&new_proc_conf) {
                new_resolved_siteproc.proc_id = existing.config.proc_id.clone();
                new_resolved_siteproc.active_port = existing.config.active_port;
                new_proc_conf.active_port = existing.config.active_port;
                new_proc_conf.set_id(new_resolved_siteproc.proc_id.clone());

                // todo - could have better checks here.. in some cases we do not actually need to restart the proc host

                if existing.config.eq(&new_resolved_siteproc) {
                    //trace!("Process {} has not changed, skipping restart",x.host_name);
                    return None;
                } else {
                    info!("Process {} has changed, will restart",new_proc_conf.host_name);
                    return Some(new_proc_conf.clone());
                }
            } else {

                warn!("Failed to resolve process configuration for process {}",new_proc_conf.host_name);
                abort = true;
                return None;
            }
        }
        return Some(new_proc_conf.clone());
    }).collect();

    new_configuration.hosted_process = Some(all_cloned_new_procs);

    if abort {
        bail!("Failed to resolve process configuration for some processes");
    }

    let cloned_rems : Vec<RemoteSiteConfig> = new_configuration.remote_target.iter().flatten().cloned().collect();
    let cloned_dirs : Vec<DirServer> = new_configuration.dir_server.iter().flatten().cloned().collect();
    
    new_configuration.reload_dashmaps();

    for mut x in crate::PROC_THREAD_MAP.iter_mut() {
        if cloned_modified_procs.iter().any(|p|p.host_name==x.config.host_name) {
            info!("Marking process {} for removal as it has changed",x.config.host_name);
            x.marked_for_removal = true; 
        } else if new_configuration.hosted_process.iter().flatten().any(|p|p.host_name==x.config.host_name) {
            // ok so this process is still in the new configuration, but it has not changed
            //info!("Process {} is still in the new configuration, but has not changed",x.config.host_name);
        } else {
            info!("Marking process {} for removal as it is no longer in the configuration",x.config.host_name);
            x.marked_for_removal = true;   
        }
         
    }

    loop {
        {
            if crate::PROC_THREAD_MAP.iter().any(|x|x.marked_for_removal) {
                info!("Waiting for all marked processes to exit before starting new ones");
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        }
        break;
    }

   
    // note - we must not clear here as it would cause update event to be sent for unchanged processes
    global_state.app_state.site_status_map.retain(|k,v|{
        match v {
            // keep remotes that exist in the new config
            ProcState::Remote => cloned_rems.iter().find(|y|y.host_name==*k).is_some(),
            // keep dir servers and such that exist in the new config
            ProcState::DirServer => cloned_dirs.iter().find(|y|y.host_name==*k).is_some(),
            // keep procs -
            // all other statuses can only mean they are hosted processes
            _ => if new_configuration.hosted_processes.contains_key(k) {
                tracing::warn!("retaining proc : {k:?}");
                true
            } else {
                tracing::warn!("removing proc from site status map: {k:?}");
                false
            }
        }
    });

    // Add any remotes to the site list (doesnt matter if the already exist, they just get replaced)
    for x in cloned_rems {
        global_state.app_state.site_status_map.insert(x.host_name.to_owned(), ProcState::Remote);
    }

    // Add any hosted dirs to site list (doesnt matter if the already exist, they just get replaced)
    for x in cloned_dirs {
        global_state.app_state.site_status_map.insert(x.host_name.to_owned(), ProcState::DirServer);
    }

   
    // And spawn the hosted process worker loops - this will also update/re-add the site to site_status_map
    for x in cloned_modified_procs {
        match new_configuration.resolve_process_configuration(&x) {
            Ok(x) => {
                tokio::task::spawn(proc_host::host(
                    x,
                    global_state.proc_broadcaster.subscribe(),
                    global_state.clone(),
                ));
            }
            Err(e) => bail!("Failed to resolve process configuration for:\n=====================================================\n{:?}.\n=====================================================\n\nThe error was: {:?}",x,e)
        }
    }

    let new_log_level = new_configuration.log_level.clone();
    let mut guard = global_state.config.write().await;
    *guard = new_configuration;
    drop(guard);

    
    let log_level : LevelFilter = match new_log_level {
        Some(LogLevel::Info) => LevelFilter::INFO,
        Some(LogLevel::Error) => LevelFilter::ERROR,
        Some(LogLevel::Warn) => LevelFilter::WARN,
        Some(LogLevel::Trace) => LevelFilter::TRACE,
        Some(LogLevel::Debug) => LevelFilter::DEBUG,
        _ => LevelFilter::INFO
    };

    let what = EnvFilter::from_default_env()
        .add_directive(format!("odd_box={}", log_level).parse().expect("This directive should always work"))
        .add_directive("odd_box::proc_host=trace".parse().expect("This directive should always work"));

    match &global_state.log_handle {
        crate::OddLogHandle::CLI(rw_lock) => {
            match rw_lock.write().await.reload(what) {
                Ok(_) => {
                    tracing::warn!("LOG LEVEL WAS CHANGED DUE TO CONFIGURATION FILE MODIFIED - NEW VALUE: {log_level:?}")
                },
                Err(e) => {
                    tracing::error!("failed to change log level due to error {e:?}")
                },
            }
        },
        crate::OddLogHandle::TUI(rw_lock) => {
            match rw_lock.write().await.reload(what) {
                Ok(_) => {
                    tracing::warn!("LOG LEVEL WAS CHANGED DUE TO CONFIGURATION FILE MODIFIED - NEW VALUE: {log_level:?}")
                },
                Err(e) => {
                    tracing::error!("failed to change log level due to error {e:?}")
                },
            }
        },
        crate::OddLogHandle::None => {
            tracing::error!("NO LOG HANDLE EXISTS!!")
        },
    };



    if has_changed_le_mail {
        if active_configuration.lets_encrypt_account_email.is_some() {
            global_state.cert_resolver.enable_lets_encrypt();
        } else {
            global_state.cert_resolver.disable_lets_encrypt();
        }
    }

    
    info!("Configuration reloaded successfully, invalidading caches.");
    global_state.invalidate_cache();
    info!("Configuration swap complete - caches invalidated.");
    Ok(())
}