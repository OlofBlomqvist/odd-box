
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = Some("ODD-BOX MAIN REPOSITORY: https://github.com/OlofBlomqvist/odd-box"))]
pub struct Args {

    /// Path to your configuration file. By default we will look for odd-box.toml and Config.toml.
    #[arg(index = 1)]
    pub configuration: Option<String>,

    #[arg(long,default_value="true")]
    pub tui: Option<bool>,

    /// Updates odd-box to the latest release from github.
    #[arg(long)]
    pub update: bool,
    
    /// Create a bare minimum example configuration file. 
    #[arg(long)]
    pub init: bool,

    #[arg(long)]
    pub config_schema: bool
}

