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
) -> std::io::Result<Option<(String, u32)>> {

    match (client_socket, odd_box_socket) {
        (std::net::SocketAddr::V4(_), std::net::SocketAddr::V4(_)) => get_process_by_socket_ipv4(client_socket, odd_box_socket),
        (std::net::SocketAddr::V6(_), std::net::SocketAddr::V6(_)) => get_process_by_socket_ipv6(client_socket, odd_box_socket),
        _ => Ok(None), // Mismatched address types or unsupported
    }
}


#[cfg(target_os = "windows")]
pub fn get_process_by_socket_ipv6(
    client_socket: &std::net::SocketAddr,
    server_socket: &std::net::SocketAddr,
) -> std::io::Result<Option<(String, u32)>> {

    use std::net::{Ipv6Addr, SocketAddr};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetTcpTable2, MIB_TCPTABLE2, MIB_TCPROW2, GetTcp6Table2, MIB_TCP6TABLE2, MIB_TCP6ROW2,
    };
    use windows_sys::Win32::System::ProcessStatus::K32GetProcessImageFileNameA;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};


    // Ensure the sockets are IPv6
    let client_ipv6 = match client_socket {
        SocketAddr::V6(addr) => addr,
        SocketAddr::V4(_) => return Ok(None), // Or handle as needed
    };
    let server_ipv6 = match server_socket {
        SocketAddr::V6(addr) => addr,
        SocketAddr::V4(_) => return Ok(None), // Or handle as needed
    };

    unsafe {
        let mut table_size: u32 = 0;

        // First call to determine the buffer size
        let result = GetTcp6Table2(None, &mut table_size, 0);
        if result != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to determine IPv6 TCP table size",
            ));
        }

        // Allocate buffer with the required size
        let mut buffer = vec![0u8; table_size as usize];
        let tcp_table: *mut MIB_TCP6TABLE2 = buffer.as_mut_ptr() as *mut MIB_TCP6TABLE2;

        // Actual call to get the TCP table
        let result = GetTcp6Table2(Some(tcp_table), &mut table_size, 0);
        if result != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to retrieve IPv6 TCP table",
            ));
        }

        let table = &*tcp_table;
        let rows: &[MIB_TCP6ROW2] = std::slice::from_raw_parts(
            table.table.as_ptr(),
            table.dwNumEntries as usize,
        );

        for row in rows {
            let local_addr = Ipv6Addr::from(row.ucLocalAddr);
            let remote_addr = Ipv6Addr::from(row.ucRemoteAddr);
            let local_socket = SocketAddr::new(local_addr.into(), u16::from_be(row.dwLocalPort));
            let remote_socket = SocketAddr::new(remote_addr.into(), u16::from_be(row.dwRemotePort));

            if &local_socket == client_socket && &remote_socket == server_socket {
                let pid = row.dwOwningPid;
                if let Ok(name) = get_process_name_by_pid(pid) {
                    return Ok(Some((name, pid)));
                }
            }
        }
    }

    Ok(None) // No matching process found
}

#[cfg(target_os = "windows")]
pub fn get_process_by_socket_ipv4(
    client_socket: &std::net::SocketAddr,
    server_socket: &std::net::SocketAddr,
) -> std::io::Result<Option<(String, u32)>> {



    use std::net::{Ipv4Addr, SocketAddr};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetTcpTable2, MIB_TCPTABLE2, MIB_TCPROW2, GetTcp6Table2, MIB_TCP6TABLE2, MIB_TCP6ROW2,
    };
    


    // Ensure the sockets are IPv4
    let client_ipv4 = match client_socket {
        SocketAddr::V4(addr) => addr,
        SocketAddr::V6(_) => return Ok(None), // Or handle as needed
    };
    let server_ipv4 = match server_socket {
        SocketAddr::V4(addr) => addr,
        SocketAddr::V6(_) => return Ok(None), // Or handle as needed
    };

    unsafe {
        let mut table_size: u32 = 0;

        // First call to determine the buffer size
        let result = GetTcpTable2(None, &mut table_size, 0);
        if result != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to determine IPv4 TCP table size",
            ));
        }

        // Allocate buffer with the required size
        let mut buffer = vec![0u8; table_size as usize];
        let tcp_table: *mut MIB_TCPTABLE2 = buffer.as_mut_ptr() as *mut MIB_TCPTABLE2;

        // Actual call to get the TCP table
        let result = GetTcpTable2(Some(tcp_table), &mut table_size, 0);
        if result != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to retrieve IPv4 TCP table",
            ));
        }

        let table = &*tcp_table;
        let rows: &[MIB_TCPROW2] = std::slice::from_raw_parts(
            table.table.as_ptr(),
            table.dwNumEntries as usize,
        );

        for row in rows {
            let local_addr = Ipv4Addr::from(u32::from_be(row.dwLocalAddr));
            let remote_addr = Ipv4Addr::from(u32::from_be(row.dwRemoteAddr));
            let local_socket = SocketAddr::new(local_addr.into(), u16::from_be(row.dwLocalPort));
            let remote_socket = SocketAddr::new(remote_addr.into(), u16::from_be(row.dwRemotePort));

            if &local_socket == client_socket && &remote_socket == server_socket {
                let pid = row.dwOwningPid;
                if let Ok(name) = get_process_name_by_pid(pid) {
                    return Ok(Some((name, pid)));
                }
            }
        }
    }

    Ok(None) // No matching process found
}

#[cfg(target_os = "windows")]
fn get_process_name_by_pid(pid: u32) -> std::io::Result<String> {

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetTcpTable2, MIB_TCPTABLE2, MIB_TCPROW2, GetTcp6Table2, MIB_TCP6TABLE2, MIB_TCP6ROW2,
    };
    use windows_sys::Win32::System::ProcessStatus::K32GetProcessImageFileNameA;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};



    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if handle == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to open process with PID {}", pid),
            ));
        }

        let mut buffer = [0u8; 260];
        let len = K32GetProcessImageFileNameA(
            handle,
            buffer.as_ptr() as *mut i8,
            buffer.len() as u32,
        );

        CloseHandle(handle);

        if len == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to retrieve process name for PID {}", pid),
            ));
        }

        // Convert the buffer to a Rust String
        let process_name = String::from_utf8_lossy(&buffer[..len as usize]).to_string();
        Ok(process_name)
    }
}