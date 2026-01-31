use parking_lot::RwLock;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::Level;

use crate::logging::LogMsg;
use crate::types::odd_box_event::EventForWebsocketClients;

/// A single log entry with all metadata
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: std::time::Instant,
    pub level: Level,
    pub message: String,
    pub source: String,
    pub thread: Option<String>,
}

impl From<(u64, LogMsg)> for LogEntry {
    fn from((id, msg): (u64, LogMsg)) -> Self {
        Self {
            id,
            timestamp: std::time::Instant::now(),
            level: msg.lvl,
            message: msg.msg,
            source: msg.src,
            thread: msg.thread,
        }
    }
}

/// Shared log state that collects and stores log messages
pub struct LogState {
    entries: VecDeque<LogEntry>,
    max_entries: usize,
    next_id: u64,
    /// All unique sources seen (for filter UI)
    known_sources: HashSet<String>,
}

impl LogState {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            next_id: 0,
            known_sources: HashSet::new(),
        }
    }

    /// Add a new log message
    pub fn push(&mut self, msg: LogMsg) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        // Track sources for filtering
        if !msg.src.is_empty() {
            self.known_sources.insert(msg.src.clone());
        }
        if let Some(ref thread) = msg.thread {
            if !thread.is_empty() {
                self.known_sources.insert(thread.clone());
            }
        }

        let entry = LogEntry::from((id, msg));
        self.entries.push_back(entry);

        // Remove oldest if over capacity
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
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

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get max entries setting
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Update max entries (will trim if needed)
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max;
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }
}

/// Thread-safe handle to log state
pub type SharedLogState = Arc<RwLock<LogState>>;

/// Create a new shared log state
pub fn create_shared(max_entries: usize) -> SharedLogState {
    Arc::new(RwLock::new(LogState::new(max_entries)))
}

/// Spawn a background task that consumes log messages from the broadcast channel
pub fn spawn_collector(
    log_state: SharedLogState,
    mut receiver: broadcast::Receiver<EventForWebsocketClients>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(EventForWebsocketClients::Log(msg)) => {
                    log_state.write().push(msg);
                }
                Ok(_) => {
                    // Ignore non-log events
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Log collector lagged, missed {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::debug!("Log broadcast channel closed");
                    break;
                }
            }
        }
    })
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
