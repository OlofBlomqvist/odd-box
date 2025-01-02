use serde::Serialize;

use crate::{configuration::Backend, tcp_proxy::ReverseTcpProxyTarget};


#[derive(Debug,Clone,Serialize,Eq,PartialEq)]
pub enum ConnectionType {
    
    /// Incoming connection is TLS and we terminated it, then established a clear text tunnel towards a specific backend
    /// in which we just tunnel the original data to and from the client.
    /// We can see all data going back and forth in unencrypted form.
    TlsTerminatedWithOutgoingClearText(ReverseTcpProxyTarget,Backend),

    /// Incoming connection is TLS and we terminated it, then established a new TLS connection towards a specific backend.
    /// We can see all data going back and forth in unencrypted form.
    TlsTerminatedWithOutgoingTLS(ReverseTcpProxyTarget,Backend),

    /// Incoming is tls which we terminated, and we also terminate http requests. as such, we do not have a specific tcp tunnel 
    /// set up, but instead handle each http request in this tcp session separately. 
    TlsTermination,

    /// Incoming is TLS and we just looked at the SNI then established a raw TCP connection to some backend, blindly
    /// forwarding the traffic between the client and backend.
    /// We are unable to see any of the data in clear text.
    TlsPassthru(ReverseTcpProxyTarget,Backend),

    /// Incoming is clear text and we just established a raw TCP connection to some backend, forwarding data 
    /// between client and backend without modifying it in any way.
    HttpPassthru(ReverseTcpProxyTarget,Backend),

    /// Incoming is clear text. We terminate http requests here: no tcp tunnel is established to any backend for this connection;
    /// that is handled per http requests received in the hyper/terminating proxy service.
    HttpTermination,

    /// Incoming is clear text but we have set up a TLS tunnel towards a specific backend and just forward data between
    /// client and backend. No per-http request routing-logic happening in here.
    HttpWithOutgoingTLS(ReverseTcpProxyTarget,Backend),

    /// Before we have detected enough information about the incoming tcp connection
    PendingInit,

    /// Something is fishy
    Invalid(String)
}
