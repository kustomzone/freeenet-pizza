//! Build script for pizza-ui.
//!
//! This script compiles the pizza-contract and pizza-delegate to WASM and places
//! them in locations where the UI can include them at compile time.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Tell Cargo to rerun this build script if the contract or delegate source changes
    println!("cargo:rerun-if-changed=../../contracts/pizza-contract/");
    println!("cargo:rerun-if-changed=../../delegates/pizza-delegate/");
    println!("cargo:rerun-if-changed=../../common/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let project_root = PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // Build contract
    build_wasm(
        &project_root,
        "pizza-contract",
        "contracts/pizza-contract",
        "pizza_contract.wasm",
    );

    // Build delegate
    build_wasm(
        &project_root,
        "pizza-delegate",
        "delegates/pizza-delegate",
        "pizza_delegate.wasm",
    );
}

fn build_wasm(project_root: &PathBuf, package: &str, subdir: &str, wasm_name: &str) {
    let target_dir = project_root.join(subdir);
    let build_dir = target_dir.join("build");

    // Create build directory if it doesn't exist
    fs::create_dir_all(&build_dir).expect(&format!("Failed to create build directory for {}", package));

    // Build the WASM using cargo
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            package,
        ])
        .current_dir(project_root)
        .status();

    match status {
        Ok(s) if s.success() => {
            // Copy the WASM file to the build directory
            let wasm_src = project_root
                .join("target/wasm32-unknown-unknown/release")
                .join(wasm_name);
            let wasm_dst = build_dir.join(wasm_name);
            println!("cargo:warning={} built at {:?}", package, wasm_src);

            if wasm_src.exists() {
                fs::copy(&wasm_src, &wasm_dst).expect(&format!("Failed to copy {} WASM file", package));
                println!(
                    "cargo:warning={} WASM built successfully at {:?}",
                    package, wasm_dst
                );
            } else {
                panic!(
                    "WASM file not found at {:?} after successful build. This should not happen.",
                    wasm_src
                );
            }
        }
        Ok(_) => {
            panic!(
                "{} build failed. Please fix the errors and try again.",
                package
            );
        }
        Err(e) => {
            panic!(
                "Could not run cargo for {} build: {}. Make sure cargo is installed and in PATH.",
                package, e
            );
        }
    }
}
