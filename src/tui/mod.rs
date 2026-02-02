use std::fmt::Write as _;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;
use crate::global_state::ProcState;
use crate::global_state::GlobalState;

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
    let snapshot = global_state.process_registry.snapshot();
    let cfg = global_state.config.read().await;
    let cruma_assignment = global_state.cruma_assignment.read().await.clone();

    let mut out = String::new();
    writeln!(&mut out, "odd-box status (read-only)").ok();
    writeln!(&mut out, "=======================").ok();

    if let Some(assignment) = cruma_assignment {
        writeln!(
            &mut out,
            "Cruma tunnel: {} ({})",
            assignment.assigned_domain, assignment.welcome_message
        )
        .ok();
    } else {
        writeln!(&mut out, "Cruma tunnel: pending assignment").ok();
    }

    let hosted: Vec<_> = cfg
        .hosted_processes
        .iter()
        .map(|kv| (kv.key().clone(), kv.value().clone()))
        .collect();
    writeln!(&mut out, "Hosted processes ({}):", hosted.len()).ok();
    for (backend_id, _proc) in hosted {
        let handle = snapshot.get(&backend_id);
        let state = handle
            .map(|h| h.proc_state())
            .unwrap_or(ProcState::Stopped);
        let port = handle
            .and_then(|h| h.active_port())
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        writeln!(
            &mut out,
            " - {:<30} {:<10} port: {}",
            backend_id,
            fmt_state(state),
            port
        )
        .ok();
    }

    let remotes: Vec<_> = cfg
        .remote_sites
        .iter()
        .map(|kv| (kv.key().clone(), kv.value().clone()))
        .collect();
    writeln!(&mut out, "\nRemote sites ({}):", remotes.len()).ok();
    for (backend_id, remote) in remotes {
        let state = snapshot
            .state_of(&backend_id)
            .unwrap_or(ProcState::Remote);
        writeln!(
            &mut out,
            " - {:<30} {:<10} endpoints: {}",
            backend_id,
            fmt_state(state),
            remote.endpoints.len()
        )
        .ok();
    }

    let dirs: Vec<_> = cfg
        .static_sites
        .iter()
        .map(|kv| (kv.key().clone(), kv.value().clone()))
        .collect();
    writeln!(&mut out, "\nStatic sites ({}):", dirs.len()).ok();
    for (backend_id, dir) in dirs {
        let state = snapshot
            .state_of(&backend_id)
            .unwrap_or(ProcState::DirServer);
        writeln!(
            &mut out,
            " - {:<30} {:<10} dir: {}",
            backend_id,
            fmt_state(state),
            dir.dir
        )
        .ok();
    }

    let docker: Vec<_> = cfg
        .docker_containers
        .iter()
        .map(|kv| kv.value().clone())
        .collect();
    writeln!(&mut out, "\nDocker ({}):", docker.len()).ok();
    for cont in docker {
        let host = cont.generate_host_name();
        let state = snapshot
            .state_of(&host)
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
    global_state: Arc<GlobalState>
) {
    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    loop {
        if global_state
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
