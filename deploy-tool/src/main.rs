use std::convert::Into;
use std::error::Error;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;

pub const PROJECT: &str = "pizza-freenet";

fn default_storage_path() -> PathBuf {
    let mut p = dirs::config_dir().expect("Could not find config directory");
    p.push(PROJECT);
    p
}


#[derive(Parser)]
#[command(name = "deploy-tool")]
#[command(about = "Deployment tool for freenet pizza app")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deploy the application
    Deploy { },
    /// Launch application in development mode
    Dev { },
}

fn cargo_build(package: &str) -> Result<(), Box<dyn Error>> {
    println!("Building package: {}", package);
    let status = Command::new("cargo")
        .args(["build", "--release", "--package", package])
        .status()?;

    if !status.success() {
        return Err(format!("cargo build failed for package {}", package).into());
    }

    Ok(())
}

fn dev() -> Result<(), Box<dyn Error>> {
    Ok(())
}

fn deploy() -> Result<(), Box<dyn Error>> {
    cargo_build("pizza-contract");
    cargo_build("pizza-ui");

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Deploy { } => deploy(),
        Commands::Dev { } => dev(),
    }
}
