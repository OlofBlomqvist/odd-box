/*

    This is just a POC implementation to figure out which process is calling us when we see loopback connections
    to the proxy. Useful for tracking site-to-site calls more easily.

    Code quality: horrible :<

*/

#[cfg(target_os = "linux")]
pub fn get_process_by_socket(client_socket: &std::net::SocketAddr, odd_box_socket: &std::net::SocketAddr) -> std::io::Result<Option<(String,i32)>> {
   
    use procfs::{process::FDTarget, ProcResult};
    let all_procs = procfs::process::all_processes().unwrap();

    let mut target_inode = None;

    let tcp = procfs::net::tcp().unwrap();
    let tcp6 = procfs::net::tcp6().unwrap();


    for entry in tcp.into_iter().chain(tcp6) {
        if entry.local_address == *client_socket && entry.remote_address == *odd_box_socket {
            target_inode = Some(entry.inode);
            break;
        }
    }

    match target_inode {
        None => {
            // Perhaps you can find it at 29°58′45″N 31°08′03″E
        },
        Some(n) => {
            for process in all_procs.filter_map(|p|p.ok()) {
                if let ProcResult::Ok(fds) = process.fd() {
                    for fd in fds.filter_map(|fd|fd.ok()) {
                        if let FDTarget::Socket(inode) = fd.target {
                            if inode == n {
                                match process.exe() {
                                    Ok(v) => {
                                        match v.file_name() {
                                            Some(name) => {
                                                let name = name.to_string_lossy().to_string();
                                                return Ok(Some((name,process.pid)));
                                            },
                                            None => {},
                                        }
                                    }
                                    _ => {}
                                }                                
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

#[cfg(target_os = "macos")]
pub fn get_process_by_socket(client_socket: &std::net::SocketAddr, odd_box_socket: &std::net::SocketAddr) -> std::io::Result<Option<(String,i32)>> {
    use std::{fs, net::Ipv4Addr};
    let output = std::process::Command::new("lsof")
        .arg("-i")
        .arg(format!("tcp:{}->{}", client_socket.port(), odd_box_socket.ip()))
        .output()?;

    if !output.status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to execute lsof"));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);

    for line in output_str.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() > 1 {
            if let Ok(pid) = fields[1].parse::<i32>() {
                return Ok(Some((fields[0].to_string(),pid)));
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