mod tls;
mod http1;
mod http2;
mod tcp;
mod managed_stream;
pub mod h1_parser;
pub mod h2_parser;
pub mod h1_initial_parser;
pub use managed_stream::ManagedStream;
pub use managed_stream::GenericManagedStream;

pub use managed_stream::Peekable;
pub use tcp::*;
