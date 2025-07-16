use crate::configuration::{self, Backend, DirServer, OddBoxConfiguration, RemoteSiteConfig};
    
use std::{
    fs, 
    path::Path,
    process::{Child, Command},
};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener, select, time::{sleep, Duration}};
 

/// We dont want to keep the child process running after the test is done 
/// even if the test fails, so we create a guard that will kill the child process
struct ChildGuard {
    child: Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        println!("Dropping ChildGuard, killing child process..."); 
        let _ = self.child.kill(); 
        let _ = self.child.wait();
        println!("Child process killed and reaped.");
    }
}


#[tokio::test]
async fn proxy_dir_server_works() -> anyhow::Result<()> {
    
    let config_path = Path::new("test_odd_box_proxy_12.toml"); 
    let proxy_server_http_port = 9991;
    let proxy_server_tls_port = 9992;

    let mut test_config = configuration::OddBoxV3Config::example();    
    test_config.hosted_process = None;
    test_config.http_port = Some(proxy_server_http_port);
    test_config.tls_port = Some(proxy_server_tls_port);
    test_config.remote_target = None;
    test_config.dir_server = Some(vec![
        DirServer { 
            dir: "$cfg_dir".to_string(), 
            host_name: "dir.localhost".to_string(), 
            enable_directory_browsing: Some(true), 
            redirect_to_https: Some(false), 
            render_markdown: Some(true), 
            ..Default::default() 
        }
    ]);

    let toml = toml::to_string(&test_config)?;
    fs::write(&config_path, toml)?;

    let child = Command::new("./target/debug/odd-box")
        .arg("--tui=false")
        .arg(&config_path)
        .spawn()?;

    let _proxy = ChildGuard { child };

    sleep(Duration::from_secs(1)).await; 

    let client = reqwest::Client::new();
    let req = client
        .get(format!("http://127.0.0.1:{proxy_server_http_port}/"))
        .header("Host", "dir.localhost")
        .build()?;

    println!("This is the request that we are sending to the proxy: {req:?}");

    let resp = client.execute(req).await?;
     
    println!("This is the request that we got back from the proxy: {:?}",resp);

    let forwarded_host = resp.headers()
        .get("x-forwarded-host")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("No x-forwarded-host header found in response"))?;

    let forwarded_proto = resp.headers()
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("No x-forwarded-proto header found in response"))?;


    let forwarded_for = resp.headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("No x-forwarded-for header found in response"))?;

    if forwarded_for != "127.0.0.1" {
        anyhow::bail!("Unexpected x-forwarded-for header: {}", forwarded_for);
    }

    if forwarded_proto != "http" {
        anyhow::bail!("Unexpected x-forwarded-proto header: {}", forwarded_proto);
    }

    if forwarded_host != "dir.localhost" {
        anyhow::bail!("Unexpected x-forwarded-host header: {}", forwarded_host);
    }

    resp.status()
        .is_success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("Request did not succeed"))?;

    fs::remove_file(&config_path)?;

    Ok(())
}



#[tokio::test]
async fn proxy_test_requests_thru_proxy_receive_correct_x_forwarded_host_header() -> anyhow::Result<()> {
    
    let config_path = Path::new("test_odd_box_proxy_1.toml");
    let backend_port = 8222;
    let proxy_server_http_port = 9993;
    let proxy_server_tls_port = 9994;

    let mut test_config = configuration::OddBoxV3Config::example();    
    test_config.hosted_process = None;
    test_config.http_port = Some(proxy_server_http_port);
    test_config.tls_port = Some(proxy_server_tls_port);
    
    test_config.remote_target = Some(vec![
        RemoteSiteConfig {
            host_name: "banana.localhost".to_string(),
            backends: vec![ Backend {
                address: "127.0.0.1".to_string(),
                port: backend_port,
                https: Some(false),
                hints: None,
            } ],
            keep_original_host_header: Some(true),
            terminate_http: Some(true),
            ..Default::default()
        }
    ]);

    let toml = toml::to_string(&test_config)?;
    fs::write(&config_path, toml)?;

    let child = Command::new("./target/debug/odd-box")
        .arg("--tui=false")
        .arg(&config_path)
        .spawn()?;

    let _proxy = ChildGuard { child };


    sleep(Duration::from_secs(1)).await; 

    let (response_tx, mut response_rx) = tokio::sync::mpsc::channel(1);
    
    tokio::spawn(basic_tcp_server(backend_port, 5, response_tx));

    let client = reqwest::Client::new();
    let req = client
        .get(format!("http://127.0.0.1:{proxy_server_http_port}/"))
        .header("Host", "banana.localhost")
        .build()?;

    println!("This is the request that we are sending to the proxy: {req:?}");

    let resp = client.execute(req).await?;
     

    let incoming_req_to_backend = match tokio::time::timeout(Duration::from_secs(5), response_rx.recv()).await {
        Ok(Some(resp)) => resp,
        Ok(None) => anyhow::bail!("No response received from backend server"),
        Err(_) => anyhow::bail!("Timeout after 5 seconds waiting for response from backend server"),
    };


    println!("This is the request that the backend received: {}", incoming_req_to_backend);
    
    println!("This is the request that we got back from the proxy: {:?}",resp);

    let forwarded_host = resp.headers()
        .get("x-forwarded-host")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("No x-forwarded-host header found in response"))?;

    let forwarded_proto = resp.headers()
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("No x-forwarded-proto header found in response"))?;


    let forwarded_for = resp.headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("No x-forwarded-for header found in response"))?;

    if forwarded_for != "127.0.0.1" {
        anyhow::bail!("Unexpected x-forwarded-for header: {}", forwarded_for);
    }

    if forwarded_proto != "http" {
        anyhow::bail!("Unexpected x-forwarded-proto header: {}", forwarded_proto);
    }

    if forwarded_host != "banana.localhost" {
        anyhow::bail!("Unexpected x-forwarded-host header: {}", forwarded_host);
    }

    resp.status()
        .is_success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("Request did not succeed"))?;

    fs::remove_file(&config_path)?;

    Ok(())
}



async fn basic_tcp_server(port_to_listen_on:u16,timeout_seconds:u64,result_sender_tx:tokio::sync::mpsc::Sender<String>) -> anyhow::Result<()> { 
    let backend_server_bind_addr = format!("127.0.0.1:{port_to_listen_on}"); 
    let listener = TcpListener::bind(&backend_server_bind_addr).await?; 
    println!("Backend server listening on {backend_server_bind_addr}"); 
    select! {
        _ = sleep(Duration::from_secs(timeout_seconds)) => {
            anyhow::bail!("timeout waiting for backend to receive request");
        },
        Ok((mut stream, _)) = listener.accept() => {
            println!("Backend server received a connection!");
            let mut buf = [0; 1024];
            let n = stream.read(&mut buf).await?;
            let received = String::from_utf8_lossy(&buf[..n]);
            println!("Received request: {}", received);
            result_sender_tx.send(received.to_string()).await?;
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").await?;
        }
    } 
    println!("Backend server finished.");
    Ok(()) 
}