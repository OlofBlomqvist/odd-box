use std::fmt::Write as _;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use crate::control::ProcMessage;
use crate::global_state::GlobalState;
use crate::types::app_state::ProcState;
use crate::types::odd_box_event::EventForWebsocketClients;

pub fn init() {
    println!("Starting odd-box TUI (press Ctrl+C to exit)...");
}

fn fmt_state(state: ProcState) -> &'static str {
    match state {
        ProcState::Running => "running",
        ProcState::Starting => "starting",
        ProcState::Stopping => "stopping",
        ProcState::Stopped => "stopped",
        ProcState::Faulty => "faulty",
        ProcState::Remote => "remote",
        ProcState::DirServer => "dir",
        ProcState::Docker => "docker",
    }
}

async fn build_snapshot(global_state: &GlobalState) -> String {
    let status_map = global_state.app_state.site_status_map.clone();
    let cfg = global_state.config.read().await;

    let mut out = String::new();
    writeln!(&mut out, "odd-box status (read-only)").ok();
    writeln!(&mut out, "=======================").ok();

    let hosted: Vec<_> = cfg.hosted_processes.iter().map(|kv| kv.value().clone()).collect();
    writeln!(&mut out, "Hosted processes ({}):", hosted.len()).ok();
    for proc in hosted {
        let state = status_map
            .get(&proc.host_name)
            .map(|v| v.value().clone())
            .unwrap_or(ProcState::Stopped);
        let port = proc
            .active_port
            .or(proc.port)
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".into());
        writeln!(
            &mut out,
            " - {:<30} {:<10} port: {}",
            proc.host_name,
            fmt_state(state),
            port
        )
        .ok();
    }

    let remotes: Vec<_> = cfg.remote_sites.iter().map(|kv| kv.value().clone()).collect();
    writeln!(&mut out, "\nRemote sites ({}):", remotes.len()).ok();
    for remote in remotes {
        let state = status_map
            .get(&remote.host_name)
            .map(|v| v.value().clone())
            .unwrap_or(ProcState::Remote);
        writeln!(
            &mut out,
            " - {:<30} {:<10} backends: {}",
            remote.host_name,
            fmt_state(state),
            remote.backends.len()
        )
        .ok();
    }

    let dirs: Vec<_> = cfg.dir_server.clone().unwrap_or_default();
    writeln!(&mut out, "\nDir servers ({}):", dirs.len()).ok();
    for dir in dirs {
        let state = status_map
            .get(&dir.host_name)
            .map(|v| v.value().clone())
            .unwrap_or(ProcState::DirServer);
        writeln!(
            &mut out,
            " - {:<30} {:<10} dir: {}",
            dir.host_name,
            fmt_state(state),
            dir.dir
        )
        .ok();
    }

    let docker: Vec<_> = cfg.docker_containers.iter().map(|kv| kv.value().clone()).collect();
    writeln!(&mut out, "\nDocker ({}):", docker.len()).ok();
    for cont in docker {
        let host = cont.generate_host_name();
        let state = status_map
            .get(&host)
            .map(|v| v.value().clone())
            .unwrap_or(ProcState::Docker);
        writeln!(
            &mut out,
            " - {:<30} {:<10} image: {}",
            host,
            fmt_state(state),
            cont.image_name
        )
        .ok();
    }

    out
}

pub async fn run(
    global_state: Arc<GlobalState>,
    _tx: tokio::sync::broadcast::Sender<ProcMessage>,
    _trace_msg_broadcaster: tokio::sync::broadcast::Sender<EventForWebsocketClients>,
) {
    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    loop {
        if global_state
            .app_state
            .exit
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            break;
        }

        let snapshot = build_snapshot(&global_state).await;
        print!("\x1b[2J\x1b[H{}", snapshot);
        let _ = io::stdout().flush();

        ticker.tick().await;
    }
}
