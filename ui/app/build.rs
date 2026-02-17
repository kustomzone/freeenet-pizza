//! Build script for pizza-ui.
//!
//! This script compiles the pizza-contract to WASM and places it in a location
//! where the UI can include it at compile time.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Tell Cargo to rerun this build script if the contract source changes
    println!("cargo:rerun-if-changed=../../contracts/pizza-contract/");
    println!("cargo:rerun-if-changed=../../common/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let project_root = PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let contract_dir = project_root.join("contracts/pizza-contract");
    let build_dir = contract_dir.join("build");

    // Create build directory if it doesn't exist
    fs::create_dir_all(&build_dir).expect("Failed to create build directory");

    // Build the contract WASM using cargo
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "pizza-contract",
        ])
        .current_dir(&project_root)
        .status();

    match status {
        Ok(s) if s.success() => {
            // Copy the WASM file to the build directory
            let wasm_src =
                project_root.join("target/wasm32-unknown-unknown/release/pizza_contract.wasm");
            let wasm_dst = build_dir.join("pizza_contract.wasm");
            println!("cargo:warning=Contract built at {:?}", wasm_src);

            if wasm_src.exists() {
                fs::copy(&wasm_src, &wasm_dst).expect("Failed to copy WASM file");
                println!(
                    "cargo:warning=Contract WASM built successfully at {:?}",
                    wasm_dst
                );
            } else {
                // Create a placeholder file for initial compilation
                create_placeholder_wasm(&wasm_dst);
            }
        }
        Ok(_) => {
            eprintln!("cargo:warning=Contract build failed, using placeholder WASM");
            create_placeholder_wasm(&build_dir.join("pizza_contract.wasm"));
        }
        Err(e) => {
            eprintln!(
                "cargo:warning=Could not run cargo for contract build: {}",
                e
            );
            create_placeholder_wasm(&build_dir.join("pizza_contract.wasm"));
        }
    }
}

fn create_placeholder_wasm(path: &PathBuf) {
    // Create a minimal valid WASM file (magic bytes + version)
    // This is just for compilation to succeed; actual WASM is needed at runtime
    let minimal_wasm = [
        0x00, 0x61, 0x73, 0x6D, // \0asm - WASM magic
        0x01, 0x00, 0x00, 0x00, // version 1
    ];
    fs::write(path, &minimal_wasm).expect("Failed to create placeholder WASM");
    println!("cargo:warning=Created placeholder WASM at {:?}", path);
}
