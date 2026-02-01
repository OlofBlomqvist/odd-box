use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use arc_swap::ArcSwap;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

pub type ProcessKey = String;

#[derive(Clone, Debug)]
pub struct ProcessSnapshot {
    pub backend_id: String,
    pub state: crate::global_state::ProcState,
    pub pid: Option<u32>,
    pub active_port: Option<u16>,
    pub started_at: Option<SystemTime>,
    pub last_transition_at: Option<SystemTime>,
    pub last_exit_at: Option<SystemTime>,
    pub enabled: bool,
    pub is_cancelled: bool,
    pub is_removed: bool,
}

#[derive(Clone, Debug)]
pub struct RegistrySnapshot {
    pub version: u64,
    pub entries: Vec<ProcessSnapshot>,
}

#[derive(Debug)]
struct ProcessEntry {
    backend_id: String,
    state: crate::global_state::ProcState,
    pid: Option<u32>,
    active_port: Option<u16>,
    started_at: Option<SystemTime>,
    last_transition_at: Option<SystemTime>,
    last_exit_at: Option<SystemTime>,
    enabled: bool,
    token: CancellationToken,
    removed: bool,
}

#[derive(Debug, Default)]
struct Inner {
    version: u64,
    entries: HashMap<ProcessKey, ProcessEntry>,
}

/// Read-optimized registry for proc_host lifecycle/state.
///
/// - Readers call `snapshot()` which is synchronous and lock-free (`ArcSwap`).
/// - Writers update the internal map under a small mutex, then publish a new
///   snapshot (fine for odd-box's low mutation rate).
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
    pub fn snapshot(&self) -> Arc<RegistrySnapshot> {
        self.snapshot.load_full()
    }

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
                state: initial_state,
                pid: None,
                active_port,
                started_at: None,
                last_transition_at: Some(now),
                last_exit_at: None,
                enabled,
                token,
                removed: false,
            },
        );
        self.publish_locked(&mut inner);
    }

    pub fn update_state(
        &self,
        backend_id: &str,
        state: crate::global_state::ProcState,
        pid: Option<u32>,
        active_port: Option<u16>,
        enabled: Option<bool>,
    ) {
        let is_running = state == crate::global_state::ProcState::Running;
        let mut inner = self.inner.lock();
        let Some(entry) = inner.entries.get_mut(backend_id) else {
            return;
        };

        let now = SystemTime::now();
        if entry.state != state {
            if entry.state == crate::global_state::ProcState::Running && state != crate::global_state::ProcState::Running {
                entry.last_exit_at = Some(now);
            }
            entry.state = state.clone();
            entry.last_transition_at = Some(now);
        }
        if let Some(pid) = pid {
            entry.pid = Some(pid);
        } else if !is_running {
            entry.pid = None;
        }
        if let Some(port) = active_port {
            entry.active_port = Some(port);
        }
        if let Some(enabled) = enabled {
            entry.enabled = enabled;
        }

        if is_running && entry.started_at.is_none() {
            entry.started_at = Some(now);
        }

        self.publish_locked(&mut inner);
    }

    pub fn mark_removed(&self, backend_id: &str) {
        let mut inner = self.inner.lock();
        if let Some(entry) = inner.entries.get_mut(backend_id) {
            entry.removed = true;
            entry.last_transition_at = Some(SystemTime::now());
            self.publish_locked(&mut inner);
        }
    }

    /// Drops entries whose host task has exited (token cancelled) or was marked removed.
    pub fn cleanup_cancelled(&self) {
        let mut inner = self.inner.lock();
        inner
            .entries
            .retain(|_, e| !(e.removed || e.token.is_cancelled()));
        self.publish_locked(&mut inner);
    }

    fn publish_locked(&self, inner: &mut Inner) {
        inner.version = inner.version.wrapping_add(1);
        let mut entries: Vec<_> = inner
            .entries
            .values()
            .map(|e| ProcessSnapshot {
                backend_id: e.backend_id.clone(),
                state: e.state.clone(),
                pid: e.pid,
                active_port: e.active_port,
                started_at: e.started_at,
                last_transition_at: e.last_transition_at,
                last_exit_at: e.last_exit_at,
                enabled: e.enabled,
                is_cancelled: e.token.is_cancelled(),
                is_removed: e.removed,
            })
            .collect();
        entries.sort_by(|a, b| a.backend_id.cmp(&b.backend_id));
        self.snapshot.store(Arc::new(RegistrySnapshot {
            version: inner.version,
            entries,
        }));
    }
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}
