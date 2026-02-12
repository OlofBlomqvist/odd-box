use anyhow::bail;
use self_update::cargo_crate_version;
use serde::Deserialize;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallSource {
    Homebrew,
    Nix,
    Snap,
    Cargo,
    SystemPackageManager,
    Manual,
}

impl InstallSource {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Homebrew => "homebrew",
            Self::Nix => "nix",
            Self::Snap => "snap",
            Self::Cargo => "cargo",
            Self::SystemPackageManager => "system package manager",
            Self::Manual => "manual",
        }
    }

    fn is_package_managed(&self) -> bool {
        !matches!(self, Self::Manual)
    }

    fn update_hint(&self) -> &'static str {
        match self {
            Self::Homebrew => "brew upgrade odd-box",
            Self::Nix => "nix profile upgrade (or your nix flake/channel workflow)",
            Self::Snap => "snap refresh odd-box",
            Self::Cargo => "cargo install odd-box --force",
            Self::SystemPackageManager => {
                "use your package manager (for example apt/dnf/pacman) to upgrade odd-box"
            }
            Self::Manual => "odd-box --update",
        }
    }
}

pub struct InstallSourceInfo {
    pub source: &'static str,
    pub update_hint: &'static str,
    pub resolved_path: Option<String>,
    pub package_managed: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct Release {
    #[allow(dead_code)]
    html_url: Option<String>,
    tag_name: Option<String>,
}

fn update_from_github(target_tag: &str, current_version: &str) -> anyhow::Result<()> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("OlofBlomqvist")
        .repo_name("odd-box")
        .bin_name("odd-box")
        .show_download_progress(true)
        .target_version_tag(target_tag)
        .current_version(current_version)
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    Ok(())
}

fn parse_install_source_override(value: &str) -> Option<InstallSource> {
    match value.trim().to_lowercase().as_str() {
        "homebrew" | "brew" => Some(InstallSource::Homebrew),
        "nix" => Some(InstallSource::Nix),
        "snap" => Some(InstallSource::Snap),
        "cargo" => Some(InstallSource::Cargo),
        "system" | "package-manager" | "package_manager" | "pkg" | "apt" | "dnf" | "pacman" => {
            Some(InstallSource::SystemPackageManager)
        }
        "manual" => Some(InstallSource::Manual),
        _ => None,
    }
}

fn looks_like_cargo_bin(path: &Path) -> bool {
    let Some(home_dir) = dirs::home_dir() else {
        return false;
    };
    let cargo_bin = home_dir.join(".cargo").join("bin");
    path.starts_with(&cargo_bin)
}

fn classify_install_source(path: &Path) -> InstallSource {
    let path_text = path.to_string_lossy().to_lowercase();

    if path_text.contains("/cellar/odd-box/") || path_text.contains("/caskroom/odd-box/") {
        return InstallSource::Homebrew;
    }

    if path_text.starts_with("/nix/store/") {
        return InstallSource::Nix;
    }

    if path_text.starts_with("/snap/") || path_text.contains("/var/lib/snapd/snap/") {
        return InstallSource::Snap;
    }

    if looks_like_cargo_bin(path) {
        return InstallSource::Cargo;
    }

    if path_text.starts_with("/usr/bin/")
        || path_text.starts_with("/usr/sbin/")
        || path_text.starts_with("/opt/local/bin/")
    {
        return InstallSource::SystemPackageManager;
    }

    InstallSource::Manual
}

fn detect_install_source() -> (InstallSource, Option<PathBuf>) {
    if let Ok(override_value) = std::env::var("ODDBOX_INSTALL_SOURCE") {
        if let Some(source) = parse_install_source_override(&override_value) {
            return (source, None);
        }
    }

    let exe_path = std::env::current_exe().ok();
    let Some(path) = exe_path else {
        return (InstallSource::Manual, None);
    };

    // Resolve symlinks so brew-style wrappers in /usr/local/bin classify correctly.
    let resolved = std::fs::canonicalize(&path).unwrap_or(path);
    let source = classify_install_source(&resolved);
    (source, Some(resolved))
}

pub fn install_source_info() -> InstallSourceInfo {
    let (source, path) = detect_install_source();
    InstallSourceInfo {
        source: source.as_str(),
        update_hint: source.update_hint(),
        resolved_path: path.map(|p| p.display().to_string()),
        package_managed: source.is_package_managed(),
    }
}

pub async fn update() -> anyhow::Result<()> {
    let source_info = install_source_info();
    if source_info.package_managed {
        let install_path_hint = source_info
            .resolved_path
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unknown path".to_string());
        bail!(
            "Self-update is disabled for {} installs (detected binary: {}). Use `{}`.",
            source_info.source,
            install_path_hint,
            source_info.update_hint
        );
    }

    let current_version = current_version();
    let latest_tag = find_latest_version(false).await?;
    if format!("v{current_version}") == latest_tag {
        println!("already running latest version: {latest_tag}");
        return Ok(());
    }

    update_from_github(&latest_tag, &current_version)
}

pub fn current_version() -> &'static str {
    cargo_crate_version!()
}

/// returns Some(newer_version) or None if current is latest.
// pub async fn current_is_latest() -> anyhow::Result<Option<String>> {
//     let current_version = current_version();
//     match find_latest_version(false).await {
//         Ok(v) if current_version != v => Ok(Some(v)),
//         Ok(_) => Ok(None),
//         Err(e) => Err(e)
//     }
// }

pub async fn find_latest_version(include_pre: bool) -> anyhow::Result<String> {
    let allow_preview = include_pre
        || std::env::vars()
            .find(|(key, _)| key == "ODDBOX_ALLOW_PREVIEW")
            .map(|x| x.1.to_lowercase())
            .unwrap_or_default()
            .eq_ignore_ascii_case("true");

    let releases_url = "https://api.github.com/repos/OlofBlomqvist/odd-box/releases";
    let c = reqwest::Client::new();
    let latest_release_tag: Option<String> = c
        .get(releases_url)
        .header("user-agent", "odd-box")
        .send()
        .await?
        .json::<Vec<Release>>()
        .await?
        .iter()
        .filter(|x| {
            if let Some(t) = &x.tag_name {
                allow_preview
                    || t.to_lowercase().contains("-preview") == false
                        && t.to_lowercase().contains("-alpha") == false
                        && t.to_lowercase().contains("-beta") == false
                        && t.to_lowercase().contains("-rc") == false
            } else {
                false
            }
        })
        .next()
        .map(|x| x.tag_name.clone())
        .unwrap_or(None);

    if let Some(v) = latest_release_tag {
        Ok(v.clone())
    } else {
        bail!("Failed to find latest release version")
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallSource, classify_install_source};
    use std::path::Path;

    #[test]
    fn classifies_homebrew_cellar_path() {
        let source = classify_install_source(Path::new(
            "/opt/homebrew/Cellar/odd-box/0.1.13/bin/odd-box",
        ));
        assert_eq!(source, InstallSource::Homebrew);
    }

    #[test]
    fn classifies_nix_store_path() {
        let source = classify_install_source(Path::new(
            "/nix/store/abc123-odd-box-0.1.13/bin/odd-box",
        ));
        assert_eq!(source, InstallSource::Nix);
    }

    #[test]
    fn classifies_manual_install_path() {
        let source = classify_install_source(Path::new("/usr/local/bin/odd-box"));
        assert_eq!(source, InstallSource::Manual);
    }
}
