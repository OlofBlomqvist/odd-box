use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crossbeam_queue::ArrayQueue;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

#[derive(Debug, Clone)]
pub struct LogMsg {
    pub msg: String,
    pub lvl: tracing::Level,
    pub src: String,
    pub thread: Option<String>,
}

impl serde::Serialize for LogMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("LogMsg", 4)?;
        state.serialize_field("msg", &self.msg)?;
        state.serialize_field("lvl", &self.lvl.as_str())?;
        state.serialize_field("src", &self.src)?;
        state.serialize_field("thread", &self.thread)?;
        state.end()
    }
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

pub struct NonTuiLoggerLayer {}
impl<S: Subscriber> tracing_subscriber::Layer<S> for NonTuiLoggerLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let target = metadata.target();

        // Create a visitor to format the fields of the event.
        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);

        let mut msg = visitor.result();
        let mut src = String::new();

        if msg.starts_with("[") && msg.contains("]") {
            let end = msg.find("]").unwrap_or_default();
            src = msg[1..end].to_string().trim().to_string();
            msg = msg[end + 1..].to_string().trim().to_string();
        }

        if src.is_empty() {
            if !target.ends_with("proc_host") {
                src = target.into();
            }
        }

        let current_thread = std::thread::current();
        let current_thread_name = current_thread
            .name()
            .and_then(|x| Some(x.to_string()))
            .unwrap_or(format!("HAH!"));
        let mut skip_src = false;
        let thread_name = if current_thread_name == "tokio-runtime-worker" {
            skip_src = true;
            Some(src.to_string())
        } else {
            Some(current_thread_name)
        };

        let log_message = LogMsg {
            thread: thread_name.clone(),
            lvl: metadata.level().clone(),
            src: if skip_src { "".into() } else { src },
            msg,
        };

        let _ = log_message;
    }
}

#[derive(Debug)]
pub struct SharedLogBuffer {
    logs: ArrayQueue<LogMsg>,
    limit: AtomicUsize,
    pause: AtomicBool,
    capacity: usize,
}

impl SharedLogBuffer {
    pub fn new() -> Self {
        let capacity = 1000;
        SharedLogBuffer {
            logs: ArrayQueue::new(capacity),
            limit: AtomicUsize::new(500),
            pause: AtomicBool::new(false),
            capacity,
        }
    }

    fn effective_limit(&self) -> usize {
        let limit = self.limit.load(Ordering::Relaxed);
        if limit == 0 {
            self.capacity
        } else {
            limit.min(self.capacity)
        }
    }

    fn push(&self, message: LogMsg) {
        if self.pause.load(Ordering::Relaxed) {
            return;
        }

        let limit = self.effective_limit();
        let mut msg = message;
        loop {
            while self.logs.len() >= limit {
                let _ = self.logs.pop();
            }
            match self.logs.push(msg) {
                Ok(()) => break,
                Err(m) => {
                    msg = m;
                    let _ = self.logs.pop();
                }
            }
        }
    }

    pub fn drain(&self) -> Vec<LogMsg> {
        let mut out = Vec::new();
        while let Some(msg) = self.logs.pop() {
            out.push(msg);
        }
        out
    }

    pub fn clear(&self) {
        while self.logs.pop().is_some() {}
    }
}

pub struct TuiLoggerLayer {
    pub log_buffer: Arc<SharedLogBuffer>,
}

impl<S: Subscriber> Layer<S> for TuiLoggerLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        let target = metadata.target();

        // Create a visitor to format the fields of the event.
        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);

        let mut msg = visitor.result();
        let mut src = String::new();

        if msg.starts_with("[") && msg.contains("]") {
            let end = msg.find("]").unwrap_or_default();
            src = msg[1..end].to_string().trim().to_string();
            msg = msg[end + 1..].to_string().trim().to_string();
        }

        if src.is_empty() {
            if !target.ends_with("proc_host") {
                src = target.into();
            }
        }

        let current_thread = std::thread::current();
        let current_thread_name = current_thread
            .name()
            .and_then(|x| Some(x.to_string()))
            .unwrap_or(format!("HAH!"));
        let mut skip_src = false;
        let mut thread_name = if current_thread_name == "tokio-runtime-worker" {
            skip_src = true;
            Some(src.to_string())
        } else {
            Some(current_thread_name)
        };

        if let Some(x) = &thread_name {
            if x == "" && skip_src {
                thread_name = Some(metadata.module_path().unwrap_or_default().to_string());
            }
        }

        let log_message = LogMsg {
            thread: thread_name.clone(),
            lvl: metadata.level().clone(),
            src: if skip_src { "".into() } else { src },
            msg,
        };

        self.log_buffer.push(log_message.clone());
    }
}
