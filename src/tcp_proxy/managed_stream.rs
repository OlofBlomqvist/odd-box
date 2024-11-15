use anyhow::bail;
use hyper::Version;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use tracing::trace;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use std::vec;
use bytes::BytesMut;
use std::io::{Error, ErrorKind};
use crate::global_state::GlobalState;
use crate::tcp_proxy::DataType;
use crate::types::proxy_state::{ConnectionKey, ProxyActiveTCPConnection};

use super::h1_initial_parser::ParsedHttpRequest;
use super::tls::client_hello::TlsClientHello;
use super::{h2_parser, PeekError, PeekResult};


pub trait Peekable {
    async fn peek_async(&mut self) -> anyhow::Result<(bool,Vec<u8>)>;
    fn seal(&mut self);
}
#[allow(dead_code)]
#[derive(Debug)]
pub struct ManagedStream<T> where T: AsyncRead + AsyncWrite + Unpin {
    global_state : std::sync::Arc<crate::global_state::GlobalState>,
    enable_inspection: bool,
    pub stream: T,
    buffer: BytesMut,
    sealed: bool,
    h2_observer: h2_parser::H2Observer,
    h1_in: BytesMut,
    h1_out: BytesMut,
    pub tcp_connection_id: Arc<ConnectionKey>,
    pub events : Vec<String>,
    // fields that will be set once we know more about the stream
    pub is_tls: bool,
    pub is_tls_terminated: bool,
    pub is_http_terminated: bool,
    pub is_wrapped: bool,
    pub http_version: Option<Version>,
    pub is_websocket: bool,
    pub contains_h2c_upgrade_header: bool,


    // State to manage reading and calling test
    //state: ReadState,

}


#[derive(Debug)]
pub enum GenericManagedStream{
    // TLS(PeekableTlsStream),
    TCP(ManagedStream<TcpStream>),
    TerminatedTLS(ManagedStream<TlsStream<ManagedStream<TcpStream>>>)
}
impl GenericManagedStream {
    pub fn add_event(&mut self, event: String) {
        match self {
            GenericManagedStream::TCP(peekable_tcp_stream) => peekable_tcp_stream.events.push(event),
            GenericManagedStream::TerminatedTLS(stream) => stream.events.push(event)
        }
    }
    pub fn global_state(&self) -> std::sync::Arc<crate::global_state::GlobalState> {
        match self {
            // GenericManagedStream::TLS(peekable_tls_stream) => peekable_tls_stream.managed_tls_stream.global_state.clone(),
            GenericManagedStream::TCP(peekable_tcp_stream) => peekable_tcp_stream.global_state.clone(),
            GenericManagedStream::TerminatedTLS(stream) => stream.global_state.clone()
        }
    }
    pub fn mark_as_tls(&mut self) {
        match self {
            // GenericManagedStream::TLS(peekable_tls_stream) => peekable_tls_stream.managed_tls_stream.is_tls = true,
            GenericManagedStream::TCP(peekable_tcp_stream) => peekable_tcp_stream.is_tls = true,
            GenericManagedStream::TerminatedTLS(stream) => stream.is_tls = true
        }
    }
    pub fn get_id(&self) -> Arc<ConnectionKey> {
        match self {
            // GenericManagedStream::TLS(peekable_tls_stream) => peekable_tls_stream.managed_tls_stream.tcp_connection_id,
            GenericManagedStream::TCP(peekable_tcp_stream) => peekable_tcp_stream.tcp_connection_id.clone(),
            GenericManagedStream::TerminatedTLS(stream) => stream.tcp_connection_id.clone()
        }
    }
}

impl Peekable for GenericManagedStream {
    async fn peek_async(&mut self) -> anyhow::Result<(bool,Vec<u8>)> {
        match self {
           // GenericManagedStream::TLS(peekable_tls_stream) => Ok(peekable_tls_stream.managed_tls_stream.peek_async().await.map_err(|e| anyhow::Error::msg(e))?),
            GenericManagedStream::TCP(peekable_tcp_stream) => Ok(peekable_tcp_stream.peek_async().await.map_err(|e| anyhow::Error::msg(e))?),
            GenericManagedStream::TerminatedTLS(peekable_tcp_stream) => {
                Ok(peekable_tcp_stream.peek_async().await.map_err(|e| anyhow::Error::msg(e))?)
            }
            
        }
    }
    fn seal(&mut self) {
        match self {
            GenericManagedStream::TerminatedTLS(peekable_tls_stream) => {
                peekable_tls_stream.sealed=true;
                peekable_tls_stream.enable_inspection = true;
                peekable_tls_stream.stream.get_mut().0.seal();
            },
            GenericManagedStream::TCP(peekable_tcp_stream) => peekable_tcp_stream.seal()
        }
    }
}

impl GenericManagedStream {
    /// Updates the tracked connection with the given function.
    /// Note that this also refreshes the connection's TLS and HTTP termination status based on the current state of the stream.
    pub fn update_tracked_info(&self,f:impl FnOnce(&mut ProxyActiveTCPConnection) -> ()) {
        crate::proxy::mutate_tracked_connection(&self.global_state(), &self.get_id(), |x|{
            match self {
                GenericManagedStream::TCP(peekable_tcp_stream) => {
                    x.incoming_connection_uses_tls = peekable_tcp_stream.is_tls;
                    x.tls_terminated = peekable_tcp_stream.is_tls_terminated;
                    x.http_terminated = peekable_tcp_stream.is_http_terminated;
                },
                GenericManagedStream::TerminatedTLS(managed_stream) => {
                    x.incoming_connection_uses_tls = managed_stream.is_tls;
                    x.tls_terminated = managed_stream.is_tls_terminated;
                    x.http_terminated = managed_stream.is_http_terminated;
                }
            }
            
            f(x)
        });
    }
    pub fn track(&self) {
        let my_id = self.get_id();
        match self {
            GenericManagedStream::TCP(peekable_tcp_stream) => {
                let client_addr = peekable_tcp_stream.stream.peer_addr();
                let client_addr = if let Ok(v) = client_addr {
                    format!("{:?}",v)
                } else {
                    "???".to_string()
                };
                crate::proxy::add_or_update_connection(
                    self.global_state(),
                    ProxyActiveTCPConnection {
                        resolved_connection_type: None,
                        resolved_connection_type_description: None,
                        tcp_peer_addr: peekable_tcp_stream.stream.peer_addr().ok()
                            .and_then(|x|Some(format!("{:?}",x))).unwrap_or("???".to_string()),
                        connection_key: *my_id,
                        connection_key_pointer: std::sync::Arc::<u64>::downgrade(&my_id),
                        client_addr: client_addr,
                        target: None,
                        backend: None,
                        incoming_connection_uses_tls: peekable_tcp_stream.is_tls,
                        tls_terminated: peekable_tcp_stream.is_tls_terminated,
                        http_terminated: peekable_tcp_stream.is_http_terminated,
                        outgoing_connection_is_tls: false,
                        is_odd_box_admin_api_req: false,
                        dir_server: None,
                        version: 1

                    }
                );
            },
            GenericManagedStream::TerminatedTLS(managed_stream) => {
                let client_addr = managed_stream.stream.get_ref().0.stream.peer_addr();
                let client_addr = if let Ok(v) = client_addr {
                    format!("{:?}",v)
                } else {
                    "???".to_string()
                };
                crate::proxy::add_or_update_connection(
                    self.global_state(),
                    ProxyActiveTCPConnection {
                        resolved_connection_type: None,
                        resolved_connection_type_description: None,
                        tcp_peer_addr: format!("{:?}",managed_stream.stream.get_ref().0.stream.peer_addr()),
                        connection_key: *my_id,
                        connection_key_pointer: std::sync::Arc::<u64>::downgrade(&my_id),
                        client_addr: client_addr,
                        target: None,
                        backend: None,
                        incoming_connection_uses_tls: true,
                        tls_terminated: true,
                        http_terminated: managed_stream.is_http_terminated,
                        outgoing_connection_is_tls: false,
                        is_odd_box_admin_api_req: false,
                        dir_server: None,
                        version: 1

                    }
                );
            }
        }
    }
    pub fn from_terminated_tls_stream(mut stream: ManagedStream<TlsStream<ManagedStream<TcpStream>>>) -> Self {
        stream.sealed = false;
        stream.is_tls_terminated = true;
        stream.is_tls;
        stream.stream.get_mut().0.is_wrapped = true;
        stream.stream.get_mut().0.is_tls = true;        
        Self::TerminatedTLS(stream)
    }
    pub fn from_tcp_stream(stream: TcpStream,state: Arc<GlobalState>) -> Self {
        Self::TCP(ManagedStream::from_tcp_stream(stream, state))
    }
    // pub fn from_tls_stream(stream: TlsStream<TcpStream>) -> Self {
    //     Self::TLS(PeekableTlsStream {
    //         managed_tls_stream: ManagedStream::from_tls_stream(stream)
    //     })
    // }
    pub async fn peek_managed_stream(
         &mut self,
        _client_address: SocketAddr,
    ) -> Result<PeekResult, PeekError> {
        
        let mut attempts = 0;
        let is_tls = false;

        loop {

            if attempts > 2 {
                break;
            }

            let (tcp_stream_closed,buf) = match self.peek_async().await {
                Ok(v) => v,
                Err(e) => {
                    return Err(PeekError::Unknown(format!("Error while peeking into TCP stream: {:?}",e)));
                }
            };



            if tcp_stream_closed {
                return Err(PeekError::StreamIsClosed);
            }
            
            if buf.len() == 0 {
                _ = tokio::time::sleep(Duration::from_millis(150)).await;
                attempts+=1;
                continue;

            }

            // Check for TLS handshake (0x16 is the ContentType for Handshake in TLS)
            if buf[0] == 0x16 {
                self.mark_as_tls();
                match TlsClientHello::try_from(&buf[..]) {
                    Ok(v) => {
                        if let Ok(v) = v.read_sni_hostname() {
                            trace!("Got TLS client hello with SNI '{v}' while peeking a managed stream. ");
                            return Ok(PeekResult { 
                                typ: DataType::TLS, 
                                http_version: None, 
                                target_host: Some(v),
                                is_h2c_upgrade: false
                            });
                        }
                    }
                    Err(crate::tcp_proxy::tls::client_hello::TlsClientHelloError::MessageIncomplete(_)) => {
                        tracing::trace!("Incomplete TLS client handshake detected; waiting for more data... (we have got {} bytes)",buf.len());
                        continue;
                    }
                    _ => {
                        return Err(PeekError::Unknown("Invalid TLS client handshake".to_string()));
                    }
                }
                // Continue loop to check for more data if TLS isn't fully validated
                tokio::time::sleep(Duration::from_millis(20)).await;
                attempts += 1;
                continue;
            }

            // Check for valid HTTP/1.x request
            if let Ok(http_version) = super::http1::is_valid_http_request(&buf) {
                //if let Ok(str_data) = std::str::from_utf8(&buf) {
                    if let Ok(ParsedHttpRequest{ host, is_h2c_upgrade }) = super::http1::try_decode_http_host_and_h2c(&buf) {
                        trace!("Found valid HTTP/1 host header while peeking into TCP stream: {host}");
                        match self {
                            GenericManagedStream::TerminatedTLS(stream) => {
                                stream.http_version = Some(http_version);
                                stream.contains_h2c_upgrade_header = is_h2c_upgrade;
                            },
                            GenericManagedStream::TCP(stream) => {
                                stream.http_version = Some(http_version);
                                stream.contains_h2c_upgrade_header = is_h2c_upgrade;
                            }
                        }
                        return Ok(PeekResult { 
                            typ: DataType::ClearText, 
                            http_version: Some(http_version), 
                            target_host: Some(host),
                            is_h2c_upgrade: is_h2c_upgrade
                        });
                    } else {
                        tracing::trace!("HTTP/1.x request detected but missing host header; waiting for more data...");
                    }
                // } else {
                //     tracing::trace!("Incomplete UTF-8 data detected; waiting for more data...");
                // }
            } else if super::http2::is_valid_http2_request(&buf) {

                //tracing::info!("is valid h2... creating new h2o for buf with len: {}",buf.len());
                let observer = match self {
                    GenericManagedStream::TCP(managed_stream) => {
                        managed_stream.http_version = Some(Version::HTTP_2);
                        &mut managed_stream.h2_observer
                    },
                    GenericManagedStream::TerminatedTLS(managed_stream) => {
                        managed_stream.http_version = Some(Version::HTTP_2);
                        &mut managed_stream.h2_observer
                    },
                };

                observer.write_incoming(&buf);

                let items  = observer.get_all_events();
                
                


                if items.len() < 2 {
                    tracing::info!("not enough http2 frames found (yet)...");
                } else {

                    for frame in items {
                        // note: this does work but we want to do it in the managedstream read/write polling instead
                        //       and not need to do it manually here
                        // _ = self.global_state().global_broadcast_channel.send(crate::types::odd_box_event::Event::Http2Event(format!("{:?}",frame)));
                        match frame {
                            h2_parser::H2Event::IncomingRequest(rq) => {
                               
                                if let Some(host) = rq.headers.get(":authority") {
                                    tracing::trace!("Found valid HTTP/2 authority header while peeking into TCP stream: {host}");
                                    return Ok(PeekResult { 
                                        typ: DataType::ClearText, 
                                        http_version: Some(Version::HTTP_2), 
                                        target_host: Some(host.to_string()),
                                        is_h2c_upgrade: false
                                    });
                                }
                                if let Some(host) = rq.headers.get("Host") {
                                    tracing::trace!("Found valid HTTP/2 host header while peeking into TCP stream: {host}");
                                    return Ok(PeekResult { 
                                        typ: DataType::ClearText, 
                                        http_version: Some(Version::HTTP_2), 
                                        target_host: Some(host.to_string()),
                                        is_h2c_upgrade: false
                                    });
                                }
                            },
                            _ => {}
                        }
                    }
                        
                    if is_tls {
                        return Err(PeekError::Unknown("this is a valid http2 request over tls, but we found no authority.. will attempt to find sni from tls and otherwise use http terminating proxy mode.".into())); 
                    } else {
                        // http2 over clear text must be terminated as client wont send the http headers before
                        // receiving a http2 settings response back , which it wont get if we dont terminate the connection.
                        // (we wont know where to forward the request to otherwise)
                        return Err(PeekError::H2PriorKnowledgeNeedsToBeTerminated); 
                    }
                    
                    
                   
                }   
      
            } else {
                tracing::warn!("NOT VALID H1 OR H2");
            }


            tokio::time::sleep(Duration::from_millis(20)).await;
            attempts += 1;
        }
    
        Err(PeekError::Unknown("TCP peek failed to find any useful information".into()))
    }

}


// impl GenericManagedStream {
//     // pub async fn inspect(&mut self) {
//     //     match self {
//     //         GenericManagedStream::TCP(managed_stream) => {
//     //             managed_stream.inspect().await;
//     //         },
//     //         // GenericManagedStream::TLS(managed_stream) => {
//     //         //     tracing::error!("TerminatedTLS stream cannot be inspected. This is a bug in odd-box as inspect should not have been called for this stream.");
//     //         // },
//     //         GenericManagedStream::TerminatedTLS(stream) => {
//     //             stream.inspect().await;
                
//     //         }
//     //     }
//     // }
// }



impl<T> ManagedStream<T> where T: AsyncRead + AsyncWrite + Unpin {

    #[cfg(debug_assertions)]
    pub async fn inspect(&mut self) {

        // tracing::info!("Starting to pull h2 observer stream events");
        // while let Some(x) = self.h2_observer.next().await {
        //     tracing::info!("H2 Observer: {:?}", x);
        // }

    }
    #[cfg(not(debug_assertions))]
    pub async fn inspect(&mut self) {}
}
impl<T> Drop for ManagedStream<T> where  T: AsyncWrite + AsyncRead + Unpin  {
    fn drop(&mut self) {
        let my_id = self.tcp_connection_id.clone();
        // if self.is_wrapped {
        //     tracing::trace!("Dropping inner TCP connection with id: {:?}",my_id);
        // } else {
        //     tracing::trace!("Dropping TCP connection with id: {:?}",my_id);
        // }
        crate::proxy::del_connection(
            self.global_state.clone(),
            &my_id
        );

    }
}
impl ManagedStream<TcpStream> {
    pub fn from_tcp_stream(stream: tokio::net::TcpStream,state:Arc<GlobalState>) -> Self {
        let connection_id = crate::generate_unique_id();
        let state = state.clone();
        //tracing::info!("Creating ManagedStream from TcpStream");
        ManagedStream::<tokio::net::TcpStream> {
            //state: ReadState::Reading,
            http_version: None,
            contains_h2c_upgrade_header: false,
            is_websocket: false,
            global_state : state.clone(),
            is_wrapped: false,
            is_http_terminated: false,
            is_tls_terminated: false,
            is_tls: false,
            enable_inspection: false, // we wont inspect the data in this stream once its been sealed
            h1_in: BytesMut::new(),
            h1_out: BytesMut::new(),
            h2_observer: h2_parser::H2Observer::new(),
            stream,
            buffer: BytesMut::new(),
            sealed: false,
            tcp_connection_id: Arc::new(connection_id),
            events: vec![
                "Created by wrapping a TCP stream.. Content of stream not yet known (may be tls or clear-text)".to_string()
            ],
        }

    }
}

impl ManagedStream<tokio_rustls::server::TlsStream<ManagedStream<tokio::net::TcpStream>>> {
    pub fn from_tls_stream(mut stream: tokio_rustls::server::TlsStream<ManagedStream<tokio::net::TcpStream>>) -> Self {
        let state = stream.get_ref().0.global_state.clone();
        //tracing::info!("Creating ManagedStream from TlsStream");
        stream.get_mut().0.events.push("The stream was found to be TLS".to_string());
        stream.get_mut().0.events.push("The stream has been wrapped by an outer ManagedStream struct, this inner instance will now only be used as a transparant proxy.".to_string());
        stream.get_mut().0.enable_inspection = false; // just to be sure
        stream.get_mut().0.is_wrapped = true;
        stream.get_mut().0.is_tls = true;
        ManagedStream {
            //state: ReadState::Reading,
            http_version: None,
            contains_h2c_upgrade_header: false,
            is_websocket: false,
            global_state: state.clone(),
            is_wrapped: false,
            is_http_terminated: false,
            is_tls_terminated: true,
            is_tls: true, // we know this is a tls stream since we terminated it
            enable_inspection: false, 
            h1_in: BytesMut::new(),
            h1_out: BytesMut::new(),
            h2_observer: h2_parser::H2Observer::new(),
            sealed: stream.get_ref().0.sealed,
            tcp_connection_id: stream.get_ref().0.tcp_connection_id.clone(),
            buffer: BytesMut::new(),       
            stream,
            events: vec![
                "A TLS session has been terminated successfully, we can now observe the data in clear text".to_string()
            ]
        }
    }
}

impl Peekable for ManagedStream<TlsStream<TcpStream>>  {
    fn seal(&mut self) {
        self.sealed = true;
        self.enable_inspection = true;
    }
    /// peeks data from the tcpstream without consuming it.
    /// consequent calls to this function will further read data from the TcpStream
    /// in a nondestructive manner as the data is stored in an internal managed buffer.
    /// returns: (tcp_stream_is_closed:bool, data:Vec<u8>)
    async fn peek_async(&mut self) -> anyhow::Result<(bool,Vec<u8>)>  {
        
        use futures::future::poll_fn;

        if self.sealed {
            bail!("Stream is sealed")
        }
        
        if let Ok(Some(e)) = self.stream.get_mut().0.take_error() {
            bail!(e)
        }
        
        let mut buf = [0u8; 1024]; // Temporary buffer for reading
        let mut temp_buf = ReadBuf::new(&mut buf);
        
        let result = poll_fn(|cx| {
            let pin_stream = Pin::new(&mut self.stream);
            let result = match pin_stream.poll_read(cx, &mut temp_buf) {
                Poll::Ready(Ok(_n)) => Poll::Ready(Ok(1)),
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                // we dont want to keep waiting here if the underlying stream has no more bytes for us right now.
                Poll::Pending => Poll::Ready(Ok(-1))
            };
            result
        })
        .await?;
    
        if result == -1 {
            return Ok((false,self.buffer.to_vec()))
        }

        match temp_buf.filled() {
            read_bytes if read_bytes.len() == 0 => {
                // End of stream, no more data is expected to come in
                return Ok((true,self.buffer.to_vec()));
            }
            read_bytes => {
                // Append the read data to the internal buffer
                self.buffer.extend_from_slice(&read_bytes);
            }
        }
        
        let byte_vec = self.buffer.to_vec();
        
        // for x in h1_parser::parse_http_requests(&byte_vec).iter().flatten() {
        //     tracing::info!("INCOMING HTTPs REQUEST: {:?}", x);
        // }
        

        // Return a copy of the buffered data without consuming it
        Ok((false,byte_vec))

    }

}


impl Peekable for ManagedStream<TlsStream<ManagedStream<TcpStream>>> {
        fn seal(&mut self) {
            self.sealed = true;
        }
        /// peeks data from the tcpstream without consuming it.
        /// consequent calls to this function will further read data from the TcpStream
        /// in a nondestructive manner as the data is stored in an internal managed buffer.
        /// returns: (tcp_stream_is_closed:bool, data:Vec<u8>)
        async fn peek_async(&mut self) -> anyhow::Result<(bool,Vec<u8>)>  {
            
            use futures::future::poll_fn;
    
            if self.sealed {
                bail!("Stream is sealed")
            }
            
            if let Ok(Some(e)) = self.stream.get_mut().0.stream.take_error() {
                bail!(e)
            }
            
            let mut buf = [0u8; 1024]; // Temporary buffer for reading
            let mut temp_buf = ReadBuf::new(&mut buf);
            
            let result = poll_fn(|cx| {
                let pin_stream = Pin::new(&mut self.stream);
                let result = match pin_stream.poll_read(cx, &mut temp_buf) {
                    Poll::Ready(Ok(_n)) => Poll::Ready(Ok(1)),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    // we dont want to keep waiting here if the underlying stream has no more bytes for us right now.
                    Poll::Pending => Poll::Ready(Ok(-1))
                };
                result
            })
            .await?;
        
            if result == -1 {
                return Ok((false,self.buffer.to_vec()))
            }
    
            match temp_buf.filled() {
                read_bytes if read_bytes.len() == 0 => {
                    // End of stream, no more data is expected to come in
                    return Ok((true,self.buffer.to_vec()));
                }
                read_bytes => {
                    // Append the read data to the internal buffer
                    self.buffer.extend_from_slice(&read_bytes);
                }
            }
            
            let byte_vec = self.buffer.to_vec();
            
            // for x in h1_parser::parse_http_requests(&byte_vec).iter().flatten() {
            //     tracing::info!("INCOMING HTTPs REQUEST: {:?}", x);
            // }
            
    
            // Return a copy of the buffered data without consuming it
            Ok((false,byte_vec))
    
        }
    
    }

impl Peekable for ManagedStream<TcpStream>  {
    fn seal(&mut self) {
        self.sealed = true;
    }
    /// peeks data from the tcpstream without consuming it.
    /// consequent calls to this function will further read data from the TcpStream
    /// in a nondestructive manner as the data is stored in an internal managed buffer.
    /// returns: (tcp_stream_is_closed:bool, data:Vec<u8>)
    async fn peek_async(&mut self) -> anyhow::Result<(bool,Vec<u8>)>  {

        if self.sealed {
            bail!("Stream is sealed")
        }
        
        if let Ok(Some(e)) = self.stream.take_error() {
            bail!(e);
        }

        // Always attempt to read more data from the TcpStream
        let mut temp_buf = Vec::with_capacity(4096);
        
        match self.stream.try_read_buf(&mut temp_buf) {
            Ok(0) => {
                // End of stream, no more data is expected to come in
                return Ok((true,self.buffer.to_vec()));
            }
            Ok(n) => {
                self.buffer.extend_from_slice(&temp_buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
        
        let byte_vec = self.buffer.to_vec();
        
        // for x in h1_parser::parse_http_requests(&byte_vec).iter().flatten() {
        //     tracing::info!("INCOMING HTTP REQUEST: {:?}", x);
        // }

        // Return a copy of the buffered data without consuming it
        Ok((false,byte_vec))
    }

}

impl<T> AsyncRead for ManagedStream<T> where T: AsyncWrite + AsyncRead + Unpin {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), Error>> {
        
        if !self.sealed {
            return Poll::Ready(Err(Error::new(
                ErrorKind::Other,
                "Stream has not been properly sealed",
            )));
        }

        // If stream is sealed and buffer is empty, read directly from stream
        if self.buffer.is_empty() {
            return Pin::new(&mut self.stream).poll_read(cx, buf);
        } 

        // Otherwise, drain any buffered data into the output buffer
        if !self.buffer.is_empty() {
            let to_read = std::cmp::min(buf.remaining(), self.buffer.len());
            buf.put_slice(&self.buffer.split_to(to_read));
            if self.buffer.is_empty() {
                self.buffer = BytesMut::new(); // drop the old buffer to reclaim memory
            }
            if buf.remaining() == 0 {
                // // Buffer is full after draining self.buffer
                // if self.enable_inspection && !self.is_wrapped {
                //     match self.http_version {
                //         Some(Version::HTTP_2) => {
                //               self.h2_observer.write_incoming(buf.filled());
                //         },
                //         _ => {
                //             self.h1_in.extend_from_slice(buf.filled());
                //         }
                //     }
                // }
                return Poll::Ready(Ok(()));
            }
            // Else, buf still has space, so we can try to read from stream
        }

        // Now, for efficiency, we use any remaining space in buf to read directly from stream
        match Pin::new(&mut self.stream).poll_read(cx, buf) {
            Poll::Pending => {
                if buf.filled().is_empty() {
                    // No data has been read yet, return Pending
                    Poll::Pending
                } else {
                    // Data has been read from self.buffer, return Ready
                    // if self.enable_inspection && !self.is_wrapped {
                    //     match self.http_version {
                    //         Some(Version::HTTP_2) => {
                    //            self.h2_observer.write_incoming(buf.filled());
                    //         },
                    //         _ => {
                    //             self.h1_in.extend_from_slice(buf.filled());
                    //         }
                    //     }
                    // }
                    Poll::Ready(Ok(()))
                }
            }
            Poll::Ready(Ok(())) => {
                // Successfully read from stream into buf
                // if self.enable_inspection && !self.is_wrapped {
                //     match self.http_version {
                //         Some(Version::HTTP_2) => {
                //            self.h2_observer.write_incoming(buf.filled());
                //         },
                //         _ => {
                //             self.h1_in.extend_from_slice(buf.filled());
                //         }
                //     }
                // }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                if buf.filled().is_empty() {
                    // No data was read at all, return the error
                    Poll::Ready(Err(e))
                } else {
                    // if self.enable_inspection && !self.is_wrapped {
                    //     match self.http_version {
                    //         Some(Version::HTTP_2) => {
                    //            self.h2_observer.write_incoming(buf.filled());
                    //         },
                    //         _ => {
                    //             self.h1_in.extend_from_slice(buf.filled());
                    //         }
                    //     }
                    // }
                    // Data was read from self.buffer, return Ok
                    // The error can be returned on the next poll_read
                    Poll::Ready(Ok(()))
                }
            }
        }
    }
}

impl<T> AsyncWrite for ManagedStream<T> where T: AsyncWrite + AsyncRead + Unpin {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        // if self.enable_inspection && !self.is_wrapped{
        //     match self.http_version {
        //         Some(Version::HTTP_2) => {
        //            self.h2_observer.write_outgoing(buf);
        //         },
        //         _ => {
        //             self.h1_out.extend_from_slice(buf);
        //         }
        //     }
        // }

        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
