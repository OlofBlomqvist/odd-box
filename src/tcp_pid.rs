/*

    This is just a POC implementation to figure out which process is calling us when we see loopback connections
    to the proxy. Useful for tracking site-to-site calls more easily..
*/



// turns out spawning lsof this way is pretty darn fast so just going to do that for now rather than
// directly reading /proc fs as this will also work on macos
fn get_pid_via_lsof(ip: &str, port: u16) -> std::io::Result<Option<u32>> {
    
    let my_pid = std::process::id();
    let address = format!("{}:{}", ip, port);
    let output = std::process::Command::new("lsof")
        .args(&[
            "-nPOl",
            &format!("-iTCP@{address}"),
            "-sTCP:ESTABLISHED",
            "-t", 
        ]).output()?;

        
    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(None);
    }

    let lines : Vec<String> = stdout.lines().map(|x| x.to_string()).collect();

    for pid_str in lines {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            if my_pid != pid {
                return Ok(Some(pid));
            }
            
        } 
    }
    Ok(None)
}

#[cfg(any(target_os = "linux",target_os = "macos"))]
pub fn get_process_by_socket(client_socket: &std::net::SocketAddr, _odd_box_socket: &std::net::SocketAddr) -> std::io::Result<Option<(String,i32)>> {
    use sysinfo::{Pid, ProcessRefreshKind, RefreshKind};
    if !client_socket.ip().is_loopback() {
        return Ok(None)
    }
    if let Ok(Some(pid)) = get_pid_via_lsof(&client_socket.ip().to_string(),client_socket.port()) {
        let s = sysinfo::System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet)),
        );
        if let Some(process) = s.process(Pid::from_u32(pid)) {
            if let Some(exe) = process.exe() {
                if let Some(file_name) = exe.file_name() {
                    return Ok(Some((file_name.to_str().unwrap_or("?").to_owned(),pid as i32)));
                }
            }
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