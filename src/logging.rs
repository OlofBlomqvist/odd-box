use std::collections::VecDeque;
use std::{collections::HashMap, sync::Arc};

use std::sync::Mutex;
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

#[derive(Clone)]
pub (crate) struct LogMsg {
    pub (crate) msg: String,
    pub (crate) lvl: tracing::Level,
    pub (crate) src: String,
    pub (crate) thread: Option<String>
}



struct LogVisitor {
    fields: HashMap<String, String>,
}

impl tracing::field::Visit for LogVisitor {

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {        
        self.fields.insert(field.name().to_string(), format!("{:?}",value));
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

pub struct NonTuiLoggerLayer {
    pub broadcaster: tokio::sync::broadcast::Sender<String>
}
impl<S: Subscriber> tracing_subscriber::Layer<S> for NonTuiLoggerLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {

        let metadata = event.metadata();
        let target = metadata.target();
        
        // Create a visitor to format the fields of the event.
        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);
        
        let mut msg =  visitor.result();
        let mut src = String::new();

        if msg.starts_with("[") && msg.contains("]") {
            let end = msg.find("]").unwrap_or_default();
            src = msg[1..end].to_string().trim().to_string();
            msg = msg[end+1..].to_string().trim().to_string();
        }

        if src.is_empty() {
            if !target.ends_with("proc_host") {
                src = target.into();
            }
        }

        let current_thread = std::thread::current();
        let current_thread_name = current_thread.name().and_then(|x|Some(x.to_string())).unwrap_or(format!("HAH!"));
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
            src: if skip_src { "".into() } else {src},
            msg,
        };
        
        _ = self.broadcaster.send(serde_json::to_string_pretty(&log_message).expect("should always be possible to serialize log messages"));

    
    }
}




pub struct SharedLogBuffer {
    pub (crate) logs: VecDeque<LogMsg>,
    pub (crate) limit : Option<usize>
}

impl SharedLogBuffer {
    
    pub fn new() -> Self {
        SharedLogBuffer {
            logs: VecDeque::new(),
            limit: Some(500)
        }
    }

    fn push(&mut self, message: LogMsg) {

        self.logs.push_back(message);
        match self.limit {
            Some(x) => {
                while self.logs.len() > x {
                    self.logs.pop_front();
                }
            },
            None => {},
        }
        
    }


}

pub struct TuiLoggerLayer {
    pub log_buffer: Arc<Mutex<SharedLogBuffer>>,
    pub broadcaster: tokio::sync::broadcast::Sender<String>
}

impl<S: Subscriber> Layer<S> for TuiLoggerLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();


        let target = metadata.target();
        
        // Create a visitor to format the fields of the event.
        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);
        
        let mut msg =  visitor.result();
        let mut src = String::new();
        
        if msg.starts_with("[") && msg.contains("]") {
            let end = msg.find("]").unwrap_or_default();
            src = msg[1..end].to_string().trim().to_string();
            msg = msg[end+1..].to_string().trim().to_string();
        }

        if src.is_empty() {
            if !target.ends_with("proc_host") {
                src = target.into();
            }
            
        }

        let current_thread = std::thread::current();
        let current_thread_name = current_thread.name().and_then(|x|Some(x.to_string())).unwrap_or(format!("HAH!"));
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
            src: if skip_src { "".into() } else {src},
            msg,
        };
        
        _ = self.broadcaster.send(serde_json::to_string_pretty(&log_message).expect("should always be possible to serialize log messages"));
        let mut buffer = self.log_buffer.lock().expect("must always be able to lock log buffer");
        buffer.push(log_message.clone());
        
    
    }
}

