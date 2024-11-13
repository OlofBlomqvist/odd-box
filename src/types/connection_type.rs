use serde::Serialize;


#[derive(Debug,Clone,Serialize)]
pub enum ConnectionType {
    /// Full-TLS Terminated Proxy: The incoming connection uses TLS and is terminated. HTTP processing occurs,
    /// and a new outgoing connection is established with re-encrypted TLS.
    FullTlsTerminatedWithHttpReEncryptedOutgoingTls,

    /// Mixed Termination: The incoming connection uses TLS, is terminated, HTTP processing occurs,
    /// and a plain (unencrypted) outgoing connection is established.
    MixedTerminationWithPlainOutgoing,

    /// TLS Terminated Proxy with End-to-End TLS: Incoming connection uses TLS and is terminated,
    /// and a new TLS connection is established with the outgoing target.
    TlsTerminatedWithReEncryptedOutgoingTls,

    /// TLS Terminated Proxy: Incoming connection uses TLS, is terminated, and the outgoing connection is plain (unencrypted).
    TlsTerminatedWithPlainOutgoing,

    /// Opaque HTTP with TLS: The incoming connection is plaintext, HTTP is processed, and the outgoing
    /// connection is encrypted with TLS.
    HttpTerminatedWithOutgoingTls,

    /// Plain HTTP Proxy: The incoming connection is plaintext, HTTP is processed, and the outgoing connection
    /// is also plaintext.
    HttpTerminatedWithPlainOutgoing,

    /// Opaque TLS Forwarding: Incoming TLS traffic is not terminated. Data is forwarded as-is to the outgoing
    /// target, preserving end-to-end encryption without decryption or re-encryption.
    OpaqueTlsForwarding,

    /// Plain End-to-End Forwarding: No TLS or HTTP termination occurs. Data is forwarded in plaintext
    /// from incoming to outgoing without modification.
    PlainEndToEndForwarding,

    /// Opaque Incoming TLS Passthrough with Plain Outgoing: Incoming connection uses TLS but is not terminated,
    /// and data is forwarded as plaintext to the outgoing connection.
    OpaqueIncomingTlsPassthroughWithPlainOutgoing,

    /// Opaque Incoming TLS Passthrough with Re-encrypted TLS: Incoming connection uses TLS but is not terminated,
    /// and data is forwarded as opaque to an outgoing TLS connection.
    OpaqueIncomingTlsPassthroughWithOutgoingTls,
}
