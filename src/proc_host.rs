use crate::configuration::{EnvVar, LogFormat};
use crate::global_state::{self, GlobalState};
use crate::http_proxy::ProcMessage;
use crate::types::app_state::ProcState;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;


pub async fn host(
    mut resolved_proc: crate::configuration::v2::FullyResolvedInProcessSiteConfig,
    mut rcv:tokio::sync::broadcast::Receiver<ProcMessage>,
    state: Arc<GlobalState>
) {

    let my_arc = std::sync::Arc::new(AtomicBool::new(true));

    crate::PROC_THREAD_MAP.insert(resolved_proc.proc_id.clone(), crate::ProcInfo { 
        config: resolved_proc.clone(),
        pid: None,
        liveness_ptr: std::sync::Arc::<AtomicBool>::downgrade(&my_arc) 
    });



    // if auto_start is not set in the config, we assume that user wants to start site automatically like before
    let mut enabled = {
        // if auto_start is at all set for the specific process, use that value, otherwise use the global value
        // and otherwise fallback to assume that the site should be started automatically.
        match resolved_proc.auto_start {
            Some(v) => v,
            None => {
                let guard = state.config.read().await;
                guard.auto_start.unwrap_or(true)                
            }
        }
    };

    let excluded_from_auto_start = resolved_proc.excluded_from_start_all;

    let mut initialized = false;
    let domsplit = resolved_proc.host_name.split(".").collect::<Vec<&str>>();
    
    let mut acceptable_names = vec![resolved_proc.host_name.clone()];

    if domsplit.len() > 0 {
        acceptable_names.push(domsplit[0].to_owned());
    }
    
    let re = regex::Regex::new(r"^\d* *\[.*?\] .*? - ").expect("host regex always works");
    

    let mut selected_port: Option<u16> = None;

    loop {

        {
            let entry = crate::PROC_THREAD_MAP.get_mut(&resolved_proc.proc_id);
            match entry {
                Some(mut item) => {
                    item.pid = None;
                },
                None => {
                    tracing::warn!("Something has gone very wrong! A thread is missing from the global thread map.. this is a bug in odd-box.")
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
        let mut time_to_sleep_ms_after_loop = 500;
        // scope for clarity
        {
            let exit = state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) == true;
            
            if exit {
                state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Stopped);
                tracing::debug!("exiting host for {}",&resolved_proc.host_name);
                break
            }

            if initialized == false {
                state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Stopped);
                initialized = true;
            } else {
                state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Stopped);
                
            }
            
            let is_enabled_before = enabled == true;
            
            while let Ok(msg) = rcv.try_recv() {
                match msg {
                    ProcMessage::StartAll if excluded_from_auto_start => tracing::debug!("Refusing to start {} as thru the start all command as it is disabled",&resolved_proc.host_name),
                    ProcMessage::Start(s) if excluded_from_auto_start && s == "all" => tracing::debug!("Refusing to start {} as thru the start all command as it is disabled",&resolved_proc.host_name),

                    ProcMessage::Delete(s,sender) => {
                        if acceptable_names.contains(&s) {
                            tracing::warn!("[{}] Dropping due to having been deleted by proxy.", resolved_proc.host_name);
                            state.app_state.site_status_map.remove(&resolved_proc.host_name);
                            match sender.send(0).await {
                                Ok(_) => {},
                                Err(e) => {tracing::warn!("Failed to send confirmation to proxy service that we stopped! {e:?}")
                                },
                            }
                            return
                        }
                    },
                    ProcMessage::StartAll => enabled = true,
                    ProcMessage::StopAll => enabled = false,
                    ProcMessage::Start(s) => {
                        let is_for_me = s == "all"  || acceptable_names.contains(&s); 
                        if is_for_me {
                            enabled = true;
                        }
                    },
                    ProcMessage::Stop(s) => {
                        let is_for_me = s == "all" || acceptable_names.contains(&s); 
                        if is_for_me {
                            enabled = false;
                        }
                    }
                }
            }
            
            if !enabled {
                if enabled != is_enabled_before {
                    tracing::info!("[{}] Disabled via command from proxy service",&resolved_proc.host_name);
                    {
                        state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Stopped);
                    }
                }
                continue;
            }

            
            if enabled != is_enabled_before {
                tracing::info!("[{}] Enabled via command from proxy service",&resolved_proc.host_name);
            }

            
    
            
            if selected_port == None {
                
                let mut guard = state.config.write().await;
                
                if let Ok(p) = guard.set_active_port(&mut resolved_proc) {
                    selected_port = Some(p);
                    resolved_proc.active_port = selected_port;
                }
                
                if selected_port.is_none() {
                    let ms = 3000;
                    tracing::warn!("[{}] No usable port found. Waiting for {}ms before retrying..",&resolved_proc.host_name,ms);
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    continue;
                }
        

            }
            else {
                tracing::info!("[{}] Using the previously selected port '{}'",&resolved_proc.host_name,selected_port.unwrap());    
            }

            let current_work_dir = std::env::current_dir().expect("could not get current directory").to_str().expect("could not convert current directory to string").to_string();
            
            let workdir = &resolved_proc.dir.clone().unwrap_or(current_work_dir);

            tracing::warn!("[{}] Executing command '{}' in directory '{}'",resolved_proc.host_name,resolved_proc.bin,workdir);

            let mut bin_path = std::path::PathBuf::from(&workdir);
            bin_path.push(&resolved_proc.bin);

            let mut process_specific_environment_variables = HashMap::new();
            
            {
                let state_guard = state.config.read().await;
                for kvp in &state_guard.env_vars.clone() {
                    tracing::debug!("[{}] ADDING GLOBAL ENV VAR '{}': {}", &resolved_proc.host_name,&kvp.key,&kvp.value);
                    process_specific_environment_variables.insert(kvp.key.clone(), kvp.value.clone());
                }  
            }

            // more specific env vars should override globals
            for kvp in &resolved_proc.env_vars.clone().unwrap_or_default() {
                tracing::debug!("[{}] ADDING ENV VAR '{}': {}", &resolved_proc.host_name,&kvp.key,&kvp.value);
                process_specific_environment_variables.insert(kvp.key.clone(), kvp.value.clone());
            }  

            let port = selected_port
                .expect("it should not be possible to start a process without a port first having been chosen - this is a bug in odd-box").to_string();

            process_specific_environment_variables.insert("PORT".into(), port.clone());


            let mut pre_resolved_args = resolved_proc.args.clone().unwrap_or_default();

            for p in &mut pre_resolved_args {
                *p = p.replace("$port",&port);
            }

        
            state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Starting);
        



            const _CREATE_NO_WINDOW: u32 = 0x08000000;
            
            #[cfg(target_os = "windows")] 
            const DETACHED_PROCESS: u32 = 0x00000008;
                
            #[cfg(target_os="windows")]
            use std::os::windows::process::CommandExt;
            
            #[cfg(target_os = "windows")] 
            let cmd = Command::new(bin_path)
                .args(pre_resolved_args)
                .envs(&process_specific_environment_variables)
                .current_dir(&workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                // dont want windows to let child take over our keyboard input and such
                .creation_flags(DETACHED_PROCESS).spawn(); 

            #[cfg(not(target_os = "windows"))]
            let cmd = Command::new(bin_path)
                .args(pre_resolved_args)
                .envs(&process_specific_environment_variables)
                .current_dir(&workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn();

            match cmd {
                Ok(mut child) => {

                    
                    state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Running);
                    {
                        let entry = crate::PROC_THREAD_MAP.get_mut(&resolved_proc.proc_id);
                        match entry {
                            Some(mut item) => {
                                item.pid = Some(child.id().to_string());
                                // this is the only thing that is supposed to change during the lifetime of a proc loop
                                item.config.active_port = resolved_proc.active_port;
                            },
                            None => {
                                tracing::warn!("Something has gone very wrong! A thread is missing from the global thread map.. this is a bug in odd-box.")
                            }
                        }
                    }

                    //let stdin = child.stdin.take().expect("Failed to capture stdin");

                    let stdout = child.stdout.take().expect("Failed to capture stdout");
                    let stderr = child.stderr.take().expect("Failed to capture stderr");


                    let stdout_reader = std::io::BufReader::new(stdout);
                    let stderr_reader = std::io::BufReader::new(stderr);
                    let procname = resolved_proc.host_name.clone();
                    let reclone = re.clone();
                    let logformat = resolved_proc.log_format.clone();
                    _ = std::thread::Builder::new().name(format!("{procname}")).spawn(move || {
                        
                        let mut current_log_level = 0;
                        
                        for line in std::io::BufRead::lines(stdout_reader) {
                            if let Ok(line) = line{

                                // todo: should move custom logging elsewhere if theres ever more than one
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

                    let procname = resolved_proc.host_name.clone();
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
                        
                        let exit = state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) == true;
                        if exit {
                            tracing::info!("[{}] Stopping due to app exit", resolved_proc.host_name);
                            state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Stopping);
                            _ = child.kill();
                            break
                        }
                        
                    
                        while let Ok(msg) = rcv.try_recv() {
                            match msg {
                                ProcMessage::Delete(s,sender) => {
                                    if acceptable_names.contains(&s) {
                                        tracing::warn!("[{}] Dropping due to having been deleted by proxy.", resolved_proc.host_name);
                                        state.app_state.site_status_map.remove(&resolved_proc.host_name);
                                        if let Some(mut stdin) = child.stdin.take() {
                                            _ = stdin.write_all(b"q");
                                        } 
                                        _ = child.kill();
                                        // inform sender that we actually stopped the process and that we are exiting our loop
                                        match sender.send(0).await {
                                            Ok(_) => {},
                                            Err(e) => {
                                                tracing::warn!("Failed to send confirmation to proxy service that we stopped! {e:?}")
                                            },
                                        }
                                        return
                                    }
                                },
                                ProcMessage::StartAll => enabled = true,
                                ProcMessage::StopAll => enabled = false,
                                ProcMessage::Stop(s) => {
                                    let is_for_me = s == "all" || acceptable_names.contains(&s); 
                                    if is_for_me {
                                        enabled = false;
                                    }
                                },
                                _ => {}
                            }
                        }
                        if !enabled {
                            tracing::warn!("[{}] Stopping due to having been disabled by proxy.", resolved_proc.host_name);
                            // note: we just send q here because some apps like iisexpress requires it
                            
                            state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Stopping);
                            
                            if let Some(mut stdin) = child.stdin.take() {
                                _ = stdin.write_all(b"q");
                            } 
                            _ = child.kill();
                            break;
                        } 
                        
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    state.app_state.site_status_map.insert(procname, ProcState::Stopped);
                    
                },
                Err(e) => {
                    tracing::info!("[{}] Failed to start! {e:?}",resolved_proc.host_name);
                    state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Faulty);                
                },
            }
            
            if enabled {
                if !state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) {
                    tracing::warn!("[{}] Stopped unexpectedly.. Will automatically restart the process in 5 seconds unless stopped.",resolved_proc.host_name);
                    state.app_state.site_status_map.insert(resolved_proc.host_name.clone(), ProcState::Faulty);
                    time_to_sleep_ms_after_loop = 5000; // wait 5 seconds before restarting but NOT in here as we have a lock
                } else {
                    tracing::info!("[{}] Stopped due to exit signal. Will not restart.",resolved_proc.host_name);
                    break
                }
            }
            
        }
        tokio::time::sleep(Duration::from_millis(time_to_sleep_ms_after_loop)).await;
    }
   
}

