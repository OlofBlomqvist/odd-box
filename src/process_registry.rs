use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use arc_swap::ArcSwap;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

pub type ProcessKey = String;

/// Dynamic state that can change without rebuilding the snapshot.
#[derive(Clone, Debug)]
pub struct ProcessState {
    pub proc_state: crate::global_state::ProcState,
    pub pid: Option<u32>,
    pub active_port: Option<u16>,
    pub started_at: Option<SystemTime>,
    pub last_transition_at: Option<SystemTime>,
    pub last_exit_at: Option<SystemTime>,
    pub enabled: bool,
}

/// A handle to a registered backend with live state access.
/// State is always current (accessed via Arc indirection).
#[derive(Clone, Debug)]
pub struct ProcessHandle {
    pub backend_id: String,
    state: Arc<ArcSwap<ProcessState>>,
    token: Option<CancellationToken>,
    marked_for_removal: Arc<AtomicBool>,
}

impl ProcessHandle {
    /// Get current state (always live, not a point-in-time copy).
    pub fn state(&self) -> ProcessState {
        (**self.state.load()).clone()
    }

    /// Get just the proc_state enum.
    pub fn proc_state(&self) -> crate::global_state::ProcState {
        self.state.load().proc_state.clone()
    }

    /// Check if the proc_host has exited (token cancelled).
    pub fn is_cancelled(&self) -> bool {
        self.token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false)
    }

    /// Check if this entry is marked for removal.
    pub fn is_marked_for_removal(&self) -> bool {
        self.marked_for_removal.load(Ordering::Relaxed)
    }

    /// Get the active port if set.
    pub fn active_port(&self) -> Option<u16> {
        self.state.load().active_port
    }
}

/// The registry snapshot - list of backends (only rebuilt on add/remove).
/// Individual entry state is live via Arc indirection.
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
    pub fn state_of(&self, backend_id: &str) -> Option<crate::global_state::ProcState> {
        self.get(backend_id).map(|e| e.proc_state())
    }
}

/// Internal entry stored in the registry.
#[derive(Debug)]
struct ProcessEntry {
    backend_id: String,
    token: Option<CancellationToken>,
    state: Arc<ArcSwap<ProcessState>>,
    marked_for_removal: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
struct Inner {
    version: u64,
    entries: HashMap<ProcessKey, ProcessEntry>,
}

/// Registry for proc_host lifecycle and backend state.
///
/// - Snapshot is only rebuilt on structural changes (add/remove backends)
/// - State updates use interior mutability (no snapshot rebuild)
/// - Readers get live state through Arc indirection
#[derive(Debug)]
pub struct ProcessRegistry {
    snapshot: ArcSwap<RegistrySnapshot>,
    inner: Mutex<Inner>,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self {
            snapshot: ArcSwap::from_pointee(RegistrySnapshot {
                version: 0,
                entries: Vec::new(),
            }),
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Returns the latest registry snapshot, synchronously.
    /// State within entries is always live (not point-in-time).
    pub fn snapshot(&self) -> Arc<RegistrySnapshot> {
        self.snapshot.load_full()
    }

    /// Register a hosted process backend with a cancellation token.
    /// The token should be created by the caller and passed to the proc_host.
    /// When the proc_host exits, it should cancel the token (via drop_guard).
    pub fn register_host(
        &self,
        backend_id: impl Into<String>,
        token: CancellationToken,
        initial_state: crate::global_state::ProcState,
        enabled: bool,
        active_port: Option<u16>,
    ) {
        let backend_id = backend_id.into();
        let now = SystemTime::now();
        let mut inner = self.inner.lock();
        inner.entries.insert(
            backend_id.clone(),
            ProcessEntry {
                backend_id,
                token: Some(token),
                state: Arc::new(ArcSwap::from_pointee(ProcessState {
                    proc_state: initial_state,
                    pid: None,
                    active_port,
                    started_at: None,
                    last_transition_at: Some(now),
                    last_exit_at: None,
                    enabled,
                })),
                marked_for_removal: Arc::new(AtomicBool::new(false)),
            },
        );
        self.publish_locked(&inner);
    }

    /// Register a non-process backend (Remote, Static, Docker) without a cancellation token.
    pub fn register_backend(
        &self,
        backend_id: impl Into<String>,
        state: crate::global_state::ProcState,
    ) {
        let backend_id = backend_id.into();
        let now = SystemTime::now();
        let mut inner = self.inner.lock();
        inner.entries.insert(
            backend_id.clone(),
            ProcessEntry {
                backend_id,
                token: None,
                state: Arc::new(ArcSwap::from_pointee(ProcessState {
                    proc_state: state,
                    pid: None,
                    active_port: None,
                    started_at: None,
                    last_transition_at: Some(now),
                    last_exit_at: None,
                    enabled: true,
                })),
                marked_for_removal: Arc::new(AtomicBool::new(false)),
            },
        );
        self.publish_locked(&inner);
    }

    /// Update the state of a backend. Does NOT rebuild the snapshot.
    pub fn update_state(
        &self,
        backend_id: &str,
        state: crate::global_state::ProcState,
        pid: Option<u32>,
        active_port: Option<u16>,
        enabled: Option<bool>,
    ) {
        let inner = self.inner.lock();
        let Some(entry) = inner.entries.get(backend_id) else {
            return;
        };

        let now = SystemTime::now();
        let is_running = state == crate::global_state::ProcState::Running;
        let current = entry.state.load();

        let mut new_state = (**current).clone();

        if new_state.proc_state != state {
            if new_state.proc_state == crate::global_state::ProcState::Running
                && state != crate::global_state::ProcState::Running
            {
                new_state.last_exit_at = Some(now);
            }
            new_state.proc_state = state;
            new_state.last_transition_at = Some(now);
        }

        if let Some(pid) = pid {
            new_state.pid = Some(pid);
        } else if !is_running {
            new_state.pid = None;
        }

        if let Some(port) = active_port {
            new_state.active_port = Some(port);
        }

        if let Some(enabled) = enabled {
            new_state.enabled = enabled;
        }

        if is_running && new_state.started_at.is_none() {
            new_state.started_at = Some(now);
        }

        entry.state.store(Arc::new(new_state));
        // No snapshot rebuild needed!
    }

    /// Mark a backend for removal. Returns the token if present so caller can await exit.
    /// The proc_host should check `is_marked_for_removal()` and exit when true.
    pub fn mark_for_removal(&self, backend_id: &str) -> Option<CancellationToken> {
        let inner = self.inner.lock();
        if let Some(entry) = inner.entries.get(backend_id) {
            entry.marked_for_removal.store(true, Ordering::SeqCst);
            return entry.token.clone();
        }
        None
    }

    /// Check if a backend is marked for removal.
    pub fn is_marked_for_removal(&self, backend_id: &str) -> bool {
        let inner = self.inner.lock();
        inner
            .entries
            .get(backend_id)
            .map(|e| e.marked_for_removal.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    /// Set the enabled state for a backend (used for start/stop from GUI).
    /// Returns true if the backend was found and updated.
    pub fn set_enabled(&self, backend_id: &str, enabled: bool) -> bool {
        let inner = self.inner.lock();
        if let Some(entry) = inner.entries.get(backend_id) {
            let current = entry.state.load();
            let mut new_state = (**current).clone();
            new_state.enabled = enabled;
            entry.state.store(Arc::new(new_state));
            return true;
        }
        false
    }

    /// Check if a backend is enabled.
    pub fn is_enabled(&self, backend_id: &str) -> bool {
        let inner = self.inner.lock();
        inner
            .entries
            .get(backend_id)
            .map(|e| e.state.load().enabled)
            .unwrap_or(false)
    }

    /// Remove entries that are marked for removal AND whose token is cancelled (proc_host exited).
    /// For non-process backends, removes if marked for removal.
    pub fn cleanup_finished(&self) {
        let mut inner = self.inner.lock();
        let before = inner.entries.len();
        inner.entries.retain(|_, e| {
            let marked = e.marked_for_removal.load(Ordering::Relaxed);
            if !marked {
                return true;
            }
            // Marked for removal - check if it's actually finished
            match &e.token {
                Some(token) => !token.is_cancelled(), // Keep if not yet cancelled
                None => false,                        // No token = remove immediately
            }
        });
        if inner.entries.len() != before {
            self.publish_locked(&inner);
        }
    }

    /// Remove an entry immediately (for non-process backends like docker).
    pub fn remove(&self, backend_id: &str) {
        let mut inner = self.inner.lock();
        if inner.entries.remove(backend_id).is_some() {
            self.publish_locked(&inner);
        }
    }

    fn publish_locked(&self, inner: &Inner) {
        let mut entries: Vec<_> = inner
            .entries
            .values()
            .map(|e| ProcessHandle {
                backend_id: e.backend_id.clone(),
                state: e.state.clone(),
                token: e.token.clone(),
                marked_for_removal: e.marked_for_removal.clone(),
            })
            .collect();
        entries.sort_by(|a, b| a.backend_id.cmp(&b.backend_id));

        let version = inner.version.wrapping_add(1);
        self.snapshot.store(Arc::new(RegistrySnapshot { version, entries }));
    }

    /// Mark all process backends for removal (used during app shutdown).
    pub fn mark_all_for_removal(&self) {
        let inner = self.inner.lock();
        for entry in inner.entries.values() {
            if entry.token.is_some() {
                entry.marked_for_removal.store(true, Ordering::SeqCst);
            }
        }
    }
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}
