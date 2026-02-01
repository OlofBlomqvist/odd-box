# ProcessRegistry Refactor TODO

Goal: replace the current “process host registry” split across `PROC_THREAD_MAP` + `site_status_map` + `proc_broadcaster` (+ ad-hoc cleanup) with a single, well-defined `ProcessRegistry` abstraction that:

- Is the primary source of truth for “which proc_hosts exist” and their current runtime status.
- Is cheap to read from GUI/TUI/Web without async locking (prefer `ArcSwap` for shared snapshots).
- Avoids polling/broadcast fanout where possible, while keeping a clean command interface for start/stop/delete.
- Makes cleanup explicit and reliable when a proc_host exits (shutdown/config removal).

---

## 0. Inventory / Current Behavior (verify & document)

- Confirm proc_host lifecycle: one long-lived `proc_host::host` task per configured process backend; it survives start/stop and only exits on shutdown or removal/replacement.
- Document current responsibilities split:
  - `PROC_THREAD_MAP` (`DashMap<ProcId, ProcInfo>`) = “host liveness + pid + removal flag”
  - `app_state.site_status_map` = “status by backend_id”
  - `proc_broadcaster` (`broadcast::Sender<ProcMessage>`) = commands, polled via `try_recv`
  - websocket/global broadcast channels = logs/status fanout

Deliverable: short doc section in this file linking to the relevant code locations.

### Current code locations (Feb 1, 2026)

- Proc host map / liveness tracking:
  - `src/main.rs` defines `PROC_THREAD_MAP: Arc<DashMap<ProcId, ProcInfo>>`
  - `src/proc_host.rs` inserts/updates it (host registration, pid updates, `marked_for_removal` checks)
  - `src/main.rs` `generic_cleanup_thread` retains entries by `liveness_ptr`
- Runtime status map (UI/TUI/GUI reads):
  - `src/types/app_state.rs` has `AppState.site_status_map: DashMap<String, ProcState>`
  - `src/main.rs` seeds initial values when spawning backends
  - `src/proc_host.rs` updates it via `update_status(...)`
  - `src/tui/mod.rs` and `src/gui/pages/mod.rs` read it for display
- Commands to proc_hosts:
  - `src/main.rs` stores `GlobalState.proc_broadcaster: broadcast::Sender<ProcMessage>`
  - `src/proc_host.rs` receives via `broadcast::Receiver<ProcMessage>` and currently polls with `try_recv()`
  - Message type is `ProcMessage` (`src/control.rs`)

---

## 1. Define the `ProcessRegistry` API

Create `src/process_registry.rs` (or `src/registry/process.rs`) with:

### 1.1 Types

- `ProcessKey` (likely `backend_id: String`; keep `ProcId` if needed for uniqueness).
- `ProcessSnapshot` (read-only UI/Web view):
  - `backend_id`
  - `state` (Running/Stopped/Starting/Stopping/Faulty)
  - `pid` (optional)
  - `active_port` (optional)
  - timestamps (started_at / last_transition / last_exit, optional)
  - `enabled` flag (whether host is enabled to run child)
  - `cancelled`/`alive` (derived from token)
- `RegistrySnapshot`:
  - `version: u64`
  - `entries: Vec<ProcessSnapshot>` (sorted deterministically for stable UI)

### 1.2 Storage strategy (recommended)

- Membership snapshot: `ArcSwap<RegistrySnapshot>` (or `ArcSwap<Arc<RegistrySnapshot>>`).
- Per-entry live state:
  - Either (A) store everything in the snapshot and only update snapshot on change (simpler; okay if change rate is low),
  - Or (B) store per-entry state behind atomics/`ArcSwap` and only refresh the snapshot periodically (more complex; best for high update rates).

Given odd-box behavior (few processes, updates not extremely high), start with (A).

### 1.3 Lifecycle signals

- Each proc_host gets a `CancellationToken`.
- Inside the proc_host task, create a local `_guard = token.drop_guard()` so the token is cancelled when the task exits.
- The registry entry stores a clone of the token to expose `is_cancelled()` to readers.

### 1.4 Commands (keep this simple)

Even with ArcSwap for state distribution, keep a *single* command interface:

- `ProcessController`:
  - `start(backend_id)`
  - `stop(backend_id)`
  - `start_all()`
  - `stop_all()`
  - `delete(backend_id)` (or “remove”)

Implementation options:
- `tokio::sync::mpsc::Sender<ProcCommand>` (preferred; point-to-point, no polling).
- If multiple consumers truly needed: keep broadcast, but avoid polling (`recv().await` instead of `try_recv` loops).

Deliverable: compileable API + docs.

---

## 2. Implement `ProcessRegistry` internals

- Provide functions:
  - `register_host(backend_id, token, initial_state, ...)`
  - `update_state(backend_id, new_state, pid, port, ...)`
  - `mark_removed(backend_id)` (for config reload removal)
  - `cleanup_cancelled()` (optional; can be called from a low-frequency background task)
  - `snapshot()` returns `Arc<RegistrySnapshot>` synchronously.

Ensure updates are:
- O(1) to find entry (use `HashMap` internally when rebuilding snapshot).
- Deterministic order of `entries` (sort by backend_id) to avoid UI churn.

---

## 3. Migrate `proc_host` to the new registry

### 3.1 Replace `PROC_THREAD_MAP` writes

- On proc_host start, call `registry.register_host(...)`.
- Replace `marked_for_removal` usage with an explicit “removed” command/state in the registry and/or a per-host cancellation token.
- Ensure proc_host updates the registry state on transitions (Starting/Running/Stopping/Stopped/Faulty).

### 3.2 Replace command delivery mechanism

- Stop using `broadcast::Receiver::try_recv()` in hot loops.
- Preferred:
  - each proc_host has a `watch::Receiver<DesiredState>` or `mpsc::Receiver<ProcCommand>` routed by backend_id.
  - or a shared `DashMap<backend_id, watch::Sender<DesiredState>>`.

Goal: no polling loops that repeatedly scan messages.

### 3.3 Cleanup semantics

- On task exit, token cancels via drop_guard.
- A background cleanup pass removes cancelled entries from the registry snapshot.

---

## 4. Update configuration reload integration

- When config reload removes a process backend:
  - send `delete(backend_id)` via controller to stop & exit that proc_host (or cancel its token).
- When config reload adds a new process backend:
  - spawn new proc_host + register it.
- When config reload changes a backend but keeps the same `backend_id`:
  - decide whether to restart in-place or spawn a new host and retire the old one (document this policy).

Eliminate dependencies on scanning `PROC_THREAD_MAP` for “marked_for_removal”.

---

## 5. Migrate UI/TUI/Web to read from `ProcessRegistry`

### 5.1 GUI

- Remove `fetch_config` refresh-on-navigation for process-related pages.
- Read the registry snapshot synchronously in `view_*` functions.
- Use `lazy` only where it materially helps, but prefer stable snapshots to reduce rebuilds.

### 5.2 TUI

- Replace `global_state.config.read().await` polling with:
  - registry snapshot (process statuses),
  - and/or a separate config snapshot if needed.
 
---

## 6. Logs registry (follow the same pattern)

Create `LogRegistry` with:

- `ArcSwap<Arc<LogSnapshot>>` where `LogSnapshot` contains:
  - ring buffer of last N log lines (already formatted for UI)
  - `version: u64`
- Producers append and publish a new snapshot (or keep ring buffer behind lock + version counter).
- GUI/web/tui read snapshot synchronously; no broadcast collector needed.

---

## 7. Remove legacy globals (only after migrations)

- Remove `PROC_THREAD_MAP` and any cleanup loops depending on it.
- Remove/replace `proc_broadcaster` usage from `GlobalState`. 

---

## 8. Validation / Benchmarks

- Add a “stress” mode (dev-only) that:
  - generates lots of log lines,
  - toggles pages rapidly,
  - measures time to switch tabs and time per update.
- Acceptance criteria:
  - Switching between Backends/Frontends is “instant” (no 500ms stalls).
  - Monitoring/log view scroll remains responsive while logs stream.
  - No periodic heavy work on the UI thread unless data actually changes.
