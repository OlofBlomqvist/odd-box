use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ── Data model ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ProfilesConfig {
    pub profiles: Vec<ProfileEntry>,
    pub default_profile: Option<String>,
    #[serde(default)]
    pub ask_on_startup: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProfileEntry {
    pub name: String,
    pub path: PathBuf,
}

// ── File-system helpers ──────────────────────────────────────────────────────

/// Returns the platform-specific path for `profiles.toml`.
///
/// - Linux:   `~/.config/odd-box/profiles.toml`
/// - macOS:   `~/Library/Application Support/odd-box/profiles.toml`
/// - Windows: `%APPDATA%\odd-box\profiles.toml`
pub fn profiles_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("odd-box").join("profiles.toml"))
}

/// Load `profiles.toml` from disk.  Returns an empty default on any error.
pub fn load_profiles() -> ProfilesConfig {
    let Some(path) = profiles_file_path() else {
        return ProfilesConfig::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return ProfilesConfig::default();
    };
    toml::from_str(&text).unwrap_or_default()
}

/// Persist `ProfilesConfig` to `profiles.toml`, creating the directory if needed.
pub fn save_profiles(cfg: &ProfilesConfig) -> Result<()> {
    let path = profiles_file_path()
        .ok_or_else(|| anyhow::anyhow!("cannot determine platform config directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, toml::to_string_pretty(cfg)?)?;
    Ok(())
}

// ── Name derivation ──────────────────────────────────────────────────────────

/// Derive a human-readable profile name from a config file path.
///
/// Strategy:
/// 1. If the file stem is a generic name (`odd-box`, `oddbox`, `config`),
///    use the **parent directory name** instead.
/// 2. Otherwise use the file stem.
/// 3. Append a numeric suffix (`-2`, `-3`, …) if the result collides with
///    an existing profile name.
pub fn derive_profile_name(path: &Path, existing: &[ProfileEntry]) -> String {
    const GENERIC_STEMS: &[&str] = &["odd-box", "oddbox", "config", "odd_box"];

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("profile");

    let candidate = if GENERIC_STEMS.contains(&stem) {
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(stem)
    } else {
        stem
    };

    // Deduplicate against existing names.
    if !existing.iter().any(|e| e.name == candidate) {
        return candidate.to_string();
    }
    let mut suffix = 2u32;
    loop {
        let with_suffix = format!("{candidate}-{suffix}");
        if !existing.iter().any(|e| e.name == with_suffix) {
            return with_suffix;
        }
        suffix += 1;
    }
}

/// Returns `true` if `path` (resolved to an absolute path) is already
/// registered in `profiles`.
pub fn path_already_registered(path: &Path, profiles: &[ProfileEntry]) -> bool {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    profiles.iter().any(|e| {
        let entry_canonical = std::fs::canonicalize(&e.path).unwrap_or_else(|_| e.path.clone());
        entry_canonical == canonical
    })
}
