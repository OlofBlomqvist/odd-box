use tokio_util::sync::CancellationToken;

use crate::configuration::{LogFormat, LogLevel};
use crate::process_registry::ProcessRegistry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use std::io;
use std::process::{Child, ExitStatus};
use std::time::Instant;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(unix)]
pub fn graceful_stop_pid_only(
    mut parent: Child,
    _include_direct_children: bool,
    total_timeout: Duration,
) -> io::Result<ExitStatus> {
    use nix::sys::signal::{
        Signal::{SIGINT, SIGKILL, SIGTERM},
        kill,
    };
    use nix::unistd::Pid;

    let parent_pid = parent.id() as i32;
    
    // Since we spawn children with process_group(0), each child is its own process group leader.
    // The process group ID (PGID) equals the child's PID.
    // By signaling the negative PGID, we signal the entire process group (including grandchildren).
    let pgid = parent_pid;

    /// Send a signal to an entire process group by using negative PID.
    /// This signals all processes in the group, not just the leader.
    #[inline]
    fn send_to_group(pgid: i32, sig: nix::sys::signal::Signal) {
        // Negative PID means "signal the entire process group with PGID = |pid|"
        let _ = kill(Pid::from_raw(-pgid), sig);
    }
    
    /// Send a signal to a single process (fallback for direct signaling)
    #[inline]
    fn send(pid: i32, sig: nix::sys::signal::Signal) {
        let _ = kill(Pid::from_raw(pid), sig);
    }

    let t_int = total_timeout.mul_f64(0.5);
    let t_term = total_timeout.mul_f64(0.35);
    let t_kill = total_timeout - t_int - t_term;

    // Phase 1: SIGINT (Ctrl-C) - signal the entire process group
    send_to_group(pgid, SIGINT);
    if let Some(st) = wait_with_deadline(&mut parent, t_int)? {
        tracing::info!("Stopped the process group using sigint (ctrl-c)");
        return Ok(st);
    }

    // Phase 2: SIGTERM - signal the entire process group
    send_to_group(pgid, SIGTERM);
    if let Some(st) = wait_with_deadline(&mut parent, t_term)? {
        tracing::info!("Stopped the process group using sigterm");
        return Ok(st);
    }

    // Phase 3: SIGKILL (last resort) - signal the entire process group
    send_to_group(pgid, SIGKILL);
    if let Some(st) = wait_with_deadline(&mut parent, t_kill)? {
        tracing::warn!("Stopped the process group using sigkill - this may leave resources allocated");
        return Ok(st);
    }
    
    // Final fallback: try to kill just the parent process directly
    send(parent_pid, SIGKILL);

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

fn kill_process_and_its_children(parent: std::process::Child) {
    #[cfg(unix)]
    {
        let _ = graceful_stop_pid_only(parent, true, Duration::from_secs(5));
        return;
    }

    #[cfg(not(unix))]
    {
        use std::thread;
        use sysinfo::{ProcessRefreshKind, RefreshKind, System};

        let parent_pid = parent.id();

        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );

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

        let _ = parent.wait();
    }
}

/// Runs a hosted process backend.
///
/// - `resolved_proc`: The fully resolved process configuration (immutable)
/// - `registry`: The process registry for state updates
/// - `token`: Cancellation token - when this proc_host exits, the token will be cancelled
///
/// The proc_host will exit when marked for removal in the registry.
/// It will start/stop the child process based on the `enabled` state in the registry.
/// On exit, the token is automatically cancelled via drop_guard.
pub async fn host(
    resolved_proc: crate::configuration::ResolvedProcessBackend,
    registry: Arc<ProcessRegistry>,
    state: Arc<crate::global_state::GlobalState>,
    token: CancellationToken,
) {
    // Drop guard ensures token is cancelled when this function exits
    let _guard = token.drop_guard();

    let backend_id = &resolved_proc.backend_id;

    let mut previous_state = crate::global_state::ProcState::Stopped;
    let mut missing_bin = false;

    let re = regex::Regex::new(r"^\d* *\[.*?\] .*? - ").expect("host regex always works");

    loop {
        if missing_bin {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
        let mut time_to_sleep_ms = 500;

        // Check if we're marked for removal
        if registry.is_marked_for_removal(backend_id) {
            tracing::debug!("[{}] Marked for removal, exiting", backend_id);
            update_state(
                &registry,
                backend_id,
                &mut previous_state,
                crate::global_state::ProcState::Stopped,
            );
            crate::cruma_integration::rebuild_cruma_config(state.clone());
            break;
        }

        // Check enabled state from registry (can be changed via GUI)
        let enabled = registry.is_enabled(backend_id);

        if !enabled {
            update_state(
                &registry,
                backend_id,
                &mut previous_state,
                crate::global_state::ProcState::Stopped,
            );
            crate::cruma_integration::rebuild_cruma_config(state.clone());
            continue;
        }

        let Some(port) = resolved_proc
            .port
            .or_else(|| crate::configuration::get_random_free_port())
        else {
            tracing::error!("[{}] Failed to get a free port", backend_id);
            continue;
        };

        let current_work_dir = std::env::current_dir()
            .expect("could not get current directory")
            .to_str()
            .expect("could not convert current directory to string")
            .to_string();

        let workdir = resolved_proc
            .dir
            .as_ref()
            .map_or(current_work_dir, |x| x.to_string());

        let resolved_bin_path =
            if let Some(p) = resolve_bin_path(backend_id, &workdir, &resolved_proc.bin) {
                missing_bin = false;
                p
            } else {
                tracing::error!(
                    "[{}] Failed to resolve binary path - workdir: {}, bin: {}",
                    backend_id,
                    workdir,
                    resolved_proc.bin
                );
                update_state(
                    &registry,
                    backend_id,
                    &mut previous_state,
                    crate::global_state::ProcState::Faulty,
                );
                crate::cruma_integration::rebuild_cruma_config(state.clone());
                missing_bin = true;
                continue;
            };

        // Build environment variables (expand $port)
        let mut env_vars: HashMap<String, String> = resolved_proc
            .env_vars
            .iter()
            .map(|kvp| {
                (
                    kvp.key.clone(),
                    kvp.value.replace("$port", &port.to_string()),
                )
            })
            .collect();
        env_vars.insert("PORT".into(), port.to_string());

        // Resolve $port in args
        let args: Vec<String> = resolved_proc
            .args
            .iter()
            .map(|a| a.replace("$port", &port.to_string()))
            .collect();

        update_state(
            &registry,
            backend_id,
            &mut previous_state,
            crate::global_state::ProcState::Starting,
        );

        const _CREATE_NO_WINDOW: u32 = 0x08000000;

        #[cfg(target_os = "windows")]
        const DETACHED_PROCESS: u32 = 0x00000008;

        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;

        #[cfg(target_os = "windows")]
        let cmd = Command::new(&resolved_bin_path)
            .args(&args)
            .envs(&env_vars)
            .current_dir(&workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .creation_flags(DETACHED_PROCESS)
            .spawn();

        #[cfg(not(target_os = "windows"))]
        let cmd = {
            let mut command = Command::new(&resolved_bin_path);
            command
                .args(&args)
                .envs(&env_vars)
                .current_dir(&workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                // Make each child process its own process group leader.
                // This allows us to signal the entire process group (including grandchildren)
                // when stopping the process, ensuring complete cleanup.
                .process_group(0);
            
            // On Linux, also set up the child process to receive SIGTERM when the parent dies.
            // This ensures managed processes are cleaned up even if odd-box crashes or is killed.
            #[cfg(target_os = "linux")]
            unsafe {
                use nix::sys::prctl;
                use nix::sys::signal::Signal;
                command.pre_exec(|| {
                    prctl::set_pdeathsig(Signal::SIGTERM)?;
                    Ok(())
                });
            }
            
            command.spawn()
        };

        match cmd {
            Ok(mut child) => {
                let child_pid = child.id();
                registry.update_state(
                    backend_id,
                    crate::global_state::ProcState::Running,
                    Some(child_pid),
                    Some(port),
                    None,
                    Some(
                        env_vars
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    ),
                    Some(args.clone()),
                    Some(workdir.clone()),
                    Some(resolved_bin_path.clone().to_string_lossy().to_string()),
                );
                previous_state = crate::global_state::ProcState::Running;
                crate::cruma_integration::rebuild_cruma_config(state.clone());

                let stdout = child.stdout.take().expect("Failed to capture stdout");
                let stderr = child.stderr.take().expect("Failed to capture stderr");

                // Spawn stdout reader thread
                let stdout_reader = std::io::BufReader::new(stdout);
                let procname = backend_id.clone();
                let reclone = re.clone();
                let logformat = resolved_proc
                    .log_format
                    .clone()
                    .unwrap_or(LogFormat::standard);
                let proc_loglevel = resolved_proc.log_level.clone().unwrap_or(LogLevel::Info);

                _ = std::thread::Builder::new()
                    .name(procname.clone())
                    .spawn(move || {
                        handle_stdout(stdout_reader, &reclone, &logformat, &proc_loglevel);
                    });

                // Spawn stderr reader thread
                let stderr_reader = std::io::BufReader::new(stderr);
                let procname = backend_id.clone();
                _ = std::thread::Builder::new().name(procname).spawn(move || {
                    for line in std::io::BufRead::lines(stderr_reader) {
                        if let Ok(line) = line {
                            if !line.is_empty() {
                                tracing::error!("{}", line.trim());
                            }
                        }
                    }
                });

                // Monitor the running process
                while let Ok(None) = child.try_wait() {
                    // Check if we're marked for removal
                    if registry.is_marked_for_removal(backend_id) {
                        tracing::info!("[{}] Marked for removal, stopping process", backend_id);
                        update_state(
                            &registry,
                            backend_id,
                            &mut previous_state,
                            crate::global_state::ProcState::Stopping,
                        );
                        kill_process_and_its_children(child);
                        break;
                    }

                    // Check if we've been disabled (stop requested from GUI)
                    if !registry.is_enabled(backend_id) {
                        tracing::info!("[{}] Disabled, stopping process", backend_id);
                        update_state(
                            &registry,
                            backend_id,
                            &mut previous_state,
                            crate::global_state::ProcState::Stopping,
                        );
                        kill_process_and_its_children(child);
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                update_state(
                    &registry,
                    backend_id,
                    &mut previous_state,
                    crate::global_state::ProcState::Stopped,
                );
                crate::cruma_integration::rebuild_cruma_config(state.clone());
            }
            Err(e) => {
                tracing::error!("[{}] Failed to start: {:?}", backend_id, e);
                update_state(
                    &registry,
                    backend_id,
                    &mut previous_state,
                    crate::global_state::ProcState::Faulty,
                );
                crate::cruma_integration::rebuild_cruma_config(state.clone());
            }
        }

        // Check again if we should exit
        if registry.is_marked_for_removal(backend_id) {
            break;
        }

        // If still enabled, the process stopped unexpectedly - restart after delay
        if registry.is_enabled(backend_id) {
            tracing::warn!(
                "[{}] Stopped unexpectedly, will restart in 5 seconds",
                backend_id
            );
            update_state(
                &registry,
                backend_id,
                &mut previous_state,
                crate::global_state::ProcState::Faulty,
            );
            crate::cruma_integration::rebuild_cruma_config(state.clone());
            time_to_sleep_ms = 5000;
        }

        tokio::time::sleep(Duration::from_millis(time_to_sleep_ms)).await;
    }

    // _guard is dropped here, cancelling the token
}

fn update_state(
    registry: &ProcessRegistry,
    backend_id: &str,
    previous: &mut crate::global_state::ProcState,
    new_state: crate::global_state::ProcState,
) {
    if *previous != new_state {
        registry.update_state(
            backend_id,
            new_state.clone(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        *previous = new_state;
    }
}

fn handle_stdout(
    reader: std::io::BufReader<std::process::ChildStdout>,
    re: &regex::Regex,
    logformat: &LogFormat,
    proc_loglevel: &LogLevel,
) {
    let min_level = match proc_loglevel {
        LogLevel::Trace => 1,
        LogLevel::Debug => 2,
        LogLevel::Info => 3,
        LogLevel::Warn => 4,
        LogLevel::Error => 5,
    };

    let mut current_level = 0;

    for line in std::io::BufRead::lines(reader) {
        if let Ok(line) = line {
            if let LogFormat::dotnet = logformat {
                if !line.is_empty() {
                    let mut trimmed = re.replace(&line, "").to_string();
                    if trimmed.contains(" WARN ") || trimmed.contains("warn:") {
                        current_level = 4;
                        trimmed = trimmed.replace("warn:", "").trim().to_string();
                    } else if trimmed.contains("ERROR") || trimmed.contains("error:") {
                        current_level = 5;
                        trimmed = trimmed.replace("error:", "").trim().to_string();
                    } else if trimmed.contains("DEBUG")
                        || trimmed.contains("debug:")
                        || trimmed.contains("dbug:")
                    {
                        current_level = 2;
                        trimmed = trimmed.replace("debug:", "").trim().to_string();
                    } else if trimmed.contains("INFO") || trimmed.contains("info:") {
                        current_level = 3;
                        trimmed = trimmed.replace("info:", "").trim().to_string();
                    }

                    if current_level >= min_level {
                        match current_level {
                            1 => tracing::trace!("{}", trimmed),
                            2 => tracing::debug!("{}", trimmed),
                            3 => tracing::info!("{}", trimmed),
                            4 => tracing::warn!("{}", trimmed),
                            5 => tracing::error!("{}", trimmed),
                            _ => tracing::info!("{}", trimmed),
                        }
                    } else if current_level == 0 {
                        tracing::info!("{}", trimmed);
                    }
                } else {
                    current_level = 0;
                }
            } else {
                tracing::info!("{}", line);
            }
        }
    }
}

fn resolve_bin_path(backend_id: &str, workdir: &str, bin: &str) -> Option<PathBuf> {
    let mut attempts: Vec<String> = Vec::new();
    let bin_path = Path::new(bin);

    if bin_path.is_absolute() {
        attempts.push(bin_path.display().to_string());
        if bin_path.exists() {
            return Some(bin_path.to_path_buf());
        }
    } else {
        let relative_path = Path::new(workdir).join(bin);
        attempts.push(relative_path.display().to_string());
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
    attempts.push(relative_path.display().to_string());
    if relative_path.exists() {
        return Some(relative_path);
    }

    match which::which(bin) {
        Ok(path) => Some(path),
        Err(_) => {
            tracing::warn!(
                backend_id = %backend_id,
                bin = %bin,
                workdir = %workdir,
                attempts = ?attempts,
                "Failed to resolve binary path"
            );
            if let Ok(path_env) = std::env::var("PATH") {
                tracing::debug!(
                    backend_id = %backend_id,
                    path_env = %path_env,
                    "PATH used for binary resolution"
                );
            }
            None
        }
    }
}
