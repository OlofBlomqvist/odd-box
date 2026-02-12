use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = Some("ODD-BOX MAIN REPOSITORY: https://github.com/OlofBlomqvist/odd-box"))]
pub struct Args {
    /// Path to your configuration file. By default we look for odd-box.yaml, odd-box.toml, or Config.toml.
    #[arg(index = 1)]
    pub configuration: Option<String>,

    /// Launch with TUI (default: true). Can be passed as `--tui` or `--tui=false`.
    #[arg(
        long,
        default_value_t = true,
        num_args = 0..=1,
        default_missing_value = "true"
    )]
    pub tui: bool,

    /// Launch with graphical user interface
    #[arg(long)]
    pub gui: bool,

    /// Force a specific theme for the GUI (light, dark, or system)
    #[arg(long, value_parser = ["light", "dark", "system"])]
    pub theme: Option<String>,

    /// Updates odd-box to the latest release from github.
    #[arg(long)]
    pub update: bool,

    /// Print detected installation source (homebrew/nix/snap/cargo/system/manual) and exit.
    #[arg(long)]
    pub install_source: bool,

    /// Create a bare minimum example configuration file (odd-box.yaml).
    #[arg(long)]
    pub init: bool,

    #[arg(long)]
    pub config_schema: bool,
}
