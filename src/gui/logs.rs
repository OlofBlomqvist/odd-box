use chrono::{DateTime, Local};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::Level;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use arc_swap::ArcSwap;
use crossbeam_queue::ArrayQueue;

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

/// A single log entry with all metadata
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: DateTime<Local>,
    pub level: Level,
    pub message: String,
    pub source: String,
    pub thread: Option<String>,
}

impl From<(u64, LogMsg)> for LogEntry {
    fn from((id, msg): (u64, LogMsg)) -> Self {
        Self {
            id,
            timestamp: Local::now(),
            level: msg.lvl,
            message: msg.msg,
            source: msg.src,
            thread: msg.thread,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogSnapshot {
    entries: VecDeque<LogEntry>,
    first_id: Option<u64>,
    last_id: Option<u64>,
    /// All unique sources seen (for filter UI)
    known_sources: HashSet<String>,
}

/// Shared log state that collects and stores log messages
pub struct LogState {
    queue: ArrayQueue<QueuedLog>,
    snapshot: ArcSwap<LogSnapshot>,
    max_entries: usize,
    next_id: AtomicU64,
}

struct QueuedLog {
    id: u64,
    msg: LogMsg,
}

impl LogSnapshot {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            first_id: None,
            last_id: None,
            known_sources: HashSet::new(),
        }
    }

    /// Get all entries (newest last)
    pub fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all known sources (for filter dropdown)
    pub fn known_sources(&self) -> &HashSet<String> {
        &self.known_sources
    }

    /// Get the ID of the most recent entry (for change detection)
    pub fn last_id(&self) -> Option<u64> {
        self.last_id
    }
}

impl LogState {
    pub fn new(max_entries: usize) -> Self {
        let queue_cap = max_entries.saturating_mul(4).max(1024);
        let snapshot = ArcSwap::from_pointee(LogSnapshot::new());
        Self {
            queue: ArrayQueue::new(queue_cap),
            snapshot,
            max_entries,
            next_id: AtomicU64::new(0),
        }
    }

    /// Add a new log message
    pub fn push(&self, msg: LogMsg) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut item = QueuedLog { id, msg };
        loop {
            match self.queue.push(item) {
                Ok(()) => break,
                Err(returned) => {
                    item = returned;
                    let _ = self.queue.pop();
                }
            }
        }
    }

    /// Drain queued log messages and publish a new snapshot.
    /// Returns true if new logs were added.
    pub fn drain(&self) -> bool {
        if self.queue.is_empty() {
            return false;
        }

        let mut drained = Vec::new();
        while let Some(item) = self.queue.pop() {
            drained.push(item);
        }

        if drained.is_empty() {
            return false;
        }

        self.snapshot.rcu(|current| {
            let mut next = (**current).clone();
            for item in drained.drain(..) {
                let msg = item.msg;

                if !msg.src.is_empty() {
                    next.known_sources.insert(msg.src.clone());
                }
                if let Some(ref thread) = msg.thread {
                    if !thread.is_empty() {
                        next.known_sources.insert(thread.clone());
                    }
                }

                let entry = LogEntry::from((item.id, msg));
                next.entries.push_back(entry);
            }

            while next.entries.len() > self.max_entries {
                next.entries.pop_front();
            }

            next.first_id = next.entries.front().map(|e| e.id);
            next.last_id = next.entries.back().map(|e| e.id);

            Arc::new(next)
        });

        true
    }

    /// Get the latest snapshot.
    pub fn snapshot(&self) -> Arc<LogSnapshot> {
        self.snapshot.load_full()
    }

    /// Clear all entries and queued logs.
    pub fn clear(&self) {
        while self.queue.pop().is_some() {}
        self.snapshot.store(Arc::new(LogSnapshot::new()));
    }
}

/// Thread-safe handle to log state
pub type SharedLogState = Arc<LogState>;

/// Create a new shared log state
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

/// Spawn a background task that consumes log messages from the broadcast channel
pub fn spawn_collector(log_state: SharedLogState) -> tokio::task::JoinHandle<()> {
    // TODO: i dont think we should have channels for this but impl a real tracing
    // subscriber sort of thing?
    todo!()
}

/// Filter criteria for log display
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    /// Text to search for in message
    pub text: String,
    /// Minimum log level to show
    pub min_level: Option<Level>,
    /// Only show entries from these sources (empty = show all)
    pub sources: HashSet<String>,
    /// Show trace level
    pub show_trace: bool,
    /// Show debug level
    pub show_debug: bool,
    /// Show info level
    pub show_info: bool,
    /// Show warn level
    pub show_warn: bool,
    /// Show error level
    pub show_error: bool,
}

impl LogFilter {
    pub fn new() -> Self {
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

    /// Check if an entry matches this filter
    pub fn matches(&self, entry: &LogEntry) -> bool {
        // Check level
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

        // Check text filter
        if !self.text.is_empty() {
            let text_lower = self.text.to_lowercase();
            let in_message = entry.message.to_lowercase().contains(&text_lower);
            let in_source = entry.source.to_lowercase().contains(&text_lower);
            let in_thread = entry
                .thread
                .as_ref()
                .map(|t| t.to_lowercase().contains(&text_lower))
                .unwrap_or(false);

            if !in_message && !in_source && !in_thread {
                return false;
            }
        }

        // Check source filter
        if !self.sources.is_empty() {
            let source_match = self.sources.contains(&entry.source)
                || entry
                    .thread
                    .as_ref()
                    .map(|t| self.sources.contains(t))
                    .unwrap_or(false);
            if !source_match {
                return false;
            }
        }

        true
    }

    /// Apply filter to entries and return matching ones
    pub fn apply<'a>(&self, entries: &'a VecDeque<LogEntry>) -> Vec<&'a LogEntry> {
        entries.iter().filter(|e| self.matches(e)).collect()
    }
}
