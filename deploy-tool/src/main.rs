use std::convert::Into;
use std::error::Error;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;
use byteorder::{BigEndian, WriteBytesExt};

macro_rules! println {
    ($($arg:tt)*) => {
        std::println!("\x1b[35m{}\x1b[0m", format_args!($($arg)*));
    };
}

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
    /// Initial deployment of the webapp
    InitialWebDeploy {},
    /// Get web contract id
    GetWebContractId {}
}

fn execute(cmd: &mut Command) -> Result<(), Box<dyn Error>> {
    println!("Running: {:?}", cmd);
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("Command {:?} failed with status: {}", cmd, status).into());
    }
    Ok(())
}

fn cargo_bin_or_path(cmd: &str, pkg: &str) -> String {
    if let Ok(path) = std::env::var("PATH") {
        for p in std::env::split_paths(&path) {
            let bin_path = p.join(cmd);
            if bin_path.exists() {
                return bin_path.to_str().unwrap().into();
            }
        }
    }
    let mut home = dirs::home_dir().expect("Could not find home directory");
    home.push(".cargo");
    home.push("bin");
    home.push(cmd);
    if !home.exists() {
        println!("fdev not found, installing...");
        execute(Command::new("cargo").args(["install", pkg])).unwrap();
    }
    home.to_string_lossy().to_string()
}

fn cargo_build(package: &str) -> Result<(), Box<dyn Error>> {
    println!("Building package: {}", package);
    let args = vec!["build", "--release", "--target", "wasm32-unknown-unknown", "--package", package];
    execute(Command::new("cargo").args(args))
}

fn web_container_sign(
    input: PathBuf,
    output: PathBuf,
    parameters: PathBuf,
    version: u32,
) -> Result<(), Box<dyn Error>> {
    println!("Signing web container: {} -> {}", input.display(), output.display());
    execute(Command::new("cargo")
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
        ]))
}

fn web_container_generate() -> Result<(), Box<dyn Error>> {
    println!("Generating web container keys...");
    execute(Command::new("cargo")
        .args([
            "run",
            "--bin",
            "web-container-tool",
            "--",
            "generate",
        ]))
}

fn fdev_publish(
    contract_wasm: PathBuf,
    webapp_parameters: PathBuf,
    state: PathBuf,
) -> Result<(), Box<dyn Error>> {
    println!("Publishing contract...");
    execute(Command::new(cargo_bin_or_path("fdev", "fdev"))
        .args([
            "publish",
            "--code",
            &contract_wasm.to_string_lossy(),
            "--parameters",
            &webapp_parameters.to_string_lossy(),
            "contract",
            "--state",
            &state.to_string_lossy(),
        ]))
}

fn fdev_get_contract_id(
    contract_wasm: PathBuf,
    webapp_parameters: PathBuf,
) -> Result<(), Box<dyn Error>> {
    println!("Contract ID:");
    execute(Command::new(cargo_bin_or_path("fdev", "fdev"))
        .args([
            "get-contract-id",
            "--code",
            &contract_wasm.to_string_lossy(),
            "--parameters",
            &webapp_parameters.to_string_lossy(),
        ]))
}

fn dev() -> Result<(), Box<dyn Error>> {
    Ok(())
}

fn deploy(version: u32) -> Result<(), Box<dyn Error>> {
    cargo_build("pizza-ui")?;

    let contract_wasm = default_storage_path("web.contract.wasm");
    let webapp_archive = default_storage_path("webapp.bootstrap.tar.xz");
    // Create an empty .tar.xz archive
    let file = std::fs::File::create(&webapp_archive)?;
    let enc = xz2::write::XzEncoder::new(file, 6);
    let mut tar = tar::Builder::new(enc);
    let mut header = tar::Header::new_gnu();
    let content = b"<tt>wip</tt>";
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    tar.append_data(&mut header, "index.html", &content[..])?;
    tar.finish()?;
    let webapp_metadata = default_storage_path("webapp.metadata");
    let webapp_parameters = default_storage_path("webapp.parameters");

    web_container_sign(
        webapp_archive.clone(),
        webapp_metadata.clone(),
        webapp_parameters.clone(),
        version
    )?;

    let state_path = default_storage_path("webapp.state");
    merge_state(&webapp_metadata, &webapp_archive, &state_path)?;

    fdev_publish(
        contract_wasm,
        webapp_parameters,
        state_path,
    )?;

    Ok(())
}

fn initial_web_deploy() -> Result<(), Box<dyn Error>> {
    println!("Performing initial web deployment...");

    // NOTE: this means all folders needed already exist after this
    let keys_path = default_storage_path("web-container-keys.toml");
    if !keys_path.exists() {
        web_container_generate()?;
    }

    cargo_build("web-container-contract")?;

    let contract_wasm_src = PathBuf::from("target/wasm32-unknown-unknown/release/web_container_contract.wasm");
    let contract_wasm = default_storage_path("web.contract.wasm");
    std::fs::copy(contract_wasm_src, &contract_wasm)?;
    let webapp_archive = default_storage_path("webapp.bootstrap.tar.xz");
    // Create an empty .tar.xz archive
    let file = std::fs::File::create(&webapp_archive)?;
    let enc = xz2::write::XzEncoder::new(file, 6);
    let mut tar = tar::Builder::new(enc);
    let mut header = tar::Header::new_gnu();
    let content = b"<tt>wip</tt>";
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    tar.append_data(&mut header, "index.html", &content[..])?;
    tar.finish()?;
    let webapp_metadata = default_storage_path("webapp.metadata");
    let webapp_parameters = default_storage_path("webapp.parameters");

    web_container_sign(
        webapp_archive.clone(),
        webapp_metadata.clone(),
        webapp_parameters.clone(),
        1
    )?;

    let state_path = default_storage_path("webapp.state");
    merge_state(&webapp_metadata, &webapp_archive, &state_path)?;

    fdev_publish(
        contract_wasm,
        webapp_parameters,
        state_path,
    )?;

    Ok(())
}

fn get_web_contract_id() -> Result<(), Box<dyn Error>> {
    let contract_wasm = default_storage_path("web.contract.wasm");
    let webapp_parameters = default_storage_path("webapp.parameters");

    fdev_get_contract_id(
        contract_wasm,
        webapp_parameters,
    )?;

    Ok(())
}

fn merge_state(
    metadata_path: &PathBuf,
    webapp_archive_path: &PathBuf,
    output_state_path: &PathBuf,
) -> Result<(), Box<dyn Error>> {
    let metadata_bytes = std::fs::read(metadata_path)?;
    let webapp_bytes = std::fs::read(webapp_archive_path)?;
    let mut state = Vec::new();
    state.write_u64::<BigEndian>(metadata_bytes.len() as u64)?;
    state.extend_from_slice(&metadata_bytes);
    state.write_u64::<BigEndian>(webapp_bytes.len() as u64)?;
    state.extend_from_slice(&webapp_bytes);
    std::fs::write(output_state_path, state)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Deploy { version } => deploy(version),
        Commands::Dev {} => dev(),
        Commands::InitialWebDeploy {} => initial_web_deploy(),
        Commands::GetWebContractId {} => get_web_contract_id(),
    }
}
