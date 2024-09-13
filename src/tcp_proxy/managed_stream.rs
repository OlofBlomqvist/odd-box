use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use std::pin::Pin;
use std::task::{Context, Poll};
use bytes::BytesMut;
use std::io::{Error, ErrorKind};

#[derive(Debug)]
pub struct ManagedStream {
    stream: TcpStream,
    buffer: BytesMut,
    sealed: bool
}



// ManagedStream is a wrapper around TcpStream that provides a buffered read
// with custom peek support. This exists to give us more control over the
// data and allows us to modify the stream for things like injecting 
// custom http headers or other data manipulation. (todo)
impl ManagedStream {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            sealed: false,
            buffer: BytesMut::with_capacity(4096)
        }
    }
    pub fn seal(&mut self) {
        self.sealed = true;
    }
    /// peeks data from the tcpstream without consuming it.
    /// consequent calls to this function will further read data from the TcpStream
    /// in a nondestructive manner as the data is stored in an internal managed buffer.
    /// returns: (tcp_stream_is_closed:bool, data:Vec<u8>)
    pub async fn peek_async(&mut self) -> Result<(bool,Vec<u8>), Error> {

        if self.sealed {
            return Err(Error::new(ErrorKind::Other, "Stream is sealed"));
        }

        if let Ok(Some(e)) = self.stream.take_error() {
            return Err(e);
        }

        // Always attempt to read more data from the TcpStream
        let mut temp_buf = [0u8; 1024]; // Temporary buffer for reading
        match self.stream.read(&mut temp_buf).await {
            Ok(0) => {
                // End of stream, no more data is expected to come in
                return Ok((true,self.buffer.to_vec()));
            }
            Ok(n) => {
                // Append the read data to the internal buffer
                self.buffer.extend_from_slice(&temp_buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // If the operation would block, simply continue without adding to the buffer
            }
            Err(e) => return Err(e),
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
                    Poll::Ready(Ok(()))
                }
            }
            Poll::Ready(Ok(())) => {
                // Successfully read from stream into buf
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                if buf.filled().is_empty() {
                    // No data was read at all, return the error
                    Poll::Ready(Err(e))
                } else {
                    // Data was read from self.buffer, return Ok
                    // The error can be returned on the next poll_read
                    Poll::Ready(Ok(()))
                }
            }
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