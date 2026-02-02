// this module is responsible for reloading the configuration file at runtime (hot-reload)

use anyhow::{Result, bail};
use std::{io::Read, sync::Arc, time::Duration};
use tracing::{info, level_filters::LevelFilter, trace, warn};
use tracing_subscriber::EnvFilter;

use tokio_util::sync::CancellationToken;

use crate::{
    configuration::{LogLevel, v4},
    cruma_integration::{ build_config as build_cruma_config,
    },
    global_state::GlobalState,
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

    // Mark removed backends in the process registry and collect tokens to wait on
    let mut tokens_to_wait: Vec<CancellationToken> = Vec::new();
    {
        let snapshot = global_state.process_registry.snapshot();
        for entry in &snapshot.entries {
            let should_remove = match entry.proc_state() {
                crate::global_state::ProcState::Remote => {
                    !cloned_rems.iter().any(|(id, _)| id == &entry.backend_id)
                }
                crate::global_state::ProcState::DirServer => {
                    !cloned_dirs.iter().any(|(id, _)| id == &entry.backend_id)
                }
                crate::global_state::ProcState::Docker => {
                    // Docker entries are managed by docker_thread, don't touch them here
                    false
                }
                _ => {
                    // Process backends - mark for removal if config changed or removed
                    if cloned_modified_procs.iter().any(|(id, _)| id == &entry.backend_id) {
                        info!("Marking process {} for removal as it has changed", entry.backend_id);
                        true
                    } else if !new_configuration.hosted_processes.contains_key(&entry.backend_id) {
                        info!("Marking process {} for removal as it is no longer in the configuration", entry.backend_id);
                        true
                    } else {
                        false
                    }
                }
            };
            if should_remove {
                if let Some(token) = global_state.process_registry.mark_for_removal(&entry.backend_id) {
                    tokens_to_wait.push(token);
                }
            }
        }
    }

    // Wait for marked proc_hosts to exit before spawning replacements
    for token in tokens_to_wait {
        let _ = tokio::time::timeout(Duration::from_secs(10), token.cancelled()).await;
    }
    global_state.process_registry.cleanup_finished();

    // Register any new remotes
    for (backend_id, _) in &cloned_rems {
        global_state
            .process_registry
            .register_backend(backend_id.clone(), crate::global_state::ProcState::Remote);
    }

    // Register any new static backends
    for (backend_id, _) in &cloned_dirs {
        global_state
            .process_registry
            .register_backend(backend_id.clone(), crate::global_state::ProcState::DirServer);
    }

    // Spawn new/updated process backends
    for (backend_id, proc) in cloned_modified_procs {
        match new_configuration.resolve_process_backend(&backend_id, &proc) {
            Ok(resolved) => {
                // Create token externally and register before spawning
                let token = CancellationToken::new();
                let enabled = resolved.auto_start.unwrap_or(new_configuration.auto_start);
                global_state.process_registry.register_host(
                    backend_id.clone(),
                    token.clone(),
                    crate::global_state::ProcState::Stopped,
                    enabled,
                    resolved.port,
                );
                tokio::task::spawn(proc_host::host(
                    resolved,
                    global_state.process_registry.clone(),
                    token,
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
        Ok((cfg, notes)) => {

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
