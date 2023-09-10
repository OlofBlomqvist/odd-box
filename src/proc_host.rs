use super::types::*;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;


pub (crate) async fn host(proc:SiteConfig,mut rcv:tokio::sync::broadcast::Receiver<(String, bool)>) {

    let mut enabled = true;

    let domsplit = proc.host_name.split(".").collect::<Vec<&str>>();
    
    let mut acceptable_names = vec![proc.host_name.clone()];

    if domsplit.len() > 0 {
        acceptable_names.push(domsplit[0].to_owned());
    }
    
    let re = regex::Regex::new(r"^\d* *\[.*?\] .*? - ").unwrap();
    
    loop {
        
        let is_enabled_before = enabled == true;

        while let Ok((msg,state)) = rcv.try_recv() {
            let is_for_me = msg == "all" || acceptable_names.contains(&msg); 
            if is_for_me {
                enabled = state;
            }
        }
        
        if !enabled {
            if enabled != is_enabled_before {
                tracing::info!("[{}] Disabled via command from proxy service",proc.host_name);
            }
            tokio::time::sleep(Duration::from_millis(1111)).await;
            continue;
        }
        
        if enabled != is_enabled_before {
            tracing::info!("[{}] Enabled via command from proxy service",proc.host_name);
        }

        tracing::info!("[{}] Executing command '{}' in directory '{}'",proc.host_name,proc.bin,proc.path);

        let mut bin_path = std::path::PathBuf::from(&proc.path);
        bin_path.push(&proc.bin);
        
        let mut process_specific_environment_variables = HashMap::new();
        
        for kvp in &proc.env_vars.clone(){
            tracing::debug!("[{}] ADDING ENV VAR '{}': {}", &proc.host_name,&kvp.key,&kvp.value);
            process_specific_environment_variables.insert(kvp.key.clone(), kvp.value.clone());
        }  
        
        
        let cmd = Command::new(bin_path)
            .args(proc.args.clone())
            .envs(&process_specific_environment_variables)
            .current_dir(&proc.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn();

        match cmd {
            Ok(mut child) => {

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
                                    let trimmed = reclone.replace(&line, "");                            
                                    if trimmed.contains(" WARN ") || trimmed.contains("warn:") {
                                        current_log_level = 3;
                                    } else if trimmed.contains("ERROR") || trimmed.contains("error:"){
                                        current_log_level = 4;
                                    } else if trimmed.contains("DEBUG")|| trimmed.contains("debug:"){
                                        current_log_level = 1;
                                    } else if trimmed.contains("INFO")|| trimmed.contains("info:"){
                                        current_log_level = 2;
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
                        let is_for_me = msg == "all" || acceptable_names.contains(&msg); 
                        if is_for_me {
                            enabled = state;
                        }
                    }
                    if !enabled {
                        tracing::warn!("[{}] Stopping due to having been disabled by proxy.", proc.host_name);
                        // note: we just send q here because some apps like iisexpress requires it
                        if let Some(mut stdin) = child.stdin.take() {
                            _ = stdin.write_all(b"q");
                        } 
                        _ = child.kill();
                       
                        break;
                    } 
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                tracing::warn!("[{}] Stopped.",proc.host_name)
            },
            Err(e) => {
                tracing::info!("[{}] Failed to start! {e:?}",proc.host_name)
            },
        }
        
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
   
}