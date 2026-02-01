use arc_swap::ArcSwap;
use tokio_util::sync::CancellationToken;

use crate::configuration::{LogFormat, LogLevel, ProcessBackend};
use crate::cruma_integration::{ build_config as build_cruma_config,
};
use crate::global_state::GlobalState;
use crate::types::proc_info::ProcId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use std::io;
use std::process::{Child, ExitStatus};
use std::time::Instant;

// for mac & linux we send sigint rather than sigkill such that we trigger
// a more graceful shutdown of the child process, giving it a chance to cleanup
// resources and such. duno how this works on windows atm so wont change that for now.
#[cfg(unix)]
pub fn graceful_stop_pid_only(
    mut parent: Child,
    include_direct_children: bool,
    total_timeout: Duration,
) -> io::Result<ExitStatus> {
    use nix::sys::signal::{
        Signal::{SIGINT, SIGKILL, SIGTERM},
        kill,
    };
    use nix::unistd::Pid;
    use sysinfo::{ProcessRefreshKind, RefreshKind, System};

    let parent_pid = parent.id() as i32;

    // Snapshot direct children once (optional), PID-by-PID only.
    let child_pids: Vec<i32> = if include_direct_children {
        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );

        sys.processes()
            .values()
            .filter(|p| p.thread_kind().is_none())
            .filter(|p| p.parent().map(|pp| pp.as_u32()) == Some(parent_pid as u32))
            .map(|p| p.pid().as_u32() as i32)
            .collect()
    } else {
        Vec::new()
    };

    // Helper: send a signal to one PID, ignore errors like ESRCH (already gone).
    #[inline]
    fn send(pid: i32, sig: nix::sys::signal::Signal) {
        let _ = kill(Pid::from_raw(pid), sig);
    }

    // Partition total timeout across phases.
    let t_int = total_timeout.mul_f64(0.5);
    let t_term = total_timeout.mul_f64(0.35);
    let t_kill = total_timeout - t_int - t_term;

    // Phase 1: SIGINT (Ctrl-C)
    for &cpid in &child_pids {
        send(cpid, SIGINT);
    }
    send(parent_pid, SIGINT);
    if let Some(st) = wait_with_deadline(&mut parent, t_int)? {
        tracing::info!("Stopped the process using sigint (ctrl-c)");
        return Ok(st);
    }

    // Phase 2: SIGTERM
    for &cpid in &child_pids {
        send(cpid, SIGTERM);
    }
    send(parent_pid, SIGTERM);
    if let Some(st) = wait_with_deadline(&mut parent, t_term)? {
        tracing::info!("Stopped the process using sigterm");
        return Ok(st);
    }

    // Phase 3: SIGKILL (last resort)
    for &cpid in &child_pids {
        send(cpid, SIGKILL);
    }
    send(parent_pid, SIGKILL);
    if let Some(st) = wait_with_deadline(&mut parent, t_kill)? {
        tracing::warn!("Stopped the process using sigkill - this may leave resources allocated");
        return Ok(st);
    }

    // Not really expecting to get here so lets just see if
    // perhaps we already stopped while we were sending signals.
    // Just doing a one-shot check here to not hang around forever
    if let Some(st) = parent.try_wait()? {
        return Ok(st);
    }

    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "failed to stop process within the given timeout",
    ))
}

#[cfg(unix)]
fn wait_with_deadline(child: &mut Child, dur: Duration) -> io::Result<Option<ExitStatus>> {
    let start = Instant::now();
    loop {
        if let Some(st) = child.try_wait()? {
            return Ok(Some(st));
        }
        if start.elapsed() >= dur {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub struct ProcHostState {
    pub pid: Option<u32>,
    pub started_at_time_stamp: std::time::SystemTime,
    pub backend_id: String,
    pub marked_for_removal: bool,
    pub selected_port: Option<u16>
}

pub struct ProcHost {
    /// This will be cancelled if no thread is running.
    /// ProcHost is never re-used - it is dropped when the process exits,
    /// or of the backend it manages gets removed from the active configuration.
    cancellation_token: CancellationToken,
    pub state : ArcSwap<ProcHostState>,
    pub configuration: ProcessBackend
}

impl ProcHost {

    pub async fn host(
        mut resolved_proc: crate::configuration::ResolvedProcessBackend,
        state: Arc<GlobalState>,
    ) {
        let my_arc = std::sync::Arc::new(AtomicBool::new(true));

        // TODO: update our state with the new process information
        // crate::PROC_THREAD_MAP.insert(
        //     resolved_proc.proc_id.clone(),
        //     crate::types::proc_info::ProcInfo {
        //         started_at_time_stamp: std::time::SystemTime::now(),
        //         marked_for_removal: false,
        //         backend_id: resolved_proc.backend_id.clone(),
        //         pid: None,
        //         liveness_ptr: std::sync::Arc::<AtomicBool>::downgrade(&my_arc),
        //     },
        // );

        let my_id = resolved_proc.proc_id.clone();

        let mut previous_update = crate::global_state::ProcState::Stopped;

        // if auto_start is not set in the config, we assume that user wants to start site automatically like before
        let mut enabled = {
            match resolved_proc.auto_start {
                Some(v) => v,
                None => {
                    let guard = state.config.read().await;
                    guard.auto_start
                }
            }
        };

        let excluded_from_auto_start = resolved_proc.exclude_from_start_all;

        let mut initialized = false;
        let domsplit = resolved_proc.backend_id.split(".").collect::<Vec<&str>>();

        let mut acceptable_names = vec![resolved_proc.backend_id.clone()];

        if domsplit.len() > 0 {
            acceptable_names.push(domsplit[0].to_owned());
        }

        let re = regex::Regex::new(r"^\d* *\[.*?\] .*? - ").expect("host regex always works");

        pub fn kill_process_and_its_children(parent: std::process::Child) {
            #[cfg(unix)]
            {
                let _ = graceful_stop_pid_only(parent, true, Duration::from_secs(5));
                return;
            }

            #[cfg(not(unix))]
            {
                use std::thread;
                let parent_pid = parent.id();

                let mut sys = System::new_with_specifics(
                    sysinfo::RefreshKind::nothing()
                        .with_processes(sysinfo::ProcessRefreshKind::everything()),
                );

                sys.refresh_all();

                let child_pids: Vec<u32> = sys
                    .processes()
                    .values()
                    .filter(|p| p.thread_kind().is_none())
                    .filter(|p| p.parent().map(|pp| pp.as_u32()) == Some(parent_pid))
                    .map(|p| p.pid().as_u32())
                    .collect();

                for pid_u32 in &child_pids {
                    if let Some(p) = sys.process(sysinfo::Pid::from_u32(*pid_u32)) {
                        if p.kill() {
                            tracing::debug!("Sent kill to child process with pid {}", pid_u32);
                        } else {
                            tracing::warn!("Failed to kill child process with pid {}", pid_u32);
                        }
                    }
                }

                thread::sleep(Duration::from_millis(50));

                match parent.kill() {
                    Ok(()) => tracing::debug!("Sent kill to main process with pid {}", parent_pid),
                    Err(e) => tracing::warn!("Failed to kill main process {}: {}", parent_pid, e),
                }

                // dont want no zombies
                let _ = parent.wait();
            }
        }

        let mut missing_bin: bool = false;

        loop {

            if missing_bin {
                // dont want to try this too often if file is gone
                tokio::time::sleep(Duration::from_secs(10)).await;
            }

            // TODO: check if this process is still expected to be running/managed,
            // otherwise we should break now. probably on Self of the proc_host
            // so that it can be toggled by other threads.


            tokio::time::sleep(Duration::from_millis(200)).await;
            let mut time_to_sleep_ms_after_each_iteration = 500;

            let exit = state
                .exit
                .load(std::sync::atomic::Ordering::SeqCst)
                == true;

            if exit {
                _ = update_status(
                    &previous_update,
                    &resolved_proc.backend_id,
                    &my_id,
                    &state,
                    crate::global_state::ProcState::Stopped,
                    "stop due to exit",
                );
                tracing::debug!("exiting host for {}", &resolved_proc.backend_id);
                break;
            }

            if initialized == false {
                previous_update = update_status(
                    &previous_update,
                    &resolved_proc.backend_id,
                    &my_id,
                    &state,
                    crate::global_state::ProcState::Stopped,
                    "stopping due to init false",
                );
                initialized = true;
            } else {
                previous_update = update_status(
                    &previous_update,
                    &resolved_proc.backend_id,
                    &my_id,
                    &state,
                    crate::global_state::ProcState::Stopped,
                    "stopped due to init true",
                );
            }

            // TODO: enabled needs to be moved somewhere living on "self"
            // such that others can toggle it
            if !enabled {
                // TODO: update our status to stopped if not already?
                // TODO - wait time here to not create a hot loop?
                continue;
            }

            // just to make sure we haven't messed up timing-wise and selected the same port for two different processes
            // we will always call this function to get a new port (or keep the old one if we are the only one using it)

            let Some(port) = resolved_proc.port.or_else(|| crate::configuration::get_random_free_port()) else {
                tracing::error!("failed to get a free port for backend process '{}'", resolved_proc.backend_id);
                continue;
            };

            let current_work_dir = std::env::current_dir()
                .expect("could not get current directory")
                .to_str()
                .expect("could not convert current directory to string")
                .to_string();

            let workdir = &resolved_proc
                .dir
                .as_ref()
                .map_or(current_work_dir, |x| x.to_string());

            let (global_min_loglevel, _global_default_log_format) = {
                let guard = state.config.read().await;
                (guard.log_level.clone(), guard.default_log_format.clone())
            };

            let do_initial_trace = if let Some(ref ll) = resolved_proc.log_level {
                ll == &LogLevel::Trace
            } else {
                global_min_loglevel == LogLevel::Trace
            };

            if do_initial_trace {
                tracing::trace!(
                    "[{}] Executing command '{}' in directory '{}'",
                    resolved_proc.backend_id,
                    resolved_proc.bin,
                    workdir
                );
            }

            let resolved_bin_path = if let Some(p) = resolve_bin_path(&workdir, &resolved_proc.bin) {
                missing_bin = false;
                p
            } else {
                tracing::error!(
                    "Failed to resolve path of binary for site: '{}' - workdir: {}, bin: {}",
                    &resolved_proc.backend_id,
                    workdir,
                    resolved_proc.bin
                );
                previous_update = update_status(
                    &previous_update,
                    &resolved_proc.backend_id,
                    &my_id,
                    &state,
                    crate::global_state::ProcState::Faulty,
                    "faulty since we could not resolve bin path",
                );
                missing_bin = true;
                continue;
            };

            let mut process_specific_environment_variables = HashMap::new();

            {
                let state_guard = state.config.read().await;
                for (key, value) in &state_guard.env {
                    if do_initial_trace {
                        tracing::trace!(
                            "[{}] ADDING GLOBAL ENV VAR '{}': {}",
                            &resolved_proc.backend_id,
                            key,
                            value
                        );
                    }
                    process_specific_environment_variables.insert(key.clone(), value.clone());
                }
            }

            // more specific env vars should override globals
            for kvp in &resolved_proc.env_vars {
                if do_initial_trace {
                    tracing::trace!(
                        "[{}] ADDING ENV VAR '{}': {}",
                        &resolved_proc.backend_id,
                        &kvp.key,
                        &kvp.value
                    );
                }
                process_specific_environment_variables.insert(kvp.key.clone(), kvp.value.clone());
            }

            process_specific_environment_variables.insert("PORT".into(), port.clone().to_string());

            let mut pre_resolved_args = resolved_proc.args.clone();

            for p in &mut pre_resolved_args {
                *p = p.replace("$port", &(port.to_string()));
            }

            previous_update = update_status(
                &previous_update,
                &resolved_proc.backend_id,
                &my_id,
                &state,
                crate::global_state::ProcState::Starting,
                "starting!",
            );

            const _CREATE_NO_WINDOW: u32 = 0x08000000;

            #[cfg(target_os = "windows")]
            const DETACHED_PROCESS: u32 = 0x00000008;

            #[cfg(target_os = "windows")]
            use std::os::windows::process::CommandExt;

            #[cfg(target_os = "windows")]
            let cmd = Command::new(resolved_bin_path)
                .args(pre_resolved_args)
                .envs(&process_specific_environment_variables)
                .current_dir(&workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                // dont want windows to let child take over our keyboard input and such
                .creation_flags(DETACHED_PROCESS)
                .spawn();

            #[cfg(not(target_os = "windows"))]
            let cmd = Command::new(resolved_bin_path)
                .args(pre_resolved_args)
                .envs(&process_specific_environment_variables)
                .current_dir(&workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn();

            match cmd {
                Ok(mut child) => {
                    previous_update = update_status(
                        &previous_update,
                        &resolved_proc.backend_id,
                        &my_id,
                        &state,
                        crate::global_state::ProcState::Running,
                        "running!",
                    );

                    // TODO: Update our own state with new pid and started_at_time_stamp
                    //item.pid = Some(child.id().to_string());
                    //item.started_at_time_stamp = std::time::SystemTime::now();

                    //let stdin = child.stdin.take().expect("Failed to capture stdin");

                    let stdout = child.stdout.take().expect("Failed to capture stdout");
                    let stderr = child.stderr.take().expect("Failed to capture stderr");

                    let stdout_reader = std::io::BufReader::new(stdout);
                    let stderr_reader = std::io::BufReader::new(stderr);
                    let procname = resolved_proc.backend_id.clone();
                    let reclone = re.clone();

                    let (_global_min_loglevel, global_default_log_format) = {
                        let guard = state.config.read().await;
                        (guard.log_level.clone(), guard.default_log_format.clone())
                    };

                    let logformat = resolved_proc
                        .log_format
                        .clone()
                        .unwrap_or(global_default_log_format);

                    // note: global min loglevel IS NOT supposed to be used as default for processes - processes should always default to info
                    let proc_loglevel = resolved_proc.log_level.clone().unwrap_or(LogLevel::Info);

                    _ = std::thread::Builder::new()
                        .name(format!("{procname}"))
                        .spawn(move || {
                            let mut current_log_level = 0;

                            let min_log_level_for_the_process = match proc_loglevel {
                                LogLevel::Trace => 1,
                                LogLevel::Debug => 2,
                                LogLevel::Info => 3,
                                LogLevel::Warn => 4,
                                LogLevel::Error => 5,
                            };

                            for line in std::io::BufRead::lines(stdout_reader) {
                                if let Ok(line) = line {
                                    // todo: should move custom logging elsewhere if theres ever more than one
                                    if let LogFormat::dotnet = &logformat {
                                        if line.len() > 0 {
                                            let mut trimmed = reclone.replace(&line, "").to_string();
                                            if trimmed.contains(" WARN ") || trimmed.contains("warn:") {
                                                current_log_level = 4;
                                                trimmed.replace("warn:", "").trim().to_string();
                                            } else if trimmed.contains("ERROR")
                                                || trimmed.contains("error:")
                                            {
                                                current_log_level = 5;
                                                trimmed.replace("error:", "").trim().to_string();
                                            } else if trimmed.contains("DEBUG")
                                                || trimmed.contains("debug:")
                                                || trimmed.contains("dbug:")
                                            {
                                                current_log_level = 2;
                                                trimmed.replace("debug:", "").trim().to_string();
                                            } else if trimmed.contains("INFO")
                                                || trimmed.contains("info:")
                                            {
                                                current_log_level = 3;
                                                trimmed =
                                                    trimmed.replace("info:", "").trim().to_string()
                                            }

                                            if current_log_level >= min_log_level_for_the_process {
                                                match &current_log_level {
                                                    1 => {
                                                        tracing::trace!("{}", trimmed)
                                                    }
                                                    2 => {
                                                        tracing::debug!("{}", trimmed)
                                                    }
                                                    3 => {
                                                        tracing::info!("{}", trimmed)
                                                    }
                                                    4 => {
                                                        tracing::warn!("{}", trimmed)
                                                    }
                                                    5 => {
                                                        tracing::error!("{}", trimmed)
                                                    }
                                                    _ => tracing::info!("{}", trimmed),
                                                }
                                            } else if current_log_level == 0 {
                                                tracing::info!("{}", trimmed)
                                            }
                                        } else {
                                            current_log_level = 0;
                                        }
                                    } else {
                                        tracing::info!("{}", line)
                                    }
                                }
                            }
                        });

                    let procname = resolved_proc.backend_id.clone();
                    _ = std::thread::Builder::new()
                        .name(format!("{procname}"))
                        .spawn(move || {
                            for line in std::io::BufRead::lines(stderr_reader) {
                                if let Ok(line) = line {
                                    if line.len() > 0 {
                                        tracing::error!("{}", line.trim());
                                    }
                                }
                            }
                        });

                    while let Ok(None) = child.try_wait() {
                        let exit = state
                            .exit
                            .load(std::sync::atomic::Ordering::SeqCst)
                            == true;
                        if exit {
                            tracing::info!("[{}] Stopping due to app exit", resolved_proc.backend_id);
                            previous_update = update_status(
                                &previous_update,
                                &resolved_proc.backend_id,
                                &my_id,
                                &state,
                                crate::global_state::ProcState::Stopping,
                                "stopping..exiting",
                            );
                            kill_process_and_its_children(child);
                            break;
                        }

                        let live_proc_config = {
                            let entry = state.config.read().await;
                            let config = entry.hosted_processes.get(&resolved_proc.backend_id);
                            if let Some(c) = config {
                                Some(c.clone())
                            } else {
                                None
                            }
                        };

                        if let Some(live_proc_config) = live_proc_config {
                            if live_proc_config.proc_id != resolved_proc.proc_id {
                                tracing::warn!(
                                    "[{}] Stopping due to having been replaced by a new process with the same name",
                                    resolved_proc.backend_id
                                );
                                previous_update = update_status(
                                    &previous_update,
                                    &resolved_proc.backend_id,
                                    &my_id,
                                    &state,
                                    crate::global_state::ProcState::Stopping,
                                    "stopping due to being replaced",
                                );
                                kill_process_and_its_children(child);
                                break;
                            }
                            resolved_proc.log_format = live_proc_config.log_format.clone();
                            resolved_proc.log_level = live_proc_config.log_level.clone();
                        }

                        previous_update = update_status(
                            &previous_update,
                            &resolved_proc.backend_id,
                            &my_id,
                            &state,
                            crate::global_state::ProcState::Running,
                            "running!!",
                        );

                        // TODO: check if we have been marked for removal/not be managed anymore,
                        // and if so we should do {
                        //  kill_process_and_its_children(child);
                        //  return;
                        // }


                        if !enabled {
                            tracing::warn!(
                                "[{}] Stopping due to having been disabled by proxy.",
                                resolved_proc.backend_id
                            );
                            previous_update = update_status(
                                &previous_update,
                                &resolved_proc.backend_id,
                                &my_id,
                                &state,
                                crate::global_state::ProcState::Stopping,
                                "stopping because not enabled",
                            );

                            // Not enabled for now, just kill the process and get back to the outer
                            // loop waiting to be enabled or marked for removal, or app exit.
                            kill_process_and_its_children(child);
                            break;
                        }

                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    previous_update = update_status(
                        &previous_update,
                        &resolved_proc.backend_id,
                        &my_id,
                        &state,
                        crate::global_state::ProcState::Stopped,
                        "stopped!!",
                    );
                }
                Err(e) => {
                    tracing::info!("[{}] Failed to start! {e:?}", resolved_proc.backend_id);
                    previous_update = update_status(
                        &previous_update,
                        &resolved_proc.backend_id,
                        &my_id,
                        &state,
                        crate::global_state::ProcState::Faulty,
                        "something is wrong",
                    );
                }
            }

            if enabled {
                if !state
                    .exit
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    tracing::warn!(
                        "[{}] Stopped unexpectedly.. Will automatically restart the process in 5 seconds unless stopped.",
                        resolved_proc.backend_id
                    );
                    previous_update = update_status(
                        &previous_update,
                        &resolved_proc.backend_id,
                        &my_id,
                        &state,
                        crate::global_state::ProcState::Faulty,
                        "something is wrong with the process..",
                    );
                    time_to_sleep_ms_after_each_iteration = 5000; // wait 5 seconds before restarting but NOT in here as we have a lock
                } else {
                    tracing::info!(
                        "[{}] Stopped due to exit signal. Bye!",
                        resolved_proc.backend_id
                    );
                    break;
                }
            }

            tokio::time::sleep(Duration::from_millis(time_to_sleep_ms_after_each_iteration)).await;
        }
    }
}

fn resolve_bin_path(workdir: &str, bin: &str) -> Option<PathBuf> {
    let bin_path = Path::new(bin);

    if bin_path.is_absolute() {
        if bin_path.exists() {
            return Some(bin_path.to_path_buf());
        }
    } else {
        let relative_path = Path::new(workdir).join(bin);
        if relative_path.exists() {
            return Some(relative_path);
        }
    }

    let current_work_dir = std::env::current_dir()
        .expect("could not get current directory")
        .to_str()
        .expect("could not convert current directory to string")
        .to_string();
    let relative_path = Path::new(&current_work_dir).join(bin);
    if relative_path.exists() {
        return Some(relative_path);
    }

    match which::which(bin) {
        Ok(path) => Some(path),
        Err(_) => None,
    }
}

fn update_status(
    previous: &crate::global_state::ProcState,
    x: &str,
    id: &ProcId,
    g: &Arc<GlobalState>,
    s: crate::global_state::ProcState,
    from_msg: &str,
) -> crate::global_state::ProcState {
    todo!()
}
