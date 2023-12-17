use tokio::sync::Mutex;

use crate::{ProcState, AppState};

use super::types::*;
use std::collections::HashMap;
use std::io::Write;

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

#[cfg(target_os="windows")]
use std::os::windows::process::CommandExt;

pub (crate) async fn host(proc:SiteConfig,mut rcv:tokio::sync::broadcast::Receiver<(String, bool)>, state: Arc<Mutex<AppState>>) {

    let mut enabled = true;
    let mut exit = false;

    let domsplit = proc.host_name.split(".").collect::<Vec<&str>>();
    
    let mut acceptable_names = vec![proc.host_name.clone()];

    if domsplit.len() > 0 {
        acceptable_names.push(domsplit[0].to_owned());
    }
    
    let re = regex::Regex::new(r"^\d* *\[.*?\] .*? - ").unwrap();
    
    loop {

        if exit {
            tracing::debug!("exiting host for {}",&proc.host_name);
            break;
        }
        
        let is_enabled_before = enabled == true;

        while let Ok((msg,state)) = rcv.try_recv() {
            exit = msg == "exit";
            let is_for_me = exit || msg == "all" || acceptable_names.contains(&msg); 
            if is_for_me {
                enabled = if exit { false } else { state };
            }
        }
        
        if !enabled {
            if enabled != is_enabled_before {
                tracing::info!("[{}] Disabled via command from proxy service",&proc.host_name);
            }
            tokio::time::sleep(Duration::from_millis(1111)).await;
            continue;
        }
        
        if enabled != is_enabled_before {
            tracing::info!("[{}] Enabled via command from proxy service",&proc.host_name);
        }

        
        {
            let mut guard = state.lock().await;
            guard.procs.insert(proc.host_name.clone(), ProcState::Starting);
        }

        tracing::info!("[{}] Executing command '{}' in directory '{}'",proc.host_name,proc.bin,proc.path);

        let mut bin_path = std::path::PathBuf::from(&proc.path);
        bin_path.push(&proc.bin);
        
        let mut process_specific_environment_variables = HashMap::new();
        
        for kvp in &proc.env_vars.clone(){
            tracing::debug!("[{}] ADDING ENV VAR '{}': {}", &proc.host_name,&kvp.key,&kvp.value);
            process_specific_environment_variables.insert(kvp.key.clone(), kvp.value.clone());
        }  

        const _CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;
        
        #[cfg(target_os = "windows")] 
        let cmd = Command::new(bin_path)
            .args(proc.args.clone())
            .envs(&process_specific_environment_variables)
            .current_dir(&proc.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            // dont want windows to let child take over our keyboard input and such
            .creation_flags(DETACHED_PROCESS).spawn(); 

        #[cfg(not(target_os = "windows"))]
        let cmd = Command::new(bin_path)
            .args(proc.args.clone())
            .envs(&process_specific_environment_variables)
            .current_dir(&proc.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped()).spawn();

        match cmd {
            Ok(mut child) => {

                
                {
                    let mut guard = state.lock().await;
                    guard.procs.insert(proc.host_name.clone(), ProcState::Running);
                }

                //let stdin = child.stdin.take().expect("Failed to capture stdin");

                let stdout = child.stdout.take().expect("Failed to capture stdout");
                let stderr = child.stderr.take().expect("Failed to capture stderr");


                let stdout_reader = std::io::BufReader::new(stdout);
                let stderr_reader = std::io::BufReader::new(stderr);
                let procname = proc.host_name.clone();
                let reclone = re.clone();
                let logformat = proc.log_format.clone();
                _ = std::thread::Builder::new().name(format!("{procname}")).spawn(move || {
                    
                    let mut current_log_level = 0;
                    
                    for line in std::io::BufRead::lines(stdout_reader) {
                        if let Ok(line) = line{

                            // should move custom logging elsewhere if theres ever more than one
                            if let Some(LogFormat::dotnet) = &logformat {
                                if line.len() > 0 {
                                    let mut trimmed = reclone.replace(&line, "").to_string();                       
                                    if trimmed.contains(" WARN ") || trimmed.contains("warn:") {
                                        current_log_level = 3;
                                        trimmed.replace("warn:", "").trim().to_string();
                                    } else if trimmed.contains("ERROR") || trimmed.contains("error:"){
                                        current_log_level = 4;
                                        trimmed.replace("error:", "").trim().to_string();
                                    } else if trimmed.contains("DEBUG")|| trimmed.contains("debug:"){
                                        current_log_level = 1;
                                        trimmed.replace("debug:", "").trim().to_string();
                                    } else if trimmed.contains("INFO")|| trimmed.contains("info:"){
                                        current_log_level = 2;
                                        trimmed = trimmed.replace("info:", "").trim().to_string()
                                    }
                                    match &current_log_level {
                                        1 => tracing::debug!("{}",trimmed),
                                        2 => tracing::info!("{}",trimmed), 
                                        3 => tracing::warn!("{}",trimmed),
                                        4 => tracing::error!("{}",trimmed),
                                        _ => tracing::trace!("{}",trimmed) // hide anything does has no explicit level unless running in trace mode
                                    }  
                                } else {
                                    current_log_level = 0;
                                }
                            } else {
                                tracing::info!("{}",line)
                            }
                        }                        
                    }
                });

                let procname = proc.host_name.clone();
                _ = std::thread::Builder::new().name(format!("{procname}")).spawn(move || {
                    for line in std::io::BufRead::lines(stderr_reader) {
                        if let Ok(line) = line{
                            if line.len() > 0 {
                                tracing::error!("{}",line.trim());
                            }
                        }                        
                    }
                });
                
                while let Ok(None) = child.try_wait() {
                                    
                    while let Ok((msg,state)) = rcv.try_recv() {
                        exit = msg=="exit";
                        let is_for_me = exit || msg == "all" || acceptable_names.contains(&msg); 
                        if is_for_me {
                            enabled = if exit { false } else { state };
                        }
                    }
                    if !enabled {
                        tracing::warn!("[{}] Stopping due to having been disabled by proxy.", proc.host_name);
                        // note: we just send q here because some apps like iisexpress requires it
                        {
                            let mut guard = state.lock().await;
                            guard.procs.insert(proc.host_name.clone(), ProcState::Stopping);
                        }
                        if let Some(mut stdin) = child.stdin.take() {
                            _ = stdin.write_all(b"q");
                        } 
                        _ = child.kill();
                       
                        break;
                    } 
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                let mut guard = state.lock().await;
                guard.procs.insert(procname, ProcState::Stopped);
                tracing::warn!("[{}] Stopped.",proc.host_name)
            },
            Err(e) => {
                tracing::info!("[{}] Failed to start! {e:?}",proc.host_name);
                {
                    let mut guard = state.lock().await;
                    guard.procs.insert(proc.host_name.clone(), ProcState::Faulty);
                }
            },
        }
        
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
   
}

