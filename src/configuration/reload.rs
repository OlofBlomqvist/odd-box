// this module is responsible for reloading the configuration file at runtime (hot-reload)

use anyhow::{Result, bail};
use std::{io::Read, sync::Arc, time::Duration};
use tracing::{info, level_filters::LevelFilter, trace, warn};
use tracing_subscriber::EnvFilter;

use crate::{
    configuration::{LogLevel, v4},
    cruma_integration::{ build_config as build_cruma_config,
    },
    global_state::{GlobalState, ProcState},
    proc_host
};

use super::{AnyOddBoxConfig, ConfigWrapper};

pub async fn reload_from_disk(global_state: Arc<GlobalState>) -> Result<()> {
    trace!("Reading configuration from disk");

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let active_configuration = { global_state.config.read().await.clone() };
    let mut file = std::fs::File::open(
        &active_configuration
            .path
            .clone()
            .ok_or(anyhow::Error::msg("cfg path not valid"))?,
    )?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    drop(file);
    let (mut new_configuration, _original_version) = match AnyOddBoxConfig::parse(&contents) {
        Ok(configuration) => {
            let (a, b, _) = configuration
                .try_upgrade_to_latest_version()
                .expect("configuration upgrade failed. this is a bug in odd-box");
            (ConfigWrapper::new(a,active_configuration.path.clone()), b)
        }
        Err(e) => anyhow::bail!(e),
    };

    new_configuration.internal_version = active_configuration.internal_version + 1;

    new_configuration.is_valid()?;


    if new_configuration.eq(&active_configuration) {
        trace!("Configuration has not changed, skipping reload");
        return Ok(());
    } else {
        warn!("Configuration has changed on disk, reloading");
    }

    // Collect all new process backends
    let mut all_cloned_new_procs: Vec<(String, v4::ProcessBackend)> = new_configuration
        .hosted_processes
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();

    // Filter out processes that are already running with same configuration - we don't need to restart them
    let cloned_modified_procs: Vec<(String, v4::ProcessBackend)> = all_cloned_new_procs
        .iter_mut()
        .filter_map(|(backend_id, new_proc_conf)| {

            // TODO: instead of just true for is_running, we need to find out if there actually is a proc_host running
            // for this backend_id.. now we just always say true and kill even when it has the current config..
            let is_running = true;


            if is_running {
                // Check if the process backend exists in active config and compare
                if let Some(active_proc) = active_configuration.hosted_processes.get(backend_id) {

                    // Compare the configs (excluding active_port since we just synced it)
                    if active_proc.value() == new_proc_conf {
                        // Config unchanged, skip restart
                        return None;
                    } else {
                        info!("Process {} has changed, will restart", backend_id);
                        return Some((backend_id.clone(), new_proc_conf.clone()));
                    }
                }
            }
            Some((backend_id.clone(), new_proc_conf.clone()))
        })
        .collect();

    // Update the hosted_processes in new_configuration with preserved active_ports
    for (backend_id, proc) in &all_cloned_new_procs {
        new_configuration
            .hosted_processes
            .insert(backend_id.clone(), proc.clone());
    }

    // Collect remotes and static backends for site status map updates
    let cloned_rems: Vec<(String, v4::RemoteBackend)> = new_configuration
        .remote_sites
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();
    let cloned_dirs: Vec<(String, v4::StaticBackend)> = new_configuration
        .static_sites
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();

    new_configuration.reload_dashmaps();

    // TODO : actually iterate through wherever we have info regarding running processes?
    for mut x in vec![crate::types::proc_info::ProcInfo {
        liveness_ptr: todo!(), backend_id: todo!(), pid: todo!(),
        marked_for_removal: todo!(), started_at_time_stamp: todo!()
    }] {
        if cloned_modified_procs
            .iter()
            .any(|(id, _)| id == &x.backend_id)
        {
            info!(
                "Marking process {} for removal as it has changed",
                x.backend_id
            );
            x.marked_for_removal = true;
        } else if new_configuration
            .hosted_processes
            .contains_key(&x.backend_id)
        {
            // Process is still in new configuration but hasn't changed
        } else {
            info!(
                "Marking process {} for removal as it is no longer in the configuration",
                x.backend_id
            );
            x.marked_for_removal = true;
        }
    }

    loop {
        {
            // TODO: actually iterate through active proc_hosts and wait for them to exit prior
            // to continuing with adding new ones
            // if crate::PROC_THREAD_MAP.iter().any(|x| x.marked_for_removal) {
            //     info!("Waiting for all marked processes to exit before starting new ones");
            //     tokio::time::sleep(Duration::from_millis(500)).await;
            //     continue;
            // }
        }
        break;
    }

    // note - we must not clear here as it would cause update event to be sent for unchanged processes
    global_state.site_status_map.retain(|k, v| {
        match v {
            // keep remotes that exist in the new config
            ProcState::Remote => cloned_rems.iter().any(|(id, _)| id == k),
            // keep dir servers and such that exist in the new config
            ProcState::DirServer => cloned_dirs.iter().any(|(id, _)| id == k),
            // keep procs -
            // all other statuses can only mean they are hosted processes
            _ => {
                if new_configuration.hosted_processes.contains_key(k) {
                    tracing::warn!("retaining proc : {k:?}");
                    true
                } else {
                    tracing::warn!("removing proc from site status map: {k:?}");
                    false
                }
            }
        }
    });

    // Add any remotes to the site list (doesn't matter if they already exist, they just get replaced)
    for (backend_id, _) in &cloned_rems {
        global_state
            .site_status_map
            .insert(backend_id.clone(), ProcState::Remote);
    }

    // Add any hosted dirs to site list (doesn't matter if they already exist, they just get replaced)
    for (backend_id, _) in &cloned_dirs {
        global_state
            .site_status_map
            .insert(backend_id.clone(), ProcState::DirServer);
    }

    // And spawn the hosted process worker loops - this will also update/re-add the site to site_status_map
    for (backend_id, proc) in cloned_modified_procs {
        match new_configuration.resolve_process_backend(&backend_id, &proc) {
            Ok(resolved) => {
                tokio::task::spawn(proc_host::ProcHost::host(
                    resolved,
                    global_state.clone(),
                ));
            }
            Err(e) => bail!(
                "Failed to resolve process configuration for:\n=====================================================\n{:?}.\n=====================================================\n\nThe error was: {:?}",
                proc,
                e
            ),
        }
    }

    // Rebuild cruma configuration to keep hosting/tunnel stack in sync with the latest config.
    let cruma_port_offset = std::env::var("ODD_BOX_CRUMA_PORT_OFFSET")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let rebuilt_cruma_config = match build_cruma_config(&new_configuration) {
        Ok((mut cfg, notes)) => {

            if !notes.unsupported.is_empty() {
                tracing::warn!(
                    "cruma config placeholders/unsupported after reload: {:?}",
                    notes.unsupported
                );
            }
            Some(cfg)
        }
        Err(e) => {
            tracing::error!(error=%e, "Failed to rebuild cruma config during reload");
            None
        }
    };

    let new_log_level = new_configuration.log_level.clone();
    let mut guard = global_state.config.write().await;
    *guard = new_configuration;
    drop(guard);

    if let Some(new_cruma_cfg) = rebuilt_cruma_config {
        global_state
            .cruma_config
            .store(std::sync::Arc::new(new_cruma_cfg));
    }

    let log_level: LevelFilter = match new_log_level {
        LogLevel::Info => LevelFilter::INFO,
        LogLevel::Error => LevelFilter::ERROR,
        LogLevel::Warn => LevelFilter::WARN,
        LogLevel::Trace => LevelFilter::TRACE,
        LogLevel::Debug => LevelFilter::DEBUG,
    };

    let what = EnvFilter::from_default_env()
        .add_directive(
            format!("odd_box={}", log_level)
                .parse()
                .expect("This directive should always work"),
        )
        .add_directive(
            "odd_box::proc_host=trace"
                .parse()
                .expect("This directive should always work"),
        );

    match &global_state.log_handle {
        crate::OddLogHandle::CLI(rw_lock) => match rw_lock.write().await.reload(what) {
            Ok(_) => {
                tracing::warn!(
                    "LOG LEVEL WAS CHANGED DUE TO CONFIGURATION FILE MODIFIED - NEW VALUE: {log_level:?}"
                )
            }
            Err(e) => {
                tracing::error!("failed to change log level due to error {e:?}")
            }
        },
        crate::OddLogHandle::TUI(rw_lock) => match rw_lock.write().await.reload(what) {
            Ok(_) => {
                tracing::warn!(
                    "LOG LEVEL WAS CHANGED DUE TO CONFIGURATION FILE MODIFIED - NEW VALUE: {log_level:?}"
                )
            }
            Err(e) => {
                tracing::error!("failed to change log level due to error {e:?}")
            }
        },
        crate::OddLogHandle::None => {
            tracing::error!("NO LOG HANDLE EXISTS!!")
        }
    };

    info!("Configuration reloaded successfully.");
    Ok(())
}
