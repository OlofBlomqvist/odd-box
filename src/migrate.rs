//! Config migration: reads any old odd-box config (legacy TOML, V1–V3 TOML)
//! and writes the equivalent cruma agent YAML config.
//!
//! Instead of hand-parsing serde_yaml::Value trees, we deserialize into the
//! strongly-typed structs in `crate::configuration`, upgrade through the chain
//! legacy → V1 → V2 → V3 → TunnelCliConfiguration, then serialize.

use anyhow::{Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::configuration::AnyOddBoxConfig;

/// Automatically migrate a legacy config file to the current TOML format.
///
/// 1. Reads and parses the old config (any generation).
/// 2. Upgrades through the chain to `TunnelCliConfiguration`.
/// 3. Writes the migrated TOML next to the original (`.toml` extension).
/// 4. Backs up the original to `<file>.backup<N>`.
///
/// Returns `(config, new_toml_path)` so the caller can continue booting
/// with the migrated config without re-reading from disk.
pub fn auto_migrate(old_path: &str) -> Result<(cruma::config::TunnelCliConfiguration, PathBuf)> {
    let rendered = render_migrated_config(old_path)?;
    let output_path = toml_output_path(old_path);

    let backup_path =
        write_migrated_in_place(Path::new(old_path), &output_path, &rendered.content)?;

    eprintln!();
    eprintln!(
        "Automatically migrated {} config to the current TOML format.",
        rendered.source_format
    );
    eprintln!("  new config : {}", output_path.display());
    eprintln!("  backup     : {}", backup_path.display());
    eprintln!();

    // Re-parse the written TOML so the caller gets an identical result to
    // what `load_config_from_path` would produce.
    let cfg: cruma::config::TunnelCliConfiguration =
        toml::from_str(&rendered.content)
            .map_err(|e| anyhow::anyhow!("BUG: migrated TOML failed to parse: {e}"))?;

    Ok((cfg, output_path))
}

/// Check whether a config file looks like a **legacy** odd-box TOML config
/// (V1/V2/V3) as opposed to a new-format cruma TOML config.
pub fn looks_like_legacy_toml(path: &Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // YAML / YML files are never legacy TOML.
    let is_yaml = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "yaml" | "yml"))
        .unwrap_or(false);
    if is_yaml {
        return false;
    }

    // Look for markers that only appear in legacy odd-box configs.
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Explicit version tag from V1/V2/V3 configs.
        if trimmed.starts_with("version") && trimmed.contains('"') {
            return true;
        }
        // Section headers unique to the legacy schema.
        if trimmed == "[[hosted_process]]"
            || trimmed == "[[remote_target]]"
            || trimmed == "[[dir_server]]"
            || trimmed.starts_with("[[hosted_process.") // e.g. [[hosted_process.backends]]
            || trimmed.starts_with("[[remote_target.")
        {
            return true;
        }
        // Legacy top-level keys that don't exist in the new format.
        if trimmed.starts_with("root_dir")
            || trimmed.starts_with("port_range_start")
            || trimmed.starts_with("hosted_process")
            || trimmed.starts_with("remote_target")
        {
            return true;
        }
    }

    false
}

struct MigrationRender {
    content: String,
    source_format: &'static str,
}

fn render_migrated_config(old_path: &str) -> Result<MigrationRender> {
    let contents = std::fs::read_to_string(old_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {old_path}: {e}"))?;

    // If the file is already valid cruma config, there's nothing to migrate.
    if serde_yaml::from_str::<cruma::config::TunnelCliConfiguration>(&contents).is_ok()
        || toml::from_str::<cruma::config::TunnelCliConfiguration>(&contents).is_ok()
    {
        bail!(
            "The config file '{old_path}' is already in the current format.\n\
             No migration needed — you can use it directly:\n\n\
             \x20 odd-box -c {old_path}"
        );
    }

    // Parse into whatever old generation it is, then upgrade to cruma format.
    let any_config = AnyOddBoxConfig::parse(&contents)
        .map_err(|e| anyhow::anyhow!("Failed to parse config {old_path}: {e}"))?;

    let source_format = match &any_config {
        AnyOddBoxConfig::Legacy(_) => "Legacy TOML",
        AnyOddBoxConfig::V1(_) => "V1 TOML",
        AnyOddBoxConfig::V2(_) => "V2 TOML",
        AnyOddBoxConfig::V3(_) => "V3 TOML",
    };

    let (cruma_cfg, _version) = any_config
        .upgrade_to_cruma()
        .map_err(|e| anyhow::anyhow!("Failed to upgrade config: {e}"))?;

    // Serialize the TunnelCliConfiguration to TOML.
    let toml_str = toml::to_string_pretty(&cruma_cfg)
        .map_err(|e| anyhow::anyhow!("Failed to serialize migrated config: {e}"))?;

    let header = format!(
        "# Migrated from odd-box {source_format} config\n\
         # Review this file carefully before using it.\n\n"
    );
    Ok(MigrationRender {
        content: format!("{header}{toml_str}"),
        source_format,
    })
}

// ─── File I/O helpers ──────────────────────────────────────────────────────

fn write_migrated_in_place(old_path: &Path, output_path: &Path, content: &str) -> Result<PathBuf> {
    if !old_path.exists() {
        bail!(
            "Cannot migrate in-place: '{}' does not exist",
            old_path.display()
        );
    }

    let backup_path = next_backup_path(old_path)?;
    let temp_path = temp_output_path(output_path)?;

    let mut temp_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .map_err(|e| {
            anyhow::anyhow!("Failed to create temp file '{}': {e}", temp_path.display())
        })?;
    temp_file
        .write_all(content.as_bytes())
        .and_then(|_| temp_file.sync_all())
        .map_err(|e| {
            anyhow::anyhow!("Failed to write temp file '{}': {e}", temp_path.display())
        })?;
    drop(temp_file);

    if let Err(err) = std::fs::rename(old_path, &backup_path) {
        let _ = std::fs::remove_file(&temp_path);
        bail!("Failed to create backup '{}': {err}", backup_path.display());
    }

    if let Err(err) = std::fs::rename(&temp_path, output_path) {
        let restore_err = std::fs::rename(&backup_path, old_path).err();
        let _ = std::fs::remove_file(&temp_path);
        match restore_err {
            Some(restore_err) => bail!(
                "Failed to write '{}': {err}. \
                 Also failed to restore original config from '{}': {restore_err}",
                output_path.display(),
                backup_path.display(),
            ),
            None => bail!(
                "Failed to write '{}': {err}. \
                 Original file was restored to '{}'.",
                output_path.display(),
                old_path.display(),
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

    Ok(parent.join(format!(
        ".{file_name}.migrating.{pid}.{now_nanos}.tmp"
    )))
}

/// Compute the TOML output path for a migration.
///
/// If the input already ends in `.toml`, reuse it as-is.
/// Otherwise replace the extension (e.g. `.yaml` → `.toml`) or append `.toml`.
fn toml_output_path(old_path: &str) -> PathBuf {
    let p = Path::new(old_path);
    match p.extension().and_then(|e| e.to_str()) {
        Some("toml") => p.to_path_buf(),
        _ => p.with_extension("toml"),
    }
}



// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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

        let old_path = root.join("odd-box.yaml");
        let output_path = root.join("odd-box.yaml");
        std::fs::write(&old_path, "old-config: true\n").unwrap();

        let backup = write_migrated_in_place(&old_path, &output_path, "new-config: true\n").unwrap();
        assert_eq!(backup, root.join("odd-box.yaml.backup1"));

        let current = std::fs::read_to_string(&output_path).unwrap();
        let previous = std::fs::read_to_string(&backup).unwrap();
        assert_eq!(current, "new-config: true\n");
        assert_eq!(previous, "old-config: true\n");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn write_migrated_in_place_writes_to_new_output_path() {
        let unique = format!(
            "odd-box-migrate-rename-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();

        let old_path = root.join("odd-box.toml");
        let output_path = root.join("odd-box.toml");
        std::fs::write(&old_path, "version = \"V3\"\n").unwrap();

        let backup = write_migrated_in_place(&old_path, &output_path, "backends = []\nfrontends = []\n").unwrap();
        assert_eq!(backup, root.join("odd-box.toml.backup1"));

        // Output path has the migrated content (same path as old, overwritten)
        assert_eq!(std::fs::read_to_string(&output_path).unwrap(), "backends = []\nfrontends = []\n");
        // Backup has the original content
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "version = \"V3\"\n");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn toml_output_path_replaces_yaml_extension() {
        assert_eq!(toml_output_path("odd-box.yaml"), PathBuf::from("odd-box.toml"));
    }

    #[test]
    fn toml_output_path_keeps_toml_extension() {
        assert_eq!(toml_output_path("config.toml"), PathBuf::from("config.toml"));
    }

    #[test]
    fn toml_output_path_appends_toml_when_no_extension() {
        assert_eq!(toml_output_path("myconfig"), PathBuf::from("myconfig.toml"));
        assert_eq!(toml_output_path("odd-box"), PathBuf::from("odd-box.toml"));
    }

    // ── Typed migration tests ──────────────────────────────────────────

    fn migrate_from_str(input: &str) -> Result<cruma::config::TunnelCliConfiguration> {
        let any = AnyOddBoxConfig::parse(input)
            .map_err(|e| anyhow::anyhow!("parse failed: {e}"))?;
        let (cfg, _) = any
            .upgrade_to_cruma()
            .map_err(|e| anyhow::anyhow!("upgrade failed: {e}"))?;

        // Verify the config round-trips through TOML (catches serialization issues early)
        let toml_str = toml::to_string_pretty(&cfg)
            .map_err(|e| anyhow::anyhow!("TOML serialize failed: {e}"))?;
        let _: cruma::config::TunnelCliConfiguration = toml::from_str(&toml_str)
            .map_err(|e| anyhow::anyhow!("TOML round-trip failed: {e}"))?;

        Ok(cfg)
    }

    #[test]
    fn v3_process_produces_process_not_backend() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "api.localhost"
bin = "my-api"
args = ["--port", "$port"]
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.processes.len(), 1);
        assert_eq!(cfg.processes[0].id, "api.localhost");
        assert_eq!(cfg.processes[0].command, "my-api");

        // Should NOT have a backend for the process
        assert!(
            cfg.backends.is_empty(),
            "process should not produce a backend entry"
        );

        // Frontend should reference process_id
        let fe = cfg
            .frontends
            .iter()
            .find(|f| f.hostname.0 == "api.localhost")
            .expect("should have frontend for api.localhost");
        assert_eq!(fe.process_id.as_deref(), Some("api.localhost"));
        assert!(fe.backend_id.is_none());
    }

    #[test]
    fn v3_remote_still_produces_backend() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343

[[remote_target]]
host_name = "cdn.localhost"

[[remote_target.backends]]
address = "cdn.example.com"
port = 443
https = true
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.backends.len(), 1);
        assert_eq!(cfg.backends[0].id, "cdn.localhost");

        let fe = cfg
            .frontends
            .iter()
            .find(|f| f.hostname.0 == "cdn.localhost")
            .unwrap();
        assert!(fe.backend_id.is_some());
        assert!(fe.process_id.is_none());
    }

    #[test]
    fn v3_process_carries_upstream_settings() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "grpc.localhost"
bin = "grpc-server"
args = []
https = true
hints = ["H2CPK"]
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.processes.len(), 1);
        assert!(cfg.processes[0].upstream_tls);
        assert_eq!(
            cfg.processes[0].upstream_protocol,
            cruma::config::UpstreamProtocol::H2PK
        );
    }

    #[test]
    fn v1_full_config_migration_typed() {
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

[[remote_target]]
host_name = "images.local"
target_hostname = "cdn.example.com"
port = 443
https = true
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        // Should have one process and one backend
        assert_eq!(cfg.processes.len(), 1);
        assert_eq!(cfg.backends.len(), 1);

        // Listeners should use All for 0.0.0.0
        assert!(cfg.listeners.iter().any(|l| l.addr
            == cruma::config::ListenerBindAddress::All));

        // Global env should be present
        assert_eq!(
            cfg.global_env.get("GLOBAL_KEY").map(|s| s.as_str()),
            Some("global_val")
        );
    }

    #[test]
    fn v3_capture_subdomains_creates_wildcard_frontend() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "app.localhost"
bin = "my-app"
args = []
capture_subdomains = true
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        // Should have two frontends: the exact and the wildcard
        assert_eq!(cfg.frontends.len(), 2);
        assert!(cfg
            .frontends
            .iter()
            .any(|f| f.hostname.0 == "app.localhost"));
        assert!(cfg
            .frontends
            .iter()
            .any(|f| f.hostname.0 == "*.app.localhost"));
    }

    #[test]
    fn v3_dir_server_produces_backend() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343

[[dir_server]]
host_name = "docs.localhost"
dir = "/var/docs"
enable_directory_browsing = true
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.backends.len(), 1);
        assert_eq!(cfg.backends[0].id, "docs.localhost");
        assert_eq!(
            cfg.backends[0].kind,
            cruma::config::TargetKind::LocalDirectory
        );
        assert_eq!(cfg.backends[0].destination, "/var/docs");
        assert_eq!(cfg.backends[0].allow_directory_indexing, Some(true));
    }

    #[test]
    fn serialization_round_trips_toml() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "api.localhost"
bin = "my-api"
args = []
"#;
        let cfg = migrate_from_str(toml_input).unwrap();
        let toml_out = toml::to_string_pretty(&cfg).unwrap();

        // Should be valid TOML that parses back
        let parsed: cruma::config::TunnelCliConfiguration =
            toml::from_str(&toml_out).unwrap();
        assert_eq!(parsed.processes.len(), 1);
        assert_eq!(parsed.listeners.len(), 2);
        assert_eq!(parsed.frontends.len(), 1);
    }

    #[test]
    fn v3_root_dir_preserved_in_process_bin_and_dir() {
        let toml_input = r#"
version = "V3"
root_dir = "/srv/odd-box"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "app.localhost"
bin = "$root_dir/bin/my-server"
dir = "$root_dir/apps/myapp"
args = ["--config", "$root_dir/etc/app.toml"]
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        // root_dir is passed through to cruma so it can expand at load time
        assert_eq!(cfg.root_dir.as_deref(), Some("/srv/odd-box"));
        assert_eq!(cfg.processes.len(), 1);
        let proc = &cfg.processes[0];
        assert_eq!(proc.command, "$root_dir/bin/my-server");
        assert_eq!(
            proc.working_directory.as_deref(),
            Some(std::path::Path::new("$root_dir/apps/myapp"))
        );
        assert_eq!(proc.args, vec!["--config", "$root_dir/etc/app.toml"]);
    }

    #[test]
    fn v3_root_dir_preserved_in_dir_server() {
        let toml_input = r#"
version = "V3"
root_dir = "/srv/odd-box"
http_port = 8080
tls_port = 4343

[[dir_server]]
host_name = "docs.localhost"
dir = "$root_dir/static/docs"
enable_directory_browsing = true
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.root_dir.as_deref(), Some("/srv/odd-box"));
        assert_eq!(cfg.backends.len(), 1);
        assert_eq!(cfg.backends[0].destination, "$root_dir/static/docs");
    }

    #[test]
    fn v3_port_left_for_cruma_to_expand() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "svc.localhost"
bin = "my-svc"
args = ["--port", "$port", "--bind", "0.0.0.0:$port"]
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.processes.len(), 1);
        let proc = &cfg.processes[0];
        // $port is handled by cruma at runtime, so it must NOT be expanded here
        assert_eq!(proc.args, vec!["--port", "$port", "--bind", "0.0.0.0:$port"]);
    }

    #[test]
    fn v3_root_dir_preserved_in_env_vars() {
        let toml_input = r#"
version = "V3"
root_dir = "/opt/apps"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "env.localhost"
bin = "server"
args = []
env_vars = [
    { key = "DATA_DIR", value = "$root_dir/data" },
    { key = "LOG_FILE", value = "$root_dir/logs/app.log" },
]
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.root_dir.as_deref(), Some("/opt/apps"));
        assert_eq!(cfg.processes.len(), 1);
        let proc = &cfg.processes[0];
        assert_eq!(proc.env.get("DATA_DIR").map(|s| s.as_str()), Some("$root_dir/data"));
        assert_eq!(proc.env.get("LOG_FILE").map(|s| s.as_str()), Some("$root_dir/logs/app.log"));
    }

    #[test]
    fn v3_root_dir_preserved_in_global_env_vars() {
        let toml_input = r#"
version = "V3"
root_dir = "/srv"
http_port = 8080
tls_port = 4343
port_range_start = 4200
env_vars = [
    { key = "BASE", value = "$root_dir/shared" },
]

[[hosted_process]]
host_name = "g.localhost"
bin = "server"
args = []
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.root_dir.as_deref(), Some("/srv"));
        // $root_dir is preserved for cruma to expand at load time
        assert_eq!(cfg.global_env.get("BASE").map(|s| s.as_str()), Some("$root_dir/shared"));
        // Global vars must NOT be duplicated into individual process envs
        assert!(!cfg.processes[0].env.contains_key("BASE"));
    }

    #[test]
    fn v3_no_root_dir_means_no_root_dir_field() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "rel.localhost"
bin = "$root_dir/bin/server"
dir = "$root_dir/work"
args = []
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        // When root_dir is unset in V3, cruma gets None and falls back to cwd
        assert_eq!(cfg.root_dir, None);
        let proc = &cfg.processes[0];
        assert_eq!(proc.command, "$root_dir/bin/server");
        assert_eq!(
            proc.working_directory.as_deref(),
            Some(std::path::Path::new("$root_dir/work"))
        );
    }

    #[test]
    fn v3_dollar_port_preserved_even_with_explicit_port() {
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343
port_range_start = 4200

[[hosted_process]]
host_name = "fixed.localhost"
bin = "server"
port = 9999
args = ["--listen", "$port"]
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        let proc = &cfg.processes[0];
        // $port is resolved by cruma at runtime, not during migration
        assert_eq!(proc.args, vec!["--listen", "$port"]);
    }

    #[test]
    fn v3_combined_vars_in_single_string() {
        let toml_input = r#"
version = "V3"
root_dir = "/srv"
http_port = 8080
tls_port = 4343
port_range_start = 5000

[[hosted_process]]
host_name = "combo.localhost"
bin = "$root_dir/bin/app"
args = ["--dir", "$root_dir/data", "--port", "$port"]
env_vars = [
    { key = "LISTEN", value = "0.0.0.0:$port" },
    { key = "STORAGE", value = "$root_dir/storage" },
]
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        assert_eq!(cfg.root_dir.as_deref(), Some("/srv"));
        let proc = &cfg.processes[0];
        // Both $root_dir and $port are preserved for cruma to expand
        assert_eq!(proc.command, "$root_dir/bin/app");
        assert_eq!(proc.args, vec!["--dir", "$root_dir/data", "--port", "$port"]);
        assert_eq!(proc.env.get("LISTEN").map(|s| s.as_str()), Some("0.0.0.0:$port"));
        assert_eq!(proc.env.get("STORAGE").map(|s| s.as_str()), Some("$root_dir/storage"));
    }

    #[test]
    fn init_toml_template_parses_as_valid_cruma_config() {
        let toml_input = format!(
            r#"# odd-box configuration
# Generated by {} v{}
# See https://github.com/OlofBlomqvist/odd-box for documentation

backends = []
frontends = []

[[listeners]]
port = 8080
addr = "localhost"
kind = "http"
tls = false

[[listeners]]
port = 4343
addr = "localhost"
kind = "https"
tls = true
"#,
            crate::NAME,
            crate::VERSION,
        );
        let cfg: cruma::config::TunnelCliConfiguration =
            toml::from_str(&toml_input).expect("init template must parse as valid TOML config");

        assert_eq!(cfg.listeners.len(), 2);
        assert!(cfg.backends.is_empty());
        assert!(cfg.frontends.is_empty());
        assert!(cfg.processes.is_empty());
        // tunnel_id / tunnel_secret should get serde defaults
        assert_eq!(cfg.tunnel_id, "ANON");
        assert_eq!(cfg.tunnel_secret, "ANON");
    }

    #[test]
    fn v3_cfg_dir_preserved_in_migrated_output() {
        // $cfg_dir must survive V3→cruma conversion unchanged so that
        // cruma can expand it at process-spawn time.
        let toml_input = r#"
version = "V3"
http_port = 8080
tls_port = 4343
port_range_start = 4200
env_vars = [
    { key = "GLOBAL_VAR", value = "$cfg_dir/shared" },
]

[[hosted_process]]
host_name = "app.localhost"
bin = "$cfg_dir/bin/server"
dir = "$cfg_dir/work"
args = ["--config", "$cfg_dir/etc/app.toml"]
env_vars = [
    { key = "DATA_DIR", value = "$cfg_dir/data" },
]

[[dir_server]]
host_name = "docs.localhost"
dir = "$cfg_dir/static/docs"
"#;
        let cfg = migrate_from_str(toml_input).unwrap();

        let proc = &cfg.processes[0];
        assert_eq!(proc.command, "$cfg_dir/bin/server",
            "bin should preserve $cfg_dir");
        assert_eq!(proc.working_directory.as_ref().map(|p| p.to_str().unwrap()),
            Some("$cfg_dir/work"),
            "dir should preserve $cfg_dir");
        assert_eq!(proc.args, vec!["--config", "$cfg_dir/etc/app.toml"],
            "args should preserve $cfg_dir");
        assert_eq!(proc.env.get("DATA_DIR").map(|s| s.as_str()),
            Some("$cfg_dir/data"),
            "per-process env should preserve $cfg_dir");
        assert_eq!(cfg.global_env.get("GLOBAL_VAR").map(|s| s.as_str()),
            Some("$cfg_dir/shared"),
            "global env should preserve $cfg_dir");

        let dir_backend = cfg.backends.iter().find(|b| b.id == "docs.localhost").unwrap();
        assert_eq!(dir_backend.destination, "$cfg_dir/static/docs",
            "dir_server destination should preserve $cfg_dir");
    }
}
