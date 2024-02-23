#![feature(ascii_char)]
use std::{boxed, convert::Infallible, future::{self, IntoFuture}, time::Duration};
use futures::TryStreamExt;
use hyper::{body::{Bytes, Frame}, client::conn, Request};
use hyper_util::rt::{TokioExecutor,TokioIo};
use tokio::{io::{DuplexStream, Empty}, pin};
use http_body_util::{combinators::BoxBody, BodyExt, BodyStream, StreamBody};
use tokio::io::{AsyncWriteExt as _, self};
use bytes::{BufMut, BytesMut};


#[tokio::main]
async fn main() -> ! {

    

    let url = "http://127.0.0.1:80?test=stream_test_odd_box".parse::<hyper::Uri>().unwrap();
    // Get the host and the port
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);
    let address = format!("{}:{}", host, port);

    let s = tokio::net::TcpStream::connect(address).await.unwrap();
    let t = TokioIo::new(s);

    let (body_tx, body_rx) = tokio::sync::mpsc::channel::<Result<hyper::body::Bytes, String>>(21);
   
    let body_stream = http_body_util::StreamBody::new(
        tokio_stream::wrappers::ReceiverStream::new(body_rx).map_ok(hyper::body::Frame::data));

    //let simple_stream_future = std::pin::pin!(&x);

    let (mut sender,mut conn) = 
        hyper::client::conn::http2::Builder::new(TokioExecutor::new()).handshake(t).await.unwrap();
    

    let f = async move {
        println!("stream connected!");
        match conn.await {
            Ok(()) => println!("DONE"),
            Err(e) =>  println!("Connection failed: {:?}", e)
        }     
    };
  
    tokio::spawn(f);
    
    
    // The authority of our URL will be the hostname of the httpbin remote
    let authority = url.authority().unwrap().clone();
    
    // Create an HTTP request with an empty body and a HOST header
    let req = Request::builder()
        .uri(url)
        .header(hyper::header::HOST, authority.as_str())
        .body(body_stream)
        .unwrap();

    // this creates a stream on the http2 connection
    let mut res = sender.send_request(req).await.unwrap();
  
   
    println!("SENDING!!!");
    // //tokio::spawn(async move {
    //     match body_tx.send(Ok(bytes::Bytes::from(make_fd_proto()))).await {
    //         Ok(()) => println!("sent the packet!"),
    //         Err(e) => println!("boo: {e:?}")
    //     }
    // //});

    
    tokio::spawn(async move {
        loop {
            _ = body_tx.send(Ok(bytes::Bytes::from("yo!".as_bytes()))).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    while let Some(Ok(frame)) = res.frame().await {
        if let Some(chunk) = frame.data_ref() {
            match chunk.as_ascii() {
                Some(text) => println!("got this from server: {}",text.as_str()),
                _ => println!("FROM SERVER: {chunk:?}")
            }
        } else {
            println!("nah.. {:?}",frame);
        }
    }
   panic!("boo")

}
