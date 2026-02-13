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
        #[arg(long, short, default_value = "0")]
        version: u32,
    },
    /// Launch application in development mode
    Dev {},
    /// Initial deployment of the webapp
    InitialWebDeploy {},
}

fn cargo_build(package: &str) -> Result<(), Box<dyn Error>> {
    println!("Building package: {}", package);
    let args = vec!["build", "--release", "--target", "wasm32-unknown-unknown", "--package", package];
    let status = Command::new("cargo")
        .args(args)
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

fn web_container_generate() -> Result<(), Box<dyn Error>> {
    println!("Generating web container keys...");
    let status = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "web-container-tool",
            "--",
            "generate",
        ])
        .status()?;

    if !status.success() {
        return Err("web-container-tool generate failed".into());
    }

    Ok(())
}

fn fdev_publish(
    contract_wasm: PathBuf,
    webapp_parameters: PathBuf,
    webapp_archive: PathBuf,
    webapp_metadata: PathBuf,
) -> Result<(), Box<dyn Error>> {
    println!("Publishing contract...");
    let status = Command::new("fdev")
        .args([
            "publish",
            "--code",
            &contract_wasm.to_string_lossy(),
            "--parameters",
            &webapp_parameters.to_string_lossy(),
            "contract",
            "--webapp-archive",
            &webapp_archive.to_string_lossy(),
            "--webapp-metadata",
            &webapp_metadata.to_string_lossy(),
        ])
        .status()?;

    if !status.success() {
        return Err("fdev publish failed".into());
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

fn initial_web_deploy() -> Result<(), Box<dyn Error>> {
    println!("Performing initial web deployment...");

    let keys_path = default_storage_path("web-container-keys.toml");
    if !keys_path.exists() {
        web_container_generate()?;
    }

    cargo_build("web-container-contract")?;

    let contract_wasm = PathBuf::from("target/wasm32-unknown-unknown/release/web_container_contract.wasm");
    let webapp_archive = default_storage_path("webapp.bootstrap.tar.xz");
    if let Some(parent) = webapp_archive.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Create an empty .tar.xz archive
    let file = std::fs::File::create(&webapp_archive)?;
    let enc = xz2::write::XzEncoder::new(file, 6);
    let mut tar = tar::Builder::new(enc);
    tar.finish()?;
    let webapp_metadata = default_storage_path("webapp.metadata");
    let webapp_parameters = default_storage_path("webapp.parameters");

    web_container_sign(
        webapp_archive.clone(),
        webapp_metadata.clone(),
        webapp_parameters.clone(),
        0
    )?;

    fdev_publish(
        contract_wasm,
        webapp_parameters,
        webapp_archive,
        webapp_metadata,
    )?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Deploy { version } => deploy(version),
        Commands::Dev {} => dev(),
        Commands::InitialWebDeploy {} => initial_web_deploy(),
    }
}
