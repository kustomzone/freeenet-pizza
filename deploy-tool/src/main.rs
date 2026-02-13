use std::convert::Into;
use std::error::Error;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;

pub const PROJECT: &str = "pizza-freenet";

fn default_storage_path(file: &str) -> PathBuf {
    let mut p = dirs::config_dir().expect("Could not find config directory");
    p.push(PROJECT);
    p.push(file);
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
    Deploy {
        /// Version number
        #[arg(long, short, default_value = "1")]
        version: u32,
    },
    /// Launch application in development mode
    Dev {},
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

fn web_container_sign(
    input: PathBuf,
    output: PathBuf,
    parameters: PathBuf,
    version: u32,
) -> Result<(), Box<dyn Error>> {
    println!("Signing web container: {} -> {}", input.display(), output.display());
    let status = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "web-container-tool",
            "--",
            "sign",
            "--input",
            &input.to_string_lossy(),
            "--output",
            &output.to_string_lossy(),
            "--parameters",
            &parameters.to_string_lossy(),
            "--version",
            &version.to_string(),
        ])
        .status()?;

    if !status.success() {
        return Err("web-container-tool sign failed".into());
    }

    Ok(())
}

fn dev() -> Result<(), Box<dyn Error>> {
    Ok(())
}

fn deploy(version: u32) -> Result<(), Box<dyn Error>> {
    cargo_build("pizza-contract")?;
    cargo_build("pizza-ui")?;

    let input = PathBuf::from("webapp-bootstrap.tar.xz");
    let output = default_storage_path("webapp.metadata");
    web_container_sign(input, output, default_storage_path("webapp.parameters"), version)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Deploy { version } => deploy(version),
        Commands::Dev {} => dev(),
    }
}
