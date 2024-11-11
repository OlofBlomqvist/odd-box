mod tls;
mod http1;
mod http2;
mod tcp;
mod managed_stream;
mod h1_parser;
mod h2_parser;
mod h1_initial_parser;
pub use managed_stream::ManagedStream;
pub use managed_stream::GenericManagedStream;

pub use managed_stream::Peekable;
pub use tcp::*;
