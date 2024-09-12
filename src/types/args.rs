
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = Some("ODD-BOX MAIN REPOSITORY: https://github.com/OlofBlomqvist/odd-box"))]
pub struct Args {

    /// Path to your configuration file. By default we will look for odd-box.toml and Config.toml.
    #[arg(index = 1)]
    pub configuration: Option<String>,

    /// Port to listen on. Overrides configuration port. Defaults to 8080
    #[arg(long,short)]
    pub port: Option<u16>,

    /// Port to listen on for using https. Overrides configuration port. Defaults to 4343
    #[arg(long,short)]
    pub tls_port: Option<u16>,

    #[arg(long,default_value="true")]
    pub tui: Option<bool>,

    /// Updates odd-box to the latest release from github.
    #[arg(long)]
    pub update: bool,

    /// Creates a configuration file with default values.
    #[arg(long)]
    pub generate_example_cfg : bool,

    /// Create a bare minimum example configuration file. 
    #[arg(long)]
    pub init: bool,

    /// Upgrade configuration file to latest version.
    #[arg(long)]
    pub upgrade_config: bool,

    #[arg(long)]
    pub config_schema: bool
}

