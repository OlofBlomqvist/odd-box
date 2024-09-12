use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use std::pin::Pin;
use std::task::{Context, Poll};
use bytes::BytesMut;
use std::io::Error;

#[derive(Debug)]
pub struct ManagedStream {
    stream: TcpStream,
    buffer: BytesMut,
}



// ManagedStream is a wrapper around TcpStream that provides a buffered read
// with custom peek support. This exists to give us more control over the
// data and allows us to modify the stream for things like injecting 
// custom http headers or other data manipulation. (todo)
impl ManagedStream {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buffer: BytesMut::with_capacity(4096)
        }
    }

    /// peeks data from the tcpstream without consuming it.
    /// consequent calls to this function will further read data from the TcpStream
    /// in a nondestructive manner as the data is stored in an internal managed buffer.
    /// returns: (tcp_stream_is_closed:bool, data:Vec<u8>)
    pub async fn peek_async(&mut self) -> Result<(bool,Vec<u8>), Error> {

        if let Ok(Some(e)) = self.stream.take_error() {
            return Err(e);
        }

        // Always attempt to read more data from the TcpStream
        let mut temp_buf = [0u8; 1024]; // Temporary buffer for reading
        match self.stream.read(&mut temp_buf).await {
            Ok(0) => {
                // End of stream, no more data
                return Ok((true,self.buffer.to_vec()));
            }
            Ok(n) => {
                // Append the read data to the internal buffer
                self.buffer.extend_from_slice(&temp_buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // If the operation would block, simply continue without adding to the buffer
            }
            Err(e) => return Err(e), // Return the error if something went wrong
        }

        // Return a copy of the buffered data without consuming it
        Ok((false,self.buffer.to_vec()))
    }

}

impl AsyncRead for ManagedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), Error>> {
        // First, drain any buffered data into the output buffer
        if !self.buffer.is_empty() {
            let to_read = std::cmp::min(buf.remaining(), self.buffer.len());
            buf.put_slice(&self.buffer.split_to(to_read));
            return Poll::Ready(Ok(()));
        }

        // If buffer is empty, read from the TcpStream directly
        let mut internal_buf = [0u8; 4096];
        let mut read_buf = ReadBuf::new(&mut internal_buf);
        
        match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => {
                let filled = read_buf.filled();
                self.buffer.extend_from_slice(filled);
                let to_read = std::cmp::min(buf.remaining(), self.buffer.len());
                buf.put_slice(&self.buffer.split_to(to_read));
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncWrite for ManagedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
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