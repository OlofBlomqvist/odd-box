use tokio::sync::mpsc::Sender;

/// Minimal process control messages used by hosted process management.
#[derive(Clone, Debug)]
pub enum ProcMessage {
    StartAll,
    StopAll,
    Start(String),
    Stop(String),
    Delete(String, Sender<u8>),
}
