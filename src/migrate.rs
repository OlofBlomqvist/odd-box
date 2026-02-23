//! One-time migration tool: reads an old odd-box config (V3 TOML or V4 YAML)
//! and writes the equivalent cruma agent YAML config to stdout.

use anyhow::{Result, bail};
use serde_yaml::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Main entry point for config migration.
///
/// Detects the input format (TOML vs YAML) automatically based on file
/// extension and content, then delegates to the appropriate parser.
pub fn migrate_v4_config(old_path: &str) -> Result<()> {
    let rendered = render_migrated_config(old_path)?;
    print!("{}", rendered.yaml);
    print_stdout_guidance(old_path, rendered.source_format);
    Ok(())
}

/// Interactive migration workflow for terminal usage.
///
/// Prompts the user to confirm in-place migration. On confirmation, writes the
/// migrated config back to the same path and moves the original file to
/// `<file>.backupN`.
pub fn migrate_v4_config_with_in_place_prompt(old_path: &str) -> Result<()> {
    let rendered = render_migrated_config(old_path)?;
    if confirm_in_place_write(old_path, rendered.source_format)? {
        let backup_path = write_migrated_in_place(Path::new(old_path), &rendered.yaml)?;
        eprintln!();
        eprintln!("Migration complete (from {}).", rendered.source_format);
        eprintln!("Wrote migrated config to: {old_path}");
        eprintln!("Previous config backup: {}", backup_path.display());
    } else {
        print!("{}", rendered.yaml);
        print_stdout_guidance(old_path, rendered.source_format);
    }
    Ok(())
}

struct MigrationRender {
    yaml: String,
    source_format: &'static str,
}

fn render_migrated_config(old_path: &str) -> Result<MigrationRender> {
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

fn print_stdout_guidance(old_path: &str, source_format: &str) {
    eprintln!();
    eprintln!("Migration complete (from {source_format}).");
    eprintln!("Review the output above, then save it:");
    eprintln!("  odd-box --migrate {old_path} > odd-box.yaml");
}

fn confirm_in_place_write(old_path: &str, source_format: &str) -> Result<bool> {
    eprintln!();
    eprintln!("Migration ready (from {source_format}).");
    eprintln!("Write migrated config in-place?");
    eprintln!("  target : {old_path}");
    eprintln!("  action : move current file to backup[n], then write migrated YAML to target");
    eprint!("Proceed? [y/N]: ");
    std::io::stderr().flush()?;

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| anyhow::anyhow!("Failed to read confirmation input: {e}"))?;

    let answer = input.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

fn write_migrated_in_place(target: &Path, yaml: &str) -> Result<PathBuf> {
    if !target.exists() {
        bail!(
            "Cannot migrate in-place: '{}' does not exist",
            target.display()
        );
    }

    let backup_path = next_backup_path(target)?;
    let temp_path = temp_output_path(target)?;

    let mut temp_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|e| {
            anyhow::anyhow!("Failed to create temp file '{}': {e}", temp_path.display())
        })?;
    temp_file
        .write_all(yaml.as_bytes())
        .and_then(|_| temp_file.sync_all())
        .map_err(|e| anyhow::anyhow!("Failed to write temp file '{}': {e}", temp_path.display()))?;
    drop(temp_file);

    if let Err(err) = std::fs::rename(target, &backup_path) {
        let _ = std::fs::remove_file(&temp_path);
        bail!("Failed to create backup '{}': {err}", backup_path.display());
    }

    if let Err(err) = std::fs::rename(&temp_path, target) {
        let restore_err = std::fs::rename(&backup_path, target).err();
        let _ = std::fs::remove_file(&temp_path);
        match restore_err {
            Some(restore_err) => bail!(
                "Failed to replace '{}' with migrated config: {err}. \
                 Also failed to restore original config from '{}': {restore_err}",
                target.display(),
                backup_path.display(),
            ),
            None => bail!(
                "Failed to replace '{}' with migrated config: {err}. \
                 Original file was restored.",
                target.display(),
            ),
        }
    }

    Ok(backup_path)
}

fn next_backup_path(target: &Path) -> Result<PathBuf> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid target path '{}'", target.display()))?;

    for idx in 1..=10_000u32 {
        let candidate = parent.join(format!("{file_name}.backup{idx}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    bail!(
        "Could not allocate backup name for '{}' (too many existing backups)",
        target.display()
    );
}

fn temp_output_path(target: &Path) -> Result<PathBuf> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid target path '{}'", target.display()))?;
    let pid = std::process::id();
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("System clock error while creating temp path: {e}"))?
        .as_nanos();

    Ok(parent.join(format!(".{file_name}.migrating.{pid}.{now_nanos}.tmp")))
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
fn migrate_v3_toml(old_path: &str, contents: &str) -> Result<MigrationRender> {
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
    // V1 format: env_vars = [{ key = "k", value = "v" }, ...]
    // V3 format: [env_vars]  k = "v"
    if let Some(env_val) = table.get("env_vars") {
        if let Some(env_table) = env_val.as_table() {
            // V3 table format: { k = "v", ... }
            for (k, v) in env_table {
                if let Some(val) = v.as_str() {
                    global_env.insert(y_str(k), Value::String(val.to_string()));
                }
            }
        } else if let Some(env_arr) = env_val.as_array() {
            // V1 array-of-pairs format: [{ key = "k", value = "v" }, ...]
            for pair in env_arr {
                if let Some(pair_table) = pair.as_table() {
                    let key = pair_table.get("key").and_then(|v| v.as_str());
                    let val = pair_table.get("value").and_then(|v| v.as_str());
                    if let (Some(k), Some(v)) = (key, val) {
                        global_env.insert(y_str(k), Value::String(v.to_string()));
                    }
                }
            }
        }
    }

    // ── Listeners ──────────────────────────────────────────────────────
    let mut listeners = Vec::<Value>::new();

    let mut http_listener = serde_yaml::Mapping::new();
    http_listener.insert(y_str("port"), Value::Number(http_port.into()));
    http_listener.insert(y_str("addr"), Value::String(listener_addr(ip)));
    http_listener.insert(y_str("kind"), Value::String("http".into()));
    listeners.push(Value::Mapping(http_listener));

    let mut https_listener = serde_yaml::Mapping::new();
    https_listener.insert(y_str("port"), Value::Number(tls_port.into()));
    https_listener.insert(y_str("addr"), Value::String(listener_addr(ip)));
    https_listener.insert(y_str("kind"), Value::String("https".into()));
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
            // Supports both V1 array-of-pairs and V3 table formats.
            let mut proc_env = global_env.clone();
            if let Some(env_val) = entry.get("env_vars").or_else(|| entry.get("env")) {
                if let Some(env_table) = env_val.as_table() {
                    for (k, v) in env_table {
                        if let Some(val) = v.as_str() {
                            proc_env.insert(y_str(k), Value::String(val.to_string()));
                        }
                    }
                } else if let Some(env_arr) = env_val.as_array() {
                    for pair in env_arr {
                        if let Some(pair_table) = pair.as_table() {
                            let key = pair_table.get("key").and_then(|v| v.as_str());
                            let val = pair_table.get("value").and_then(|v| v.as_str());
                            if let (Some(k), Some(v)) = (key, val) {
                                proc_env.insert(y_str(k), Value::String(v.to_string()));
                            }
                        }
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

            // Check if the hosted process uses HTTPS upstream
            let use_https = entry
                .get("https")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if use_https {
                process.insert(y_str("upstream_tls"), Value::Bool(true));
            }

            processes.push(Value::Mapping(process));

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

            // V1 format: target_hostname + port + https (flat fields)
            // V3 format: backends = [{ address, port, https }]
            let remote_backends = entry
                .get("backends")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let (address, port, use_https) = if let Some(first) =
                remote_backends.first().and_then(|b| b.as_table())
            {
                // V3 nested backends array
                let addr = first
                    .get("address")
                    .and_then(|v| v.as_str())
                    .unwrap_or("localhost");
                let p = first.get("port").and_then(|v| v.as_integer()).unwrap_or(80) as u16;
                let https = first
                    .get("https")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                (addr.to_string(), p, https)
            } else if let Some(target_host) = entry.get("target_hostname").and_then(|v| v.as_str())
            {
                // V1 flat fields
                let p = entry.get("port").and_then(|v| v.as_integer()).unwrap_or(80) as u16;
                let https = entry
                    .get("https")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                (target_host.to_string(), p, https)
            } else {
                eprintln!(
                    "Warning: remote_target '{host_name}' has no backends array \
                         and no target_hostname, skipping backend creation"
                );
                // Still create the frontend so we don't silently drop it,
                // but mark the issue.
                let mut frontend = serde_yaml::Mapping::new();
                frontend.insert(y_str("hostname"), Value::String(host_name));
                frontend.insert(y_str("backend_id"), Value::String(backend_id));
                frontends.push(Value::Mapping(frontend));
                continue;
            };

            let kind = if use_https { "https" } else { "http" };

            let mut backend = serde_yaml::Mapping::new();
            backend.insert(y_str("id"), Value::String(backend_id.clone()));
            backend.insert(y_str("kind"), Value::String(kind.into()));
            backend.insert(
                y_str("destination"),
                Value::String(format!("{address}:{port}")),
            );
            backends.push(Value::Mapping(backend));

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
    if table.contains_key("root_dir") {
        eprintln!(
            "Note: $root_dir / $cfg_dir variable expansion is not supported in the new \
             format. You may need to replace $root_dir and $cfg_dir with absolute paths \
             in the generated config."
        );
    }
    for dropped in ["admin_api_port", "log_level", "default_log_format", "alpn"] {
        if table.contains_key(dropped) {
            eprintln!("Note: '{dropped}' is no longer used and has been dropped.");
        }
    }

    // ── Emit output ────────────────────────────────────────────────────
    emit_output(
        &listeners,
        &backends,
        &processes,
        &frontends,
        &global_env,
        // V3 TOML had no cruma tunnel config — default to no `cruma` listener
        "ANON",
        "ANON",
        true,
        "V3 TOML",
        old_path,
    )
}

// ─── V4 YAML migration ────────────────────────────────────────────────────

/// Migrate a V4 YAML config to the agent YAML format.
fn migrate_v4_yaml(old_path: &str, contents: &str) -> Result<MigrationRender> {
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
    let mut process_ids = std::collections::HashSet::<String>::new();

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

                    // Carry over upstream protocol hints (H2, H2CPK, etc.)
                    let hints: Vec<String> = def
                        .get(&Value::String("hints".into()))
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
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
                        process.insert(
                            y_str("upstream_protocol"),
                            Value::String(proto.to_string()),
                        );
                    }

                    // Carry over HTTPS upstream flag.
                    let use_https = def
                        .get(&Value::String("https".into()))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if use_https {
                        process.insert(y_str("upstream_tls"), Value::Bool(true));
                    }

                    processes.push(Value::Mapping(process));

                    // Track this ID as a process so that frontends
                    // referencing it emit `process_id` instead of
                    // `backend_id`.
                    process_ids.insert(id);
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
        listener.insert(y_str("kind"), Value::String("http".into()));
        listeners.push(Value::Mapping(listener));

        parse_yaml_routes(http, &mut frontends, &process_ids);
    }

    // ── Parse frontends (HTTPS) ────────────────────────────────────────
    if let Some(https) = v4.get("frontends").and_then(|f| f.get("https")) {
        let port = https.get("port").and_then(|p| p.as_u64()).unwrap_or(4343) as u16;
        let addr = v4.get("ip").and_then(|v| v.as_str()).unwrap_or("127.0.0.1");

        let mut listener = serde_yaml::Mapping::new();
        listener.insert(y_str("port"), Value::Number(port.into()));
        listener.insert(y_str("addr"), Value::String(listener_addr(addr)));
        listener.insert(y_str("kind"), Value::String("https".into()));
        listeners.push(Value::Mapping(listener));

        // "inherit" means the HTTPS routes are the same as HTTP routes
        let routes_val = https.get("routes");
        let is_inherit = routes_val
            .and_then(|v| v.as_str())
            .map(|s| s == "inherit")
            .unwrap_or(false);

        if !is_inherit {
            parse_yaml_routes(https, &mut frontends, &process_ids);
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
    source_format: &'static str,
    _old_path: &str,
) -> Result<MigrationRender> {
    let mut output = serde_yaml::Mapping::new();
    let mut listeners_out = listeners.to_vec();

    if !local_only && !listeners_have_kind(&listeners_out, "cruma") {
        let mut cruma_listener = serde_yaml::Mapping::new();
        cruma_listener.insert(y_str("kind"), Value::String("cruma".into()));
        listeners_out.push(Value::Mapping(cruma_listener));
    }

    output.insert(y_str("tunnel_id"), Value::String(tunnel_id.to_string()));
    output.insert(
        y_str("tunnel_secret"),
        Value::String(tunnel_secret.to_string()),
    );

    // In explicit listener-kind configs, `kind: cruma` is the source of truth.
    // Keep `local_only` only for legacy implicit-listener outputs.
    if !listeners_use_explicit_kinds(&listeners_out) {
        output.insert(y_str("local_only"), Value::Bool(local_only));
    }

    if !listeners_out.is_empty() {
        output.insert(y_str("listeners"), Value::Sequence(listeners_out));
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
    Ok(MigrationRender {
        yaml: format!("{header}{yaml}"),
        source_format,
    })
}

/// Parse route mappings from a V4 YAML frontends.http or frontends.https block.
fn parse_yaml_routes(
    section: &Value,
    frontends: &mut Vec<Value>,
    process_ids: &std::collections::HashSet<String>,
) {
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
        frontend.insert(y_str("hostname"), Value::String(hostname.clone()));
        if process_ids.contains(&backend_id) {
            frontend.insert(y_str("process_id"), Value::String(backend_id));
        } else {
            frontend.insert(y_str("backend_id"), Value::String(backend_id));
        }

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

    // No cruma config -> no `cruma` listener
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

fn listeners_have_kind(listeners: &[Value], kind: &str) -> bool {
    listeners.iter().any(|value| {
        value
            .as_mapping()
            .and_then(|mapping| mapping.get(&y_str("kind")))
            .and_then(|v| v.as_str())
            .is_some_and(|v| v == kind)
    })
}

fn listeners_use_explicit_kinds(listeners: &[Value]) -> bool {
    listeners.iter().any(|value| {
        value
            .as_mapping()
            .is_some_and(|mapping| mapping.contains_key(&y_str("kind")))
    })
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

    #[test]
    fn v1_remote_target_with_target_hostname() {
        let toml_input = r#"
version = "V1"
http_port = 8080
tls_port = 4343

[[remote_target]]
host_name = "images.local"
target_hostname = "images.dev.bookvisit.com"
port = 443
https = true
"#;
        let toml_val: toml::Value = toml_input.parse().unwrap();
        let table = toml_val.as_table().unwrap();

        let remotes = table.get("remote_target").unwrap().as_array().unwrap();
        let entry = remotes[0].as_table().unwrap();

        // Verify the V1 fields are present
        assert_eq!(
            entry.get("target_hostname").unwrap().as_str().unwrap(),
            "images.dev.bookvisit.com"
        );
        assert_eq!(entry.get("port").unwrap().as_integer().unwrap(), 443);
        assert!(entry.get("https").unwrap().as_bool().unwrap());

        // The old code would look for entry.get("backends") which is None
        assert!(entry.get("backends").is_none());

        // Verify sanitize_id produces the expected backend_id
        let host_name = entry.get("host_name").unwrap().as_str().unwrap();
        assert_eq!(sanitize_id(host_name), "images-local");

        // Now run the full migration and verify it doesn't error
        let result = migrate_v3_toml("<test>", toml_input);
        assert!(result.is_ok(), "migration failed: {:?}", result.err());
    }

    #[test]
    fn v1_env_vars_array_of_pairs() {
        let toml_input = r#"
version = "V1"
http_port = 8080
tls_port = 4343
env_vars = [
    { key = "MY_KEY", value = "my_value" },
    { key = "OTHER", value = "other_val" },
]

[[hosted_process]]
host_name = "app.local"
bin = "myapp"
args = []
env_vars = [
    { key = "PORT", value = "5000" },
]
"#;
        let toml_val: toml::Value = toml_input.parse().unwrap();
        let table = toml_val.as_table().unwrap();

        // Global env_vars should be an array, not a table
        let env_val = table.get("env_vars").unwrap();
        assert!(
            env_val.as_array().is_some(),
            "env_vars should be an array in V1"
        );
        assert!(env_val.as_table().is_none());

        // Per-process env_vars should also be an array
        let hosted = table.get("hosted_process").unwrap().as_array().unwrap();
        let proc_env = hosted[0].as_table().unwrap().get("env_vars").unwrap();
        assert!(proc_env.as_array().is_some());

        // Run the full migration
        let result = migrate_v3_toml("<test>", toml_input);
        assert!(result.is_ok(), "migration failed: {:?}", result.err());
    }

    #[test]
    fn v1_hosted_process_with_https() {
        let toml_input = r#"
version = "V1"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "payment.local"
bin = "PaymentService"
https = true
args = []
env_vars = []
"#;
        // Run the full migration — should not error
        let result = migrate_v3_toml("<test>", toml_input);
        assert!(result.is_ok(), "migration failed: {:?}", result.err());
    }

    #[test]
    fn v1_full_config_migration() {
        // A realistic V1 config with all the tricky bits:
        // - array-of-pairs env_vars (global and per-process)
        // - remote_target with target_hostname
        // - hosted_process with https flag
        // - hosted_process with explicit port
        let toml_input = r#"
version = "V1"
http_port = 8080
tls_port = 4343
ip = "0.0.0.0"
port_range_start = 4200
auto_start = true
env_vars = [
    { key = "GLOBAL_KEY", value = "global_val" },
]

[[hosted_process]]
host_name = "api.local"
dir = "/app/api"
bin = "api-server"
args = ["--port", "$port"]
env_vars = [
    { key = "DB_HOST", value = "localhost" },
]

[[hosted_process]]
host_name = "payment.local"
dir = "/app/payment"
bin = "payment-svc"
https = true
args = []
port = 5003
env_vars = [
    { key = "PORT", value = "5003" },
]

[[remote_target]]
host_name = "images.local"
target_hostname = "cdn.example.com"
port = 443
https = true

[[dir_server]]
host_name = "docs.local"
dir = "/var/docs"
"#;
        let result = migrate_v3_toml("<test>", toml_input);
        assert!(result.is_ok(), "migration failed: {:?}", result.err());
    }

    #[test]
    fn next_backup_path_rotates_suffix() {
        let unique = format!(
            "odd-box-migrate-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();

        let target = root.join("odd-box.yaml");
        std::fs::write(&target, "version: V4\n").unwrap();
        std::fs::write(root.join("odd-box.yaml.backup1"), "a").unwrap();
        std::fs::write(root.join("odd-box.yaml.backup2"), "b").unwrap();

        let next = next_backup_path(&target).unwrap();
        assert_eq!(next, root.join("odd-box.yaml.backup3"));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn write_migrated_in_place_creates_backup_and_replaces_target() {
        let unique = format!(
            "odd-box-migrate-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();

        let target = root.join("odd-box.yaml");
        std::fs::write(&target, "old-config: true\n").unwrap();

        let backup = write_migrated_in_place(&target, "new-config: true\n").unwrap();
        assert_eq!(backup, root.join("odd-box.yaml.backup1"));

        let current = std::fs::read_to_string(&target).unwrap();
        let previous = std::fs::read_to_string(&backup).unwrap();
        assert_eq!(current, "new-config: true\n");
        assert_eq!(previous, "old-config: true\n");

        std::fs::remove_dir_all(&root).unwrap();
    }
}


#[cfg(test)]
mod process_migration_tests {
    use super::*;

    /// V3 TOML: hosted processes should NOT produce a backend entry.
    #[test]
    fn v3_process_produces_no_backend() {
        let toml_input = "version = \"V3\"\nhttp_port = 8080\ntls_port = 4343\nport_range_start = 4200\n\n[[hosted_process]]\nhost_name = \"api.localhost\"\nbin = \"my-api\"\nargs = [\"--port\", \"$port\"]\n";
        let result = migrate_v3_toml("<test>", toml_input).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result.yaml).unwrap();

        let processes = parsed.get("processes").and_then(|p| p.as_sequence()).unwrap();
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].get("id").unwrap().as_str().unwrap(), "api-localhost");

        // Should NOT have a backend for the process
        let backends = parsed.get("backends").and_then(|b| b.as_sequence());
        assert!(backends.is_none() || backends.unwrap().is_empty(),
            "process should not produce a backend entry");

        // Frontend should use process_id
        let frontends = parsed.get("frontends").and_then(|f| f.as_sequence()).unwrap();
        assert_eq!(frontends.len(), 1);
        assert!(frontends[0].get("process_id").is_some(), "frontend should reference process_id");
        assert!(frontends[0].get("backend_id").is_none(), "frontend should NOT have backend_id");
    }

    /// V3 TOML: remote targets should still produce backends normally.
    #[test]
    fn v3_remote_still_produces_backend() {
        let toml_input = "version = \"V3\"\nhttp_port = 8080\ntls_port = 4343\n\n[[remote_target]]\nhost_name = \"cdn.localhost\"\ntarget_hostname = \"cdn.example.com\"\nport = 443\nhttps = true\n";
        let result = migrate_v3_toml("<test>", toml_input).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result.yaml).unwrap();

        let backends = parsed.get("backends").and_then(|b| b.as_sequence()).unwrap();
        assert_eq!(backends.len(), 1);

        let frontends = parsed.get("frontends").and_then(|f| f.as_sequence()).unwrap();
        assert!(frontends[0].get("backend_id").is_some());
        assert!(frontends[0].get("process_id").is_none());
    }

    /// V3 TOML: upstream_tls and upstream_protocol are carried over.
    #[test]
    fn v3_process_carries_upstream_settings() {
        let toml_input = "version = \"V3\"\nhttp_port = 8080\ntls_port = 4343\nport_range_start = 4200\n\n[[hosted_process]]\nhost_name = \"grpc.localhost\"\nbin = \"grpc-server\"\nargs = []\nhttps = true\nhints = [\"H2CPK\"]\n";
        let result = migrate_v3_toml("<test>", toml_input).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result.yaml).unwrap();

        let processes = parsed.get("processes").and_then(|p| p.as_sequence()).unwrap();
        assert_eq!(processes[0].get("upstream_tls").unwrap().as_bool().unwrap(), true);
        assert_eq!(processes[0].get("upstream_protocol").unwrap().as_str().unwrap(), "H2PK");
    }

    /// V4 YAML: process-type backends should NOT produce a backend entry.
    #[test]
    fn v4_process_produces_no_backend_and_frontend_uses_process_id() {
        let yaml_input = "version: V4\nport_range_start: 4200\nbackends:\n  my-api:\n    type: process\n    bin: my-api-server\n    dir: /app\nfrontends:\n  http:\n    port: 8080\n    routes:\n      api.localhost: my-api\n";
        let result = migrate_v4_yaml("<test>", yaml_input).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result.yaml).unwrap();

        let processes = parsed.get("processes").and_then(|p| p.as_sequence()).unwrap();
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].get("id").unwrap().as_str().unwrap(), "my-api");

        // Should NOT have a backend for the process
        let backends = parsed.get("backends").and_then(|b| b.as_sequence());
        assert!(backends.is_none() || backends.unwrap().is_empty(),
            "process should not produce a backend entry");

        // Frontend should use process_id
        let frontends = parsed.get("frontends").and_then(|f| f.as_sequence()).unwrap();
        let api_fe = frontends.iter().find(|f| {
            f.get("hostname").and_then(|h| h.as_str()).map(|h| h == "api.localhost").unwrap_or(false)
        }).expect("should have frontend for api.localhost");
        assert_eq!(api_fe.get("process_id").unwrap().as_str().unwrap(), "my-api");
        assert!(api_fe.get("backend_id").is_none(), "should NOT have backend_id");
    }

    /// V4 YAML: remote-type backends should still produce backends.
    #[test]
    fn v4_remote_still_produces_backend() {
        let yaml_input = "version: V4\nbackends:\n  cdn:\n    type: remote\n    endpoints: [\"cdn.example.com:443\"]\n    https: true\nfrontends:\n  http:\n    port: 8080\n    routes:\n      cdn.localhost: cdn\n";
        let result = migrate_v4_yaml("<test>", yaml_input).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result.yaml).unwrap();

        let backends = parsed.get("backends").and_then(|b| b.as_sequence()).unwrap();
        assert_eq!(backends.len(), 1);
        assert_eq!(backends[0].get("id").unwrap().as_str().unwrap(), "cdn");

        let frontends = parsed.get("frontends").and_then(|f| f.as_sequence()).unwrap();
        let cdn_fe = frontends.iter().find(|f| {
            f.get("hostname").and_then(|h| h.as_str()).map(|h| h == "cdn.localhost").unwrap_or(false)
        }).unwrap();
        assert!(cdn_fe.get("backend_id").is_some());
        assert!(cdn_fe.get("process_id").is_none());
    }

    /// V4 YAML: upstream_tls and upstream_protocol should be carried over.
    #[test]
    fn v4_process_carries_upstream_settings() {
        let yaml_input = "version: V4\nport_range_start: 4200\nbackends:\n  grpc-svc:\n    type: process\n    bin: grpc-server\n    https: true\n    hints: [\"H2CPK\"]\nfrontends:\n  http:\n    port: 8080\n    routes:\n      grpc.localhost: grpc-svc\n";
        let result = migrate_v4_yaml("<test>", yaml_input).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result.yaml).unwrap();

        let processes = parsed.get("processes").and_then(|p| p.as_sequence()).unwrap();
        assert_eq!(processes[0].get("upstream_tls").unwrap().as_bool().unwrap(), true);
        assert_eq!(processes[0].get("upstream_protocol").unwrap().as_str().unwrap(), "H2PK");
    }

    /// V4 YAML: mixed config with both processes and remotes.
    #[test]
    fn v4_mixed_process_and_remote() {
        let yaml_input = "version: V4\nport_range_start: 4200\nbackends:\n  my-app:\n    type: process\n    bin: app-server\n  my-cdn:\n    type: remote\n    endpoints: [\"cdn.example.com:443\"]\n    https: true\nfrontends:\n  http:\n    port: 8080\n    routes:\n      app.localhost: my-app\n      cdn.localhost: my-cdn\n";
        let result = migrate_v4_yaml("<test>", yaml_input).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result.yaml).unwrap();

        let processes = parsed.get("processes").and_then(|p| p.as_sequence()).unwrap();
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].get("id").unwrap().as_str().unwrap(), "my-app");

        let backends = parsed.get("backends").and_then(|b| b.as_sequence()).unwrap();
        assert_eq!(backends.len(), 1);
        assert_eq!(backends[0].get("id").unwrap().as_str().unwrap(), "my-cdn");

        let frontends = parsed.get("frontends").and_then(|f| f.as_sequence()).unwrap();
        let app_fe = frontends.iter().find(|f| f.get("hostname").unwrap().as_str().unwrap() == "app.localhost").unwrap();
        assert!(app_fe.get("process_id").is_some());
        assert!(app_fe.get("backend_id").is_none());

        let cdn_fe = frontends.iter().find(|f| f.get("hostname").unwrap().as_str().unwrap() == "cdn.localhost").unwrap();
        assert!(cdn_fe.get("backend_id").is_some());
        assert!(cdn_fe.get("process_id").is_none());
    }
}
