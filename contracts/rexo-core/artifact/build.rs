// Copyright (c) Subzero Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

fn main() {
    println!("cargo:rerun-if-changed=../src");
    println!("cargo:rerun-if-changed=../wit");
    println!("cargo:rerun-if-changed=../Cargo.toml");
    println!("cargo:rerun-if-changed=../Cargo.lock");

    println!("cargo:warning=Starting PolkaVM artifact build for program_path ..");

    // Build the Rialo VM (RISC-V/PolkaVM) program
    match rialo_build_lib::build_script::setup_polkavm_artifact_build()
        .program_path("..")
        .run()
    {
        Ok(_) => {
            println!("cargo:warning=PolkaVM artifact build finished successfully");
        }
        Err(e) => {
            eprintln!("==================================================");
            eprintln!("POLKAVM BUILD ERROR DETAILS:\n{:#?}", e);
            eprintln!("==================================================");
            panic!("PolkaVM artifact build failed: {:#?}", e);
        }
    }

    // Compile rex WASM components (if any rex blocks are present)
    if let Err(e) = rialo_venus_build_helper::compile_rex_components("..") {
        println!("cargo:warning=Rex component compilation notice: {:?}", e);
    }
}


