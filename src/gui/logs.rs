use std::borrow::Borrow;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::{DateTime, Local};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tracing::Level;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

pub struct LogMsg {
    pub lvl: Level,
    pub msg: String,
    pub src: String,
    pub thread: Option<String>,
}

struct LogVisitor {
    fields: HashMap<String, String>,
}

impl tracing::field::Visit for LogVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{:?}", value));
    }
}

impl LogVisitor {
    fn new() -> Self {
        LogVisitor {
            fields: HashMap::new(),
        }
    }

    fn result(self) -> String {
        self.fields
            .iter()
            .map(|(_key, value)| format!("{}", value))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn is_source_tag(tag: &str) -> bool {
    let tag = tag.trim();
    if tag.is_empty() || tag.contains(char::is_whitespace) || tag.contains(':') {
        return false;
    }
    if tag.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    true
}

/// A single log entry with all metadata.
/// Uses Arc<str> for strings to enable cheap cloning in filtered snapshots.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: DateTime<Local>,
    pub level: Level,
    pub message: Arc<str>,
    pub source: Arc<str>,
    pub thread: Option<Arc<str>>,
    // Pre-computed lowercase versions for fast text search.
    message_lower: Arc<str>,
    source_lower: Arc<str>,
    thread_lower: Option<Arc<str>>,
}

impl From<(u64, LogMsg)> for LogEntry {
    fn from((id, msg): (u64, LogMsg)) -> Self {
        let message_lower: Arc<str> = msg.msg.to_lowercase().into();
        let source_lower: Arc<str> = msg.src.to_lowercase().into();
        let thread_lower: Option<Arc<str>> = msg.thread.as_ref().map(|t| t.to_lowercase().into());
        Self {
            id,
            timestamp: Local::now(),
            level: msg.lvl,
            message: msg.msg.into(),
            source: msg.src.into(),
            thread: msg.thread.map(|t| t.into()),
            message_lower,
            source_lower,
            thread_lower,
        }
    }
}

/// Pre-filtered view published for GUI consumption.
/// The GUI only reads this structure and never scans the full log store.
#[derive(Debug, Clone)]
pub struct FilteredSnapshot {
    pub entries: Arc<Vec<Arc<LogEntry>>>,
    pub total_count: usize,
    pub filtered_count: usize,
    pub known_sources: Vec<String>,
    pub last_filtered_id: Option<u64>,
}

impl Default for FilteredSnapshot {
    fn default() -> Self {
        Self {
            entries: Arc::new(Vec::new()),
            total_count: 0,
            filtered_count: 0,
            known_sources: Vec::new(),
            last_filtered_id: None,
        }
    }
}

enum LogCommand {
    Append(LogMsg),
    SetFilter(LogFilter),
    Clear,
}

/// Shared handle used by producers and GUI.
/// A dedicated background worker owns all logs and filter state.
pub struct LogState {
    cmd_tx: mpsc::UnboundedSender<LogCommand>,
    cmd_rx: Mutex<Option<mpsc::UnboundedReceiver<LogCommand>>>,
    filtered_snapshot: ArcSwap<FilteredSnapshot>,
    max_entries: usize,
}

struct LogWorker {
    logs: VecDeque<Arc<LogEntry>>,
    known_sources: HashSet<String>,
    filter: LogFilter,
    max_entries: usize,
    next_id: u64,
}

impl LogWorker {
    fn new(max_entries: usize) -> Self {
        Self {
            logs: VecDeque::new(),
            known_sources: HashSet::new(),
            filter: LogFilter::default(),
            max_entries,
            next_id: 0,
        }
    }

    fn apply_command(&mut self, cmd: LogCommand) -> bool {
        match cmd {
            LogCommand::Append(msg) => {
                if !msg.src.is_empty() {
                    self.known_sources.insert(msg.src.to_string());
                }
                if let Some(ref thread) = msg.thread {
                    if !thread.is_empty() {
                        self.known_sources.insert(thread.to_string());
                    }
                }

                let id = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                let entry = Arc::new(LogEntry::from((id, msg)));
                self.logs.push_back(entry);

                while self.logs.len() > self.max_entries {
                    self.logs.pop_front();
                }
                true
            }
            LogCommand::SetFilter(filter) => {
                self.filter = filter;
                true
            }
            LogCommand::Clear => {
                self.logs.clear();
                self.known_sources.clear();
                true
            }
        }
    }

    fn rebuild_filtered_snapshot(&self) -> FilteredSnapshot {
        let text_lower = if self.filter.text.is_empty() {
            None
        } else {
            Some(self.filter.text.to_lowercase())
        };

        let filtered_entries: Vec<Arc<LogEntry>> = self
            .logs
            .iter()
            .filter(|e| self.filter.matches_with_text_lower(e.as_ref(), text_lower.as_deref()))
            .cloned()
            .collect();

        let filtered_count = filtered_entries.len();
        let last_filtered_id = filtered_entries.last().map(|e| e.id);

        let mut sources: Vec<String> = self.known_sources.iter().cloned().collect();
        sources.sort();

        FilteredSnapshot {
            entries: Arc::new(filtered_entries),
            total_count: self.logs.len(),
            filtered_count,
            known_sources: sources,
            last_filtered_id,
        }
    }
}

impl LogState {
    pub fn new(max_entries: usize) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        Self {
            cmd_tx,
            cmd_rx: Mutex::new(Some(cmd_rx)),
            filtered_snapshot: ArcSwap::from_pointee(FilteredSnapshot::default()),
            max_entries,
        }
    }

    /// Add a new log message.
    pub fn push(&self, msg: LogMsg) {
        let _ = self.cmd_tx.send(LogCommand::Append(msg));
    }

    /// Clear all entries.
    pub fn clear(&self) {
        let _ = self.cmd_tx.send(LogCommand::Clear);
    }

    /// Set filter criteria (called from GUI thread).
    /// This only sends a command to the worker.
    pub fn set_filter(&self, filter: LogFilter) {
        let _ = self.cmd_tx.send(LogCommand::SetFilter(filter));
    }

    /// Get the pre-computed filtered snapshot (called from GUI thread - very cheap).
    pub fn filtered_snapshot(&self) -> Arc<FilteredSnapshot> {
        self.filtered_snapshot.load_full()
    }

    fn take_worker_receiver(&self) -> Option<mpsc::UnboundedReceiver<LogCommand>> {
        self.cmd_rx.lock().take()
    }
}

/// Thread-safe handle to log state.
pub type SharedLogState = Arc<LogState>;

/// Spawn a background task that owns all log entries and filtering state.
/// This keeps expensive filtering off the GUI thread and publishes only
/// pre-filtered data to readers.
pub fn spawn_filter_task(log_state: SharedLogState) -> tokio::task::JoinHandle<()> {
    let Some(mut rx) = log_state.take_worker_receiver() else {
        return tokio::spawn(async {});
    };

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
        let mut worker = LogWorker::new(log_state.max_entries);

        loop {
            interval.tick().await;

            let mut changed = false;
            loop {
                match rx.try_recv() {
                    Ok(cmd) => {
                        changed |= worker.apply_command(cmd);
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => return,
                }
            }

            if changed {
                let filtered_snapshot = worker.rebuild_filtered_snapshot();
                log_state.filtered_snapshot.store(Arc::new(filtered_snapshot));
            }
        }
    })
}

/// Create a new shared log state.
pub fn create_shared(max_entries: usize) -> SharedLogState {
    Arc::new(LogState::new(max_entries))
}

pub struct GuiLoggerLayer {
    log_state: SharedLogState,
}

impl GuiLoggerLayer {
    pub fn new(log_state: SharedLogState) -> Self {
        Self { log_state }
    }
}

impl<S: Subscriber> Layer<S> for GuiLoggerLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let target = metadata.target();

        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);

        let mut msg = visitor.result();
        let mut src = String::new();

        if msg.starts_with("[") && msg.contains("]") {
            let end = msg.find("]").unwrap_or_default();
            let candidate = msg[1..end].to_string().trim().to_string();
            if is_source_tag(&candidate) {
                src = candidate;
                msg = msg[end + 1..].to_string().trim().to_string();
            }
        }

        if src.is_empty() && !target.ends_with("proc_host") {
            src = target.into();
        }

        let current_thread = std::thread::current();
        let current_thread_name = current_thread
            .name()
            .map(|x| x.to_string())
            .unwrap_or_else(|| "HAH!".to_string());
        let mut skip_src = false;
        let mut thread_name = if current_thread_name == "tokio-runtime-worker" {
            skip_src = true;
            Some(src.to_string())
        } else {
            Some(current_thread_name)
        };

        if let Some(x) = &thread_name {
            if x.is_empty() && skip_src {
                thread_name = Some(metadata.module_path().unwrap_or_default().to_string());
            }
        }

        let log_message = LogMsg {
            thread: thread_name,
            lvl: *metadata.level(),
            src: if skip_src { "".into() } else { src },
            msg,
        };

        self.log_state.push(log_message);
    }
}

/// Filter criteria for log display.
#[derive(Debug, Clone)]
pub struct LogFilter {
    /// Text to search for in message.
    pub text: String,
    /// Minimum log level to show.
    pub min_level: Option<Level>,
    /// Only show entries from these sources (empty = show all).
    pub sources: HashSet<String>,
    /// Show trace level.
    pub show_trace: bool,
    /// Show debug level.
    pub show_debug: bool,
    /// Show info level.
    pub show_info: bool,
    /// Show warn level.
    pub show_warn: bool,
    /// Show error level.
    pub show_error: bool,
}

impl Default for LogFilter {
    fn default() -> Self {
        Self {
            text: String::new(),
            min_level: None,
            sources: HashSet::new(),
            show_trace: true,
            show_debug: true,
            show_info: true,
            show_warn: true,
            show_error: true,
        }
    }
}

impl LogFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if an entry matches this filter, with pre-computed lowercase text filter.
    /// Pass None if self.text is empty, or Some(&lowercase_text) otherwise.
    #[inline]
    pub fn matches_with_text_lower(&self, entry: &LogEntry, text_lower: Option<&str>) -> bool {
        let level_ok = match entry.level {
            Level::TRACE => self.show_trace,
            Level::DEBUG => self.show_debug,
            Level::INFO => self.show_info,
            Level::WARN => self.show_warn,
            Level::ERROR => self.show_error,
        };
        if !level_ok {
            return false;
        }

        if let Some(text_lower) = text_lower {
            let in_message = entry.message_lower.contains(text_lower);
            let in_source = entry.source_lower.contains(text_lower);
            let in_thread = entry
                .thread_lower
                .as_ref()
                .map(|t| t.contains(text_lower))
                .unwrap_or(false);

            if !in_message && !in_source && !in_thread {
                return false;
            }
        }

        if !self.sources.is_empty() {
            let source_match = self.sources.contains(entry.source.borrow() as &str)
                || entry
                    .thread
                    .as_ref()
                    .map(|t| self.sources.contains(t.borrow() as &str))
                    .unwrap_or(false);
            if !source_match {
                return false;
            }
        }

        true
    }
}
