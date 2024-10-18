use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use std::pin::Pin;
use std::task::{Context, Poll};
use bytes::BytesMut;
use std::io::{Error, ErrorKind};
use super::{h2_parser, h1_parser};


pub trait Peekable {
    async fn peek_async(&mut self) -> Result<(bool,Vec<u8>), Error>;
    fn seal(&mut self);
}

#[derive(Debug)]
pub struct ManagedStream<T> where T: AsyncRead + AsyncWrite + Unpin {
    stream: T,
    buffer: BytesMut,
    sealed: bool,
    h2_observer: h2_parser::H2Observer,
    // h1_in: BytesMut,
    // h1_out: BytesMut,
    

}

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
        tracing::info!("ManagedStream dropped");
        // self.h1_in.clear();
        // self.h1_out.clear();
        // self.h1_in.resize(0, 0);
        // self.h1_out.resize(0, 0);
        
    }
}
impl ManagedStream<TcpStream> {
    pub fn from_tcp_stream(stream: tokio::net::TcpStream) -> Self {
        tracing::info!("Creating ManagedStream from TcpStream");
        ManagedStream::<tokio::net::TcpStream> {
            // h1_in: BytesMut::new(),
            // h1_out: BytesMut::new(),
            h2_observer: h2_parser::H2Observer::new(),
            stream,
            buffer: BytesMut::new(),
            sealed: false
        }
    }
}

impl ManagedStream<tokio_rustls::server::TlsStream<TcpStream>> {
    pub fn from_tls_stream(stream: tokio_rustls::server::TlsStream<TcpStream>) -> Self {
        tracing::info!("Creating ManagedStream from TlsStream");
        ManagedStream {
            // h1_in: BytesMut::new(),
            // h1_out: BytesMut::new(),
            h2_observer: h2_parser::H2Observer::new(),
            stream,
            buffer: BytesMut::new(),
            sealed: false
        }
    }
}

impl Peekable for ManagedStream<TlsStream<TcpStream>>  {
    fn seal(&mut self) {
        self.sealed = true;
    }
    /// peeks data from the tcpstream without consuming it.
    /// consequent calls to this function will further read data from the TcpStream
    /// in a nondestructive manner as the data is stored in an internal managed buffer.
    /// returns: (tcp_stream_is_closed:bool, data:Vec<u8>)
    async fn peek_async(&mut self) -> Result<(bool,Vec<u8>), Error>  {
        
        use futures::future::poll_fn;

        if self.sealed {
            return Err(Error::new(ErrorKind::Other, "Stream is sealed"));
        }
        
        if let Ok(Some(e)) = self.stream.get_mut().0.take_error() {
            return Err(e);
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
    async fn peek_async(&mut self) -> Result<(bool,Vec<u8>), Error>  {

        if self.sealed {
            return Err(Error::new(ErrorKind::Other, "Stream is sealed"));
        }
        
        if let Ok(Some(e)) = self.stream.take_error() {
            return Err(e);
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
            Err(e) => return Err(e),
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

        // First, drain any buffered data into the output buffer
        if !self.buffer.is_empty() {
            let to_read = std::cmp::min(buf.remaining(), self.buffer.len());
            buf.put_slice(&self.buffer.split_to(to_read));

            if buf.remaining() == 0 {
                // Buffer is full after draining self.buffer
                //self.h1_in.extend_from_slice(buf.filled());
                //self.h2_observer.write_incoming(buf.filled());
                return Poll::Ready(Ok(()));
            }
            // Else, buf still has space, so we can try to read from stream
        }

        // Now, read from the stream directly into buf
        match Pin::new(&mut self.stream).poll_read(cx, buf) {
            Poll::Pending => {
                if buf.filled().is_empty() {
                    // No data has been read yet, return Pending
                    Poll::Pending
                } else {
                    // Data has been read from self.buffer, return Ready
                    //self.h1_in.extend_from_slice(buf.filled());
                    // self.h2_observer.write_incoming(buf.filled());
                    Poll::Ready(Ok(()))
                }
            }
            Poll::Ready(Ok(())) => {
                // Successfully read from stream into buf
                //self.h1_in.extend_from_slice(buf.filled());
                // self.h2_observer.write_incoming(buf.filled());
                
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                if buf.filled().is_empty() {
                    // No data was read at all, return the error
                    Poll::Ready(Err(e))
                } else {
                    //self.h1_in.extend_from_slice(buf.filled());
                    // self.h2_observer.write_incoming(buf.filled());
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
        //self.h1_out.extend_from_slice(buf);
        //self.h2_observer.write_outgoing(buf);
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