// this module is responsible for reloading the configuration file at runtime (hot-reload)

use std::{io::Read, sync::Arc, time::Duration};
use anyhow::{bail, Result};
use tracing::{info, trace, warn};

use crate::{configuration::v2::{DirServer, InProcessSiteConfig, RemoteSiteConfig}, global_state::GlobalState, proc_host, types::app_state::ProcState};

use super::{ConfigWrapper, OddBoxConfig};

pub async fn reload_from_disk(global_state: Arc<GlobalState>) -> Result<()> {
 
    trace!("Reading configuration from disk");

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let active_configuration = { global_state.config.read().await.clone() };
    let mut file = std::fs::File::open(&active_configuration.path.clone().ok_or(anyhow::Error::msg("cfg path not valid"))?)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;    
    drop(file);
    let (mut new_configuration,_original_version) = 
        match OddBoxConfig::parse(&contents) {
            Ok(configuration) => {
                let (a,b) = 
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
    let cloned_modified_procs : Vec<InProcessSiteConfig> = all_cloned_new_procs.iter_mut().filter_map(|x|{
        let existing = crate::PROC_THREAD_MAP.iter().find(|y|y.config.host_name == x.host_name);
        if let Some(existing) = existing {
            if let Ok(mut r) = new_configuration.resolve_process_configuration(&x) {
                r.proc_id = existing.config.proc_id.clone();
                r.active_port = existing.config.active_port;
                x.set_id(r.proc_id.clone());
                if existing.config.eq(&r) {
                    warn!("Process {} has not changed, skipping restart",x.host_name);
                    return None;
                } else {
                    info!("Process {} has changed, will restart",x.host_name);
                    return Some(x.clone());
                }
            } else {

                warn!("Failed to resolve process configuration for process {}",x.host_name);
                abort = true;
                return None;
            }
        }
        return Some(x.clone());
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
            info!("Process {} is still in the new configuration, but has not changed",x.config.host_name);
        } else {
            info!("Marking process {} for removal as it is no longer in the configuration",x.config.host_name);
            x.marked_for_removal = true;   
        }
         
    }

    loop {
        {
            if crate::PROC_THREAD_MAP.iter().any(|x|x.marked_for_removal) {
                info!("Waiting for all processes to exit before starting new ones");
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        }
        break;
    }

   
    global_state.app_state.site_status_map.clear();

    // Add any remotes to the site list
    for x in cloned_rems {
        global_state.app_state.site_status_map.insert(x.host_name.to_owned(), ProcState::Remote);
    }

    // Add any hosted dirs to site list
    for x in cloned_dirs {
        global_state.app_state.site_status_map.insert(x.host_name.to_owned(), ProcState::Dynamic);
    }

   
    // And spawn the hosted process worker loops
    for x in cloned_modified_procs {
        match new_configuration.resolve_process_configuration(&x) {
            Ok(x) => {
                tokio::task::spawn(proc_host::host(
                    x,
                    global_state.broadcaster.subscribe(),
                    global_state.clone(),
                ));
            }
            Err(e) => bail!("Failed to resolve process configuration for:\n=====================================================\n{:?}.\n=====================================================\n\nThe error was: {:?}",x,e)
        }
    }


    let mut guard = global_state.config.write().await;
    *guard = new_configuration;
    drop(guard);



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