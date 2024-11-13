use crate::configuration::{LogFormat, LogLevel};
use crate::global_state::GlobalState;
use crate::http_proxy::ProcMessage;
use crate::types::app_state::ProcState;
use crate::types::odd_box_event::Event;
use crate::types::proc_info::ProcId;
use crate::types::site_status::{SiteStatusEvent, State};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
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

    crate::PROC_THREAD_MAP.insert(resolved_proc.proc_id.clone(), crate::types::proc_info::ProcInfo { 
        marked_for_removal: false,
        config: resolved_proc.clone(),
        pid: None,
        liveness_ptr: std::sync::Arc::<AtomicBool>::downgrade(&my_arc) 
    });


    let my_id = resolved_proc.proc_id.clone();

    let mut previous_update = ProcState::Stopped;

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


    let mut missing_bin: bool = false;

    loop {

        if missing_bin {
            // dont want to try this too often if file is gone
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        {
            let entry = crate::PROC_THREAD_MAP.get_mut(&resolved_proc.proc_id);
            match entry {
                Some(mut item) => {
                    item.pid = None;
                    if item.marked_for_removal {
                        tracing::warn!("Detected mark of removal, leaving main loop for {}",resolved_proc.host_name);
                        state.app_state.site_status_map.remove(&resolved_proc.host_name);
                        break;
                    }
                },
                None => {
                    tracing::warn!("Something has gone very wrong! A thread is missing from the global thread map.. this is a bug in odd-box.")
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
        let mut time_to_sleep_ms_after_each_iteration = 500;

        let exit = state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) == true;
        
        if exit {
            _ = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopped);
            tracing::debug!("exiting host for {}",&resolved_proc.host_name);
            break
        }

        if initialized == false {
            previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopped);
            initialized = true;
        } else {
            previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopped);
            
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
                    previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopped);
                }
            }
            continue;
        }

        
        if enabled != is_enabled_before {
            tracing::info!("[{}] Enabled via command from proxy service",&resolved_proc.host_name);
        }

        
        // just to make sure we havnt messed up timing-wise and selected the same port for two different processes
        // we will always call this function to get a new port (or keep the old one if we are the only one using it)
    
        let mut guard = state.config.write().await;
        if let Ok(p) = guard.set_active_port(&mut resolved_proc) {
            resolved_proc.active_port = Some(p);
        }
        drop(guard);
        
        if resolved_proc.active_port.is_none() {
            let ms = 3000;
            tracing::warn!("[{}] No usable port found. Waiting for {}ms before retrying..",&resolved_proc.host_name,ms);
            tokio::time::sleep(Duration::from_millis(ms)).await;
            continue;
        }
            
        let current_work_dir = std::env::current_dir().expect("could not get current directory").to_str().expect("could not convert current directory to string").to_string();
        
        let workdir = &resolved_proc.dir.as_ref().map_or(current_work_dir, |x|x.to_string());


        
        let (global_min_loglevel,_global_default_log_format) = {
            let guard = state.config.read().await;
            (guard.log_level.clone().unwrap_or(LogLevel::Info),guard.default_log_format.clone())
        };

        let do_initial_trace = if let Some(ref ll) = resolved_proc.log_level { ll == &LogLevel::Trace } else { global_min_loglevel == LogLevel::Trace };

        if do_initial_trace {
            tracing::trace!("[{}] Executing command '{}' in directory '{}'",resolved_proc.host_name,resolved_proc.bin,workdir);
        }

        
        let resolved_bin_path = if let Some(p) = resolve_bin_path(&workdir, &resolved_proc.bin) {
            missing_bin = false;
            p
        } else {
            tracing::error!("Failed to resolve path of binary for site: '{}' - workdir: {}, bin: {}",&resolved_proc.host_name,workdir,resolved_proc.bin);
            previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Faulty);
            missing_bin = true;
            continue
        };

        
        let mut process_specific_environment_variables = HashMap::new();
        
        {
            let state_guard = state.config.read().await;
            for kvp in &state_guard.env_vars.clone() {
                if do_initial_trace {
                    tracing::trace!("[{}] ADDING GLOBAL ENV VAR '{}': {}", &resolved_proc.host_name,&kvp.key,&kvp.value);
                }
                process_specific_environment_variables.insert(kvp.key.clone(), kvp.value.clone());
            }  
        }

        // more specific env vars should override globals
        for kvp in resolved_proc.env_vars.iter().flatten() {
            if do_initial_trace {
                tracing::trace!("[{}] ADDING ENV VAR '{}': {}", &resolved_proc.host_name,&kvp.key,&kvp.value);
            }
            process_specific_environment_variables.insert(kvp.key.clone(), kvp.value.clone());
        }  

        let port = resolved_proc.active_port
            .expect("it should not be possible to start a process without a port first having been chosen - this is a bug in odd-box").to_string();

        process_specific_environment_variables.insert("PORT".into(), port.clone());


        let mut pre_resolved_args = resolved_proc.args.clone().unwrap_or_default();

        for p in &mut pre_resolved_args {
            *p = p.replace("$port",&port);
        }

    
        previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Starting);

        const _CREATE_NO_WINDOW: u32 = 0x08000000;
        
        #[cfg(target_os = "windows")] 
        const DETACHED_PROCESS: u32 = 0x00000008;
            
        #[cfg(target_os="windows")]
        use std::os::windows::process::CommandExt;
        
        #[cfg(target_os = "windows")] 
        let cmd = Command::new(resolved_bin_path)
            .args(pre_resolved_args)
            .envs(&process_specific_environment_variables)
            .current_dir(&workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            // dont want windows to let child take over our keyboard input and such
            .creation_flags(DETACHED_PROCESS).spawn(); 

        #[cfg(not(target_os = "windows"))]
        let cmd = Command::new(resolved_bin_path)
            .args(pre_resolved_args)
            .envs(&process_specific_environment_variables)
            .current_dir(&workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn();

        match cmd {
            Ok(mut child) => {

                
                previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Running);

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
                

                let (_global_min_loglevel,global_default_log_format) = {
                    let guard = state.config.read().await;
                    (guard.log_level.clone().unwrap_or(LogLevel::Info),guard.default_log_format.clone())
                };

                let logformat = resolved_proc.log_format.clone().unwrap_or(global_default_log_format);


                // --- USE THE DEAFULT GLOBAL LOG FORMNAT!!!

                // note: global min loglevel IS NOT supposed to be used as default for processes - processes should always default to info
                let proc_loglevel = resolved_proc.log_level.clone().unwrap_or(LogLevel::Info);
                
                
                _ = std::thread::Builder::new().name(format!("{procname}")).spawn(move || {
                    
                    let mut current_log_level = 0;
                    
                    let min_log_level_for_the_process = match proc_loglevel {
                        LogLevel::Trace => 1,
                        LogLevel::Debug => 2,
                        LogLevel::Info => 3,
                        LogLevel::Warn => 4,
                        LogLevel::Error => 5
                    };

                    for line in std::io::BufRead::lines(stdout_reader) {
                        if let Ok(line) = line{

                            // todo: should move custom logging elsewhere if theres ever more than one
                            if let LogFormat::dotnet = &logformat {
                                if line.len() > 0 {
                                    let mut trimmed = reclone.replace(&line, "").to_string();                       
                                    if trimmed.contains(" WARN ") || trimmed.contains("warn:") {
                                        current_log_level = 4;
                                        trimmed.replace("warn:", "").trim().to_string();
                                    } else if trimmed.contains("ERROR") || trimmed.contains("error:") {
                                        current_log_level = 5;
                                        trimmed.replace("error:", "").trim().to_string();
                                    } else if trimmed.contains("DEBUG") || trimmed.contains("debug:") || trimmed.contains("dbug:") {
                                        current_log_level = 2;
                                        trimmed.replace("debug:", "").trim().to_string();
                                    } else if trimmed.contains("INFO")|| trimmed.contains("info:") {
                                        current_log_level = 3;
                                        trimmed = trimmed.replace("info:", "").trim().to_string()
                                    }
                                   
                                    if current_log_level >= min_log_level_for_the_process {
                                        match &current_log_level {
                                            1  => { 
                                                tracing::trace!("{}",trimmed)
                                            },
                                            2  => { 
                                                tracing::debug!("{}",trimmed)
                                            },
                                            3  => { 
                                                tracing::info!("{}",trimmed)
                                            },
                                            4 => { 
                                                tracing::warn!("{}",trimmed)
                                            }, 
                                            5  => { 
                                                tracing::error!("{}",trimmed)
                                            },
                                            _ => tracing::info!("{}",trimmed) 
                                        }  
                                    } else if current_log_level == 0 {
                                        tracing::info!("{}",trimmed)
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
                        previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopping);
                        _ = child.kill();
                        break
                    }

                    let live_proc_config = {
                        let entry = state.config.read().await;
                        let config = entry.hosted_processes.get(&resolved_proc.host_name);
                        if let Some(c) = config {
                            Some(c.clone())
                        } else {
                            None
                        }
                    };

                    if let Some(live_proc_config) = live_proc_config {
                        if live_proc_config.get_id() != &resolved_proc.proc_id {
                            tracing::warn!("[{}] Stopping due to having been replaced by a new process with the same name", resolved_proc.host_name);
                            previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopping);
                            _ = child.kill();
                            break
                        }
                        resolved_proc.log_format = live_proc_config.log_format;
                        resolved_proc.log_level = live_proc_config.log_level;
                    }
                    
                    previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Running);
                    
                
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

                    {
                        let entry = crate::PROC_THREAD_MAP.get_mut(&resolved_proc.proc_id);
                        match entry {
                            Some(item) => {
                                if item.marked_for_removal {               
                                    tracing::warn!("Detected mark of removal, leaving main loop for {}",resolved_proc.host_name);                     
                                    _ = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopping);
                                    if let Some(mut stdin) = child.stdin.take() {
                                        _ = stdin.write_all(b"q");
                                    } 
                                    _ = child.kill();
                                    return;
                                }
                            },
                            None => {
                                tracing::warn!("Something has gone very wrong! A thread is missing from the global thread map.. this is a bug in odd-box.")
                            }
                        }
                    }
            

                    if !enabled {
                        tracing::warn!("[{}] Stopping due to having been disabled by proxy.", resolved_proc.host_name);
                        // note: we just send q here because some apps like iisexpress requires it
                        
                        previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopping);
                        
                        if let Some(mut stdin) = child.stdin.take() {
                            _ = stdin.write_all(b"q");
                        } 
                        _ = child.kill();
                        break;
                    } 
                    
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Stopped);
                
            },
            Err(e) => {
                tracing::info!("[{}] Failed to start! {e:?}",resolved_proc.host_name);
                previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Faulty);    

                            
            },
        }
        
        if enabled {
            if !state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) {
                tracing::warn!("[{}] Stopped unexpectedly.. Will automatically restart the process in 5 seconds unless stopped.",resolved_proc.host_name);
                previous_update = update_status(&previous_update,&resolved_proc.host_name, &my_id,&state,ProcState::Faulty);
                time_to_sleep_ms_after_each_iteration = 5000; // wait 5 seconds before restarting but NOT in here as we have a lock
            } else {
                tracing::info!("[{}] Stopped due to exit signal. Bye!",resolved_proc.host_name);
                break
            }
        }
            
        tokio::time::sleep(Duration::from_millis(time_to_sleep_ms_after_each_iteration)).await;
    }
}


fn resolve_bin_path(workdir: &str, bin: &str) -> Option<PathBuf> {
    
    let bin_path = Path::new(bin);

    if bin_path.is_absolute() {
        if bin_path.exists() {
            return Some(bin_path.to_path_buf());
        }
    } else {
        let relative_path = Path::new(workdir).join(bin);
        if relative_path.exists() {
            return Some(relative_path);
        }
    }

    let current_work_dir = std::env::current_dir().expect("could not get current directory").to_str().expect("could not convert current directory to string").to_string();
    let relative_path = Path::new(&current_work_dir).join(bin);
    if relative_path.exists() {
        return Some(relative_path);
    }

    match which::which(bin) {
        Ok(path) => Some(path),
        Err(_) => None,
    }
}


fn update_status(previous:&ProcState,x:&str,id:&ProcId,g:&Arc<GlobalState>,s:ProcState,) -> ProcState {
  
    let emit = 
        if let Some(old_status) = g.app_state.site_status_map.insert(x.to_owned(),s.clone()) {
            if old_status != s {
                true
            } else {
                return s
            }
        } else { 
            true
        };

    // let emit = emit && match s {
    //     ProcState::Stopped => true,
    //     ProcState::Running => true,
    //     _ => false
    // };

    if emit {
       
        match g.global_broadcast_channel.send(Event::SiteStatusChange(SiteStatusEvent { 
            host_name: x.to_string(), 
            state: State::from_procstate(&s),
            id: id.clone()
        })) {
            Ok(_) => {
                s
            },
            Err(e) => {
                tracing::warn!("Failed to broadcast site status change event: {e:?}");
                previous.clone()
            }
        }
        
    } else {
        s
    }
}