//! One-time migration tool: reads an old odd-box config (V3 TOML or V4 YAML)
//! and writes the equivalent cruma agent YAML config to stdout.

use anyhow::{Result, bail};
use serde_yaml::Value;

/// Main entry point for config migration.
///
/// Detects the input format (TOML vs YAML) automatically based on file
/// extension and content, then delegates to the appropriate parser.
pub fn migrate_v4_config(old_path: &str) -> Result<()> {
    let contents = std::fs::read_to_string(old_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {old_path}: {e}"))?;

    let is_toml = old_path.ends_with(".toml")
        || (!old_path.ends_with(".yaml")
            && !old_path.ends_with(".yml")
            && looks_like_toml(&contents));

    if is_toml {
        migrate_v3_toml(old_path, &contents)
    } else {
        migrate_v4_yaml(old_path, &contents)
    }
}

/// Heuristic: if the content contains TOML table headers like `[[hosted_process]]`
/// or key = "value" style assignments on the first few significant lines, treat
/// it as TOML.
fn looks_like_toml(contents: &str) -> bool {
    for line in contents.lines().take(30) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // TOML table header
        if trimmed.starts_with("[[") || trimmed.starts_with('[') {
            return true;
        }
        // TOML key = value (with equals sign, no colon)
        if trimmed.contains('=') && !trimmed.contains(':') {
            return true;
        }
        // YAML key: value
        if trimmed.contains(':') && !trimmed.contains('=') {
            return false;
        }
        break;
    }
    false
}

// ─── V3 TOML migration ────────────────────────────────────────────────────

/// Migrate a V3 (or V2) TOML config to the agent YAML format.
///
/// V3 TOML uses:
///   - `version = "V3"` (or "V2")
///   - `ip`, `http_port`, `tls_port`
///   - `[[hosted_process]]` with `host_name`, `bin`, `args`, `port`, `auto_start`, `env`, `hints`
///   - `[[remote_target]]` with `host_name`, `backends = [{ address, port, https }]`
///   - `[[dir_server]]` with `host_name`, `dir`
///   - `odd_box_url`, `odd_box_password` (dropped — no equivalent)
///   - `env_vars` for global environment
///   - `port_range_start` for auto-assigned ports
fn migrate_v3_toml(old_path: &str, contents: &str) -> Result<()> {
    let toml_val: toml::Value = contents
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse {old_path} as TOML: {e}"))?;

    let table = toml_val
        .as_table()
        .ok_or_else(|| anyhow::anyhow!("Expected a TOML table at the root of {old_path}"))?;

    let version = table.get("version").and_then(|v| v.as_str()).unwrap_or("");

    if !version.is_empty() && version != "V3" && version != "V2" && version != "V1" {
        bail!(
            "Expected a V1/V2/V3 TOML config, found version: {version}. \
             If this is a V4 YAML config, rename it with a .yaml extension."
        );
    }

    let ip = table
        .get("ip")
        .and_then(|v| v.as_str())
        .unwrap_or("127.0.0.1");
    let http_port = table
        .get("http_port")
        .and_then(|v| v.as_integer())
        .unwrap_or(8080) as u16;
    let tls_port = table
        .get("tls_port")
        .and_then(|v| v.as_integer())
        .unwrap_or(4343) as u16;
    let port_range_start = table
        .get("port_range_start")
        .and_then(|v| v.as_integer())
        .unwrap_or(4200) as u16;

    let default_auto_start = table
        .get("auto_start")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut backends = Vec::<Value>::new();
    let mut processes = Vec::<Value>::new();
    let mut frontends = Vec::<Value>::new();
    let mut global_env = serde_yaml::Mapping::new();

    // ── Global env vars ────────────────────────────────────────────────
    if let Some(env_table) = table.get("env_vars").and_then(|v| v.as_table()) {
        for (k, v) in env_table {
            if let Some(val) = v.as_str() {
                global_env.insert(y_str(k), Value::String(val.to_string()));
            }
        }
    }

    // ── Listeners ──────────────────────────────────────────────────────
    let mut listeners = Vec::<Value>::new();

    let mut http_listener = serde_yaml::Mapping::new();
    http_listener.insert(y_str("port"), Value::Number(http_port.into()));
    http_listener.insert(y_str("addr"), Value::String(listener_addr(ip)));
    http_listener.insert(y_str("tls"), Value::Bool(false));
    listeners.push(Value::Mapping(http_listener));

    let mut https_listener = serde_yaml::Mapping::new();
    https_listener.insert(y_str("port"), Value::Number(tls_port.into()));
    https_listener.insert(y_str("addr"), Value::String(listener_addr(ip)));
    https_listener.insert(y_str("tls"), Value::Bool(true));
    listeners.push(Value::Mapping(https_listener));

    // ── Hosted processes ───────────────────────────────────────────────
    let mut port_offset: u16 = 0;
    if let Some(hosted) = table.get("hosted_process").and_then(|v| v.as_array()) {
        for entry in hosted {
            let entry = match entry.as_table() {
                Some(t) => t,
                None => continue,
            };

            let host_name = entry
                .get("host_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let bin = entry
                .get("bin")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let args: Vec<String> = entry
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let working_dir = entry
                .get("dir")
                .or_else(|| entry.get("working_directory"))
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .to_string();

            let auto_start = entry
                .get("auto_start")
                .and_then(|v| v.as_bool())
                .unwrap_or(default_auto_start);

            // Use explicit port if given, otherwise auto-assign
            let assigned_port = entry
                .get("port")
                .and_then(|v| v.as_integer())
                .map(|p| p as u16)
                .unwrap_or_else(|| {
                    let p = port_range_start + port_offset;
                    port_offset += 1;
                    p
                });

            // Derive an ID from the hostname (strip TLD-like suffixes)
            let proc_id = sanitize_id(&host_name);

            // Per-process env (merge global + per-process)
            let mut proc_env = global_env.clone();
            if let Some(env_table) = entry.get("env").and_then(|v| v.as_table()) {
                for (k, v) in env_table {
                    if let Some(val) = v.as_str() {
                        proc_env.insert(y_str(k), Value::String(val.to_string()));
                    }
                }
            }

            // Build the process entry
            let mut process = serde_yaml::Mapping::new();
            process.insert(y_str("id"), Value::String(proc_id.clone()));
            process.insert(y_str("command"), Value::String(bin));
            if !args.is_empty() {
                // Replace $port with the assigned port in args
                let args_resolved: Vec<String> = args
                    .iter()
                    .map(|a| a.replace("$port", &assigned_port.to_string()))
                    .collect();
                process.insert(
                    y_str("args"),
                    Value::Sequence(args_resolved.into_iter().map(Value::String).collect()),
                );
            }
            process.insert(y_str("working_directory"), Value::String(working_dir));
            process.insert(y_str("auto_start"), Value::Bool(auto_start));
            if !proc_env.is_empty() {
                process.insert(y_str("env"), Value::Mapping(proc_env));
            }

            // Check for hints (H2, H2CPK, etc.)
            let hints: Vec<String> = entry
                .get("hints")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let upstream_protocol = if hints.iter().any(|h| h == "H2CPK") {
                Some("H2PK")
            } else if hints.iter().any(|h| h == "H2") {
                Some("H2")
            } else {
                None
            };

            if let Some(proto) = upstream_protocol {
                process.insert(y_str("upstream_protocol"), Value::String(proto.to_string()));
            }

            processes.push(Value::Mapping(process));

            // Create a backend for this process
            let mut backend = serde_yaml::Mapping::new();
            backend.insert(y_str("id"), Value::String(proc_id.clone()));
            backend.insert(y_str("kind"), Value::String("http".into()));
            backend.insert(
                y_str("destination"),
                Value::String(format!("localhost:{assigned_port}")),
            );
            backends.push(Value::Mapping(backend));

            // Create a frontend mapping hostname → process
            let mut frontend = serde_yaml::Mapping::new();
            frontend.insert(y_str("hostname"), Value::String(host_name));
            frontend.insert(y_str("process_id"), Value::String(proc_id));
            frontends.push(Value::Mapping(frontend));
        }
    }

    // ── Remote targets ─────────────────────────────────────────────────
    if let Some(remotes) = table.get("remote_target").and_then(|v| v.as_array()) {
        for entry in remotes {
            let entry = match entry.as_table() {
                Some(t) => t,
                None => continue,
            };

            let host_name = entry
                .get("host_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let backend_id = sanitize_id(&host_name);

            // Parse backends array: [{ address, port, https }]
            let remote_backends = entry
                .get("backends")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if let Some(first) = remote_backends.first().and_then(|b| b.as_table()) {
                let address = first
                    .get("address")
                    .and_then(|v| v.as_str())
                    .unwrap_or("localhost");
                let port = first.get("port").and_then(|v| v.as_integer()).unwrap_or(80) as u16;
                let use_https = first
                    .get("https")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let kind = if use_https { "https" } else { "http" };

                let mut backend = serde_yaml::Mapping::new();
                backend.insert(y_str("id"), Value::String(backend_id.clone()));
                backend.insert(y_str("kind"), Value::String(kind.into()));
                backend.insert(
                    y_str("destination"),
                    Value::String(format!("{address}:{port}")),
                );
                backends.push(Value::Mapping(backend));
            }

            // Frontend
            let mut frontend = serde_yaml::Mapping::new();
            frontend.insert(y_str("hostname"), Value::String(host_name));
            frontend.insert(y_str("backend_id"), Value::String(backend_id));
            frontends.push(Value::Mapping(frontend));
        }
    }

    // ── Directory servers ──────────────────────────────────────────────
    if let Some(dirs) = table.get("dir_server").and_then(|v| v.as_array()) {
        for entry in dirs {
            let entry = match entry.as_table() {
                Some(t) => t,
                None => continue,
            };

            let host_name = entry
                .get("host_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let dir = entry
                .get("dir")
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .to_string();

            let backend_id = sanitize_id(&host_name);

            let mut backend = serde_yaml::Mapping::new();
            backend.insert(y_str("id"), Value::String(backend_id.clone()));
            backend.insert(y_str("kind"), Value::String("local-directory".into()));
            backend.insert(y_str("destination"), Value::String(dir));
            backend.insert(y_str("allow_directory_indexing"), Value::Bool(true));
            backends.push(Value::Mapping(backend));

            let mut frontend = serde_yaml::Mapping::new();
            frontend.insert(y_str("hostname"), Value::String(host_name));
            frontend.insert(y_str("backend_id"), Value::String(backend_id));
            frontends.push(Value::Mapping(frontend));
        }
    }

    // ── Warnings for dropped features ──────────────────────────────────
    if table.contains_key("odd_box_url") || table.contains_key("odd_box_password") {
        eprintln!(
            "Note: odd_box_url and odd_box_password are no longer used. \
             The admin UI is built into the GUI/TUI."
        );
    }

    // ── Emit output ────────────────────────────────────────────────────
    emit_output(
        &listeners,
        &backends,
        &processes,
        &frontends,
        &global_env,
        // V3 TOML had no cruma tunnel config — default to local-only
        "ANON",
        "ANON",
        true,
        "V3 TOML",
        old_path,
    )
}

// ─── V4 YAML migration ────────────────────────────────────────────────────

/// Migrate a V4 YAML config to the agent YAML format.
fn migrate_v4_yaml(old_path: &str, contents: &str) -> Result<()> {
    let v4: Value = serde_yaml::from_str(contents)
        .map_err(|e| anyhow::anyhow!("Failed to parse {old_path} as YAML: {e}"))?;

    // Verify this looks like a V4 config
    let version = v4.get("version").and_then(|v| v.as_str()).unwrap_or("");
    if !version.is_empty() && version != "V4" {
        bail!(
            "Expected a V4 config (or no version field), found version: {version}. \
             Only V4 YAML configs can be migrated with this path. \
             If this is a V3 TOML config, rename it with a .toml extension."
        );
    }

    let mut backends = Vec::<Value>::new();
    let mut processes = Vec::<Value>::new();
    let mut frontends = Vec::<Value>::new();
    let mut listeners = Vec::<Value>::new();
    let mut global_env = serde_yaml::Mapping::new();

    // ── Parse global env ───────────────────────────────────────────────
    if let Some(env_map) = v4.get("env").and_then(|e| e.as_mapping()) {
        global_env = env_map.clone();
    }

    let auto_start_global = v4
        .get("auto_start")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let port_range_start = v4
        .get("port_range_start")
        .and_then(|v| v.as_u64())
        .unwrap_or(4200) as u16;

    // ── Parse backends ─────────────────────────────────────────────────
    let mut port_offset: u16 = 0;
    if let Some(v4_backends) = v4.get("backends").and_then(|b| b.as_mapping()) {
        for (name, def) in v4_backends {
            let id = name.as_str().unwrap_or("unknown").to_string();
            let def = match def.as_mapping() {
                Some(m) => m,
                None => continue,
            };

            let backend_type = def
                .get(&Value::String("type".into()))
                .and_then(|t| t.as_str())
                .unwrap_or("");

            match backend_type {
                "process" => {
                    let bin = def
                        .get(&Value::String("bin".into()))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let args: Vec<String> = def
                        .get(&Value::String("args".into()))
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();

                    let working_dir = def
                        .get(&Value::String("dir".into()))
                        .and_then(|v| v.as_str())
                        .unwrap_or(".")
                        .to_string();

                    let auto_start = def
                        .get(&Value::String("auto_start".into()))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(auto_start_global);

                    let assigned_port = port_range_start + port_offset;
                    port_offset += 1;

                    // Process env (merge global + per-process)
                    let mut proc_env = global_env.clone();
                    if let Some(env) = def
                        .get(&Value::String("env".into()))
                        .and_then(|v| v.as_mapping())
                    {
                        for (k, v) in env {
                            proc_env.insert(k.clone(), v.clone());
                        }
                    }

                    // Inject $PORT into env if not already present
                    let port_key = Value::String("PORT".into());
                    if !proc_env.contains_key(&port_key) {
                        proc_env.insert(port_key, Value::String(assigned_port.to_string()));
                    }

                    let mut process = serde_yaml::Mapping::new();
                    process.insert(y_str("id"), Value::String(id.clone()));
                    process.insert(y_str("command"), Value::String(bin));
                    if !args.is_empty() {
                        process.insert(
                            y_str("args"),
                            Value::Sequence(args.into_iter().map(Value::String).collect()),
                        );
                    }
                    process.insert(y_str("working_directory"), Value::String(working_dir));
                    process.insert(y_str("auto_start"), Value::Bool(auto_start));
                    if !proc_env.is_empty() {
                        process.insert(y_str("env"), Value::Mapping(proc_env));
                    }
                    processes.push(Value::Mapping(process));

                    // Process-backed routes use process_id in frontends,
                    // but we also create a backend entry so the proxy knows
                    // where to forward traffic.
                    let mut backend = serde_yaml::Mapping::new();
                    backend.insert(y_str("id"), Value::String(id));
                    backend.insert(y_str("kind"), Value::String("http".into()));
                    backend.insert(
                        y_str("destination"),
                        Value::String(format!("localhost:{assigned_port}")),
                    );
                    backends.push(Value::Mapping(backend));
                }
                "remote" => {
                    let endpoints = def
                        .get(&Value::String("endpoints".into()))
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    let destination = endpoints.first().cloned().unwrap_or_default();

                    let use_https = def
                        .get(&Value::String("https".into()))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let kind = if use_https { "https" } else { "http" };

                    let mut backend = serde_yaml::Mapping::new();
                    backend.insert(y_str("id"), Value::String(id));
                    backend.insert(y_str("kind"), Value::String(kind.into()));
                    backend.insert(y_str("destination"), Value::String(destination));
                    backends.push(Value::Mapping(backend));
                }
                "static" => {
                    let dir = def
                        .get(&Value::String("dir".into()))
                        .and_then(|v| v.as_str())
                        .unwrap_or(".")
                        .to_string();

                    let allow_listing = def
                        .get(&Value::String("list_dir".into()))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let render_markdown = def
                        .get(&Value::String("render_markdown".into()))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let spa_fallback = def
                        .get(&Value::String("spa_fallback".into()))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let mut backend = serde_yaml::Mapping::new();
                    backend.insert(y_str("id"), Value::String(id));
                    backend.insert(y_str("kind"), Value::String("local-directory".into()));
                    backend.insert(y_str("destination"), Value::String(dir));
                    if allow_listing {
                        backend.insert(
                            y_str("allow_directory_indexing"),
                            Value::Bool(allow_listing),
                        );
                    }
                    if render_markdown {
                        backend.insert(y_str("render_markdown"), Value::Bool(render_markdown));
                    }
                    if spa_fallback {
                        backend.insert(y_str("spa_fallback"), Value::Bool(spa_fallback));
                    }
                    backends.push(Value::Mapping(backend));
                }
                other => {
                    eprintln!("Warning: unknown backend type '{other}' for '{id}', skipping");
                }
            }
        }
    }

    // ── Parse frontends (HTTP) ─────────────────────────────────────────
    if let Some(http) = v4.get("frontends").and_then(|f| f.get("http")) {
        let port = http.get("port").and_then(|p| p.as_u64()).unwrap_or(8080) as u16;
        let addr = v4.get("ip").and_then(|v| v.as_str()).unwrap_or("127.0.0.1");

        let mut listener = serde_yaml::Mapping::new();
        listener.insert(y_str("port"), Value::Number(port.into()));
        listener.insert(y_str("addr"), Value::String(listener_addr(addr)));
        listener.insert(y_str("tls"), Value::Bool(false));
        listeners.push(Value::Mapping(listener));

        parse_yaml_routes(http, &mut frontends);
    }

    // ── Parse frontends (HTTPS) ────────────────────────────────────────
    if let Some(https) = v4.get("frontends").and_then(|f| f.get("https")) {
        let port = https.get("port").and_then(|p| p.as_u64()).unwrap_or(4343) as u16;
        let addr = v4.get("ip").and_then(|v| v.as_str()).unwrap_or("127.0.0.1");

        let mut listener = serde_yaml::Mapping::new();
        listener.insert(y_str("port"), Value::Number(port.into()));
        listener.insert(y_str("addr"), Value::String(listener_addr(addr)));
        listener.insert(y_str("tls"), Value::Bool(true));
        listeners.push(Value::Mapping(listener));

        // "inherit" means the HTTPS routes are the same as HTTP routes
        let routes_val = https.get("routes");
        let is_inherit = routes_val
            .and_then(|v| v.as_str())
            .map(|s| s == "inherit")
            .unwrap_or(false);

        if !is_inherit {
            parse_yaml_routes(https, &mut frontends);
        }
    }

    // ── Parse cruma tunnel config ──────────────────────────────────────
    let (tunnel_id, tunnel_secret, local_only) = parse_cruma_config(&v4);

    emit_output(
        &listeners,
        &backends,
        &processes,
        &frontends,
        &global_env,
        &tunnel_id,
        &tunnel_secret,
        local_only,
        "V4 YAML",
        old_path,
    )
}

// ─── Shared helpers ────────────────────────────────────────────────────────

/// Emit the final YAML output to stdout with migration info on stderr.
#[allow(clippy::too_many_arguments)]
fn emit_output(
    listeners: &[Value],
    backends: &[Value],
    processes: &[Value],
    frontends: &[Value],
    global_env: &serde_yaml::Mapping,
    tunnel_id: &str,
    tunnel_secret: &str,
    local_only: bool,
    source_format: &str,
    old_path: &str,
) -> Result<()> {
    let mut output = serde_yaml::Mapping::new();

    output.insert(y_str("tunnel_id"), Value::String(tunnel_id.to_string()));
    output.insert(
        y_str("tunnel_secret"),
        Value::String(tunnel_secret.to_string()),
    );
    output.insert(y_str("local_only"), Value::Bool(local_only));

    if !listeners.is_empty() {
        output.insert(y_str("listeners"), Value::Sequence(listeners.to_vec()));
    }
    if !backends.is_empty() {
        output.insert(y_str("backends"), Value::Sequence(backends.to_vec()));
    }
    if !processes.is_empty() {
        output.insert(y_str("processes"), Value::Sequence(processes.to_vec()));
    }
    if !frontends.is_empty() {
        output.insert(y_str("frontends"), Value::Sequence(frontends.to_vec()));
    }
    if !global_env.is_empty() {
        output.insert(y_str("global_env"), Value::Mapping(global_env.clone()));
    }

    let yaml = serde_yaml::to_string(&Value::Mapping(output))?;
    let header = format!(
        "# Migrated from odd-box {source_format} config\n\
         # Review this file carefully before using it.\n\n"
    );
    println!("{header}{yaml}");

    eprintln!();
    eprintln!("Migration complete (from {source_format}).");
    eprintln!("Review the output above, then save it:");
    eprintln!("  odd-box --migrate {old_path} > odd-box.yaml");

    Ok(())
}

/// Parse route mappings from a V4 YAML frontends.http or frontends.https block.
fn parse_yaml_routes(section: &Value, frontends: &mut Vec<Value>) {
    let routes = match section.get("routes").and_then(|r| r.as_mapping()) {
        Some(r) => r,
        None => return,
    };

    for (hostname, target) in routes {
        let hostname = match hostname.as_str() {
            Some(h) => h.to_string(),
            None => continue,
        };

        let backend_id = match target {
            Value::String(s) => s.clone(),
            Value::Mapping(m) => m
                .get(&Value::String("backend".into()))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            _ => continue,
        };

        if backend_id.is_empty() {
            continue;
        }

        let mut frontend = serde_yaml::Mapping::new();
        frontend.insert(y_str("hostname"), Value::String(hostname));
        frontend.insert(y_str("backend_id"), Value::String(backend_id));

        // Carry over forward_host if present
        if let Value::Mapping(m) = target {
            if let Some(fh) = m
                .get(&Value::String("forward_host".into()))
                .and_then(|v| v.as_bool())
            {
                frontend.insert(y_str("forward_host"), Value::Bool(fh));
            }
        }

        frontends.push(Value::Mapping(frontend));
    }
}

/// Extract cruma tunnel credentials from a V4 YAML config.
fn parse_cruma_config(v4: &Value) -> (String, String, bool) {
    let cruma = match v4.get("cruma") {
        Some(c) => c,
        None => return ("ANON".into(), "ANON".into(), true),
    };

    // Check for "anon: true" mode
    if let Some(true) = cruma.get("anon").and_then(|v| v.as_bool()) {
        return ("ANON".into(), "ANON".into(), false);
    }

    // Check for authenticated mode
    if let Some(auth) = cruma.get("auth").and_then(|a| a.as_mapping()) {
        let id = auth
            .get(&Value::String("id".into()))
            .and_then(|v| v.as_str())
            .unwrap_or("ANON")
            .to_string();
        let key = auth
            .get(&Value::String("key".into()))
            .and_then(|v| v.as_str())
            .unwrap_or("ANON")
            .to_string();
        let local_only = id == "ANON" && key == "ANON";
        return (id, key, local_only);
    }

    // No cruma config → local-only mode
    ("ANON".into(), "ANON".into(), true)
}

/// Map old bind address to agent listener addr string.
fn listener_addr(ip: &str) -> String {
    match ip {
        "0.0.0.0" => "all".into(),
        "127.0.0.1" | "localhost" => "localhost".into(),
        other => other.into(),
    }
}

/// Derive a config-safe ID from a hostname.
///
/// "py.localtest.me" → "py-localtest-me"
/// "docs.localhost"  → "docs-localhost"
fn sanitize_id(hostname: &str) -> String {
    hostname
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Helper: create a YAML string key.
fn y_str(s: &str) -> Value {
    Value::String(s.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cruma_config_none() {
        let v4: Value = serde_yaml::from_str("version: V4").unwrap();
        let (id, secret, local) = parse_cruma_config(&v4);
        assert_eq!(id, "ANON");
        assert_eq!(secret, "ANON");
        assert!(local);
    }

    #[test]
    fn parse_cruma_config_anon() {
        let v4: Value = serde_yaml::from_str("cruma:\n  anon: true").unwrap();
        let (id, secret, local) = parse_cruma_config(&v4);
        assert_eq!(id, "ANON");
        assert_eq!(secret, "ANON");
        assert!(!local);
    }

    #[test]
    fn parse_cruma_config_auth() {
        let v4: Value =
            serde_yaml::from_str("cruma:\n  auth:\n    id: my-tunnel\n    key: s3cret").unwrap();
        let (id, secret, local) = parse_cruma_config(&v4);
        assert_eq!(id, "my-tunnel");
        assert_eq!(secret, "s3cret");
        assert!(!local);
    }

    #[test]
    fn listener_addr_mapping() {
        assert_eq!(listener_addr("0.0.0.0"), "all");
        assert_eq!(listener_addr("127.0.0.1"), "localhost");
        assert_eq!(listener_addr("10.0.0.1"), "10.0.0.1");
    }

    #[test]
    fn sanitize_id_from_hostname() {
        assert_eq!(sanitize_id("py.localtest.me"), "py-localtest-me");
        assert_eq!(sanitize_id("docs.localhost"), "docs-localhost");
        assert_eq!(sanitize_id("my-app.example.com"), "my-app-example-com");
    }

    #[test]
    fn looks_like_toml_detects_toml() {
        let toml_content = r#"
# comment
version = "V3"
http_port = 8080
"#;
        assert!(looks_like_toml(toml_content));
    }

    #[test]
    fn looks_like_toml_detects_yaml() {
        let yaml_content = r#"
# comment
version: V4
backends:
  my-api:
    type: remote
"#;
        assert!(!looks_like_toml(yaml_content));
    }

    #[test]
    fn looks_like_toml_with_table_header() {
        let toml_content = r#"
[[hosted_process]]
host_name = "test.localhost"
"#;
        assert!(looks_like_toml(toml_content));
    }
}
