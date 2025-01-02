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
pub fn get_process_by_socket(client_socket: &std::net::SocketAddr, odd_box_socket: &std::net::SocketAddr) -> std::io::Result<Option<String>> {
    use std::{fs, net::Ipv4Addr};
    use windows::{
        core::PCSTR,
        Win32::NetworkManagement::IpHelper::{
            GetTcpTable2, MIB_TCPTABLE2, MIB_TCPROW2, TCP_TABLE_OWNER_PID_ALL,
        },
        Win32::Foundation::NO_ERROR,
    };
    
    fn get_process_name_by_pid(pid: u32) -> std::io::Result<String> {
        use windows::Win32::System::ProcessStatus::K32GetProcessImageFileNameA;
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid);
            if handle.is_invalid() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to open process with PID {}", pid),
                ));
            }
    
            let mut buffer = [0u8; 260];
            let len = K32GetProcessImageFileNameA(handle, PCSTR(buffer.as_mut_ptr()), buffer.len() as u32);
    
            CloseHandle(handle);
    
            if len == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to retrieve process name for PID {}", pid),
                ));
            }
    
            Ok(String::from_utf8_lossy(&buffer[..len as usize]).into_owned())
        }
    }

    unsafe {
        let mut table_size: u32 = 0;

        let result = GetTcpTable2(std::ptr::null_mut(), &mut table_size, false);
        if result != NO_ERROR {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to determine TCP table size",
            ));
        }

        let mut buffer = vec![0u8; table_size as usize];
        let tcp_table: *mut MIB_TCPTABLE2 = buffer.as_mut_ptr() as _;

        let result = GetTcpTable2(tcp_table, &mut table_size, false);
        if result != NO_ERROR {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to retrieve TCP table",
            ));
        }

        let table = &*tcp_table;
        let rows: &[MIB_TCPROW2] =
            std::slice::from_raw_parts(&table.table[0], table.dwNumEntries as usize);

        for row in rows {
            let local_addr = SocketAddr::new(
                Ipv4Addr::from(u32::from_be(row.dwLocalAddr)).into(),
                u16::from_be((row.dwLocalPort as u16).to_be()),
            );
            let remote_addr = SocketAddr::new(
                Ipv4Addr::from(u32::from_be(row.dwRemoteAddr)).into(),
                u16::from_be((row.dwRemotePort as u16).to_be()),
            );

            if local_addr == *client_socket && remote_addr == *odd_box_socket {
                let pid = row.dwOwningPid;
                if let Ok(name) = get_process_name_by_pid(pid) {
                    return Some((name,pid));
                }
                
            }
        }
    }

    Ok(None) // No matching process found
}
