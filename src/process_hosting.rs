//! Adapter module that bridges odd-box's configuration types and the
//! `cruma_proc_host` crate for process lifecycle management.
//!
//! This module replaces the old `proc_host.rs` (supervisor loop) and
//! `process_registry.rs` (state registry) with a unified adapter that
//! delegates process management to `cruma_proc_host` while keeping
//! backward-compatible types for the rest of the application.
//!
//! # Architecture
//!
//! * **Process backends** are managed by [`cruma_proc_host::NativeProcessHost`]
//!   and its embedded [`cruma_proc_host::ProcessOrchestrator`].
//! * **Non-process backends** (Remote, DirServer, Docker) are tracked in a
//!   simple `ArcSwap`-based map since they don't need supervision.
//! * [`ProcessRegistry`] provides a combined view (snapshot) of both,
//!   presenting the same API surface that the rest of odd-box expects.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use arc_swap::ArcSwap;

use crate::configuration::{LogLevel, ResolvedProcessBackend};
use crate::global_state::ProcState;

// ── Re-exports from cruma_proc_host ────────────────────────────────────

pub use cruma_proc_host::{
    DefaultProcessHost, HostedProcessSpec, HostedProcessStatus, LogStore, NativeProcessHost,
    ProcHost, ProcHostCommand, ProcessHost, ProcessHostError, ProcessHostResult, ProcessLogEntry,
    ProcessLogLevel, ProcessLogLevelFilter, ProcessLogSource, ProcessOrchestrator, ResolvedSpec,
    RestartPolicy,
};

// ── Metadata keys ──────────────────────────────────────────────────────
//
// Proxy-specific fields stored in `spec.metadata` under these well-known
// keys so the process-hosting crate stays framework-agnostic.

/// Metadata key: protocol variant (e.g. `"H1"`, `"H2"`, `"H2C"`, `"H2CPK"`).
pub const META_PROTOCOL: &str = "protocol";

/// Metadata key: `"true"` if the upstream connection uses TLS.
pub const META_HTTPS: &str = "https";

/// Metadata key: log level name (`"Trace"`, `"Debug"`, `"Info"`, etc.).
pub const META_LOG_LEVEL: &str = "log_level";

/// Metadata key: log format name (`"standard"`, `"dotnet"`).
pub const META_LOG_FORMAT: &str = "log_format";

// ── Config → HostedProcessSpec conversion ──────────────────────────────

/// Convert an odd-box [`ResolvedProcessBackend`] into a
/// [`HostedProcessSpec`] ready for the process-hosting crate.
///
/// Proxy-specific fields (`protocol`, `https`, `log_level`, `log_format`)
/// are stashed in `spec.metadata` under well-known keys.
pub fn spec_from_resolved(
    resolved: &ResolvedProcessBackend,
    global_auto_start: bool,
) -> HostedProcessSpec {
    // Convert Vec<EnvVar> → HashMap<String, String>
    let mut env: HashMap<String, String> = resolved
        .env_vars
        .iter()
        .map(|kv| (kv.key.clone(), kv.value.clone()))
        .collect();

    // If port is fixed, inject it so resolve_for_spawn uses it
    // rather than auto-allocating.
    if let Some(port) = resolved.port {
        env.insert("PORT".into(), port.to_string());
    }

    // Store proxy-specific fields in metadata
    let mut metadata = HashMap::new();
    metadata.insert(META_PROTOCOL.into(), format!("{:?}", resolved.protocol));
    metadata.insert(META_HTTPS.into(), resolved.https.to_string());
    if let Some(ref ll) = resolved.log_level {
        metadata.insert(META_LOG_LEVEL.into(), format!("{:?}", ll));
    }
    if let Some(ref lf) = resolved.log_format {
        metadata.insert(META_LOG_FORMAT.into(), format!("{:?}", lf));
    }

    let log_level_filter = match resolved.log_level.as_ref().unwrap_or(&LogLevel::Info) {
        LogLevel::Trace => ProcessLogLevelFilter::Trace,
        LogLevel::Debug => ProcessLogLevelFilter::Debug,
        LogLevel::Info => ProcessLogLevelFilter::Info,
        LogLevel::Warn => ProcessLogLevelFilter::Warn,
        LogLevel::Error => ProcessLogLevelFilter::Error,
    };

    let enabled = resolved.auto_start.unwrap_or(global_auto_start);

    HostedProcessSpec {
        id: resolved.backend_id.clone(),
        command: resolved.bin.clone(),
        args: resolved.args.clone(),
        working_directory: resolved.dir.as_ref().map(|d| d.into()),
        env,
        restart_policy: RestartPolicy::OnFailure,
        start_on_request: false,
        idle_timeout_seconds: None,
        enabled,
        log_level_filter,
        metadata,
    }
}

// ── ProcState mapping ──────────────────────────────────────────────────

/// Derive a [`ProcState`] from a [`ProcHost`]'s live status.
pub fn proc_state_from_host(host: &ProcHost) -> ProcState {
    let status = host.status();
    if status.running {
        ProcState::Running
    } else if !host.is_enabled() {
        ProcState::Stopped
    } else if status.last_error.is_some() || host.has_recent_error() {
        ProcState::Faulty
    } else {
        // Enabled but not running yet — supervisor will start it soon
        ProcState::Starting
    }
}

// ── Non-process backend state ──────────────────────────────────────────

/// Simple state holder for non-process backends (Remote, DirServer, Docker).
#[derive(Debug, Clone)]
struct NonProcessBackend {
    backend_id: String,
    proc_state: ProcState,
}

// ── ProcessHandle ──────────────────────────────────────────────────────

/// A unified handle representing either a process backend (backed by
/// [`ProcHost`]) or a non-process backend (Remote, DirServer, Docker).
#[derive(Clone, Debug)]
pub struct ProcessHandle {
    pub backend_id: String,
    /// If this is a process backend, the ProcHost handle.
    proc_host: Option<ProcHost>,
    /// If this is a non-process backend, the static state.
    non_process_state: Option<Arc<ArcSwap<ProcState>>>,
}

impl ProcessHandle {
    /// Get the current process state.
    pub fn proc_state(&self) -> ProcState {
        if let Some(host) = &self.proc_host {
            proc_state_from_host(host)
        } else if let Some(state) = &self.non_process_state {
            (**state.load()).clone()
        } else {
            ProcState::Stopped
        }
    }

    /// Get the active port if this is a process backend with a port allocated.
    pub fn active_port(&self) -> Option<u16> {
        self.proc_host
            .as_ref()
            .and_then(|h| h.status().allocated_port)
    }

    /// Check if the ProcHost token is cancelled (process host exited).
    pub fn is_cancelled(&self) -> bool {
        self.proc_host
            .as_ref()
            .map(|h| h.is_cancelled())
            .unwrap_or(false)
    }

    /// Check if this entry is marked for removal.
    pub fn is_marked_for_removal(&self) -> bool {
        self.proc_host
            .as_ref()
            .map(|h| h.is_marked_for_removal())
            .unwrap_or(false)
    }

    /// Get the full process status (for process backends).
    pub fn status(&self) -> Option<HostedProcessStatus> {
        self.proc_host.as_ref().map(|h| h.status())
    }

    /// Get the underlying ProcHost handle (for process backends).
    pub fn proc_host(&self) -> Option<&ProcHost> {
        self.proc_host.as_ref()
    }

    /// Full state snapshot for backward compatibility with the old
    /// ProcessState struct.
    pub fn state(&self) -> ProcessState {
        if let Some(host) = &self.proc_host {
            let status = host.status();
            ProcessState {
                proc_state: proc_state_from_host(host),
                pid: status.pid,
                active_port: status.allocated_port,
                started_at: status.started_at,
                last_transition_at: status.last_transition_at,
                last_exit_at: status.last_exit_at,
                enabled: host.is_enabled(),
                resolved_env: status.resolved_env.map(|m| m.into_iter().collect()),
                resolved_args: status.resolved_args,
                resolved_dir: status.resolved_dir,
                resolved_bin: status.resolved_bin,
                last_error: status.last_error,
            }
        } else {
            ProcessState {
                proc_state: self.proc_state(),
                pid: None,
                active_port: None,
                started_at: None,
                last_transition_at: None,
                last_exit_at: None,
                enabled: true,
                resolved_env: None,
                resolved_args: None,
                resolved_dir: None,
                resolved_bin: None,
                last_error: None,
            }
        }
    }
}

/// Backward-compatible state struct mirroring the old `ProcessState`.
#[derive(Clone, Debug)]
pub struct ProcessState {
    pub proc_state: ProcState,
    pub pid: Option<u32>,
    pub active_port: Option<u16>,
    pub started_at: Option<SystemTime>,
    pub last_transition_at: Option<SystemTime>,
    pub last_exit_at: Option<SystemTime>,
    pub enabled: bool,
    pub resolved_env: Option<Vec<(String, String)>>,
    pub resolved_args: Option<Vec<String>>,
    pub resolved_dir: Option<String>,
    pub resolved_bin: Option<String>,
    pub last_error: Option<String>,
}

// ── RegistrySnapshot ───────────────────────────────────────────────────

/// Combined snapshot of all backends (process + non-process).
#[derive(Clone, Debug)]
pub struct RegistrySnapshot {
    pub version: u64,
    pub entries: Vec<ProcessHandle>,
}

impl RegistrySnapshot {
    /// Look up a backend by ID.
    pub fn get(&self, backend_id: &str) -> Option<&ProcessHandle> {
        self.entries.iter().find(|e| e.backend_id == backend_id)
    }

    /// Get just the state for a backend.
    pub fn state_of(&self, backend_id: &str) -> Option<ProcState> {
        self.get(backend_id).map(|e| e.proc_state())
    }
}

// ── ProcessRegistry ────────────────────────────────────────────────────

/// Unified registry for process and non-process backends.
///
/// Process backends are managed by [`NativeProcessHost`] /
/// [`ProcessOrchestrator`].  Non-process backends (Remote, DirServer,
/// Docker) are tracked in a simple map.
#[derive(Debug)]
pub struct ProcessRegistry {
    /// The process host handles starting/stopping/supervising processes.
    process_host: Arc<NativeProcessHost>,
    /// Non-process backends (Remote, DirServer, Docker).
    non_process: ArcSwap<Vec<NonProcessBackend>>,
    /// Monotonic version counter, bumped on structural changes.
    version: AtomicU64,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self {
            process_host: Arc::new(NativeProcessHost::default()),
            non_process: ArcSwap::from_pointee(Vec::new()),
            version: AtomicU64::new(0),
        }
    }

    /// Access the underlying NativeProcessHost.
    pub fn process_host(&self) -> &Arc<NativeProcessHost> {
        &self.process_host
    }

    /// Access the underlying ProcessOrchestrator.
    pub fn orchestrator(&self) -> &Arc<ProcessOrchestrator> {
        self.process_host.orchestrator()
    }

    /// Returns a combined snapshot of all backends.
    pub fn snapshot(&self) -> Arc<RegistrySnapshot> {
        let orch = self.process_host.orchestrator().snapshot();
        let non_proc = self.non_process.load();

        let mut entries: Vec<ProcessHandle> = Vec::new();

        // Add process backends
        for host in orch.iter() {
            entries.push(ProcessHandle {
                backend_id: host.id.clone(),
                proc_host: Some(host.clone()),
                non_process_state: None,
            });
        }

        // Add non-process backends
        for np in non_proc.iter() {
            entries.push(ProcessHandle {
                backend_id: np.backend_id.clone(),
                proc_host: None,
                non_process_state: Some(Arc::new(ArcSwap::from_pointee(np.proc_state.clone()))),
            });
        }

        entries.sort_by(|a, b| a.backend_id.cmp(&b.backend_id));

        Arc::new(RegistrySnapshot {
            version: self.version.load(Ordering::Relaxed),
            entries,
        })
    }

    /// Start a hosted process backend.
    ///
    /// This replaces the old `register_host()` + `tokio::spawn(proc_host::host())`
    /// pattern.  The `NativeProcessHost` internally spawns a supervisor thread.
    pub fn start_hosted_process(&self, spec: HostedProcessSpec) {
        // If there's an existing host for this ID, stop it first
        if self.process_host.orchestrator().contains_host(&spec.id) {
            let _ = self.process_host.stop_process(&spec.id);
        }

        match self.process_host.start_process(spec) {
            Ok(status) => {
                tracing::info!(
                    backend_id = %status.id,
                    pid = ?status.pid,
                    port = ?status.allocated_port,
                    "Started hosted process"
                );
            }
            Err(err) => {
                tracing::error!(error = %err, "Failed to start hosted process");
            }
        }
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Register a process spec without starting it (for on-demand / disabled processes).
    pub fn register_process(&self, spec: HostedProcessSpec) {
        // Just register in the orchestrator — no supervisor thread started
        let (_host, _rx) = self.process_host.orchestrator().register_spec(spec);
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Register a non-process backend (Remote, Static, Docker).
    pub fn register_backend(&self, backend_id: impl Into<String>, state: ProcState) {
        let backend_id = backend_id.into();
        self.non_process.rcu(|current| {
            let mut next = (**current).clone();
            // Remove existing entry if present
            next.retain(|np| np.backend_id != backend_id);
            next.push(NonProcessBackend {
                backend_id: backend_id.clone(),
                proc_state: state.clone(),
            });
            Arc::new(next)
        });
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Set the enabled state for a process backend.
    /// Returns `true` if the backend was found and updated.
    pub fn set_enabled(&self, backend_id: &str, enabled: bool) -> bool {
        self.process_host
            .orchestrator()
            .set_enabled(backend_id, enabled)
    }

    /// Check if a backend is enabled.
    pub fn is_enabled(&self, backend_id: &str) -> bool {
        // Check process backends first
        if self.process_host.orchestrator().contains_host(backend_id) {
            return self.process_host.orchestrator().is_enabled(backend_id);
        }
        // Non-process backends are always "enabled"
        true
    }

    /// Mark a process backend for removal.
    /// Returns the cancellation token if present.
    pub fn mark_for_removal(
        &self,
        backend_id: &str,
    ) -> Option<tokio_util::sync::CancellationToken> {
        // Check if it's a process backend
        if let Some(host) = self.process_host.orchestrator().find_host(backend_id) {
            // Send stop command and mark for removal
            host.send_command(ProcHostCommand::Stop);
            host.mark_for_removal();
            return Some(host.cancellation_token().clone());
        }

        // Non-process backend — remove directly
        self.non_process.rcu(|current| {
            let mut next = (**current).clone();
            next.retain(|np| np.backend_id != backend_id);
            Arc::new(next)
        });
        self.version.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Check if a backend is marked for removal.
    pub fn is_marked_for_removal(&self, backend_id: &str) -> bool {
        if let Some(host) = self.process_host.orchestrator().find_host(backend_id) {
            return host.is_marked_for_removal();
        }
        false
    }

    /// Mark all process backends for removal.
    pub fn mark_all_for_removal(&self) {
        let hosts = self.process_host.orchestrator().snapshot();
        for host in hosts.iter() {
            host.send_command(ProcHostCommand::Stop);
            host.mark_for_removal();
        }
    }

    /// Remove entries that are marked for removal AND whose token is
    /// cancelled (process host thread exited).
    pub fn cleanup_finished(&self) {
        self.process_host.orchestrator().cleanup_finished_hosts();
    }

    /// Remove a non-process backend immediately.
    pub fn remove(&self, backend_id: &str) {
        // Try process backend first
        if self.process_host.orchestrator().contains_host(backend_id) {
            self.process_host.orchestrator().remove_host(backend_id);
        }
        // Also try non-process
        self.non_process.rcu(|current| {
            let mut next = (**current).clone();
            let before = next.len();
            next.retain(|np| np.backend_id != backend_id);
            if next.len() == before {
                return current.clone();
            }
            Arc::new(next)
        });
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the state of a backend.
    ///
    /// For process backends, this maps to enabling/disabling. State
    /// transitions (Starting, Running, Stopped, etc.) are managed
    /// automatically by the supervisor thread inside NativeProcessHost.
    ///
    /// For non-process backends, the state is updated directly.
    pub fn update_state(
        &self,
        backend_id: &str,
        state: ProcState,
        _pid: Option<u32>,
        _active_port: Option<u16>,
        enabled: Option<bool>,
        _resolved_env: Option<Vec<(String, String)>>,
        _resolved_args: Option<Vec<String>>,
        _resolved_dir: Option<String>,
        _resolved_bin: Option<String>,
    ) {
        // For process backends: the key action is toggling enabled.
        // The supervisor thread handles Starting → Running → Stopped
        // transitions automatically.
        if let Some(host) = self.process_host.orchestrator().find_host(backend_id) {
            if let Some(en) = enabled {
                host.set_enabled(en);
            }
            return;
        }

        // For non-process backends: update state directly.
        self.non_process.rcu(|current| {
            let mut next = (**current).clone();
            if let Some(np) = next.iter_mut().find(|np| np.backend_id == backend_id) {
                np.proc_state = state.clone();
            }
            Arc::new(next)
        });
    }
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn process_log_source_label(source: ProcessLogSource) -> &'static str {
    match source {
        ProcessLogSource::Stdout => "stdout",
        ProcessLogSource::Stderr => "stderr",
        ProcessLogSource::System => "system",
    }
}

fn forward_process_log_to_tracing(entry: &ProcessLogEntry) {
    let process_id = entry.process_id.as_deref().unwrap_or("proc-host");
    let source = process_log_source_label(entry.source);
    let source_tag = format!("{process_id}.{source}");
    let msg = format!("[{source_tag}] {}", entry.message);

    match entry.level {
        ProcessLogLevel::Trace => tracing::trace!(target: "cruma_proc_host", "{msg}"),
        ProcessLogLevel::Debug => tracing::debug!(target: "cruma_proc_host", "{msg}"),
        ProcessLogLevel::Info => tracing::info!(target: "cruma_proc_host", "{msg}"),
        ProcessLogLevel::Warn => tracing::warn!(target: "cruma_proc_host", "{msg}"),
        ProcessLogLevel::Error => tracing::error!(target: "cruma_proc_host", "{msg}"),
    }
}

/// Bridge aggregate process-host logs into the app's tracing pipeline so
/// existing GUI/TUI log viewers keep receiving hosted-process output.
pub fn spawn_process_log_bridge(
    state: Arc<crate::global_state::GlobalState>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let orchestrator = state.process_registry.orchestrator().clone();
        let aggregate_store = orchestrator.log_store();
        let mut receiver = orchestrator.subscribe();
        let mut last_sequence: Option<u64> = None;

        loop {
            if state.exit.load(Ordering::Relaxed) {
                break;
            }

            match tokio::time::timeout(std::time::Duration::from_millis(250), receiver.recv()).await
            {
                Ok(Ok(entry)) => {
                    last_sequence = Some(entry.sequence);
                    forward_process_log_to_tracing(&entry);
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                    let catch_up_entries = match last_sequence {
                        Some(seq) => aggregate_store.since(seq),
                        None => aggregate_store.snapshot(),
                    };
                    for entry in catch_up_entries {
                        last_sequence = Some(entry.sequence);
                        forward_process_log_to_tracing(&entry);
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                Err(_) => {}
            }
        }
    })
}

// ── State change monitor ───────────────────────────────────────────────

/// Background task that polls for process state changes and calls
/// `rebuild_cruma_config` when needed.
///
/// This replaces the inline `rebuild_cruma_config()` calls that the old
/// `proc_host::host()` supervisor used to make on every state transition.
pub async fn state_change_monitor(state: Arc<crate::global_state::GlobalState>) {
    let mut previous_states: HashMap<String, ProcState> = HashMap::new();
    let mut previous_ports: HashMap<String, Option<u16>> = HashMap::new();

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;

        if state.exit.load(Ordering::Relaxed) {
            break;
        }

        let snapshot = state.process_registry.snapshot();
        let mut changed = false;

        for entry in &snapshot.entries {
            let current_state = entry.proc_state();
            let current_port = entry.active_port();

            if previous_states.get(&entry.backend_id) != Some(&current_state) {
                tracing::debug!(
                    backend_id = %entry.backend_id,
                    old_state = ?previous_states.get(&entry.backend_id),
                    new_state = ?current_state,
                    "Process state changed"
                );
                previous_states.insert(entry.backend_id.clone(), current_state);
                changed = true;
            }
            if previous_ports.get(&entry.backend_id) != Some(&current_port) {
                previous_ports.insert(entry.backend_id.clone(), current_port);
                changed = true;
            }
        }

        // Detect removed entries
        let current_ids: std::collections::HashSet<&str> = snapshot
            .entries
            .iter()
            .map(|e| e.backend_id.as_str())
            .collect();
        let removed: Vec<String> = previous_states
            .keys()
            .filter(|k| !current_ids.contains(k.as_str()))
            .cloned()
            .collect();
        for key in &removed {
            previous_states.remove(key);
            previous_ports.remove(key);
            changed = true;
        }

        if changed {
            crate::cruma_integration::rebuild_cruma_config(state.clone());
        }
    }
}
