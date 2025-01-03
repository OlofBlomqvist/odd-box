/*

    This is just a POC implementation to figure out which process is calling us when we see loopback connections
    to the proxy. Useful for tracking site-to-site calls more easily..
*/


fn get_pid_via_lsof(ip: &str, port: u16) -> std::io::Result<Option<u32>> {

    let address = format!("{}:{}", ip, port);
    let output = std::process::Command::new("lsof")
        .args(&[
            "-nP",
            &format!("-iTCP@{address}"),
            "-sTCP:ESTABLISHED",
            "-t", // Output only PIDs
        ])
        .output()?;

    if !output.status.success() {
        tracing::error!("lsof command failed with status: {}", String::from_utf8_lossy(&output.stderr));
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(None);
    }

    if let Some(pid_str) = stdout.lines().next() {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            return Ok(Some(pid));
        }
    }

    Ok(None)
}

#[cfg(any(target_os = "linux",target_os = "macos"))]
pub fn get_process_by_socket(client_socket: &std::net::SocketAddr, _odd_box_socket: &std::net::SocketAddr) -> std::io::Result<Option<(String,i32)>> {
    
    if !client_socket.ip().is_loopback() {
        return Ok(None)
    }

    // turns out spawning lsof this way is pretty darn fast so just going to do that for now rather than
    // directly reading /proc fs as this will also work on macos
    if let Ok(Some(pid)) = get_pid_via_lsof(&client_socket.ip().to_string(),client_socket.port()) {
        match procfs::process::Process::new(pid as i32) {
            Ok(process) => {
                match process.exe() {
                    Ok(v) => {
                        match v.file_name() {
                            Some(name) => {
                                let name = name.to_string_lossy().to_string();
                                return Ok(Some((name,process.pid)));
                            },
                            None => { },
                        }
                    }
                    _ => {}
                }   
            },
            Err(_) => {},
        }
    }
    Ok(None)

    
}

#[cfg(target_os = "windows")]
pub fn get_process_by_socket(
    client_socket: &std::net::SocketAddr,
    odd_box_socket: &std::net::SocketAddr,
) -> std::io::Result<Option<(String, i32)>> {
    // yeah im not working on this shit today
    Ok(None)
}